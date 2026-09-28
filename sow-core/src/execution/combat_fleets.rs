use crate::engine::SowEngine;
use crate::execution::refund_fleet_troops_to_player;
use crate::game::{GameEvent, GamePhase};
use crate::warp_fleet::best_shore_spawn_for_transport;

impl SowEngine {
    /// Transport ship: 1 tile/tick over water, retreat, landing → conquer + attack execution.
    pub fn execute_fleets(&mut self) {
        if self.state.phase != GamePhase::Playing {
            return;
        }

        // Spatial hash of fleets: current_tile -> fleet indices
        let mut tile_to_fleets: std::collections::HashMap<u32, Vec<usize>> =
            std::collections::HashMap::new();
        for (idx, fleet) in self.fleets.iter().enumerate() {
            if fleet.troops > 0.0 {
                tile_to_fleets
                    .entry(fleet.current_tile)
                    .or_default()
                    .push(idx);
            }
        }

        // Simple Naval Combat: Warships damage enemy fleets on the same tile or adjacent tiles
        let mut damages = Vec::new();
        let w = self.state.map.width;
        for i in 0..self.fleets.len() {
            if self.fleets[i].unit_type == crate::game::UnitType::Warship
                && self.fleets[i].troops > 0.0
            {
                let mut neighbors = [0u32; 7];
                neighbors[0] = self.fleets[i].current_tile;
                let mut n_count = 1;
                let fx = self.fleets[i].current_tile % w;
                let fy = self.fleets[i].current_tile / w;
                self.state.map.for_each_neighbor(fx, fy, |nx, ny| {
                    neighbors[n_count] = ny * w + nx;
                    n_count += 1;
                });

                'outer: for &t in neighbors.iter().take(n_count) {
                    if let Some(indices) = tile_to_fleets.get(&t) {
                        for &j in indices {
                            if i != j
                                && self.fleets[j].owner_id != self.fleets[i].owner_id
                                && self.fleets[j].troops > 0.0
                            {
                                damages.push((j, 100.0));
                                break 'outer;
                            }
                        }
                    }
                }
            }
        }
        for (target_idx, dmg) in damages {
            self.fleets[target_idx].troops -= dmg;
        }

        // fleets are sorted on insertion
        let mut to_remove = Vec::new();
        let mut elimination_candidates = Vec::new();

