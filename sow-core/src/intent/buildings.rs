use crate::building::{
    Building, CityArsenalBuild, city_arsenal_cost_gold, structure_build_cost_gold,
    structure_upgrade_cost_gold, validate_structure_site,
};
use crate::engine::SowEngine;
use crate::game::{BuildingKind, CityArsenalKind, GameEvent, GamePhase, NavalActionFailure, UnitType};

impl SowEngine {
    /// Boudica's tutorial grants one free City level-2 upgrade.
    pub fn tutorial_city_upgrade_is_free(&self, player_id: u16, building_id: u64) -> bool {
        if !self.state.config.tutorial
            || !self.state.config.buildings_enabled
            || self.state.config.player_leader.id() != "boudica"
        {
            return false;
        }
        let Some(building) = self
            .buildings
            .iter()
            .find(|building| building.id == building_id && building.owner_id == player_id)
        else {
            return false;
        };
        building.kind == BuildingKind::City
            && building.level == 1
            && !building.under_construction
            && !self.buildings.iter().any(|candidate| {
                candidate.owner_id == player_id
                    && candidate.kind == BuildingKind::City
                    && candidate.level >= 2
            })
    }

    /// Shared price for the confirmation UI and authoritative upgrade intent.
    pub fn structure_upgrade_quote(&self, building_id: u64) -> Option<(f64, bool)> {
        let index = self
            .buildings
            .binary_search_by_key(&building_id, |building| building.id)
            .ok()?;
        let building = self.buildings[index];
        Some(
            if self.tutorial_city_upgrade_is_free(building.owner_id, building_id) {
                (0.0, false)
            } else {
                self.paid_structure_upgrade_quote(building)
            },
        )
    }

    pub(super) fn paid_structure_upgrade_quote(&self, building: Building) -> (f64, bool) {
        let owned_levels =
            crate::building::count_kind(&self.buildings, building.owner_id, building.kind);
        let cost = structure_upgrade_cost_gold(
            building.kind,
            building.level.saturating_add(1),
            owned_levels,
            &self.state.config,
        );
        let discount = self.hero_city_upgrade_discount(building);
        if discount > 0 && cost.is_finite() && cost > 0.0 {
            (cost * f64::from(100 - discount) / 100.0, true)
        } else {
            (cost, false)
        }
    }

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
        if !self.campaign_allows_building(kind, 1) {
            return;
        }
        let w = self.state.map.width;
        let area = w.saturating_mul(self.state.map.height);
        if area == 0 || target_tile >= area {
            return;
        }
        self.refresh_building_grid();

        if self.legacy_building_rules && kind == BuildingKind::Farm {
            let map = &self.state.map;
            let x = target_tile % w;
            let y = target_tile / w;
            let footprint = crate::building::BuildingFootprint::legacy_at(x, y);
            if map.owner_id(x, y) != player_id
                || map.terrain_type(x, y) != crate::map::TerrainType::Land
                || !crate::building::footprint_fits(map, player_id, footprint)
                || self
                    .building_grid
                    .iter_all_in_range(x, y, 3)
                    .any(|building| {
                        footprint.intersects(crate::building::BuildingFootprint::legacy_at(
                            building.x, building.y,
                        ))
                    })
            {
                return;
            }
        }

