use crate::bitset::DenseBitSet;
use crate::building::BuildingGrid;
use crate::diplomacy::is_traitor_active;
use crate::engine::SowEngine;
use crate::execution::{PrioritizedTile, RETREAT_PENALTY_VS_PLAYER};
use crate::game::{GameEvent, GamePhase, GameState, TerritoryChange};
use crate::game_config::max_tiles_cap_for_troops;
use crate::geo_entities::{Ability, LossWhen, Side, Source, Target};
use crate::map::TerrainType;
use crate::rng::{NextIntExt, stable_tile_sample};
use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};

const CAPTURE_COST_VARIATION_SALT: u64 = 0xA076_1D64_78BD_642F;
const CAPTURE_COST_VARIATION: f64 = 0.10;
const NEUTRAL_CAPTURE_SPEED_MULTIPLIER: f64 = 3.0;

struct BorderCluster {
    tiles: Vec<u32>,
    bounds: [u32; 4],
}

#[inline]
fn terrain_capture_speed(terrain: TerrainType) -> f64 {
    match terrain {
        TerrainType::Land => 16.5,
        TerrainType::Highland => 20.0,
        TerrainType::Mountain => 25.0,
        _ => 16.5,
    }
}

#[inline]
fn terrain_loss_multiplier(terrain: TerrainType, config: &crate::game_config::GameConfig) -> f64 {
    match terrain {
        TerrainType::Land => 1.0,
        TerrainType::Highland => config.terrain_multiplier_highland,
        TerrainType::Mountain => config.terrain_multiplier_mountain,
        _ => 1.0,
    }
}

fn occupied_land_share(state: &GameState) -> f64 {
    if state.total_land_tiles == 0 {
        return 0.0;
    }

    let occupied_tiles: u64 = state
        .players
        .iter()
        .map(|player| u64::from(player.tile_count))
        .sum();
    (occupied_tiles as f64 / f64::from(state.total_land_tiles)).clamp(0.0, 1.0)
}

fn neutral_expansion_pacing_multiplier(occupied_share: f64) -> f64 {
    let occupied_share = if occupied_share.is_finite() {
        occupied_share.clamp(0.0, 1.0)
    } else {
        0.0
    };

    if occupied_share <= 0.10 {
        0.70
    } else if occupied_share <= 0.50 {
        0.70 + (occupied_share - 0.10) / (0.50 - 0.10) * (1.00 - 0.70)
    } else if occupied_share < 0.90 {
        1.00 + (occupied_share - 0.50) / (0.90 - 0.50) * (1.15 - 1.00)
    } else {
        1.15
    }
}

fn attack_pacing_multiplier(
    target_owner: u16,
    neutral_pacing: f64,
    neutral_capture_speed_multiplier: f64,
) -> f64 {
    if target_owner == 0 {
        neutral_pacing * neutral_capture_speed_multiplier
    } else {
        1.0
    }
}

/// Whether an army is empowered by an outnumbered or return-campaign situation.
/// Shared by the strength formula and the player snapshot, so the visual and the
/// combat can never disagree.
/// `army` is the troops this attack sends, so an attack that is itself outnumbered
/// is judged by its own numbers, not by what the attacker keeps at home.
pub(crate) fn empowered_attack_applies(
    state: &GameState,
    attacker: u16,
    target: u16,
    army: f64,
) -> bool {
    let Some(p) = state.player(attacker) else {
        return false;
    };
    let outnumbered = matches!(
        p.leader.ability(),
        Ability::AttackStrength {
            against: Target::Outnumbering,
            ..
        }
    ) && target != 0
        && state
            .player(target)
            .is_some_and(|rival| rival.troops > army);
    outnumbered || return_campaign_multiplier(state, attacker, target) > 1.0
}

/// Whether `target` describes `subject` (0 = neutral land) for an action by `actor`
/// that sends `army` troops. `actor` is the hero's player: the attacker for attack
/// strength and conquest, and the defender whose land a retreat leaves for the
/// retreat penalty.
pub(crate) fn target_applies(
    state: &GameState,
    actor: u16,
    subject: u16,
    army: f64,
    target: Target,
) -> bool {
    use crate::player::{Player, PlayerType};
    let owner = state.player(subject);
    let compared = |pick: fn(&Player) -> f64, more: bool| match (owner, state.player(actor)) {
        (Some(theirs), Some(mine)) if subject != 0 => {
            if more {
                pick(theirs) > pick(mine)
            } else {
                pick(theirs) < pick(mine)
            }
        }
        _ => false,
    };
    // Whether the subject is shown as the same people as the hero's player (None without both).
    let same_people = || match (owner, state.player(actor)) {
        (Some(theirs), Some(mine)) if subject != 0 => Some(theirs.realm() == mine.realm()),
        _ => None,
    };
    match target {
        Target::Anyone => true,
        Target::Neutral => subject == 0,
        Target::OtherPlayer => subject != 0,
        Target::Human => owner.is_some_and(|t| t.player_type == PlayerType::Human),
        Target::Tribe => owner.is_some_and(|t| t.player_type == PlayerType::Bot),
        Target::Nation => owner.is_some_and(|t| t.player_type == PlayerType::Nation),
        Target::ActiveTraitor => owner.is_some_and(|t| is_traitor_active(t, state.tick as u32)),
        Target::Outnumbering => subject != 0 && owner.is_some_and(|rival| rival.troops > army),
        Target::Outnumbered => subject != 0 && owner.is_some_and(|rival| rival.troops < army),
        Target::Larger => compared(|p| f64::from(p.tile_count), true),
        Target::Smaller => compared(|p| f64::from(p.tile_count), false),
        Target::Richer => compared(|p| p.gold, true),
        Target::Poorer => compared(|p| p.gold, false),
        Target::Kin => subject != actor && same_people() == Some(true),
        Target::Foreign => same_people() == Some(false),
    }
}

/// The product of `multiplier` over the targets that describe `subject`. A hero has
/// one targeted ability, so only that ability's target changes the product.
pub(crate) fn targeted_multiplier(
    state: &GameState,
    actor: u16,
    subject: u16,
    army: f64,
    multiplier: impl Fn(Target) -> f64,
) -> f64 {
    Target::ALL
        .into_iter()
        .filter(|&target| target_applies(state, actor, subject, army, target))
        .map(multiplier)
        .product()
}

/// Shows an attack's hero ability when it applies to this attack's target. Called
/// once when the attack launches, so the banner marks the attack, not each tile.
pub(crate) fn announce_attack_abilities(
    state: &mut GameState,
    attacker: u16,
    target: u16,
    army: f64,
    tile: Option<(u32, u32)>,
) {
    let Some(ability) = state.player(attacker).map(|p| p.leader.ability()) else {
        return;
    };
    let applies = match ability {
        Ability::AttackStrength { against, .. } | Ability::ConquestSpeed { against, .. } => {
            target_applies(state, attacker, target, army, against)
        }
        Ability::ReturnCampaign { .. } => return_campaign_multiplier(state, attacker, target) > 1.0,
        _ => false,
    };
    if applies {
        state.announce_leader_ability_to(attacker, &[target], tile);
    }
}

/// How much each of `attacker`'s troops counts against `target` (0 = neutral land)
/// from hero abilities. The only owner: tile combat and attack clashes both use it.
pub(crate) fn attack_strength(state: &GameState, attacker: u16, target: u16, army: f64) -> f64 {
    let Some(p) = state.player(attacker) else {
        return 1.0;
    };
    targeted_multiplier(state, attacker, target, army, |t| {
        p.leader.attack_strength_multiplier(t)
    }) * return_campaign_multiplier(state, attacker, target)
}

fn return_campaign_multiplier(state: &GameState, attacker: u16, target: u16) -> f64 {
    match state.player(attacker) {
        Some(p)
            if p.return_campaign.is_some_and(|(enemy, until)| {
                target != 0 && enemy == target && state.tick < until
            }) =>
        {
            match p.leader.ability() {
                Ability::ReturnCampaign { percent } => 1.0 + f64::from(percent) / 100.0,
                _ => 1.0,
            }
        }
        _ => 1.0,
    }
}

/// How much harder a defender's land is to conquer because of its hero ability.
pub(crate) fn defense_strength(state: &GameState, attacker: u16, defender: u16, army: f64) -> f64 {
    let Some(player) = state.player(defender) else {
        return 1.0;
    };
    targeted_multiplier(state, defender, attacker, army, |target| {
        player.leader.defense_strength_multiplier(target)
    })
}

/// Hero target loss ability: tiles taken from land its target describes cost `attacker`'s
/// army fewer troops. Judged like `attack_strength`, once per attack tick.
fn target_attacker_loss(state: &GameState, attacker: u16, target: u16, army: f64) -> f64 {
    let Some(p) = state.player(attacker) else {
        return 1.0;
    };
    targeted_multiplier(state, attacker, target, army, |t| {
        p.leader.attacker_loss_multiplier(LossWhen::Against(t))
    })
}

/// Hero coast ability: conquering a land tile next to water costs the attacker fewer troops.
fn coast_attacker_loss(state: &GameState, (tile_x, tile_y): (u32, u32), attacker: u16) -> f64 {
    if !crate::building::placement::is_shore_land_tile(&state.map, tile_x, tile_y) {
        return 1.0;
    }
    state
        .player(attacker)
        .map_or(1.0, |p| p.leader.attacker_loss_multiplier(LossWhen::Coast))
}

/// Hero landing ability: troops that came ashore from ships lose less taking land from there.
fn landing_attacker_loss(state: &GameState, attacker: u16, landed: bool) -> f64 {
    if !landed {
        return 1.0;
    }
    state.player(attacker).map_or(1.0, |p| {
        p.leader.attacker_loss_multiplier(LossWhen::Landing)
    })
}

/// Hero mountain ability: mountain tiles cost the attacker fewer troops.
fn terrain_attacker_loss(state: &GameState, attacker: u16, terrain: TerrainType) -> f64 {
    let when = match terrain {
        TerrainType::Mountain => LossWhen::Mountain,
        TerrainType::Highland => LossWhen::Highland,
        _ => return 1.0,
    };
    state
        .player(attacker)
        .map_or(1.0, |p| p.leader.attacker_loss_multiplier(when))
}

/// Hero city-tile ability: conquering a city tile costs the attacker fewer troops.
/// Takes disjoint fields because the attack loop holds a mutable borrow of `attacks`.
fn city_attacker_loss(
    state: &GameState,
    buildings: &BuildingGrid,
    (tile_x, tile_y): (u32, u32),
    attacker: u16,
) -> f64 {
    let loss = state
        .player(attacker)
        .map_or(1.0, |p| p.leader.attacker_loss_multiplier(LossWhen::City));
    if loss == 1.0 || !buildings.city_covers_tile(tile_x, tile_y) {
        return 1.0;
    }
    loss
}

/// How long (30 s at the standard speed) a recapture discount lasts after a city is taken.
const RECAPTURE_WINDOW_TICKS: u32 = 300;

/// Hero recapture ability: while the window opened by losing a city to `target` is
/// open, `attacker`'s attacks on `target` cost less. The window lives on the player who
/// lost the city, so only that player's leader can ever have a discount.
fn recapture_multiplier(state: &GameState, attacker: u16, target: u16) -> f64 {
    state
        .player(attacker)
        .filter(|p| p.recapture_ticks > 0 && p.recapture_target == target)
        .map_or(1.0, |p| {
            p.leader.attacker_loss_multiplier(LossWhen::RecaptureWindow)
        })
}

/// Hero city abilities: a city tile costs more to take from a defender with `CityDefense`
/// and less for an attacker with `CityAssault`. Takes disjoint fields because the attack
/// loop holds a mutable borrow of `attacks`.
#[inline]
fn city_capture_multiplier(
    state: &GameState,
    buildings: &BuildingGrid,
    (tile_x, tile_y): (u32, u32),
    attacker: u16,
    defender: u16,
    army: f64,
) -> f64 {
    let defense = state.player(defender).map_or(1.0, |p| {
        p.leader.city_capture_multiplier(Side::Defender)
            * targeted_multiplier(state, defender, attacker, army, |target| {
                p.leader.city_defense_multiplier(target)
            })
    });
    let assault = state
        .player(attacker)
        .map_or(1.0, |p| p.leader.city_capture_multiplier(Side::Attacker));
    let ratio = defense / assault;
    if ratio == 1.0 || !buildings.city_covers_tile(tile_x, tile_y) {
        return 1.0;
    }
    ratio
}

