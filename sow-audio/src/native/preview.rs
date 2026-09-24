use std::fs;
use std::io;
use std::path::Path;

use super::engine::{SAMPLE_RATE, capture_preview_output};
use crate::{BuildingSoundKind, CombatSoundKind, SpatialSoundParams};

#[derive(Clone, Copy)]
enum Event {
    Combat(CombatSoundKind),
    BuildingComplete(BuildingSoundKind),
    NuclearLaunch,
    NuclearImpact(u8),
}

const EVENTS: [(&str, Event); 8] = [
    (
        "proposal_neutral_expansion",
        Event::Combat(CombatSoundKind::WildernessExpansion),
    ),
    (
        "proposal_territory_taken_from_tribe",
        Event::Combat(CombatSoundKind::AttackTribe),
    ),
    (
        "proposal_territory_taken_from_nation",
        Event::Combat(CombatSoundKind::AttackEmpire),
    ),
    (
        "proposal_territory_taken_from_human",
        Event::Combat(CombatSoundKind::AttackHuman),
    ),
    (
        "proposal_territory_lost",
        Event::Combat(CombatSoundKind::CounterAttack),
    ),
    (
        "proposal_city_complete",
        Event::BuildingComplete(BuildingSoundKind::City),
    ),
    ("proposal_nuclear_launch", Event::NuclearLaunch),
    ("proposal_nuclear_impact_level_1", Event::NuclearImpact(1)),
];

pub fn export_sfx_preview(output_dir: &Path) -> io::Result<usize> {
    fs::create_dir_all(output_dir)?;
    let spatial = center_spatial();

    for (name, event) in EVENTS {
        let frames = render_clip(event, spatial)?;
        let wav = encode_wav(&frames)?;
        let path = output_dir.join(format!("{name}.wav"));
        fs::write(&path, wav)?;
        println!("Wrote {}", path.display());
    }

    Ok(EVENTS.len())
}

fn render_clip(event: Event, spatial: SpatialSoundParams) -> io::Result<Vec<[f32; 2]>> {
    let frames = capture_preview_output(|| match event {
        Event::Combat(kind) => crate::play_combat_sound(kind, 5000.0, 0x534f_5701, spatial),
        Event::BuildingComplete(kind) => crate::play_building_completed_sound(kind, spatial),
        Event::NuclearLaunch => crate::play_nuke_launch_sound(spatial),
        Event::NuclearImpact(level) => crate::play_nuke_impact_sound(level, spatial),
    });

    if frames.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "SFX runtime source rendered no samples",
        ));
    }
    Ok(frames)
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
                    "SFX runtime output contains an invalid or clipped sample",
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
    fn runtime_pack_has_one_repeatable_clip_per_event() {
        assert_eq!(EVENTS.len(), 8);
        let spatial = center_spatial();
        for (_, event) in EVENTS {
            let frames = render_clip(event, spatial).unwrap();
            assert_eq!(frames, render_clip(event, spatial).unwrap());
            let rms = (frames
                .iter()
                .flatten()
                .map(|sample| f64::from(*sample) * f64::from(*sample))
                .sum::<f64>()
                / (frames.len() * 2) as f64)
                .sqrt() as f32;
            assert!(rms > 0.000_1, "SFX preview is silent: RMS {rms}");

            let wav = encode_wav(&frames).unwrap();
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
