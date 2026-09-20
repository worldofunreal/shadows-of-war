use super::placement::{PlacementQuery, resolve_build_target_tile};
use crate::app::{MapContextMenu, SowApp};
use serde::Deserialize;

pub(crate) const TOUCH_HOLD_MS: u128 = 300;
const MAP_CLICK_MAX_DISTANCE_SQ: f64 = 400.0;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MapMenuAction {
    Spawn,
    Attack,
    Fleet,
    Transfer,
    Alliance,
    BuildCity,
    BuildFactory,
    BuildPort,
    BuildBunker,
    Nuke,
    UpgradeTile,
    UpgradeArsenal,
    UpgradePort,
    UpgradeFoundry,
    BuildWarship,
    BuildTradeShip,
}

impl MapMenuAction {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Spawn => "spawn",
            Self::Attack => "attack",
            Self::Fleet => "fleet",
            Self::Transfer => "transfer",
            Self::Alliance => "alliance",
            Self::BuildCity => "build_city",
            Self::BuildFactory => "build_factory",
            Self::BuildPort => "build_port",
            Self::BuildBunker => "build_bunker",
            Self::Nuke => "nuke",
            Self::UpgradeTile => "upgrade_tile",
            Self::UpgradeArsenal => "upgrade_arsenal",
            Self::UpgradePort => "upgrade_port",
            Self::UpgradeFoundry => "upgrade_foundry",
            Self::BuildWarship => "build_warship",
            Self::BuildTradeShip => "build_trade_ship",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct MapMenuItem {
    pub action: MapMenuAction,
    pub cost: Option<f64>,
    pub level: Option<u8>,
    pub disabled: bool,
}

pub(crate) fn is_quick_tap(elapsed_ms: u128, distance_sq: f64) -> bool {
    elapsed_ms < TOUCH_HOLD_MS && distance_sq <= MAP_CLICK_MAX_DISTANCE_SQ
}

#[derive(Clone, Copy)]
struct MapTarget {
    owner: u16,
    is_land: bool,
    my_id: u16,
    is_allied: bool,
    is_teammate: bool,
    has_alliance_request: bool,
    has_proposed_alliance: bool,
    is_in_renewal_window: bool,
}

impl MapTarget {
    fn is_friendly(self) -> bool {
        self.is_allied || self.is_teammate
    }

    fn is_player(self) -> bool {
        self.owner != 0 && self.owner != self.my_id
    }

    fn is_enemy(self) -> bool {
        self.is_player() && !self.is_friendly()
    }

    fn is_attackable(self) -> bool {
        self.owner == 0 || self.is_enemy()
    }

    fn menu_actions(
        self,
        spawning: bool,
        can_attack: bool,
        can_fleet: bool,
    ) -> Vec<MapMenuAction> {
        if spawning {
            return self
                .is_land
                .then_some(MapMenuAction::Spawn)
                .into_iter()
                .collect();
        }
        if self.owner == self.my_id && self.is_land {
            return vec![
                MapMenuAction::BuildCity,
                MapMenuAction::BuildFactory,
                MapMenuAction::BuildPort,
                MapMenuAction::BuildBunker,
            ];
        }
        if self.owner == 0 {
            let mut actions = Vec::new();
            if self.is_land && can_attack {
                actions.push(MapMenuAction::Attack);
            }
            if can_fleet {
                actions.push(MapMenuAction::Fleet);
            }
            return actions;
        }
        if !self.is_player() {
            return Vec::new();
        }

        let mut actions = Vec::new();
        actions.push(MapMenuAction::Transfer);
        if self.is_friendly() {
            if self.is_allied && self.is_land {
                actions.push(MapMenuAction::Attack);
            }
            if !self.is_teammate {
                actions.push(MapMenuAction::Fleet);
            }
        } else {
            if self.is_land && can_attack {
                actions.push(MapMenuAction::Attack);
            }
            actions.push(MapMenuAction::Fleet);
            if self.is_land {
                actions.push(MapMenuAction::Nuke);
            }
        }
        if !self.is_teammate {
            actions.push(MapMenuAction::Alliance);
        }
        actions
    }
}

impl SowApp {
    fn show_observer_notice(&mut self, x: f64, y: f64) {
        const MESSAGES: [&str; 8] = [
            "Enjoying the view? 🍿",
            "Best seat in the house! 🏟️",
            "Wave at the players! 👋",
            "The crowd goes wild! 🎉",
            "Grab some popcorn! 🍿",
            "Great game to watch! ⭐",
            "Cheer them on! 📣",
            "Spectating in style! 😎",
        ];
        let message = MESSAGES[(x + y) as usize % MESSAGES.len()];
        self.add_notice_at_screen(message, x, y, 1500, crate::rgb(203, 213, 225));
    }

    pub(crate) fn try_attack_at(&mut self, x: f64, y: f64) -> bool {
        if self.ui.observing
            || self.ui.app.phase != crate::ClientPhase::Playing
            || self.ui.app.hud_state.selected_building_kind.is_some()
            || self.ui.app.hud_state.selected_nuke_kind.is_some()
        {
            return false;
        }
        if !self
            .sim
            .current_snapshot
            .as_ref()
            .is_some_and(|snapshot| matches!(snapshot.phase, sow_core::game::GamePhase::Playing))
        {
            return false;
        }
        let Some((col, row)) = self.mouse_to_tile(x, y) else {
            return false;
        };
        let tile_idx = (row * self.sim.map_w as i32 + col) as u32;
        let Some(target) = self.map_target(tile_idx) else {
            return false;
        };
        if !target.is_land || target.owner == target.my_id || target.is_friendly() {
            return false;
        }
        self.attack_from_tile(tile_idx, (x, y))
    }

