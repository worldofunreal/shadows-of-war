use super::engine::{AudioSource, SAMPLE_RATE, SimpleRng, SoundPriority, queue_spatial};
use crate::{BuildingSoundKind, CombatSoundKind, SpatialSoundParams};

#[derive(Clone, Copy)]
enum Event {
    Combat(CombatSoundKind),
    BuildingComplete(BuildingSoundKind),
    NuclearLaunch,
    NuclearImpact(u8),
}

impl Event {
    fn duration(self) -> f32 {
        match self {
            Self::Combat(CombatSoundKind::WildernessExpansion) => 0.20,
            Self::Combat(CombatSoundKind::AttackTribe) => 0.23,
            Self::Combat(CombatSoundKind::AttackEmpire) => 0.39,
            Self::Combat(CombatSoundKind::AttackHuman) => 0.55,
            Self::Combat(CombatSoundKind::CounterAttack) => 0.39,
            Self::BuildingComplete(_) => 0.50,
            Self::NuclearLaunch => 1.55,
            Self::NuclearImpact(level) => 1.20 + f32::from(level) * 0.30,
        }
    }

    fn seed(self) -> u32 {
        match self {
            Self::Combat(CombatSoundKind::WildernessExpansion) => 1,
            Self::Combat(CombatSoundKind::AttackTribe) => 2,
            Self::Combat(CombatSoundKind::AttackEmpire) => 3,
            Self::Combat(CombatSoundKind::AttackHuman) => 4,
            Self::Combat(CombatSoundKind::CounterAttack) => 5,
            Self::BuildingComplete(BuildingSoundKind::City) => 6,
            Self::BuildingComplete(BuildingSoundKind::Bunker) => 7,
            Self::BuildingComplete(BuildingSoundKind::Factory) => 8,
            Self::BuildingComplete(BuildingSoundKind::Port) => 9,
            Self::NuclearLaunch => 10,
            Self::NuclearImpact(_) => 11,
        }
    }
}

struct SfxSource {
    event: Event,
    sample_idx: u64,
    duration_samples: u64,
    rng: SimpleRng,
    gain: f32,
    phases: [f32; 3],
    low_noise: f32,
    mid_noise: f32,
}

impl SfxSource {
    fn new(event: Event, seed: u32, gain: f32) -> Self {
        Self {
            event,
            sample_idx: 0,
            duration_samples: (SAMPLE_RATE as f32 * event.duration()) as u64,
            rng: SimpleRng::new(seed ^ event.seed()),
            gain,
            phases: [0.0; 3],
            low_noise: 0.0,
            mid_noise: 0.0,
        }
    }

    fn next_sample(&mut self, t: f32, progress: f32) -> f32 {
        let white = self.rng.range(-1.0, 1.0);
        let mid_filter = if matches!(self.event, Event::NuclearLaunch) {
            0.035
        } else {
            0.22
        };
        self.low_noise += (white - self.low_noise) * 0.025;
        self.mid_noise += (white - self.mid_noise) * mid_filter;
        let filtered_noise = self.mid_noise - self.low_noise;

        match self.event {
            Event::Combat(kind) => self.combat_sample(kind, t, filtered_noise),
            Event::BuildingComplete(kind) => self.city_sample(kind, t, filtered_noise),
            Event::NuclearLaunch => self.launch_sample(t, progress),
            Event::NuclearImpact(_) => self.impact_sample(t, filtered_noise),
        }
    }

    fn combat_sample(&self, kind: CombatSoundKind, t: f32, noise: f32) -> f32 {
        match kind {
            CombatSoundKind::WildernessExpansion => {
                let rise = smoothstep(t / 0.12);
                ring(t, 265.0 + 220.0 * rise, 15.0) * 0.52
                    + ring(t, 480.0 + 260.0 * rise, 25.0) * 0.17
                    + noise * (-t * 95.0).exp() * 0.34
            }
            CombatSoundKind::AttackTribe => strike(
                t,
                0.0,
                noise,
                [155.0, 300.0, 520.0],
                [9.0, 17.0, 27.0],
                0.72,
            ),
            CombatSoundKind::AttackEmpire => {
                strike(
                    t,
                    0.0,
                    noise,
                    [135.0, 260.0, 430.0],
                    [8.0, 15.0, 24.0],
                    0.78,
                ) + strike(
                    t,
                    0.115,
                    noise,
                    [175.0, 345.0, 560.0],
                    [13.0, 21.0, 30.0],
                    0.52,
                )
            }
            CombatSoundKind::AttackHuman => {
                strike(t, 0.0, noise, [82.0, 172.0, 325.0], [2.2, 4.5, 8.0], 0.82)
                    + strike(
                        t,
                        0.13,
                        noise,
                        [145.0, 285.0, 470.0],
                        [8.0, 15.0, 23.0],
                        0.30,
                    )
            }
            CombatSoundKind::CounterAttack => {
                strike(
                    t,
                    0.0,
                    noise,
                    [215.0, 365.0, 560.0],
                    [12.0, 20.0, 29.0],
                    0.50,
                ) + strike(
                    t,
                    0.085,
                    noise,
                    [185.0, 305.0, 470.0],
                    [12.0, 19.0, 28.0],
                    0.58,
                ) + strike(
                    t,
                    0.17,
                    noise,
                    [145.0, 230.0, 360.0],
                    [11.0, 17.0, 25.0],
                    0.42,
                )
            }
        }
    }

