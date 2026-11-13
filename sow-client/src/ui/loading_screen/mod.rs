use crate::ClientPhase;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplashJob {
    Boot,
    EnterGame,
    ExitGame,
}

pub struct SplashState {
    pub cycle_id: u32,
    pub job: SplashJob,
    pub loader_leader: Option<sow_core::player::Leader>,
    pub use_random_art: bool,
    pub show_hero_briefing: bool,
    pub progress: f32,
    pub frames_drawn: u32,
    pub gpu_load_step: u8,
    pub done: bool,
    pub target_phase: Option<ClientPhase>,
}

impl Default for SplashState {
    fn default() -> Self {
        Self {
            cycle_id: 0,
            job: SplashJob::Boot,
            loader_leader: None,
            use_random_art: false,
            show_hero_briefing: false,
            progress: 0.0,
            frames_drawn: 0,
            gpu_load_step: 0,
            done: false,
            target_phase: None,
        }
    }
}

impl SplashState {
    fn next_cycle(&mut self) {
        self.cycle_id = self
            .cycle_id
            .checked_add(1)
            .expect("loader cycle id exhausted");
    }

    pub fn reset_anim(&mut self, new_job: SplashJob) {
        self.next_cycle();
        self.job = new_job;
        self.loader_leader = None;
        self.use_random_art = false;
        self.show_hero_briefing = false;
        self.done = false;
        self.target_phase = None;
        self.frames_drawn = 0;
        self.gpu_load_step = 0;
        self.progress = 0.0;
    }

    pub fn transition_anim(&mut self, new_job: SplashJob, leader: sow_core::player::Leader) {
        self.next_cycle();
        self.job = new_job;
        self.loader_leader = Some(leader);
        self.use_random_art = false;
        self.show_hero_briefing = false;
        self.done = false;
        self.target_phase = None;
        self.frames_drawn = 0;
        self.gpu_load_step = 0;
    }
}

