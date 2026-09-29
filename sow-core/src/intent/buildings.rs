use crate::building::{
    Building, CityModules, ModuleKind, resolve_structure_spawn_tile, structure_build_cost_gold,
    structure_upgrade_cost_gold, structure_kind_enabled,
};
use crate::engine::SowEngine;
use crate::game::{BuildingKind, GameEvent, GamePhase};

impl SowEngine {
    pub(super) fn apply_build_structure_intent(
        &mut self,
        player_id: u16,
        kind: BuildingKind,
        target_tile: u32,
    ) {
        if self.state.phase != GamePhase::Playing {
            return;
        }
        if !self.state.config.buildings_enabled {
            return;
        }
        let Some(player) = self.state.player(player_id) else {
            return;
        };
        if !player.alive {
            return;
        }
        if !structure_kind_enabled(kind) {
            return;
        }
        if kind == BuildingKind::Factory
            && !self.buildings.iter().any(|building| {
                building.owner_id == player_id && building.kind == BuildingKind::City
                    && building.active_level() >= 3
            })
        {
            return;
        }
        let w = self.state.map.width;
        let area = w.saturating_mul(self.state.map.height);
        if area == 0 || target_tile >= area {
            return;
        }

        if kind == BuildingKind::Farm {
            let map = &self.state.map;
            let x = target_tile % w;
            let y = target_tile / w;
            let farm_count = self
                .buildings
                .iter()
                .filter(|b| b.owner_id == player_id && b.kind == BuildingKind::Farm)
                .count() as u32;
            let farm_slots = crate::building::player_farm_slots(&self.buildings, player_id);
            if map.owner_id(x, y) != player_id
                || map.terrain_type(x, y) != crate::map::TerrainType::Land
                || self.buildings.iter().any(|b| b.tile_idx == target_tile)
                || farm_count >= farm_slots
            {
                return;
            }
        }

        // Placement is intentionally separate from upgrades. A nearby building
        // never absorbs a new foundation order.
        self.refresh_building_grid();
        let spawn_idx = if kind == BuildingKind::Farm {
            target_tile
        } else {
            let Some(spawn_idx) = resolve_structure_spawn_tile(
                &self.state.map,
                player_id,
                kind,
                target_tile,
                &self.building_grid,
                &mut self.placement_scratch,
            ) else {
                return;
            };
            spawn_idx
        };

        let count = crate::building::count_kind(&self.buildings, player_id, kind);
        let cost = structure_build_cost_gold(kind, count, &self.state.config);
        let Some(player_mut) = self.state.player_mut(player_id) else {
            return;
        };
        if player_mut.gold < cost || !cost.is_finite() {
            return;
        }
        player_mut.gold = (player_mut.gold - cost).max(0.0);
        let id = self.state.next_building_id;
        self.state.next_building_id = self.state.next_building_id.wrapping_add(1).max(1);
        let (factory_time_levels, _, _) =
            crate::building::factory_perk_counts(&self.buildings, player_id);
        let dur = crate::building::structure_build_duration_ticks(kind, factory_time_levels);
        let under = dur > 0;
        let ticks = if under { dur } else { 0 };
        self.add_building(Building {
            id,
            owner_id: player_id,
            tile_idx: spawn_idx,
            kind,
            level: 1,
            under_construction: under,
            ticks_until_complete: ticks,
            modules: CityModules::default(),
        });
        self.state.events.push(GameEvent::StructureSpawned {
            id,
            owner_id: player_id,
            tile_idx: spawn_idx,
            kind,
            level: 1,
        });
    }

