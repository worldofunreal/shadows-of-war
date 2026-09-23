use web_time::{Duration, Instant};

/// Subset of [`sow_core::protocol::ProjectileSnapshot`] for detonation / launch detection.
#[derive(Clone, Copy, Debug)]
pub struct TrackedProjectile {
    pub kind: sow_core::game::ProjectileKind,
    pub dst_tile: u32,
    pub path_cursor: usize,
    pub steps_per_tick: u8,
    pub path_len: usize,
}

impl TrackedProjectile {
    pub fn from_snapshot(proj: &sow_core::protocol::ProjectileSnapshot) -> Self {
        Self {
            kind: proj.kind,
            dst_tile: proj.dst_tile,
            path_cursor: proj.path_cursor,
            steps_per_tick: proj.steps_per_tick,
            path_len: proj.path.len(),
        }
    }

    pub fn at_path_end(&self) -> bool {
        self.path_cursor + self.steps_per_tick as usize >= self.path_len
    }
}

pub struct GraphicsState {
    pub window: Option<Box<dyn winit::window::Window>>,
    pub surface: Option<blade_graphics::Surface>,
    pub render_ctx: Option<crate::render::gpu::RenderContext>,
    pub map_renderer: Option<crate::render::gpu::MapRenderer>,
    pub mover_renderer: Option<crate::render::gpu::MoverRenderer>,
    pub text_renderer: Option<crate::render::gpu::TextRenderer>,
    pub prev_sync_point: Option<blade_graphics::SyncPoint>,
    pub needs_first_upload: bool,
    pub configured_physical: winit::dpi::PhysicalSize<u32>,
}

/// One-shot timings for a multiplayer match handoff and loader. These events are
/// intentionally client-local: they do not alter the wire protocol or gate
/// relay admission.
/// The timestamps let us separate relay connection time from local engine/GPU work.
#[derive(Default)]
pub struct LoadTelemetry {
    pub started_at: Option<web_time::Instant>,
    pub relay_connected_at: Option<web_time::Instant>,
    pub engine_init_complete_at: Option<web_time::Instant>,
    pub gpu_upload_complete_at: Option<web_time::Instant>,
    pub snapshot_available_at: Option<web_time::Instant>,
    pub ready_sent_at: Option<web_time::Instant>,
}

#[derive(Default)]
pub struct TutorialObservation {
    pub seen_attacks: std::collections::HashSet<u64>,
    pub seen_fleets: std::collections::HashSet<u64>,
    pub seen_structures: std::collections::HashSet<u64>,
    pub seen_defeated: std::collections::HashSet<u16>,
    pub seen_defeated_names: std::collections::HashSet<String>,
    pub seen_contacts: std::collections::HashSet<u16>,
    pub seen_nukes: std::collections::HashSet<(u32, u32, u16)>,
}

impl TutorialObservation {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

impl LoadTelemetry {
    pub fn reset_for_relay_handoff(&mut self) {
        *self = Self::default();
        let now = web_time::Instant::now();
        self.started_at = Some(now);
        Self::log_phase("relay_connect_start", None, now);
    }

    pub fn mark_relay_connected(&mut self) {
        self.mark_once("relay_connect_complete", |s| &mut s.relay_connected_at);
    }

    pub fn mark_engine_init_complete(&mut self) {
        self.mark_once("engine_init_complete", |s| &mut s.engine_init_complete_at);
    }

    pub fn mark_gpu_upload_complete(&mut self) {
        self.mark_once("gpu_upload_complete", |s| &mut s.gpu_upload_complete_at);
    }

    pub fn mark_snapshot_available(&mut self) {
        self.mark_once("snapshot_available", |s| &mut s.snapshot_available_at);
    }

    pub fn mark_ready_sent(&mut self) {
        self.mark_once("ready_sent", |s| &mut s.ready_sent_at);
    }

