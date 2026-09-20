use super::death::PulseSource;
use super::engine::{AudioSource, SAMPLE_RATE, SimpleRng, SoundPriority, queue_spatial};
use super::tone::{sweep_envelope, warm_at};
use crate::{CombatSoundKind, SpatialSoundParams};

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

struct SweepSource {
    sample_idx: u64,
    start_freq: f32,
    end_freq: f32,
    decay_rate: f32,
    duration_samples: u64,
    amplitude: f32,
}

impl SweepSource {
    fn new(
        start_freq: f32,
        end_freq: f32,
        duration_secs: f32,
        decay_rate: f32,
        amplitude: f32,
    ) -> Self {
        Self {
            sample_idx: 0,
            start_freq: start_freq.max(20.0),
            end_freq: end_freq.max(20.0),
            decay_rate,
            duration_samples: (SAMPLE_RATE as f32 * duration_secs.max(0.01)) as u64,
            amplitude,
        }
    }
}

impl Iterator for SweepSource {
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
        let envelope = sweep_envelope(t, duration, self.decay_rate, 0.008, 0.03);
        self.sample_idx += 1;
        Some(val * envelope * self.amplitude)
    }
}

impl AudioSource for SweepSource {
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

enum ProceduralSound {
    Pulse(PulseSource),
    DoublePulse(DoublePulseSource),
    Sweep(SweepSource),
}

impl Iterator for ProceduralSound {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Pulse(source) => source.next(),
            Self::DoublePulse(source) => source.next(),
            Self::Sweep(source) => source.next(),
        }
    }
}

impl AudioSource for ProceduralSound {
    fn sample_rate(&self) -> u32 {
        SAMPLE_RATE
    }
}

fn combat_amplitude(troops: f32) -> f32 {
    0.075 + 0.045 * (troops / 5000.0).clamp(0.15, 1.0).sqrt()
}

fn build_procedural_sound(
    kind: CombatSoundKind,
    troops: f32,
    seed: u32,
    wx: f32,
    wy: f32,
) -> ProceduralSound {
    let position_seed = wx.to_bits().rotate_left(11) ^ wy.to_bits().rotate_left(23);
    let mut rng = SimpleRng::new(position_seed ^ seed);
    let base_freq = match kind {
        CombatSoundKind::WildernessExpansion => 260.0,
        CombatSoundKind::AttackHuman => 245.0,
        CombatSoundKind::AttackEmpire => 315.0,
        CombatSoundKind::AttackTribe => 205.0,
        CombatSoundKind::CounterAttack => 285.0,
    } * rng.range(0.97, 1.03);
    let amp = combat_amplitude(troops) * rng.range(0.9, 1.1);

    match kind {
        CombatSoundKind::WildernessExpansion => {
            let troop_scale = (troops / 2000.0).clamp(0.0, 1.0);
            ProceduralSound::Sweep(SweepSource::new(
                base_freq * 0.82,
                base_freq * (1.08 + 0.08 * troop_scale),
                0.09 + 0.04 * troop_scale,
                12.0,
                amp,
            ))
        }
        CombatSoundKind::AttackHuman => {
            let first = PulseSource::new(base_freq, 0.075, 13.0, amp);
            let second = PulseSource::new(base_freq * 0.86, 0.075, 13.0, amp * 0.88);
            ProceduralSound::DoublePulse(DoublePulseSource::new(first, second, 0.065))
        }
        CombatSoundKind::AttackEmpire => {
            let first = PulseSource::new(base_freq, 0.075, 11.0, amp);
            let second = PulseSource::new(base_freq * 1.24, 0.09, 10.0, amp * 0.9);
            ProceduralSound::DoublePulse(DoublePulseSource::new(first, second, 0.045))
        }
        CombatSoundKind::AttackTribe => {
            let duration = 0.085 + 0.025 * (troops / 3000.0).clamp(0.0, 1.0);
            ProceduralSound::Pulse(PulseSource::new(base_freq, duration, 13.0, amp * 0.9))
        }
        CombatSoundKind::CounterAttack => {
            let troop_scale = (troops / 3000.0).clamp(0.0, 1.0);
            ProceduralSound::Sweep(SweepSource::new(
                base_freq * 1.12,
                base_freq * 0.72,
                0.13 + 0.04 * troop_scale,
                9.0,
                amp,
            ))
        }
    }
}

pub fn play_deploy_sound(spatial: SpatialSoundParams) {
    queue_spatial(DeploySource::new(), spatial, SoundPriority::Normal);
}

pub fn play_combat_sound(
    kind: CombatSoundKind,
    troops: f32,
    seed: u32,
    spatial: SpatialSoundParams,
) {
    let SpatialSoundParams { wx, wy, .. } = spatial;
    queue_spatial(
        build_procedural_sound(kind, troops, seed, wx, wy),
        spatial,
        SoundPriority::Background,
    );
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