        // Placement is intentionally separate from upgrades. A nearby building
        // never absorbs a new foundation order.
        let spawn_idx = if !self.legacy_building_rules {
            if validate_structure_site(
                &self.state.map,
                player_id,
                kind,
                target_tile,
                &self.building_grid,
            )
            .is_err()
            {
                return;
            }
            target_tile
        } else if kind == BuildingKind::Farm {
            target_tile
        } else {
            let Some(spawn_idx) = crate::building::find_structure_site_near(
                &self.state.map,
                player_id,
                target_tile,
                kind,
                &self.building_grid,
                &mut self.placement_scratch,
                true,
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
        player_mut.structures_built = player_mut.structures_built.saturating_add(1);
        let id = self.state.next_building_id;
        self.state.next_building_id = self.state.next_building_id.wrapping_add(1).max(1);
        let base_dur = crate::building::structure_build_duration_ticks(kind);
        let dur = if self.state.config.tutorial {
            (base_dur / 2).max(1)
        } else {
            base_dur
        };
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
        if building.owner_id != player_id
            || building.under_construction
            || self
                .building_productions
                .get(&building.id)
                .is_some_and(|state| state.arsenal_build.is_some())
        {
            return;
        }
        let target_level = building.level.saturating_add(1);
        if target_level > building.kind.max_level()
            || !self.campaign_allows_building(building.kind, target_level)
        {
            return;
        }

        let Some((cost, discounted)) = self.structure_upgrade_quote(building_id) else {
            return;
        };
        let Some(player_mut) = self.state.player_mut(player_id) else {
            return;
        };
        if player_mut.gold < cost || !cost.is_finite() {
            return;
        }
        player_mut.gold = (player_mut.gold - cost).max(0.0);

        let b = &mut self.buildings[idx];
        b.level = target_level;
        b.under_construction = true;
        let upgrade_ticks = crate::building::structure_upgrade_duration_ticks(b.kind, target_level);
        b.ticks_until_complete = if self.state.config.tutorial {
            (upgrade_ticks / 2).max(1)
        } else {
            upgrade_ticks
        };
        if b.kind == BuildingKind::Tower {
            self.defense_grid_dirty = true;
        }
        self.building_aggregates_dirty = true;
        self.bot_sam_tiles_cache = None;
        self.sea_lanes_dirty = true;
        self.state.events.push(GameEvent::StructureUpgraded {
            id: b.id,
            tile_idx: b.tile_idx,
            kind: b.kind,
            level: b.level,
        });
        if discounted {
            let width = self.state.map.width;
            self.state.announce_leader_ability(
                player_id,
                Some((building.tile_idx % width, building.tile_idx / width)),
            );
        }
    }

    pub(super) fn apply_building_production_intent(
        &mut self,
        player_id: u16,
        building_id: u64,
        kind: CityArsenalKind,
    ) {
        self.queue_building_production(player_id, building_id, kind, None);
    }

    /// Old replays address a City tile; resolve their former co-located producer if present.
    pub(super) fn apply_city_arsenal_intent(
        &mut self,
        player_id: u16,
        tile_idx: u32,
        kind: CityArsenalKind,
    ) {
        let producer_kind = match kind {
            CityArsenalKind::Warship => BuildingKind::Port,
            CityArsenalKind::Nuke => return,
            CityArsenalKind::AntiNuke => return,
        };
        let Some(building_id) = self
            .buildings
            .iter()
            .find(|b| b.tile_idx == tile_idx && b.owner_id == player_id && b.kind == producer_kind)
            .map(|b| b.id)
        else {
            return;
        };
        self.queue_building_production(player_id, building_id, kind, None);
    }

    pub(super) fn apply_build_ship_intent(
        &mut self,
        player_id: u16,
        target_tile: u32,
        unit_type: UnitType,
    ) {
        if unit_type != UnitType::Warship {
            return;
        }
        if !self.campaign_allows_action(crate::campaign::CampaignAction::Warship) {
            self.state.events.push(GameEvent::NavalActionRejected {
                player_id,
                reason: NavalActionFailure::CampaignLocked,
            });
            return;
        }
        let route = match crate::warp_fleet::resolve_warship_route(
            &self.state.map,
            &self.water,
            &mut self.path_scratch,
            &self.buildings,
            player_id,
            target_tile,
        ) {
            Ok(route) => route,
            Err(error) => {
                let reason = match error {
                    crate::warp_fleet::WarshipLaunchError::InvalidTile => NavalActionFailure::InvalidTarget,
                    crate::warp_fleet::WarshipLaunchError::TargetNotWater => NavalActionFailure::TargetNotWater,
                    crate::warp_fleet::WarshipLaunchError::NoPort => NavalActionFailure::NoPort,
                    crate::warp_fleet::WarshipLaunchError::NoConnectedPort => NavalActionFailure::NoConnectedPort,
                    crate::warp_fleet::WarshipLaunchError::NoWaterPath => NavalActionFailure::NoWaterPath,
                };
                self.state.events.push(GameEvent::NavalActionRejected { player_id, reason });
                return;
            }
        };
        self.queue_building_production(
            player_id,
            route.port_id,
            CityArsenalKind::Warship,
            Some(route),
        );
    }

    fn queue_building_production(
        &mut self,
        player_id: u16,
        building_id: u64,
        kind: CityArsenalKind,
        warship_route: Option<crate::warp_fleet::WarshipRoute>,
    ) {
        if self.state.phase != GamePhase::Playing || !self.state.config.buildings_enabled {
            return;
        }
        let Some(player) = self.state.player(player_id) else {
            return;
        };
        if !player.alive {
            return;
        }
        let leader = player.leader;
        let allowed = match kind {
            CityArsenalKind::Warship => {
                self.campaign_allows_action(crate::campaign::CampaignAction::Warship)
            }
            CityArsenalKind::Nuke => {
                self.campaign_allows_action(crate::campaign::CampaignAction::Nuke)
            }
            CityArsenalKind::AntiNuke => false,
        };
        if !allowed {
            if kind == CityArsenalKind::Warship {
                self.state.events.push(GameEvent::NavalActionRejected {
                    player_id,
                    reason: NavalActionFailure::CampaignLocked,
                });
            }
            return;
        }

        let Some(building) = self.buildings.iter().find(|b| b.id == building_id).copied() else {
            return;
        };
        if building.owner_id != player_id
            || building.under_construction
            || self
                .building_productions
                .get(&building_id)
                .is_some_and(|state| state.arsenal_build.is_some())
        {
            return;
        }

        match kind {
            CityArsenalKind::Warship => {
                if building.kind != BuildingKind::Port || building.active_level() < 1 {
                    self.state.events.push(GameEvent::NavalActionRejected {
                        player_id,
                        reason: NavalActionFailure::NoPort,
                    });
                    return;
                }
                if warship_route.is_none()
                    && crate::warp_fleet::coastal_warship_spawn_tile(
                        &self.state.map,
                        &self.water,
                        building.tile_idx,
                    )
                    .is_none()
                {
                    self.state.events.push(GameEvent::NavalActionRejected {
                        player_id,
                        reason: NavalActionFailure::NoConnectedPort,
                    });
                    return;
                }
                let capacity = crate::building::player_fleet_capacity(&self.buildings, player_id);
                let queued = self
                    .building_productions
                    .values()
                    .filter(|state| {
                        state
                            .arsenal_build
                            .is_some_and(|build| build.kind == CityArsenalKind::Warship)
                    })
                    .count() as u32;
                if self.boat_slots_used(player_id).saturating_add(queued) >= capacity {
                    self.state.events.push(GameEvent::NavalActionRejected {
                        player_id,
                        reason: NavalActionFailure::CapacityFull,
                    });
                    return;
                }
            }
            CityArsenalKind::Nuke => {
                if !matches!(building.kind, BuildingKind::Factory | BuildingKind::Tower)
                    || building.active_level() < crate::game::NukeKind::AtomBomb.required_district_level()
                    || self
                        .building_productions
                        .get(&building_id)
                        .is_some_and(|state| state.nuke_ready)
                {
                    return;
                }
            }
            CityArsenalKind::AntiNuke => return,
        }

        let queued_warships = self.building_productions.iter().filter(|(building_id, state)| {
            state
                .arsenal_build
                .is_some_and(|build| build.kind == CityArsenalKind::Warship)
                && self.buildings.iter().any(|building| {
                    building.id == **building_id && building.owner_id == player_id
                })
        }).count() as u32;
        let owned_warships = (self
            .fleets
            .iter()
            .filter(|f| f.owner_id == player_id && f.unit_type == UnitType::Warship)
            .count() as u32)
            .saturating_add(queued_warships);
        let cost = city_arsenal_cost_gold(kind, owned_warships, leader, &self.state.config);
        let Some(player_mut) = self.state.player_mut(player_id) else {
            return;
        };
        if player_mut.gold < cost || !cost.is_finite() {
            if kind == CityArsenalKind::Warship {
                self.state.events.push(GameEvent::NavalActionRejected {
                    player_id,
                    reason: NavalActionFailure::InsufficientGold,
                });
            }
            return;
        }
        player_mut.gold = (player_mut.gold - cost).max(0.0);

        let duration = kind.duration_ticks();
        self.building_productions.entry(building_id).or_default().arsenal_build = Some(CityArsenalBuild {
            kind,
            ticks_until_complete: duration,
            total_ticks: duration,
            warship_source_tile: warship_route.as_ref().map(|route| route.src_tile),
            warship_target_tile: warship_route.as_ref().map(|route| route.landing_tile),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::GameState;
    use crate::game_config::GameConfig;
    use crate::player::Player;
    use crate::water_components::WaterComponents;

    fn engine(tutorial: bool) -> SowEngine {
        let config = GameConfig {
            tutorial,
            buildings_enabled: true,
            player_leader: if tutorial {
                crate::player::Leader::of("boudica")
            } else {
                crate::player::Leader::of("caesar")
            },
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
    fn confirmed_foundations_touch_without_upgrading_or_relocating_and_rejection_costs_nothing() {
        let config = GameConfig::default();
        let mut state = GameState::new(1, 32, 32, config.clone());
        state.phase = GamePhase::Playing;
        let mut player = Player::new_human(1, "Builder".into(), [1.0, 0.0, 0.0], &config);
        player.gold = 1_000_000.0;
        state.register_player(player);
        for y in 0..32 {
            for x in 0..32 {
                state.map.set_owner_id(x, y, 1);
            }
        }
        let mut engine = SowEngine::new(state, WaterComponents::default());
        for (kind, x, y) in [
            (BuildingKind::City, 10, 10),
            (BuildingKind::City, 14, 10),
        ] {
            engine.apply_build_structure_intent(1, kind, y * 32 + x);
            assert!(
                engine
                    .buildings
                    .iter()
                    .any(|b| b.kind == kind && b.tile_idx == y * 32 + x && b.level == 1)
            );
        }
        let max_ticks = engine
            .buildings
            .iter()
            .map(|b| b.ticks_until_complete)
            .max()
            .unwrap_or(0);
        for _ in 0..max_ticks {
            engine.execute_construction();
        }
        engine.apply_build_structure_intent(1, BuildingKind::Factory, 10 * 32 + 17);
        assert!(engine.buildings.iter().any(|b| {
            b.kind == BuildingKind::Factory && b.tile_idx == 10 * 32 + 17 && b.level == 1
        }));
        let gold = engine.state.player(1).unwrap().gold;
        engine.apply_build_structure_intent(1, BuildingKind::City, 10 * 32 + 13);
        assert_eq!(engine.buildings.len(), 3);
        assert_eq!(engine.state.player(1).unwrap().gold, gold);
        assert!(engine.buildings.iter().all(|b| b.level == 1));
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
        for (id, y0) in [(1u16, 0u32), (2, 8)] {
            let mut player =
                Player::new_human(id, format!("Player {id}"), [0.4, 0.6, 0.8], &config);
            player.has_spawned = true;
            player.tile_count = 32;
            player.gold = config.starting_gold;
            player.troops = config.starting_troops;
            let x_ranges: &[(u32, u32)] = if id == 1 {
                &[(0, 4), (6, 10)]
            } else {
                &[(12, 20)]
            };
            for y in y0..y0 + 4 {
                for &(x_start, x_end) in x_ranges {
                    for x in x_start..x_end {
                        let tile = y * game.map.width + x;
                        game.map.set_owner_id(x, y, id);
                        player.border_insert(tile);
                        player.sum_x += u64::from(x);
                        player.sum_y += u64::from(y);
                    }
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
        }
    }

    #[test]
    fn structure_upgrade_charges_the_reduced_price_and_activates_when_finished() {
        let mut game = engine(false);
        game.buildings.push(building(1, BuildingKind::City, 1));
        game.buildings.push(Building {
            tile_idx: 1,
            ..building(2, BuildingKind::Factory, 1)
        });
        let before = game.state.player(1).unwrap().gold;
        let expected = structure_upgrade_cost_gold(BuildingKind::Factory, 2, 1, &game.state.config);

        game.apply_upgrade_structure_intent(1, 2);

        let factory = game.buildings[1];
        assert!((game.state.player(1).unwrap().gold - (before - expected)).abs() < 1e-9);
        assert!(factory.under_construction);
        assert_eq!(factory.active_level(), 1);

        for _ in 0..factory.ticks_until_complete {
            game.execute_construction();
        }
        assert_eq!(game.buildings[1].active_level(), 2);
    }

    #[test]
    fn first_boudica_tutorial_city_upgrade_is_free_once() {
        let mut game = engine(true);
        assert!(game.set_campaign_unlocks(crate::campaign::CampaignUnlocks {
            buildings: std::collections::HashMap::from([
                (BuildingKind::City, 2),
                (BuildingKind::Factory, 2),
            ]),
            actions: vec![],
        }));
        game.buildings.push(building(1, BuildingKind::City, 1));
        game.buildings.push(Building {
            tile_idx: 1,
            ..building(2, BuildingKind::Factory, 1)
        });
        game.state.player_mut(1).unwrap().gold = 0.0;

        assert!(!game.tutorial_city_upgrade_is_free(1, 2));
        assert!(game.tutorial_city_upgrade_is_free(1, 1));
        game.apply_upgrade_structure_intent(1, 1);

        assert_eq!(game.buildings[0].level, 2);
        assert_eq!(game.state.player(1).unwrap().gold, 0.0);
        assert!(!game.tutorial_city_upgrade_is_free(1, 1));

        game.buildings.push(Building {
            tile_idx: 100,
            ..building(3, BuildingKind::City, 1)
        });
        assert!(!game.tutorial_city_upgrade_is_free(1, 3));
        game.state.player_mut(1).unwrap().gold = 1_000_000.0;
        let cost = structure_upgrade_cost_gold(BuildingKind::City, 2, 3, &game.state.config);
        game.apply_upgrade_structure_intent(1, 3);
        assert!((game.state.player(1).unwrap().gold - (1_000_000.0 - cost)).abs() < 1e-9);
    }

    #[test]
    fn first_city_upgrade_is_not_free_outside_boudica_tutorial() {
        let mut game = engine(false);
        game.buildings.push(building(1, BuildingKind::City, 1));
        assert!(!game.tutorial_city_upgrade_is_free(1, 1));
    }

    #[test]
    fn factory_upgrades_without_a_city_level_requirement() {
        let mut game = engine(false);
        game.buildings.push(building(1, BuildingKind::Factory, 1));

        game.apply_upgrade_structure_intent(1, 1);
        assert_eq!(game.buildings[0].level, 2);
        assert!(game.buildings[0].under_construction);
    }

    #[test]
    fn factory_build_order_has_no_city_level_requirement() {
        let mut game = engine(true);
        assert!(game.set_campaign_unlocks(crate::campaign::CampaignUnlocks {
            buildings: std::collections::HashMap::from([(BuildingKind::Factory, 1)]),
            actions: vec![],
        }));
        game.state.map = crate::map::GameMap::new(32, 32);
        for y in 0..32 {
            for x in 0..32 {
                game.state.map.set_owner_id(x, y, 1);
            }
        }
        let factory_tile = 16 * 32 + 16;
        game.state.player_mut(1).unwrap().tile_count = 32 * 32;
        let gold = game.state.player(1).unwrap().gold;

        game.apply_build_structure_intent(1, BuildingKind::Factory, factory_tile);

        assert_eq!(game.buildings.len(), 1);
        assert_eq!(game.buildings[0].kind, BuildingKind::Factory);
        assert!(game.state.player(1).unwrap().gold < gold);
    }

    #[test]
    fn negative_gold_cannot_build_or_upgrade() {
        let mut game = engine(false);
        game.state.map = crate::map::GameMap::new(32, 32);
        for y in 0..32 {
            for x in 0..32 {
                game.state.map.set_owner_id(x, y, 1);
            }
        }
        game.state.player_mut(1).unwrap().gold = -1.0;
        game.buildings.push(building(1, BuildingKind::City, 1));

        game.apply_build_structure_intent(1, BuildingKind::Factory, 16 * 32 + 16);
        game.apply_upgrade_structure_intent(1, 1);

        assert_eq!(game.buildings.len(), 1);
        assert_eq!(game.buildings[0].level, 1);
        assert!(!game.buildings[0].under_construction);
        assert_eq!(game.state.player(1).unwrap().gold, -1.0);
    }

    #[test]
    fn farms_require_owned_lowland_and_allow_multiple_owned_plots() {
        let mut game = engine(false);
        game.state.map = crate::map::GameMap::new(20, 20);
        game.state.next_building_id = 3;
        let city_a = game.state.map.ref_id(4, 4) as u32;
        let city_b = game.state.map.ref_id(12, 12) as u32;
        game.buildings.push(Building {
            tile_idx: city_a,
            ..building(1, BuildingKind::City, 1)
        });
        game.buildings.push(Building {
            tile_idx: city_b,
            ..building(2, BuildingKind::City, 1)
        });
        for y in 0..20 {
            for x in 0..20 {
                let idx = game.state.map.ref_id(x, y);
                game.state.map.terrain[idx] = crate::map::MapTile::from_byte(0b1000_0000);
                game.state.map.set_owner_id(x, y, 1);
            }
        }
        game.state.map.set_owner_id(2, 2, 0);

        let unowned_tile = game.state.map.ref_id(2, 2) as u32;
        let empty_land_tile = game.state.map.ref_id(4, 12) as u32;
        let highland = game.state.map.ref_id(12, 4);
        game.state.map.terrain[highland] = crate::map::MapTile::from_byte(0x80 | 10);
        game.apply_build_structure_intent(1, BuildingKind::Farm, unowned_tile);
        game.apply_build_structure_intent(1, BuildingKind::Farm, highland as u32);
        assert_eq!(game.buildings.len(), 2);

        game.apply_build_structure_intent(1, BuildingKind::Farm, empty_land_tile);
        assert_eq!(game.buildings.len(), 3);
        assert_eq!(game.buildings[2].tile_idx, empty_land_tile);
        assert!(game.buildings[2].under_construction);

        // Occupied foundations reject both another Farm and overlap with a City.
        game.apply_build_structure_intent(1, BuildingKind::Farm, empty_land_tile);
        game.apply_build_structure_intent(1, BuildingKind::Farm, city_a);
        assert_eq!(game.buildings.len(), 3);

        game.state.map.terrain[highland] = crate::map::MapTile::from_byte(0x80);
        game.apply_build_structure_intent(1, BuildingKind::Farm, highland as u32);
        assert_eq!(game.buildings.len(), 4);
    }

    #[test]
    fn nuclear_launches_require_a_finished_factory_at_the_minimum_level() {
        let mut game = engine(false);
        game.state.player_mut(1).unwrap().player_type = crate::player::PlayerType::Bot;
        let required_level = crate::game::NukeKind::AtomBomb.required_district_level();
        game.buildings.push(building(1, BuildingKind::Factory, required_level.saturating_sub(1)));
        let gold = game.state.player(1).unwrap().gold;

        game.apply_building_production_intent(1, 1, CityArsenalKind::Nuke);
        game.apply_launch_nuke_intent(1, 4);
        assert!(game.projectiles.is_empty());
        assert_eq!(game.state.player(1).unwrap().gold, gold);

        game.buildings[0].level = required_level;
        game.buildings[0].under_construction = true;
        game.apply_building_production_intent(1, 1, CityArsenalKind::Nuke);
        game.apply_launch_nuke_intent(1, 4);
        assert!(game.projectiles.is_empty());

        game.buildings[0].under_construction = false;
        game.apply_building_production_intent(1, 1, CityArsenalKind::Nuke);
        assert_eq!(
            game.state.player(1).unwrap().gold,
            gold - game.state.config.nuke_cost
        );
        game.apply_launch_nuke_intent(1, 4);
        assert!(game.projectiles.is_empty());

        for _ in 0..CityArsenalKind::Nuke.duration_ticks() {
            game.execute_construction();
        }
        game.apply_launch_nuke_intent(1, 4);
        assert_eq!(game.projectiles.len(), 1);
        assert_eq!(
            game.state.player(1).unwrap().gold,
            gold - game.state.config.nuke_cost
        );
    }

    #[test]
    fn tower_nuclear_production_is_independent_of_a_city() {
        let mut game = engine(false);
        game.add_building(building(1, BuildingKind::Tower, 4));
        game.apply_building_production_intent(1, 1, CityArsenalKind::Nuke);
        for _ in 0..CityArsenalKind::Nuke.duration_ticks() {
            game.execute_construction();
        }
        game.apply_launch_nuke_intent(1, 4);
        assert_eq!(game.projectiles.len(), 1);
    }

    #[test]
    fn first_city_becomes_affordable_and_is_building_within_game_time_budget() {
        let mut game = pacing_match();
        let config = game.state.config.clone();
        assert_eq!(game.state.player(1).unwrap().gold, config.starting_gold);
        let city_tile = 2 * game.state.map.width + 8;
        let mut city_ordered = false;
        // Preserve the original game-time budget when the default speed or tick rate changes.
        let ticks = (52.8 / config.per_tick(1.0)).ceil() as u64;
        for _ in 0..ticks {
            game.execute_income();
            game.state.tick += 1;
            if !city_ordered && game.state.player(1).unwrap().gold >= config.cost_city {
                game.apply_build_structure_intent(1, BuildingKind::City, city_tile);
                city_ordered = true;
            }
        }
        assert!(
            city_ordered,
            "starting gold and ordinary income should afford a City within 52.8 game seconds"
        );
        assert!(game.buildings.iter().any(|building| {
            building.owner_id == 1
                && building.kind == BuildingKind::City
                && building.under_construction
        }));
        let player = game.state.player(1).unwrap();
        assert!(player.gold.is_finite() && player.gold >= 0.0);
        assert!(player.troops.is_finite() && player.max_troops.is_finite());
    }

    #[test]
    fn upgrades_charge_the_agreed_fraction_of_equivalent_new_production() {
        let mut game = pacing_match();
        game.add_building(building(1, BuildingKind::City, 1));
        game.add_building(Building {
            tile_idx: 1,
            ..building(2, BuildingKind::Factory, 1)
        });
        game.state.player_mut(1).unwrap().gold = 1_000.0;
        let config = game.state.config.clone();
        let upgrade_cost = structure_upgrade_cost_gold(BuildingKind::Factory, 2, 1, &config);
        let marginal = (1.45_f64 / 0.85).cbrt();
        let expected = structure_build_cost_gold(BuildingKind::Factory, 1, &config) * marginal * 0.75;
        assert!((upgrade_cost - expected).abs() < 1e-9);

        let gold = game.state.player(1).unwrap().gold;
        game.apply_upgrade_structure_intent(1, 2);
        assert!(game.buildings[1].under_construction);
        let after_upgrade = game.state.player(1).unwrap().gold;
        assert!((after_upgrade - (gold - upgrade_cost)).abs() < 1e-9);

        game.state.player_mut(1).unwrap().leader = crate::player::Leader::of("queen_nefertiti");
        let city_cost = structure_upgrade_cost_gold(BuildingKind::City, 2, 1, &config) * 0.65;
        let (quoted, discounted) = game.structure_upgrade_quote(1).unwrap();
        assert!(discounted && (quoted - city_cost).abs() < 1e-9);
        game.apply_upgrade_structure_intent(1, 1);
        assert!((game.state.player(1).unwrap().gold - (after_upgrade - city_cost)).abs() < 1e-9);
    }

    #[test]
    fn gold_income_does_not_charge_troop_upkeep_and_trade_ships_add_income() {
        fn trace(
            troops: f64,
            tiles: u32,
            factory_level: u8,
            trade_ships: u32,
            include_assisted_defeat: bool,
        ) -> [f64; 4] {
            let mut game = pacing_match();
            let player = game.state.player_mut(1).unwrap();
            player.troops = troops;
            player.tile_count = tiles;
            if factory_level > 0 {
                game.add_building(building(1, BuildingKind::Factory, factory_level));
            }
            if trade_ships > 0 {
                game.add_building(building(2, BuildingKind::Port, 4));
                for id in 1..=u64::from(trade_ships) {
                    game.add_fleet(crate::warp_fleet::WarpFleet::new(
                        id,
                        1,
                        0,
                        crate::game::UnitType::TradeShip,
                        0.0,
                        (0, 1),
                        vec![0, 1],
                    ));
                }
            }
            if include_assisted_defeat {
                game.state.player_mut(2).unwrap().player_type = crate::player::PlayerType::Nation;
                let config = game.state.config.clone();
                let mut assistant = crate::player::Player::new_human(
                    3,
                    "Assistant".into(),
                    [0.2, 0.7, 0.4],
                    &config,
                );
                assistant.tile_count = 1;
                assistant.gold = 0.0;
                game.state.register_player(assistant);
                game.state
                    .player_mut(1)
                    .unwrap()
                    .tile_conquests
                    .insert(2, 5);
                game.state
                    .player_mut(3)
                    .unwrap()
                    .tile_conquests
                    .insert(2, 1);
            }
            let mut checkpoints = [0.0; 4];
            for tick in 1..=6_000 {
                game.state.tick = tick;
                game.execute_income();
                if include_assisted_defeat && tick == 1_200 {
                    game.eliminate_player(2, 1, 0, 0, false);
                    assert!(matches!(
                        game.state.events.last(),
                        Some(crate::game::GameEvent::PlayerEliminated {
                            gold_bounty: 75,
                            assists,
                            ..
                        }) if assists == &vec![(3, 75)]
                    ));
                }
                if matches!(tick, 300 | 1_200 | 3_600 | 6_000) {
                    let player = game.state.player(1).unwrap();
                    eprintln!(
                        "economy tick={tick} tiles={} troops={:.0} gold={:.2} trade={trade_ships}",
                        player.tile_count, player.troops, player.gold,
                    );
                }
                match tick {
                    300 => checkpoints[0] = game.state.player(1).unwrap().gold,
                    1_200 => checkpoints[1] = game.state.player(1).unwrap().gold,
                    3_600 => checkpoints[2] = game.state.player(1).unwrap().gold,
                    6_000 => checkpoints[3] = game.state.player(1).unwrap().gold,
                    _ => {}
                }
            }
            checkpoints
        }

        let early = trace(1_000.0, 32, 0, 0, false);
        let large_army_same_territory = trace(100_000.0, 32, 0, 0, false);
        let growing_army = trace(25_000.0, 1_024, 0, 0, true);
        let industrial_trade = trace(100_000.0, 10_000, 4, 3, false);
        eprintln!(
            "gold at 30s/2m/6m/10m: early={early:?}, growing_army={growing_army:?}, industrial_trade={industrial_trade:?}"
        );

        assert!(early.iter().all(|gold| gold.is_finite()));
        assert!(growing_army.iter().all(|gold| gold.is_finite()));
        assert!(industrial_trade.iter().all(|gold| gold.is_finite()));
        assert_eq!(early, large_army_same_territory);
        assert!(early.windows(2).all(|pair| pair[1] > pair[0]));
        assert!(growing_army.windows(2).all(|pair| pair[1] > pair[0]));
        assert!(industrial_trade.windows(2).all(|pair| pair[1] > pair[0]));
        assert!(industrial_trade[1] > industrial_trade[0]);
    }
}
