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

struct AttackOrderSource {
    sample_idx: u64,
    duration_samples: u64,
    start_freq: f32,
    end_freq: f32,
    amplitude: f32,
}

impl AttackOrderSource {
    fn new(commitment: f32, against_enemy: bool) -> Self {
        let c = commitment.clamp(0.0, 1.0);
        let (start_freq, end_freq) = if against_enemy {
            (250.0 + 40.0 * c, 175.0 - 25.0 * c)
        } else {
            (290.0 + 35.0 * c, 240.0 + 20.0 * c)
        };
        Self {
            sample_idx: 0,
            duration_samples: (SAMPLE_RATE as f32 * (0.095 + 0.025 * c)) as u64,
            start_freq,
            end_freq,
            amplitude: 0.18 + 0.08 * c,
        }
    }
}

impl Iterator for AttackOrderSource {
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
        let envelope = sweep_envelope(t, duration, 8.0, 0.005, 0.02);
        self.sample_idx += 1;
        Some(val * envelope * self.amplitude)
    }
}

impl AudioSource for AttackOrderSource {
    fn sample_rate(&self) -> u32 {
        SAMPLE_RATE
    }
}

struct AttackStalledSource {
    sample_idx: u64,
    duration_samples: u64,
    start_freq: f32,
    end_freq: f32,
    amplitude: f32,
}

impl AttackStalledSource {
    fn new(severity: f32) -> Self {
        let s = severity.clamp(0.0, 1.0);
        Self {
            sample_idx: 0,
            duration_samples: (SAMPLE_RATE as f32 * 0.16) as u64,
            start_freq: 190.0 - 20.0 * s,
            end_freq: 85.0 - 15.0 * s,
            amplitude: 0.20 + 0.10 * s,
        }
    }
}

impl Iterator for AttackStalledSource {
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
        let envelope = sweep_envelope(t, duration, 9.0, 0.005, 0.03);
        self.sample_idx += 1;
        Some(val * envelope * self.amplitude)
    }
}

impl AudioSource for AttackStalledSource {
    fn sample_rate(&self) -> u32 {
        SAMPLE_RATE
    }
}

pub fn play_attack_order_sound(
    commitment: f32,
    against_enemy: bool,
    spatial: SpatialSoundParams,
) {
    queue_spatial(
        AttackOrderSource::new(commitment, against_enemy),
        spatial,
        SoundPriority::Normal,
    );
}

pub fn play_attack_stalled_sound(severity: f32, spatial: SpatialSoundParams) {
    queue_spatial(
        AttackStalledSource::new(severity),
        spatial,
        SoundPriority::Normal,
    );
}

pub fn play_deploy_sound(spatial: SpatialSoundParams) {
    queue_spatial(DeploySource::new(), spatial, SoundPriority::Normal);
}

pub fn play_under_attack_sound(severity: f32, spatial: SpatialSoundParams) {
    let s = severity.clamp(0.0, 1.0);
    let amp = 0.18 + 0.12 * s;
    let first = PulseSource::new(150.0 - 18.0 * s, 0.10, 10.0, amp);
    let second = PulseSource::new(132.0 - 16.0 * s, 0.10, 10.0, amp * 0.9);
    queue_spatial(
        DoublePulseSource::new(first, second, 0.08),
        spatial,
        SoundPriority::Foreground,
    );
}
