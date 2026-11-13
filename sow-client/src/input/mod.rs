pub(crate) mod intents;
pub(crate) mod map_click;
pub(crate) mod placement;
pub(crate) mod surface;
pub(crate) mod window;

pub use placement::{resolve_build_target_tile, resolve_building_placement_tile};
