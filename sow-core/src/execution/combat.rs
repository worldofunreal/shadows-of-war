#[path = "combat_fleets.rs"]
mod combat_fleets;
#[path = "combat_tiles.rs"]
mod combat_tiles;
pub(crate) use combat_tiles::{
    announce_attack_abilities, attack_strength, outnumbered_bonus_applies, targeted_multiplier,
};

