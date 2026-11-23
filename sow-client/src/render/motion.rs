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

#[inline]
fn lerp(from: f32, to: f32, t: f32) -> f32 {
    from + (to - from) * t
}

#[inline]
fn ease_in_quad(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t
}

#[inline]
fn ease_in_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * t
}

#[inline]
fn ease_out_cubic(t: f32) -> f32 {
    let u = 1.0 - t.clamp(0.0, 1.0);
    1.0 - u * u * u
}

/// Continuous callout motion: snappy anticipation + overshoot entry, calm hold, smooth taper exit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CascadeCalloutMotion {
    /// Normalized progression [0..1] for upward world drift.
    pub rise_progress: f32,
    /// Vertical nudge in scaled logical pixels (entry spring + subtle exit lift).
    pub offset_y_px: f32,
    /// Uniform glyph scale (preserves MSDF kerning and baseline alignment).
    pub text_scale: f32,
    /// Independent [width, height] squash & stretch for the callout icon.
    pub icon_scale: [f32; 2],
    pub alpha: f32,
    /// Additive highlight mix [0..1] during the initial pop beat.
    pub flash: f32,
    /// Compact expanding shockwave ring progression [0..1], if active on this frame.
    pub ring_t: Option<f32>,
}

pub(crate) fn cascade_callout_motion(
    elapsed: f32,
    duration: f32,
    emphatic: bool,
    reduced_motion: bool,
) -> Option<CascadeCalloutMotion> {
    if !elapsed.is_finite() || !duration.is_finite() || duration <= 0.0 || elapsed < 0.0
        || elapsed >= duration
    {
        return None;
    }

    let progress = (elapsed / duration).clamp(0.0, 1.0);
    let fade_out_secs = 0.34_f32.min(duration * 0.4);
    let fade_out = 1.0
        - smoothstep(
            ((elapsed - (duration - fade_out_secs)) / fade_out_secs.max(0.001)).clamp(0.0, 1.0),
        );

    if reduced_motion {
        let fade_in = smoothstep((elapsed / 0.10).clamp(0.0, 1.0));
        return Some(CascadeCalloutMotion {
            rise_progress: progress,
            offset_y_px: 0.0,
            text_scale: 1.0,
            icon_scale: [1.0; 2],
            alpha: (fade_in * fade_out).clamp(0.0, 1.0),
            flash: 0.0,
            ring_t: None,
        });
    }

    let alpha_in = smoothstep((elapsed / 0.06).clamp(0.0, 1.0));
    let alpha = (alpha_in * fade_out).clamp(0.0, 1.0);

    let snap_secs = 0.08;
    let entry_end = if emphatic { 0.36 } else { 0.26 };
    let peak_overshoot = if emphatic { 0.16 } else { 0.09 };
    let icon_squash = if emphatic { 0.24 } else { 0.12 };
    let icon_stretch = if emphatic { 0.22 } else { 0.12 };

    let (mut text_scale, mut icon_scale, mut offset_y_px) = if elapsed < snap_secs {
        let e = ease_out_cubic(elapsed / snap_secs);
        (
            lerp(0.56, 1.0 + peak_overshoot, e),
            [
                lerp(1.0 + icon_squash, 1.0 - icon_squash * 0.45, e),
                lerp(1.0 - icon_squash * 1.1, 1.0 + icon_stretch, e),
            ],
            lerp(5.5, -1.5, e),
        )
    } else if elapsed < entry_end {
        let t = (elapsed - snap_secs) / (entry_end - snap_secs);
        let osc = damped_settle(t, 9.5, 4.4);
        (
            1.0 + peak_overshoot * osc,
            [1.0 - icon_squash * 0.45 * osc, 1.0 + icon_stretch * osc],
            -1.5 * osc,
        )
    } else {
        (1.0, [1.0; 2], 0.0)
    };

    let exit_start = duration - fade_out_secs;
    if elapsed > exit_start {
        let et = smoothstep((elapsed - exit_start) / fade_out_secs.max(0.001));
        let taper = 1.0 - 0.14 * et;
        text_scale *= taper;
        icon_scale[0] *= taper;
        icon_scale[1] *= taper;
        offset_y_px -= 3.5 * et;
    }

    let flash = if emphatic && elapsed < 0.24 {
        let peak = if elapsed < 0.05 {
            smoothstep(elapsed / 0.05)
        } else {
            1.0 - smoothstep((elapsed - 0.05) / 0.19)
        };
        0.42 * peak * peak
    } else {
        0.0
    };

    let ring_duration = 0.34;
    let ring_t = (elapsed < ring_duration).then(|| (elapsed / ring_duration).clamp(0.0, 1.0));

    Some(CascadeCalloutMotion {
        rise_progress: progress,
        offset_y_px,
        text_scale,
        icon_scale,
        alpha,
        flash,
        ring_t,
    })
}

