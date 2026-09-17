use super::placement::{resolve_build_target_tile, PlacementQuery};
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
        }
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

    fn menu_actions(self, spawning: bool, can_attack: bool) -> Vec<MapMenuAction> {
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
            return (self.is_land && can_attack)
                .then_some(MapMenuAction::Attack)
                .into_iter()
                .collect();
        }
        if !self.is_player() {
            return Vec::new();
        }

        let mut actions = Vec::new();
        if self.is_friendly() {
            actions.push(MapMenuAction::Transfer);
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
        self.add_notice_at_screen(
            message,
            x,
            y,
            1500,
            crate::rgb(203, 213, 225),
        );
    }

    pub(crate) fn try_attack_at(&mut self, x: f64, y: f64) -> bool {
        if self.ui.observing
            || self.ui.app.phase != crate::ClientPhase::Playing
            || self.ui.app.hud_state.selected_building_kind.is_some()
            || self.ui.app.hud_state.selected_nuke_kind.is_some()
        {
            return false;
        }
        if !self.sim.current_snapshot.as_ref().is_some_and(|snapshot| {
            matches!(snapshot.phase, sow_core::game::GamePhase::Playing)
        }) {
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

        if self.sim.current_snapshot.as_ref().is_some_and(|snapshot| {
            matches!(snapshot.phase, sow_core::game::GamePhase::Playing)
        }) {
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

    pub(crate) fn map_menu_actions(&self, tile_idx: u32) -> Vec<MapMenuAction> {
        let Some(target) = self.map_target(tile_idx) else {
            return Vec::new();
        };
        let spawning = self.sim.current_snapshot.as_ref().is_some_and(|snapshot| {
            matches!(snapshot.phase, sow_core::game::GamePhase::Spawning { .. })
        });
        target.menu_actions(
            spawning,
            target.is_land && self.can_attack(tile_idx, target.owner),
        )
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
            | MapMenuAction::BuildBunker => {
                let kind = match action {
                    MapMenuAction::BuildCity => sow_core::game::BuildingKind::City,
                    MapMenuAction::BuildFactory => sow_core::game::BuildingKind::Factory,
                    MapMenuAction::BuildPort => sow_core::game::BuildingKind::Port,
                    MapMenuAction::BuildBunker => sow_core::game::BuildingKind::Bunker,
                    _ => unreachable!(),
                };
                if let Some((col, row)) = self.tile_coords(tile_idx) {
                    self.build_structure_at(kind, col, row, anchor);
                }
            }
            MapMenuAction::Nuke => {
                self.launch_nuke_at(sow_core::game::NukeKind::AtomBomb, tile_idx);
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
            self.add_notice_at_screen(
                text,
                anchor.0,
                anchor.1,
                2000,
                crate::rgb(248, 113, 113),
            );
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
        self.send_intent(sow_core::protocol::GameplayIntent::BuildStructure {
            kind,
            target_tile,
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
        if !target.is_attackable() || !self.can_attack(tile_idx, target.owner) {
            const MESSAGES: [&str; 5] = [
                "Too far! 🌌",
                "Out of reach! 🏃‍♂️",
                "No border, no battle! ⚔️",
                "Teleportation not researched! 📡",
                "Build a path first! 🗺️",
            ];
            let message = MESSAGES[(anchor.0 + anchor.1) as usize % MESSAGES.len()];
            self.add_notice_at_screen(
                message,
                anchor.0,
                anchor.1,
                1500,
                crate::rgb(248, 113, 113),
            );
            return false;
        }
        let troops = self.ui.app.hud_state.troops * self.ui.app.hud_state.attack_ratio as f64;
        if troops <= 0.0 {
            return false;
        }
        let text = format!("⚔️ +{}", crate::utils::format_number(troops));
        self.add_notice_at_screen(
            text,
            anchor.0,
            anchor.1,
            1500,
            crate::rgb(6, 182, 212),
        );
        self.send_intent(sow_core::protocol::GameplayIntent::Attack(
            sow_core::protocol::AttackIntent {
                target_owner: target.owner,
                troops: Some(troops),
            },
        ));
        true
    }

    pub(crate) fn launch_fleet_from_tile(&mut self, tile_idx: u32) -> bool {
        let Some(target) = self.map_target(tile_idx) else {
            return false;
        };
        if !target.is_enemy() {
            return false;
        }
        let troops = self.ui.app.hud_state.troops * self.ui.app.hud_state.attack_ratio as f64;
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
            return false;
        }
        self.ui.app.hud_state.show_ask_panel = Some(target.owner);
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
        let intent = if target.is_allied {
            sow_core::protocol::GameplayIntent::BreakAlliance {
                target_player: target.owner,
            }
        } else if target.has_alliance_request {
            sow_core::protocol::GameplayIntent::AcceptAlliance {
                target_player: target.owner,
            }
        } else {
            sow_core::protocol::GameplayIntent::ProposeAlliance {
                target_player: target.owner,
            }
        };
        self.send_intent(intent);
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
        let is_betrayer = other.is_some_and(|player| {
            player.active_emoji.as_deref() == Some("🗡️")
        });
        let is_teammate = me
            .zip(other)
            .is_some_and(|(me, other)| me.team.is_some() && me.team == other.team);
        let is_allied = me.is_some_and(|player| {
            player.alliances.contains(&owner) && !is_betrayer
        });
        let has_alliance_request = me.is_some_and(|player| {
            player.alliance_requests.contains(&owner)
        });
        Some(MapTarget {
            owner,
            is_land,
            my_id,
            is_allied,
            is_teammate,
            has_alliance_request,
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
        Some(((tile_idx % self.sim.map_w) as i32, (tile_idx / self.sim.map_w) as i32))
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
                    && terrain
                        .get(neighbor)
                        .is_some_and(|value| value & 0x80 != 0)
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
    use super::{is_quick_tap, shares_land_border, MapMenuAction, MapTarget, TOUCH_HOLD_MS};

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
        };
        assert!(neutral.is_attackable());
        assert_eq!(neutral.menu_actions(false, true), vec![MapMenuAction::Attack]);
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
        };
        assert_eq!(
            enemy.menu_actions(false, true),
            vec![
                MapMenuAction::Attack,
                MapMenuAction::Fleet,
                MapMenuAction::Nuke,
                MapMenuAction::Alliance,
            ]
        );
        assert_eq!(
            enemy.menu_actions(false, false),
            vec![MapMenuAction::Fleet, MapMenuAction::Nuke, MapMenuAction::Alliance]
        );

        let ally = MapTarget {
            is_allied: true,
            ..enemy
        };
        assert_eq!(ally.menu_actions(false, true), vec![MapMenuAction::Transfer]);

        let own_land = MapTarget {
            owner: 1,
            ..enemy
        };
        assert_eq!(
            own_land.menu_actions(false, false),
            vec![
                MapMenuAction::BuildCity,
                MapMenuAction::BuildFactory,
                MapMenuAction::BuildPort,
                MapMenuAction::BuildBunker,
            ]
        );

        assert_eq!(enemy.menu_actions(true, false), vec![MapMenuAction::Spawn]);
    }
}
