//! Web menu bridge.
//!
//! The browser/WebView owns presentation and input for the main menu. Rust remains the
//! source of truth for connection state, lobby state, identity, and match transitions.
//! Commands cross this boundary as small JSON messages; the existing UiAction and network
//! paths execute them unchanged.

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
    SetTutorialPaused {
        paused: bool,
        #[serde(default)]
        camera_only: bool,
        #[serde(default)]
        paused_action: Option<crate::app::TutorialPausedAction>,
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
    AcknowledgeRewardPresentation {
        account_id: String,
        receipt_id: String,
    },
    OpenBrowser,
    OpenCreate,
    CloseOverlay,
    LeaveLobby,
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
    SetStickyBuildingMode {
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
    CounterAttack {
        target_player_id: u16,
    },
    Surrender,
    ToggleLeaderboard,
    ToggleDevSidebar,
    SetDevConfig {
        field: WebDevConfigField,
        value: f32,
    },
    ResetDevConfig,
    DevVfxAll {
        on: bool,
    },
    SetShowDevTools {
        value: bool,
    },
    ReturnToMenu,
    ContinueObserving,
    ZoomIn,
    ZoomOut,
    CenterCamera,
    ExpressEmoji {
        emoji: String,
        #[serde(default)]
        pinned: bool,
    },
    SetEmojiPinned {
        pinned: bool,
    },
    FocusPlayer {
        player_id: u16,
    },
    FocusWorld {
        x: f32,
        y: f32,
    },
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum WebDevConfigField {
    Thickness,
    Darkness,
    ShoreThickness,
    ShoreDarkness,
    ConquestDuration,
    TerritoryOpacity,
    BlendMode,
    FontSizeScale,
    FontFaceDilate,
    FontOutlineThickness,
    FontShadowY,
    FontUnderlaySoftness,
    FontCharSpacing,
    VfxConquer,
    VfxBorderBreathe,
    VfxEnergyFlow,
    VfxHeartbeat,
    VfxWarFog,
    FogOfWar,
    VfxFallout,
    VfxAmbientGrade,
    VfxHoloGrid,
    VfxMoverTrails,
    VfxClickMarkers,
    VfxAttackBadges,
    VfxWorldBuildings,
    VfxBotAvatars,
    VfxNameplateNames,
    VfxNameplateTroops,
}

thread_local! {
    static COMMANDS: RefCell<VecDeque<WebMenuCommand>> = RefCell::new(VecDeque::new());
    static EVENT_LOOP_WAKE: RefCell<Option<winit::event_loop::EventLoopProxy>> =
        const { RefCell::new(None) };
    /// Last payload handed to JS. publish_state runs every frame; without this
    /// guard each frame allocates a fresh JSON string plus a JS-side copy.
    static LAST_PUBLISHED: RefCell<String> = RefCell::new(String::new());
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
    static LAST_WEB_INBOX_PAYLOAD:
        RefCell<Option<(u64, u16, serde_json::Value)>> = const { RefCell::new(None) };

}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    selected_building: u8,
    selected_warship_build: bool,
    selected_nuke: u8,
    building_costs: [u64; 9],
    settings_mute: bool,
    settings_music_volume: u32,
    settings_reduced_motion: bool,
    settings_free_zoom_out: bool,
    settings_sticky_building_mode: bool,
    settings_show_dev_tools: bool,
    leaderboard_open: bool,
    leaderboard_publish_revision: u64,
    tutorial_active: bool,
    dev_sidebar_open: bool,
    dev_config: [u32; 29],
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
    player_kda: [u32; 3],
    snapshot_tick: u64,
    camera_zoom_hundredths: i32,
    camera_zoom_floor_hundredths: i32,
    camera_zoom_ceiling_hundredths: i32,
    tutorial_camera_zoom_hundredths: i32,
    tutorial_target_zoom_hundredths: i32,
    tutorial_marker_player_id: Option<u16>,
    tutorial_pointer: Option<(u64, u64)>,
    tutorial_camera_viewport: Option<(u32, u32, u32, u32)>,
    hovered_tile: u32,
    hovered_owner: u16,
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
    kills: u32,
    deaths: u32,
    assists: u32,
    inbox_count: usize,
    nuke_available: bool,
    nuke_cooldown_ticks: u32,
    boats_in_use: u32,
    boat_capacity: u32,
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
                kills: player.kills,
                deaths: player.deaths,
                assists: player.assists,
                inbox_count: player.alliance_requests.len() + player.resource_requests.len(),
                nuke_available: player.nuke_available,
                nuke_cooldown_ticks: player.nuke_cooldown_ticks,
                boats_in_use: player.boats_in_use,
                boat_capacity: player.boat_capacity,
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
                            self.process_ui_actions(Some(UiAction::StartSinglePlayer(
                                Box::new(config),
                            )))
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
                        self.ui.tutorial_paused_action =
                            if paused && !camera_only { paused_action } else { None };
                        self.input.tutorial_camera_drag_recorded = false;
                        if paused {
                            self.sim.offline_intents.clear();
                        }
                        if self.ui.tutorial_camera_only {
                            self.clear_placement();
                            self.cancel_hold_build();
                            self.close_map_context_menu();
                        }
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
                                engine.campaign_faction_ids.iter().find_map(|(player_id, id)| {
                                    (id == &target_faction_id).then_some(*player_id)
                                })
                            })
                        };
                        if let (Some(target_player_id), Some(engine)) =
                            (target_player_id, self.sim.engine.as_mut())
                        {
                            let reinforcement = reinforcement.map(|settings| (settings.capacity_ratio, settings.interval_seconds));
                            if engine
                                .activate_campaign_assault(attacker_team, target_player_id, preserve_relation, target_allies, reinforcement, hold_last_tile)
                                == 0
                            {
                                log::warn!("[CAMPAIGN] assault had no valid scripted attackers or target");
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
                        self.complete_boudica_tutorial();
                    } else if self
                        .progress
                        .complete_episode(campaign.episode_id(), campaign.advisor())
                    {
                        self.save_local_progress();
                    }
                    self.begin_exit_to_main_menu();
                }
                WebMenuCommand::SetLeader { leader_id } => {
                    let value = serde_json::Value::String(leader_id);
                    match serde_json::from_value::<sow_core::player::Leader>(value) {
                        Ok(leader) => {
                            self.ui
                                .app
                                .main_menu_state
                                .set_selected_leader(leader, true);
                        }
                        Err(error) => log::warn!("[WEB MENU] invalid leader: {error}"),
                    }
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
                WebMenuCommand::AcknowledgeRewardPresentation {
                    account_id,
                    receipt_id,
                } => {
                    if self.progress_account_id.as_deref() == Some(account_id.as_str())
                        && self.exit_reward_preview.as_ref().is_some_and(|preview| {
                            preview.account_id == account_id && preview.receipt_id == receipt_id
                        })
                    {
                        self.exit_reward_preview = None;
                    }
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
                    let volume = if value {
                        0.0
                    } else {
                        self.ui.app.settings_state.music_volume
                    };
                    sow_audio::set_master_volume(volume);
                }
                WebMenuCommand::SetMusicVolume { value } => {
                    let volume = value.clamp(0.0, 1.0);
                    self.ui.app.settings_state.music_volume = volume;
                    if !self.ui.app.settings_state.mute_all {
                        sow_audio::set_master_volume(volume);
                    }
                }
                WebMenuCommand::SetReducedMotion { value } => {
                    self.ui.app.settings_state.reduced_motion = value;
                }
                WebMenuCommand::SetFreeZoomOut { value } => {
                    self.ui.app.settings_state.free_zoom_out = value;
                    self.clamp_camera_to_map();
                }
                WebMenuCommand::SetStickyBuildingMode { value } => {
                    self.ui.app.settings_state.sticky_building_mode = value;
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
                    let accepted = self.send_intent(sow_core::protocol::GameplayIntent::SendResources {
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
                WebMenuCommand::CounterAttack { target_player_id } => {
                    let counter_attack = self.sim.current_snapshot.as_ref().and_then(|snapshot| {
                        let my_player_id = self.sim.my_player_id?;
                        if target_player_id == 0 || target_player_id == my_player_id {
                            return None;
                        }
                        let me = snapshot.players.iter().find(|player| player.id == my_player_id)?;
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
                WebMenuCommand::Surrender => {
                    self.send_intent(sow_core::protocol::GameplayIntent::Resign);
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
                WebMenuCommand::ReturnToMenu => {
                    self.process_ui_actions(Some(UiAction::ReturnToMenu));
                }
                WebMenuCommand::ContinueObserving => {
                    self.ui.is_spectating = true;
                    self.ui.endgame_cache = None;
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
                        if self.ui.show_dev_sidebar && value.is_finite() {
                            crate::theme::dev_config::DevConfig::update(|config| match field {
                                WebDevConfigField::Thickness => {
                                    config.thickness = value.clamp(0.0, 1.0)
                                }
                                WebDevConfigField::Darkness => {
                                    config.darkness = value.clamp(0.0, 1.0)
                                }
                                WebDevConfigField::ShoreThickness => {
                                    config.shore_thickness = value.clamp(0.0, 1.0)
                                }
                                WebDevConfigField::ShoreDarkness => {
                                    config.shore_darkness = value.clamp(0.0, 1.0)
                                }
                                WebDevConfigField::ConquestDuration => {
                                    config.conquest_duration = value.clamp(0.1, 10.0)
                                }
                                WebDevConfigField::TerritoryOpacity => {
                                    config.territory_opacity = value.clamp(0.0, 1.0)
                                }
                                WebDevConfigField::BlendMode => {
                                    config.blend_mode = value.clamp(0.0, 3.0)
                                }
                                WebDevConfigField::FontSizeScale => {
                                    config.font_size_scale = value.clamp(0.5, 2.5)
                                }
                                WebDevConfigField::FontFaceDilate => {
                                    config.font_face_dilate = value.clamp(-1.0, 2.0)
                                }
                                WebDevConfigField::FontOutlineThickness => {
                                    config.font_outline_thickness = value.clamp(0.0, 3.0)
                                }
                                WebDevConfigField::FontShadowY => {
                                    config.font_shadow_y = value.clamp(0.0, 5.0)
                                }
                                WebDevConfigField::FontUnderlaySoftness => {
                                    config.font_underlay_softness = value.clamp(0.0, 2.0)
                                }
                                WebDevConfigField::FontCharSpacing => {
                                    config.font_char_spacing = value.clamp(0.8, 1.8)
                                }
                                WebDevConfigField::VfxConquer => config.vfx_conquer = value != 0.0,
                                WebDevConfigField::VfxBorderBreathe => {
                                    config.vfx_border_breathe = value != 0.0
                                }
                                WebDevConfigField::VfxEnergyFlow => {
                                    config.vfx_energy_flow = value != 0.0
                                }
                                WebDevConfigField::VfxHeartbeat => {
                                    config.vfx_heartbeat = value != 0.0
                                }
                                WebDevConfigField::VfxWarFog => config.vfx_war_fog = value != 0.0,
                                WebDevConfigField::FogOfWar => config.fog_of_war = value != 0.0,
                                WebDevConfigField::VfxFallout => config.vfx_fallout = value != 0.0,
                                WebDevConfigField::VfxAmbientGrade => {
                                    config.vfx_ambient_grade = value != 0.0
                                }
                                WebDevConfigField::VfxHoloGrid => {
                                    config.vfx_holo_grid = value != 0.0
                                }
                                WebDevConfigField::VfxMoverTrails => {
                                    config.vfx_mover_trails = value != 0.0
                                }
                                WebDevConfigField::VfxClickMarkers => {
                                    config.vfx_click_markers = value != 0.0
                                }
                                WebDevConfigField::VfxAttackBadges => {
                                    config.vfx_attack_badges = value != 0.0
                                }
                                WebDevConfigField::VfxWorldBuildings => {
                                    config.vfx_world_buildings = value != 0.0
                                }
                                WebDevConfigField::VfxBotAvatars => {
                                    config.vfx_bot_avatars = value != 0.0
                                }
                                WebDevConfigField::VfxNameplateNames => {
                                    config.vfx_nameplate_names = value != 0.0
                                }
                                WebDevConfigField::VfxNameplateTroops => {
                                    config.vfx_nameplate_troops = value != 0.0
                                }
                            });
                        }
                    }
                    #[cfg(not(any(feature = "dev", debug_assertions)))]
                    let _ = (field, value);
                }
                WebMenuCommand::DevVfxAll { on } => {
                    #[cfg(any(feature = "dev", debug_assertions))]
                    {
                        crate::theme::dev_config::DevConfig::update(|config| {
                            config.vfx_conquer = on;
                            config.vfx_border_breathe = on;
                            config.vfx_energy_flow = on;
                            config.vfx_heartbeat = on;
                            config.vfx_war_fog = on;
                            config.fog_of_war = on;
                            config.vfx_fallout = on;
                            config.vfx_ambient_grade = on;
                            config.vfx_holo_grid = on;
                            config.vfx_mover_trails = on;
                            config.vfx_click_markers = on;
                            config.vfx_attack_badges = on;
                            config.vfx_world_buildings = on;
                            config.vfx_bot_avatars = on;
                            config.vfx_nameplate_names = on;
                            config.vfx_nameplate_troops = on;
                        });
                    }
                    #[cfg(not(any(feature = "dev", debug_assertions)))]
                    let _ = on;
                }
                WebMenuCommand::ResetDevConfig => {
                    #[cfg(any(feature = "dev", debug_assertions))]
                    {
                        crate::theme::dev_config::DevConfig::set(
                            crate::theme::dev_config::DevConfig::default(),
                        );
                    }
                }
                WebMenuCommand::SetShowDevTools { value } => {
                    self.ui.app.settings_state.show_dev_tools = value;
                    if !value {
                        self.ui.show_dev_sidebar = false;
                    }
                }
                WebMenuCommand::ExpressEmoji { emoji, pinned } => {
                    self.send_intent(sow_core::protocol::GameplayIntent::ExpressEmoji {
                        emoji,
                        pinned,
                    });
                }
                WebMenuCommand::SetEmojiPinned { pinned } => {
                    self.ui.app.hud_state.pin_emoji = pinned;
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
                WebMenuCommand::FocusWorld { x, y } => {
                    if x.is_finite()
                        && y.is_finite()
                        && x >= 0.0
                        && y >= 0.0
                        && x < self.sim.map_w as f32
                        && y < self.sim.map_h as f32
                    {
                        let tutorial_focus = self.ui.tutorial_active && self.net.is_offline;
                        self.input.has_snapped_camera_to_spawn = true;
                        self.input.target_zoom = self.input.camera_zoom;
                        self.input.camera_focus_target = Some((x, y));
                        self.input.tutorial_camera_focus = tutorial_focus;
                        self.input.camera_focus_waiting_for_input_release = tutorial_focus
                            && self.input.is_pointer_gesture_active();
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

fn hud_publish_key(app: &SowApp) -> HudPublishKey {
    let hud = &app.ui.app.hud_state;
    let (hovered_tile, hovered_owner) = hovered_tile_owner(app);
    let tutorial_active = app.ui.tutorial_active && app.net.is_offline;
    let map_menu = app.input.map_context_menu;
    let (dev_sidebar_open, dev_config_key) = dev_config_key(app);
    let snapshot_tick = app
        .sim
        .current_snapshot
        .as_ref()
        .map(|snapshot| snapshot.tick)
        .unwrap_or(0);
    let cold_open = app.ui.show_leaderboard
        || hovered_owner != 0
        || hud.show_alliance_inbox
        || hud.show_ask_panel.is_some()
        || hud.show_betrayal_warning.is_some()
        || hud.sync_state.is_some()
        || tutorial_active
        || map_menu.is_some();
    let my_pid = app.sim.my_player_id.unwrap_or(hud.my_player_id);
    let me = my_player_summary(app, snapshot_tick, my_pid);
    let snapshot = app.sim.current_snapshot.as_ref();
    let endgame_active = !app.ui.is_spectating
        && snapshot.is_some_and(|snapshot| {
            snapshot.winner.is_some()
                || me.is_some_and(|player| !player.alive && player.has_spawned)
        });
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
        selected_building: hud
            .selected_building_kind
            .map(|kind| kind as u8)
            .unwrap_or(u8::MAX),
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
        settings_sticky_building_mode: app.ui.app.settings_state.sticky_building_mode,
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
        player_kda: me
            .map(|player| [player.kills, player.deaths, player.assists])
            .unwrap_or_default(),
        tutorial_active,
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
        tutorial_pointer: if tutorial_active {
            Some((app.input.last_mouse_x.to_bits(), app.input.last_mouse_y.to_bits()))
        } else {
            None
        },
        tutorial_camera_viewport: tutorial_active.then_some((
            app.input.camera_x.to_bits(),
            app.input.camera_y.to_bits(),
            app.input.screen_w.to_bits(),
            app.input.screen_h.to_bits(),
        )),
        dev_sidebar_open,
        dev_config: dev_config_key,
        snapshot_tick: if cold_open { snapshot_tick } else { 0 },
        hovered_tile,
        hovered_owner,
        map_menu_view: map_menu.map(|menu| menu.view),
        map_menu_session: map_menu.map(|menu| menu.session).unwrap_or(0),
        map_menu_tile: map_menu.map(|menu| menu.tile_idx).unwrap_or(u32::MAX),
    }
}

#[cfg(any(feature = "dev", debug_assertions))]
fn dev_config_key(app: &SowApp) -> (bool, [u32; 29]) {
    if !app.ui.show_dev_sidebar {
        return (false, [0; 29]);
    }
    let config = crate::theme::dev_config::DevConfig::get();
    let mut key = [0u32; 29];
    key[..13].copy_from_slice(&[
        config.thickness.to_bits(),
        config.darkness.to_bits(),
        config.shore_thickness.to_bits(),
        config.shore_darkness.to_bits(),
        config.conquest_duration.to_bits(),
        config.territory_opacity.to_bits(),
        config.blend_mode.to_bits(),
        config.font_size_scale.to_bits(),
        config.font_face_dilate.to_bits(),
        config.font_outline_thickness.to_bits(),
        config.font_shadow_y.to_bits(),
        config.font_underlay_softness.to_bits(),
        config.font_char_spacing.to_bits(),
    ]);
    for (index, value) in [
        config.vfx_conquer,
        config.vfx_border_breathe,
        config.vfx_energy_flow,
        config.vfx_heartbeat,
        config.vfx_war_fog,
        config.fog_of_war,
        config.vfx_fallout,
        config.vfx_ambient_grade,
        config.vfx_holo_grid,
        config.vfx_mover_trails,
        config.vfx_click_markers,
        config.vfx_attack_badges,
        config.vfx_world_buildings,
        config.vfx_bot_avatars,
        config.vfx_nameplate_names,
        config.vfx_nameplate_troops,
    ]
    .into_iter()
    .enumerate()
    {
        key[13 + index] = u32::from(value);
    }
    (true, key)
}

#[cfg(not(any(feature = "dev", debug_assertions)))]
fn dev_config_key(_app: &SowApp) -> (bool, [u32; 29]) {
    (false, [0; 29])
}

#[cfg(any(feature = "dev", debug_assertions))]
fn dev_tools_payload(app: &SowApp) -> serde_json::Value {
    let open = app.ui.show_dev_sidebar;
    let mut payload = serde_json::json!({
        "available": app.ui.app.settings_state.show_dev_tools,
        "open": open,
    });
    if open {
        let config = crate::theme::dev_config::DevConfig::get();
        payload["config"] = serde_json::json!({
            "thickness": config.thickness,
            "darkness": config.darkness,
            "shore_thickness": config.shore_thickness,
            "shore_darkness": config.shore_darkness,
            "conquest_duration": config.conquest_duration,
            "territory_opacity": config.territory_opacity,
            "blend_mode": config.blend_mode,
            "font_size_scale": config.font_size_scale,
            "font_face_dilate": config.font_face_dilate,
            "font_outline_thickness": config.font_outline_thickness,
            "font_shadow_y": config.font_shadow_y,
            "font_underlay_softness": config.font_underlay_softness,
            "font_char_spacing": config.font_char_spacing,
            "vfx_conquer": config.vfx_conquer,
            "vfx_border_breathe": config.vfx_border_breathe,
            "vfx_energy_flow": config.vfx_energy_flow,
            "vfx_heartbeat": config.vfx_heartbeat,
            "vfx_war_fog": config.vfx_war_fog,
            "fog_of_war": config.fog_of_war,
            "vfx_fallout": config.vfx_fallout,
            "vfx_ambient_grade": config.vfx_ambient_grade,
            "vfx_holo_grid": config.vfx_holo_grid,
            "vfx_mover_trails": config.vfx_mover_trails,
            "vfx_click_markers": config.vfx_click_markers,
            "vfx_attack_badges": config.vfx_attack_badges,
            "vfx_world_buildings": config.vfx_world_buildings,
            "vfx_bot_avatars": config.vfx_bot_avatars,
            "vfx_nameplate_names": config.vfx_nameplate_names,
            "vfx_nameplate_troops": config.vfx_nameplate_troops,
        });
    }
    payload
}

#[cfg(not(any(feature = "dev", debug_assertions)))]
fn dev_tools_payload(_app: &SowApp) -> serde_json::Value {
    serde_json::json!({
        "available": false,
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
                            sow_core::game::BuildingKind::Bunker => "Bunker",
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
        .selected_building_kind
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
        nameplate_anchor[0].floor().clamp(0.0, map_w.saturating_sub(1) as f32) as i32,
        nameplate_anchor[1].floor().clamp(0.0, map_h.saturating_sub(1) as f32) as i32,
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
            let nameplate_distance = sow_core::building::hex_distance(
                nameplate_tile.0,
                nameplate_tile.1,
                col,
                row,
            ) as u32;
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
                consider_build_site(
                    idx,
                    border_tiles.is_some_and(|tiles| tiles.contains(idx)),
                );
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

fn tutorial_payload(app: &mut SowApp, my_pid: u16) -> serde_json::Value {
    let Some(snapshot) = app.sim.current_snapshot.clone() else {
        return serde_json::json!({ "active": true, "episode_id": app.ui.tutorial_campaign.episode_id(), "tick": 0 });
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
    let zoom_ceiling = crate::camera_zoom_upper_bound(app.input.screen_w, app.input.screen_h)
        .max(zoom_floor);
    let zoom_in_target = TUTORIAL_MAX_ZOOM.clamp(zoom_floor, zoom_ceiling);
    let zoom_in_complete =
        app.input.camera_zoom.is_finite() && app.input.camera_zoom >= zoom_in_target;
    let hover_pointer = match app.input.hover_pointer {
        HoverPointer::Mouse => Some(serde_json::json!({
            "x": app.input.last_mouse_x / f64::from(sf),
            "y": app.input.last_mouse_y / f64::from(sf),
        })),
        HoverPointer::Touch if app.input.active_touches.len() == 1 => Some(serde_json::json!({
            "x": app.input.last_mouse_x / f64::from(sf),
            "y": app.input.last_mouse_y / f64::from(sf),
        })),
        _ => None,
    };
    let tutorial_target_hovered = match app.input.hover_pointer {
        HoverPointer::Mouse => app.ui.nameplates.tutorial_target_contains(
            marker_player_id,
            app.input.last_mouse_x,
            app.input.last_mouse_y,
        ),
        HoverPointer::Touch if app.input.active_touches.len() == 1 => {
            app.ui.nameplates.tutorial_target_contains(
                marker_player_id,
                app.input.last_mouse_x,
                app.input.last_mouse_y,
            )
        }
        _ => false,
    };
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
    let upgrade_buildings = tutorial_building_anchors(
        &snapshot,
        my_pid,
        map_w,
        map_h,
        &app.input,
        sf,
        false,
    );
    let upgrading_buildings = tutorial_building_anchors(
        &snapshot,
        my_pid,
        map_w,
        map_h,
        &app.input,
        sf,
        true,
    );
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
    serde_json::json!({
        "active": true,
        "episode_id": app.ui.tutorial_campaign.episode_id(),
        "tick": snapshot.tick,
        "expand": guide_screen(expand_tile),
        "assault": guide_screen(assault_tile),
        "target_action": guide_screen(target_tile),
        "build_site": build_site_screen,
        "upgrade_buildings": upgrade_buildings,
        "upgrading_buildings": upgrading_buildings,
        "nameplate": nameplate_screen,
        "target_nameplate": target_nameplate,
        "player": player_screen,
        "camera": {
            "x": app.input.camera_x,
            "y": app.input.camera_y,
            "zoom": app.input.camera_zoom,
            "width": app.input.screen_w,
            "height": app.input.screen_h,
            "scale": sf,
        },
        "pointer": hover_pointer,
        "facts": {
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
            "contacts": observation.seen_contacts.len(),
            "contact_faction_ids": observation.seen_contact_faction_ids,
            "attacks": observation.seen_attacks.len(),
            "attacks_by_faction_id": attacks_by_faction_id,
            "buildings": observation.seen_structures.len(),
            "cities": observation.seen_cities.len(),
            "farms": observation.seen_buildings_by_kind.get("farms").map_or(0, |ids| ids.len()),
            "factories": observation.seen_buildings_by_kind.get("factories").map_or(0, |ids| ids.len()),
            "ports": observation.seen_buildings_by_kind.get("ports").map_or(0, |ids| ids.len()),
            "bunkers": observation.seen_buildings_by_kind.get("bunkers").map_or(0, |ids| ids.len()),
            "ally_support_deliveries": observation.ally_support_deliveries,
            "support_deliveries_by_faction_id": support_deliveries_by_faction_id,
            "structure_upgrades": observation.structure_upgrades,
            "structure_levels": observation.structure_levels,
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
        "civilization": player.civilization.name(),
        "team": player.team,
        "active_emoji": &player.active_emoji,
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
    let mut bunkers = 0;
    let mut farms = 0;
    for building in &snapshot.buildings {
        if building.owner_id != owner_id {
            continue;
        }
        match building.kind {
            sow_core::game::BuildingKind::City => cities += 1,
            sow_core::game::BuildingKind::Factory => factories += 1,
            sow_core::game::BuildingKind::Port => ports += 1,
            sow_core::game::BuildingKind::Bunker => bunkers += 1,
            sow_core::game::BuildingKind::Farm => farms += 1,
        }
    }
    let mut payload = player_json(player, my_pid, snapshot.total_land_tiles, None);
    payload["cities"] = serde_json::json!(cities);
    payload["factories"] = serde_json::json!(factories);
    payload["ports"] = serde_json::json!(ports);
    payload["bunkers"] = serde_json::json!(bunkers);
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
                "leader": leader_id(requester.leader),
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
                "gold": request.gold,
                "troops": request.troops,
            }));
        }
    }
    serde_json::Value::Array(requests)
}

fn building_benefit_label(
    kind: sow_core::game::BuildingKind,
    level: u8,
    config: &sow_core::game_config::GameConfig,
) -> String {
    use sow_core::game::BuildingKind as Kind;
    match kind {
        Kind::City => format!(
            "Adds +{:.0} troop capacity and +{:.2} troops/s{}.",
            config.city_max_troops * f64::from(level),
            config.city_troop_income * f64::from(level),
            if level == Kind::City.max_level() { "; unlocks nuclear attacks" } else { "" },
        ),
        Kind::Port => format!(
            "Adds {level} boat slots and +{level}% boat speed.",
        ),
        Kind::Factory => format!("Produces +{:.2} gold/s.", config.factory_gold_income * f64::from(level)),
        Kind::Bunker => {
            let range = (config.bunker_range.round() as u32
                + u32::from(level.saturating_sub(1)) * 2)
                .min(20);
            let defense = u32::from(level) * 5;
            if level >= sow_core::game::BuildingKind::Bunker.max_level() {
                format!(
                    "Raises nearby enemy attack losses by up to {defense}% within range {range}; intercepts nuclear bombs. Combined defense stops at 50%."
                )
            } else {
                format!(
                    "Raises nearby enemy attack losses by up to {defense}% within range {range}. Combined defense stops at 50%."
                )
            }
        }
        Kind::Farm => format!(
            "This plot produces +{:.2} troops/s.",
            config.farm_troop_income * f64::from(level)
        ),
    }
}

fn building_metric(
    icon: &'static str,
    label: &'static str,
    value: f64,
    prefix: &'static str,
    unit: &'static str,
) -> serde_json::Value {
    serde_json::json!({ "icon": icon, "label": label, "value": value, "prefix": prefix, "unit": unit })
}

fn building_metrics(
    kind: sow_core::game::BuildingKind,
    level: u8,
    config: &sow_core::game_config::GameConfig,
) -> Vec<serde_json::Value> {
    use sow_core::game::BuildingKind as Kind;
    if level == 0 {
        return Vec::new();
    }
    let level = f64::from(level);
    let metrics = match kind {
        Kind::City => vec![
            building_metric(
                "troops",
                "Troop capacity",
                config.city_max_troops * level,
                "+",
                "",
            ),
            building_metric(
                "troops",
                "Troop income",
                config.city_troop_income * level,
                "+",
                "/s",
            ),
        ],
        Kind::Port => vec![
            building_metric("port", "Boat slots", level, "+", ""),
            building_metric("speed", "Boat speed", level, "+", "%"),
        ],
        Kind::Factory => vec![building_metric(
                "gold",
                "Gold income",
                config.factory_gold_income * level,
                "+",
                "/s",
            )],
        Kind::Bunker => {
            let range = (config.bunker_range.round() as u32 + (level as u32 - 1) * 2).min(20);
            let mut stats = vec![
                building_metric("defense", "Enemy attack losses", level * 5.0, "+", "%"),
                building_metric("range", "Defense range", f64::from(range), "", ""),
            ];
            if level as u8 >= Kind::Bunker.max_level() {
                stats.push(serde_json::json!({
                    "icon": "nuke",
                    "label": "Nuclear interception",
                    "value": "✓",
                }));
            }
            stats
        }
        Kind::Farm => vec![building_metric(
            "troops",
            "Troop income",
            config.farm_troop_income * level,
            "+",
            "/s",
        )],
    };
    metrics
}

fn building_detail_payload(
    app: &SowApp,
    building: &sow_core::protocol::BuildingSnapshot,
) -> serde_json::Value {
    let active_level = building.active_level();
    let next_level = active_level.saturating_add(1);
    let my_id = app
        .sim
        .my_player_id
        .unwrap_or(app.ui.app.hud_state.my_player_id);
    let owns = building.owner_id == my_id;
    let snapshot = app.sim.current_snapshot.as_ref();
    let cost = app
        .map_menu_cost(
            crate::input::map_click::MapMenuAction::UpgradeStructure,
            building.tile_idx,
        )
        .0
        .unwrap_or_default();
    let has_gold = app.current_player_gold() >= cost;
    let maxed = next_level > building.kind.max_level();
    let duration_ticks = sow_core::building::structure_upgrade_duration_ticks(
        building.kind,
        next_level,
    );
    let boat_slots = snapshot.and_then(|snapshot| {
        snapshot
            .players
            .iter()
            .find(|player| player.id == my_id)
            .map(|player| {
                let port_levels = snapshot
                    .buildings
                    .iter()
                    .filter(|candidate| {
                        candidate.owner_id == my_id
                            && candidate.kind == sow_core::game::BuildingKind::Port
                    })
                    .map(|candidate| u32::from(candidate.active_level()))
                    .sum::<u32>();
                serde_json::json!({
                    "used": player.boats_in_use,
                    "total": player.boat_capacity,
                    "speed_percent": port_levels.min(30),
                })
            })
    });
    serde_json::json!({
        "id": building.id,
        "kind": building.kind.as_str(),
        "level": active_level,
        "name": if active_level == 0 {
            building.kind.as_str()
        } else {
            building.kind.level_name(active_level)
        },
        "benefit_label": building_benefit_label(building.kind, active_level, &app.sim.config),
        "metrics": building_metrics(building.kind, active_level, &app.sim.config),
        "next_level": (!building.under_construction && !maxed).then_some(next_level),
        "next_benefit_label": (!building.under_construction && !maxed)
            .then(|| building_benefit_label(building.kind, next_level, &app.sim.config)),
        "next_metrics": (!building.under_construction && !maxed)
            .then(|| building_metrics(building.kind, next_level, &app.sim.config)),
        "cost": cost,
        "duration_seconds": (!building.under_construction && !maxed).then_some(duration_ticks as f64 * app.sim.config.tick_rate_ms as f64 / 1000.0),
        "under_construction": building.under_construction,
        "boat_slots": (building.kind == sow_core::game::BuildingKind::Port).then_some(boat_slots).flatten(),
        "owns": owns,
        "can_upgrade": owns && !building.under_construction && !maxed && has_gold
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
            attack.owner_id == attacker_id
                && attack.target_owner == target_id
                && !attack.retreating
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
    let players: HashMap<_, _> = snapshot.players.iter().map(|player| (player.id, player)).collect();
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
            let avatar = match sow_core::player::avatar_identity_ref(player) {
                sow_core::player::AvatarIdentityRef::Portrait { slug, .. } => Some(slug),
                _ => None,
            };
            (player.name.as_str(), avatar)
        })
    };
    let mut operations = Vec::new();

    for incoming in [true, false] {
        for attack in snapshot.attacks.iter().filter(|attack| {
            let is_incoming = attack.owner_id != my_player_id
                && attack.target_owner == my_player_id;
            let is_outgoing = attack.owner_id == my_player_id;
            if incoming { is_incoming } else { is_outgoing }
        }) {
            let neutral = !incoming && attack.target_owner == 0;
            let subject_id = if incoming {
                attack.owner_id
            } else {
                attack.target_owner
            };
            let (name, avatar) = player_identity(subject_id)
                .map(|(name, avatar)| (Some(name), avatar))
                .unwrap_or((None, None));
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
            let (name, avatar) = player_identity(subject_id)
                .map(|(name, avatar)| (Some(name), avatar))
                .unwrap_or((None, None));
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
    let map_menu = app
        .input
        .map_context_menu
        .map(|menu| {
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
                .map(|building| building_detail_payload(app, building));
            serde_json::json!({
                "open": true,
                "view": match menu.view {
                    MapContextMenuView::BuildingDetails => "building_details",
                    MapContextMenuView::Radial => "radial",
                },
                "x": menu.x / screen_scale,
                "y": menu.y / screen_scale,
                "tile_idx": menu.tile_idx,
                "session": menu.session,
                "actions": actions,
                "items": items,
                "alliance_state": alliance_state,
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
    let (hovered_tile, hovered_owner) = hovered_tile_owner(app);
    // The store is only shown after a match. Keep its catalog out of the hot HUD path,
    // and use the browser clock because SystemTime::now() panics on wasm32.
    let featured_skin = if match_over {
        let rotation_period = ((js_sys::Date::now().max(0.0) / 1000.0) as u64)
            / sow_data::commerce::ROTATION_PERIOD_SECS;
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
    let selected_building = hud.selected_building_kind.map(|kind| match kind {
        sow_core::game::BuildingKind::City => "City",
        sow_core::game::BuildingKind::Bunker => "Bunker",
        sow_core::game::BuildingKind::Factory => "Factory",
        sow_core::game::BuildingKind::Port => "Port",
        sow_core::game::BuildingKind::Farm => "Farm",
    });
    let selected_nuke = hud.selected_nuke_kind.map(|kind| match kind {
        sow_core::game::NukeKind::AtomBomb => "AtomBomb",
    });
    let costs = &hud.building_costs;
    let selected_building_detail = map_menu
        .get("building")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let (port_levels, city_level, trade_ships, warships) = snapshot
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
            let warships = snapshot
                .fleets
                .iter()
                .filter(|fleet| {
                    fleet.owner_id == my_pid
                        && fleet.troops > 0.0
                        && fleet.unit_type == sow_core::game::UnitType::Warship
                })
                .count() as u32;
            (port_levels, city_level, trade_ships, warships)
        })
        .unwrap_or_default();
    let trade_ship = sow_core::game::UnitType::TradeShip;
    let warship = sow_core::game::UnitType::Warship;
    let nuke = sow_core::game::NukeKind::AtomBomb;
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
        "selected_warship_build": hud.selected_warship_build,
        "selected_nuke": selected_nuke,
        "fleet_panel": {
            "trade_ships": trade_ships,
            "trade_capacity": port_levels,
            "warships": warships,
            "port_levels": port_levels,
            "city_level": city_level,
            "trade_required_port_level": trade_ship.required_port_level(),
            "warship_required_port_level": warship.required_port_level(),
            "nuke_required_city_level": nuke.required_city_level(),
            "warship_cost": warship.gold_cost(),
            "nuke_cost": app.sim.config.nuke_cost,
            "nuke_available": me.map(|player| player.nuke_available).unwrap_or(false),
            "nuke_cooldown_ticks": me.map(|player| player.nuke_cooldown_ticks).unwrap_or(0),
            "military_used": me.map(|player| player.boats_in_use).unwrap_or(0),
            "military_capacity": me.map(|player| player.boat_capacity).unwrap_or(0),
        },
        "selected_building_detail": selected_building_detail,
        "building_costs": {
            "city": costs[0],
            "bunker": costs[1],
            "factory": costs[2],
            "port": costs[3],
            "farm": costs[4],
        },
        "pin_emoji": hud.pin_emoji,
        "fps": (app.time.current_fps > 0).then_some(app.time.current_fps),
        "ping": app.net.current_ping_ms,
        "hovered_tile": if hovered_tile == u32::MAX { serde_json::Value::Null } else { serde_json::json!(hovered_tile) },
        "hovered": serde_json::Value::Null,
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
        "player_kda": {
            "kills": me.map(|player| player.kills).unwrap_or(0),
            "deaths": me.map(|player| player.deaths).unwrap_or(0),
            "assists": me.map(|player| player.assists).unwrap_or(0),
        },
    });
    if let Some(players) = tutorial_players {
        payload["players"] = players;
    }

    if let Some(snapshot) = snapshot {
        if hovered_owner != 0 {
            payload["hovered"] =
                cached_hover_payload(snapshot, hovered_tile, hovered_owner, my_pid);
        }
        if app.ui.show_leaderboard && include_leaderboard {
            payload["leaderboard"] = cached_leaderboard_payload(snapshot, my_pid);
        }
        if hud.show_alliance_inbox {
            payload["inbox"] = cached_inbox_payload(snapshot, my_pid);
        }
        if let Some(target_id) = hud.show_ask_panel {
            if let Some(target) = snapshot
                .players
                .iter()
                .find(|player| player.id == target_id)
            {
                payload["transfer"] = serde_json::json!({
                    "target_id": target.id,
                    "target_name": &target.name,
                    "target_alive": target.alive,
                    "confirm_pending": hud.transfer_confirm_pending,
                    "suggested_gold": hud.ask_gold,
                    "suggested_troops": hud.ask_troops,
                });
            }
        }
    }

    if hud.show_betrayal_warning.is_some() || hud.betrayal_warning_cached.is_some() {
        let warning = hud
            .show_betrayal_warning
            .as_ref()
            .or(hud.betrayal_warning_cached.as_ref());
        if let Some((ally_id, _)) = warning {
            payload["betrayal"] = serde_json::json!({
                "ally_id": ally_id,
                "ally_name": snapshot
                    .and_then(|snapshot| snapshot.players.iter().find(|player| player.id == *ally_id))
                    .map(|player| player.name.clone())
                    .unwrap_or_else(|| "Ally".to_string()),
            });
        }
    }
    if let Some(sync) = &hud.sync_state {
        payload["sync"] = serde_json::to_value(sync).unwrap_or(serde_json::Value::Null);
    }
    payload
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
    serde_json::to_value(leader)
        .ok()
        .and_then(|value| value.as_str().map(ToOwned::to_owned))
        .unwrap_or_else(|| leader.name().replace(' ', ""))
}

fn notice_name(notice: Option<crate::LobbyNotice>) -> Option<&'static str> {
    match notice {
        Some(crate::LobbyNotice::HostLeft) => Some("host_left"),
        Some(crate::LobbyNotice::Kicked) => Some("kicked"),
        Some(crate::LobbyNotice::Banned) => Some("banned"),
        Some(crate::LobbyNotice::ConnectionLost) => Some("connection_lost"),
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
    let screen = crate::render::world::overlays::world_to_screen(
        world_x,
        world_y,
        &app.input,
        scale,
    );
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

/// DOM never receives transient textures, map bytes, or internal auth/session material.
pub(crate) fn publish_state(app: &mut SowApp) {
    refresh_web_leaderboard_cache(app);

    let state = &app.ui.app.main_menu_state;
    let progress = &app.progress;

    let payload = if app.ui.app.phase == crate::ClientPhase::Playing {
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
            if *last == Some(hud_key) {
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
            "loader_cycle_id": app.ui.app.splash_state.cycle_id,
            "loader_job": splash_job_name(&app.ui.app.splash_state.job),
            "loader_leader": app.ui.app.splash_state.loader_leader.map(leader_id),
            "loader_progress": app.ui.app.splash_state.progress.clamp(0.0, 1.0),
            "loader_done": app.ui.app.splash_state.done,
            "hud": hud_payload,
            "settings": {
                "mute_all": app.ui.app.settings_state.mute_all,
                "music_volume": app.ui.app.settings_state.music_volume,
                "reduced_motion": app.ui.app.settings_state.reduced_motion,
                "free_zoom_out": app.ui.app.settings_state.free_zoom_out,
                "sticky_building_mode": app.ui.app.settings_state.sticky_building_mode,
                "show_dev_tools": app.ui.app.settings_state.show_dev_tools,
            },
        })
    } else {
        LAST_HUD_KEY.with(|last| *last.borrow_mut() = None);
        LAST_MY_PLAYER.with(|cache| *cache.borrow_mut() = None);
        LAST_WEB_TOP_THREE_TICK.with(|tick| tick.set(None));
        LAST_WEB_LEADERBOARD_PAYLOAD.with(|cache| *cache.borrow_mut() = None);
        LAST_WEB_HOVER_PAYLOAD.with(|cache| *cache.borrow_mut() = None);
        LAST_WEB_INBOX_PAYLOAD.with(|cache| *cache.borrow_mut() = None);
        app.ui.leaderboard_top_three = [None; 3];
        app.ui.leaderboard_refresh_at = None;
        app.ui.leaderboard_publish_pending = false;
        let rotation_period = ((js_sys::Date::now().max(0.0) / 1000.0) as u64)
            / sow_data::commerce::ROTATION_PERIOD_SECS;
        let mut store_catalog = sow_data::commerce::catalog_for_profile(
            &progress.owned_leaders,
            &progress.owned_skins,
            progress.crowns,
            progress.gems,
            rotation_period,
        );
        // Direct products are enabled only by the authenticated store catalog
        // endpoint after provider mappings have been verified. Do not render
        // placeholder purchase buttons during the initial frame.
        for leader in &mut store_catalog.leaders {
            leader.direct_product_id.clear();
            leader.direct_price_label.clear();
        }
        for skin in &mut store_catalog.skins {
            skin.direct_product_id.clear();
            skin.direct_price_label.clear();
        }
        let leaders: Vec<serde_json::Value> = sow_core::player::Leader::ALL
            .into_iter()
            .map(|leader| {
                let slug = sow_data::commerce::leader_id(leader);
                let perk_slug = match leader {
                    sow_core::player::Leader::RichardTheLionheart => "richard".to_string(),
                    _ => slug.replace('_', ""),
                };
                let offer = store_catalog
                    .leaders
                    .iter()
                    .find(|offer| offer.id.as_str() == slug)
                    .expect("leader catalog must contain every leader");
                serde_json::json!({
                    "id": leader_id(leader),
                    "name": leader.name(),
                    "civilization_key": format!(
                        "heroes.civilization_{}",
                        match leader.civilization() {
                            sow_core::player::Civilization::Rome => "rome",
                            sow_core::player::Civilization::Egypt => "egypt",
                            sow_core::player::Civilization::Vikings => "vikings",
                            sow_core::player::Civilization::China => "china",
                            sow_core::player::Civilization::Macedon => "macedon",
                            sow_core::player::Civilization::Mongols => "mongols",
                            sow_core::player::Civilization::Angevin => "angevin",
                            sow_core::player::Civilization::Gallic => "gallic",
                            sow_core::player::Civilization::Iceni => "iceni",
                            sow_core::player::Civilization::Maya => "maya",
                            sow_core::player::Civilization::Sparta => "sparta",
                            sow_core::player::Civilization::France => "france",
                        }
                    ),
                    "perk_key": format!(
                        "site.leader_{}_description",
                        perk_slug
                    ),
                    "slug": slug,
                    "free_rotation": offer.free_rotation,
                    "owned": offer.owned,
                    "available": offer.available,
                    "cost_crowns": offer.cost_crowns,
                    "cost_gems": offer.cost_gems,
                })
            })
            .collect();
        let map_catalog: Vec<serde_json::Value> = app
            .ui
            .app
            .asset_loader
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
            "phase": phase_name(app.ui.app.phase),
            "loader_cycle_id": app.ui.app.splash_state.cycle_id,
            "loader_job": splash_job_name(&app.ui.app.splash_state.job),
            "loader_leader": app.ui.app.splash_state.loader_leader.map(leader_id),
            "loader_progress": app.ui.app.splash_state.progress.clamp(0.0, 1.0),
            "loader_status": "",
            "loader_done": app.ui.app.splash_state.done,
            "boot_campaign": app.boot_campaign_pending.clone(),
            "connected": state.is_connected,
            "connecting": state.is_connecting,
            "waiting": state.is_waiting,
            "player_name": state.player_name,
            "selected_leader": leader_id(state.selected_leader),
            "selected_leader_name": state.selected_leader.name(),
            "selected_civilization": state.selected_civilization.name(),
            "show_browser": matches!(
                state.visible_route(),
                crate::ui::main_menu::MainMenuRoute::Browser
            ),
            "show_create": matches!(
                state.visible_route(),
                crate::ui::main_menu::MainMenuRoute::Create
            ),
            "join_lobby_code": state.join_lobby_code,
            "joined_lobby_id": state.joined_lobby_id,
            "pending_lobby_id": state.pending_join_lobby_id,
            "is_lobby_host": state.is_lobby_host,
            "my_player_id": state.my_player_id,
            "downloading_map": state.is_downloading_map,
            "map_download_progress": state.map_download_progress,
            "lobbies": state.lobbies,
            "error": state.error_message.as_ref().map(localized_text_payload),
            "notice": notice_name(state.notice),
            "level": progress.level,
            "xp": progress.xp,
            "crowns": progress.crowns,
            "laurels": progress.laurels,
            "gems": progress.gems,
            "reward_receipts": progress.reward_receipts.values().collect::<Vec<_>>(),
            "exit_reward_preview": app.exit_reward_preview.as_ref().map(|preview| {
                serde_json::json!({
                    "receipt_id": preview.receipt_id,
                    "account_id": preview.account_id,
                    "xp": preview.reward.xp,
                    "leader_xp": preview.reward.leader_xp,
                    "crowns": preview.reward.crowns,
                    "laurels": preview.reward.laurels,
                    "base_xp": preview.base_xp,
                    "base_level": preview.base_level,
                    "base_crowns": preview.base_crowns,
                    "base_laurels": preview.base_laurels,
                })
            }),
            "achievements": progress.unlocked_achievements,
            "campaign": campaign_payload(progress),
            "selected_skin": progress.selected_skin,
            "store_busy": state.store_busy,
            "store": store_catalog,
            "native_purchase_scheme": "sow://purchase",
            "native_restore_scheme": "sow://restore",
            "account_id": app.progress_account_id,
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
            "custom_game_config": &*state.custom_game_config,
            "custom_game_is_private": state.custom_game_is_private,
            "custom_game_is_sp": state.custom_game_is_sp,
            "hud": serde_json::Value::Null,
            "settings": {
                "mute_all": app.ui.app.settings_state.mute_all,
                "music_volume": app.ui.app.settings_state.music_volume,
                "reduced_motion": app.ui.app.settings_state.reduced_motion,
                "free_zoom_out": app.ui.app.settings_state.free_zoom_out,
                "sticky_building_mode": app.ui.app.settings_state.sticky_building_mode,
                "show_dev_tools": app.ui.app.settings_state.show_dev_tools,
            },
        })
    };

    let Ok(serialized) = serde_json::to_string(&payload) else {
        log::warn!("[WEB MENU] failed to serialize state");
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
            let _ = func.call1(window.as_ref(), &js_str);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sow_core::game::GamePhase;
    use sow_core::player::{Civilization, Leader, PlayerType};
    use sow_core::protocol::{PlayerSnapshot, SimSnapshot};
    use std::sync::Arc;

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
            active_emoji: None,
            traitor: false,
            civilization: Civilization::Rome,
            leader: Leader::Caesar,
            campaign_avatar: None,
            skin_style: 0,
            kills: 0,
            deaths: 0,
            assists: 0,
            boats_in_use: 0,
            boat_capacity: 1,
            nuke_available: false,
            nuke_cooldown_ticks: 0,
        }
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
        let mut candidates = vec![
            (true, 20, 0, 1),
            (false, 2, 1, 9),
            (false, 6, 1, 8),
        ];
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
        assert_eq!(distances[24], 3, "distance beyond the relevant range is capped");
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
        engine.add_fleet(WarpFleet::new(10, 2, 1, UnitType::TransportShip, 40.0, (0, 4), vec![4]));
        engine.add_fleet(WarpFleet::new(11, 2, 0, UnitType::TransportShip, 40.0, (0, 4), vec![4]));
        engine.add_fleet(WarpFleet::new(12, 2, 1, UnitType::Warship, 40.0, (0, 4), vec![4]));

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
    fn combat_operations_join_only_troop_transports_to_local_targets() {
        use sow_core::game::{GameState, UnitType};
        use sow_core::game_config::GameConfig;
        use sow_core::protocol::FleetSnapshot;
        use sow_core::warp_fleet::WarpFleet;
        use sow_core::water_components::WaterComponents;

        let mut snapshot = test_snapshot(vec![
            test_player(1, 5, 100.0),
            test_player(2, 5, 100.0),
        ]);
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

        let state = GameState::new(1, 5, 5, GameConfig::default());
        let water = WaterComponents::compute(&state.map, |_| {});
        let mut engine = sow_core::engine::SowEngine::new(state, water);
        engine.add_fleet(WarpFleet::new(10, 1, 0, UnitType::TransportShip, 40.0, (0, 6), vec![6]));
        engine.add_fleet(WarpFleet::new(11, 2, 1, UnitType::TransportShip, 40.0, (0, 6), vec![6]));
        engine.add_fleet(WarpFleet::new(12, 1, 2, UnitType::TradeShip, 40.0, (0, 6), vec![6]));
        engine.add_fleet(WarpFleet::new(13, 1, 2, UnitType::Warship, 40.0, (0, 6), vec![6]));

        let operations = build_combat_operations_payload(&snapshot, 1, Some(&engine), 5);
        assert_eq!(operations.len(), 2);
        assert_eq!(operations[0]["direction"], "incoming");
        assert_eq!(operations[0]["kind"], "fleet");
        assert_eq!(operations[0]["id"], 11);
        assert_eq!(operations[1]["direction"], "outgoing");
        assert_eq!(operations[1]["id"], 10);
        assert_eq!(operations[1]["neutral"], true);
    }

    #[test]
    fn building_metrics_keep_effect_values_compact_and_accurate() {
        let port = building_metrics(
            sow_core::game::BuildingKind::Port,
            2,
            &sow_core::game_config::GameConfig::default(),
        );
        assert!(
            port.iter()
                .any(|metric| { metric["icon"] == "port" && metric["value"] == 2.0 })
        );
        assert!(port.iter().any(|metric| {
            metric["icon"] == "troops" && metric["value"] == 25.0 && metric["unit"] == "/s"
        }));
        let bunker = building_metrics(
            sow_core::game::BuildingKind::Bunker,
            2,
            &sow_core::game_config::GameConfig::default(),
        );
        assert!(
            bunker
                .iter()
                .any(|metric| { metric["icon"] == "defense" && metric["value"] == 10.0 })
        );
        assert!(
            bunker
                .iter()
                .any(|metric| { metric["icon"] == "range" && metric["value"] == 16.0 })
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
