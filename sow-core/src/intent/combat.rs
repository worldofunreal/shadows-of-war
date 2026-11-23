use crate::building::DefenseGrid;
use crate::engine::SowEngine;
use crate::execution::{
    ATTACK_DIR_SCALE, AttackExecution, AttackFocus, PrioritizedTile, directional_bias, isqrt_u128,
};
use crate::game::GameState;
use crate::map::TerrainType;
use crate::protocol::*;
use crate::rng::NextIntExt;
use std::collections::BinaryHeap;
use wyrand::WyRand;

const LOCAL_BORDER_RADIUS_SQ: i64 = 32 * 32;
const MAX_COAST_BACKSTEP: i64 = 4;

fn normalize_dir_fixed(dx: i64, dy: i64) -> Option<(i32, i32)> {
    let sx = i128::from(dx) * ATTACK_DIR_SCALE as i128;
    let sy = i128::from(dy) * ATTACK_DIR_SCALE as i128;
    let mag_sq = (sx * sx + sy * sy) as u128;
    if mag_sq == 0 {
        return None;
    }
    let mag = isqrt_u128(mag_sq) as i128;
    if mag == 0 {
        return None;
    }
    Some((
        ((sx * ATTACK_DIR_SCALE as i128) / mag) as i32,
        ((sy * ATTACK_DIR_SCALE as i128) / mag) as i32,
    ))
}

/// Computes a deterministic local origin and unit direction vector from `player_id`'s border
/// toward `target_tile` (`y * map_width + x`).
pub fn compute_attack_focus(
    game: &GameState,
    player_id: u16,
    target_owner: u16,
    target_tile: u32,
) -> Option<AttackFocus> {
    let map_w = game.map.width;
    let map_h = game.map.height;
    let total_tiles = map_w.checked_mul(map_h)?;
    if map_w == 0 || target_tile >= total_tiles {
        return None;
    }
    let player = game.player(player_id)?;
    let tx = i64::from(target_tile % map_w);
    let ty = i64::from(target_tile / map_w);

    let mut best_dist_sq = i64::MAX;
    let mut best_border: Option<(i64, i64)> = None;
    for raw_idx in player.border_tiles.ones() {
        let bx = raw_idx % map_w;
        let by = raw_idx / map_w;
        let mut touches_target = false;
        game.map.for_each_neighbor(bx, by, |nx, ny| {
            if game.map.owner_id(nx, ny) == target_owner
                && game.map.terrain[game.map.ref_id(nx, ny)].is_land()
            {
                touches_target = true;
            }
        });
        if !touches_target {
            continue;
        }
        let dx = i64::from(bx) - tx;
        let dy = i64::from(by) - ty;
        let dist_sq = dx * dx + dy * dy;
        if dist_sq < best_dist_sq {
            best_dist_sq = dist_sq;
            best_border = Some((i64::from(bx), i64::from(by)));
        }
    }
    let (best_x, best_y) = best_border?;

    let mut sum_x: i64 = 0;
    let mut sum_y: i64 = 0;
    let mut count: i64 = 0;
    for raw_idx in player.border_tiles.ones() {
        let bx = i64::from(raw_idx % map_w);
        let by = i64::from(raw_idx / map_w);
        let dx = bx - best_x;
        let dy = by - best_y;
        if dx * dx + dy * dy <= LOCAL_BORDER_RADIUS_SQ {
            sum_x += bx;
            sum_y += by;
            count += 1;
        }
    }
    let (local_x, local_y) = if count > 0 {
        (sum_x / count, sum_y / count)
    } else {
        (best_x, best_y)
    };

    let edge_dx = tx - best_x;
    let edge_dy = ty - best_y;
    let local_dx = tx - local_x;
    let local_dy = ty - local_y;
    let use_local = local_dx * edge_dx + local_dy * edge_dy > 0;
    let (mut origin_x, mut origin_y, raw_dx, raw_dy) = if use_local {
        (local_x, local_y, local_dx, local_dy)
    } else {
        (best_x, best_y, edge_dx, edge_dy)
    };

    let (dir_x, dir_y) = normalize_dir_fixed(raw_dx, raw_dy)?;

    let forward_from_origin =
        ((best_x - origin_x) * i64::from(dir_x) + (best_y - origin_y) * i64::from(dir_y))
            / ATTACK_DIR_SCALE;
    if forward_from_origin < 2 {
        let mut ox = origin_x;
        let mut oy = origin_y;
        for step in 1..=MAX_COAST_BACKSTEP {
            let cx = origin_x - (i64::from(dir_x) * step) / ATTACK_DIR_SCALE;
            let cy = origin_y - (i64::from(dir_y) * step) / ATTACK_DIR_SCALE;
            if cx < 0
                || cy < 0
                || cx >= i64::from(map_w)
                || cy >= i64::from(map_h)
                || game.map.owner_id(cx as u32, cy as u32) != player_id
            {
                break;
            }
            ox = cx;
            oy = cy;
        }
        origin_x = ox;
        origin_y = oy;
    }

    Some(AttackFocus {
        origin_x: origin_x as i32,
        origin_y: origin_y as i32,
        dir_x,
        dir_y,
        target_x: tx as i32,
        target_y: ty as i32,
    })
}

