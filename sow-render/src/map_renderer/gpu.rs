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
    skin_sampler: gpu::Sampler,
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
    pub pipeline: gpu::RenderPipeline,
    tabletop: Option<TabletopDraw>,
    surface_format: gpu::TextureFormat,
    pub width: u32,
    pub height: u32,
    pub terrain: Vec<u8>,
    pub owners: Vec<u16>,
    pub conquest_flash: Vec<u8>,
    /// Tiles with active flash intensity, for sparse decay (avoids scanning all tiles).
    flash_active: Vec<u32>,
    pub vision_fade: Vec<u8>,
    fade_active: Vec<u32>,
    is_fading: Vec<bool>,
    last_visible_blocks: Vec<u64>,
    pub terrain_bytes_per_row: u32,
    pub owner_bytes_per_row: u32,
    pub chunk_h: u32,
    pub dirty_chunks: Vec<bool>,
    pub last_update: Option<web_time::Instant>,
    pub has_water_neighbor: Vec<bool>,
    pub decay_accumulator: f32,
    pub fade_decay_accumulator: f32,
    crater_reveals: CraterRevealQueue,
    revealed_scratch: Vec<u32>,
    pack_stamp: Vec<u32>,
    current_pack_stamp: u32,
}

const FLASH_ABILITY_BIT: u8 = 0x80;
const FLASH_INTENSITY_MASK: u8 = !FLASH_ABILITY_BIT;

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
            pipeline,
            tabletop: None,
            surface_format,
            width,
            height,
            terrain: initial_terrain.to_vec(),
            owners: vec![0; total],
            conquest_flash: vec![0; total],
            flash_active: Vec::new(),
            vision_fade: vec![0; total],
            fade_active: Vec::new(),
            is_fading: vec![false; total],
            last_visible_blocks: Vec::new(),
            terrain_bytes_per_row,
            owner_bytes_per_row,
            chunk_h,
            dirty_chunks: vec![false; num_chunks as usize],
            last_update: None,
            has_water_neighbor,
            decay_accumulator: 0.0,
            fade_decay_accumulator: 0.0,
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
        let dt = match self.last_update {
            Some(last) => now.duration_since(last).as_secs_f32(),
            None => 0.016,
        };
        self.last_update = Some(now);

        // Scale decay based on elapsed time so it completes in exactly `conquest_duration` seconds on all frame rates.
        let decay_rate = if conquest_duration > 0.0 {
            f32::from(FLASH_INTENSITY_MASK) / conquest_duration
        } else {
            f32::from(FLASH_INTENSITY_MASK) / 3.0
        };
        self.decay_accumulator += dt * decay_rate;
        let decay_amount = self.decay_accumulator.floor() as u32;
        if decay_amount > 0 {
            self.decay_accumulator -= decay_amount as f32;
        }

        let total = (self.width * self.height) as usize;
        let u32_per_row = self.owner_bytes_per_row / 4;
        let total_u32 = (u32_per_row * self.height) as usize;
        let width = self.width;
        let height = self.height;
        let chunk_h = self.chunk_h;

        let dst_ptr = self.owner_buffer.data();
        let slice = unsafe { std::slice::from_raw_parts_mut(dst_ptr as *mut u32, total_u32) };

        // Helper: pack owner + flash + border/water/fog flags into the GPU buffer.
        // Uses visual owners only while an expanding nuke crater is active (0 idle cost).
        let pack = |slice: &mut [u32],
                    owners: &[u16],
                    crater_reveals: &CraterRevealQueue,
                    flash: &[u8],
                    fade: u8,
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

            let mut val = owner as u32 | ((flash[i] as u32) << 16);
            if has_border {
                val |= 1 << 24;
            }
            if has_water {
                val |= 1 << 25;
            }
            let fade_6bit = (fade >> 2) as u32; // scale 0..255 to 0..63
            val |= fade_6bit << 26;
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
                    start_tile_flash(
                        &mut self.conquest_flash,
                        &mut self.flash_active,
                        dt.index,
                        false,
                    );
                }
                self.owners[i] = dt.new_owner;
            }
        }
        self.crater_reveals.finalize_pending();
        self.crater_reveals
            .advance(now, width, &mut self.revealed_scratch);

        // 1. Reset chunk tracking
        self.dirty_chunks.fill(false);

        // 2. Update vision fade targets based on visibility:
        //    - Init or FoW toggle runs a full pass and uploads the full owner texture.
        //    - Normal frames diff `visible.blocks` against `last_visible_blocks` via XOR.
        let mut any_fade_changed = false;
        let full_vis_upload =
            force_full_upload || self.last_visible_blocks.len() != visible.blocks.len();
        {
            let owners = &self.owners;
            let crater_reveals = &self.crater_reveals;
            let conquest_flash = &self.conquest_flash;
            let vision_fade = &mut self.vision_fade;
            let is_fading = &mut self.is_fading;
            let fade_active = &mut self.fade_active;
            let dirty_chunks = &mut self.dirty_chunks;
            let mut apply_vis_tile = |tile_idx: u32, is_vis: bool| {
                let i = tile_idx as usize;
                let current_fade = vision_fade[i];
                if is_vis {
                    if current_fade < 255 {
                        vision_fade[i] = 255;
                        is_fading[i] = false;
                        pack(
                            slice,
                            owners,
                            crater_reveals,
                            conquest_flash,
                            255,
                            tile_idx,
                            has_water[i],
                        );
                        let y = tile_idx / width;
                        dirty_chunks[(y / chunk_h) as usize] = true;
                        any_fade_changed = true;
                    }
                } else if current_fade > 0 && !is_fading[i] {
                    is_fading[i] = true;
                    fade_active.push(tile_idx);
                }
            };

            if full_vis_upload {
                for tile_idx in 0..total as u32 {
                    apply_vis_tile(tile_idx, visible.contains(tile_idx));
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
                        apply_vis_tile(tile_idx, ((new_word >> bit) & 1) != 0);
                    }
                }
            }
        }

        if full_vis_upload {
            for tile_idx in 0..total as u32 {
                let fade = self.vision_fade[tile_idx as usize];
                pack(
                    slice,
                    &self.owners,
                    &self.crater_reveals,
                    &self.conquest_flash,
                    fade,
                    tile_idx,
                    has_water[tile_idx as usize],
                );
            }
            context.sync_buffer(self.owner_buffer, 0, self.owner_buffer.size());
            let src_piece: gpu::BufferPiece = self.owner_buffer.into();
            let dst_piece: gpu::TexturePiece = self.owner_texture.into();
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
            return;
        }

        // 3. Decay existing flash (sparse — only active entries)
        let mut decay_dirty = false;
        if decay_amount > 0 {
            self.flash_active.retain(|&tile_idx| {
                let i = tile_idx as usize;
                if i >= total {
                    return false;
                }
                self.conquest_flash[i] = decay_tile_flash(self.conquest_flash[i], decay_amount);
                let intensity = self.conquest_flash[i] & FLASH_INTENSITY_MASK;
                let fade = self.vision_fade[i];
                pack(
                    slice,
                    &self.owners,
                    &self.crater_reveals,
                    &self.conquest_flash,
                    fade,
                    tile_idx,
                    has_water[i],
                );
                let y = tile_idx / width;
                self.dirty_chunks[(y / chunk_h) as usize] = true;
                decay_dirty = true;
                intensity > 0
            });
        }

        // 4. Decay vision active fades (smoothly over 1.5 seconds)
        let fade_decay_rate = 170.0;
        self.fade_decay_accumulator += dt * fade_decay_rate;
        let fade_decay_amount = self.fade_decay_accumulator.floor() as u32;
        if fade_decay_amount > 0 {
            self.fade_decay_accumulator -= fade_decay_amount as f32;
        }

        let mut fade_dirty = false;
        if fade_decay_amount > 0 && !self.fade_active.is_empty() {
            let has_water = &self.has_water_neighbor;
            let vision_fade = &mut self.vision_fade;
            let is_fading = &mut self.is_fading;
            let owners = &self.owners;
            let crater_reveals = &self.crater_reveals;
            let conquest_flash = &self.conquest_flash;
            let dirty_chunks = &mut self.dirty_chunks;

            self.fade_active.retain(|&tile_idx| {
                let i = tile_idx as usize;
                if i >= total {
                    return false;
                }
                if !is_fading[i] {
                    return false; // Remove immediately since it became visible again
                }
                let f = vision_fade[i].saturating_sub(fade_decay_amount.min(255) as u8);
                vision_fade[i] = f;
                pack(
                    slice,
                    owners,
                    crater_reveals,
                    conquest_flash,
                    f,
                    tile_idx,
                    has_water[i],
                );
                let y = tile_idx / width;
                dirty_chunks[(y / chunk_h) as usize] = true;
                fade_dirty = true;
                let keep = f > 0;
                if !keep {
                    is_fading[i] = false;
                }
                keep
            });
        }

        // 5. Pack new dirty tiles (skipping crater tiles still waiting for the blast front)
        //    plus any crater tiles newly uncovered this frame, and their 8 neighbors.
        //    Stamp tiles so interior cells touched by multiple neighbors pack at most once.
        let mut any_tile_packed = false;
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
                    let fade = self.vision_fade[i];
                    pack(
                        slice,
                        &self.owners,
                        &self.crater_reveals,
                        &self.conquest_flash,
                        fade,
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
                            let n_fade = self.vision_fade[ni];
                            pack(
                                slice,
                                &self.owners,
                                &self.crater_reveals,
                                &self.conquest_flash,
                                n_fade,
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
                any_tile_packed = true;
            }
            for &tile_idx in &self.revealed_scratch {
                pack_with_neighbors(tile_idx, &mut self.dirty_chunks);
                any_tile_packed = true;
            }
        }

        if !any_tile_packed && !decay_dirty && !fade_dirty && !any_fade_changed {
            return;
        }

        // Upload dirty chunks
        let num_chunks = self.dirty_chunks.len();
        let mut start_chunk = None;

        let mut upload_range = |start: usize, end: usize| {
            let min_y = (start as u32) * self.chunk_h;
            let max_y = (((end as u32) + 1) * self.chunk_h - 1).min(self.height - 1);

            let offset_bytes = (min_y * self.owner_bytes_per_row) as u64;
            let size_bytes =
                ((max_y - min_y) * self.owner_bytes_per_row) as u64 + self.width as u64 * 4;

            context.sync_buffer(self.owner_buffer, offset_bytes, size_bytes);

            let src_piece: gpu::BufferPiece = self.owner_buffer.at(offset_bytes);
            let mut dst_piece: gpu::TexturePiece = self.owner_texture.into();
            dst_piece.origin = [0, min_y, 0];

            let mut transfer = encoder.transfer("owner_upload");
            transfer.copy_buffer_to_texture(
                src_piece,
                self.owner_bytes_per_row,
                dst_piece,
                gpu::Extent {
                    width: self.width,
                    height: max_y - min_y + 1,
                    depth: 1,
                },
            );
        };

        for i in 0..num_chunks {
            if self.dirty_chunks[i] {
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
    }

    /// Starts the existing per-tile flash channel for a locally visible hero ability.
    pub fn flash_ability_tile(&mut self, x: u32, y: u32) {
        if x >= self.width || y >= self.height {
            return;
        }
        let tile = y * self.width + x;
        start_tile_flash(&mut self.conquest_flash, &mut self.flash_active, tile, true);
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
        rc.bind(
            0,
            &MapShaderData {
                globals,
                player_colors,
                player_skin_styles,
                terrain_texture: self.terrain_view,
                owner_texture: self.owner_view,
                skin_texture: self.skin_view,
                terrain_art_texture: self
                    .terrain_art
                    .as_ref()
                    .map_or(self.terrain_view, |texture| texture.view),
                skin_sampler: self.skin_sampler,
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
        render_ctx.context.destroy_texture_view(self.skin_view);
        render_ctx.context.destroy_texture(self.skin_texture);
        render_ctx.context.destroy_buffer(self.skin_buffer);
        render_ctx.context.destroy_sampler(self.skin_sampler);
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

/// Starts one flash per tile; the high bit marks skill flashes for localized shader pulses.
fn start_tile_flash(flashes: &mut [u8], flash_active: &mut Vec<u32>, tile: u32, ability: bool) {
    let i = tile as usize;
    if (flashes[i] & FLASH_INTENSITY_MASK) == 0 {
        flash_active.push(tile);
    }
    flashes[i] = FLASH_INTENSITY_MASK | if ability { FLASH_ABILITY_BIT } else { 0 };
}

fn decay_tile_flash(flash: u8, amount: u32) -> u8 {
    (flash & FLASH_ABILITY_BIT)
        | (flash & FLASH_INTENSITY_MASK)
            .saturating_sub(amount.min(u32::from(FLASH_INTENSITY_MASK)) as u8)
}