// Attack execution tick (territory expansion and combat resolution)
impl SowEngine {
    pub(crate) fn annex_enclosed_components(
        &mut self,
        changes: Vec<TerritoryChange>,
    ) -> Vec<(u16, u16, u32, u32)> {
        // Check each affected owner once, in ID order, using the final capture batch.
        // Neutral captures can close an adjacent owner's last opening without taking its land.
        let mut changed_victims = DenseBitSet::new();
        for change in changes {
            if change.old_owner != 0 {
                changed_victims.insert(u32::from(change.old_owner));
            } else if change.new_owner != 0 {
                let captor = self.state.map.owner_id(change.x, change.y);
                if captor == 0 {
                    continue;
                }
                self.state
                    .map
                    .for_each_neighbor4(change.x, change.y, |x, y| {
                        let owner = self.state.map.owner_id(x, y);
                        if owner != 0 && owner != captor {
                            changed_victims.insert(u32::from(owner));
                        }
                    });
            }
        }

        let mut eliminated = Vec::new();
        for victim_id in changed_victims.ones().map(|id| id as u16) {
            if !self
                .state
                .player(victim_id)
                .is_some_and(|player| player.alive && player.tile_count > 0)
            {
                continue;
            }
            // Bulk annexation must not bypass a campaign's scripted final capture.
            if self
                .campaign_assault
                .as_ref()
                .is_some_and(|assault| assault.hold_last_tile && assault.root_target == victim_id)
                || Self::pre_defeat_watches(
                    &self.campaign_pre_defeat_targets,
                    self.campaign_pre_defeat_pending,
                    victim_id,
                )
            {
                continue;
            }
            let clusters = self.border_clusters(victim_id);
            if clusters.is_empty() {
                continue;
            }

            let primary = self.primary_border_cluster(victim_id, &clusters);
            let mut candidates = Vec::new();
            if clusters.len() == 1 {
                if let Some(captor) = self.surrounded_by_single_enemy(victim_id, &clusters[0]) {
                    candidates.push((0, captor));
                }
            } else {
                let main = &clusters[primary];
                if let Some(captor) = self.surrounded_by_single_enemy(victim_id, main) {
                    candidates.push((primary, captor));
                }
                for (index, cluster) in clusters.iter().enumerate() {
                    if index == primary || !self.secondary_cluster_is_surrounded(victim_id, cluster)
                    {
                        continue;
                    }
                    if let Some(captor) = self.capturing_player(victim_id, cluster) {
                        candidates.push((index, captor));
                    }
                }
            }

            for (index, captor_id) in candidates {
                let cluster = &clusters[index].tiles;
                if captor_id == victim_id
                    || self.annex_players_friendly(captor_id, victim_id)
                    || self
                        .campaign_relations
                        .get(&captor_id)
                        .is_some_and(|relation| {
                            *relation != crate::protocol::CampaignRelation::Enemy
                        })
                    || cluster.iter().any(|&tile| {
                        self.state
                            .map
                            .owner_id(tile % self.state.map.width, tile / self.state.map.width)
                            != victim_id
                    })
                    || !cluster
                        .first()
                        .is_some_and(|&start| self.territory_is_enclosed(victim_id, start))
                {
                    continue;
                }
                eliminated.extend(
                    self.state
                        .capture_enclosed_component(captor_id, victim_id, cluster[0])
                        .into_iter()
                        .map(|(victim, x, y)| (victim, captor_id, x, y)),
                );
            }
        }
        eliminated
    }

    fn border_clusters(&self, player_id: u16) -> Vec<BorderCluster> {
        let Some(player) = self.state.player(player_id) else {
            return Vec::new();
        };
        if self.state.map.width == 0 {
            return Vec::new();
        }

        let mut seen = HashSet::new();
        let mut clusters = Vec::new();
        for start in player.border_tiles.ones() {
            if !seen.insert(start) {
                continue;
            }
            let mut tiles = vec![start];
            let mut cursor = 0;
            let (mut min_x, mut min_y, mut max_x, mut max_y) = (
                start % self.state.map.width,
                start / self.state.map.width,
                start % self.state.map.width,
                start / self.state.map.width,
            );
            while cursor < tiles.len() {
                let tile = tiles[cursor];
                cursor += 1;
                let x = tile % self.state.map.width;
                let y = tile / self.state.map.width;
                self.state.map.for_each_neighbor(x, y, |nx, ny| {
                    let neighbor = ny * self.state.map.width + nx;
                    if player.border_tiles.contains(neighbor) && seen.insert(neighbor) {
                        min_x = min_x.min(nx);
                        min_y = min_y.min(ny);
                        max_x = max_x.max(nx);
                        max_y = max_y.max(ny);
                        tiles.push(neighbor);
                    }
                });
            }
            clusters.push(BorderCluster {
                tiles,
                bounds: [min_x, min_y, max_x, max_y],
            });
        }
        clusters
    }

    fn primary_border_cluster(&self, player_id: u16, clusters: &[BorderCluster]) -> usize {
        let largest = clusters
            .iter()
            .enumerate()
            .max_by_key(|(index, cluster)| (cluster.tiles.len(), std::cmp::Reverse(*index)))
            .map_or(0, |(index, _)| index);
        if !self.border_cluster_is_hole(player_id, largest, clusters) {
            return largest;
        }

        clusters
            .iter()
            .enumerate()
            .filter(|(index, _)| !self.border_cluster_is_hole(player_id, *index, clusters))
            .max_by_key(|(index, cluster)| (cluster.tiles.len(), std::cmp::Reverse(*index)))
            .map_or(largest, |(index, _)| index)
    }

    fn border_cluster_is_hole(
        &self,
        player_id: u16,
        index: usize,
        clusters: &[BorderCluster],
    ) -> bool {
        let cluster = &clusters[index];
        clusters.iter().enumerate().any(|(other_index, other)| {
            other_index != index
                && other.bounds[0] <= cluster.bounds[0]
                && other.bounds[1] <= cluster.bounds[1]
                && other.bounds[2] >= cluster.bounds[2]
                && other.bounds[3] >= cluster.bounds[3]
                && self.border_clusters_share_territory(player_id, cluster, other)
        })
    }

    fn border_clusters_share_territory(
        &self,
        player_id: u16,
        first: &BorderCluster,
        second: &BorderCluster,
    ) -> bool {
        let second_tiles: HashSet<u32> = second.tiles.iter().copied().collect();
        let mut seen = HashSet::new();
        let mut stack = VecDeque::new();
        for &tile in &first.tiles {
            if self
                .state
                .map
                .owner_id(tile % self.state.map.width, tile / self.state.map.width)
                == player_id
                && seen.insert(tile)
            {
                stack.push_back(tile);
            }
        }
        while let Some(tile) = stack.pop_front() {
            if second_tiles.contains(&tile) {
                return true;
            }
            self.state.map.for_each_neighbor(
                tile % self.state.map.width,
                tile / self.state.map.width,
                |nx, ny| {
                    let neighbor = ny * self.state.map.width + nx;
                    if self.state.map.owner_id(nx, ny) == player_id && seen.insert(neighbor) {
                        stack.push_back(neighbor);
                    }
                },
            );
        }
        false
    }

    fn surrounded_by_single_enemy(&self, victim_id: u16, cluster: &BorderCluster) -> Option<u16> {
        let map = &self.state.map;
        let width = map.width;
        let height = map.height;
        let (mut min_x, mut min_y, mut max_x, mut max_y) = (u32::MAX, u32::MAX, 0, 0);
        let (mut min_enemy_x, mut min_enemy_y, mut max_enemy_x, mut max_enemy_y) =
            (u32::MAX, u32::MAX, 0, 0);
        let mut enemies = BTreeSet::new();

        for &tile in &cluster.tiles {
            let x = tile % width;
            let y = tile / width;
            if x == 0
                || y == 0
                || x + 1 == width
                || y + 1 == height
                || map.terrain[map.ref_id(x, y)].is_shoreline()
            {
                return None;
            }
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
            let mut open = false;
            map.for_each_neighbor4(x, y, |nx, ny| {
                let owner = map.owner_id(nx, ny);
                if owner == 0 {
                    open = true;
                } else if owner != victim_id {
                    enemies.insert(owner);
                    min_enemy_x = min_enemy_x.min(nx);
                    min_enemy_y = min_enemy_y.min(ny);
                    max_enemy_x = max_enemy_x.max(nx);
                    max_enemy_y = max_enemy_y.max(ny);
                }
            });
            if open || enemies.len() != 1 {
                return None;
            }
        }
        if enemies.len() != 1
            || min_enemy_x > min_x
            || min_enemy_y > min_y
            || max_enemy_x < max_x
            || max_enemy_y < max_y
        {
            return None;
        }
        enemies.into_iter().next()
    }

    fn secondary_cluster_is_surrounded(&self, victim_id: u16, cluster: &BorderCluster) -> bool {
        let map = &self.state.map;
        let width = map.width;
        let height = map.height;
        let mut has_enemy = false;
        let (mut min_enemy_x, mut min_enemy_y, mut max_enemy_x, mut max_enemy_y) =
            (u32::MAX, u32::MAX, 0, 0);
        for &tile in &cluster.tiles {
            let x = tile % width;
            let y = tile / width;
            if x == 0
                || y == 0
                || x + 1 == width
                || y + 1 == height
                || map.terrain[map.ref_id(x, y)].is_shoreline()
            {
                return false;
            }
            map.for_each_neighbor4(x, y, |nx, ny| {
                let owner = map.owner_id(nx, ny);
                if owner != 0 && owner != victim_id {
                    has_enemy = true;
                    min_enemy_x = min_enemy_x.min(nx);
                    min_enemy_y = min_enemy_y.min(ny);
                    max_enemy_x = max_enemy_x.max(nx);
                    max_enemy_y = max_enemy_y.max(ny);
                }
            });
        }
        has_enemy
            && min_enemy_x <= cluster.bounds[0]
            && min_enemy_y <= cluster.bounds[1]
            && max_enemy_x >= cluster.bounds[2]
            && max_enemy_y >= cluster.bounds[3]
    }

    fn territory_is_enclosed(&self, victim_id: u16, start: u32) -> bool {
        let width = self.state.map.width;
        let height = self.state.map.height;
        let mut seen = HashSet::from([start]);
        let mut stack = vec![start];
        while let Some(tile) = stack.pop() {
            let x = tile % width;
            let y = tile / width;
            if x == 0 || y == 0 || x + 1 == width || y + 1 == height {
                return false;
            }
            let mut open = false;
            self.state.map.for_each_neighbor4(x, y, |nx, ny| {
                let neighbor = ny * width + nx;
                if seen.contains(&neighbor) {
                    return;
                }
                let owner = self.state.map.owner_id(nx, ny);
                if owner != 0 && owner != victim_id {
                    return;
                }
                if owner == 0 && !self.state.map.terrain[self.state.map.ref_id(nx, ny)].is_land() {
                    open = true;
                    return;
                }
                seen.insert(neighbor);
                stack.push(neighbor);
            });
            if open {
                return false;
            }
        }
        true
    }

