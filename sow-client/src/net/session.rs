use crate::app::SowApp;
use crate::get_build_version;
use crate::ui::loading_screen::SplashJob;
use crate::ClientPhase;

fn should_complete_boudica_intro_on_exit(
    tutorial_active: bool,
    is_offline: bool,
    campaign: crate::campaign::CampaignId,
    intro_completed: Option<bool>,
) -> bool {
    tutorial_active
        && is_offline
        && campaign == crate::campaign::CampaignId::Boudica
        && !intro_completed.unwrap_or(false)
}

fn should_use_exit_game_loader(phase: ClientPhase) -> bool {
    matches!(phase, ClientPhase::Playing)
}

impl SowApp {
    pub(crate) fn make_relay_ready_message(
        &self,
        lobby_id: u64,
        player_id: u16,
    ) -> Option<sow_core::protocol::ClientMessage> {
        self.sim
            .relay_ticket
            .clone()
            .map(|ticket| sow_core::protocol::ClientMessage::ReadyWithTicket {
                lobby_id,
                player_id,
                ticket,
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
        let payload = sow_core::protocol::JoinPayload {
            name: self.ui.app.main_menu_state.player_name.clone(),
            target_lobby_id,
            host_private,
            build_version: get_build_version(),
            clan_tag: self.ui.app.main_menu_state.clan_tag.clone(),
            civilization: self.ui.app.main_menu_state.selected_civilization,
            leader: self.ui.app.main_menu_state.selected_leader,
            host_config,
            password,
        };
        self.make_auth_proof().map(|auth| {
            sow_core::protocol::ClientMessage::JoinWithAuth {
                join: Box::new(payload),
                auth,
            }
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

    /// Tear down the current match and use ExitGame only for an active game.
    pub(crate) fn begin_exit_to_main_menu(&mut self) {
        let phase = self.ui.app.phase;
        let entering_game = matches!(phase, ClientPhase::Splash)
            && matches!(&self.ui.app.splash_state.job, SplashJob::EnterGame);
        let use_loader = should_use_exit_game_loader(phase);
        let was_playing = phase == crate::ClientPhase::Playing;
        let exiting_boudica_intro = should_complete_boudica_intro_on_exit(
            self.ui.tutorial_active,
            self.net.is_offline,
            self.ui.tutorial_campaign,
            self.progress.intro_completed,
        );
        if exiting_boudica_intro {
            crate::analytics::track("tutorial_exit_early");
            if self.progress.complete_tutorial_with_reward() {
                self.save_local_progress();
                self.persist_tutorial_completion();
                log::info!("tutorial: intro completed on early exit");
            }
        }
        if was_playing {
            if !self.progress_match_recorded {
                crate::store_portals::measure("match", "round", "abandon");
            }
            crate::store_portals::gameplay_stop();
        }
        crate::store_portals::left_room();
        self.net.is_offline = false;
        self.net.ws_url = self.net.orchestrator_url.clone();
        self.ui.app.main_menu_state.server_address = self.net.ws_url.clone();

        // Drop relay connection and force orchestrator reconnect
        self.net.client = None;
        self.net.current_ping_ms = None;
        self.ui.app.main_menu_state.is_connected = false;
        self.ui.app.main_menu_state.is_connecting = false;
        if entering_game {
            self.reset_game_session();
        }
        #[cfg(target_arch = "wasm32")]
        {
            self.web_exit_lobbies_ready = false;
        }
        while self.net.connect_rx.try_recv().is_ok() {}
        self.net.ws_connect_not_before = web_time::Instant::now();

        self.ui.app.main_menu_state.is_waiting = false;
        self.ui.app.main_menu_state.go_home();
        self.ui.app.main_menu_state.pending_join_lobby_id = None;
        self.ui.app.main_menu_state.joined_lobby_id = None;
        self.join_waiting_for_identity = false;
        self.join_matchmaking = false;
        self.ui.app.hud_state.sync_state = None;
        self.sim.my_lobby_id = None;
        self.sim.my_player_id = None;
        self.sim.relay_ticket = None;
        self.sim.relay_reconnect_ticket = None;
        if use_loader {
            self.ui.app.phase = ClientPhase::Splash;
            self.ui
                .app
                .splash_state
                .reset_anim(SplashJob::ExitGame);
        } else {
            self.ui.app.phase = ClientPhase::MainMenu;
        }
        self.ui.is_spectating = false;
        self.ui.endgame_cache = None;
        self.reset_progress_session();

        if was_playing
            && self.progress_account_id.is_some()
            && crate::store_portals::should_fetch_cloud_profile()
        {
            self.fetch_cloud_progress();
        }
    }

    /// Enter the EnterGame splash (fade-in, progress bar, fade-out to Playing).
    pub(crate) fn begin_enter_game_loader(&mut self) {
        self.ui.app.phase = crate::ClientPhase::Splash;
        self.ui
            .app
            .splash_state
            .reset_anim(crate::ui::loading_screen::SplashJob::EnterGame);
    }

    /// Whether the map/mover GPU path should paint this frame (hidden during splash loads).
    pub(crate) fn should_draw_world(&self) -> bool {
        use crate::ui::loading_screen::SplashJob;
        use crate::ClientPhase;

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
    use super::{should_complete_boudica_intro_on_exit, should_use_exit_game_loader};
    use crate::campaign::CampaignId;
    use crate::ClientPhase;

    #[test]
    fn exit_loader_only_runs_for_active_game() {
        assert!(!should_use_exit_game_loader(ClientPhase::MainMenu));
        assert!(should_use_exit_game_loader(ClientPhase::Playing));
        assert!(!should_use_exit_game_loader(ClientPhase::Splash));
    }

    #[test]
    fn only_unfinished_offline_boudica_tutorial_exit_completes_intro() {
        assert!(should_complete_boudica_intro_on_exit(
            true,
            true,
            CampaignId::Boudica,
            None
        ));
        assert!(!should_complete_boudica_intro_on_exit(
            true,
            false,
            CampaignId::Boudica,
            None
        ));
        assert!(!should_complete_boudica_intro_on_exit(
            false,
            true,
            CampaignId::Boudica,
            None
        ));
        assert!(!should_complete_boudica_intro_on_exit(
            true,
            true,
            CampaignId::SixSkyEp1,
            None
        ));
        assert!(!should_complete_boudica_intro_on_exit(
            true,
            true,
            CampaignId::Boudica,
            Some(true)
        ));
    }
}
