//! Web menu bridge.
//!
//! The browser/WebView owns presentation and input for the main menu. Rust remains the
//! source of truth for connection state, lobby state, identity, and match transitions.
//! Commands cross this boundary as small JSON messages; the existing UiAction and network
//! paths execute them unchanged.
//! Return/reward/guide ownership and ordering: docs/player-journeys.md.

use std::cell::{Cell, RefCell};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet, VecDeque};

use serde::Deserialize;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_time::{Duration, Instant};

use crate::UiAction;
use crate::app::{HoverPointer, MapContextMenuView, SowApp};
use crate::campaign::CampaignId;

const LEADERBOARD_LIMIT: usize = 100;
const TUTORIAL_MAX_ZOOM: f32 = 10.0;
const TUTORIAL_ATTACK_NEIGHBORS: [(i32, i32); 8] = [
    (1, 0),
    (-1, 0),
    (0, -1),
    (0, 1),
    (1, -1),
    (-1, -1),
    (1, 1),
    (-1, 1),
];

#[derive(Debug, Deserialize)]
struct CampaignAssaultReinforcement {
    capacity_ratio: f64,
    interval_seconds: u32,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum WebMenuCommand {
    QuickMatch,
    JoinLobby {
        lobby_id: u64,
    },
    JoinWithPassword {
        lobby_id: u64,
        password: String,
    },
    JoinCode {
        code: String,
    },
    StartSinglePlayer {
        config: serde_json::Value,
    },
    StartCampaignEpisode {
        episode_id: String,
        roster: serde_json::Value,
        #[serde(rename = "match")]
        match_config: serde_json::Value,
    },
    ActivateCampaignAssault {
        attacker_team: sow_core::protocol::Team,
        target_faction_id: String,
        #[serde(default)]
        preserve_relation: bool,
        #[serde(default)]
        reinforcement: Option<CampaignAssaultReinforcement>,
        #[serde(default)]
        hold_last_tile: bool,
    },
    AcceptCampaignAlliance {
        faction_id: String,
        campaign_session_id: u32,
    },
    SetTutorialPaused {
        paused: bool,
        #[serde(default)]
        camera_only: bool,
        #[serde(default)]
        paused_action: Option<crate::app::TutorialPausedAction>,
    },
    SetCampaignUnlocks {
        campaign_session_id: u32,
        unlocks: sow_core::campaign::CampaignUnlocks,
    },
    SetCampaignAttackPermission {
        faction_id: String,
        can_attack_player: bool,
    },
    HaltCampaignFaction {
        faction_id: String,
    },
    SetTutorialMarker {
        #[serde(default)]
        player_id: Option<u16>,
    },
    SetDialogBorderHighlight {
        #[serde(default)]
        player_id: Option<u16>,
    },
    ResolveCampaignDiplomacy {
        target_player_id: u16,
        relation: sow_core::protocol::CampaignRelation,
        gold_cost: f64,
    },
    MapMenuAction {
        session: u64,
        tile_idx: u32,
        action: crate::input::map_click::MapMenuAction,
    },
    CloseMapContextMenu,
    CompleteCampaignEpisode {
        episode_id: String,
    },
    CreateGame {
        config: serde_json::Value,
        #[serde(default)]
        is_private: bool,
        #[serde(default)]
        password: Option<String>,
    },
    SetLeader {
        leader_id: String,
    },
    SetRandomLeader,
    UnlockLeader {
        leader_id: String,
        currency: String,
    },
    UnlockSkin {
        skin_id: String,
    },
    EquipSkin {
        skin_id: String,
    },
    SaveDisplayName {
        name: String,
    },
    AcknowledgeRewardReceipts {
        account_id: String,
        receipt_ids: Vec<String>,
    },
    SaveMenuGuideProgress {
        guide_id: String,
        completed_steps: Vec<String>,
        #[serde(default)]
        current_step: Option<String>,
        status: sow_data::profile::MenuGuideStatus,
        #[serde(default)]
        selected_leader: Option<String>,
    },
    OpenBrowser,
    OpenCreate,
    CloseOverlay,
    LeaveLobby,
    RetryMapDownload,
    StartPrivate {
        lobby_id: u64,
    },
    KickPlayer {
        lobby_id: u64,
        target_player_id: u16,
    },
    BanPlayer {
        lobby_id: u64,
        target_player_id: u16,
    },
    MovePlayerTeam {
        lobby_id: u64,
        target_player_id: u16,
    },
    RefreshProfile,
    SignIn,
    SignOut,
    SetMute {
        value: bool,
    },
    SetMusicVolume {
        value: f32,
    },
    SetReducedMotion {
        value: bool,
    },
    SetFreeZoomOut {
        value: bool,
    },
    SetAttackRatio {
        ratio: f32,
    },
    SpawnTroops,
    SelectBuilding {
        kind: sow_core::game::BuildingKind,
    },
    SelectWarship,
    SelectNuke,
    ToggleInbox,
    AcceptAlliance {
        target_player_id: u16,
    },
    RejectAlliance {
        target_player_id: u16,
    },
    OpenTransfer {
        target_player_id: u16,
    },
    CloseTransfer,
    SendResources {
        target_player_id: u16,
        gold: f64,
        troops: f64,
    },
    RequestResources {
        target_player_id: u16,
        gold: f64,
        troops: f64,
    },
    AcceptResourceRequest {
        target_player_id: u16,
    },
    RejectResourceRequest {
        target_player_id: u16,
    },
    ConfirmBetrayal,
    CancelBetrayal,
    CancelAttack {
        attack_id: u64,
    },
    RecallFleet {
        fleet_id: u64,
    },
    ScuttleFleet {
        fleet_id: u64,
    },
    CounterAttack {
        target_player_id: u16,
    },
    ToggleLeaderboard,
    ToggleDevSidebar,
    SetDevConfig {
        field: String,
        value: serde_json::Value,
    },
    SetDevVfxPreview {
        #[serde(default)]
        target: Option<u8>,
        #[serde(default)]
        phase: Option<f32>,
        #[serde(default)]
        paused: Option<bool>,
        #[serde(default)]
        repeat: Option<bool>,
    },
    ReplayDevVfxPreview,
    ResetDevConfig {
        #[serde(default)]
        section: Option<String>,
        #[serde(default)]
        field: Option<String>,
    },
    DevVfxAll {
        on: bool,
    },
    DevVfxGroup {
        group: String,
        on: bool,
    },
    SetShowFpsPing {
        value: bool,
    },
    SetShowDevTools {
        value: bool,
    },
    ReturnToMenu {
        #[serde(default)]
        destination: Option<String>,
    },
    DismissRankedResult,
    RequestEndgameStats,
    ClearEndgameStats,
    ContinueObserving,
    ZoomIn,
    ZoomOut,
    CenterCamera,
    ExpressEmote {
        session: u64,
        tile_idx: u32,
        kind: sow_core::emote::EmoteKind,
    },
    TutorialEmote {
        kind: Option<sow_core::emote::EmoteKind>,
        #[serde(default)]
        pinned: bool,
        /// Campaign faction that emotes; `None` is the player.
        #[serde(default)]
        faction_id: Option<String>,
        #[serde(default)]
        token: Option<String>,
        #[serde(default)]
        campaign_session_id: Option<u32>,
        #[serde(default)]
        message: Option<String>,
    },
    SetCampaignPreDefeatTargets {
        faction_ids: Vec<String>,
        campaign_session_id: u32,
    },
    ContinueCampaignPreDefeat {
        faction_id: String,
        campaign_session_id: u32,
    },
    ClearEmote,
    SetEmotePinned {
        pinned: bool,
    },
    SetEmoteTexts {
        texts: [String; 4],
        #[serde(default)]
        safe_area: [f32; 4],
    },
    FocusPlayer {
        player_id: u16,
    },
    FocusWorld {
        x: f32,
        y: f32,
        #[serde(default)]
        shot: Option<CameraShotRequest>,
    },
}

/// Campaign narration shot that ends on the focused world point.
#[derive(Debug, Deserialize)]
struct CameraShotRequest {
    kind: CameraShotKind,
    from_x: f32,
    from_y: f32,
    seconds: f32,
    /// Code-directed move: the player cannot break it with drag, zoom or key pan.
    #[serde(default)]
    machinima: bool,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum CameraShotKind {
    /// Glide toward the speaker and the target together, easing back a little.
    Reveal,
    /// Glide onto the target, easing in a little.
    #[serde(alias = "threat")]
    Approach,
    /// Glide onto the target at the current zoom.
    Link,
}

#[cfg(any(feature = "dev", debug_assertions))]
fn decode_dev_config_value(field: &str, raw: &serde_json::Value) -> Option<[f32; 4]> {
    use sow_render::shader_params::{self, ParamKind};
    let def = shader_params::definition(field)?;
    let mut values = [0.0; 4];
    match def.kind {
        ParamKind::Bool => {
            values[0] = if raw.as_bool()? { 1.0 } else { 0.0 };
        }
        ParamKind::Number | ParamKind::Select => {
            let value = raw.as_f64()? as f32;
            if !value.is_finite() {
                return None;
            }
            values[0] = if def.kind == ParamKind::Select {
                def.options
                    .iter()
                    .min_by(|a, b| (a.value - value).abs().total_cmp(&(b.value - value).abs()))?
                    .value
            } else {
                value
            };
        }
        ParamKind::Color | ParamKind::Vector => {
            let input = raw.as_array()?;
            if input.len() < 3 {
                return None;
            }
            for i in 0..3 {
                let value = input[i].as_f64()? as f32;
                if !value.is_finite() {
                    return None;
                }
                values[i] = value;
            }
            values[3] = if def.kind == ParamKind::Color {
                input
                    .get(3)
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(1.0) as f32
            } else {
                0.0
            };
            if !values[3].is_finite() {
                return None;
            }
            if def.kind == ParamKind::Color {
                values[3] = values[3].clamp(0.0, 1.0);
            }
        }
    }
    Some(values)
}

thread_local! {
    static COMMANDS: RefCell<VecDeque<WebMenuCommand>> = RefCell::new(VecDeque::new());
    static EVENT_LOOP_WAKE: RefCell<Option<winit::event_loop::EventLoopProxy>> =
        const { RefCell::new(None) };
    /// Last payload handed to JS. publish_state runs every frame; without this
    /// guard each frame allocates a fresh JSON string plus a JS-side copy.
    static LAST_PUBLISHED: RefCell<String> = RefCell::new(String::new());
    /// Inputs behind the last menu payload. An idle menu (mouse moves, no state change)
    /// skips the payload rebuild while these still match.
    static LAST_MENU_INPUTS: RefCell<Option<MenuInputs>> = const { RefCell::new(None) };
    /// Cheap fingerprint for the small, hot in-match HUD payload. Heavy cold panels are
    /// represented by the snapshot tick only while a panel that needs them is open.
    static LAST_HUD_KEY: RefCell<Option<HudPublishKey>> = const { RefCell::new(None) };
    static LAST_TUTORIAL_CAMERA_ANCHOR: RefCell<Option<(u16, u32, u32)>> =
        const { RefCell::new(None) };
    /// Player-derived hot values are cached by snapshot tick so the player list is not scanned
    /// on every publish attempt.
    static LAST_MY_PLAYER: RefCell<Option<(u64, u16, Option<MyPlayerSummary>)>> =
        const { RefCell::new(None) };
    /// WASM nameplates need the same rank cache for the top-three laurels/medals.
    /// Refreshing is keyed by the authoritative snapshot tick, not the render/publish cadence.
    static LAST_WEB_TOP_THREE_TICK: Cell<Option<u64>> = const { Cell::new(None) };
    /// Cold panel JSON is rebuilt only when its snapshot input changes. Hot HUD updates reuse it.
    static LAST_WEB_LEADERBOARD_PAYLOAD:
        RefCell<Option<(u64, u16, serde_json::Value)>> = const { RefCell::new(None) };
    static LAST_WEB_HOVER_PAYLOAD:
        RefCell<Option<(u64, u32, u16, u16, serde_json::Value)>> = const { RefCell::new(None) };
    /// Last hover card JSON handed to JS. The hover card is published every frame, so it dedupes here.
    static LAST_HOVER_CARD: RefCell<String> = RefCell::new(String::new());
    /// Last tutorial pointer JSON handed to JS. The pointer follows the mouse, so it dedupes here.
    static LAST_TUTORIAL_POINTER: RefCell<String> = RefCell::new(String::new());
    static LAST_WEB_INBOX_PAYLOAD:
        RefCell<Option<(u64, u16, serde_json::Value)>> = const { RefCell::new(None) };

}

#[derive(Clone, Debug, PartialEq, Eq)]
struct HudPublishKey {
    fps: u32,
    ping_ms: Option<u32>,
    gold: u64,
    troops: u64,
    max_troops: u64,
    troop_rate: u64,
    gold_rate: u64,
    attack_ratio: u32,
    spawn_timer_tenths: i32,
    selected_building: Option<crate::ui::hud::state::BuildingPlacement>,
    selected_warship_build: bool,
    selected_nuke: u8,
    building_costs: [u64; 9],
    settings_mute: bool,
    settings_music_volume: u32,
    settings_reduced_motion: bool,
    settings_free_zoom_out: bool,
    settings_show_fps_ping: bool,
    settings_show_dev_tools: bool,
    leaderboard_open: bool,
    leaderboard_publish_revision: u64,
    tutorial_active: bool,
    campaign_session_id: u32,
    campaign_unlock_revision: u64,
    dev_sidebar_open: bool,
    dev_config: Vec<u32>,
    dev_preview: Option<(u8, u8, bool, bool, bool)>,
    inbox_open: bool,
    transfer_target: Option<u16>,
    betrayal_open: bool,
    sync_open: bool,
    inbox_count: usize,
    notification_revision: u64,
    is_spectating: bool,
    endgame_active: bool,
    endgame_winner: Option<u16>,
    endgame_team: Option<sow_core::protocol::Team>,
    player_endgame_stats: Option<[u64; 5]>,
    snapshot_tick: u64,
    combat_operations_tick: u64,
    camera_zoom_hundredths: i32,
    camera_zoom_floor_hundredths: i32,
    camera_zoom_ceiling_hundredths: i32,
    tutorial_camera_zoom_hundredths: i32,
    tutorial_target_zoom_hundredths: i32,
    tutorial_marker_player_id: Option<u16>,
    tutorial_emote_token: Option<String>,
    tutorial_target_hovered: bool,
    tutorial_camera_viewport: Option<(u32, u32, u32, u32)>,
    map_menu_view: Option<MapContextMenuView>,
    map_menu_session: u64,
    map_menu_tile: u32,
}

#[derive(Clone, Copy)]
struct MyPlayerSummary {
    gold: f64,
    troops: f64,
    max_troops: f64,
    alive: bool,
    has_spawned: bool,
    team: Option<sow_core::protocol::Team>,
    leader: sow_core::player::Leader,
    inbox_count: usize,
    nuke_available: bool,
    nuke_cooldown_ticks: u32,
    boats_in_use: u32,
    boat_capacity: u32,
    emote_active: bool,
    emote_pinned: bool,
}

fn my_player_summary(app: &SowApp, snapshot_tick: u64, my_pid: u16) -> Option<MyPlayerSummary> {
    LAST_MY_PLAYER.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some((cached_tick, cached_pid, summary)) = *cache {
            if cached_tick == snapshot_tick && cached_pid == my_pid {
                return summary;
            }
        }
        let summary = app
            .sim
            .current_snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.players.iter().find(|player| player.id == my_pid))
            .map(|player| MyPlayerSummary {
                gold: player.gold,
                troops: player.troops,
                max_troops: player.max_troops,
                alive: player.alive,
                has_spawned: player.has_spawned,
                team: player.team,
                leader: player.leader,
                inbox_count: player.alliance_requests.len() + player.resource_requests.len(),
                nuke_available: player.nuke_available,
                nuke_cooldown_ticks: player.nuke_cooldown_ticks,
                boats_in_use: player.boats_in_use,
                boat_capacity: player.boat_capacity,
                emote_active: player.active_emote.is_some(),
                emote_pinned: player.active_emote.is_some_and(|event| event.pinned),
            });
        *cache = Some((snapshot_tick, my_pid, summary));
        summary
    })
}

fn refresh_web_leaderboard_cache(app: &mut SowApp) {
    let Some(snapshot) = app.sim.current_snapshot.as_ref() else {
        LAST_WEB_TOP_THREE_TICK.with(|tick| tick.set(None));
        app.ui.leaderboard_top_three = [None; 3];
        return;
    };

    let snapshot_tick = snapshot.tick;
    let changed = LAST_WEB_TOP_THREE_TICK.with(|tick| {
        if tick.get() == Some(snapshot_tick) {
            false
        } else {
            tick.set(Some(snapshot_tick));
            true
        }
    });
    if !changed {
        return;
    }

    let mut rankings: Vec<&sow_core::protocol::PlayerSnapshot> = snapshot
        .players
        .iter()
        .filter(|player| player.alive)
        .collect();
    if rankings.len() > 3 {
        rankings.select_nth_unstable_by(2, leaderboard_cmp);
        rankings.truncate(3);
    }
    rankings.sort_unstable_by(leaderboard_cmp);
    app.ui.leaderboard_top_three =
        std::array::from_fn(|index| rankings.get(index).map(|player| player.id));
}

#[inline]
fn leaderboard_cmp(
    a: &&sow_core::protocol::PlayerSnapshot,
    b: &&sow_core::protocol::PlayerSnapshot,
) -> Ordering {
    b.tile_count
        .cmp(&a.tile_count)
        .then_with(|| b.troops.total_cmp(&a.troops))
        .then_with(|| a.id.cmp(&b.id))
}

pub(crate) fn register_event_loop_wake(proxy: winit::event_loop::EventLoopProxy) {
    EVENT_LOOP_WAKE.with(|wake| *wake.borrow_mut() = Some(proxy));
}

pub(crate) fn wake_event_loop() {
    EVENT_LOOP_WAKE.with(|wake| {
        if let Some(proxy) = wake.borrow().as_ref() {
            proxy.wake_up();
        }
    });
}

pub(crate) fn wake_event_loop_after(delay_ms: u64) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let callback = Closure::once_into_js(wake_event_loop);
    let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
        callback.unchecked_ref(),
        delay_ms.min(i32::MAX as u64) as i32,
    );
}

/// Wake the Rust event loop after a browser-side command or async result is queued.
#[wasm_bindgen(js_name = SOW_wake_event_loop)]
pub fn sow_wake_event_loop() {
    wake_event_loop();
}

/// Called by the vanilla shell after the WASM module has initialized.
#[wasm_bindgen]
pub fn sow_menu_command(json: String) {
    match serde_json::from_str::<WebMenuCommand>(&json) {
        Ok(command) => {
            COMMANDS.with(|commands| commands.borrow_mut().push_back(command));
            wake_event_loop();
        }
        Err(error) => log::warn!("[WEB MENU] invalid command: {error}"),
    }
}

fn take_commands() -> Vec<WebMenuCommand> {
    COMMANDS.with(|commands| commands.borrow_mut().drain(..).collect())
}

