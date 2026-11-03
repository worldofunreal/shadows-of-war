use blade_graphics as gpu;
use bytemuck::{Pod, Zeroable};

const DEFAULT_EMOJI_REFERENCE_DIAMETER: f32 = 32.0;

fn default_emoji_reference_diameter() -> f32 {
    DEFAULT_EMOJI_REFERENCE_DIAMETER
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct TextGlobals {
    pub screen_size: [f32; 2],
    pub _pad: [f32; 2],
    pub shader_params: [[f32; 4]; crate::shader_params::PARAM_CAPACITY],
}

/// One outline/shadow contract shared by glyph and RGBA-emoji instances.
#[derive(Clone, Copy, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct OutlineStyle {
    pub color: [f32; 4],
    pub thickness: f32,
    pub shadow_y: f32,
    /// Content diameter at which the configured outline keeps its full size.
    #[serde(default = "default_emoji_reference_diameter")]
    pub reference_diameter: f32,
}

impl OutlineStyle {
    pub const EMOJI_REFERENCE_DIAMETER: f32 = DEFAULT_EMOJI_REFERENCE_DIAMETER;

    pub const NONE: Self = Self {
        color: [0.0; 4],
        thickness: 0.0,
        shadow_y: 0.0,
        reference_diameter: 0.0,
    };

    pub const TACTICAL: Self = Self {
        color: [0.0, 0.0, 0.0, 0.9],
        thickness: 1.5,
        shadow_y: 1.5,
        reference_diameter: Self::EMOJI_REFERENCE_DIAMETER,
    };

    /// Screen-space room required outside the logical emoji bounds.
    #[inline]
    pub const fn effect_padding(self) -> f32 {
        let thickness = if self.thickness > 0.0 {
            self.thickness
        } else {
            0.0
        };
        let shadow_y = if self.shadow_y > 0.0 {
            self.shadow_y
        } else {
            0.0
        };
        if thickness > shadow_y {
            thickness
        } else {
            shadow_y
        }
    }
}

/// MSDF text settings plus the outline inherited by inline emojis.
#[derive(Clone, Copy, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct TextPaintStyle {
    pub face_dilate: f32,
    pub outline: OutlineStyle,
    pub underlay_softness: f32,
}

impl TextPaintStyle {
    #[inline]
    pub fn scaled(self, scale: f32) -> Self {
        Self {
            face_dilate: self.face_dilate * scale,
            outline: OutlineStyle {
                thickness: self.outline.thickness * scale,
                shadow_y: self.outline.shadow_y * scale,
                reference_diameter: self.outline.reference_diameter * scale,
                ..self.outline
            },
            underlay_softness: self.underlay_softness * scale,
        }
    }
}

impl Default for OutlineStyle {
    fn default() -> Self {
        Self {
            color: [0.0, 0.0, 0.0, 1.0],
            thickness: 0.8,
            shadow_y: 1.5,
            reference_diameter: Self::EMOJI_REFERENCE_DIAMETER,
        }
    }
}

impl Default for TextPaintStyle {
    fn default() -> Self {
        Self {
            face_dilate: 0.0,
            outline: OutlineStyle::default(),
            underlay_softness: 0.0,
        }
    }
}

pub const KIND_GLYPH: f32 = 0.0;
pub const KIND_DISC: f32 = 2.0;
pub const KIND_RING: f32 = 3.0;
pub const KIND_SPRITE: f32 = 4.0;
pub const KIND_RECT: f32 = 5.0;
pub const KIND_TRIANGLE: f32 = 6.0;
pub const KIND_CROSS: f32 = 7.0;
pub const KIND_ARC: f32 = 8.0;
pub const KIND_HORN: f32 = 9.0;
pub const KIND_ROUNDED_RECT: f32 = 10.0;
pub const KIND_BUILDING_SPRITE: f32 = 11.0;
pub const KIND_STATUS_SPRITE: f32 = 12.0;
/// Badge-atlas art drawn with the emoji outline and shadow.
pub const KIND_ICON: f32 = 13.0;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameplateStatusSprite {
    Request,
    Allied,
    Traitor,
    Rank1,
    Rank2,
    Rank3,
    You,
    Disconnected,
    UpgradeSparkle,
    Wilderness,
    Troops,
    Death,
    Radiation,
    Construction,
    Hammer,
    ArrowUp,
    ArrowRight,
    ArrowDown,
    ArrowLeft,
    Cancel,
    Confirm,
    Gold,
    Boat,
    Clock,
    Shield,
    Range,
    Speed,
}

