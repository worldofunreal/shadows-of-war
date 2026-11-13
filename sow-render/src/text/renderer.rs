use crate::context::RenderContext;
use crate::text::msdf::{FontAtlas, MsdfChar};
use crate::text::texture::{AvatarAtlasTexture, FontAtlasTexture, TextAtlasImages};
use crate::text::types::{
    KIND_ARC, KIND_BUILDING_SPRITE, KIND_CROSS, KIND_DISC, KIND_GLYPH, KIND_ICON,
    KIND_RECT, KIND_RING, KIND_ROUNDED_RECT, KIND_SPRITE, KIND_STATUS_SPRITE, KIND_TRIANGLE,
    NameplateStatusSprite, OutlineStyle, TextGlobals, TextInstanceGpu, TextPaintStyle,
    TextShaderData,
};
use blade_graphics as gpu;

pub fn building_sprite_uv(kind: sow_core::game::BuildingKind, level: u8) -> Option<[f32; 4]> {
    crate::text::building_atlas::uv_rect(kind, level)
}

pub const MAX_TEXT_GLYPHS: usize = 32_768;
const RING_AA_MARGIN: f32 = 1.5;
const CROSS_AA_MARGIN: f32 = 1.0;
#[cfg(not(target_arch = "wasm32"))]
const ARABIC_FONT_BYTES: &[u8] =
    include_bytes!("../../../sow-web/site/fonts/noto-sans-arabic-regular.ttf");

/// The web build downloads the Arabic shaping font only after a name needs it; until then
/// Arabic runs are left out and `arabic_font_wanted` asks the client to fetch it.
#[cfg(target_arch = "wasm32")]
static ARABIC_FONT: std::sync::OnceLock<&'static [u8]> = std::sync::OnceLock::new();
static ARABIC_FONT_WANTED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Install the downloaded Arabic font. Returns false if it does not parse or one is installed.
#[cfg(target_arch = "wasm32")]
pub fn set_arabic_font(bytes: Vec<u8>) -> bool {
    if rustybuzz::Face::from_slice(&bytes, 0).is_none() {
        return false;
    }
    ARABIC_FONT.set(Vec::leak(bytes)).is_ok()
}

/// True once some text needed the Arabic font and it is not installed yet.
pub fn arabic_font_wanted() -> bool {
    ARABIC_FONT_WANTED.load(std::sync::atomic::Ordering::Relaxed) && arabic_font_bytes().is_none()
}

/// Changes when the Arabic font arrives, so callers that cache shaped text re-prepare it.
pub fn arabic_font_generation() -> u32 {
    u32::from(arabic_font_bytes().is_some())
}

fn arabic_font_bytes() -> Option<&'static [u8]> {
    #[cfg(target_arch = "wasm32")]
    return ARABIC_FONT.get().copied();
    #[cfg(not(target_arch = "wasm32"))]
    return Some(ARABIC_FONT_BYTES);
}

fn arabic_face() -> Option<&'static rustybuzz::Face<'static>> {
    static FACE: std::sync::OnceLock<rustybuzz::Face<'static>> = std::sync::OnceLock::new();
    if let Some(face) = FACE.get() {
        return Some(face);
    }
    let Some(bytes) = arabic_font_bytes() else {
        ARABIC_FONT_WANTED.store(true, std::sync::atomic::Ordering::Relaxed);
        return None;
    };
    Some(FACE.get_or_init(|| {
        rustybuzz::Face::from_slice(bytes, 0).expect("Arabic font was validated before install")
    }))
}

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
struct PreparedTextItem {
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