impl SowApp {
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn process_web_menu_commands(&mut self) {
        for command in take_commands() {
            match command {
                WebMenuCommand::QuickMatch => {
                    crate::store_portals::track_product_event("menu_quick_match");
                    self.request_join(None, false, None, None);
                    if self.join_matchmaking && self.ui.app.main_menu_state.is_waiting {
                        self.quick_match_revision = self.quick_match_revision.wrapping_add(1);
                    }
                }
                WebMenuCommand::JoinLobby { lobby_id } => {
                    crate::store_portals::track_product_event("menu_join_attempt");
                    self.process_ui_actions(Some(UiAction::JoinLobby(lobby_id)));
                }
                WebMenuCommand::JoinWithPassword { lobby_id, password } => {
                    crate::store_portals::track_product_event("menu_password_join_attempt");
                    self.ui.app.main_menu_state.join_password_input = password;
                    self.ui.app.main_menu_state.join_password_for_lobby = Some(lobby_id);
                    self.process_ui_actions(Some(UiAction::JoinWithPassword(lobby_id)));
                }
                WebMenuCommand::JoinCode { code } => {
                    crate::store_portals::track_product_event("menu_code_join_attempt");
                    self.ui.app.main_menu_state.join_lobby_code = code;
                    self.process_ui_actions(Some(UiAction::JoinWithCode));
                }
                WebMenuCommand::CreateGame {
                    config,
                    is_private,
                    password,
                } => match serde_json::from_value::<sow_core::game_config::GameConfig>(config) {
                    Ok(config) => {
                        crate::store_portals::track_product_event("menu_custom_create");
                        self.process_ui_actions(Some(UiAction::CreateGame {
                            config: Box::new(config),
                            is_private,
                            password,
                        }))
                    }
                    Err(error) => {
                        log::warn!("[WEB MENU] invalid create-game config: {error}");
                        self.ui.app.main_menu_state.error_message =
                            Some(crate::ui::UiText::new("menu.invalid_game_configuration"));
                    }
                },
                WebMenuCommand::StartSinglePlayer { config } => {
                    match serde_json::from_value::<sow_core::game_config::GameConfig>(config) {
                        Ok(config) => {
                            crate::store_portals::track_product_event("menu_single_player_start");
                            self.process_ui_actions(Some(UiAction::StartSinglePlayer(Box::new(
                                config,
                            ))))
                        }
                        Err(error) => {
                            log::warn!("[WEB MENU] invalid single-player config: {error}");
                            self.ui.app.main_menu_state.error_message =
                                Some(crate::ui::UiText::new("menu.invalid_game_configuration"));
                        }
                    }
                }
                WebMenuCommand::StartCampaignEpisode {
                    episode_id,
                    roster,
                    match_config,
                } => {
                    let Some(campaign) = CampaignId::from_episode_id(&episode_id) else {
                        self.ui.app.main_menu_state.error_message =
                            Some(crate::ui::UiText::new("tutorial.unavailable"));
                        continue;
                    };
                    if !campaign.is_unlocked(&self.progress) {
                        self.ui.app.main_menu_state.error_message =
                            Some(crate::ui::UiText::new("menu.campaign_episode_locked"));
                        continue;
                    }
                    crate::store_portals::track_product_event("menu_campaign_start");
                    self.ui.tutorial_marker_player_id = None;
                    self.ui.dialog_border_highlight = None;
                    self.boot_campaign_pending = None;
                    if let Err(error) =
                        self.start_campaign_episode_from_web(campaign, roster, match_config)
                    {
                        log::warn!("[WEB MENU] invalid campaign episode: {error}");
                        self.ui.app.main_menu_state.error_message =
                            Some(crate::ui::UiText::new("tutorial.invalid"));
                    }
                }
                WebMenuCommand::SetTutorialPaused {
                    paused,
                    camera_only,
                    paused_action,
                } => {
                    if self.ui.tutorial_active && self.net.is_offline {
                        self.sim.paused = paused;
                        self.ui.tutorial_camera_only = paused && camera_only;
                        self.ui.tutorial_paused_action = if paused && !camera_only {
                            paused_action
                        } else {
                            None
                        };
                        self.input.tutorial_camera_drag_recorded = false;
                        if paused {
                            self.sim.offline_intents.clear();
                        }
                        if self.ui.tutorial_camera_only {
                            self.clear_placement();
                            self.close_map_context_menu();
                        }
                    }
                }
                WebMenuCommand::SetCampaignUnlocks {
                    campaign_session_id,
                    unlocks,
                } => {
                    if self.ui.tutorial_active
                        && self.net.is_offline
                        && campaign_session_id == self.ui.campaign_session_id
                        && let Some(engine) = self.sim.engine.as_mut()
                        && engine.state.config.tutorial
                    {
                        engine.set_campaign_unlocks(unlocks);
                    }
                }
                WebMenuCommand::SetCampaignAttackPermission {
                    faction_id,
                    can_attack_player,
                } => {
                    if self.ui.tutorial_active
                        && self.net.is_offline
                        && let Some(engine) = self.sim.engine.as_mut()
                        && engine.state.config.tutorial
                        && !engine.set_campaign_attack_permission(&faction_id, can_attack_player)
                    {
                        log::warn!(
                            "[CAMPAIGN] attack permission target is missing or not an enemy: {faction_id}"
                        );
                    }
                }
                WebMenuCommand::HaltCampaignFaction { faction_id } => {
                    if self.ui.tutorial_active
                        && self.net.is_offline
                        && let Some(engine) = self.sim.engine.as_mut()
                        && engine.state.config.tutorial
                        && !engine.halt_campaign_faction(&faction_id)
                    {
                        log::warn!("[CAMPAIGN] halt target is missing: {faction_id}");
                    }
                }
                WebMenuCommand::AcceptCampaignAlliance { faction_id, campaign_session_id } => {
                    if !self.ui.tutorial_active || !self.net.is_offline || campaign_session_id != self.ui.campaign_session_id {
                        continue;
                    }
                    let Some(human) = self.sim.my_player_id else { continue; };
                    let actor = self.sim.engine.as_ref().and_then(|engine| {
                        let actor = engine.campaign_player_id(&faction_id)?;
                        (engine.state.config.tutorial && engine.has_alliance_proposal(human, actor))
                            .then_some(actor)
                    });
                    if let Some(actor) = actor {
                        self.apply_paused_tutorial_intent_as(actor, sow_core::protocol::GameplayIntent::AcceptAlliance { target_player: human });
                    }
                }
                WebMenuCommand::ActivateCampaignAssault {
                    attacker_team,
                    target_faction_id,
                    preserve_relation,
                    reinforcement,
                    hold_last_tile,
                } => {
                    if self.ui.tutorial_active && self.net.is_offline {
                        let target_allies = target_faction_id == "player_and_allies";
                        let target_player_id = if target_faction_id == "player" || target_allies {
                            self.sim.my_player_id
                        } else {
                            self.sim.engine.as_ref().and_then(|engine| {
                                engine
                                    .campaign_faction_ids
                                    .iter()
                                    .find_map(|(player_id, id)| {
                                        (id == &target_faction_id).then_some(*player_id)
                                    })
                            })
                        };
                        if let (Some(target_player_id), Some(engine)) =
                            (target_player_id, self.sim.engine.as_mut())
                        {
                            let reinforcement = reinforcement.map(|settings| {
                                (settings.capacity_ratio, settings.interval_seconds)
                            });
                            if engine.activate_campaign_assault(
                                attacker_team,
                                target_player_id,
                                preserve_relation,
                                target_allies,
                                reinforcement,
                                hold_last_tile,
                            ) == 0
                            {
                                log::warn!(
                                    "[CAMPAIGN] assault had no valid scripted attackers or target"
                                );
                            }
                        } else {
                            log::warn!("[CAMPAIGN] assault target not found: {target_faction_id}");
                        }
                    }
                }
                WebMenuCommand::SetTutorialMarker { player_id } => {
                    if self.ui.tutorial_active && self.net.is_offline {
                        self.ui.tutorial_marker_player_id = player_id.filter(|player_id| {
                            self.sim.current_snapshot.as_ref().is_some_and(|snapshot| {
                                snapshot
                                    .players
                                    .iter()
                                    .any(|player| player.id == *player_id)
                            })
                        });
                    }
                }
                WebMenuCommand::SetDialogBorderHighlight { player_id } => {
                    if self.ui.tutorial_active && self.net.is_offline {
                        self.ui.dialog_border_highlight = player_id
                            .filter(|player_id| {
                                self.sim.current_snapshot.as_ref().is_some_and(|snapshot| {
                                    snapshot
                                        .players
                                        .iter()
                                        .any(|player| player.id == *player_id)
                                })
                            })
                            .map(|player_id| crate::app::DialogBorderHighlight {
                                player_id,
                                start_time: Instant::now(),
                            });
                    }
                }
                WebMenuCommand::ResolveCampaignDiplomacy {
                    target_player_id,
                    relation,
                    gold_cost,
                } => {
                    if self.ui.tutorial_active
                        && self.net.is_offline
                        && gold_cost.is_finite()
                        && (0.0..=1_000_000.0).contains(&gold_cost)
                        && self.sim.current_snapshot.as_ref().is_some_and(|snapshot| {
                            snapshot
                                .players
                                .iter()
                                .any(|player| player.id == target_player_id)
                        })
                    {
                        self.send_intent(
                            sow_core::protocol::GameplayIntent::ResolveCampaignDiplomacy {
                                target_player: target_player_id,
                                relation,
                                gold_cost,
                            },
                        );
                    }
                }
                WebMenuCommand::MapMenuAction {
                    session,
                    tile_idx,
                    action,
                } => {
                    if !self.ui.tutorial_camera_only {
                        self.handle_map_menu_action(session, tile_idx, action);
                    }
                }
                WebMenuCommand::CloseMapContextMenu => self.close_map_context_menu(),
                WebMenuCommand::CompleteCampaignEpisode { episode_id } => {
                    let Some(campaign) = CampaignId::from_episode_id(&episode_id) else {
                        self.ui.app.main_menu_state.error_message =
                            Some(crate::ui::UiText::new("tutorial.unavailable"));
                        continue;
                    };
                    if !self.ui.tutorial_active
                        || !self.net.is_offline
                        || self.ui.tutorial_campaign != campaign
                    {
                        log::warn!(
                            "[WEB MENU] campaign completion rejected outside active episode"
                        );
                        continue;
                    }
                    if campaign == CampaignId::Boudica {
                        self.begin_exit_to_main_menu();
                        continue;
                    }
                    if !sow_data::campaign::episode_is_published(campaign.episode_id()) {
                        self.ui.app.main_menu_state.error_message =
                            Some(crate::ui::UiText::new("tutorial.unavailable"));
                        continue;
                    }
                    if self.progress.complete_episode(campaign.episode_id()) {
                        self.save_local_progress();
                    }
                    if self
                        .progress
                        .pending_campaign_completions
                        .contains(campaign.episode_id())
                    {
                        self.pending_campaign_exit = Some(campaign.episode_id().to_string());
                        self.profile_action_retry_at = Some(web_time::Instant::now());
                        self.poll_profile_actions(web_time::Instant::now());
                    } else {
                        self.begin_exit_to_main_menu();
                    }
                }
                WebMenuCommand::SetLeader { leader_id } => {
                    let value = serde_json::Value::String(leader_id);
                    match serde_json::from_value::<sow_core::player::Leader>(value) {
                        Ok(leader) => {
                            self.ui
                                .app
                                .main_menu_state
                                .set_selected_leader(leader, true);
                            self.progress.preferred_leader = Some(leader);
                            self.progress.prefer_random_leader = false;
                            self.progress.leader_selection_pending = true;
                            self.save_local_progress();
                            self.persist_leader_selection();
                        }
                        Err(error) => log::warn!("[WEB MENU] invalid leader: {error}"),
                    }
                }
                WebMenuCommand::SetRandomLeader => {
                    self.ui.app.main_menu_state.set_random_leader(true, true);
                    self.progress.prefer_random_leader = true;
                    self.progress.leader_selection_pending = true;
                    self.save_local_progress();
                    self.persist_leader_selection();
                }
                WebMenuCommand::UnlockLeader {
                    leader_id,
                    currency,
                } => {
                    self.process_ui_actions(Some(UiAction::UnlockLeader {
                        leader_id,
                        currency,
                    }));
                }
                WebMenuCommand::UnlockSkin { skin_id } => {
                    self.process_ui_actions(Some(UiAction::UnlockSkin(skin_id)));
                }
                WebMenuCommand::EquipSkin { skin_id } => {
                    self.process_ui_actions(Some(UiAction::EquipSkin(skin_id)));
                }
                WebMenuCommand::SaveDisplayName { name } => {
                    self.process_ui_actions(Some(UiAction::SaveDisplayName(name)));
                }
                WebMenuCommand::AcknowledgeRewardReceipts {
                    account_id,
                    receipt_ids,
                } => {
                    if self.progress_account_id.as_deref() == Some(account_id.as_str()) {
                        self.acknowledge_reward_receipts(receipt_ids);
                    }
                }
                WebMenuCommand::SaveMenuGuideProgress {
                    guide_id,
                    completed_steps,
                    current_step,
                    status,
                    selected_leader,
                } => {
                    self.save_menu_guide_progress(
                        guide_id,
                        completed_steps,
                        current_step,
                        status,
                        selected_leader,
                    );
                }
                WebMenuCommand::OpenBrowser => {
                    self.process_ui_actions(Some(UiAction::OpenJoinBrowser));
                }
                WebMenuCommand::OpenCreate => {
                    self.process_ui_actions(Some(UiAction::OpenCreateGame));
                }
                WebMenuCommand::CloseOverlay => {
                    self.process_ui_actions(Some(UiAction::CloseOverlay));
                }
                WebMenuCommand::LeaveLobby => {
                    self.process_ui_actions(Some(UiAction::LeaveLobby));
                }
                WebMenuCommand::RetryMapDownload => self.retry_map_download(),
                WebMenuCommand::StartPrivate { lobby_id } => {
                    self.process_ui_actions(Some(UiAction::StartPrivateLobby(lobby_id)));
                }
                WebMenuCommand::KickPlayer {
                    lobby_id,
                    target_player_id,
                } => {
                    self.process_ui_actions(Some(UiAction::KickPlayer {
                        lobby_id,
                        target_player_id,
                    }));
                }
                WebMenuCommand::BanPlayer {
                    lobby_id,
                    target_player_id,
                } => {
                    self.process_ui_actions(Some(UiAction::BanPlayer {
                        lobby_id,
                        target_player_id,
                    }));
                }
                WebMenuCommand::MovePlayerTeam {
                    lobby_id,
                    target_player_id,
                } => {
                    self.process_ui_actions(Some(UiAction::MovePlayerTeam {
                        lobby_id,
                        target_player_id,
                    }));
                }
                WebMenuCommand::RefreshProfile => {
                    self.fetch_cloud_progress();
                }
                WebMenuCommand::SignIn => {
                    crate::store_portals::show_auth_prompt();
                }
                WebMenuCommand::SignOut => {
                    crate::store_portals::sign_out();
                }
                WebMenuCommand::SetMute { value } => {
                    self.ui.app.settings_state.mute_all = value;
                    self.ui.app.settings_state.persist_local();
                    sow_audio::set_master_volume(if value {
                        0.0
                    } else {
                        1.0
                    });
                }
                WebMenuCommand::SetMusicVolume { value } => {
                    let volume = value.clamp(0.0, 1.0);
                    self.ui.app.settings_state.music_volume = volume;
                    self.ui.app.settings_state.persist_local();
                }
                WebMenuCommand::SetReducedMotion { value } => {
                    self.ui.app.settings_state.set_reduced_motion(value);
                    self.ui.app.settings_state.persist_local();
                }
                WebMenuCommand::SetFreeZoomOut { value } => {
                    self.ui.app.settings_state.free_zoom_out = value;
                    self.ui.app.settings_state.persist_local();
                    self.clamp_camera_to_map();
                }
                WebMenuCommand::SetAttackRatio { ratio } => {
                    self.ui.app.hud_state.attack_ratio = ratio.clamp(0.05, 1.0);
                }
                WebMenuCommand::SpawnTroops => {
                    if let Some(player_id) = self.sim.my_player_id {
                        if let Some(player) = self
                            .sim
                            .current_snapshot
                            .as_ref()
                            .and_then(|s| s.players.iter().find(|p| p.id == player_id))
                        {
                            let (cap_x, cap_y) =
                                (player.centroid_x as u32, player.centroid_y as u32);
                            self.send_intent(sow_core::protocol::GameplayIntent::Spawn {
                                x: cap_x,
                                y: cap_y,
                            });
                        }
                    }
                }
                WebMenuCommand::SelectBuilding { kind } => {
                    if !self.ui.tutorial_camera_only {
                        self.select_building_kind(kind);
                    }
                }
                WebMenuCommand::SelectWarship => {
                    if !self.ui.tutorial_camera_only {
                        self.select_warship_build_mode();
                    }
                }
                WebMenuCommand::SelectNuke => {
                    if !self.ui.tutorial_camera_only {
                        self.select_nuke_kind(sow_core::game::NukeKind::AtomBomb);
                    }
                }
                WebMenuCommand::ToggleInbox => {
                    self.ui.app.hud_state.show_alliance_inbox =
                        !self.ui.app.hud_state.show_alliance_inbox;
                }
                WebMenuCommand::AcceptAlliance { target_player_id } => {
                    self.send_intent(sow_core::protocol::GameplayIntent::AcceptAlliance {
                        target_player: target_player_id,
                    });
                }
                WebMenuCommand::RejectAlliance { target_player_id } => {
                    self.send_intent(sow_core::protocol::GameplayIntent::RejectAlliance {
                        target_player: target_player_id,
                    });
                }
                WebMenuCommand::OpenTransfer { target_player_id } => {
                    self.ui.app.hud_state.show_ask_panel = Some(target_player_id);
                    self.ui.app.hud_state.ask_gold = 0.0;
                    self.ui.app.hud_state.ask_troops = 0.0;
                    self.ui.app.hud_state.transfer_confirm_pending = false;
                }
                WebMenuCommand::CloseTransfer => {
                    self.ui.app.hud_state.show_ask_panel = None;
                    self.ui.app.hud_state.transfer_confirm_pending = false;
                }
                WebMenuCommand::SendResources {
                    target_player_id,
                    gold,
                    troops,
                } => {
                    let accepted =
                        self.send_intent(sow_core::protocol::GameplayIntent::SendResources {
                            target_player: target_player_id,
                            gold: gold.max(0.0),
                            troops: troops.max(0.0),
                        });
                    if accepted {
                        self.ui.app.hud_state.show_ask_panel = None;
                        self.ui.app.hud_state.transfer_confirm_pending = false;
                    }
                }
                WebMenuCommand::RequestResources {
                    target_player_id,
                    gold,
                    troops,
                } => {
                    self.send_intent(sow_core::protocol::GameplayIntent::RequestResources {
                        target_player: target_player_id,
                        gold: gold.max(0.0),
                        troops: troops.max(0.0),
                    });
                    self.ui.app.hud_state.show_ask_panel = None;
                    self.ui.app.hud_state.transfer_confirm_pending = false;
                }
                WebMenuCommand::AcceptResourceRequest { target_player_id } => {
                    self.send_intent(sow_core::protocol::GameplayIntent::AcceptResourceRequest {
                        target_player: target_player_id,
                    });
                }
                WebMenuCommand::RejectResourceRequest { target_player_id } => {
                    self.send_intent(sow_core::protocol::GameplayIntent::RejectResourceRequest {
                        target_player: target_player_id,
                    });
                }
                WebMenuCommand::ConfirmBetrayal => {
                    let warning = self
                        .ui
                        .app
                        .hud_state
                        .show_betrayal_warning
                        .clone()
                        .or_else(|| self.ui.app.hud_state.betrayal_warning_cached.clone());
                    if let Some((ally_id, intent)) = warning {
                        self.send_intent(sow_core::protocol::GameplayIntent::BreakAlliance {
                            target_player: ally_id,
                        });
                        self.send_intent(intent);
                    }
                    self.ui.app.hud_state.show_betrayal_warning = None;
                    self.ui.app.hud_state.betrayal_warning_cached = None;
                }
                WebMenuCommand::CancelBetrayal => {
                    self.ui.app.hud_state.show_betrayal_warning = None;
                    self.ui.app.hud_state.betrayal_warning_cached = None;
                }
                WebMenuCommand::CancelAttack { attack_id } => {
                    self.send_intent(sow_core::protocol::GameplayIntent::CancelAttack {
                        attack_id,
                    });
                }
                WebMenuCommand::RecallFleet { fleet_id } => {
                    self.send_intent(sow_core::protocol::GameplayIntent::RecallFleet { fleet_id });
                }
                WebMenuCommand::ScuttleFleet { fleet_id } => {
                    self.send_intent(sow_core::protocol::GameplayIntent::ScuttleFleet { fleet_id });
                }
                WebMenuCommand::CounterAttack { target_player_id } => {
                    let counter_attack = self.sim.current_snapshot.as_ref().and_then(|snapshot| {
                        let my_player_id = self.sim.my_player_id?;
                        if target_player_id == 0 || target_player_id == my_player_id {
                            return None;
                        }
                        let me = snapshot
                            .players
                            .iter()
                            .find(|player| player.id == my_player_id)?;
                        let target = snapshot
                            .players
                            .iter()
                            .find(|player| player.id == target_player_id)?;
                        if me.team.is_some() && me.team == target.team {
                            return None;
                        }
                        let incoming_troops = incoming_combat_troops(
                            snapshot,
                            self.sim.engine.as_ref(),
                            target_player_id,
                            my_player_id,
                        );
                        if !incoming_troops.is_finite() || incoming_troops <= 0.0 {
                            return None;
                        }
                        let troops = selected_counterattack_troops(
                            me.troops,
                            self.ui.app.hud_state.attack_ratio,
                            self.sim.config.attack_cost_neutral,
                        )?;
                        Some((
                            sow_core::protocol::GameplayIntent::Attack(
                                sow_core::protocol::AttackIntent {
                                    target_owner: target_player_id,
                                    troops: Some(troops),
                                },
                            ),
                            me.alliances.contains(&target_player_id),
                        ))
                    });
                    if let Some((intent, allied)) = counter_attack {
                        if allied {
                            self.ui.app.hud_state.show_betrayal_warning =
                                Some((target_player_id, intent));
                        } else {
                            self.send_intent(intent);
                        }
                    }
                }
                WebMenuCommand::ToggleLeaderboard => {
                    self.ui.show_leaderboard = !self.ui.show_leaderboard;
                    if self.ui.show_leaderboard {
                        self.ui.leaderboard_refresh_at = None;
                        self.ui.leaderboard_publish_pending = true;
                        #[cfg(any(feature = "dev", debug_assertions))]
                        {
                            self.ui.show_dev_sidebar = false;
                        }
                    } else {
                        self.ui.leaderboard_publish_pending = false;
                    }
                }
                WebMenuCommand::ReturnToMenu { destination } => {
                    self.process_ui_actions(Some(UiAction::ReturnToMenu));
                    self.ui.manual_endgame_stats = None;
                    if let Some(context) = &mut self.menu_return {
                        context.requested_screen = destination.filter(|value| value == "store");
                    }
                    self.save_menu_return_context();
                }
                WebMenuCommand::DismissRankedResult => {
                    let dismissed = self.menu_return.as_mut().is_some_and(|context| {
                        if context.ranked_result.is_some() {
                            context.ranked_result_presented = true;
                            true
                        } else {
                            false
                        }
                    });
                    if dismissed {
                        self.save_menu_return_context();
                    }
                }
                WebMenuCommand::RequestEndgameStats => {
                    let tutorial_active = self.ui.tutorial_active && self.net.is_offline;
                    let stats = if self.ui.app.phase == crate::ClientPhase::Playing
                        && !self.ui.is_spectating
                        && !tutorial_active
                    {
                        let my_pid = self
                            .sim
                            .my_player_id
                            .unwrap_or(self.ui.app.hud_state.my_player_id);
                        endgame_stats_snapshot(self, my_pid)
                    } else {
                        None
                    };
                    self.ui.manual_endgame_stats = stats;
                }
                WebMenuCommand::ClearEndgameStats => {
                    self.ui.manual_endgame_stats = None;
                }
                WebMenuCommand::ContinueObserving => {
                    self.ui.is_spectating = true;
                    self.ui.endgame_cache = None;
                    self.ui.manual_endgame_stats = None;
                }
                WebMenuCommand::ZoomIn => {
                    self.process_ui_actions(Some(UiAction::ZoomIn));
                }
                WebMenuCommand::ZoomOut => {
                    self.process_ui_actions(Some(UiAction::ZoomOut));
                }
                WebMenuCommand::CenterCamera => {
                    self.process_ui_actions(Some(UiAction::CenterCamera));
                }
                WebMenuCommand::ToggleDevSidebar => {
                    #[cfg(any(feature = "dev", debug_assertions))]
                    {
                        self.ui.show_dev_sidebar = !self.ui.show_dev_sidebar;
                        if self.ui.show_dev_sidebar {
                            self.ui.show_leaderboard = false;
                        }
                    }
                }
                WebMenuCommand::SetDevConfig { field, value } => {
                    #[cfg(any(feature = "dev", debug_assertions))]
                    {
                        if self.ui.show_dev_sidebar
                            && let Some(value) = decode_dev_config_value(&field, &value)
                        {
                            crate::theme::dev_config::DevConfig::update(|config| {
                                config.assign(&field, value);
                            });
                        }
                    }
                    #[cfg(not(any(feature = "dev", debug_assertions)))]
                    let _ = (field, value);
                }
                WebMenuCommand::SetDevVfxPreview {
                    target,
                    phase,
                    paused,
                    repeat,
                } => {
                    #[cfg(any(feature = "dev", debug_assertions))]
                    {
                        if self.ui.show_dev_sidebar {
                            let now = Instant::now();
                            let config = crate::theme::dev_config::DevConfig::get();
                            let duration = self.ui.dev_vfx_preview.duration(&config);
                            self.ui.dev_vfx_preview.advance(now, duration);
                            if let Some(target) = target {
                                self.ui.dev_vfx_preview.set_target(target, now);
                            }
                            if let Some(phase) = phase {
                                self.ui.dev_vfx_preview.set_phase(phase, now);
                            }
                            if let Some(repeat) = repeat {
                                self.ui.dev_vfx_preview.repeat = repeat;
                            }
                            if let Some(paused) = paused {
                                self.ui.dev_vfx_preview.set_paused(paused, now);
                            }
                        }
                    }
                    #[cfg(not(any(feature = "dev", debug_assertions)))]
                    let _ = (target, phase, paused, repeat);
                }
                WebMenuCommand::ReplayDevVfxPreview =>
                {
                    #[cfg(any(feature = "dev", debug_assertions))]
                    if self.ui.show_dev_sidebar {
                        self.ui.dev_vfx_preview.replay(Instant::now());
                    }
                }
                WebMenuCommand::DevVfxAll { on } => {
                    #[cfg(any(feature = "dev", debug_assertions))]
                    {
                        crate::theme::dev_config::DevConfig::update(|config| {
                            config.vfx_conquer = on;
                            config.vfx_border_breathe = on;
                            config.vfx_energy_flow = on;
                            config.vfx_heartbeat = on;
                            config.fog_of_war = on;
                            config.vfx_ambient_grade = on;
                            config.vfx_screen_vignette = on;
                            config.vfx_holo_grid = on;
                            config.vfx_movers = on;
                            config.vfx_tutorial_highlight = on;
                            config.vfx_mover_trails = on;
                            config.vfx_click_markers = on;
                            config.vfx_attack_badges = on;
                            config.vfx_world_buildings = on;
                            config.vfx_bot_avatars = on;
                            config.vfx_nameplate_names = on;
                            config.vfx_nameplate_troops = on;
                            for param in sow_render::shader_params::PARAMS.iter().filter(|param| {
                                param.kind == sow_render::shader_params::ParamKind::Bool
                            }) {
                                config
                                    .assign(param.id, [if on { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0]);
                            }
                        });
                    }
                    #[cfg(not(any(feature = "dev", debug_assertions)))]
                    let _ = on;
                }
                WebMenuCommand::DevVfxGroup { group, on } => {
                    #[cfg(any(feature = "dev", debug_assertions))]
                    crate::theme::dev_config::DevConfig::update(|config| {
                        config.set_switch_group(&group, on);
                    });
                    #[cfg(not(any(feature = "dev", debug_assertions)))]
                    let _ = (group, on);
                }
                WebMenuCommand::ResetDevConfig { section, field } => {
                    #[cfg(any(feature = "dev", debug_assertions))]
                    {
                        if let Some(field) = field
                            .filter(|field| sow_render::shader_params::definition(field).is_some())
                        {
                            crate::theme::dev_config::DevConfig::update(|config| {
                                config.reset_field(&field);
                            });
                        } else if let Some(section) =
                            section.filter(|section| !section.is_empty() && section != "all")
                        {
                            crate::theme::dev_config::DevConfig::update(|config| {
                                config.reset_section(&section);
                            });
                        } else {
                            crate::theme::dev_config::DevConfig::set(
                                crate::theme::dev_config::DevConfig::default(),
                            );
                        }
                    }
                    #[cfg(not(any(feature = "dev", debug_assertions)))]
                    let _ = (section, field);
                }
                WebMenuCommand::SetShowDevTools { value } => {
                    self.ui.app.settings_state.show_dev_tools = value;
                    self.ui.app.settings_state.persist_local();
                    self.ui.show_dev_sidebar = value;
                }
                WebMenuCommand::SetShowFpsPing { value } => {
                    self.ui.app.settings_state.show_fps_ping = value;
                    self.ui.app.settings_state.persist_local();
                }
                WebMenuCommand::TutorialEmote { kind, pinned, faction_id, token, campaign_session_id, message } => {
                    let has_scripted_line = token.is_some() || message.is_some();
                    if kind.is_none() && message.is_none() && let Some(token) = token.as_deref() {
                        if !self.net.is_offline { continue; }
                        if let Some((player_id, event_id)) = self.ui.nameplates.clear_campaign_emote_token(token)
                            && let Some(engine) = self.sim.engine.as_mut()
                        {
                            let tick = engine.state.tick;
                            if let Some(player) = engine.state.player_mut(player_id)
                                && player.active_emote.is_some_and(|event| event.scripted && event.event_id == event_id)
                            {
                                sow_core::emote::express(player, None, false, true, tick);
                                self.sim.current_snapshot = Some(engine.build_snapshot());
                            }
                        }
                        continue;
                    }
                    if !self.ui.tutorial_active || !self.net.is_offline
                        || (has_scripted_line && (self.ui.campaign_session_id == 0 || campaign_session_id != Some(self.ui.campaign_session_id)))
                        || token.as_ref().is_some_and(|value| value.len() > 192)
                        || message.as_ref().is_some_and(|value| {
                            let trimmed = value.trim();
                            trimmed.is_empty() || trimmed.chars().count() > 160 || trimmed.chars().any(char::is_control)
                        })
                        || (message.is_some() && token.is_none())
                        || (token.is_some() && kind.is_some() && message.is_none())
                        || (token.is_some() && kind.is_none() && message.is_some())
                    {
                        continue;
                    }
                    if faction_id.is_none() && token.is_none() && message.is_none() {
                        if let Some(player_id) = self.sim.my_player_id {
                            self.ui.nameplates.clear_campaign_emote(None, player_id);
                        }
                        let intent = match kind {
                            Some(kind) => sow_core::protocol::GameplayIntent::ExpressEmote { kind, pinned },
                            None => sow_core::protocol::GameplayIntent::ClearEmote,
                        };
                        self.send_intent(intent);
                        continue;
                    }
                    let Some(engine) = self.sim.engine.as_mut().filter(|engine| engine.state.config.tutorial) else { continue; };
                    let player_id = faction_id.as_deref().and_then(|id| engine.campaign_player_id(id))
                        .or_else(|| faction_id.is_none().then_some(self.sim.my_player_id).flatten());
                    let Some(player_id) = player_id else {
                        log::warn!("[CAMPAIGN] emote speaker is missing: {faction_id:?}");
                        continue;
                    };
                    if let Some(token) = token.as_deref()
                        && kind.is_none()
                        && !self.ui.nameplates.campaign_emote_matches(token, player_id)
                    {
                        continue;
                    }
                    let tick = engine.state.tick;
                    let Some(player) = engine.state.player_mut(player_id).filter(|player| player.alive) else { continue; };
                    sow_core::emote::express(player, kind, pinned, true, tick);
                    if let (Some(token), Some(message), Some(event)) = (token.as_ref(), message, player.active_emote) {
                        if !self.ui.nameplates.set_campaign_emote(player_id, event.event_id, token.clone(), message) {
                            log::warn!("[CAMPAIGN] invalid authored emote line rejected");
                            continue;
                        }
                    } else {
                        self.ui.nameplates.clear_campaign_emote(token.as_deref(), player_id);
                    }
                    // Story commands publish immediately, including while the simulation is paused.
                    self.sim.current_snapshot = Some(engine.build_snapshot());
                }
                WebMenuCommand::SetCampaignPreDefeatTargets { faction_ids, campaign_session_id } => {
                    if !self.ui.tutorial_active || !self.net.is_offline || self.ui.campaign_session_id == 0
                        || campaign_session_id != self.ui.campaign_session_id || faction_ids.len() > 16
                    {
                        continue;
                    }
                    let mut unique = std::collections::HashSet::new();
                    if faction_ids.iter().any(|id| !unique.insert(id.as_str())) {
                        continue;
                    }
                    let Some(engine) = self.sim.engine.as_mut().filter(|engine| engine.state.config.tutorial) else { continue; };
                    if faction_ids.iter().any(|id| engine.campaign_player_id(id).is_none()) {
                        log::warn!("[CAMPAIGN] pre-defeat watch references an unknown faction");
                        continue;
                    }
                    engine.set_campaign_pre_defeat_targets(&faction_ids);
                }
                WebMenuCommand::ContinueCampaignPreDefeat { faction_id, campaign_session_id } => {
                    if !self.ui.tutorial_active || !self.net.is_offline || self.ui.campaign_session_id == 0
                        || campaign_session_id != self.ui.campaign_session_id
                    {
                        continue;
                    }
                    let Some(engine) = self.sim.engine.as_mut().filter(|engine| engine.state.config.tutorial) else { continue; };
                    if !engine.continue_campaign_pre_defeat(&faction_id) {
                        log::warn!("[CAMPAIGN] ignored pre-defeat continuation without a matching held faction");
                    }
                }
                WebMenuCommand::ExpressEmote {
                    session,
                    tile_idx,
                    kind,
                } => {
                    let matches_menu = self
                        .input
                        .map_context_menu
                        .is_some_and(|menu| menu.session == session && menu.tile_idx == tile_idx);
                    if matches_menu
                        && self
                            .map_menu_actions(tile_idx)
                            .contains(&crate::input::map_click::MapMenuAction::Emote)
                    {
                        self.send_intent(sow_core::protocol::GameplayIntent::ExpressEmote {
                            kind,
                            pinned: false,
                        });
                        self.close_map_context_menu();
                    }
                }
                WebMenuCommand::ClearEmote => {
                    self.send_intent(sow_core::protocol::GameplayIntent::ClearEmote);
                }
                WebMenuCommand::SetEmotePinned { pinned } => {
                    self.send_intent(sow_core::protocol::GameplayIntent::SetEmotePinned { pinned });
                }
                WebMenuCommand::SetEmoteTexts { texts, safe_area } => {
                    self.ui.nameplates.set_emote_texts(texts, safe_area);
                }
                WebMenuCommand::FocusPlayer { player_id } => {
                    if let Some(snap) = &self.sim.current_snapshot {
                        if let Some(player) = snap.players.iter().find(|p| p.id == player_id) {
                            if player.tile_count > 0 && player.alive {
                                let world_cx = player.centroid_x + 0.5;
                                let world_cy = player.centroid_y + 0.5;
                                self.input.has_snapped_camera_to_spawn = true;
                                self.input.camera_focus_target = Some((world_cx, world_cy));
                                self.input.target_zoom = 8.0;
                                self.input.tutorial_camera_focus = false;
                                self.input.camera_focus_waiting_for_input_release = false;
                            }
                        }
                    }
                }
                WebMenuCommand::FocusWorld { x, y, shot } => {
                    let on_map = |x: f32, y: f32| {
                        x.is_finite()
                            && y.is_finite()
                            && x >= 0.0
                            && y >= 0.0
                            && x < self.sim.map_w as f32
                            && y < self.sim.map_h as f32
                    };
                    if !on_map(x, y) {
                        continue;
                    }
                    if let Some(shot) = shot.filter(|shot| {
                        on_map(shot.from_x, shot.from_y) && shot.seconds.is_finite()
                    }) {
                        let screen = (self.input.screen_w, self.input.screen_h);
                        let map = (self.sim.map_w, self.sim.map_h);
                        let (from, to) = ((shot.from_x, shot.from_y), (x, y));
                        // Light moves only: one glide that eases the zoom by at most a quarter.
                        let zoom = self.input.camera_zoom;
                        let ease = |(center, z): ((f32, f32), f32), lo: f32, hi: f32| {
                            (center, z.clamp(zoom * lo, zoom * hi))
                        };
                        let key = match shot.kind {
                            CameraShotKind::Reveal => ease(
                                crate::campaign::camera_frame(from, to, screen, map, 0.0),
                                0.75,
                                1.0,
                            ),
                            CameraShotKind::Approach => ease(
                                crate::campaign::camera_frame(
                                    to,
                                    to,
                                    screen,
                                    map,
                                    crate::campaign::TUTORIAL_MIN_TILE_CSS,
                                ),
                                1.0,
                                1.25,
                            ),
                            CameraShotKind::Link => (to, zoom),
                        };
                        self.start_camera_shot(vec![key], shot.seconds.clamp(1.0, 5.0), shot.machinima);
                    } else {
                        let tutorial_focus = self.ui.tutorial_active && self.net.is_offline;
                        self.input.has_snapped_camera_to_spawn = true;
                        self.input.target_zoom = self.input.camera_zoom;
                        self.input.camera_focus_target = Some((x, y));
                        self.input.tutorial_camera_focus = tutorial_focus;
                        self.input.camera_focus_waiting_for_input_release =
                            tutorial_focus && self.input.is_pointer_gesture_active();
                    }
                }
            }
        }
    }
}

fn phase_name(phase: crate::ClientPhase) -> &'static str {
    match phase {
        crate::ClientPhase::Splash => "Splash",
        crate::ClientPhase::MainMenu => "MainMenu",
        crate::ClientPhase::Playing => "Playing",
    }
}

