use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample, Stream, StreamConfig};
use web_time::{Duration, Instant};

use super::tone::{note_envelope, warm_at};

pub(super) const SAMPLE_RATE: u32 = 22050;
pub(super) const OPEN_BACKOFF: Duration = Duration::from_secs(2);
pub(super) const MAX_VOICES: u8 = 3;
const MAX_TOTAL_VOICES: u8 = MAX_VOICES + 2;

pub(super) static MASTER_VOLUME: AtomicU32 = AtomicU32::new(800);

pub fn set_master_volume(volume: f32) {
    let vol_u32 = (volume * 1000.0).clamp(0.0, 1000.0) as u32;
    MASTER_VOLUME.store(vol_u32, Ordering::Relaxed);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SoundPriority {
    Background,
    Normal,
    Foreground,
}

pub(super) trait AudioSource: Iterator<Item = f32> + Send {
    fn sample_rate(&self) -> u32;
}

struct Voice {
    source: Box<dyn AudioSource>,
    left_gain: f32,
    right_gain: f32,
    source_rate: f32,
    phase: f32,
    current: Option<f32>,
    next: Option<f32>,
}

impl Voice {
    fn new(source: Box<dyn AudioSource>, left_gain: f32, right_gain: f32) -> Self {
        let source_rate = source.sample_rate() as f32;
        let mut source = source;
        let current = source.next();
        let next = source.next();
        Self {
            source,
            left_gain,
            right_gain,
            source_rate,
            phase: 0.0,
            current,
            next,
        }
    }

    fn next_frame(&mut self, output_rate: u32) -> Option<(f32, f32)> {
        let current = self.current?;
        let next = self.next.unwrap_or(current);
        let sample = current + (next - current) * self.phase;

        self.phase += self.source_rate / output_rate.max(1) as f32;
        while self.phase >= 1.0 {
            self.phase -= 1.0;
            self.current = self.next;
            self.next = self.source.next();
            if self.current.is_none() {
                break;
            }
        }

        Some((sample * self.left_gain, sample * self.right_gain))
    }
}

struct AudioMixer {
    voices: Vec<Voice>,
    sample_rate: u32,
}

impl AudioMixer {
    fn new() -> Self {
        Self {
            voices: Vec::new(),
            sample_rate: SAMPLE_RATE,
        }
    }

    fn set_sample_rate(&mut self, sample_rate: u32) {
        self.sample_rate = sample_rate;
    }

    fn push(&mut self, source: Box<dyn AudioSource>, left_gain: f32, right_gain: f32) {
        self.voices.push(Voice::new(source, left_gain, right_gain));
    }

    fn next_frame(&mut self) -> (f32, f32) {
        let mut left = 0.0;
        let mut right = 0.0;
        for voice in &mut self.voices {
            if let Some((voice_left, voice_right)) = voice.next_frame(self.sample_rate) {
                left += voice_left;
                right += voice_right;
            }
        }
        self.voices.retain(|voice| voice.current.is_some());

        let master = MASTER_VOLUME.load(Ordering::Relaxed) as f32 / 1000.0;
        (
            (left * master).clamp(-1.0, 1.0),
            (right * master).clamp(-1.0, 1.0),
        )
    }
}

struct AudioState {
    mixer: Arc<Mutex<AudioMixer>>,
    stream: Option<Stream>,
    open_backoff_until: Option<Instant>,
}

static AUDIO_STATE: OnceLock<Mutex<AudioState>> = OnceLock::new();

fn audio_state() -> &'static Mutex<AudioState> {
    AUDIO_STATE.get_or_init(|| {
        Mutex::new(AudioState {
            mixer: Arc::new(Mutex::new(AudioMixer::new())),
            stream: None,
            open_backoff_until: None,
        })
    })
}

pub(super) struct SimpleRng {
    state: u32,
}

impl SimpleRng {
    pub(super) fn new(seed: u32) -> Self {
        Self {
            state: if seed == 0 { 0x12345678 } else { seed },
        }
    }

    pub(super) fn next_u32(&mut self) -> u32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.state = x;
        x
    }

    fn next_f32(&mut self) -> f32 {
        (self.next_u32() & 0xFFFFFF) as f32 / 16777216.0
    }

    pub(super) fn range(&mut self, min: f32, max: f32) -> f32 {
        min + self.next_f32() * (max - min)
    }
}

pub(super) struct ArpeggioSource {
    sample_idx: u64,
    note_freqs: [f32; 4],
    note_durations: [u32; 4],
    decay_rate: f32,
    amplitude: f32,
    total_samples: u64,
}

