#[inline]
pub fn spring_overshoot(t: f32) -> f32 { 1.0 - (t * 7.5).cos() * (-3.5 * t).exp() }
pub const PANEL_Y_SLIDE: f32 = 80.0;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlideDir { Down, Right }
#[inline]
pub fn ease_out_quart(t: f32) -> f32 { 1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(4) }
pub const LEADER_PAGE_TURN_DURATION: f32 = 0.28;
pub const LEADER_PAGE_LOADING_MIN: f32 = 0.12;
pub const LEADER_PAGE_FADE_OUT_END: f32 = 0.55;
pub const LEADER_PAGE_FADE_IN_START: f32 = 0.45;
pub fn leader_page_turn_t(elapsed: f32) -> f32 { ease_out_quart((elapsed / LEADER_PAGE_TURN_DURATION).clamp(0.0, 1.0)) }
pub fn leader_page_out_alpha(turn_t: f32) -> f32 { 1.0 - ease_out_quart((turn_t / LEADER_PAGE_FADE_OUT_END).clamp(0.0, 1.0)) }
pub fn leader_page_in_alpha(turn_t: f32) -> f32 { ease_out_quart(((turn_t - LEADER_PAGE_FADE_IN_START) / (1.0 - LEADER_PAGE_FADE_IN_START)).clamp(0.0, 1.0)) }
