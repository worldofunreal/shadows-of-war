use crate::building::BuildingGrid;
use crate::diplomacy::is_traitor_active;
use crate::engine::SowEngine;
use crate::execution::{PrioritizedTile, RETREAT_PENALTY_VS_PLAYER};
use crate::game::{GameEvent, GamePhase, GameState};
use crate::game_config::max_tiles_cap_for_troops;
use crate::geo_entities::{Ability, LossWhen, Side, Source, Target};
use crate::map::TerrainType;
use crate::rng::NextIntExt;

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

/// Hero "against the odds" ability: `attacker` fights a player with more troops.
/// Shared by the strength formula and the player snapshot, so the visual and the
/// combat can never disagree.
/// `army` is the troops this attack sends, so an attack that is itself outnumbered
/// is judged by its own numbers, not by what the attacker keeps at home.
pub(crate) fn outnumbered_bonus_applies(
    state: &GameState,
    attacker: u16,
    target: u16,
    army: f64,
) -> bool {
    let Some(p) = state.player(attacker) else {
        return false;
    };
    matches!(
        p.leader.ability(),
        Ability::AttackStrength {
            against: Target::Outnumbering,
            ..
        }
    ) && target != 0
        && state
            .player(target)
            .is_some_and(|rival| rival.troops > army)
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
        Target::Outnumbering => {
            subject != 0 && owner.is_some_and(|rival| rival.troops > army)
        }
        Target::Outnumbered => {
            subject != 0 && owner.is_some_and(|rival| rival.troops < army)
        }
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
        _ => false,
    };
    if applies {
        state.announce_leader_ability(attacker, tile);
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
    })
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
    state
        .player(attacker)
        .map_or(1.0, |p| p.leader.attacker_loss_multiplier(LossWhen::Landing))
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
        .map_or(1.0, |p| p.leader.attacker_loss_multiplier(LossWhen::RecaptureWindow))
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
) -> f64 {
    let defense = state
        .player(defender)
        .map_or(1.0, |p| p.leader.city_capture_multiplier(Side::Defender));
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
    pub fn execute_combat(&mut self) {
        if self.state.phase != GamePhase::Playing {
            return;
        }
        self.release_expired_pre_defeat_hold();

        // Poka-Yoke: Collections are now strictly maintained sorted on insertion.
        let map_w = self.state.map.width;
        let map_h = self.state.map.height;

        let tick_now = self.state.tick;
        let held_target = self
            .campaign_assault
            .as_ref()
            .filter(|assault| assault.hold_last_tile)
            .map(|assault| assault.root_target);
        let pre_defeat_targets = self.campaign_pre_defeat_targets.clone();
        let mut pre_defeat_pending = self.campaign_pre_defeat_pending;

        // Rebuild only when buildings changed; each tile then queries nearby cells.
        if !self.attacks.is_empty() {
            self.refresh_defense_grid();
            self.refresh_building_grid();
        }

        // attacks are sorted on insertion

        let mut elimination_candidates = Vec::new();
        let mut to_remove = Vec::new();

        for i in 0..self.attacks.len() {
            // Need to borrow state and buildings mutably but not the entire self array
            // Actually, we can borrow state and defense grid and attacks[i] simultaneously
            let execution = &mut self.attacks[i];

            if execution.retreating {
                let refund = execution.troops.max(0.0);
                if refund > 0.0 && refund.is_finite() {
                    let factor = if execution.target_owner == 0 {
                        1.0
                    } else {
                        // The defender's leader can make retreating from its land costlier;
                        // the retreating leader's `Retreat` ability softens that loss.
                        let penalty = self.state.player(execution.target_owner).map_or(1.0, |p| {
                            targeted_multiplier(
                                &self.state,
                                execution.target_owner,
                                execution.owner_id,
                                execution.troops,
                                |t| p.leader.enemy_retreat_penalty_multiplier(t),
                            )
                        });
                        let softening = self
                            .state
                            .player(execution.owner_id)
                            .map_or(1.0, |p| p.leader.attacker_loss_multiplier(LossWhen::Retreat));
                        if penalty != 1.0 {
                            self.state
                                .announce_leader_ability(execution.target_owner, None);
                        }
                        if softening != 1.0 {
                            self.state.announce_leader_ability(execution.owner_id, None);
                        }
                        1.0 - RETREAT_PENALTY_VS_PLAYER * penalty * softening
                    };
                    let back = refund * factor;
                    if let Some(player) = self.state.player_mut(execution.owner_id) {
                        player.credit_troops_capped(back, player.max_troops);
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

            // Fast approximation of active frontier size without scanning the entire empire border
            let adjacent = (execution.to_conquer.len() as f64).max(1.0);

            let troop_strength =
                attack_strength(
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

            let mut stale_pops = 0u32;
            let max_stale_pops = 256;

            loop {
                if held_target == Some(execution.target_owner)
                    && !Self::pre_defeat_watches(&pre_defeat_targets, pre_defeat_pending, execution.target_owner)
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

                    let target_has_one_tile = execution.target_owner != 0
                        && self.state.player(execution.target_owner).is_some_and(|target| target.tile_count <= 1);
                    if target_has_one_tile
                        && Self::pre_defeat_watches(&pre_defeat_targets, pre_defeat_pending, execution.target_owner)
                    {
                        if pre_defeat_pending.is_none() {
                            pre_defeat_pending = Some(execution.target_owner);
                            self.campaign_pre_defeat_since = tick_now;
                        }
                        execution.to_conquer.push(target_tile);
                        break;
                    }

                    if defense_strength != 1.0 {
                        self.state.announce_leader_ability(
                            execution.target_owner,
                            Some((target_tile.x, target_tile.y)),
                        );
                    }

                    // Determine conquest cost for this specific tile (matching OpenFront's terrain & Tower coefficients)
                    let base_speed = terrain_capture_speed(terrain_type);

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
                        let raw_cost = (2000.0 * speed_term) / effective_troops;
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
                        ratio_clamp * base_speed * defense.capture_multiplier
                    };
                    let city_multiplier = city_capture_multiplier(
                        &self.state,
                        &self.building_grid,
                        (target_tile.x, target_tile.y),
                        execution.owner_id,
                        execution.target_owner,
                    );
                    if city_multiplier != 1.0 {
                        // The side whose city ability decided the cost is the one shown.
                        let shown = if city_multiplier > 1.0 {
                            execution.target_owner
                        } else {
                            execution.owner_id
                        };
                        self.state
                            .announce_leader_ability(shown, Some((target_tile.x, target_tile.y)));
                    }
                    let tile_cost = tile_cost * city_multiplier;

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
                    let landing_loss = landing_attacker_loss(
                        &self.state,
                        execution.owner_id,
                        execution.landed,
                    );
                    if terrain_loss != 1.0
                        || city_loss != 1.0
                        || target_loss != 1.0
                        || coast_loss != 1.0
                        || landing_loss != 1.0
                    {
                        self.state.announce_leader_ability(
                            execution.owner_id,
                            Some((target_tile.x, target_tile.y)),
                        );
                    }
                    let terrain_multiplier =
                        terrain_loss_multiplier(terrain_type, &self.state.config) * terrain_loss;
                    // Hero city-tile, coast, landing and target loss abilities scale the same per-tile cost.
                    let loss_multiplier = terrain_multiplier
                        * city_loss
                        * target_loss
                        * coast_loss
                        * landing_loss;

                    if execution.target_owner == 0 {
                        // Neutral: attacker pays constant base cost scaled by terrain multiplier
                        execution.troops -= (self.state.config.attack_cost_neutral
                            * loss_multiplier)
                            / troop_strength;
                    } else {
                        // PvP: Combat resolution
                        let mut def_loss = 0.0;
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
                        let tower_siege = self
                            .state
                            .player(execution.owner_id)
                            .map_or(1.0, |player| {
                                player.leader.tower_loss_multiplier(Side::Attacker)
                            });
                        let tower_extra =
                            (defense.attacker_loss_multiplier - 1.0) * tower_defense * tower_siege;
                        let tile = Some((target_tile.x, target_tile.y));
                        if defense.attacker_loss_multiplier > 1.0 {
                            if tower_defense != 1.0 {
                                self.state
                                    .announce_leader_ability(execution.target_owner, tile);
                            }
                            if tower_siege != 1.0 {
                                self.state.announce_leader_ability(execution.owner_id, tile);
                            }
                        }
                        let recapture = recapture_multiplier(
                            &self.state,
                            execution.owner_id,
                            execution.target_owner,
                        );
                        if recapture != 1.0 {
                            self.state.announce_leader_ability(execution.owner_id, tile);
                        }
                        let atk_loss = (self.state.config.attack_cost_enemy
                            * loss_multiplier
                            * (1.0 + tower_extra)
                            * recapture)
                            / troop_strength;

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
                    elimination_candidates.extend(
                        self.state
                            .set_tile_owner_with_eliminations(
                                target_tile.x,
                                target_tile.y,
                                execution.owner_id,
                            )
                            .into_iter()
                            .map(|(victim, x, y)| (victim, execution.owner_id, x, y)),
                    );

                    if self.state.player(execution.owner_id).is_some_and(|player| {
                        player.leader.has_resource_bonus_from(Source::Territory)
                    }) {
                        self.state.announce_leader_ability(
                            execution.owner_id,
                            Some((target_tile.x, target_tile.y)),
                        );
                    }

                    // A player whose leader can recapture opens its counterattack window when it loses a city.
                    if self.building_grid.city_covers_tile(target_tile.x, target_tile.y)
                        && let Some(victim) = self.state.player_mut(execution.target_owner)
                        && victim
                            .leader
                            .has_attacker_loss(LossWhen::RecaptureWindow)
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

        for (victim, conqueror, x, y) in elimination_candidates {
            self.eliminate_player(victim, conqueror, x, y, false);
        }
        self.campaign_pre_defeat_pending = pre_defeat_pending;
    }
}

#[cfg(test)]
mod tests {
    use super::{terrain_capture_speed, terrain_loss_multiplier};
    use crate::geo_entities::Target;
    use crate::building::Building;
    use crate::execution::{AttackExecution, PrioritizedTile};
    use crate::game::{BuildingKind, GamePhase, GameState};
    use crate::game_config::{GameConfig, max_tiles_cap_for_troops};
    use crate::map::{MapTile, TerrainType};
    use crate::player::Player;
    use crate::rng::NextIntExt;
    use crate::water_components::WaterComponents;
    use std::collections::BinaryHeap;
    use wyrand::WyRand;

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
        let tile_cost = (1_000.0_f64 / (5.0 * 10_000.0_f64)).clamp(0.2, 1.5)
            * terrain_capture_speed(TerrainType::Land)
            * current_influence.capture_multiplier;
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

        engine.add_attack(AttackExecution { landed: false,
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
        engine.add_attack(AttackExecution { landed: false,
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
        engine.add_attack(AttackExecution { landed: false,
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
