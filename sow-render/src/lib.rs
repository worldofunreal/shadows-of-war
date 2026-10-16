pub mod context;
pub mod map_renderer;
pub mod mover_renderer;
pub mod nameplate;
pub mod shader_params;
pub mod sprite_atlas;
pub mod text;

#[cfg(test)]
mod shader_validation;

pub use context::*;
pub use map_renderer::*;
pub use mover_renderer::*;
pub use sprite_atlas::*;
pub use text::*;

#[macro_export]
macro_rules! repo_asset_bytes {
    ($path:expr) => {
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../assets/", $path))
    };
}

pub static EMOJI_ATLAS_BYTES: &[u8] = repo_asset_bytes!("generated/emoji/atlas.webp");
pub static BUILDING_ATLAS_BYTES: &[u8] = repo_asset_bytes!("generated/buildings-atlas.webp");
pub static UNITS_ATLAS_BYTES: &[u8] = repo_asset_bytes!("generated/units-atlas.webp");
pub static NAMEPLATE_STATUS_ATLAS_BYTES: &[u8] = repo_asset_bytes!("generated/badges-atlas.webp");