    fn mark_once<F>(&mut self, phase: &'static str, slot: F)
    where
        F: FnOnce(&mut Self) -> &mut Option<web_time::Instant>,
    {
        let target = slot(self);
        if target.is_some() {
            return;
        }
        let now = web_time::Instant::now();
        *target = Some(now);
        Self::log_phase(phase, self.started_at, now);
        crate::analytics::track_with("load_stage", serde_json::json!({ "stage": phase }));
    }

    fn log_phase(
        phase: &'static str,
        started_at: Option<web_time::Instant>,
        now: web_time::Instant,
    ) {
        let elapsed_ms = started_at
            .map(|start| now.duration_since(start).as_millis())
            .unwrap_or(0);
        log::info!(
            "[CLIENT TELEMETRY] phase={} elapsed_ms={}",
            phase,
            elapsed_ms
        );
    }
}

pub struct NetState {
    pub client: Option<sow_net::client::SowClient>,
    pub connect_tx: WakeSender<Result<sow_net::client::SowClient, String>>,
    pub connect_rx: crossbeam_channel::Receiver<Result<sow_net::client::SowClient, String>>,
    pub ws_url: String,
    pub orchestrator_url: String,
    pub is_offline: bool,
    pub ws_connect_fail_backoff_ms: u64,
    pub ws_connect_not_before: web_time::Instant,
    pub ws_reconnect_after_resume: bool,
    pub pending_lobby_rejoin: bool,
    pub current_ping_ms: Option<u32>,
    pub last_ping_time: web_time::Instant,
    pub relay_connect_start: Option<web_time::Instant>,
    pub relay_retry_count: u32,
    pub load_telemetry: LoadTelemetry,
    /// Set when the orchestrator handed us off to a game relay (Start carried
    /// relay_host). The loader uses this instead of string-matching ws_url.
    pub relay_handoff_done: bool,
}

pub struct SimState {
    pub engine: Option<sow_core::engine::SowEngine>,
    pub current_snapshot: Option<sow_core::protocol::SimSnapshot>,
    pub turn_queue: std::collections::VecDeque<sow_core::protocol::Turn>,
    pub my_player_id: Option<u16>,
    pub my_lobby_id: Option<u64>,
    /// Short-lived capability used only for the direct relay Ready frame.
    pub relay_ticket: Option<String>,
    /// Rotating capability returned by the relay for the next reconnect.
    pub relay_reconnect_ticket: Option<String>,
    pub map_w: u32,
    pub map_h: u32,
    pub offline_tick_timer: f32,
    /// Wall-clock anchor for offline sim pacing (decoupled from render interp).
    pub offline_last_update: web_time::Instant,
    pub offline_intents: Vec<sow_core::protocol::GameplayIntent>,
    pub last_synced_cost_tick: Option<u64>,
    pub tile_upgrades: Vec<u32>,
    pub config: sow_core::game_config::GameConfig,
    pub paused: bool,
    pub tutorial_observation: TutorialObservation,
    pub fog_explored: sow_core::bitset::DenseBitSet,
    pub fog_visible: sow_core::bitset::DenseBitSet,
    pub force_fog_upload: bool,
}

pub struct InputState {
    pub camera_x: f32,
    pub camera_y: f32,
    pub camera_zoom: f32,
    pub target_zoom: f32,
    pub screen_w: f32,
    pub screen_h: f32,
    pub dragging: bool,
    pub last_mouse_x: f64,
    pub last_mouse_y: f64,
    pub hover_pointer: HoverPointer,
    pub active_touches: std::collections::HashMap<u64, (f64, f64)>,
    pub map_pointer_start: Option<MapPointerStart>,
    pub last_pinch_state: Option<(f64, f64, f64)>,
    pub map_context_menu: Option<MapContextMenu>,
    pub map_context_menu_session: u64,
    /// Hold-to-build
    pub hold_build_active: bool,
    pub hold_build_accum: f32,
    pub has_snapped_camera_to_spawn: bool,
    pub selected_warships: Vec<u64>,
    pub key_pan_up: bool,
    pub key_pan_down: bool,
    pub key_pan_left: bool,
    pub key_pan_right: bool,
    pub camera_focus_target: Option<(f32, f32)>,
    pub input_focused: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HoverPointer {
    None,
    Mouse,
    Touch,
}

pub struct MapPointerStart {
    pub started_at: web_time::Instant,
    pub x: f64,
    pub y: f64,
    pub is_touch: bool,
    pub attack_sent: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct MapContextMenu {
    pub x: f32,
    pub y: f32,
    pub tile_idx: u32,
    pub session: u64,
}

#[derive(Clone, Debug)]
pub struct FalloutZone {
    pub x: f32,
    pub y: f32,
    pub radius: f32,
    pub start_time: web_time::Instant,
}

#[derive(Clone, Debug)]
pub struct ClickMarker {
    pub world_x: f32,
    pub world_y: f32,
    pub start_time: web_time::Instant,
}

#[derive(Clone, Debug)]
pub struct FloatingNotice {
    pub text: String,
    pub world_x: f32,
    pub world_y: f32,
    pub start_time: web_time::Instant,
    pub duration: web_time::Duration,
    pub color: [f32; 4],
}

#[derive(Clone, Debug)]
pub struct AttackBadgeLabel {
    pub troops: f64,
    pub text: String,
    pub width: f32,
    pub height: f32,
    pub last_update: web_time::Instant,
}

#[derive(Clone, Debug)]
pub struct NameplateVisualState {
    pub from_center: [f32; 2],
    pub to_center: [f32; 2],
    pub from_size: f32,
    pub to_size: f32,
    pub source_name: String,
    pub player_type: sow_core::player::PlayerType,
    pub display_name: String,
    pub troops_bits: u64,
    pub troops_text: String,
    pub name_measure_unit: [f32; 2],
    pub troops_measure_unit: [f32; 2],
    pub metrics_style_key: Option<[u32; 2]>,
}

pub struct UiState {
    pub app: crate::ClientApp,
    /// True during an offline scripted tutorial or campaign match.
    pub tutorial_active: bool,
    /// Which scripted campaign the running tutorial match belongs to.
    pub tutorial_campaign: crate::campaign::CampaignId,
    pub show_leaderboard: bool,
    pub leaderboard_top_three: [Option<u16>; 3],
    pub leaderboard_refresh_at: Option<web_time::Instant>,
    pub leaderboard_publish_pending: bool,
    pub leaderboard_publish_revision: u64,
    pub show_dev_sidebar: bool,
    pub update_available: bool,
    pub is_spectating: bool,
    /// Cached once per turn from the snapshot: true when my player is dead or otherwise observing.
    pub observing: bool,
    pub fallout_zones: Vec<FalloutZone>,
    pub last_projectiles: std::collections::HashMap<u64, TrackedProjectile>,
    pub last_projectile_snapshot_tick: Option<u64>,
    pub detonation_scratch: Vec<(f32, f32, sow_core::game::ProjectileKind)>,
    /// Cached endgame copy for panel fade-out (is_victory, title, subtitle).
    pub endgame_cache: Option<(bool, String, String)>,
    /// Deterministic reward preview for the current match, cached when the
    /// outcome is first observed so the endgame panel does not double-award.
    pub reward_cache: Option<sow_data::rewards::MatchReward>,

