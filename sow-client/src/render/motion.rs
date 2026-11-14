//! Small shared curves for floating rewards and nameplate emotes.
use sow_core::emote::{EMOTE_DURATION_SECS, EmoteKind};

#[inline]
pub(crate) fn spring_overshoot(t: f32) -> f32 {
    1.0 - (t * 7.5).cos() * (-3.5 * t).exp()
}

#[inline]
pub(crate) fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct EmoteMotion {
    /// Offset in avatar diameters, scale about its existing center.
    pub offset: [f32; 2],
    pub scale: [f32; 2],
    pub bubble_alpha: f32,
    pub bubble_scale: f32,
}

impl Default for EmoteMotion {
    fn default() -> Self {
        Self {
            offset: [0.0; 2],
            scale: [1.0; 2],
            bubble_alpha: 0.0,
            bubble_scale: 1.0,
        }
    }
}

fn hop(age: f32, start: f32, apex: f32, end: f32, height: f32) -> f32 {
    if age < start || age >= end {
        return 0.0;
    }
    if age < apex {
        return -height * smoothstep((age - start) / (apex - start));
    }
    let t = (age - apex) / (end - apex);
    -height * (1.0 - spring_overshoot(t * 1.7)) * (1.0 - smoothstep(t))
}

/// Squash, lift and settle in half a second; offset is in font/icon heights.
pub(crate) fn troop_transfer_motion(age: f32) -> ([f32; 2], f32) {
    if !(0.0..0.5).contains(&age) {
        return ([1.0; 2], 0.0);
    }
    let anticipation = smoothstep(age / 0.06) * (1.0 - smoothstep((age - 0.06) / 0.09));
    let stretch = smoothstep((age - 0.06) / 0.08) * (1.0 - smoothstep((age - 0.14) / 0.22));
    (
        [
            1.0 + 0.10 * anticipation - 0.08 * stretch,
            1.0 - 0.14 * anticipation + 0.16 * stretch,
        ],
        hop(age, 0.06, 0.14, 0.5, 0.20),
    )
}

pub(crate) fn emote_motion(kind: EmoteKind, age: f32, reduced: bool, pinned: bool) -> EmoteMotion {
    if !age.is_finite() || age < 0.0 || (!pinned && age >= EMOTE_DURATION_SECS) {
        return EmoteMotion::default();
    }
    let entry = smoothstep((age - 0.08) / 0.16);
    // A pinned emote holds its bubble until it is released; only unpinned ones fade out by time.
    let exit = if pinned {
        1.0
    } else {
        1.0 - smoothstep((age - (EMOTE_DURATION_SECS - 0.3)) / 0.3)
    };
    let mut motion = EmoteMotion {
        bubble_alpha: entry * exit,
        ..EmoteMotion::default()
    };
    if reduced {
        return motion;
    }
    let entry_time = ((age - 0.08) / 0.16).clamp(0.0, 1.0);
    motion.bubble_scale =
        0.86 + 0.14 * (spring_overshoot(entry_time) / spring_overshoot(1.0)).clamp(0.0, 1.15);
    let anticipation = smoothstep(age / 0.07) * (1.0 - smoothstep((age - 0.07) / 0.11));
    let stretch = ((age - 0.07) / 0.11).clamp(0.0, 1.0) * (1.0 - smoothstep((age - 0.18) / 0.25));
    motion.scale = [
        1.0 + 0.04 * anticipation - 0.04 * stretch,
        1.0 - 0.08 * anticipation + 0.08 * stretch,
    ];
    match kind {
        EmoteKind::Greet => motion.offset[1] = hop(age, 0.07, 0.18, 0.5, 0.10),
        EmoteKind::Help => {
            motion.offset[1] = hop(age, 0.07, 0.18, 0.5, 0.04);
            let t = ((age - 0.18) / 0.32).clamp(0.0, 1.0);
            motion.offset[0] = 0.04 * anticipation
                + 0.015
                    * (t * std::f32::consts::TAU * 3.0).sin()
                    * (std::f32::consts::PI * t).sin();
        }
        EmoteKind::Threat => {
            motion.offset[1] = hop(age, 0.07, 0.18, 0.5, 0.08);
            let t = ((age - 0.24) / 0.26).clamp(0.0, 1.0);
            motion.offset[0] =
                0.02 * (t * std::f32::consts::TAU * 2.0).sin() * (std::f32::consts::PI * t).sin();
        }
        EmoteKind::Celebrate => {
            motion.offset[1] = hop(age, 0.07, 0.18, 0.38, 0.20) + hop(age, 0.38, 0.48, 0.65, 0.08);
        }
    }
    motion
}

const MAX_SHAKE_CSS_PX: f32 = 9.0;
const MAX_ACTIVE_SHAKES: usize = 8;

