use blade_graphics as gpu;
use bytemuck::{Pod, Zeroable};

const DEFAULT_EMOJI_REFERENCE_DIAMETER: f32 = 32.0;
const MIN_EMOJI_OUTLINE_SCALE: f32 = 0.4;

fn default_emoji_reference_diameter() -> f32 {
    DEFAULT_EMOJI_REFERENCE_DIAMETER
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct TextGlobals {
    pub screen_size: [f32; 2],
    pub _pad: [f32; 2],
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

    /// Reduce emoji effects with the content while preserving a readable minimum.
    #[inline]
    pub fn scaled_for_emoji(self, content_diameter: f32) -> Self {
        if self.reference_diameter <= 0.0 {
            return self;
        }
        let scale = (content_diameter.max(0.0) / self.reference_diameter)
            .clamp(MIN_EMOJI_OUTLINE_SCALE, 1.0);
        Self {
            thickness: self.thickness * scale,
            shadow_y: self.shadow_y * scale,
            ..self
        }
    }

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
pub const KIND_EMOJI: f32 = 1.0;
pub const KIND_DISC: f32 = 2.0;
pub const KIND_RING: f32 = 3.0;
pub const KIND_SPRITE: f32 = 4.0;
pub const KIND_RECT: f32 = 5.0;
pub const KIND_TRIANGLE: f32 = 6.0;
pub const KIND_CROSS: f32 = 7.0;
pub const KIND_ARC: f32 = 8.0;
pub const KIND_ROUNDED_RECT: f32 = 10.0;
pub const KIND_BUILDING_SPRITE: f32 = 11.0;
pub const KIND_STATUS_SPRITE: f32 = 12.0;
pub const AVATAR_CORNER_RADIUS_RATIO: f32 = 0.14;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameplateStatusSprite {
    Request,
    Allied,
    Traitor,
}

impl NameplateStatusSprite {
    pub const fn uv_rect(self) -> [f32; 4] {
        const CELL_WIDTH: f32 = 1.0 / 3.0;
        let column = match self {
            Self::Request => 0.0,
            Self::Allied => 1.0,
            Self::Traitor => 2.0,
        };
        [column * CELL_WIDTH, 0.0, (column + 1.0) * CELL_WIDTH, 1.0]
    }
}

// 64 portrait slots cover current leaders and every distinct geo-entity portrait, with room to grow.
pub const AVATAR_CELL: u32 = 256;
pub const AVATAR_COLS: u32 = 8;
pub const AVATAR_ROWS: u32 = 8;
pub const AVATAR_SLOT_COUNT: usize = (AVATAR_COLS * AVATAR_ROWS) as usize;

pub fn avatar_slot_uv(slot: usize) -> [f32; 4] {
    let i = slot as u32;
    let col = i % AVATAR_COLS;
    let row = i / AVATAR_COLS;
    let aw = (AVATAR_COLS * AVATAR_CELL) as f32;
    let ah = (AVATAR_ROWS * AVATAR_CELL) as f32;
    let u0 = (col * AVATAR_CELL) as f32 / aw;
    let v0 = (row * AVATAR_CELL) as f32 / ah;
    [
        u0,
        v0,
        u0 + AVATAR_CELL as f32 / aw,
        v0 + AVATAR_CELL as f32 / ah,
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
    pub emoji_atlas: gpu::TextureView,
    pub emoji_sampler: gpu::Sampler,
    pub avatar_atlas: gpu::TextureView,
    pub avatar_sampler: gpu::Sampler,
    pub building_atlas: gpu::TextureView,
    pub building_sampler: gpu::Sampler,
    pub status_atlas: gpu::TextureView,
    pub status_sampler: gpu::Sampler,
}
