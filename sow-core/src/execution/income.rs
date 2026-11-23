use crate::building::BuildingAggregate;
use crate::engine::SowEngine;
use crate::execution::income_rates::{
    alliance_gold_per_second, alliance_troops_per_second, gold_income_per_second,
    diminishing_passive_income, imperial_standard_income, land_trade_gold_per_second, tafaifa_council_multiplier,
    trade_income_per_second, trade_troops_per_second, troop_income_per_second,
    GOLD_PASSIVE_EXCESS_RATE, GOLD_PASSIVE_FULL_RATE_CAP, TROOP_PASSIVE_EXCESS_RATE,
    TROOP_PASSIVE_FULL_RATE_CAP,
};
use crate::game::GamePhase;

impl SowEngine {
    fn collect_land_trade(&mut self) -> Option<Vec<f64>> {
        use std::cmp::Reverse;
        let tick = self.state.tick;
        self.trade_last_delivery_tick = tick;
        let mut payments: Option<Vec<f64>> = None;
        while let Some(&Reverse((due, index))) = self.trade_delivery_queue.peek() {
            if due > tick { break; }
            self.trade_delivery_queue.pop();
            let Some(link) = self.railway_links.get_mut(index) else { continue };
            let open = link.is_open(&self.state);
            let owner = usize::from(link.owner_id);
            if let Some(aggregate) = self.building_aggregates.get_mut(owner) {
                if open == link.blocked {
                    let previous_weight = link.weight();
                    link.blocked = !open;
                    aggregate.land_trade_income_weight += link.weight() - previous_weight;
                    if open {
                        aggregate.railway_connections += 1;
                    } else {
                        aggregate.railway_connections = aggregate.railway_connections.saturating_sub(1);
                    }
                } else if open {
                    let gold = link.cargo_gold(&self.state.config);
                    payments.get_or_insert_with(|| vec![0.0; self.building_aggregates.len()])[owner] += gold;
                }
            }
            self.trade_delivery_queue.push(Reverse((tick + u64::from(link.period_ticks), index)));
        }
        payments
    }

    pub(crate) fn troop_capacity(
        &self,
        player: &crate::player::Player,
        agg: BuildingAggregate,
    ) -> f64 {
        let tiles = f64::from(player.tile_count);
        let half = tiles.sqrt();
        let territory_weight = half * half.sqrt().sqrt();
        let config = &self.state.config;
        let mut capacity = config.max_troops_base
            + territory_weight * config.max_troops_scale
            + agg.city_capacity_level_weight
                * config.city_max_troops
                * player.leader.city_troop_capacity_multiplier()
            + agg.district_capacity_level_weight * config.district_max_troops;
        if player.player_type == crate::player::PlayerType::Bot {
            capacity /= 1.5;
        }
        if let Some(cap) = player.max_troops_cap {
            capacity = capacity.min(cap);
        }
        if let Some(cap) = self.campaign_assault_troop_caps.get(&player.id) {
            capacity = cap.max(0.0);
        }
        capacity
    }