    pub(crate) fn handle_map_click(&mut self, x: f64, y: f64) {
        if self.ui.observing {
            self.show_observer_notice(x, y);
            self.clear_placement();
            return;
        }

        let Some((col, row)) = self.mouse_to_tile(x, y) else {
            return;
        };
        let is_spawning = self.sim.current_snapshot.as_ref().is_some_and(|snapshot| {
            matches!(snapshot.phase, sow_core::game::GamePhase::Spawning { .. })
        });

        if is_spawning {
            self.spawn_at(col, row, (x, y));
            return;
        }

        let tile_idx = (row * self.sim.map_w as i32 + col) as u32;
        if let Some(kind) = self.ui.app.hud_state.selected_nuke_kind {
            self.launch_nuke_at(kind, tile_idx);
            return;
        }
        if let Some(kind) = self.ui.app.hud_state.selected_building_kind {
            self.build_structure_at(kind, col, row, (x, y));
            return;
        }

        if self.select_warships_at(x, y) {
            return;
        }
        self.input.selected_warships.clear();

        if self
            .sim
            .current_snapshot
            .as_ref()
            .is_some_and(|snapshot| matches!(snapshot.phase, sow_core::game::GamePhase::Playing))
        {
            self.primary_target(tile_idx, (x, y));
        }
    }

    pub(crate) fn open_map_context_menu(&mut self, x: f64, y: f64) {
        if self.ui.observing {
            self.show_observer_notice(x, y);
            return;
        }
        if self.ui.app.phase != crate::ClientPhase::Playing {
            return;
        }
        let Some((col, row)) = self.mouse_to_tile(x, y) else {
            self.close_map_context_menu();
            return;
        };
        let tile_idx = (row * self.sim.map_w as i32 + col) as u32;
        if self.map_menu_actions(tile_idx).is_empty() {
            self.show_map_menu_unavailable(tile_idx, (x, y));
            self.close_map_context_menu();
            return;
        }
        let session = self.input.map_context_menu_session.wrapping_add(1);
        self.input.map_context_menu_session = session;
        self.input.map_context_menu = Some(MapContextMenu {
            x: x as f32,
            y: y as f32,
            tile_idx,
            session,
        });
    }

    pub(crate) fn close_map_context_menu(&mut self) {
        self.input.map_context_menu = None;
    }

    fn fleet_route_check(
        &mut self,
        tile_idx: u32,
        target_owner: u16,
    ) -> Result<(), sow_core::warp_fleet::FleetLaunchError> {
        let player_id = self.sim.my_player_id.unwrap_or(0);
        let Some(engine) = self.sim.engine.as_mut() else {
            return Err(sow_core::warp_fleet::FleetLaunchError::NoWaterAccess);
        };
        let border_tiles = engine
            .state
            .player(player_id)
            .map(|player| &player.border_tiles)
            .ok_or(sow_core::warp_fleet::FleetLaunchError::NoWaterAccess)?;
        let target_border = if target_owner == 0 {
            None
        } else {
            Some(
                engine
                    .state
                    .player(target_owner)
                    .map(|player| &player.border_tiles)
                    .ok_or(sow_core::warp_fleet::FleetLaunchError::TargetPlayerNotFound {
                        target_owner,
                    })?,
            )
        };
        sow_core::warp_fleet::resolve_fleet_route(
            &engine.state.map,
            &engine.water,
            &mut engine.path_scratch,
            player_id,
            (target_owner, tile_idx),
            border_tiles,
            target_border,
        )
        .map(|_| ())
    }

    fn show_fleet_unavailable(
        &mut self,
        error: sow_core::warp_fleet::FleetLaunchError,
        anchor: (f64, f64),
    ) {
        self.add_notice_at_screen(
            format!("Fleet unavailable: {error}."),
            anchor.0,
            anchor.1,
            2000,
            crate::rgb(248, 113, 113),
        );
    }

    fn show_map_menu_unavailable(&mut self, tile_idx: u32, anchor: (f64, f64)) {
        let message = match self.map_target(tile_idx) {
            Some(target) if target.owner == 0 => match self.fleet_route_check(tile_idx, 0) {
                Err(error) => format!("Fleet unavailable: {error}."),
                Ok(()) => "No action is available here.".to_string(),
            },
            Some(target) if target.is_teammate => {
                "Teammates cannot be targeted. 🤝".to_string()
            }
            Some(target) if target.owner == target.my_id && !target.is_land => {
                "Buildings require owned land. 🗺️".to_string()
            }
            Some(target) if target.owner == target.my_id => {
                let building = self.sim.current_snapshot.as_ref().and_then(|snapshot| {
                    snapshot
                        .buildings
                        .iter()
                        .find(|building| building.tile_idx == tile_idx)
                });
                match building {
                    Some(building) if building.under_construction => {
                        "Building under construction. 🏗️".to_string()
                    }
                    Some(building) => format!(
                        "{} level {} has no available action.",
                        building.kind.as_str(),
                        building.level
                    ),
                    None => "No construction action is available here.".to_string(),
                }
            }
            Some(_) => "No action is available here.".to_string(),
            None => "No action is available here.".to_string(),
        };
        self.add_notice_at_screen(
            message,
            anchor.0,
            anchor.1,
            2000,
            crate::rgb(248, 113, 113),
        );
    }