    pub(super) fn apply_upgrade_structure_intent(&mut self, player_id: u16, building_id: u64) {
        if self.state.phase != GamePhase::Playing || !self.state.config.buildings_enabled {
            return;
        }
        let Some(player) = self.state.player(player_id) else {
            return;
        };
        if !player.alive {
            return;
        }

        let Some(idx) = self
            .buildings
            .binary_search_by_key(&building_id, |b| b.id)
            .ok()
        else {
            return;
        };
        let building = self.buildings[idx];
        if building.owner_id != player_id || building.under_construction {
            return;
        }
        let target_level = building.level.saturating_add(1);
        if target_level > building.kind.max_level()
            || !self.structure_upgrade_requirements_met(player_id, &building, target_level)
        {
            return;
        }

        let owned_levels = crate::building::count_kind(&self.buildings, player_id, building.kind);
        let (_, factory_discount_levels, _) =
            crate::building::factory_perk_counts(&self.buildings, player_id);
        let cost = structure_upgrade_cost_gold(
            building.kind,
            target_level,
            owned_levels,
            factory_discount_levels,
            &self.state.config,
        );
        let Some(player_mut) = self.state.player_mut(player_id) else {
            return;
        };
        if player_mut.gold < cost || !cost.is_finite() {
            return;
        }
        player_mut.gold = (player_mut.gold - cost).max(0.0);

        let (factory_time_levels, _, _) =
            crate::building::factory_perk_counts(&self.buildings, player_id);
        let b = &mut self.buildings[idx];
        b.level = target_level;
        b.under_construction = true;
        b.ticks_until_complete = crate::building::structure_upgrade_duration_ticks(
            b.kind,
            target_level,
            factory_time_levels,
        );
        self.building_aggregates_dirty = true;
        self.bot_sam_tiles_cache = None;
        self.sea_lanes_dirty = true;
        self.state.events.push(GameEvent::StructureUpgraded {
            id: b.id,
            tile_idx: b.tile_idx,
            kind: b.kind,
            level: b.level,
        });
    }

    fn structure_upgrade_requirements_met(
        &self,
        player_id: u16,
        building: &Building,
        target_level: u8,
    ) -> bool {
        if building.kind == BuildingKind::Factory && target_level == 2 {
            return self.buildings.iter().any(|other| {
                other.owner_id == player_id
                    && other.kind == BuildingKind::City
                    && other.active_level() >= 3
            });
        }
        true
    }

    pub(super) fn apply_upgrade_city_module_intent(
        &mut self,
        player_id: u16,
        building_id: u64,
        module: ModuleKind,
    ) {
        // The campaign teaches Foundry income; other legacy modules remain retired.
        if !self.state.config.tutorial
            || module != ModuleKind::Foundry
            || self.state.phase != GamePhase::Playing
        {
            return;
        }
        let Some(player) = self.state.player(player_id) else {
            return;
        };
        if !player.alive {
            return;
        }
        let Ok(idx) = self.buildings.binary_search_by_key(&building_id, |b| b.id) else {
            return;
        };
        let building = self.buildings[idx];
        if building.owner_id != player_id
            || building.kind != BuildingKind::City
            || building.under_construction
        {
            return;
        }
        let current_level = building.modules.foundry;
        let new_level = current_level.saturating_add(1);
        if new_level > 5 {
            return;
        }
        let cost = crate::building::module_upgrade_cost_gold(ModuleKind::Foundry, new_level);
        let Some(player_mut) = self.state.player_mut(player_id) else {
            return;
        };
        if player_mut.gold < cost || !cost.is_finite() {
            return;
        }
        player_mut.gold = (player_mut.gold - cost).max(0.0);

        let b = &mut self.buildings[idx];
        b.modules.foundry = new_level;
        self.building_aggregates_dirty = true;
        self.state.events.push(GameEvent::StructureUpgraded {
            id: building_id,
            tile_idx: b.tile_idx,
            kind: b.kind,
            level: b.level,
        });
    }

