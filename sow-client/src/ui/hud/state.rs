use crate::ui::UiText;
use std::collections::{HashMap, VecDeque};
use web_time::{Duration, Instant};

const ABILITY_NOTICE_INTERVAL: Duration = Duration::from_secs(8);

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
    pub fleet_id: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct HudMapFeedback {
    pub id: u64,
    pub text: UiText,
    pub spawned_at: Instant,
    pub position: [f32; 2],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuildingPlacement {
    pub kind: sow_core::game::BuildingKind,
    pub tile_idx: u32,
    pub pending: bool,
}

pub(crate) const DEFAULT_ATTACK_RATIO: f32 = 0.25;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransferMode {
    Send,
    Request,
}

impl TransferMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Send => "send",
            Self::Request => "request",
        }
    }
}

pub struct HudState {
    pub gold: f64,
    pub troops: f64,
    pub max_troops: f64,
    pub troop_rate: f64,
    pub gold_rate: f64,
    pub hero_income_rate: f64,
    pub attack_ratio: f32,
    pub spawn_timer_secs: Option<f32>,
    pub awaiting_players: bool,
    pub sync_state: Option<sow_core::protocol::ServerSyncStateMessage>,
    pub my_player_id: u16,
    pub map_w: u32,
    pub safe_area_top: f32,
    pub safe_area_bottom: f32,
    pub selected_tile: Option<SelectedTileInfo>,
    pub show_alliance_inbox: bool,
    pub show_betrayal_warning: Option<(u16, sow_core::protocol::GameplayIntent)>,
    pub betrayal_warning_cached: Option<(u16, sow_core::protocol::GameplayIntent)>,
    pub building_placement: Option<BuildingPlacement>,
    pub building_costs: [f64; 9],
    pub building_foundation_levels: [u8; 7],
    pub selected_nuke_kind: Option<sow_core::game::NukeKind>,
    pub selected_warship_ids: Vec<u64>,
    pub warship_move_targeting: bool,
    pub hud_notifications: VecDeque<HudNotification>,
    last_ability_notices: HashMap<u16, Instant>,
    pub map_feedback: Option<HudMapFeedback>,
    pub notification_revision: u64,
    pub show_ask_panel: Option<u16>,
    /// The radial's Send or Request. `None` lets the panel lead with the stronger side.
    pub transfer_mode: Option<TransferMode>,
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
            gold_rate: 0.0,
            hero_income_rate: 0.0,
            attack_ratio: DEFAULT_ATTACK_RATIO,
            spawn_timer_secs: None,
            awaiting_players: false,
            sync_state: None,
            my_player_id: 0,
            map_w: 0,
            safe_area_top: 0.0,
            safe_area_bottom: 0.0,
            selected_tile: None,
            show_alliance_inbox: false,
            show_betrayal_warning: None,
            betrayal_warning_cached: None,
            building_placement: None,
            building_costs: [0.0; 9],
            building_foundation_levels: [1; 7],
            selected_nuke_kind: None,
            selected_warship_ids: Vec::new(),
            warship_move_targeting: false,
            hud_notifications: VecDeque::with_capacity(32),
            last_ability_notices: HashMap::new(),
            map_feedback: None,
            notification_revision: 0,
            show_ask_panel: None,
            transfer_mode: None,
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
            fleet_id: None,
        });
        self.notification_revision = id;
    }

    pub(crate) fn push_ability_notice(
        &mut self,
        owner_id: u16,
        viewer_id: u16,
        leader: sow_core::player::Leader,
        now: Instant,
    ) {
        if self
            .last_ability_notices
            .get(&owner_id)
            .is_some_and(|last| now.duration_since(*last) < ABILITY_NOTICE_INTERVAL)
        {
            return;
        }
        self.last_ability_notices.insert(owner_id, now);

        let hero = sow_core::commerce::leader_id(leader);
        self.push_notification_for_players(
            UiText::new("hud.ability_notice")
                .with("hero", hero)
                .with("hero_art", leader.art_slug())
                .with("ability_key", format!("site.leader_{hero}_ability"))
                .with("description_key", format!("site.leader_{hero}_description")),
            [Some(owner_id), (viewer_id != owner_id).then_some(viewer_id)],
            3,
            Some(format!("ability:{owner_id}")),
            false,
        );
    }

    pub fn push_warship_launch(&mut self, fleet_id: u64) {
        self.push_notification_for_players(
            UiText::new("hud.warship_ready"),
            [None, None],
            3,
            None,
            false,
        );
        if let Some(notice) = self.hud_notifications.back_mut() {
            notice.fleet_id = Some(fleet_id);
        }
    }

    pub fn clear_notifications(&mut self) {
        self.hud_notifications.clear();
        self.last_ability_notices.clear();
        self.map_feedback = None;
        self.notification_revision = self.notification_revision.wrapping_add(1);
    }
}