    pub(crate) fn map_menu_actions(&mut self, tile_idx: u32) -> Vec<MapMenuAction> {
        let Some(target) = self.map_target(tile_idx) else {
            return Vec::new();
        };
        let spawning = self.sim.current_snapshot.as_ref().is_some_and(|snapshot| {
            matches!(snapshot.phase, sow_core::game::GamePhase::Spawning { .. })
        });
        let can_fleet = !spawning
            && target.owner == 0
            && self.fleet_route_check(tile_idx, target.owner).is_ok();
        let mut actions = target.menu_actions(
            spawning,
            target.is_land && self.can_attack(tile_idx, target.owner),
            can_fleet,
        );
        if target.owner != target.my_id || !target.is_land || spawning {
            return actions;
        }

        actions.clear();

        let building = self
            .sim
            .current_snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.buildings.iter().find(|b| b.tile_idx == tile_idx));
        match building {
            Some(building) if !building.under_construction => match building.kind {
                sow_core::game::BuildingKind::City => {
                    for module in [
                        sow_core::building::ModuleKind::Arsenal,
                        sow_core::building::ModuleKind::Port,
                        sow_core::building::ModuleKind::Foundry,
                    ] {
                        if self.city_module_is_available(building, module, tile_idx) {
                            actions.push(match module {
                                sow_core::building::ModuleKind::Arsenal => {
                                    MapMenuAction::UpgradeArsenal
                                }
                                sow_core::building::ModuleKind::Port => MapMenuAction::UpgradePort,
                                sow_core::building::ModuleKind::Foundry => {
                                    MapMenuAction::UpgradeFoundry
                                }
                                _ => continue,
                            });
                        }
                    }
                    if building.modules.port > 0 {
                        actions
                            .extend([MapMenuAction::BuildWarship, MapMenuAction::BuildTradeShip]);
                    }
                }
                sow_core::game::BuildingKind::Port => {
                    actions.extend([MapMenuAction::BuildWarship, MapMenuAction::BuildTradeShip]);
                }
                _ => {}
            },
            Some(_) => {}
            None => {
                actions.extend([
                    MapMenuAction::UpgradeTile,
                    MapMenuAction::BuildCity,
                    MapMenuAction::BuildFactory,
                    MapMenuAction::BuildPort,
                    MapMenuAction::BuildBunker,
                ]);
            }
        }
        actions
    }

    pub(crate) fn map_menu_items(&mut self, tile_idx: u32) -> Vec<MapMenuItem> {
        self.map_menu_actions(tile_idx)
            .into_iter()
            .map(|action| {
                let (cost, level) = self.map_menu_cost(action, tile_idx);
                MapMenuItem {
                    action,
                    cost,
                    level,
                    disabled: cost.is_some_and(|value| {
                        !value.is_finite() || self.ui.app.hud_state.gold < value
                    }),
                }
            })
            .collect()
    }

    pub(crate) fn handle_map_menu_action(
        &mut self,
        session: u64,
        tile_idx: u32,
        action: MapMenuAction,
    ) {
        let Some(menu) = self.input.map_context_menu else {
            return;
        };
        if menu.session != session || menu.tile_idx != tile_idx {
            return;
        }
        let anchor = (menu.x as f64, menu.y as f64);
        if !self.map_menu_actions(tile_idx).contains(&action) {
            self.show_map_menu_unavailable(tile_idx, anchor);
            self.close_map_context_menu();
            return;
        }
        if let Some(cost) = self.map_menu_cost(action, tile_idx).0
            && (!cost.is_finite() || self.ui.app.hud_state.gold < cost)
        {
            let message = if cost.is_finite() {
                format!("Need {} gold.", crate::utils::format_number(cost))
            } else {
                "Action unavailable here.".to_string()
            };
            self.add_notice_at_screen(
                message,
                anchor.0,
                anchor.1,
                2000,
                crate::rgb(248, 113, 113),
            );
            self.close_map_context_menu();
            return;
        }

        match action {
            MapMenuAction::Spawn => {
                if let Some((col, row)) = self.tile_coords(tile_idx) {
                    self.spawn_at(col, row, anchor);
                }
            }
            MapMenuAction::Attack => {
                self.attack_from_tile(tile_idx, anchor);
            }
            MapMenuAction::Fleet => {
                self.launch_fleet_from_tile(tile_idx, anchor);
            }
            MapMenuAction::Transfer => {
                self.open_transfer_from_tile(tile_idx);
            }
            MapMenuAction::Alliance => {
                self.alliance_from_tile(tile_idx, anchor);
            }
            MapMenuAction::BuildCity
            | MapMenuAction::BuildFactory
            | MapMenuAction::BuildPort
            | MapMenuAction::BuildBunker => {
                let kind = match action {
                    MapMenuAction::BuildCity => sow_core::game::BuildingKind::City,
                    MapMenuAction::BuildFactory => sow_core::game::BuildingKind::Factory,
                    MapMenuAction::BuildPort => sow_core::game::BuildingKind::Port,
                    MapMenuAction::BuildBunker => sow_core::game::BuildingKind::Bunker,
                    _ => unreachable!(),
                };
                self.ui.app.hud_state.selected_building_kind = Some(kind);
                self.ui.app.hud_state.selected_nuke_kind = None;
                self.input.hold_build_active = false;
                self.input.hold_build_accum = 0.0;
            }
            MapMenuAction::Nuke => {
                self.launch_nuke_at(sow_core::game::NukeKind::AtomBomb, tile_idx);
            }
            MapMenuAction::UpgradeTile => {
                self.upgrade_tile_at(tile_idx);
            }
            MapMenuAction::UpgradeArsenal
            | MapMenuAction::UpgradePort
            | MapMenuAction::UpgradeFoundry => {
                let module = match action {
                    MapMenuAction::UpgradeArsenal => sow_core::building::ModuleKind::Arsenal,
                    MapMenuAction::UpgradePort => sow_core::building::ModuleKind::Port,
                    MapMenuAction::UpgradeFoundry => sow_core::building::ModuleKind::Foundry,
                    _ => unreachable!(),
                };
                self.upgrade_city_module_at(tile_idx, module);
            }
            MapMenuAction::BuildWarship | MapMenuAction::BuildTradeShip => {
                let kind = match action {
                    MapMenuAction::BuildWarship => sow_core::game::UnitType::Warship,
                    MapMenuAction::BuildTradeShip => sow_core::game::UnitType::TradeShip,
                    _ => unreachable!(),
                };
                self.build_ship_at(tile_idx, kind);
            }
        }
        self.close_map_context_menu();
    }

