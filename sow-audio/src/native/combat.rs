use super::clip::{SfxId, pick_clip};
use super::engine::{SoundPriority, queue_spatial};
use crate::SpatialSoundParams;

pub fn play_attack_order_sound(
    _commitment: f32,
    _against_enemy: bool,
    spatial: SpatialSoundParams,
) {
    if let Some(clip) = pick_clip(SfxId::AttackOrder, 0.45) {
        queue_spatial(clip, spatial, SoundPriority::Normal);
    }
}

pub fn play_attack_stalled_sound(severity: f32, spatial: SpatialSoundParams) {
    if let Some(clip) = pick_clip(
        SfxId::AttackStalled,
        0.40 + 0.15 * severity.clamp(0.0, 1.0),
    ) {
        queue_spatial(clip, spatial, SoundPriority::Normal);
    }
}

pub fn play_deploy_sound(spatial: SpatialSoundParams) {
    if let Some(clip) = pick_clip(SfxId::DeployTroops, 0.40) {
        queue_spatial(clip, spatial, SoundPriority::Normal);
    }
}

pub fn play_under_attack_sound(severity: f32, spatial: SpatialSoundParams) {
    let s = severity.clamp(0.0, 1.0);
    if let Some(clip) = pick_clip(SfxId::UnderAttack, 0.40 + 0.20 * s) {
        queue_spatial(clip, spatial, SoundPriority::Foreground);
    }
}
