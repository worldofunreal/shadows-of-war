use super::clip::{SfxId, pick_clip};
use super::engine::{SoundPriority, queue_spatial};
use crate::{PlayerSoundType, SpatialSoundParams};

pub fn play_death_sound(
    _player_type: PlayerSoundType,
    _seed: u32,
    spatial: SpatialSoundParams,
) {
    if let Some(clip) = pick_clip(SfxId::PlayerEliminated, 0.45) {
        queue_spatial(clip, spatial, SoundPriority::Foreground);
    }
}