fn hovered_tile_owner(app: &SowApp) -> (u32, u16) {
    if matches!(app.input.hover_pointer, HoverPointer::None)
        || (matches!(app.input.hover_pointer, HoverPointer::Touch)
            && app.input.active_touches.len() != 1)
    {
        return (u32::MAX, 0);
    }
    if !app.input.camera_zoom.is_finite() || app.input.camera_zoom <= 0.0 {
        return (u32::MAX, 0);
    }
    let Some((col, row)) = app.mouse_to_tile(app.input.last_mouse_x, app.input.last_mouse_y) else {
        return (u32::MAX, 0);
    };
    let idx = (row * app.sim.map_w as i32 + col) as usize;
    let owner = app
        .gfx
        .map_renderer
        .as_ref()
        .and_then(|renderer| renderer.owners.get(idx).copied())
        .unwrap_or(0);
    (idx as u32, owner)
}

fn combat_operations_snapshot_tick(
    snapshot: &sow_core::protocol::SimSnapshot,
    my_player_id: u16,
    engine: Option<&sow_core::engine::SowEngine>,
) -> u64 {
    let has_attack = snapshot.attacks.iter().any(|attack| {
        attack.owner_id == my_player_id
            || (attack.owner_id != my_player_id && attack.target_owner == my_player_id)
    });
    let has_outgoing_transport = snapshot.fleets.iter().any(|fleet| {
        fleet.unit_type == sow_core::game::UnitType::TransportShip && fleet.owner_id == my_player_id
    });
    let has_incoming_transport = engine.is_some_and(|engine| {
        engine.fleets.iter().any(|fleet| {
            fleet.unit_type == sow_core::game::UnitType::TransportShip
                && fleet.owner_id != my_player_id
                && fleet.target_owner == my_player_id
                && snapshot.fleets.iter().any(|item| {
                    item.id == fleet.id && item.unit_type == sow_core::game::UnitType::TransportShip
                })
        })
    });

    if has_attack || has_outgoing_transport || has_incoming_transport {
        snapshot.tick
    } else {
        0
    }
}

fn endgame_stats_snapshot(app: &SowApp, player_id: u16) -> Option<[u64; 5]> {
    app.sim
        .engine
        .as_ref()
        .and_then(|engine| engine.state.player(player_id))
        .map(|player| {
            [
                u64::from(player.kills),
                u64::from(player.assists),
                u64::from(player.max_tile_count),
                player.gold_received.to_bits(),
                player.troops_received.to_bits(),
            ]
        })
}

fn hud_publish_key(app: &SowApp) -> HudPublishKey {
    let hud = &app.ui.app.hud_state;
    let tutorial_active = app.ui.tutorial_active && app.net.is_offline;
    let map_menu = app.input.map_context_menu;
    let (dev_sidebar_open, dev_config_key) = dev_config_key(app);
    let dev_preview = dev_sidebar_open.then(|| {
        let preview = &app.ui.dev_vfx_preview;
        (
            preview.target,
            (preview.phase.clamp(0.0, 1.0) * 100.0).round() as u8,
            preview.paused,
            preview.repeat,
            preview.active,
        )
    });
    let snapshot_tick = app
        .sim
        .current_snapshot
        .as_ref()
        .map(|snapshot| snapshot.tick)
        .unwrap_or(0);
    let cold_open = app.ui.show_leaderboard
        || hud.building_placement.is_some()
        || hud.show_alliance_inbox
        || hud.show_ask_panel.is_some()
        || hud.show_betrayal_warning.is_some()
        || hud.sync_state.is_some()
        || tutorial_active
        || map_menu.is_some();
    let my_pid = app.sim.my_player_id.unwrap_or(hud.my_player_id);
    let me = my_player_summary(app, snapshot_tick, my_pid);
    let snapshot = app.sim.current_snapshot.as_ref();
    let combat_operations_tick = snapshot.map_or(0, |snapshot| {
        combat_operations_snapshot_tick(snapshot, my_pid, app.sim.engine.as_ref())
    });
    let endgame_active = !app.ui.is_spectating
        && snapshot.is_some_and(|snapshot| {
            snapshot.winner.is_some()
                || me.is_some_and(|player| !player.alive && player.has_spawned)
        });
    let player_endgame_stats = if endgame_active {
        endgame_stats_snapshot(app, my_pid)
    } else {
        app.ui.manual_endgame_stats
    };
    let inbox_count = me.map(|player| player.inbox_count).unwrap_or(0);
    HudPublishKey {
        fps: app.time.current_fps,
        ping_ms: app.net.current_ping_ms,
        gold: hud.gold.to_bits(),
        troops: hud.troops.to_bits(),
        max_troops: hud.max_troops.to_bits(),
        troop_rate: hud.troop_rate.to_bits(),
        gold_rate: hud.gold_rate.to_bits(),
        attack_ratio: hud.attack_ratio.to_bits(),
        spawn_timer_tenths: hud
            .spawn_timer_secs
            .map(|secs| (secs.max(0.0) * 10.0).round() as i32)
            .unwrap_or(-1),
        selected_building: hud.building_placement,
        selected_warship_build: hud.selected_warship_build,
        selected_nuke: hud
            .selected_nuke_kind
            .map(|kind| kind as u8)
            .unwrap_or(u8::MAX),
        building_costs: std::array::from_fn(|index| hud.building_costs[index].to_bits()),
        settings_mute: app.ui.app.settings_state.mute_all,
        settings_music_volume: app.ui.app.settings_state.music_volume.to_bits(),
        settings_reduced_motion: app.ui.app.settings_state.reduced_motion,
        settings_free_zoom_out: app.ui.app.settings_state.free_zoom_out,
        settings_show_fps_ping: app.ui.app.settings_state.show_fps_ping,
        settings_show_dev_tools: app.ui.app.settings_state.show_dev_tools,
        leaderboard_open: app.ui.show_leaderboard,
        leaderboard_publish_revision: app.ui.leaderboard_publish_revision,
        inbox_open: hud.show_alliance_inbox,
        transfer_target: hud.show_ask_panel,
        betrayal_open: hud.show_betrayal_warning.is_some(),
        sync_open: hud.sync_state.is_some(),
        inbox_count,
        notification_revision: hud.notification_revision,
        is_spectating: app.ui.is_spectating,
        endgame_active,
        endgame_winner: snapshot.and_then(|snapshot| snapshot.winner),
        endgame_team: snapshot.and_then(|snapshot| snapshot.winning_team),
        player_endgame_stats,
        tutorial_active,
        campaign_session_id: if tutorial_active {
            app.ui.campaign_session_id
        } else {
            0
        },
        campaign_unlock_revision: if tutorial_active {
            app.sim
                .engine
                .as_ref()
                .map_or(0, |engine| engine.campaign_unlock_revision)
        } else {
            0
        },
        camera_zoom_hundredths: (app.input.camera_zoom * 100.0).round() as i32,
        camera_zoom_floor_hundredths: (app.zoom_floor() * 100.0).round() as i32,
        camera_zoom_ceiling_hundredths: (crate::camera_zoom_upper_bound(
            app.input.screen_w,
            app.input.screen_h,
        )
        .max(app.zoom_floor())
            * 100.0)
            .round() as i32,
        tutorial_camera_zoom_hundredths: if tutorial_active {
            (app.input.camera_zoom * 100.0).round() as i32
        } else {
            0
        },
        tutorial_target_zoom_hundredths: if tutorial_active {
            (app.input.target_zoom * 100.0).round() as i32
        } else {
            0
        },
        tutorial_marker_player_id: if tutorial_active {
            app.ui.tutorial_marker_player_id
        } else {
            None
        },
        tutorial_emote_token: if tutorial_active { app.ui.nameplates.campaign_emote_ready_token().map(str::to_owned) } else { None },
        tutorial_target_hovered: tutorial_active
            && tutorial_target_under_pointer(app, app.ui.tutorial_marker_player_id.unwrap_or(my_pid)),
        tutorial_camera_viewport: tutorial_active.then_some((
            app.input.camera_x.to_bits(),
            app.input.camera_y.to_bits(),
            app.input.screen_w.to_bits(),
            app.input.screen_h.to_bits(),
        )),
        dev_sidebar_open,
        dev_config: dev_config_key,
        dev_preview,
        snapshot_tick: if cold_open { snapshot_tick } else { 0 },
        combat_operations_tick,
        map_menu_view: map_menu.map(|menu| menu.view),
        map_menu_session: map_menu.map(|menu| menu.session).unwrap_or(0),
        map_menu_tile: map_menu.map(|menu| menu.tile_idx).unwrap_or(u32::MAX),
    }
}

#[cfg(any(feature = "dev", debug_assertions))]
fn dev_config_key(app: &SowApp) -> (bool, Vec<u32>) {
    if !app.ui.show_dev_sidebar {
        return (false, Vec::new());
    }
    let config = crate::theme::dev_config::DevConfig::get();
    (true, config.fingerprint())
}

#[cfg(not(any(feature = "dev", debug_assertions)))]
fn dev_config_key(_app: &SowApp) -> (bool, Vec<u32>) {
    (false, Vec::new())
}

