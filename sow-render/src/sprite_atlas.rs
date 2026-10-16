use blade_graphics as gpu;

/// Fixed sprite slots for GPU-instanced movers (must match WGSL / client mapping).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum MoverSpriteId {
    TransportShip = 0,
    TradeShip = 1,
    Warship = 2,
    AtomBomb = 3,
    SamMissile = 4,
}

impl MoverSpriteId {
    pub const COUNT: usize = 5;
    pub const ATLAS_CELL: u32 = 64;

    pub fn uv_rect(self) -> [f32; 4] {
        let i = self as u32;
        let cell = Self::ATLAS_CELL as f32;
        let cols = 4u32;
        let col = i % cols;
        let row = i / cols;
        let u0 = (col * Self::ATLAS_CELL) as f32 / (cols * Self::ATLAS_CELL) as f32;
        let v0 = (row * Self::ATLAS_CELL) as f32 / (2.0 * cell);
        let u1 = u0 + cell / (cols * Self::ATLAS_CELL) as f32;
        let v1 = v0 + cell / (2.0 * cell);
        [u0, v0, u1, v1]
    }
}

const UNITS_ATLAS_BYTES: &[u8] = crate::UNITS_ATLAS_BYTES;

pub struct SpriteAtlas {
    pub texture: gpu::Texture,
    pub view: gpu::TextureView,
    pub buffer: gpu::Buffer,
    pub width: u32,
    pub height: u32,
}

impl SpriteAtlas {
    pub fn new(context: &gpu::Context) -> Self {
        let cell = MoverSpriteId::ATLAS_CELL;
        let cols = 4u32;
        let rows = 2u32;
        let width = cols * cell;
        let height = rows * cell;
        let bytes_per_row = width * 4;
        let total = (bytes_per_row * height) as usize;

        let buffer = context.create_buffer(gpu::BufferDesc {
            name: "mover_atlas_upload",
            size: total as u64,
            memory: gpu::Memory::Upload,
        });
        let dst = buffer.data();
        let slice = unsafe { std::slice::from_raw_parts_mut(dst, total) };
        // assets/art/units packed 4x2 in MoverSpriteId order by pack-building-atlas.
        let atlas_img = image::load_from_memory(UNITS_ATLAS_BYTES)
            .expect("failed to load units atlas bytes")
            .to_rgba8();
        assert_eq!(
            atlas_img.dimensions(),
            (width, height),
            "units atlas must be {cols}x{rows} cells of {cell}px; run pack-building-atlas"
        );
        slice.copy_from_slice(atlas_img.as_raw());
        context.sync_buffer(buffer, 0, buffer.size());

        let texture = context.create_texture(gpu::TextureDesc {
            name: "mover_atlas",
            format: gpu::TextureFormat::Rgba8Unorm,
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
                name: "mover_atlas_view",
                format: gpu::TextureFormat::Rgba8Unorm,
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

    pub fn upload(&self, encoder: &mut gpu::CommandEncoder, context: &gpu::Context) {
        let bytes_per_row = self.width * 4;
        context.sync_buffer(self.buffer, 0, self.buffer.size());
        let src: gpu::BufferPiece = self.buffer.into();
        let dst: gpu::TexturePiece = self.texture.into();
        let mut transfer = encoder.transfer("mover_atlas_upload");
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

    pub fn destroy(&mut self, render_ctx: &crate::context::RenderContext) {
        render_ctx.context.destroy_texture_view(self.view);
        render_ctx.context.destroy_texture(self.texture);
        render_ctx.context.destroy_buffer(self.buffer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn units_atlas_has_one_cell_per_mover_sprite() {
        let cell = MoverSpriteId::ATLAS_CELL;
        let atlas = image::load_from_memory(UNITS_ATLAS_BYTES).unwrap();
        assert_eq!((atlas.width(), atlas.height()), (4 * cell, 2 * cell));
        assert!(MoverSpriteId::COUNT <= 8);
    }
}
