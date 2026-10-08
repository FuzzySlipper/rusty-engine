//! Precipitation around the camera (`RenderDiff::SetPrecipitation`,
//! `precipitation.wgsl`): one instanced quad per drop, placed by its index
//! in a box around the camera that wraps as the camera moves, drawn after
//! the world over its HDR target with the world's depth bound for hiding
//! and fading, in a timed pass of its own (`gpu.passes` "precipitation").
//! Nothing is simulated: the drops' travel is the presentation time, taken
//! here in double precision so drops do not jitter late in a long session.

use render_model::{PrecipitationDescriptor, PrecipitationShape};
use render_shaders::{Entry, Features};

use crate::gpu::Gpu;
use crate::pipelines::standard;
use crate::shaders::Shaders;
use crate::target::ColorTarget;

/// Bytes of `Precipitation` in `precipitation.wgsl`: five rows.
const UNIFORM_BYTES: u64 = 5 * 16;

pub(crate) struct Precipitation {
    shader: wgpu::ShaderModule,
    uniform: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    /// Single-sample and multisampled depth: the pipeline layout.
    layouts: [wgpu::PipelineLayout; 2],
    /// Per colour target and blend (alpha, additive).
    pipelines: Vec<(ColorTarget, [wgpu::RenderPipeline; 2])>,
}

impl Precipitation {
    pub fn new(
        device: &wgpu::Device,
        shaders: &mut Shaders,
        frame_layout: &wgpu::BindGroupLayout,
        depth_layouts: [&wgpu::BindGroupLayout; 2],
    ) -> Self {
        let shader = standard(shaders.module(device, Entry::Precipitation, Features::default()));
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("render-wgpu precipitation"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(UNIFORM_BYTES),
                },
                count: None,
            }],
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("render-wgpu precipitation"),
            size: UNIFORM_BYTES,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("render-wgpu precipitation"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        let layouts = depth_layouts.map(|depth| {
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("render-wgpu precipitation"),
                bind_group_layouts: &[Some(frame_layout), Some(&layout), Some(depth)],
                immediate_size: 0,
            })
        });
        Self {
            shader,
            uniform,
            bind_group,
            layouts,
            pipelines: Vec::new(),
        }
    }

    /// Write the volume's parameters for presentation time `time` (seconds).
    pub fn write(&self, gpu: &Gpu, precipitation: &PrecipitationDescriptor, time: f64) {
        let box_size = [
            precipitation.radius * 2.0,
            precipitation.height * 2.0,
            precipitation.radius * 2.0,
        ];
        // How far the drops have fallen, as a share of the box per axis.
        let phase: [f32; 3] = std::array::from_fn(|axis| {
            (f64::from(precipitation.velocity[axis]) * time / f64::from(box_size[axis]))
                .rem_euclid(1.0) as f32
        });
        let shape = match precipitation.shape {
            PrecipitationShape::Streak => 0.0,
            PrecipitationShape::Flake => 1.0,
        };
        let [vx, vy, vz] = precipitation.velocity;
        let values: [f32; 20] = [
            box_size[0],
            box_size[1],
            box_size[2],
            shape,
            vx,
            vy,
            vz,
            precipitation.streak_seconds,
            phase[0],
            phase[1],
            phase[2],
            precipitation.size,
            precipitation.color[0],
            precipitation.color[1],
            precipitation.color[2],
            precipitation.color[3],
            if precipitation.additive { 1.0 } else { 0.0 },
            0.0,
            0.0,
            0.0,
        ];
        gpu.queue
            .write_buffer(&self.uniform, 0, bytemuck::cast_slice(&values));
    }

    /// Draw `drops` drops into a pass over the world's HDR target, with the
    /// frame in group 0 and the world's depth in group 2.
    pub fn draw(
        &mut self,
        device: &wgpu::Device,
        pass: &mut wgpu::RenderPass<'_>,
        format: ColorTarget,
        precipitation: &PrecipitationDescriptor,
    ) {
        let index = match self.pipelines.iter().position(|(key, _)| *key == format) {
            Some(index) => index,
            None => {
                let pipelines =
                    [false, true].map(|additive| self.pipeline(device, format, additive));
                self.pipelines.push((format, pipelines));
                self.pipelines.len() - 1
            }
        };
        pass.set_pipeline(&self.pipelines[index].1[usize::from(precipitation.additive)]);
        pass.set_bind_group(1, &self.bind_group, &[]);
        pass.draw(0..4, 0..precipitation.drops);
    }

    fn pipeline(
        &self,
        device: &wgpu::Device,
        format: ColorTarget,
        additive: bool,
    ) -> wgpu::RenderPipeline {
        let multisampled = format.samples > 1;
        let blend = if additive {
            wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendState::ALPHA_BLENDING.alpha,
            }
        } else {
            wgpu::BlendState::ALPHA_BLENDING
        };
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("render-wgpu precipitation"),
            layout: Some(&self.layouts[usize::from(multisampled)]),
            vertex: wgpu::VertexState {
                module: &self.shader,
                entry_point: Some("vs_drop"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: format.multisample(),
            fragment: Some(wgpu::FragmentState {
                module: &self.shader,
                entry_point: Some(if multisampled {
                    "fs_drop_multisampled"
                } else {
                    "fs_drop"
                }),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: format.format,
                    blend: Some(blend),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        })
    }
}