    pub(crate) fn move_selected_warships(&mut self, x: f64, y: f64) -> bool {
        if self.input.selected_warships.is_empty() {
            return false;
        }
        let Some((col, row)) = self.mouse_to_tile(x, y) else {
            return false;
        };
        let target_tile = (row * self.sim.map_w as i32 + col) as u32;
        let unit_ids = std::mem::take(&mut self.input.selected_warships);
        self.send_intent(sow_core::protocol::GameplayIntent::MoveWarships {
            unit_ids,
            target_tile,
        });
        true
    }

    fn spawn_at(&mut self, col: i32, row: i32, anchor: (f64, f64)) {
        let idx = (row * self.sim.map_w as i32 + col) as usize;
        let Some(renderer) = self.gfx.map_renderer.as_ref() else {
            return;
        };
        let is_land = renderer
            .terrain
            .get(idx)
            .is_some_and(|terrain| terrain & 0x80 != 0);
        if !is_land {
            self.show_water_feedback(col, row, anchor);
            return;
        }

        let owner = renderer.owners.get(idx).copied().unwrap_or(0);
        let (target_col, target_row) = if owner == 0 {
            (col, row)
        } else {
            let mut best_tile = None;
            let mut best_dist = i32::MAX;
            for dy in -5..=5 {
                for dx in -5..=5 {
                    let tx = col + dx;
                    let ty = row + dy;
                    if tx < 0
                        || tx >= self.sim.map_w as i32
                        || ty < 0
                        || ty >= self.sim.map_h as i32
                    {
                        continue;
                    }
                    let dist = sow_core::building::hex_distance(col, row, tx, ty);
                    let n_idx = (ty * self.sim.map_w as i32 + tx) as usize;
                    let free_land = self.gfx.map_renderer.as_ref().is_some_and(|mr| {
                        mr.owners.get(n_idx).copied() == Some(0)
                            && mr
                                .terrain
                                .get(n_idx)
                                .is_some_and(|terrain| terrain & 0x80 != 0)
                    });
                    if dist <= 5 && free_land && dist < best_dist {
                        best_dist = dist;
                        best_tile = Some((tx, ty));
                    }
                }
            }
            let Some(tile) = best_tile else {
                self.add_click_marker(col, row);
                const MESSAGES: [&str; 6] = [
                    "Hey! Too close to another player! 🛡️",
                    "Respect boundaries! 🤝",
                    "Get your own space! 🏕️",
                    "Social distancing! ↔️",
                    "Spawning blocked! 🛑",
                    "Private property! 🚫",
                ];
                let message = MESSAGES[(anchor.0 + anchor.1) as usize % MESSAGES.len()];
                self.add_notice_at_screen(
                    message,
                    anchor.0,
                    anchor.1,
                    1500,
                    crate::rgb(248, 113, 113),
                );
                return;
            };
            tile
        };

        self.send_intent(sow_core::protocol::GameplayIntent::Spawn {
            x: target_col as u32,
            y: target_row as u32,
        });
    }

    fn build_structure_at(
        &mut self,
        kind: sow_core::game::BuildingKind,
        col: i32,
        row: i32,
        anchor: (f64, f64),
    ) -> bool {
        let Some(snapshot) = self.sim.current_snapshot.as_ref() else {
            return false;
        };
        let my_id = self.sim.my_player_id.unwrap_or(0);
        let owners = self
            .gfx
            .map_renderer
            .as_ref()
            .map(|renderer| renderer.owners.as_slice())
            .unwrap_or(&[]);
        let terrain = self
            .gfx
            .map_renderer
            .as_ref()
            .map(|renderer| renderer.terrain.as_slice())
            .unwrap_or(&[]);
        let target_res = resolve_build_target_tile(&PlacementQuery {
            kind,
            click_x: col,
            click_y: row,
            map_w: self.sim.map_w,
            map_h: self.sim.map_h,
            owners,
            terrain,
            my_id,
            buildings: &snapshot.buildings,
        });
        let cost_index = sow_core::game::BuildingKind::ALL
            .iter()
            .position(|candidate| *candidate == kind)
            .unwrap_or(0);
        if self.ui.app.hud_state.gold < self.ui.app.hud_state.building_costs[cost_index] {
            let text = format!(
                "Need {} gold.",
                crate::utils::format_number(self.ui.app.hud_state.building_costs[cost_index])
            );
            self.add_notice_at_screen(text, anchor.0, anchor.1, 2000, crate::rgb(248, 113, 113));
            return false;
        }
        let target_tile = match target_res {
            Ok(target_tile) => target_tile,
            Err(message) => {
                self.add_notice_at_screen(
                    message,
                    anchor.0,
                    anchor.1,
                    2000,
                    crate::rgb(248, 113, 113),
                );
                return false;
            }
        };
        self.send_intent(sow_core::protocol::GameplayIntent::BuildStructure { kind, target_tile });
        true
    }

    fn city_module_is_available(
        &self,
        building: &sow_core::protocol::BuildingSnapshot,
        module: sow_core::building::ModuleKind,
        tile_idx: u32,
    ) -> bool {
        let current_level = building.modules.get_level(module);
        let next_level = current_level.saturating_add(1);
        if next_level > 5 || (module == sow_core::building::ModuleKind::Arsenal && next_level > 3) {
            return false;
        }
        if module == sow_core::building::ModuleKind::Arsenal && building.level < 3 {
            return false;
        }
        if module == sow_core::building::ModuleKind::Port {
            return self
                .gfx
                .map_renderer
                .as_ref()
                .and_then(|renderer| renderer.terrain.get(tile_idx as usize))
                .is_some_and(|terrain| terrain & 0xc0 == 0xc0);
        }
        true
    }

