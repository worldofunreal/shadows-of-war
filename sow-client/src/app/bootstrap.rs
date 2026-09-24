use super::state::*;
use crate::ClientApp;
use crate::render::gpu::{MapRenderer, RenderContext};
use crate::{EngineInitEvent, MapDownloadEvent, spawn_sow_client_connect};
use blade_graphics as gpu;
use sow_core::protocol::SimSnapshot;
use sow_net::client::SowClient;
use std::collections::HashMap;
use web_time::{Duration, Instant};

impl Default for SowApp {
    fn default() -> Self {
        Self::new()
    }
}

impl SowApp {
    pub fn new() -> Self {
        // ── Simulation ──────────────────────────────────────────────────────────
        let map_w: u32 = 800;
        let map_h: u32 = 600;

        let engine: Option<sow_core::engine::SowEngine> = None;
        // Sim stays idle until a real `SimCommand::Init` (EnterGame or ExitGame cleanup).
        // Eager Init here duplicated the whole map sim at startup and doubled worker snapshots.

        let current_snapshot: Option<SimSnapshot> = None;

        // ── Renderer ────────────────────────────────────────────────────────────
        let render_ctx: Option<RenderContext> = None;
        let surface: Option<gpu::Surface> = None;
        let map_renderer: Option<MapRenderer> = None;
        let mover_renderer: Option<crate::render::gpu::MoverRenderer> = None;
        let text_renderer: Option<crate::render::gpu::TextRenderer> = None;
        let window: Option<Box<dyn winit::window::Window>> = None;

        // ── UI State ────────────────────────────────────────────────────────────
        let asset_config = crate::AssetConfig::resolve();
        crate::analytics::configure(&asset_config.database_base);
        crate::analytics::track("boot_start");
        let mut app = ClientApp::new();
        crate::map_cache::hydrate_asset_maps(&mut app.asset_loader.maps);
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(window) = web_sys::window() {
                if let Ok(Some(mql)) = window.match_media("(prefers-reduced-motion: reduce)") {
                    if mql.matches() {
                        app.settings_state.reduced_motion = true;
                    }
                }
            }
        }

        // ── Network State ───────────────────────────────────────────────────────
        let net_client: Option<SowClient> = None;
        let turn_queue = std::collections::VecDeque::new();
        let my_player_id: Option<u16> = None;
        let my_lobby_id: Option<u64> = None;
        let (map_tx_raw, map_rx) = crossbeam_channel::unbounded::<MapDownloadEvent>();
        let map_tx = WakeSender::new(map_tx_raw);
        let (db_tx_raw, db_rx) = crossbeam_channel::unbounded::<crate::player_progress::DbEvent>();
        let db_tx = WakeSender::new(db_tx_raw);
        type EngineInitData = (
            sow_core::game::GameState,
            sow_core::water_components::WaterComponents,
            sow_core::protocol::ServerStartMessage,
        );
        let (engine_init_tx_raw, engine_init_rx) =
            crossbeam_channel::unbounded::<EngineInitEvent>();
        let engine_init_tx = WakeSender::new(engine_init_tx_raw);
        let pending_engine_init_data: Option<EngineInitData> = None;
        let engine_init_queued_msg: Option<sow_core::protocol::ServerStartMessage> = None;

        let stored_account_id = crate::anonymous_identity::load_account_id();
        let has_stored_account = stored_account_id.is_some();
        let pending_reward_receipt_ids = stored_account_id
            .as_deref()
            .map(crate::anonymous_identity::load_pending_reward_receipt_ids)
            .unwrap_or_default();
        let pending_display_name = match crate::anonymous_identity::load_pending_display_name() {
            Some((pending_account_id, name))
                if pending_account_id.is_none()
                    || pending_account_id.as_deref() == stored_account_id.as_deref() =>
            {
                Some(name)
            }
            Some(_) => None,
            None => None,
        };
        if let Some(name) = pending_display_name.as_deref() {
            app.main_menu_state.player_name = name.to_string();
        }
        let (connect_tx_raw, connect_rx) = crossbeam_channel::unbounded();
        let connect_tx = WakeSender::new(connect_tx_raw);

        // Reconnect scheduling (idle drop / resume / failed handshake).
        let ws_connect_fail_backoff_ms: u64 = 400;
        let ws_connect_not_before: Instant = Instant::now();
        let ws_reconnect_after_resume: bool = false;
        #[cfg(target_arch = "wasm32")]
        let wasm_doc_was_visible: bool = true;
        #[cfg(target_arch = "wasm32")]
        let web_loader_hidden: bool = false;

