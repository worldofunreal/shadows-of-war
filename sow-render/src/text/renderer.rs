use crate::context::RenderContext;
use crate::text::msdf::FontAtlas;
use crate::text::texture::FontAtlasTexture;
use crate::text::types::{
    AVATAR_CELL, AVATAR_COLS, AVATAR_CORNER_RADIUS_RATIO, AVATAR_ROWS, AVATAR_SLOT_COUNT, KIND_ARC,
    KIND_BUILDING_SPRITE, KIND_CROSS, KIND_DISC, KIND_EMOJI, KIND_GLYPH, KIND_RECT, KIND_RING,
    KIND_ROUNDED_RECT, KIND_SPRITE, KIND_TRIANGLE, OutlineStyle, TextGlobals, TextInstanceGpu,
    TextPaintStyle, TextShaderData, avatar_slot_uv,
};
use blade_graphics as gpu;

pub fn emoji_uv_opt(emoji: &str) -> Option<[f32; 4]> {
    sow_data::emoji::lookup(emoji).map(|r| {
        [
            r.x as f32 / sow_data::emoji::ATLAS_WIDTH as f32,
            r.y as f32 / sow_data::emoji::ATLAS_HEIGHT as f32,
            (r.x + r.w) as f32 / sow_data::emoji::ATLAS_WIDTH as f32,
            (r.y + r.h) as f32 / sow_data::emoji::ATLAS_HEIGHT as f32,
        ]
    })
}

pub fn building_sprite_uv(kind: sow_core::game::BuildingKind, level: u8) -> Option<[f32; 4]> {
    crate::text::building_atlas::uv_rect(kind, level)
}

pub const MAX_TEXT_GLYPHS: usize = 32_768;
const RING_AA_MARGIN: f32 = 1.5;
const CROSS_AA_MARGIN: f32 = 1.0;

#[inline]
fn clamp_arc_progress(progress: f32) -> f32 {
    if progress.is_finite() {
        progress.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

#[inline]
fn clamp_arc_phase(phase: f32) -> f32 {
    if phase.is_finite() {
        phase.rem_euclid(1.0)
    } else {
        0.0
    }
}

/// Layout bounds returned by the same atlas-aware measurement used by the GPU text path.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextMeasure {
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Copy, Debug)]
enum PreparedTextKind {
    Glyph,
    Emoji,
}

#[derive(Clone, Copy, Debug)]
struct PreparedTextItem {
    kind: PreparedTextKind,
    offset: [f32; 2],
    size: [f32; 2],
    uv_rect: [f32; 4],
}

/// Atlas-shaped text. Character spacing and alignment are resolved at creation;
/// drawing can only translate and uniformly scale the prepared geometry.
#[derive(Clone, Debug)]
pub struct PreparedText {
    items: Vec<PreparedTextItem>,
    advance_width: f32,
    align_x: f32,
    font_size: f32,
    line_height: f32,
    visual_bounds: Option<[f32; 4]>,
    complete: bool,
}

impl PreparedText {
    pub fn instance_count(&self) -> usize {
        if self.complete {
            self.items.len()
        } else {
            usize::MAX
        }
    }

    pub fn measure_at(&self, font_size: f32) -> TextMeasure {
        let scale = font_size.max(0.0) / self.font_size;
        TextMeasure {
            width: self.advance_width * scale,
            height: self.line_height * scale,
        }
    }

    /// Bounds of the exact glyph and inline-emoji quads emitted at this size,
    /// relative to the same anchor passed to `push_prepared_text`.
    pub fn visual_bounds_at(&self, font_size: f32) -> Option<[f32; 4]> {
        let scale = font_size.max(0.0) / self.font_size;
        self.visual_bounds
            .map(|bounds| bounds.map(|value| value * scale))
    }
}

impl Default for PreparedText {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            advance_width: 0.0,
            align_x: 0.0,
            font_size: 48.0,
            line_height: 0.0,
            visual_bounds: None,
            complete: true,
        }
    }
}

fn prepared_text_bounds(
    items: &[PreparedTextItem],
    advance_width: f32,
    align_x: f32,
) -> Option<[f32; 4]> {
    let scale = 1.0;
    let align_offset = advance_width * align_x * scale;
    let mut bounds = [
        f32::INFINITY,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NEG_INFINITY,
    ];
    for item in items {
        let x = item.offset[0] * scale - align_offset;
        let y = item.offset[1] * scale;
        let width = item.size[0] * scale;
        let height = item.size[1] * scale;
        bounds[0] = bounds[0].min(x);
        bounds[1] = bounds[1].min(y);
        bounds[2] = bounds[2].max(x + width);
        bounds[3] = bounds[3].max(y + height);
    }
    bounds[0].is_finite().then_some(bounds)
}

#[inline]
fn text_measure_height(font_size: f32, emoji_scale: f32, has_emoji: bool) -> f32 {
    if has_emoji {
        font_size.max(font_size * emoji_scale)
    } else {
        font_size
    }
}

fn emoji_geometry(
    screen_pos: [f32; 2],
    half_size: f32,
    outline: OutlineStyle,
) -> ([f32; 2], [f32; 2], [f32; 4]) {
    let content_half = half_size.max(0.0);
    let padding = outline.effect_padding();
    let quad_half = content_half + padding;
    let diameter = quad_half * 2.0;
    let content_min = if diameter > 0.0 {
        padding / diameter
    } else {
        0.0
    };
    (
        [screen_pos[0] - quad_half, screen_pos[1] - quad_half],
        [diameter; 2],
        [
            content_min,
            content_min,
            1.0 - content_min,
            1.0 - content_min,
        ],
    )
}

fn emoji_instance(
    uv_rect: [f32; 4],
    screen_pos: [f32; 2],
    half_size: f32,
    tint: [f32; 4],
    outline: OutlineStyle,
    underlay_softness: f32,
) -> TextInstanceGpu {
    let outline = outline.scaled_for_emoji(half_size.max(0.0) * 2.0);
    let (screen_pos, size, content_rect) = emoji_geometry(screen_pos, half_size, outline);
    TextInstanceGpu {
        screen_pos,
        size,
        uv_rect,
        content_rect,
        color: tint,
        outline_color: outline.color,
        face_dilate: 0.0,
        outline_thickness: outline.thickness,
        underlay_offset_y: outline.shadow_y,
        underlay_softness,
        kind: KIND_EMOJI,
    }
}

