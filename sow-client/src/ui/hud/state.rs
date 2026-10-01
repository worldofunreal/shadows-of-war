use crate::ui::UiText;
use sow_core::protocol::{AttackSnapshot, FleetSnapshot};
use std::collections::VecDeque;
use web_time::Instant;

#[derive(Clone, Debug)]
pub struct SelectedTileInfo {
    pub tile_idx: u32,
    pub owner_id: u16,
    pub is_own_territory: bool,
    pub is_friendly: bool,
    pub is_spawning: bool,
    pub is_land: bool,
}
#[derive(Clone, Debug)]
pub struct HudNotification {
    pub id: u64,
    pub text: UiText,
    pub spawned_at: Instant,
    pub players: [Option<u16>; 2],
    pub priority: u8,
    pub group: Option<String>,
    pub sum_values: bool,
}

#[derive(Clone, Debug)]
pub struct HudMapFeedback {
    pub id: u64,
    pub text: UiText,
    pub spawned_at: Instant,
    pub position: [f32; 2],
}

pub struct HudState {
    pub gold: f64,
    pub troops: f64,
    pub max_troops: f64,
    pub troop_rate: f64,
    pub attack_ratio: f32,
    pub spawn_timer_secs: Option<f32>,
    pub sync_state: Option<sow_core::protocol::ServerSyncStateMessage>,
    pub my_player_id: u16,
    pub map_w: u32,
    pub attacks: Vec<AttackSnapshot>,
    pub fleets: Vec<FleetSnapshot>,
    pub safe_area_top: f32,
    pub safe_area_bottom: f32,
    pub selected_tile: Option<SelectedTileInfo>,
    pub show_emoji_panel: bool,
    pub emoji_panel_pos: Option<[f32; 2]>,
    pub emoji_panel_just_opened: bool,
    pub pin_emoji: bool,
    pub show_alliance_inbox: bool,
    pub show_betrayal_warning: Option<(u16, sow_core::protocol::GameplayIntent)>,
    pub betrayal_warning_cached: Option<(u16, sow_core::protocol::GameplayIntent)>,
    pub selected_building_kind: Option<sow_core::game::BuildingKind>,
    pub building_costs: [f64; 9],
    pub selected_nuke_kind: Option<sow_core::game::NukeKind>,
    pub hud_notifications: VecDeque<HudNotification>,
    pub map_feedback: Option<HudMapFeedback>,
    pub notification_revision: u64,
    pub show_ask_panel: Option<u16>,
    pub ask_gold: f64,
    pub ask_troops: f64,
    pub prev_resource_requests: Vec<u16>,
    pub transfer_confirm_pending: bool,
}

impl Default for HudState {
    fn default() -> Self {
        Self {
            gold: 0.0,
            troops: 0.0,
            max_troops: 0.0,
            troop_rate: 0.0,
            attack_ratio: 0.5,
            spawn_timer_secs: None,
            sync_state: None,
            my_player_id: 0,
            map_w: 0,
            attacks: Vec::new(),
            fleets: Vec::new(),
            safe_area_top: 0.0,
            safe_area_bottom: 0.0,
            selected_tile: None,
            show_emoji_panel: false,
            emoji_panel_pos: None,
            emoji_panel_just_opened: false,
            pin_emoji: false,
            show_alliance_inbox: false,
            show_betrayal_warning: None,
            betrayal_warning_cached: None,
            selected_building_kind: None,
            building_costs: [0.0; 9],
            selected_nuke_kind: None,
            hud_notifications: VecDeque::with_capacity(32),
            map_feedback: None,
            notification_revision: 0,
            show_ask_panel: None,
            ask_gold: 0.0,
            ask_troops: 0.0,
            prev_resource_requests: Vec::new(),
            transfer_confirm_pending: false,
        }
    }
}

impl HudState {
    pub fn push_map_feedback(&mut self, text: UiText, position: [f32; 2]) {
        let id = self.notification_revision.wrapping_add(1);
        self.map_feedback = Some(HudMapFeedback {
            id,
            text,
            spawned_at: Instant::now(),
            position: position.map(|value| if value.is_finite() { value } else { 0.0 }),
        });
        self.notification_revision = id;
    }

    pub fn push_notification(&mut self, text: UiText) {
        self.push_notification_for_players(text, [None, None], 1, None, false);
    }

    pub fn push_notification_for_players(
        &mut self,
        text: UiText,
        players: [Option<u16>; 2],
        priority: u8,
        group: Option<String>,
        sum_values: bool,
    ) {
        if self.hud_notifications.len() >= 32 {
            self.hud_notifications.pop_front();
        }
        let id = self.notification_revision.wrapping_add(1);
        let group = group.or_else(|| {
            players[0]
                .or(players[1])
                .map(|_| format!("{}:{:?}:{:?}", text.key, players[0], players[1]))
        });
        self.hud_notifications.push_back(HudNotification {
            id,
            text,
            spawned_at: Instant::now(),
            players,
            priority,
            group,
            sum_values,
        });
        self.notification_revision = id;
    }

    pub fn clear_notifications(&mut self) {
        self.hud_notifications.clear();
        self.map_feedback = None;
        self.notification_revision = self.notification_revision.wrapping_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notifications_keep_only_the_32_most_recent_entries() {
        let mut hud = HudState::default();
        for _ in 0..40 {
            hud.push_notification(UiText::new("hud.event"));
        }
        assert_eq!(hud.hud_notifications.len(), 32);
        assert_eq!(hud.notification_revision, 40);
    }

    #[test]
    fn notification_retains_both_entity_ids_and_priority() {
        let mut hud = HudState::default();
        hud.push_notification_for_players(
            UiText::new("hud.resource_received_both"),
            [Some(17), Some(23)],
            4,
            Some("nuke:17:23".into()),
            false,
        );
        let receipt = hud.hud_notifications.front().unwrap();
        assert_eq!(receipt.id, 1);
        assert_eq!(receipt.players, [Some(17), Some(23)]);
        assert_eq!(receipt.priority, 4);
        assert_eq!(receipt.group.as_deref(), Some("nuke:17:23"));
    }

    #[test]
    fn clearing_notifications_advances_the_revision() {
        let mut hud = HudState::default();
        hud.push_notification(UiText::new("hud.event"));
        hud.push_map_feedback(UiText::new("hud.build_land"), [10.0, 20.0]);
        let revision = hud.notification_revision;
        hud.clear_notifications();
        assert!(hud.hud_notifications.is_empty());
        assert!(hud.map_feedback.is_none());
        assert_eq!(hud.notification_revision, revision + 1);
    }

    #[test]
    fn map_feedback_uses_one_slot_and_retriggers_identical_messages() {
        let mut hud = HudState::default();
        hud.push_map_feedback(
            UiText::new("hud.need_gold").with("cost", "100"),
            [10.0, 20.0],
        );
        let first_id = hud.map_feedback.as_ref().unwrap().id;
        hud.push_map_feedback(
            UiText::new("hud.need_gold").with("cost", "100"),
            [30.0, 40.0],
        );
        let repeated = hud.map_feedback.as_ref().unwrap();
        assert_eq!(repeated.id, first_id + 1);
        assert_eq!(repeated.position, [30.0, 40.0]);

        hud.push_map_feedback(UiText::new("hud.build_land"), [50.0, 60.0]);
        let replaced = hud.map_feedback.as_ref().unwrap();
        assert_eq!(replaced.id, first_id + 2);
        assert_eq!(replaced.text.key, "hud.build_land");
        assert_eq!(replaced.position, [50.0, 60.0]);
    }
}