#[cfg(any(feature = "dev", debug_assertions))]
fn dev_tools_payload(app: &SowApp) -> serde_json::Value {
    let open = app.ui.show_dev_sidebar;
    let mut payload = serde_json::json!({
        "open": open,
    });
    if open {
        use sow_render::shader_params::{self, ParamKind};
        let config = crate::theme::dev_config::DevConfig::get();
        let defaults = crate::theme::dev_config::DevConfig::default();
        let mut values = serde_json::Map::new();
        let schema = shader_params::PARAMS
            .iter()
            .map(|param| {
                let value = config.value(param.id);
                values.insert(param.id.to_owned(), setting_value(param.kind, value));
                serde_json::json!({
                    "id": param.id,
                    "label": param.label,
                    "section": param.section,
                    "kind": match param.kind {
                        ParamKind::Number => "number",
                        ParamKind::Bool => "bool",
                        ParamKind::Color => "color",
                        ParamKind::Vector => "vector",
                        ParamKind::Select => "select",
                    },
                    "unit": param.unit,
                    "alpha": param.alpha,
                    "min": param.min,
                    "max": param.max,
                    "step": param.step,
                    "default": setting_value(param.kind, defaults.value(param.id)),
                    "options": param.options.iter().map(|option| serde_json::json!({
                        "label": option.label,
                        "value": option.value,
                    })).collect::<Vec<_>>(),
                })
            })
            .collect::<Vec<_>>();
        payload["config"] = serde_json::Value::Object(values);
        payload["schema"] = serde_json::Value::Array(schema);
        payload["groups"] = shader_params::SECTION_GROUPS
            .iter()
            .map(|(name, sections)| {
                serde_json::json!({
                    "name": name,
                    "sections": sections.iter().map(|section| serde_json::json!({
                        "name": section,
                        "preview": shader_params::SECTION_PREVIEWS
                            .iter()
                            .find(|(owner, _)| owner == section)
                            .map(|(_, target)| *target),
                    })).collect::<Vec<_>>(),
                })
            })
            .collect::<Vec<_>>()
            .into();
        payload["perf"] = shader_params::PERF_SWITCHES
            .iter()
            .map(|(name, _)| serde_json::json!({ "name": name, "state": config.switch_group_state(name) }))
            .collect::<Vec<_>>()
            .into();
        let preview = &app.ui.dev_vfx_preview;
        payload["preview"] = serde_json::json!({
            "target": preview.target,
            "phase": preview.phase,
            "paused": preview.paused,
            "repeat": preview.repeat,
            "active": preview.active,
            "playing": preview.playing,
        });
    }
    payload
}

#[cfg(any(feature = "dev", debug_assertions))]
fn setting_value(kind: sow_render::shader_params::ParamKind, value: [f32; 4]) -> serde_json::Value {
    use sow_render::shader_params::ParamKind;
    match kind {
        ParamKind::Bool => serde_json::json!(value[0] != 0.0),
        ParamKind::Number | ParamKind::Select => serde_json::json!(value[0]),
        ParamKind::Color => serde_json::json!(value),
        ParamKind::Vector => serde_json::json!([value[0], value[1], value[2]]),
    }
}

#[cfg(not(any(feature = "dev", debug_assertions)))]
fn dev_tools_payload(_app: &SowApp) -> serde_json::Value {
    serde_json::json!({
        "open": false,
    })
}

fn tutorial_owner_is_attackable(
    owner: u16,
    my_pid: u16,
    me: &sow_core::protocol::PlayerSnapshot,
    players: &[sow_core::protocol::PlayerSnapshot],
) -> bool {
    if owner == 0 {
        return true;
    }
    let Some(other) = players
        .iter()
        .find(|player| player.id == owner && player.alive && player.tile_count > 0)
    else {
        return false;
    };
    let teammate = me.team.is_some() && me.team == other.team;
    let allied = me.alliances.contains(&owner);
    owner != my_pid && !teammate && !allied
}

fn tutorial_assault_tile(
    owners: &[u16],
    terrain: &[u8],
    map_w: u32,
    map_h: u32,
    border_tiles: &sow_core::bitset::DenseBitSet,
    my_pid: u16,
    me: &sow_core::protocol::PlayerSnapshot,
    players: &[sow_core::protocol::PlayerSnapshot],
) -> Option<u32> {
    if map_w == 0 || map_w.checked_mul(map_h).is_none() {
        return None;
    }
    let width = i32::try_from(map_w).ok()?;
    let height = i32::try_from(map_h).ok()?;
    let (center_x, center_y) = (me.centroid_x as i32, me.centroid_y as i32);
    let mut best: Option<(i64, u32)> = None;
    for border_idx in border_tiles.ones() {
        if border_idx / map_w >= map_h || owners.get(border_idx as usize).copied() != Some(my_pid) {
            continue;
        }
        let (col, row) = ((border_idx % map_w) as i32, (border_idx / map_w) as i32);
        for (dc, dr) in TUTORIAL_ATTACK_NEIGHBORS {
            let (next_col, next_row) = (col + dc, row + dr);
            if next_col < 0 || next_col >= width || next_row < 0 || next_row >= height {
                continue;
            }
            let tile = (next_row as u32 * map_w + next_col as u32) as usize;
            let Some(owner) = owners.get(tile).copied() else {
                continue;
            };
            if !terrain.get(tile).is_some_and(|value| value & 0x80 != 0)
                || !tutorial_owner_is_attackable(owner, my_pid, me, players)
            {
                continue;
            }
            let dx = i64::from(next_col - center_x);
            let dy = i64::from(next_row - center_y);
            let distance = dx * dx + dy * dy;
            if best.is_none_or(|(best_distance, _)| distance < best_distance) {
                best = Some((distance, tile as u32));
            }
        }
    }
    best.map(|(_, tile)| tile)
}

fn tutorial_target_action_tile(
    owners: &[u16],
    terrain: &[u8],
    map_w: u32,
    map_h: u32,
    border_tiles: &sow_core::bitset::DenseBitSet,
    my_pid: u16,
    target_owner: u16,
    me: &sow_core::protocol::PlayerSnapshot,
    players: &[sow_core::protocol::PlayerSnapshot],
) -> Option<u32> {
    let target = players.iter().find(|player| player.id == target_owner)?;
    let target_is_attackable = tutorial_owner_is_attackable(target_owner, my_pid, me, players);
    if target_owner == my_pid
        || !target.alive
        || target.tile_count == 0
        || map_w == 0
        || map_w.checked_mul(map_h).is_none()
    {
        return None;
    }
    let width = i32::try_from(map_w).ok()?;
    let height = i32::try_from(map_h).ok()?;
    let (goal_x, goal_y) = (target.centroid_x as i32, target.centroid_y as i32);
    let mut best: Option<(i64, u32)> = None;
    for border_idx in border_tiles.ones() {
        if border_idx / map_w >= map_h || owners.get(border_idx as usize).copied() != Some(my_pid) {
            continue;
        }
        let (col, row) = ((border_idx % map_w) as i32, (border_idx / map_w) as i32);
        for (dc, dr) in TUTORIAL_ATTACK_NEIGHBORS {
            let (next_col, next_row) = (col + dc, row + dr);
            if next_col < 0 || next_col >= width || next_row < 0 || next_row >= height {
                continue;
            }
            let next = (next_row as u32 * map_w + next_col as u32) as usize;
            let Some(&owner) = owners.get(next) else {
                continue;
            };
            if !terrain.get(next).is_some_and(|value| value & 0x80 != 0) {
                continue;
            }
            if owner == target.id && target_is_attackable {
                return Some(next as u32);
            }
            let legal_frontier = if target_is_attackable {
                tutorial_owner_is_attackable(owner, my_pid, me, players)
            } else {
                owner == 0
            };
            if !legal_frontier {
                continue;
            }
            let source_dx = i64::from(col - goal_x);
            let source_dy = i64::from(row - goal_y);
            let next_dx = i64::from(next_col - goal_x);
            let next_dy = i64::from(next_row - goal_y);
            let source_distance = source_dx * source_dx + source_dy * source_dy;
            let next_distance = next_dx * next_dx + next_dy * next_dy;
            if next_distance < source_distance
                && best.is_none_or(|(best_distance, _)| next_distance < best_distance)
            {
                best = Some((next_distance, next as u32));
            }
        }
    }
    best.map(|(_, tile)| tile)
}

impl SowApp {
    pub(crate) fn tutorial_campaign_attack_tile(&self) -> Option<u32> {
        if !self.ui.tutorial_waiting_for_first_attack
            || self.ui.tutorial_campaign != CampaignId::Boudica
        {
            return None;
        }
        let engine = self.sim.engine.as_ref()?;
        let renderer = self.gfx.map_renderer.as_ref()?;
        let snapshot = self.sim.current_snapshot.as_ref()?;
        let my_pid = self.sim.my_player_id?;
        let target_owner = engine
            .campaign_relations
            .iter()
            .filter_map(|(&id, relation)| {
                (*relation == sow_core::protocol::CampaignRelation::Enemy
                    && engine.campaign_faction_ids.contains_key(&id))
                .then_some(id)
            })
            .min()
            .or_else(|| engine.campaign_faction_ids.keys().copied().min())?;
        let me = snapshot.players.iter().find(|player| player.id == my_pid)?;
        let border_tiles = &engine.state.player(my_pid)?.border_tiles;
        let tile = tutorial_target_action_tile(
            &renderer.owners,
            &renderer.terrain,
            self.sim.map_w,
            self.sim.map_h,
            border_tiles,
            my_pid,
            target_owner,
            me,
            &snapshot.players,
        )?;
        (renderer.owners.get(tile as usize).copied() == Some(target_owner)).then_some(tile)
    }
}

fn tutorial_screen_point_visible(point: [f32; 2], width: f32, height: f32) -> bool {
    point[0].is_finite()
        && point[1].is_finite()
        && point[0] >= 0.0
        && point[1] >= 0.0
        && point[0] <= width
        && point[1] <= height
}

fn tutorial_screen_anchor(point: [f32; 2], width: f32, height: f32) -> Option<([f32; 2], bool)> {
    if !point[0].is_finite() || !point[1].is_finite() || width <= 0.0 || height <= 0.0 {
        return None;
    }
    let margin = width.min(height).min(32.0).max(12.0);
    let max_x = (width - margin).max(margin);
    let max_y = (height - margin).max(margin);
    let visible = tutorial_screen_point_visible(point, width, height);
    Some((
        [point[0].clamp(margin, max_x), point[1].clamp(margin, max_y)],
        !visible,
    ))
}

fn tutorial_rank_build_site_candidates(candidates: &mut [(bool, u32, u32, u32)]) {
    candidates.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then_with(|| b.1.min(b.2).cmp(&a.1.min(a.2)))
            .then_with(|| b.1.cmp(&a.1))
            .then_with(|| b.2.cmp(&a.2))
            .then_with(|| a.3.cmp(&b.3))
    });
}

fn tutorial_border_distance_map(
    map: &sow_core::map::GameMap,
    owners: &[u16],
    terrain: &[u8],
    border_tiles: Option<&sow_core::bitset::DenseBitSet>,
    my_pid: u16,
    max_depth: u32,
) -> Vec<u32> {
    let (map_w, map_h) = (map.width, map.height);
    let Some(area) = map_w.checked_mul(map_h).map(|area| area as usize) else {
        return Vec::new();
    };
    if max_depth == 0 {
        return vec![0; area];
    }
    let mut distances = vec![u32::MAX; area];
    let Some(border_tiles) = border_tiles else {
        return vec![max_depth; area];
    };
    let mut queue = VecDeque::new();
    for raw in border_tiles.ones() {
        let index = raw as usize;
        if index >= area
            || owners.get(index).copied() != Some(my_pid)
            || terrain.get(index).copied().unwrap_or(0) & 0x80 == 0
        {
            continue;
        }
        distances[index] = 0;
        queue.push_back(index);
    }
    while let Some(index) = queue.pop_front() {
        let depth = distances[index];
        if depth >= max_depth {
            continue;
        }
        let (col, row) = ((index as u32) % map_w, (index as u32) / map_w);
        map.for_each_neighbor(col, row, |next_col, next_row| {
            let next = (next_row * map_w + next_col) as usize;
            if distances[next] != u32::MAX
                || owners.get(next).copied() != Some(my_pid)
                || terrain.get(next).copied().unwrap_or(0) & 0x80 == 0
            {
                return;
            }
            distances[next] = depth + 1;
            queue.push_back(next);
        });
    }
    distances
        .into_iter()
        .map(|depth| depth.min(max_depth))
        .collect()
}

fn tutorial_visible_build_site_candidate(
    candidates: &[(bool, u32, u32, u32)],
    mut is_visible: impl FnMut(u32) -> bool,
) -> Option<u32> {
    candidates
        .iter()
        .find_map(|&(_, _, _, tile)| is_visible(tile).then_some(tile))
}

fn tutorial_project_tile(
    tile: Option<u32>,
    map_w: u32,
    map_h: u32,
    input: &crate::app::InputState,
    sf: f32,
) -> Option<[f32; 2]> {
    let tile = tile?;
    if map_w == 0 || tile >= map_w.checked_mul(map_h)? {
        return None;
    }
    Some(crate::render::world::overlays::world_to_screen(
        (tile % map_w) as f32 + 0.5,
        (tile / map_w) as f32 + 0.5,
        input,
        sf,
    ))
}

fn tutorial_building_anchors(
    snapshot: &sow_core::protocol::SimSnapshot,
    my_pid: u16,
    map_w: u32,
    map_h: u32,
    input: &crate::app::InputState,
    sf: f32,
    under_construction: bool,
) -> std::collections::BTreeMap<String, serde_json::Value> {
    if map_w == 0 {
        return std::collections::BTreeMap::new();
    }
    let viewport_w = input.screen_w / sf;
    let viewport_h = input.screen_h / sf;
    sow_core::game::BuildingKind::ALL
        .iter()
        .filter_map(|kind| {
            snapshot
                .buildings
                .iter()
                .filter(|building| {
                    building.owner_id == my_pid
                        && building.kind == *kind
                        && building.under_construction == under_construction
                })
                .min_by_key(|building| (building.active_level(), building.id))
                .and_then(|building| {
                    let x = building.tile_idx % map_w;
                    let y = building.tile_idx / map_w;
                    let footprint = sow_core::building::BuildingFootprint::at(*kind, x, y);
                    tutorial_project_tile(
                        Some(
                            (footprint.left + footprint.width as i32 / 2) as u32
                                + (footprint.top + footprint.height as i32 / 2) as u32 * map_w,
                        ),
                        map_w,
                        map_h,
                        input,
                        sf,
                    )
                    .and_then(|point| tutorial_screen_anchor(point, viewport_w, viewport_h))
                    .map(|([x, y], offscreen)| {
                        let key = match kind {
                            sow_core::game::BuildingKind::City => "City",
                            sow_core::game::BuildingKind::Farm => "Farm",
                            sow_core::game::BuildingKind::Factory => "Factory",
                            sow_core::game::BuildingKind::Tower => "Tower",
                            sow_core::game::BuildingKind::Port => "Port",
                        };
                        (
                            key.to_string(),
                            serde_json::json!({"x":x,"y":y,"offscreen":offscreen}),
                        )
                    })
                })
        })
        .collect()
}

/// Tutorial hand anchors, as tile indices cached per snapshot tick.
/// `expand` is neutral land touching the player's border (where to tap to grow);
/// `assault` is attackable land touching the player's border;
/// `target_action` is legal land toward a named faction: neutral land to approach allies,
/// or neutral/enemy land to approach or attack an enemy; `build_site` prefers visible,
/// buildable land far from both the player's nameplate and its border.
fn tutorial_guide_tiles(
    app: &mut SowApp,
    snapshot: &sow_core::protocol::SimSnapshot,
    my_pid: u16,
    target_owner: u16,
) -> (Option<u32>, Option<u32>, Option<u32>, Option<u32>) {
    let (map_w, map_h) = (app.sim.map_w, app.sim.map_h);
    if map_w == 0 || map_h == 0 {
        return (None, None, None, None);
    }
    let build_kind = app
        .ui
        .app
        .hud_state
        .building_placement
        .map(|placement| placement.kind)
        .unwrap_or(sow_core::game::BuildingKind::City);
    let Some(me) = snapshot
        .players
        .iter()
        .find(|player| player.id == my_pid && player.alive && player.tile_count > 0)
    else {
        return (None, None, None, None);
    };
    let nameplate_anchor = app
        .ui
        .nameplates
        .anchor_for(my_pid)
        .filter(|[x, y]| x.is_finite() && y.is_finite())
        .unwrap_or([me.centroid_x, me.centroid_y]);
    let nameplate_tile = (
        nameplate_anchor[0]
            .floor()
            .clamp(0.0, map_w.saturating_sub(1) as f32) as i32,
        nameplate_anchor[1]
            .floor()
            .clamp(0.0, map_h.saturating_sub(1) as f32) as i32,
    );
    let cache_hit = {
        let observation = &app.sim.tutorial_observation;
        observation.guide_tick == snapshot.tick
            && observation.guide_target_owner == target_owner
            && observation.guide_build_site_kind == Some(build_kind)
            && observation.guide_build_site_nameplate_tile == Some(nameplate_tile)
    };
    if !cache_hit {
        {
            let observation = &mut app.sim.tutorial_observation;
            observation.guide_tick = snapshot.tick;
            observation.guide_target_owner = target_owner;
            observation.guide_build_site_kind = Some(build_kind);
            observation.guide_build_site_nameplate_tile = Some(nameplate_tile);
            observation.guide_expand = None;
            observation.guide_assault = None;
            observation.guide_target_action = None;
            observation.guide_build_site_candidates.clear();
        }
        let Some(renderer) = app.gfx.map_renderer.as_ref() else {
            return (None, None, None, None);
        };
        let (owners, terrain) = (&renderer.owners, &renderer.terrain);
        let (cx, cy) = (me.centroid_x as i32, me.centroid_y as i32);
        const RADIUS: i32 = 48;
        let mut expand = None;
        'scan: for ring in 0..=RADIUS {
            for dy in -ring..=ring {
                for dx in -ring..=ring {
                    if dx.abs().max(dy.abs()) != ring {
                        continue;
                    }
                    let (col, row) = (cx + dx, cy + dy);
                    if col < 0 || row < 0 || col >= map_w as i32 || row >= map_h as i32 {
                        continue;
                    }
                    let idx = (row as u32 * map_w + col as u32) as usize;
                    if owners.get(idx).copied().unwrap_or(0) != my_pid {
                        continue;
                    }
                    let odd = (row & 1) != 0;
                    let deltas = if odd {
                        [(1, 0), (-1, 0), (0, -1), (1, -1), (0, 1), (1, 1)]
                    } else {
                        [(1, 0), (-1, 0), (-1, -1), (0, -1), (-1, 1), (0, 1)]
                    };
                    for (ndx, ndy) in deltas {
                        let (ncol, nrow) = (col + ndx, row + ndy);
                        if ncol < 0 || nrow < 0 || ncol >= map_w as i32 || nrow >= map_h as i32 {
                            continue;
                        }
                        let nidx = (nrow as u32 * map_w + ncol as u32) as usize;
                        if terrain.get(nidx).copied().unwrap_or(0) & 0x80 == 0 {
                            continue;
                        }
                        let Some(owner) = owners.get(nidx).copied() else {
                            continue;
                        };
                        if owner == 0 && expand.is_none() {
                            expand = Some(nidx as u32);
                            break 'scan;
                        }
                    }
                }
            }
        }

        let border_tiles = app
            .sim
            .engine
            .as_ref()
            .and_then(|engine| engine.state.player(my_pid))
            .map(|player| &player.border_tiles);
        let assault = border_tiles.and_then(|border_tiles| {
            tutorial_assault_tile(
                owners,
                terrain,
                map_w,
                map_h,
                border_tiles,
                my_pid,
                me,
                &snapshot.players,
            )
        });
        let target_action = border_tiles.and_then(|border_tiles| {
            tutorial_target_action_tile(
                owners,
                terrain,
                map_w,
                map_h,
                border_tiles,
                my_pid,
                target_owner,
                me,
                &snapshot.players,
            )
        });
        let radius = 48_i32.min(map_w.min(map_h) as i32);
        let map_area = map_w.checked_mul(map_h).unwrap_or(0);
        let mut build_site_candidates = Vec::new();
        let mut max_nameplate_distance = 0;
        let placement_cache = &mut app.ui.building_placement_cache;
        let mut consider_build_site = |idx: u32, is_border: bool| {
            if idx >= map_area {
                return;
            }
            let (col, row) = ((idx % map_w) as i32, (idx / map_w) as i32);
            let (dx, dy) = (col - cx, row - cy);
            let terrain_byte = terrain.get(idx as usize).copied().unwrap_or(0);
            if dx * dx + dy * dy > radius * radius
                || owners.get(idx as usize).copied() != Some(my_pid)
                || terrain_byte & 0x80 == 0
            {
                return;
            }
            let query = crate::input::placement::PlacementQuery {
                kind: build_kind,
                click_x: col,
                click_y: row,
                map_w,
                map_h,
                owners,
                terrain,
                my_id: my_pid,
                buildings: &snapshot.buildings,
            };
            if !placement_cache.is_legal_site(snapshot.tick, &query, idx) {
                return;
            }
            let nameplate_distance =
                sow_core::building::hex_distance(nameplate_tile.0, nameplate_tile.1, col, row)
                    as u32;
            max_nameplate_distance = max_nameplate_distance.max(nameplate_distance);
            build_site_candidates.push((is_border, nameplate_distance, 0, idx));
        };
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                let (col, row) = (cx + dx, cy + dy);
                if dx * dx + dy * dy > radius * radius
                    || col < 0
                    || row < 0
                    || col >= map_w as i32
                    || row >= map_h as i32
                {
                    continue;
                }
                let idx = row as u32 * map_w + col as u32;
                consider_build_site(idx, border_tiles.is_some_and(|tiles| tiles.contains(idx)));
            }
        }
        let map = app.sim.engine.as_ref().map(|engine| &engine.state.map);
        let border_distances = map
            .map(|map| {
                tutorial_border_distance_map(
                    map,
                    owners,
                    terrain,
                    border_tiles,
                    my_pid,
                    max_nameplate_distance,
                )
            })
            .unwrap_or_default();
        for candidate in &mut build_site_candidates {
            candidate.2 = border_distances
                .get(candidate.3 as usize)
                .copied()
                .unwrap_or(max_nameplate_distance);
        }
        tutorial_rank_build_site_candidates(&mut build_site_candidates);
        let observation = &mut app.sim.tutorial_observation;
        observation.guide_expand = expand;
        observation.guide_assault = assault;
        observation.guide_target_action = target_action;
        observation.guide_build_site_candidates = build_site_candidates;
    }
    let sf = (crate::web_canvas::device_pixel_ratio() as f32).max(0.01);
    let viewport_w = app.input.screen_w / sf;
    let viewport_h = app.input.screen_h / sf;
    let build_site = tutorial_visible_build_site_candidate(
        &app.sim.tutorial_observation.guide_build_site_candidates,
        |tile| {
            tutorial_project_tile(Some(tile), map_w, map_h, &app.input, sf)
                .is_some_and(|point| tutorial_screen_point_visible(point, viewport_w, viewport_h))
        },
    );
    let observation = &mut app.sim.tutorial_observation;
    (
        observation.guide_expand,
        observation.guide_assault,
        observation.guide_target_action,
        build_site,
    )
}

