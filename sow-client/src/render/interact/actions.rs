use crate::UiAction;
use crate::app::SowApp;
use crate::spawn_sow_client_connect;

impl SowApp {
    pub(crate) fn process_ui_actions(&mut self, action: Option<crate::UiAction>) {
        if let Some(action) = action {
            match action {
                UiAction::StartSinglePlayer(config) => {
                    self.start_offline_match(*config, false);
                }
                UiAction::ConnectToServer(addr) => {
                    self.ui.app.main_menu_state.is_connecting = true;
                    let url = addr.clone();
                    spawn_sow_client_connect(url, &self.net.connect_tx);
                }
                UiAction::RetryConnection => {
                    self.ui.app.main_menu_state.error_message = None;
                    self.ui.app.main_menu_state.notice = None;
                    self.ui.app.main_menu_state.is_connecting = true;
                    let url = self.ui.app.main_menu_state.server_address.clone();
                    spawn_sow_client_connect(url, &self.net.connect_tx);
                }
                UiAction::JoinLobby(id) => {
                    if self.ui.app.main_menu_state.is_waiting {
                        return;
                    }
                    self.request_join(Some(id), false, None, None);
                    self.ui.app.main_menu_state.go_home();
                }
                UiAction::HostPrivateLobby => {
                    self.request_join(None, true, None, None);
                }
                UiAction::OpenCreateGame => {
                    self.ui
                        .app
                        .main_menu_state
                        .open_route(crate::ui::main_menu::MainMenuRoute::Create);
                    self.ui.app.main_menu_state.custom_game_is_sp = false;
                    self.ui.app.main_menu_state.error_message = None;
                }
                UiAction::OpenJoinBrowser => {
                    self.ui
                        .app
                        .main_menu_state
                        .open_route(crate::ui::main_menu::MainMenuRoute::Browser);
                    self.ui.app.main_menu_state.error_message = None;
                }
                UiAction::CloseOverlay => {
                    self.ui.app.main_menu_state.go_home();
                }
                UiAction::CreateGame {
                    config,
                    is_private,
                    password,
                } => {
                    self.request_join(None, is_private, Some(config), password);
                    self.ui.app.main_menu_state.go_home();
                }
                UiAction::JoinWithCode => {
                    let code = self
                        .ui
                        .app
                        .main_menu_state
                        .join_lobby_code
                        .trim()
                        .to_string();
                    if let Ok(lobby_id) = code.parse::<u64>() {
                        self.request_join(Some(lobby_id), false, None, None);
                        self.ui.app.main_menu_state.go_home();
                    } else {
                        self.ui.app.main_menu_state.error_message =
                            Some(crate::ui::UiText::new("menu.invalid_lobby_code"));
                    }
                }
                UiAction::JoinWithPassword(lobby_id) => {
                    let password = self.ui.app.main_menu_state.join_password_input.clone();
                    let pw = if password.is_empty() {
                        None
                    } else {
                        Some(password)
                    };
                    self.request_join(Some(lobby_id), false, None, pw);
                    self.ui.app.main_menu_state.go_home();
                    self.ui.app.main_menu_state.join_password_for_lobby = None;
                    self.ui.app.main_menu_state.join_password_input.clear();
                }
                UiAction::LeaveLobby => {
                    self.input.camera_x = 0.0;
                    self.input.camera_y = 0.0;
                    self.input.camera_zoom = 2.0;
                    self.input.target_zoom = 2.0;
                    self.leave_lobby_to_main_menu();
                }
                UiAction::ReturnToMenu => {
                    self.input.camera_x = 0.0;
                    self.input.camera_y = 0.0;
                    self.input.camera_zoom = 2.0;
                    self.input.target_zoom = 2.0;
                    self.send_leave_message();
                    self.begin_exit_to_main_menu();
                }
                UiAction::SetAttackRatio(r) => {
                    self.ui.app.hud_state.attack_ratio = r;
                }
                UiAction::CenterCamera => {
                    let pid = self.sim.my_player_id.unwrap_or(1);
                    if let Some(player) = self
                        .sim
                        .current_snapshot
                        .as_ref()
                        .and_then(|s| s.players.iter().find(|p| p.id == pid))
                        && player.tile_count > 0
                        && player.alive
                    {
                        let cx = player.centroid_x;
                        let cy = player.centroid_y;

                        let world_cx = cx + 0.5;
                        let world_cy = cy + 0.5;

                        self.input.camera_focus_target = Some((world_cx, world_cy));
                        self.input.target_zoom = 10.0;
                    }
                }
                UiAction::FocusTile(col, row) => {
                    let world_cx = col + 0.5;
                    let world_cy = row + 0.5;

                    // Zoom in to a comfortable battle-focus level
                    let target_zoom = 3.0_f32.max(self.input.camera_zoom);
                    self.input.camera_zoom = target_zoom;
                    self.input.target_zoom = target_zoom;
                    self.input.camera_x = self.input.screen_w * 0.5 - world_cx * target_zoom;
                    self.input.camera_y = self.input.screen_h * 0.5 - world_cy * target_zoom;
                    self.clamp_camera_to_map();
                }
                UiAction::ToggleDevSidebar => {
                    self.ui.show_dev_sidebar = !self.ui.show_dev_sidebar;
                    if self.ui.show_dev_sidebar {
                        self.ui.show_leaderboard = false;
                    }
                }
                UiAction::ToggleSettings => {
                    // Handle settings toggle if it's there
                }
                UiAction::ToggleCredits => {
                    self.ui.app.is_credits_open = !self.ui.app.is_credits_open;
                }
                UiAction::TogglePrivacy => {
                    self.ui.app.is_privacy_open = !self.ui.app.is_privacy_open;
                }
                UiAction::ToggleTerms => {
                    self.ui.app.is_terms_open = !self.ui.app.is_terms_open;
                }
                UiAction::ToggleShowcase => {
                    self.ui.app.is_showcase_open = !self.ui.app.is_showcase_open;
                }
                UiAction::OpenStorePage => {
                    self.ui
                        .app
                        .main_menu_state
                        .open_route(crate::ui::main_menu::MainMenuRoute::Store);
                    self.ui.app.main_menu_state.error_message = None;
                }
                UiAction::OpenProfilePage => {
                    self.ui
                        .app
                        .main_menu_state
                        .open_route(crate::ui::main_menu::MainMenuRoute::Profile);
                    self.ui.app.main_menu_state.profile.account_id =
                        self.profile_account_id.clone();
                    self.ui.app.main_menu_state.profile.error = None;
                    self.ui.app.main_menu_state.profile.view = None;
                    self.ui.app.main_menu_state.profile.history.clear();
                    self.ui.app.main_menu_state.profile.ratings.clear();
                    self.ui.app.main_menu_state.profile.search_results.clear();
                    self.ui.app.main_menu_state.profile.history_cursor = 0;
                    self.ui.app.main_menu_state.profile.history_has_next = false;
                    self.ui.app.main_menu_state.profile.ratings_loaded = false;
                    self.ui.app.main_menu_state.profile.match_detail = None;
                    self.ui.app.main_menu_state.profile.active_tab =
                        crate::ui::main_menu::profile::ProfileTab::Overview;
                    self.ui.app.main_menu_state.profile.loading = false;
                }
                UiAction::LoadOwnProfile => {
                    self.load_profile();
                }
                UiAction::OpenPublicProfilePage(account_id) => {
                    self.ui
                        .app
                        .main_menu_state
                        .open_route(crate::ui::main_menu::MainMenuRoute::Profile);
                    self.ui.app.main_menu_state.profile.account_id = Some(account_id);
                    self.ui.app.main_menu_state.profile.view = None;
                    self.ui.app.main_menu_state.profile.history.clear();
                    self.ui.app.main_menu_state.profile.ratings.clear();
                    self.ui.app.main_menu_state.profile.history_cursor = 0;
                    self.ui.app.main_menu_state.profile.history_has_next = false;
                    self.ui.app.main_menu_state.profile.ratings_loaded = false;
                    self.ui.app.main_menu_state.profile.match_detail = None;
                    self.ui.app.main_menu_state.profile.error = None;
                    self.ui.app.main_menu_state.profile.loading = false;
                }
                UiAction::LoadProfileHistory => {
                    self.load_profile_history();
                }
                UiAction::LoadProfileRatings => {
                    self.load_profile_ratings();
                }
                UiAction::SearchProfiles(query) => {
                    self.search_profiles(query);
                }
                UiAction::LoadMatchDetail(match_id) => {
                    self.load_match_detail(match_id);
                }
                UiAction::CloseMatchDetail => {
                    self.ui.app.main_menu_state.profile.match_detail = None;
                }
                UiAction::UnlockLeader {
                    leader_id,
                    currency,
                } => {
                    self.unlock_leader(leader_id, currency);
                }
                UiAction::UnlockSkin(skin_id) => {
                    self.unlock_skin(skin_id);
                }
                UiAction::EquipSkin(skin_id) => {
                    self.equip_skin(skin_id);
                }
                UiAction::ZoomIn => {
                    self.process_camera_zoom(
                        1.25,
                        self.input.screen_w * 0.5,
                        self.input.screen_h * 0.5,
                    );
                }
                UiAction::ZoomOut => {
                    self.process_camera_zoom(
                        0.8,
                        self.input.screen_w * 0.5,
                        self.input.screen_h * 0.5,
                    );
                }
                UiAction::StartPrivateLobby(lobby_id) => {
                    if let (Some(c), Some(player_id)) =
                        (self.net.client.as_ref(), self.sim.my_player_id)
                    {
                        let msg = sow_core::protocol::ClientMessage::ForceStart {
                            lobby_id,
                            player_id,
                        };
                        if let Ok(json) = bincode::serialize(&msg) {
                            c.send(json);
                        }
                    }
                }
                UiAction::KickPlayer {
                    lobby_id,
                    target_player_id,
                } => {
                    if let Some(c) = self.net.client.as_ref() {
                        let msg = sow_core::protocol::ClientMessage::Kick {
                            lobby_id,
                            target_player_id,
                        };
                        if let Ok(json) = bincode::serialize(&msg) {
                            c.send(json);
                        }
                    }
                }
                UiAction::BanPlayer {
                    lobby_id,
                    target_player_id,
                } => {
                    if let Some(c) = self.net.client.as_ref() {
                        let msg = sow_core::protocol::ClientMessage::Ban {
                            lobby_id,
                            target_player_id,
                        };
                        if let Ok(json) = bincode::serialize(&msg) {
                            c.send(json);
                        }
                    }
                }
                UiAction::MovePlayerTeam {
                    lobby_id,
                    target_player_id,
                } => {
                    if let Some(c) = self.net.client.as_ref() {
                        let msg = sow_core::protocol::ClientMessage::SetPlayerTeam {
                            lobby_id,
                            target_player_id,
                        };
                        if let Ok(json) = bincode::serialize(&msg) {
                            c.send(json);
                        }
                    }
                }
                UiAction::PortalShowAuthPrompt => {
                    crate::store_portals::show_auth_prompt();
                }
                UiAction::SaveDisplayName(display_name) => {
                    self.save_display_name(display_name);
                }
            }
        }
    }