fn shape_text(
    atlas: &FontAtlas,
    text: &str,
    char_spacing: f32,
    emoji_scale: f32,
    mut emit: impl FnMut(PreparedTextItem),
) -> (f32, bool, usize) {
    let atlas_width = atlas.atlas.common.scale_w.max(1) as f32;
    let atlas_height = atlas.atlas.common.scale_h.max(1) as f32;
    let base = atlas.atlas.common.base as f32;
    let mut advance = 0.0f32;
    let mut has_emoji = false;
    let mut item_count = 0usize;
    let mut prev_char = Option::<char>::None;
    let mut chars = text.char_indices().peekable();

    while let Some((byte_idx, ch)) = chars.next() {
        if let Some(glyph) = atlas.char_map.get(&ch) {
            let kern = prev_char
                .and_then(|previous| atlas.kerning_map.get(&(previous, ch)))
                .copied()
                .unwrap_or(0) as f32;
            emit(PreparedTextItem {
                kind: PreparedTextKind::Glyph,
                offset: [
                    advance + glyph.xoffset as f32 + kern * char_spacing,
                    -base + glyph.yoffset as f32,
                ],
                size: [glyph.width as f32, glyph.height as f32],
                uv_rect: [
                    glyph.x as f32 / atlas_width,
                    glyph.y as f32 / atlas_height,
                    (glyph.x + glyph.width) as f32 / atlas_width,
                    (glyph.y + glyph.height) as f32 / atlas_height,
                ],
            });
            item_count = item_count.saturating_add(1);
            advance += (glyph.xadvance as f32 + kern) * char_spacing;
            prev_char = Some(ch);
            continue;
        }

        let has_selector = chars.peek().is_some_and(|&(_, next)| next == '\u{fe0f}');
        let character_bytes = ch.len_utf8();
        let text_bytes = if has_selector {
            character_bytes + '\u{fe0f}'.len_utf8()
        } else {
            character_bytes
        };
        let candidate = &text[byte_idx..byte_idx + text_bytes];
        let emoji = if has_selector {
            &text[byte_idx..byte_idx + character_bytes]
        } else {
            candidate
        };
        if let Some(uv_rect) = emoji_uv_opt(emoji) {
            let size = atlas.atlas.info.size as f32 * emoji_scale;
            emit(PreparedTextItem {
                kind: PreparedTextKind::Emoji,
                offset: [advance, -size],
                size: [size; 2],
                uv_rect,
            });
            item_count = item_count.saturating_add(1);
            advance += size * char_spacing;
            has_emoji = true;
            prev_char = None;
            if has_selector {
                chars.next();
            }
        } else {
            prev_char = None;
        }
    }
    (advance, has_emoji, item_count)
}

fn emit_prepared_text(
    instances: &mut Vec<TextInstanceGpu>,
    items: &[PreparedTextItem],
    advance_width: f32,
    align_x: f32,
    font_unit: f32,
    scale: f32,
    pos: [f32; 2],
    color: [f32; 4],
    settings: TextPaintStyle,
) {
    let font_scale = scale / font_unit;
    let align_offset = advance_width * align_x * font_scale;
    for item in items {
        let offset_x = item.offset[0] * font_scale - align_offset;
        let offset_y = item.offset[1] * font_scale;
        let size = [item.size[0] * font_scale, item.size[1] * font_scale];
        match item.kind {
            PreparedTextKind::Glyph => instances.push(TextInstanceGpu {
                screen_pos: [pos[0] + offset_x, pos[1] + offset_y],
                size,
                uv_rect: item.uv_rect,
                content_rect: [0.0, 0.0, 1.0, 1.0],
                color,
                outline_color: settings.outline.color,
                face_dilate: settings.face_dilate,
                outline_thickness: settings.outline.thickness,
                underlay_offset_y: settings.outline.shadow_y,
                underlay_softness: settings.underlay_softness,
                kind: KIND_GLYPH,
            }),
            PreparedTextKind::Emoji => {
                let half_size = size[0] * 0.5;
                let center = [pos[0] + offset_x + half_size, pos[1] + offset_y + half_size];
                instances.push(emoji_instance(
                    item.uv_rect,
                    center,
                    half_size,
                    color,
                    settings.outline,
                    settings.underlay_softness,
                ));
            }
        }
    }
}

fn emit_prepared_text_if_fits(
    instances: &mut Vec<TextInstanceGpu>,
    items: &[PreparedTextItem],
    advance_width: f32,
    align_x: f32,
    font_unit: f32,
    scale: f32,
    pos: [f32; 2],
    color: [f32; 4],
    settings: TextPaintStyle,
) -> bool {
    if instances.len().saturating_add(items.len()) > MAX_TEXT_GLYPHS {
        return false;
    }
    emit_prepared_text(
        instances,
        items,
        advance_width,
        align_x,
        font_unit,
        scale,
        pos,
        color,
        settings,
    );
    true
}

fn ring_geometry(center: [f32; 2], radius: f32) -> ([f32; 2], [f32; 2], [f32; 4]) {
    let radius = radius.max(0.0);
    let outer = radius + RING_AA_MARGIN;
    let content_min = RING_AA_MARGIN / (outer * 2.0);
    (
        [center[0] - outer, center[1] - outer],
        [outer * 2.0, outer * 2.0],
        [
            content_min,
            content_min,
            1.0 - content_min,
            1.0 - content_min,
        ],
    )
}

fn cross_geometry(center: [f32; 2], half_size: f32) -> ([f32; 2], [f32; 2], [f32; 4]) {
    let half_size = half_size.max(0.0);
    let outer = half_size + CROSS_AA_MARGIN;
    let content_min = CROSS_AA_MARGIN / (outer * 2.0);
    (
        [center[0] - outer, center[1] - outer],
        [outer * 2.0, outer * 2.0],
        [
            content_min,
            content_min,
            1.0 - content_min,
            1.0 - content_min,
        ],
    )
}

pub struct TextRenderer {
    pub font_atlas_desc: FontAtlas,
    pub font_atlas_tex: FontAtlasTexture,
    emoji_atlas_tex: FontAtlasTexture,
    avatar_atlas_tex: FontAtlasTexture,
    building_atlas_tex: FontAtlasTexture,
    avatar_loaded: [bool; AVATAR_SLOT_COUNT],
    avatar_dirty_slots: Vec<usize>,
    prepared_text_scratch: Vec<PreparedTextItem>,
    pipeline: gpu::RenderPipeline,
    buffer: gpu::Buffer,
    sampler: gpu::Sampler,
    emoji_sampler: gpu::Sampler,
    avatar_sampler: gpu::Sampler,
    building_sampler: gpu::Sampler,
    upload_instances: Vec<TextInstanceGpu>,
}

