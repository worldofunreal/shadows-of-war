use crate::building::aggregate_buildings_per_player;
use crate::engine::SowEngine;
use crate::execution::income_rates::{
    gold_income_per_second, trade_income_per_second, troop_income_per_second,
};
use crate::game::GamePhase;

impl SowEngine {
    pub fn execute_income(&mut self) {
        if self.state.phase != GamePhase::Playing {
            return;
        }

        if self.building_aggregates_dirty {
            let max_pid = self
                .state
                .players
                .iter()
                .map(|p| p.id as usize)
                .max()
                .unwrap_or(0);
            self.building_aggregates =
                aggregate_buildings_per_player(self.buildings.iter().copied(), max_pid);
            self.building_aggregates_dirty = false;
        }

        if self.sea_lanes_dirty {
            crate::sea_lane::update_sea_lanes(self);
            self.sea_lanes_dirty = false;
        }
        let aggs = self.building_aggregates.clone();

        let config = self.state.config.clone();
        let num_players = self.state.players.len();
        for idx in 0..num_players {
            if !self.state.players[idx].alive {
                continue;
            }
            let tiles_owned = self.state.players[idx].tile_count;
            if tiles_owned == 0 {
                self.state.players[idx].alive = false;
                continue;
            }

            let player_id = self.state.players[idx].id;
            let agg = aggs.get(player_id as usize).copied().unwrap_or_default();
            let safe_troops = self.state.players[idx].troops.max(0.0);

            let t_f64 = tiles_owned as f64;
            let t_half = t_f64.sqrt();
            let t_quarter = t_half.sqrt();
            let t_eighth = t_quarter.sqrt();
            let max_troops_bonus = t_half * t_eighth;

            // Tribes (PlayerType::Bot) always eat the standard-bot handicap —
            // no id-based carve-out (that used to let a handful of "élite"
            // tribes dodge it by id%100, inverting the food chain).
            let is_standard_bot =
                self.state.players[idx].player_type == crate::player::PlayerType::Bot;

            let leader = self.state.players[idx].leader;
            let richard_mult = if leader == crate::player::Leader::RichardTheLionheart {
                1.50
            } else {
                1.0
            };
            let mut max_tr = config.max_troops_base
                + max_troops_bonus * config.max_troops_scale
                + agg.city_levels as f64 * config.city_max_troops * richard_mult;
            if is_standard_bot {
                max_tr /= 1.5;
            }
            if let Some(cap) = self.state.players[idx].max_troops_cap {
                max_tr = max_tr.min(cap);
            }
            if let Some(cap) = self.campaign_assault_troop_caps.get(&player_id) {
                max_tr = cap.max(0.0);
            }
            self.state.players[idx].max_troops = max_tr;

            let troop_ps = troop_income_per_second(tiles_owned, agg, leader, &config);
            let mut troop_income = config.per_tick(troop_ps);

            if is_standard_bot {
                troop_income *= 0.75;
            }
            self.state.players[idx].troops = (safe_troops + troop_income).min(max_tr);

            let gold_ps = gold_income_per_second(tiles_owned, agg, leader, &config);
            let mut gold_income = config.per_tick(gold_ps);

            if is_standard_bot {
                gold_income *= 0.75;
            }
            self.state.players[idx].gold += gold_income;

            let iq_gain = config.per_tick(self.state.players[idx].iq as f64 / 100.0);
            self.state.players[idx].iq_points =
                (self.state.players[idx].iq_points + iq_gain).min(500.0);
        }

        for fleet in &self.fleets {
            if fleet.unit_type == crate::game::UnitType::TradeShip
                && let Some(player) = self.state.player_mut(fleet.owner_id)
            {
                player.gold += config.per_tick(trade_income_per_second(1, &config));
            }
        }

        self.update_debt_rebellions(&config);

        let mut tribes_needing_city = Vec::new();
        for player in self.state.players.iter().filter(|p| {
            p.alive
                && self.state.config.buildings_enabled
                && self.campaign_allows_building(crate::game::BuildingKind::City, 1)
        }) {
            let has_city = aggs
                .get(player.id as usize)
                .is_some_and(|a| a.city_levels > 0);
            let is_standard_bot = player.player_type == crate::player::PlayerType::Bot;
            let needs_city = if is_standard_bot {
                player.cities == 0
            } else {
                !has_city
            };
            if player.player_type == crate::player::PlayerType::Bot
                && needs_city
                && player.tile_count >= 150
                && (self.state.tick + player.id as u64).is_multiple_of(30)
            {
                tribes_needing_city.push((
                    player.id,
                    player.sum_x,
                    player.sum_y,
                    player.tile_count,
                ));
            }
        }

        for (tid, sum_x, sum_y, tile_count) in tribes_needing_city {
            let cx = (sum_x / tile_count as u64) as u32;
            let cy = (sum_y / tile_count as u64) as u32;
            let w = self.state.map.width;
            let mut found_tile = None;
            for dy in -5..=5 {
                for dx in -5..=5 {
                    let nx = cx as i32 + dx;
                    let ny = cy as i32 + dy;
                    if self.state.map.is_valid_coord(nx, ny) {
                        let (ux, uy) = (nx as u32, ny as u32);
                        if self.state.map.owner_id(ux, uy) == tid
                            && self.state.map.terrain[self.state.map.ref_id(ux, uy)].is_land()
                        {
                            found_tile = Some(uy * w + ux);
                            break;
                        }
                    }
                }
                if found_tile.is_some() {
                    break;
                }
            }
            if let Some(tile_idx) = found_tile {
                self.refresh_building_grid();
                let spawn_ok = crate::building::resolve_structure_spawn_tile(
                    &self.state.map,
                    tid,
                    crate::game::BuildingKind::City,
                    tile_idx,
                    &self.building_grid,
                    &mut self.placement_scratch,
                );
                if let Some(spawn_idx) = spawn_ok {
                    let building_id = self.state.next_building_id;
                    self.state.next_building_id =
                        self.state.next_building_id.wrapping_add(1).max(1);

                    self.add_building(crate::building::Building {
                        id: building_id,
                        owner_id: tid,
                        tile_idx: spawn_idx,
                        kind: crate::game::BuildingKind::City,
                        level: 1,
                        under_construction: false,
                        ticks_until_complete: 0,
                    });
                    if let Some(p) = self.state.player_mut(tid) {
                        p.cities += 1;
                    }
                }
            }
        }
    }

