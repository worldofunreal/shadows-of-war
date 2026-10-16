use crate::ClientPhase;
use crate::app::SowApp;
use crate::get_build_version;
use crate::net::lobby::clear_lobby_snapshot;
use crate::ui::loading_screen::SplashJob;

fn should_use_exit_game_loader(phase: ClientPhase) -> bool {
    matches!(phase, ClientPhase::Playing)
}

impl SowApp {
    pub(crate) fn make_relay_ready_message(
        &self,
        lobby_id: u64,
        player_id: u16,
    ) -> Option<sow_core::protocol::ClientMessage> {
        self.sim.relay_ticket.clone().map(|ticket| {
            sow_core::protocol::ClientMessage::ReadyWithTicket {
                lobby_id,
                player_id,
                ticket,
            }
        })
    }

    pub(crate) fn make_initial_relay_ready_message(
        &self,
        lobby_id: u64,
        player_id: u16,
    ) -> Option<sow_core::protocol::ClientMessage> {
        self.make_relay_ready_message(lobby_id, player_id)
    }

    pub(crate) fn make_reconnect_message(
        &self,
        lobby_id: u64,
        player_id: u16,
    ) -> Option<sow_core::protocol::ClientMessage> {
        if let Some(ticket) = self.sim.relay_reconnect_ticket.clone() {
            Some(sow_core::protocol::ClientMessage::ReconnectWithTicket {
                lobby_id,
                player_id,
                ticket,
            })
        } else {
            None
        }
    }

    pub(crate) fn make_join_message(
        &self,
        target_lobby_id: Option<u64>,
        host_private: bool,
        host_config: Option<Box<sow_core::game_config::GameConfig>>,
        password: Option<String>,
    ) -> Option<sow_core::protocol::ClientMessage> {
        let selected_leader = self.ui.app.main_menu_state.selected_leader;
        let selected_civilization = self.ui.app.main_menu_state.selected_civilization;
        let random_leader = self.ui.app.main_menu_state.prefer_random_leader;
        let host_config = host_config.map(|mut config| {
            config.player_leader = selected_leader;
            config.player_civilization = selected_civilization;
            config
        });
        let payload = sow_core::protocol::JoinPayload {
            name: self.ui.app.main_menu_state.player_name.clone(),
            target_lobby_id,
            host_private,
            build_version: get_build_version(),
            clan_tag: self.ui.app.main_menu_state.clan_tag.clone(),
            civilization: selected_civilization,
            leader: selected_leader,
            random_leader,
            host_config,
            password,
        };
        self.make_auth_proof()
            .map(|auth| sow_core::protocol::ClientMessage::JoinWithAuth {
                join: Box::new(payload),
                auth,
            })
    }

    /// Identity proof for JoinWithAuth: the CrazyGames platform token for
    /// signed-in portal users, or the anonymous account secret. Online joins
    /// wait until one of these proofs is available.
    fn make_auth_proof(&self) -> Option<sow_core::protocol::AuthProof> {
        if self.net.is_offline {
            return None;
        }
        let identity = crate::store_portals::load_identity("Player");
        if identity.provider == "wou" {
            let token = identity.auth_token.clone().filter(|t| !t.is_empty())?;
            return Some(sow_core::protocol::AuthProof {
                provider: "wou".to_string(),
                account_id: identity.external_id.clone(),
                token,
            });
        }
        if identity.provider == "crazygames" {
            let token = identity.auth_token.clone().filter(|t| !t.is_empty())?;
            return Some(sow_core::protocol::AuthProof {
                provider: "crazygames".to_string(),
                account_id: None,
                token,
            });
        }
        if identity.provider == "playgames" {
            let token = identity.auth_token.clone().filter(|t| !t.is_empty())?;
            return Some(sow_core::protocol::AuthProof {
                provider: "playgames".to_string(),
                account_id: identity.external_id.clone(),
                token,
            });
        }
        let account_id = crate::anonymous_identity::load_account_id()?;
        let token = crate::anonymous_identity::load_account_secret()?;
        Some(sow_core::protocol::AuthProof {
            provider: "anonymous".to_string(),
            account_id: Some(account_id),
            token,
        })
    }

    pub(crate) fn sync_portal_room(&self, joinable: bool) {
        if let Some(id) = self.ui.app.main_menu_state.joined_lobby_id {
            crate::store_portals::update_room(id, joinable, &get_build_version());
        } else if !joinable {
            crate::store_portals::left_room();
        }
    }

    pub(crate) fn send_leave_message(&mut self) {
        if let Some(client) = self.net.client.as_ref() {
            let leave = sow_core::protocol::ClientMessage::Leave {};
            if let Ok(json) = bincode::serialize(&leave) {
                client.send(json);
            }
        }
    }