impl TextRenderer {
    pub fn new(context: &gpu::Context, surface_format: gpu::TextureFormat) -> Self {
        let font_atlas_desc = FontAtlas::load_static();
        let font_atlas_tex = FontAtlasTexture::new(context);
        let emoji_atlas_tex = FontAtlasTexture::from_bytes(
            context,
            crate::EMOJI_ATLAS_BYTES,
            "emoji_atlas",
            gpu::TextureFormat::Rgba8UnormSrgb, // ponytail: hardware sRGB decode
        );
        let avatar_atlas_tex = FontAtlasTexture::blank(
            context,
            AVATAR_COLS * AVATAR_CELL,
            AVATAR_ROWS * AVATAR_CELL,
            "avatar_atlas",
            gpu::TextureFormat::Rgba8UnormSrgb, // portraits are sRGB color, like emoji
        );
        let building_atlas_tex = FontAtlasTexture::from_bytes(
            context,
            crate::BUILDING_ATLAS_BYTES,
            "building_atlas",
            gpu::TextureFormat::Rgba8UnormSrgb,
        );
        let shader_source = include_str!("../shaders/text_glow.wgsl");
        let shader = context.create_shader(gpu::ShaderDesc {
            source: shader_source,
            naga_module: None,
        });
        assert_eq!(
            std::mem::size_of::<TextGlobals>(),
            shader.get_struct_size("TextGlobals") as usize,
        );

        let text_layout = <TextShaderData as gpu::ShaderData>::layout();
        let text_vertex_layout = <TextInstanceGpu as gpu::Vertex>::layout();
        let blend = gpu::BlendState {
            color: gpu::BlendComponent {
                src_factor: gpu::BlendFactor::SrcAlpha,
                dst_factor: gpu::BlendFactor::OneMinusSrcAlpha,
                operation: gpu::BlendOperation::Add,
            },
            alpha: gpu::BlendComponent::OVER,
        };

        // Fragment entry contract: sRGB swapchains (native) use `fs_main`; plain
        // UNORM (wasm WebGL canvas) uses `fs_main_srgb` to encode linear->sRGB
        // manually, so text/emoji aren't darker than native. Same as map.wgsl.
        let fragment_entry = if matches!(surface_format, gpu::TextureFormat::Rgba8Unorm) {
            "fs_main_srgb"
        } else {
            "fs_main"
        };

        let pipeline = context.create_render_pipeline(gpu::RenderPipelineDesc {
            name: "text_glow_pipeline",
            data_layouts: &[&text_layout],
            vertex: shader.at("vs_main"),
            vertex_fetches: &[gpu::VertexFetchState {
                layout: &text_vertex_layout,
                instanced: true,
            }],
            primitive: gpu::PrimitiveState::default(),
            depth_stencil: None,
            fragment: Some(shader.at(fragment_entry)),
            color_targets: &[gpu::ColorTargetState {
                format: surface_format,
                blend: Some(blend),
                write_mask: gpu::ColorWrites::default(),
            }],
            multisample_state: gpu::MultisampleState::default(),
        });

        let buffer = context.create_buffer(gpu::BufferDesc {
            name: "text_glow_instances",
            size: (MAX_TEXT_GLYPHS * std::mem::size_of::<TextInstanceGpu>()) as u64,
            memory: gpu::Memory::Upload,
        });

        let sampler = context.create_sampler(gpu::SamplerDesc {
            name: "font_atlas_sampler",
            mag_filter: gpu::FilterMode::Linear,
            min_filter: gpu::FilterMode::Linear,
            ..Default::default()
        });

        let emoji_sampler = context.create_sampler(gpu::SamplerDesc {
            name: "emoji_atlas_sampler",
            mag_filter: gpu::FilterMode::Linear,
            min_filter: gpu::FilterMode::Linear,
            ..Default::default()
        });

        let avatar_sampler = context.create_sampler(gpu::SamplerDesc {
            name: "avatar_atlas_sampler",
            mag_filter: gpu::FilterMode::Linear,
            min_filter: gpu::FilterMode::Linear,
            ..Default::default()
        });

        let building_sampler = context.create_sampler(gpu::SamplerDesc {
            name: "building_atlas_pixel_sampler",
            mag_filter: gpu::FilterMode::Nearest,
            min_filter: gpu::FilterMode::Nearest,
            ..Default::default()
        });

        Self {
            font_atlas_desc,
            font_atlas_tex,
            emoji_atlas_tex,
            avatar_atlas_tex,
            building_atlas_tex,
            avatar_loaded: [false; AVATAR_SLOT_COUNT],
            avatar_dirty_slots: Vec::with_capacity(AVATAR_SLOT_COUNT),
            prepared_text_scratch: Vec::with_capacity(32),
            pipeline,
            buffer,
            sampler,
            emoji_sampler,
            avatar_sampler,
            building_sampler,
            upload_instances: Vec::with_capacity(4096),
        }
    }

    /// Transition all atlas textures to a defined layout before first use. Must be called once
    /// (alongside `upload_atlas`) before any draw.
    pub fn init_textures(&self, encoder: &mut gpu::CommandEncoder) {
        encoder.init_texture(self.font_atlas_tex.texture);
        encoder.init_texture(self.emoji_atlas_tex.texture);
        encoder.init_texture(self.avatar_atlas_tex.texture);
        encoder.init_texture(self.building_atlas_tex.texture);
    }

    pub fn upload_atlas(&self, encoder: &mut gpu::CommandEncoder, context: &gpu::Context) {
        self.font_atlas_tex.upload(encoder, context);
        self.emoji_atlas_tex.upload(encoder, context);
        self.avatar_atlas_tex.upload(encoder, context);
        self.building_atlas_tex.upload(encoder, context);
    }

    pub fn begin_frame(&mut self) {
        self.upload_instances.clear();
    }

    pub fn push_glyph(&mut self, inst: TextInstanceGpu) {
        if self.upload_instances.len() < MAX_TEXT_GLYPHS {
            self.upload_instances.push(inst);
        }
    }

    /// Internal: push a glyph or emoji instance, respecting buffer limits.
    fn push_inst(&mut self, inst: TextInstanceGpu) {
        if self.upload_instances.len() < MAX_TEXT_GLYPHS {
            self.upload_instances.push(inst);
        }
    }

    pub fn prepare_string(
        &self,
        text: &str,
        char_spacing: f32,
        emoji_scale: f32,
        align_x: f32,
    ) -> PreparedText {
        let font_size = self.font_atlas_desc.atlas.info.size.max(1) as f32;
        let mut items = Vec::with_capacity(text.chars().count().min(MAX_TEXT_GLYPHS));
        let (advance_width, has_emoji, item_count) = shape_text(
            &self.font_atlas_desc,
            text,
            char_spacing,
            emoji_scale,
            |item| {
                if items.len() < MAX_TEXT_GLYPHS {
                    items.push(item);
                }
            },
        );
        let visual_bounds = prepared_text_bounds(&items, advance_width, align_x);
        PreparedText {
            items,
            advance_width,
            align_x,
            font_size,
            line_height: if text.is_empty() {
                0.0
            } else {
                text_measure_height(font_size, emoji_scale, has_emoji)
            },
            visual_bounds,
            complete: item_count <= MAX_TEXT_GLYPHS,
        }
    }