/// The mouse, or a single touch, drives the tutorial pointer. Other input does not.
fn tutorial_pointer_active(app: &SowApp) -> bool {
    match app.input.hover_pointer {
        HoverPointer::Mouse => true,
        HoverPointer::Touch => app.input.active_touches.len() == 1,
        HoverPointer::None => false,
    }
}

fn tutorial_target_under_pointer(app: &SowApp, marker_player_id: u16) -> bool {
    tutorial_pointer_active(app)
        && app.ui.nameplates.tutorial_target_contains(
            marker_player_id,
            app.input.last_mouse_x,
            app.input.last_mouse_y,
        )
}

/// Tutorial pointer in CSS pixels. It has its own channel, see publish_tutorial_pointer.
fn tutorial_pointer_screen(app: &SowApp) -> Option<(f64, f64)> {
    if !tutorial_pointer_active(app) {
        return None;
    }
    let sf = (crate::web_canvas::device_pixel_ratio() as f32).max(0.01);
    Some((
        app.input.last_mouse_x / f64::from(sf),
        app.input.last_mouse_y / f64::from(sf),
    ))
}

fn tutorial_payload(app: &mut SowApp, my_pid: u16) -> serde_json::Value {
    let campaign_unlocks = app
        .sim
        .engine
        .as_ref()
        .and_then(|engine| engine.campaign_unlocks.as_ref())
        .cloned()
        .unwrap_or_default();
    let Some(snapshot) = app.sim.current_snapshot.clone() else {
        return serde_json::json!({
            "active": true,
            "episode_id": app.ui.tutorial_campaign.episode_id(),
            "campaign_session_id": app.ui.campaign_session_id,
            "effective_unlocks": campaign_unlocks,
            "tick": 0
        });
    };
    let marker_player_id = app.ui.tutorial_marker_player_id.unwrap_or(my_pid);
    let (expand_tile, assault_tile, target_tile, build_site_tile) =
        tutorial_guide_tiles(app, &snapshot, my_pid, marker_player_id);
    let sf = (crate::web_canvas::device_pixel_ratio() as f32).max(0.01);
    let (map_w, map_h) = (app.sim.map_w, app.sim.map_h);
    let viewport_w = app.input.screen_w / sf;
    let viewport_h = app.input.screen_h / sf;
    let nameplate_position = app
        .ui
        .nameplates
        .anchor_for(marker_player_id)
        .map(|[x, y]| crate::render::world::overlays::world_to_screen(x, y, &app.input, sf));
    let nameplate_screen = nameplate_position
        .filter(|point| tutorial_screen_point_visible(*point, viewport_w, viewport_h))
        .map(|[x, y]| serde_json::json!({ "x": x, "y": y }))
        .unwrap_or(serde_json::Value::Null);
    let target_nameplate = nameplate_position
        .and_then(|point| tutorial_screen_anchor(point, viewport_w, viewport_h))
        .map(|([x, y], offscreen)| serde_json::json!({ "x": x, "y": y, "offscreen": offscreen }))
        .unwrap_or(serde_json::Value::Null);
    let zoom_floor = app.zoom_floor();
    let zoom_out_complete =
        app.input.camera_zoom <= zoom_floor + 0.02 && app.input.target_zoom <= zoom_floor + 0.02;
    let opening_zoom = crate::campaign::tutorial_camera_frame(
        app.ui.tutorial_campaign,
        &app.sim.config,
        app.input.screen_w,
        app.input.screen_h,
    )
    .map(|(_, zoom)| zoom.max(zoom_floor))
    .unwrap_or(app.input.target_zoom.max(zoom_floor));
    let zoom_ceiling =
        crate::camera_zoom_upper_bound(app.input.screen_w, app.input.screen_h).max(zoom_floor);
    let zoom_in_target = TUTORIAL_MAX_ZOOM.clamp(zoom_floor, zoom_ceiling);
    let zoom_in_complete =
        app.input.camera_zoom.is_finite() && app.input.camera_zoom >= zoom_in_target;
    let tutorial_target_hovered = tutorial_target_under_pointer(app, marker_player_id);
    let camera_center = if app.input.camera_zoom > 0.0 {
        Some((
            (app.input.screen_w * 0.5 - app.input.camera_x) / app.input.camera_zoom,
            (app.input.screen_h * 0.5 - app.input.camera_y) / app.input.camera_zoom,
        ))
    } else {
        None
    };
    let camera_target_distance = snapshot
        .players
        .iter()
        .find(|player| player.id == marker_player_id && player.alive && player.tile_count > 0)
        .and_then(|player| {
            camera_center.map(|(x, y)| (player.centroid_x - x).hypot(player.centroid_y - y))
        });
    let camera_target_radius =
        (app.input.screen_w.min(app.input.screen_h) / app.input.camera_zoom.max(0.01) * 0.24)
            .max(12.0);
    let camera_target_complete =
        camera_target_distance.is_some_and(|distance| distance <= camera_target_radius);
    let guide_screen = |tile: Option<u32>| {
        tutorial_project_tile(tile, map_w, map_h, &app.input, sf)
            .filter(|point| tutorial_screen_point_visible(*point, viewport_w, viewport_h))
            .map(|[x, y]| serde_json::json!({ "x": x, "y": y, "tile_idx": tile }))
            .unwrap_or(serde_json::Value::Null)
    };
    let build_site_screen = tutorial_project_tile(build_site_tile, map_w, map_h, &app.input, sf)
        .and_then(|point| tutorial_screen_anchor(point, viewport_w, viewport_h))
        .map(|([x, y], offscreen)| serde_json::json!({"x":x,"y":y,"offscreen":offscreen}))
        .unwrap_or(serde_json::Value::Null);
    let player_screen = snapshot
        .players
        .iter()
        .find(|player| player.id == marker_player_id && player.alive && player.tile_count > 0)
        .map(|player| {
            crate::render::world::overlays::world_to_screen(
                player.centroid_x,
                player.centroid_y,
                &app.input,
                sf,
            )
        })
        .and_then(|point| tutorial_screen_anchor(point, viewport_w, viewport_h))
        .map(|([x, y], offscreen)| serde_json::json!({ "x": x, "y": y, "offscreen": offscreen }))
        .unwrap_or(serde_json::Value::Null);
    let upgrade_buildings =
        tutorial_building_anchors(&snapshot, my_pid, map_w, map_h, &app.input, sf, false);
    let upgrading_buildings =
        tutorial_building_anchors(&snapshot, my_pid, map_w, map_h, &app.input, sf, true);
    let observation = &app.sim.tutorial_observation;
    let attacks_by_faction_id = observation
        .attacks_by_faction_id
        .iter()
        .map(|(faction_id, count)| (faction_id.clone(), count))
        .collect::<std::collections::BTreeMap<_, _>>();
    let transport_fleets_by_faction_id = observation
        .seen_transport_fleets_by_faction_id
        .iter()
        .map(|(faction_id, count)| (faction_id.clone(), count))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut alliance_faction_ids = observation
        .seen_alliance_faction_ids
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    alliance_faction_ids.sort_unstable();
    let me = snapshot.players.iter().find(|player| player.id == my_pid);
    let campaign_faction_ids = app
        .sim
        .engine
        .as_ref()
        .map(|engine| &engine.campaign_faction_ids);
    let players = snapshot
        .players
        .iter()
        .map(|player| {
            let mut value = player_json(player, my_pid, snapshot.total_land_tiles, None);
            if let Some(faction_id) = campaign_faction_ids.and_then(|ids| ids.get(&player.id)) {
                value["campaign_faction_id"] = serde_json::json!(faction_id);
            }
            value
        })
        .collect::<Vec<_>>();
    let fleets_by_type = observation
        .seen_fleets_by_type
        .iter()
        .map(|(kind, count)| (kind.clone(), count))
        .collect::<std::collections::BTreeMap<_, _>>();
    let support_deliveries_by_faction_id = observation
        .support_deliveries_by_faction_id
        .iter()
        .map(|(faction_id, receipt)| {
            (
                faction_id.clone(),
                serde_json::json!({
                    "deliveries": receipt.deliveries,
                    "gold": receipt.gold,
                    "troops": receipt.troops,
                    "first_tick": receipt.first_tick,
                }),
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    let resource_transfers_by_recipient_faction_id = observation
        .resource_transfers_by_recipient_faction_id
        .iter()
        .map(|(faction_id, counts)| {
            (
                faction_id.clone(),
                serde_json::json!({
                    "total": counts.total,
                    "gold": counts.gold,
                    "troops": counts.troops,
                    "gold_troops": counts.gold_troops,
                }),
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    let alliance_request_faction_ids: Vec<String> = app.sim.engine.as_ref().map(|engine| {
        engine.alliances_proposed.iter().filter(|proposal| proposal.proposer == my_pid)
            .filter_map(|proposal| engine.campaign_faction_ids.get(&proposal.target).cloned()).collect()
    }).unwrap_or_default();
    let hostile_faction_ids: Vec<String> = app.sim.engine.as_ref().map(|engine| {
        engine.campaign_relations.iter().filter(|(_, relation)| **relation == sow_core::protocol::CampaignRelation::Enemy)
            .filter_map(|(id, _)| engine.campaign_faction_ids.get(id).cloned()).collect()
    }).unwrap_or_default();
    serde_json::json!({
        "active": true,
        "episode_id": app.ui.tutorial_campaign.episode_id(),
        "campaign_session_id": app.ui.campaign_session_id,
        "effective_unlocks": campaign_unlocks,
        "tick": snapshot.tick,
        "expand": guide_screen(expand_tile),
        "assault": guide_screen(assault_tile),
        "target_action": guide_screen(target_tile),
        "build_site": build_site_screen,
        "upgrade_structure": app
            .building_panel_layout()
            .filter(|(panel, _)| panel.upgrade.is_some())
            .map(|(_, layout)| serde_json::json!({
                "x": layout.button_center[0] / sf,
                "y": layout.button_center[1] / sf,
            }))
            .unwrap_or(serde_json::Value::Null),
        "confirm_building_placement": app
            .placement_controls()
            .map(|controls| serde_json::json!({
                "x": controls.confirm[0] / sf,
                "y": controls.confirm[1] / sf,
            }))
            .unwrap_or(serde_json::Value::Null),
        "upgrade_buildings": upgrade_buildings,
        "upgrading_buildings": upgrading_buildings,
        "nameplate": nameplate_screen,
        "target_nameplate": target_nameplate,
        "marker_player_id": marker_player_id,
        "emote_token": app.ui.nameplates.campaign_emote_ready_token(),
        "player": player_screen,
        "camera": {
            "x": app.input.camera_x,
            "y": app.input.camera_y,
            "zoom": app.input.camera_zoom,
            "width": app.input.screen_w,
            "height": app.input.screen_h,
            "scale": sf,
        },
        "facts": {
            "alliance_request_faction_ids": alliance_request_faction_ids,
            "hostile_faction_ids": hostile_faction_ids,
            "alliance_requests_submitted": observation.alliance_requests_submitted,
            "tutorial_target_hovered": tutorial_target_hovered,
            "tiles": me.map(|player| player.tile_count).unwrap_or(0),
            "tiles_gained": observation.tiles_gained,
            "wilderness_orders_accepted": observation.wilderness_orders_accepted,
            "zoom_in_events": app.input.tutorial_zoom_in_events,
            "zoom_out_events": app.input.tutorial_zoom_out_events,
            "zoom_out_complete": zoom_out_complete,
            "zoom_in_complete": zoom_in_complete,
            "camera_zoom": app.input.camera_zoom,
            "camera_zoom_start": opening_zoom,
            "camera_zoom_floor": zoom_floor,
            "camera_zoom_target": zoom_in_target,
            "camera_zoom_ceiling": zoom_ceiling,
            "camera_target": camera_target_complete,
            "camera_target_distance": camera_target_distance,
            "camera_target_radius": camera_target_radius,
            "camera_drag_events": app.input.tutorial_camera_drag_events,
            "camera_key_pan_events": app.input.tutorial_camera_key_pan_events,
            "troops": me.map(|player| player.troops).unwrap_or(0.0),
            "kills": me.map(|player| player.kills).unwrap_or(0),
            "defeated": observation.seen_defeated.len(),
            "defeated_faction_ids": observation.seen_defeated_faction_ids,
            "eliminated_faction_ids": observation.eliminated_faction_ids,
            "pre_defeat_faction_ids": observation.pre_defeat_faction_ids,
            "contacts": observation.seen_contacts.len(),
            "contact_faction_ids": observation.seen_contact_faction_ids,
            "attacks": observation.seen_attacks.len(),
            "attacks_by_faction_id": attacks_by_faction_id,
            "buildings": observation.seen_structures.len(),
            "cities": observation.seen_cities.len(),
            "farms": observation.seen_buildings_by_kind.get("farms").map_or(0, |ids| ids.len()),
            "factories": observation.seen_buildings_by_kind.get("factories").map_or(0, |ids| ids.len()),
            "ports": observation.seen_buildings_by_kind.get("ports").map_or(0, |ids| ids.len()),
            "towers": observation.seen_buildings_by_kind.get("towers").map_or(0, |ids| ids.len()),
            "ally_support_deliveries": observation.ally_support_deliveries,
            "support_deliveries_by_faction_id": support_deliveries_by_faction_id,
            "structure_upgrades": observation.structure_upgrades,
            "structure_levels": observation.structure_levels,
            "faction_structure_levels": observation.faction_structure_levels,
            "machinima_active": app.machinima_camera_active(),
            "city_upgrades": observation.city_upgrades,
            "city_levels": observation.city_levels,
            "port_upgrades": observation.port_upgrades,
            "port_levels": observation.port_levels,
            "resource_transfers": observation.resource_transfers,
            "resource_transfers_by_recipient_faction_id": resource_transfers_by_recipient_faction_id,
            "alliances_formed": observation.alliances_formed,
            "alliance_faction_ids": alliance_faction_ids,
            "fleets": observation.seen_fleets.len(),
            "fleets_by_type": fleets_by_type,
            "transport_fleets_by_faction_id": transport_fleets_by_faction_id,
            "nukes": observation.seen_nukes.len(),
            "elapsed_ticks": snapshot.tick,
            "elapsed_seconds": snapshot.tick as f64 * f64::from(app.sim.config.tick_rate_ms) / 1000.0,
        },
        "players": players,
    })
}

fn player_json(
    player: &sow_core::protocol::PlayerSnapshot,
    my_pid: u16,
    total_land_tiles: u32,
    rank: Option<usize>,
) -> serde_json::Value {
    let territory_pct = (player.tile_count as f32 / total_land_tiles.max(1) as f32).clamp(0.0, 1.0);
    let mut payload = serde_json::json!({
        "id": player.id,
        "name": &player.name,
        "troops": player.troops,
        "max_troops": player.max_troops,
        "tile_count": player.tile_count,
        "centroid_x": player.centroid_x,
        "centroid_y": player.centroid_y,
        "territory_pct": territory_pct,
        "is_alive": player.alive,
        "is_me": player.id == my_pid,
        "leader": leader_id(player.leader),
        "avatar": &player.campaign_avatar,
                "team": player.team,
        "active_emote": player.active_emote,
        "disconnected": player.disconnected,
        "traitor": player.traitor,
        "kills": player.kills,
        "deaths": player.deaths,
        "assists": player.assists,
    });
    if let Some(rank) = rank {
        payload["rank"] = serde_json::json!(rank);
    }
    payload
}

fn build_leaderboard(snapshot: &sow_core::protocol::SimSnapshot, my_pid: u16) -> serde_json::Value {
    let mut players: Vec<&sow_core::protocol::PlayerSnapshot> = snapshot.players.iter().collect();
    if players.len() > LEADERBOARD_LIMIT {
        players.select_nth_unstable_by(LEADERBOARD_LIMIT - 1, leaderboard_cmp);
        players.truncate(LEADERBOARD_LIMIT);
    }
    players.sort_unstable_by(leaderboard_cmp);

    let mut included_ids: HashSet<u16> = players.iter().map(|player| player.id).collect();
    let mut human_players: Vec<&sow_core::protocol::PlayerSnapshot> = snapshot
        .players
        .iter()
        .filter(|player| {
            player.player_type == sow_core::player::PlayerType::Human
                && !included_ids.contains(&player.id)
        })
        .collect();
    human_players.sort_unstable_by(leaderboard_cmp);

    let my_player = snapshot.players.iter().find(|player| player.id == my_pid);
    let mut payload = players
        .into_iter()
        .enumerate()
        .map(|(index, player)| {
            player_json(player, my_pid, snapshot.total_land_tiles, Some(index + 1))
        })
        .collect::<Vec<_>>();

    for player in human_players {
        let rank = snapshot
            .players
            .iter()
            .filter(|other| leaderboard_cmp(other, &player) == Ordering::Less)
            .count()
            + 1;
        payload.push(player_json(
            player,
            my_pid,
            snapshot.total_land_tiles,
            Some(rank),
        ));
        included_ids.insert(player.id);
    }

    if !included_ids.contains(&my_pid) {
        if let Some(my_player) = my_player {
            let rank = snapshot
                .players
                .iter()
                .filter(|player| leaderboard_cmp(player, &my_player) == Ordering::Less)
                .count()
                + 1;
            payload.push(player_json(
                my_player,
                my_pid,
                snapshot.total_land_tiles,
                Some(rank),
            ));
        }
    }
    serde_json::Value::Array(payload)
}

fn build_hover_payload(
    snapshot: &sow_core::protocol::SimSnapshot,
    owner_id: u16,
    my_pid: u16,
) -> serde_json::Value {
    let Some(player) = snapshot.players.iter().find(|player| player.id == owner_id) else {
        return serde_json::Value::Null;
    };
    let mut cities = 0;
    let mut factories = 0;
    let mut ports = 0;
    let mut towers = 0;
    let mut farms = 0;
    for building in &snapshot.buildings {
        if building.owner_id != owner_id {
            continue;
        }
        match building.kind {
            sow_core::game::BuildingKind::City => cities += 1,
            sow_core::game::BuildingKind::Factory => factories += 1,
            sow_core::game::BuildingKind::Port => ports += 1,
            sow_core::game::BuildingKind::Tower => towers += 1,
            sow_core::game::BuildingKind::Farm => farms += 1,
        }
    }
    let mut payload = player_json(player, my_pid, snapshot.total_land_tiles, None);
    payload["cities"] = serde_json::json!(cities);
    payload["factories"] = serde_json::json!(factories);
    payload["ports"] = serde_json::json!(ports);
    payload["towers"] = serde_json::json!(towers);
    payload["farms"] = serde_json::json!(farms);
    payload
}

fn cached_leaderboard_payload(
    snapshot: &sow_core::protocol::SimSnapshot,
    my_pid: u16,
) -> serde_json::Value {
    LAST_WEB_LEADERBOARD_PAYLOAD.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some((tick, cached_pid, payload)) = cache.as_ref() {
            if *tick == snapshot.tick && *cached_pid == my_pid {
                return payload.clone();
            }
        }
        let payload = build_leaderboard(snapshot, my_pid);
        *cache = Some((snapshot.tick, my_pid, payload.clone()));
        payload
    })
}

fn cached_hover_payload(
    snapshot: &sow_core::protocol::SimSnapshot,
    hovered_tile: u32,
    owner_id: u16,
    my_pid: u16,
) -> serde_json::Value {
    if owner_id == 0 {
        return serde_json::Value::Null;
    }
    LAST_WEB_HOVER_PAYLOAD.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some((tick, cached_tile, cached_owner, cached_pid, payload)) = cache.as_ref() {
            if *tick == snapshot.tick
                && *cached_tile == hovered_tile
                && *cached_owner == owner_id
                && *cached_pid == my_pid
            {
                return payload.clone();
            }
        }
        let payload = build_hover_payload(snapshot, owner_id, my_pid);
        *cache = Some((
            snapshot.tick,
            hovered_tile,
            owner_id,
            my_pid,
            payload.clone(),
        ));
        payload
    })
}

fn cached_inbox_payload(
    snapshot: &sow_core::protocol::SimSnapshot,
    my_pid: u16,
) -> serde_json::Value {
    LAST_WEB_INBOX_PAYLOAD.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some((tick, cached_pid, payload)) = cache.as_ref() {
            if *tick == snapshot.tick && *cached_pid == my_pid {
                return payload.clone();
            }
        }
        let payload = build_inbox(snapshot, my_pid);
        *cache = Some((snapshot.tick, my_pid, payload.clone()));
        payload
    })
}

fn build_inbox(snapshot: &sow_core::protocol::SimSnapshot, my_pid: u16) -> serde_json::Value {
    let Some(me) = snapshot.players.iter().find(|player| player.id == my_pid) else {
        return serde_json::Value::Array(Vec::new());
    };
    let mut requests = Vec::new();
    for requester_id in &me.alliance_requests {
        if let Some(requester) = snapshot
            .players
            .iter()
            .find(|player| player.id == *requester_id)
        {
            requests.push(serde_json::json!({
                "kind": "alliance",
                "requester_id": requester.id,
                "name": &requester.name,
                "avatar": sow_core::player::avatar_identity(requester),
                "active": requester.alive,
            }));
        }
    }
    for request in &me.resource_requests {
        if let Some(requester) = snapshot
            .players
            .iter()
            .find(|player| player.id == request.requester)
        {
            requests.push(serde_json::json!({
                "kind": "resources",
                "requester_id": requester.id,
                "name": &requester.name,
                "avatar": sow_core::player::avatar_identity(requester),
                "active": requester.alive,
                "gold": request.gold,
                "troops": request.troops,
            }));
        }
    }
    serde_json::Value::Array(requests)
}

fn building_detail_payload(building: &sow_core::protocol::BuildingSnapshot) -> serde_json::Value {
    // The panel itself is drawn in the world; the shell only needs the kind
    // for the radial icon and the tutorial's building_selected fact.
    serde_json::json!({
        "kind": building.kind.as_str(),
        "level": building.active_level(),
    })
}

fn selected_counterattack_troops(
    available_troops: f64,
    ratio: f32,
    minimum_troops: f64,
) -> Option<f64> {
    if !ratio.is_finite() || !available_troops.is_finite() {
        return None;
    }
    let troops = available_troops.max(0.0) * f64::from(ratio.clamp(0.05, 1.0));
    (troops.is_finite() && troops >= minimum_troops).then_some(troops)
}

