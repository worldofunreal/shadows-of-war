use crate::context::RenderContext;
use blade_graphics as gpu;
use bytemuck::{Pod, Zeroable};

pub const MAX_BLAST_INSTANCES: usize = 64;
const VERTS_PER_OCTAGON: u32 = 18;

pub const BLAST_KIND_GROUND_SCORCH: f32 = 0.0;
pub const BLAST_KIND_NUKE_AIR: f32 = 1.0;
pub const BLAST_KIND_SAM_AIRBURST: f32 = 2.0;
pub const BLAST_KIND_BUILDING_POP: f32 = 3.0;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct BlastGlobals {
    pub camera_pos: [f32; 2],
    pub zoom: f32,
    pub time: f32,
    pub screen_size: [f32; 2],
    pub _pad: [f32; 2],
}

/// One localized blast instance rendered as an inscribed octagon (6 triangles).
/// Uploaded into an instanced vertex buffer (divisor = 1) — zero storage buffers,
/// compatible with WebGL2 / GLES 3.0 and native Vulkan/Metal.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, blade_macros::Vertex)]
pub struct BlastInstanceGpu {
    pub world_pos: [f32; 2],
    pub bound_radius: f32,
    pub inner_radius: f32,
    pub outer_radius: f32,
    pub age_secs: f32,
    pub expand_dur: f32,
    pub lifetime_secs: f32,
    pub kind: f32,
    pub seed: f32,
}

#[derive(blade_macros::ShaderData)]
struct BlastShaderData {
    globals: BlastGlobals,
}

pub struct BlastRenderer {
    pipeline: gpu::RenderPipeline,
    ground_buffer: gpu::Buffer,
    air_buffer: gpu::Buffer,
    ground_upload: Vec<BlastInstanceGpu>,
    air_upload: Vec<BlastInstanceGpu>,
}

impl BlastRenderer {
    pub fn new(context: &gpu::Context, surface_format: gpu::TextureFormat) -> Self {
        let shader = context.create_shader(gpu::ShaderDesc {
            source: include_str!("shaders/blast.wgsl"),
            naga_module: None,
        });
        assert_eq!(
            std::mem::size_of::<BlastGlobals>(),
            shader.get_struct_size("BlastGlobals") as usize,
            "BlastGlobals must match WGSL `struct BlastGlobals` uniform layout"
        );

        let layout = <BlastShaderData as gpu::ShaderData>::layout();
        let vertex_layout = <BlastInstanceGpu as gpu::Vertex>::layout();
        let blend = gpu::BlendState {
            color: gpu::BlendComponent {
                src_factor: gpu::BlendFactor::SrcAlpha,
                dst_factor: gpu::BlendFactor::OneMinusSrcAlpha,
                operation: gpu::BlendOperation::Add,
            },
            alpha: gpu::BlendComponent::OVER,
        };

        // Match the swapchain surface contract used by map.wgsl and mover_sprites.wgsl.
        let fragment_entry = if matches!(surface_format, gpu::TextureFormat::Rgba8Unorm) {
            "fs_main_srgb"
        } else {
            "fs_main"
        };

        let pipeline = context.create_render_pipeline(gpu::RenderPipelineDesc {
            name: "blast_pipeline",
            data_layouts: &[&layout],
            vertex: shader.at("vs_main"),
            vertex_fetches: &[gpu::VertexFetchState {
                layout: &vertex_layout,
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

        let buffer_size = (MAX_BLAST_INSTANCES * std::mem::size_of::<BlastInstanceGpu>()) as u64;
        let ground_buffer = context.create_buffer(gpu::BufferDesc {
            name: "blast_ground_instances",
            size: buffer_size,
            memory: gpu::Memory::Upload,
        });
        let air_buffer = context.create_buffer(gpu::BufferDesc {
            name: "blast_air_instances",
            size: buffer_size,
            memory: gpu::Memory::Upload,
        });

        Self {
            pipeline,
            ground_buffer,
            air_buffer,
            ground_upload: Vec::with_capacity(16),
            air_upload: Vec::with_capacity(16),
        }
    }

    pub fn begin_frame(&mut self) {
        self.ground_upload.clear();
        self.air_upload.clear();
    }

    pub fn push_ground(&mut self, inst: BlastInstanceGpu) {
        if self.ground_upload.len() < MAX_BLAST_INSTANCES {
            self.ground_upload.push(inst);
        }
    }

    pub fn push_air(&mut self, inst: BlastInstanceGpu) {
        if self.air_upload.len() < MAX_BLAST_INSTANCES {
            self.air_upload.push(inst);
        }
    }

    pub fn has_ground(&self) -> bool {
        !self.ground_upload.is_empty()
    }

    pub fn has_air(&self) -> bool {
        !self.air_upload.is_empty()
    }

    /// Draw scorched craters and cooling ember fissures UNDER movers and buildings.
    /// Returns immediately with zero GPU cost when no ground blasts are active.
    pub fn draw_ground(
        &self,
        encoder: &mut gpu::CommandEncoder,
        target_view: gpu::TextureView,
        globals: BlastGlobals,
        context: &gpu::Context,
    ) {
        Self::draw_batch(
            "blast_ground_pass",
            &self.pipeline,
            self.ground_buffer,
            &self.ground_upload,
            encoder,
            target_view,
            globals,
            context,
        );
    }

    /// Draw aerial flashes, fireball fronts, shockwaves, embers, smoke, SAM airbursts,
    /// and building pops OVER movers and UNDER world nameplates.
    /// Returns immediately with zero GPU cost when no aerial blasts are active.
    pub fn draw_air(
        &self,
        encoder: &mut gpu::CommandEncoder,
        target_view: gpu::TextureView,
        globals: BlastGlobals,
        context: &gpu::Context,
    ) {
        Self::draw_batch(
            "blast_air_pass",
            &self.pipeline,
            self.air_buffer,
            &self.air_upload,
            encoder,
            target_view,
            globals,
            context,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_batch(
        pass_name: &str,
        pipeline: &gpu::RenderPipeline,
        buffer: gpu::Buffer,
        instances: &[BlastInstanceGpu],
        encoder: &mut gpu::CommandEncoder,
        target_view: gpu::TextureView,
        globals: BlastGlobals,
        context: &gpu::Context,
    ) {
        let count = instances.len() as u32;
        if count == 0 {
            return;
        }
        let bytes = bytemuck::cast_slice(instances);
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), buffer.data(), bytes.len());
        }
        context.sync_buffer(buffer, 0, bytes.len() as u64);

        let mut pass = encoder.render(
            pass_name,
            gpu::RenderTargetSet {
                colors: &[gpu::RenderTarget {
                    view: target_view,
                    init_op: gpu::InitOp::Load,
                    finish_op: gpu::FinishOp::Store,
                }],
                depth_stencil: None,
            },
        );
        let data = BlastShaderData { globals };
        let mut rc = pass.with(pipeline);
        rc.bind(0, &data);
        rc.bind_vertex(0, buffer.at(0));
        rc.draw(0, VERTS_PER_OCTAGON, 0, count);
    }

    pub fn destroy(&mut self, render_ctx: &RenderContext) {
        render_ctx
            .context
            .destroy_render_pipeline(&mut self.pipeline);
        render_ctx.context.destroy_buffer(self.ground_buffer);
        render_ctx.context.destroy_buffer(self.air_buffer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blast_gpu_structs_match_expected_alignment() {
        assert_eq!(std::mem::size_of::<BlastGlobals>(), 32);
        assert_eq!(std::mem::size_of::<BlastInstanceGpu>(), 40);
    }
}
