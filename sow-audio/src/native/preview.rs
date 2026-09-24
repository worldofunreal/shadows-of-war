use std::fs;
use std::io;
use std::path::Path;

use super::engine::{
    AudioSource, SAMPLE_RATE, SimpleRng, SoundPriority, capture_preview_output, queue_spatial,
};
use crate::{BuildingSoundKind, CombatSoundKind, SpatialSoundParams};

#[derive(Clone, Copy)]
enum Event {
    Capture(CombatSoundKind),
    BuildingComplete,
    NukeLaunch,
    NukeImpact,
}

#[derive(Clone, Copy)]
enum Variant {
    Current,
    A,
    B,
}

#[derive(Clone, Copy)]
enum PrototypeStyle {
    Resonant,
    Layered,
}

const EVENTS: [(&str, Event); 6] = [
    (
        "capture_tribe",
        Event::Capture(CombatSoundKind::AttackTribe),
    ),
    (
        "capture_nation",
        Event::Capture(CombatSoundKind::AttackEmpire),
    ),
    (
        "capture_human",
        Event::Capture(CombatSoundKind::AttackHuman),
    ),
    ("building_complete_city", Event::BuildingComplete),
    ("nuke_launch", Event::NukeLaunch),
    ("nuke_impact_level_1", Event::NukeImpact),
];

pub fn export_sfx_preview(output_dir: &Path) -> io::Result<usize> {
    fs::create_dir_all(output_dir)?;
    let spatial = center_spatial();
    let mut exported = 0;

    for (name, event) in EVENTS {
        for (suffix, variant) in [
            ("current", Variant::Current),
            ("a", Variant::A),
            ("b", Variant::B),
        ] {
            let frames = render_clip(event, variant, spatial)?;
            let wav = encode_wav(&frames)?;
            let path = output_dir.join(format!("{name}_{suffix}.wav"));
            fs::write(&path, wav)?;
            println!("Wrote {}", path.display());
            exported += 1;
        }
    }

    Ok(exported)
}

fn render_clip(
    event: Event,
    variant: Variant,
    spatial: SpatialSoundParams,
) -> io::Result<Vec<[f32; 2]>> {
    let frames = capture_preview_output(|| match variant {
        Variant::Current => play_current(event, spatial),
        Variant::A | Variant::B => {
            let style = match variant {
                Variant::A => PrototypeStyle::Resonant,
                Variant::B => PrototypeStyle::Layered,
                Variant::Current => unreachable!(),
            };
            queue_spatial(
                PrototypeSource::new(event, style),
                spatial,
                event.priority(),
            );
        }
    });

    if frames.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "SFX preview rendered no samples",
        ));
    }
    Ok(frames)
}

fn play_current(event: Event, spatial: SpatialSoundParams) {
    match event {
        Event::Capture(kind) => crate::play_combat_sound(kind, 2500.0, 0x534f_5701, spatial),
        Event::BuildingComplete => {
            crate::play_building_completed_sound(BuildingSoundKind::City, spatial)
        }
        Event::NukeLaunch => crate::play_nuke_launch_sound(spatial),
        Event::NukeImpact => crate::play_nuke_impact_sound(1, spatial),
    }
}

impl Event {
    fn priority(self) -> SoundPriority {
        match self {
            Self::Capture(_) => SoundPriority::Background,
            Self::BuildingComplete | Self::NukeLaunch | Self::NukeImpact => {
                SoundPriority::Foreground
            }
        }
    }
}

fn center_spatial() -> SpatialSoundParams {
    SpatialSoundParams {
        wx: 64.0,
        wy: 36.0,
        camera_x: 0.0,
        camera_y: 0.0,
        camera_zoom: 10.0,
        screen_w: 1280.0,
        screen_h: 720.0,
    }
}

#[derive(Clone, Copy)]
struct Profile {
    frequency: f32,
    duration: f32,
    amplitude: f32,
    attack: f32,
    decay: f32,
}

fn profile(event: Event) -> Profile {
    match event {
        Event::Capture(CombatSoundKind::AttackTribe) => Profile {
            frequency: 150.0,
            duration: 0.26,
            amplitude: 0.105,
            attack: 0.012,
            decay: 7.0,
        },
        Event::Capture(CombatSoundKind::AttackEmpire) => Profile {
            frequency: 124.0,
            duration: 0.34,
            amplitude: 0.11,
            attack: 0.018,
            decay: 5.5,
        },
        Event::Capture(CombatSoundKind::AttackHuman) => Profile {
            frequency: 102.0,
            duration: 0.40,
            amplitude: 0.115,
            attack: 0.022,
            decay: 4.5,
        },
        Event::Capture(CombatSoundKind::WildernessExpansion) => Profile {
            frequency: 174.0,
            duration: 0.28,
            amplitude: 0.105,
            attack: 0.012,
            decay: 7.0,
        },
        Event::Capture(CombatSoundKind::CounterAttack) => Profile {
            frequency: 114.0,
            duration: 0.34,
            amplitude: 0.11,
            attack: 0.018,
            decay: 5.5,
        },
        Event::BuildingComplete => Profile {
            frequency: 224.0,
            duration: 0.54,
            amplitude: 0.11,
            attack: 0.028,
            decay: 4.0,
        },
        Event::NukeLaunch => Profile {
            frequency: 62.0,
            duration: 1.35,
            amplitude: 0.15,
            attack: 0.18,
            decay: 0.85,
        },
        Event::NukeImpact => Profile {
            frequency: 48.0,
            duration: 1.50,
            amplitude: 0.19,
            attack: 0.014,
            decay: 1.8,
        },
    }
}

