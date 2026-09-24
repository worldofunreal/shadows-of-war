use super::engine::{AudioSource, SAMPLE_RATE, SoundPriority, queue_spatial};
use super::tone::{sweep_envelope, warm_at};
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

pub fn play_building_placement_sound(kind: BuildingSoundKind, spatial: SpatialSoundParams) {
    queue_spatial(
        BuildingPlacementSource::new(kind),
        spatial,
        SoundPriority::Normal,
    );
}