    fn capturing_player(&self, victim_id: u16, cluster: &BorderCluster) -> Option<u16> {
        let map = &self.state.map;
        let width = map.width;
        let mut neighbors: Vec<(u16, u32)> = Vec::new();
        for &tile in &cluster.tiles {
            map.for_each_neighbor4(tile % width, tile / width, |nx, ny| {
                let owner = map.owner_id(nx, ny);
                if owner == 0 || owner == victim_id || self.annex_players_friendly(owner, victim_id)
                {
                    return;
                }
                if let Some((_, count)) = neighbors.iter_mut().find(|(id, _)| *id == owner) {
                    *count += 1;
                } else {
                    neighbors.push((owner, 1));
                }
            });
        }
        if neighbors.is_empty() {
            return None;
        }

        let mut largest_attack: Option<(u16, f64)> = None;
        for &(neighbor, _) in &neighbors {
            for attack in &self.attacks {
                if attack.owner_id == neighbor
                    && attack.target_owner == victim_id
                    && attack.troops > 0.0
                    && largest_attack.is_none_or(|(_, troops)| attack.troops > troops)
                {
                    largest_attack = Some((neighbor, attack.troops));
                }
            }
        }
        largest_attack.map(|(id, _)| id).or_else(|| {
            let mut most_border_cells = None;
            for (id, count) in neighbors {
                if most_border_cells.is_none_or(|(_, best_count)| count > best_count) {
                    most_border_cells = Some((id, count));
                }
            }
            most_border_cells.map(|(id, _)| id)
        })
    }

    fn annex_players_friendly(&self, first_id: u16, second_id: u16) -> bool {
        let (Some(first), Some(second)) =
            (self.state.player(first_id), self.state.player(second_id))
        else {
            return false;
        };
        first.alliances.contains(&second_id)
            || second.alliances.contains(&first_id)
            || (first.team.is_some() && first.team == second.team)
    }

    pub fn execute_combat(&mut self) {
        // ponytail: sum players once per tick; maintain a counter only if profiling shows this is hot.
        let neutral_pacing = neutral_expansion_pacing_multiplier(occupied_land_share(&self.state));
        self.execute_combat_using(
            |attack, _| attack.to_conquer.len(),
            move |_, attack, _| {
                attack_pacing_multiplier(
                    attack.target_owner,
                    neutral_pacing,
                    NEUTRAL_CAPTURE_SPEED_MULTIPLIER,
                )
            },
        );
    }

