use super::clip::{SfxId, pick_clip, pick_clip_pitched};
use super::engine::{AudioSource, SAMPLE_RATE, SoundPriority, queue_spatial};
use crate::{BuildingSoundKind, CombatSoundKind, SpatialSoundParams};

pub fn play_combat_sound(
    _kind: CombatSoundKind,
    troops: f32,
    _seed: u32,
    spatial: SpatialSoundParams,
) {
    let troops_scale = (troops / 5000.0).clamp(0.15, 1.0).sqrt();
    if let Some(clip) = pick_clip(SfxId::CombatImpact, 0.28 + 0.17 * troops_scale) {
        queue_spatial(clip, spatial, SoundPriority::Background);
    }
}

pub fn play_building_completed_sound(_kind: BuildingSoundKind, spatial: SpatialSoundParams) {
    if let Some(clip) = pick_clip(SfxId::BuildingCompleted, 0.45) {
        queue_spatial(clip, spatial, SoundPriority::Foreground);
    }
}

pub fn play_nuke_launch_sound(spatial: SpatialSoundParams) {
    if let Some(clip) = pick_clip(SfxId::NukeLaunch, 0.50) {
        queue_spatial(clip, spatial, SoundPriority::Foreground);
    }
}

pub fn play_nuke_impact_sound(level: u8, spatial: SpatialSoundParams) {
    let clip_gain = match level {
        1 => 0.45,
        2 => 0.58,
        _ => 0.72,
    };
    if let Some(clip) = pick_clip(SfxId::NukeImpact, clip_gain) {
        queue_spatial(clip, spatial, SoundPriority::Foreground);
    }
}

const PENTATONIC_FREQS: [f32; 8] = [
    523.25, 587.33, 659.25, 783.99, 880.00, 1046.50, 1174.66, 1318.51,
];

struct CascadeChimeSource {
    freq: f32,
    gain: f32,
    sample_idx: u32,
    total_samples: u32,
}

impl Iterator for CascadeChimeSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.sample_idx >= self.total_samples {
            return None;
        }
        let t = self.sample_idx as f32 / SAMPLE_RATE as f32;
        self.sample_idx += 1;
        let dur = self.total_samples as f32 / SAMPLE_RATE as f32;
        let attack = (t / 0.004).min(1.0);
        let release = ((dur - t) / 0.04).clamp(0.0, 1.0);
        let env = attack * (-11.0 * t).exp() * release;
        let phase = std::f32::consts::TAU * self.freq * t;
        let warm = phase.sin() * 0.72 + (phase * 2.0).sin() * 0.22 + (phase * 3.0).sin() * 0.06;
        Some(warm * env * self.gain)
    }
}

impl AudioSource for CascadeChimeSource {
    fn sample_rate(&self) -> u32 {
        SAMPLE_RATE
    }
}

/// Escalating pentatonic chime + pitch-shifted impact for chain conquests and tactical surges.
pub fn play_cascade_sound(chain_step: u8, spatial: SpatialSoundParams) {
    let idx = usize::from(chain_step.saturating_sub(1)).min(PENTATONIC_FREQS.len() - 1);
    let freq = PENTATONIC_FREQS[idx];
    let pitch_ratio = 2.0_f32.powf((idx as f32) * 1.5 / 12.0);
    let gain = 0.24 + 0.02 * (idx as f32);
    queue_spatial(
        CascadeChimeSource {
            freq,
            gain,
            sample_idx: 0,
            total_samples: (SAMPLE_RATE as f32 * 0.19) as u32,
        },
        spatial,
        SoundPriority::Foreground,
    );
    if chain_step >= 2
        && let Some(clip) = pick_clip_pitched(SfxId::BuildingCompleted, 0.34, pitch_ratio)
    {
        queue_spatial(clip, spatial, SoundPriority::Foreground);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pentatonic_cascade_chimes_ascend_and_remain_finite() {
        for pair in PENTATONIC_FREQS.windows(2) {
            assert!(pair[1] > pair[0], "cascade notes must ascend monotonically");
        }
        let mut src = CascadeChimeSource {
            freq: PENTATONIC_FREQS[0],
            gain: 0.25,
            sample_idx: 0,
            total_samples: 512,
        };
        let samples: Vec<f32> = (&mut src).collect();
        assert_eq!(samples.len(), 512);
        assert!(samples.iter().all(|s| s.is_finite() && s.abs() <= 1.0));
        assert!(samples.iter().any(|s| s.abs() > 0.01));
    }

    #[test]
    fn play_cascade_sound_handles_edge_steps_safely() {
        let spatial = SpatialSoundParams {
            wx: 10.0,
            wy: 10.0,
            camera_x: 10.0,
            camera_y: 10.0,
            camera_zoom: 1.0,
            screen_w: 800.0,
            screen_h: 600.0,
        };
        play_cascade_sound(0, spatial);
        play_cascade_sound(255, spatial);
    }
}