pub fn merge_frontiers(
    mut a: BinaryHeap<PrioritizedTile>,
    b: BinaryHeap<PrioritizedTile>,
) -> BinaryHeap<PrioritizedTile> {
    if a.capacity() < b.len() {
        a.reserve(b.len());
    }
    a.extend(b);
    a
}

pub fn build_attack_frontier(
    game: &GameState,
    defense_grid: &DefenseGrid,
    player_id: u16,
    target_owner: u16,
    border_tiles: impl IntoIterator<Item = u32>,
    focus: Option<AttackFocus>,
    rng: &mut WyRand,
    insert_seq_counter: &mut u32,
) -> BinaryHeap<PrioritizedTile> {
    let mut to_conquer = BinaryHeap::new();
    let map_w = game.map.width;
    for raw_idx in border_tiles {
        let (bx, by) = (raw_idx % map_w, raw_idx / map_w);
        game.map.for_each_neighbor(bx, by, |nx, ny| {
            if game.map.owner_id(nx, ny) != target_owner {
                return;
            }
            if !game.map.terrain[game.map.ref_id(nx, ny)].is_land() {
                return;
            }
            let mut num_owned_by_me = 0u32;
            game.map.for_each_neighbor(nx, ny, |nnx, nny| {
                if game.map.owner_id(nnx, nny) == player_id {
                    num_owned_by_me += 1;
                }
            });

            let terrain = game.map.terrain_type(nx, ny);
            let mag_x2 = match terrain {
                TerrainType::Land => 2,
                TerrainType::Highland => 3,
                TerrainType::Mountain => 4,
                TerrainType::Water | TerrainType::Lake => 3, // Fallback, won't happen normally
            };
            let r = rng.next_int(0, 7) as i64;
            // Formula scaled by 4 to maintain quartiles in integer space
            let mut prio =
                (r + 10) * (4 - (num_owned_by_me as i64 * 2) + mag_x2) + (game.tick as i64 * 4);
            prio += defense_grid.priority_bonus(
                nx,
                ny,
                map_w,
                target_owner,
                player_id,
                game.seed,
                &game.config,
            ) as i64;
            prio += directional_bias(focus, nx, ny);

            let seq = *insert_seq_counter;
            *insert_seq_counter = insert_seq_counter.wrapping_add(1);

            to_conquer.push(PrioritizedTile {
                priority: prio,
                insert_seq: seq,
                x: nx,
                y: ny,
            });
        });
    }
    to_conquer
}