    fn execute_combat_using<F, P>(&mut self, frontier_size: F, pacing_multiplier: P)
    where
        F: Fn(&crate::execution::AttackExecution, &GameState) -> usize,
        P: Fn(f64, &crate::execution::AttackExecution, &GameState) -> f64,
    {
        if self.state.phase != GamePhase::Playing {
            return;
        }
        self.release_expired_pre_defeat_hold();

        // Poka-Yoke: Collections are now strictly maintained sorted on insertion.
        let map_w = self.state.map.width;
        let map_h = self.state.map.height;

        let tick_now = self.state.tick;
        let return_min_ticks = (3_000.0 / self.state.config.tick_rate_ms.max(1.0)).ceil() as u64;
        let return_window_ticks =
            (30_000.0 / self.state.config.tick_rate_ms.max(1.0)).ceil() as u64;
        let held_target = self
            .campaign_assault
            .as_ref()
            .filter(|assault| assault.hold_last_tile)
            .map(|assault| assault.root_target);
        let pre_defeat_targets = self.campaign_pre_defeat_targets.clone();
        let pre_defeat_starting_tile_counts = self.campaign_starting_tile_counts.clone();
        let mut pre_defeat_pending = self.campaign_pre_defeat_pending;

        // Rebuild only when buildings changed; each tile then queries nearby cells.
        if !self.attacks.is_empty() {
            self.refresh_defense_grid();
            self.refresh_building_grid();
        }

        // attacks are sorted on insertion

        let intercession_possible = !self.attacks.is_empty()
            && self.state.config.buildings_enabled
            && self.state.players.iter().any(|player| {
                player.alive && matches!(player.leader.ability(), Ability::CityIntercession { .. })
            });
        let nyabingi_possible =
            !self.attacks.is_empty()
                && self.state.players.iter().any(|p| {
                    p.alive && matches!(p.leader.ability(), Ability::NyabingiNetwork { .. })
                });
        // Evaluate both fronts before resolving either one so attack order cannot
        // remove the bonus for the other army in the same combat tick.
        let mut regional_defenses = BTreeMap::new();
        if self.attacks.len() >= 2 && self.state.config.attack_cost_enemy > 0.0 {
            for p in &self.state.players {
                let multiplier = crate::execution::hero_effects::regional_commands_multiplier(
                    &self.state,
                    &self.attacks,
                    p,
                );
                if multiplier > 1.0 {
                    regional_defenses.insert(p.id, multiplier);
                }
            }
        }
        let mut elimination_candidates = Vec::new();
        let mut territory_changes = Vec::new();
        let mut to_remove = Vec::new();

        for i in 0..self.attacks.len() {
            // Need to borrow state and buildings mutably but not the entire self array
            // Actually, we can borrow state and defense grid and attacks[i] simultaneously
            let retreat = &self.attacks[i];
            let tigranes_aid = if retreat.retreating {
                self.tigranes_aid_city(retreat)
            } else {
                None
            };
            let beneficiary = if retreat.retreating {
                crate::execution::hero_effects::retreat_beneficiary(
                    &self.state,
                    &self.attacks,
                    retreat.owner_id,
                    retreat.target_owner,
                )
            } else {
                None
            };
            let pact_loss = self.state.player(retreat.owner_id).map_or(1.0, |owner| {
                owner.leader.attacker_loss_multiplier(LossWhen::JointFront)
            });
            let pact_allies = if pact_loss < 1.0 {
                crate::execution::hero_effects::joint_front_allies(
                    &self.state,
                    &self.attacks,
                    retreat,
                )
            } else {
                Vec::new()
            };
            let execution = &mut self.attacks[i];
            let regional_defense = regional_defenses
                .get(&execution.target_owner)
                .copied()
                .filter(|_| {
                    !crate::execution::hero_effects::allied(
                        &self.state,
                        execution.target_owner,
                        execution.owner_id,
                    )
                });

            if execution.retreating {
                let refund = execution.troops.max(0.0);
                if refund > 0.0 && refund.is_finite() {
                    let armistice = crate::execution::hero_effects::armistice(
                        &self.state,
                        execution.owner_id,
                        execution.target_owner,
                    );
                    let factor = if execution.target_owner == 0 {
                        1.0
                    } else if let Some((hero, percent)) = armistice {
                        let tile = execution.to_conquer.peek().map(|next| (next.x, next.y));
                        self.state.announce_leader_ability_to(
                            hero,
                            &[execution.owner_id, execution.target_owner],
                            tile,
                        );
                        1.0 - RETREAT_PENALTY_VS_PLAYER * (1.0 - f64::from(percent) / 100.0)
                    } else {
                        // The defender's leader can make retreating from its land costlier;
                        // the retreating leader's `Retreat` ability softens that loss.
                        let (penalty, softening) =
                            crate::execution::hero_effects::retreat_loss_multipliers(
                                &self.state,
                                execution,
                            );
                        if penalty != 1.0 {
                            self.state.announce_leader_ability_to(
                                execution.target_owner,
                                &[execution.owner_id],
                                None,
                            );
                        }
                        if softening != 1.0 {
                            self.state.announce_leader_ability_to(
                                execution.owner_id,
                                &[execution.target_owner],
                                None,
                            );
                        }
                        1.0 - RETREAT_PENALTY_VS_PLAYER * penalty * softening
                    };
                    let mut back = refund * factor;
                    if let Some((ally, percent)) = beneficiary
                        && back > 0.0
                        && let Some(recipient) = self.state.player_mut(ally)
                    {
                        let diverted = crate::execution::hero_effects::retreat_handover_amount(
                            recipient, back, percent,
                        );
                        if diverted > 0.0 {
                            recipient.credit_troops_capped(diverted, recipient.max_troops);
                            back -= diverted;
                            let tile = execution
                                .to_conquer
                                .peek()
                                .map(|next| (next.x, next.y))
                                .or_else(|| {
                                    self.state
                                        .player(execution.target_owner)
                                        .and_then(|p| p.home)
                                });
                            self.state.announce_leader_ability_to(
                                execution.target_owner,
                                &[ally, execution.owner_id],
                                tile,
                            );
                        }
                    }
                    if let Some(player) = self.state.player_mut(execution.owner_id) {
                        let before = player.troops;
                        player.credit_troops_capped(back, player.max_troops);
                        let returned = player.troops - before;
                        if execution.target_owner != 0
                            && matches!(player.leader.ability(), Ability::ReturnCampaign { .. })
                            && tick_now.saturating_sub(execution.created_tick) >= return_min_ticks
                        {
                            player.return_campaign =
                                Some((execution.target_owner, tick_now + return_window_ticks));
                        }
                        if let Some((city, limit)) = tigranes_aid {
                            let aid = returned
                                .min(f64::from(limit))
                                .min(player.max_troops - player.troops);
                            if aid > 0.0 {
                                player.credit_troops(aid);
                                self.hero_claims.insert((execution.owner_id, 0, 0));
                                self.state.announce_leader_ability_to(
                                    execution.owner_id,
                                    &[city.owner_id],
                                    Some((city.tile_idx % map_w, city.tile_idx / map_w)),
                                );
                            }
                        }
                    }
                }
                to_remove.push(i);
                continue;
            }

            if execution.troops < self.state.config.attack_cost_neutral || execution.troops.is_nan()
            {
                to_remove.push(i);
                continue;
            }

            let adjacent = (frontier_size(execution, &self.state) as f64).max(1.0);

            let troop_strength = attack_strength(
                &self.state,
                execution.owner_id,
                execution.target_owner,
                execution.troops,
            );
            let defense_strength = defense_strength(
                &self.state,
                execution.owner_id,
                execution.target_owner,
                execution.troops,
            );
            let effective_troops = execution.troops * troop_strength / defense_strength;
            let target_loss = target_attacker_loss(
                &self.state,
                execution.owner_id,
                execution.target_owner,
                execution.troops,
            );

            let max_cap = max_tiles_cap_for_troops(effective_troops, &self.state.config);

            // Compute the base tile budget in tile-ticks (matching OpenFront's attackTilesPerTick)
            let base_budget = if execution.target_owner == 0 {
                adjacent * 2.0
            } else {
                let defender_troops = self
                    .state
                    .player(execution.target_owner)
                    .map(|p| p.troops.max(0.0))
                    .unwrap_or(1.0)
                    .max(1.0);
                let ratio = (5.0 * effective_troops) / defender_troops;
                let power = (ratio * 2.0).clamp(0.01, 0.5);
                power * adjacent * 3.0
            };

            // Scale budget to real time (reference baseline is a 100ms standard OpenFront tick)
            let mut budget = base_budget * (self.state.config.tick_rate_ms as f64 / 100.0);
            budget *= self.state.config.global_speed_multiplier;

            // Clamp budget with scaled max_cap
            let scaled_max_cap = max_cap
                * (self.state.config.tick_rate_ms as f64 / 100.0)
                * self.state.config.global_speed_multiplier;
            budget = budget.min(scaled_max_cap);
            budget *= pacing_multiplier(effective_troops, execution, &self.state);

            // Hero expansion-speed abilities, after the cap so long fronts keep them.
            // Tribes are the bot players (not nations or humans).
            if let Some(player) = self.state.player(execution.owner_id) {
                budget *= targeted_multiplier(
                    &self.state,
                    execution.owner_id,
                    execution.target_owner,
                    execution.troops,
                    |t| player.leader.conquest_speed_multiplier(t),
                );
            }

            let steppe_discount = if execution.target_owner == 0 {
                match self
                    .state
                    .player(execution.owner_id)
                    .map(|p| p.leader.ability())
                {
                    Some(Ability::ContestedSteppe { percent }) => {
                        f64::from(percent.min(100)) / 100.0
                    }
                    _ => 0.0,
                }
            } else {
                0.0
            };

            let ascent_speed =
                self.state
                    .player(execution.owner_id)
                    .map_or(1.0, |p| match p.leader.ability() {
                        Ability::ZagrosAscent { percent } => 1.0 + f64::from(percent) / 100.0,
                        _ => 1.0,
                    });

            let factory_raid = self.state.config.buildings_enabled
                && execution.target_owner != 0
                && self.state.player(execution.owner_id).is_some_and(|player| {
                    matches!(player.leader.ability(), Ability::FactoryRaid { .. })
                });

            let mut stale_pops = 0u32;
            let max_stale_pops = 256;

            loop {
                if held_target == Some(execution.target_owner)
                    && !Self::pre_defeat_watches(
                        &pre_defeat_targets,
                        pre_defeat_pending,
                        execution.target_owner,
                    )
                    && self
                        .state
                        .player(execution.target_owner)
                        .is_some_and(|target| target.tile_count <= 1)
                {
                    let refund = execution.troops.max(0.0);
                    if refund.is_finite()
                        && let Some(player) = self.state.player_mut(execution.owner_id)
                    {
                        player.credit_troops_capped(refund, player.max_troops);
                    }
                    to_remove.push(i);
                    break;
                }
                if budget <= 0.0 || stale_pops > max_stale_pops {
                    break;
                }

                if execution.troops < self.state.config.attack_cost_neutral
                    || execution.troops.is_nan()
                {
                    to_remove.push(i);
                    break;
                }

                if let Some(target_tile) = execution.to_conquer.pop() {
                    // Check if target tile still belongs to the exact target
                    if self.state.map.owner_id(target_tile.x, target_tile.y)
                        != execution.target_owner
                    {
                        stale_pops += 1;
                        continue; // Skip, no longer controlled by target
                    }

                    // Terrain Check: Water and Mountains are impassable for basic ground attacks
                    let terrain_type = self.state.map.terrain_type(target_tile.x, target_tile.y);
                    if terrain_type == TerrainType::Water || terrain_type == TerrainType::Lake {
                        stale_pops += 1;
                        continue; // Skip, impassable terrain
                    }

                    // Check adjacency: ensure it still borders the attacker
                    if !self.state.map.is_adjacent_to_player(
                        target_tile.x,
                        target_tile.y,
                        execution.owner_id,
                    ) {
                        stale_pops += 1;
                        continue; // Skip, edge was severed
                    }

                    let target_reached_pre_defeat = execution.target_owner != 0
                        && self
                            .state
                            .player(execution.target_owner)
                            .is_some_and(|player| {
                                Self::pre_defeat_reached(
                                    &pre_defeat_starting_tile_counts,
                                    execution.target_owner,
                                    player,
                                )
                            });
                    if target_reached_pre_defeat
                        && Self::pre_defeat_watches(
                            &pre_defeat_targets,
                            pre_defeat_pending,
                            execution.target_owner,
                        )
                    {
                        if pre_defeat_pending.is_none() {
                            pre_defeat_pending = Some(execution.target_owner);
                            self.campaign_pre_defeat_since = tick_now;
                        }
                        execution.to_conquer.push(target_tile);
                        break;
                    }

                    if defense_strength != 1.0 {
                        self.state.announce_leader_ability_to(
                            execution.target_owner,
                            &[execution.owner_id],
                            Some((target_tile.x, target_tile.y)),
                        );
                    }

                    // Determine conquest cost for this specific tile (matching OpenFront's terrain & Tower coefficients)
                    let base_speed = terrain_capture_speed(terrain_type);
                    let tile_idx =
                        u64::from(target_tile.y) * u64::from(map_w) + u64::from(target_tile.x);
                    let sample = stable_tile_sample(
                        self.state.seed,
                        execution.owner_id,
                        execution.target_owner,
                        tile_idx,
                        CAPTURE_COST_VARIATION_SALT,
                    );
                    let capture_cost_factor = 1.0 + (2.0 * sample - 1.0) * CAPTURE_COST_VARIATION;

                    let defense = self.defense_grid.influence(
                        target_tile.x,
                        target_tile.y,
                        map_w,
                        execution.target_owner,
                        execution.owner_id,
                        self.state.seed,
                        &self.state.config,
                    );

                    let tile_cost = if execution.target_owner == 0 {
                        let speed_term = base_speed.max(10.0);
                        let raw_cost =
                            ((2000.0 * speed_term) / effective_troops) * capture_cost_factor;
                        raw_cost.clamp(5.0, 100.0)
                    } else {
                        let defender_troops = self
                            .state
                            .player(execution.target_owner)
                            .map(|p| p.troops.max(0.0))
                            .unwrap_or(1.0)
                            .max(1.0);
                        let ratio = defender_troops / (5.0 * effective_troops);
                        let ratio_clamp = ratio.clamp(0.2, 1.5);
                        ratio_clamp * base_speed * defense.capture_multiplier * capture_cost_factor
                    };
                    let city_multiplier = city_capture_multiplier(
                        &self.state,
                        &self.building_grid,
                        (target_tile.x, target_tile.y),
                        execution.owner_id,
                        execution.target_owner,
                        execution.troops,
                    );
                    if city_multiplier != 1.0 {
                        // The side whose city ability decided the cost is the one shown,
                        // and the other side is the player it hit.
                        let (shown, other) = if city_multiplier > 1.0 {
                            (execution.target_owner, execution.owner_id)
                        } else {
                            (execution.owner_id, execution.target_owner)
                        };
                        self.state.announce_leader_ability_to(
                            shown,
                            &[other],
                            Some((target_tile.x, target_tile.y)),
                        );
                    }
                    let ascent = ascent_speed > 1.0
                        && execution.target_owner != 0
                        && crate::execution::hero_effects::zagros_ascent_target(
                            &self.state,
                            execution.owner_id,
                            target_tile.x,
                            target_tile.y,
                        );
                    let tile_cost =
                        tile_cost * city_multiplier / if ascent { ascent_speed } else { 1.0 };

                    let tile = (target_tile.x, target_tile.y);
                    let mut tile_cost = tile_cost;
                    if nyabingi_possible
                        && terrain_type == TerrainType::Highland
                        && let Some((hero, percent)) =
                            crate::execution::hero_effects::nyabingi_support(
                                &self.state,
                                tile,
                                Some(execution.owner_id),
                            )
                    {
                        tile_cost *= 1.0 + f64::from(percent) / 100.0;
                        self.state.announce_leader_ability_to(
                            hero,
                            &[execution.target_owner, execution.owner_id],
                            Some(tile),
                        );
                    }

                    // Check if we can afford this tile on this tick using expectation-invariant deterministic RNG
                    if budget < tile_cost {
                        let fraction = budget / tile_cost;
                        let roll = execution.rng.next_int(0, 1000) as f64 / 1000.0;
                        if roll < fraction {
                            budget = 0.0; // Round up: successfully conquer, consume remaining budget
                        } else {
                            // Round down: terminate loop and push tile back to the queue
                            execution.to_conquer.push(target_tile);
                            break;
                        }
                    } else {
                        budget -= tile_cost;
                    }

                    if intercession_possible
                        && execution.target_owner != 0
                        && self
                            .building_grid
                            .iter_in_range(target_tile.x, target_tile.y, 0)
                            .any(|city| city.x == target_tile.x && city.y == target_tile.y)
                        && let Some(city) = self.buildings.iter().copied().find(|b| {
                            b.kind == crate::game::BuildingKind::City
                                && u64::from(b.tile_idx) == tile_idx
                                && b.owner_id == execution.target_owner
                        })
                        && let Some(loss) = crate::execution::hero_effects::intercede_for_city(
                            &mut self.state,
                            &mut self.hero_claims,
                            city,
                            execution.owner_id,
                        )
                    {
                        execution.troops = (execution.troops - loss).max(0.0);
                        execution.to_conquer.push(target_tile);
                        break;
                    }

                    let terrain_loss =
                        terrain_attacker_loss(&self.state, execution.owner_id, terrain_type);
                    let city_loss = city_attacker_loss(
                        &self.state,
                        &self.building_grid,
                        (target_tile.x, target_tile.y),
                        execution.owner_id,
                    );
                    let coast_loss = coast_attacker_loss(
                        &self.state,
                        (target_tile.x, target_tile.y),
                        execution.owner_id,
                    );
                    let landing_loss =
                        landing_attacker_loss(&self.state, execution.owner_id, execution.landed);
                    if ascent
                        || terrain_loss != 1.0
                        || city_loss != 1.0
                        || target_loss != 1.0
                        || coast_loss != 1.0
                        || landing_loss != 1.0
                    {
                        self.state.announce_leader_ability_to(
                            execution.owner_id,
                            &[execution.target_owner],
                            Some((target_tile.x, target_tile.y)),
                        );
                    }
                    let terrain_multiplier =
                        terrain_loss_multiplier(terrain_type, &self.state.config) * terrain_loss;
                    // Hero city-tile, coast, landing and target loss abilities scale the same per-tile cost.
                    let loss_multiplier =
                        terrain_multiplier * city_loss * target_loss * coast_loss * landing_loss;

                    let mut def_loss = 0.0;
                    if execution.target_owner == 0 {
                        let mut loss = self.state.config.attack_cost_neutral * loss_multiplier
                            / troop_strength;
                        if steppe_discount > 0.0
                            && loss > 0.0
                            && crate::execution::hero_effects::contested_steppe(
                                &self.state,
                                execution.owner_id,
                                target_tile.x,
                                target_tile.y,
                            )
                        {
                            loss *= 1.0 - steppe_discount;
                            self.state.announce_leader_ability_to(
                                execution.owner_id,
                                &[],
                                Some((target_tile.x, target_tile.y)),
                            );
                        }
                        execution.troops -= loss;
                    } else {
                        // PvP: Combat resolution
                        // Hero tower-defense ability scales the towers' extra losses.
                        let tower_defense = self
                            .state
                            .player(execution.target_owner)
                            .map_or(1.0, |player| {
                                player.leader.tower_loss_multiplier(Side::Defender)
                            });
                        if let Some(target_player) = self.state.player(execution.target_owner)
                            && target_player.tile_count > 0
                        {
                            // Defender loses proportional to their troop density.
                            // Clamp to 0: if troops went negative from multi-attack drain,
                            // a negative def_loss would ADD troops to the defender on
                            // subtract, compounding per tick and diverging across clients.
                            def_loss =
                                target_player.troops.max(0.0) / target_player.tile_count as f64;
                        }

                        // Defense posts increase attacker losses slightly. The defender's
                        // tower ability raises that extra, the attacker's tower siege lowers it.
                        let tower_siege =
                            self.state.player(execution.owner_id).map_or(1.0, |player| {
                                player.leader.tower_loss_multiplier(Side::Attacker)
                            });
                        let tower_extra =
                            (defense.attacker_loss_multiplier - 1.0) * tower_defense * tower_siege;
                        let tile = Some((target_tile.x, target_tile.y));
                        if defense.attacker_loss_multiplier > 1.0 {
                            if tower_defense != 1.0 {
                                self.state.announce_leader_ability_to(
                                    execution.target_owner,
                                    &[execution.owner_id],
                                    tile,
                                );
                            }
                            if tower_siege != 1.0 {
                                self.state.announce_leader_ability_to(
                                    execution.owner_id,
                                    &[execution.target_owner],
                                    tile,
                                );
                            }
                        }
                        let recapture = recapture_multiplier(
                            &self.state,
                            execution.owner_id,
                            execution.target_owner,
                        );
                        if recapture != 1.0 {
                            self.state.announce_leader_ability_to(
                                execution.owner_id,
                                &[execution.target_owner],
                                tile,
                            );
                        }
                        let mut atk_loss = (self.state.config.attack_cost_enemy
                            * loss_multiplier
                            * (1.0 + tower_extra)
                            * recapture)
                            / troop_strength;

                        if atk_loss > 0.0
                            && let Some(ally) =
                                crate::execution::hero_effects::joint_front_supporter(
                                    &self.state,
                                    &pact_allies,
                                    (target_tile.x, target_tile.y),
                                )
                        {
                            atk_loss *= pact_loss;
                            self.state.announce_leader_ability_to(
                                execution.owner_id,
                                &[execution.target_owner, ally],
                                tile,
                            );
                        }
                        if let Some(multiplier) = regional_defense
                            && atk_loss > 0.0
                            && atk_loss < execution.troops
                        {
                            atk_loss *= multiplier;
                            self.state.announce_leader_ability_to(
                                execution.target_owner,
                                &[execution.owner_id],
                                tile,
                            );
                        }
                        let captured = atk_loss.min(execution.troops.max(0.0))
                            * crate::execution::hero_effects::mounted_defense_capture_rate(
                                &self.state,
                                execution.target_owner,
                                execution.owner_id,
                            );
                        if captured > 0.0
                            && let Some(defender) = self.state.player_mut(execution.target_owner)
                        {
                            let before = defender.troops;
                            defender.credit_troops_capped(captured, defender.max_troops);
                            if defender.troops > before {
                                self.state.announce_leader_ability_to(
                                    execution.target_owner,
                                    &[execution.owner_id],
                                    tile,
                                );
                            }
                        }
                        execution.troops -= atk_loss;

                        // Deduct from defender
                        if let Some(target_player) = self.state.player_mut(execution.target_owner) {
                            target_player.troops = (target_player.troops - def_loss).max(0.0);
                        }
                    }

                    execution.troops = execution.troops.max(0.0);

                    // Enqueue new neutral/enemy neighbors that touch our newly acquired tile
                    let is_odd = (target_tile.y % 2) != 0;
                    let deltas = if is_odd {
                        [(1, 0), (-1, 0), (0, -1), (1, -1), (0, 1), (1, 1)]
                    } else {
                        [(1, 0), (-1, 0), (-1, -1), (0, -1), (-1, 1), (0, 1)]
                    };
                    for &(dx, dy) in &deltas {
                        let nx = target_tile.x as i32 + dx;
                        let ny = target_tile.y as i32 + dy;
                        if nx >= 0 && nx < map_w as i32 && ny >= 0 && ny < map_h as i32 {
                            let nx = nx as u32;
                            let ny = ny as u32;
                            if self.state.map.owner_id(nx, ny) != execution.target_owner {
                                continue;
                            }
                            if !self.state.map.terrain[self.state.map.ref_id(nx, ny)].is_land() {
                                continue;
                            }

                            // How many tiles bordering (nx, ny) are owned by the attacker?
                            let mut num_owned_by_me = 0;
                            let n_is_odd = !ny.is_multiple_of(2);
                            let n_deltas = if n_is_odd {
                                [(1, 0), (-1, 0), (0, -1), (1, -1), (0, 1), (1, 1)]
                            } else {
                                [(1, 0), (-1, 0), (-1, -1), (0, -1), (-1, 1), (0, 1)]
                            };
                            for &(ndx, ndy) in &n_deltas {
                                let nnx = nx as i32 + ndx;
                                let nny = ny as i32 + ndy;
                                if nnx >= 0
                                    && nnx < map_w as i32
                                    && nny >= 0
                                    && nny < map_h as i32
                                    && self.state.map.owner_id(nnx as u32, nny as u32)
                                        == execution.owner_id
                                {
                                    num_owned_by_me += 1;
                                }
                            }

                            let terrain = self.state.map.terrain_type(nx, ny);
                            let mut prio =
                                execution.calc_priority(num_owned_by_me, terrain, tick_now);
                            prio += self.defense_grid.priority_bonus(
                                nx,
                                ny,
                                map_w,
                                execution.target_owner,
                                execution.owner_id,
                                self.state.seed,
                                &self.state.config,
                            ) as i64;
                            let seq = execution.insert_seq_counter;
                            execution.insert_seq_counter =
                                execution.insert_seq_counter.wrapping_add(1);

                            execution.to_conquer.push(PrioritizedTile {
                                priority: prio,
                                insert_seq: seq,
                                x: nx,
                                y: ny,
                            });
                        }
                    }

                    // VALID CONQUEST
                    // Apply change AFTER enqueuing neighbors
                    // This ensures new neighbors don't artificially lower their priority by counting this tile as friendly yet!
                    // Guaranteed BFS spread without DFS spikes!
                    let (eliminated, candidates) = self.state.set_tile_owner_for_execution(
                        target_tile.x,
                        target_tile.y,
                        execution.owner_id,
                    );
                    elimination_candidates.extend(
                        eliminated
                            .into_iter()
                            // On the last tile, def_loss is the entire defending reserve.
                            .map(|(victim, x, y)| {
                                (victim, execution.owner_id, x, y, Some(def_loss))
                            }),
                    );
                    if let Some(change) = candidates {
                        territory_changes.push(change);
                    }

                    if self.state.player(execution.owner_id).is_some_and(|player| {
                        player.leader.has_resource_bonus_from(Source::Territory)
                    }) {
                        self.state.announce_leader_ability(
                            execution.owner_id,
                            Some((target_tile.x, target_tile.y)),
                        );
                    }

                    if factory_raid
                        && self
                            .building_grid
                            .iter_non_city_in_range(target_tile.x, target_tile.y, 0)
                            .any(|entry| {
                                entry.kind == crate::game::BuildingKind::Factory
                                    && entry.x == target_tile.x
                                    && entry.y == target_tile.y
                            })
                        && let Some(factory) = self.buildings.iter().copied().find(|building| {
                            building.kind == crate::game::BuildingKind::Factory
                                && building.owner_id == execution.target_owner
                                && u64::from(building.tile_idx) == tile_idx
                        })
                    {
                        crate::execution::hero_effects::raid_captured_factory(
                            &mut self.state,
                            &mut self.hero_claims,
                            execution,
                            factory,
                        );
                    }

                    // A player whose leader can recapture opens its counterattack window when it loses a city.
                    if self
                        .building_grid
                        .city_covers_tile(target_tile.x, target_tile.y)
                        && let Some(victim) = self.state.player_mut(execution.target_owner)
                        && victim.leader.has_attacker_loss(LossWhen::RecaptureWindow)
                    {
                        victim.recapture_target = execution.owner_id;
                        victim.recapture_ticks = RECAPTURE_WINDOW_TICKS;
                    }

                    // Send event
                    self.state.events.push(GameEvent::TileCaptured {
                        x: target_tile.x,
                        y: target_tile.y,
                        new_owner: execution.owner_id,
                        previous_owner: execution.target_owner,
                        troops: execution.troops,
                    });
                } else {
                    let refund = execution.troops.max(0.0);
                    if refund > 0.0
                        && refund.is_finite()
                        && let Some(player) = self.state.player_mut(execution.owner_id)
                    {
                        player.credit_troops_capped(refund, player.max_troops);
                    }
                    to_remove.push(i);
                    break;
                }
            }
        }

        // Remove dead attacks in O(1) and re-sort to preserve deterministic order
        let has_removals = !to_remove.is_empty();
        for i in to_remove.into_iter().rev() {
            self.attacks.swap_remove(i);
        }
        if has_removals {
            self.attacks.sort_unstable_by_key(|a| a.id);
            self.ai_attack_index_dirty = true;
        }

        elimination_candidates.extend(
            self.annex_enclosed_components(territory_changes)
                .into_iter()
                .map(|(victim, conqueror, x, y)| (victim, conqueror, x, y, None)),
        );
        for (victim, conqueror, x, y, last_defenders) in elimination_candidates {
            crate::execution::hero_effects::recruit_conquered_reserves(
                &mut self.state,
                conqueror,
                victim,
                (x, y),
                last_defenders,
            );
            self.eliminate_player(victim, conqueror, x, y, false);
        }
        self.hero_coastal_foundings();
        self.campaign_pre_defeat_pending = pre_defeat_pending;
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CAPTURE_COST_VARIATION, CAPTURE_COST_VARIATION_SALT, NEUTRAL_CAPTURE_SPEED_MULTIPLIER,
        attack_pacing_multiplier, neutral_expansion_pacing_multiplier, occupied_land_share,
        terrain_capture_speed, terrain_loss_multiplier,
    };
    use crate::building::Building;
    use crate::execution::{AttackExecution, PrioritizedTile};
    use crate::game::{BuildingKind, GameEvent, GamePhase, GameState};
    use crate::game_config::{GameConfig, max_tiles_cap_for_troops};
    use crate::geo_entities::Target;
    use crate::map::{MapTile, TerrainType};
    use crate::player::Player;
    use crate::rng::NextIntExt;
    use crate::water_components::WaterComponents;
    use std::collections::{BinaryHeap, HashSet};
    use wyrand::WyRand;

