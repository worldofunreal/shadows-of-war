#![recursion_limit = "256"]
#![warn(dead_code, unused_variables, unused_imports)]
#![cfg(target_arch = "wasm32")]
use sow_net::client::SowClient;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientPhase {
    Splash,
    MainMenu,
    Playing,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UiAction {
    ConnectToServer(String),
    RetryConnection,
    JoinLobby(u64),
    /// Queue-only exit; keep the live orchestrator connection.
    LeaveLobby,
    /// Active-match exit; tear down the current game connection.
    ReturnToMenu,
    HostPrivateLobby,
    StartSinglePlayer(Box<sow_core::game_config::GameConfig>),
    SetAttackRatio(f32),
    CenterCamera,
    FocusTile(f32, f32),
    ZoomIn,
    ZoomOut,
    ToggleSettings,
    ToggleCredits,
    TogglePrivacy,
    ToggleTerms,
    ToggleDevSidebar,
    StartPrivateLobby(u64),
    PortalShowAuthPrompt,
    SaveDisplayName(String),
    OpenCreateGame,
    CreateGame {
        config: Box<sow_core::game_config::GameConfig>,
        is_private: bool,
        password: Option<String>,
    },
    OpenJoinBrowser,
    CloseOverlay,
    JoinWithCode,
    JoinWithPassword(u64),
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
    ToggleShowcase,
    OpenStorePage,
    OpenProfilePage,
    LoadOwnProfile,
    OpenPublicProfilePage(String),
    LoadProfileHistory,
    LoadProfileRatings,
    SearchProfiles(String),
    LoadMatchDetail(String),
    CloseMatchDetail,
    UnlockLeader {
        leader_id: String,
        currency: String,
    },
    UnlockSkin(String),
    EquipSkin(String),
}

pub(crate) const fn rgb(r: u8, g: u8, b: u8) -> [f32; 4] {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0]
}

pub(crate) fn get_build_version() -> String {
    if let Some(window) = web_sys::window() {
        if let Ok(val) = js_sys::Reflect::get(
            &window,
            &wasm_bindgen::JsValue::from_str("SOW_BUILD_VERSION"),
        ) {
            if let Some(s) = val.as_string() {
                if s != "__BUILD_TS__" {
                    return s;
                }
            }
        }
    }
    "unknown".to_string()
}

mod analytics;
mod anonymous_identity;
mod asset_config;
pub mod diag;

pub use asset_config::AssetConfig;

/// Allow very wide map views (scroll / pinch clamp to this minimum).
const CAMERA_MIN_ZOOM: f32 = 0.75;
/// Hard ceiling so zoom stays finite and GPU paths stay well-behaved.
const CAMERA_MAX_ZOOM_CAP: f32 = 100.0;

/// Pixels-per-world-unit zoom max scales with window size so you can fill ~one hex tile
/// across the long screen axis (hex neighbor spacing ≈ 1 world unit in the map shader).
fn camera_zoom_upper_bound(screen_w: f32, screen_h: f32) -> f32 {
    let longest = screen_w.max(screen_h).max(1.0);
    (longest * 3.0).clamp(CAMERA_MIN_ZOOM, CAMERA_MAX_ZOOM_CAP.max(CAMERA_MIN_ZOOM))
}

/// Minimum zoom so the map always covers the viewport — never see outside.
/// Generic over all maps: uses runtime `map_w/h`, no per-map config.
pub(crate) fn camera_zoom_lower_bound(screen_w: f32, screen_h: f32, map_w: u32, map_h: u32) -> f32 {
    if map_w == 0 || map_h == 0 {
        return CAMERA_MIN_ZOOM;
    }
    let fit_w = screen_w / map_w as f32;
    let fit_h = screen_h / map_h as f32;
    fit_w.max(fit_h).max(CAMERA_MIN_ZOOM)
}

fn spawn_sow_client_connect(url: String, connect_tx: &app::WakeSender<Result<SowClient, String>>) {
    let tx = (*connect_tx).clone();
    let url_clone = url.clone();
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use wasm_bindgen::JsCast;

    let tx_for_timeout = (*connect_tx).clone();
    let finished = Arc::new(AtomicBool::new(false));
    let finished_clone = finished.clone();

    wasm_bindgen_futures::spawn_local(async move {
        let res = SowClient::connect(&url_clone).await;
        if !finished.swap(true, Ordering::SeqCst) {
            match res {
                Ok(c) => {
                    let _ = tx.send(Ok(c));
                }
                Err(e) => {
                    let _ = tx.send(Err(e.to_string()));
                }
            }
        }
    });

    let closure = wasm_bindgen::closure::Closure::<dyn FnMut()>::new(move || {
        if !finished_clone.swap(true, Ordering::SeqCst) {
            let _ = tx_for_timeout.send(Err("Connection timed out after 5 seconds".to_string()));
        }
    });

    if let Some(window) = web_sys::window() {
        let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
            closure.as_ref().unchecked_ref(),
            5000,
        );
        closure.forget();
    }
}

