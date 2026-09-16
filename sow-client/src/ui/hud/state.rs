use sow_core::protocol::{AttackSnapshot, FleetSnapshot, PlayerSnapshot};
use web_time::Instant;
const EVENT_LOG_MAX_ENTRIES: usize = 50;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BottomHudTab { #[default] Controls, BattleLog, EventLog }
#[derive(Clone, Debug)]
pub struct EventLogEntry { pub message: String, pub color: [f32; 4], pub spawned_at: Instant }
#[derive(Clone, Debug)]
pub struct SelectedTileInfo { pub tile_idx: u32, pub owner_id: u16, pub is_own_territory: bool, pub is_friendly: bool, pub is_spawning: bool, pub is_land: bool }
#[derive(Clone, Debug)]
pub struct HudNotification { pub message: String, pub color: [f32; 4], pub spawned_at: Instant }

pub struct HudState {
    pub gold: f64, pub troops: f64, pub max_troops: f64, pub troop_rate: f64, pub attack_ratio: f32,
    pub spawn_timer_secs: Option<f32>, pub sync_state: Option<sow_core::protocol::ServerSyncStateMessage>,
    pub my_player_id: u16, pub map_w: u32, pub attacks: Vec<AttackSnapshot>, pub fleets: Vec<FleetSnapshot>,
    pub players: Vec<PlayerSnapshot>, pub safe_area_top: f32, pub safe_area_bottom: f32,
    pub selected_tile: Option<SelectedTileInfo>, pub show_emoji_panel: bool, pub emoji_panel_pos: Option<[f32; 2]>,
    pub emoji_panel_just_opened: bool, pub pin_emoji: bool, pub show_alliance_inbox: bool,
    pub show_betrayal_warning: Option<(u16, sow_core::protocol::GameplayIntent)>,
    pub betrayal_warning_cached: Option<(u16, sow_core::protocol::GameplayIntent)>,
    pub selected_building_kind: Option<sow_core::game::BuildingKind>, pub building_costs: [f64; 9],
    pub selected_nuke_kind: Option<sow_core::game::NukeKind>, pub event_log: Vec<EventLogEntry>,
    pub hud_notifications: Vec<HudNotification>, pub bottom_tab: BottomHudTab,
    pub battle_log_seen_count: usize, pub event_log_seen_count: usize,
    pub show_ask_panel: Option<u16>, pub ask_gold: f64, pub ask_troops: f64, pub prev_resource_requests: Vec<u16>,
    pub transfer_confirm_pending: bool, pub chat_disabled: bool,
}

impl Default for HudState {
    fn default() -> Self { Self { gold: 0.0, troops: 0.0, max_troops: 0.0, troop_rate: 0.0, attack_ratio: 0.25,
        spawn_timer_secs: None, sync_state: None, my_player_id: 0, map_w: 0, attacks: Vec::new(), fleets: Vec::new(), players: Vec::new(),
        safe_area_top: 0.0, safe_area_bottom: 0.0, selected_tile: None, show_emoji_panel: false, emoji_panel_pos: None,
        emoji_panel_just_opened: false, pin_emoji: false, show_alliance_inbox: false,
        show_betrayal_warning: None, betrayal_warning_cached: None, selected_building_kind: None, building_costs: [0.0; 9], selected_nuke_kind: None,
        event_log: Vec::new(), hud_notifications: Vec::new(), bottom_tab: BottomHudTab::Controls, battle_log_seen_count: 0, event_log_seen_count: 0,
        show_ask_panel: None, ask_gold: 0.0, ask_troops: 0.0, prev_resource_requests: Vec::new(),
        transfer_confirm_pending: false, chat_disabled: false } }
}

impl HudState {
    pub fn push_notification(&mut self, message: String, color: [f32; 4]) { let spawned_at = Instant::now();
        self.hud_notifications.push(HudNotification { message: message.clone(), color, spawned_at });
        self.event_log.push(EventLogEntry { message, color, spawned_at });
        if self.event_log.len() > EVENT_LOG_MAX_ENTRIES { self.event_log.remove(0); } }
}
