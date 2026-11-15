use super::clip::{SfxId, pick_clip};
use super::engine::{SoundPriority, queue_spatial};
use crate::{BuildingSoundKind, SpatialSoundParams};

pub fn play_building_placement_sound(_kind: BuildingSoundKind, spatial: SpatialSoundParams) {
    if let Some(clip) = pick_clip(SfxId::BuildingPlacement, 0.45) {
        queue_spatial(clip, spatial, SoundPriority::Normal);
    }
}

pub fn play_building_upgrade_start_sound(spatial: SpatialSoundParams) {
    if let Some(clip) = pick_clip(SfxId::BuildingUpgradeStart, 0.45) {
        queue_spatial(clip, spatial, SoundPriority::Normal);
    }
}