    fn update_debt_rebellions(&mut self, config: &crate::game_config::GameConfig) {
        let tick_ms = if config.tick_rate_ms.is_finite() && config.tick_rate_ms > 0.0 {
            config.tick_rate_ms
        } else {
            100.0
        };
        let debt_grace_ticks = (30_000.0 / tick_ms).ceil() as u64;

        for index in 0..self.state.players.len() {
            let player = &self.state.players[index];
            let player_id = player.id;
            let eligible =
                player.player_type == crate::player::PlayerType::Human && !player.is_ai_controlled;
            if !eligible || player.gold >= 0.0 || !player.alive {
                self.debt_episodes.remove(&player_id);
                continue;
            }

            let (debt_start_tick, rebelled) = *self
                .debt_episodes
                .entry(player_id)
                .or_insert((self.state.tick, false));
            if rebelled || self.state.tick.saturating_sub(debt_start_tick) < debt_grace_ticks {
                continue;
            }

            let Some(building_index) = self
                .buildings
                .iter()
                .enumerate()
                .filter(|(_, building)| {
                    building.owner_id == player_id
                        && !building.under_construction
                        && building.active_level() > 0
                })
                .max_by_key(|(_, building)| {
                    (
                        building.kind == crate::game::BuildingKind::City,
                        building.active_level(),
                        std::cmp::Reverse(building.id),
                    )
                })
                .map(|(index, _)| index)
            else {
                continue;
            };
            let Some(rebel_id) = self.state.next_free_player_id() else {
                continue;
            };
            let building = self.buildings[building_index];
            let parent_name = self.state.players[index].name.clone();
            let mut rebel = crate::player::Player::new_nation(
                rebel_id,
                format!("{} Rebels", parent_name),
                [0.88, 0.20, 0.18],
                config,
            );
            rebel.has_spawned = true;
            self.state.register_player(rebel);
            let x = building.tile_idx % self.state.map.width;
            let y = building.tile_idx / self.state.map.width;
            self.state.set_tile_owner(x, y, rebel_id);
            self.buildings[building_index].owner_id = rebel_id;
            self.building_aggregates_dirty = true;
            self.sea_lanes_dirty = true;
            self.ai_attack_index_dirty = true;
            self.debt_episodes
                .insert(player_id, (debt_start_tick, true));
            self.state
                .events
                .push(crate::game::GameEvent::PlayerRebelled {
                    player_id,
                    rebel_id,
                    building_id: building.id,
                });
        }
    }
}

#[cfg(test)]
mod economy_speed_tests {
    use crate::engine::SowEngine;
    use crate::game::{GamePhase, GameState};
    use crate::game_config::GameConfig;
    use crate::player::Player;
    use crate::water_components::WaterComponents;

    fn gold_after_ten_seconds(global_speed_multiplier: f64) -> f64 {
        let config = GameConfig {
            global_speed_multiplier,
            gold_base_income: 4.0,
            troop_base_income: 0.0,
            territory_troop_amount: 0.0,
            territory_gold_amount: 0.0,
            troop_upkeep_per_1000: 0.0,
            ..GameConfig::default()
        };
        let mut state = GameState::new(1, 2, 2, config.clone());
        state.phase = GamePhase::Playing;
        let mut player = Player::new_human(1, "Player".into(), [1.0; 3], &config);
        player.gold = 0.0;
        player.tile_count = 1;
        state.register_player(player);
        let mut engine = SowEngine::new(state, WaterComponents::default());
        for _ in 0..100 {
            engine.execute_income();
        }
        engine.state.player(1).unwrap().gold
    }