impl NameplateStatusSprite {
    pub const ALL: [Self; 27] = [
        Self::Request,
        Self::Allied,
        Self::Traitor,
        Self::Rank1,
        Self::Rank2,
        Self::Rank3,
        Self::You,
        Self::Disconnected,
        Self::UpgradeSparkle,
        Self::Wilderness,
        Self::Troops,
        Self::Death,
        Self::Radiation,
        Self::Construction,
        Self::Hammer,
        Self::ArrowUp,
        Self::ArrowRight,
        Self::ArrowDown,
        Self::ArrowLeft,
        Self::Cancel,
        Self::Confirm,
        Self::Gold,
        Self::Boat,
        Self::Clock,
        Self::Shield,
        Self::Range,
        Self::Speed,
    ];

    /// File name in `assets/art/icons/` and cell name in the generated strip.
    pub const fn art_name(self) -> &'static str {
        match self {
            Self::Request => "request",
            Self::Allied => "allied",
            Self::Traitor => "traitor",
            Self::Rank1 => "rank-1",
            Self::Rank2 => "rank-2",
            Self::Rank3 => "rank-3",
            Self::You => "you",
            Self::Disconnected => "disconnected",
            Self::UpgradeSparkle => "upgrade-sparkle",
            Self::Wilderness => "wilderness",
            Self::Troops => "troops",
            Self::Death => "death",
            Self::Radiation => "radiation",
            Self::Construction => "construction",
            Self::Hammer => "hammer",
            Self::ArrowUp => "arrow-up",
            Self::ArrowRight => "arrow-right",
            Self::ArrowDown => "arrow-down",
            Self::ArrowLeft => "arrow-left",
            Self::Cancel => "cancel",
            Self::Confirm => "confirm",
            Self::Gold => "gold",
            Self::Boat => "boat",
            Self::Clock => "clock",
            Self::Shield => "shield",
            Self::Range => "range",
            Self::Speed => "speed",
        }
    }

    pub fn uv_rect(self) -> [f32; 4] {
        let cells = &super::badge_atlas::CELLS;
        let column = cells
            .iter()
            .position(|cell| *cell == self.art_name())
            .unwrap_or_default() as f32;
        let width = 1.0 / cells.len() as f32;
        [column * width, 0.0, (column + 1.0) * width, 1.0]
    }
}

#[cfg(test)]
mod badge_atlas_tests {
    use super::NameplateStatusSprite;

    #[test]
    fn every_badge_has_exactly_one_generated_cell() {
        let cells = &super::super::badge_atlas::CELLS;
        assert_eq!(cells.len(), NameplateStatusSprite::ALL.len());
        for sprite in NameplateStatusSprite::ALL {
            assert_eq!(
                cells
                    .iter()
                    .filter(|cell| **cell == sprite.art_name())
                    .count(),
                1,
                "{}",
                sprite.art_name()
            );
        }
    }
}

// One cell per portrait the match draws. The atlas grows by rows of `AVATAR_COLS` cells and
// never frees a cell during a match. 32 × 128 px keeps both sides within 4096 px for mobile
// GPUs while holding every catalog portrait at once.
pub const AVATAR_CELL: u32 = 128;
pub const AVATAR_COLS: u32 = 32;
pub const AVATAR_MAX_ROWS: u32 = 32;
pub const AVATAR_MAX_SLOTS: usize = (AVATAR_COLS * AVATAR_MAX_ROWS) as usize;

