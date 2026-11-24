use crate::app::{MapContextMenu, MapContextMenuView, SowApp};
use crate::ui::hud::state::TransferMode;
use serde::Deserialize;

pub(crate) const TOUCH_HOLD_MS: u128 = 300;
pub(crate) const LAND_BORDER_NEIGHBORS: [(i32, i32); 8] = [
    (1, 0),
    (-1, 0),
    (0, -1),
    (0, 1),
    (1, -1),
    (-1, -1),
    (1, 1),
    (-1, 1),
];
const MAP_CLICK_MAX_DISTANCE_SQ: f64 = 400.0;
/// Drag and tap snap the preview to a legal foundation within this many tiles.
const PLACEMENT_SNAP_RADIUS: i32 = 6;
/// Arrow keys look this far ahead for the next legal foundation.
const PLACEMENT_NUDGE_REACH: i32 = 8;

enum FleetRouteCheck {
    Access,
    Path,
}

pub(crate) fn attack_troops_meet_minimum(troops: f64, minimum: f64) -> bool {
    troops.is_finite() && minimum.is_finite() && troops >= minimum
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MapMenuAction {
    Emote,
    Spawn,
    Attack,
    Fleet,
    Send,
    Request,
    RequestAllies,
    Alliance,
    BuildCity,
    BuildTower,
    BuildTradingPost,
    BuildPort,
    BuildMilitaryCamp,
    BuildSam,
    BuildMissileSilo,
    UpgradeStructure,
    UpgradeStructureMax,
    Nuke,
}

impl MapMenuAction {
    const fn building_kind(self) -> Option<sow_core::game::BuildingKind> {
        use sow_core::game::BuildingKind;
        match self {
            Self::BuildCity => Some(BuildingKind::City),
            Self::BuildTower => Some(BuildingKind::Tower),
            Self::BuildTradingPost => Some(BuildingKind::TradingPost),
            Self::BuildPort => Some(BuildingKind::Port),
            Self::BuildMilitaryCamp => Some(BuildingKind::MilitaryCamp),
            Self::BuildSam => Some(BuildingKind::SAM),
            Self::BuildMissileSilo => Some(BuildingKind::MissileSilo),
            _ => None,
        }
    }

    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Emote => "emote",
            Self::Spawn => "spawn",
            Self::Attack => "attack",
            Self::Fleet => "fleet",
            Self::Send => "send",
            Self::Request => "request",
            Self::RequestAllies => "request_allies",
            Self::Alliance => "alliance",
            Self::BuildCity => "build_city",
            Self::BuildTower => "build_tower",
            Self::BuildTradingPost => "build_trading_post",
            Self::BuildPort => "build_port",
            Self::BuildMilitaryCamp => "build_military_camp",
            Self::BuildSam => "build_sam",
            Self::BuildMissileSilo => "build_missile_silo",
            Self::UpgradeStructure => "upgrade_structure",
            Self::UpgradeStructureMax => "upgrade_structure_max",
            Self::Nuke => "nuke",
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
    pub reason_level: Option<u8>,
    pub reason_seconds: Option<u32>,
    pub reason_troops: Option<u64>,
}

fn fleet_error_reason_key(error: sow_core::warp_fleet::FleetLaunchError) -> &'static str {
    use sow_core::warp_fleet::FleetLaunchError as Error;
    match error {
        Error::InvalidTile | Error::TargetPlayerNotFound { .. } => "hud.fleet_invalid_target",
        Error::SelfTarget => "hud.fleet_own_target",
        Error::NoWaterAccess | Error::NoLaunchShore { .. } => "hud.fleet_no_water_access",
        Error::NoLandingShore => "hud.fleet_no_landing_shore",
        Error::NoWaterPath => "hud.fleet_no_water_path",
    }
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
    is_betrayer: bool,
    me_can_retaliate: bool,
    team_betrayal_enabled: bool,
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

    fn can_betray_teammate(self) -> bool {
        self.is_teammate && self.team_betrayal_enabled
    }

    fn team_betrayal_open(self) -> bool {
        sow_core::diplomacy::same_team_betrayal_allows_combat(
            self.team_betrayal_enabled,
            self.is_teammate,
            self.me_can_retaliate,
            self.is_betrayer,
        )
    }

    fn needs_betrayal_confirmation(self) -> bool {
        self.is_allied || (self.can_betray_teammate() && !self.team_betrayal_open())
    }

    fn menu_actions(
        self,
        spawning: bool,
        can_attack: bool,
        can_fleet: bool,
        can_betray_fleet: bool,
    ) -> Vec<MapMenuAction> {
        if spawning {
            return self
                .is_land
                .then_some(MapMenuAction::Spawn)
                .into_iter()
                .collect();
        }
        if self.owner == self.my_id && self.is_land {
            return [
                MapMenuAction::BuildCity,
                MapMenuAction::BuildTradingPost,
                MapMenuAction::BuildPort,
                MapMenuAction::BuildTower,
                MapMenuAction::BuildMilitaryCamp,
                MapMenuAction::BuildSam,
                MapMenuAction::BuildMissileSilo,
            ]
            .into_iter()
            .filter(|action| action.building_kind().is_none_or(|kind| kind.is_enabled()))
            .collect();
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
            actions.push(MapMenuAction::Send);
            actions.push(MapMenuAction::Request);
            if (self.can_betray_teammate() || (self.is_allied && !self.is_teammate))
                && self.is_land
                && can_attack
            {
                actions.push(MapMenuAction::Attack);
            }
            if can_betray_fleet || (!self.is_teammate && can_fleet) {
                actions.push(MapMenuAction::Fleet);
            }
        } else {
            if self.is_land && can_attack {
                actions.push(MapMenuAction::Attack);
            }
            actions.push(MapMenuAction::Fleet);
        }
        if !self.is_teammate {
            actions.push(MapMenuAction::Alliance);
        }
        actions
    }
}

impl SowApp {
    pub(crate) fn campaign_building_max_level(&self, kind: sow_core::game::BuildingKind) -> u8 {
        self.sim
            .engine
            .as_ref()
            .and_then(|engine| engine.campaign_unlocks.as_ref())
            .map_or(kind.max_level(), |unlocks| unlocks.building_level(kind))
    }

    pub(crate) fn campaign_action_unlocked(
        &self,
        action: sow_core::campaign::CampaignAction,
    ) -> bool {
        self.sim
            .engine
            .as_ref()
            .is_none_or(|engine| engine.campaign_allows_action(action))
    }

    pub(crate) fn map_menu_alliance_state(
        &self,
        tile_idx: u32,
    ) -> Option<sow_core::diplomacy::AllianceActionState> {
        let target = self.map_target(tile_idx)?;
        (target.is_player() && !target.is_teammate).then(|| target.alliance_action_state())
    }

