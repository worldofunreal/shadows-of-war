use crate::app::{MapContextMenu, MapContextMenuView, SowApp};
use serde::Deserialize;

pub(crate) const TOUCH_HOLD_MS: u128 = 300;
const MAP_CLICK_MAX_DISTANCE_SQ: f64 = 400.0;
/// Drag and tap snap the preview to a legal foundation within this many tiles.
const PLACEMENT_SNAP_RADIUS: i32 = 6;
/// Arrow keys look this far ahead for the next legal foundation.
const PLACEMENT_NUDGE_REACH: i32 = 8;

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
    Emote,
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
}

impl MapMenuAction {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Emote => "emote",
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
        if self.ui.tutorial_camera_only
            || self.ui.observing
            || self.ui.app.phase != crate::ClientPhase::Playing
            || self.ui.app.hud_state.building_placement.is_some()
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
        if let Some(button) = self
            .placement_controls()
            .and_then(|controls| controls.button_at(x as f32, y as f32))
        {
            match button {
                super::placement::PlacementButton::Confirm => {
                    // A greyed confirm explains itself instead of doing nothing.
                    if !self.building_placement_can_confirm()
                        && let Some(reason) = self.building_placement_reason()
                    {
                        self.add_map_feedback(reason);
                    }
                    self.confirm_building_placement();
                }
                super::placement::PlacementButton::Cancel => self.clear_placement(),
            }
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
        if self.ui.app.hud_state.building_placement.is_some() {
            self.move_building_preview(col, row);
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
        self.primary_target(tile_idx, x, y);
    }

    fn try_tutorial_campaign_nameplate_attack(&mut self, x: f64, y: f64) -> bool {
        if self.ui.tutorial_camera_only
            || self.ui.observing
            || self.ui.app.phase != crate::ClientPhase::Playing
            || self.ui.app.hud_state.building_placement.is_some()
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
        // Buildings under construction open the same panel with the seconds left.
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
                    && has_boat_capacity
                    && self
                        .fleet_route_check(tile_idx, target.owner, FleetRouteCheck::Access)
                        .is_ok()));
        let mut actions = target.menu_actions(
            spawning,
            target.is_land && self.can_attack(tile_idx, target.owner),
            can_fleet,
        );
        if campaign_fleet {
            actions.retain(|action| match action {
                MapMenuAction::Fleet => self
                    .campaign_action_unlocked(sow_core::campaign::CampaignAction::TransportFleet),
                MapMenuAction::Nuke => {
                    self.campaign_action_unlocked(sow_core::campaign::CampaignAction::Nuke)
                }
                _ => true,
            });
        }
        if target.owner != target.my_id || !target.is_land || spawning {
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

        let building = self
            .sim
            .current_snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.buildings.iter().find(|b| b.tile_idx == tile_idx));
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
                        if kind.max_level() > 0
                            && self.campaign_building_max_level(kind) >= 1
                            && self.resolve_building_target(kind, col, row).is_ok()
                        {
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
                let mut reason_key = None;
                let mut reason_level = None;
                let mut reason_seconds = None;
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
                } else if campaign_fleet {
                    let blocker = self.map_target(tile_idx).and_then(|target| {
                        if target.is_teammate {
                            Some(("hud.fleet_teammate", None))
                        } else if target.is_allied {
                            Some(("hud.fleet_alliance", None))
                        } else if self
                            .sim
                            .current_snapshot
                            .as_ref()
                            .and_then(|snapshot| {
                                snapshot
                                    .players
                                    .iter()
                                    .find(|player| player.id == target.my_id)
                            })
                            .is_some_and(|player| player.boats_in_use >= player.boat_capacity)
                        {
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
                    let required_level = 1;
                    let ready_city = self.sim.current_snapshot.as_ref().is_some_and(|snapshot| {
                        snapshot.buildings.iter().any(|building| {
                            building.owner_id == owner_id
                                && building.kind == sow_core::game::BuildingKind::City
                                && !building.under_construction
                                && building.active_level() >= required_level
                        })
                    });
                    if !ready_city {
                        reason_key = Some("hud.map_action_city");
                        reason_level = Some(required_level);
                        true
                    } else {
                        let cooldown_ticks = self
                            .sim
                            .current_snapshot
                            .as_ref()
                            .and_then(|snapshot| {
                                snapshot.players.iter().find(|player| player.id == owner_id)
                            })
                            .map_or(0, |player| player.nuke_cooldown_ticks);
                        if cooldown_ticks > 0 {
                            reason_key = Some("lobbies.locked");
                            reason_seconds = Some(
                                ((cooldown_ticks as f32 * self.sim.config.tick_rate_ms / 1000.0)
                                    .ceil() as u32)
                                    .max(1),
                            );
                            true
                        } else {
                            false
                        }
                    }
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
        // The panel stays open after an upgrade and shows the seconds left.
        let keep_building_panel_open = action == MapMenuAction::UpgradeStructure
            && self
                .input
                .map_context_menu
                .is_some_and(|menu| menu.view == MapContextMenuView::BuildingDetails);
        if !self.map_menu_actions(tile_idx).contains(&action) {
            self.show_map_menu_unavailable(tile_idx);
            if !keep_building_panel_open {
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
            if !keep_building_panel_open {
                self.close_map_context_menu();
            }
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
                if !self
                    .ui
                    .app
                    .hud_state
                    .building_placement
                    .is_some_and(|p| p.pending)
                {
                    self.ui.app.hud_state.building_placement =
                        Some(crate::ui::hud::state::BuildingPlacement {
                            kind,
                            tile_idx,
                            pending: false,
                        });
                    self.confirm_building_placement();
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
        }
        if !keep_building_panel_open {
            self.close_map_context_menu();
        }
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

    /// Drag and tap: the preview rests on the closest legal foundation, or stays put.
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
        let my_id = self.sim.my_player_id.unwrap_or(0);
        let Some(engine) = self.sim.engine.as_mut() else {
            return;
        };
        engine.refresh_building_grid();
        if let Some(tile) = sow_core::building::nearest_structure_site(
            &engine.state.map,
            my_id,
            placement.kind,
            col,
            row,
            PLACEMENT_SNAP_RADIUS,
            &engine.building_grid,
        ) && let Some(placement) = self.ui.app.hud_state.building_placement.as_mut()
        {
            placement.tile_idx = tile;
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
        self.input.building_drag_offset = None;
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
        }
    }

    #[cfg(test)]
    pub(crate) fn building_placement_site_valid(&mut self) -> bool {
        let Some(placement) = self.ui.app.hud_state.building_placement else {
            return false;
        };
        if self.sim.map_w == 0 {
            return false;
        }
        self.resolve_building_target(
            placement.kind,
            (placement.tile_idx % self.sim.map_w) as i32,
            (placement.tile_idx / self.sim.map_w) as i32,
        )
        .is_ok()
    }

    pub(crate) fn confirm_building_placement(&mut self) {
        let Some(placement) = self.ui.app.hud_state.building_placement else {
            return;
        };
        if !self.building_placement_can_confirm() {
            return;
        }
        self.input.building_drag_offset = None;
        self.input.camera_focus_target = None;
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
        self.ui
            .app
            .hud_state
            .building_placement
            .is_some_and(|p| !p.pending)
            && !self.ui.observing
            && !self.ui.tutorial_camera_only
            && self.ui.app.phase == crate::ClientPhase::Playing
            && self.ui.app.hud_state.sync_state.is_none()
            && self.building_placement_reason().is_none()
    }

    pub(crate) fn building_placement_reason(&mut self) -> Option<crate::ui::UiText> {
        let placement = self.ui.app.hud_state.building_placement?;
        if let Err(key) = self.resolve_building_target(
            placement.kind,
            (placement.tile_idx % self.sim.map_w) as i32,
            (placement.tile_idx / self.sim.map_w) as i32,
        ) {
            return Some(crate::ui::UiText::new(key));
        }
        if self.campaign_building_max_level(placement.kind) < 1 {
            return Some(crate::ui::UiText::new("hud.campaign_unlock_required"));
        }
        self.building_gold_shortfall(placement.kind)
    }

    /// One gold rule for the dock, hotkeys and the confirm button.
    fn building_gold_shortfall(
        &self,
        kind: sow_core::game::BuildingKind,
    ) -> Option<crate::ui::UiText> {
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
            MapMenuAction::Nuke => (Some(self.sim.config.nuke_cost), None),
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
                    && building.active_level() >= 1
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
        if !self.campaign_action_unlocked(sow_core::campaign::CampaignAction::TransportFleet) {
            self.add_map_feedback(crate::ui::UiText::new("hud.campaign_unlock_required"));
            return false;
        }
        let Some(target) = self.map_target(tile_idx) else {
            return false;
        };
        if target.is_teammate {
            self.add_map_feedback(crate::ui::UiText::new("hud.fleet_teammate"));
            return false;
        }
        if target.is_allied {
            self.add_map_feedback(crate::ui::UiText::new("hud.fleet_alliance"));
            return false;
        }
        if target.owner != 0 && !target.is_enemy() {
            self.add_map_feedback(crate::ui::UiText::new("hud.fleet_own_target"));
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
            self.add_map_feedback(crate::ui::UiText::new("hud.resources_allies_only"));
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
        if self
            .ui
            .app
            .hud_state
            .building_placement
            .is_some_and(|p| p.pending)
        {
            return;
        }
        self.ui.app.hud_state.building_placement = None;
        self.ui.app.hud_state.selected_warship_build = false;
        self.ui.app.hud_state.selected_nuke_kind = None;
        self.input.building_drag_offset = None;
    }

    pub(crate) fn select_building_kind(&mut self, kind: sow_core::game::BuildingKind) {
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
        let zoom = self.input.camera_zoom.max(0.01);
        let center_x = ((self.input.screen_w * 0.5 - self.input.camera_x) / zoom) as i32;
        let center_y = ((self.input.screen_h * 0.45 - self.input.camera_y) / zoom) as i32;
        let radius = (self.input.screen_w.max(self.input.screen_h) * 0.5 / zoom).ceil() as i32;
        let my_id = self.sim.my_player_id.unwrap_or(0);
        let tutorial_site = if self.ui.tutorial_active
            && self.ui.tutorial_campaign == crate::campaign::CampaignId::Boudica
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
        let site = tutorial_site.or_else(|| {
            self.sim.engine.as_mut().and_then(|engine| {
                engine.refresh_building_grid();
                sow_core::building::nearest_structure_site(
                    &engine.state.map,
                    my_id,
                    kind,
                    center_x,
                    center_y,
                    radius,
                    &engine.building_grid,
                )
            })
        });
        let Some(tile) = site else {
            self.add_map_feedback(crate::ui::UiText::new("hud.build_no_space"));
            return;
        };
        self.input.key_pan_up = false;
        self.input.key_pan_down = false;
        self.input.key_pan_left = false;
        self.input.key_pan_right = false;
        self.ui.app.hud_state.building_placement = Some(crate::ui::hud::state::BuildingPlacement {
            kind,
            tile_idx: tile,
            pending: false,
        });
        self.ui.app.hud_state.selected_warship_build = false;
        self.ui.app.hud_state.selected_nuke_kind = None;
        self.input.building_drag_offset = None;
        self.close_map_context_menu();
        let sf = crate::web_canvas::device_pixel_ratio() as f32;
        self.input.target_zoom =
            self.input
                .target_zoom
                .max(12.0 * sf)
                .min(crate::camera_zoom_upper_bound(
                    self.input.screen_w,
                    self.input.screen_h,
                ));
        self.input.camera_focus_target = Some((
            (tile % self.sim.map_w) as f32,
            (tile / self.sim.map_w) as f32,
        ));
        self.input.camera_focus_waiting_for_input_release = true;
    }

    pub(crate) fn select_warship_build_mode(&mut self) {
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
        if !self.campaign_action_unlocked(sow_core::campaign::CampaignAction::Warship) {
            self.add_map_feedback(crate::ui::UiText::new("hud.campaign_unlock_required"));
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
                    && building.active_level() >= 1
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
        self.ui.app.hud_state.building_placement = None;
        self.ui.app.hud_state.selected_nuke_kind = None;
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
                        && building.active_level() >= 1
                })
        });
        if !available {
            self.add_action_feedback("Nuke unavailable here.".to_string());
            return;
        }
        let selected = &mut self.ui.app.hud_state.selected_nuke_kind;
        *selected = (*selected != Some(kind)).then_some(kind);
        self.ui.app.hud_state.building_placement = None;
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
    use wasm_bindgen_test::wasm_bindgen_test;

    #[wasm_bindgen_test]
    fn tap_and_hold_are_distinct_and_drag_cancels_both() {
        assert!(is_quick_tap(TOUCH_HOLD_MS - 1, 0.0));
        assert!(!is_quick_tap(TOUCH_HOLD_MS, 0.0));
        assert!(!is_quick_tap(1, 401.0));
    }

    #[wasm_bindgen_test]
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

    #[wasm_bindgen_test]
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

    #[wasm_bindgen_test]
    fn attack_amount_must_meet_the_configured_minimum() {
        assert!(!attack_troops_meet_minimum(0.5, 1.0));
        assert!(attack_troops_meet_minimum(1.0, 1.0));
        assert!(!attack_troops_meet_minimum(f64::NAN, 1.0));
    }

    #[wasm_bindgen_test]
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
            vec![
                MapMenuAction::Transfer,
                MapMenuAction::Fleet,
                MapMenuAction::Alliance
            ]
        );
        assert_eq!(
            ally.menu_actions(false, true, false),
            vec![
                MapMenuAction::Transfer,
                MapMenuAction::Attack,
                MapMenuAction::Fleet,
                MapMenuAction::Alliance,
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
            outgoing_request
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
                MapMenuAction::BuildFarm,
            ]
        );

        assert_eq!(
            enemy.menu_actions(true, false, false),
            vec![MapMenuAction::Spawn]
        );
    }

    #[wasm_bindgen_test]
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
