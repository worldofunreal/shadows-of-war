use crate::engine::SowEngine;
use crate::building::DefenseGrid;
use crate::execution::{AttackExecution, PrioritizedTile};
use crate::game::GameState;
use crate::map::TerrainType;
use crate::protocol::*;
use crate::rng::NextIntExt;
use std::collections::BinaryHeap;
use wyrand::WyRand;
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
    border_tiles: &crate::bitset::DenseBitSet,
    rng: &mut WyRand,
    insert_seq_counter: &mut u32,
) -> BinaryHeap<PrioritizedTile> {
    let mut to_conquer = BinaryHeap::new();
    let map_w = game.map.width;
    for raw_idx in border_tiles.ones() {
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

pub fn spawn_or_merge_attack_for_fleet_arrival_pure(
    engine: &mut SowEngine,
    owner_id: u16,
    target_owner: u16,
    troops: f64,
    fleet_id: u64,
) {
    if troops < engine.state.config.attack_cost_neutral || !troops.is_finite() {
        if let Some(p) = engine.state.player_mut(owner_id) {
            p.troops = (p.troops + troops.max(0.0)).min(p.max_troops);
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
    let Some(player) = engine.state.player(owner_id) else {
        return;
    };
    let fresh = build_attack_frontier(
        &engine.state,
        &engine.defense_grid,
        owner_id,
        target_owner,
        &player.border_tiles,
        &mut rng,
        &mut initial_seq,
    );

    if fresh.is_empty() {
        if let Some(p) = engine.state.player_mut(owner_id) {
            p.troops = (p.troops + troops).min(p.max_troops);
        }
        return;
    }

    let mut merge_idx = None;
    for (i, ex) in engine.attacks.iter().enumerate() {
        if ex.owner_id == owner_id && ex.target_owner == target_owner && !ex.retreating {
            merge_idx = Some(i);
            break;
        }
    }

    if let Some(i) = merge_idx {
        let ex = &mut engine.attacks[i];
        ex.troops += troops;
        let merged = merge_frontiers(std::mem::take(&mut ex.to_conquer), fresh);
        ex.to_conquer = merged;
        return;
    }

    let exec_id = engine.state.next_attack_id;
    engine.state.next_attack_id = engine.state.next_attack_id.wrapping_add(1).max(1);
    let created_tick = engine.state.tick;

    engine.add_attack(AttackExecution {
        id: exec_id,
        owner_id,
        target_owner,
        created_tick,
        troops,
        to_conquer: fresh,
        insert_seq_counter: initial_seq,
        rng,
        retreating: false,
    });
}

impl SowEngine {
    pub(super) fn apply_attack_intent(
        &mut self,
        player_id: u16,
        attack: &AttackIntent,
        intent_index: u32,
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

        if target_owner != 0
            && let Some(target) = self.state.player(target_owner)
            && player.team.is_some()
            && player.team == target.team
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

        for &e_enemy in &enemy_hits {
            if remaining < self.state.config.attack_cost_neutral {
                break;
            }
            let enemy_ex = &mut self.attacks[e_enemy];

            if enemy_ex.troops < self.state.config.attack_cost_enemy {
                continue;
            }
            let clash = enemy_ex.troops.min(remaining);
            enemy_ex.troops -= clash;
            enemy_ex.troops = enemy_ex.troops.max(0.0);
            remaining -= clash;
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
                p.troops = (p.troops + remaining).min(p.max_troops);
            }
            return;
        }

        self.refresh_defense_grid();
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
            &player.border_tiles,
            &mut rng,
            &mut initial_seq,
        );

        if fresh.is_empty() {
            if let Some(p) = self.state.player_mut(player_id) {
                p.troops = (p.troops + remaining).min(p.max_troops);
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
            ex.troops += remaining;
            let merged = merge_frontiers(std::mem::take(&mut ex.to_conquer), fresh);
            ex.to_conquer = merged;
            return;
        }

        let exec_id = self.state.next_attack_id;
        self.state.next_attack_id = self.state.next_attack_id.wrapping_add(1).max(1);
        let created_tick = self.state.tick;
        self.add_attack(AttackExecution {
            id: exec_id,
            owner_id: player_id,
            target_owner,
            created_tick,
            troops: remaining,
            to_conquer: fresh,
            insert_seq_counter: initial_seq,
            rng,
            retreating: false,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{build_attack_frontier, spawn_or_merge_attack_for_fleet_arrival_pure};
    use crate::building::{Building, DefenseGrid, DEFENSE_GRID_CELL_SIZE};
    use crate::game::{BuildingKind, GamePhase, GameState};
    use crate::game_config::GameConfig;
    use crate::map::MapTile;
    use crate::player::Player;
    use crate::water_components::WaterComponents;
    use crate::rng::NextIntExt;
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
            kind: BuildingKind::Bunker,
            level: 2,
            under_construction: false,
            ticks_until_complete: 0,
        };
        let mut grid = DefenseGrid::default();
        grid.rebuild(&[tower], width, width, DEFENSE_GRID_CELL_SIZE, &game.config);
        let border = game.player(1).unwrap().border_tiles.clone();
        let mut rng = WyRand::new(91);
        let mut sequence = 0;
        let frontier = build_attack_frontier(
            &game,
            &grid,
            1,
            2,
            &border,
            &mut rng,
            &mut sequence,
        );
        assert_eq!(frontier.len(), 1);

        let influence = grid.influence(3, 3, width, 2, 1, game.seed, &game.config);
        let mut base_rng = WyRand::new(91);
        let random = base_rng.next_int(0, 7) as i64;
        let expected_base = (random + 10) * 4 + (game.tick as i64 * 4);
        assert_eq!(frontier.peek().unwrap().priority, expected_base + influence.priority_bonus as i64);
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
            kind: BuildingKind::Bunker,
            level: 2,
            under_construction: false,
            ticks_until_complete: 0,
        });
        let fleet_id = 9;
        spawn_or_merge_attack_for_fleet_arrival_pure(&mut engine, 1, 2, 10_000.0, fleet_id);

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
        assert_eq!(tile.priority, base_priority + influence.priority_bonus as i64);
    }
}