    fn city_sample(&self, kind: BuildingSoundKind, t: f32, noise: f32) -> f32 {
        let (freqs, decay, shimmer) = match kind {
            BuildingSoundKind::City => ([245.0, 410.0, 635.0], [8.0, 14.0, 24.0], 575.0),
            BuildingSoundKind::Bunker => ([205.0, 350.0, 545.0], [7.0, 13.0, 22.0], 490.0),
            BuildingSoundKind::Factory => ([275.0, 455.0, 700.0], [9.0, 15.0, 25.0], 625.0),
            BuildingSoundKind::Port => ([230.0, 385.0, 590.0], [8.0, 14.0, 23.0], 540.0),
        };
        strike(t, 0.0, noise, freqs, decay, 0.62) + ring(t - 0.105, shimmer, 9.0) * 0.22
    }

    fn launch_sample(&mut self, t: f32, progress: f32) -> f32 {
        let duration = self.duration_samples as f32 / SAMPLE_RATE as f32;
        let envelope = smoothstep(t / 0.30) * (-0.20 * t).exp() * tail_fade(t, duration, 0.20);
        let base = 62.0 + 36.0 * smoothstep(progress);
        self.phases[0] += std::f32::consts::TAU * base / SAMPLE_RATE as f32;
        self.phases[1] += std::f32::consts::TAU * (base * 1.72) / SAMPLE_RATE as f32;
        self.phases[2] += std::f32::consts::TAU * (base * 2.81) / SAMPLE_RATE as f32;
        let rounded_body =
            self.phases[0].sin() * 0.48 + self.phases[1].sin() * 0.19 + self.phases[2].sin() * 0.06;
        (rounded_body + self.low_noise * 0.10) * envelope
    }

    fn impact_sample(&self, t: f32, noise: f32) -> f32 {
        let duration = self.duration_samples as f32 / SAMPLE_RATE as f32;
        let tail = tail_fade(t, duration, 0.24);
        let hit = noise * 0.80 * (-t * 48.0).exp();
        let body = ring(t, 76.0, 1.4) * 0.22
            + ring(t, 168.0, 2.4) * 0.38
            + ring(t, 325.0, 4.8) * 0.21
            + self.low_noise * (-1.35 * t).exp() * 0.12;
        (hit + body) * tail
    }
}

impl Iterator for SfxSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.sample_idx >= self.duration_samples {
            return None;
        }
        let t = self.sample_idx as f32 / SAMPLE_RATE as f32;
        let progress = self.sample_idx as f32 / self.duration_samples as f32;
        let sample = self.next_sample(t, progress) * self.gain;
        self.sample_idx += 1;
        Some(sample)
    }
}

impl AudioSource for SfxSource {
    fn sample_rate(&self) -> u32 {
        SAMPLE_RATE
    }
}

fn strike(
    t: f32,
    onset: f32,
    noise: f32,
    frequencies: [f32; 3],
    decays: [f32; 3],
    noise_gain: f32,
) -> f32 {
    let local_t = t - onset;
    if local_t < 0.0 {
        return 0.0;
    }
    let body = ring(local_t, frequencies[0], decays[0]) * 0.46
        + ring(local_t, frequencies[1], decays[1]) * 0.29
        + ring(local_t, frequencies[2], decays[2]) * 0.13;
    body + noise * noise_gain * (-local_t * 75.0).exp()
}

fn ring(t: f32, frequency: f32, decay: f32) -> f32 {
    if t < 0.0 {
        return 0.0;
    }
    (std::f32::consts::TAU * frequency * t).sin() * (-decay * t).exp()
}

fn smoothstep(value: f32) -> f32 {
    let x = value.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

fn tail_fade(t: f32, duration: f32, tail: f32) -> f32 {
    if t > duration - tail {
        smoothstep((duration - t) / tail)
    } else {
        1.0
    }
}

pub fn play_combat_sound(
    kind: CombatSoundKind,
    troops: f32,
    seed: u32,
    spatial: SpatialSoundParams,
) {
    let SpatialSoundParams { wx, wy, .. } = spatial;
    let position_seed = wx.to_bits().rotate_left(11) ^ wy.to_bits().rotate_left(23);
    let troops_scale = (troops / 5000.0).clamp(0.15, 1.0).sqrt();
    let gain = 0.075 + 0.045 * troops_scale;
    queue_spatial(
        SfxSource::new(Event::Combat(kind), seed ^ position_seed, gain),
        spatial,
        SoundPriority::Background,
    );
}

pub fn play_building_completed_sound(kind: BuildingSoundKind, spatial: SpatialSoundParams) {
    queue_spatial(
        SfxSource::new(
            Event::BuildingComplete(kind),
            Event::BuildingComplete(kind).seed(),
            0.16,
        ),
        spatial,
        SoundPriority::Foreground,
    );
}

pub fn play_nuke_launch_sound(spatial: SpatialSoundParams) {
    queue_spatial(
        SfxSource::new(Event::NuclearLaunch, Event::NuclearLaunch.seed(), 0.22),
        spatial,
        SoundPriority::Foreground,
    );
}

pub fn play_nuke_impact_sound(level: u8, spatial: SpatialSoundParams) {
    let event = Event::NuclearImpact(level);
    queue_spatial(
        SfxSource::new(event, event.seed(), 0.18),
        spatial,
        SoundPriority::Foreground,
    );
}