    fn map_menu_cost(&self, action: MapMenuAction, tile_idx: u32) -> (Option<f64>, Option<u8>) {
        match action {
            MapMenuAction::UpgradeTile => {
                let level = self
                    .sim
                    .tile_upgrades
                    .get(tile_idx as usize)
                    .copied()
                    .unwrap_or(0) as i32;
                let cost = (1000.0 * 1.5_f64.powi(level)) / sow_core::config::GOLD_SCALE.max(1.0);
                (Some(cost), Some(level as u8))
            }
            MapMenuAction::UpgradeArsenal
            | MapMenuAction::UpgradePort
            | MapMenuAction::UpgradeFoundry => {
                let module = match action {
                    MapMenuAction::UpgradeArsenal => sow_core::building::ModuleKind::Arsenal,
                    MapMenuAction::UpgradePort => sow_core::building::ModuleKind::Port,
                    MapMenuAction::UpgradeFoundry => sow_core::building::ModuleKind::Foundry,
                    _ => unreachable!(),
                };
                let level = self
                    .sim
                    .current_snapshot
                    .as_ref()
                    .and_then(|snapshot| {
                        snapshot
                            .buildings
                            .iter()
                            .find(|building| building.tile_idx == tile_idx)
                    })
                    .map(|building| building.modules.get_level(module))
                    .unwrap_or(0);
                (
                    Some(sow_core::building::cost::module_upgrade_cost_gold(
                        module,
                        level.saturating_add(1),
                    )),
                    Some(level),
                )
            }
            MapMenuAction::BuildWarship => {
                (Some(sow_core::game::UnitType::Warship.gold_cost()), None)
            }
            MapMenuAction::BuildTradeShip => {
                (Some(sow_core::game::UnitType::TradeShip.gold_cost()), None)
            }
            MapMenuAction::BuildCity
            | MapMenuAction::BuildFactory
            | MapMenuAction::BuildPort
            | MapMenuAction::BuildBunker => {
                let kind = match action {
                    MapMenuAction::BuildCity => sow_core::game::BuildingKind::City,
                    MapMenuAction::BuildFactory => sow_core::game::BuildingKind::Factory,
                    MapMenuAction::BuildPort => sow_core::game::BuildingKind::Port,
                    MapMenuAction::BuildBunker => sow_core::game::BuildingKind::Bunker,
                    _ => unreachable!(),
                };
                let owner = self.sim.my_player_id.unwrap_or(0);
                let count = self
                    .sim
                    .current_snapshot
                    .as_ref()
                    .map(|snapshot| {
                        snapshot
                            .buildings
                            .iter()
                            .filter(|building| building.owner_id == owner && building.kind == kind)
                            .map(|building| building.level as u32)
                            .sum()
                    })
                    .unwrap_or(0);
                (
                    Some(sow_core::building::cost::structure_build_cost_gold(
                        kind,
                        count,
                        &self.sim.config,
                    )),
                    None,
                )
            }
            _ => (None, None),
        }
    }

    fn upgrade_tile_at(&mut self, tile_idx: u32) -> bool {
        let Some(target) = self.map_target(tile_idx) else {
            return false;
        };
        if !target.is_land || target.owner != target.my_id {
            return false;
        }
        self.send_intent(sow_core::protocol::GameplayIntent::UpgradeTile { tile_idx });
        true
    }

