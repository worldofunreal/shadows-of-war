use crate::engine::SowEngine;
use crate::execution::{PrioritizedTile, RETREAT_PENALTY_VS_PLAYER};
use crate::game::{GameEvent, GamePhase};
use crate::game_config::max_tiles_cap_for_troops;
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

// Attack execution tick (territory expansion and combat resolution)
impl SowEngine {
    pub fn execute_combat(&mut self) {
        if self.state.phase != GamePhase::Playing {
            return;
        }

        // Poka-Yoke: Collections are now strictly maintained sorted on insertion.
        let map_w = self.state.map.width;
        let map_h = self.state.map.height;

        let tick_now = self.state.tick;

        // Rebuild only when buildings changed; each tile then queries nearby cells.
        if !self.attacks.is_empty() {
            self.refresh_defense_grid();
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
                        1.0 - RETREAT_PENALTY_VS_PLAYER
                    };
                    let back = refund * factor;
                    if let Some(player) = self.state.player_mut(execution.owner_id) {
                        player.troops = (player.troops + back).min(player.max_troops);
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

            let troop_strength = self
                .state
                .player(execution.owner_id)
                .map(|p| p.leader.troop_strength_multiplier())
                .unwrap_or(1.0);
            let effective_troops = execution.troops * troop_strength;

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

            // Apply leader speed modifiers
            if let Some(player) = self.state.player(execution.owner_id) {
                match player.leader {
                    crate::player::Leader::Alexander => budget *= 1.15,
                    crate::player::Leader::Napoleon => budget *= 1.20,
                    _ => {}
                }
            }

            // Clamp budget with scaled max_cap
            let scaled_max_cap = max_cap
                * (self.state.config.tick_rate_ms as f64 / 100.0)
                * self.state.config.global_speed_multiplier;
            budget = budget.min(scaled_max_cap);

            let mut stale_pops = 0u32;
            let max_stale_pops = 256;

            loop {
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

                    // Determine conquest cost for this specific tile (matching OpenFront's terrain & Bunker coefficients)
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

                    let terrain_multiplier =
                        terrain_loss_multiplier(terrain_type, &self.state.config);

                    if execution.target_owner == 0 {
                        // Neutral: attacker pays constant base cost scaled by terrain multiplier
                        execution.troops -= (self.state.config.attack_cost_neutral
                            * terrain_multiplier)
                            / troop_strength;
                    } else {
                        // PvP: Combat resolution
                        let mut def_loss = 0.0;
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

                        // Defense posts increase attacker losses slightly
                        let atk_loss = (self.state.config.attack_cost_enemy
                            * terrain_multiplier
                            * defense.attacker_loss_multiplier)
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
                        player.troops = (player.troops + refund).min(player.max_troops);
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
    }
}

#[cfg(test)]
mod tests {
    use super::{terrain_capture_speed, terrain_loss_multiplier};
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
            kind: BuildingKind::Bunker,
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
        let tile_cost = (1_000.0_f64 / (5.0 * 10_000.0_f64))
            .clamp(0.2, 1.5)
            * terrain_capture_speed(TerrainType::Land)
            * current_influence.capture_multiplier;
        let mut base_budget = 1.5;
        match engine.state.player(1).unwrap().leader {
            crate::player::Leader::Alexander => base_budget *= 1.15,
            crate::player::Leader::Napoleon => base_budget *= 1.20,
            _ => {}
        }
        engine.state.config.global_speed_multiplier =
            (tile_cost / base_budget) * (1.0 - 1e-8);
        let troops = 10_000.0;
        let speed = engine
            .state
            .player(1)
            .unwrap()
            .leader
            .troop_strength_multiplier();
        let max_cap = max_tiles_cap_for_troops(troops * speed, &engine.state.config);
        assert!(
            max_cap * engine.state.config.global_speed_multiplier > tile_cost,
            "the test must reach the capture-cost branch without the per-tick cap"
        );

        engine.add_attack(AttackExecution {
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
}