    /// Client-side nuke silo cooldown tracking: building id → tick when ready.
    pub silo_cooldowns: std::collections::HashMap<u64, u64>,
    pub mover_scene: crate::render::world::movers::MoverScene,
    pub click_markers: Vec<ClickMarker>,
    pub floating_notices: Vec<FloatingNotice>,
    pub attack_badge_labels: std::collections::HashMap<u64, AttackBadgeLabel>,
    pub attack_badge_style_key: Option<[u32; 2]>,
    pub attack_badge_cache_tick: Option<u64>,
    pub attack_badge_active_ids: std::collections::HashSet<u64>,
    pub nameplate_sample_tick: Option<u64>,
    pub nameplate_sample_at: Option<web_time::Instant>,
    pub nameplate_order_my_id: Option<u16>,
    pub nameplate_order: Vec<usize>,
    pub nameplate_visuals: std::collections::HashMap<u16, NameplateVisualState>,
    pub(crate) building_render_cache: crate::render::world::BuildingRenderCache,
    pub last_resource_notice_tick: Option<u64>,
    pub border_flashes: Vec<BorderFlashInstance>,
    pub border_flash_intensities: std::collections::HashMap<u16, f32>,
    pub placement_scratch: Vec<(i32, i32, f32, i32)>,
    pub last_player_attack_flash_time: std::collections::HashMap<u16, web_time::Instant>,
    /// Screen vignette alert state.
    pub viewport_alert: Option<ViewportAlertState>,
}

#[derive(Clone, Debug)]
pub struct BorderFlashInstance {
    pub player_id: u16,
    pub start_time: web_time::Instant,
    pub max_intensity: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ViewportAlertKind {
    UnderAttack,
    AllianceRequest,
    Betrayal,
    ConquerPlayer,
    Victory,
    Defeat,
}

pub struct ViewportAlertState {
    pub kind: ViewportAlertKind,
    pub start_time: web_time::Instant,
}

/// Shared flash duration for all one-shot visual feedback (border flash + vignette).
pub const FLASH_DURATION: f32 = 1.0;

/// Ease-out quadratic: returns 1→0 over `FLASH_DURATION`, or `None` if expired.
#[inline]
pub fn easeout_flash(elapsed: f32) -> Option<f32> {
    if elapsed >= FLASH_DURATION {
        None
    } else {
        let t = 1.0 - elapsed / FLASH_DURATION;
        Some(t * t)
    }
}

impl UiState {
    pub(crate) fn trigger_viewport_alert(&mut self, kind: ViewportAlertKind) {
        let priority = |k: ViewportAlertKind| match k {
            ViewportAlertKind::Victory | ViewportAlertKind::Defeat => 4,
            ViewportAlertKind::Betrayal => 3,
            ViewportAlertKind::ConquerPlayer => 2,
            ViewportAlertKind::AllianceRequest => 1,
            ViewportAlertKind::UnderAttack => 0,
        };

        let should_replace = if let Some(ref current) = self.viewport_alert {
            if current.kind == kind {
                // Do not reset the timer for persistent alerts that are already active
                kind != ViewportAlertKind::UnderAttack
                    && kind != ViewportAlertKind::Victory
                    && kind != ViewportAlertKind::Defeat
            } else {
                priority(kind) >= priority(current.kind)
            }
        } else {
            true
        };

        if should_replace {
            self.viewport_alert = Some(ViewportAlertState {
                kind,
                start_time: web_time::Instant::now(),
            });
        }
    }
}

/// Wall-clock anchor for render-behind-by-one-tick interpolation between sim snapshots.
pub struct InterpClock {
    pub last_applied_at: web_time::Instant,
    pub tick_dur: Duration,
}

impl InterpClock {
    #[inline]
    pub fn alpha(&self, now: Instant) -> f32 {
        let elapsed = now.duration_since(self.last_applied_at).as_secs_f32();
        let dur = self.tick_dur.as_secs_f32().max(0.001);
        let t = (elapsed / dur).clamp(0.0, 1.0);
        // Smoothstep keeps fleet/nuke overlays visually consistent.
        t * t * (3.0 - 2.0 * t)
    }

