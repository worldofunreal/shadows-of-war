use crate::text::types::{AVATAR_CELL, AVATAR_COLS, AVATAR_MAX_ROWS, avatar_slot_uv};
use blade_graphics as gpu;

pub struct FontAtlasTexture {
    pub texture: gpu::Texture,
    pub view: gpu::TextureView,
    pub buffer: gpu::Buffer,
    pub width: u32,
    pub height: u32,
}

impl FontAtlasTexture {
    pub fn from_bytes(
        context: &gpu::Context,
        image_bytes: &[u8],
        name: &str,
        format: gpu::TextureFormat,
    ) -> Self {
        let img = image::load_from_memory(image_bytes)
            .expect("Failed to load atlas image")
            .to_rgba8();
        let (width, height) = img.dimensions();
        let bytes_per_row = width * 4;
        let total = (bytes_per_row * height) as usize;

        let buffer = context.create_buffer(gpu::BufferDesc {
            name: &format!("{}_upload_buffer", name),
            size: total as u64,
            memory: gpu::Memory::Upload,
        });
        let dst = buffer.data();
        let slice = unsafe { std::slice::from_raw_parts_mut(dst, total) };
        slice.copy_from_slice(&img);
        context.sync_buffer(buffer, 0, buffer.size());

        let texture = context.create_texture(gpu::TextureDesc {
            name: &format!("{}_texture", name),
            format,
            size: gpu::Extent {
                width,
                height,
                depth: 1,
            },
            dimension: gpu::TextureDimension::D2,
            array_layer_count: 1,
            mip_level_count: 1,
            sample_count: 1,
            usage: gpu::TextureUsage::COPY | gpu::TextureUsage::RESOURCE,
            external: None,
        });

        let view = context.create_texture_view(
            texture,
            gpu::TextureViewDesc {
                name: &format!("{}_view", name),
                format,
                dimension: gpu::ViewDimension::D2,
                subresources: &gpu::TextureSubresources::default(),
            },
        );

        Self {
            texture,
            view,
            buffer,
            width,
            height,
        }
    }

    pub fn new(context: &gpu::Context) -> Self {
        let atlas_bytes = include_bytes!("../../../assets/generated/msdf-atlas.webp");
        // ponytail: font atlas contains signed distance values (linear data), emoji atlas contains sRGB colors
        Self::from_bytes(
            context,
            atlas_bytes,
            "font_atlas",
            gpu::TextureFormat::Rgba8Unorm,
        )
    }

    pub fn upload(&self, encoder: &mut gpu::CommandEncoder, context: &gpu::Context) {
        let bytes_per_row = self.width * 4;
        context.sync_buffer(self.buffer, 0, self.buffer.size());
        let src: gpu::BufferPiece = self.buffer.into();
        let dst: gpu::TexturePiece = self.texture.into();
        let mut transfer = encoder.transfer("font_atlas_upload");
        transfer.copy_buffer_to_texture(
            src,
            bytes_per_row,
            dst,
            gpu::Extent {
                width: self.width,
                height: self.height,
                depth: 1,
            },
        );
    }
}

/// The nameplate portrait atlas. Each `AVATAR_CELL` cell is filled straight from a browser-decoded
/// image, so no copy of the atlas lives in CPU memory. It grows by whole rows, keeps every filled
/// cell until `reset`, and starts the session at one row.
pub struct AvatarAtlasTexture {
    texture: gpu::Texture,
    pub view: gpu::TextureView,
    rows: u32,
    loaded: Vec<bool>,
}

impl AvatarAtlasTexture {
    pub fn new(context: &gpu::Context) -> Self {
        let (texture, view) = Self::create(context, 1);
        Self {
            texture,
            view,
            rows: 1,
            loaded: vec![false; AVATAR_COLS as usize],
        }
    }