        // Strict endpoint config: SOW_WS_URL must be declared by the shell
        // the browser shell. No default, no deriving from
        // window.location — a guessed host once pointed at the wrong origin.
        let ws_url = crate::asset_config::require_endpoint("SOW_WS_URL");
        app.main_menu_state.server_address = ws_url.clone();
        let orchestrator_url = ws_url.clone();

        #[cfg(target_arch = "wasm32")]
        {
            let identity = crate::store_portals::load_identity(&app.main_menu_state.player_name);
            if let Some(url) = identity.avatar_url {
                app.asset_loader.queue_portal_avatar(url);
            }
            if let Some(id) = crate::store_portals::take_pending_invite_lobby() {
                app.main_menu_state.pending_join_lobby_id = Some(id);
                app.main_menu_state.is_waiting = true;
            }
            if crate::store_portals::take_host_private_pending() {
                app.main_menu_state.host_private_pending = true;
                app.main_menu_state.is_waiting = true;
            }
        }

        log::info!("Auto-connecting to {}...", ws_url);
        app.main_menu_state.is_connecting = true;
        spawn_sow_client_connect(ws_url.clone(), &connect_tx);

        // ── Camera state ────────────────────────────────────────────────────────
        let camera_zoom: f32 = 0.5;
        let camera_x: f32 = 1280.0 * 0.5 - (map_w as f32 * 0.5) * camera_zoom;
        let camera_y: f32 = 720.0 * 0.5 - (map_h as f32 * 0.5) * camera_zoom;
        let screen_w: f32 = 1280.0;
        let screen_h: f32 = 720.0;

        // Mouse drag state
        let dragging = false;
        let last_mouse_x: f64 = 0.0;
        let last_mouse_y: f64 = 0.0;

        // Touch state for pinch-to-zoom
        let active_touches: HashMap<u64, (f64, f64)> = HashMap::new();
        let map_pointer_start: Option<MapPointerStart> = None;
        let last_pinch_state: Option<(f64, f64, f64)> = None;
        let map_context_menu: Option<MapContextMenu> = None;
        let map_context_menu_session = 0;

        let has_snapped_camera_to_spawn = false;

        let prev_sync_point: Option<gpu::SyncPoint> = None;
        let start_time = Instant::now();
        let interp = InterpClock {
            last_applied_at: start_time,
            tick_dur: Duration::from_millis(100),
        };
        let needs_first_upload = true;

        let frame_count = 0;
        let last_fps_time = Instant::now();
        let current_fps = 0;
        let current_ping_ms: Option<u32> = None;
        let last_ping_time = Instant::now();
        let last_frame_time = Instant::now();

