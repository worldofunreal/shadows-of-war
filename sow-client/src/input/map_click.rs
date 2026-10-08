use super::placement::PlacementQuery;
use crate::app::{MapContextMenu, MapContextMenuView, SowApp};
use serde::Deserialize;

pub(crate) const TOUCH_HOLD_MS: u128 = 300;
const MAP_CLICK_MAX_DISTANCE_SQ: f64 = 400.0;

enum FleetRouteCheck {
    Access,
    Path,
}

fn attack_troops_meet_minimum(troops: f64, minimum: f64) -> bool {
    troops.is_finite() && minimum.is_finite() && troops >= minimum
}

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
    BuildFarm,
    UpgradeStructure,
    Nuke,
    BuildWarship,
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
            Self::BuildFarm => "build_farm",
            Self::UpgradeStructure => "upgrade_structure",
            Self::Nuke => "nuke",
            Self::BuildWarship => "build_warship",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct MapMenuItem {
    pub action: MapMenuAction,
    pub cost: Option<f64>,
    pub level: Option<u8>,
    pub disabled: bool,
    pub reason_key: Option<&'static str>,
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
    fn alliance_action_state(self) -> sow_core::diplomacy::AllianceActionState {
        sow_core::diplomacy::AllianceActionState::resolve(
            self.is_allied,
            self.is_in_renewal_window,
            self.has_alliance_request,
            self.has_proposed_alliance,
        )
    }

    fn alliance_intent(self) -> Option<sow_core::protocol::GameplayIntent> {
        use sow_core::diplomacy::AllianceActionState as State;
        use sow_core::protocol::GameplayIntent;

        match self.alliance_action_state() {
            State::Request | State::Renew => Some(GameplayIntent::ProposeAlliance {
                target_player: self.owner,
            }),
            State::Accept => Some(GameplayIntent::AcceptAlliance {
                target_player: self.owner,
            }),
            State::Pending | State::Active => None,
        }
    }

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

    fn menu_actions(self, spawning: bool, can_attack: bool, can_fleet: bool) -> Vec<MapMenuAction> {
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
                MapMenuAction::BuildFarm,
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
        if self.is_friendly() {
            actions.push(MapMenuAction::Transfer);
            if self.is_allied && !self.is_teammate && self.is_land && can_attack {
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
        if !self.is_teammate && self.alliance_action_state().can_act() {
            actions.push(MapMenuAction::Alliance);
        }
        actions
    }
}

impl SowApp {
    pub(crate) fn map_menu_alliance_state(
        &self,
        tile_idx: u32,
    ) -> Option<sow_core::diplomacy::AllianceActionState> {
        let target = self.map_target(tile_idx)?;
        (target.is_player() && !target.is_teammate).then(|| target.alliance_action_state())
    }

    pub(crate) fn try_attack_at(&mut self, x: f64, y: f64) -> bool {
        if self.ui.tutorial_camera_only
            || self.ui.observing
            || self.ui.app.phase != crate::ClientPhase::Playing
            || self.ui.app.hud_state.selected_building_kind.is_some()
            || self.ui.app.hud_state.selected_nuke_kind.is_some()
            || self.ui.app.hud_state.selected_warship_build
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
        if self.try_tutorial_campaign_nameplate_attack(x, y) {
            return true;
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
        self.attack_from_tile(tile_idx)
    }

    pub(crate) fn handle_map_click(&mut self, x: f64, y: f64) {
        if self.ui.tutorial_camera_only {
            return;
        }
        if self.ui.observing {
            self.clear_placement();
            return;
        }
        if self.try_tutorial_campaign_nameplate_attack(x, y) {
            return;
        }

        let Some((col, row)) = self.mouse_to_tile(x, y) else {
            return;
        };
        let is_spawning = self.sim.current_snapshot.as_ref().is_some_and(|snapshot| {
            matches!(snapshot.phase, sow_core::game::GamePhase::Spawning { .. })
        });

        if is_spawning {
            self.spawn_at(col, row);
            return;
        }

        let tile_idx = (row * self.sim.map_w as i32 + col) as u32;
        if let Some(kind) = self.ui.app.hud_state.selected_nuke_kind {
            self.launch_nuke_at(kind, tile_idx);
            return;
        }
        if self.ui.app.hud_state.selected_warship_build {
            self.build_ship_at(tile_idx, sow_core::game::UnitType::Warship);
            return;
        }
        if let Some(kind) = self.ui.app.hud_state.selected_building_kind {
            if self.build_structure_at(kind, col, row) {
                self.input.hold_build_action_succeeded = true;
                self.input.hold_build_shift_override |= self.input.shift_pressed;
            }
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
            if self.select_owned_building(x, y) {
                return;
            }
        }
        self.primary_target(tile_idx);
    }

    fn try_tutorial_campaign_nameplate_attack(&mut self, x: f64, y: f64) -> bool {
        if self.ui.tutorial_camera_only
            || self.ui.observing
            || self.ui.app.phase != crate::ClientPhase::Playing
            || self.ui.app.hud_state.selected_building_kind.is_some()
            || self.ui.app.hud_state.selected_nuke_kind.is_some()
            || self.ui.app.hud_state.selected_warship_build
            || !self.sim.current_snapshot.as_ref().is_some_and(|snapshot| {
                matches!(snapshot.phase, sow_core::game::GamePhase::Playing)
            })
        {
            return false;
        }
        let Some(tile_idx) = self.tutorial_campaign_attack_tile() else {
            return false;
        };
        let Some(target) = self.map_target(tile_idx) else {
            return false;
        };
        if !self
            .ui
            .nameplates
            .tutorial_target_contains(target.owner, x, y)
        {
            return false;
        }
        self.attack_from_tile(tile_idx)
    }

    fn select_owned_building(&mut self, x: f64, y: f64) -> bool {
        let Some(tile_idx) = self.sim.current_snapshot.as_ref().and_then(|snapshot| {
            crate::render::world::building_at_pointer(
                snapshot,
                &self.sim,
                &mut self.ui,
                &self.input,
                x,
                y,
            )
        }) else {
            return false;
        };
        let Some(target) = self.map_target(tile_idx) else {
            return false;
        };
        if target.my_id == 0 || !target.is_land || target.owner != target.my_id {
            return false;
        }
        if self.sim.current_snapshot.as_ref().is_some_and(|snapshot| {
            snapshot
                .buildings
                .iter()
                .any(|building| building.tile_idx == tile_idx && building.under_construction)
        }) {
            self.close_map_context_menu();
            return true;
        }
        self.set_map_context_menu(x, y, tile_idx, MapContextMenuView::BuildingDetails);
        true
    }

    pub(crate) fn open_map_context_menu(&mut self, x: f64, y: f64) {
        if self.ui.tutorial_camera_only || self.ui.observing {
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
            self.show_map_menu_unavailable(tile_idx);
        }
        self.set_map_context_menu(x, y, tile_idx, MapContextMenuView::Radial);
    }

    fn set_map_context_menu(&mut self, x: f64, y: f64, tile_idx: u32, view: MapContextMenuView) {
        let session = self.input.map_context_menu_session.wrapping_add(1);
        self.input.map_context_menu_session = session;
        let building = (view == MapContextMenuView::BuildingDetails)
            .then(|| {
                self.sim
                    .current_snapshot
                    .as_ref()?
                    .buildings
                    .iter()
                    .find(|building| building.tile_idx == tile_idx)
                    .map(|building| {
                        (
                            building.kind,
                            building.active_level(),
                            building.under_construction,
                        )
                    })
            })
            .flatten();
        self.input.map_context_menu = Some(MapContextMenu {
            x: x as f32,
            y: y as f32,
            tile_idx,
            session,
            view,
            building_kind: building.map(|building| building.0),
            building_level: building.map_or(0, |building| building.1),
            building_under_construction: building.is_some_and(|building| building.2),
        });
    }

    pub(crate) fn close_map_context_menu(&mut self) {
        self.input.map_context_menu = None;
    }

    fn fleet_route_check(
        &mut self,
        tile_idx: u32,
        target_owner: u16,
        check: FleetRouteCheck,
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
                    .ok_or(
                        sow_core::warp_fleet::FleetLaunchError::TargetPlayerNotFound {
                            target_owner,
                        },
                    )?,
            )
        };
        match check {
            FleetRouteCheck::Access => sow_core::warp_fleet::resolve_fleet_endpoints(
                &engine.state.map,
                &engine.water,
                player_id,
                (target_owner, tile_idx),
                border_tiles,
                target_border,
            )
            .map(|_| ()),
            FleetRouteCheck::Path => sow_core::warp_fleet::resolve_fleet_route(
                &engine.state.map,
                &engine.water,
                &mut engine.path_scratch,
                player_id,
                (target_owner, tile_idx),
                border_tiles,
                target_border,
            )
            .map(|_| ()),
        }
    }

    fn show_fleet_unavailable(&mut self, error: sow_core::warp_fleet::FleetLaunchError) {
        let text = match error {
            sow_core::warp_fleet::FleetLaunchError::InvalidTile
            | sow_core::warp_fleet::FleetLaunchError::TargetPlayerNotFound { .. } => {
                crate::ui::UiText::new("hud.fleet_invalid_target")
            }
            sow_core::warp_fleet::FleetLaunchError::SelfTarget => {
                crate::ui::UiText::new("hud.fleet_own_target")
            }
            sow_core::warp_fleet::FleetLaunchError::NoWaterAccess
            | sow_core::warp_fleet::FleetLaunchError::NoLaunchShore { .. } => {
                crate::ui::UiText::new("hud.fleet_no_water_access")
            }
            sow_core::warp_fleet::FleetLaunchError::NoLandingShore => {
                crate::ui::UiText::new("hud.fleet_no_landing_shore")
            }
            sow_core::warp_fleet::FleetLaunchError::NoWaterPath => {
                crate::ui::UiText::new("hud.fleet_no_water_path")
            }
            sow_core::warp_fleet::FleetLaunchError::NoPort => {
                crate::ui::UiText::new("hud.fleet_no_port")
            }
        };
        self.add_map_feedback(text);
    }

    fn show_map_menu_unavailable(&mut self, tile_idx: u32) {
        let message = match self.map_target(tile_idx) {
            Some(target) if target.owner == 0 => {
                match self.fleet_route_check(tile_idx, 0, FleetRouteCheck::Access) {
                    Err(_) => "Fleet unavailable: no shoreline water access.".to_string(),
                    Ok(()) => "No action is available here.".to_string(),
                }
            }
            Some(target) if target.is_teammate => "Teammates cannot be targeted. 🤝".to_string(),
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
        self.add_map_feedback(action_notice(&message));
    }

    pub(crate) fn map_menu_actions(&mut self, tile_idx: u32) -> Vec<MapMenuAction> {
        let Some(target) = self.map_target(tile_idx) else {
            return Vec::new();
        };
        let spawning = self.sim.current_snapshot.as_ref().is_some_and(|snapshot| {
            matches!(snapshot.phase, sow_core::game::GamePhase::Spawning { .. })
        });
        let has_boat_capacity = self
            .sim
            .current_snapshot
            .as_ref()
            .and_then(|snapshot| {
                snapshot
                    .players
                    .iter()
                    .find(|player| player.id == target.my_id)
            })
            .is_none_or(|player| player.boats_in_use < player.boat_capacity);
        let can_fleet = !spawning
            && has_boat_capacity
            && target.owner == 0
            && self
                .fleet_route_check(tile_idx, target.owner, FleetRouteCheck::Access)
                .is_ok();
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
            Some(building) if !building.under_construction => {
                if building.level < building.kind.max_level() {
                    actions.push(MapMenuAction::UpgradeStructure);
                }
            }
            Some(_) => {}
            None => {
                if let Some((col, row)) = self.tile_coords(tile_idx) {
                    for (action, kind) in [
                        (MapMenuAction::BuildCity, sow_core::game::BuildingKind::City),
                        (
                            MapMenuAction::BuildFactory,
                            sow_core::game::BuildingKind::Factory,
                        ),
                        (MapMenuAction::BuildPort, sow_core::game::BuildingKind::Port),
                        (
                            MapMenuAction::BuildBunker,
                            sow_core::game::BuildingKind::Bunker,
                        ),
                        (MapMenuAction::BuildFarm, sow_core::game::BuildingKind::Farm),
                    ] {
                        if self.resolve_building_target(kind, col, row).is_ok() {
                            actions.push(action);
                        }
                    }
                }
            }
        }
        actions
    }

    pub(crate) fn map_menu_items(&mut self, tile_idx: u32) -> Vec<MapMenuItem> {
        let gold = self.current_player_gold();
        self.map_menu_actions(tile_idx)
            .into_iter()
            .map(|action| {
                let (cost, level) = self.map_menu_cost(action, tile_idx);
                MapMenuItem {
                    action,
                    cost,
                    level,
                    disabled: cost.is_some_and(|value| !value.is_finite() || gold < value),
                    reason_key: None,
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
        if self.ui.tutorial_camera_only {
            return;
        }
        let Some(menu) = self.input.map_context_menu else {
            return;
        };
        if menu.session != session || menu.tile_idx != tile_idx {
            return;
        }
        let keep_building_card_open = action == MapMenuAction::UpgradeStructure
            && self
                .input
                .map_context_menu
                .is_some_and(|menu| menu.view == MapContextMenuView::BuildingDetails);
        if !self.map_menu_actions(tile_idx).contains(&action) {
            self.show_map_menu_unavailable(tile_idx);
            if !keep_building_card_open {
                self.close_map_context_menu();
            }
            return;
        }
        if let Some(cost) = self.map_menu_cost(action, tile_idx).0
            && (!cost.is_finite() || self.current_player_gold() < cost)
        {
            let message = if cost.is_finite() {
                format!("Need {} gold.", crate::utils::format_number(cost))
            } else {
                "Action unavailable here.".to_string()
            };
            self.add_action_feedback(message);
            if !keep_building_card_open {
                self.close_map_context_menu();
            }
            return;
        }

        match action {
            MapMenuAction::Spawn => {
                if let Some((col, row)) = self.tile_coords(tile_idx) {
                    self.spawn_at(col, row);
                }
            }
            MapMenuAction::Attack => {
                self.attack_from_tile(tile_idx);
            }
            MapMenuAction::Fleet => {
                self.launch_fleet_from_tile(tile_idx);
            }
            MapMenuAction::Transfer => {
                self.open_transfer_from_tile(tile_idx);
            }
            MapMenuAction::Alliance => {
                self.alliance_from_tile(tile_idx);
            }
            MapMenuAction::BuildCity
            | MapMenuAction::BuildFactory
            | MapMenuAction::BuildPort
            | MapMenuAction::BuildBunker
            | MapMenuAction::BuildFarm => {
                let kind = match action {
                    MapMenuAction::BuildCity => sow_core::game::BuildingKind::City,
                    MapMenuAction::BuildFactory => sow_core::game::BuildingKind::Factory,
                    MapMenuAction::BuildPort => sow_core::game::BuildingKind::Port,
                    MapMenuAction::BuildBunker => sow_core::game::BuildingKind::Bunker,
                    MapMenuAction::BuildFarm => sow_core::game::BuildingKind::Farm,
                    _ => unreachable!(),
                };
                if let Some((col, row)) = self.tile_coords(tile_idx) {
                    self.build_structure_at(kind, col, row);
                }
            }
            MapMenuAction::UpgradeStructure => {
                if let Some(building) =
                    self.sim.current_snapshot.as_ref().and_then(|snapshot| {
                        snapshot.buildings.iter().find(|b| b.tile_idx == tile_idx)
                    })
                {
                    self.send_intent(sow_core::protocol::GameplayIntent::UpgradeStructure {
                        building_id: building.id,
                    });
                }
            }
            MapMenuAction::Nuke => {
                self.launch_nuke_at(sow_core::game::NukeKind::AtomBomb, tile_idx);
            }
            MapMenuAction::BuildWarship => {
                self.build_ship_at(tile_idx, sow_core::game::UnitType::Warship);
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

    fn spawn_at(&mut self, col: i32, row: i32) {
        let idx = (row * self.sim.map_w as i32 + col) as usize;
        let Some(renderer) = self.gfx.map_renderer.as_ref() else {
            return;
        };
        let is_land = renderer
            .terrain
            .get(idx)
            .is_some_and(|terrain| terrain & 0x80 != 0);
        if !is_land {
            self.add_click_marker(col, row);
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
                self.add_map_feedback(crate::ui::UiText::new("hud.spawn_too_close"));
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
    ) -> bool {
        let my_id = self.sim.my_player_id.unwrap_or(0);
        let stack_target = self.sim.current_snapshot.as_ref().and_then(|snapshot| {
            super::placement::find_stack_target_tile(
                kind,
                col,
                row,
                self.sim.map_w,
                my_id,
                &snapshot.buildings,
            )
            .and_then(|tile_idx| {
                snapshot
                    .buildings
                    .iter()
                    .find(|building| building.tile_idx == tile_idx)
                    .copied()
            })
        });
        if let Some(building) = stack_target {
            self.cancel_hold_build();
            if building.under_construction {
                self.add_action_feedback("Building under construction. 🏗️");
                return false;
            }
            if building.level >= kind.max_level() {
                self.add_map_feedback(crate::ui::UiText::new("hud.building_max_level"));
                return false;
            }
            let cost = self
                .map_menu_cost(MapMenuAction::UpgradeStructure, building.tile_idx)
                .0
                .unwrap_or(f64::INFINITY);
            let gold = self.current_player_gold();
            if !cost.is_finite() || gold < cost {
                let message = if cost.is_finite() {
                    format!("Need {} gold.", crate::utils::format_number(cost))
                } else {
                    "Action unavailable here.".to_string()
                };
                self.add_action_feedback(message);
                return false;
            }
            self.send_intent(sow_core::protocol::GameplayIntent::UpgradeStructure {
                building_id: building.id,
            });
            return true;
        }
        let target_res = self.resolve_building_target(kind, col, row);
        let cost_index = sow_core::game::BuildingKind::ALL
            .iter()
            .position(|candidate| *candidate == kind)
            .unwrap_or(0);
        if self.current_player_gold() < self.ui.app.hud_state.building_costs[cost_index] {
            let text = format!(
                "Need {} gold.",
                crate::utils::format_number(self.ui.app.hud_state.building_costs[cost_index])
            );
            self.add_action_feedback(text);
            return false;
        }
        let target_tile = match target_res {
            Ok(target_tile) => target_tile,
            Err(message) => {
                self.add_action_feedback(message);
                return false;
            }
        };
        self.send_intent(sow_core::protocol::GameplayIntent::BuildStructure { kind, target_tile });
        true
    }

    fn resolve_building_target(
        &mut self,
        kind: sow_core::game::BuildingKind,
        col: i32,
        row: i32,
    ) -> Result<u32, &'static str> {
        let Some(snapshot) = self.sim.current_snapshot.as_ref() else {
            return Err("Action unavailable here.");
        };
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
        self.ui.building_placement_cache.resolve(
            snapshot.tick,
            &PlacementQuery {
                kind,
                click_x: col,
                click_y: row,
                map_w: self.sim.map_w,
                map_h: self.sim.map_h,
                owners,
                terrain,
                my_id: self.sim.my_player_id.unwrap_or(0),
                buildings: &snapshot.buildings,
            },
        )
    }

    pub(crate) fn map_menu_cost(
        &self,
        action: MapMenuAction,
        tile_idx: u32,
    ) -> (Option<f64>, Option<u8>) {
        match action {
            MapMenuAction::BuildWarship => {
                (Some(sow_core::game::UnitType::Warship.gold_cost()), None)
            }
            MapMenuAction::BuildCity
            | MapMenuAction::BuildFactory
            | MapMenuAction::BuildPort
            | MapMenuAction::BuildBunker
            | MapMenuAction::BuildFarm => {
                let kind = match action {
                    MapMenuAction::BuildCity => sow_core::game::BuildingKind::City,
                    MapMenuAction::BuildFactory => sow_core::game::BuildingKind::Factory,
                    MapMenuAction::BuildPort => sow_core::game::BuildingKind::Port,
                    MapMenuAction::BuildBunker => sow_core::game::BuildingKind::Bunker,
                    MapMenuAction::BuildFarm => sow_core::game::BuildingKind::Farm,
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
            MapMenuAction::UpgradeStructure => {
                let building = self.sim.current_snapshot.as_ref().and_then(|snapshot| {
                    snapshot
                        .buildings
                        .iter()
                        .find(|building| building.tile_idx == tile_idx)
                });
                let Some(building) = building else {
                    return (None, None);
                };
                (
                    Some({
                        let free_tutorial_upgrade =
                            self.sim.engine.as_ref().is_some_and(|engine| {
                                engine.tutorial_city_upgrade_is_free(
                                    self.sim.my_player_id.unwrap_or_default(),
                                    building.id,
                                )
                            });
                        if free_tutorial_upgrade {
                            0.0
                        } else {
                            let snapshot = self.sim.current_snapshot.as_ref();
                            let owned_levels = snapshot
                                .map(|snapshot| {
                                    snapshot
                                        .buildings
                                        .iter()
                                        .filter(|candidate| {
                                            candidate.owner_id == building.owner_id
                                                && candidate.kind == building.kind
                                        })
                                        .map(|candidate| candidate.level as u32)
                                        .sum()
                                })
                                .unwrap_or_default();
                            sow_core::building::cost::structure_upgrade_cost_gold(
                                building.kind,
                                building.active_level().saturating_add(1),
                                owned_levels,
                                &self.sim.config,
                            )
                        }
                    }),
                    Some(building.level),
                )
            }
            _ => (None, None),
        }
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
                    && building.kind == sow_core::game::BuildingKind::Port
                    && building.active_level() >= kind.required_port_level()
            });
        if !ready_port {
            return false;
        }
        let Some(player) = self.sim.current_snapshot.as_ref().and_then(|snapshot| {
            snapshot
                .players
                .iter()
                .find(|player| player.id == target.my_id)
        }) else {
            return false;
        };
        if !kind.uses_military_capacity()
            || player.boats_in_use >= player.boat_capacity
            || player.gold < kind.gold_cost()
        {
            return false;
        }
        self.send_intent(sow_core::protocol::GameplayIntent::BuildShip {
            port_tile: tile_idx,
            kind,
        });
        self.ui.app.hud_state.selected_warship_build = false;
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

    fn primary_target(&mut self, tile_idx: u32) {
        let Some(target) = self.map_target(tile_idx) else {
            return;
        };
        if !target.is_land {
            if let Some((col, row)) = self.tile_coords(tile_idx) {
                self.add_click_marker(col, row);
            }
            return;
        }
        if target.owner == target.my_id {
            return;
        }
        if target.is_friendly() {
            self.open_transfer_from_tile(tile_idx);
        } else {
            self.attack_from_tile(tile_idx);
        }
    }

    fn attack_from_tile(&mut self, tile_idx: u32) -> bool {
        let Some(target) = self.map_target(tile_idx) else {
            return false;
        };
        if !target.is_land {
            if let Some((col, row)) = self.tile_coords(tile_idx) {
                self.add_click_marker(col, row);
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
        let minimum_troops = self.sim.config.attack_cost_neutral;
        if !attack_troops_meet_minimum(troops, minimum_troops) {
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
            self.add_map_feedback(crate::ui::UiText::new("hud.attack_out_of_range"));
            return false;
        }
        self.send_intent(intent)
    }

    pub(crate) fn launch_fleet_from_tile(&mut self, tile_idx: u32) -> bool {
        let Some(target) = self.map_target(tile_idx) else {
            return false;
        };
        if target.is_teammate {
            self.add_action_feedback("Teammates cannot be targeted. 🤝");
            return false;
        }
        if target.is_allied {
            self.add_action_feedback("Break the alliance before launching a fleet. 🛡️");
            return false;
        }
        if target.owner != 0 && !target.is_enemy() {
            self.add_action_feedback("A fleet cannot target your own territory. 🛡️");
            return false;
        }
        if let Err(error) = self.fleet_route_check(tile_idx, target.owner, FleetRouteCheck::Path) {
            self.show_fleet_unavailable(error);
            return false;
        }
        let troops = self.ui.app.hud_state.troops * self.ui.app.hud_state.attack_ratio as f64;
        if troops < self.sim.config.attack_cost_neutral {
            self.add_action_feedback(format!(
                "Need at least {} troops for a fleet. 🚢",
                crate::utils::format_number(self.sim.config.attack_cost_neutral)
            ));
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
            self.add_action_feedback("Resources can only be sent to allies. ⚖️");
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

    fn alliance_from_tile(&mut self, tile_idx: u32) -> bool {
        let Some(target) = self.map_target(tile_idx) else {
            return false;
        };
        if !target.is_player() || target.is_teammate {
            return false;
        }
        let Some(intent) = target.alliance_intent() else {
            return false;
        };
        self.send_intent(intent)
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
        let alliance_timer = me.and_then(|player| player.alliance_timers.get(&owner).copied());
        Some(MapTarget {
            owner,
            is_land,
            my_id,
            is_allied,
            is_teammate,
            has_alliance_request,
            has_proposed_alliance,
            is_in_renewal_window: sow_core::diplomacy::alliance_can_renew(
                is_allied,
                alliance_timer,
            ),
        })
    }

    fn can_attack(&self, tile_idx: u32, target_owner: u16) -> bool {
        let Some(renderer) = self.gfx.map_renderer.as_ref() else {
            return false;
        };
        let my_id = self.sim.my_player_id.unwrap_or(0);
        let Some(border_tiles) = self
            .sim
            .engine
            .as_ref()
            .and_then(|engine| engine.state.player(my_id))
            .map(|player| &player.border_tiles)
        else {
            return false;
        };
        renderer
            .terrain
            .get(tile_idx as usize)
            .is_some_and(|terrain| terrain & 0x80 != 0)
            && shares_land_border(
                &renderer.owners,
                &renderer.terrain,
                self.sim.map_w,
                self.sim.map_h,
                my_id,
                target_owner,
                border_tiles,
            )
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

    fn add_action_feedback(&mut self, text: impl Into<String>) {
        self.add_map_feedback(action_notice(&text.into()));
    }

    fn add_map_feedback(&mut self, text: crate::ui::UiText) {
        let position = self
            .input
            .map_context_menu
            .map(|menu| [menu.x, menu.y])
            .unwrap_or([
                self.input.last_mouse_x as f32,
                self.input.last_mouse_y as f32,
            ]);
        self.ui.app.hud_state.push_map_feedback(text, position);
    }

    pub(crate) fn clear_placement(&mut self) {
        self.ui.app.hud_state.selected_building_kind = None;
        self.ui.app.hud_state.selected_warship_build = false;
        self.ui.app.hud_state.selected_nuke_kind = None;
        self.cancel_hold_build();
        self.input.hold_build_action_succeeded = false;
        self.input.hold_build_shift_override = false;
    }

    pub(crate) fn select_building_kind(&mut self, kind: sow_core::game::BuildingKind) {
        if self.ui.observing
            || self.ui.app.phase != crate::ClientPhase::Playing
            || !self.sim.current_snapshot.as_ref().is_some_and(|snapshot| {
                matches!(snapshot.phase, sow_core::game::GamePhase::Playing)
            })
        {
            return;
        }
        let cost_index = match kind {
            sow_core::game::BuildingKind::City => 0,
            sow_core::game::BuildingKind::Bunker => 1,
            sow_core::game::BuildingKind::Factory => 2,
            sow_core::game::BuildingKind::Port => 3,
            sow_core::game::BuildingKind::Farm => 4,
        };
        let cost = self.ui.app.hud_state.building_costs[cost_index];
        if self.ui.app.hud_state.selected_building_kind != Some(kind)
            && self.current_player_gold() < cost
        {
            self.add_action_feedback(format!("Need {} gold.", crate::utils::format_number(cost)));
            return;
        }
        let selected = &mut self.ui.app.hud_state.selected_building_kind;
        *selected = (*selected != Some(kind)).then_some(kind);
        self.ui.app.hud_state.selected_warship_build = false;
        self.ui.app.hud_state.selected_nuke_kind = None;
        self.cancel_hold_build();
        self.input.hold_build_action_succeeded = false;
        self.input.hold_build_shift_override = false;
    }

    pub(crate) fn select_warship_build_mode(&mut self) {
        if self.ui.observing
            || self.ui.app.phase != crate::ClientPhase::Playing
            || !self.sim.current_snapshot.as_ref().is_some_and(|snapshot| {
                matches!(snapshot.phase, sow_core::game::GamePhase::Playing)
            })
        {
            return;
        }
        let owner_id = self
            .sim
            .my_player_id
            .unwrap_or(self.ui.app.hud_state.my_player_id);
        let Some(player) = self
            .sim
            .current_snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.players.iter().find(|player| player.id == owner_id))
        else {
            return;
        };
        let has_port = self.sim.current_snapshot.as_ref().is_some_and(|snapshot| {
            snapshot.buildings.iter().any(|building| {
                building.owner_id == owner_id
                    && building.kind == sow_core::game::BuildingKind::Port
                    && !building.under_construction
                    && building.active_level()
                        >= sow_core::game::UnitType::Warship.required_port_level()
            })
        });
        if !has_port
            || player.boats_in_use >= player.boat_capacity
            || player.gold < sow_core::game::UnitType::Warship.gold_cost()
        {
            self.add_action_feedback("Warship unavailable here.".to_string());
            return;
        }
        self.ui.app.hud_state.selected_warship_build =
            !self.ui.app.hud_state.selected_warship_build;
        self.ui.app.hud_state.selected_building_kind = None;
        self.ui.app.hud_state.selected_nuke_kind = None;
    }

    pub(crate) fn select_nuke_kind(&mut self, kind: sow_core::game::NukeKind) {
        if self.ui.observing
            || self.ui.app.phase != crate::ClientPhase::Playing
            || !self.sim.current_snapshot.as_ref().is_some_and(|snapshot| {
                matches!(snapshot.phase, sow_core::game::GamePhase::Playing)
            })
        {
            return;
        }
        let owner_id = self
            .sim
            .my_player_id
            .unwrap_or(self.ui.app.hud_state.my_player_id);
        let available = self.sim.current_snapshot.as_ref().is_some_and(|snapshot| {
            snapshot
                .players
                .iter()
                .find(|player| player.id == owner_id)
                .is_some_and(|player| {
                    player.nuke_available && player.gold >= self.sim.config.nuke_cost
                })
                && snapshot.buildings.iter().any(|building| {
                    building.owner_id == owner_id
                        && building.kind == sow_core::game::BuildingKind::City
                        && !building.under_construction
                        && building.active_level() >= kind.required_city_level()
                })
        });
        if !available {
            self.add_action_feedback("Nuke unavailable here.".to_string());
            return;
        }
        let selected = &mut self.ui.app.hud_state.selected_nuke_kind;
        *selected = (*selected != Some(kind)).then_some(kind);
        self.ui.app.hud_state.selected_building_kind = None;
        self.ui.app.hud_state.selected_warship_build = false;
    }
}

fn action_notice(message: &str) -> crate::ui::UiText {
    use crate::ui::UiText;

    let message = message.trim();
    if let Some(cost) = message
        .strip_prefix("Need ")
        .and_then(|value| value.strip_suffix(" gold."))
    {
        return UiText::new("hud.need_gold").with("cost", cost);
    }
    if let Some(troops) = message
        .strip_prefix("Need at least ")
        .and_then(|value| value.strip_suffix(" troops for a fleet. 🚢"))
    {
        return UiText::new("hud.fleet_need_troops").with("troops", troops);
    }
    if message.starts_with("Fleet unavailable:") {
        return if message.contains("completed port") {
            UiText::new("hud.fleet_no_port")
        } else if message.contains("landing shore") {
            UiText::new("hud.fleet_no_landing_shore")
        } else if message.contains("water path") {
            UiText::new("hud.fleet_no_water_path")
        } else {
            UiText::new("hud.fleet_no_water_access")
        };
    }
    match message {
        "No action is available here." | "Action unavailable here." => {
            UiText::new("hud.action_unavailable")
        }
        "Teammates cannot be targeted. 🤝" => UiText::new("hud.fleet_teammate"),
        "Break the alliance before launching a fleet. 🛡️" => UiText::new("hud.fleet_alliance"),
        "A fleet cannot target your own territory. 🛡️" => UiText::new("hud.fleet_own_target"),
        "Resources can only be sent to allies. ⚖️" => UiText::new("hud.resources_allies_only"),
        "Alliance renewal is already pending." => UiText::new("hud.alliance_renewal_pending"),
        "Alliance request already pending." => UiText::new("hud.alliance_request_pending"),
        "Buildings require owned land. 🗺️" | "Build inside your own land." => {
            UiText::new("hud.build_owned_land")
        }
        "Structures go on land, not water." => UiText::new("hud.build_land"),
        "Too close to another City! Minimum spacing is 6 tiles." => {
            UiText::new("hud.build_spacing_city")
        }
        "Too close to another structure! Spacing rules: City requires 6, other structures require 4." => {
            UiText::new("hud.build_spacing_structure")
        }
        "No space nearby!" => UiText::new("hud.build_no_space"),
        "Building under construction. 🏗️" => UiText::new("hud.building_in_progress"),
        _ => UiText::new("hud.action_unavailable"),
    }
}

fn shares_land_border(
    owners: &[u16],
    terrain: &[u8],
    map_w: u32,
    map_h: u32,
    my_id: u16,
    target_owner: u16,
    border_tiles: &sow_core::bitset::DenseBitSet,
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

    for raw_idx in border_tiles.ones() {
        if raw_idx / map_w >= map_h || owners.get(raw_idx as usize).copied() != Some(my_id) {
            continue;
        }
        let col = (raw_idx % map_w) as i32;
        let row = (raw_idx / map_w) as i32;
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
    false
}

#[cfg(test)]
mod tests {
    use super::{
        MapMenuAction, MapTarget, TOUCH_HOLD_MS, attack_troops_meet_minimum, is_quick_tap,
        shares_land_border,
    };
    use sow_core::bitset::DenseBitSet;

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
        let mut border_tiles = DenseBitSet::new();
        border_tiles.insert(0);
        assert!(shares_land_border(
            &owners,
            &terrain,
            2,
            2,
            1,
            2,
            &border_tiles
        ));
        assert!(shares_land_border(
            &owners,
            &terrain,
            2,
            2,
            1,
            0,
            &border_tiles
        ));
        assert!(!shares_land_border(
            &owners,
            &terrain,
            2,
            2,
            1,
            3,
            &border_tiles
        ));
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
    fn attack_amount_must_meet_the_configured_minimum() {
        assert!(!attack_troops_meet_minimum(0.5, 1.0));
        assert!(attack_troops_meet_minimum(1.0, 1.0));
        assert!(!attack_troops_meet_minimum(f64::NAN, 1.0));
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
                MapMenuAction::Attack,
                MapMenuAction::Fleet,
                MapMenuAction::Nuke,
                MapMenuAction::Alliance,
            ]
        );
        assert_eq!(
            enemy.menu_actions(false, false, false),
            vec![
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
            ally.menu_actions(false, false, false),
            vec![MapMenuAction::Transfer, MapMenuAction::Fleet]
        );
        assert_eq!(
            ally.menu_actions(false, true, false),
            vec![
                MapMenuAction::Transfer,
                MapMenuAction::Attack,
                MapMenuAction::Fleet,
            ]
        );

        let renewable_ally = MapTarget {
            is_in_renewal_window: true,
            ..ally
        };
        assert!(
            renewable_ally
                .menu_actions(false, false, false)
                .contains(&MapMenuAction::Alliance)
        );

        let incoming_request = MapTarget {
            has_alliance_request: true,
            ..enemy
        };
        assert!(
            incoming_request
                .menu_actions(false, false, false)
                .contains(&MapMenuAction::Alliance)
        );

        let outgoing_request = MapTarget {
            has_proposed_alliance: true,
            ..enemy
        };
        assert!(
            !outgoing_request
                .menu_actions(false, false, false)
                .contains(&MapMenuAction::Alliance)
        );

        let teammate = MapTarget {
            is_teammate: true,
            ..enemy
        };
        assert_eq!(
            teammate.menu_actions(false, true, false),
            vec![MapMenuAction::Transfer]
        );
        let allied_teammate = MapTarget {
            is_teammate: true,
            ..ally
        };
        assert_eq!(
            allied_teammate.menu_actions(false, true, true),
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

    #[test]
    fn lower_alliance_action_only_requests_or_accepts() {
        use sow_core::protocol::GameplayIntent;

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
        let request_intent = enemy.alliance_intent();
        assert_eq!(
            request_intent,
            Some(GameplayIntent::ProposeAlliance { target_player: 2 })
        );
        let incoming = MapTarget {
            has_alliance_request: true,
            ..enemy
        };
        let accept_intent = incoming.alliance_intent();
        assert_eq!(
            accept_intent,
            Some(GameplayIntent::AcceptAlliance { target_player: 2 })
        );

        let outgoing = MapTarget {
            has_proposed_alliance: true,
            ..enemy
        };
        assert_eq!(outgoing.alliance_action_state().as_str(), "pending");
        let pending_intent = outgoing.alliance_intent();
        assert_eq!(pending_intent, None);

        let active = MapTarget {
            is_allied: true,
            ..enemy
        };
        assert_eq!(active.alliance_action_state().as_str(), "active");
        let active_intent = active.alliance_intent();
        assert_eq!(active_intent, None);

        let renewable = MapTarget {
            is_allied: true,
            is_in_renewal_window: true,
            ..enemy
        };
        let renew_intent = renewable.alliance_intent();
        assert_eq!(
            renew_intent,
            Some(GameplayIntent::ProposeAlliance { target_player: 2 })
        );
    }
}
