use crate::ClientPhase;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplashJob {
    Boot,
    EnterGame,
    ExitGame,
}

pub struct SplashState {
    pub job: SplashJob,
    pub progress: f32,
    pub frames_drawn: u32,
    pub gpu_load_step: u8,
    pub done: bool,
    pub target_phase: Option<ClientPhase>,
}

impl Default for SplashState {
    fn default() -> Self {
        Self {
            job: SplashJob::Boot,
            progress: 0.0,
            frames_drawn: 0,
            gpu_load_step: 0,
            done: false,
            target_phase: None,
        }
    }
}

impl SplashState {
    pub fn reset_anim(&mut self, new_job: SplashJob) {
        self.job = new_job;
        self.done = false;
        self.target_phase = None;
        self.frames_drawn = 0;
        self.gpu_load_step = 0;
        self.progress = 0.0;
    }
}