/// Procedural parameters for the StarCraft-style ground selection / hover ghost ring.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SelectionRingMotion {
    pub scale: f32,
    pub alpha: f32,
    pub phase: f32,
    pub pulse: f32,
    pub acquire: f32,
}

/// Selected building ring: crisp lock-on snap, outward acquisition ripple, dignified rotation.
pub(crate) fn building_selection_ring_motion(
    elapsed: f32,
    reduced_motion: bool,
) -> SelectionRingMotion {
    let elapsed = if elapsed.is_finite() {
        elapsed.max(0.0)
    } else {
        0.0
    };
    if reduced_motion {
        return SelectionRingMotion {
            scale: 1.0,
            alpha: 0.96,
            phase: 0.0,
            pulse: 0.0,
            acquire: 0.0,
        };
    }

    let snap_secs = 0.22;
    let scale = if elapsed < snap_secs {
        let t = elapsed / snap_secs;
        // Start slightly wider (1.16) and lock briskly onto the footprint with a crisp settle.
        1.0 + 0.16 * damped_settle(t, 8.5, 5.2)
    } else {
        1.0
    };
    let alpha = 0.96 * smoothstep((elapsed / 0.07).clamp(0.0, 1.0));
    let acquire = if elapsed < 0.38 {
        let u = 1.0 - (elapsed / 0.38);
        u * u
    } else {
        0.0
    };
    let pulse = 0.5 + 0.5 * (elapsed * 2.8).sin();
    let phase = (elapsed * 0.065).rem_euclid(1.0);
    SelectionRingMotion {
        scale,
        alpha,
        phase,
        pulse,
        acquire,
    }
}

pub(crate) const BUILDING_HOVER_EXIT_SECS: f32 = 0.11;

/// Desktop ghost hover ring: ultra-snappy fade/expand on enter and silky fade-out on exit.
pub(crate) fn building_hover_ring_motion(
    elapsed: f32,
    exiting: bool,
    reduced_motion: bool,
) -> Option<SelectionRingMotion> {
    if !elapsed.is_finite() || elapsed < 0.0 {
        return None;
    }
    if exiting {
        if elapsed >= BUILDING_HOVER_EXIT_SECS {
            return None;
        }
        let t = (elapsed / BUILDING_HOVER_EXIT_SECS).clamp(0.0, 1.0);
        let ease = ease_out_cubic(t);
        return Some(SelectionRingMotion {
            scale: if reduced_motion { 1.0 } else { lerp(1.0, 1.05, ease) },
            alpha: 0.56 * (1.0 - smoothstep(t)),
            phase: 0.0,
            pulse: 0.0,
            acquire: 0.0,
        });
    }

    if reduced_motion {
        return Some(SelectionRingMotion {
            scale: 1.0,
            alpha: 0.52,
            phase: 0.0,
            pulse: 0.0,
            acquire: 0.0,
        });
    }
    let enter_t = (elapsed / 0.11).clamp(0.0, 1.0);
    let ease = ease_out_cubic(enter_t);
    Some(SelectionRingMotion {
        scale: lerp(0.88, 1.0, ease),
        alpha: 0.56 * smoothstep(enter_t),
        phase: (-elapsed * 0.042).rem_euclid(1.0),
        pulse: 0.5 + 0.5 * (elapsed * 2.2).sin(),
        acquire: 0.0,
    })
}