#[derive(Clone, Copy, Debug)]
pub(crate) struct ShakeImpulse {
    pub start_time: web_time::Instant,
    pub duration: f32,
    pub amplitude_css: f32,
    pub dir: [f32; 2],
    pub agitation: f32,
    pub seed: f32,
}

#[derive(Default)]
pub(crate) struct CameraShakeState {
    impulses: Vec<ShakeImpulse>,
    next_seed: f32,
}

impl CameraShakeState {
    pub(crate) fn push(
        &mut self,
        now: web_time::Instant,
        amplitude_css: f32,
        duration: f32,
        dir: [f32; 2],
        agitation: f32,
    ) {
        if !amplitude_css.is_finite()
            || !duration.is_finite()
            || amplitude_css <= 0.05
            || duration <= 0.01
        {
            return;
        }
        if self.impulses.len() >= MAX_ACTIVE_SHAKES {
            self.impulses.remove(0);
        }
        self.next_seed = (self.next_seed + 1.618_034) % 100.0;
        let len = (dir[0] * dir[0] + dir[1] * dir[1]).sqrt();
        let norm_dir = if len > 0.001 {
            [dir[0] / len, dir[1] / len]
        } else {
            [0.0, 0.0]
        };
        self.impulses.push(ShakeImpulse {
            start_time: now,
            duration,
            amplitude_css: amplitude_css.min(MAX_SHAKE_CSS_PX),
            dir: norm_dir,
            agitation: agitation.clamp(0.0, 1.0),
            seed: self.next_seed,
        });
    }

    pub(crate) fn clear(&mut self) {
        self.impulses.clear();
    }

    /// Returns render offset in physical pixels `[dx, dy]`.
    pub(crate) fn sample_physical(
        &mut self,
        now: web_time::Instant,
        sf: f32,
        reduced_motion: bool,
    ) -> [f32; 2] {
        if reduced_motion {
            self.impulses.clear();
            return [0.0, 0.0];
        }
        let mut total_x = 0.0f32;
        let mut total_y = 0.0f32;
        self.impulses.retain(|imp| {
            let elapsed = now.duration_since(imp.start_time).as_secs_f32();
            if elapsed < 0.0 || elapsed >= imp.duration {
                return false;
            }
            let p = (elapsed / imp.duration).clamp(0.0, 1.0);
            // Fast attack, cubic settle so nothing lingers.
            let env = (1.0 - p).powi(3) * (elapsed / 0.018).clamp(0.0, 1.0);
            let spring_wave = (p * std::f32::consts::PI * 2.5).sin();
            let dir_x = imp.dir[0] * spring_wave * (1.0 - imp.agitation * 0.45);
            let dir_y = imp.dir[1] * spring_wave * (1.0 - imp.agitation * 0.45);
            let tremor_x = (elapsed * 43.0 + imp.seed * 7.1).sin() * 0.65
                + (elapsed * 67.0 + imp.seed * 3.7).cos() * 0.35;
            let tremor_y = (elapsed * 47.0 + imp.seed * 5.3).cos() * 0.65
                + (elapsed * 71.0 + imp.seed * 9.2).sin() * 0.35;
            let x = (dir_x + tremor_x * imp.agitation) * imp.amplitude_css * env;
            let y = (dir_y + tremor_y * imp.agitation) * imp.amplitude_css * env;
            total_x += x;
            total_y += y;
            true
        });

        let mag = (total_x * total_x + total_y * total_y).sqrt();
        if mag > MAX_SHAKE_CSS_PX {
            let scale = MAX_SHAKE_CSS_PX / mag;
            total_x *= scale;
            total_y *= scale;
        }
        let sf = sf.max(0.01);
        [total_x * sf, total_y * sf]
    }
}

/// Spatial falloff factor (0.0..=1.0) based on world distance from viewport center and zoom.
pub(crate) fn viewport_impact_factor(
    wx: f32,
    wy: f32,
    input: &crate::app::InputState,
) -> f32 {
    let zoom = input.camera_zoom.max(0.001);
    let world_center_x = (input.screen_w * 0.5 - input.camera_x) / zoom;
    let world_center_y = (input.screen_h * 0.5 - input.camera_y) / zoom;
    let dx = wx - world_center_x;
    let dy = wy - world_center_y;
    let dist = (dx * dx + dy * dy).sqrt();
    let half_w = input.screen_w / (2.0 * zoom);
    let half_h = input.screen_h / (2.0 * zoom);
    let max_dist = (half_w * half_w + half_h * half_h).sqrt() * 1.5;
    let dist_factor = (1.0 - dist / max_dist.max(1.0)).clamp(0.0, 1.0);
    let zoom_factor = ((zoom - 1.0) / 9.0).clamp(0.25, 1.0);
    dist_factor.sqrt() * zoom_factor
}

