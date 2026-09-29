use crate::building::{
    Building, CityModules, ModuleKind, resolve_structure_spawn_tile, structure_build_cost_gold,
    structure_upgrade_cost_gold,
    structure_kind_enabled,
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

        // Placement is intentionally separate from upgrades. A nearby building
        // never absorbs a new foundation order.
        self.refresh_building_grid();
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
        let dur = kind.construction_duration_ticks();
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

        let cost = structure_upgrade_cost_gold(
            building.kind,
            target_level,
            &self.state.config,
        );
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
        b.ticks_until_complete = crate::building::core::upgrade_duration_ticks(
            b.kind,
            target_level,
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

        let current_level = self.state.map.tile_upgrades[tile_idx as usize];
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

        self.state.map.tile_upgrades[tile_idx as usize] = new_level;
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
}