    /// Leave a waiting lobby without replacing the live orchestrator connection.
    pub(crate) fn leave_lobby_to_main_menu(&mut self) {
        self.send_leave_message();
        self.finish_lobby_exit_to_main_menu();
    }

    /// Apply the local half of a lobby exit after the server has already closed it.
    pub(crate) fn finish_lobby_exit_to_main_menu(&mut self) {
        crate::store_portals::left_room();
        self.net.is_offline = false;
        self.net.ws_url = self.net.orchestrator_url.clone();
        self.ui.app.main_menu_state.server_address = self.net.ws_url.clone();
        self.net.relay_handoff_done = false;
        self.clear_lobby_exit_state();
        self.ui.app.phase = ClientPhase::MainMenu;
        self.ui.is_spectating = false;
        self.ui.endgame_cache = None;
        self.prepare_menu_return("lobby", None);
    }

    fn clear_lobby_exit_state(&mut self) {
        self.ui.app.main_menu_state.is_waiting = false;
        clear_lobby_snapshot(&mut self.ui.app.main_menu_state);
        self.ui.app.main_menu_state.go_home();
        self.ui.app.main_menu_state.pending_join_lobby_id = None;
        self.ui.app.main_menu_state.joined_lobby_id = None;
        self.ui.app.main_menu_state.host_private_pending = false;
        self.ui.app.main_menu_state.in_private_match = false;
        self.ui.app.main_menu_state.is_lobby_host = false;
        self.ui.app.main_menu_state.my_player_id = None;
        self.join_waiting_for_identity = false;
        self.join_matchmaking = false;
        self.net.pending_lobby_rejoin = false;
        self.ui.app.hud_state.sync_state = None;
        self.sim.my_lobby_id = None;
        self.sim.my_player_id = None;
        self.sim.relay_ticket = None;
        self.sim.relay_reconnect_ticket = None;
    }

    /// Tear down the current match and use ExitGame only for an active game.
    pub(crate) fn begin_exit_to_main_menu(&mut self) {
        self.begin_menu_return(true);
    }

    pub(crate) fn begin_relay_closed_return(&mut self) {
        self.begin_menu_return(false);
    }

    fn begin_menu_return(&mut self, voluntary: bool) {
        self.ui.tutorial_camera_only = false;
        self.ui.tutorial_paused_action = None;
        self.ui.dialog_border_highlight = None;
        self.input.tutorial_camera_drag_recorded = false;
        let phase = self.ui.app.phase;
        let entering_game = matches!(phase, ClientPhase::Splash)
            && matches!(&self.ui.app.splash_state.job, SplashJob::EnterGame);
        let use_loader =
            should_use_exit_game_loader(phase) || (entering_game && !self.net.is_offline);
        let was_playing = phase == crate::ClientPhase::Playing;
        let was_offline = self.net.is_offline;
        let was_spectating = self.ui.is_spectating;
        let campaign_episode = if was_playing && was_offline && self.ui.tutorial_active {
            self.pending_campaign_exit.take()
        } else {
            None
        };
        if was_playing
            && was_offline
            && self.ui.tutorial_active
            && !self.ui.tutorial_campaign.is_completed(&self.progress)
        {
            crate::store_portals::track_product_event_with(
                "tutorial_exit_early",
                &serde_json::json!({ "episode_id": self.ui.tutorial_campaign.episode_id() }),
            );
        }
        let reward_match_id = ((was_playing || entering_game)
            && !self.net.is_offline
            && self.sim.my_player_id.is_some())
        .then(|| self.sim.my_lobby_id)
        .flatten();
        let tutorial_reward_id = (was_playing
            && was_offline
            && !was_spectating
            && self.ui.tutorial_active
            && self.ui.tutorial_campaign == crate::campaign::CampaignId::Boudica)
            .then(|| "tutorial".to_string());
        // Every active tutorial exit joins the same one-time server reward and
        // welcome handoff, including the HUD's Leave tutorial button.
        if tutorial_reward_id.is_some() {
            self.complete_boudica_tutorial();
        }
        let exit_leader = self.ui.app.splash_state.loader_leader;
        let exit_random_art = self.ui.app.splash_state.use_random_art;
        // Keep only this authenticated game socket until the durable acknowledgment.
        // Menu connection and result observation proceed independently.
        self.net.closing_game = if voluntary && reward_match_id.is_some() && self.ws_on_relay() {
            self.send_leave_message();
            self.net.client.take()
        } else {
            None
        };
        if was_playing {
            if !self.progress_match_recorded {
                crate::store_portals::track_product_event("match_exit");
            }
            crate::store_portals::gameplay_stop();
        }
        crate::store_portals::left_room();
        self.net.is_offline = false;
        self.net.ws_url = self.net.orchestrator_url.clone();
        self.ui.app.main_menu_state.server_address = self.net.ws_url.clone();
        self.net.relay_handoff_done = false;

        // Drop relay connection and force orchestrator reconnect
        self.net.client = None;
        self.net.current_ping_ms = None;
        self.ui.app.main_menu_state.is_connected = false;
        self.ui.app.main_menu_state.is_connecting = false;
        if entering_game {
            self.reset_game_session();
        }
        while self.net.connect_rx.try_recv().is_ok() {}
        self.net.ws_connect_not_before = web_time::Instant::now();
        crate::web_menu::wake_event_loop();

        self.clear_lobby_exit_state();
        if use_loader {
            self.ui.app.phase = ClientPhase::Splash;
            self.ui.app.splash_state.reset_anim(SplashJob::ExitGame);
            self.ui.app.splash_state.loader_leader = exit_leader;
            self.ui.app.splash_state.use_random_art = exit_random_art;
        } else {
            self.ui.app.phase = ClientPhase::MainMenu;
        }
        self.ui.is_spectating = false;
        self.ui.endgame_cache = None;
        self.reset_progress_session();

        let campaign_receipt_id = campaign_episode.map(|episode| format!("campaign:{episode}"));
        let receipt_id = tutorial_reward_id
            .clone()
            .or_else(|| campaign_receipt_id.clone())
            .or_else(|| reward_match_id.map(|id| id.to_string()));
        let source = if tutorial_reward_id.is_some() {
            "tutorial"
        } else if campaign_receipt_id.is_some() {
            "campaign"
        } else if reward_match_id.is_some() {
            "multiplayer"
        } else if was_playing && was_spectating {
            "spectator"
        } else {
            "other"
        };
        self.prepare_menu_return(source, receipt_id.clone());
        if source == "multiplayer" {
            let context = self.menu_return.as_mut().unwrap();
            context.voluntary_exit = Some(voluntary);
            context.exit_recorded = !voluntary;
            context.recording_deadline = Some(
                web_time::Instant::now()
                    + if self.net.closing_game.is_some() {
                        web_time::Duration::from_secs(1)
                    } else {
                        web_time::Duration::ZERO
                    },
            );
            self.save_menu_return_context();
        }

        if (was_playing || reward_match_id.is_some())
            && (crate::store_portals::should_fetch_cloud_profile() || campaign_receipt_id.is_some())
        {
            if let Some(receipt_id) = receipt_id {
                self.begin_reward_profile_sync(Some(receipt_id), !was_offline);
            } else {
                self.fetch_cloud_progress();
            }
        }
    }

