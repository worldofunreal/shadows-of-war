use super::engine::{AudioSource, SAMPLE_RATE, SoundPriority, queue_spatial};
use super::tone::{note_envelope, sweep_envelope, warm_at};
use crate::{BuildingSoundKind, SpatialSoundParams};

struct BuildingPlacementParams {
    freq: f32,
    decay_rate: f32,
    duration_secs: f32,
    amplitude: f32,
}

fn placement_params(kind: BuildingSoundKind) -> BuildingPlacementParams {
    match kind {
        BuildingSoundKind::City => BuildingPlacementParams {
            freq: 280.0,
            decay_rate: 11.0,
            duration_secs: 0.10,
            amplitude: 0.10,
        },
        BuildingSoundKind::Bunker => BuildingPlacementParams {
            freq: 220.0,
            decay_rate: 10.0,
            duration_secs: 0.11,
            amplitude: 0.095,
        },
        BuildingSoundKind::Factory => BuildingPlacementParams {
            freq: 350.0,
            decay_rate: 12.0,
            duration_secs: 0.085,
            amplitude: 0.095,
        },
        BuildingSoundKind::Port => BuildingPlacementParams {
            freq: 300.0,
            decay_rate: 10.0,
            duration_secs: 0.10,
            amplitude: 0.09,
        },
    }
}

struct BuildingPlacementSource {
    sample_idx: u64,
    freq: f32,
    decay_rate: f32,
    amplitude: f32,
    duration_samples: u64,
}

impl BuildingPlacementSource {
    fn new(kind: BuildingSoundKind) -> Self {
        let p = placement_params(kind);
        Self {
            sample_idx: 0,
            freq: p.freq,
            decay_rate: p.decay_rate,
            amplitude: p.amplitude,
            duration_samples: (SAMPLE_RATE as f32 * p.duration_secs) as u64,
        }
    }
}

impl Iterator for BuildingPlacementSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.sample_idx >= self.duration_samples {
            return None;
        }
        let t = self.sample_idx as f32 / SAMPLE_RATE as f32;
        let duration = self.duration_samples as f32 / SAMPLE_RATE as f32;
        let wave_val = warm_at(self.freq, t);
        let envelope = sweep_envelope(t, duration, self.decay_rate, 0.008, 0.03);
        self.sample_idx += 1;
        Some(wave_val * envelope * self.amplitude)
    }
}

impl AudioSource for BuildingPlacementSource {
    fn sample_rate(&self) -> u32 {
        SAMPLE_RATE
    }
}

struct BuildingCompletionSource {
    sample_idx: u64,
    duration_samples: u64,
    freqs: [f32; 2],
}

impl BuildingCompletionSource {
    fn new(kind: BuildingSoundKind) -> Self {
        let freqs = match kind {
            BuildingSoundKind::City => [330.0, 440.0],
            BuildingSoundKind::Bunker => [220.0, 294.0],
            BuildingSoundKind::Factory => [294.0, 392.0],
            BuildingSoundKind::Port => [262.0, 349.0],
        };
        Self {
            sample_idx: 0,
            duration_samples: (SAMPLE_RATE as f32 * 0.22) as u64,
            freqs,
        }
    }
}

impl Iterator for BuildingCompletionSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.sample_idx >= self.duration_samples {
            return None;
        }
        let t = self.sample_idx as f32 / SAMPLE_RATE as f32;
        let mut val = 0.0_f32;
        let amp = 0.12;

        let note_dur = 0.11;
        let note_idx = (t / note_dur) as usize;
        if note_idx < self.freqs.len() {
            let freq = self.freqs[note_idx];
            let t_note = t - note_idx as f32 * note_dur;
            let wave = warm_at(freq, t_note);
            let envelope = note_envelope(t_note, note_dur, 8.0, 0.008, 0.02);
            val = wave * envelope;
        }

        self.sample_idx += 1;
        Some(val * amp)
    }
}

impl AudioSource for BuildingCompletionSource {
    fn sample_rate(&self) -> u32 {
        SAMPLE_RATE
    }
}

struct NukeLaunchSource {
    sample_idx: u64,
    duration_samples: u64,
}

impl NukeLaunchSource {
    fn new() -> Self {
        Self {
            sample_idx: 0,
            duration_samples: (SAMPLE_RATE as f32 * 1.2) as u64,
        }
    }
}

impl Iterator for NukeLaunchSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.sample_idx >= self.duration_samples {
            return None;
        }
        let t = self.sample_idx as f32 / SAMPLE_RATE as f32;
        let duration = self.duration_samples as f32 / SAMPLE_RATE as f32;
        let progress = self.sample_idx as f32 / self.duration_samples as f32;

        let base_freq = 180.0 + 340.0 * progress;
        let fm = (2.0 * std::f32::consts::PI * 35.0 * t).sin() * 12.0;
        let freq = (base_freq + fm).max(20.0);
        let wave_val = warm_at(freq, t);

        let envelope = sweep_envelope(t, duration, 2.5, 0.01, 0.15);

        self.sample_idx += 1;
        Some(wave_val * envelope * 0.18)
    }
}

impl AudioSource for NukeLaunchSource {
    fn sample_rate(&self) -> u32 {
        SAMPLE_RATE
    }
}

struct NukeImpactSource {
    sample_idx: u64,
    duration_samples: u64,
    level: u8,
}

impl NukeImpactSource {
    fn new(level: u8) -> Self {
        let duration_secs = 1.2 + (level as f32 * 0.3);
        Self {
            sample_idx: 0,
            duration_samples: (SAMPLE_RATE as f32 * duration_secs) as u64,
            level,
        }
    }
}

impl Iterator for NukeImpactSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.sample_idx >= self.duration_samples {
            return None;
        }
        let t = self.sample_idx as f32 / SAMPLE_RATE as f32;
        let duration_secs = 1.2 + (self.level as f32 * 0.3);

        let low_freq = 80.0 * (-3.5 * t).exp() + 35.0;
        let mid_freq = 180.0 * (-5.0 * t).exp() + 60.0;
        let low = warm_at(low_freq, t);
        let mid = warm_at(mid_freq, t) * 0.45;

        let val = low * 0.65 + mid * 0.35;

        let mut envelope = (-2.2 * t).exp();
        let fade_start = duration_secs - 0.2;
        if t > fade_start {
            let linear_fade = (duration_secs - t) / 0.2;
            envelope *= linear_fade.clamp(0.0, 1.0);
        }

        self.sample_idx += 1;
        Some(val * envelope * 0.24)
    }
}

impl AudioSource for NukeImpactSource {
    fn sample_rate(&self) -> u32 {
        SAMPLE_RATE
    }
}

pub fn play_building_placement_sound(kind: BuildingSoundKind, spatial: SpatialSoundParams) {
    queue_spatial(
        BuildingPlacementSource::new(kind),
        spatial,
        SoundPriority::Normal,
    );
}

pub fn play_building_completed_sound(kind: BuildingSoundKind, spatial: SpatialSoundParams) {
    queue_spatial(
        BuildingCompletionSource::new(kind),
        spatial,
        SoundPriority::Foreground,
    );
}

pub fn play_nuke_launch_sound(spatial: SpatialSoundParams) {
    queue_spatial(NukeLaunchSource::new(), spatial, SoundPriority::Foreground);
}

pub fn play_nuke_impact_sound(level: u8, spatial: SpatialSoundParams) {
    queue_spatial(
        NukeImpactSource::new(level),
        spatial,
        SoundPriority::Foreground,
    );
}