    #[test]
    fn recurring_gold_income_scales_with_global_game_speed() {
        let half_speed = gold_after_ten_seconds(0.5);
        let normal_speed = gold_after_ten_seconds(1.0);
        assert!((half_speed * 2.0 - normal_speed).abs() < 1e-9);
        assert!((half_speed - 20.0).abs() < 1e-9);
        assert!((normal_speed - 40.0).abs() < 1e-9);
    }
}

#[cfg(test)]
mod zero_troop_bot_income_tests {
    use crate::building::Building;
    use crate::engine::SowEngine;
    use crate::game::{BuildingKind, GameEvent, GamePhase, GameState};
    use crate::game_config::GameConfig;
    use crate::map::MapTile;
    use crate::player::Player;
    use crate::water_components::WaterComponents;

    #[test]
    fn bot_with_zero_starting_troops_uses_normal_income() {
        let config = GameConfig::default();
        let mut state = GameState::new(7, 2, 1, config.clone());
        state.phase = GamePhase::Playing;
        for (id, tile) in [(1, 0), (2, 1)] {
            let mut player = if id == 1 {
                Player::new_bot(id, format!("Faction {id}"), [0.2, 0.5, 1.0], &config)
            } else {
                Player::new_human(id, format!("Faction {id}"), [0.2, 0.5, 1.0], &config)
            };
            player.alive = true;
            player.tile_count = 1;
            player.troops = 0.0;
            state.players.push(player);
            state.map.set_owner_id(tile, 0, id);
            let ref_id = state.map.ref_id(tile, 0);
            state.map.terrain[ref_id] = MapTile::from_byte(0b1000_0000);
        }
        state.player_lookup = vec![None, Some(0), Some(1)];

        let mut engine = SowEngine::new(state, WaterComponents::default());
        engine.execute_income();

        assert!(engine.state.player(1).unwrap().troops > 0.0);
        assert!(engine.state.player(2).unwrap().troops > 0.0);
    }

    #[test]
    fn debt_rebellion_waits_thirty_seconds_and_rearms_after_debt_ends() {
        let mut config = GameConfig::default();
        config.gold_base_income = 0.0;
        config.factory_gold_income = 0.0;
        config.territory_gold_amount = 0.0;
        let mut state = GameState::new(9, 8, 4, config.clone());
        state.phase = GamePhase::Playing;
        for (id, name) in [(1, "Human"), (2, "Opponent")] {
            let mut player = Player::new_human(id, name.into(), [0.3, 0.5, 0.8], &config);
            player.alive = true;
            player.troops = 50_000.0;
            if id == 1 {
                player.gold = -1.0;
            }
            state.register_player(player);
        }
        for tile in 0..4 {
            state.set_tile_owner(tile, 0, 1);
        }
        for tile in 4..8 {
            state.set_tile_owner(tile, 0, 2);
        }
        let mut engine = SowEngine::new(state, WaterComponents::default());
        engine.add_building(Building {
            id: 1,
            owner_id: 1,
            tile_idx: 0,
            kind: BuildingKind::City,
            level: 1,
            under_construction: false,
            ticks_until_complete: 0,
        });
        engine.add_building(Building {
            id: 2,
            owner_id: 1,
            tile_idx: 1,
            kind: BuildingKind::Factory,
            level: 1,
            under_construction: false,
            ticks_until_complete: 0,
        });

        for tick in 0..300 {
            engine.state.tick = tick;
            engine.execute_income();
        }
        assert!(
            engine
                .state
                .events
                .iter()
                .all(|event| !matches!(event, GameEvent::PlayerRebelled { .. }))
        );
        engine.state.tick = 300;
        engine.execute_income();
        assert_eq!(
            engine
                .state
                .events
                .iter()
                .filter(|event| matches!(event, GameEvent::PlayerRebelled { .. }))
                .count(),
            1
        );

        engine.state.player_mut(1).unwrap().gold = 100.0;
        engine.state.tick = 301;
        engine.execute_income();
        engine.state.player_mut(1).unwrap().gold = -1.0;
        for tick in 302..602 {
            engine.state.tick = tick;
            engine.execute_income();
        }
        assert_eq!(
            engine
                .state
                .events
                .iter()
                .filter(|event| matches!(event, GameEvent::PlayerRebelled { .. }))
                .count(),
            1
        );
        engine.state.tick = 602;
        engine.execute_income();
        assert_eq!(
            engine
                .state
                .events
                .iter()
                .filter(|event| matches!(event, GameEvent::PlayerRebelled { .. }))
                .count(),
            2
        );
    }
}