    /// Emit prepared text at one final physical font size; fitting is already resolved by caller.
    /// Returns false without emitting any instances if the frame budget is insufficient.
    pub fn push_prepared_text(
        &mut self,
        prepared: &PreparedText,
        pos: [f32; 2],
        final_font_size: f32,
        color: [f32; 4],
        settings: TextPaintStyle,
    ) -> bool {
        if !prepared.complete || !final_font_size.is_finite() || final_font_size <= 0.0 {
            return false;
        }
        emit_prepared_text_if_fits(
            &mut self.upload_instances,
            &prepared.items,
            prepared.advance_width,
            prepared.align_x,
            prepared.font_size,
            final_font_size,
            pos,
            color,
            settings,
        )
    }

    pub fn remaining_instance_capacity(&self) -> usize {
        MAX_TEXT_GLYPHS.saturating_sub(self.upload_instances.len())
    }

    pub fn instance_count(&self) -> usize {
        self.upload_instances.len()
    }

    pub fn push_string(
        &mut self,
        text: &str,
        pos: [f32; 2],
        font_size: f32,
        color: [f32; 4],
        settings: TextPaintStyle,
        layout: (f32, f32, f32),
    ) {
        let (align_x, char_spacing, emoji_scale) = layout;
        let font_unit = self.font_atlas_desc.atlas.info.size.max(1) as f32;
        let TextRenderer {
            font_atlas_desc,
            prepared_text_scratch,
            upload_instances,
            ..
        } = self;
        prepared_text_scratch.clear();
        let (advance_width, _, item_count) =
            shape_text(font_atlas_desc, text, char_spacing, emoji_scale, |item| {
                if prepared_text_scratch.len() < MAX_TEXT_GLYPHS {
                    prepared_text_scratch.push(item);
                }
            });
        if item_count > MAX_TEXT_GLYPHS {
            return;
        }
        let _ = emit_prepared_text_if_fits(
            upload_instances,
            prepared_text_scratch,
            advance_width,
            align_x,
            font_unit,
            font_size.max(0.0),
            pos,
            color,
            settings,
        );
    }

    /// Measure `text` in the same units as `font_size`, using the exact advance math
    /// `push_string` emits. The height expands for an emoji only when the string actually
    /// contains an atlas emoji, so ASCII names do not reserve emoji-sized vertical space.
    pub fn measure_string(
        &self,
        text: &str,
        font_size: f32,
        char_spacing: f32,
        emoji_scale: f32,
    ) -> TextMeasure {
        let (advance_width, has_emoji, _) = shape_text(
            &self.font_atlas_desc,
            text,
            char_spacing,
            emoji_scale,
            |_| {},
        );
        let font_unit = self.font_atlas_desc.atlas.info.size.max(1) as f32;
        let scale = font_size / font_unit;
        TextMeasure {
            width: advance_width * scale,
            height: if text.is_empty() {
                0.0
            } else {
                text_measure_height(font_size, emoji_scale, has_emoji)
            },
        }
    }

    /// Push a screen-space emoji with alpha-dilated outline + drop shadow.
    /// `screen_pos` is the center in physical pixels, `half_size` the logical content half-extent;
    /// the submitted quad grows only by the size-adjusted effect padding.
    /// Returns `false` if the emoji isn't in the atlas.
    pub fn push_emoji(
        &mut self,
        emoji: &str,
        screen_pos: [f32; 2],
        half_size: f32,
        tint: [f32; 4],
        outline: OutlineStyle,
    ) -> bool {
        let Some(emoji_uv) = emoji_uv_opt(emoji) else {
            return false;
        };
        self.push_inst(emoji_instance(
            emoji_uv, screen_pos, half_size, tint, outline, 0.0,
        ));
        true
    }

    /// Push an emoji with standard tactical outline (90% black alpha, 1.5px thickness, 1.5px shadow).
    pub fn push_emoji_tactical(
        &mut self,
        emoji: &str,
        screen_pos: [f32; 2],
        half_size: f32,
    ) -> bool {
        self.push_emoji(
            emoji,
            screen_pos,
            half_size,
            [1.0, 1.0, 1.0, 1.0],
            OutlineStyle::TACTICAL,
        )
    }

    /// Push a filled, anti-aliased disc. `center`/`radius` are physical pixels.
    pub fn push_disc(&mut self, center: [f32; 2], radius: f32, color: [f32; 4]) {
        self.push_inst(TextInstanceGpu {
            screen_pos: [center[0] - radius, center[1] - radius],
            size: [radius * 2.0, radius * 2.0],
            uv_rect: [0.0, 0.0, 1.0, 1.0],
            content_rect: [0.0, 0.0, 1.0, 1.0],
            color,
            outline_color: [0.0; 4],
            face_dilate: 0.0,
            outline_thickness: 0.0,
            underlay_offset_y: 0.0,
            underlay_softness: 0.0,
            kind: KIND_DISC,
        });
    }

    /// Push an anti-aliased ring (stroke) drawn inward from `radius` (the outer edge).
    /// `radius`/`thickness` are physical pixels. The quad is slightly padded so the
    /// fragment shader can render its antialias fringe instead of clipping it at the
    /// primitive boundary.
    pub fn push_ring(&mut self, center: [f32; 2], radius: f32, color: [f32; 4], thickness: f32) {
        let (screen_pos, size, content_rect) = ring_geometry(center, radius);
        self.push_inst(TextInstanceGpu {
            screen_pos,
            size,
            uv_rect: [0.0, 0.0, 1.0, 1.0],
            content_rect,
            color,
            outline_color: [0.0; 4],
            face_dilate: 0.0,
            outline_thickness: thickness,
            underlay_offset_y: 0.0,
            underlay_softness: 0.0,
            kind: KIND_RING,
        });
    }

    /// Push an anti-aliased clockwise progress arc. `progress` is normalized to 0..1 and is
    /// packed into the otherwise unused shape `uv_rect`, preserving the existing instance layout.
    pub fn push_arc(
        &mut self,
        center: [f32; 2],
        radius: f32,
        progress: f32,
        color: [f32; 4],
        thickness: f32,
    ) {
        self.push_rotating_arc(center, radius, progress, 0.0, color, thickness);
    }