    pub(crate) fn request_join(
        &mut self,
        lobby_id: Option<u64>,
        is_private: bool,
        config: Option<Box<sow_core::game_config::GameConfig>>,
        password: Option<String>,
    ) {
        self.ui.app.main_menu_state.error_message = None;
        self.ui.app.main_menu_state.notice = None;
        let matchmaking_join = lobby_id.is_none() && !is_private && config.is_none();
        if matchmaking_join {
            crate::store_portals::measure("matchmaking", "queue", "interact");
        } else if lobby_id.is_some() {
            crate::store_portals::measure("lobby", "join", "interact");
        } else {
            crate::store_portals::measure("lobby", "create", "interact");
        }
        self.join_matchmaking = matchmaking_join;
        if let Some(cfg) = config {
            self.ui.app.main_menu_state.custom_game_config = cfg;
        }
        let selected_leader = self.ui.app.main_menu_state.selected_leader;
        self.ui
            .app
            .main_menu_state
            .set_selected_leader(selected_leader, false);
        self.ui.app.main_menu_state.custom_game_is_private = is_private;
        self.ui.app.main_menu_state.custom_game_password = password.clone().unwrap_or_default();
        self.ui.app.main_menu_state.pending_join_lobby_id = lobby_id;
        self.ui.app.main_menu_state.is_waiting = true;
        self.ui.app.main_menu_state.is_lobby_host = if lobby_id.is_some() {
            false
        } else {
            !self.join_matchmaking
        };
        let identity_ready = self.progress_account_id.is_some() || self.net.is_offline;
        self.join_waiting_for_identity = !identity_ready || self.net.client.is_none();

        if let Some(c) = self.net.client.as_ref() {
            if identity_ready {
                let config_opt = (!self.join_matchmaking)
                    .then(|| self.ui.app.main_menu_state.custom_game_config.clone());
                let join_msg = self.make_join_message(lobby_id, is_private, config_opt, password);
                if let Some(join_msg) = join_msg
                    && let Ok(json) = bincode::serialize(&join_msg)
                {
                    c.send(json);
                    self.join_waiting_for_identity = false;
                } else {
                    self.join_waiting_for_identity = true;
                    log::info!("Queueing Join until the identity proof is ready");
                }
            } else {
                // Identity must settle before Join; otherwise the server sees a
                // second anonymous player on every refresh/race.
                log::info!("Queueing Join until the canonical account is loaded");
            }
        } else {
            log::info!(
                "No active connection, spawning lazy connection to {}",
                self.net.ws_url
            );
            self.ui.app.main_menu_state.is_connecting = true;
            let url = self.net.ws_url.clone();
            spawn_sow_client_connect(url, &self.net.connect_tx);
        }
    }
}
