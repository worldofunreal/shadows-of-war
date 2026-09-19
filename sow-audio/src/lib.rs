//! Procedural mobile-RTS harmonic SFX with spatial panning.
//! Uses CPAL's native and Web Audio backends through one shared mixer.

/// Combat / expansion sound category for procedural synthesis.
#[derive(Clone, Copy, Debug)]
pub enum CombatSoundKind {
    WildernessExpansion,
    AttackHuman,
    AttackEmpire,
    AttackTribe,
    CounterAttack,
}

/// Player archetype for death-sound synthesis.
#[derive(Clone, Copy, Debug)]
pub enum PlayerSoundType {
    Human,
    Nation,
    Bot,
}

/// Structure type for placement-sound synthesis.
#[derive(Clone, Copy, Debug)]
pub enum BuildingSoundKind {
    City,
    Bunker,
    Factory,
    Port,
}

/// World position and camera state for spatial audio panning/attenuation.
#[derive(Clone, Copy, Debug)]
pub struct SpatialSoundParams {
    pub wx: f32,
    pub wy: f32,
    pub camera_x: f32,
    pub camera_y: f32,
    pub camera_zoom: f32,
    pub screen_w: f32,
    pub screen_h: f32,
}

pub fn play_death_sound(player_type: PlayerSoundType, seed: u32, spatial: SpatialSoundParams) {
    native::play_death_sound(player_type, seed, spatial);
}

pub fn play_deploy_sound(spatial: SpatialSoundParams) {
    native::play_deploy_sound(spatial);
}

pub fn play_combat_sound(
    kind: CombatSoundKind,
    troops: f32,
    seed: u32,
    spatial: SpatialSoundParams,
) {
    native::play_combat_sound(kind, troops, seed, spatial);
}

pub fn play_building_placement_sound(kind: BuildingSoundKind, spatial: SpatialSoundParams) {
    native::play_building_placement_sound(kind, spatial);
}

pub fn play_building_completed_sound(kind: BuildingSoundKind, spatial: SpatialSoundParams) {
    native::play_building_completed_sound(kind, spatial);
}

pub fn play_nuke_launch_sound(spatial: SpatialSoundParams) {
    native::play_nuke_launch_sound(spatial);
}

pub fn play_nuke_impact_sound(level: u8, spatial: SpatialSoundParams) {
    native::play_nuke_impact_sound(level, spatial);
}

pub fn play_bunker_defense_sound(seed: u32, spatial: SpatialSoundParams) {
    native::play_bunker_defense_sound(seed, spatial);
}

pub fn set_music_context(seed: u32, anchor_wx: f32, anchor_wy: f32) {
    native::set_music_context(seed, anchor_wx, anchor_wy);
}

pub fn play_victory_sound() {
    native::play_victory_sound();
}

pub fn play_defeat_sound() {
    native::play_defeat_sound();
}

pub fn set_master_volume(volume: f32) {
    native::set_master_volume(volume);
}

mod native;

pub use native::play_spatial;
