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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emote_phases_are_bounded_continuous_and_settle() {
        for definition in sow_core::emote::EMOTES {
            let mut previous = emote_motion(definition.kind, 0.0, false, false);
            for ms in 1..=3100 {
                let current = emote_motion(definition.kind, ms as f32 / 1000.0, false, false);
                assert!(
                    current.bubble_alpha.is_finite() && (0.0..=1.0).contains(&current.bubble_alpha)
                );
                for axis in 0..2 {
                    assert!((current.offset[axis] - previous.offset[axis]).abs() < 0.02);
                    assert!(current.offset[axis].abs() <= 0.21);
                    assert!((0.9..=1.1).contains(&current.scale[axis]));
                }
                previous = current;
            }
            assert_eq!(
                emote_motion(definition.kind, 3.0, false, false),
                EmoteMotion::default()
            );
            assert_eq!(
                emote_motion(definition.kind, 3.0, false, true).bubble_alpha,
                1.0
            );
            let reduced = emote_motion(definition.kind, 0.18, true, false);
            assert_eq!(reduced.offset, [0.0; 2]);
            assert_eq!(reduced.scale, [1.0; 2]);
            assert!(reduced.bubble_alpha > 0.0);
        }
    }
}