    /// Bounds of the exact glyph quads emitted at this size,
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

fn outlined_geometry(
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

fn outlined_instance(
    uv_rect: [f32; 4],
    screen_pos: [f32; 2],
    half_size: f32,
    tint: [f32; 4],
    outline: OutlineStyle,
    underlay_softness: f32,
    kind: f32,
) -> TextInstanceGpu {
    let (screen_pos, size, content_rect) = outlined_geometry(screen_pos, half_size, outline);
    TextInstanceGpu {
        screen_pos,
        size,
        uv_rect,
        content_rect,
        color: tint,
        outline_color: outline.color,
        face_dilate: outline.reference_diameter,
        outline_thickness: outline.thickness,
        underlay_offset_y: outline.shadow_y,
        underlay_softness,
        kind,
    }
}

fn troop_icon_tint(alpha: f32) -> [f32; 4] {
    [1.0, 1.0, 1.0, alpha]
}

/// Shapes `text` with the msdf atlas; characters the atlas lacks are skipped.
fn shape_text(
    atlas: &FontAtlas,
    text: &str,
    char_spacing: f32,
    mut emit: impl FnMut(PreparedTextItem),
) -> (f32, usize) {
    shape_text_inner(atlas, text, char_spacing, &mut emit)
}

fn shape_text_inner(
    atlas: &FontAtlas,
    text: &str,
    char_spacing: f32,
    emit: &mut dyn FnMut(PreparedTextItem),
) -> (f32, usize) {
    if text.chars().any(is_arabic_character) {
        return shape_bidi_text(atlas, text, char_spacing, emit);
    }

    let atlas_width = atlas.atlas.common.scale_w.max(1) as f32;
    let atlas_height = atlas.atlas.common.scale_h.max(1) as f32;
    let base = atlas.atlas.common.base as f32;
    let mut advance = 0.0f32;
    let mut item_count = 0usize;
    let mut prev_char = Option::<char>::None;

    for ch in text.chars() {
        if let Some(glyph) = atlas.char_map.get(&ch) {
            let kern = prev_char
                .and_then(|previous| atlas.kerning_map.get(&(previous, ch)))
                .copied()
                .unwrap_or(0) as f32;
            emit(PreparedTextItem {
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

        prev_char = None;
    }
    (advance, item_count)
}

fn is_arabic_character(ch: char) -> bool {
    matches!(
        ch as u32,
        0x0600..=0x06FF | 0x0750..=0x077F | 0x08A0..=0x08FF | 0xFB50..=0xFDFF | 0xFE70..=0xFEFF
    )
}

fn shape_bidi_text(
    atlas: &FontAtlas,
    text: &str,
    char_spacing: f32,
    emit: &mut dyn FnMut(PreparedTextItem),
) -> (f32, usize) {
    let bidi = unicode_bidi::BidiInfo::new(text, None);
    let Some(paragraph) = bidi.paragraphs.first() else {
        return (0.0, 0);
    };
    let (levels, runs) = bidi.visual_runs(paragraph, 0..text.len());
    let mut advance = 0.0;
    let mut item_count = 0usize;

    for run in runs {
        let run_text = &text[run.clone()];
        if run_text.chars().any(is_arabic_character) {
            let (run_advance, count) = shape_arabic_run(
                atlas,
                run_text,
                levels[run.start].is_rtl(),
                char_spacing,
                emit,
                advance,
            );
            advance += run_advance;
            item_count = item_count.saturating_add(count);
        } else {
            let run_start = advance;
            let (run_advance, count) =
                shape_text_inner(atlas, run_text, char_spacing, &mut |mut item| {
                    item.offset[0] += run_start;
                    emit(item);
                });
            advance += run_advance;
            item_count = item_count.saturating_add(count);
        }
    }
    (advance, item_count)
}

fn shape_arabic_run(
    atlas: &FontAtlas,
    text: &str,
    right_to_left: bool,
    char_spacing: f32,
    emit: &mut dyn FnMut(PreparedTextItem),
    initial_advance: f32,
) -> (f32, usize) {
    let Some(face) = arabic_face() else {
        return (0.0, 0);
    };
    let mut buffer = rustybuzz::UnicodeBuffer::new();
    buffer.push_str(text);
    buffer.set_direction(if right_to_left {
        rustybuzz::Direction::RightToLeft
    } else {
        rustybuzz::Direction::LeftToRight
    });
    buffer.guess_segment_properties();
    let shaped = rustybuzz::shape(face, &[], buffer);
    let infos = shaped.glyph_infos();
    let positions = shaped.glyph_positions();
    let font_scale = atlas.atlas.info.size.max(1) as f32 / face.units_per_em().max(1) as f32;
    let glyphs: Vec<Option<(&MsdfChar, bool)>> = infos
        .iter()
        .map(|info| {
            atlas
                .arabic_glyph_map
                .get(&info.glyph_id)
                .map(|glyph| (glyph, false))
                .or_else(|| {
                    let character = text
                        .get(info.cluster as usize..)
                        .and_then(|tail| tail.chars().next())
                        .filter(|character| {
                            !is_arabic_character(*character) && !character.is_whitespace()
                        })?;
                    atlas.char_map.get(&character).map(|glyph| (glyph, true))
                })
        })
        .collect();
    let advances: Vec<f32> = positions
        .iter()
        .zip(&glyphs)
        .map(|(position, glyph)| {
            let advance = match glyph {
                Some((glyph, true)) => glyph.xadvance as f32 * char_spacing,
                _ => position.x_advance as f32 * font_scale * char_spacing,
            };
            if right_to_left {
                advance.abs()
            } else {
                advance
            }
        })
        .collect();
    let run_advance: f32 = advances.iter().sum();
    let mut pen_x = if right_to_left {
        initial_advance + run_advance
    } else {
        initial_advance
    };
    let atlas_width = atlas.atlas.common.scale_w.max(1) as f32;
    let atlas_height = atlas.atlas.common.scale_h.max(1) as f32;
    let base = atlas.atlas.common.base as f32;
    let mut item_count = 0usize;

    for ((position, advance), glyph) in positions.iter().zip(advances).zip(glyphs) {
        let glyph = glyph.map(|(glyph, _)| glyph);
        if let Some(glyph) = glyph {
            emit(PreparedTextItem {
                offset: [
                    pen_x + glyph.xoffset as f32 + position.x_offset as f32 * font_scale,
                    -base + glyph.yoffset as f32 - position.y_offset as f32 * font_scale,
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
        }
        pen_x += if right_to_left { -advance } else { advance };
    }

    (run_advance, item_count)
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
        instances.push(TextInstanceGpu {
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
        });
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
    avatar_atlas: AvatarAtlasTexture,
    building_atlas_tex: FontAtlasTexture,
    status_atlas_tex: FontAtlasTexture,
    prepared_text_scratch: Vec<PreparedTextItem>,
    pipeline: gpu::RenderPipeline,
    buffer: gpu::Buffer,
    sampler: gpu::Sampler,
    avatar_sampler: gpu::Sampler,
    building_sampler: gpu::Sampler,
    status_sampler: gpu::Sampler,
    upload_instances: Vec<TextInstanceGpu>,
}

impl TextRenderer {
    pub fn new(
        context: &gpu::Context,
        surface_format: gpu::TextureFormat,
        atlases: &TextAtlasImages,
    ) -> Self {
        let font_atlas_desc = FontAtlas::load_static();
        // The font atlas holds signed distances (linear data); building and status art is sRGB.
        #[cfg(target_arch = "wasm32")]
        let (font_atlas_tex, building_atlas_tex, status_atlas_tex) = (
            FontAtlasTexture::from_image_bitmap(
                context,
                &atlases.text,
                "font_atlas",
                gpu::TextureFormat::Rgba8Unorm,
            ),
            FontAtlasTexture::from_image_bitmap(
                context,
                &atlases.buildings,
                "building_atlas",
                gpu::TextureFormat::Rgba8UnormSrgb,
            ),
            FontAtlasTexture::from_image_bitmap(
                context,
                &atlases.badges,
                "nameplate_status_atlas",
                gpu::TextureFormat::Rgba8UnormSrgb,
            ),
        );
        #[cfg(not(target_arch = "wasm32"))]
        let (font_atlas_tex, building_atlas_tex, status_atlas_tex) = {
            let _: &TextAtlasImages = atlases;
            (
                FontAtlasTexture::from_bytes(
                    context,
                    crate::FONT_ATLAS_BYTES,
                    "font_atlas",
                    gpu::TextureFormat::Rgba8Unorm,
                ),
                FontAtlasTexture::from_bytes(
                    context,
                    crate::BUILDING_ATLAS_BYTES,
                    "building_atlas",
                    gpu::TextureFormat::Rgba8UnormSrgb,
                ),
                FontAtlasTexture::from_bytes(
                    context,
                    crate::NAMEPLATE_STATUS_ATLAS_BYTES,
                    "nameplate_status_atlas",
                    gpu::TextureFormat::Rgba8UnormSrgb,
                ),
            )
        };
        let avatar_atlas = AvatarAtlasTexture::new(context);
        let shader_source = crate::shader_params::prepare_shader(
            "TEXT_",
            include_str!("../shaders/text_glow.wgsl"),
        );
        let shader = context.create_shader(gpu::ShaderDesc {
            source: &shader_source,
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

        let status_sampler = context.create_sampler(gpu::SamplerDesc {
            name: "nameplate_status_pixel_sampler",
            mag_filter: gpu::FilterMode::Nearest,
            min_filter: gpu::FilterMode::Nearest,
            ..Default::default()
        });

        Self {
            font_atlas_desc,
            font_atlas_tex,
            avatar_atlas,
            building_atlas_tex,
            status_atlas_tex,
            prepared_text_scratch: Vec::with_capacity(32),
            pipeline,
            buffer,
            sampler,
            avatar_sampler,
            building_sampler,
            status_sampler,
            upload_instances: Vec::with_capacity(4096),
        }
    }

    /// Transition all atlas textures to a defined layout before first use. Must be called once
    /// (alongside `upload_atlas`) before any draw.
    pub fn init_textures(&self, encoder: &mut gpu::CommandEncoder) {
        encoder.init_texture(self.font_atlas_tex.texture);
        self.avatar_atlas.init(encoder);
        encoder.init_texture(self.building_atlas_tex.texture);
        encoder.init_texture(self.status_atlas_tex.texture);
    }

    pub fn upload_atlas(&self, encoder: &mut gpu::CommandEncoder, context: &gpu::Context) {
        self.font_atlas_tex.upload(encoder, context);
        self.building_atlas_tex.upload(encoder, context);
        self.status_atlas_tex.upload(encoder, context);
    }

    pub fn begin_frame(&mut self) {
        self.upload_instances.clear();
    }

    /// Internal: push an instance, respecting buffer limits.
    fn push_inst(&mut self, inst: TextInstanceGpu) {
        if self.upload_instances.len() < MAX_TEXT_GLYPHS {
            self.upload_instances.push(inst);
        }
    }

    pub fn prepare_string(&self, text: &str, char_spacing: f32, align_x: f32) -> PreparedText {
        let font_size = self.font_atlas_desc.atlas.info.size.max(1) as f32;
        let mut items = Vec::with_capacity(text.chars().count().min(MAX_TEXT_GLYPHS));
        let (advance_width, item_count) =
            shape_text(&self.font_atlas_desc, text, char_spacing, |item| {
                if items.len() < MAX_TEXT_GLYPHS {
                    items.push(item);
                }
            });
        let visual_bounds = prepared_text_bounds(&items, advance_width, align_x);
        PreparedText {
            items,
            advance_width,
            align_x,
            font_size,
            line_height: if text.is_empty() { 0.0 } else { font_size },
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

    /// Transform only the just-emitted glyphs, keeping shaping and the frame budget unchanged.
    pub fn push_prepared_text_transformed(
        &mut self,
        prepared: &PreparedText,
        pos: [f32; 2],
        final_font_size: f32,
        color: [f32; 4],
        settings: TextPaintStyle,
        mut transform: impl FnMut(usize, &mut TextInstanceGpu),
    ) -> bool {
        let start = self.upload_instances.len();
        if !self.push_prepared_text(prepared, pos, final_font_size, color, settings) {
            return false;
        }
        for (index, glyph) in self.upload_instances[start..].iter_mut().enumerate() {
            transform(index, glyph);
        }
        true
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
        layout: (f32, f32),
    ) {
        let (align_x, char_spacing) = layout;
        let font_unit = self.font_atlas_desc.atlas.info.size.max(1) as f32;
        let TextRenderer {
            font_atlas_desc,
            prepared_text_scratch,
            upload_instances,
            ..
        } = self;
        prepared_text_scratch.clear();
        let (advance_width, item_count) = shape_text(font_atlas_desc, text, char_spacing, |item| {
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
    /// `push_string` emits.
    pub fn measure_string(&self, text: &str, font_size: f32, char_spacing: f32) -> TextMeasure {
        let (advance_width, _) = shape_text(&self.font_atlas_desc, text, char_spacing, |_| {});
        let font_unit = self.font_atlas_desc.atlas.info.size.max(1) as f32;
        let scale = font_size / font_unit;
        TextMeasure {
            width: advance_width * scale,
            height: if text.is_empty() { 0.0 } else { font_size },
        }
    }

    /// Push the troop sword with its atlas colors and only the requested opacity.
    pub fn push_troop_icon(
        &mut self,
        screen_pos: [f32; 2],
        half_size: f32,
        alpha: f32,
        outline: OutlineStyle,
    ) -> bool {
        self.push_icon(
            NameplateStatusSprite::Troops,
            screen_pos,
            half_size,
            troop_icon_tint(alpha),
            outline,
        );
        true
    }

    /// Push art from `assets/art/icons` with an alpha-dilated outline and drop shadow.
    pub fn push_icon(
        &mut self,
        sprite: NameplateStatusSprite,
        screen_pos: [f32; 2],
        half_size: f32,
        tint: [f32; 4],
        outline: OutlineStyle,
    ) {
        self.push_inst(outlined_instance(
            sprite.uv_rect(),
            screen_pos,
            half_size,
            tint,
            outline,
            0.0,
            KIND_ICON,
        ));
    }

    /// Push a filled, anti-aliased disc. `center`/`size` are physical pixels.
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

    /// Push an anti-aliased filled triangle; negative height points upward. `center`/`size` are physical pixels.
    pub fn push_triangle(&mut self, center: [f32; 2], size: [f32; 2], color: [f32; 4]) {
        let points_up = size[1] < 0.0;
        let size = [size[0].max(0.0), size[1].abs()];
        self.push_inst(TextInstanceGpu {
            screen_pos: [center[0] - size[0] * 0.5, center[1] - size[1] * 0.5],
            size,
            uv_rect: [0.0, 0.0, 1.0, 1.0],
            content_rect: [0.0, 0.0, 1.0, 1.0],
            color,
            outline_color: [0.0; 4],
            face_dilate: if points_up { 1.0 } else { 0.0 },
            outline_thickness: 0.0,
            underlay_offset_y: 0.0,
            underlay_softness: 0.0,
            kind: KIND_TRIANGLE,
        });
    }

    /// Push a circularly masked image sprite from the avatar atlas. `center`/`size` are physical
    /// pixels; `uv_rect` comes from [`avatar_uv`](Self::avatar_uv); `tint` multiplies the texels.
    pub fn push_sprite(
        &mut self,
        center: [f32; 2],
        size: [f32; 2],
        uv_rect: [f32; 4],
        tint: [f32; 4],
    ) {
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

    /// Push one of the pixel-art diplomacy sprites used by nameplates.
    pub fn push_status_sprite(
        &mut self,
        sprite: NameplateStatusSprite,
        center: [f32; 2],
        diameter: f32,
        tint: [f32; 4],
    ) {
        self.push_status_sprite_with_desaturation(sprite, center, [diameter; 2], tint, 0.0);
    }

    /// Push a status sprite with separate horizontal and vertical scale.
    pub fn push_status_sprite_scaled(
        &mut self,
        sprite: NameplateStatusSprite,
        center: [f32; 2],
        size: [f32; 2],
        tint: [f32; 4],
    ) {
        self.push_status_sprite_with_desaturation(sprite, center, size, tint, 0.0);
    }

    fn push_status_sprite_with_desaturation(
        &mut self,
        sprite: NameplateStatusSprite,
        center: [f32; 2],
        size: [f32; 2],
        tint: [f32; 4],
        desaturation: f32,
    ) {
        let size = size.map(|axis| axis.max(0.0));
        self.push_inst(TextInstanceGpu {
            screen_pos: [center[0] - size[0] * 0.5, center[1] - size[1] * 0.5],
            size,
            uv_rect: sprite.uv_rect(),
            content_rect: [0.0, 0.0, 1.0, 1.0],
            color: tint,
            outline_color: [0.0; 4],
            face_dilate: desaturation.clamp(0.0, 1.0),
            outline_thickness: 0.0,
            underlay_offset_y: 0.0,
            underlay_softness: 0.0,
            kind: KIND_STATUS_SPRITE,
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

    /// Grow the avatar atlas to hold `slots` cells. Call before this frame reads any `avatar_uv`.
    pub fn reserve_avatars(&mut self, context: &gpu::Context, slots: usize) {
        self.avatar_atlas.reserve(context, slots);
    }

    /// Empty the avatar atlas between matches.
    pub fn reset_avatars(&mut self, context: &gpu::Context) {
        self.avatar_atlas.reset(context);
    }

    /// Copy a browser-decoded portrait into its avatar atlas slot.
    #[cfg(target_arch = "wasm32")]
    pub fn upload_avatar(
        &mut self,
        context: &gpu::Context,
        slot: usize,
        image: &web_sys::ImageBitmap,
    ) -> Result<(), String> {
        self.avatar_atlas.upload(context, slot, image)
    }

    /// UV rect for a filled avatar slot, or `None` until its image arrives.
    pub fn avatar_uv(&self, slot: usize) -> Option<[f32; 4]> {
        self.avatar_atlas.uv(slot)
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
        shader_params: &[[f32; 4]; crate::shader_params::PARAM_CAPACITY],
        context: &gpu::Context,
    ) {
        let glyph_count = self.upload_instances.len() as u32;
        if glyph_count == 0 {
            return;
        }

        self.write_buffers(context);

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
            shader_params: *shader_params,
        };

        let shader_data = TextShaderData {
            globals,
            font_atlas: self.font_atlas_tex.view,
            font_sampler: self.sampler,
            avatar_atlas: self.avatar_atlas.view,
            avatar_sampler: self.avatar_sampler,
            building_atlas: self.building_atlas_tex.view,
            building_sampler: self.building_sampler,
            status_atlas: self.status_atlas_tex.view,
            status_sampler: self.status_sampler,
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
        render_ctx.context.destroy_sampler(self.avatar_sampler);
        render_ctx.context.destroy_sampler(self.building_sampler);
        render_ctx.context.destroy_sampler(self.status_sampler);
        self.font_atlas_tex.destroy(&render_ctx.context);
        self.avatar_atlas.destroy(&render_ctx.context);
        self.building_atlas_tex.destroy(&render_ctx.context);
        self.status_atlas_tex.destroy(&render_ctx.context);
    }
}