    #[test]
    #[ignore = "wall-clock benchmark; run in release with --ignored --nocapture --test-threads=1"]
    fn neutral_annexation_combat_benchmark() {
        use std::hint::black_box;
        use std::time::Instant;

        const WIDTH: u32 = 512;
        const SAMPLES: usize = 201;
        for (name, enemy_x, captures) in [
            ("open-neutral", 256, 256),
            ("large-open-frontier", 34, 1),
            ("same-victim-batch", 34, 256),
        ] {
            let mut state = GameState::new(
                7,
                WIDTH,
                WIDTH,
                GameConfig {
                    tick_rate_ms: 100.0,
                    global_speed_multiplier: 1.0,
                    ..Default::default()
                },
            );
            state.phase = GamePhase::Playing;
            state.map.terrain.fill(MapTile::from_byte(0x80));
            for id in [1, 2] {
                let mut player = Player::new_human(id, format!("P{id}"), [1.0; 3], &state.config);
                player.leader = crate::player::Leader::of("caesar");
                state.register_player(player);
            }
            for y in 1..WIDTH - 1 {
                state.set_tile_owner(32, y, 1);
            }
            for y in 8..WIDTH - 8 {
                for x in enemy_x..WIDTH - 8 {
                    state.set_tile_owner(x, y, 2);
                }
            }
            let mut template = crate::engine::SowEngine::new(state, WaterComponents::default());
            let border_tiles = template.state.player(2).unwrap().border_tiles.count_ones();
            template.add_attack(AttackExecution {
                landed: false,
                id: 1,
                owner_id: 1,
                target_owner: 0,
                created_tick: 0,
                troops: 1_000_000.0,
                to_conquer: (0..captures)
                    .map(|seq| PrioritizedTile {
                        priority: i64::MAX,
                        insert_seq: seq,
                        x: 33,
                        y: 128 + seq,
                    })
                    .collect(),
                insert_seq_counter: captures,
                rng: WyRand::new(7),
                retreating: false,
            });
            // Exclude setup/cloning and keep the capture count identical across rule changes.
            // Neutral tiles cost five at this troop strength; base budget is two.
            let mut samples = Vec::with_capacity(SAMPLES);
            for sample in 0..SAMPLES + 16 {
                let mut engine = template.clone();
                let started = Instant::now();
                engine.execute_combat_using(|_, _| 1, |_, _, _| 2.5 * f64::from(captures));
                let elapsed = started.elapsed().as_nanos();
                black_box(&engine);
                assert_eq!(
                    engine
                        .state
                        .events
                        .iter()
                        .filter(|event| matches!(event, GameEvent::TileCaptured { .. }))
                        .count(),
                    captures as usize,
                    "{name}: benchmark workload changed"
                );
                assert!(engine.state.player(2).unwrap().alive);
                if sample >= 16 {
                    samples.push(elapsed);
                }
            }
            samples.sort_unstable();
            println!(
                "neutral annexation {name}: captures={captures} border={border_tiles} median={}ns p95={}ns",
                samples[SAMPLES / 2],
                samples[SAMPLES * 95 / 100]
            );
        }
    }

    #[test]
    fn occupied_land_share_uses_actual_land_and_handles_missing_land() {
        let mut state = GameState::new(1, 10, 10, GameConfig::default());
        state.map.terrain.fill(MapTile::from_byte(0x20));
        for y in 0..10 {
            for x in 0..2 {
                let idx = state.map.ref_id(x, y);
                state.map.terrain[idx] = MapTile::from_byte(0x80);
            }
        }
        state.total_land_tiles = 20;
        state.register_player(Player::new_human(1, "One".into(), [1.0; 3], &state.config));
        state.register_player(Player::new_human(2, "Two".into(), [0.5; 3], &state.config));
        for y in 0..5 {
            state.set_tile_owner(0, y, 1);
        }
        for y in 5..7 {
            state.set_tile_owner(1, y, 2);
        }

        assert_eq!(
            state
                .map
                .terrain
                .iter()
                .filter(|tile| tile.is_land())
                .count(),
            state.total_land_tiles as usize
        );
        assert!((occupied_land_share(&state) - 0.35).abs() < 1e-12);

        state.total_land_tiles = 0;
        assert_eq!(occupied_land_share(&state), 0.0);
    }