pub fn spawn_attack_for_fleet_arrival(
    engine: &mut SowEngine,
    owner_id: u16,
    target_owner: u16,
    troops: f64,
    fleet_id: u64,
    landed: bool,
    landing_tile: u32,
) {
    let scripted_assault = engine.campaign_assault.as_ref().is_some_and(|assault| {
        assault.attacker_ids.contains(&owner_id) && assault.target_ids.contains(&target_owner)
    });
    let target_allowed = target_owner == 0
        || engine.state.player(target_owner).is_some_and(|target| {
            target.alive
                && engine.state.player(owner_id).is_some_and(|owner| {
                    let same_team = owner.team.is_some() && owner.team == target.team;
                    let betrayal_open = crate::diplomacy::same_team_betrayal_allows_combat(
                        engine.state.config.game_mode == "Teams"
                            && engine.campaign_unlocks.is_none(),
                        same_team,
                        crate::diplomacy::can_retaliate_against(owner),
                        crate::diplomacy::can_retaliate_against(target),
                    );
                    scripted_assault
                        || (!(owner.alliances.contains(&target_owner)
                            || target.alliances.contains(&owner_id))
                            && (!same_team || betrayal_open))
                })
        });
    if !target_allowed || troops < engine.state.config.attack_cost_neutral || !troops.is_finite() {
        if let Some(p) = engine.state.player_mut(owner_id) {
            p.credit_troops_capped(troops, p.max_troops);
        }
        return;
    }

    let exec_seed = engine
        .state
        .seed
        .wrapping_add(engine.state.tick)
        .wrapping_add(fleet_id)
        .wrapping_add(0xB04F_0000);
    let mut rng = WyRand::new(exec_seed);
    let mut initial_seq = 0u32;
    engine.refresh_defense_grid();
    if engine.state.player(owner_id).is_none() {
        return;
    }
    let fresh = build_attack_frontier(
        &engine.state,
        &engine.defense_grid,
        owner_id,
        target_owner,
        std::iter::once(landing_tile),
        None,
        &mut rng,
        &mut initial_seq,
    );

    if fresh.is_empty() {
        if let Some(p) = engine.state.player_mut(owner_id) {
            p.credit_troops_capped(troops, p.max_troops);
        }
        return;
    }

    let exec_id = engine.state.next_attack_id;
    engine.state.next_attack_id = engine.state.next_attack_id.wrapping_add(1).max(1);
    let created_tick = engine.state.tick;

    engine.add_attack(AttackExecution {
        landed,
        id: exec_id,
        owner_id,
        target_owner,
        created_tick,
        troops,
        to_conquer: fresh,
        insert_seq_counter: initial_seq,
        rng,
        retreating: false,
        focus: None,
    });
}

impl SowEngine {
    pub(super) fn apply_attack_intent(
        &mut self,
        player_id: u16,
        attack: &AttackIntent,
        intent_index: u32,
    ) {
        self.apply_attack_intent_inner(player_id, attack, intent_index, false);
    }

    pub(crate) fn apply_campaign_assault_attack(
        &mut self,
        player_id: u16,
        attack: &AttackIntent,
        intent_index: u32,
    ) {
        self.apply_attack_intent_inner(player_id, attack, intent_index, true);
    }

