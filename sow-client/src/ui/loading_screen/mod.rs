use crate::ClientPhase;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplashJob { Boot, EnterGame, ExitGame }

pub struct SplashState {
    pub job: SplashJob, pub status_text: String, pub status_override: Option<String>, pub progress: f32,
    pub frames_drawn: u32, pub gpu_load_step: u8, pub done: bool, pub start_time: Option<f64>,
    pub fadeout_start: Option<f64>, pub opacity: f32, pub target_phase: Option<ClientPhase>,
    pub visual_progress: f32, pub last_update_time: Option<f64>, pub random_speed: f32,
}

impl Default for SplashState {
    fn default() -> Self { Self { job: SplashJob::Boot, status_text: String::new(), status_override: None,
        progress: 0.0, frames_drawn: 0, gpu_load_step: 0, done: false, start_time: None,
        fadeout_start: None, opacity: 1.0, target_phase: None, visual_progress: 0.0,
        last_update_time: None, random_speed: 0.0 } }
}

impl SplashState {
    pub fn reset_anim(&mut self, new_job: SplashJob, _lang: crate::ui::settings::Language) {
        self.job = new_job; self.done = false; self.start_time = None; self.fadeout_start = None;
        self.opacity = 1.0; self.target_phase = None; self.frames_drawn = 0; self.gpu_load_step = 0;
        self.progress = 0.0; self.visual_progress = 0.0; self.last_update_time = None;
        self.random_speed = 0.0; self.status_override = None; self.status_text.clear();
    }
}