/// Damped cosine settled cleanly to exact 0.0 at `t = 1.0` with no final pop.
#[inline]
fn damped_settle(t: f32, freq: f32, decay: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let tail = 1.0 - smoothstep((t - 0.78) / 0.22);
    (-decay * t).exp() * (t * freq).cos() * tail
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RankSpriteSnapshot {
    pub offset: [f32; 2],
    pub scale: [f32; 2],
    pub alpha: f32,
}

impl Default for RankSpriteSnapshot {
    fn default() -> Self {
        Self {
            offset: [0.0; 2],
            scale: [1.0; 2],
            alpha: 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RankRingVfx {
    /// Radius in badge diameters.
    pub radius: f32,
    /// Stroke thickness in badge diameters.
    pub thickness: f32,
    pub color: [f32; 4],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RankDiscVfx {
    /// Radius in badge diameters.
    pub radius: f32,
    pub color: [f32; 4],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RankSparkleVfx {
    /// Offset from badge anchor in badge diameters.
    pub offset: [f32; 2],
    /// Diameter in badge diameters.
    pub scale: f32,
    pub color: [f32; 4],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RankBadgeMotion {
    /// Offset from `rank_center` in badge diameters.
    pub offset: [f32; 2],
    /// Independent `[width, height]` scale for squash & stretch.
    pub scale: [f32; 2],
    pub alpha: f32,
    /// Optional brief outline glow `[r, g, b, blend_weight]`.
    pub outline_glow: [f32; 4],
    pub ring: Option<RankRingVfx>,
    pub flash_disc: Option<RankDiscVfx>,
    pub sparkles: [Option<RankSparkleVfx>; 2],
}

impl RankBadgeMotion {
    pub const fn steady() -> Self {
        Self {
            offset: [0.0; 2],
            scale: [1.0; 2],
            alpha: 1.0,
            outline_glow: [0.0; 4],
            ring: None,
            flash_disc: None,
            sparkles: [None, None],
        }
    }

    pub const fn hidden() -> Self {
        Self {
            offset: [0.0; 2],
            scale: [1.0; 2],
            alpha: 0.0,
            outline_glow: [0.0; 4],
            ring: None,
            flash_disc: None,
            sparkles: [None, None],
        }
    }

    #[inline]
    pub const fn snapshot(self) -> RankSpriteSnapshot {
        RankSpriteSnapshot {
            offset: self.offset,
            scale: self.scale,
            alpha: self.alpha,
        }
    }
}

/// Entry duration scales with rank prestige: 1st max drama, 2nd medium, 3rd crisp.
#[inline]
pub(crate) const fn rank_badge_enter_duration(rank: u8, reduced_motion: bool) -> f32 {
    if reduced_motion {
        return match rank {
            1 => 0.30,
            2 => 0.24,
            _ => 0.20,
        };
    }
    match rank {
        1 => 0.84,
        2 => 0.58,
        _ => 0.42,
    }
}

/// Exit duration scales with rank prestige: losing 1st is most dramatic.
#[inline]
pub(crate) const fn rank_badge_exit_duration(rank: u8, reduced_motion: bool) -> f32 {
    if reduced_motion {
        return match rank {
            1 => 0.28,
            2 => 0.22,
            _ => 0.18,
        };
    }
    match rank {
        1 => 0.74,
        2 => 0.50,
        _ => 0.34,
    }
}

/// Short handoff delay when swapping directly between podium places so the
/// departing sprite clearly yields before the incoming badge claims the seat.
#[inline]
pub(crate) const fn rank_badge_swap_delay(next_rank: u8, reduced_motion: bool) -> f32 {
    if reduced_motion {
        return 0.04;
    }
    match next_rank {
        1 => 0.11,
        2 => 0.13,
        _ => 0.12,
    }
}

/// Analytic entry motion for podium badges (rank 1 = max drama, 2 = medium, 3 = crisp).
pub(crate) fn rank_badge_enter_motion(
    rank: u8,
    age_secs: f32,
    reduced_motion: bool,
) -> RankBadgeMotion {
    if !age_secs.is_finite() || age_secs < 0.0 {
        return RankBadgeMotion::hidden();
    }
    let duration = rank_badge_enter_duration(rank, reduced_motion);
    if age_secs >= duration {
        return RankBadgeMotion::steady();
    }
    let p = (age_secs / duration).clamp(0.0, 1.0);
    if reduced_motion {
        return RankBadgeMotion {
            alpha: smoothstep(p),
            ..RankBadgeMotion::steady()
        };
    }

    match rank {
        1 => rank1_enter_motion(p),
        2 => rank2_enter_motion(p),
        _ => rank3_enter_motion(p),
    }
}

/// Analytic exit motion for podium badges (losing 1st = max drama, 2nd = medium, 3rd = subtle).
pub(crate) fn rank_badge_exit_motion(
    rank: u8,
    age_secs: f32,
    from: RankSpriteSnapshot,
    reduced_motion: bool,
) -> RankBadgeMotion {
    if !age_secs.is_finite() || age_secs < 0.0 || from.alpha <= 0.001 {
        return RankBadgeMotion::hidden();
    }
    let duration = rank_badge_exit_duration(rank, reduced_motion);
    if age_secs >= duration {
        return RankBadgeMotion::hidden();
    }
    let p = (age_secs / duration).clamp(0.0, 1.0);
    if reduced_motion {
        return RankBadgeMotion {
            alpha: (from.alpha * (1.0 - smoothstep(p))).clamp(0.0, 1.0),
            ..RankBadgeMotion::steady()
        };
    }

    match rank {
        1 => rank1_exit_motion(p, from),
        2 => rank2_exit_motion(p, from),
        _ => rank3_exit_motion(p, from),
    }
}

fn rank3_enter_motion(p: f32) -> RankBadgeMotion {
    let (offset_y, scale, alpha) = if p < 0.34 {
        let e = ease_out_cubic(p / 0.34);
        (
            lerp(0.28, -0.06, e),
            [lerp(0.45, 1.14, e), lerp(0.55, 1.18, e)],
            smoothstep(p / 0.24),
        )
    } else {
        let osc = damped_settle((p - 0.34) / 0.66, 8.5, 4.5);
        (-0.06 * osc, [1.0 + 0.14 * osc, 1.0 + 0.18 * osc], 1.0)
    };
    RankBadgeMotion {
        offset: [0.0, offset_y],
        scale,
        alpha: alpha.clamp(0.0, 1.0),
        outline_glow: [0.0; 4],
        ring: None,
        flash_disc: None,
        sparkles: [None, None],
    }
}

fn rank2_enter_motion(p: f32) -> RankBadgeMotion {
    let (offset_y, scale, alpha) = if p < 0.24 {
        let t = p / 0.24;
        (
            lerp(-0.62, 0.08, ease_in_quad(t)),
            [lerp(0.50, 0.84, t), lerp(0.55, 1.28, smoothstep(t))],
            smoothstep(p / 0.16),
        )
    } else if p < 0.38 {
        let s = smoothstep((p - 0.24) / 0.14);
        (
            lerp(0.08, 0.12, s),
            [lerp(0.84, 1.28, s), lerp(1.28, 0.78, s)],
            1.0,
        )
    } else if p < 0.56 {
        let e = ease_out_cubic((p - 0.38) / 0.18);
        (
            lerp(0.12, -0.14, e),
            [lerp(1.28, 1.22, e), lerp(0.78, 1.30, e)],
            1.0,
        )
    } else {
        let osc = damped_settle((p - 0.56) / 0.44, 9.0, 4.2);
        (-0.14 * osc, [1.0 + 0.22 * osc, 1.0 + 0.30 * osc], 1.0)
    };

    let gleam = if (0.22..0.64).contains(&p) {
        let s = (std::f32::consts::PI * ((p - 0.22) / 0.42)).sin().max(0.0);
        s * s
    } else {
        0.0
    };
    let ring = (0.24..0.78).contains(&p).then(|| {
        let rt = (p - 0.24) / 0.54;
        let re = ease_out_cubic(rt);
        let fade = 1.0 - rt;
        RankRingVfx {
            radius: lerp(0.42, 1.32, re),
            thickness: lerp(0.11, 0.025, re),
            color: [0.86, 0.92, 1.0, 0.72 * fade * fade],
        }
    });

    RankBadgeMotion {
        offset: [0.0, offset_y],
        scale,
        alpha: alpha.clamp(0.0, 1.0),
        outline_glow: [0.85, 0.92, 1.0, 0.32 * gleam],
        ring,
        flash_disc: None,
        sparkles: [None, None],
    }
}

fn rank1_enter_motion(p: f32) -> RankBadgeMotion {
    let (offset_y, scale, alpha) = if p < 0.22 {
        let t = p / 0.22;
        let s = smoothstep(t);
        (
            lerp(-1.15, 0.10, ease_in_cubic(t)),
            [lerp(0.30, 0.76, s), lerp(0.30, 1.54, s)],
            smoothstep(p / 0.12),
        )
    } else if p < 0.34 {
        let s = smoothstep((p - 0.22) / 0.12);
        (
            lerp(0.10, 0.18, s),
            [lerp(0.76, 1.50, s), lerp(1.54, 0.64, s)],
            1.0,
        )
    } else if p < 0.56 {
        let e = ease_out_cubic((p - 0.34) / 0.22);
        (
            lerp(0.18, -0.26, e),
            [lerp(1.50, 1.44, e), lerp(0.64, 1.52, e)],
            1.0,
        )
    } else {
        let osc = damped_settle((p - 0.56) / 0.44, 10.5, 3.8);
        (-0.26 * osc, [1.0 + 0.44 * osc, 1.0 + 0.52 * osc], 1.0)
    };

    let gleam = if p < 0.20 || p >= 0.72 {
        0.0
    } else if p < 0.27 {
        smoothstep((p - 0.20) / 0.07)
    } else {
        let d = 1.0 - smoothstep((p - 0.27) / 0.45);
        d * d
    };

    let flash_disc = (0.21..0.50).contains(&p).then(|| {
        let dt = (p - 0.21) / 0.29;
        let fade = 1.0 - dt;
        RankDiscVfx {
            radius: lerp(0.32, 1.05, ease_out_cubic(dt)),
            color: [1.0, 0.88, 0.32, 0.52 * fade * fade],
        }
    });

    let ring = (0.22..0.78).contains(&p).then(|| {
        let rt = (p - 0.22) / 0.56;
        let re = ease_out_cubic(rt);
        let fade = 1.0 - rt;
        RankRingVfx {
            radius: lerp(0.40, 1.82, re),
            thickness: lerp(0.16, 0.025, re),
            color: [1.0, 0.84, 0.24, 0.90 * fade * fade],
        }
    });

    let sparkle_left = (0.24..0.76).contains(&p).then(|| {
        let st = (p - 0.24) / 0.52;
        let se = ease_out_cubic(st);
        let s = (std::f32::consts::PI * st).sin().max(0.0);
        let pop = s * (2.0 - s);
        RankSparkleVfx {
            offset: [lerp(-0.28, -0.88, se), lerp(-0.22, -0.78, se)],
            scale: 0.46 * pop,
            color: [1.0, 0.94, 0.55, 1.0 - smoothstep((st - 0.55) / 0.45)],
        }
    });

    let sparkle_right = (0.32..0.86).contains(&p).then(|| {
        let st = (p - 0.32) / 0.54;
        let se = ease_out_cubic(st);
        let s = (std::f32::consts::PI * st).sin().max(0.0);
        let pop = s * (2.0 - s);
        RankSparkleVfx {
            offset: [lerp(0.26, 0.92, se), lerp(-0.18, -0.68, se)],
            scale: 0.42 * pop,
            color: [1.0, 0.96, 0.68, 1.0 - smoothstep((st - 0.55) / 0.45)],
        }
    });

    RankBadgeMotion {
        offset: [0.0, offset_y],
        scale,
        alpha: alpha.clamp(0.0, 1.0),
        outline_glow: [1.0, 0.80, 0.22, 0.70 * gleam],
        ring,
        flash_disc,
        sparkles: [sparkle_left, sparkle_right],
    }
}

fn rank3_exit_motion(p: f32, from: RankSpriteSnapshot) -> RankBadgeMotion {
    let (offset, scale, alpha) = if p < 0.22 {
        let s = smoothstep(p / 0.22);
        (
            [lerp(from.offset[0], 0.0, s), lerp(from.offset[1], -0.06, s)],
            [lerp(from.scale[0], 1.07, s), lerp(from.scale[1], 1.07, s)],
            from.alpha,
        )
    } else {
        let t = (p - 0.22) / 0.78;
        let e = ease_in_quad(t);
        (
            [0.0, lerp(-0.06, 0.28, e)],
            [lerp(1.07, 0.50, e), lerp(1.07, 0.50, e)],
            from.alpha * (1.0 - smoothstep(t)),
        )
    };
    RankBadgeMotion {
        offset,
        scale,
        alpha: alpha.clamp(0.0, 1.0),
        outline_glow: [0.0; 4],
        ring: None,
        flash_disc: None,
        sparkles: [None, None],
    }
}

fn rank2_exit_motion(p: f32, from: RankSpriteSnapshot) -> RankBadgeMotion {
    let (offset, scale, alpha) = if p < 0.24 {
        let t = p / 0.24;
        let s = smoothstep(t);
        let jolt_x = 0.09 * (t * std::f32::consts::TAU * 1.5).sin() * (1.0 - 0.4 * t);
        (
            [
                lerp(from.offset[0], 0.0, s) + jolt_x,
                lerp(from.offset[1], -0.15, s),
            ],
            [lerp(from.scale[0], 1.18, s), lerp(from.scale[1], 0.86, s)],
            from.alpha,
        )
    } else {
        let t = (p - 0.24) / 0.76;
        let s = smoothstep(t);
        let drop = ease_in_quad(t);
        (
            [lerp(0.0, 0.20, s), lerp(-0.15, 0.65, drop)],
            [lerp(1.18, 0.40, s), lerp(0.86, 0.62, drop)],
            from.alpha * (1.0 - smoothstep((p - 0.34) / 0.66)),
        )
    };

    let ring = (0.14..0.64).contains(&p).then(|| {
        let rt = (p - 0.14) / 0.50;
        let re = ease_out_cubic(rt);
        let fade = 1.0 - rt;
        RankRingVfx {
            radius: lerp(0.55, 1.28, re),
            thickness: lerp(0.09, 0.02, re),
            color: [0.76, 0.84, 0.94, 0.48 * fade * fade * from.alpha],
        }
    });

    RankBadgeMotion {
        offset,
        scale,
        alpha: alpha.clamp(0.0, 1.0),
        outline_glow: [0.0; 4],
        ring,
        flash_disc: None,
        sparkles: [None, None],
    }
}

fn rank1_exit_motion(p: f32, from: RankSpriteSnapshot) -> RankBadgeMotion {
    let (offset, scale, alpha) = if p < 0.26 {
        let t = p / 0.26;
        let s = smoothstep(t);
        let shake_x = 0.18 * (t * std::f32::consts::TAU * 2.5).sin() * (1.0 - 0.25 * t);
        (
            [
                lerp(from.offset[0], 0.0, s) + shake_x,
                lerp(from.offset[1], -0.18, s),
            ],
            [lerp(from.scale[0], 1.36, s), lerp(from.scale[1], 0.76, s)],
            from.alpha,
        )
    } else if p < 0.50 {
        let t = (p - 0.26) / 0.24;
        let e = ease_out_cubic(t);
        let alpha_dip = 1.0 - 0.15 * smoothstep((p - 0.36) / 0.14);
        (
            [lerp(0.0, 0.28, e), lerp(-0.18, -0.44, e)],
            [lerp(1.36, 0.90, e), lerp(0.76, 1.32, e)],
            from.alpha * alpha_dip,
        )
    } else {
        let t = (p - 0.50) / 0.50;
        let s = smoothstep(t);
        let fall = ease_in_cubic(t);
        (
            [lerp(0.28, 0.58, s), lerp(-0.44, 0.96, fall)],
            [lerp(0.90, 0.26, s), lerp(1.32, 0.44, s)],
            from.alpha * 0.85 * (1.0 - s),
        )
    };

    let flare = if p < 0.22 {
        smoothstep(p / 0.22)
    } else if p < 0.48 {
        1.0 - smoothstep((p - 0.22) / 0.26)
    } else {
        0.0
    };
    let flash_disc = (0.08..0.38).contains(&p).then(|| {
        let dt = (p - 0.08) / 0.30;
        let fade = 1.0 - dt;
        RankDiscVfx {
            radius: lerp(0.35, 0.92, ease_out_cubic(dt)),
            color: [1.0, 0.68, 0.25, 0.42 * fade * fade * from.alpha],
        }
    });

    let ring = (0.14..0.74).contains(&p).then(|| {
        let rt = (p - 0.14) / 0.60;
        let re = ease_out_cubic(rt);
        let fade = 1.0 - rt;
        RankRingVfx {
            radius: lerp(0.48, 1.68, re),
            thickness: lerp(0.14, 0.025, re),
            color: [
                1.0,
                lerp(0.72, 0.36, rt),
                lerp(0.24, 0.14, rt),
                0.82 * fade * fade * from.alpha,
            ],
        }
    });

    RankBadgeMotion {
        offset,
        scale,
        alpha: alpha.clamp(0.0, 1.0),
        outline_glow: [1.0, 0.45, 0.16, 0.65 * flare],
        ring,
        flash_disc,
        sparkles: [None, None],
    }
}