    #[test]
    fn neutral_occupancy_curve_hits_anchors_is_monotonic_and_bounded() {
        assert_eq!(neutral_expansion_pacing_multiplier(0.0), 0.70);
        assert_eq!(neutral_expansion_pacing_multiplier(0.10), 0.70);
        assert_eq!(neutral_expansion_pacing_multiplier(0.50), 1.00);
        assert_eq!(neutral_expansion_pacing_multiplier(0.90), 1.15);
        assert_eq!(neutral_expansion_pacing_multiplier(1.0), 1.15);
        assert_eq!(neutral_expansion_pacing_multiplier(f64::NAN), 0.70);
        assert_eq!(neutral_expansion_pacing_multiplier(-1.0), 0.70);
        assert_eq!(neutral_expansion_pacing_multiplier(2.0), 1.15);

        let shares = [0.0, 0.10, 0.25, 0.50, 0.70, 0.90, 1.0];
        for pair in shares.windows(2) {
            assert!(
                neutral_expansion_pacing_multiplier(pair[0])
                    <= neutral_expansion_pacing_multiplier(pair[1])
            );
        }
    }

    #[test]
    fn occupancy_pacing_only_changes_neutral_attacks() {
        assert!(
            (attack_pacing_multiplier(0, 0.70, NEUTRAL_CAPTURE_SPEED_MULTIPLIER) - 2.10).abs()
                < 1e-12
        );
        assert_eq!(
            attack_pacing_multiplier(0, 1.00, NEUTRAL_CAPTURE_SPEED_MULTIPLIER),
            3.00
        );
        assert!(
            (attack_pacing_multiplier(0, 1.15, NEUTRAL_CAPTURE_SPEED_MULTIPLIER) - 3.45).abs()
                < 1e-12
        );
        assert_eq!(attack_pacing_multiplier(0, 0.70, 1.0), 0.70);
        assert_eq!(
            attack_pacing_multiplier(2, 0.70, NEUTRAL_CAPTURE_SPEED_MULTIPLIER),
            1.0
        );
        assert_eq!(
            attack_pacing_multiplier(2, 1.15, NEUTRAL_CAPTURE_SPEED_MULTIPLIER),
            1.0
        );
    }

    fn multi_neutral_attack_engine(
        seed: u64,
        initial_occupied_share: f64,
    ) -> crate::engine::SowEngine {
        const WIDTH: u32 = 100;
        const HEIGHT: u32 = 100;
        const LAND_WIDTH: u32 = 80;
        const TOTAL_LAND: u32 = LAND_WIDTH * HEIGHT;

        let mut config = GameConfig::default();
        config.map_width = WIDTH;
        config.map_height = HEIGHT;
        let mut state = GameState::new(seed, WIDTH, HEIGHT, config);
        state.phase = GamePhase::Playing;
        state.map.terrain.fill(MapTile::from_byte(0x80));
        for y in 0..HEIGHT {
            for x in LAND_WIDTH..WIDTH {
                let idx = state.map.ref_id(x, y);
                state.map.terrain[idx] = MapTile::from_byte(0x20);
            }
        }
        state.total_land_tiles = TOTAL_LAND;
        for (player_id, name) in [(1, "One"), (2, "Two"), (3, "Filler")] {
            state.register_player(Player::new_human(
                player_id,
                name.into(),
                [1.0; 3],
                &state.config,
            ));
        }

        for x in 20..68 {
            state.set_tile_owner(x, 30, 1);
            state.set_tile_owner(x, 70, 2);
        }

        let target_occupied_tiles = (f64::from(TOTAL_LAND) * initial_occupied_share).round() as u64;
        let mut occupied_tiles: u64 = state
            .players
            .iter()
            .map(|player| u64::from(player.tile_count))
            .sum();
        'fill: for y in 0..HEIGHT {
            for x in 0..LAND_WIDTH {
                let protected =
                    (18..70).contains(&x) && ((28..33).contains(&y) || (68..73).contains(&y));
                if !protected && state.map.owner_id(x, y) == 0 {
                    if occupied_tiles >= target_occupied_tiles {
                        break 'fill;
                    }
                    state.set_tile_owner(x, y, 3);
                    occupied_tiles += 1;
                }
            }
        }
        assert_eq!(occupied_tiles, target_occupied_tiles);
        assert!((occupied_land_share(&state) - initial_occupied_share).abs() < 1e-12);
        assert_eq!(
            state
                .map
                .terrain
                .iter()
                .filter(|tile| tile.is_land())
                .count(),
            TOTAL_LAND as usize
        );