fn incoming_combat_troops(
    snapshot: &sow_core::protocol::SimSnapshot,
    engine: Option<&sow_core::engine::SowEngine>,
    attacker_id: u16,
    target_id: u16,
) -> f64 {
    let local_fleets: HashMap<_, _> = engine
        .into_iter()
        .flat_map(|engine| engine.fleets.iter())
        .map(|fleet| (fleet.id, fleet))
        .collect();
    let attack_troops = snapshot
        .attacks
        .iter()
        .filter(|attack| {
            attack.owner_id == attacker_id && attack.target_owner == target_id && !attack.retreating
        })
        .map(|attack| attack.troops.max(0.0))
        .sum::<f64>();
    let transport_troops = snapshot
        .fleets
        .iter()
        .filter(|fleet| {
            fleet.owner_id == attacker_id
                && fleet.unit_type == sow_core::game::UnitType::TransportShip
                && !fleet.retreating
                && local_fleets
                    .get(&fleet.id)
                    .is_some_and(|fleet| fleet.target_owner == target_id)
        })
        .map(|fleet| fleet.troops.max(0.0))
        .sum::<f64>();
    attack_troops + transport_troops
}

fn build_combat_operations_payload(
    snapshot: &sow_core::protocol::SimSnapshot,
    my_player_id: u16,
    engine: Option<&sow_core::engine::SowEngine>,
    map_width: u32,
) -> Vec<serde_json::Value> {
    let players: HashMap<_, _> = snapshot
        .players
        .iter()
        .map(|player| (player.id, player))
        .collect();
    let local_attacks: HashMap<_, _> = engine
        .into_iter()
        .flat_map(|engine| engine.attacks.iter())
        .map(|attack| (attack.id, attack))
        .collect();
    let local_fleets: HashMap<_, _> = engine
        .into_iter()
        .flat_map(|engine| engine.fleets.iter())
        .map(|fleet| (fleet.id, fleet))
        .collect();
    let player_identity = |player_id: u16| {
        players.get(&player_id).map(|player| {
            (
                player.name.as_str(),
                sow_core::player::avatar_identity(player),
            )
        })
    };
    let mut operations = Vec::new();

    for incoming in [true, false] {
        for attack in snapshot.attacks.iter().filter(|attack| {
            let is_incoming =
                attack.owner_id != my_player_id && attack.target_owner == my_player_id;
            let is_outgoing = attack.owner_id == my_player_id;
            if incoming { is_incoming } else { is_outgoing }
        }) {
            let neutral = !incoming && attack.target_owner == 0;
            let subject_id = if incoming {
                attack.owner_id
            } else {
                attack.target_owner
            };
            let (name, avatar) = if neutral {
                (None, sow_core::player::AvatarIdentity::Wilderness)
            } else {
                player_identity(subject_id)
                    .map(|(name, avatar)| (Some(name), avatar))
                    .unwrap_or((None, sow_core::player::AvatarIdentity::Fallback))
            };
            let focus = local_attacks
                .get(&attack.id)
                .filter(|attack| !attack.to_conquer.is_empty())
                .map(|attack| attack.frontier_centroid())
                .map(|(x, y)| (x + 0.5, y + 0.5))
                .filter(|(x, y)| x.is_finite() && y.is_finite())
                .or_else(|| {
                    (attack.front_cx.is_finite()
                        && attack.front_cy.is_finite()
                        && (attack.front_cx != 0.0 || attack.front_cy != 0.0))
                        .then_some((attack.front_cx + 0.5, attack.front_cy + 0.5))
                });
            operations.push(serde_json::json!({
                "kind": "attack",
                "id": attack.id,
                "direction": if incoming { "incoming" } else { "outgoing" },
                "player_id": subject_id,
                "name": name,
                "avatar": avatar,
                "neutral": neutral,
                "troops": attack.troops.max(0.0),
                "retreating": attack.retreating,
                "focus_x": focus.map(|point| point.0),
                "focus_y": focus.map(|point| point.1),
            }));
        }
    }

    for incoming in [true, false] {
        for fleet in snapshot
            .fleets
            .iter()
            .filter(|fleet| fleet.unit_type == sow_core::game::UnitType::TransportShip)
        {
            let target_owner = local_fleets.get(&fleet.id).map(|fleet| fleet.target_owner);
            let is_incoming = fleet.owner_id != my_player_id && target_owner == Some(my_player_id);
            let is_outgoing = fleet.owner_id == my_player_id;
            if (if incoming { is_incoming } else { is_outgoing }) == false {
                continue;
            }
            let neutral = !incoming && target_owner == Some(0);
            let subject_id = if incoming {
                fleet.owner_id
            } else {
                target_owner.unwrap_or(0)
            };
            let (name, avatar) = if neutral {
                (None, sow_core::player::AvatarIdentity::Wilderness)
            } else {
                player_identity(subject_id)
                    .map(|(name, avatar)| (Some(name), avatar))
                    .unwrap_or((None, sow_core::player::AvatarIdentity::Fallback))
            };
            let focus = (map_width > 0).then_some((
                (fleet.current_tile % map_width) as f32 + 0.5,
                (fleet.current_tile / map_width) as f32 + 0.5,
            ));
            operations.push(serde_json::json!({
                "kind": "fleet",
                "id": fleet.id,
                "direction": if incoming { "incoming" } else { "outgoing" },
                "player_id": subject_id,
                "name": name,
                "avatar": avatar,
                "neutral": neutral,
                "troops": fleet.troops.max(0.0),
                "eta_seconds": if is_outgoing { fleet.eta_seconds } else { None },
                "retreating": fleet.retreating,
                "focus_x": focus.map(|point| point.0),
                "focus_y": focus.map(|point| point.1),
            }));
        }
    }
    operations
}

fn build_hud_payload(app: &mut SowApp, include_leaderboard: bool) -> serde_json::Value {
    if app.ui.app.phase != crate::ClientPhase::Playing {
        return serde_json::Value::Null;
    }

    let placement_can_confirm = app.building_placement_can_confirm();
    let placement_payload = app.ui.app.hud_state.building_placement.map(|placement| {
        serde_json::json!({
            "pending": placement.pending,
            "can_confirm": placement_can_confirm,
        })
    });
    let tutorial_pid = app
        .sim
        .my_player_id
        .unwrap_or(app.ui.app.hud_state.my_player_id);
    let mut tutorial_state = if app.ui.tutorial_active && app.net.is_offline {
        tutorial_payload(app, tutorial_pid)
    } else {
        serde_json::json!({ "active": false })
    };
    let tutorial_players = tutorial_state
        .as_object_mut()
        .and_then(|tutorial| tutorial.remove("players"));
    let screen_scale = (crate::web_canvas::device_pixel_ratio() as f32).max(0.01);
    let map_menu =
        app.input
            .map_context_menu
            .map(|menu| {
                let target_player_id = app
                    .gfx
                    .map_renderer
                    .as_ref()
                    .and_then(|renderer| renderer.owners.get(menu.tile_idx as usize).copied())
                    .unwrap_or(0);
                let (target_name, target_avatar) =
                    app.sim
                        .current_snapshot
                        .as_ref()
                        .and_then(|snapshot| {
                            snapshot.players.iter().find(|player| {
                                target_player_id != 0 && player.id == target_player_id
                            })
                        })
                        .map(|player| {
                            (
                                player.name.clone(),
                                sow_core::player::avatar_identity(player),
                            )
                        })
                        .unzip();
                let alliance_state = app
                    .map_menu_alliance_state(menu.tile_idx)
                    .map(sow_core::diplomacy::AllianceActionState::as_str);
                let items = app
                    .map_menu_items(menu.tile_idx)
                    .into_iter()
                    .map(|item| {
                        serde_json::json!({
                            "action": item.action.name(),
                            "cost": item.cost,
                            "level": item.level,
                            "disabled": item.disabled,
                            "reason_key": item.reason_key,
                            "reason_level": item.reason_level,
                            "reason_seconds": item.reason_seconds,
                            "reason_troops": item.reason_troops,
                        })
                    })
                    .collect::<Vec<_>>();
                let actions = items
                    .iter()
                    .filter_map(|item| item.get("action").and_then(|action| action.as_str()))
                    .collect::<Vec<_>>();
                let building = app
                    .sim
                    .current_snapshot
                    .as_ref()
                    .and_then(|snapshot| {
                        snapshot
                            .buildings
                            .iter()
                            .find(|building| building.tile_idx == menu.tile_idx)
                    })
                    .map(building_detail_payload);
                serde_json::json!({
                    "open": true,
                    "view": match menu.view {
                        MapContextMenuView::BuildingDetails => "building_details",
                        MapContextMenuView::Radial => "radial",
                    },
                    "x": menu.x / screen_scale,
                    "y": menu.y / screen_scale,
                    "tile_idx": menu.tile_idx,
                    "target_player_id": target_player_id,
                    "session": menu.session,
                    "actions": actions,
                    "items": items,
                    "alliance_state": alliance_state,
                    "relation": app.map_menu_relation(menu.tile_idx),
                    "target_name": target_name,
                    "target_avatar": target_avatar,
                    "building": building,
                })
            })
            .unwrap_or(serde_json::Value::Null);
    let hud = &app.ui.app.hud_state;
    let my_pid = app.sim.my_player_id.unwrap_or(hud.my_player_id);
    let snapshot = app.sim.current_snapshot.as_ref();
    let snapshot_tick = snapshot.map(|snapshot| snapshot.tick).unwrap_or(0);
    let me = my_player_summary(app, snapshot_tick, my_pid);
    let operations = snapshot
        .map(|snapshot| {
            build_combat_operations_payload(
                snapshot,
                my_pid,
                app.sim.engine.as_ref(),
                app.sim.map_w,
            )
        })
        .unwrap_or_default();
    let match_over = !app.ui.is_spectating
        && snapshot.is_some_and(|snapshot| {
            snapshot.winner.is_some()
                || me.is_some_and(|player| !player.alive && player.has_spawned)
        });
    let is_winner = snapshot.is_some_and(|snapshot| {
        if let Some(team) = snapshot.winning_team {
            me.and_then(|player| player.team) == Some(team)
        } else {
            snapshot.winner == Some(my_pid)
        }
    });
    let winner_name = snapshot
        .and_then(|snapshot| {
            snapshot.winner.and_then(|winner_id| {
                snapshot
                    .players
                    .iter()
                    .find(|player| player.id == winner_id)
            })
        })
        .map(|player| player.name.clone())
        .unwrap_or_default();
    // The store is only shown after a match. Keep its catalog out of the hot HUD path,
    // and use the browser clock because SystemTime::now() panics on wasm32.
    let featured_skin = if match_over {
        let rotation_period = sow_data::commerce::rotation_period(
            (js_sys::Date::now().max(0.0) / 1000.0) as u64,
        );
        sow_data::commerce::catalog_for_profile(
            &app.progress.owned_leaders,
            &app.progress.owned_skins,
            app.progress.crowns,
            app.progress.gems,
            rotation_period,
        )
        .skins
        .into_iter()
        .find(|skin| !skin.owned)
    } else {
        None
    };
    let selected_building = hud
        .building_placement
        .map(|placement| match placement.kind {
            sow_core::game::BuildingKind::City => "City",
            sow_core::game::BuildingKind::Tower => "Tower",
            sow_core::game::BuildingKind::Factory => "Factory",
            sow_core::game::BuildingKind::Port => "Port",
            sow_core::game::BuildingKind::Farm => "Farm",
        });
    let selected_nuke = hud.selected_nuke_kind.map(|kind| match kind {
        sow_core::game::NukeKind::AtomBomb => "AtomBomb",
    });
    let costs = &hud.building_costs;
    let (port_levels, city_level, trade_ships) = snapshot
        .map(|snapshot| {
            let port_levels = snapshot
                .buildings
                .iter()
                .filter(|building| {
                    building.owner_id == my_pid
                        && building.kind == sow_core::game::BuildingKind::Port
                })
                .map(|building| u32::from(building.active_level()))
                .sum::<u32>();
            let city_level = snapshot
                .buildings
                .iter()
                .filter(|building| {
                    building.owner_id == my_pid
                        && building.kind == sow_core::game::BuildingKind::City
                })
                .map(|building| building.active_level())
                .max()
                .unwrap_or(0);
            let trade_ships = snapshot
                .fleets
                .iter()
                .filter(|fleet| {
                    fleet.owner_id == my_pid
                        && fleet.troops > 0.0
                        && fleet.unit_type == sow_core::game::UnitType::TradeShip
                })
                .count() as u32;
            (port_levels, city_level, trade_ships)
        })
        .unwrap_or_default();
    let trade_ship = sow_core::game::UnitType::TradeShip;
    let warship = sow_core::game::UnitType::Warship;
    let notifications = hud
        .hud_notifications
        .iter()
        .map(|notification| {
            let text = localized_text_payload(&notification.text);
            let players = snapshot
                .map(|snapshot| snapshot.players.as_slice())
                .unwrap_or(&[]);
            let avatars: Vec<_> = sow_core::player::notification_avatar_identities(
                players,
                notification.players,
            )
                .into_iter()
                .map(|avatar| {
                    avatar.map(|avatar| {
                        serde_json::to_value(avatar)
                            .unwrap_or_else(|_| serde_json::json!({ "kind": "fallback" }))
                    })
                })
                .collect();
            serde_json::json!({
                "id": notification.id,
                "key": text.get("key").cloned().unwrap_or(serde_json::Value::Null),
                "values": text.get("values").cloned().unwrap_or(serde_json::Value::Null),
                "avatars": avatars,
                "priority": notification.priority,
                "group": &notification.group,
                "sum_values": notification.sum_values,
                "age_ms": notification.spawned_at.elapsed().as_millis().min(u64::MAX as u128) as u64,
            })
        })
        .collect::<Vec<_>>();
    let map_feedback = hud.map_feedback.as_ref().map(|feedback| {
        let text = localized_text_payload(&feedback.text);
        serde_json::json!({
            "id": feedback.id,
            "key": text.get("key").cloned().unwrap_or(serde_json::Value::Null),
            "values": text.get("values").cloned().unwrap_or(serde_json::Value::Null),
            "x": feedback.position[0] / screen_scale,
            "y": feedback.position[1] / screen_scale,
            "age_ms": feedback.spawned_at.elapsed().as_millis().min(u64::MAX as u128) as u64,
        })
    });
    let mut payload = serde_json::json!({
        "camera_zoom_state": {
            "current": app.input.camera_zoom,
            "floor": app.zoom_floor(),
            "ceiling": crate::camera_zoom_upper_bound(app.input.screen_w, app.input.screen_h).max(app.zoom_floor()),
        },
        "gold": me.map(|player| player.gold).unwrap_or(hud.gold),
        "troops": me.map(|player| player.troops).unwrap_or(hud.troops),
        "max_troops": me.map(|player| player.max_troops).unwrap_or(hud.max_troops),
        "troop_rate": hud.troop_rate,
        "gold_rate": hud.gold_rate,
        "attack_ratio": hud.attack_ratio,
        "operations": operations,
        "spawn_timer_secs": hud.spawn_timer_secs,
        "selected_building": selected_building,
        "building_placement": placement_payload,
        "selected_warship_build": hud.selected_warship_build,
        "selected_nuke": selected_nuke,
        "fleet_panel": {
            "trade_ships": trade_ships,
            "trade_capacity": port_levels,
            "port_levels": port_levels,
            "city_level": city_level,
            "trade_required_port_level": trade_ship.required_port_level(),
            "warship_required_port_level": 1,
            "nuke_required_city_level": 1,
            "warship_cost": warship.gold_cost(),
            "nuke_cost": app.sim.config.nuke_cost,
            "nuke_available": me.map(|player| player.nuke_available).unwrap_or(false),
            "nuke_cooldown_ticks": me.map(|player| player.nuke_cooldown_ticks).unwrap_or(0),
            "military_used": me.map(|player| player.boats_in_use).unwrap_or(0),
            "military_capacity": me.map(|player| player.boat_capacity).unwrap_or(0),
        },
        "building_costs": {
            "city": costs[0],
            "tower": costs[1],
            "factory": costs[2],
            "port": costs[3],
            "farm": costs[4],
        },
        "emotes": sow_core::emote::EMOTES,
        "emote_active": me.is_some_and(|player| player.emote_active),
        "emote_pinned": me.is_some_and(|player| player.emote_pinned),
        "fps": (app.time.current_fps > 0).then_some(app.time.current_fps),
        "ping": app.net.current_ping_ms,
        "inbox_count": me.map(|player| player.inbox_count).unwrap_or(0),
        "match_over": match_over,
        "is_spectating": app.ui.is_spectating,
        "tutorial": tutorial_state,
        "notifications": notifications,
        "map_feedback": map_feedback,
        "map_menu": map_menu,
        "dev_tools": dev_tools_payload(app),
        "is_winner": is_winner,
        "winner_name": winner_name,
        "featured_skin": featured_skin.map(|skin| serde_json::json!({
            "id": skin.id,
            "name": skin.name,
            "asset_path": skin.asset_path,
            "cost_gems": skin.cost_gems,
        })).unwrap_or(serde_json::Value::Null),
        "player_leader": me.map(|player| leader_id(player.leader)),
        "player_name": snapshot
            .and_then(|snapshot| snapshot.players.iter().find(|player| player.id == my_pid))
            .map(|player| player.name.as_str())
            .unwrap_or_default(),
    });
    let endgame_stats = if match_over {
        endgame_stats_snapshot(app, my_pid).or(app.ui.manual_endgame_stats)
    } else {
        app.ui.manual_endgame_stats
    };
    if let Some([kills, assists, max_territory, gold_received, troops_received]) = endgame_stats {
        payload["player_endgame_stats"] = serde_json::json!({
            "kills": kills,
            "assists": assists,
            "max_territory": max_territory,
            "gold_received": f64::from_bits(gold_received),
            "troops_received": f64::from_bits(troops_received),
        });
    }
    if let Some(players) = tutorial_players {
        payload["players"] = players;
    }

    if let Some(snapshot) = snapshot {
        if app.ui.show_leaderboard && include_leaderboard {
            payload["leaderboard"] = cached_leaderboard_payload(snapshot, my_pid);
        }
        if hud.show_alliance_inbox {
            payload["inbox"] = cached_inbox_payload(snapshot, my_pid);
        }
    }
    append_player_dialog_payloads(&mut payload, snapshot, hud);
    if let Some(sync) = &hud.sync_state {
        payload["sync"] = serde_json::to_value(sync).unwrap_or(serde_json::Value::Null);
    }
    payload
}

fn append_player_dialog_payloads(
    payload: &mut serde_json::Value,
    snapshot: Option<&sow_core::protocol::SimSnapshot>,
    hud: &crate::ui::hud::HudState,
) {
    let players = snapshot
        .map(|snapshot| snapshot.players.as_slice())
        .unwrap_or_default();
    if let Some(target) = hud
        .show_ask_panel
        .and_then(|id| players.iter().find(|player| player.id == id))
    {
        payload["transfer"] = serde_json::json!({
            "target_id": target.id,
            "target_name": &target.name,
            "target_avatar": sow_core::player::avatar_identity(target),
            "target_alive": target.alive,
            "confirm_pending": hud.transfer_confirm_pending,
            "suggested_gold": hud.ask_gold,
            "suggested_troops": hud.ask_troops,
        });
    }
    if let Some((ally_id, _)) = hud
        .show_betrayal_warning
        .as_ref()
        .or(hud.betrayal_warning_cached.as_ref())
    {
        payload["betrayal"] = serde_json::json!({
            "ally_id": ally_id,
            "ally_name": players.iter().find(|player| player.id == *ally_id).map(|player| player.name.as_str()).unwrap_or_default(),
            "ally_avatar": sow_core::player::avatar_identity_for_player_id(players, *ally_id),
        });
    }
}

fn localized_text_payload(text: &crate::ui::UiText) -> serde_json::Value {
    let values = text
        .values
        .iter()
        .map(|(key, value)| ((*key).to_string(), serde_json::Value::String(value.clone())))
        .collect::<serde_json::Map<_, _>>();
    serde_json::json!({ "key": text.key, "values": values })
}

fn splash_job_name(job: &crate::ui::loading_screen::SplashJob) -> &'static str {
    match job {
        crate::ui::loading_screen::SplashJob::Boot => "Boot",
        crate::ui::loading_screen::SplashJob::EnterGame => "EnterGame",
        crate::ui::loading_screen::SplashJob::ExitGame => "ExitGame",
    }
}

fn leader_id(leader: sow_core::player::Leader) -> String {
    leader.wire_id().to_string()
}

fn loader_leader_id(splash: &crate::ui::loading_screen::SplashState) -> Option<String> {
    if splash.use_random_art {
        Some("random".to_string())
    } else {
        splash.loader_leader.map(leader_id)
    }
}

fn loader_info_leader_slug(
    splash: &crate::ui::loading_screen::SplashState,
    selected_leader: sow_core::player::Leader,
) -> String {
    splash.loader_leader.unwrap_or(selected_leader).id().to_string()
}

fn notice_name(notice: Option<crate::LobbyNotice>) -> Option<&'static str> {
    match notice {
        Some(crate::LobbyNotice::HostLeft) => Some("host_left"),
        Some(crate::LobbyNotice::Kicked) => Some("kicked"),
        Some(crate::LobbyNotice::Banned) => Some("banned"),
        None => None,
    }
}

fn campaign_payload(progress: &crate::player_progress::PlayerProgress) -> serde_json::Value {
    let episodes: Vec<serde_json::Value> = CampaignId::ALL
        .into_iter()
        .map(|episode| {
            let completed = episode.is_completed(progress);
            let unlocked = episode.is_unlocked(progress);
            serde_json::json!({
                "id": episode.episode_id(),
                "title": localized_text_payload(&episode.menu_title_text()),
                "subtitle": localized_text_payload(&episode.menu_subtitle_text()),
                "completed": completed,
                "unlocked": unlocked,
                "replayable": unlocked,
            })
        })
        .collect();
    let continue_episode = CampaignId::ALL
        .into_iter()
        .find(|episode| episode.is_unlocked(progress) && !episode.is_completed(progress));
    serde_json::json!({
        "tutorial_completed": CampaignId::Boudica.is_completed(progress),
        "episodes": episodes,
        "continue_episode": continue_episode.map(CampaignId::episode_id),
    })
}