        for i in 0..self.fleets.len() {
            let fleet = &mut self.fleets[i];

            if fleet.troops <= 0.0 {
                to_remove.push(i);
                continue;
            }

            if fleet.retreating && fleet.retreat_dst.is_none() {
                if let Some(player) = self.state.player(fleet.owner_id) {
                    let here_comp = self.water.component_of(fleet.current_tile);
                    let comp_filter = if here_comp == 0 {
                        None
                    } else {
                        Some((&self.water, here_comp))
                    };
                    if let Some(r_dst) = best_shore_spawn_for_transport(
                        &self.state.map,
                        fleet.owner_id,
                        &player.border_tiles,
                        fleet.current_tile,
                        comp_filter,
                    ) {
                        fleet.retreat_dst = Some(r_dst);
                        if let Some(path) = self.path_scratch.astar.find_path(
                            &self.state.map,
                            &[fleet.current_tile],
                            r_dst,
                        ) {
                            fleet.path = std::sync::Arc::new(path);
                            fleet.path_cursor = 0;
                        } else {
                            refund_fleet_troops_to_player(
                                &mut self.state,
                                fleet.owner_id,
                                fleet.troops,
                            );
                            to_remove.push(i);
                            continue;
                        }
                    } else {
                        refund_fleet_troops_to_player(
                            &mut self.state,
                            fleet.owner_id,
                            fleet.troops,
                        );
                        to_remove.push(i);
                        continue;
                    }
                } else {
                    to_remove.push(i);
                    continue;
                }
            }

            if fleet.flow_target.is_none() && fleet.path.is_empty() {
                refund_fleet_troops_to_player(&mut self.state, fleet.owner_id, fleet.troops);
                to_remove.push(i);
                continue;
            }

            if let Some(target) = fleet.flow_target {
                let map = &self.state.map;
                let flow_field = self.flow_field_cache.get_or_compute(target, map);
                let dir = flow_field.directions[fleet.current_tile as usize];
                if dir < 8 {
                    let w = map.width;
                    let cx = fleet.current_tile % w;
                    let cy = fleet.current_tile / w;
                    let dx = [0, 1, 1, 1, 0, -1, -1, -1];
                    let dy = [-1, -1, 0, 1, 1, 1, 0, -1];
                    let nx = cx as i32 + dx[dir as usize];
                    let ny = cy as i32 + dy[dir as usize];
                    fleet.current_tile = ny as u32 * w + nx as u32;
                } else if dir == 8 {
                    // Reached destination!
                    fleet.path = std::sync::Arc::new(Vec::new());
                    fleet.path_cursor = 0;
                } else {
                    // Unreachable via FlowField
                    refund_fleet_troops_to_player(&mut self.state, fleet.owner_id, fleet.troops);
                    to_remove.push(i);
                    continue;
                }
            } else if fleet.path_cursor < fleet.path.len() {
                fleet.current_tile = fleet.path[fleet.path_cursor];
                fleet.path_cursor += 1;
            }

            if fleet.unit_type == crate::game::UnitType::TradeShip {
                if let Some(p) = self.state.player_mut(fleet.owner_id) {
                    p.gold += 15.0; // Passive gold generation
                }
                if fleet.path_cursor >= fleet.path.len() && !fleet.path.is_empty() {
                    // Loop back
                    let mut p = (*fleet.path).clone();
                    p.reverse();
                    fleet.path = std::sync::Arc::new(p);
                    fleet.path_cursor = 0;
                }
                continue;
            }

            if fleet.unit_type == crate::game::UnitType::Warship {
                // Stop at destination
                if fleet.path_cursor >= fleet.path.len() {
                    fleet.path = std::sync::Arc::new(Vec::new());
                    fleet.path_cursor = 0;
                }
                continue;
            }

            if fleet.path_cursor < fleet.path.len() {
                continue;
            }

            let w = self.state.map.width;
            let lx = fleet.current_tile % w;
            let ly = fleet.current_tile / w;
            let owner_here = self.state.map.owner_id(lx, ly);

            if fleet.retreating {
                if owner_here == fleet.owner_id {
                    let deaths = fleet.troops * crate::warp_fleet::FLEET_RETREAT_SHORE_MALUS;
                    let survivors = fleet.troops - deaths;
                    if let Some(p) = self.state.player_mut(fleet.owner_id) {
                        p.troops = (p.troops + survivors).min(p.max_troops);
                    }
                } else {
                    refund_fleet_troops_to_player(&mut self.state, fleet.owner_id, fleet.troops);
                }
                to_remove.push(i);
                continue;
            }

            if owner_here == fleet.owner_id {
                let deaths = fleet.troops * crate::warp_fleet::FLEET_RETREAT_SHORE_MALUS;
                let survivors = fleet.troops - deaths;
                if let Some(p) = self.state.player_mut(fleet.owner_id) {
                    p.troops = (p.troops + survivors).min(p.max_troops);
                }
                to_remove.push(i);
                continue;
            }

            let prev_owner = owner_here;
            let f_owner = fleet.owner_id;
            let f_troops = fleet.troops;
            let f_id = fleet.id;

            elimination_candidates.extend(
                self.state
                    .set_tile_owner_with_eliminations(lx, ly, f_owner)
                    .into_iter()
                    .map(|(victim, x, y)| (victim, f_owner, x, y)),
            );
            self.state.events.push(GameEvent::TileCaptured {
                x: lx,
                y: ly,
                new_owner: f_owner,
                previous_owner: prev_owner,
                troops: f_troops,
            });

            // Drop mutable borrow of self.fleets before calling method on self
            crate::intent::spawn_or_merge_attack_for_fleet_arrival_pure(
                self, f_owner, prev_owner, f_troops, f_id,
            );
            to_remove.push(i);
        }

