use super::cpu_prep::{
    MapGlobals, PlayerColors, PlayerSkinStyles, compute_has_border, compute_has_border_with,
    fill_terrain_buffer, get_neighbors_8,
};
use super::crater::CraterRevealQueue;
use super::tabletop::{Tabletop, TabletopVertex};
use std::sync::Arc;
use crate::context::RenderContext;
use crate::text::FontAtlasTexture;
use blade_graphics as gpu;
use blade_macros::ShaderData;

#[derive(ShaderData)]
struct MapShaderData {
    globals: MapGlobals,
    player_colors: PlayerColors,
    player_skin_styles: PlayerSkinStyles,
    terrain_texture: gpu::TextureView,
    owner_texture: gpu::TextureView,
    skin_texture: gpu::TextureView,
    terrain_art_texture: gpu::TextureView,
    terrain_tile_0_texture: gpu::TextureView,
    terrain_tile_1_texture: gpu::TextureView,
    terrain_tile_2_texture: gpu::TextureView,
    terrain_tile_3_texture: gpu::TextureView,
    skin_sampler: gpu::Sampler,
    clock_texture: gpu::TextureView,
}

struct TabletopDraw {
    projection: Arc<Tabletop>,
    pipeline: gpu::RenderPipeline,
    vertices: gpu::Buffer,
    indices: gpu::Buffer,
    index_count: u32,
}

pub struct MapRenderer {
    pub terrain_texture: gpu::Texture,
    pub terrain_view: gpu::TextureView,
    pub terrain_buffer: gpu::Buffer,
    pub owner_texture: gpu::Texture,
    pub owner_view: gpu::TextureView,
    pub owner_buffer: gpu::Buffer,
    skin_texture: gpu::Texture,
    skin_view: gpu::TextureView,
    skin_buffer: gpu::Buffer,
    skin_sampler: gpu::Sampler,
    terrain_art: Option<FontAtlasTexture>,
    terrain_art_key: Option<String>,
    terrain_tiles: Option<(String, Vec<FontAtlasTexture>)>,
    pub pipeline: gpu::RenderPipeline,
    tabletop: Option<TabletopDraw>,
    surface_format: gpu::TextureFormat,
    pub width: u32,
    pub height: u32,
    pub terrain: Vec<u8>,
    pub owners: Vec<u16>,
    /// Per-tile visual clocks (Rg32Uint): x = flash start tick (bit 31 = ability),
    /// y = tick the tile became hidden (0 = visible, `FADE_HIDDEN_SENTINEL` = dark
    /// from the first frame). The buffer is host-mapped, so it is also the CPU's
    /// record of the current state; no separate mirror vec is kept.
    pub clock_texture: gpu::Texture,
    pub clock_view: gpu::TextureView,
    pub clock_buffer: gpu::Buffer,
    pub clock_bytes_per_row: u32,
    pub dirty_clock_chunks: Vec<bool>,
    pub flash_epoch: web_time::Instant,
    /// Tick used by the most recent update; mirrored into `MapGlobals` for the shader.
    pub flash_now_tick: u32,
    /// Conquest flash length in ticks; the fragment shader fades over it.
    pub flash_duration_ticks: f32,
    last_visible_blocks: Vec<u64>,
    pub terrain_bytes_per_row: u32,
    pub owner_bytes_per_row: u32,
    pub chunk_h: u32,
    pub dirty_chunks: Vec<bool>,
    pub last_update: Option<web_time::Instant>,
    pub has_water_neighbor: Vec<bool>,
    crater_reveals: CraterRevealQueue,
    revealed_scratch: Vec<u32>,
    pack_stamp: Vec<u32>,
    current_pack_stamp: u32,
}

/// Visual clock rate for the GPU-side flash and fog-fade decay.
pub const FLASH_TICK_HZ: f64 = 64.0;
/// High bit of the flash texel marks a hero-ability flash (localized shader pulse).
const FLASH_ABILITY_BIT: u32 = 0x8000_0000;
/// Fade texel value for a tile that was already dark on the first frame: it must
/// not fade in when the map appears. Any real hidden tick is far below this.
const FADE_HIDDEN_SENTINEL: u32 = u32::MAX;

// Sixteen 64px columns hold the store silhouettes (styles 10-25).
const SKIN_ATLAS_SIZE: gpu::Extent = gpu::Extent {
    width: 1024,
    height: 64,
    depth: 1,
};
const SKIN_ATLAS: &[u8; 1024 * 64] = include_bytes!("territory_skins.r8");