        let sow_app = Self {
            gfx: GraphicsState {
                window,
                surface,
                render_ctx,
                map_renderer,
                mover_renderer,
                text_renderer,
                prev_sync_point,
                needs_first_upload,
                configured_physical: winit::dpi::PhysicalSize::new(0, 0),
            },
            net: NetState {
                client: net_client,
                connect_tx,
                connect_rx,
                ws_url,
                orchestrator_url,
                is_offline: false,
                ws_connect_fail_backoff_ms,
                ws_connect_not_before,
                ws_reconnect_after_resume,
                pending_lobby_rejoin: false,
                current_ping_ms,
                last_ping_time,
                relay_connect_start: None,
                relay_retry_count: 0,
                load_telemetry: LoadTelemetry::default(),
                relay_handoff_done: false,
            },
            sim: SimState {
                engine,
                current_snapshot,
                turn_queue,
                my_player_id,
                my_lobby_id,
                relay_ticket: None,
                relay_reconnect_ticket: None,
                map_w,
                map_h,
                offline_tick_timer: 0.0,
                offline_last_update: web_time::Instant::now(),
                offline_intents: Vec::new(),
                last_synced_cost_tick: None,
                tile_upgrades: Vec::new(),
                config: sow_core::game_config::GameConfig::default(),
                paused: false,
                tutorial_observation: crate::app::TutorialObservation::default(),
                fog_explored: sow_core::bitset::DenseBitSet::new(),
                fog_visible: sow_core::bitset::DenseBitSet::new(),
                force_fog_upload: true,
            },
            input: InputState {
                camera_x,
                camera_y,
                camera_zoom,
                target_zoom: camera_zoom,
                screen_w,
                screen_h,
                dragging,
                last_mouse_x,
                last_mouse_y,
                hover_pointer: HoverPointer::None,
                active_touches,
                map_pointer_start,
                last_pinch_state,
                map_context_menu,
                map_context_menu_session,
                hold_build_active: false,
                hold_build_accum: 0.0,
                has_snapped_camera_to_spawn,
                selected_warships: Vec::new(),
                key_pan_up: false,
                key_pan_down: false,
                key_pan_left: false,
                key_pan_right: false,
                camera_focus_target: None,
                input_focused: false,
            },
            ui: UiState {
                app,
                tutorial_active: false,
                tutorial_campaign: crate::campaign::CampaignId::Boudica,
                show_leaderboard: false,
                leaderboard_top_three: [None; 3],
                leaderboard_refresh_at: None,
                leaderboard_publish_pending: false,
                leaderboard_publish_revision: 0,
                show_dev_sidebar: false,
                update_available: false,
                is_spectating: false,
                observing: false,
                fallout_zones: Vec::new(),
                last_projectiles: std::collections::HashMap::new(),
                last_projectile_snapshot_tick: None,
                detonation_scratch: Vec::new(),
                endgame_cache: None,
                silo_cooldowns: std::collections::HashMap::new(),
                mover_scene: crate::render::world::movers::MoverScene::new(),
                click_markers: Vec::new(),
                floating_notices: Vec::new(),
                death_nameplates: Vec::with_capacity(crate::app::MAX_DEATH_NAMEPLATES),
                attack_badge_labels: std::collections::HashMap::new(),
                attack_badge_style_key: None,
                attack_badge_cache_tick: None,
                attack_badge_active_ids: std::collections::HashSet::new(),
                nameplate_sample_tick: None,
                nameplate_sample_at: None,
                nameplate_order_my_id: None,
                nameplate_order: Vec::new(),
                nameplate_visuals: std::collections::HashMap::new(),
                building_render_cache: crate::render::world::BuildingRenderCache::default(),
                last_resource_notice_tick: None,
                border_flashes: Vec::new(),
                border_flash_intensities: std::collections::HashMap::new(),
                placement_scratch: Vec::new(),
                last_player_attack_flash_time: std::collections::HashMap::new(),
                viewport_alert: None,
            },
            sfx: crate::app::sfx::SfxDirector::default(),
            time: TimeState {
                interp,
                start_time,
                frame_count,
                last_fps_time,
                current_fps,
                last_frame_time,
                last_debug_print: None,
                turn_queue_peak: 0,
            },
            tasks: TaskState {
                map_tx,
                map_rx,
                engine_init_tx,
                engine_init_rx,
                db_tx,
                db_rx,
                pending_engine_init_data,
                engine_init_queued_msg,
            },
            asset_config,
            #[cfg(target_arch = "wasm32")]
            wasm_doc_was_visible,
            #[cfg(target_arch = "wasm32")]
            web_loader_hidden,
            #[cfg(target_arch = "wasm32")]
            web_exit_lobbies_ready: false,
            gpu_init_failed: false,
            progress: crate::player_progress::PlayerProgress::default(),
            exit_reward_preview: None,
            progress_account_id: stored_account_id,
            profile_account_id: None,
            progress_provider: if has_stored_account {
                String::from("anonymous")
            } else {
                String::from("local")
            },
            pending_display_name,
            display_name_save_request_id: None,
            profile_request_in_flight: false,
            profile_refresh_pending: false,
            pending_reward_receipt_ids,
            reward_profile_retry_at: None,
            identity_request_seq: 0,
            profile_last_applied_request: 0,
            join_waiting_for_identity: false,
            join_matchmaking: false,
            progress_match_recorded: false,
            progress_session_defeats: crate::player_progress::SessionDefeats::default(),
            #[cfg(target_arch = "wasm32")]
            boot_db_settled: false,
            boot_campaign_pending: None,
        };
        let mut sow_app = sow_app;
        if let Some(portal) = crate::store_portals::load_portal_progress() {
            sow_app.progress = portal;
        }
        #[cfg(target_arch = "wasm32")]
        {
            if crate::store_portals::should_fetch_cloud_profile() {
                sow_app.fetch_cloud_progress();
            } else if crate::store_portals::is_portal_embed() {
                sow_app.boot_db_settled = true;
            }
        }
        sow_app
    }
}