pub const AVATAR_FALLBACK_SLOT: usize = 0;
pub const AVATAR_GOLD_SLOT: usize = 1;

pub fn avatar_slot_uv(slot: usize, rows: u32) -> [f32; 4] {
    let i = slot as u32;
    let col = i % AVATAR_COLS;
    let row = i / AVATAR_COLS;
    let aw = (AVATAR_COLS * AVATAR_CELL) as f32;
    let ah = (rows * AVATAR_CELL) as f32;
    let u0 = ((col * AVATAR_CELL) as f32 + 0.5) / aw;
    let v0 = ((row * AVATAR_CELL) as f32 + 0.5) / ah;
    [
        u0,
        v0,
        u0 + (AVATAR_CELL - 1) as f32 / aw,
        v0 + (AVATAR_CELL - 1) as f32 / ah,
    ]
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, blade_macros::Vertex)]
pub struct TextInstanceGpu {
    pub screen_pos: [f32; 2],
    pub size: [f32; 2],
    pub uv_rect: [f32; 4],
    /// Normalized quad area containing the logical content inside the instance quad. Emoji use
    /// this to reserve transparent room for the outline/shadow; rings use it to reserve their
    /// antialias fringe. Other primitives use the full quad.
    pub content_rect: [f32; 4],
    pub color: [f32; 4],
    pub outline_color: [f32; 4],
    pub face_dilate: f32,
    pub outline_thickness: f32,
    pub underlay_offset_y: f32,
    pub underlay_softness: f32,
    pub kind: f32,
}

#[derive(blade_macros::ShaderData)]
pub struct TextShaderData {
    pub globals: TextGlobals,
    pub font_atlas: gpu::TextureView,
    pub font_sampler: gpu::Sampler,
    pub avatar_atlas: gpu::TextureView,
    pub avatar_sampler: gpu::Sampler,
    pub building_atlas: gpu::TextureView,
    pub building_sampler: gpu::Sampler,
    pub status_atlas: gpu::TextureView,
    pub status_sampler: gpu::Sampler,
}

#[cfg(test)]
mod avatar_atlas_tests {
    /// Cells are never freed during a match, so the full atlas must hold every catalog portrait
    /// at once: both fixed cells, four faces per hero and every bot. A catalog that outgrows it
    /// needs a smaller `AVATAR_CELL`, not eviction.
    #[test]
    fn full_avatar_atlas_holds_every_catalog_portrait_within_gpu_limits() {
        use sow_core::geo_entities::{Role, bots};
        assert!(super::AVATAR_COLS * super::AVATAR_CELL <= 4096);
        assert!(super::AVATAR_MAX_ROWS * super::AVATAR_CELL <= 4096);
        let catalog = 2
            + sow_core::player::Leader::count() * sow_core::emote::AvatarExpression::ALL.len()
            + bots(Role::Tribe).count()
            + bots(Role::Nation).count();
        assert!(catalog <= super::AVATAR_MAX_SLOTS, "{catalog} portraits");

        for rows in [1, 7, super::AVATAR_MAX_ROWS] {
            let expected_u_span = (super::AVATAR_CELL - 1) as f32
                / (super::AVATAR_COLS * super::AVATAR_CELL) as f32;
            let expected_v_span = (super::AVATAR_CELL - 1) as f32 / (rows * super::AVATAR_CELL) as f32;
            for slot in 0..(rows * super::AVATAR_COLS) as usize {
                let [u0, v0, u1, v1] = super::avatar_slot_uv(slot, rows);
                assert!(u0 >= 0.0 && v0 >= 0.0 && u1 <= 1.0 && v1 <= 1.0);
                assert!(u0 < u1 && v0 < v1);
                assert!((u1 - u0 - expected_u_span).abs() < 1e-6);
                assert!((v1 - v0 - expected_v_span).abs() < 1e-6);
            }
        }
    }
}