    fn upgrade_city_module_at(
        &mut self,
        tile_idx: u32,
        module: sow_core::building::ModuleKind,
    ) -> bool {
        let Some(target) = self.map_target(tile_idx) else {
            return false;
        };
        if target.owner != target.my_id || !target.is_land {
            return false;
        }
        let Some(building) = self
            .sim
            .current_snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.buildings.iter().find(|b| b.tile_idx == tile_idx))
        else {
            return false;
        };
        if building.kind != sow_core::game::BuildingKind::City
            || building.under_construction
            || !self.city_module_is_available(building, module, tile_idx)
        {
            return false;
        }
        self.send_intent(sow_core::protocol::GameplayIntent::UpgradeCityModule {
            building_id: building.id,
            module,
        });
        true
    }

    fn build_ship_at(&mut self, tile_idx: u32, kind: sow_core::game::UnitType) -> bool {
        let Some(target) = self.map_target(tile_idx) else {
            return false;
        };
        if target.owner != target.my_id || !target.is_land {
            return false;
        }
        let ready_port = self
            .sim
            .current_snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.buildings.iter().find(|b| b.tile_idx == tile_idx))
            .is_some_and(|building| {
                !building.under_construction
                    && (building.kind == sow_core::game::BuildingKind::Port
                        || (building.kind == sow_core::game::BuildingKind::City
                            && building.modules.port > 0))
            });
        if !ready_port {
            return false;
        }
        self.send_intent(sow_core::protocol::GameplayIntent::BuildShip {
            port_tile: tile_idx,
            kind,
        });
        true
    }

    fn launch_nuke_at(&mut self, kind: sow_core::game::NukeKind, tile_idx: u32) -> bool {
        let Some(target) = self.map_target(tile_idx) else {
            return false;
        };
        if !target.is_land || !target.is_enemy() {
            return false;
        }
        self.send_intent(sow_core::protocol::GameplayIntent::LaunchNuke {
            kind,
            target_tile: tile_idx,
        });
        self.ui.app.hud_state.selected_nuke_kind = None;
        true
    }

    fn primary_target(&mut self, tile_idx: u32, anchor: (f64, f64)) {
        let Some(target) = self.map_target(tile_idx) else {
            return;
        };
        if !target.is_land {
            if let Some((col, row)) = self.tile_coords(tile_idx) {
                self.show_water_feedback(col, row, anchor);
            }
            return;
        }
        if target.owner == target.my_id {
            return;
        }
        if target.is_friendly() {
            self.open_transfer_from_tile(tile_idx);
        } else {
            self.attack_from_tile(tile_idx, anchor);
        }
    }

    fn attack_from_tile(&mut self, tile_idx: u32, anchor: (f64, f64)) -> bool {
        let Some(target) = self.map_target(tile_idx) else {
            return false;
        };
        if !target.is_land {
            if let Some((col, row)) = self.tile_coords(tile_idx) {
                self.show_water_feedback(col, row, anchor);
            }
            return false;
        }
        if target.is_teammate {
            return false;
        }
        let troops = self.ui.app.hud_state.troops * self.ui.app.hud_state.attack_ratio as f64;
        if troops <= 0.0 {
            return false;
        }
        let intent = sow_core::protocol::GameplayIntent::Attack(sow_core::protocol::AttackIntent {
            target_owner: target.owner,
            troops: Some(troops),
        });
        if target.is_allied {
            self.ui.app.hud_state.show_betrayal_warning = Some((target.owner, intent));
            return true;
        }
        if !target.is_attackable() || !self.can_attack(tile_idx, target.owner) {
            const MESSAGES: [&str; 5] = [
                "Too far! 🌌",
                "Out of reach! 🏃‍♂️",
                "No border, no battle! ⚔️",
                "Teleportation not researched! 📡",
                "Build a path first! 🗺️",
            ];
            let message = MESSAGES[(anchor.0 + anchor.1) as usize % MESSAGES.len()];
            self.add_notice_at_screen(message, anchor.0, anchor.1, 1500, crate::rgb(248, 113, 113));
            return false;
        }
        let text = format!("⚔️ +{}", crate::utils::format_number(troops));
        self.add_notice_at_screen(text, anchor.0, anchor.1, 1500, crate::rgb(6, 182, 212));
        self.send_intent(intent);
        true
    }

    pub(crate) fn launch_fleet_from_tile(&mut self, tile_idx: u32, anchor: (f64, f64)) -> bool {
        let Some(target) = self.map_target(tile_idx) else {
            return false;
        };
        if target.is_teammate {
            self.add_notice_at_screen(
                "Teammates cannot be targeted. 🤝",
                anchor.0,
                anchor.1,
                2000,
                crate::rgb(248, 113, 113),
            );
            return false;
        }
        if target.is_allied {
            self.add_notice_at_screen(
                "Break the alliance before launching a fleet. 🛡️",
                anchor.0,
                anchor.1,
                2000,
                crate::rgb(248, 113, 113),
            );
            return false;
        }
        if target.owner != 0 && !target.is_enemy() {
            self.add_notice_at_screen(
                "A fleet cannot target your own territory. 🛡️",
                anchor.0,
                anchor.1,
                2000,
                crate::rgb(248, 113, 113),
            );
            return false;
        }
        if let Err(error) = self.fleet_route_check(tile_idx, target.owner) {
            self.show_fleet_unavailable(error, anchor);
            return false;
        }
        let troops = self.ui.app.hud_state.troops * self.ui.app.hud_state.attack_ratio as f64;
        if troops < self.sim.config.attack_cost_neutral {
            self.add_notice_at_screen(
                format!(
                    "Need at least {} troops for a fleet. 🚢",
                    crate::utils::format_number(self.sim.config.attack_cost_neutral)
                ),
                anchor.0,
                anchor.1,
                2000,
                crate::rgb(248, 113, 113),
            );
            return false;
        }
        self.send_intent(sow_core::protocol::GameplayIntent::LaunchFleet {
            target_tile: tile_idx,
            troops: Some(troops),
        });
        true
    }

    fn open_transfer_from_tile(&mut self, tile_idx: u32) -> bool {
        let Some(target) = self.map_target(tile_idx) else {
            return false;
        };
        if !target.is_friendly() {
            self.add_notice_at_screen(
                "Resources can only be sent to allies. ⚖️",
                self.input.last_mouse_x,
                self.input.last_mouse_y,
                2000,
                crate::rgb(248, 113, 113),
            );
            return false;
        }
        self.ui.app.hud_state.show_ask_panel = Some(target.owner);
        if target.is_allied || target.is_teammate {
            if let Some(player) = self.sim.current_snapshot.as_ref().and_then(|snapshot| {
                snapshot
                    .players
                    .iter()
                    .find(|player| player.id == target.owner)
            }) {
                self.ui.app.hud_state.ask_gold = (player.gold * 0.10).floor();
                self.ui.app.hud_state.ask_troops = (player.troops * 0.10).floor();
            }
        } else {
            self.ui.app.hud_state.ask_gold = 0.0;
            self.ui.app.hud_state.ask_troops = 0.0;
        }
        self.ui.app.hud_state.transfer_confirm_pending = false;
        true
    }

    fn alliance_from_tile(&mut self, tile_idx: u32, anchor: (f64, f64)) -> bool {
        let Some(target) = self.map_target(tile_idx) else {
            return false;
        };
        if !target.is_player() || target.is_teammate {
            return false;
        }
        if target.is_allied {
            if target.is_in_renewal_window {
                if target.has_alliance_request {
                    self.send_intent(sow_core::protocol::GameplayIntent::AcceptAlliance {
                        target_player: target.owner,
                    });
                } else if target.has_proposed_alliance {
                    self.add_notice_at_screen(
                        "Alliance renewal is already pending.",
                        anchor.0,
                        anchor.1,
                        2000,
                        crate::rgb(248, 113, 113),
                    );
                } else {
                    self.send_intent(sow_core::protocol::GameplayIntent::ProposeAlliance {
                        target_player: target.owner,
                    });
                    self.add_notice_at_screen(
                        "Alliance renewal requested. 🤝",
                        anchor.0,
                        anchor.1,
                        2000,
                        crate::rgb(74, 222, 128),
                    );
                }
            } else {
                self.send_intent(sow_core::protocol::GameplayIntent::BreakAlliance {
                    target_player: target.owner,
                });
            }
        } else if target.has_alliance_request {
            self.send_intent(sow_core::protocol::GameplayIntent::AcceptAlliance {
                target_player: target.owner,
            });
        } else if target.has_proposed_alliance {
            self.add_notice_at_screen(
                "Alliance request already pending.",
                anchor.0,
                anchor.1,
                2000,
                crate::rgb(248, 113, 113),
            );
        } else {
            self.send_intent(sow_core::protocol::GameplayIntent::ProposeAlliance {
                target_player: target.owner,
            });
            self.add_notice_at_screen(
                "Alliance requested. 🤝",
                anchor.0,
                anchor.1,
                2000,
                crate::rgb(74, 222, 128),
            );
        }
        true
    }

    fn map_target(&self, tile_idx: u32) -> Option<MapTarget> {
        let map_len = self.sim.map_w.checked_mul(self.sim.map_h)?;
        if tile_idx >= map_len {
            return None;
        }
        let renderer = self.gfx.map_renderer.as_ref()?;
        let index = tile_idx as usize;
        let owner = renderer.owners.get(index).copied().unwrap_or(0);
        let is_land = renderer
            .terrain
            .get(index)
            .is_some_and(|terrain| terrain & 0x80 != 0);
        let my_id = self.sim.my_player_id.unwrap_or(0);
        let snapshot = self.sim.current_snapshot.as_ref()?;
        let me = snapshot.players.iter().find(|player| player.id == my_id);
        let other = snapshot.players.iter().find(|player| player.id == owner);
        let is_betrayer = other.is_some_and(|player| player.active_emoji.as_deref() == Some("🗡️"));
        let is_teammate = me
            .zip(other)
            .is_some_and(|(me, other)| me.team.is_some() && me.team == other.team);
        let is_allied = me.is_some_and(|player| player.alliances.contains(&owner) && !is_betrayer);
        let has_alliance_request =
            me.is_some_and(|player| player.alliance_requests.contains(&owner));
        let has_proposed_alliance =
            other.is_some_and(|player| player.alliance_requests.contains(&my_id));
        let alliance_timer = me
            .and_then(|player| player.alliance_timers.get(&owner).copied())
            .unwrap_or(2400);
        Some(MapTarget {
            owner,
            is_land,
            my_id,
            is_allied,
            is_teammate,
            has_alliance_request,
            has_proposed_alliance,
            is_in_renewal_window: is_allied && alliance_timer <= 300,
        })
    }

    fn can_attack(&self, tile_idx: u32, target_owner: u16) -> bool {
        let Some(renderer) = self.gfx.map_renderer.as_ref() else {
            return false;
        };
        shares_land_border(
            &renderer.owners,
            &renderer.terrain,
            self.sim.map_w,
            self.sim.map_h,
            self.sim.my_player_id.unwrap_or(0),
            target_owner,
        ) && renderer
            .terrain
            .get(tile_idx as usize)
            .is_some_and(|terrain| terrain & 0x80 != 0)
    }

    fn select_warships_at(&mut self, x: f64, y: f64) -> bool {
        let Some(snapshot) = self.sim.current_snapshot.as_ref() else {
            return false;
        };
        if self.sim.map_w == 0 || self.input.camera_zoom <= 0.0 {
            return false;
        }
        let my_pid = self.sim.my_player_id.unwrap_or(0);
        let world_x = (x as f32 - self.input.camera_x) / self.input.camera_zoom;
        let world_y = (y as f32 - self.input.camera_y) / self.input.camera_zoom;
        let selected = snapshot
            .fleets
            .iter()
            .filter(|fleet| {
                fleet.unit_type == sow_core::game::UnitType::Warship && fleet.owner_id == my_pid
            })
            .filter(|fleet| {
                let col = (fleet.current_tile % self.sim.map_w) as f32;
                let row = (fleet.current_tile / self.sim.map_w) as f32;
                (col + 0.5 - world_x).abs() < 0.5 && (row + 0.5 - world_y).abs() < 0.5
            })
            .map(|fleet| fleet.id)
            .collect::<Vec<_>>();
        if selected.is_empty() {
            false
        } else {
            self.input.selected_warships = selected;
            true
        }
    }

    fn tile_coords(&self, tile_idx: u32) -> Option<(i32, i32)> {
        if self.sim.map_w == 0 || tile_idx >= self.sim.map_w.checked_mul(self.sim.map_h)? {
            return None;
        }
        Some((
            (tile_idx % self.sim.map_w) as i32,
            (tile_idx / self.sim.map_w) as i32,
        ))
    }

    fn add_click_marker(&mut self, col: i32, row: i32) {
        self.ui.click_markers.push(crate::app::ClickMarker {
            world_x: col as f32 + 0.5,
            world_y: row as f32 + 0.5,
            start_time: web_time::Instant::now(),
        });
    }

    fn show_water_feedback(&mut self, col: i32, row: i32, anchor: (f64, f64)) {
        self.add_click_marker(col, row);
        const MESSAGES: [&str; 7] = [
            "Splat! That's water! 🌊",
            "Do you have gills? 🐠",
            "Boats are for later! 🚢",
            "Cannot build Atlantis yet! 🏛️",
            "Water deployment failed! 💧",
            "Too wet! ☔",
            "Glug glug... ⚓",
        ];
        let message = MESSAGES[(anchor.0 + anchor.1) as usize % MESSAGES.len()];
        self.add_notice_at_screen(message, anchor.0, anchor.1, 1500, crate::rgb(96, 165, 250));
    }

    fn add_notice_at_screen(
        &mut self,
        text: impl Into<String>,
        x: f64,
        y: f64,
        duration_ms: u64,
        color: [f32; 4],
    ) {
        let zoom = self.input.camera_zoom.max(0.01);
        self.ui.floating_notices.push(crate::app::FloatingNotice {
            text: text.into(),
            world_x: (x as f32 - self.input.camera_x) / zoom,
            world_y: (y as f32 - 60.0 - self.input.camera_y) / zoom,
            start_time: web_time::Instant::now(),
            duration: web_time::Duration::from_millis(duration_ms),
            color,
        });
    }

    pub(crate) fn clear_placement(&mut self) {
        self.ui.app.hud_state.selected_building_kind = None;
        self.ui.app.hud_state.selected_nuke_kind = None;
        self.input.hold_build_active = false;
        self.input.hold_build_accum = 0.0;
    }
}

