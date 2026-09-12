use crate::context::RenderContext;
use crate::text::msdf::FontAtlas;
use crate::text::texture::FontAtlasTexture;
use crate::text::types::{
    AVATAR_CELL, AVATAR_COLS, AVATAR_ROWS, AVATAR_SLOT_COUNT, KIND_DISC, KIND_EMOJI, KIND_GLYPH,
    KIND_RECT, KIND_RING, KIND_SPRITE, OutlineStyle, TextGlobals, TextInstanceGpu, TextPaintStyle,
    TextShaderData, avatar_slot_uv,
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

pub const MAX_TEXT_GLYPHS: usize = 32_768;

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

pub struct TextRenderer {
    pub font_atlas_desc: FontAtlas,
    pub font_atlas_tex: FontAtlasTexture,
    emoji_atlas_tex: FontAtlasTexture,
    avatar_atlas_tex: FontAtlasTexture,
    avatar_loaded: [bool; AVATAR_SLOT_COUNT],
    avatar_dirty: bool,
    pipeline: gpu::RenderPipeline,
    buffer: gpu::Buffer,
    sampler: gpu::Sampler,
    emoji_sampler: gpu::Sampler,
    avatar_sampler: gpu::Sampler,
    pub upload_instances: Vec<TextInstanceGpu>,
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

        Self {
            font_atlas_desc,
            font_atlas_tex,
            emoji_atlas_tex,
            avatar_atlas_tex,
            avatar_loaded: [false; AVATAR_SLOT_COUNT],
            avatar_dirty: true, // force first (blank) upload so the texture is defined
            pipeline,
            buffer,
            sampler,
            emoji_sampler,
            avatar_sampler,
            upload_instances: Vec::with_capacity(4096),
        }
    }

    /// Transition all atlas textures to a defined layout before first use. Must be called once
    /// (alongside `upload_atlas`) before any draw — the avatar atlas in particular is sampled by
    /// KIND_SPRITE and, like the font/terrain textures, needs init or it reads as undefined.
    pub fn init_textures(&self, encoder: &mut gpu::CommandEncoder) {
        encoder.init_texture(self.font_atlas_tex.texture);
        encoder.init_texture(self.emoji_atlas_tex.texture);
        encoder.init_texture(self.avatar_atlas_tex.texture);
    }

    pub fn upload_atlas(&self, encoder: &mut gpu::CommandEncoder, context: &gpu::Context) {
        self.font_atlas_tex.upload(encoder, context);
        self.emoji_atlas_tex.upload(encoder, context);
        self.avatar_atlas_tex.upload(encoder, context);
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

    pub fn push_string(
        &mut self,
        text: &str,
        pos: [f32; 2],
        font_size: f32,
        color: [f32; 4],
        settings: TextPaintStyle,
        layout: (f32, f32, f32),
    ) {
        let outline_color = settings.outline.color;
        let (align_x, char_spacing, emoji_scale) = layout;
        if text.is_empty() {
            return;
        }

        let scale = font_size / 48.0;
        let aw = self.font_atlas_tex.width as f32;
        let ah = self.font_atlas_tex.height as f32;
        let base = self.font_atlas_desc.atlas.common.base as f32;

        // Real zero-allocation layout: emit instances straight into the persistent
        // `upload_instances` buffer (already capacity-reserved + cleared per frame),
        // remember where this string started, then apply horizontal alignment as a
        // single in-place shift. No per-call scratch Vec — see README "Zero-Allocation
        // Hot Path". Replaces the mislabeled with_capacity(32)/(8) per-call heap allocs.
        let start = self.upload_instances.len();
        let mut x_advance = 0.0f32;
        let mut prev_char = Option::<char>::None;
        let mut chars = text.char_indices().peekable();

        while let Some((byte_idx, ch)) = chars.next() {
            if let Some(glyph) = self.font_atlas_desc.char_map.get(&ch) {
                let kern = prev_char
                    .and_then(|p| self.font_atlas_desc.kerning_map.get(&(p, ch)))
                    .copied()
                    .unwrap_or(0) as f32;
                let char_x = x_advance + (glyph.xoffset as f32 + kern) * scale;
                x_advance += (glyph.xadvance as f32 + kern) * scale * char_spacing;
                prev_char = Some(ch);
                // Disjoint field borrow: `glyph` borrows `font_atlas_desc` while we
                // push to `upload_instances` (a different field), so no intermediate
                // buffer is needed to satisfy the borrow checker.
                if self.upload_instances.len() < MAX_TEXT_GLYPHS {
                    let gw = glyph.width as f32 * scale;
                    let gh = glyph.height as f32 * scale;
                    let y_off = glyph.yoffset as f32 * scale;
                    self.upload_instances.push(TextInstanceGpu {
                        screen_pos: [pos[0] + char_x, pos[1] - base * scale + y_off],
                        size: [gw, gh],
                        uv_rect: [
                            glyph.x as f32 / aw,
                            glyph.y as f32 / ah,
                            (glyph.x + glyph.width) as f32 / aw,
                            (glyph.y + glyph.height) as f32 / ah,
                        ],
                        content_rect: [0.0, 0.0, 1.0, 1.0],
                        color,
                        outline_color,
                        face_dilate: settings.face_dilate,
                        outline_thickness: settings.outline.thickness,
                        underlay_offset_y: settings.outline.shadow_y,
                        underlay_softness: settings.underlay_softness,
                        kind: KIND_GLYPH,
                    });
                }
                continue;
            }
            let has_selector = chars
                .peek()
                .is_some_and(|&(_, next_ch)| next_ch == '\u{fe0f}');
            let char_len = ch.len_utf8();
            let total_len = if has_selector {
                char_len + '\u{fe0f}'.len_utf8()
            } else {
                char_len
            };
            let candidate = &text[byte_idx..byte_idx + total_len];
            let stripped = if has_selector {
                &text[byte_idx..byte_idx + char_len]
            } else {
                candidate
            };
            if let Some(uv) = emoji_uv_opt(stripped) {
                let emoji_size = font_size * emoji_scale;
                let advance = x_advance;
                x_advance += emoji_size * char_spacing;
                prev_char = None;
                if self.upload_instances.len() < MAX_TEXT_GLYPHS {
                    let center = [
                        pos[0] + advance + emoji_size * 0.5,
                        pos[1] - emoji_size * 0.5,
                    ];
                    self.upload_instances.push(emoji_instance(
                        uv,
                        center,
                        emoji_size * 0.5,
                        color,
                        settings.outline,
                        settings.underlay_softness,
                    ));
                }
                if has_selector {
                    chars.next();
                }
                continue;
            }
            prev_char = None;
        }

        // Alignment is one cheap in-place pass over the instances we just emitted,
        // replacing the old second buffer-building loop.
        let align_offset = x_advance * align_x;
        if align_offset != 0.0 {
            for inst in &mut self.upload_instances[start..] {
                inst.screen_pos[0] -= align_offset;
            }
        }
    }

    /// Measure the rendered width of `text` in the same units as `font_size`, using the exact
    /// advance math `push_string` emits (glyph xadvance + kerning; emoji = `font_size * emoji_scale`;
    /// everything scaled by `char_spacing`). Lets callers size text boxes from the real GPU layout
    /// instead of an egui galley. Keep this loop in lockstep with `push_string`'s advance path.
    pub fn measure_string(
        &self,
        text: &str,
        font_size: f32,
        char_spacing: f32,
        emoji_scale: f32,
    ) -> f32 {
        if text.is_empty() {
            return 0.0;
        }
        let scale = font_size / 48.0;
        let mut x_advance = 0.0f32;
        let mut prev_char = Option::<char>::None;
        let mut chars = text.char_indices().peekable();

        while let Some((byte_idx, ch)) = chars.next() {
            if let Some(glyph) = self.font_atlas_desc.char_map.get(&ch) {
                let kern = prev_char
                    .and_then(|p| self.font_atlas_desc.kerning_map.get(&(p, ch)))
                    .copied()
                    .unwrap_or(0) as f32;
                x_advance += (glyph.xadvance as f32 + kern) * scale * char_spacing;
                prev_char = Some(ch);
                continue;
            }
            let has_selector = chars
                .peek()
                .is_some_and(|&(_, next_ch)| next_ch == '\u{fe0f}');
            let char_len = ch.len_utf8();
            let stripped = &text[byte_idx..byte_idx + char_len];
            if emoji_uv_opt(stripped).is_some() {
                x_advance += font_size * emoji_scale * char_spacing;
                prev_char = None;
                if has_selector {
                    chars.next();
                }
                continue;
            }
            prev_char = None;
        }

        x_advance
    }

    /// Push a screen-space emoji with alpha-dilated outline + drop shadow.
    /// `screen_pos` is the center in physical pixels, `half_size` the logical content half-extent;
    /// the submitted quad grows only by the effect padding.
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
    /// `radius`/`thickness` are physical pixels.
    pub fn push_ring(&mut self, center: [f32; 2], radius: f32, color: [f32; 4], thickness: f32) {
        self.push_inst(TextInstanceGpu {
            screen_pos: [center[0] - radius, center[1] - radius],
            size: [radius * 2.0, radius * 2.0],
            uv_rect: [0.0, 0.0, 1.0, 1.0],
            content_rect: [0.0, 0.0, 1.0, 1.0],
            color,
            outline_color: [0.0; 4],
            face_dilate: 0.0,
            outline_thickness: thickness,
            underlay_offset_y: 0.0,
            underlay_softness: 0.0,
            kind: KIND_RING,
        });
    }

    /// Push a circle-clipped image sprite from the avatar atlas. `center`/`radius` are physical
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
            face_dilate: 0.0,
            outline_thickness: 0.0,
            underlay_offset_y: 0.0,
            underlay_softness: 0.0,
            kind: KIND_SPRITE,
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
        self.avatar_loaded[slot] = true;
        self.avatar_dirty = true;
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

        // Flush any newly-arrived avatar portraits into the atlas before sampling them.
        if self.avatar_dirty {
            self.avatar_atlas_tex.upload(encoder, context);
            self.avatar_dirty = false;
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emoji_geometry_keeps_logical_size_and_reserves_effect_room() {
        let outline = OutlineStyle {
            color: [0.0, 0.0, 0.0, 1.0],
            thickness: 3.0,
            shadow_y: 5.0,
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
    fn inline_emoji_inherits_the_configured_outline() {
        let outline = OutlineStyle {
            color: [0.0, 0.0, 0.0, 0.8],
            thickness: 2.25,
            shadow_y: 3.5,
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
        assert_eq!(instance.outline_thickness, outline.thickness);
        assert_eq!(instance.underlay_offset_y, outline.shadow_y);
        assert_eq!(instance.underlay_softness, 0.25);
    }
}