    pub(super) fn apply_upgrade_tile_intent(&mut self, player_id: u16, tile_idx: u32) {
        if self.state.phase != GamePhase::Playing {
            return;
        }
        let Some(player) = self.state.player(player_id) else {
            return;
        };
        if !player.alive {
            return;
        }

        let w = self.state.map.width;
        let h = self.state.map.height;
        if tile_idx >= w * h {
            return;
        }

        if self.state.map.owner_id(tile_idx % w, tile_idx / w) != player_id {
            return;
        }

        let current_level = self
            .state
            .map
            .tile_upgrades
            .get(&tile_idx)
            .copied()
            .unwrap_or_default();
        let new_level = current_level.saturating_add(1);

        let s = crate::config::GOLD_SCALE.max(1.0);
        let cost = (1000.0 * 1.5f64.powi(current_level as i32)) / s;

        let Some(player_mut) = self.state.player_mut(player_id) else {
            return;
        };
        if player_mut.gold < cost || !cost.is_finite() {
            return;
        }
        player_mut.gold = (player_mut.gold - cost).max(0.0);

        self.state.map.tile_upgrades.insert(tile_idx, new_level);
        self.state.map.dirty_tiles.push(tile_idx as usize);

        self.state.events.push(GameEvent::TileUpgraded {
            tile_idx,
            level: new_level,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{GameEvent, GameState};
    use crate::game_config::GameConfig;
    use crate::player::Player;
    use crate::water_components::WaterComponents;

    fn engine(tutorial: bool) -> SowEngine {
        let config = GameConfig {
            tutorial,
            buildings_enabled: true,
            ..GameConfig::default()
        };
        let mut game = GameState::new(1, 5, 5, config.clone());
        game.phase = GamePhase::Playing;
        let mut player = Player::new_human(1, "Boudica".into(), [1.0, 0.0, 0.0], &config);
        player.gold = 1_000_000.0;
        game.register_player(player);
        SowEngine::new(game, WaterComponents::default())
    }

    #[test]
    fn tile_upgrade_stores_only_the_upgraded_tile() {
        let mut engine = engine(false);
        engine.state.map.set_owner_id(0, 0, 1);

        engine.apply_upgrade_tile_intent(1, 0);

        assert_eq!(engine.state.map.tile_upgrades.len(), 1);
        assert_eq!(engine.state.map.tile_upgrades.get(&0), Some(&1));
    }

    fn pacing_match() -> SowEngine {
        let config = GameConfig::default();
        let mut game = GameState::new(7, 20, 12, config.clone());
        game.phase = GamePhase::Playing;
        game.config.map_control_win_percentage = 2.0;
        for terrain in &mut game.map.terrain {
            *terrain = crate::map::MapTile::from_byte(0b1000_0000);
        }

        let mut players = Vec::new();
        for (id, x0, y0) in [(1u16, 0u32, 0u32), (2, 14, 8)] {
            let mut player =
                Player::new_human(id, format!("Player {id}"), [0.4, 0.6, 0.8], &config);
            player.has_spawned = true;
            player.tile_count = 24;
            player.gold = config.starting_gold;
            player.troops = config.starting_troops;
            for y in y0..y0 + 4 {
                for x in x0..x0 + 6 {
                    let tile = y * game.map.width + x;
                    game.map.set_owner_id(x, y, id);
                    player.border_insert(tile);
                    player.sum_x += u64::from(x);
                    player.sum_y += u64::from(y);
                }
            }
            players.push(player);
        }
        game.players = players;
        game.player_lookup = vec![None, Some(0), Some(1)];
        game.total_land_tiles = 20 * 12;
        SowEngine::new(game, WaterComponents::default())
    }

    fn building(id: u64, kind: BuildingKind, level: u8) -> Building {
        Building {
            id,
            owner_id: 1,
            tile_idx: id as u32,
            kind,
            level,
            under_construction: false,
            ticks_until_complete: 0,
            modules: CityModules::default(),
        }
    }

    #[test]
    fn foundry_upgrade_is_campaign_only_and_emits_a_city_upgrade() {
        let mut campaign = engine(true);
        campaign.buildings.push(Building {
            id: 1,
            owner_id: 1,
            tile_idx: 0,
            kind: BuildingKind::City,
            level: 1,
            under_construction: false,
            ticks_until_complete: 0,
            modules: CityModules::default(),
        });
        campaign.apply_upgrade_city_module_intent(1, 1, ModuleKind::Foundry);
        assert_eq!(campaign.buildings[0].modules.foundry, 1);
        assert!(campaign.state.events.iter().any(|event| matches!(
            event,
            GameEvent::StructureUpgraded { kind: BuildingKind::City, .. }
        )));

        let mut multiplayer = engine(false);
        let mut city = campaign.buildings[0];
        city.modules = CityModules::default();
        multiplayer.buildings.push(city);
        multiplayer.apply_upgrade_city_module_intent(1, 1, ModuleKind::Foundry);
        assert_eq!(multiplayer.buildings[0].modules.foundry, 0);
    }

    #[test]
    fn structure_upgrade_charges_the_reduced_price_and_activates_when_finished() {
        let mut game = engine(false);
        game.buildings.push(building(1, BuildingKind::City, 1));
        let before = game.state.player(1).unwrap().gold;
        let expected = structure_upgrade_cost_gold(BuildingKind::City, 2, 1, 0, &game.state.config);

        game.apply_upgrade_structure_intent(1, 1);

        let city = game.buildings[0];
        assert!((game.state.player(1).unwrap().gold - (before - expected)).abs() < 1e-9);
        assert!(city.under_construction);
        assert_eq!(city.active_level(), 1);

        for _ in 0..city.ticks_until_complete {
            game.execute_construction();
        }
        assert_eq!(game.buildings[0].active_level(), 2);
    }

    #[test]
    fn factory_manufactory_upgrade_requires_a_village() {
        let mut game = engine(false);
        game.buildings.push(building(1, BuildingKind::Factory, 1));
        game.buildings.push(building(2, BuildingKind::City, 2));

        game.apply_upgrade_structure_intent(1, 1);
        assert_eq!(game.buildings[0].level, 1);
        assert!(!game.buildings[0].under_construction);

        game.buildings[1].level = 3;
        game.apply_upgrade_structure_intent(1, 1);
        assert_eq!(game.buildings[0].level, 2);
        assert!(game.buildings[0].under_construction);
    }

    #[test]
    fn farms_require_owned_lowland_and_stop_at_city_plot_limit() {
        let mut game = engine(false);
        game.state.next_building_id = 2;
        game.buildings.push(building(1, BuildingKind::City, 1));
        for y in 0..5 {
            for x in 0..5 {
                let idx = game.state.map.ref_id(x, y);
                game.state.map.terrain[idx] = crate::map::MapTile::from_byte(0b1000_0000);
                game.state.map.set_owner_id(x, y, 1);
            }
        }
        game.state.map.set_owner_id(4, 0, 0);
        let water = game.state.map.ref_id(1, 0);
        game.state.map.terrain[water] = crate::map::MapTile::from_byte(0);

        game.apply_build_structure_intent(1, BuildingKind::Farm, 4);
        game.apply_build_structure_intent(1, BuildingKind::Farm, 1);
        assert_eq!(game.buildings.len(), 1);

        game.apply_build_structure_intent(1, BuildingKind::Farm, 2);
        assert_eq!(game.buildings.len(), 2);
        assert_eq!(game.buildings[1].tile_idx, 2);
        assert!(game.buildings[1].under_construction);

        game.apply_build_structure_intent(1, BuildingKind::Farm, 3);
        assert_eq!(game.buildings.len(), 2);
    }

    #[test]
    fn nuclear_launches_require_a_finished_metropolis() {
        let mut game = engine(false);
        game.buildings.push(building(1, BuildingKind::City, 5));
        let gold = game.state.player(1).unwrap().gold;

        game.apply_launch_nuke_intent(1, 4);
        assert!(game.projectiles.is_empty());
        assert_eq!(game.state.player(1).unwrap().gold, gold);

        game.buildings[0].level = BuildingKind::City.max_level();
        game.buildings[0].under_construction = true;
        game.apply_launch_nuke_intent(1, 4);
        assert!(game.projectiles.is_empty());

        game.buildings[0].under_construction = false;
        game.apply_launch_nuke_intent(1, 4);
        assert_eq!(game.projectiles.len(), 1);
        assert_eq!(
            game.state.player(1).unwrap().gold,
            gold - game.state.config.nuke_cost
        );
    }

    #[test]
    fn building_economy_simulation_reports_2_6_and_10_minute_checkpoints() {
        let mut game = pacing_match();
        let config = game.state.config.clone();
        let starting_player = game.state.player(1).unwrap();
        assert_eq!(starting_player.gold, config.starting_gold);
        assert_eq!(starting_player.troops, config.starting_troops);

        let checkpoints = [(1_200u64, 2u8), (3_600, 6), (6_000, 10)];
        let mut next_checkpoint = 0;
        let city_tile = 2 * game.state.map.width + 2;
        let farm_tile = 0;
        for _ in 0..6_000 {
            let city = game
                .buildings
                .iter()
                .find(|building| building.owner_id == 1 && building.kind == BuildingKind::City)
                .copied();
            match city {
                None => game.apply_build_structure_intent(1, BuildingKind::City, city_tile),
                Some(city) if city.under_construction => {}
                Some(city) => {
                    let farms = game
                        .buildings
                        .iter()
                        .filter(|building| {
                            building.owner_id == 1 && building.kind == BuildingKind::Farm
                        })
                        .count();
                    if city.active_level() == 1 && farms == 0 {
                        game.apply_build_structure_intent(1, BuildingKind::Farm, farm_tile);
                    } else if city.level < BuildingKind::City.max_level() {
                        game.apply_upgrade_structure_intent(1, city.id);
                    }
                }
            }

            game.tick();
            if next_checkpoint < checkpoints.len()
                && game.state.tick == checkpoints[next_checkpoint].0
            {
                let (tick, minute) = checkpoints[next_checkpoint];
                let player = game.state.player(1).unwrap();
                let aggregate = crate::building::aggregate_buildings_per_player(
                    game.buildings.iter().copied(),
                    2,
                )[1];
                let gold_rate = crate::execution::income_rates::gold_income_per_second(
                    player.tile_count,
                    aggregate,
                    player.leader,
                    &config,
                );
                let troop_rate = crate::execution::income_rates::troop_income_per_second(
                    player.tile_count,
                    aggregate,
                    player.leader,
                    &config,
                );
                let city = game
                    .buildings
                    .iter()
                    .find(|building| building.owner_id == 1 && building.kind == BuildingKind::City)
                    .copied();
                let farm_count = game
                    .buildings
                    .iter()
                    .filter(|building| {
                        building.owner_id == 1 && building.kind == BuildingKind::Farm
                    })
                    .count();
                eprintln!(
                    "building pace {minute}m: gold={:.1}, troops={:.1}/{:.1}, city={}, farms={}, income={:.2}g/s + {:.2}troops/s",
                    player.gold,
                    player.troops,
                    player.max_troops,
                    city.map_or(0, |building| building.active_level()),
                    farm_count,
                    gold_rate,
                    troop_rate,
                );
                assert!(player.gold.is_finite() && player.gold >= 0.0);
                assert!(player.troops.is_finite() && player.max_troops.is_finite());
                assert!(gold_rate.is_finite() && troop_rate.is_finite());
                if minute == 2 {
                    assert_eq!(city.map(|building| building.active_level()), Some(1));
                    assert_eq!(farm_count, 1);
                }
                if minute == 6 {
                    assert!(city.is_some_and(|building| building.active_level() >= 2));
                }
                assert_eq!(game.state.tick, tick);
                next_checkpoint += 1;
            }
        }
        assert_eq!(next_checkpoint, checkpoints.len());
    }
}
