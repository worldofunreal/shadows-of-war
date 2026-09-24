use super::death::PulseSource;
use super::engine::{AudioSource, SAMPLE_RATE, SoundPriority, queue_spatial};
use super::tone::{sweep_envelope, warm_at};
use crate::SpatialSoundParams;

struct DoublePulseSource {
    pulse1: PulseSource,
    pulse2: PulseSource,
    silence_samples: u64,
    sample_idx: u64,
}

impl DoublePulseSource {
    fn new(p1: PulseSource, p2: PulseSource, gap_secs: f32) -> Self {
        Self {
            pulse1: p1,
            pulse2: p2,
            silence_samples: (SAMPLE_RATE as f32 * gap_secs) as u64,
            sample_idx: 0,
        }
    }
}

impl Iterator for DoublePulseSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.sample_idx < self.pulse1.duration_samples {
            self.sample_idx += 1;
            self.pulse1.next()
        } else if self.sample_idx < self.pulse1.duration_samples + self.silence_samples {
            self.sample_idx += 1;
            Some(0.0)
        } else {
            self.sample_idx += 1;
            self.pulse2.next()
        }
    }
}

impl AudioSource for DoublePulseSource {
    fn sample_rate(&self) -> u32 {
        SAMPLE_RATE
    }
}

struct DeploySource {
    sample_idx: u64,
    duration_samples: u64,
    start_freq: f32,
    end_freq: f32,
    amplitude: f32,
}

impl DeploySource {
    fn new() -> Self {
        Self {
            sample_idx: 0,
            duration_samples: (SAMPLE_RATE as f32 * 0.11) as u64,
            start_freq: 210.0,
            end_freq: 330.0,
            amplitude: 0.12,
        }
    }
}

impl Iterator for DeploySource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.sample_idx >= self.duration_samples {
            return None;
        }
        let t = self.sample_idx as f32 / SAMPLE_RATE as f32;
        let duration = self.duration_samples as f32 / SAMPLE_RATE as f32;
        let progress = self.sample_idx as f32 / self.duration_samples.max(1) as f32;
        let freq = self.start_freq + (self.end_freq - self.start_freq) * progress;
        let val = warm_at(freq, t);
        let envelope = sweep_envelope(t, duration, 10.0, 0.008, 0.025);
        self.sample_idx += 1;
        Some(val * envelope * self.amplitude)
    }
}

impl AudioSource for DeploySource {
    fn sample_rate(&self) -> u32 {
        SAMPLE_RATE
    }
}

pub fn play_deploy_sound(spatial: SpatialSoundParams) {
    queue_spatial(DeploySource::new(), spatial, SoundPriority::Normal);
}

pub fn play_under_attack_sound(spatial: SpatialSoundParams) {
    let first = PulseSource::new(150.0, 0.10, 10.0, 0.11);
    let second = PulseSource::new(132.0, 0.10, 10.0, 0.10);
    queue_spatial(
        DoublePulseSource::new(first, second, 0.08),
        spatial,
        SoundPriority::Foreground,
    );
}