    fn apply_attack_intent_inner(
        &mut self,
        player_id: u16,
        attack: &AttackIntent,
        intent_index: u32,
        allow_same_team: bool,
    ) {
        let target_owner = attack.target_owner;

        let Some(player) = self.state.player(player_id) else {
            log::debug!("apply_attack_intent: player {} not found", player_id);
            return;
        };

        if !player.alive {
            return;
        }

        if target_owner == player_id {
            return;
        }

        if !allow_same_team
            && target_owner != 0
            && let Some(target) = self.state.player(target_owner)
            && player.team.is_some()
            && player.team == target.team
            && !crate::diplomacy::same_team_betrayal_allows_combat(
                self.state.config.game_mode == "Teams" && self.campaign_unlocks.is_none(),
                player.team.is_some() && player.team == target.team,
                crate::diplomacy::can_retaliate_against(player),
                crate::diplomacy::can_retaliate_against(target),
            )
        {
            return;
        }

        let pool_cap = player.troops;
        let requested = attack.troops.unwrap_or(pool_cap).max(0.0).min(pool_cap);

        if requested < self.state.config.attack_cost_neutral {
            return;
        }

        let Some(p) = self.state.player_mut(player_id) else {
            return;
        };
        p.troops -= requested;
        p.troops = p.troops.max(0.0);

        let mut remaining = requested;

        // Mutual annihilation with enemy attacks that target us (deterministic by enemy exec id).
        let mut enemy_hits: Vec<usize> = (0..self.attacks.len())
            .filter(|&i| {
                let ex = &self.attacks[i];
                ex.owner_id == target_owner && ex.target_owner == player_id && !ex.retreating
            })
            .collect();
        enemy_hits.sort_by_key(|&i| self.attacks[i].id);

        let capture_rate = crate::execution::hero_effects::mounted_capture_rate(
            &self.state,
            player_id,
            target_owner,
        );
        let mut recruits = 0.0;
        let mut capture_tile = None;
        // Hero combat abilities count in the clash too, not only in tile combat.
        for &e_enemy in &enemy_hits {
            if remaining < self.state.config.attack_cost_neutral {
                break;
            }
            if self.attacks[e_enemy].troops < self.state.config.attack_cost_enemy {
                continue;
            }
            let my_strength = crate::execution::combat::attack_strength(
                &self.state,
                player_id,
                target_owner,
                remaining,
            );
            let enemy_strength = crate::execution::combat::attack_strength(
                &self.state,
                target_owner,
                player_id,
                self.attacks[e_enemy].troops,
            );
            let enemy_ex = &mut self.attacks[e_enemy];
            let clash = (enemy_ex.troops * enemy_strength).min(remaining * my_strength);
            let defeated = clash / enemy_strength;
            enemy_ex.troops -= defeated;
            enemy_ex.troops = enemy_ex.troops.max(0.0);
            remaining = (remaining - clash / my_strength).max(0.0);
            if capture_rate > 0.0 && defeated > 0.0 {
                recruits += defeated * capture_rate;
                if capture_tile.is_none() {
                    capture_tile = enemy_ex.to_conquer.peek().map(|tile| (tile.x, tile.y));
                }
            }
        }
        if recruits > 0.0
            && let Some(p) = self.state.player_mut(player_id)
        {
            let recruits = recruits.min((p.max_troops - p.troops).max(0.0));
            if recruits > 0.0 {
                p.credit_troops(recruits);
                self.state
                    .announce_leader_ability_to(player_id, &[target_owner], capture_tile);
            }
        }

        let mut to_remove = Vec::new();
        for &e_enemy in &enemy_hits {
            if self.attacks[e_enemy].troops < self.state.config.attack_cost_neutral {
                to_remove.push(e_enemy);
            }
        }

        // Sort reverse so we can remove safely
        to_remove.sort_unstable_by(|a, b| b.cmp(a));
        let has_removals = !to_remove.is_empty();
        for i in to_remove {
            self.attacks.swap_remove(i);
        }
        if has_removals {
            self.attacks.sort_unstable_by_key(|a| a.id);
            self.ai_attack_index_dirty = true;
        }

        if remaining < self.state.config.attack_cost_neutral {
            if let Some(p) = self.state.player_mut(player_id) {
                p.credit_troops_capped(remaining, p.max_troops);
            }
            return;
        }

        crate::execution::combat::announce_attack_abilities(
            &mut self.state,
            player_id,
            target_owner,
            remaining,
            None,
        );

        self.refresh_defense_grid();
        let focus = attack
            .target_tile
            .and_then(|tile_idx| compute_attack_focus(&self.state, player_id, target_owner, tile_idx));
        let mut rng = WyRand::new(
            self.state
                .seed
                .wrapping_add(self.state.tick)
                .wrapping_add(intent_index as u64),
        );
        let mut initial_seq = 0;
        let player = self.state.player(player_id).unwrap();
        let fresh = build_attack_frontier(
            &self.state,
            &self.defense_grid,
            player_id,
            target_owner,
            player.border_tiles.ones(),
            focus,
            &mut rng,
            &mut initial_seq,
        );

        if fresh.is_empty() {
            if let Some(p) = self.state.player_mut(player_id) {
                p.credit_troops_capped(remaining, p.max_troops);
            }
            return;
        }

        // Merge into existing outgoing attack to same target (lowest exec id wins).
        let mut merge_idx = None;
        for (i, ex) in self.attacks.iter().enumerate() {
            if ex.owner_id == player_id && ex.target_owner == target_owner && !ex.retreating {
                merge_idx = Some(i);
                break;
            }
        }

        if let Some(i) = merge_idx {
            let ex = &mut self.attacks[i];
            if let Some(new_focus) = focus {
                ex.retarget_focus(new_focus);
            }
            ex.absorb_troops(remaining, false);
            let merged = merge_frontiers(std::mem::take(&mut ex.to_conquer), fresh);
            ex.to_conquer = merged;
            return;
        }

        if let Some(frontier) = fresh.peek() {
            remaining +=
                self.hero_equip_allied_attack(player_id, target_owner, (frontier.x, frontier.y));
        }
        let exec_id = self.state.next_attack_id;
        self.state.next_attack_id = self.state.next_attack_id.wrapping_add(1).max(1);
        let created_tick = self.state.tick;
        self.add_attack(AttackExecution {
            landed: false,
            id: exec_id,
            owner_id: player_id,
            target_owner,
            created_tick,
            troops: remaining,
            to_conquer: fresh,
            insert_seq_counter: initial_seq,
            rng,
            retreating: false,
            focus,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{build_attack_frontier, spawn_attack_for_fleet_arrival};
    use crate::building::{Building, DEFENSE_GRID_CELL_SIZE, DefenseGrid};
    use crate::game::{BuildingKind, GamePhase, GameState};
    use crate::game_config::GameConfig;
    use crate::map::MapTile;
    use crate::player::Player;
    use crate::protocol::{StampedIntent, Team};
    use crate::rng::NextIntExt;
    use crate::water_components::WaterComponents;
    use wyrand::WyRand;

    #[test]
    fn initial_frontier_uses_the_same_tower_priority_sample() {
        let width = 8;
        let mut game = GameState::new(77, width, width, GameConfig::default());
        game.phase = GamePhase::Playing;
        game.map.terrain.fill(MapTile::from_byte(0x80));
        for id in [1, 2] {
            game.register_player(Player::new_human(
                id,
                format!("P{id}"),
                [0.5; 3],
                &game.config,
            ));
        }
        game.set_tile_owner(2, 3, 1);
        game.set_tile_owner(3, 3, 2);

        let tower = Building {
            id: 1,
            owner_id: 2,
            tile_idx: 3 * width + 3,
            kind: BuildingKind::Tower,
            level: 2,
            under_construction: false,
            ticks_until_complete: 0,
        };
        let mut grid = DefenseGrid::default();
        grid.rebuild(&[tower], width, width, DEFENSE_GRID_CELL_SIZE, &game.config);
        let border = game.player(1).unwrap().border_tiles.clone();
        let mut rng = WyRand::new(91);
        let mut sequence = 0;
        let frontier =
            build_attack_frontier(&game, &grid, 1, 2, border.ones(), None, &mut rng, &mut sequence);
        assert_eq!(frontier.len(), 1);

        let influence = grid.influence(3, 3, width, 2, 1, game.seed, &game.config);
        let mut base_rng = WyRand::new(91);
        let random = base_rng.next_int(0, 7) as i64;
        let expected_base = (random + 10) * 4 + (game.tick as i64 * 4);
        assert_eq!(
            frontier.peek().unwrap().priority,
            expected_base + influence.priority_bonus as i64
        );
    }

    #[test]
    fn fleet_arrival_frontier_uses_tower_priority() {
        let width = 8;
        let mut game = GameState::new(77, width, width, GameConfig::default());
        game.phase = GamePhase::Playing;
        game.map.terrain.fill(MapTile::from_byte(0x80));
        for id in [1, 2] {
            game.register_player(Player::new_human(
                id,
                format!("P{id}"),
                [0.5; 3],
                &game.config,
            ));
        }
        game.set_tile_owner(2, 3, 1);
        game.set_tile_owner(3, 3, 2);

        let mut engine = crate::engine::SowEngine::new(game, WaterComponents::default());
        engine.add_building(Building {
            id: 1,
            owner_id: 2,
            tile_idx: 3 * width + 3,
            kind: BuildingKind::Tower,
            level: 2,
            under_construction: false,
            ticks_until_complete: 0,
        });
        let fleet_id = 9;
        spawn_attack_for_fleet_arrival(&mut engine, 1, 2, 10_000.0, fleet_id, false, 3 * width + 2);

        let frontier = &engine.attacks[0].to_conquer;
        assert_eq!(frontier.len(), 1);
        let tile = frontier.peek().unwrap();
        assert_eq!((tile.x, tile.y), (3, 3));

        let mut rng = WyRand::new(
            engine
                .state
                .seed
                .wrapping_add(engine.state.tick)
                .wrapping_add(fleet_id)
                .wrapping_add(0xB04F_0000),
        );
        let random = rng.next_int(0, 7) as i64;
        let base_priority = (random + 10) * 4 + engine.state.tick as i64 * 4;
        let influence = engine.defense_grid.influence(
            tile.x,
            tile.y,
            width,
            2,
            1,
            engine.state.seed,
            &engine.state.config,
        );
        assert_eq!(
            tile.priority,
            base_priority + influence.priority_bonus as i64
        );
    }

    /// A landed attack keeps the landing discount only while every troop in it landed. An
    /// ordinary attack merging in brings troops that never came ashore, so the flag clears.
    #[test]
    fn ordinary_attack_merging_into_landed_troops_clears_the_landing_discount() {
        let width = 8;
        let mut game = GameState::new(77, width, width, GameConfig::default());
        game.phase = GamePhase::Playing;
        game.map.terrain.fill(MapTile::from_byte(0x80));
        for id in [1, 2] {
            game.register_player(Player::new_human(
                id,
                format!("P{id}"),
                [0.5; 3],
                &game.config,
            ));
        }
        game.set_tile_owner(2, 3, 1);
        game.set_tile_owner(3, 3, 2);
        game.player_mut(1).unwrap().troops = 20_000.0;

        let mut engine = crate::engine::SowEngine::new(game, WaterComponents::default());
        spawn_attack_for_fleet_arrival(&mut engine, 1, 2, 10_000.0, 9, true, 3 * width + 2);
        assert!(engine.attacks[0].landed);

        engine.apply_intents(&[crate::protocol::StampedIntent {
            player_id: 1,
            intent: crate::protocol::GameplayIntent::Attack(crate::protocol::AttackIntent {
                target_owner: 2,
                troops: Some(5_000.0),
                target_tile: None,
            }),
        }]);

        assert_eq!(
            engine.attacks.len(),
            1,
            "the ordinary attack merges into the landed one"
        );
        assert!(!engine.attacks[0].landed);
        assert_eq!(engine.attacks[0].troops, 15_000.0);
    }

    #[test]
    fn teams_betrayal_opens_reciprocal_combat_for_the_retaliation_window() {
        let width = 8;
        let mut game = GameState::new(77, width, width, GameConfig::default());
        game.phase = GamePhase::Playing;
        game.config.game_mode = "Teams".to_string();
        game.map.terrain.fill(MapTile::from_byte(0x80));
        for id in [1, 2] {
            let mut player = Player::new_human(id, format!("P{id}"), [0.5; 3], &game.config);
            player.team = Some(Team::Red);
            player.troops = 20_000.0;
            game.register_player(player);
        }
        game.set_tile_owner(2, 3, 1);
        game.set_tile_owner(3, 3, 2);
        let mut engine = crate::engine::SowEngine::new(game, WaterComponents::default());
        let attack = |target_owner| {
            crate::protocol::GameplayIntent::Attack(crate::protocol::AttackIntent {
                target_owner,
                troops: Some(1_000.0),
                target_tile: None,
            })
        };

        engine.apply_intents(&[StampedIntent {
            player_id: 1,
            intent: attack(2),
        }]);
        assert!(engine.attacks.is_empty());

        engine.apply_intents(&[
            StampedIntent {
                player_id: 1,
                intent: crate::protocol::GameplayIntent::BreakAlliance { target_player: 2 },
            },
            StampedIntent {
                player_id: 1,
                intent: attack(2),
            },
        ]);
        assert_eq!(engine.attacks.len(), 1);
        assert_eq!(
            engine.state.player(1).unwrap().betrayal_retaliation_ticks,
            crate::diplomacy::HUMAN_BETRAYAL_RETALIATION_TICKS
        );

        let troops_before_counterattack = engine.state.player(2).unwrap().troops;
        engine.apply_intents(&[StampedIntent {
            player_id: 2,
            intent: attack(1),
        }]);
        assert!(engine.state.player(2).unwrap().troops < troops_before_counterattack);

        engine
            .state
            .player_mut(1)
            .unwrap()
            .betrayal_retaliation_ticks = 0;
        let troops_before = engine.state.player(2).unwrap().troops;
        engine.apply_intents(&[StampedIntent {
            player_id: 2,
            intent: attack(1),
        }]);
        assert_eq!(engine.state.player(2).unwrap().troops, troops_before);
    }

    #[test]
    fn team_betrayal_fleet_arrival_requires_an_active_retaliation_window() {
        let width = 8;
        let mut game = GameState::new(77, width, width, GameConfig::default());
        game.phase = GamePhase::Playing;
        game.config.game_mode = "Teams".to_string();
        game.map.terrain.fill(MapTile::from_byte(0x80));
        for id in [1, 2] {
            let mut player = Player::new_human(id, format!("P{id}"), [0.5; 3], &game.config);
            player.team = Some(Team::Red);
            player.troops = 20_000.0;
            game.register_player(player);
        }
        game.set_tile_owner(2, 3, 1);
        game.set_tile_owner(3, 3, 2);
        let mut engine = crate::engine::SowEngine::new(game, WaterComponents::default());

        spawn_attack_for_fleet_arrival(&mut engine, 1, 2, 1_000.0, 9, false, 3 * width + 2);
        assert_eq!(engine.attacks.len(), 0);

        engine
            .state
            .player_mut(1)
            .unwrap()
            .betrayal_retaliation_ticks = 1;
        spawn_attack_for_fleet_arrival(&mut engine, 1, 2, 1_000.0, 10, false, 3 * width + 2);
        assert_eq!(engine.attacks.len(), 1);
    }

    #[test]
    fn directional_wilderness_attack_expands_toward_tapped_tile() {
        let width = 64;
        let height = 64;
        let mut game = GameState::new(77, width, height, GameConfig::default());
        game.phase = GamePhase::Playing;
        game.map.terrain.fill(MapTile::from_byte(0x80));
        let mut player = Player::new_human(1, "P1".to_string(), [0.5; 3], &game.config);
        player.troops = 50_000.0;
        game.register_player(player);
        for y in 29..=34 {
            for x in 29..=34 {
                game.set_tile_owner(x, y, 1);
            }
        }
        let mut engine = crate::engine::SowEngine::new(game, WaterComponents::default());
        // Tap due East of player territory
        let target_tile = 31 * width + 55;
        engine.apply_intents(&[StampedIntent {
            player_id: 1,
            intent: crate::protocol::GameplayIntent::Attack(crate::protocol::AttackIntent {
                target_owner: 0,
                troops: Some(25_000.0),
                target_tile: Some(target_tile),
            }),
        }]);
        assert!(engine.attacks[0].focus.is_some());

        for _ in 0..10 {
            engine.tick();
        }
        let east_gains: usize = (27..=36)
            .flat_map(|y| (35..50).map(move |x| (x, y)))
            .filter(|&(x, y)| engine.state.map.owner_id(x, y) == 1)
            .count();
        let west_gains: usize = (27..=36)
            .flat_map(|y| (15..29).map(move |x| (x, y)))
            .filter(|&(x, y)| engine.state.map.owner_id(x, y) == 1)
            .count();
        assert!(
            east_gains > west_gains * 2 && east_gains >= 6,
            "expected directional East expansion (east={east_gains}, west={west_gains})"
        );
    }

    #[test]
    fn directional_attack_on_rival_player_focuses_tapped_flank_and_merges_retarget() {
        let width = 48;
        let height = 48;
        let mut game = GameState::new(91, width, height, GameConfig::default());
        game.phase = GamePhase::Playing;
        game.map.terrain.fill(MapTile::from_byte(0x80));
        for id in [1, 2] {
            let mut p = Player::new_human(id, format!("P{id}"), [0.5; 3], &game.config);
            p.troops = 50_000.0;
            game.register_player(p);
        }
        // Enemy island in the middle surrounded by player 1 on North and South flanks.
        for y in 18..=29 {
            for x in 18..=29 {
                game.set_tile_owner(x, y, 2);
            }
        }
        for y in 14..=17 {
            for x in 14..=33 {
                game.set_tile_owner(x, y, 1);
            }
        }
        for y in 30..=33 {
            for x in 14..=33 {
                game.set_tile_owner(x, y, 1);
            }
        }
        let mut engine = crate::engine::SowEngine::new(game, WaterComponents::default());
        // Tap South flank of player 2 first
        engine.apply_intents(&[StampedIntent {
            player_id: 1,
            intent: crate::protocol::GameplayIntent::Attack(crate::protocol::AttackIntent {
                target_owner: 2,
                troops: Some(2_000.0),
                target_tile: Some(28 * width + 24),
            }),
        }]);
        for _ in 0..4 {
            engine.tick();
        }
        let south_captured = (26..=29)
            .flat_map(|y| (18..=29).map(move |x| (x, y)))
            .filter(|&(x, y)| engine.state.map.owner_id(x, y) == 1)
            .count();
        let north_captured = (18..=21)
            .flat_map(|y| (18..=29).map(move |x| (x, y)))
            .filter(|&(x, y)| engine.state.map.owner_id(x, y) == 1)
            .count();
        assert!(
            south_captured > north_captured,
            "expected South flank priority (south={south_captured}, north={north_captured})"
        );

        // Redirect active attack to North flank via merge
        let prev_focus = engine.attacks[0].focus;
        engine.apply_intents(&[StampedIntent {
            player_id: 1,
            intent: crate::protocol::GameplayIntent::Attack(crate::protocol::AttackIntent {
                target_owner: 2,
                troops: Some(5_000.0),
                target_tile: Some(19 * width + 24),
            }),
        }]);
        assert_ne!(engine.attacks[0].focus, prev_focus);
        let north_before = (18..=21)
            .flat_map(|y| (18..=29).map(move |x| (x, y)))
            .filter(|&(x, y)| engine.state.map.owner_id(x, y) == 1)
            .count();
        for _ in 0..8 {
            engine.tick();
        }
        let north_after = (18..=21)
            .flat_map(|y| (18..=29).map(move |x| (x, y)))
            .filter(|&(x, y)| engine.state.map.owner_id(x, y) == 1)
            .count();
        assert!(
            north_after > north_before,
            "expected redirected attack to advance on North flank"
        );
    }

    #[test]
    fn directional_attack_multi_island_and_bounds_guard() {
        let width = 128;
        let height = 32;
        let mut game = GameState::new(7, width, height, GameConfig::default());
        game.phase = GamePhase::Playing;
        game.map.terrain.fill(MapTile::from_byte(0x80));
        let mut player = Player::new_human(1, "P1".to_string(), [0.5; 3], &game.config);
        player.troops = 50_000.0;
        game.register_player(player);
        // Island A (west)
        for y in 12..=18 {
            for x in 8..=14 {
                game.set_tile_owner(x, y, 1);
            }
        }
        // Island B (east)
        for y in 12..=18 {
            for x in 96..=102 {
                game.set_tile_owner(x, y, 1);
            }
        }

        assert!(super::compute_attack_focus(&game, 1, 0, u32::MAX).is_none());

        let mut engine = crate::engine::SowEngine::new(game, WaterComponents::default());
        // Tap East of Island B
        engine.apply_intents(&[StampedIntent {
            player_id: 1,
            intent: crate::protocol::GameplayIntent::Attack(crate::protocol::AttackIntent {
                target_owner: 0,
                troops: Some(15_000.0),
                target_tile: Some(15 * width + 115),
            }),
        }]);
        for _ in 0..6 {
            engine.tick();
        }
        let island_a_gains: usize = (10..=20)
            .flat_map(|y| (1..22).map(move |x| (x, y)))
            .filter(|&(x, y)| engine.state.map.owner_id(x, y) == 1)
            .count()
            .saturating_sub(49);
        let island_b_gains: usize = (10..=20)
            .flat_map(|y| (89..120).map(move |x| (x, y)))
            .filter(|&(x, y)| engine.state.map.owner_id(x, y) == 1)
            .count()
            .saturating_sub(49);
        assert!(
            island_b_gains > island_a_gains,
            "expected Island B expansion (b={island_b_gains}, a={island_a_gains})"
        );
    }
}