fn shares_land_border(
    owners: &[u16],
    terrain: &[u8],
    map_w: u32,
    map_h: u32,
    my_id: u16,
    target_owner: u16,
) -> bool {
    if my_id == 0 || my_id == target_owner || map_w == 0 {
        return false;
    }
    let width = map_w as i32;
    let height = map_h as i32;
    let neighbors = [
        (1, 0),
        (-1, 0),
        (0, -1),
        (0, 1),
        (1, -1),
        (-1, -1),
        (1, 1),
        (-1, 1),
    ];

    for row in 0..height {
        for col in 0..width {
            let index = (row * width + col) as usize;
            if owners.get(index).copied() != Some(my_id) {
                continue;
            }
            for (dc, dr) in neighbors {
                let neighbor_col = col + dc;
                let neighbor_row = row + dr;
                if neighbor_col < 0
                    || neighbor_col >= width
                    || neighbor_row < 0
                    || neighbor_row >= height
                {
                    continue;
                }
                let neighbor = (neighbor_row * width + neighbor_col) as usize;
                if owners.get(neighbor).copied() == Some(target_owner)
                    && terrain.get(neighbor).is_some_and(|value| value & 0x80 != 0)
                {
                    return true;
                }
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::{MapMenuAction, MapTarget, TOUCH_HOLD_MS, is_quick_tap, shares_land_border};

    #[test]
    fn tap_and_hold_are_distinct_and_drag_cancels_both() {
        assert!(is_quick_tap(TOUCH_HOLD_MS - 1, 0.0));
        assert!(!is_quick_tap(TOUCH_HOLD_MS, 0.0));
        assert!(!is_quick_tap(1, 401.0));
    }

    #[test]
    fn land_border_is_required_for_a_click_attack() {
        let owners = [1, 2, 0, 0];
        let terrain = [0x80, 0x80, 0x80, 0x80];
        assert!(shares_land_border(&owners, &terrain, 2, 2, 1, 2));
        assert!(shares_land_border(&owners, &terrain, 2, 2, 1, 0));
        assert!(!shares_land_border(&owners, &terrain, 2, 2, 1, 3));
    }

    #[test]
    fn neutral_land_is_a_valid_attack_target() {
        let neutral = MapTarget {
            owner: 0,
            is_land: true,
            my_id: 1,
            is_allied: false,
            is_teammate: false,
            has_alliance_request: false,
            has_proposed_alliance: false,
            is_in_renewal_window: false,
        };
        assert!(neutral.is_attackable());
        assert_eq!(
            neutral.menu_actions(false, true, false),
            vec![MapMenuAction::Attack]
        );
        assert_eq!(
            neutral.menu_actions(false, true, true),
            vec![MapMenuAction::Attack, MapMenuAction::Fleet]
        );
    }

    #[test]
    fn map_menu_keeps_actions_on_the_rust_route() {
        let enemy = MapTarget {
            owner: 2,
            is_land: true,
            my_id: 1,
            is_allied: false,
            is_teammate: false,
            has_alliance_request: false,
            has_proposed_alliance: false,
            is_in_renewal_window: false,
        };
        assert_eq!(
            enemy.menu_actions(false, true, false),
            vec![
                MapMenuAction::Transfer,
                MapMenuAction::Attack,
                MapMenuAction::Fleet,
                MapMenuAction::Nuke,
                MapMenuAction::Alliance,
            ]
        );
        assert_eq!(
            enemy.menu_actions(false, false, false),
            vec![
                MapMenuAction::Transfer,
                MapMenuAction::Fleet,
                MapMenuAction::Nuke,
                MapMenuAction::Alliance
            ]
        );

        let ally = MapTarget {
            is_allied: true,
            ..enemy
        };
        assert_eq!(
            ally.menu_actions(false, true, false),
            vec![
                MapMenuAction::Transfer,
                MapMenuAction::Attack,
                MapMenuAction::Fleet,
                MapMenuAction::Alliance,
            ]
        );

        let teammate = MapTarget {
            is_teammate: true,
            ..enemy
        };
        assert_eq!(
            teammate.menu_actions(false, true, false),
            vec![MapMenuAction::Transfer]
        );

        let own_land = MapTarget { owner: 1, ..enemy };
        assert_eq!(
            own_land.menu_actions(false, false, false),
            vec![
                MapMenuAction::BuildCity,
                MapMenuAction::BuildFactory,
                MapMenuAction::BuildPort,
                MapMenuAction::BuildBunker,
            ]
        );

        assert_eq!(
            enemy.menu_actions(true, false, false),
            vec![MapMenuAction::Spawn]
        );
    }
}