    /// Push an anti-aliased clockwise arc with a normalized rotational phase.
    /// `rotation_phase` is measured in complete turns and wraps at 1.0.
    pub fn push_rotating_arc(
        &mut self,
        center: [f32; 2],
        radius: f32,
        progress: f32,
        rotation_phase: f32,
        color: [f32; 4],
        thickness: f32,
    ) {
        let (screen_pos, size, content_rect) = ring_geometry(center, radius);
        self.push_inst(TextInstanceGpu {
            screen_pos,
            size,
            uv_rect: [
                0.0,
                clamp_arc_phase(rotation_phase),
                clamp_arc_progress(progress),
                1.0,
            ],
            content_rect,
            color,
            outline_color: [0.0; 4],
            face_dilate: 0.0,
            outline_thickness: thickness,
            underlay_offset_y: 0.0,
            underlay_softness: 0.0,
            kind: KIND_ARC,
        });
    }

    /// Push an anti-aliased diagonal cross. All dimensions are physical pixels.
    pub fn push_cross(
        &mut self,
        center: [f32; 2],
        half_size: f32,
        color: [f32; 4],
        thickness: f32,
    ) {
        let (screen_pos, size, content_rect) = cross_geometry(center, half_size);
        self.push_inst(TextInstanceGpu {
            screen_pos,
            size,
            uv_rect: [0.0, 0.0, 1.0, 1.0],
            content_rect,
            color,
            outline_color: [0.0; 4],
            face_dilate: 0.0,
            outline_thickness: thickness,
            underlay_offset_y: 0.0,
            underlay_softness: 0.0,
            kind: KIND_CROSS,
        });
    }

    /// Push an anti-aliased filled downward triangle. `center`/`size` are physical pixels.
    pub fn push_triangle(&mut self, center: [f32; 2], size: [f32; 2], color: [f32; 4]) {
        let size = [size[0].max(0.0), size[1].max(0.0)];
        self.push_inst(TextInstanceGpu {
            screen_pos: [center[0] - size[0] * 0.5, center[1] - size[1] * 0.5],
            size,
            uv_rect: [0.0, 0.0, 1.0, 1.0],
            content_rect: [0.0, 0.0, 1.0, 1.0],
            color,
            outline_color: [0.0; 4],
            face_dilate: 0.0,
            outline_thickness: 0.0,
            underlay_offset_y: 0.0,
            underlay_softness: 0.0,
            kind: KIND_TRIANGLE,
        });
    }

    /// Push a rounded-square image sprite from the avatar atlas. `center`/`radius` are physical
    /// pixels; `uv_rect` comes from [`avatar_uv`](Self::avatar_uv); `tint` multiplies the texels.
    pub fn push_sprite(
        &mut self,
        center: [f32; 2],
        radius: f32,
        uv_rect: [f32; 4],
        tint: [f32; 4],
    ) {
        self.push_inst(TextInstanceGpu {
            screen_pos: [center[0] - radius, center[1] - radius],
            size: [radius * 2.0, radius * 2.0],
            uv_rect,
            content_rect: [0.0, 0.0, 1.0, 1.0],
            color: tint,
            outline_color: [0.0; 4],
            face_dilate: AVATAR_CORNER_RADIUS_RATIO,
            outline_thickness: 0.0,
            underlay_offset_y: 0.0,
            underlay_softness: 0.0,
            kind: KIND_SPRITE,
        });
    }

    /// Push a pixel-art building sprite from the generated gameplay atlas.
    pub fn push_building_sprite(
        &mut self,
        center: [f32; 2],
        size: [f32; 2],
        uv_rect: [f32; 4],
        tint: [f32; 4],
    ) {
        let size = [size[0].max(0.0), size[1].max(0.0)];
        self.push_inst(TextInstanceGpu {
            screen_pos: [center[0] - size[0] * 0.5, center[1] - size[1] * 0.5],
            size,
            uv_rect,
            content_rect: [0.0, 0.0, 1.0, 1.0],
            color: tint,
            outline_color: [0.0; 4],
            face_dilate: 0.0,
            outline_thickness: 0.0,
            underlay_offset_y: 0.0,
            underlay_softness: 0.0,
            kind: KIND_BUILDING_SPRITE,
        });
    }

    /// Push an anti-aliased filled rectangle. `screen_pos`/`size` are physical pixels.
    pub fn push_rect(&mut self, screen_pos: [f32; 2], size: [f32; 2], color: [f32; 4]) {
        self.push_inst(TextInstanceGpu {
            screen_pos,
            size,
            uv_rect: [0.0; 4],
            content_rect: [0.0, 0.0, 1.0, 1.0],
            color,
            outline_color: [0.0; 4],
            face_dilate: 0.0,
            outline_thickness: 0.0,
            underlay_offset_y: 0.0,
            underlay_softness: 0.0,
            kind: KIND_RECT,
        });
    }

    /// Push a filled rounded rectangle with an optional inward outline.
    /// `size` and `outline_thickness` are physical pixels; `corner_radius_ratio` is
    /// relative to the shorter side.
    pub fn push_rounded_rect(
        &mut self,
        center: [f32; 2],
        size: [f32; 2],
        corner_radius_ratio: f32,
        color: [f32; 4],
        outline_color: [f32; 4],
        outline_thickness: f32,
    ) {
        let size = [size[0].max(0.0), size[1].max(0.0)];
        self.push_inst(TextInstanceGpu {
            screen_pos: [center[0] - size[0] * 0.5, center[1] - size[1] * 0.5],
            size,
            uv_rect: [0.0; 4],
            content_rect: [0.0, 0.0, 1.0, 1.0],
            color,
            outline_color,
            face_dilate: if corner_radius_ratio.is_finite() {
                corner_radius_ratio.clamp(0.0, 0.5)
            } else {
                0.0
            },
            outline_thickness: outline_thickness.max(0.0),
            underlay_offset_y: 0.0,
            underlay_softness: 0.0,
            kind: KIND_ROUNDED_RECT,
        });
    }

    /// Write a decoded `AVATAR_CELL`×`AVATAR_CELL` RGBA portrait into an atlas slot. The atlas
    /// re-uploads on the next `draw`. Ignores out-of-range slots or wrong-sized data.
    pub fn upload_avatar(&mut self, slot: usize, rgba_cell: &[u8]) {
        let cell = AVATAR_CELL as usize;
        if slot >= AVATAR_SLOT_COUNT || rgba_cell.len() != cell * cell * 4 {
            return;
        }
        let atlas_w = (AVATAR_COLS * AVATAR_CELL) as usize;
        let x0 = (slot % AVATAR_COLS as usize) * cell;
        let y0 = (slot / AVATAR_COLS as usize) * cell;
        let dst_ptr = self.avatar_atlas_tex.buffer.data();
        let total = atlas_w * (AVATAR_ROWS * AVATAR_CELL) as usize * 4;
        let dst = unsafe { std::slice::from_raw_parts_mut(dst_ptr, total) };
        for y in 0..cell {
            let s = y * cell * 4;
            let d = ((y0 + y) * atlas_w + x0) * 4;
            dst[d..d + cell * 4].copy_from_slice(&rgba_cell[s..s + cell * 4]);
        }
        if !self.avatar_dirty_slots.contains(&slot) {
            self.avatar_dirty_slots.push(slot);
        }
        self.avatar_loaded[slot] = true;
    }