    pub fn execute_income(&mut self) {
        if self.state.phase != GamePhase::Playing {
            return;
        }

        self.refresh_building_aggregates();
        let deliveries = self.collect_land_trade();
        let aggs = self.building_aggregates.clone();

        let config = self.state.config.clone();
        let num_players = self.state.players.len();
        let mut trade_ships_by_player = vec![0u32; aggs.len()];
        for fleet in self.fleets.iter().filter(|fleet| {
            fleet.unit_type == crate::game::UnitType::TradeShip
                && fleet.captured_trade_return.is_none()
        }) {
            if let Some(count) = trade_ships_by_player.get_mut(usize::from(fleet.owner_id)) {
                *count = count.saturating_add(1);
            }
        }
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

            // Tribes (PlayerType::Bot) always eat the standard-bot handicap —
            // no id-based carve-out (that used to let a handful of "élite"
            // tribes dodge it by id%100, inverting the food chain).
            let is_standard_bot =
                self.state.players[idx].player_type == crate::player::PlayerType::Bot;

            let leader = self.state.players[idx].leader;
            let trade_ships = trade_ships_by_player
                .get(usize::from(player_id))
                .copied()
                .unwrap_or_default();
            let council_multiplier = tafaifa_council_multiplier(
                leader,
                crate::execution::hero_effects::living_allies(&self.state, player_id),
            );
            let council_active = council_multiplier > 1.0;
            let max_tr = self.troop_capacity(&self.state.players[idx], agg);
            self.state.players[idx].max_troops = max_tr;

            let hero_gold = self.hero_gold_per_second(player_id);
            let (_, standards_troops) = imperial_standard_income(agg, leader, &config);
            if hero_gold > 0.0 || standards_troops > 0.0 || council_active {
                if self.hero_income_active.insert(player_id) {
                    let marker = if matches!(
                        leader.ability(),
                        crate::geo_entities::Ability::ImperialStandards { .. }
                    ) {
                        let width = self.state.map.width;
                        self.buildings
                            .iter()
                            .find(|building| {
                                building.owner_id == player_id
                                    && building.kind == crate::game::BuildingKind::City
                                    && building.active_level() > 0
                            })
                            .map(|city| (city.tile_idx % width, city.tile_idx / width))
                    } else {
                        self.state.players[idx].home
                    };
                    self.state.announce_leader_ability(player_id, marker);
                }
            } else {
                self.hero_income_active.remove(&player_id);
            }

            let bot_income_multiplier = if is_standard_bot { 0.75 } else { 1.0 };
            let troop_ps = (troop_income_per_second(tiles_owned, agg, leader, &config)
                * council_multiplier
                + alliance_troops_per_second(
                    self.state.players[idx].alliances.len(),
                    leader,
                    &config,
                )
                + standards_troops)
                * bot_income_multiplier
                + trade_troops_per_second(trade_ships, leader, &config);
            let troop_income = config.per_tick(diminishing_passive_income(
                troop_ps,
                TROOP_PASSIVE_FULL_RATE_CAP,
                TROOP_PASSIVE_EXCESS_RATE,
            ));
            let player = &mut self.state.players[idx];
            player.troops = safe_troops;
            player.troops = (safe_troops + troop_income).min(max_tr);
            let received_troops = (player.troops - safe_troops).max(0.0);
            if received_troops.is_finite() {
                player.troops_received += received_troops;
            }

            let gold_ps = (gold_income_per_second(tiles_owned, agg, leader, &config)
                * council_multiplier
                + alliance_gold_per_second(
                    self.state.players[idx].alliances.len(),
                    leader,
                    &config,
                )) * bot_income_multiplier
                + trade_income_per_second(trade_ships, leader, &config)
                + hero_gold;
            let combined_rate = diminishing_passive_income(
                gold_ps,
                GOLD_PASSIVE_FULL_RATE_CAP,
                GOLD_PASSIVE_EXCESS_RATE,
            );
            let land_trade_rate = land_trade_gold_per_second(agg, leader, &config);
            let scaled_trade_rate = land_trade_rate * council_multiplier * bot_income_multiplier;
            let passive_rate = diminishing_passive_income(
                gold_ps - scaled_trade_rate, GOLD_PASSIVE_FULL_RATE_CAP, GOLD_PASSIVE_EXCESS_RATE,
            );
            let delivered = deliveries.as_ref().and_then(|gold| gold.get(usize::from(player_id)))
                .copied().unwrap_or(0.0);
            let gold_income = config.per_tick(passive_rate) + if land_trade_rate > 0.0 {
                delivered * (combined_rate - passive_rate).max(0.0)
                    / (agg.land_trade_income_weight * config.factory_gold_income)
            } else { 0.0 };
            self.state.players[idx].credit_gold(gold_income);

            let iq_gain = config.per_tick(self.state.players[idx].iq as f64 / 100.0);
            self.state.players[idx].iq_points =
                (self.state.players[idx].iq_points + iq_gain).min(500.0);
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

    fn income_after_ten_seconds(global_speed_multiplier: f64) -> (f64, f64) {
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
        let received_before = player.gold_received;
        player.tile_count = 1;
        state.register_player(player);
        let mut engine = SowEngine::new(state, WaterComponents::default());
        for _ in 0..100 {
            engine.execute_income();
        }
        let player = engine.state.player(1).unwrap();
        (player.gold, player.gold_received - received_before)
    }

    #[test]
    fn recurring_gold_income_scales_with_global_game_speed() {
        let (half_speed, half_received) = income_after_ten_seconds(0.5);
        let (normal_speed, normal_received) = income_after_ten_seconds(1.0);
        assert!((half_speed * 2.0 - normal_speed).abs() < 1e-9);
        assert!((half_speed - 20.0).abs() < 1e-9);
        assert!((normal_speed - 40.0).abs() < 1e-9);
        assert!((half_received - half_speed).abs() < 1e-9);
        assert!((normal_received - normal_speed).abs() < 1e-9);
    }
}

#[cfg(test)]
mod zero_troop_bot_income_tests {
    use crate::engine::SowEngine;
    use crate::game::{GamePhase, GameState};
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
        let received_before: Vec<_> = engine
            .state
            .players
            .iter()
            .map(|player| player.troops_received)
            .collect();
        engine.execute_income();

        for (index, id) in [1, 2].into_iter().enumerate() {
            let player = engine.state.player(id).unwrap();
            assert!(player.troops > 0.0);
            assert!(player.troops_received > received_before[index]);
        }
    }

    #[test]
    fn bot_with_150_tiles_does_not_receive_a_free_city_below_cost() {
        let config = GameConfig::default();
        let mut state = GameState::new(1, 20, 20, config.clone());
        state.phase = GamePhase::Playing;
        let mut player = Player::new_bot(1, "Snettisham".into(), [0.2, 0.5, 1.0], &config);
        player.alive = true;
        player.tile_count = 150;
        player.troops = 500.0;
        player.gold = 0.0;
        state.players.push(player);
        for tile in 0..150 {
            state.map.set_owner_id(tile % 20, tile / 20, 1);
            let ref_id = state.map.ref_id(tile % 20, tile / 20);
            state.map.terrain[ref_id] = MapTile::from_byte(0b1000_0000);
        }
        state.player_lookup = vec![None, Some(0)];

        let mut engine = SowEngine::new(state, WaterComponents::default());
        engine.state.tick = 29;
        engine.execute_income();
        assert!(engine.state.player(1).unwrap().gold < config.cost_city);
        assert!(
            !engine
                .buildings
                .iter()
                .any(|b| { b.owner_id == 1 && b.kind == crate::game::BuildingKind::City })
        );

        engine.state.player_mut(1).unwrap().gold = config.cost_city - 1.0;
        engine.state.tick = 59;
        engine.execute_income();
        assert!(engine.state.player(1).unwrap().gold < config.cost_city);
        assert!(
            !engine
                .buildings
                .iter()
                .any(|b| { b.owner_id == 1 && b.kind == crate::game::BuildingKind::City })
        );
    }
}