/// Keep the DOM hand aligned to its Rust-projected world anchor during camera interpolation.
pub(crate) fn publish_tutorial_camera_anchor_frame(app: &SowApp) {
    let Some(player_id) = app
        .ui
        .tutorial_active
        .then(|| {
            app.ui.tutorial_marker_player_id.unwrap_or_else(|| {
                app.sim
                    .my_player_id
                    .unwrap_or(app.ui.app.hud_state.my_player_id)
            })
        })
        .filter(|_| app.net.is_offline)
    else {
        LAST_TUTORIAL_CAMERA_ANCHOR.with(|last| *last.borrow_mut() = None);
        return;
    };
    let Some([world_x, world_y]) = app.ui.nameplates.anchor_for(player_id) else {
        LAST_TUTORIAL_CAMERA_ANCHOR.with(|last| *last.borrow_mut() = None);
        return;
    };
    let scale = (crate::web_canvas::device_pixel_ratio() as f32).max(0.01);
    let viewport_w = app.input.screen_w / scale;
    let viewport_h = app.input.screen_h / scale;
    let screen =
        crate::render::world::overlays::world_to_screen(world_x, world_y, &app.input, scale);
    let Some(([x, y], _)) = tutorial_screen_anchor(screen, viewport_w, viewport_h) else {
        LAST_TUTORIAL_CAMERA_ANCHOR.with(|last| *last.borrow_mut() = None);
        return;
    };
    let key = (player_id, x.to_bits(), y.to_bits());
    if LAST_TUTORIAL_CAMERA_ANCHOR.with(|last| *last.borrow() == Some(key)) {
        return;
    }
    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(value) = js_sys::Reflect::get(
        window.as_ref(),
        &JsValue::from_str("SOW_tutorial_camera_anchor_update"),
    ) else {
        return;
    };
    let Ok(update) = value.dyn_into::<js_sys::Function>() else {
        return;
    };
    if update
        .call2(
            window.as_ref(),
            &JsValue::from_f64(f64::from(x)),
            &JsValue::from_f64(f64::from(y)),
        )
        .is_ok()
    {
        LAST_TUTORIAL_CAMERA_ANCHOR.with(|last| *last.borrow_mut() = Some(key));
    }
}

/// The hover card and the tutorial pointer each have their own channel. Both follow the mouse, so
/// folding them into the HUD payload rebuilt and re-parsed the whole HUD on every move.
fn publish_json_channel(
    last: &'static std::thread::LocalKey<RefCell<String>>,
    channel: &str,
    value: &serde_json::Value,
) {
    let Ok(serialized) = serde_json::to_string(value) else {
        return;
    };
    let unchanged = last.with(|last| last.borrow().as_str() == serialized.as_str());
    if unchanged {
        return;
    }
    last.with(|last| *last.borrow_mut() = serialized.clone());

    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(function) = js_sys::Reflect::get(window.as_ref(), &JsValue::from_str(channel)) else {
        return;
    };
    let Ok(update) = function.dyn_into::<js_sys::Function>() else {
        return;
    };
    if let Err(error) = update.call1(window.as_ref(), &JsValue::from_str(&serialized)) {
        web_sys::console::error_2(
            &JsValue::from_str(&format!("[WEB MENU] {channel} failed")),
            &error,
        );
    }
}

pub(crate) fn publish_hover_card(app: &SowApp) {
    let (hovered_tile, hovered_owner) = hovered_tile_owner(app);
    let my_pid = app.sim.my_player_id.unwrap_or(app.ui.app.hud_state.my_player_id);
    let hovered = match app.sim.current_snapshot.as_ref() {
        Some(snapshot) if hovered_owner != 0 => {
            cached_hover_payload(snapshot, hovered_tile, hovered_owner, my_pid)
        }
        _ => serde_json::Value::Null,
    };
    publish_json_channel(&LAST_HOVER_CARD, "SOW_hover_card_update", &hovered);
}

pub(crate) fn publish_tutorial_pointer(app: &SowApp) {
    let pointer = if app.ui.tutorial_active && app.net.is_offline {
        tutorial_pointer_screen(app)
    } else {
        None
    };
    let value = match pointer {
        Some((x, y)) => serde_json::json!({ "x": x, "y": y }),
        None => serde_json::Value::Null,
    };
    publish_json_channel(&LAST_TUTORIAL_POINTER, "SOW_tutorial_pointer_update", &value);
}

/// DOM never receives transient textures, map bytes, or internal auth/session material.
pub(crate) fn publish_state(app: &mut SowApp) {
    refresh_web_leaderboard_cache(app);

    let selected_leader_for_loader = app.ui.app.main_menu_state.selected_leader;
    let loader_progress = loader_progress_percent(&app.ui.app.splash_state);

    let payload = if app.ui.app.phase == crate::ClientPhase::Playing {
        LAST_MENU_INPUTS.with(|last| *last.borrow_mut() = None);
        let include_leaderboard = if app.ui.show_leaderboard {
            let now = Instant::now();
            let due = app.ui.leaderboard_publish_pending
                || app.ui.leaderboard_refresh_at.map_or(true, |last| {
                    now.duration_since(last) >= Duration::from_millis(250)
                });
            if due {
                app.ui.leaderboard_refresh_at = Some(now);
                app.ui.leaderboard_publish_pending = false;
                app.ui.leaderboard_publish_revision =
                    app.ui.leaderboard_publish_revision.wrapping_add(1);
            }
            due
        } else {
            false
        };
        let hud_key = hud_publish_key(app);
        let hud_changed = LAST_HUD_KEY.with(|last| {
            let mut last = last.borrow_mut();
            if last.as_ref() == Some(&hud_key) {
                false
            } else {
                *last = Some(hud_key);
                true
            }
        });
        if !hud_changed {
            return;
        }
        let hud_payload = build_hud_payload(app, include_leaderboard);
        serde_json::json!({
            "phase": "Playing",
            "account_id": app.progress_account_id,
            "loader_cycle_id": app.ui.app.splash_state.cycle_id,
            "loader_job": splash_job_name(&app.ui.app.splash_state.job),
            "loader_leader": loader_leader_id(&app.ui.app.splash_state),
            "loader_info_leader": loader_info_leader_slug(&app.ui.app.splash_state, selected_leader_for_loader),
            "loader_show_hero_briefing": app.ui.app.splash_state.show_hero_briefing,
            "loader_progress": loader_progress,
            "loader_done": app.ui.app.splash_state.done,
            "hud": hud_payload,
            "settings": settings_payload(app),
        })
    } else {
        LAST_HUD_KEY.with(|last| *last.borrow_mut() = None);
        LAST_MY_PLAYER.with(|cache| *cache.borrow_mut() = None);
        LAST_WEB_TOP_THREE_TICK.with(|tick| tick.set(None));
        LAST_WEB_LEADERBOARD_PAYLOAD.with(|cache| *cache.borrow_mut() = None);
        LAST_WEB_HOVER_PAYLOAD.with(|cache| *cache.borrow_mut() = None);
        LAST_HOVER_CARD.with(|last| last.borrow_mut().clear());
        LAST_TUTORIAL_POINTER.with(|last| last.borrow_mut().clear());
        LAST_WEB_INBOX_PAYLOAD.with(|cache| *cache.borrow_mut() = None);
        app.ui.leaderboard_top_three = [None; 3];
        app.ui.leaderboard_refresh_at = None;
        app.ui.leaderboard_publish_pending = false;

        let inputs = menu_inputs(app);
        if LAST_MENU_INPUTS.with(|last| last.borrow().as_ref() == Some(&inputs)) {
            return;
        }
        let payload = menu_payload(&inputs);
        LAST_MENU_INPUTS.with(|last| *last.borrow_mut() = Some(inputs));
        payload
    };

    let Ok(serialized) = serde_json::to_string(&payload) else {
        log::warn!("[WEB MENU] failed to serialize state");
        LAST_MENU_INPUTS.with(|last| *last.borrow_mut() = None);
        return;
    };

    let unchanged = LAST_PUBLISHED.with(|last| last.borrow().as_str() == serialized.as_str());
    if unchanged {
        return;
    }
    LAST_PUBLISHED.with(|last| *last.borrow_mut() = serialized.clone());

    let Some(window) = web_sys::window() else {
        return;
    };
    let js_str = JsValue::from_str(&serialized);
    let _ = js_sys::Reflect::set(
        window.as_ref(),
        &JsValue::from_str("SOW_MENU_STATE"),
        &js_str,
    );
    if let Ok(func_val) =
        js_sys::Reflect::get(window.as_ref(), &JsValue::from_str("SOW_onStateUpdate"))
    {
        if let Ok(func) = func_val.dyn_into::<js_sys::Function>() {
            if let Err(error) = func.call1(window.as_ref(), &js_str) {
                web_sys::console::error_2(
                    &JsValue::from_str("[WEB MENU] state update failed"),
                    &error,
                );
            }
        }
    }
}

/// Whole percent: the loader bar shows no finer step, and a steady value lets the dedupe
/// in `publish_state` skip the JS side while the loader creeps forward.
fn loader_progress_percent(splash: &crate::ui::loading_screen::SplashState) -> f32 {
    (splash.progress.clamp(0.0, 1.0) * 100.0).round() / 100.0
}

fn settings_payload(app: &SowApp) -> serde_json::Value {
    let settings = &app.ui.app.settings_state;
    serde_json::json!({
        "mute_all": settings.mute_all,
        "music_volume": settings.music_volume,
        "reduced_motion": settings.reduced_motion,
        "free_zoom_out": settings.free_zoom_out,
        "show_fps_ping": settings.show_fps_ping,
        "show_dev_tools": settings.show_dev_tools,
        "show_dev_tools_available": cfg!(any(feature = "dev", debug_assertions)),
    })
}

/// Every value the non-Playing menu payload reads, owned. `publish_state` compares it with the
/// last published set and skips the rebuild when nothing changed. `menu_payload` takes only this
/// struct, so no payload value can come from live state that `menu_inputs` does not capture.
#[derive(PartialEq)]
struct MenuInputs {
    phase: crate::ClientPhase,
    loader_cycle_id: u32,
    loader_job: crate::ui::loading_screen::SplashJob,
    loader_leader: Option<String>,
    loader_info_leader: String,
    loader_show_hero_briefing: bool,
    loader_progress: f32,
    loader_done: bool,
    boot_campaign: Option<String>,
    connected: bool,
    connecting: bool,
    waiting: bool,
    ranked_search_active: bool,
    ranked_search_elapsed_secs: u32,
    update_available: bool,
    match_preparing: bool,
    player_name: String,
    selected_leader: sow_core::player::Leader,
    prefer_random_leader: bool,
    show_browser: bool,
    show_create: bool,
    join_lobby_code: String,
    joined_lobby_id: Option<u64>,
    pending_lobby_id: Option<u64>,
    is_lobby_host: bool,
    my_player_id: Option<u16>,
    downloading_map: bool,
    map_download_progress: u8,
    map_download_error: bool,
    lobbies: Vec<sow_core::protocol::LobbyInfo>,
    error: Option<serde_json::Value>,
    notice: Option<crate::LobbyNotice>,
    custom_game_config: Box<sow_core::game_config::GameConfig>,
    custom_game_is_private: bool,
    custom_game_is_sp: bool,
    store_busy: bool,
    progress: crate::player_progress::PlayerProgress,
    rotation_period: u64,
    account_id: Option<String>,
    profile_ready: bool,
    name_save_pending: bool,
    quick_match_revision: u64,
    menu_guide_save_pending: bool,
    menu_return: Option<crate::app::MenuReturnContext>,
    map_catalog: Option<Vec<sow_core::maps::MapCatalogEntry>>,
    settings: serde_json::Value,
}

fn menu_inputs(app: &SowApp) -> MenuInputs {
    let state = &app.ui.app.main_menu_state;
    let splash = &app.ui.app.splash_state;
    let selected_leader = state.selected_leader;
    let visible_route = state.visible_route();
    MenuInputs {
        phase: app.ui.app.phase,
        loader_cycle_id: splash.cycle_id,
        loader_job: splash.job,
        loader_leader: loader_leader_id(splash),
        loader_info_leader: loader_info_leader_slug(splash, selected_leader),
        loader_show_hero_briefing: splash.show_hero_briefing,
        loader_progress: loader_progress_percent(splash),
        loader_done: splash.done,
        boot_campaign: app.boot_campaign_pending.clone(),
        connected: state.is_connected,
        connecting: state.is_connecting,
        waiting: state.is_waiting,
        ranked_search_active: state.ranked_search_active,
        ranked_search_elapsed_secs: state.ranked_search_elapsed_secs,
        update_available: app.ui.update_available,
        match_preparing: app
            .ui
            .app
            .hud_state
            .sync_state
            .as_ref()
            .is_some_and(|sync| sync.is_starting),
        player_name: state.player_name.clone(),
        selected_leader,
        prefer_random_leader: state.prefer_random_leader,
        show_browser: matches!(visible_route, crate::ui::main_menu::MainMenuRoute::Browser),
        show_create: matches!(visible_route, crate::ui::main_menu::MainMenuRoute::Create),
        join_lobby_code: state.join_lobby_code.clone(),
        joined_lobby_id: state.joined_lobby_id,
        pending_lobby_id: state.pending_join_lobby_id,
        is_lobby_host: state.is_lobby_host,
        my_player_id: state.my_player_id,
        downloading_map: state.is_downloading_map,
        map_download_progress: state.map_download_progress,
        map_download_error: state.map_download_error.is_some(),
        lobbies: state.lobbies.clone(),
        error: state.error_message.as_ref().map(localized_text_payload),
        notice: state.notice,
        custom_game_config: state.custom_game_config.clone(),
        custom_game_is_private: state.custom_game_is_private,
        custom_game_is_sp: state.custom_game_is_sp,
        store_busy: state.store_busy,
        progress: app.progress.clone(),
        rotation_period: sow_data::commerce::rotation_period(
            (js_sys::Date::now().max(0.0) / 1000.0) as u64,
        ),
        account_id: app.progress_account_id.clone(),
        profile_ready: app.profile_last_applied_request > 0,
        name_save_pending: app.pending_display_name.is_some(),
        quick_match_revision: app.quick_match_revision,
        menu_guide_save_pending: app.menu_guide_save_pending,
        menu_return: app.menu_return.clone(),
        map_catalog: app.ui.app.asset_loader.map_catalog.clone(),
        settings: settings_payload(app),
    }
}