    fn create(context: &gpu::Context, rows: u32) -> (gpu::Texture, gpu::TextureView) {
        let format = gpu::TextureFormat::Rgba8UnormSrgb; // portraits are sRGB color
        let texture = context.create_texture(gpu::TextureDesc {
            name: "avatar_atlas_texture",
            format,
            size: gpu::Extent {
                width: AVATAR_COLS * AVATAR_CELL,
                height: rows * AVATAR_CELL,
                depth: 1,
            },
            dimension: gpu::TextureDimension::D2,
            array_layer_count: 1,
            mip_level_count: 1,
            sample_count: 1,
            usage: gpu::TextureUsage::COPY | gpu::TextureUsage::RESOURCE,
            external: None,
        });
        let view = context.create_texture_view(
            texture,
            gpu::TextureViewDesc {
                name: "avatar_atlas_view",
                format,
                dimension: gpu::ViewDimension::D2,
                subresources: &gpu::TextureSubresources::default(),
            },
        );
        (texture, view)
    }

    pub fn init(&self, encoder: &mut gpu::CommandEncoder) {
        encoder.init_texture(self.texture);
    }

    /// Make room for `slots` cells. Filled cells move into the larger texture unchanged.
    pub fn reserve(&mut self, context: &gpu::Context, slots: usize) {
        let rows = (slots.div_ceil(AVATAR_COLS as usize) as u32).clamp(1, AVATAR_MAX_ROWS);
        if rows > self.rows {
            self.rebuild(context, rows, true);
        }
    }

    /// Empty every cell for the next match and give the memory back.
    pub fn reset(&mut self, context: &gpu::Context) {
        self.rebuild(context, 1, false);
        self.loaded.fill(false);
    }

    fn rebuild(&mut self, context: &gpu::Context, rows: u32, keep_cells: bool) {
        let (texture, view) = Self::create(context, rows);
        let mut encoder = context.create_command_encoder(gpu::CommandEncoderDesc {
            name: "avatar_atlas_rebuild",
            buffer_count: 1,
            manual_barriers: false,
        });
        encoder.start();
        encoder.init_texture(texture);
        if keep_cells && self.loaded.contains(&true) {
            encoder.transfer("avatar_atlas_grow").copy_texture_to_texture(
                self.texture.into(),
                texture.into(),
                gpu::Extent {
                    width: AVATAR_COLS * AVATAR_CELL,
                    height: self.rows * AVATAR_CELL,
                    depth: 1,
                },
            );
        }
        let sync_point = context.submit(&mut encoder);
        let _ = context.wait_for(&sync_point, !0);
        context.destroy_command_encoder(&mut encoder);
        context.destroy_texture_view(self.view);
        context.destroy_texture(self.texture);
        self.texture = texture;
        self.view = view;
        self.rows = rows;
        self.loaded.resize((rows * AVATAR_COLS) as usize, false);
    }

    /// Copy one browser-decoded `AVATAR_CELL` square into `slot`.
    #[cfg(target_arch = "wasm32")]
    pub fn upload(
        &mut self,
        context: &gpu::Context,
        slot: usize,
        image: &web_sys::ImageBitmap,
    ) -> Result<(), String> {
        if slot >= self.loaded.len() {
            return Err(format!("slot {slot} is outside the {} reserved cells", self.loaded.len()));
        }
        if image.width() != AVATAR_CELL || image.height() != AVATAR_CELL {
            return Err(format!(
                "image is {}x{}, not one {AVATAR_CELL} px cell",
                image.width(),
                image.height()
            ));
        }
        let cols = AVATAR_COLS as usize;
        let origin = [
            (slot % cols) as u32 * AVATAR_CELL,
            (slot / cols) as u32 * AVATAR_CELL,
            0,
        ];
        context
            .copy_image_bitmap_to_texture(
                image,
                gpu::TexturePiece {
                    texture: self.texture,
                    mip_level: 0,
                    array_layer: 0,
                    origin,
                },
            )
            .map_err(|error| format!("{error:?}"))?;
        self.loaded[slot] = true;
        Ok(())
    }

    /// UV rect of a filled cell; `None` until its image arrives.
    pub fn uv(&self, slot: usize) -> Option<[f32; 4]> {
        self.loaded
            .get(slot)
            .copied()
            .unwrap_or(false)
            .then(|| avatar_slot_uv(slot, self.rows))
    }

    pub fn destroy(&mut self, context: &gpu::Context) {
        context.destroy_texture_view(self.view);
        context.destroy_texture(self.texture);
    }
}