    /// Enter the EnterGame splash (fade-in, progress bar, fade-out to Playing).
    pub(crate) fn begin_enter_game_loader(&mut self, leader: sow_core::player::Leader) {
        let reuse_enter_game_loader = self.ui.app.phase == ClientPhase::Splash
            && self.ui.app.splash_state.job == SplashJob::EnterGame
            && self.ui.app.splash_state.loader_leader == Some(leader)
            && !self.ui.app.splash_state.done;
        if reuse_enter_game_loader {
            return;
        }
        let reuse_boot_loader = self.ui.app.phase == ClientPhase::Splash
            && self.ui.app.splash_state.job == SplashJob::Boot
            && !self.ui.app.splash_state.done;
        self.ui.app.phase = crate::ClientPhase::Splash;
        if reuse_boot_loader {
            self.ui
                .app
                .splash_state
                .transition_anim(crate::ui::loading_screen::SplashJob::EnterGame, leader);
        } else {
            self.ui
                .app
                .splash_state
                .reset_anim(crate::ui::loading_screen::SplashJob::EnterGame);
            self.ui.app.splash_state.loader_leader = Some(leader);
        }
    }

    /// Whether the map/mover GPU path should paint this frame (hidden during splash loads).
    pub(crate) fn should_draw_world(&self) -> bool {
        use crate::ClientPhase;
        use crate::ui::loading_screen::SplashJob;

        match self.ui.app.phase {
            ClientPhase::Playing => true,
            ClientPhase::Splash => {
                let s = &self.ui.app.splash_state;
                matches!(s.job, SplashJob::EnterGame) && s.done
            }
            ClientPhase::MainMenu => false,
        }
    }

    #[inline]
    pub(crate) fn ws_on_relay(&self) -> bool {
        self.net.relay_handoff_done
            || self.net.ws_url.contains("/relay/")
            || self.net.ws_url.contains("relay.shadowsofwar.io")
            || self.net.ws_url.contains("2557")
    }
}

#[cfg(test)]
mod tests {
    use super::should_use_exit_game_loader;
    use crate::ClientPhase;

    #[test]
    fn exit_loader_only_runs_for_active_game() {
        assert!(!should_use_exit_game_loader(ClientPhase::MainMenu));
        assert!(should_use_exit_game_loader(ClientPhase::Playing));
        assert!(!should_use_exit_game_loader(ClientPhase::Splash));
    }
}