    #[inline]
    pub fn linear_alpha(&self, now: Instant) -> f32 {
        let elapsed = now.duration_since(self.last_applied_at).as_secs_f32();
        let dur = self.tick_dur.as_secs_f32().max(0.001);
        (elapsed / dur).clamp(0.0, 1.0)
    }

    pub fn stamp_applied(&mut self, now: Instant) {
        self.last_applied_at = now;
    }

    pub fn set_tick_dur_ms(&mut self, tick_rate_ms: f32) {
        self.tick_dur = Duration::from_secs_f32((tick_rate_ms / 1000.0).max(0.001));
    }
}

pub struct TimeState {
    pub interp: InterpClock,
    pub start_time: web_time::Instant,
    pub frame_count: u32,
    pub last_fps_time: web_time::Instant,
    pub current_fps: u32,
    pub last_frame_time: web_time::Instant,
    pub last_debug_print: Option<web_time::Instant>,
    pub turn_queue_peak: usize,
}

/// Cross-thread task results wake the single UI event loop after enqueueing.
/// This keeps idle menu phases asleep without delaying network or asset results.
pub struct WakeSender<T> {
    sender: crossbeam_channel::Sender<T>,
}

impl<T> Clone for WakeSender<T> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
        }
    }
}

impl<T> WakeSender<T> {
    pub fn new(sender: crossbeam_channel::Sender<T>) -> Self {
        Self { sender }
    }

