use super::engine::{
    ArpeggioSource, AudioSource, SAMPLE_RATE, SimpleRng, SoundPriority, queue_spatial,
};
use super::tone::{sweep_envelope, warm_at};
use crate::{PlayerSoundType, SpatialSoundParams};

pub(super) struct PulseSource {
    sample_idx: u64,
    freq: f32,
    decay_rate: f32,
    pub(super) duration_samples: u64,
    amplitude: f32,
}

impl PulseSource {
    pub(super) fn new(freq: f32, duration_secs: f32, decay_rate: f32, amplitude: f32) -> Self {
        Self {
            sample_idx: 0,
            freq: freq.max(20.0),
            decay_rate,
            duration_samples: (SAMPLE_RATE as f32 * duration_secs.max(0.01)) as u64,
            amplitude,
        }
    }
}

impl Iterator for PulseSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.sample_idx >= self.duration_samples {
            return None;
        }
        let t = self.sample_idx as f32 / SAMPLE_RATE as f32;
        let duration = self.duration_samples as f32 / SAMPLE_RATE as f32;
        let val = warm_at(self.freq, t);
        let envelope = sweep_envelope(t, duration, self.decay_rate, 0.008, 0.02);
        self.sample_idx += 1;
        Some(val * envelope * self.amplitude)
    }
}

impl AudioSource for PulseSource {
    fn sample_rate(&self) -> u32 {
        SAMPLE_RATE
    }
}

fn note_samples(secs: f32) -> u32 {
    (SAMPLE_RATE as f32 * secs) as u32
}

fn build_death_sound(player_type: PlayerSoundType, seed: u32, wx: f32, wy: f32) -> ArpeggioSource {
    let mut rng = SimpleRng::new(seed ^ wx.to_bits().rotate_left(7) ^ wy.to_bits().rotate_left(19));
    let jitter = rng.range(0.985, 1.015);
    let (freqs, note_count, note_duration, decay) = match player_type {
        PlayerSoundType::Human => ([392.0, 330.0, 277.0, 0.0], 3, 0.11, 6.5),
        PlayerSoundType::Nation => ([330.0, 277.0, 220.0, 0.0], 3, 0.12, 6.0),
        PlayerSoundType::Bot => ([294.0, 247.0, 0.0, 0.0], 2, 0.10, 7.5),
    };
    let note_duration = note_samples(note_duration);
    let mut note_freqs = freqs;
    for freq in note_freqs.iter_mut().take(note_count) {
        *freq *= jitter;
    }
    let mut note_durations = [0; 4];
    for duration in note_durations.iter_mut().take(note_count) {
        *duration = note_duration;
    }
    ArpeggioSource::new(note_freqs, note_durations, decay, 0.095)
}

pub fn play_death_sound(player_type: PlayerSoundType, seed: u32, spatial: SpatialSoundParams) {
    let SpatialSoundParams { wx, wy, .. } = spatial;
    queue_spatial(
        build_death_sound(player_type, seed, wx, wy),
        spatial,
        SoundPriority::Foreground,
    );
}