    /// UV rect for a loaded avatar slot, or `None` if that slot hasn't been uploaded yet.
    pub fn avatar_uv(&self, slot: usize) -> Option<[f32; 4]> {
        if slot < AVATAR_SLOT_COUNT && self.avatar_loaded[slot] {
            Some(avatar_slot_uv(slot))
        } else {
            None
        }
    }

    fn write_buffers(&self, context: &gpu::Context) {
        if !self.upload_instances.is_empty() {
            let bytes = bytemuck::cast_slice(&self.upload_instances);
            let dst = self.buffer.data();
            unsafe {
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), dst, bytes.len());
            }
            // ponytail: only sync active slice to avoid massive WASM/WebGL overhead
            context.sync_buffer(self.buffer, 0, bytes.len() as u64);
        }
    }

    pub fn draw(
        &mut self,
        encoder: &mut gpu::CommandEncoder,
        target_view: gpu::TextureView,
        screen_size: [f32; 2],
        context: &gpu::Context,
    ) {
        let glyph_count = self.upload_instances.len() as u32;
        if glyph_count == 0 {
            return;
        }

        self.write_buffers(context);

        // The atlas was initialized by upload_atlas; update only cells that just arrived.
        for slot in self.avatar_dirty_slots.drain(..) {
            let cell = AVATAR_CELL as usize;
            let x = (slot % AVATAR_COLS as usize) * cell;
            let y = (slot / AVATAR_COLS as usize) * cell;
            self.avatar_atlas_tex.upload_region(
                encoder,
                context,
                [x as u32, y as u32],
                [AVATAR_CELL, AVATAR_CELL],
            );
        }

        let mut pass = encoder.render(
            "text_pass",
            gpu::RenderTargetSet {
                colors: &[gpu::RenderTarget {
                    view: target_view,
                    init_op: gpu::InitOp::Load,
                    finish_op: gpu::FinishOp::Store,
                }],
                depth_stencil: None,
            },
        );

        let globals = TextGlobals {
            screen_size,
            _pad: [0.0; 2],
        };

        let shader_data = TextShaderData {
            globals,
            font_atlas: self.font_atlas_tex.view,
            font_sampler: self.sampler,
            emoji_atlas: self.emoji_atlas_tex.view,
            emoji_sampler: self.emoji_sampler,
            avatar_atlas: self.avatar_atlas_tex.view,
            avatar_sampler: self.avatar_sampler,
            building_atlas: self.building_atlas_tex.view,
            building_sampler: self.building_sampler,
        };

        let mut rc = pass.with(&self.pipeline);
        rc.bind(0, &shader_data);
        rc.bind_vertex(0, self.buffer.at(0));
        rc.draw(0, 6, 0, glyph_count);
    }

    pub fn destroy(&mut self, render_ctx: &RenderContext) {
        render_ctx
            .context
            .destroy_render_pipeline(&mut self.pipeline);
        render_ctx.context.destroy_buffer(self.buffer);
        render_ctx.context.destroy_sampler(self.sampler);
        render_ctx.context.destroy_sampler(self.emoji_sampler);
        render_ctx.context.destroy_sampler(self.avatar_sampler);
        render_ctx.context.destroy_sampler(self.building_sampler);
        render_ctx
            .context
            .destroy_texture_view(self.font_atlas_tex.view);
        render_ctx
            .context
            .destroy_texture(self.font_atlas_tex.texture);
        render_ctx
            .context
            .destroy_buffer(self.font_atlas_tex.buffer);
        render_ctx
            .context
            .destroy_texture_view(self.emoji_atlas_tex.view);
        render_ctx
            .context
            .destroy_texture(self.emoji_atlas_tex.texture);
        render_ctx
            .context
            .destroy_buffer(self.emoji_atlas_tex.buffer);
        render_ctx
            .context
            .destroy_texture_view(self.avatar_atlas_tex.view);
        render_ctx
            .context
            .destroy_texture(self.avatar_atlas_tex.texture);
        render_ctx
            .context
            .destroy_buffer(self.avatar_atlas_tex.buffer);
        render_ctx
            .context
            .destroy_texture_view(self.building_atlas_tex.view);
        render_ctx
            .context
            .destroy_texture(self.building_atlas_tex.texture);
        render_ctx
            .context
            .destroy_buffer(self.building_atlas_tex.buffer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prepared_test_text(atlas: &FontAtlas, text: &str, spacing: f32) -> PreparedText {
        prepared_test_text_aligned(atlas, text, spacing, 0.5)
    }

    fn prepared_test_text_aligned(
        atlas: &FontAtlas,
        text: &str,
        spacing: f32,
        align_x: f32,
    ) -> PreparedText {
        let font_size = atlas.atlas.info.size.max(1) as f32;
        let mut items = Vec::new();
        let (advance_width, has_emoji, item_count) =
            shape_text(atlas, text, spacing, 1.4, |item| items.push(item));
        PreparedText {
            visual_bounds: prepared_text_bounds(&items, advance_width, align_x),
            items,
            advance_width,
            align_x,
            font_size,
            line_height: text_measure_height(font_size, 1.4, has_emoji),
            complete: item_count <= MAX_TEXT_GLYPHS,
        }
    }

    fn paint_test_text(
        atlas: &FontAtlas,
        prepared: &PreparedText,
        font_size: f32,
    ) -> Vec<TextInstanceGpu> {
        paint_test_text_with_settings(atlas, prepared, font_size, TextPaintStyle::default())
    }

    fn paint_test_text_with_settings(
        atlas: &FontAtlas,
        prepared: &PreparedText,
        font_size: f32,
        settings: TextPaintStyle,
    ) -> Vec<TextInstanceGpu> {
        paint_test_text_at(atlas, prepared, font_size, [80.0, 60.0], settings)
    }

    fn paint_test_text_at(
        atlas: &FontAtlas,
        prepared: &PreparedText,
        font_size: f32,
        pos: [f32; 2],
        settings: TextPaintStyle,
    ) -> Vec<TextInstanceGpu> {
        let mut instances = Vec::new();
        emit_prepared_text(
            &mut instances,
            &prepared.items,
            prepared.advance_width,
            prepared.align_x,
            atlas.atlas.info.size.max(1) as f32,
            font_size,
            pos,
            [1.0; 4],
            settings,
        );
        instances
    }

    #[test]
    fn ring_geometry_keeps_requested_radius_inside_antialias_padding() {
        let (screen_pos, size, content_rect) = ring_geometry([100.0, 50.0], 10.0);
        let min = [
            screen_pos[0] + size[0] * content_rect[0],
            screen_pos[1] + size[1] * content_rect[1],
        ];
        let max = [
            screen_pos[0] + size[0] * content_rect[2],
            screen_pos[1] + size[1] * content_rect[3],
        ];

        assert!((min[0] - 90.0).abs() < 1e-5);
        assert!((min[1] - 40.0).abs() < 1e-5);
        assert!((max[0] - 110.0).abs() < 1e-5);
        assert!((max[1] - 60.0).abs() < 1e-5);
    }

    #[test]
    fn cross_geometry_keeps_requested_extent_inside_antialias_padding() {
        let (screen_pos, size, content_rect) = cross_geometry([100.0, 50.0], 4.0);
        let min = [
            screen_pos[0] + size[0] * content_rect[0],
            screen_pos[1] + size[1] * content_rect[1],
        ];
        let max = [
            screen_pos[0] + size[0] * content_rect[2],
            screen_pos[1] + size[1] * content_rect[3],
        ];

        assert!((min[0] - 96.0).abs() < 1e-5);
        assert!((min[1] - 46.0).abs() < 1e-5);
        assert!((max[0] - 104.0).abs() < 1e-5);
        assert!((max[1] - 54.0).abs() < 1e-5);
    }

    #[test]
    fn arc_progress_is_finite_and_clamped() {
        assert_eq!(clamp_arc_progress(-1.0), 0.0);
        assert_eq!(clamp_arc_progress(0.35), 0.35);
        assert_eq!(clamp_arc_progress(2.0), 1.0);
        assert_eq!(clamp_arc_progress(f32::NAN), 0.0);
    }

    #[test]
    fn emoji_geometry_keeps_logical_size_and_reserves_effect_room() {
        let outline = OutlineStyle {
            color: [0.0, 0.0, 0.0, 1.0],
            thickness: 3.0,
            shadow_y: 5.0,
            reference_diameter: 0.0,
        };
        let (screen_pos, size, content_rect) = emoji_geometry([100.0, 50.0], 12.0, outline);
        let content_size = [
            size[0] * (content_rect[2] - content_rect[0]),
            size[1] * (content_rect[3] - content_rect[1]),
        ];

        assert_eq!(screen_pos, [83.0, 33.0]);
        assert_eq!(size, [34.0, 34.0]);
        assert!((content_size[0] - 24.0).abs() < 1e-5);
        assert!((content_size[1] - 24.0).abs() < 1e-5);
    }

    #[test]
    fn emoji_geometry_does_not_expand_without_effects() {
        let (screen_pos, size, content_rect) =
            emoji_geometry([10.0, 20.0], 7.0, OutlineStyle::NONE);

        assert_eq!(screen_pos, [3.0, 13.0]);
        assert_eq!(size, [14.0, 14.0]);
        assert_eq!(content_rect, [0.0, 0.0, 1.0, 1.0]);
    }

    #[test]
    fn inline_emoji_uses_the_size_adjusted_outline() {
        let outline = OutlineStyle {
            color: [0.0, 0.0, 0.0, 0.8],
            thickness: 2.25,
            shadow_y: 3.5,
            reference_diameter: OutlineStyle::EMOJI_REFERENCE_DIAMETER,
        };
        let instance = emoji_instance(
            [0.0, 0.0, 1.0, 1.0],
            [0.0, 0.0],
            4.0,
            [1.0; 4],
            outline,
            0.25,
        );

        assert_eq!(instance.outline_color, outline.color);
        assert!((instance.outline_thickness - 0.9).abs() < 1e-5);
        assert!((instance.underlay_offset_y - 1.4).abs() < 1e-5);
        assert_eq!(instance.underlay_softness, 0.25);
    }

    #[test]
    fn emoji_outline_scales_down_only_for_small_content() {
        let outline = OutlineStyle {
            color: [0.0, 0.0, 0.0, 1.0],
            thickness: 1.4,
            shadow_y: 2.0,
            reference_diameter: OutlineStyle::EMOJI_REFERENCE_DIAMETER,
        };
        let close = outline.scaled_for_emoji(32.0);
        let far = outline.scaled_for_emoji(16.0);
        let tiny = outline.scaled_for_emoji(12.0);

        assert_eq!(close.thickness, 1.4);
        assert_eq!(close.shadow_y, 2.0);
        assert!((far.thickness - 0.7).abs() < 1e-5);
        assert!((far.shadow_y - 1.0).abs() < 1e-5);
        assert!((tiny.thickness - 0.56).abs() < 1e-5);
        assert!((tiny.shadow_y - 0.8).abs() < 1e-5);
    }

    #[test]
    fn text_measure_height_only_expands_for_real_emoji() {
        assert_eq!(text_measure_height(48.0, 1.4, false), 48.0);
        assert_eq!(text_measure_height(48.0, 1.4, true), 67.2);
        assert_eq!(text_measure_height(48.0, 0.8, true), 48.0);
    }

    #[test]
    fn prepared_text_uniformly_scales_glyphs_and_letter_advances() {
        let atlas = FontAtlas::load_static();
        let prepared = prepared_test_text(&atlas, "Nameplate 42", 0.95);
        assert!(prepared.complete);
        assert!(prepared.instance_count() > 4);
        assert!(prepared.advance_width > 0.0);

        let reference = paint_test_text(&atlas, &prepared, 32.0);
        for factor in [0.75, 0.5, 0.25] {
            let scaled = paint_test_text(&atlas, &prepared, 32.0 * factor);
            assert_eq!(scaled.len(), reference.len());
            for (base, actual) in reference.iter().zip(&scaled) {
                assert!((actual.size[0] - base.size[0] * factor).abs() < 1e-4);
                assert!((actual.size[1] - base.size[1] * factor).abs() < 1e-4);
                assert!(
                    (actual.screen_pos[0] - 80.0 - (base.screen_pos[0] - 80.0) * factor).abs()
                        < 1e-4
                );
                assert!(
                    (actual.screen_pos[1] - 60.0 - (base.screen_pos[1] - 60.0) * factor).abs()
                        < 1e-4
                );
            }
        }
    }

    #[test]
    fn prepared_text_measurement_uses_the_same_shaped_advances_as_drawing() {
        let atlas = FontAtlas::load_static();
        let prepared = prepared_test_text(&atlas, "Ação 123 🏅", 0.95);
        let measure = prepared.measure_at(24.0);
        let expected_advance = prepared.advance_width * 24.0 / prepared.font_size;
        assert!((measure.width - expected_advance).abs() < 1e-4);
        assert_eq!(
            prepared.instance_count(),
            paint_test_text(&atlas, &prepared, 24.0).len()
        );
    }

    #[test]
    fn prepared_text_bounds_match_emitted_quads_at_zoom_scales() {
        let atlas = FontAtlas::load_static();
        let settings = TextPaintStyle {
            outline: OutlineStyle::NONE,
            ..TextPaintStyle::default()
        };

        for value in ["Li", "Ação 42 🏅", "Alexandria de Todos os Mundos 123"] {
            let prepared = prepared_test_text(&atlas, value, 0.95);
            for scale in [1.0, 0.75, 0.5, 0.25] {
                let font_size = 32.0 * scale;
                let expected = prepared.visual_bounds_at(font_size).unwrap();
                let instances =
                    paint_test_text_with_settings(&atlas, &prepared, font_size, settings);
                let mut actual = [
                    f32::INFINITY,
                    f32::INFINITY,
                    f32::NEG_INFINITY,
                    f32::NEG_INFINITY,
                ];
                for instance in instances {
                    actual[0] = actual[0].min(instance.screen_pos[0] - 80.0);
                    actual[1] = actual[1].min(instance.screen_pos[1] - 60.0);
                    actual[2] = actual[2].max(instance.screen_pos[0] + instance.size[0] - 80.0);
                    actual[3] = actual[3].max(instance.screen_pos[1] + instance.size[1] - 60.0);
                }
                for (measured, emitted) in expected.into_iter().zip(actual) {
                    assert!(
                        (measured - emitted).abs() < 1e-4,
                        "value={value:?} scale={scale} measured={measured} emitted={emitted}"
                    );
                }
            }
        }
    }

    #[test]
    fn nameplate_fit_bounds_contain_the_real_prepared_name_and_troops_quads() {
        use crate::nameplate::{NameplateLayout, NameplateMetrics, NameplateStatus, ScreenPoint};
        use sow_core::player::PlayerType;

        let atlas = FontAtlas::load_static();
        let name = prepared_test_text(&atlas, "Ação 42 🏅", 0.95);
        let troops = prepared_test_text_aligned(&atlas, "1 234 567", 0.95, 0.0);
        let center = ScreenPoint([100.0, 90.0]);
        let metrics = NameplateMetrics::compute(16.0, PlayerType::Human, true);
        let status = NameplateStatus {
            show_names: true,
            show_troops: true,
            ..Default::default()
        };
        let layout = NameplateLayout::compute(center, metrics, &name, &troops, 1.0, status, 2.0);
        let settings = TextPaintStyle {
            outline: OutlineStyle::NONE,
            ..TextPaintStyle::default()
        };

        for scale in [1.0, 0.75, 0.5, 0.25] {
            let scaled = layout.scaled_about(center, scale);
            let bounds = scaled.visual_bounds(center, 2.0 * scale);
            let extents = bounds.extents_about(center);
            let minimum = [center.0[0] - extents[0], center.0[1] - extents[1]];
            let maximum = [center.0[0] + extents[0], center.0[1] + extents[1]];

            for (prepared, font_size, anchor) in [
                (
                    &name,
                    metrics.render_size() * scale,
                    scaled.name_anchor(center),
                ),
                (
                    &troops,
                    metrics.troops_render_size() * scale,
                    scaled.troops_text_anchor(center),
                ),
            ] {
                let instances = paint_test_text_at(&atlas, prepared, font_size, anchor.0, settings);
                for instance in instances {
                    assert!(
                        instance.screen_pos[0] >= minimum[0] - 1e-4,
                        "scale={scale} x-min={} bounds={minimum:?}",
                        instance.screen_pos[0]
                    );
                    assert!(
                        instance.screen_pos[1] >= minimum[1] - 1e-4,
                        "scale={scale} y-min={} bounds={minimum:?}",
                        instance.screen_pos[1]
                    );
                    assert!(
                        instance.screen_pos[0] + instance.size[0] <= maximum[0] + 1e-4,
                        "scale={scale} x-max={} bounds={maximum:?}",
                        instance.screen_pos[0] + instance.size[0]
                    );
                    assert!(
                        instance.screen_pos[1] + instance.size[1] <= maximum[1] + 1e-4,
                        "scale={scale} y-max={} bounds={maximum:?}",
                        instance.screen_pos[1] + instance.size[1]
                    );
                }
            }
        }
    }

    #[test]
    fn kerning_and_measurement_use_the_same_character_spacing() {
        let mut atlas = FontAtlas::load_static();
        let left = 'A';
        let right = 'V';
        let kern = -5;
        assert!(atlas.char_map.contains_key(&left));
        assert!(atlas.char_map.contains_key(&right));
        atlas.kerning_map.insert((left, right), kern);
        let spacing = 0.55;
        let mut prepared = prepared_test_text(&atlas, &format!("{left}{right}"), spacing);
        prepared.align_x = 0.0;
        let instances = paint_test_text(&atlas, &prepared, prepared.font_size);
        let left_glyph = atlas.char_map.get(&left).unwrap();
        let right_glyph = atlas.char_map.get(&right).unwrap();
        let expected_delta = (left_glyph.xadvance as f32 + kern as f32) * spacing
            + right_glyph.xoffset as f32
            - left_glyph.xoffset as f32;
        assert_eq!(instances.len(), 2);
        assert!(
            (instances[1].screen_pos[0] - instances[0].screen_pos[0] - expected_delta).abs() < 1e-4
        );
    }

    #[test]
    fn prepared_text_emission_is_all_or_nothing_at_the_instance_limit() {
        let item = PreparedTextItem {
            kind: PreparedTextKind::Glyph,
            offset: [0.0; 2],
            size: [10.0; 2],
            uv_rect: [0.0, 0.0, 1.0, 1.0],
        };
        let mut instances =
            vec![<TextInstanceGpu as bytemuck::Zeroable>::zeroed(); MAX_TEXT_GLYPHS - 1];
        let accepted = emit_prepared_text_if_fits(
            &mut instances,
            &[item, item],
            20.0,
            0.0,
            48.0,
            24.0,
            [0.0; 2],
            [1.0; 4],
            TextPaintStyle::default(),
        );
        assert!(!accepted);
        assert_eq!(instances.len(), MAX_TEXT_GLYPHS - 1);
    }
}
