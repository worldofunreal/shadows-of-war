use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock, RwLock};

use super::engine::{AudioSource, SAMPLE_RATE};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SfxId {
    AttackOrder,
    AttackStalled,
    UnderAttack,
    CombatImpact,
    DeployTroops,
    PlayerEliminated,
    BuildingPlacement,
    BuildingUpgradeStart,
    BuildingCompleted,
    NukeLaunch,
    NukeImpact,
    Victory,
    Defeat,
}

static SFX_BANK: OnceLock<RwLock<HashMap<SfxId, Vec<Arc<[f32]>>>>> = OnceLock::new();
static RNG_COUNTER: AtomicU64 = AtomicU64::new(0x9e37_79b9_7f4a_7c15);

fn bank() -> &'static RwLock<HashMap<SfxId, Vec<Arc<[f32]>>>> {
    SFX_BANK.get_or_init(|| RwLock::new(HashMap::new()))
}

fn next_random_index(len: usize) -> usize {
    let prev = RNG_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut x = prev ^ (prev << 13);
    x ^= x >> 7;
    x ^= x << 17;
    (x as usize) % len
}

/// Parses a mono 16-bit PCM WAV file at 44100 Hz into normalized `f32` samples.
pub fn parse_wav_mono_f32(bytes: &[u8]) -> Option<Vec<f32>> {
    if bytes.len() < 44 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return None;
    }
    let mut pos = 12;
    let mut channels = 0u16;
    let mut sample_rate = 0u32;
    let mut bits_per_sample = 0u16;
    let mut data_slice: Option<&[u8]> = None;

    while pos + 8 <= bytes.len() {
        let chunk_id = &bytes[pos..pos + 4];
        let chunk_len = u32::from_le_bytes(bytes[pos + 4..pos + 8].try_into().ok()?) as usize;
        pos += 8;
        if pos + chunk_len > bytes.len() {
            break;
        }
        let chunk = &bytes[pos..pos + chunk_len];
        if chunk_id == b"fmt " && chunk.len() >= 16 {
            let audio_fmt = u16::from_le_bytes([chunk[0], chunk[1]]);
            if audio_fmt != 1 {
                return None;
            }
            channels = u16::from_le_bytes([chunk[2], chunk[3]]);
            sample_rate = u32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]);
            bits_per_sample = u16::from_le_bytes([chunk[14], chunk[15]]);
        } else if chunk_id == b"data" {
            data_slice = Some(chunk);
        }
        pos += chunk_len + (chunk_len & 1);
    }

    if channels != 1 || sample_rate != SAMPLE_RATE || bits_per_sample != 16 {
        return None;
    }
    let data = data_slice?;
    let samples = data
        .chunks_exact(2)
        .map(|pair| i16::from_le_bytes([pair[0], pair[1]]) as f32 / 32768.0)
        .collect();
    Some(samples)
}

/// Registers a decoded WAV clip as a randomizable variant for `id`.
pub fn register_sfx_wav(id: SfxId, wav_bytes: &[u8]) -> bool {
    let Some(samples) = parse_wav_mono_f32(wav_bytes) else {
        return false;
    };
    if let Ok(mut map) = bank().write() {
        map.entry(id).or_default().push(Arc::from(samples));
        true
    } else {
        false
    }
}

/// Picks a random registered clip for `id`, returning a `ClipSource` scaled by `gain`.
pub fn pick_clip(id: SfxId, gain: f32) -> Option<ClipSource> {
    pick_clip_pitched(id, gain, 1.0)
}

/// Picks a random registered clip for `id`, scaled by `gain` and playback `pitch_ratio`.
pub fn pick_clip_pitched(id: SfxId, gain: f32, pitch_ratio: f32) -> Option<ClipSource> {
    let map = bank().read().ok()?;
    let variants = map.get(&id)?;
    if variants.is_empty() {
        return None;
    }
    let idx = if variants.len() == 1 {
        0
    } else {
        next_random_index(variants.len())
    };
    Some(ClipSource {
        samples: Arc::clone(&variants[idx]),
        idx: 0,
        gain,
        pitch_ratio: pitch_ratio.clamp(0.5, 2.5),
    })
}

pub struct ClipSource {
    samples: Arc<[f32]>,
    idx: usize,
    gain: f32,
    pitch_ratio: f32,
}

impl Iterator for ClipSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let s = *self.samples.get(self.idx)?;
        self.idx += 1;
        Some(s * self.gain)
    }
}

impl AudioSource for ClipSource {
    fn sample_rate(&self) -> u32 {
        ((SAMPLE_RATE as f32) * self.pitch_ratio).round().max(8000.0) as u32
    }
}
