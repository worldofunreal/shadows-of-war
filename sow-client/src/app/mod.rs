mod account;
pub(crate) mod audio;
mod bootstrap;
mod gpu;
mod progress;
pub(crate) mod sfx;
mod state;

pub use state::*;

impl SowApp {
    pub fn update(&mut self, _event_loop: &dyn winit::event_loop::ActiveEventLoop) {
        self.check_surface();

        let now = web_time::Instant::now();
        self.sfx
            .set_client_active(self.ui.app.phase == crate::ClientPhase::Playing);
        #[cfg(target_arch = "wasm32")]
        self.process_web_menu_commands();
        self.update_net(now);
        self.update_assets();
        self.update_loader();
        self.send_game_ready_if_needed();
        self.poll_reward_profile_sync(now);
        self.poll_identity_profile_retry(now);
        self.poll_profile_actions(now);
        self.poll_pointer_hold();
        let sim_started_at = crate::diag::startup_perf_timer();
        self.update_sim(now);
        if let Some(started_at) = sim_started_at {
            crate::diag::record_startup_perf(crate::diag::StartupMetric::UpdateSim, started_at);
        }
        crate::diag::finish_startup_perf_window_if_due(now);
        if self.ui.app.phase != crate::ClientPhase::Playing {
            crate::web_menu::publish_state(self);
        }
    }

    fn send_game_ready_if_needed(&mut self) {
        if self.ui.app.phase != crate::ClientPhase::Playing
            || self.net.is_offline
            || !self.ws_on_relay()
            || self.net.game_ready_sent
        {
            return;
        }
        let (Some(client), Some(lobby_id), Some(player_id)) = (
            self.net.client.as_ref(),
            self.sim.my_lobby_id,
            self.sim.my_player_id,
        ) else {
            return;
        };
        let message = sow_core::protocol::ClientMessage::GameReady {};
        if let Ok(frame) = bincode::serialize(&message) {
            client.send(frame);
            self.net.game_ready_sent = true;
            self.ui.app.hud_state.awaiting_players = true;
            log::info!("[CLIENT NET] Game load complete; waiting for relay turn lobby={lobby_id} player={player_id}");
        }
    }
}