        let mut engine = crate::engine::SowEngine::new(state, WaterComponents::default());
        for (id, owner_id, y) in [(1, 1, 31), (2, 2, 71)] {
            let to_conquer = (20..68)
                .enumerate()
                .map(|(seq, x)| PrioritizedTile {
                    priority: 0,
                    insert_seq: seq as u32,
                    x,
                    y,
                })
                .collect();
            engine.add_attack(AttackExecution {
                landed: false,
                id,
                owner_id,
                target_owner: 0,
                created_tick: 0,
                troops: 25_000.0,
                to_conquer,
                insert_seq_counter: 48,
                rng: WyRand::new(seed.wrapping_add(u64::from(owner_id))),
                retreating: false,
            });
        }
        engine
    }

    fn multi_neutral_attack_capture_rate(
        seed: u64,
        initial_occupied_share: f64,
        neutral_capture_speed_multiplier: f64,
        fixed_frontier: bool,
    ) -> (f64, Vec<u32>) {
        const TICKS: u64 = 5;
        let mut engine = multi_neutral_attack_engine(seed, initial_occupied_share);
        let initial_frontier_size = engine
            .attacks
            .first()
            .map_or(1, |attack| attack.to_conquer.len());
        let mut captures = 0u64;
        for tick in 0..TICKS {
            engine.state.tick = tick;
            engine.state.events.clear();
            let neutral_pacing =
                neutral_expansion_pacing_multiplier(occupied_land_share(&engine.state));
            engine.execute_combat_using(
                move |attack, _| {
                    if fixed_frontier {
                        initial_frontier_size
                    } else {
                        attack.to_conquer.len()
                    }
                },
                move |_, attack, _| {
                    attack_pacing_multiplier(
                        attack.target_owner,
                        neutral_pacing,
                        neutral_capture_speed_multiplier,
                    )
                },
            );
            captures += engine
                .state
                .events
                .iter()
                .filter(|event| matches!(event, crate::game::GameEvent::TileCaptured { .. }))
                .count() as u64;
        }
        let duration_seconds = TICKS as f64 * f64::from(engine.state.config.tick_rate_ms) / 1000.0;
        let final_tiles = engine
            .state
            .players
            .iter()
            .map(|player| player.tile_count)
            .collect();
        (captures as f64 / duration_seconds, final_tiles)
    }

    #[test]
    fn global_occupancy_curve_speeds_neutral_multi_attacks_deterministically() {
        for seed in 1..=3 {
            let early_baseline = multi_neutral_attack_capture_rate(seed, 0.10, 1.0, false);
            let middle_baseline = multi_neutral_attack_capture_rate(seed, 0.50, 1.0, false);
            let late_baseline = multi_neutral_attack_capture_rate(seed, 0.90, 1.0, false);
            let early = multi_neutral_attack_capture_rate(
                seed,
                0.10,
                NEUTRAL_CAPTURE_SPEED_MULTIPLIER,
                false,
            );
            let middle = multi_neutral_attack_capture_rate(
                seed,
                0.50,
                NEUTRAL_CAPTURE_SPEED_MULTIPLIER,
                false,
            );
            let late = multi_neutral_attack_capture_rate(
                seed,
                0.90,
                NEUTRAL_CAPTURE_SPEED_MULTIPLIER,
                false,
            );
            assert!(early.0 < middle.0, "seed {seed}: {early:?} >= {middle:?}");
            assert!(middle.0 < late.0, "seed {seed}: {middle:?} >= {late:?}");
            assert!(
                early.0 > early_baseline.0
                    && middle.0 > middle_baseline.0
                    && late.0 > late_baseline.0,
                "seed {seed}: tripled neutral speed must increase capture rate at each occupancy"
            );
            println!(
                "global_neutral_pacing seed={seed} occupied=10/50/90 baseline_captures_per_second={:.2}/{:.2}/{:.2} accelerated_captures_per_second={:.2}/{:.2}/{:.2} ratios={:.3}/{:.3}/{:.3}",
                early_baseline.0,
                middle_baseline.0,
                late_baseline.0,
                early.0,
                middle.0,
                late.0,
                early.0 / early_baseline.0,
                middle.0 / middle_baseline.0,
                late.0 / late_baseline.0,
            );
            assert_eq!(
                early_baseline,
                multi_neutral_attack_capture_rate(seed, 0.10, 1.0, false)
            );
            assert_eq!(
                middle_baseline,
                multi_neutral_attack_capture_rate(seed, 0.50, 1.0, false)
            );
            assert_eq!(
                late_baseline,
                multi_neutral_attack_capture_rate(seed, 0.90, 1.0, false)
            );
            assert_eq!(
                early,
                multi_neutral_attack_capture_rate(
                    seed,
                    0.10,
                    NEUTRAL_CAPTURE_SPEED_MULTIPLIER,
                    false,
                )
            );
            assert_eq!(
                middle,
                multi_neutral_attack_capture_rate(
                    seed,
                    0.50,
                    NEUTRAL_CAPTURE_SPEED_MULTIPLIER,
                    false,
                )
            );
            assert_eq!(
                late,
                multi_neutral_attack_capture_rate(
                    seed,
                    0.90,
                    NEUTRAL_CAPTURE_SPEED_MULTIPLIER,
                    false,
                )
            );
        }
    }

    #[test]
    fn neutral_capture_speedup_approaches_three_times_at_occupancy_anchors() {
        for seed in 1..=3 {
            for occupied_share in [0.10, 0.50, 0.90] {
                let baseline = multi_neutral_attack_capture_rate(seed, occupied_share, 1.0, true);
                let accelerated = multi_neutral_attack_capture_rate(
                    seed,
                    occupied_share,
                    NEUTRAL_CAPTURE_SPEED_MULTIPLIER,
                    true,
                );
                let ratio = accelerated.0 / baseline.0;
                println!(
                    "neutral_capture_speedup_fixed_frontier seed={seed} occupied={:.0}% baseline_captures_per_second={:.2} accelerated_captures_per_second={:.2} ratio={ratio:.3}",
                    occupied_share * 100.0,
                    baseline.0,
                    accelerated.0,
                );
                assert!(
                    (2.5..=3.5).contains(&ratio),
                    "seed={seed} occupied={:.0}% baseline={baseline:?} accelerated={accelerated:?} ratio={ratio:.3}",
                    occupied_share * 100.0
                );
                assert_eq!(
                    accelerated,
                    multi_neutral_attack_capture_rate(
                        seed,
                        occupied_share,
                        NEUTRAL_CAPTURE_SPEED_MULTIPLIER,
                        true,
                    )
                );
            }
        }
    }

    fn unique_valid_frontier_size(attack: &AttackExecution, state: &GameState) -> usize {
        attack
            .to_conquer
            .iter()
            .filter(|tile| {
                state.map.owner_id(tile.x, tile.y) == attack.target_owner
                    && state.map.terrain[state.map.ref_id(tile.x, tile.y)].is_land()
                    && state
                        .map
                        .is_adjacent_to_player(tile.x, tile.y, attack.owner_id)
            })
            .map(|tile| (tile.x, tile.y))
            .collect::<HashSet<_>>()
            .len()
    }

    fn experimental_troop_pacing_multiplier(troops: f64) -> f64 {
        const EARLY: (f64, f64) = (2_500.0, 0.70);
        const MID: (f64, f64) = (25_000.0, 1.00);
        const LATE: (f64, f64) = (100_000.0, 1.15);

        let (low, high) = if troops <= MID.0 {
            (EARLY, MID)
        } else {
            (MID, LATE)
        };
        if troops <= EARLY.0 {
            return EARLY.1;
        }
        if troops >= LATE.0 {
            return LATE.1;
        }

        let progress = (troops.log10() - low.0.log10()) / (high.0.log10() - low.0.log10());
        low.1 + (high.1 - low.1) * progress
    }

    fn experimental_territory_pacing_multiplier(share: f64) -> f64 {
        const EARLY: (f64, f64) = (0.01, 0.70);
        const MID: (f64, f64) = (0.10, 1.00);
        const LATE: (f64, f64) = (0.30, 1.15);

        let (low, high) = if share <= MID.0 {
            (EARLY, MID)
        } else {
            (MID, LATE)
        };
        if share <= EARLY.0 {
            return EARLY.1;
        }
        if share >= LATE.0 {
            return LATE.1;
        }

        let progress = (share.log10() - low.0.log10()) / (high.0.log10() - low.0.log10());
        low.1 + (high.1 - low.1) * progress
    }

    fn pacing_experiment_engine_at_share(
        seed: u64,
        troops: f64,
        frontier_size: usize,
        target_owner: u16,
        territory_share: Option<f64>,
    ) -> crate::engine::SowEngine {
        let (width, height) = (256, 256);
        let mut config = GameConfig::default();
        config.map_width = width;
        config.map_height = height;
        let mut state = GameState::new(seed, width, height, config);
        state.phase = GamePhase::Playing;
        state.map.terrain.fill(MapTile::from_byte(0x80));
        state.register_player(Player::new_human(
            1,
            "Attacker".into(),
            [1.0; 3],
            &state.config,
        ));
        if target_owner != 0 {
            state.register_player(Player::new_human(
                2,
                "Defender".into(),
                [0.5; 3],
                &state.config,
            ));
            for y in 16..height - 16 {
                for x in 16..width - 16 {
                    state.set_tile_owner(x, y, target_owner);
                }
            }
        }

        let center_y = height / 2;
        let center_x = width / 3;
        if let Some(share) = territory_share {
            let area = ((width * height) as f64 * share).round() as u32;
            let region_width = (area as f64).sqrt().floor().max(1.0) as u32;
            let region_height = area.div_ceil(region_width);
            let start_x = width / 2 - region_width / 2;
            let start_y = center_y - region_height / 2;
            for idx in 0..area {
                let x = start_x + idx % region_width;
                let y = start_y + idx / region_width;
                state.set_tile_owner(x, y, 1);
            }
        } else {
            let owned_width = (frontier_size / 2).max(1) as u32;
            for x in center_x..center_x + owned_width {
                state.set_tile_owner(x, center_y, 1);
            }
        }

        let mut frontier = Vec::new();
        for y in 1..height - 1 {
            for x in 1..width - 1 {
                if state.map.owner_id(x, y) == target_owner
                    && state.map.terrain[state.map.ref_id(x, y)].is_land()
                    && state.map.is_adjacent_to_player(x, y, 1)
                {
                    frontier.push((x, y));
                }
            }
        }
        frontier.truncate(frontier_size);
        assert_eq!(
            frontier.len(),
            frontier_size,
            "scenario frontier is too small"
        );

        let to_conquer = frontier
            .into_iter()
            .enumerate()
            .map(|(seq, (x, y))| PrioritizedTile {
                priority: 0,
                insert_seq: seq as u32,
                x,
                y,
            })
            .collect();
        let mut engine = crate::engine::SowEngine::new(state, WaterComponents::default());
        engine.add_attack(AttackExecution {
            landed: false,
            id: 1,
            owner_id: 1,
            target_owner,
            created_tick: 0,
            troops,
            to_conquer,
            insert_seq_counter: frontier_size as u32,
            rng: WyRand::new(seed.wrapping_add(1)),
            retreating: false,
        });
        engine
    }

    #[derive(Default)]
    struct PacingExperimentResult {
        captures: u64,
        active_ticks: u64,
        queue_entries: u64,
        unique_frontier: u64,
        cap_ticks: u64,
        troops_remaining: f64,
    }

    fn run_frontier_experiment<F, P>(
        seed: u64,
        troops: f64,
        frontier_size: usize,
        target_owner: u16,
        frontier_counter: F,
        pacing_multiplier: P,
        territory_share: Option<f64>,
    ) -> PacingExperimentResult
    where
        F: Fn(&AttackExecution, &GameState) -> usize + Copy,
        P: Fn(f64, &AttackExecution, &GameState) -> f64 + Copy,
    {
        let mut engine = pacing_experiment_engine_at_share(
            seed,
            troops,
            frontier_size,
            target_owner,
            territory_share,
        );
        let mut result = PacingExperimentResult::default();
        let tick_scale = engine.state.config.tick_rate_ms as f64 / 100.0
            * engine.state.config.global_speed_multiplier;
        for tick in 0..300 {
            engine.state.tick = tick;
            engine.state.events.clear();
            if let Some(attack) = engine.attacks.first() {
                result.active_ticks += 1;
                result.queue_entries += attack.to_conquer.len() as u64;
                result.unique_frontier += unique_valid_frontier_size(attack, &engine.state) as u64;
                let adjacent = frontier_counter(attack, &engine.state).max(1) as f64;
                let base_budget = if target_owner == 0 {
                    adjacent * 2.0
                } else {
                    let defender_troops = engine
                        .state
                        .player(target_owner)
                        .map(|player| player.troops.max(1.0))
                        .unwrap_or(1.0);
                    let ratio = (5.0 * attack.troops) / defender_troops;
                    (ratio * 2.0).clamp(0.01, 0.5) * adjacent * 3.0
                };
                let cap =
                    max_tiles_cap_for_troops(attack.troops, &engine.state.config) * tick_scale;
                result.cap_ticks += u64::from(base_budget * tick_scale > cap);
                result.troops_remaining = attack.troops;
            }
            engine.execute_combat_using(frontier_counter, pacing_multiplier);
            result.captures += engine
                .state
                .events
                .iter()
                .filter(|event| matches!(event, crate::game::GameEvent::TileCaptured { .. }))
                .count() as u64;
        }
        result
    }

    #[test]
    fn attack_frontier_size_experiment_compares_queue_and_unique_tiles() {
        for target_owner in [0, 2] {
            for troops in [2_500.0, 25_000.0, 100_000.0] {
                for frontier_size in [6, 24, 96] {
                    let mut queue_result = PacingExperimentResult::default();
                    let mut unique_result = PacingExperimentResult::default();
                    for seed in 1..=10 {
                        let queue = run_frontier_experiment(
                            seed,
                            troops,
                            frontier_size,
                            target_owner,
                            |attack, _| attack.to_conquer.len(),
                            |_, _, _| 1.0,
                            None,
                        );
                        let unique = run_frontier_experiment(
                            seed,
                            troops,
                            frontier_size,
                            target_owner,
                            unique_valid_frontier_size,
                            |_, _, _| 1.0,
                            None,
                        );
                        queue_result.captures += queue.captures;
                        queue_result.active_ticks += queue.active_ticks;
                        queue_result.queue_entries += queue.queue_entries;
                        queue_result.unique_frontier += queue.unique_frontier;
                        queue_result.cap_ticks += queue.cap_ticks;
                        queue_result.troops_remaining += queue.troops_remaining;
                        unique_result.captures += unique.captures;
                        unique_result.active_ticks += unique.active_ticks;
                        unique_result.queue_entries += unique.queue_entries;
                        unique_result.unique_frontier += unique.unique_frontier;
                        unique_result.cap_ticks += unique.cap_ticks;
                        unique_result.troops_remaining += unique.troops_remaining;
                    }
                    let seconds = 30.0 * 10.0;
                    let avg_queue =
                        queue_result.queue_entries as f64 / queue_result.active_ticks.max(1) as f64;
                    let avg_unique = queue_result.unique_frontier as f64
                        / queue_result.active_ticks.max(1) as f64;
                    println!(
                        "frontier_experiment target={target_owner} troops={troops:.0} frontier={frontier_size} queue_tiles_per_sec={:.2} unique_tiles_per_sec={:.2} queue_entries_per_tick={avg_queue:.2} unique_per_tick={avg_unique:.2} cap_ticks_queue={}/{} cap_ticks_unique={}/{} remaining={:.1}/{:.1}",
                        queue_result.captures as f64 / seconds,
                        unique_result.captures as f64 / seconds,
                        queue_result.cap_ticks,
                        queue_result.active_ticks,
                        unique_result.cap_ticks,
                        unique_result.active_ticks,
                        queue_result.troops_remaining / 10.0,
                        unique_result.troops_remaining / 10.0,
                    );
                    assert!(queue_result.captures > 0);
                    assert!(unique_result.captures > 0);
                    assert!(queue_result.queue_entries >= queue_result.unique_frontier);
                }
            }
        }
    }

    #[test]
    fn experimental_troop_pacing_curve_hits_anchors_and_is_monotonic() {
        assert_eq!(experimental_troop_pacing_multiplier(2_500.0), 0.70);
        assert_eq!(experimental_troop_pacing_multiplier(25_000.0), 1.00);
        assert_eq!(experimental_troop_pacing_multiplier(100_000.0), 1.15);
        let first_log_midpoint = (2_500.0_f64 * 25_000.0).sqrt();
        let second_log_midpoint = (25_000.0_f64 * 100_000.0).sqrt();
        assert!((experimental_troop_pacing_multiplier(first_log_midpoint) - 0.85).abs() < 1e-12);
        assert!((experimental_troop_pacing_multiplier(second_log_midpoint) - 1.075).abs() < 1e-12);
        for pair in [
            [1_000.0, 2_500.0],
            [2_500.0, 10_000.0],
            [10_000.0, 25_000.0],
            [25_000.0, 50_000.0],
            [50_000.0, 100_000.0],
            [100_000.0, 500_000.0],
        ] {
            assert!(
                experimental_troop_pacing_multiplier(pair[0])
                    <= experimental_troop_pacing_multiplier(pair[1])
            );
        }
    }

    #[test]
    fn experimental_territory_pacing_curve_hits_anchors_and_is_monotonic() {
        assert_eq!(experimental_territory_pacing_multiplier(0.01), 0.70);
        assert_eq!(experimental_territory_pacing_multiplier(0.10), 1.00);
        assert_eq!(experimental_territory_pacing_multiplier(0.30), 1.15);
        let first_log_midpoint = (0.01_f64 * 0.10).sqrt();
        let second_log_midpoint = (0.10_f64 * 0.30).sqrt();
        assert!(
            (experimental_territory_pacing_multiplier(first_log_midpoint) - 0.85).abs() < 1e-12
        );
        assert!(
            (experimental_territory_pacing_multiplier(second_log_midpoint) - 1.075).abs() < 1e-12
        );
        for pair in [
            [0.005, 0.01],
            [0.01, 0.05],
            [0.05, 0.10],
            [0.10, 0.20],
            [0.20, 0.30],
            [0.30, 0.50],
        ] {
            assert!(
                experimental_territory_pacing_multiplier(pair[0])
                    <= experimental_territory_pacing_multiplier(pair[1])
            );
        }
    }

    #[test]
    fn attack_troop_curve_experiment_compares_against_unique_frontier_baseline() {
        for target_owner in [0, 2] {
            for troops in [2_500.0, 25_000.0, 100_000.0] {
                for frontier_size in [6, 24, 96] {
                    let mut baseline = PacingExperimentResult::default();
                    let mut curved = PacingExperimentResult::default();
                    for seed in 1..=10 {
                        let base = run_frontier_experiment(
                            seed,
                            troops,
                            frontier_size,
                            target_owner,
                            unique_valid_frontier_size,
                            |_, _, _| 1.0,
                            None,
                        );
                        let curve = run_frontier_experiment(
                            seed,
                            troops,
                            frontier_size,
                            target_owner,
                            unique_valid_frontier_size,
                            |effective_troops, _, _| {
                                experimental_troop_pacing_multiplier(effective_troops)
                            },
                            None,
                        );
                        baseline.captures += base.captures;
                        baseline.active_ticks += base.active_ticks;
                        baseline.cap_ticks += base.cap_ticks;
                        baseline.troops_remaining += base.troops_remaining;
                        curved.captures += curve.captures;
                        curved.active_ticks += curve.active_ticks;
                        curved.cap_ticks += curve.cap_ticks;
                        curved.troops_remaining += curve.troops_remaining;
                    }
                    let seconds = 30.0 * 10.0;
                    let baseline_rate = baseline.captures as f64 / seconds;
                    let curved_rate = curved.captures as f64 / seconds;
                    let change = (curved_rate / baseline_rate - 1.0) * 100.0;
                    println!(
                        "troop_curve_experiment target={target_owner} troops={troops:.0} frontier={frontier_size} multiplier={:.2} baseline_tiles_per_sec={baseline_rate:.2} curve_tiles_per_sec={curved_rate:.2} change={change:+.1}% active_ticks={}/{} cap_ticks={}/{} vs {}/{} remaining={:.1}/{:.1}",
                        experimental_troop_pacing_multiplier(troops),
                        baseline.active_ticks,
                        curved.active_ticks,
                        baseline.cap_ticks,
                        baseline.active_ticks,
                        curved.cap_ticks,
                        curved.active_ticks,
                        baseline.troops_remaining / 10.0,
                        curved.troops_remaining / 10.0,
                    );
                    assert!(baseline.captures > 0);
                    assert!(curved.captures > 0);
                }
            }
        }
    }

    #[test]
    fn attack_territory_curve_experiment_compares_against_same_map_baseline() {
        for target_owner in [0, 2] {
            for territory_share in [0.01, 0.10, 0.30] {
                for frontier_size in [6, 24, 96] {
                    let mut baseline = PacingExperimentResult::default();
                    let mut curved = PacingExperimentResult::default();
                    for seed in 1..=10 {
                        let base = run_frontier_experiment(
                            seed,
                            25_000.0,
                            frontier_size,
                            target_owner,
                            unique_valid_frontier_size,
                            |_, _, _| 1.0,
                            Some(territory_share),
                        );
                        let curve = run_frontier_experiment(
                            seed,
                            25_000.0,
                            frontier_size,
                            target_owner,
                            unique_valid_frontier_size,
                            |_, attack, state| {
                                let owned = state
                                    .player(attack.owner_id)
                                    .map_or(0, |player| player.tile_count);
                                let total_land = (state.map.width * state.map.height) as f64;
                                experimental_territory_pacing_multiplier(
                                    f64::from(owned) / total_land,
                                )
                            },
                            Some(territory_share),
                        );
                        baseline.captures += base.captures;
                        baseline.active_ticks += base.active_ticks;
                        baseline.cap_ticks += base.cap_ticks;
                        baseline.troops_remaining += base.troops_remaining;
                        curved.captures += curve.captures;
                        curved.active_ticks += curve.active_ticks;
                        curved.cap_ticks += curve.cap_ticks;
                        curved.troops_remaining += curve.troops_remaining;
                    }
                    let seconds = 30.0 * 10.0;
                    let baseline_rate = baseline.captures as f64 / seconds;
                    let curved_rate = curved.captures as f64 / seconds;
                    let change = (curved_rate / baseline_rate - 1.0) * 100.0;
                    println!(
                        "territory_curve_experiment target={target_owner} attack_troops=25000 share={:.0}% frontier={frontier_size} multiplier={:.2} baseline_tiles_per_sec={baseline_rate:.2} curve_tiles_per_sec={curved_rate:.2} change={change:+.1}% active_ticks={}/{} cap_ticks={}/{} vs {}/{} remaining={:.1}/{:.1}",
                        territory_share * 100.0,
                        experimental_territory_pacing_multiplier(territory_share),
                        baseline.active_ticks,
                        curved.active_ticks,
                        baseline.cap_ticks,
                        baseline.active_ticks,
                        curved.cap_ticks,
                        curved.active_ticks,
                        baseline.troops_remaining / 10.0,
                        curved.troops_remaining / 10.0,
                    );
                    assert!(baseline.captures > 0);
                    assert!(curved.captures > 0);
                }
            }
        }
    }

    #[test]
    fn tower_defense_keeps_height_costs() {
        let config = crate::game_config::GameConfig::default();
        assert_eq!(terrain_capture_speed(TerrainType::Land), 16.5);
        assert_eq!(terrain_capture_speed(TerrainType::Highland), 20.0);
        assert_eq!(terrain_capture_speed(TerrainType::Mountain), 25.0);
        assert_eq!(terrain_loss_multiplier(TerrainType::Land, &config), 1.0);
        assert_eq!(
            terrain_loss_multiplier(TerrainType::Highland, &config),
            config.terrain_multiplier_highland
        );
        assert_eq!(
            terrain_loss_multiplier(TerrainType::Mountain, &config),
            config.terrain_multiplier_mountain
        );
    }

    #[test]
    fn advancing_frontier_uses_the_current_tower_priority_sample() {
        let width = 8;
        let mut config = GameConfig::default();
        config.tick_rate_ms = 100.0;
        let mut state = GameState::new(77, width, width, config);
        state.phase = GamePhase::Playing;
        state.map.terrain.fill(MapTile::from_byte(0x80));
        for id in [1, 2] {
            state.register_player(Player::new_human(
                id,
                format!("P{id}"),
                [0.5; 3],
                &state.config,
            ));
        }
        for (x, y, owner) in [(2, 3, 1), (3, 3, 2), (4, 3, 2)] {
            state.set_tile_owner(x, y, owner);
        }
        state.player_mut(1).unwrap().troops = 10_000.0;
        state.player_mut(2).unwrap().troops = 1_000.0;

        let mut engine = crate::engine::SowEngine::new(state, WaterComponents::default());
        engine.add_building(Building {
            id: 1,
            owner_id: 2,
            tile_idx: 3 * width + 4,
            kind: BuildingKind::Tower,
            level: 2,
            under_construction: false,
            ticks_until_complete: 0,
        });
        engine.refresh_defense_grid();

        let current_influence = engine.defense_grid.influence(
            3,
            3,
            width,
            2,
            1,
            engine.state.seed,
            &engine.state.config,
        );
        let tile_idx = u64::from(3 * width + 3);
        let sample = crate::rng::stable_tile_sample(
            engine.state.seed,
            1,
            2,
            tile_idx,
            CAPTURE_COST_VARIATION_SALT,
        );
        let capture_cost_factor = 1.0 + (2.0 * sample - 1.0) * CAPTURE_COST_VARIATION;
        let tile_cost = (1_000.0_f64 / (5.0 * 10_000.0_f64)).clamp(0.2, 1.5)
            * terrain_capture_speed(TerrainType::Land)
            * current_influence.capture_multiplier
            * capture_cost_factor;
        let mut base_budget = 1.5;
        base_budget *= engine
            .state
            .player(1)
            .unwrap()
            .leader
            .conquest_speed_multiplier(Target::Anyone);
        engine.state.config.global_speed_multiplier = (tile_cost / base_budget) * (1.0 - 1e-8);
        let troops = 10_000.0;
        let speed = engine
            .state
            .player(1)
            .unwrap()
            .leader
            .attack_strength_multiplier(Target::Anyone);
        let max_cap = max_tiles_cap_for_troops(troops * speed, &engine.state.config);
        assert!(
            max_cap * engine.state.config.global_speed_multiplier > tile_cost,
            "the test must reach the capture-cost branch without the per-tick cap"
        );

        engine.add_attack(AttackExecution {
            landed: false,
            id: 1,
            owner_id: 1,
            target_owner: 2,
            created_tick: 0,
            troops,
            to_conquer: BinaryHeap::from([PrioritizedTile {
                priority: 0,
                insert_seq: 0,
                x: 3,
                y: 3,
            }]),
            insert_seq_counter: 0,
            rng: WyRand::new(1),
            retreating: false,
        });
        engine.execute_combat();

        assert_eq!(engine.state.map.owner_id(3, 3), 1);
        let next_tile = engine.attacks[0].to_conquer.peek().unwrap();
        assert_eq!((next_tile.x, next_tile.y), (4, 3));
        let mut rng = WyRand::new(1);
        rng.next_int(0, 1_000); // capture-budget roll
        let random = rng.next_int(0, 7) as i64;
        let base_priority = (random + 10) * 6;
        let next_influence = engine.defense_grid.influence(
            4,
            3,
            width,
            2,
            1,
            engine.state.seed,
            &engine.state.config,
        );
        assert_eq!(
            next_tile.priority,
            base_priority + next_influence.priority_bonus as i64
        );
    }

    #[test]
    fn campaign_assault_holds_one_tile_until_resistance_is_chosen() {
        let width = 8;
        let mut config = GameConfig::default();
        config.global_speed_multiplier = 10.0;
        let mut state = GameState::new(88, width, width, config);
        state.phase = GamePhase::Playing;
        state.map.terrain.fill(MapTile::from_byte(0x80));
        for id in [1, 2] {
            state.register_player(Player::new_human(
                id,
                format!("P{id}"),
                [0.5; 3],
                &state.config,
            ));
        }
        for (x, y, owner) in [(2, 3, 1), (3, 3, 2), (4, 3, 2)] {
            state.set_tile_owner(x, y, owner);
        }
        state.player_mut(1).unwrap().troops = 100_000.0;
        state.player_mut(1).unwrap().max_troops = 200_000.0;
        state.player_mut(2).unwrap().troops = 100.0;

        let mut engine = crate::engine::SowEngine::new(state, WaterComponents::default());
        engine.campaign_assault = Some(crate::engine::CampaignAssaultState {
            root_target: 2,
            team: crate::protocol::Team::Red,
            include_allies: true,
            preserve_relation: false,
            hold_last_tile: true,
            capacity_ratio: 2.0,
            interval_ticks: 100,
            next_tick: 100,
            phase_index: 1,
            attacker_ids: vec![1],
            target_ids: vec![2],
            wave_index: 0,
            cycle_released_troops: std::collections::HashMap::new(),
            target_last_dispatched_wave: std::collections::HashMap::new(),
            focus_targets: std::collections::HashMap::new(),
            reserve_spawned: false,
        });
        engine.add_attack(AttackExecution {
            landed: false,
            id: 1,
            owner_id: 1,
            target_owner: 2,
            created_tick: 0,
            troops: 100_000.0,
            to_conquer: BinaryHeap::from([
                PrioritizedTile {
                    priority: 0,
                    insert_seq: 0,
                    x: 3,
                    y: 3,
                },
                PrioritizedTile {
                    priority: 0,
                    insert_seq: 1,
                    x: 4,
                    y: 3,
                },
            ]),
            insert_seq_counter: 2,
            rng: WyRand::new(88),
            retreating: false,
        });
        engine.execute_combat();
        assert_eq!(engine.state.player(2).unwrap().tile_count, 1);
        assert!(engine.state.player(2).unwrap().alive);

        engine.campaign_assault.as_mut().unwrap().hold_last_tile = false;
        let last_tile = [(3, 3), (4, 3)]
            .into_iter()
            .find(|(x, y)| engine.state.map.owner_id(*x, *y) == 2)
            .unwrap();
        engine.add_attack(AttackExecution {
            landed: false,
            id: 2,
            owner_id: 1,
            target_owner: 2,
            created_tick: engine.state.tick,
            troops: 100_000.0,
            to_conquer: BinaryHeap::from([PrioritizedTile {
                priority: 0,
                insert_seq: 0,
                x: last_tile.0,
                y: last_tile.1,
            }]),
            insert_seq_counter: 1,
            rng: WyRand::new(89),
            retreating: false,
        });
        engine.execute_combat();
        assert!(!engine.state.player(2).unwrap().alive);
    }
}