impl ArpeggioSource {
    pub(super) fn new(
        note_freqs: [f32; 4],
        note_durations: [u32; 4],
        decay_rate: f32,
        amplitude: f32,
    ) -> Self {
        let total_samples = note_durations.iter().map(|&d| d as u64).sum();
        Self {
            sample_idx: 0,
            note_freqs,
            note_durations,
            decay_rate,
            amplitude,
            total_samples,
        }
    }
}

impl Iterator for ArpeggioSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.sample_idx >= self.total_samples {
            return None;
        }

        let mut accumulated_samples = 0;
        let mut note_idx = 0;
        for i in 0..4 {
            let next_accum = accumulated_samples + self.note_durations[i] as u64;
            if self.sample_idx < next_accum {
                note_idx = i;
                break;
            }
            accumulated_samples = next_accum;
        }

        let note_sample_idx = self.sample_idx - accumulated_samples;
        let note_t = note_sample_idx as f32 / SAMPLE_RATE as f32;
        let freq = self.note_freqs[note_idx];

        self.sample_idx += 1;
        if freq < 1.0 {
            return Some(0.0);
        }

        let val = warm_at(freq, note_t);

        let note_dur_secs = self.note_durations[note_idx] as f32 / SAMPLE_RATE as f32;
        let envelope = note_envelope(note_t, note_dur_secs, self.decay_rate, 0.008, 0.015);

        Some(val * envelope * self.amplitude)
    }
}

impl AudioSource for ArpeggioSource {
    fn sample_rate(&self) -> u32 {
        SAMPLE_RATE
    }
}

fn active_voice_count(mixer: &AudioMixer) -> u8 {
    mixer.voices.len().min(u8::MAX as usize) as u8
}

fn should_play(priority: SoundPriority, active: u8) -> bool {
    match priority {
        SoundPriority::Background => active < MAX_VOICES,
        SoundPriority::Normal => active < MAX_VOICES + 1,
        SoundPriority::Foreground => active < MAX_TOTAL_VOICES,
    }
}

fn priority_gain(priority: SoundPriority, active: u8) -> f32 {
    let base = match priority {
        SoundPriority::Background => 0.30,
        SoundPriority::Normal => 0.70,
        SoundPriority::Foreground => 1.0,
    };
    if priority == SoundPriority::Background {
        base / (1.0 + active as f32 * 0.2)
    } else {
        base
    }
}

fn build_output_stream<T>(
    device: &cpal::Device,
    config: &StreamConfig,
    mixer: Arc<Mutex<AudioMixer>>,
) -> Result<Stream, cpal::BuildStreamError>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = config.channels.max(1) as usize;
    device.build_output_stream(
        config,
        move |data: &mut [T], _| {
            let mut mixer = mixer.lock().unwrap_or_else(|e| e.into_inner());
            for frame in data.chunks_mut(channels) {
                let (left, right) = mixer.next_frame();
                for (channel, sample) in frame.iter_mut().enumerate() {
                    let value = match channel {
                        0 => left,
                        1 => right,
                        _ => (left + right) * 0.5,
                    };
                    *sample = T::from_sample(value);
                }
            }
        },
        |error| log::warn!("Audio stream error: {error}"),
        None,
    )
}

fn open_audio_stream(state: &mut AudioState) -> bool {
    if state.stream.is_some() {
        return true;
    }
    if state
        .open_backoff_until
        .is_some_and(|until| Instant::now() < until)
    {
        return false;
    }

    let host = cpal::default_host();
    let Some(device) = host.default_output_device() else {
        log::warn!("No default audio output device");
        state.open_backoff_until = Some(Instant::now() + OPEN_BACKOFF);
        return false;
    };
    let supported = match device.default_output_config() {
        Ok(config) => config,
        Err(error) => {
            log::warn!("Failed to read default audio output config: {error}");
            state.open_backoff_until = Some(Instant::now() + OPEN_BACKOFF);
            return false;
        }
    };
    let config = supported.config();
    state
        .mixer
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .set_sample_rate(config.sample_rate);

    let stream = match supported.sample_format() {
        SampleFormat::I8 => build_output_stream::<i8>(&device, &config, state.mixer.clone()),
        SampleFormat::I16 => build_output_stream::<i16>(&device, &config, state.mixer.clone()),
        SampleFormat::I24 => {
            build_output_stream::<cpal::I24>(&device, &config, state.mixer.clone())
        }
        SampleFormat::I32 => build_output_stream::<i32>(&device, &config, state.mixer.clone()),
        SampleFormat::I64 => build_output_stream::<i64>(&device, &config, state.mixer.clone()),
        SampleFormat::U8 => build_output_stream::<u8>(&device, &config, state.mixer.clone()),
        SampleFormat::U16 => build_output_stream::<u16>(&device, &config, state.mixer.clone()),
        SampleFormat::U24 => {
            build_output_stream::<cpal::U24>(&device, &config, state.mixer.clone())
        }
        SampleFormat::U32 => build_output_stream::<u32>(&device, &config, state.mixer.clone()),
        SampleFormat::U64 => build_output_stream::<u64>(&device, &config, state.mixer.clone()),
        SampleFormat::F32 => build_output_stream::<f32>(&device, &config, state.mixer.clone()),
        SampleFormat::F64 => build_output_stream::<f64>(&device, &config, state.mixer.clone()),
        _ => Err(cpal::BuildStreamError::StreamConfigNotSupported),
    };
    let Ok(stream) = stream else {
        log::warn!("Failed to build default audio output stream");
        state.open_backoff_until = Some(Instant::now() + OPEN_BACKOFF);
        return false;
    };
    if let Err(error) = stream.play() {
        log::warn!("Failed to start default audio output stream: {error}");
        state.open_backoff_until = Some(Instant::now() + OPEN_BACKOFF);
        return false;
    }
    state.stream = Some(stream);
    state.open_backoff_until = None;
    true
}