    pub fn send(&self, value: T) -> Result<(), crossbeam_channel::SendError<T>> {
        let result = self.sender.send(value);
        if result.is_ok() {
            crate::web_menu::wake_event_loop();
        }
        result
    }
}

pub struct TaskState {
    pub map_tx: WakeSender<crate::MapDownloadEvent>,
    pub map_rx: crossbeam_channel::Receiver<crate::MapDownloadEvent>,
    pub engine_init_tx: WakeSender<crate::EngineInitEvent>,
    pub engine_init_rx: crossbeam_channel::Receiver<crate::EngineInitEvent>,
    pub db_tx: WakeSender<crate::player_progress::DbEvent>,
    pub db_rx: crossbeam_channel::Receiver<crate::player_progress::DbEvent>,
    pub pending_engine_init_data: Option<(
        sow_core::game::GameState,
        sow_core::water_components::WaterComponents,
        sow_core::protocol::ServerStartMessage,
    )>,
    pub engine_init_queued_msg: Option<sow_core::protocol::ServerStartMessage>,
}

pub struct SowApp {
    pub gfx: GraphicsState,
    pub net: NetState,
    pub sim: SimState,
    pub input: InputState,
    pub ui: UiState,
    pub(crate) sfx: crate::app::sfx::SfxDirector,
    pub time: TimeState,
    pub tasks: TaskState,

    #[cfg(target_arch = "wasm32")]
    pub wasm_doc_was_visible: bool,
    pub asset_config: crate::AssetConfig,
    #[cfg(target_arch = "wasm32")]
    pub(crate) web_loader_hidden: bool,
    #[cfg(target_arch = "wasm32")]
    pub(crate) web_exit_lobbies_ready: bool,
    /// Set when Blade/Vulkan init fails; event loop exits on next tick.
    pub gpu_init_failed: bool,
    pub progress: crate::player_progress::PlayerProgress,
    pub progress_account_id: Option<String>,
    pub profile_account_id: Option<String>,
    pub progress_provider: String,
    /// Last requested name, kept until the canonical profile acknowledges it.
    pub pending_display_name: Option<String>,
    pub display_name_save_request_id: Option<u64>,
    /// Prevents a profile refresh from racing an in-flight rename.
    pub profile_request_in_flight: bool,
    pub profile_refresh_pending: bool,
    /// Monotonic identity request sequence; used to reject stale async responses.
    pub identity_request_seq: u64,
    pub profile_last_applied_request: u64,
    /// A Join request held until the canonical account ID is available.
    pub join_waiting_for_identity: bool,
    /// A queued join with no target/config enters the server matchmaking pool.
    pub join_matchmaking: bool,
    pub progress_match_recorded: bool,
    pub progress_stats_submitted: bool,
    pub progress_result_submitted: bool,
    pub progress_session_defeats: crate::player_progress::SessionDefeats,
    #[cfg(target_arch = "wasm32")]
    pub boot_db_settled: bool,
    pub boot_campaign_pending: Option<String>,
}
