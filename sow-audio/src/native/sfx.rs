use super::clip::{SfxId, pick_clip};
use super::engine::{SoundPriority, queue_spatial};
use crate::{BuildingSoundKind, CombatSoundKind, SpatialSoundParams};

pub fn play_combat_sound(
    _kind: CombatSoundKind,
    troops: f32,
    _seed: u32,
    spatial: SpatialSoundParams,
) {
    let troops_scale = (troops / 5000.0).clamp(0.15, 1.0).sqrt();
    if let Some(clip) = pick_clip(SfxId::CombatImpact, 0.28 + 0.17 * troops_scale) {
        queue_spatial(clip, spatial, SoundPriority::Background);
    }
}

pub fn play_building_completed_sound(_kind: BuildingSoundKind, spatial: SpatialSoundParams) {
    if let Some(clip) = pick_clip(SfxId::BuildingCompleted, 0.45) {
        queue_spatial(clip, spatial, SoundPriority::Foreground);
    }
}

pub fn play_nuke_launch_sound(spatial: SpatialSoundParams) {
    if let Some(clip) = pick_clip(SfxId::NukeLaunch, 0.50) {
        queue_spatial(clip, spatial, SoundPriority::Foreground);
    }
}

pub fn play_nuke_impact_sound(level: u8, spatial: SpatialSoundParams) {
    let clip_gain = match level {
        1 => 0.45,
        2 => 0.58,
        _ => 0.72,
    };
    if let Some(clip) = pick_clip(SfxId::NukeImpact, clip_gain) {
        queue_spatial(clip, spatial, SoundPriority::Foreground);
    }
}