fn menu_payload(inputs: &MenuInputs) -> serde_json::Value {
    let progress = &inputs.progress;
    let store_catalog = sow_data::commerce::catalog_for_profile(
        &progress.owned_leaders,
        &progress.owned_skins,
        progress.crowns,
        progress.gems,
        inputs.rotation_period,
    );
    let leaders: Vec<serde_json::Value> = sow_core::player::Leader::all()
        .map(|leader| {
            let slug = leader.id();
            let offer = store_catalog
                .leaders
                .iter()
                .find(|offer| offer.id.as_str() == slug)
                .expect("leader catalog must contain every leader");
            serde_json::json!({
                "id": leader_id(leader),
                "name": leader.display_name(),
                "civilization": leader.civilization(),
                "civilization_key": format!("heroes.leader_{slug}_civilization"),
                "perk_key": format!("site.leader_{slug}_description"),
                "region": leader.character().region,
                "slug": slug,
                "art_slug": leader.art_slug(),
                "free_rotation": offer.free_rotation,
                "permanently_free": offer.permanently_free,
                "owned": offer.owned,
                "available": offer.available,
                "cost_crowns": offer.cost_crowns,
                "cost_gems": offer.cost_gems,
            })
        })
        .collect();
    let map_catalog: Vec<serde_json::Value> = inputs
        .map_catalog
        .as_ref()
        .map(|entries| {
            entries
                .iter()
                .map(|entry| {
                    serde_json::json!({
                        "key": &entry.key,
                        "display_name": &entry.display_name,
                        "width": entry.width,
                        "height": entry.height,
                        "default_roster": &entry.default_roster,
                        "roster_presets": &entry.roster_presets,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    serde_json::json!({
        "phase": phase_name(inputs.phase),
        "loader_cycle_id": inputs.loader_cycle_id,
        "loader_job": splash_job_name(&inputs.loader_job),
        "loader_leader": inputs.loader_leader,
        "loader_info_leader": inputs.loader_info_leader,
        "loader_show_hero_briefing": inputs.loader_show_hero_briefing,
        "loader_progress": inputs.loader_progress,
        "loader_done": inputs.loader_done,
        "boot_campaign": inputs.boot_campaign,
        "connected": inputs.connected,
        "connecting": inputs.connecting,
        "update_available": inputs.update_available,
        "waiting": inputs.waiting,
        "ranked_search_active": inputs.ranked_search_active,
        "ranked_search_elapsed_secs": inputs.ranked_search_elapsed_secs,
        "match_preparing": inputs.match_preparing,
        "player_name": inputs.player_name,
        "selected_leader": leader_id(inputs.selected_leader),
        "selected_leader_name": inputs.selected_leader.display_name(),
        "selected_civilization": inputs.selected_leader.civilization(),
        "prefer_random_leader": inputs.prefer_random_leader,
        "show_browser": inputs.show_browser,
        "show_create": inputs.show_create,
        "join_lobby_code": inputs.join_lobby_code,
        "joined_lobby_id": inputs.joined_lobby_id,
        "pending_lobby_id": inputs.pending_lobby_id,
        "is_lobby_host": inputs.is_lobby_host,
        "my_player_id": inputs.my_player_id,
        "downloading_map": inputs.downloading_map,
        "map_download_progress": inputs.map_download_progress,
        "map_download_error": inputs.map_download_error,
        "lobbies": inputs.lobbies,
        "error": inputs.error,
        "notice": notice_name(inputs.notice),
        "level": progress.level,
        "xp": progress.xp,
        "crowns": progress.crowns,
        "laurels": progress.laurels,
        "gems": progress.gems,
        "reward_receipts": progress.reward_receipts.values().collect::<Vec<_>>(),
        "menu_return": inputs.menu_return,
        "profile_ready": inputs.profile_ready,
        "menu_guides": progress.menu_guides,
        "name_save_pending": inputs.name_save_pending,
        "quick_match_revision": inputs.quick_match_revision,
        "menu_guide_save_pending": inputs.menu_guide_save_pending,
        "achievements": progress.unlocked_achievements,
        "campaign": campaign_payload(progress),
        "selected_skin": progress.selected_skin,
        "store_busy": inputs.store_busy,
        "store": store_catalog,
        "native_purchase_scheme": "sow://purchase",
        "native_restore_scheme": "sow://restore",
        "account_id": inputs.account_id,
        "profile_stats": {
            "wins": progress.wins,
            "matches_played": progress.matches_played,
            "players_defeated": progress.players_defeated,
            "empires_defeated": progress.empires_defeated,
            "tribes_defeated": progress.tribes_defeated,
            "kills": progress.kills,
            "deaths": progress.deaths,
            "assists": progress.assists,
            "leader_xp": progress.leader_xp,
        },
        "leaders": leaders,
        "map_catalog": map_catalog,
        "custom_game_config": inputs.custom_game_config,
        "bot_limits": {
            "tribes": sow_core::game_config::GameConfig::max_tribe_bots(),
            "nations": sow_core::game_config::GameConfig::max_nation_bots(),
        },
        "custom_game_is_private": inputs.custom_game_is_private,
        "custom_game_is_sp": inputs.custom_game_is_sp,
        "hud": serde_json::Value::Null,
        "settings": inputs.settings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sow_core::game::GamePhase;
    use sow_core::player::{Leader, PlayerType};
    use sow_core::protocol::{PlayerSnapshot, SimSnapshot};
    use std::sync::Arc;

    #[wasm_bindgen_test::wasm_bindgen_test]
    fn loader_art_random_keeps_the_real_leader_and_campaign_reset() {
        let mut splash = crate::ui::loading_screen::SplashState::default();
        assert_eq!(loader_leader_id(&splash), None);
        assert_eq!(loader_info_leader_slug(&splash, Leader::of("caesar")), "caesar");
        splash.transition_anim(
            crate::ui::loading_screen::SplashJob::EnterGame,
            Leader::of("caesar"),
        );
        assert_eq!(loader_leader_id(&splash).as_deref(), Some("Caesar"));

        splash.use_random_art = true;
        assert_eq!(loader_leader_id(&splash).as_deref(), Some("random"));
        assert_eq!(loader_info_leader_slug(&splash, Leader::of("boudica")), "caesar");
        assert_eq!(splash.loader_leader, Some(Leader::of("caesar")));

        splash.transition_anim(
            crate::ui::loading_screen::SplashJob::EnterGame,
            Leader::of("boudica"),
        );
        assert_eq!(loader_leader_id(&splash).as_deref(), Some("Boudica"));
        assert_eq!(loader_info_leader_slug(&splash, Leader::of("caesar")), "boudica");
        assert_eq!(splash.loader_leader, Some(Leader::of("boudica")));
    }

    fn test_player(id: u16, tile_count: u32, troops: f64) -> PlayerSnapshot {
        PlayerSnapshot {
            id,
            name: format!("Player {id}"),
            troops,
            max_troops: 100.0,
            gold: 100.0,
            tile_count,
            centroid_x: 0.0,
            centroid_y: 0.0,
            player_type: PlayerType::Bot,
            color: [0.5; 3],
            team: None,
            has_spawned: true,
            alive: true,
            iq: 100,
            alliances: Vec::new(),
            alliance_timers: std::collections::HashMap::new(),
            alliance_requests: Vec::new(),
            resource_requests: Vec::new(),
            disconnected: false,
            active_emote: None,
            can_express_emote: false,
            can_retaliate: false,
            traitor: false,
            leader: Leader::of("caesar"),
            empowered_attack: false,
            campaign_avatar: None,
            is_campaign_faction: false,
            skin_style: 0,
            kills: 0,
            deaths: 0,
            assists: 0,
            boats_in_use: 0,
            boat_capacity: 3,
            nuke_available: false,
            nuke_cooldown_ticks: 0,
        }
    }

    #[test]
    fn dev_numeric_inputs_can_extend_past_slider_ranges() {
        assert_eq!(
            decode_dev_config_value("water.depth_cap", &serde_json::json!(64.0)).unwrap()[0],
            64.0
        );
        assert_eq!(
            decode_dev_config_value("lighting.direction", &serde_json::json!([8.0, -8.0, 3.0]))
                .unwrap(),
            [8.0, -8.0, 3.0, 0.0]
        );
        assert_eq!(
            decode_dev_config_value(
                "lighting.specular.color",
                &serde_json::json!([8.0, 2.0, 1.0, 3.0])
            )
            .unwrap(),
            [8.0, 2.0, 1.0, 1.0]
        );
    }

    #[test]
    fn tutorial_build_site_prefers_clearance_from_nameplate_and_border() {
        let mut candidates = vec![
            (true, 8, 0, 1),
            (false, 1, 6, 9),
            (false, 6, 1, 8),
            (false, 4, 4, 5),
            (false, 4, 4, 2),
        ];
        tutorial_rank_build_site_candidates(&mut candidates);

        assert_eq!(candidates[0], (false, 4, 4, 2));
        assert_eq!(candidates[1], (false, 4, 4, 5));
        assert_eq!(candidates.last(), Some(&(true, 8, 0, 1)));
        assert_eq!(
            tutorial_visible_build_site_candidate(&candidates, |_| true),
            Some(2)
        );
    }

    #[test]
    fn tutorial_build_site_prefers_interior_before_border_fallback() {
        let mut candidates = vec![(true, 20, 0, 1), (false, 2, 1, 9), (false, 6, 1, 8)];
        tutorial_rank_build_site_candidates(&mut candidates);

        let ranked_tiles = candidates
            .iter()
            .map(|candidate| candidate.3)
            .collect::<Vec<_>>();
        assert_eq!(ranked_tiles, [8, 9, 1]);
    }

    #[test]
    fn tutorial_build_site_border_clearance_uses_owned_land_neighbors() {
        let owners = vec![1; 25];
        let terrain = vec![0x80; 25];
        let mut border = sow_core::bitset::DenseBitSet::new();
        border.insert(0);

        let map = sow_core::map::GameMap::new(5, 5);
        let distances = tutorial_border_distance_map(&map, &owners, &terrain, Some(&border), 1, 3);

        assert_eq!(distances[0], 0);
        assert_eq!(distances[6], 1);
        assert_eq!(distances[12], 2);
        assert_eq!(
            distances[24], 3,
            "distance beyond the relevant range is capped"
        );
    }

    #[test]
    fn tutorial_build_site_tracks_camera_changes_while_paused_with_border_fallback() {
        let candidates = [(false, 1, 4, 10), (false, 2, 4, 11), (true, 0, 0, 12)];

        // Reuse the same tick's candidates as the camera moves between publishes.
        assert_eq!(
            tutorial_visible_build_site_candidate(&candidates, |tile| tile == 11 || tile == 12),
            Some(11)
        );
        assert_eq!(
            tutorial_visible_build_site_candidate(&candidates, |tile| tile == 12),
            Some(12)
        );
        assert_eq!(
            tutorial_visible_build_site_candidate(&candidates, |tile| tile == 10),
            Some(10)
        );
    }

    #[test]
    fn tutorial_target_action_guides_neutral_land_toward_an_ally() {
        use sow_core::protocol::Team;

        let mut me = test_player(1, 1, 500.0);
        me.centroid_x = 1.0;
        me.centroid_y = 2.0;
        me.team = Some(Team::Red);
        let mut ally = test_player(2, 1, 500.0);
        ally.centroid_x = 4.0;
        ally.centroid_y = 2.0;
        ally.team = Some(Team::Red);

        let mut owners = vec![0; 25];
        owners[11] = me.id;
        owners[14] = ally.id;
        let terrain = vec![0x80; 25];
        let mut border = sow_core::bitset::DenseBitSet::new();
        border.insert(11);

        assert_eq!(
            tutorial_target_action_tile(
                &owners,
                &terrain,
                5,
                5,
                &border,
                me.id,
                ally.id,
                &me,
                &[me.clone(), ally],
            ),
            Some(12)
        );
    }

    fn test_snapshot(players: Vec<PlayerSnapshot>) -> SimSnapshot {
        SimSnapshot {
            tick: 1,
            phase: GamePhase::Playing,
            spawn_timer_secs: None,
            players,
            dirty_tiles: Vec::new(),
            fleets: Vec::new(),
            attacks: Vec::new(),
            buildings: Vec::new(),
            projectiles: Vec::new(),
            nuke_alerts: Vec::new(),
            resource_transfers: Vec::new(),
            resource_rejections: Vec::new(),
            resource_transfer_rejections: Vec::new(),
            winner: None,
            winning_team: None,
            defense_posts: Vec::new(),
            defense_dirty: false,
            total_land_tiles: 10_000,
            sea_lanes: Arc::new(Vec::new()),
            debug_mem_info: String::new(),
        }
    }

    #[wasm_bindgen_test::wasm_bindgen_test]
    fn inbox_reuses_typed_identity_for_both_received_request_kinds() {
        use sow_core::protocol::ResourceRequest;

        let mut me = test_player(1, 5, 100.0);
        me.alliance_requests = (2..=7).chain([99]).collect();
        me.resource_requests = me
            .alliance_requests
            .iter()
            .map(|&requester| ResourceRequest {
                requester,
                gold: 25.0,
                troops: 10.0,
            })
            .collect();
        let mut human = test_player(2, 5, 100.0);
        human.player_type = PlayerType::Human;
        let bot = test_player(3, 5, 100.0);
        let mut nation = test_player(4, 5, 100.0);
        nation.player_type = PlayerType::Nation;
        let mut campaign = test_player(5, 5, 100.0);
        campaign.campaign_avatar = Some("boudica".into());
        let mut fallback = test_player(6, 5, 100.0);
        fallback.campaign_avatar = Some("null".into());
        let mut eliminated = test_player(7, 5, 100.0);
        eliminated.alive = false;
        let snapshot = test_snapshot(vec![me, human, bot, nation, campaign, fallback, eliminated]);
        let payload = build_inbox(&snapshot, 1);
        let rows = payload.as_array().unwrap();
        assert_eq!(rows.len(), 12);
        for requester in &snapshot.players[1..] {
            for kind in ["alliance", "resources"] {
                let row = rows
                    .iter()
                    .find(|row| row["kind"] == kind && row["requester_id"] == requester.id)
                    .unwrap();
                assert_eq!(
                    row["avatar"],
                    serde_json::to_value(sow_core::player::avatar_identity(requester)).unwrap()
                );
                assert_eq!(row["name"], requester.name);
                assert_eq!(row["active"], requester.alive);
                assert!(row.get("leader").is_none());
                if kind == "resources" {
                    assert_eq!(row["gold"], 25.0);
                    assert_eq!(row["troops"], 10.0);
                }
            }
        }
        assert!(build_inbox(&snapshot, 99).as_array().unwrap().is_empty());
        assert!(build_inbox(&snapshot, 2).as_array().unwrap().is_empty());
    }

    #[wasm_bindgen_test::wasm_bindgen_test]
    fn player_dialogs_use_snapshot_identity_and_keep_pending_intents_unchanged() {
        let mut players = vec![
            test_player(1, 5, 100.0),
            test_player(2, 5, 100.0),
            test_player(3, 5, 100.0),
            test_player(4, 5, 100.0),
            test_player(5, 5, 100.0),
        ];
        players[0].player_type = PlayerType::Human;
        players[1].player_type = PlayerType::Nation;
        players[2].campaign_avatar = Some("boudica".into());
        players[3].campaign_avatar = Some("null".into());
        players[4].alive = false;
        let snapshot = test_snapshot(players);
        for player in &snapshot.players {
            let intent =
                sow_core::protocol::GameplayIntent::Attack(sow_core::protocol::AttackIntent {
                    target_owner: player.id,
                    troops: Some(37.0),
                });
            let mut hud = crate::ui::hud::HudState::default();
            hud.show_ask_panel = Some(player.id);
            hud.ask_gold = 25.0;
            hud.ask_troops = 12.0;
            hud.betrayal_warning_cached = Some((player.id, intent.clone()));
            let mut payload = serde_json::json!({});
            append_player_dialog_payloads(&mut payload, Some(&snapshot), &hud);
            let identity = serde_json::to_value(sow_core::player::avatar_identity(player)).unwrap();
            assert_eq!(payload["transfer"]["target_avatar"], identity);
            assert_eq!(payload["betrayal"]["ally_avatar"], identity);
            assert_eq!(payload["transfer"]["target_name"], player.name);
            assert_eq!(payload["transfer"]["target_alive"], player.alive);
            assert_eq!(payload["betrayal"]["ally_name"], player.name);
            assert_eq!(payload["transfer"]["suggested_gold"], 25.0);
            assert_eq!(payload["transfer"]["suggested_troops"], 12.0);
            assert_eq!(hud.betrayal_warning_cached.as_ref().unwrap().1, intent);
            hud.show_ask_panel = None;
            hud.betrayal_warning_cached = None;
            let mut closed = serde_json::json!({});
            append_player_dialog_payloads(&mut closed, Some(&snapshot), &hud);
            assert!(closed.get("transfer").is_none() && closed.get("betrayal").is_none());
        }
        let mut hud = crate::ui::hud::HudState::default();
        hud.show_ask_panel = Some(99);
        hud.show_betrayal_warning = Some((
            99,
            sow_core::protocol::GameplayIntent::BreakAlliance { target_player: 99 },
        ));
        for missing in [None, Some(&snapshot)] {
            let mut payload = serde_json::json!({});
            append_player_dialog_payloads(&mut payload, missing, &hud);
            assert!(payload.get("transfer").is_none());
            assert_eq!(payload["betrayal"]["ally_name"], "");
            assert_eq!(payload["betrayal"]["ally_avatar"]["kind"], "fallback");
        }
    }

    #[test]
    fn counterattack_uses_the_selected_troop_ratio_without_changing_attack_amount() {
        assert_eq!(selected_counterattack_troops(100.0, 0.5, 1.0), Some(50.0));
        assert_eq!(selected_counterattack_troops(100.0, 1.0, 1.0), Some(100.0));
        assert_eq!(selected_counterattack_troops(1.0, 0.5, 1.0), None);
        assert_eq!(selected_counterattack_troops(f64::NAN, 0.5, 1.0), None);
    }

    #[test]
    fn counterattack_threat_includes_only_live_attacks_and_incoming_troop_transports() {
        use sow_core::game::{GameState, UnitType};
        use sow_core::game_config::GameConfig;
        use sow_core::protocol::{AttackSnapshot, FleetSnapshot};
        use sow_core::warp_fleet::WarpFleet;
        use sow_core::water_components::WaterComponents;

        let mut snapshot = test_snapshot(vec![test_player(1, 5, 100.0), test_player(2, 5, 100.0)]);
        snapshot.attacks = vec![
            AttackSnapshot {
                id: 1,
                owner_id: 2,
                target_owner: 1,
                troops: 25.0,
                retreating: false,
                front_cx: 1.0,
                front_cy: 1.0,
            },
            AttackSnapshot {
                id: 2,
                owner_id: 2,
                target_owner: 1,
                troops: 35.0,
                retreating: true,
                front_cx: 1.0,
                front_cy: 1.0,
            },
        ];
        let make_snapshot = |id, unit_type| FleetSnapshot {
            id,
            owner_id: 2,
            unit_type,
            troops: 40.0,
            current_tile: 4,
            path: Arc::new(vec![4]),
            path_cursor: 0,
            movement_progress: 0.0,
            eta_seconds: None,
            retreating: false,
        };
        snapshot.fleets = vec![
            make_snapshot(10, UnitType::TransportShip),
            make_snapshot(11, UnitType::TransportShip),
            make_snapshot(12, UnitType::Warship),
        ];
        let state = GameState::new(1, 4, 4, GameConfig::default());
        let water = WaterComponents::compute(&state.map, |_| {});
        let mut engine = sow_core::engine::SowEngine::new(state, water);
        engine.add_fleet(WarpFleet::new(
            10,
            2,
            1,
            UnitType::TransportShip,
            40.0,
            (0, 4),
            vec![4],
        ));
        engine.add_fleet(WarpFleet::new(
            11,
            2,
            0,
            UnitType::TransportShip,
            40.0,
            (0, 4),
            vec![4],
        ));
        engine.add_fleet(WarpFleet::new(
            12,
            2,
            1,
            UnitType::Warship,
            40.0,
            (0, 4),
            vec![4],
        ));

        assert_eq!(incoming_combat_troops(&snapshot, Some(&engine), 2, 1), 65.0);
    }

    #[test]
    fn combat_operations_include_neutral_and_pvp_attacks_but_not_other_players() {
        use sow_core::protocol::AttackSnapshot;

        let mut snapshot = test_snapshot(vec![
            test_player(1, 5, 100.0),
            test_player(2, 5, 100.0),
            test_player(3, 5, 100.0),
        ]);
        snapshot.attacks = vec![
            AttackSnapshot {
                id: 1,
                owner_id: 1,
                target_owner: 0,
                troops: 25.0,
                retreating: false,
                front_cx: 4.0,
                front_cy: 3.0,
            },
            AttackSnapshot {
                id: 2,
                owner_id: 1,
                target_owner: 2,
                troops: 50.0,
                retreating: false,
                front_cx: 7.0,
                front_cy: 8.0,
            },
            AttackSnapshot {
                id: 3,
                owner_id: 2,
                target_owner: 1,
                troops: 75.0,
                retreating: false,
                front_cx: 10.0,
                front_cy: 11.0,
            },
            AttackSnapshot {
                id: 4,
                owner_id: 2,
                target_owner: 3,
                troops: 90.0,
                retreating: false,
                front_cx: 12.0,
                front_cy: 13.0,
            },
        ];

        let operations = build_combat_operations_payload(&snapshot, 1, None, 20);
        assert_eq!(operations.len(), 3);
        let neutral = operations.iter().find(|row| row["id"] == 1).unwrap();
        assert_eq!(neutral["direction"], "outgoing");
        assert_eq!(neutral["neutral"], true);
        let threat = operations.iter().find(|row| row["id"] == 3).unwrap();
        assert_eq!(threat["direction"], "incoming");
        assert_eq!(threat["player_id"], 2);
        assert!(operations.iter().any(|row| row["id"] == 2));
        assert!(!operations.iter().any(|row| row["id"] == 4));
    }

    #[test]
    fn combat_operation_snapshot_tick_tracks_add_update_remove_without_unrelated_state() {
        use sow_core::protocol::AttackSnapshot;

        let mut snapshot = test_snapshot(Vec::new());
        assert_eq!(combat_operations_snapshot_tick(&snapshot, 1, None), 0);

        snapshot.attacks.push(AttackSnapshot {
            id: 1,
            owner_id: 1,
            target_owner: 0,
            troops: 25.0,
            retreating: false,
            front_cx: 4.0,
            front_cy: 3.0,
        });
        assert_eq!(
            combat_operations_snapshot_tick(&snapshot, 1, None),
            snapshot.tick
        );

        snapshot.tick += 1;
        assert_eq!(
            combat_operations_snapshot_tick(&snapshot, 1, None),
            snapshot.tick
        );

        snapshot.attacks.clear();
        snapshot.attacks.push(AttackSnapshot {
            id: 2,
            owner_id: 2,
            target_owner: 3,
            troops: 25.0,
            retreating: false,
            front_cx: 4.0,
            front_cy: 3.0,
        });
        assert_eq!(combat_operations_snapshot_tick(&snapshot, 1, None), 0);
    }

    #[test]
    fn combat_operations_keep_typed_player_avatars_and_mark_wilderness() {
        use sow_core::protocol::AttackSnapshot;

        let mut human = test_player(1, 5, 100.0);
        human.player_type = PlayerType::Human;
        human.leader = Leader::of("caesar");
        let bot = test_player(2, 5, 100.0);
        let mut nation = test_player(3, 5, 100.0);
        nation.player_type = PlayerType::Nation;
        let mut snapshot = test_snapshot(vec![human, bot, nation]);
        let attack = |id, owner_id, target_owner| AttackSnapshot {
            id,
            owner_id,
            target_owner,
            troops: 25.0,
            retreating: false,
            front_cx: 4.0,
            front_cy: 3.0,
        };
        snapshot.attacks = vec![
            attack(1, 4, 0),
            attack(2, 1, 4),
            attack(3, 2, 4),
            attack(4, 3, 4),
            attack(5, 9, 4),
        ];

        let operations = build_combat_operations_payload(&snapshot, 4, None, 20);
        assert_eq!(operations.len(), 5);
        assert_eq!(
            operations.iter().find(|row| row["id"] == 1).unwrap()["avatar"],
            serde_json::json!({ "kind": "wilderness" })
        );
        let human_avatar = &operations.iter().find(|row| row["id"] == 2).unwrap()["avatar"];
        assert_eq!(human_avatar["kind"], "portrait");
        assert_eq!(human_avatar["slug"], "caesar");
        // Bots without a catalog portrait show the shared fallback, never an emoji.
        let bot_avatar = &operations.iter().find(|row| row["id"] == 3).unwrap()["avatar"];
        assert_eq!(bot_avatar["kind"], "fallback");
        assert_eq!(
            operations.iter().find(|row| row["id"] == 4).unwrap()["avatar"]["kind"],
            "fallback"
        );
        assert_eq!(
            operations.iter().find(|row| row["id"] == 5).unwrap()["avatar"],
            serde_json::json!({ "kind": "fallback" })
        );
    }

    #[test]
    fn combat_operations_join_only_troop_transports_to_local_targets() {
        use sow_core::game::{GameState, UnitType};
        use sow_core::game_config::GameConfig;
        use sow_core::protocol::FleetSnapshot;
        use sow_core::warp_fleet::WarpFleet;
        use sow_core::water_components::WaterComponents;

        let mut snapshot = test_snapshot(vec![test_player(1, 5, 100.0), test_player(2, 5, 100.0)]);
        let make_snapshot = |id, owner_id, unit_type| FleetSnapshot {
            id,
            owner_id,
            unit_type,
            troops: 40.0,
            current_tile: 6,
            path: Arc::new(vec![6]),
            path_cursor: 0,
            movement_progress: 0.0,
            eta_seconds: None,
            retreating: false,
        };
        snapshot.fleets = vec![
            make_snapshot(10, 1, UnitType::TransportShip),
            make_snapshot(11, 2, UnitType::TransportShip),
            make_snapshot(12, 1, UnitType::TradeShip),
            make_snapshot(13, 1, UnitType::Warship),
        ];
        snapshot.fleets[0].eta_seconds = Some(3.25);

        let state = GameState::new(1, 5, 5, GameConfig::default());
        let water = WaterComponents::compute(&state.map, |_| {});
        let mut engine = sow_core::engine::SowEngine::new(state, water);
        engine.add_fleet(WarpFleet::new(
            10,
            1,
            0,
            UnitType::TransportShip,
            40.0,
            (0, 6),
            vec![6],
        ));
        engine.add_fleet(WarpFleet::new(
            11,
            2,
            1,
            UnitType::TransportShip,
            40.0,
            (0, 6),
            vec![6],
        ));
        engine.add_fleet(WarpFleet::new(
            12,
            1,
            2,
            UnitType::TradeShip,
            40.0,
            (0, 6),
            vec![6],
        ));
        engine.add_fleet(WarpFleet::new(
            13,
            1,
            2,
            UnitType::Warship,
            40.0,
            (0, 6),
            vec![6],
        ));

        let operations = build_combat_operations_payload(&snapshot, 1, Some(&engine), 5);
        assert_eq!(operations.len(), 2);
        assert_eq!(
            combat_operations_snapshot_tick(&snapshot, 1, Some(&engine)),
            snapshot.tick
        );
        assert_eq!(operations[0]["direction"], "incoming");
        assert_eq!(operations[0]["kind"], "fleet");
        assert_eq!(operations[0]["id"], 11);
        assert_eq!(operations[1]["direction"], "outgoing");
        assert_eq!(operations[1]["id"], 10);
        assert_eq!(operations[1]["neutral"], true);
        assert_eq!(operations[1]["eta_seconds"], 3.25);
        assert!(operations[0]["eta_seconds"].is_null());

        snapshot.fleets[0].retreating = true;
        snapshot.fleets[0].eta_seconds = Some(1.25);
        let returning = build_combat_operations_payload(&snapshot, 1, Some(&engine), 5);
        assert_eq!(returning[1]["eta_seconds"], 1.25);
        snapshot.fleets.clear();
        assert_eq!(
            combat_operations_snapshot_tick(&snapshot, 1, Some(&engine)),
            0
        );
    }

    #[test]
    fn optimized_top_three_matches_the_full_order() {
        let players = vec![
            test_player(3, 10, 5.0),
            test_player(2, 10, 5.0),
            test_player(1, 10, 6.0),
            test_player(0, 10, 5.0),
            test_player(4, 9, 100.0),
        ];
        let mut full: Vec<&PlayerSnapshot> = players.iter().collect();
        full.sort_unstable_by(leaderboard_cmp);
        let expected: Vec<u16> = full.iter().take(3).map(|player| player.id).collect();

        let mut optimized: Vec<&PlayerSnapshot> = players.iter().collect();
        optimized.select_nth_unstable_by(2, leaderboard_cmp);
        optimized.truncate(3);
        optimized.sort_unstable_by(leaderboard_cmp);
        assert_eq!(
            optimized.iter().map(|player| player.id).collect::<Vec<_>>(),
            expected
        );
    }

    #[test]
    fn leaderboard_has_at_most_100_rows_plus_the_local_player() {
        let snapshot = test_snapshot(
            (0..150)
                .map(|id| test_player(id, 150 - id as u32, 0.0))
                .collect(),
        );
        let serde_json::Value::Array(rows) = build_leaderboard(&snapshot, 149) else {
            panic!("leaderboard must be an array");
        };

        assert_eq!(rows.len(), 101);
        assert_eq!(
            rows.iter()
                .filter_map(|row| row.get("id").and_then(serde_json::Value::as_u64))
                .collect::<std::collections::HashSet<_>>()
                .len(),
            101
        );
        assert_eq!(rows[0]["rank"], serde_json::json!(1));
        assert_eq!(rows[99]["rank"], serde_json::json!(100));
        assert_eq!(rows[100]["id"], serde_json::json!(149));
        assert_eq!(rows[100]["rank"], serde_json::json!(150));
    }

    #[test]
    fn leaderboard_includes_humans_outside_the_top_100() {
        let mut players: Vec<PlayerSnapshot> = (0..150)
            .map(|id| test_player(id, 150 - id as u32, 0.0))
            .collect();
        players[140].player_type = PlayerType::Human;
        players[141].player_type = PlayerType::Human;
        let snapshot = test_snapshot(players);

        let serde_json::Value::Array(rows) = build_leaderboard(&snapshot, 0) else {
            panic!("leaderboard must be an array");
        };

        let human_rows: Vec<&serde_json::Value> = rows
            .iter()
            .filter(|row| {
                matches!(
                    row.get("id").and_then(serde_json::Value::as_u64),
                    Some(140 | 141)
                )
            })
            .collect();
        assert_eq!(human_rows.len(), 2);
        assert_eq!(human_rows[0]["rank"], serde_json::json!(141));
        assert_eq!(human_rows[1]["rank"], serde_json::json!(142));
    }
}