pub enum MapDownloadEvent {
    CatalogReady(Vec<sow_core::maps::MapCatalogEntry>),
    MapReady(String, Vec<u8>),
    /// `leader == None` is the null/fallback avatar (`null.webp`).
    AvatarReady {
        leader: Option<sow_core::player::Leader>,
        bytes: Vec<u8>,
    },
    AvatarFailed {
        leader: Option<sow_core::player::Leader>,
        reason: String,
    },
    /// Portal identity avatar (CrazyGames profile picture, arbitrary remote URL).
    PortalAvatarReady {
        bytes: Vec<u8>,
    },
    PortalAvatarFailed {
        reason: String,
    },
    Progress(String, u8),
    Error(String),
}

pub enum EngineInitEvent {
    Progress(f32),
    Complete(
        Box<sow_core::game::GameState>,
        sow_core::water_components::WaterComponents,
        Box<sow_core::protocol::ServerStartMessage>,
    ),
}

pub mod app;
pub mod asset;
pub mod campaign;
pub mod input;
pub mod loader;
mod map_cache;
#[cfg(target_arch = "wasm32")]
pub mod net;
pub mod platform_identity;
pub mod player_progress;
pub mod render;
pub mod store_portals;
pub mod ui;
pub use ui::app::ClientApp;
pub use ui::main_menu::LobbyNotice;
pub mod theme {
    pub use crate::ui::theme::*;
}
pub mod utils {
    pub use crate::ui::utils::*;
}
mod viewport;
#[cfg(target_arch = "wasm32")]
mod web_canvas;
#[cfg(target_arch = "wasm32")]
mod web_menu;

pub(crate) fn player_sound_type(value: sow_core::player::PlayerType) -> sow_audio::PlayerSoundType {
    match value {
        sow_core::player::PlayerType::Human => sow_audio::PlayerSoundType::Human,
        sow_core::player::PlayerType::Nation => sow_audio::PlayerSoundType::Nation,
        sow_core::player::PlayerType::Bot => sow_audio::PlayerSoundType::Bot,
    }
}

pub(crate) fn building_sound_kind(
    value: sow_core::game::BuildingKind,
) -> sow_audio::BuildingSoundKind {
    match value {
        sow_core::game::BuildingKind::City => sow_audio::BuildingSoundKind::City,
        sow_core::game::BuildingKind::Bunker => sow_audio::BuildingSoundKind::Bunker,
        sow_core::game::BuildingKind::Factory => sow_audio::BuildingSoundKind::Factory,
        sow_core::game::BuildingKind::Port => sow_audio::BuildingSoundKind::Port,
    }
}

use app::SowApp;
use winit::application::ApplicationHandler;

impl ApplicationHandler for SowApp {
    fn resumed(&mut self, event_loop: &dyn winit::event_loop::ActiveEventLoop) {
        crate::web_menu::register_event_loop_wake(event_loop.create_proxy());
        self.handle_resumed(event_loop);
    }

    fn can_create_surfaces(&mut self, event_loop: &dyn winit::event_loop::ActiveEventLoop) {
        crate::web_menu::register_event_loop_wake(event_loop.create_proxy());
        self.handle_resumed(event_loop);
    }

    fn suspended(&mut self, event_loop: &dyn winit::event_loop::ActiveEventLoop) {
        self.handle_suspended(event_loop);
    }

    fn window_event(
        &mut self,
        event_loop: &dyn winit::event_loop::ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        if self.gfx.window.is_none() || self.gfx.window.as_ref().unwrap().id() != window_id {
            return;
        }
        if let winit::event::WindowEvent::RedrawRequested = event {
            self.render_frame(event_loop);
        } else {
            self.handle_window_event(event_loop, event);
        }
    }

    fn about_to_wait(&mut self, event_loop: &dyn winit::event_loop::ActiveEventLoop) {
        if self.gpu_init_failed {
            event_loop.exit();
            return;
        }
        self.update(event_loop);

        event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);

        if matches!(
            self.ui.app.phase,
            ClientPhase::Playing | ClientPhase::Splash
        ) && let Some(win) = self.active_window()
        {
            win.request_redraw();
        }
    }
}

pub fn run_game(event_loop: winit::event_loop::EventLoop) {
    let app = SowApp::new();
    let _ = event_loop.run_app(app);
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn wasm_main() {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    diag::init_from_url();
    console_log::init_with_level(log::Level::Info).expect("error initializing logger");
    log::info!("SOW ENGINE WASM STARTING...");
    crate::web_canvas::install_viewport_listeners();

    let event_loop = winit::event_loop::EventLoop::new().unwrap();
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);

    run_game(event_loop);
}
pub mod sim;