        // Remove finished fleets in O(1) and re-sort to preserve deterministic order
        let has_removals = !to_remove.is_empty();
        for i in to_remove.into_iter().rev() {
            self.fleets.swap_remove(i);
        }
        if has_removals {
            self.fleets.sort_unstable_by_key(|f| f.id);
        }

        for (victim, conqueror, x, y) in elimination_candidates {
            self.eliminate_player(victim, conqueror, x, y, false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SowEngine;
    use crate::execution::{AttackExecution, PrioritizedTile, fractional_extra_tiles_milli};
    use crate::game::{GameEvent, GameState, UnitType};
    use crate::game_config::GameConfig;
    use crate::map::MapTile;
    use crate::player::Player;
    use crate::protocol::Team;
    use crate::warp_fleet::WarpFleet;
    use crate::water_components::WaterComponents;
    use std::collections::BinaryHeap;

    #[test]
    fn landing_on_last_tile_records_one_defeat_for_the_fleet_owner() {
        for has_other_tile in [false, true] {
            let mut state = GameState::new(
                1,
                5,
                3,
                GameConfig {
                    random_spawn: true,
                    ..Default::default()
                },
            );
            state.map.terrain.fill(MapTile::from_byte(0x20));
            for id in [1, 2] {
                let mut player = Player::new_human(id, format!("P{id}"), [1.0; 3], &state.config);
                player.gold = 0.0;
                player.has_spawned = true;
                state.register_player(player);
            }
            for (x, y, owner) in [(0, 1, 1), (4, 1, 2)] {
                state.map.terrain[(y * 5 + x) as usize] = MapTile::from_byte(0xC0);
                state.set_tile_owner(x, y, owner);
            }
            if has_other_tile {
                state.map.terrain[14] = MapTile::from_byte(0xC0);
                state.set_tile_owner(4, 2, 2);
            }
            let mut engine = SowEngine::new(state, WaterComponents::default());
            // The route is already at its final shore tile; exercise the real landing path.
            engine.add_fleet(WarpFleet::new(
                1,
                1,
                2,
                UnitType::TransportShip,
                50.0,
                (5, 9),
                vec![9],
            ));
            engine.execute_fleets();
            let gold_after_landing = engine.state.player(1).unwrap().gold;
            engine.execute_fleets();

            let killer = engine.state.player(1).unwrap();
            let victim = engine.state.player(2).unwrap();
            assert_eq!(engine.state.map.owner_id(4, 1), 1);
            assert_eq!(victim.alive, has_other_tile);
            assert_eq!(victim.deaths, u32::from(!has_other_tile));
            assert_eq!(killer.kills, u32::from(!has_other_tile));
            assert_eq!(killer.gold, gold_after_landing);
            assert_eq!(killer.gold > 0.0, !has_other_tile);
            let defeats = engine
                .state
                .events
                .iter()
                .filter_map(|event| match event {
                    GameEvent::PlayerEliminated {
                        player_id,
                        conqueror_id,
                        elimination_x,
                        elimination_y,
                        ..
                    } => Some((*player_id, *conqueror_id, *elimination_x, *elimination_y)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(
                defeats,
                if has_other_tile {
                    vec![]
                } else {
                    vec![(2, 1, 4, 1)]
                }
            );
            assert!(engine.fleets.is_empty());
        }
    }

    #[test]
    fn enclosure_eliminations_are_credited_once_and_never_capture_teammates() {
        for by_fleet in [false, true] {
            for same_team in [false, true] {
                let mut state = GameState::new(
                    1,
                    5,
                    5,
                    GameConfig {
                        random_spawn: true,
                        global_speed_multiplier: 100.0,
                        ..Default::default()
                    },
                );
                state.map.terrain.fill(MapTile::from_byte(0x20));

                for (id, team) in [
                    (1, Team::Blue),
                    (2, if same_team { Team::Blue } else { Team::Red }),
                ] {
                    let mut player =
                        Player::new_human(id, format!("P{id}"), [1.0; 3], &state.config);
                    player.gold = 0.0;
                    player.has_spawned = true;
                    player.team = Some(team);
                    state.register_player(player);
                }

                // P1 surrounds P2 except for one neutral land tile to be captured.
                for y in 1..=3 {
                    for x in 1..=3 {
                        if (x, y) != (2, 1) && (x, y) != (2, 2) {
                            state.map.terrain[(y * 5 + x) as usize] = MapTile::from_byte(0xC0);
                            state.set_tile_owner(x, y, 1);
                        }
                    }
                }
                state.map.terrain[2 * 5 + 2] = MapTile::from_byte(0xC0);
                state.set_tile_owner(2, 2, 2);
                state.map.terrain[1 * 5 + 2] = MapTile::from_byte(0xC0); // (x=2, y=1): row-major

                let mut engine = SowEngine::new(state, WaterComponents::default());
                if by_fleet {
                    // The fleet is already at the neutral gap's landing tile.
                    engine.add_fleet(WarpFleet::new(
                        1,
                        1,
                        0,
                        UnitType::TransportShip,
                        100.0,
                        (5, 7),
                        vec![7],
                    ));
                    engine.execute_fleets();
                    engine.execute_fleets();
                } else {
                    engine.add_attack(AttackExecution {
                        id: 1,
                        owner_id: 1,
                        target_owner: 0,
                        troops: 100_000.0,
                        to_conquer: BinaryHeap::from([PrioritizedTile {
                            priority: 0,
                            insert_seq: 0,
                            x: 2,
                            y: 1,
                        }]),
                        insert_seq_counter: 1,
                        rng: wyrand::WyRand::new(1),
                        retreating: false,
                    });
                    engine.execute_combat();
                    engine.execute_combat();
                }

                let eliminated = !same_team;
                assert_eq!(engine.state.map.owner_id(2, 1), 1);
                assert_eq!(engine.state.map.owner_id(2, 2), if eliminated { 1 } else { 2 });
                let attacker = engine.state.player(1).unwrap();
                let victim = engine.state.player(2).unwrap();
                assert_eq!(victim.alive, !eliminated);
                assert_eq!(victim.deaths, u32::from(eliminated));
                assert_eq!(attacker.kills, u32::from(eliminated));
                assert_eq!(attacker.gold > 0.0, eliminated);

                let defeats = engine
                    .state
                    .events
                    .iter()
                    .filter_map(|event| match event {
                        GameEvent::PlayerEliminated {
                            player_id,
                            conqueror_id,
                            elimination_x,
                            elimination_y,
                            ..
                        } => Some((*player_id, *conqueror_id, *elimination_x, *elimination_y)),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                assert_eq!(
                    defeats,
                    if eliminated {
                        vec![(2, 1, 2, 2)]
                    } else {
                        vec![]
                    },
                    "by_fleet={by_fleet}, same_team={same_team}"
                );
            }
        }
    }

    #[test]
    fn fractional_extra_tile_milli_threshold_is_stable() {
        // 12.499 -> threshold 499 (roll 498 adds one; 499 does not)
        assert_eq!(fractional_extra_tiles_milli(12.499, 498), 1);
        assert_eq!(fractional_extra_tiles_milli(12.499, 499), 0);
        // Very small fraction below one milli should never add.
        assert_eq!(fractional_extra_tiles_milli(7.0009, 0), 0);
        // Near-one fraction clamps to 999 max threshold.
        assert_eq!(fractional_extra_tiles_milli(3.999_999_999, 998), 1);
        assert_eq!(fractional_extra_tiles_milli(3.999_999_999, 999), 0);
    }
}
