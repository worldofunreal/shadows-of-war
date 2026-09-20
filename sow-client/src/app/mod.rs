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
        self.poll_pointer_hold();
        self.update_sim(now);
        if self.ui.app.phase != crate::ClientPhase::Playing {
            crate::web_menu::publish_state(self);
        }
    }
}
