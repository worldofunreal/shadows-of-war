//! Shared catalog for the live shader controls in Dev Tools.

pub const PARAM_CAPACITY: usize = 512;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParamKind {
    Number,
    Bool,
    Color,
    Vector,
    Select,
}

#[derive(Clone, Copy, Debug)]
pub struct ParamOption {
    pub label: &'static str,
    pub value: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct ParamDef {
    pub id: &'static str,
    pub label: &'static str,
    pub section: &'static str,
    pub kind: ParamKind,
    pub default: [f32; 4],
    pub min: f32,
    pub max: f32,
    pub step: f32,
    pub unit: &'static str,
    /// True when the fourth color channel is a live opacity, not a fixed 1.0.
    pub alpha: bool,
    /// Names a live WGSL slot. Slots are assigned from this catalog in order.
    pub shader_token: Option<&'static str>,
    pub options: &'static [ParamOption],
}

const NO_OPTIONS: &[ParamOption] = &[];
const BLEND_OPTIONS: &[ParamOption] = &[
    ParamOption {
        label: "Normal mix",
        value: 0.0,
    },
    ParamOption {
        label: "Multiply",
        value: 1.0,
    },
    ParamOption {
        label: "Overlay",
        value: 2.0,
    },
    ParamOption {
        label: "Albedo only",
        value: 3.0,
    },
];

macro_rules! number {
    ($id:literal, $label:literal, $section:literal, $default:expr, $min:expr, $max:expr, $step:expr, $unit:literal, $token:literal) => {
        ParamDef {
            id: $id,
            label: $label,
            section: $section,
            kind: ParamKind::Number,
            default: [$default, 0.0, 0.0, 0.0],
            min: $min,
            max: $max,
            step: $step,
            unit: $unit,
            alpha: false,
            shader_token: Some($token),
            options: NO_OPTIONS,
        }
    };
}
macro_rules! color {
    ($id:literal, $label:literal, $section:literal, [$r:expr, $g:expr, $b:expr], $token:literal) => {
        ParamDef {
            id: $id,
            label: $label,
            section: $section,
            kind: ParamKind::Color,
            default: [$r, $g, $b, 1.0],
            min: 0.0,
            max: 1.0,
            step: 0.004,
            unit: "sRGB",
            alpha: false,
            shader_token: Some($token),
            options: NO_OPTIONS,
        }
    };
}
macro_rules! linear_color {
    ($id:literal, $label:literal, $section:literal, [$r:expr, $g:expr, $b:expr], $token:literal) => {
        ParamDef {
            id: $id,
            label: $label,
            section: $section,
            kind: ParamKind::Color,
            default: [$r, $g, $b, 1.0],
            min: 0.0,
            max: 4.0,
            step: 0.004,
            unit: "linear RGB",
            alpha: false,
            shader_token: Some($token),
            options: NO_OPTIONS,
        }
    };
}
macro_rules! linear_rgba {
    ($id:literal, $label:literal, $section:literal, [$r:expr, $g:expr, $b:expr, $a:expr], $token:literal) => {
        ParamDef {
            id: $id,
            label: $label,
            section: $section,
            kind: ParamKind::Color,
            default: [$r, $g, $b, $a],
            min: 0.0,
            max: 4.0,
            step: 0.004,
            unit: "linear RGB",
            alpha: true,
            shader_token: Some($token),
            options: NO_OPTIONS,
        }
    };
}
macro_rules! vector {
    ($id:literal, $label:literal, $section:literal, [$x:expr, $y:expr, $z:expr], $min:expr, $max:expr, $step:expr, $unit:literal, $token:literal) => {
        ParamDef {
            id: $id,
            label: $label,
            section: $section,
            kind: ParamKind::Vector,
            default: [$x, $y, $z, 0.0],
            min: $min,
            max: $max,
            step: $step,
            unit: $unit,
            alpha: false,
            shader_token: Some($token),
            options: NO_OPTIONS,
        }
    };
}
macro_rules! flag {
    ($id:literal, $label:literal, $section:literal, $default:expr, $token:literal) => {
        ParamDef {
            id: $id,
            label: $label,
            section: $section,
            kind: ParamKind::Bool,
            default: [$default, 0.0, 0.0, 0.0],
            min: 0.0,
            max: 1.0,
            step: 1.0,
            unit: "",
            alpha: false,
            shader_token: Some($token),
            options: NO_OPTIONS,
        }
    };
}
macro_rules! legacy_number {
    ($id:literal, $label:literal, $section:literal, $default:expr, $min:expr, $max:expr, $step:expr, $unit:literal) => {
        ParamDef {
            id: $id,
            label: $label,
            section: $section,
            kind: ParamKind::Number,
            default: [$default, 0.0, 0.0, 0.0],
            min: $min,
            max: $max,
            step: $step,
            unit: $unit,
            alpha: false,
            shader_token: None,
            options: NO_OPTIONS,
        }
    };
}
macro_rules! legacy_bool {
    ($id:literal, $label:literal, $section:literal, $default:expr) => {
        ParamDef {
            id: $id,
            label: $label,
            section: $section,
            kind: ParamKind::Bool,
            default: [if $default { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0],
            min: 0.0,
            max: 1.0,
            step: 1.0,
            unit: "",
            alpha: false,
            shader_token: None,
            options: NO_OPTIONS,
        }
    };
}
macro_rules! selection {
    ($id:literal, $label:literal, $section:literal, $default:expr, $options:expr) => {
        ParamDef {
            id: $id,
            label: $label,
            section: $section,
            kind: ParamKind::Select,
            default: [$default, 0.0, 0.0, 0.0],
            min: 0.0,
            max: 3.0,
            step: 1.0,
            unit: "",
            alpha: false,
            shader_token: None,
            options: $options,
        }
    };
}

pub static PARAMS: &[ParamDef] = &[
    // Existing live controls are represented here too, so the panel and state
    // publisher share one list instead of separately maintained field tables.
    legacy_number!(
        "thickness",
        "Border width",
        "Borders",
        0.5,
        0.0,
        1.0,
        0.01,
        "cells"
    ),
    legacy_number!(
        "darkness",
        "Border brightness",
        "Borders",
        0.35,
        0.0,
        1.0,
        0.01,
        ""
    ),
    legacy_number!(
        "shore_thickness",
        "Land shore width",
        "Coast",
        0.35,
        0.0,
        1.0,
        0.01,
        "cells"
    ),
    legacy_number!(
        "shore_darkness",
        "Land shore brightness",
        "Coast",
        1.0,
        0.0,
        1.0,
        0.01,
        ""
    ),
    legacy_number!(
        "conquest_duration",
        "Conquest duration",
        "Conquest",
        1.5,
        0.1,
        10.0,
        0.1,
        "s"
    ),
    legacy_number!(
        "territory_opacity",
        "Territory opacity",
        "Territories",
        1.0,
        0.0,
        1.0,
        0.01,
        ""
    ),
    selection!(
        "blend_mode",
        "Territory blend",
        "Territories",
        0.0,
        BLEND_OPTIONS
    ),
    legacy_number!(
        "font_size_scale",
        "Text size",
        "Text",
        2.0,
        0.5,
        2.5,
        0.01,
        "×"
    ),
    legacy_number!(
        "font_face_dilate",
        "Glyph fill",
        "Text",
        -0.2,
        -1.0,
        2.0,
        0.01,
        ""
    ),
    legacy_number!(
        "font_outline_thickness",
        "Glyph outline",
        "Text",
        1.4,
        0.0,
        3.0,
        0.01,
        "px"
    ),
    legacy_number!(
        "font_shadow_y",
        "Glyph shadow offset",
        "Text",
        2.0,
        0.0,
        5.0,
        0.01,
        "px"
    ),
    legacy_number!(
        "font_underlay_softness",
        "Glyph shadow softness",
        "Text",
        0.1,
        0.0,
        2.0,
        0.01,
        ""
    ),
    legacy_number!(
        "font_char_spacing",
        "Glyph spacing",
        "Text",
        0.95,
        0.8,
        1.8,
        0.01,
        "×"
    ),
    legacy_bool!("vfx_conquer", "Conquest effect", "Conquest", true),
    legacy_bool!("vfx_border_breathe", "Border breathing", "Borders", true),
    legacy_bool!(
        "vfx_dialog_border",
        "Terms dialog border",
        "Dialog highlight",
        true
    ),
    legacy_bool!(
        "vfx_energy_flow",
        "Contested border shimmer",
        "Borders",
        true
    ),
    legacy_bool!("vfx_heartbeat", "Territory pulse", "Territories", true),
    legacy_bool!("fog_of_war", "Visibility shading", "Fog of war", false),
    legacy_bool!("vfx_ambient_grade", "Warm color cycle", "Lighting", true),
    legacy_bool!(
        "vfx_screen_vignette",
        "Screen vignette",
        "Screen atmosphere",
        true
    ),
    flag!(
        "vfx.alerts",
        "Screen alerts",
        "Screen alerts",
        1.0,
        "MAP_ALERTS_ENABLED"
    ),
    legacy_bool!(
        "vfx_holo_grid",
        "Placement grid glow",
        "Placement grid",
        true
    ),
    legacy_bool!("vfx_movers", "Mover sprites", "Movers", true),
    legacy_bool!(
        "vfx_tutorial_highlight",
        "Tutorial target highlight",
        "Tutorial highlight",
        true
    ),
    legacy_bool!("vfx_mover_trails", "Mover trails", "Movers", true),
    legacy_bool!("vfx_click_markers", "Click markers", "World text", true),
    legacy_bool!(
        "vfx_attack_badges",
        "Attack troop badges",
        "World text",
        true
    ),
    legacy_bool!("vfx_world_buildings", "World buildings", "World text", true),
    legacy_bool!("vfx_bot_avatars", "Bot avatars", "World text", true),
    legacy_bool!("vfx_nameplate_names", "Nameplate names", "World text", true),
    legacy_bool!(
        "vfx_nameplate_troops",
        "Nameplate troops",
        "World text",
        true
    ),
    // Ocean palette is evaluated per frame from the depth byte, not baked at map upload.
    color!(
        "water.ocean.shore",
        "Shore water",
        "Ocean water",
        [100.0 / 255.0, 143.0 / 255.0, 255.0 / 255.0],
        "MAP_WATER_SHORE"
    ),
    color!(
        "water.ocean.shallow",
        "Shallow water",
        "Ocean water",
        [78.0 / 255.0, 151.0 / 255.0, 208.0 / 255.0],
        "MAP_WATER_SHALLOW"
    ),
    color!(
        "water.ocean.shelf",
        "Coastal shelf",
        "Ocean water",
        [63.0 / 255.0, 132.0 / 255.0, 188.0 / 255.0],
        "MAP_WATER_SHELF"
    ),
    color!(
        "water.ocean.mid",
        "Mid water",
        "Ocean water",
        [52.0 / 255.0, 115.0 / 255.0, 170.0 / 255.0],
        "MAP_WATER_MID"
    ),
    color!(
        "water.ocean.deep",
        "Deep water",
        "Ocean water",
        [43.0 / 255.0, 97.0 / 255.0, 151.0 / 255.0],
        "MAP_WATER_DEEP"
    ),
    color!(
        "water.ocean.abyss",
        "Deep ocean",
        "Ocean water",
        [34.0 / 255.0, 80.0 / 255.0, 132.0 / 255.0],
        "MAP_WATER_ABYSS"
    ),
    color!(
        "water.inland.color",
        "Lakes and rivers",
        "Inland water",
        [52.0 / 255.0, 160.0 / 255.0, 196.0 / 255.0],
        "MAP_WATER_INLAND"
    ),
    number!(
        "water.inland.strength",
        "Inland water strength",
        "Inland water",
        1.0,
        0.0,
        2.0,
        0.01,
        "×",
        "MAP_WATER_INLAND_STRENGTH"
    ),
    number!(
        "water.inland.brightness",
        "Inland water brightness",
        "Inland water",
        1.0,
        0.0,
        2.0,
        0.01,
        "×",
        "MAP_WATER_INLAND_BRIGHTNESS"
    ),
    number!(
        "water.depth_cap",
        "Depth color limit",
        "Ocean water",
        11.0,
        1.0,
        31.0,
        1.0,
        "depth steps",
        "MAP_WATER_DEPTH_CAP"
    ),
    number!(
        "water.depth_scale",
        "Depth scale",
        "Ocean water",
        2.0,
        0.0,
        4.0,
        0.01,
        "×",
        "MAP_WATER_DEPTH_SCALE"
    ),
    number!(
        "water.shallow_limit",
        "Shallow band end",
        "Ocean water",
        2.0,
        0.0,
        31.0,
        1.0,
        "depth steps",
        "MAP_WATER_SHALLOW_LIMIT"
    ),
    number!(
        "water.shelf_limit",
        "Shelf band end",
        "Ocean water",
        3.0,
        0.0,
        31.0,
        1.0,
        "depth steps",
        "MAP_WATER_SHELF_LIMIT"
    ),
    number!(
        "water.mid_limit",
        "Mid band end",
        "Ocean water",
        12.0,
        0.0,
        31.0,
        1.0,
        "depth steps",
        "MAP_WATER_MID_LIMIT"
    ),
    number!(
        "water.deep_limit",
        "Deep band end",
        "Ocean water",
        20.0,
        0.0,
        31.0,
        1.0,
        "depth steps",
        "MAP_WATER_DEEP_LIMIT"
    ),
    number!(
        "water.transition",
        "Band transition width",
        "Ocean water",
        2.70,
        0.0,
        8.0,
        0.05,
        "depth steps",
        "MAP_WATER_TRANSITION"
    ),
    number!(
        "water.shore_band_width",
        "Ocean edge width",
        "Ocean water",
        1.0,
        0.0,
        8.0,
        0.1,
        "cells",
        "MAP_WATER_SHORE_WIDTH"
    ),
    number!(
        "water.shore_fade_width",
        "Ocean edge softness",
        "Ocean water",
        0.25,
        0.0,
        2.0,
        0.05,
        "cells",
        "MAP_WATER_SHORE_FADE"
    ),
    number!(
        "water.shore_strength",
        "Ocean edge tint strength",
        "Ocean water",
        0.0,
        0.0,
        1.0,
        0.01,
        "×",
        "MAP_WATER_SHORE_STRENGTH"
    ),
    number!(
        "water.palette_strength",
        "Palette strength",
        "Ocean water",
        1.0,
        0.0,
        2.0,
        0.01,
        "×",
        "MAP_WATER_STRENGTH"
    ),
    number!(
        "water.brightness",
        "Water brightness",
        "Ocean water",
        1.0,
        0.0,
        2.0,
        0.01,
        "×",
        "MAP_WATER_BRIGHTNESS"
    ),
    color!(
        "land.shore.color",
        "Shore land",
        "Terrain",
        [204.0 / 255.0, 203.0 / 255.0, 158.0 / 255.0],
        "MAP_LAND_SHORE"
    ),
    color!(
        "land.plains.color",
        "Plains",
        "Terrain",
        [190.0 / 255.0, 220.0 / 255.0, 138.0 / 255.0],
        "MAP_LAND_PLAINS"
    ),
    color!(
        "land.highlands.color",
        "Highlands at elevation 0",
        "Terrain",
        [200.0 / 255.0, 183.0 / 255.0, 138.0 / 255.0],
        "MAP_LAND_HIGHLANDS"
    ),
    color!(
        "land.mountains.color",
        "Mountain base",
        "Terrain",
        [240.0 / 255.0, 223.0 / 255.0, 178.0 / 255.0],
        "MAP_LAND_MOUNTAINS"
    ),
    color!(
        "land.snow.color",
        "Snow peak",
        "Terrain",
        [248.0 / 255.0, 248.0 / 255.0, 248.0 / 255.0],
        "MAP_LAND_SNOW"
    ),
    number!(
        "land.plains.limit",
        "Plains end elevation",
        "Terrain",
        10.0,
        1.0,
        31.0,
        1.0,
        "height",
        "MAP_LAND_PLAINS_LIMIT"
    ),
    number!(
        "land.highlands.limit",
        "Highlands end elevation",
        "Terrain",
        20.0,
        1.0,
        31.0,
        1.0,
        "height",
        "MAP_LAND_HIGHLANDS_LIMIT"
    ),
    number!(
        "land.snow.start",
        "Snow blend starts",
        "Terrain",
        20.0,
        0.0,
        31.0,
        1.0,
        "height",
        "MAP_LAND_SNOW_START"
    ),
    number!(
        "land.snow.blend_range",
        "Snow blend range",
        "Terrain",
        11.0,
        0.1,
        31.0,
        0.1,
        "height",
        "MAP_LAND_SNOW_RANGE"
    ),
    number!(
        "land.plains.elevation_tint",
        "Plains elevation tint",
        "Terrain",
        2.0 / 255.0,
        -0.05,
        0.05,
        0.0001,
        "per height",
        "MAP_LAND_PLAINS_SLOPE"
    ),
    number!(
        "land.highlands.elevation_tint",
        "Highlands elevation tint",
        "Terrain",
        2.0 / 255.0,
        -0.05,
        0.05,
        0.0001,
        "per height",
        "MAP_LAND_HIGHLANDS_SLOPE"
    ),
    number!(
        "land.noise_strength",
        "Cell color variation",
        "Terrain",
        0.03,
        0.0,
        0.2,
        0.001,
        "×",
        "MAP_LAND_NOISE"
    ),
    number!(
        "land.shore_noise_scale",
        "Shore variation scale",
        "Terrain",
        0.5,
        0.0,
        2.0,
        0.01,
        "×",
        "MAP_LAND_SHORE_NOISE"
    ),
    number!(
        "land.highlands_noise_scale",
        "Highlands variation scale",
        "Terrain",
        1.2,
        0.0,
        3.0,
        0.01,
        "×",
        "MAP_LAND_HIGHLANDS_NOISE"
    ),
    number!(
        "land.mountains_noise_scale",
        "Mountain variation scale",
        "Terrain",
        0.8,
        0.0,
        3.0,
        0.01,
        "×",
        "MAP_LAND_MOUNTAINS_NOISE"
    ),
    number!(
        "land.relief_base",
        "Valley brightness",
        "Lighting",
        0.82,
        0.0,
        2.0,
        0.01,
        "×",
        "MAP_LAND_RELIEF_BASE"
    ),
    flag!(
        "lighting.relief.enabled",
        "Terrain relief",
        "Lighting",
        1.0,
        "MAP_RELIEF_ENABLED"
    ),
    number!(
        "land.relief_gain",
        "Peak brightness gain",
        "Lighting",
        0.28,
        0.0,
        2.0,
        0.01,
        "×",
        "MAP_LAND_RELIEF_GAIN"
    ),
    number!(
        "land.normal_scale",
        "Relief normal scale",
        "Lighting",
        0.1,
        0.0,
        2.0,
        0.01,
        "×",
        "MAP_LAND_NORMAL_SCALE"
    ),
    linear_color!(
        "map.backdrop.color",
        "Outside map",
        "Map",
        [0.015, 0.015, 0.02],
        "MAP_BACKDROP"
    ),
    linear_color!(
        "land.unclaimed.haze_color",
        "Unclaimed land haze",
        "Terrain",
        [0.55, 0.58, 0.68],
        "MAP_LAND_HAZE_COLOR"
    ),
    number!(
        "land.unclaimed.haze_strength",
        "Haze base",
        "Terrain",
        0.08,
        0.0,
        1.0,
        0.01,
        "",
        "MAP_LAND_HAZE_BASE"
    ),
    number!(
        "land.unclaimed.haze_wave",
        "Haze variation",
        "Terrain",
        0.04,
        0.0,
        1.0,
        0.01,
        "",
        "MAP_LAND_HAZE_WAVE"
    ),
    number!(
        "land.unclaimed.haze_scale_x",
        "Haze X scale",
        "Terrain",
        0.05,
        0.0,
        1.0,
        0.001,
        "",
        "MAP_LAND_HAZE_SCALE_X"
    ),
    number!(
        "land.unclaimed.haze_scale_y",
        "Haze Y scale",
        "Terrain",
        0.07,
        0.0,
        1.0,
        0.001,
        "",
        "MAP_LAND_HAZE_SCALE_Y"
    ),
    number!(
        "territory.local.fill",
        "Local territory shade",
        "Territories",
        0.72,
        0.0,
        2.0,
        0.01,
        "×",
        "MAP_LOCAL_FILL"
    ),
    number!(
        "territory.local.border",
        "Local border brightness",
        "Territories",
        1.0,
        0.0,
        2.0,
        0.01,
        "×",
        "MAP_LOCAL_BORDER"
    ),
    flag!(
        "territory.skin.stripe.enabled",
        "Stripes enabled",
        "Skin patterns",
        1.0,
        "MAP_SKIN_STRIPE_ENABLED"
    ),
    number!(
        "territory.skin.stripe.scale",
        "Stripe scale",
        "Skin patterns",
        0.22,
        0.0,
        2.0,
        0.001,
        "",
        "MAP_SKIN_STRIPE_SCALE"
    ),
    vector!(
        "territory.skin.stripe.direction",
        "Stripe direction",
        "Skin patterns",
        [1.0, 1.0, 0.0],
        -4.0,
        4.0,
        0.01,
        "vector",
        "MAP_SKIN_STRIPE_DIRECTION"
    ),
    number!(
        "territory.skin.stripe.width",
        "Stripe width",
        "Skin patterns",
        0.08,
        0.001,
        0.5,
        0.001,
        "",
        "MAP_SKIN_STRIPE_WIDTH"
    ),
    number!(
        "territory.skin.stripe.strength",
        "Stripe strength",
        "Skin patterns",
        0.7,
        0.0,
        1.0,
        0.01,
        "",
        "MAP_SKIN_STRIPE_STRENGTH"
    ),
    linear_color!(
        "territory.skin.stripe.tint",
        "Stripe tint",
        "Skin patterns",
        [1.45, 0.78, 0.38],
        "MAP_SKIN_STRIPE_TINT"
    ),
    flag!(
        "territory.skin.grid.enabled",
        "Grid enabled",
        "Skin patterns",
        1.0,
        "MAP_SKIN_GRID_ENABLED"
    ),
    number!(
        "territory.skin.grid.scale",
        "Grid scale",
        "Skin patterns",
        0.20,
        0.0,
        2.0,
        0.001,
        "",
        "MAP_SKIN_GRID_SCALE"
    ),
    number!(
        "territory.skin.grid.width",
        "Grid line width",
        "Skin patterns",
        0.07,
        0.001,
        0.5,
        0.001,
        "",
        "MAP_SKIN_GRID_WIDTH"
    ),
    number!(
        "territory.skin.grid.strength",
        "Grid strength",
        "Skin patterns",
        0.24,
        0.0,
        1.0,
        0.01,
        "",
        "MAP_SKIN_GRID_STRENGTH"
    ),
    linear_color!(
        "territory.skin.grid.tint",
        "Grid tint",
        "Skin patterns",
        [0.45, 0.85, 0.95],
        "MAP_SKIN_GRID_TINT"
    ),
    flag!(
        "territory.skin.diamond.enabled",
        "Diamonds enabled",
        "Skin patterns",
        1.0,
        "MAP_SKIN_DIAMOND_ENABLED"
    ),
    number!(
        "territory.skin.diamond.scale",
        "Diamond scale",
        "Skin patterns",
        0.16,
        0.0,
        2.0,
        0.001,
        "",
        "MAP_SKIN_DIAMOND_SCALE"
    ),
    number!(
        "territory.skin.diamond.width",
        "Diamond line width",
        "Skin patterns",
        0.08,
        0.001,
        0.5,
        0.001,
        "",
        "MAP_SKIN_DIAMOND_WIDTH"
    ),
    number!(
        "territory.skin.diamond.offset",
        "Diamond spacing",
        "Skin patterns",
        0.20,
        0.0,
        0.5,
        0.001,
        "",
        "MAP_SKIN_DIAMOND_OFFSET"
    ),
    number!(
        "territory.skin.diamond.strength",
        "Diamond strength",
        "Skin patterns",
        0.28,
        0.0,
        1.0,
        0.01,
        "",
        "MAP_SKIN_DIAMOND_STRENGTH"
    ),
    linear_color!(
        "territory.skin.diamond.tint",
        "Diamond tint",
        "Skin patterns",
        [1.0, 0.78, 0.30],
        "MAP_SKIN_DIAMOND_TINT"
    ),
    number!(
        "border.edge_softness",
        "Border edge softness",
        "Borders",
        0.04,
        0.001,
        0.3,
        0.001,
        "cells",
        "MAP_BORDER_SOFTNESS"
    ),
    number!(
        "border.tribe_zoom_cutoff",
        "Hide tribe borders below zoom",
        "Borders",
        0.6,
        0.0,
        4.0,
        0.01,
        "×",
        "MAP_TRIBE_ZOOM_CUTOFF"
    ),
    linear_color!(
        "border.tribe.color",
        "Tribe border",
        "Borders",
        [0.2, 0.8, 0.2],
        "MAP_BORDER_TRIBE_COLOR"
    ),
    linear_color!(
        "border.neutral.color",
        "Neutral border",
        "Borders",
        [0.025, 0.020, 0.015],
        "MAP_BORDER_NEUTRAL_COLOR"
    ),
    linear_color!(
        "coast.color",
        "Coast tint",
        "Coast",
        [0.55, 0.50, 0.32],
        "MAP_COAST_COLOR"
    ),
    linear_color!(
        "coast.shoreline_color",
        "Shore land tint",
        "Coast",
        [0.72, 0.68, 0.42],
        "MAP_COAST_SHORELINE_COLOR"
    ),
    number!(
        "coast.edge_softness",
        "Coast edge softness",
        "Coast",
        0.04,
        0.001,
        0.3,
        0.001,
        "cells",
        "MAP_COAST_SOFTNESS"
    ),
    number!(
        "coast.tint_strength",
        "Coast tint strength",
        "Coast",
        0.65,
        0.0,
        1.0,
        0.01,
        "",
        "MAP_COAST_STRENGTH"
    ),
    number!(
        "border.breathe.rate",
        "Border breathing speed",
        "Borders",
        3.0,
        0.0,
        20.0,
        0.05,
        "rad/s",
        "MAP_BORDER_BREATHE_RATE"
    ),
    number!(
        "border.breathe.amount",
        "Border breathing width",
        "Borders",
        0.05,
        0.0,
        0.5,
        0.005,
        "cells",
        "MAP_BORDER_BREATHE_AMOUNT"
    ),
    number!(
        "border.conquest.width",
        "Conquest border expansion",
        "Conquest",
        0.2,
        0.0,
        1.0,
        0.01,
        "cells",
        "MAP_BORDER_CONQUEST_WIDTH"
    ),
    number!(
        "border.conquest.color_gain",
        "Conquest border color gain",
        "Conquest",
        1.35,
        0.0,
        3.0,
        0.01,
        "×",
        "MAP_BORDER_CONQUEST_COLOR_GAIN"
    ),
    number!(
        "border.conquest.blend",
        "Conquest border blend",
        "Conquest",
        0.5,
        0.0,
        1.0,
        0.01,
        "",
        "MAP_BORDER_CONQUEST_BLEND"
    ),
    number!(
        "conquest.interior_flash",
        "Interior flash intensity",
        "Conquest",
        0.8,
        0.0,
        2.0,
        0.01,
        "×",
        "MAP_CONQUEST_INTERIOR_FLASH"
    ),
    linear_color!(
        "conquest.mountain_color",
        "Mountain conquest flash (Hannibal)",
        "Conquest",
        [0.737, 0.376, 0.019],
        "MAP_CONQUEST_MOUNTAIN_COLOR"
    ),
    flag!(
        "vfx.attack_flash",
        "Attack flash",
        "Borders",
        1.0,
        "MAP_ATTACK_FLASH_ENABLED"
    ),
    number!(
        "attack.flash.duration",
        "Attack flash duration",
        "Borders",
        1.0,
        0.0,
        10.0,
        0.05,
        "s",
        "MAP_ATTACK_FLASH_DURATION"
    ),
    number!(
        "attack.flash.base",
        "Attack flash base",
        "Borders",
        1.0,
        0.0,
        4.0,
        0.01,
        "×",
        "MAP_ATTACK_FLASH_BASE"
    ),
    number!(
        "attack.flash.per_attacker",
        "Added per attacker",
        "Borders",
        0.2,
        0.0,
        2.0,
        0.01,
        "×",
        "MAP_ATTACK_FLASH_PER_ATTACKER"
    ),
    number!(
        "attack.flash.max",
        "Attack flash maximum",
        "Borders",
        1.5,
        0.0,
        8.0,
        0.01,
        "×",
        "MAP_ATTACK_FLASH_MAX"
    ),
    number!(
        "border.contested.rate",
        "Contested shimmer speed",
        "Borders",
        2.5,
        0.0,
        20.0,
        0.05,
        "rad/s",
        "MAP_BORDER_CONTESTED_RATE"
    ),
    number!(
        "border.contested.strength",
        "Contested shimmer strength",
        "Borders",
        0.22,
        0.0,
        1.0,
        0.01,
        "",
        "MAP_BORDER_CONTESTED_STRENGTH"
    ),
    number!(
        "border.contested.brightness",
        "Contested shimmer brightness",
        "Borders",
        1.0,
        0.0,
        4.0,
        0.01,
        "×",
        "MAP_BORDER_CONTESTED_BRIGHTNESS"
    ),
    number!(
        "border.attack_flash.blend",
        "Attack flash blend",
        "Borders",
        0.85,
        0.0,
        1.0,
        0.01,
        "",
        "MAP_BORDER_ATTACK_BLEND"
    ),
    linear_color!(
        "border.attack_flash.color",
        "Attack flash color",
        "Borders",
        [1.0, 0.15, 0.1],
        "MAP_BORDER_ATTACK_COLOR"
    ),
    linear_color!(
        "border.tutorial.color",
        "Tutorial target color",
        "Tutorial highlight",
        [1.0, 0.72, 0.2],
        "MAP_BORDER_TUTORIAL_COLOR"
    ),
    number!(
        "border.tutorial.pulse_base",
        "Tutorial border base",
        "Tutorial highlight",
        0.52,
        0.0,
        1.0,
        0.01,
        "",
        "MAP_BORDER_TUTORIAL_BASE"
    ),
    number!(
        "border.tutorial.pulse_amount",
        "Tutorial border pulse",
        "Tutorial highlight",
        0.14,
        0.0,
        1.0,
        0.01,
        "",
        "MAP_BORDER_TUTORIAL_AMOUNT"
    ),
    number!(
        "border.tutorial.pulse_rate",
        "Tutorial border speed",
        "Tutorial highlight",
        3.0,
        0.0,
        20.0,
        0.05,
        "rad/s",
        "MAP_BORDER_TUTORIAL_RATE"
    ),
    number!(
        "border.tutorial.glow",
        "Tutorial border glow",
        "Tutorial highlight",
        1.0,
        0.0,
        2.0,
        0.01,
        "×",
        "MAP_BORDER_TUTORIAL_GLOW"
    ),
    number!(
        "territory.heartbeat.base",
        "Territory pulse base",
        "Territories",
        0.97,
        0.0,
        2.0,
        0.01,
        "×",
        "MAP_HEARTBEAT_BASE"
    ),
    number!(
        "territory.heartbeat.amount",
        "Territory pulse amount",
        "Territories",
        0.03,
        0.0,
        1.0,
        0.005,
        "×",
        "MAP_HEARTBEAT_AMOUNT"
    ),
    number!(
        "territory.heartbeat.rate",
        "Territory pulse speed",
        "Territories",
        1.8,
        0.0,
        20.0,
        0.05,
        "rad/s",
        "MAP_HEARTBEAT_RATE"
    ),
    number!(
        "territory.heartbeat.phase",
        "Player pulse phase",
        "Territories",
        2.3,
        0.0,
        20.0,
        0.05,
        "rad/player",
        "MAP_HEARTBEAT_PHASE"
    ),
    // Effects that are evaluated in the same map fragment shader.
    linear_color!(
        "dialog.color",
        "Dialog highlight color",
        "Dialog highlight",
        [1.0, 0.78, 0.22],
        "MAP_DIALOG_COLOR"
    ),
    number!(
        "dialog.interior.base",
        "Interior tint base",
        "Dialog highlight",
        0.05,
        0.0,
        1.0,
        0.01,
        "",
        "MAP_DIALOG_INTERIOR_BASE"
    ),
    number!(
        "dialog.interior.shimmer",
        "Interior shimmer",
        "Dialog highlight",
        0.05,
        0.0,
        1.0,
        0.01,
        "",
        "MAP_DIALOG_INTERIOR_SHIMMER"
    ),
    number!(
        "dialog.interior.intro",
        "Interior opening flash",
        "Dialog highlight",
        0.12,
        0.0,
        1.0,
        0.01,
        "",
        "MAP_DIALOG_INTERIOR_INTRO"
    ),
    number!(
        "dialog.interior.max",
        "Interior tint limit",
        "Dialog highlight",
        0.35,
        0.0,
        1.0,
        0.01,
        "",
        "MAP_DIALOG_INTERIOR_MAX"
    ),
    number!(
        "dialog.interior.spatial_rate",
        "Interior pattern scale",
        "Dialog highlight",
        1.8,
        0.0,
        20.0,
        0.05,
        "",
        "MAP_DIALOG_INTERIOR_SPATIAL"
    ),
    number!(
        "dialog.interior.time_rate",
        "Interior shimmer speed",
        "Dialog highlight",
        2.6,
        0.0,
        20.0,
        0.05,
        "rad/s",
        "MAP_DIALOG_INTERIOR_RATE"
    ),
    number!(
        "dialog.border.base_width",
        "Extra border width",
        "Dialog highlight",
        0.14,
        0.0,
        1.0,
        0.01,
        "cells",
        "MAP_DIALOG_BORDER_BASE"
    ),
    number!(
        "dialog.border.pulse_width",
        "Pulsing border width",
        "Dialog highlight",
        0.08,
        0.0,
        1.0,
        0.01,
        "cells",
        "MAP_DIALOG_BORDER_PULSE_WIDTH"
    ),
    number!(
        "dialog.border.pulse_rate",
        "Border breathing speed",
        "Dialog highlight",
        2.4,
        0.0,
        20.0,
        0.05,
        "rad/s",
        "MAP_DIALOG_BORDER_PULSE_RATE"
    ),
    number!(
        "dialog.border.blend",
        "Border glow blend",
        "Dialog highlight",
        0.95,
        0.0,
        1.0,
        0.01,
        "",
        "MAP_DIALOG_BORDER_BLEND"
    ),
    number!(
        "dialog.intro.duration",
        "Opening fade duration",
        "Dialog highlight",
        1.1,
        0.05,
        10.0,
        0.05,
        "s",
        "MAP_DIALOG_INTRO_DURATION"
    ),
    number!(
        "dialog.streak.spatial_rate",
        "Streak spacing",
        "Dialog highlight",
        2.4,
        0.0,
        20.0,
        0.05,
        "",
        "MAP_DIALOG_STREAK_SPATIAL"
    ),
    number!(
        "dialog.streak.time_rate",
        "Streak speed",
        "Dialog highlight",
        3.2,
        0.0,
        20.0,
        0.05,
        "rad/s",
        "MAP_DIALOG_STREAK_RATE"
    ),
    number!(
        "dialog.streak.low",
        "Streak threshold",
        "Dialog highlight",
        0.45,
        0.0,
        1.0,
        0.01,
        "",
        "MAP_DIALOG_STREAK_LOW"
    ),
    number!(
        "dialog.streak.high",
        "Streak softness",
        "Dialog highlight",
        0.95,
        0.0,
        1.0,
        0.01,
        "",
        "MAP_DIALOG_STREAK_HIGH"
    ),
    number!(
        "dialog.glint.rate",
        "Glint speed",
        "Dialog highlight",
        1.6,
        0.0,
        20.0,
        0.05,
        "rad/s",
        "MAP_DIALOG_GLINT_RATE"
    ),
    number!(
        "dialog.glint.rise.start",
        "Glint fade in",
        "Dialog highlight",
        0.80,
        0.0,
        1.0,
        0.01,
        "",
        "MAP_DIALOG_GLINT_RISE"
    ),
    number!(
        "dialog.glint.rise.end",
        "Glint peak",
        "Dialog highlight",
        0.90,
        0.0,
        1.0,
        0.01,
        "",
        "MAP_DIALOG_GLINT_PEAK"
    ),
    number!(
        "dialog.glint.fall.start",
        "Glint fade out",
        "Dialog highlight",
        0.95,
        0.0,
        1.0,
        0.01,
        "",
        "MAP_DIALOG_GLINT_FALL"
    ),
    number!(
        "dialog.glint.fall.end",
        "Glint end",
        "Dialog highlight",
        1.0,
        0.0,
        1.0,
        0.01,
        "",
        "MAP_DIALOG_GLINT_END"
    ),
    number!(
        "dialog.glow.breathe_rate",
        "Border breathing speed",
        "Dialog highlight",
        2.2,
        0.0,
        20.0,
        0.05,
        "rad/s",
        "MAP_DIALOG_GLOW_BREATHE_RATE"
    ),
    number!(
        "dialog.glint.strength",
        "Glint brightness",
        "Dialog highlight",
        0.5,
        0.0,
        2.0,
        0.01,
        "×",
        "MAP_DIALOG_GLINT_STRENGTH"
    ),
    number!(
        "dialog.glow.base",
        "Border glow base",
        "Dialog highlight",
        0.55,
        0.0,
        2.0,
        0.01,
        "×",
        "MAP_DIALOG_GLOW_BASE"
    ),
    number!(
        "dialog.glow.breathe",
        "Border glow breathing",
        "Dialog highlight",
        0.25,
        0.0,
        2.0,
        0.01,
        "×",
        "MAP_DIALOG_GLOW_BREATHE"
    ),
    number!(
        "dialog.glow.streak",
        "Border streak brightness",
        "Dialog highlight",
        0.65,
        0.0,
        2.0,
        0.01,
        "×",
        "MAP_DIALOG_GLOW_STREAK"
    ),
    number!(
        "dialog.glow.intro",
        "Border intro brightness",
        "Dialog highlight",
        0.90,
        0.0,
        2.0,
        0.01,
        "×",
        "MAP_DIALOG_GLOW_INTRO"
    ),
    linear_color!(
        "alliance.player.accent",
        "First participant",
        "Alliance celebration",
        [0.18, 0.78, 1.0],
        "MAP_ALLIANCE_PLAYER_COLOR"
    ),
    flag!(
        "vfx.alliance_celebration",
        "Alliance celebration",
        "Alliance celebration",
        1.0,
        "MAP_ALLIANCE_ENABLED"
    ),
    number!(
        "alliance.duration",
        "Celebration duration",
        "Alliance celebration",
        1.6,
        0.0,
        10.0,
        0.05,
        "s",
        "MAP_ALLIANCE_DURATION"
    ),
    linear_color!(
        "alliance.partner.accent",
        "Second participant",
        "Alliance celebration",
        [1.0, 0.70, 0.18],
        "MAP_ALLIANCE_PARTNER_COLOR"
    ),
    number!(
        "alliance.fade.start",
        "Fade starts",
        "Alliance celebration",
        0.72,
        0.0,
        1.0,
        0.01,
        "progress",
        "MAP_ALLIANCE_FADE_START"
    ),
    number!(
        "alliance.intro.duration",
        "Intro duration",
        "Alliance celebration",
        0.3,
        0.01,
        2.0,
        0.01,
        "progress",
        "MAP_ALLIANCE_INTRO"
    ),
    number!(
        "alliance.wave.speed",
        "Wave reach",
        "Alliance celebration",
        24.0,
        0.0,
        100.0,
        0.1,
        "cells/s",
        "MAP_ALLIANCE_WAVE_SPEED"
    ),
    number!(
        "alliance.wave.inner",
        "Wave inner width",
        "Alliance celebration",
        0.6,
        0.0,
        10.0,
        0.05,
        "cells",
        "MAP_ALLIANCE_WAVE_INNER"
    ),
    number!(
        "alliance.wave.outer",
        "Wave outer width",
        "Alliance celebration",
        2.2,
        0.0,
        10.0,
        0.05,
        "cells",
        "MAP_ALLIANCE_WAVE_OUTER"
    ),
    number!(
        "alliance.sparkle.rate",
        "Sparkle speed",
        "Alliance celebration",
        5.0,
        0.0,
        30.0,
        0.05,
        "Hz",
        "MAP_ALLIANCE_SPARKLE_RATE"
    ),
    number!(
        "alliance.sparkle.chance",
        "Sparkle density",
        "Alliance celebration",
        0.24,
        0.0,
        1.0,
        0.01,
        "",
        "MAP_ALLIANCE_SPARKLE_CHANCE"
    ),
    number!(
        "alliance.sparkle.low",
        "Sparkle fade in",
        "Alliance celebration",
        0.68,
        0.0,
        1.0,
        0.01,
        "",
        "MAP_ALLIANCE_SPARKLE_LOW"
    ),
    number!(
        "alliance.sparkle.peak",
        "Sparkle peak",
        "Alliance celebration",
        0.82,
        0.0,
        1.0,
        0.01,
        "",
        "MAP_ALLIANCE_SPARKLE_PEAK"
    ),
    number!(
        "alliance.sparkle.high",
        "Sparkle fade out",
        "Alliance celebration",
        0.90,
        0.0,
        1.0,
        0.01,
        "",
        "MAP_ALLIANCE_SPARKLE_HIGH"
    ),
    number!(
        "alliance.sparkle.end",
        "Sparkle end",
        "Alliance celebration",
        0.98,
        0.0,
        1.0,
        0.01,
        "",
        "MAP_ALLIANCE_SPARKLE_END"
    ),
    number!(
        "alliance.fill.base",
        "Fill base",
        "Alliance celebration",
        0.10,
        0.0,
        1.0,
        0.01,
        "",
        "MAP_ALLIANCE_FILL_BASE"
    ),
    number!(
        "alliance.fill.intro",
        "Intro fill",
        "Alliance celebration",
        0.28,
        0.0,
        1.0,
        0.01,
        "",
        "MAP_ALLIANCE_FILL_INTRO"
    ),
    number!(
        "alliance.fill.wave",
        "Wave fill",
        "Alliance celebration",
        0.48,
        0.0,
        1.0,
        0.01,
        "",
        "MAP_ALLIANCE_FILL_WAVE"
    ),
    number!(
        "alliance.fill.sparkle",
        "Sparkle fill",
        "Alliance celebration",
        0.42,
        0.0,
        1.0,
        0.01,
        "",
        "MAP_ALLIANCE_FILL_SPARKLE"
    ),
    number!(
        "alliance.fill.max",
        "Maximum fill",
        "Alliance celebration",
        0.78,
        0.0,
        1.0,
        0.01,
        "",
        "MAP_ALLIANCE_FILL_MAX"
    ),
    number!(
        "alliance.sparkle.add",
        "Sparkle light",
        "Alliance celebration",
        0.32,
        0.0,
        2.0,
        0.01,
        "×",
        "MAP_ALLIANCE_SPARKLE_ADD"
    ),
    number!(
        "alliance.border.base",
        "Border base sweep",
        "Alliance celebration",
        0.18,
        0.0,
        1.0,
        0.01,
        "",
        "MAP_ALLIANCE_BORDER_BASE"
    ),
    number!(
        "alliance.border.wave",
        "Border wave sweep",
        "Alliance celebration",
        0.82,
        0.0,
        1.0,
        0.01,
        "",
        "MAP_ALLIANCE_BORDER_WAVE"
    ),
    // Expansion, frontline combat haze, and attacks use independent visual controls.
    // Nuclear fallout color, spread, pulse and blast lighting.
    // Lighting, screen atmosphere, and build placement.
    vector!(
        "lighting.direction",
        "Light direction",
        "Lighting",
        [-1.0, -1.0, 1.6],
        -2.0,
        2.0,
        0.01,
        "",
        "MAP_LIGHT_DIRECTION"
    ),
    flag!(
        "lighting.diffuse.enabled",
        "Diffuse lighting",
        "Lighting",
        1.0,
        "MAP_DIFFUSE_ENABLED"
    ),
    number!(
        "lighting.diffuse_floor",
        "Minimum diffuse light",
        "Lighting",
        0.68,
        0.0,
        2.0,
        0.01,
        "×",
        "MAP_LIGHT_DIFFUSE_FLOOR"
    ),
    number!(
        "lighting.diffuse_gain",
        "Diffuse light gain",
        "Lighting",
        1.12,
        0.0,
        3.0,
        0.01,
        "×",
        "MAP_LIGHT_DIFFUSE_GAIN"
    ),
    flag!(
        "lighting.specular.enabled",
        "Specular highlights",
        "Lighting",
        0.0,
        "MAP_SPECULAR_ENABLED"
    ),
    number!(
        "lighting.specular.power",
        "Highlight concentration",
        "Lighting",
        96.0,
        1.0,
        256.0,
        1.0,
        "×",
        "MAP_SPECULAR_POWER"
    ),
    number!(
        "lighting.specular.strength",
        "Highlight strength",
        "Lighting",
        0.15,
        0.0,
        2.0,
        0.01,
        "×",
        "MAP_SPECULAR_STRENGTH"
    ),
    linear_color!(
        "lighting.specular.color",
        "Highlight color",
        "Lighting",
        [0.15, 0.15, 0.15],
        "MAP_SPECULAR_COLOR"
    ),
    number!(
        "lighting.warm_cycle.speed",
        "Warm cycle speed",
        "Lighting",
        0.08,
        0.0,
        1.0,
        0.001,
        "rad/s",
        "MAP_WARM_SPEED"
    ),
    number!(
        "lighting.warm_cycle.red_base",
        "Warm cycle red base",
        "Lighting",
        1.02,
        0.0,
        2.0,
        0.01,
        "×",
        "MAP_WARM_RED_BASE"
    ),
    number!(
        "lighting.warm_cycle.red_gain",
        "Warm cycle red range",
        "Lighting",
        0.03,
        0.0,
        1.0,
        0.005,
        "×",
        "MAP_WARM_RED_GAIN"
    ),
    number!(
        "lighting.warm_cycle.green_base",
        "Warm cycle green base",
        "Lighting",
        1.0,
        0.0,
        2.0,
        0.01,
        "×",
        "MAP_WARM_GREEN_BASE"
    ),
    number!(
        "lighting.warm_cycle.green_gain",
        "Warm cycle green range",
        "Lighting",
        0.01,
        0.0,
        1.0,
        0.005,
        "×",
        "MAP_WARM_GREEN_GAIN"
    ),
    number!(
        "lighting.warm_cycle.blue_base",
        "Warm cycle blue base",
        "Lighting",
        1.0,
        0.0,
        2.0,
        0.01,
        "×",
        "MAP_WARM_BLUE_BASE"
    ),
    number!(
        "lighting.warm_cycle.blue_gain",
        "Warm cycle blue range",
        "Lighting",
        -0.02,
        -1.0,
        1.0,
        0.005,
        "×",
        "MAP_WARM_BLUE_GAIN"
    ),
    number!(
        "screen.vignette.inner",
        "Vignette inner radius",
        "Screen atmosphere",
        0.45,
        0.0,
        2.0,
        0.01,
        "screen radius",
        "MAP_VIGNETTE_INNER"
    ),
    number!(
        "screen.vignette.outer",
        "Vignette outer radius",
        "Screen atmosphere",
        0.8,
        0.0,
        2.0,
        0.01,
        "screen radius",
        "MAP_VIGNETTE_OUTER"
    ),
    number!(
        "screen.vignette.center_gain",
        "Center brightness",
        "Screen atmosphere",
        1.0,
        0.0,
        2.0,
        0.01,
        "×",
        "MAP_VIGNETTE_CENTER_GAIN"
    ),
    number!(
        "screen.vignette.edge_gain",
        "Edge brightness",
        "Screen atmosphere",
        0.82,
        0.0,
        1.0,
        0.01,
        "×",
        "MAP_VIGNETTE_EDGE_GAIN"
    ),
    number!(
        "fog.visibility.gain",
        "Visibility brightness",
        "Fog of war",
        1.0,
        0.0,
        2.0,
        0.01,
        "×",
        "MAP_FOG_VISIBILITY_GAIN"
    ),
    number!(
        "fog.visibility.floor",
        "Unseen area brightness",
        "Fog of war",
        0.0,
        0.0,
        1.0,
        0.01,
        "×",
        "MAP_FOG_VISIBILITY_FLOOR"
    ),
    linear_color!(
        "alert.color",
        "Screen alert tint",
        "Screen alerts",
        [1.0, 1.0, 1.0],
        "MAP_ALERT_COLOR"
    ),
    number!(
        "alert.duration",
        "Alert duration",
        "Screen alerts",
        1.0,
        0.0,
        20.0,
        0.05,
        "s",
        "MAP_ALERT_DURATION"
    ),
    linear_rgba!(
        "alert.under_attack.color",
        "Under-attack color",
        "Screen alerts",
        [1.0, 0.05, 0.05, 0.55],
        "MAP_ALERT_UNDER_ATTACK"
    ),
    linear_rgba!(
        "alert.victory.color",
        "Victory color",
        "Screen alerts",
        [0.1, 0.9, 0.25, 0.45],
        "MAP_ALERT_VICTORY"
    ),
    linear_rgba!(
        "alert.defeat.color",
        "Defeat color",
        "Screen alerts",
        [0.25, 0.25, 0.3, 0.5],
        "MAP_ALERT_DEFEAT"
    ),
    linear_rgba!(
        "alert.conquer.color",
        "Conquest color",
        "Screen alerts",
        [0.95, 0.8, 0.0, 0.5],
        "MAP_ALERT_CONQUER"
    ),
    linear_rgba!(
        "alert.alliance.color",
        "Alliance request color",
        "Screen alerts",
        [0.0, 0.8, 0.8, 0.4],
        "MAP_ALERT_ALLIANCE"
    ),
    linear_rgba!(
        "alert.betrayal.color",
        "Betrayal color",
        "Screen alerts",
        [0.6, 0.0, 0.8, 0.65],
        "MAP_ALERT_BETRAYAL"
    ),
    number!(
        "alert.edge_width",
        "Alert edge width",
        "Screen alerts",
        0.08,
        0.0,
        0.5,
        0.001,
        "screen",
        "MAP_ALERT_EDGE_WIDTH"
    ),
    number!(
        "alert.edge_softness",
        "Alert edge softness",
        "Screen alerts",
        0.08,
        0.0,
        0.5,
        0.001,
        "screen",
        "MAP_ALERT_EDGE_SOFTNESS"
    ),
    number!(
        "alert.breathe.rate",
        "Alert pulse speed",
        "Screen alerts",
        4.5,
        0.0,
        20.0,
        0.05,
        "rad/s",
        "MAP_ALERT_BREATHE_RATE"
    ),
    number!(
        "alert.breathe.amount",
        "Alert pulse amount",
        "Screen alerts",
        0.15,
        0.0,
        1.0,
        0.01,
        "×",
        "MAP_ALERT_BREATHE_AMOUNT"
    ),
    number!(
        "alert.breathe.base",
        "Alert pulse base",
        "Screen alerts",
        0.85,
        0.0,
        2.0,
        0.01,
        "×",
        "MAP_ALERT_BREATHE_BASE"
    ),
    // Text, emoji, image, and shape effects. Instance colors remain the data source;
    // these controls multiply them so identity and semantic colors stay intact.
    number!(
        "text.sdf.fill_gain",
        "Glyph fill gain",
        "Text",
        1.0,
        0.0,
        3.0,
        0.01,
        "×",
        "TEXT_SDF_FILL_GAIN"
    ),
    number!(
        "text.sdf.outline_gain",
        "Glyph outline gain",
        "Text",
        1.0,
        0.0,
        3.0,
        0.01,
        "×",
        "TEXT_SDF_OUTLINE_GAIN"
    ),
    number!(
        "text.shadow.gain",
        "Glyph shadow gain",
        "Text",
        1.0,
        0.0,
        3.0,
        0.01,
        "×",
        "TEXT_SHADOW_GAIN"
    ),
    number!(
        "text.shadow.offset_scale",
        "Glyph shadow offset scale",
        "Text",
        1.0,
        0.0,
        4.0,
        0.01,
        "×",
        "TEXT_SHADOW_OFFSET_SCALE"
    ),
    number!(
        "text.shadow.softness_scale",
        "Glyph shadow softness scale",
        "Text",
        1.0,
        0.0,
        4.0,
        0.01,
        "×",
        "TEXT_SHADOW_SOFTNESS_SCALE"
    ),
    linear_color!(
        "text.fill.tint",
        "Text tint",
        "Text",
        [1.0, 1.0, 1.0],
        "TEXT_FILL_TINT"
    ),
    linear_color!(
        "text.outline.tint",
        "Text outline tint",
        "Text",
        [1.0, 1.0, 1.0],
        "TEXT_OUTLINE_TINT"
    ),
    number!(
        "text.opacity",
        "Text opacity",
        "Text",
        1.0,
        0.0,
        1.0,
        0.01,
        "",
        "TEXT_OPACITY"
    ),
    number!(
        "emoji.size_scale",
        "Emoji size",
        "Emoji",
        1.0,
        0.25,
        3.0,
        0.01,
        "×",
        "TEXT_EMOJI_SIZE"
    ),
    linear_color!(
        "emoji.outline_tint",
        "Emoji outline tint",
        "Emoji",
        [1.0, 1.0, 1.0],
        "TEXT_EMOJI_OUTLINE_TINT"
    ),
    number!(
        "emoji.outline_scale",
        "Emoji outline size",
        "Emoji",
        1.0,
        0.0,
        4.0,
        0.01,
        "×",
        "TEXT_EMOJI_OUTLINE_SCALE"
    ),
    number!(
        "emoji.shadow_scale",
        "Emoji shadow offset",
        "Emoji",
        1.0,
        0.0,
        4.0,
        0.01,
        "×",
        "TEXT_EMOJI_SHADOW_SCALE"
    ),
    number!(
        "emoji.minimum_outline",
        "Emoji minimum outline ratio",
        "Emoji",
        0.4,
        0.0,
        1.0,
        0.01,
        "×",
        "TEXT_EMOJI_MIN_OUTLINE"
    ),
    linear_color!(
        "emoji.tint",
        "Emoji tint",
        "Emoji",
        [1.0, 1.0, 1.0],
        "TEXT_EMOJI_TINT"
    ),
    number!(
        "emoji.opacity",
        "Emoji opacity",
        "Emoji",
        1.0,
        0.0,
        1.0,
        0.01,
        "",
        "TEXT_EMOJI_OPACITY"
    ),
    linear_color!(
        "avatar.tint",
        "Avatar tint",
        "Avatars",
        [1.0, 1.0, 1.0],
        "TEXT_AVATAR_TINT"
    ),
    number!(
        "avatar.size_scale",
        "Avatar size",
        "Avatars",
        1.0,
        0.25,
        4.0,
        0.01,
        "×",
        "TEXT_AVATAR_SIZE"
    ),
    number!(
        "avatar.opacity",
        "Avatar opacity",
        "Avatars",
        1.0,
        0.0,
        1.0,
        0.01,
        "",
        "TEXT_AVATAR_OPACITY"
    ),
    linear_color!(
        "building_icon.tint",
        "Building icon tint",
        "Icons",
        [1.0, 1.0, 1.0],
        "TEXT_BUILDING_TINT"
    ),
    number!(
        "building_icon.size_scale",
        "Building icon size",
        "Icons",
        1.0,
        0.25,
        4.0,
        0.01,
        "×",
        "TEXT_BUILDING_SIZE"
    ),
    number!(
        "building_icon.opacity",
        "Building icon opacity",
        "Icons",
        1.0,
        0.0,
        1.0,
        0.01,
        "",
        "TEXT_BUILDING_OPACITY"
    ),
    linear_color!(
        "status_icon.tint",
        "Status icon tint",
        "Icons",
        [1.0, 1.0, 1.0],
        "TEXT_STATUS_TINT"
    ),
    number!(
        "status_icon.size_scale",
        "Status icon size",
        "Icons",
        1.0,
        0.25,
        4.0,
        0.01,
        "×",
        "TEXT_STATUS_SIZE"
    ),
    number!(
        "status_icon.opacity",
        "Status icon opacity",
        "Icons",
        1.0,
        0.0,
        1.0,
        0.01,
        "",
        "TEXT_STATUS_OPACITY"
    ),
    number!(
        "shape.antialias_scale",
        "Shape edge smoothing",
        "Drawn shapes",
        1.0,
        0.1,
        4.0,
        0.01,
        "×",
        "TEXT_SHAPE_AA"
    ),
    number!(
        "shape.fill.opacity",
        "Shape fill opacity",
        "Drawn shapes",
        1.0,
        0.0,
        1.0,
        0.01,
        "",
        "TEXT_SHAPE_FILL_OPACITY"
    ),
    number!(
        "shape.stroke.scale",
        "Shape stroke width",
        "Drawn shapes",
        1.0,
        0.0,
        4.0,
        0.01,
        "×",
        "TEXT_SHAPE_STROKE"
    ),
    linear_color!(
        "shape.fill.tint",
        "Shape fill tint",
        "Drawn shapes",
        [1.0, 1.0, 1.0],
        "TEXT_SHAPE_FILL_TINT"
    ),
    linear_color!(
        "shape.stroke.tint",
        "Shape stroke tint",
        "Drawn shapes",
        [1.0, 1.0, 1.0],
        "TEXT_SHAPE_STROKE_TINT"
    ),
    number!(
        "shape.disc.scale",
        "Disc size",
        "Drawn shapes",
        1.0,
        0.25,
        3.0,
        0.01,
        "×",
        "TEXT_DISC_SCALE"
    ),
    number!(
        "shape.ring.width",
        "Ring width",
        "Drawn shapes",
        1.0,
        0.0,
        4.0,
        0.01,
        "×",
        "TEXT_RING_WIDTH"
    ),
    number!(
        "shape.arc.width",
        "Arc width",
        "Drawn shapes",
        1.0,
        0.0,
        4.0,
        0.01,
        "×",
        "TEXT_ARC_WIDTH"
    ),
    number!(
        "shape.arc.progress_aa",
        "Arc progress smoothing",
        "Drawn shapes",
        1.0,
        0.1,
        4.0,
        0.01,
        "×",
        "TEXT_ARC_PROGRESS_AA"
    ),
    number!(
        "shape.rect.opacity",
        "Rectangle opacity",
        "Drawn shapes",
        1.0,
        0.0,
        1.0,
        0.01,
        "",
        "TEXT_RECT_OPACITY"
    ),
    number!(
        "shape.rounded_rect.radius_scale",
        "Rounded rectangle corners",
        "Drawn shapes",
        1.0,
        0.0,
        2.0,
        0.01,
        "×",
        "TEXT_ROUNDED_RADIUS"
    ),
    number!(
        "shape.rounded_rect.stroke",
        "Rounded rectangle stroke",
        "Drawn shapes",
        1.0,
        0.0,
        4.0,
        0.01,
        "×",
        "TEXT_ROUNDED_STROKE"
    ),
    number!(
        "shape.triangle.opacity",
        "Triangle opacity",
        "Drawn shapes",
        1.0,
        0.0,
        1.0,
        0.01,
        "",
        "TEXT_TRIANGLE_OPACITY"
    ),
    number!(
        "shape.cross.width",
        "Cross width",
        "Drawn shapes",
        1.0,
        0.0,
        4.0,
        0.01,
        "×",
        "TEXT_CROSS_WIDTH"
    ),
    // Mover sprite and trail presentation. World data and movement remain untouched.
    number!(
        "mover.sprite.scale",
        "All mover sizes",
        "Movers",
        1.0,
        0.25,
        3.0,
        0.01,
        "×",
        "MOVER_SPRITE_SCALE"
    ),
    number!(
        "mover.transport.scale",
        "Transport ship size",
        "Movers",
        1.0,
        0.25,
        3.0,
        0.01,
        "×",
        "MOVER_TRANSPORT_SCALE"
    ),
    number!(
        "mover.trade_ship.scale",
        "Trade ship size",
        "Movers",
        1.0,
        0.25,
        3.0,
        0.01,
        "×",
        "MOVER_TRADE_SCALE"
    ),
    number!(
        "mover.warship.scale",
        "Warship size",
        "Movers",
        1.0,
        0.25,
        3.0,
        0.01,
        "×",
        "MOVER_WARSHIP_SCALE"
    ),
    number!(
        "mover.projectile.scale",
        "Projectile size",
        "Movers",
        1.0,
        0.25,
        3.0,
        0.01,
        "×",
        "MOVER_PROJECTILE_SCALE"
    ),
    number!(
        "mover.nuke.scale",
        "Nuclear projectile size",
        "Movers",
        1.0,
        0.25,
        3.0,
        0.01,
        "×",
        "MOVER_NUKE_SCALE"
    ),
    number!(
        "mover.sam.scale",
        "SAM projectile size",
        "Movers",
        1.0,
        0.25,
        3.0,
        0.01,
        "×",
        "MOVER_SAM_SCALE"
    ),
    number!(
        "mover.transport.minimum_px",
        "Minimum ship size",
        "Movers",
        12.0,
        1.0,
        64.0,
        0.5,
        "screen px",
        "MOVER_TRANSPORT_MIN_PX"
    ),
    number!(
        "mover.trade_ship.minimum_px",
        "Minimum trade ship size",
        "Movers",
        0.0,
        0.0,
        64.0,
        0.5,
        "screen px",
        "MOVER_TRADE_MIN_PX"
    ),
    number!(
        "mover.warship.minimum_px",
        "Minimum warship size",
        "Movers",
        0.0,
        0.0,
        64.0,
        0.5,
        "screen px",
        "MOVER_WARSHIP_MIN_PX"
    ),
    number!(
        "mover.nuke.minimum_px",
        "Minimum nuclear projectile size",
        "Movers",
        11.0,
        0.0,
        64.0,
        0.5,
        "screen px",
        "MOVER_NUKE_MIN_PX"
    ),
    number!(
        "mover.sam.minimum_px",
        "Minimum SAM size",
        "Movers",
        11.0,
        0.0,
        64.0,
        0.5,
        "screen px",
        "MOVER_SAM_MIN_PX"
    ),
    number!(
        "mover.height_scale",
        "Projectile height scaling",
        "Movers",
        0.5,
        0.0,
        3.0,
        0.01,
        "×",
        "MOVER_HEIGHT_SCALE"
    ),
    number!(
        "mover.height_limit",
        "Projectile height size limit",
        "Movers",
        2.0,
        1.0,
        5.0,
        0.01,
        "×",
        "MOVER_HEIGHT_LIMIT"
    ),
    number!(
        "mover.rotation_scale",
        "Sprite rotation",
        "Movers",
        1.0,
        0.0,
        2.0,
        0.01,
        "×",
        "MOVER_ROTATION_SCALE"
    ),
    linear_color!(
        "mover.sprite.tint",
        "Mover sprite tint",
        "Movers",
        [1.0, 1.0, 1.0],
        "MOVER_SPRITE_TINT"
    ),
    number!(
        "mover.sprite.opacity",
        "Mover opacity",
        "Movers",
        1.0,
        0.0,
        1.0,
        0.01,
        "",
        "MOVER_SPRITE_OPACITY"
    ),
    number!(
        "mover.trail.length",
        "Trail length",
        "Mover trails",
        1.0,
        0.0,
        1.0,
        0.01,
        "×",
        "MOVER_TRAIL_LENGTH"
    ),
    number!(
        "mover.trail.fade",
        "Trail tail fade",
        "Mover trails",
        1.0,
        0.0,
        2.0,
        0.01,
        "×",
        "MOVER_TRAIL_FADE"
    ),
    number!(
        "mover.trail.width",
        "Trail width",
        "Mover trails",
        1.0,
        0.0,
        4.0,
        0.01,
        "×",
        "MOVER_TRAIL_WIDTH"
    ),
    number!(
        "mover.trail.opacity",
        "Trail opacity",
        "Mover trails",
        1.0,
        0.0,
        1.0,
        0.01,
        "",
        "MOVER_TRAIL_OPACITY"
    ),
    linear_color!(
        "mover.trail.tint",
        "Trail tint",
        "Mover trails",
        [1.0, 1.0, 1.0],
        "MOVER_TRAIL_TINT"
    ),
    number!(
        "mover.trail.fleet_width",
        "Ship trail width at zoom 1",
        "Mover trails",
        0.4,
        0.0,
        8.0,
        0.01,
        "screen px",
        "MOVER_FLEET_TRAIL_WIDTH"
    ),
    number!(
        "mover.trail.fleet_min",
        "Ship trail minimum width",
        "Mover trails",
        1.0,
        0.0,
        20.0,
        0.1,
        "screen px",
        "MOVER_FLEET_TRAIL_MIN"
    ),
    number!(
        "mover.trail.fleet_max",
        "Ship trail maximum width",
        "Mover trails",
        6.0,
        0.0,
        40.0,
        0.1,
        "screen px",
        "MOVER_FLEET_TRAIL_MAX"
    ),
    number!(
        "mover.trail.projectile_zoom",
        "Projectile trail zoom gain",
        "Mover trails",
        0.5,
        0.0,
        5.0,
        0.01,
        "×",
        "MOVER_PROJECTILE_TRAIL_ZOOM"
    ),
    number!(
        "mover.trail.projectile_base",
        "Projectile trail base width",
        "Mover trails",
        2.0,
        0.0,
        20.0,
        0.1,
        "screen px",
        "MOVER_PROJECTILE_TRAIL_BASE"
    ),
    number!(
        "mover.trail.projectile_min",
        "Projectile trail minimum",
        "Mover trails",
        3.0,
        0.0,
        20.0,
        0.1,
        "screen px",
        "MOVER_PROJECTILE_TRAIL_MIN"
    ),
    number!(
        "mover.trail.projectile_max",
        "Projectile trail maximum",
        "Mover trails",
        10.0,
        0.0,
        40.0,
        0.1,
        "screen px",
        "MOVER_PROJECTILE_TRAIL_MAX"
    ),
    number!(
        "mover.nuke.arc_height",
        "Nuclear arc height",
        "Mover trails",
        50.0,
        0.0,
        200.0,
        0.5,
        "cells",
        "MOVER_NUKE_ARC_HEIGHT"
    ),
    number!(
        "mover.nuke.arc_peak",
        "Nuclear arc sprite lift",
        "Mover trails",
        4.0,
        0.0,
        30.0,
        0.1,
        "cells",
        "MOVER_NUKE_ARC_PEAK"
    ),
    number!(
        "mover.nuke.arc_samples",
        "Nuclear arc smoothness",
        "Mover trails",
        128.0,
        8.0,
        512.0,
        1.0,
        "segments",
        "MOVER_NUKE_ARC_SAMPLES"
    ),
    number!(
        "mover.nuke.curve_min_samples",
        "Nuclear curve minimum detail",
        "Mover trails",
        64.0,
        8.0,
        1024.0,
        1.0,
        "samples",
        "MOVER_NUKE_CURVE_MIN"
    ),
    number!(
        "mover.nuke.curve_max_samples",
        "Nuclear curve maximum detail",
        "Mover trails",
        4096.0,
        64.0,
        8192.0,
        1.0,
        "samples",
        "MOVER_NUKE_CURVE_MAX"
    ),
    number!(
        "mover.viewport_margin",
        "Mover draw margin",
        "Movers",
        64.0,
        0.0,
        512.0,
        1.0,
        "screen px",
        "MOVER_VIEWPORT_MARGIN"
    ),
];

pub fn definition(id: &str) -> Option<&'static ParamDef> {
    PARAMS.iter().find(|param| param.id == id)
}

/// Dev Tools tabs in display order. Every `PARAMS` section belongs to exactly one group.
pub const SECTION_GROUPS: &[(&str, &[&str])] = &[
    (
        "World",
        &[
            "Terrain",
            "Ocean water",
            "Inland water",
            "Coast",
            "Map",
            "Lighting",
            "Fog of war",
            "Screen atmosphere",
        ],
    ),
    (
        "Territory",
        &[
            "Territories",
            "Borders",
            "Skin patterns",
            "Tutorial highlight",
        ],
    ),
    ("Combat", &["Conquest"]),
    (
        "Effects",
        &["Alliance celebration", "Screen alerts", "Dialog highlight"],
    ),
    ("Units", &["Movers", "Mover trails", "Placement grid"]),
    (
        "Text & Icons",
        &[
            "World text",
            "Text",
            "Emoji",
            "Avatars",
            "Icons",
            "Drawn shapes",
        ],
    ),
];

/// Section that owns each effect preview, as `DevVfxPreview` target ids.
pub const SECTION_PREVIEWS: &[(&str, u8)] = &[
    ("Borders", 1),
    ("Conquest", 2),
    ("Alliance celebration", 3),
    ("Dialog highlight", 7),
    ("Screen alerts", 8),
];

/// One-tap switches for isolating frame cost. "On" restores each id's default;
/// "off" zeroes it (a bool goes false, a width goes to 0).
pub const PERF_SWITCHES: &[(&str, &[&str])] = &[
    (
        "Content",
        &[
            "vfx_movers",
            "vfx_mover_trails",
            "vfx_nameplate_names",
            "vfx_nameplate_troops",
            "vfx_bot_avatars",
            "vfx_world_buildings",
            "vfx_attack_badges",
            "vfx_click_markers",
        ],
    ),
    (
        "Effects",
        &[
            "vfx_conquer",
            "vfx_border_breathe",
            "vfx_energy_flow",
            "vfx_heartbeat",
            "vfx_ambient_grade",
            "vfx_screen_vignette",
            "vfx_holo_grid",
            "vfx_dialog_border",
            "vfx_tutorial_highlight",
            "vfx.alerts",
            "vfx.attack_flash",
            "vfx.alliance_celebration",
            "territory.skin.stripe.enabled",
            "territory.skin.grid.enabled",
            "territory.skin.diamond.enabled",
        ],
    ),
    ("Shore", &["water.shore_band_width"]),
    (
        "Lighting",
        &[
            "lighting.relief.enabled",
            "lighting.diffuse.enabled",
            "lighting.specular.enabled",
            "fog_of_war",
        ],
    ),
];

pub fn shader_slot(id: &str) -> Option<usize> {
    let target = definition(id)?.shader_token?;
    Some(
        PARAMS
            .iter()
            .take_while(|param| param.shader_token != Some(target))
            .filter(|param| param.shader_token.is_some())
            .count(),
    )
}

pub fn shader_param_count() -> usize {
    PARAMS
        .iter()
        .filter(|param| param.shader_token.is_some())
        .count()
}

pub fn shader_defaults() -> [[f32; 4]; PARAM_CAPACITY] {
    let mut values = [[0.0; 4]; PARAM_CAPACITY];
    let mut slot = 0;
    for param in PARAMS.iter().filter(|param| param.shader_token.is_some()) {
        values[slot] = param.default;
        slot += 1;
    }
    values
}

/// Resolve WGSL slot markers from the parameter catalog before shader compilation.
pub fn prepare_shader(shader_prefix: &str, source: &str) -> String {
    let mut prepared = source.to_owned();
    let mut slot = 0;
    for param in PARAMS.iter().filter(|param| param.shader_token.is_some()) {
        if param
            .shader_token
            .is_some_and(|token| token.starts_with(shader_prefix))
        {
            let token = format!("__{}__", param.shader_token.unwrap());
            prepared = prepared.replace(&token, &slot.to_string());
        }
        slot += 1;
    }
    prepared
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_shader_slots_fit_and_markers_resolve() {
        assert!(shader_param_count() <= PARAM_CAPACITY);
        for (i, param) in PARAMS.iter().enumerate() {
            assert!(
                !PARAMS[..i].iter().any(|other| other.id == param.id),
                "duplicate id {}",
                param.id
            );
            if let Some(token) = param.shader_token {
                assert!(
                    !PARAMS[..i]
                        .iter()
                        .any(|other| other.shader_token == Some(token)),
                    "duplicate shader token {token}"
                );
            }
        }
        for prefix in ["MAP_", "TEXT_", "MOVER_"] {
            let source = PARAMS
                .iter()
                .filter_map(|param| param.shader_token)
                .filter(|token| token.starts_with(prefix))
                .map(|token| format!("__{token}__"))
                .collect::<Vec<_>>()
                .join(" ");
            assert!(!prepare_shader(prefix, &source).contains("__"));
        }
    }

    #[test]
    fn every_section_belongs_to_exactly_one_dev_tools_group() {
        for param in PARAMS {
            let owners = SECTION_GROUPS
                .iter()
                .filter(|(_, sections)| sections.contains(&param.section))
                .count();
            assert_eq!(
                owners, 1,
                "section '{}' needs exactly one group",
                param.section
            );
        }
        for (_, sections) in SECTION_GROUPS {
            for section in *sections {
                assert!(
                    PARAMS.iter().any(|param| param.section == *section),
                    "group lists unknown section '{section}'"
                );
            }
        }
        for (section, _) in SECTION_PREVIEWS {
            assert!(PARAMS.iter().any(|param| param.section == *section));
        }
        for (name, ids) in PERF_SWITCHES {
            for id in *ids {
                assert!(
                    definition(id).is_some(),
                    "perf switch '{name}' lists unknown id '{id}'"
                );
            }
        }
    }

    /// Perf policy in `map.wgsl`: a per-pixel loop is either tiny (<= 4) or
    /// bounded by a CPU-supplied `globals.*_count`, so idle slots cost nothing.
    #[test]
    fn map_shader_loops_follow_perf_policy() {
        for line in include_str!("shaders/map.wgsl")
            .lines()
            .filter(|line| line.trim_start().starts_with("for (var"))
        {
            let bound = line.split("; ").nth(1).unwrap_or_default();
            let tiny = bound
                .split("< ")
                .nth(1)
                .and_then(|n| n.trim().parse::<u32>().ok())
                .is_some_and(|n| n <= 4);
            let counted = bound.contains("globals.") && bound.contains("_count");
            assert!(
                tiny || counted,
                "map.wgsl loop breaks the perf policy: {line}"
            );
        }
    }

    #[test]
    fn every_visual_control_reaches_a_shader_or_client_renderer() {
        let shader_sources = [
            ("MAP_", include_str!("shaders/map.wgsl")),
            ("TEXT_", include_str!("shaders/text_glow.wgsl")),
            ("MOVER_", include_str!("shaders/mover_sprites.wgsl")),
            ("MOVER_", include_str!("shaders/mover_trails.wgsl")),
        ];
        let client_sources = [
            include_str!("../../sow-client/src/render/frame/mod.rs"),
            include_str!("../../sow-client/src/render/world/movers.rs"),
        ];
        for param in PARAMS.iter().filter(|param| param.shader_token.is_some()) {
            let token = param.shader_token.unwrap();
            let marker = format!("__{token}__");
            let prefix = if token.starts_with("MAP_") {
                "MAP_"
            } else if token.starts_with("TEXT_") {
                "TEXT_"
            } else {
                "MOVER_"
            };
            assert!(
                shader_sources
                    .iter()
                    .any(|(shader_prefix, shader)| *shader_prefix == prefix
                        && shader.contains(&marker))
                    || client_sources
                        .iter()
                        .any(|source| source.contains(&format!("\"{}\"", param.id))),
                "visual control '{}' has no shader or client-render consumer",
                param.id,
            );
        }
    }
}