fn queue_source<S>(source: S, left: f32, right: f32, priority: SoundPriority)
where
    S: AudioSource + 'static,
{
    let state_lock = audio_state();
    let mut state = state_lock.lock().unwrap_or_else(|e| e.into_inner());
    let active = {
        let mixer = state.mixer.lock().unwrap_or_else(|e| e.into_inner());
        active_voice_count(&mixer)
    };
    if !should_play(priority, active) {
        return;
    }
    if !open_audio_stream(&mut state) {
        return;
    }
    let gain = priority_gain(priority, active);
    state.mixer.lock().unwrap_or_else(|e| e.into_inner()).push(
        Box::new(source),
        left * gain,
        right * gain,
    );
}

pub(super) fn spatial_gains(spatial: crate::SpatialSoundParams) -> (f32, f32, f32) {
    let crate::SpatialSoundParams {
        wx,
        wy,
        camera_x,
        camera_y,
        camera_zoom,
        screen_w,
        screen_h,
    } = spatial;
    const ZOOM_FLOOR: f32 = 1.0;
    const ZOOM_FULL: f32 = 10.0;
    const ZOOM_MIN_GAIN: f32 = 0.0;

    let screen_x = camera_x + wx * camera_zoom;

    let p = (screen_x / screen_w.max(1.0)).clamp(0.0, 1.0);
    let pan = 0.15 + p * 0.70;
    let mut left = (1.0 - pan).sqrt();
    let mut right = pan.sqrt();

    let zoom = camera_zoom.max(0.001);
    let world_center_x = (screen_w / 2.0 - camera_x) / zoom;
    let world_center_y = (screen_h / 2.0 - camera_y) / zoom;
    let dx_world = wx - world_center_x;
    let dy_world = wy - world_center_y;
    let distance_tiles = (dx_world * dx_world + dy_world * dy_world).sqrt();
    let half_w = screen_w / (2.0 * zoom);
    let half_h = screen_h / (2.0 * zoom);
    let max_dist = (half_w * half_w + half_h * half_h).sqrt() * 1.5;
    let distance_factor = (1.0 - distance_tiles / max_dist.max(1.0)).clamp(0.0, 1.0);
    let distance_attenuation = distance_factor.sqrt();

    let zoom_attenuation = if camera_zoom >= ZOOM_FULL {
        1.0
    } else if camera_zoom <= ZOOM_FLOOR {
        ZOOM_MIN_GAIN
    } else {
        ZOOM_MIN_GAIN
            + (1.0 - ZOOM_MIN_GAIN) * (camera_zoom - ZOOM_FLOOR) / (ZOOM_FULL - ZOOM_FLOOR)
    };

    let total_volume = distance_attenuation * zoom_attenuation;
    left *= total_volume;
    right *= total_volume;

    (left, right, total_volume)
}

pub(super) fn queue_spatial<S>(
    source: S,
    spatial: crate::SpatialSoundParams,
    priority: SoundPriority,
) where
    S: AudioSource + 'static,
{
    let (left, right, total_volume) = spatial_gains(spatial);

    if total_volume > 0.01 {
        queue_source(source, left, right, priority);
    }
}

pub(super) fn play_ui<S>(source: S)
where
    S: AudioSource + 'static,
{
    queue_source(source, 1.0, 1.0, SoundPriority::Foreground);
}
