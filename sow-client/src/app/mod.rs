mod account;
pub(crate) mod audio;
mod bootstrap;
mod gpu;
mod progress;
pub(crate) mod sfx;
mod state;

#[cfg(all(test, target_arch = "wasm32"))]
pub(crate) use account::reward_exit_tests::{restore_tutorial_exit, setup_tutorial_exit};
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
        self.poll_reward_profile_sync(now);
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
}