impl MapRenderer {
    pub fn new(
        context: &gpu::Context,
        width: u32,
        height: u32,
        surface_format: gpu::TextureFormat,
        initial_terrain: &[u8],
    ) -> Self {
        // Rgba8Unorm: 4 bytes per texel, row alignment to 256 bytes
        let terrain_bytes_per_row = (width * 4 + 255) & !255;
        // R32Uint: 4 bytes per texel, row alignment to 256 bytes
        let owner_bytes_per_row = (width * 4 + 255) & !255;

        // --- Terrain texture (Rgba8Unorm, static) ---
        let terrain_texture = context.create_texture(gpu::TextureDesc {
            name: "terrain_map",
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
        let terrain_view = context.create_texture_view(
            terrain_texture,
            gpu::TextureViewDesc {
                name: "terrain_map_view",
                format: gpu::TextureFormat::Rgba8Unorm,
                dimension: gpu::ViewDimension::D2,
                subresources: &gpu::TextureSubresources::default(),
            },
        );
        let terrain_buffer = context.create_buffer(gpu::BufferDesc {
            name: "terrain_raw",
            size: (terrain_bytes_per_row * height) as u64,
            memory: gpu::Memory::Upload,
        });

        // Compute static has_water_neighbor list
        let total = (width * height) as usize;
        let mut has_water_neighbor = vec![false; total];
        for idx in 0..total as u32 {
            let terrain_byte = initial_terrain[idx as usize];
            let is_land = (terrain_byte & 0x80) != 0;
            if is_land {
                // Include diagonal neighbors so reflex 90° coastline corners pass `has_water_neighbor`.
                let neighbors = get_neighbors_8(idx, width, height);
                let mut water = false;
                for &n_opt in &neighbors {
                    if let Some(n) = n_opt {
                        let n_terrain = initial_terrain[n as usize];
                        let n_is_land = (n_terrain & 0x80) != 0;
                        if !n_is_land {
                            water = true;
                            break;
                        }
                    } else {
                        // Map edge has out-of-bounds neighbor (treated as water/non-land)
                        water = true;
                        break;
                    }
                }
                has_water_neighbor[idx as usize] = water;
            }
        }

        // Fill terrain buffer with RGBA8 terrain bytes (R: terrain, G: normal dx, B: normal dy, A: CPU noise seed)
        let terrain_total = (terrain_bytes_per_row * height) as usize;
        let terrain_ptr = terrain_buffer.data();
        let terrain_slice = unsafe { std::slice::from_raw_parts_mut(terrain_ptr, terrain_total) };
        fill_terrain_buffer(
            initial_terrain,
            width,
            height,
            terrain_bytes_per_row,
            terrain_slice,
        );
        context.sync_buffer(terrain_buffer, 0, terrain_buffer.size());

        // --- Owner texture (R32Uint, dynamic) ---
        // Bits 0..15 = owner_id, bits 16..23 = conquest flash
        let owner_texture = context.create_texture(gpu::TextureDesc {
            name: "owner_map",
            format: gpu::TextureFormat::R32Uint,
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
        let owner_view = context.create_texture_view(
            owner_texture,
            gpu::TextureViewDesc {
                name: "owner_map_view",
                format: gpu::TextureFormat::R32Uint,
                dimension: gpu::ViewDimension::D2,
                subresources: &gpu::TextureSubresources::default(),
            },
        );
        let owner_buffer = context.create_buffer(gpu::BufferDesc {
            name: "owner_raw",
            size: (owner_bytes_per_row * height) as u64,
            memory: gpu::Memory::Upload,
        });

        let skin_texture = context.create_texture(gpu::TextureDesc {
            name: "territory_skins",
            format: gpu::TextureFormat::R8Unorm,
            size: SKIN_ATLAS_SIZE,
            dimension: gpu::TextureDimension::D2,
            array_layer_count: 1,
            mip_level_count: 1,
            sample_count: 1,
            usage: gpu::TextureUsage::COPY | gpu::TextureUsage::RESOURCE,
            external: None,
        });
        let skin_view = context.create_texture_view(
            skin_texture,
            gpu::TextureViewDesc {
                name: "territory_skins_view",
                format: gpu::TextureFormat::R8Unorm,
                dimension: gpu::ViewDimension::D2,
                subresources: &gpu::TextureSubresources::default(),
            },
        );
        let skin_buffer = context.create_buffer(gpu::BufferDesc {
            name: "territory_skins_upload",
            size: SKIN_ATLAS.len() as u64,
            memory: gpu::Memory::Upload,
        });
        unsafe {
            std::ptr::copy_nonoverlapping(SKIN_ATLAS.as_ptr(), skin_buffer.data(), SKIN_ATLAS.len());
        }
        context.sync_buffer(skin_buffer, 0, skin_buffer.size());
        let skin_sampler = context.create_sampler(gpu::SamplerDesc {
            name: "territory_skins_sampler",
            min_filter: gpu::FilterMode::Linear,
            mag_filter: gpu::FilterMode::Linear,
            ..Default::default()
        });
        // --- Shader & pipeline ---
        let source =
            crate::shader_params::prepare_shader("MAP_", include_str!("../shaders/map.wgsl"));
        let shader = context.create_shader(gpu::ShaderDesc {
            source: &source,
            naga_module: None,
        });
        assert_eq!(
            std::mem::size_of::<MapGlobals>(),
            shader.get_struct_size("Globals") as usize,
            "MapGlobals must match WGSL `struct Globals` uniform layout"
        );
        assert_eq!(
            std::mem::size_of::<PlayerColors>(),
            shader.get_struct_size("PlayerColors") as usize,
            "PlayerColors must match WGSL `struct PlayerColors` uniform layout"
        );
        assert_eq!(
            std::mem::size_of::<PlayerSkinStyles>(),
            shader.get_struct_size("PlayerSkinStyles") as usize,
            "PlayerSkinStyles must match WGSL `struct PlayerSkinStyles` uniform layout"
        );

        let pipeline = Self::pipeline(context, &shader, surface_format, "vs_main", &[]);

        let chunk_h = 32;
        let num_chunks = height.div_ceil(chunk_h);

        // Per-tile visual clocks (Rg32Uint): x = flash start tick, y = fog hidden
        // tick. 8 bytes per texel, rows aligned to 256 bytes. Ticks are full u32 so
        // a tile that stays hidden for a whole match never wraps back to "visible".
        let clock_bytes_per_row = (width * 8 + 255) & !255;
        let clock_texture = context.create_texture(gpu::TextureDesc {
            name: "tile_clocks",
            format: gpu::TextureFormat::Rg32Uint,
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
        let clock_view = context.create_texture_view(
            clock_texture,
            gpu::TextureViewDesc {
                name: "tile_clocks_view",
                format: gpu::TextureFormat::Rg32Uint,
                dimension: gpu::ViewDimension::D2,
                subresources: &gpu::TextureSubresources::default(),
            },
        );
        let clock_buffer = context.create_buffer(gpu::BufferDesc {
            name: "tile_clocks_raw",
            size: u64::from(clock_bytes_per_row * height),
            memory: gpu::Memory::Upload,
        });
        // Zero means "no flash / fully visible", which is the idle state.
        unsafe {
            std::ptr::write_bytes(clock_buffer.data(), 0, (clock_bytes_per_row * height) as usize)
        };
        context.sync_buffer(clock_buffer, 0, clock_buffer.size());

        Self {
            terrain_texture,
            terrain_view,
            terrain_buffer,
            owner_texture,
            owner_view,
            owner_buffer,
            skin_texture,
            skin_view,
            skin_buffer,
            skin_sampler,
            terrain_art: None,
            terrain_art_key: None,
            terrain_tiles: None,
            pipeline,
            tabletop: None,
            surface_format,
            width,
            height,
            terrain: initial_terrain.to_vec(),
            owners: vec![0; total],
            clock_texture,
            clock_view,
            clock_buffer,
            clock_bytes_per_row,
            dirty_clock_chunks: vec![false; num_chunks as usize],
            flash_epoch: web_time::Instant::now(),
            flash_now_tick: 0,
            flash_duration_ticks: 1.0,
            last_visible_blocks: Vec::new(),
            terrain_bytes_per_row,
            owner_bytes_per_row,
            chunk_h,
            dirty_chunks: vec![false; num_chunks as usize],
            last_update: None,
            has_water_neighbor,
            crater_reveals: CraterRevealQueue::new(),
            revealed_scratch: Vec::new(),
            pack_stamp: vec![0; total],
            current_pack_stamp: 0,
        }
    }

    fn pipeline(
        context: &gpu::Context,
        shader: &gpu::Shader,
        surface_format: gpu::TextureFormat,
        vertex_entry: &str,
        vertex_fetches: &[gpu::VertexFetchState<'_>],
    ) -> gpu::RenderPipeline {
        let layout = <MapShaderData as gpu::ShaderData>::layout();
        // Fragment entry contract: Linear swapchains use `fs_main`; plain UNORM (wasm WebGL
        // canvas) uses `fs_main_srgb`. Match on the Blade surface format.
        let fragment_entry = if matches!(surface_format, gpu::TextureFormat::Rgba8Unorm) {
            "fs_main_srgb"
        } else {
            "fs_main"
        };
        context.create_render_pipeline(gpu::RenderPipelineDesc {
            name: "map_pipeline",
            data_layouts: &[&layout],
            vertex: shader.at(vertex_entry),
            vertex_fetches,
            primitive: gpu::PrimitiveState {
                topology: gpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            fragment: Some(shader.at(fragment_entry)),
            color_targets: &[gpu::ColorTargetState {
                format: surface_format,
                blend: None,
                write_mask: gpu::ColorWrites::default(),
            }],
            multisample_state: gpu::MultisampleState::default(),
        })
    }

    pub fn tabletop_projection(&mut self, context: &gpu::Context) -> Arc<Tabletop> {
        let draw = self.tabletop.get_or_insert_with(|| {
            let projection = Arc::new(Tabletop::new(self.width, self.height, &self.terrain));
            let (vertices, indices) = projection.mesh();
            let upload = |name, bytes: &[u8]| {
                let buffer = context.create_buffer(gpu::BufferDesc {
                    name,
                    size: bytes.len() as u64,
                    memory: gpu::Memory::Upload,
                });
                // Upload buffers are host-mapped by Blade.
                unsafe {
                    std::ptr::copy_nonoverlapping(bytes.as_ptr(), buffer.data().cast(), bytes.len());
                }
                context.sync_buffer(buffer, 0, buffer.size());
                buffer
            };
            let source = crate::shader_params::prepare_shader("MAP_", include_str!("../shaders/map.wgsl"));
            let shader = context.create_shader(gpu::ShaderDesc { source: &source, naga_module: None });
            let pipeline = Self::pipeline(
                context,
                &shader,
                self.surface_format,
                "vs_tabletop",
                &[gpu::VertexFetchState {
                    layout: &<TabletopVertex as gpu::Vertex>::layout(),
                    instanced: false,
                }],
            );
            TabletopDraw {
                projection,
                pipeline,
                vertices: upload("tabletop_vertices", bytemuck::cast_slice(&vertices)),
                indices: upload("tabletop_indices", bytemuck::cast_slice(&indices)),
                index_count: indices.len() as u32,
            }
        });
        Arc::clone(&draw.projection)
    }

    /// Queue an outward-expanding visual reveal for a newly detonated nuke crater.
    /// Authoritative `self.owners` still updates immediately on the detonation tick.
    pub fn enqueue_crater_reveal(
        &mut self,
        cx: u32,
        cy: u32,
        inner_radius: u32,
        duration: f32,
        now: web_time::Instant,
    ) {
        self.crater_reveals
            .enqueue(cx, cy, inner_radius, duration, now, self.width, self.height);
    }

    /// Clear any active crater reveal animations.
    pub fn clear_crater_reveals(&mut self) {
        self.crater_reveals.clear();
        self.revealed_scratch.clear();
    }

    /// Upload static terrain texture once. Call after creating the command encoder.
    pub fn upload_terrain(&self, encoder: &mut gpu::CommandEncoder) {
        encoder.init_texture(self.skin_texture);
        encoder.init_texture(self.clock_texture);
        let src_piece: gpu::BufferPiece = self.terrain_buffer.into();
        let dst_piece: gpu::TexturePiece = self.terrain_texture.into();
        let mut transfer = encoder.transfer("terrain_upload");
        transfer.copy_buffer_to_texture(
            src_piece,
            self.terrain_bytes_per_row,
            dst_piece,
            gpu::Extent {
                width: self.width,
                height: self.height,
                depth: 1,
            },
        );
        transfer.copy_buffer_to_texture(
            self.skin_buffer.into(),
            SKIN_ATLAS_SIZE.width,
            self.skin_texture.into(),
            SKIN_ATLAS_SIZE,
        );
        transfer.copy_buffer_to_texture(
            self.clock_buffer.into(),
            self.clock_bytes_per_row,
            self.clock_texture.into(),
            gpu::Extent {
                width: self.width,
                height: self.height,
                depth: 1,
            },
        );
    }

    /// Upload the baked terrain art once per map key.
    pub fn load_terrain_art(
        &mut self,
        context: &gpu::Context,
        encoder: &mut gpu::CommandEncoder,
        key: &str,
        art: &image::RgbaImage,
    ) {
        if self.terrain_art.is_some() && self.terrain_art_key.as_deref() == Some(key) {
            return;
        }
        if let Some(previous) = self.terrain_art.take() {
            previous.destroy(context);
        }
        let format = gpu::TextureFormat::Rgba8Unorm;
        let texture = FontAtlasTexture::from_rgba(context, art, "terrain_art", format);
        encoder.init_texture(texture.texture);
        texture.upload(encoder, context);
        self.terrain_art = Some(texture);
        self.terrain_art_key = Some(key.to_string());
    }

    /// Upload four 2x2 campaign terrain art tiles as individual 2D textures.
    pub fn load_terrain_tiles(
        &mut self,
        context: &gpu::Context,
        encoder: &mut gpu::CommandEncoder,
        key: &str,
        tiles: &[image::RgbaImage],
    ) {
        if self
            .terrain_tiles
            .as_ref()
            .is_some_and(|(loaded_key, _)| loaded_key == key)
        {
            return;
        }
        assert_eq!(tiles.len(), 4, "campaign terrain needs four 2x2 tiles");
        let (width, height) = tiles[0].dimensions();
        assert!(
            tiles.iter().all(|tile| tile.dimensions() == (width, height)),
            "terrain tile dimensions must match"
        );
        let textures = tiles
            .iter()
            .enumerate()
            .map(|(index, tile)| {
                let texture = FontAtlasTexture::from_rgba(
                    context,
                    tile,
                    &format!("terrain_tile_{index}"),
                    gpu::TextureFormat::Rgba8Unorm,
                );
                encoder.init_texture(texture.texture);
                texture.upload(encoder, context);
                texture
            })
            .collect();
        self.clear_terrain_tiles(context);
        self.terrain_tiles = Some((key.to_string(), textures));
    }

    pub fn clear_terrain_tiles(&mut self, context: &gpu::Context) {
        if let Some((_, tiles)) = self.terrain_tiles.take() {
            for tile in tiles {
                tile.destroy(context);
            }
        }
    }

    /// Write dirty ownership tiles to the upload buffer and copy to GPU.
    pub fn update(
        &mut self,
        encoder: &mut gpu::CommandEncoder,
        context: &gpu::Context,
        dirty_tiles: &[sow_core::protocol::DirtyTile],
        conquest_duration: f32,
        my_player_id: u16,
        visible: &sow_core::bitset::DenseBitSet,
        force_full_upload: bool,
    ) {
        let now = web_time::Instant::now();
        self.last_update = Some(now);
        let now_tick = self.tick_at(now);
        self.flash_now_tick = now_tick;
        // The shader fades a flash over this many ticks of the shared clock.
        self.flash_duration_ticks =
            (conquest_duration.max(0.05) * FLASH_TICK_HZ as f32).clamp(1.0, 30_000.0);

        let total = (self.width * self.height) as usize;
        let u32_per_row = self.owner_bytes_per_row / 4;
        let total_u32 = (u32_per_row * self.height) as usize;
        let width = self.width;
        let height = self.height;
        let chunk_h = self.chunk_h;

        let dst_ptr = self.owner_buffer.data();
        let slice = unsafe { std::slice::from_raw_parts_mut(dst_ptr as *mut u32, total_u32) };

        // Host-mapped visual-clock buffer: 2 u32 per tile (flash start, fog hidden).
        let clock_u32_per_row = self.clock_bytes_per_row / 4;
        let clock_total_u32 = (clock_u32_per_row * height) as usize;
        let clock = unsafe {
            std::slice::from_raw_parts_mut(self.clock_buffer.data() as *mut u32, clock_total_u32)
        };

        // Pack owner + border/water flags into the GPU buffer. Flash and fog fade
        // live in their own textures, so a tile that is merely darkening is never
        // repacked or re-uploaded. Uses visual owners only while a nuke crater is
        // expanding (0 idle cost).
        let pack = |slice: &mut [u32],
                    owners: &[u16],
                    crater_reveals: &CraterRevealQueue,
                    tile_idx: u32,
                    has_water: bool| {
            let i = tile_idx as usize;
            let x = tile_idx % width;
            let y = tile_idx / width;
            let dst = (y * u32_per_row + x) as usize;

            let (owner, has_border) = if crater_reveals.is_active() {
                (
                    crater_reveals.visual_owner(tile_idx, owners, width),
                    compute_has_border_with(
                        tile_idx,
                        |n| crater_reveals.visual_owner(n, owners, width),
                        width,
                        height,
                    ),
                )
            } else {
                (owners[i], compute_has_border(tile_idx, owners, width, height))
            };

            let mut val = owner as u32;
            if has_border {
                val |= 1 << 24;
            }
            if has_water {
                val |= 1 << 25;
            }
            slice[dst] = val;
        };

        let has_water = &self.has_water_neighbor;

        // Pre-apply dirty tiles to self.owners and set conquest flash at the start
        // so both the full upload path and decay updates use the correct owners.
        for dt in dirty_tiles {
            let i = dt.index as usize;
            if i >= total {
                continue;
            }
            let old_owner = self.owners[i];
            if old_owner != dt.new_owner {
                let held_for_crater = self.crater_reveals.intercept_dirty_tile(
                    dt.index,
                    old_owner,
                    dt.new_owner,
                    width,
                );
                if !held_for_crater
                    && conquest_involves_player(old_owner, dt.new_owner, my_player_id)
                {
                    start_flash_at(
                        clock,
                        clock_u32_per_row,
                        &mut self.dirty_clock_chunks,
                        dt.index,
                        now_tick,
                        false,
                        chunk_h,
                        width,
                    );
                }
                self.owners[i] = dt.new_owner;
            }
        }
        self.crater_reveals.finalize_pending();
        self.crater_reveals
            .advance(now, width, &mut self.revealed_scratch);

        // 1. Reset owner chunk tracking. Flash/fade chunks stay dirty until their
        //    upload runs, so an ability flash set between frames is not lost.
        self.dirty_chunks.fill(false);

        // 2. Vision: only the fade clock changes, so the owner texture is untouched.
        let full_vis_upload =
            force_full_upload || self.last_visible_blocks.len() != visible.blocks.len();
        let initial_vis = self.last_visible_blocks.is_empty();
        if full_vis_upload {
            for tile_idx in 0..total as u32 {
                set_tile_visibility(
                    clock,
                    clock_u32_per_row,
                    &mut self.dirty_clock_chunks,
                    tile_idx,
                    visible.contains(tile_idx),
                    now_tick,
                    initial_vis,
                    chunk_h,
                    width,
                );
            }
            self.last_visible_blocks.resize(visible.blocks.len(), 0);
            self.last_visible_blocks.copy_from_slice(&visible.blocks);
        } else {
            for (block_idx, (&new_word, old_word)) in visible
                .blocks
                .iter()
                .zip(self.last_visible_blocks.iter_mut())
                .enumerate()
            {
                let mut diff = new_word ^ *old_word;
                if diff == 0 {
                    continue;
                }
                *old_word = new_word;
                let base_tile = (block_idx as u32) * 64;
                while diff != 0 {
                    let bit = diff.trailing_zeros();
                    diff &= diff - 1;
                    let tile_idx = base_tile + bit;
                    if tile_idx >= total as u32 {
                        break;
                    }
                    set_tile_visibility(
                        clock,
                        clock_u32_per_row,
                        &mut self.dirty_clock_chunks,
                        tile_idx,
                        ((new_word >> bit) & 1) != 0,
                        now_tick,
                        false,
                        chunk_h,
                        width,
                    );
                }
            }
        }

        if full_vis_upload {
            for tile_idx in 0..total as u32 {
                pack(
                    slice,
                    &self.owners,
                    &self.crater_reveals,
                    tile_idx,
                    has_water[tile_idx as usize],
                );
            }
            context.sync_buffer(self.owner_buffer, 0, self.owner_buffer.size());
            let src_piece: gpu::BufferPiece = self.owner_buffer.into();
            let dst_piece: gpu::TexturePiece = self.owner_texture.into();
            {
                let mut transfer = encoder.transfer("owner_upload_full");
                transfer.copy_buffer_to_texture(
                    src_piece,
                    self.owner_bytes_per_row,
                    dst_piece,
                    gpu::Extent {
                        width: self.width,
                        height: self.height,
                        depth: 1,
                    },
                );
            }
            upload_rows(
                context,
                encoder,
                &mut self.dirty_clock_chunks,
                self.clock_buffer,
                self.clock_texture,
                width,
                height,
                chunk_h,
                self.clock_bytes_per_row,
                8,
                "clock_upload",
            );
            return;
        }

        // 3. Pack new dirty tiles (skipping crater tiles still waiting for the blast front)
        //    plus any crater tiles newly uncovered this frame, and their 8 neighbors.
        //    Stamp tiles so interior cells touched by multiple neighbors pack at most once.
        if !dirty_tiles.is_empty() || !self.revealed_scratch.is_empty() {
            self.current_pack_stamp = self.current_pack_stamp.wrapping_add(1);
            if self.current_pack_stamp == 0 {
                self.pack_stamp.fill(0);
                self.current_pack_stamp = 1;
            }
            let stamp = self.current_pack_stamp;
            let pack_stamp = &mut self.pack_stamp;
            let mut pack_with_neighbors = |tile_idx: u32, dirty_chunks: &mut [bool]| {
                let i = tile_idx as usize;
                if i >= total {
                    return;
                }
                if pack_stamp[i] != stamp {
                    pack_stamp[i] = stamp;
                    pack(
                        slice,
                        &self.owners,
                        &self.crater_reveals,
                        tile_idx,
                        self.has_water_neighbor[i],
                    );
                    let y = tile_idx / width;
                    dirty_chunks[(y / chunk_h) as usize] = true;
                }

                let neighbors = get_neighbors_8(tile_idx, width, height);
                for &n_opt in &neighbors {
                    if let Some(n_idx) = n_opt {
                        let ni = n_idx as usize;
                        if pack_stamp[ni] != stamp {
                            pack_stamp[ni] = stamp;
                            pack(
                                slice,
                                &self.owners,
                                &self.crater_reveals,
                                n_idx,
                                self.has_water_neighbor[ni],
                            );
                            let n_y = n_idx / width;
                            dirty_chunks[(n_y / chunk_h) as usize] = true;
                        }
                    }
                }
            };

            for dt in dirty_tiles {
                if self.crater_reveals.is_tile_held(dt.index, width) {
                    continue;
                }
                pack_with_neighbors(dt.index, &mut self.dirty_chunks);
            }
            for &tile_idx in &self.revealed_scratch {
                pack_with_neighbors(tile_idx, &mut self.dirty_chunks);
            }
        }

        // Upload the chunks each texture actually dirtied. A frame where only a
        // flash is fading uploads 2 bytes per tile instead of a repacked owner row.
        upload_rows(
            context,
            encoder,
            &mut self.dirty_chunks,
            self.owner_buffer,
            self.owner_texture,
            width,
            height,
            chunk_h,
            self.owner_bytes_per_row,
            4,
            "owner_upload",
        );
        upload_rows(
            context,
            encoder,
            &mut self.dirty_clock_chunks,
            self.clock_buffer,
            self.clock_texture,
            width,
            height,
            chunk_h,
            self.clock_bytes_per_row,
            8,
            "clock_upload",
        );
    }

    /// Monotonic 64 Hz tick shared with the fragment shader's decay. Full u32 so a
    /// tile hidden for a whole match never wraps back to "visible".
    fn tick_at(&self, now: web_time::Instant) -> u32 {
        let secs = now.duration_since(self.flash_epoch).as_secs_f64();
        (secs * FLASH_TICK_HZ) as u32
    }

    /// Starts the per-tile flash for a locally visible hero ability.
    pub fn flash_ability_tile(&mut self, x: u32, y: u32) {
        if x >= self.width || y >= self.height {
            return;
        }
        let tile = y * self.width + x;
        let now_tick = self.tick_at(web_time::Instant::now());
        self.flash_now_tick = now_tick;
        let clock_u32_per_row = self.clock_bytes_per_row / 4;
        let clock_total_u32 = (clock_u32_per_row * self.height) as usize;
        let clock = unsafe {
            std::slice::from_raw_parts_mut(self.clock_buffer.data() as *mut u32, clock_total_u32)
        };
        start_flash_at(
            clock,
            clock_u32_per_row,
            &mut self.dirty_clock_chunks,
            tile,
            now_tick,
            true,
            self.chunk_h,
            self.width,
        );
    }

    pub fn draw(
        &self,
        encoder: &mut gpu::CommandEncoder,
        target_view: gpu::TextureView,
        globals: MapGlobals,
        player_colors: PlayerColors,
        player_skin_styles: PlayerSkinStyles,
        tabletop: bool,
    ) {
        let mut pass = encoder.render(
            "map_pass",
            gpu::RenderTargetSet {
                colors: &[gpu::RenderTarget {
                    view: target_view,
                    init_op: gpu::InitOp::Clear(gpu::TextureColor::OpaqueBlack),
                    finish_op: gpu::FinishOp::Store,
                }],
                depth_stencil: None,
            },
        );
        let tabletop = self.tabletop.as_ref().filter(|_| tabletop);
        let mut rc = pass.with(tabletop.map_or(&self.pipeline, |draw| &draw.pipeline));
        let terrain_art_view = self
            .terrain_art
            .as_ref()
            .map_or(self.terrain_view, |texture| texture.view);
        let terrain_tiles = self
            .terrain_tiles
            .as_ref()
            .map_or(&[][..], |(_, tiles)| tiles.as_slice());
        let terrain_tile_view = |index: usize| {
            terrain_tiles
                .get(index)
                .map_or(terrain_art_view, |tile| tile.view)
        };
        rc.bind(
            0,
            &MapShaderData {
                globals,
                player_colors,
                player_skin_styles,
                terrain_texture: self.terrain_view,
                owner_texture: self.owner_view,
                skin_texture: self.skin_view,
                terrain_art_texture: terrain_art_view,
                terrain_tile_0_texture: terrain_tile_view(0),
                terrain_tile_1_texture: terrain_tile_view(1),
                terrain_tile_2_texture: terrain_tile_view(2),
                terrain_tile_3_texture: terrain_tile_view(3),
                skin_sampler: self.skin_sampler,
                clock_texture: self.clock_view,
            },
        );
        if let Some(draw) = tabletop {
            rc.bind_vertex(0, draw.vertices.at(0));
            rc.draw_indexed(draw.indices.at(0), gpu::IndexType::U32, draw.index_count, 0, 0, 1);
        } else {
            rc.draw(0, 3, 0, 1);
        }
    }

    pub fn destroy(&mut self, render_ctx: &RenderContext) {
        if let Some(mut draw) = self.tabletop.take() {
            render_ctx.context.destroy_buffer(draw.vertices);
            render_ctx.context.destroy_buffer(draw.indices);
            render_ctx.context.destroy_render_pipeline(&mut draw.pipeline);
        }
        render_ctx.context.destroy_texture_view(self.terrain_view);
        render_ctx.context.destroy_texture(self.terrain_texture);
        render_ctx.context.destroy_buffer(self.terrain_buffer);
        render_ctx.context.destroy_texture_view(self.owner_view);
        render_ctx.context.destroy_texture(self.owner_texture);
        render_ctx.context.destroy_buffer(self.owner_buffer);
        render_ctx.context.destroy_texture_view(self.clock_view);
        render_ctx.context.destroy_texture(self.clock_texture);
        render_ctx.context.destroy_buffer(self.clock_buffer);
        render_ctx.context.destroy_texture_view(self.skin_view);
        render_ctx.context.destroy_texture(self.skin_texture);
        render_ctx.context.destroy_buffer(self.skin_buffer);
        render_ctx.context.destroy_sampler(self.skin_sampler);
        if let Some((_, tiles)) = self.terrain_tiles.take() {
            for tile in tiles {
                tile.destroy(&render_ctx.context);
            }
        }
        if let Some(texture) = self.terrain_art.take() {
            texture.destroy(&render_ctx.context);
        }
        self.terrain_art_key = None;
        render_ctx
            .context
            .destroy_render_pipeline(&mut self.pipeline);
    }
}

fn conquest_involves_player(old_owner: u16, new_owner: u16, my_player_id: u16) -> bool {
    my_player_id != 0 && (old_owner == my_player_id || new_owner == my_player_id)
}

/// Records the 64 Hz tick a conquest or ability flash started on this tile.
/// The fragment shader turns the age into intensity, so nothing has to revisit
/// the tile again while the flash fades out. Writes the flash (x) channel.
#[allow(clippy::too_many_arguments)]
fn start_flash_at(
    clock: &mut [u32],
    u32_per_row: u32,
    dirty_chunks: &mut [bool],
    tile: u32,
    tick: u32,
    ability: bool,
    chunk_h: u32,
    width: u32,
) {
    let dst = ((tile / width) * u32_per_row + (tile % width) * 2) as usize;
    clock[dst] = tick.max(1) | if ability { FLASH_ABILITY_BIT } else { 0 };
    dirty_chunks[((tile / width) / chunk_h) as usize] = true;
}

/// Records when a tile became hidden so the shader can fade the fog of war.
/// Only transitions touch the texture; a tile that stays hidden keeps its start.
/// Writes the fade (y) channel. `initial` marks the first visibility pass, where
/// already-dark tiles get the sentinel so they do not fade in with the map.
#[allow(clippy::too_many_arguments)]
fn set_tile_visibility(
    clock: &mut [u32],
    u32_per_row: u32,
    dirty_chunks: &mut [bool],
    tile: u32,
    visible: bool,
    tick: u32,
    initial: bool,
    chunk_h: u32,
    width: u32,
) {
    let dst = ((tile / width) * u32_per_row + (tile % width) * 2 + 1) as usize;
    let prev = clock[dst];
    let new = if visible {
        0
    } else if prev != 0 {
        return; // already hidden: keep its original start tick
    } else if initial {
        FADE_HIDDEN_SENTINEL
    } else {
        tick.max(1)
    };
    if prev == new {
        return;
    }
    clock[dst] = new;
    dirty_chunks[((tile / width) / chunk_h) as usize] = true;
}

/// Uploads only the row chunks marked dirty, then clears the marks.
#[allow(clippy::too_many_arguments)]
fn upload_rows(
    context: &gpu::Context,
    encoder: &mut gpu::CommandEncoder,
    dirty_chunks: &mut [bool],
    buffer: gpu::Buffer,
    texture: gpu::Texture,
    width: u32,
    height: u32,
    chunk_h: u32,
    bytes_per_row: u32,
    bytes_per_texel: u32,
    label: &str,
) {
    let num_chunks = dirty_chunks.len();
    let mut start_chunk = None;
    let mut upload_range = |start: usize, end: usize| {
        let min_y = (start as u32) * chunk_h;
        let max_y = (((end as u32) + 1) * chunk_h - 1).min(height - 1);
        let offset_bytes = u64::from(min_y * bytes_per_row);
        let size_bytes = u64::from((max_y - min_y) * bytes_per_row)
            + u64::from(width) * u64::from(bytes_per_texel);

        context.sync_buffer(buffer, offset_bytes, size_bytes);

        let src_piece: gpu::BufferPiece = buffer.at(offset_bytes);
        let mut dst_piece: gpu::TexturePiece = texture.into();
        dst_piece.origin = [0, min_y, 0];

        let mut transfer = encoder.transfer(label);
        transfer.copy_buffer_to_texture(
            src_piece,
            bytes_per_row,
            dst_piece,
            gpu::Extent {
                width,
                height: max_y - min_y + 1,
                depth: 1,
            },
        );
    };

    for i in 0..num_chunks {
        if dirty_chunks[i] {
            if start_chunk.is_none() {
                start_chunk = Some(i);
            }
        } else if let Some(start) = start_chunk {
            upload_range(start, i - 1);
            start_chunk = None;
        }
    }
    if let Some(start) = start_chunk {
        upload_range(start, num_chunks - 1);
    }
    dirty_chunks.fill(false);
}