struct PrototypeSource {
    sample_idx: u64,
    duration_samples: u64,
    event: Event,
    style: PrototypeStyle,
    profile: Profile,
    rng: SimpleRng,
    filtered_noise: f32,
}

impl PrototypeSource {
    fn new(event: Event, style: PrototypeStyle) -> Self {
        let profile = profile(event);
        let style_seed = match style {
            PrototypeStyle::Resonant => 0xA11C_E001,
            PrototypeStyle::Layered => 0xB01D_E002,
        };
        Self {
            sample_idx: 0,
            duration_samples: (SAMPLE_RATE as f32 * profile.duration) as u64,
            event,
            style,
            profile,
            rng: SimpleRng::new(style_seed ^ event_seed(event)),
            filtered_noise: 0.0,
        }
    }
}

fn event_seed(event: Event) -> u32 {
    match event {
        Event::Capture(CombatSoundKind::AttackTribe) => 1,
        Event::Capture(CombatSoundKind::AttackEmpire) => 2,
        Event::Capture(CombatSoundKind::AttackHuman) => 3,
        Event::Capture(CombatSoundKind::WildernessExpansion) => 4,
        Event::Capture(CombatSoundKind::CounterAttack) => 5,
        Event::BuildingComplete => 6,
        Event::NukeLaunch => 7,
        Event::NukeImpact => 8,
    }
}

fn smoothstep(value: f32) -> f32 {
    let x = value.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

impl Iterator for PrototypeSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.sample_idx >= self.duration_samples {
            return None;
        }

        let t = self.sample_idx as f32 / SAMPLE_RATE as f32;
        let duration = self.profile.duration;
        let (ratios, weights, attack_scale, noise_amount) = match self.style {
            PrototypeStyle::Resonant => ([1.0, 1.57, 2.31], [0.50, 0.31, 0.19], 1.0, 0.20),
            PrototypeStyle::Layered => ([1.0, 1.29, 1.93], [0.62, 0.25, 0.13], 1.5, 0.0),
        };
        let attack = smoothstep(t / (self.profile.attack * attack_scale));
        let fade_secs = (duration * 0.24).clamp(0.06, 0.28);
        let tail = if t > duration - fade_secs {
            smoothstep((duration - t) / fade_secs)
        } else {
            1.0
        };
        let envelope = attack * (-self.profile.decay * t).exp() * tail;
        let phase = std::f32::consts::TAU * self.profile.frequency * t;
        let resonances = (0..3)
            .map(|i| (phase * ratios[i] + i as f32 * 0.19).sin() * weights[i])
            .sum::<f32>();

        let noise = self.rng.range(-1.0, 1.0);
        self.filtered_noise += (noise - self.filtered_noise) * 0.09;
        let transient = if matches!(self.event, Event::NukeLaunch) {
            0.0
        } else {
            self.filtered_noise * smoothstep(t / 0.012) * (-t * 45.0).exp() * noise_amount
        };

        self.sample_idx += 1;
        Some((resonances * envelope + transient) * self.profile.amplitude)
    }
}

impl AudioSource for PrototypeSource {
    fn sample_rate(&self) -> u32 {
        SAMPLE_RATE
    }
}

fn encode_wav(frames: &[[f32; 2]]) -> io::Result<Vec<u8>> {
    if frames.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "cannot write an empty SFX preview",
        ));
    }

    let data_len = frames
        .len()
        .checked_mul(4)
        .and_then(|len| u32::try_from(len).ok())
        .ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "SFX preview exceeds WAV limit")
        })?;
    let mut wav = Vec::with_capacity(data_len as usize + 44);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(data_len + 36).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16_u32.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&2_u16.to_le_bytes());
    wav.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    wav.extend_from_slice(&(SAMPLE_RATE * 4).to_le_bytes());
    wav.extend_from_slice(&4_u16.to_le_bytes());
    wav.extend_from_slice(&16_u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());

    for frame in frames {
        for sample in frame {
            if !sample.is_finite() || sample.abs() >= 1.0 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "SFX preview contains an invalid or clipped sample",
                ));
            }
            let pcm = (sample * i16::MAX as f32).round() as i16;
            wav.extend_from_slice(&pcm.to_le_bytes());
        }
    }

    Ok(wav)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_pack_is_repeatable_and_writes_safe_pcm_wav() {
        let spatial = center_spatial();
        for (_, event) in EVENTS {
            let current = render_clip(event, Variant::Current, spatial).unwrap();
            let current_again = render_clip(event, Variant::Current, spatial).unwrap();
            let variant_a = render_clip(event, Variant::A, spatial).unwrap();
            let variant_a_again = render_clip(event, Variant::A, spatial).unwrap();
            let variant_b = render_clip(event, Variant::B, spatial).unwrap();
            let variant_b_again = render_clip(event, Variant::B, spatial).unwrap();

            assert_eq!(current, current_again);
            assert_eq!(variant_a, variant_a_again);
            assert_eq!(variant_b, variant_b_again);
            assert_ne!(variant_a, variant_b);

            for frames in [&current, &variant_a, &variant_b] {
                assert!(
                    frames
                        .iter()
                        .any(|frame| frame[0] != 0.0 || frame[1] != 0.0)
                );
                let wav = encode_wav(frames).unwrap();
                assert_eq!(&wav[0..4], b"RIFF");
                assert_eq!(&wav[8..12], b"WAVE");
                assert_eq!(u16::from_le_bytes([wav[22], wav[23]]), 2);
                assert_eq!(
                    u32::from_le_bytes(wav[24..28].try_into().unwrap()),
                    SAMPLE_RATE
                );
                assert_eq!(
                    u32::from_le_bytes(wav[40..44].try_into().unwrap()) as usize,
                    frames.len() * 4
                );
            }
        }
    }
}