    /// Who owns the tile, from the local player's point of view; the HUD
    /// uses it to name and color the menu's center.
    pub(crate) fn map_menu_relation(&self, tile_idx: u32) -> Option<&'static str> {
        let target = self.map_target(tile_idx)?;
        Some(if target.owner == 0 {
            if target.is_land { "neutral" } else { "water" }
        } else if target.owner == target.my_id {
            "own"
        } else if target.is_teammate {
            "teammate"
        } else if target.is_allied {
            "ally"
        } else {
            "enemy"
        })
    }

    pub(crate) fn try_attack_at(&mut self, x: f64, y: f64) -> bool {
        if self.try_tutorial_marker_attack(x, y) {
            return true;
        }
        if self.ui.tutorial_camera_only
            || self.ui.observing
            || self.ui.app.phase != crate::ClientPhase::Playing
            || self.ui.app.hud_state.building_placement.is_some()
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
        // Out-of-range targets are handled on pointer release, where the full
        // click path can try a fleet without dispatching the same click twice.
        if !self.can_attack(tile_idx, target.owner) {
            return false;
        }
        self.attack_from_tile(tile_idx)
    }

    pub(crate) fn handle_map_click(&mut self, x: f64, y: f64) {
        if self.ui.tutorial_camera_only {
            return;
        }
        if self.try_tutorial_marker_attack(x, y) {
            return;
        }
        if self.ui.observing {
            self.clear_placement();
            return;
        }
        // Buttons may hang past the map edge, so they resolve before the tile.
        if self.building_panel_button_at(x as f32, y as f32)
            && let Some(menu) = self.input.map_context_menu
        {
            self.handle_map_menu_action(
                menu.session,
                menu.tile_idx,
                MapMenuAction::UpgradeStructure,
            );
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
        if self.ui.app.hud_state.warship_move_targeting
            && !self.ui.app.hud_state.selected_warship_ids.is_empty()
        {
            if self.move_selected_warships_to_tile(tile_idx) {
                return;
            }
            self.ui.app.hud_state.warship_move_targeting = false;
            return;
        }
        if let Some(kind) = self.ui.app.hud_state.selected_nuke_kind {
            self.launch_nuke_at(kind, tile_idx);
            return;
        }
        if self.ui.app.hud_state.building_placement.is_some() {
            self.move_building_preview(col, row);
            return;
        }

        if self
            .sim
            .current_snapshot
            .as_ref()
            .is_some_and(|snapshot| matches!(snapshot.phase, sow_core::game::GamePhase::Playing))
        {
            if self.select_owned_warship(x, y) {
                return;
            }
            if self.select_owned_building(x, y) {
                return;
            }
        }
        self.primary_target(tile_idx, x, y);
    }

    fn try_tutorial_marker_attack(&mut self, x: f64, y: f64) -> bool {
        if self.ui.tutorial_camera_only
            || self.ui.observing
            || self.ui.app.phase != crate::ClientPhase::Playing
            || self.ui.app.hud_state.building_placement.is_some()
            || self.ui.app.hud_state.selected_nuke_kind.is_some()
            || self.ui.tutorial_marker_action != Some(crate::app::TutorialMarkerAction::Attack)
            || !self.ui.tutorial_active
            || !self.net.is_offline
            || !self.sim.current_snapshot.as_ref().is_some_and(|snapshot| {
                matches!(snapshot.phase, sow_core::game::GamePhase::Playing)
            })
        {
            return false;
        }
        let Some(target_owner) = self.ui.tutorial_marker_player_id else {
            return false;
        };
        if !self
            .ui
            .nameplates
            .tutorial_target_contains(target_owner, x, y)
        {
            return false;
        }
        let Some(tile_idx) = self.sim.tutorial_observation.guide_target_attack else {
            return false;
        };
        if !self
            .map_target(tile_idx)
            .is_some_and(|target| target.owner == target_owner)
        {
            return false;
        }
        self.attack_from_tile(tile_idx)
    }

    fn select_owned_warship(&mut self, x: f64, y: f64) -> bool {
        let Some(fleet_id) = self.sim.current_snapshot.as_ref().and_then(|snapshot| {
            crate::render::world::overlays::warship_at_pointer(
                snapshot,
                &self.sim,
                &self.input,
                x,
                y,
            )
        }) else {
            return false;
        };
        self.close_map_context_menu();
        self.ui.app.hud_state.warship_move_targeting = false;
        if self.input.shift_pressed {
            let ids = &mut self.ui.app.hud_state.selected_warship_ids;
            if let Some(pos) = ids.iter().position(|id| *id == fleet_id) {
                ids.remove(pos);
            } else {
                ids.push(fleet_id);
            }
        } else {
            self.ui.app.hud_state.selected_warship_ids = vec![fleet_id];
        }
        true
    }

    pub(crate) fn move_selected_warships_to_tile(&mut self, tile_idx: u32) -> bool {
        let unit_ids = self.live_selected_warship_ids();
        if unit_ids.is_empty() {
            self.ui.app.hud_state.warship_move_targeting = false;
            return false;
        }
        let Some(target) = self.map_target(tile_idx) else {
            return false;
        };
        if target.is_land {
            self.add_map_feedback(crate::ui::UiText::new("hud.fleet_invalid_target"));
            return false;
        }
        self.ui.app.hud_state.warship_move_targeting = false;
        self.send_intent(sow_core::protocol::GameplayIntent::MoveWarships {
            unit_ids,
            target_tile: tile_idx,
        });
        true
    }

    pub(crate) fn patrol_selected_warships(&mut self) {
        let unit_ids = self.live_selected_warship_ids();
        self.ui.app.hud_state.warship_move_targeting = false;
        if unit_ids.is_empty() {
            return;
        }
        self.send_intent(sow_core::protocol::GameplayIntent::PatrolWarships { unit_ids });
    }

    pub(crate) fn toggle_warship_move_targeting(&mut self) {
        if self.live_selected_warship_ids().is_empty() {
            self.ui.app.hud_state.warship_move_targeting = false;
            return;
        }
        self.close_map_context_menu();
        self.ui.app.hud_state.warship_move_targeting =
            !self.ui.app.hud_state.warship_move_targeting;
    }

    pub(crate) fn clear_warship_selection(&mut self) {
        self.ui.app.hud_state.selected_warship_ids.clear();
        self.ui.app.hud_state.warship_move_targeting = false;
    }

    fn live_selected_warship_ids(&mut self) -> Vec<u64> {
        let my_id = self
            .sim
            .my_player_id
            .unwrap_or(self.ui.app.hud_state.my_player_id);
        let Some(snapshot) = self.sim.current_snapshot.as_ref() else {
            return Vec::new();
        };
        self.ui.app.hud_state.selected_warship_ids.retain(|id| {
            snapshot.fleets.iter().any(|f| {
                f.id == *id
                    && f.owner_id == my_id
                    && f.unit_type == sow_core::game::UnitType::Warship
                    && f.troops > 0.0
            })
        });
        self.ui.app.hud_state.selected_warship_ids.clone()
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
        // Buildings under construction open the same panel with the seconds left.
        self.set_map_context_menu(x, y, tile_idx, MapContextMenuView::BuildingDetails);
        true
    }

    pub(crate) fn open_map_context_menu(&mut self, x: f64, y: f64) {
        if self.ui.tutorial_camera_only {
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
        let already_selected = self.input.map_context_menu.is_some_and(|prev| {
            prev.view == MapContextMenuView::BuildingDetails && prev.tile_idx == tile_idx
        });
        let session = self.input.map_context_menu_session.wrapping_add(1);
        self.input.map_context_menu_session = session;
        let building = (view == MapContextMenuView::BuildingDetails)
            .then(|| {
                let buildings = &self.sim.current_snapshot.as_ref()?.buildings;
                buildings
                    .iter()
                    .find(|building| {
                        building.tile_idx == tile_idx
                            && building.kind == sow_core::game::BuildingKind::City
                    })
                    .or_else(|| buildings.iter().find(|building| building.tile_idx == tile_idx))
                    .map(|building| {
                        (
                            building.kind,
                            building.active_level(),
                            building.under_construction,
                        )
                    })
            })
            .flatten();
        if view == MapContextMenuView::BuildingDetails
            && !already_selected
            && let Some((kind, _, _)) = building
            && self.sim.map_w > 0
        {
            let wx = (tile_idx % self.sim.map_w) as f32 + 0.5;
            let wy = (tile_idx / self.sim.map_w) as f32 + 0.5;
            self.sfx.play_building_select(
                web_time::Instant::now(),
                crate::building_sound_kind(kind),
                self.spatial_sound_params(wx, wy),
            );
        }
        self.input.map_context_menu = Some(MapContextMenu {
            x: x as f32,
            y: y as f32,
            tile_idx,
            session,
            view,
            building_kind: building.map(|building| building.0),
            building_level: building.map_or(0, |building| building.1),
            building_under_construction: building.is_some_and(|building| building.2),
            opened_at: web_time::Instant::now(),
        });
    }

    #[track_caller]
    pub(crate) fn close_map_context_menu(&mut self) {
        if let Some(menu) = self.input.map_context_menu {
            log::info!(
                "[MAP MENU TRACE] closed session={} view={:?} at {}",
                menu.session,
                menu.view,
                std::panic::Location::caller()
            );
        }
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
        use sow_core::warp_fleet::FleetLaunchError as Error;
        let text = match error {
            Error::InvalidTile | Error::TargetPlayerNotFound { .. } => {
                crate::ui::UiText::new("hud.fleet_invalid_target")
            }
            Error::SelfTarget => crate::ui::UiText::new("hud.fleet_own_target"),
            Error::NoWaterAccess | Error::NoLaunchShore { .. } => {
                crate::ui::UiText::new("hud.fleet_no_water_access")
            }
            Error::NoLandingShore => crate::ui::UiText::new("hud.fleet_no_landing_shore"),
            Error::NoWaterPath => crate::ui::UiText::new("hud.fleet_no_water_path"),
        };
        self.add_map_feedback(text);
    }

    fn show_map_menu_unavailable(&mut self, tile_idx: u32) {
        if self.ui.observing {
            return;
        }
        if self
            .map_target(tile_idx)
            .is_some_and(|target| target.owner == 0)
        {
            if let Err(error) = self.fleet_route_check(tile_idx, 0, FleetRouteCheck::Access) {
                self.show_fleet_unavailable(error);
                return;
            }
        }
        let key = match self.map_target(tile_idx) {
            Some(target) if target.is_teammate => "hud.fleet_teammate",
            Some(target) if target.owner == target.my_id && !target.is_land => {
                "hud.build_owned_land"
            }
            Some(target) if target.owner == target.my_id => {
                let building = self.sim.current_snapshot.as_ref().and_then(|snapshot| {
                    snapshot
                        .buildings
                        .iter()
                        .find(|building| building.tile_idx == tile_idx)
                });
                match building {
                    Some(building) if building.under_construction => "hud.building_in_progress",
                    _ => "hud.action_unavailable",
                }
            }
            _ => "hud.action_unavailable",
        };
        self.add_map_feedback(crate::ui::UiText::new(key));
    }

    fn has_fleet_capacity(&self, player_id: u16) -> bool {
        self.sim
            .current_snapshot
            .as_ref()
            .and_then(|snapshot| {
                snapshot
                    .players
                    .iter()
                    .find(|player| player.id == player_id)
            })
            .is_none_or(|player| player.boats_in_use < player.boat_capacity)
    }

    pub(crate) fn map_menu_actions(&mut self, tile_idx: u32) -> Vec<MapMenuAction> {
        if self.ui.observing {
            return Vec::new();
        }
        let Some(target) = self.map_target(tile_idx) else {
            return Vec::new();
        };
        let spawning = self.sim.current_snapshot.as_ref().is_some_and(|snapshot| {
            matches!(snapshot.phase, sow_core::game::GamePhase::Spawning { .. })
        });
        let campaign_fleet = self
            .sim
            .engine
            .as_ref()
            .is_some_and(|engine| engine.campaign_unlocks.is_some());
        let can_fleet = !spawning
            && target.owner == 0
            && ((campaign_fleet
                && target.is_land
                && self
                    .campaign_action_unlocked(sow_core::campaign::CampaignAction::TransportFleet))
                || (!campaign_fleet
                    && self.has_fleet_capacity(target.my_id)
                    && self
                        .fleet_route_check(tile_idx, target.owner, FleetRouteCheck::Access)
                        .is_ok()));
        let can_betray_fleet = !spawning
            && !campaign_fleet
            && (target.can_betray_teammate() || (target.is_allied && !target.is_teammate))
            && self.has_fleet_capacity(target.my_id)
            && attack_troops_meet_minimum(
                self.ui.app.hud_state.troops * self.ui.app.hud_state.attack_ratio as f64,
                self.sim.config.attack_cost_neutral,
            )
            && self
                .fleet_route_check(tile_idx, target.owner, FleetRouteCheck::Path)
                .is_ok();
        let mut actions = target.menu_actions(
            spawning,
            target.is_land && self.can_attack(tile_idx, target.owner),
            can_fleet,
            can_betray_fleet,
        );
        if campaign_fleet {
            actions.retain(|action| match action {
                MapMenuAction::Fleet => self
                    .campaign_action_unlocked(sow_core::campaign::CampaignAction::TransportFleet),
                _ => true,
            });
        }
        let can_launch_nuke = !spawning
            && self.campaign_action_unlocked(sow_core::campaign::CampaignAction::Nuke);
        if target.owner != target.my_id || !target.is_land || spawning {
            if can_launch_nuke {
                actions.push(MapMenuAction::Nuke);
            }
            return actions;
        }

        actions.clear();
        if !self.ui.is_spectating
            && !self.ui.tutorial_camera_only
            && self.sim.current_snapshot.as_ref().is_some_and(|snapshot| {
                snapshot
                    .players
                    .iter()
                    .any(|p| p.id == target.my_id && p.can_express_emote)
            })
        {
            actions.push(MapMenuAction::Emote);
        }

        let building = self.sim.current_snapshot.as_ref().and_then(|snapshot| {
            snapshot
                .buildings
                .iter()
                .find(|b| b.tile_idx == tile_idx && b.kind == sow_core::game::BuildingKind::City)
                .or_else(|| snapshot.buildings.iter().find(|b| b.tile_idx == tile_idx))
        });
        match building {
            Some(building) if !building.under_construction => {
                if building.level < building.kind.max_level()
                    && building.level.saturating_add(1)
                        <= self.campaign_building_max_level(building.kind)
                {
                    actions.push(MapMenuAction::UpgradeStructure);
                }
            }
            Some(_) => {}
            None => {
                if let Some((col, row)) = self.tile_coords(tile_idx) {
                    for action in [
                        MapMenuAction::BuildCity,
                        MapMenuAction::BuildTradingPost,
                        MapMenuAction::BuildPort,
                        MapMenuAction::BuildTower,
                        MapMenuAction::BuildMilitaryCamp,
                        MapMenuAction::BuildSam,
                MapMenuAction::BuildMissileSilo,
                    ] {
                        let Some(kind) = action.building_kind() else {
                            continue;
                        };
                        if self.campaign_building_max_level(kind) >= 1
                            && self.resolve_building_target(kind, col, row).is_ok()
                        {
                            actions.push(action);
                        }
                    }
                }
            }
        }
        actions.push(MapMenuAction::RequestAllies);
        if can_launch_nuke {
            actions.push(MapMenuAction::Nuke);
        }
        actions
    }

    pub(crate) fn map_menu_items(&mut self, tile_idx: u32) -> Vec<MapMenuItem> {
        let gold = self.current_player_gold();
        self.map_menu_actions(tile_idx)
            .into_iter()
            .map(|action| {
                let (cost, level) = self.map_menu_cost(action, tile_idx);
                let mut reason_key = None;
                let reason_level = None;
                let reason_seconds = None;
                let mut reason_troops = None;
                let campaign_fleet = action == MapMenuAction::Fleet
                    && self
                        .sim
                        .engine
                        .as_ref()
                        .is_some_and(|engine| engine.campaign_unlocks.is_some());
                let action_unavailable = if action == MapMenuAction::Alliance {
                    self.map_menu_alliance_state(tile_idx)
                        .is_none_or(|state| !state.can_act())
                } else if campaign_fleet && action == MapMenuAction::Fleet {
                    let blocker = self.map_target(tile_idx).and_then(|target| {
                        if target.is_teammate {
                            Some(("hud.fleet_teammate", None))
                        } else if target.is_allied {
                            Some(("hud.fleet_alliance", None))
                        } else if !self.has_fleet_capacity(target.my_id) {
                            Some(("hud.fleet_capacity_full", None))
                        } else if let Err(error) =
                            self.fleet_route_check(tile_idx, target.owner, FleetRouteCheck::Access)
                        {
                            Some((fleet_error_reason_key(error), None))
                        } else {
                            let troops = self.ui.app.hud_state.troops
                                * self.ui.app.hud_state.attack_ratio as f64;
                            let minimum = self.sim.config.attack_cost_neutral;
                            if !attack_troops_meet_minimum(troops, minimum) {
                                reason_troops = Some(minimum.ceil() as u64);
                                Some(("hud.fleet_need_troops", reason_troops))
                            } else {
                                None
                            }
                        }
                    });
                    if let Some((key, troops)) = blocker {
                        reason_key = Some(key);
                        reason_troops = troops;
                        true
                    } else {
                        false
                    }
                } else if action == MapMenuAction::Nuke {
                    let owner_id = self.sim.my_player_id.unwrap_or_default();
                    let has_nuclear_city = self.sim.current_snapshot.as_ref().is_some_and(|snapshot| {
                        sow_core::building::snapshot_has_nuclear_launch_city(
                            &snapshot.buildings,
                            owner_id,
                            self.sim.config.tutorial,
                        )
                    });
                    let has_ready_nuke = self.sim.current_snapshot.as_ref().is_some_and(|snapshot| {
                        snapshot.players.iter().any(|player| {
                            player.id == owner_id && player.nuke_available
                        })
                    });
                    if !self.campaign_action_unlocked(sow_core::campaign::CampaignAction::Nuke) {
                        reason_key = Some("hud.campaign_unlock_required");
                        true
                    } else if !has_nuclear_city {
                        reason_key = Some(self.nuclear_city_requirement_key());
                        true
                    } else if !has_ready_nuke {
                        reason_key = Some("hud.nuke_no_charges");
                        false
                    } else {
                        false
                    }
                } else if action == MapMenuAction::RequestAllies {
                    reason_key = (!self.has_trade_partners()).then_some("hud.resources_no_allies");
                    reason_key.is_some()
                } else {
                    false
                };
                MapMenuItem {
                    action,
                    cost,
                    level,
                    disabled: action_unavailable
                        || cost.is_some_and(|value| !value.is_finite() || gold < value),
                    reason_key,
                    reason_level,
                    reason_seconds,
                    reason_troops,
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
        // The radial's upgrade opens the same panel instead of a second flow.
        if action == MapMenuAction::UpgradeStructure && menu.view == MapContextMenuView::Radial {
            self.set_map_context_menu(
                f64::from(menu.x),
                f64::from(menu.y),
                tile_idx,
                MapContextMenuView::BuildingDetails,
            );
            return;
        }
        let keep_building_panel_open = menu.view == MapContextMenuView::BuildingDetails
            && matches!(action, MapMenuAction::UpgradeStructure | MapMenuAction::UpgradeStructureMax);
        if keep_building_panel_open {
            let Some(offer) = self.building_panel().and_then(|panel| panel.upgrade) else {
                return;
            };
            let available = match action {
                MapMenuAction::UpgradeStructure => offer.enabled,
                MapMenuAction::UpgradeStructureMax => offer.max_offer.is_some(),
                _ => false,
            };
            // A stale HUD tap must never dispatch an upgrade the live state rejects.
            if !available {
                return;
            }
        }
        if action == MapMenuAction::UpgradeStructureMax {
            self.upgrade_structure_max(tile_idx);
            return;
        }
        if !self.map_menu_actions(tile_idx).contains(&action) {
            if keep_building_panel_open {
                return;
            }
            self.show_map_menu_unavailable(tile_idx);
            self.close_map_context_menu();
            return;
        }
        if let Some(cost) = self.map_menu_cost(action, tile_idx).0
            && (!cost.is_finite() || self.current_player_gold() < cost)
        {
            if keep_building_panel_open {
                return;
            }
            let message = if cost.is_finite() {
                format!("Need {} gold.", crate::utils::format_number(cost))
            } else {
                "Action unavailable here.".to_string()
            };
            self.add_action_feedback(message);
            self.close_map_context_menu();
            return;
        }

        match action {
            MapMenuAction::Emote => return,
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
            MapMenuAction::Send => {
                self.open_transfer_from_tile(tile_idx, TransferMode::Send);
            }
            MapMenuAction::Request => {
                self.open_transfer_from_tile(tile_idx, TransferMode::Request);
            }
            MapMenuAction::RequestAllies => {
                // Our own id aims the panel at every ally; it sends one offer to each.
                if let Some(my_id) = self.sim.my_player_id
                    && self.has_trade_partners()
                {
                    self.open_transfer(my_id, TransferMode::Request);
                }
            }
            MapMenuAction::Alliance => {
                self.alliance_from_tile(tile_idx);
            }
            MapMenuAction::BuildCity => {
                self.start_building_from_map_menu(sow_core::game::BuildingKind::City, tile_idx);
            }
            MapMenuAction::BuildTower => {
                self.start_building_from_map_menu(sow_core::game::BuildingKind::Tower, tile_idx);
            }
            MapMenuAction::BuildTradingPost => {
                self.start_building_from_map_menu(sow_core::game::BuildingKind::TradingPost, tile_idx);
            }
            MapMenuAction::BuildPort => {
                self.start_building_from_map_menu(sow_core::game::BuildingKind::Port, tile_idx);
            }
            MapMenuAction::BuildMilitaryCamp => {
                self.start_building_from_map_menu(sow_core::game::BuildingKind::MilitaryCamp, tile_idx);
            }
            MapMenuAction::BuildMissileSilo => {
                self.start_building_from_map_menu(sow_core::game::BuildingKind::MissileSilo, tile_idx);
            }
            MapMenuAction::BuildSam => {
                self.start_building_from_map_menu(sow_core::game::BuildingKind::SAM, tile_idx);
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
            MapMenuAction::UpgradeStructureMax => {
                if let Some(building) =
                    self.sim.current_snapshot.as_ref().and_then(|snapshot| {
                        snapshot.buildings.iter().find(|b| b.tile_idx == tile_idx)
                    })
                {
                    self.send_intent(sow_core::protocol::GameplayIntent::UpgradeStructureTo {
                        building_id: building.id,
                        target_level: building.kind.max_level(),
                    });
                }
            }
            MapMenuAction::Nuke => {
                self.launch_nuke_at(sow_core::game::NukeKind::AtomBomb, tile_idx);
            }
        }
        if !keep_building_panel_open {
            self.close_map_context_menu();
        }
    }

    fn start_building_from_map_menu(
        &mut self,
        kind: sow_core::game::BuildingKind,
        tile_idx: u32,
    ) {
        if !kind.is_enabled() {
            return;
        }
        if self
            .ui
            .app
            .hud_state
            .building_placement
            .is_some_and(|placement| placement.pending)
        {
            return;
        }
        self.ui.app.hud_state.building_placement =
            Some(crate::ui::hud::state::BuildingPlacement {
                kind,
                tile_idx,
                pending: false,
            });
        self.confirm_building_placement();
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
        let owner = renderer.owners.get(idx).copied().unwrap_or(0);
        let (target_col, target_row) = if is_land && owner == 0 {
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
                let key = if is_land {
                    "hud.spawn_too_close"
                } else {
                    "hud.build_land"
                };
                self.add_map_feedback(crate::ui::UiText::new(key));
                return;
            };
            tile
        };

        self.send_intent(sow_core::protocol::GameplayIntent::Spawn {
            x: target_col as u32,
            y: target_row as u32,
        });
    }

    fn nearest_visible_structure_site(
        &mut self,
        kind: sow_core::game::BuildingKind,
        x: i32,
        y: i32,
        bounds: (i32, i32, i32, i32),
    ) -> Option<u32> {
        let (min_x, min_y, max_x, max_y) = bounds;
        let radius = x
            .abs_diff(min_x)
            .max(x.abs_diff(max_x))
            .max(y.abs_diff(min_y))
            .max(y.abs_diff(max_y)) as i32;
        let engine = self.sim.engine.as_mut()?;
        engine.refresh_building_grid();
        sow_core::building::nearest_structure_site_in_bounds(
            &engine.state.map,
            self.sim.my_player_id.unwrap_or(0),
            kind,
            x,
            y,
            radius,
            &engine.building_grid,
            bounds,
        )
    }

    /// Snap to a nearby foundation; keep the requested tile when none is found.
    pub(crate) fn move_building_preview(&mut self, col: i32, row: i32) {
        let Some(placement) = self
            .ui
            .app
            .hud_state
            .building_placement
            .filter(|p| !p.pending)
        else {
            return;
        };
        let (map_w, map_h) = (self.sim.map_w, self.sim.map_h);
        if map_w == 0 || map_h == 0 {
            return;
        }
        let (col, row) = (col.clamp(0, map_w as i32 - 1), row.clamp(0, map_h as i32 - 1));
        let requested_tile = row as u32 * map_w + col as u32;
        let bounds = if self.input.building_drag.is_some_and(|drag| drag.mobile_touch) {
            Some((0, 0, map_w as i32 - 1, map_h as i32 - 1))
        } else {
            self.visible_building_anchor_bounds(placement.kind)
        };
        let Some(engine) = self.sim.engine.as_mut() else {
            return;
        };
        engine.refresh_building_grid();
        let tile = bounds.and_then(|bounds| {
            sow_core::building::nearest_structure_site_in_bounds(
                &engine.state.map,
                self.sim.my_player_id.unwrap_or(0),
                placement.kind,
                col,
                row,
                PLACEMENT_SNAP_RADIUS,
                &engine.building_grid,
                bounds,
            )
        });
        let tile_idx = tile.unwrap_or(requested_tile);
        if let Some(placement) = self.ui.app.hud_state.building_placement.as_mut() {
            placement.tile_idx = tile_idx;
            if crate::web_canvas::has_mobile_pointer() {
                let footprint = sow_core::building::BuildingFootprint::at(
                    placement.kind,
                    tile_idx % engine.state.map.width,
                    tile_idx / engine.state.map.width,
                );
                let cx = footprint.left as f32 + footprint.width as f32 * 0.5;
                let cy = footprint.top as f32 + footprint.height as f32 * 0.5;
                self.input.touch_pan_residual = (0.0, 0.0);
                self.input.camera_focus_target = Some((cx, cy));
                self.input.camera_focus_waiting_for_input_release = false;
            }
        }
    }

    pub(crate) fn nudge_building_placement(
        &mut self,
        direction: super::placement::PlacementDirection,
    ) {
        if self.ui.observing
            || self.ui.tutorial_camera_only
            || self.ui.app.phase != crate::ClientPhase::Playing
            || self.ui.app.hud_state.sync_state.is_some()
            || self.sim.map_w == 0
        {
            return;
        }
        let Some(placement) = self
            .ui
            .app
            .hud_state
            .building_placement
            .filter(|p| !p.pending)
        else {
            return;
        };
        let (dx, dy) = direction.offset();
        self.input.building_drag = None;
        if !crate::web_canvas::has_mobile_pointer() {
            let (width, height) = (self.sim.map_w, self.sim.map_h);
            if height == 0 {
                return;
            }
            let x = (i64::from(placement.tile_idx % width) + i64::from(dx))
                .clamp(0, i64::from(width - 1)) as u32;
            let y = (i64::from(placement.tile_idx / width) + i64::from(dy))
                .clamp(0, i64::from(height - 1)) as u32;
            if let Some(preview) = self.ui.app.hud_state.building_placement.as_mut() {
                preview.tile_idx = y * width + x;
            }
            return;
        }
        let my_id = self.sim.my_player_id.unwrap_or(0);
        let Some(engine) = self.sim.engine.as_mut() else {
            return;
        };
        engine.refresh_building_grid();
        let map = &engine.state.map;
        let (x, y) = (
            (placement.tile_idx % map.width) as i32,
            (placement.tile_idx / map.width) as i32,
        );
        // Arrows jump over blocked tiles to the next legal foundation.
        let next = (1..=PLACEMENT_NUDGE_REACH)
            .map(|step| (x + dx * step, y + dy * step))
            .take_while(|&(nx, ny)| map.is_valid_coord(nx, ny))
            .map(|(nx, ny)| ny as u32 * map.width + nx as u32)
            .find(|&tile| {
                sow_core::building::validate_structure_site(
                    map,
                    my_id,
                    placement.kind,
                    tile,
                    &engine.building_grid,
                )
                .is_ok()
            });
        if let Some(tile) = next
            && let Some(placement) = self.ui.app.hud_state.building_placement.as_mut()
        {
            placement.tile_idx = tile;
            if crate::web_canvas::has_mobile_pointer() {
                let footprint = sow_core::building::BuildingFootprint::at(
                    placement.kind,
                    tile % map.width,
                    tile / map.width,
                );
                let cx = footprint.left as f32 + footprint.width as f32 * 0.5;
                let cy = footprint.top as f32 + footprint.height as f32 * 0.5;
                self.input.touch_pan_residual = (0.0, 0.0);
                self.input.camera_focus_target = Some((cx, cy));
                self.input.camera_focus_waiting_for_input_release = false;
            }
        }
    }

    pub(crate) fn confirm_building_placement(&mut self) {
        let Some(placement) = self.ui.app.hud_state.building_placement else {
            return;
        };
        if !self.building_placement_can_confirm() {
            return;
        }
        self.input.building_drag = None;
        self.input.camera_focus_target = None;
        self.input.touch_pan_residual = (0.0, 0.0);
        self.input.target_zoom = self.input.camera_zoom;
        self.ui
            .app
            .hud_state
            .building_placement
            .as_mut()
            .unwrap()
            .pending = true;
        if !self.send_intent(sow_core::protocol::GameplayIntent::BuildStructure {
            kind: placement.kind,
            target_tile: placement.tile_idx,
        }) && let Some(placement) = self
            .ui
            .app
            .hud_state
            .building_placement
            .as_mut()
            .filter(|p| p.pending)
        {
            // A processed paused-tutorial rejection already cleared pending.
            placement.pending = false;
        }
    }

    pub(crate) fn finish_building_confirmation(&mut self, accepted: bool) {
        let Some(placement) = self
            .ui
            .app
            .hud_state
            .building_placement
            .filter(|p| p.pending)
        else {
            return;
        };
        if accepted {
            let footprint = sow_core::building::BuildingFootprint::at(
                placement.kind,
                placement.tile_idx % self.sim.map_w,
                placement.tile_idx / self.sim.map_w,
            );
            let (x, y) = (footprint.left as f32 + 2.0, footprint.top as f32 + 2.0);
            self.ui.click_markers.push(crate::app::ClickMarker {
                world_x: x,
                world_y: y,
                start_time: web_time::Instant::now(),
                kind: crate::app::ClickMarkerKind::Generic,
            });
            self.sfx.play_placement(
                web_time::Instant::now(),
                crate::building_sound_kind(placement.kind),
                self.spatial_sound_params(x, y),
            );
            self.ui.app.hud_state.building_placement = None;
        } else {
            self.ui
                .app
                .hud_state
                .building_placement
                .as_mut()
                .unwrap()
                .pending = false;
        }
    }

    /// One rule for the drawn confirm button, the tutorial guide and the order itself.
    pub(crate) fn building_placement_can_confirm(&mut self) -> bool {
        let Some(placement) = self.ui
            .app
            .hud_state
            .building_placement
        else {
            return false;
        };
        !placement.pending
            && !self.ui.observing
            && !self.ui.tutorial_camera_only
            && self.ui.app.phase == crate::ClientPhase::Playing
            && self.ui.app.hud_state.sync_state.is_none()
            && self.building_placement_target_reason(placement).is_none()
            && self.campaign_building_max_level(placement.kind) >= 1
            && self.building_gold_shortfall(placement.kind).is_none()
    }

    pub(crate) fn building_placement_reason(&mut self) -> Option<crate::ui::UiText> {
        let placement = self.ui.app.hud_state.building_placement?;
        if let Some(reason) = self.building_placement_target_reason(placement) {
            return Some(reason);
        }
        if self.campaign_building_max_level(placement.kind) < 1 {
            return Some(crate::ui::UiText::new("hud.campaign_unlock_required"));
        }
        self.building_gold_shortfall(placement.kind)
    }

    fn building_placement_target_reason(
        &mut self,
        placement: crate::ui::hud::state::BuildingPlacement,
    ) -> Option<crate::ui::UiText> {
        if let Err(key) = self.resolve_building_target(
            placement.kind,
            (placement.tile_idx % self.sim.map_w) as i32,
            (placement.tile_idx / self.sim.map_w) as i32,
        ) {
            return Some(crate::ui::UiText::new(key));
        }
        if !self.building_footprint_is_visible(placement.kind, placement.tile_idx) {
            return Some(crate::ui::UiText::new("hud.build_no_space"));
        }
        None
    }

    pub(crate) fn nuclear_city_requirement_key(&self) -> &'static str {
        if self.sim.config.tutorial { "hud.nuke_requires_city" } else { "hud.industrial_city_required" }
    }

    pub(crate) fn building_foundation_level(&self, kind: sow_core::game::BuildingKind) -> u8 {
        self.ui.app.hud_state.building_foundation_levels[kind as usize]
    }

    /// One gold rule for the dock, hotkeys and the confirm button.
    fn building_gold_shortfall(
        &self,
        kind: sow_core::game::BuildingKind,
    ) -> Option<crate::ui::UiText> {
        if self.building_foundation_level(kind) == 0 {
            return Some(crate::ui::UiText::new(self.nuclear_city_requirement_key()));
        }
        let cost = self.ui.app.hud_state.building_costs[kind as usize];
        (!cost.is_finite() || self.current_player_gold() < cost).then(|| {
            crate::ui::UiText::new("hud.need_gold").with("cost", crate::utils::format_number(cost))
        })
    }

    fn resolve_building_target(
        &mut self,
        kind: sow_core::game::BuildingKind,
        col: i32,
        row: i32,
    ) -> Result<u32, &'static str> {
        if self.sim.map_w == 0 || self.sim.map_h == 0 {
            return Err("hud.action_unavailable");
        }
        let Some(engine) = self.sim.engine.as_mut() else {
            return Err("hud.action_unavailable");
        };
        if col < 0
            || row < 0
            || col >= engine.state.map.width as i32
            || row >= engine.state.map.height as i32
        {
            return Err("hud.build_outside_map");
        }
        // Paused tutorial actions change ownership before the next GPU upload.
        // Validate against the simulation that will accept the order.
        engine.refresh_building_grid();
        let tile = row as u32 * engine.state.map.width + col as u32;
        sow_core::building::validate_structure_site(
            &engine.state.map,
            self.sim.my_player_id.unwrap_or(0),
            kind,
            tile,
            &engine.building_grid,
        )
        .map(|()| tile)
        .map_err(|error| error.reason_key())
    }

    pub(crate) fn map_menu_cost(
        &self,
        action: MapMenuAction,
        tile_idx: u32,
    ) -> (Option<f64>, Option<u8>) {
        match action {
            MapMenuAction::Nuke => (
                Some(self.sim.engine.as_ref().map_or_else(
                    || sow_core::game::NukeKind::AtomBomb.gold_cost(0),
                    |engine| engine.nuclear_price(sow_core::game::NukeKind::AtomBomb),
                )),
                None,
            ),
            action if let Some(kind) = action.building_kind() => {
                let level = self.building_foundation_level(kind);
                (Some(self.ui.app.hud_state.building_costs[kind as usize]).filter(|c| c.is_finite()), Some(level))
            }
            MapMenuAction::UpgradeStructure => {
                let Some(snapshot) = self.sim.current_snapshot.as_ref() else {
                    return (None, None);
                };
                let Some(building) = snapshot
                    .buildings
                    .iter()
                    .find(|building| building.tile_idx == tile_idx)
                else {
                    return (None, None);
                };
                let cost = self
                    .sim
                    .engine
                    .as_ref()
                    .and_then(|engine| engine.structure_upgrade_quote(building.id))
                    .map_or_else(
                        || {
                            let built_levels = snapshot
                                .players
                                .iter()
                                .find(|player| player.id == building.owner_id)
                                .map_or(0, |player| player.structure_levels_built[building.kind as usize]);
                            sow_core::building::cost::structure_upgrade_cost_gold(
                                building.kind,
                                building.active_level().saturating_add(1),
                                built_levels,
                                &self.sim.config,
                            )
                        },
                        |(cost, _)| cost,
                    );
                (Some(cost), Some(building.level))
            }
            _ => (None, None),
        }
    }

    fn upgrade_structure_max(&mut self, tile_idx: u32) {
        let Some(building_id) = self
            .sim
            .current_snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.buildings.iter().find(|b| b.tile_idx == tile_idx))
            .map(|b| (b.id, b.kind))
        else {
            return;
        };
        let (id, kind) = building_id;
        let cap = self
            .campaign_building_max_level(kind)
            .min(kind.max_level());
        let gold = self.current_player_gold();
        let Some((target_level, _, _)) = self
            .sim
            .engine
            .as_ref()
            .and_then(|engine| engine.max_affordable_upgrade_level(id, gold, cap))
        else {
            return;
        };
        self.send_intent(sow_core::protocol::GameplayIntent::UpgradeStructureTo {
            building_id: id,
            target_level,
        });
    }

    fn launch_nuke_at(&mut self, kind: sow_core::game::NukeKind, tile_idx: u32) -> bool {
        if self.map_target(tile_idx).is_none() {
            return false;
        }
        self.send_intent(sow_core::protocol::GameplayIntent::LaunchNuke {
            kind,
            target_tile: tile_idx,
        });
        self.ui.app.hud_state.selected_nuke_kind = None;
        true
    }

    fn primary_target(&mut self, tile_idx: u32, x: f64, y: f64) {
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
            self.open_map_context_menu(x, y);
        } else if target.is_attackable() && !self.can_attack(tile_idx, target.owner) {
            self.launch_fleet_from_tile(tile_idx);
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
        if target.is_teammate && !target.can_betray_teammate() {
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
            target_tile: Some(tile_idx),
        });
        if !target.is_attackable() && !target.is_allied && !target.can_betray_teammate() {
            self.add_map_feedback(crate::ui::UiText::new("hud.attack_out_of_range"));
            return false;
        }
        if !self.can_attack(tile_idx, target.owner) {
            self.add_map_feedback(crate::ui::UiText::new("hud.attack_out_of_range"));
            return false;
        }
        if target.needs_betrayal_confirmation() {
            self.ui.app.hud_state.show_betrayal_warning = Some((target.owner, intent));
            return true;
        }
        self.send_intent(intent)
    }

    pub(crate) fn launch_fleet_from_tile(&mut self, tile_idx: u32) -> bool {
        if !self.campaign_action_unlocked(sow_core::campaign::CampaignAction::TransportFleet) {
            self.add_map_feedback(crate::ui::UiText::new("hud.campaign_unlock_required"));
            return false;
        }
        let Some(target) = self.map_target(tile_idx) else {
            return false;
        };
        if target.is_teammate && !target.can_betray_teammate() {
            self.add_map_feedback(crate::ui::UiText::new("hud.fleet_teammate"));
            return false;
        }
        let can_betray_allied = target.is_allied && !target.is_teammate;
        if can_betray_allied
            && self
                .sim
                .engine
                .as_ref()
                .is_some_and(|engine| engine.campaign_unlocks.is_some())
        {
            self.add_map_feedback(crate::ui::UiText::new("hud.fleet_alliance"));
            return false;
        }
        if target.owner != 0
            && !target.is_enemy()
            && !target.can_betray_teammate()
            && !can_betray_allied
        {
            self.add_map_feedback(crate::ui::UiText::new("hud.fleet_own_target"));
            return false;
        }
        if !self.has_fleet_capacity(target.my_id) {
            self.add_map_feedback(crate::ui::UiText::new("hud.fleet_capacity_full"));
            return false;
        }
        if let Err(error) = self.fleet_route_check(tile_idx, target.owner, FleetRouteCheck::Path) {
            self.show_fleet_unavailable(error);
            return false;
        }
        let troops = self.ui.app.hud_state.troops * self.ui.app.hud_state.attack_ratio as f64;
        if troops < self.sim.config.attack_cost_neutral {
            self.add_map_feedback(crate::ui::UiText::new("hud.fleet_need_troops").with(
                "troops",
                crate::utils::format_number(self.sim.config.attack_cost_neutral),
            ));
            return false;
        }
        let intent = sow_core::protocol::GameplayIntent::LaunchFleet {
            target_tile: tile_idx,
            troops: Some(troops),
        };
        if target.needs_betrayal_confirmation() {
            self.ui.app.hud_state.show_betrayal_warning = Some((target.owner, intent));
            true
        } else {
            self.send_intent(intent)
        }
    }

    fn open_transfer_from_tile(&mut self, tile_idx: u32, mode: TransferMode) -> bool {
        let Some(target) = self.map_target(tile_idx) else {
            return false;
        };
        if !target.is_friendly() {
            self.add_map_feedback(crate::ui::UiText::new("hud.resources_allies_only"));
            return false;
        }
        self.open_transfer(target.owner, mode);
        true
    }

    fn open_transfer(&mut self, target: u16, mode: TransferMode) {
        self.ui.app.hud_state.show_ask_panel = Some(target);
        self.ui.app.hud_state.transfer_mode = Some(mode);
        self.ui.app.hud_state.transfer_confirm_pending = false;
    }

    /// Whether we have a living ally or teammate; the self-click panel needs one.
    fn has_trade_partners(&self) -> bool {
        let Some(snapshot) = self.sim.current_snapshot.as_ref() else {
            return false;
        };
        let my_id = self.sim.my_player_id.unwrap_or_default();
        snapshot
            .players
            .iter()
            .find(|player| player.id == my_id)
            .is_some_and(|me| me.trade_partners(&snapshot.players).next().is_some())
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

    pub(super) fn is_water_map_target(&self, tile_idx: u32) -> bool {
        self.map_target(tile_idx).is_some_and(|target| !target.is_land)
    }

    fn map_target(&self, tile_idx: u32) -> Option<MapTarget> {
        let map = &self.sim.engine.as_ref()?.state.map;
        let map_len = map.width.checked_mul(map.height)?;
        if tile_idx >= map_len {
            return None;
        }
        let owner = map.owner_id(tile_idx % map.width, tile_idx / map.width);
        let is_land = map.terrain.get(tile_idx as usize)?.is_land();
        let my_id = self.sim.my_player_id.unwrap_or(0);
        let snapshot = self.sim.current_snapshot.as_ref()?;
        let me = snapshot.players.iter().find(|player| player.id == my_id);
        let other = snapshot.players.iter().find(|player| player.id == owner);
        let is_betrayer = other.is_some_and(|player| player.can_retaliate);
        let me_can_retaliate = me.is_some_and(|player| player.can_retaliate);
        let is_teammate = me
            .zip(other)
            .is_some_and(|(me, other)| me.team.is_some() && me.team == other.team);
        let team_betrayal_enabled = self.sim.config.game_mode == "Teams"
            && self
                .sim
                .engine
                .as_ref()
                .is_some_and(|engine| engine.campaign_unlocks.is_none());
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
            is_betrayer,
            me_can_retaliate,
            team_betrayal_enabled,
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
            kind: crate::app::ClickMarkerKind::Generic,
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
        self.sfx.play_action_error(web_time::Instant::now());
    }

    pub(crate) fn clear_placement(&mut self) {
        self.clear_placement_inner(true);
    }

    fn clear_placement_inner(&mut self, emit_exit_sfx: bool) {
        if self
            .ui
            .app
            .hud_state
            .building_placement
            .is_some_and(|p| p.pending)
        {
            return;
        }
        if self.ui.app.hud_state.building_placement.is_some() {
            self.input.camera_focus_target = None;
            self.input.touch_pan_residual = (0.0, 0.0);
            self.input.target_zoom = self.input.camera_zoom;
            if emit_exit_sfx {
                self.sfx.play_building_mode(2);
            }
        }
        self.ui.app.hud_state.building_placement = None;
        self.ui.app.hud_state.selected_nuke_kind = None;
        self.input.building_drag = None;
    }

    pub(crate) fn select_building_kind(&mut self, kind: sow_core::game::BuildingKind) {
        if !kind.is_enabled() {
            return;
        }
        if self.ui.observing
            || self.ui.tutorial_camera_only
            || self.ui.app.phase != crate::ClientPhase::Playing
            || self.ui.app.hud_state.sync_state.is_some()
            || self
                .ui
                .app
                .hud_state
                .building_placement
                .is_some_and(|p| p.pending)
            || !self
                .sim
                .current_snapshot
                .as_ref()
                .is_some_and(|s| matches!(s.phase, sow_core::game::GamePhase::Playing))
        {
            return;
        }
        if self.campaign_building_max_level(kind) < 1 {
            self.add_map_feedback(crate::ui::UiText::new("hud.campaign_unlock_required"));
            return;
        }
        if self
            .ui
            .app
            .hud_state
            .building_placement
            .is_some_and(|p| p.kind == kind)
        {
            self.clear_placement();
            return;
        }
        if let Some(shortfall) = self.building_gold_shortfall(kind) {
            self.add_map_feedback(shortfall);
            return;
        }
        let [origin_x, origin_y] = self.input.screen_to_world([
            self.input.screen_w * 0.5,
            self.input.screen_h * 0.45,
        ]);
        let (origin_x, origin_y) = (origin_x as i32, origin_y as i32);
        let my_id = self.sim.my_player_id.unwrap_or(0);
        let tutorial_site = if self.ui.tutorial_active
            && self.ui.tutorial_campaign.is_boudica()
            && kind == sow_core::game::BuildingKind::City
        {
            self.sim
                .current_snapshot
                .as_ref()
                .filter(|snapshot| {
                    !snapshot
                        .buildings
                        .iter()
                        .any(|building| building.owner_id == my_id && building.kind == kind)
                })
                .and_then(|snapshot| {
                    let observation = &self.sim.tutorial_observation;
                    (self.sim.map_w > 0
                        && observation.guide_tick == snapshot.tick
                        && observation.guide_build_site_kind == Some(kind))
                    .then(|| {
                        observation
                            .guide_build_site_candidates
                            .iter()
                            .max_by_key(|candidate| (candidate.0, candidate.3 % self.sim.map_w))
                            .map(|candidate| candidate.3)
                    })
                    .flatten()
                })
        } else {
            None
        };
        let (map_w, map_h) = (self.sim.map_w, self.sim.map_h);
        if map_w == 0 || map_h == 0 {
            return;
        }
        let requested_x = origin_x.clamp(0, map_w as i32 - 1);
        let requested_y = origin_y.clamp(0, map_h as i32 - 1);
        let requested_tile = requested_y as u32 * map_w + requested_x as u32;
        let site = tutorial_site.or_else(|| {
            self.visible_building_anchor_bounds(kind)
                .and_then(|bounds| {
                    self.nearest_visible_structure_site(kind, origin_x, origin_y, bounds)
                })
        });
        let tile = site.unwrap_or(requested_tile);
        let was_building_mode = self.ui.app.hud_state.building_placement.is_some();
        self.input.key_pan_up = false;
        self.input.key_pan_down = false;
        self.input.key_pan_left = false;
        self.input.key_pan_right = false;
        self.ui.app.hud_state.building_placement = Some(crate::ui::hud::state::BuildingPlacement {
            kind,
            tile_idx: tile,
            pending: false,
        });
        self.sfx
            .play_building_mode(u8::from(was_building_mode));
        self.ui.app.hud_state.selected_nuke_kind = None;
        self.input.building_drag = None;
        self.input.touch_pan_residual = (0.0, 0.0);
        self.input.camera_focus_target = None;
        self.input.camera_focus_waiting_for_input_release = false;
        self.close_map_context_menu();
        if crate::web_canvas::has_mobile_pointer() {
            let sf = crate::web_canvas::device_pixel_ratio() as f32;
            self.input.target_zoom =
                self.input
                    .target_zoom
                    .max(14.0 * sf)
                    .min(crate::camera_zoom_upper_bound(
                        self.input.screen_w,
                        self.input.screen_h,
                    ));
            let map_w = self.sim.map_w.max(1);
            let footprint =
                sow_core::building::BuildingFootprint::at(kind, tile % map_w, tile / map_w);
            let cx = footprint.left as f32 + footprint.width as f32 * 0.5;
            let cy = footprint.top as f32 + footprint.height as f32 * 0.5;
            self.input.camera_focus_target = Some((cx, cy));
        } else {
            self.input.target_zoom = self.input.camera_zoom;
        }
    }

    pub(crate) fn structure_touches_water(
        &self,
        kind: sow_core::game::BuildingKind,
        tile_idx: u32,
    ) -> bool {
        if let Some(engine) = self.sim.engine.as_ref() {
            let (w, h) = (engine.state.map.width, engine.state.map.height);
            if w == 0 || h == 0 {
                return false;
            }
            return sow_core::building::structure_footprint_touches_water(
                kind,
                tile_idx % w,
                tile_idx / w,
                w,
                h,
                |fx, fy| engine.state.map.terrain[(fy * w + fx) as usize].as_byte(),
            );
        }
        let Some(renderer) = self.gfx.map_renderer.as_ref() else {
            return false;
        };
        let (w, h) = (renderer.width, renderer.height);
        if w == 0 || h == 0 || renderer.terrain.len() < (w * h) as usize {
            return false;
        }
        sow_core::building::structure_footprint_touches_water(
            kind,
            tile_idx % w,
            tile_idx / w,
            w,
            h,
            |fx, fy| renderer.terrain[(fy * w + fx) as usize],
        )
    }

    pub(crate) fn build_or_upgrade_building(
        &mut self,
        tile_idx: u32,
        kind: sow_core::game::BuildingKind,
    ) {
        if self.ui.observing
            || self.ui.tutorial_camera_only
            || self.ui.app.phase != crate::ClientPhase::Playing
        {
            return;
        }
        let owner_id = self
            .sim
            .my_player_id
            .unwrap_or(self.ui.app.hud_state.my_player_id);
        let Some(snapshot) = self.sim.current_snapshot.as_ref() else {
            return;
        };
        if !matches!(snapshot.phase, sow_core::game::GamePhase::Playing) {
            return;
        }
        let existing = snapshot.buildings.iter().find(|b| {
            b.tile_idx == tile_idx
                && b.owner_id == owner_id
                && b.kind == kind
        });
        if existing.is_some_and(|building| {
            building.under_construction || building.arsenal_build.is_some()
        }) {
            return;
        }
        let player_gold = snapshot
            .players
            .iter()
            .find(|p| p.id == owner_id)
            .map_or(0.0, |p| p.gold);
        if let Some(district) = existing {
            let next_level = district.active_level().saturating_add(1);
            if next_level > kind.max_level()
                || next_level > self.campaign_building_max_level(kind)
            {
                self.add_map_feedback(crate::ui::UiText::new("hud.campaign_unlock_required"));
                return;
            }
            let built_levels = snapshot
                .players
                .iter()
                .find(|player| player.id == owner_id)
                .map_or(0, |player| player.structure_levels_built[kind as usize]);
            let cost = self
                .sim
                .engine
                .as_ref()
                .and_then(|engine| engine.structure_upgrade_quote(district.id))
                .map_or_else(
                    || {
                        sow_core::building::cost::structure_upgrade_cost_gold(
                            kind,
                            next_level,
                            built_levels,
                            &self.sim.config,
                        )
                    },
                    |(cost, _)| cost,
                );
            if !cost.is_finite() || player_gold < cost {
                self.add_map_feedback(
                    crate::ui::UiText::new("hud.need_gold")
                        .with("cost", crate::utils::format_number(cost)),
                );
                return;
            }
            self.send_intent(sow_core::protocol::GameplayIntent::UpgradeStructure {
                building_id: district.id,
            });
            return;
        }
        if self.campaign_building_max_level(kind) < 1 {
            self.add_map_feedback(crate::ui::UiText::new("hud.campaign_unlock_required"));
            return;
        }
        if kind == sow_core::game::BuildingKind::Port
            && !self.structure_touches_water(sow_core::game::BuildingKind::Port, tile_idx)
        {
            self.add_map_feedback(crate::ui::UiText::new("hud.fleet_no_water_access"));
            return;
        }
        let cost = self.ui.app.hud_state.building_costs[kind as usize];
        if !cost.is_finite() || player_gold < cost {
            self.add_map_feedback(
                crate::ui::UiText::new("hud.need_gold")
                    .with("cost", crate::utils::format_number(cost)),
            );
            return;
        }
        self.send_intent(sow_core::protocol::GameplayIntent::BuildStructure {
            kind,
            target_tile: tile_idx,
        });
    }

    pub(crate) fn queue_building_production(
        &mut self,
        building_id: u64,
        kind: sow_core::game::CityArsenalKind,
    ) {
        if self.ui.observing
            || self.ui.tutorial_camera_only
            || self.ui.app.phase != crate::ClientPhase::Playing
        {
            return;
        }
        match kind {
            sow_core::game::CityArsenalKind::Warship => {
                if !self.campaign_action_unlocked(sow_core::campaign::CampaignAction::Warship) {
                    self.add_map_feedback(crate::ui::UiText::new("hud.campaign_unlock_required"));
                    return;
                }
            }
            sow_core::game::CityArsenalKind::Nuke => {
                if !self.campaign_action_unlocked(sow_core::campaign::CampaignAction::Nuke) {
                    self.add_map_feedback(crate::ui::UiText::new("hud.campaign_unlock_required"));
                    return;
                }
            }
            sow_core::game::CityArsenalKind::AntiNuke => return,
        }
        let owner_id = self
            .sim
            .my_player_id
            .unwrap_or(self.ui.app.hud_state.my_player_id);
        let Some(snapshot) = self.sim.current_snapshot.as_ref() else {
            return;
        };
        if !matches!(snapshot.phase, sow_core::game::GamePhase::Playing) {
            return;
        }
        let Some(producer) = snapshot
            .buildings
            .iter()
            .find(|building| building.id == building_id && building.owner_id == owner_id)
        else {
            return;
        };
        let qualifies = match kind {
            sow_core::game::CityArsenalKind::Warship => {
                producer.kind == sow_core::game::BuildingKind::Port && producer.active_level() >= 1
            }
            sow_core::game::CityArsenalKind::Nuke | sow_core::game::CityArsenalKind::AntiNuke => false,
        };
        if !qualifies || producer.under_construction {
            return;
        }
        let queue_len = producer
            .warship_queue
            .len()
            .max(usize::from(producer.arsenal_build.is_some_and(|b| {
                b.kind == sow_core::game::CityArsenalKind::Warship
            })));
        if kind == sow_core::game::CityArsenalKind::Warship
            && queue_len >= sow_core::building::MAX_WARSHIP_QUEUE
        {
            self.add_map_feedback(crate::ui::UiText::new("hud.warship_queue_full"));
            return;
        }
        let Some(player) = snapshot.players.iter().find(|p| p.id == owner_id) else {
            return;
        };
        let queued_warships: u32 = snapshot
            .buildings
            .iter()
            .filter(|building| building.owner_id == owner_id)
            .map(|building| {
                building
                    .warship_queue
                    .len()
                    .max(usize::from(building.arsenal_build.is_some_and(|build| {
                        build.kind == sow_core::game::CityArsenalKind::Warship
                    }))) as u32
            })
            .sum();
        if kind == sow_core::game::CityArsenalKind::Warship
            && player.boats_in_use.saturating_add(queued_warships) >= player.boat_capacity
        {
            self.add_map_feedback(crate::ui::UiText::new("hud.fleet_capacity_full"));
            return;
        }
        let owned_warships = if kind == sow_core::game::CityArsenalKind::Warship {
            (snapshot
                .fleets
                .iter()
                .filter(|f| {
                    f.owner_id == owner_id && f.unit_type == sow_core::game::UnitType::Warship
                })
                .count() as u32)
                .saturating_add(queued_warships)
        } else {
            0
        };
        let cost = sow_core::building::city_arsenal_cost_gold(
            kind,
            owned_warships,
            player.leader,
            &self.sim.config,
        );
        if !cost.is_finite() || player.gold < cost {
            self.add_map_feedback(
                crate::ui::UiText::new("hud.need_gold")
                    .with("cost", crate::utils::format_number(cost)),
            );
            return;
        }
        self.send_intent(sow_core::protocol::GameplayIntent::BuildingProduction {
            building_id,
            kind,
        });
    }

    pub(crate) fn queue_building_production_max(
        &mut self,
        building_id: u64,
        kind: sow_core::game::CityArsenalKind,
    ) {
        if kind != sow_core::game::CityArsenalKind::Warship {
            self.queue_building_production(building_id, kind);
            return;
        }
        if self.ui.observing
            || self.ui.app.phase != crate::ClientPhase::Playing
            || !self.campaign_action_unlocked(sow_core::campaign::CampaignAction::Warship)
        {
            return;
        }
        self.send_intent(sow_core::protocol::GameplayIntent::BuildingProductionMax {
            building_id,
            kind,
        });
    }

    pub(crate) fn cancel_warship_queue(&mut self, building_id: u64, index: u8) {
        if self.ui.observing || self.ui.app.phase != crate::ClientPhase::Playing {
            return;
        }
        self.send_intent(sow_core::protocol::GameplayIntent::CancelWarshipQueue {
            building_id,
            index,
        });
    }

    pub(crate) fn select_nuke_kind(&mut self, kind: sow_core::game::NukeKind) {
        if self
            .ui
            .app
            .hud_state
            .building_placement
            .is_some_and(|p| p.pending)
        {
            return;
        }
        if self.ui.observing
            || self.ui.app.phase != crate::ClientPhase::Playing
            || !self.sim.current_snapshot.as_ref().is_some_and(|snapshot| {
                matches!(snapshot.phase, sow_core::game::GamePhase::Playing)
            })
        {
            return;
        }
        if !self.campaign_action_unlocked(sow_core::campaign::CampaignAction::Nuke) {
            self.add_map_feedback(crate::ui::UiText::new("hud.campaign_unlock_required"));
            return;
        }
        let owner_id = self
            .sim
            .my_player_id
            .unwrap_or(self.ui.app.hud_state.my_player_id);
        if !self.sim.current_snapshot.as_ref().is_some_and(|snapshot| {
            sow_core::building::snapshot_has_nuclear_launch_city(&snapshot.buildings, owner_id, self.sim.config.tutorial)
        }) {
            self.add_map_feedback(crate::ui::UiText::new(self.nuclear_city_requirement_key()));
            return;
        }
        let cost = self.sim.engine.as_ref().map_or_else(|| kind.gold_cost(0), |engine| engine.nuclear_price(kind));
        let Some(player) = self
            .sim
            .current_snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.players.iter().find(|player| player.id == owner_id))
        else {
            return;
        };
        if !player.nuke_available {
            self.add_map_feedback(crate::ui::UiText::new("hud.nuke_no_charges"));
            return;
        }
        if !cost.is_finite() || cost < 0.0 || player.gold < cost {
            self.add_map_feedback(
                crate::ui::UiText::new("hud.not_enough_gold")
                    .with("cost", cost.ceil().to_string())
                    .with("available", player.gold.floor().to_string()),
            );
            return;
        }
        if self.ui.app.hud_state.building_placement.is_some() {
            self.clear_placement_inner(false);
        }
        let selected = &mut self.ui.app.hud_state.selected_nuke_kind;
        *selected = (*selected != Some(kind)).then_some(kind);
        self.ui.app.hud_state.building_placement = None;
        if let Some(armed) = self.ui.app.hud_state.selected_nuke_kind {
            self.sfx
                .play_nuke_armed(crate::app::audio::nuke_kind_level(armed));
            self.close_map_context_menu();
        }
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
    match message {
        "No action is available here." | "Action unavailable here." => {
            UiText::new("hud.action_unavailable")
        }
        "Alliance renewal is already pending." => UiText::new("hud.alliance_renewal_pending"),
        "Alliance request already pending." => UiText::new("hud.alliance_request_pending"),
        "Build inside your own land." => UiText::new("hud.build_owned_land"),
        "Structures go on land, not water." => UiText::new("hud.build_land"),
        "No space nearby!" => UiText::new("hud.build_no_space"),
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
    for raw_idx in border_tiles.ones() {
        if raw_idx / map_w >= map_h || owners.get(raw_idx as usize).copied() != Some(my_id) {
            continue;
        }
        let col = (raw_idx % map_w) as i32;
        let row = (raw_idx / map_w) as i32;
        for (dc, dr) in LAND_BORDER_NEIGHBORS {
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

