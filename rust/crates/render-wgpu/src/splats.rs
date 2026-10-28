//! Skinned 3D Gaussian splats: a spike that measures what splat characters
//! cost inside this renderer (docs/gaussian-splats.md).
//!
//! A `VIGGCHAR` v1 `.vsplat` decodes into rest splats, each bound to four
//! bones. Every frame one compute pass skins and projects the splats of
//! every instance into one visible list (`splat_skin.wgsl`), a GPU radix
//! sort orders that list back to front by view depth (`splat_sort.wgsl`),
//! and one instanced draw lays each splat as a screen-space Gaussian quad,
//! premultiplied, over the world's colour and tested against its depth
//! (`splat_draw.wgsl`). Instances share the sort, so splats of different
//! characters interleave correctly.
//!
//! The spike API is direct: the host adds assets and sets instances with
//! their bone palettes on the renderer. There is no retained-model or
//! service path yet.

use bytemuck::{Pod, Zeroable};
use glam::{DMat4, Mat4, Quat, Vec3};

use crate::camera::CameraMatrices;
use crate::target::{ColorTarget, DEPTH_FORMAT};
use crate::timing::{GpuPassTiming, PassTimer};
use crate::Gpu;

const SKIN_GROUP: u32 = 256;
const SORT_BLOCK: u32 = 1024;
const SORT_BINS: u64 = 16;
/// Dynamic-offset stride of the sort's per-pass uniform.
const SORT_UNIFORM_STRIDE: u64 = 256;
const MAX_SORT_PASSES: u64 = 8;
/// Indirect draw arguments start at this byte in the counter buffer.
const ARGUMENTS_OFFSET: u64 = 16;

/// One rest splat as the skin pass reads it (`splat_skin.wgsl` `Rest`).
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Rest {
    position: [f32; 4],
    rotation: [f32; 4],
    scale: [f32; 4],
    bones: [u32; 2],
    weights: u32,
    pad: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuInstance {
    model: [f32; 16],
    palette: u32,
    first: u32,
    count: u32,
    pad: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct FrameUniform {
    view: [f32; 16],
    projection: [f32; 16],
    viewport: [f32; 2],
    depth_near: f32,
    depth_far: f32,
    key_max: u32,
    capacity: u32,
    pad: [u32; 2],
}

/// One rest splat a host hands the renderer, decoded from its file.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SplatPoint {
    pub position: [f32; 3],
    /// Unit quaternion, xyzw.
    pub rotation: [f32; 4],
    /// Per-axis standard deviation.
    pub sigma: [f32; 3],
    /// sRGB colour.
    pub color: [u8; 3],
    /// Opacity, 0 to 1.
    pub opacity: f32,
    /// Four rig bone indices and their weights out of 255.
    pub bones: [u16; 4],
    pub weights: [u8; 4],
}

/// A skinned splat character's rest splats and rig joints.
#[derive(Clone)]
pub struct SplatAsset {
    rest: Vec<Rest>,
    /// World rest position of each rig bone, the index space the splats'
    /// bone indices land in.
    pub joints: Vec<Vec3>,
    /// The rest splats' centres' bounds.
    pub bounds: (Vec3, Vec3),
}

impl SplatAsset {
    pub fn new(points: &[SplatPoint], joints: Vec<Vec3>) -> Self {
        let mut low = Vec3::splat(f32::MAX);
        let mut high = Vec3::splat(f32::MIN);
        let rest = points
            .iter()
            .map(|point| {
                let p = Vec3::from_array(point.position);
                low = low.min(p);
                high = high.max(p);
                let [r, g, b] = point.color;
                let [b0, b1, b2, b3] = point.bones.map(u32::from);
                Rest {
                    position: [p.x, p.y, p.z, point.opacity],
                    rotation: point.rotation,
                    scale: [
                        point.sigma[0],
                        point.sigma[1],
                        point.sigma[2],
                        f32::from_bits(u32::from_le_bytes([r, g, b, 255])),
                    ],
                    bones: [b0 | b1 << 16, b2 | b3 << 16],
                    weights: u32::from_le_bytes(point.weights),
                    pad: 0,
                }
            })
            .collect();
        Self {
            rest,
            joints,
            bounds: (low, high),
        }
    }

    pub fn splats(&self) -> usize {
        self.rest.len()
    }
}

/// A bone palette for `joints` posed by `rotations`: bone `j` turns by
/// `rotations[j]` about its own rest joint, in world-aligned axes, after its
/// parent's motion (`parents[j]`, -1 for a root). The result is each bone's
/// rest-to-posed matrix, the matrices skinning blends.
pub fn posed_palette(joints: &[Vec3], parents: &[i32], rotations: &[Quat]) -> Vec<Mat4> {
    fn resolve(
        j: usize,
        joints: &[Vec3],
        parents: &[i32],
        rotations: &[Quat],
        out: &mut [Option<Mat4>],
    ) -> Mat4 {
        if let Some(m) = out[j] {
            return m;
        }
        let parent = match usize::try_from(parents.get(j).copied().unwrap_or(-1)) {
            Ok(p) if p < joints.len() && p != j => resolve(p, joints, parents, rotations, out),
            _ => Mat4::IDENTITY,
        };
        let rotation = rotations.get(j).copied().unwrap_or(Quat::IDENTITY);
        let local = Mat4::from_translation(joints[j])
            * Mat4::from_quat(rotation)
            * Mat4::from_translation(-joints[j]);
        let m = parent * local;
        out[j] = Some(m);
        m
    }
    let mut out = vec![None; joints.len()];
    (0..joints.len())
        .map(|j| resolve(j, joints, parents, rotations, &mut out))
        .collect()
}

/// An added asset's handle on the renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SplatAssetId(usize);

/// One posed character to draw.
#[derive(Debug, Clone)]
pub struct SplatInstance {
    pub asset: SplatAssetId,
    pub transform: Mat4,
    /// One rest-to-posed matrix per rig bone (`posed_palette`); empty is the
    /// rest pose.
    pub palette: Vec<Mat4>,
}

struct AssetRange {
    first: u32,
    count: u32,
    joints: u32,
    center: Vec3,
    radius: f32,
}

struct Buffers {
    capacity: u32,
    skin: wgpu::BindGroup,
    sort: [wgpu::BindGroup; 2],
    draw: wgpu::BindGroup,
}

pub(crate) struct Splats {
    skin_pipeline: wgpu::ComputePipeline,
    arguments_pipeline: wgpu::ComputePipeline,
    count_pipeline: wgpu::ComputePipeline,
    scan_pipeline: wgpu::ComputePipeline,
    scatter_pipeline: wgpu::ComputePipeline,
    skin_layout: wgpu::BindGroupLayout,
    sort_layout: wgpu::BindGroupLayout,
    draw_layout: wgpu::BindGroupLayout,
    draw_pipeline_layout: wgpu::PipelineLayout,
    draw_shader: wgpu::ShaderModule,
    draw_pipelines: Vec<(ColorTarget, wgpu::RenderPipeline)>,
    frame: wgpu::Buffer,
    sort_uniform: wgpu::Buffer,
    counter: wgpu::Buffer,
    rest: Vec<Rest>,
    rest_buffer: Option<wgpu::Buffer>,
    assets: Vec<AssetRange>,
    instances: Vec<SplatInstance>,
    instance_buffer: Option<wgpu::Buffer>,
    palette_buffer: Option<wgpu::Buffer>,
    buffers: Option<Buffers>,
    /// Bits of view depth the sort orders by: 16, 24 or 32.
    sort_bits: u32,
    skin_timer: Option<PassTimer>,
    sort_timer: Option<PassTimer>,
    draw_timer: Option<PassTimer>,
}

fn storage(
    binding: u32,
    read_only: bool,
    visibility: wgpu::ShaderStages,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn uniform(binding: u32, dynamic: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: dynamic,
            min_binding_size: None,
        },
        count: None,
    }
}

fn entries<'a>(buffers: &[wgpu::BindingResource<'a>]) -> Vec<wgpu::BindGroupEntry<'a>> {
    buffers
        .iter()
        .enumerate()
        .map(|(binding, resource)| wgpu::BindGroupEntry {
            binding: binding as u32,
            resource: resource.clone(),
        })
        .collect()
}

impl Splats {
    pub fn new(gpu: &Gpu) -> Self {
        let device = &gpu.device;
        let compute = wgpu::ShaderStages::COMPUTE;
        let skin_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("render-wgpu splat skin"),
            source: wgpu::ShaderSource::Wgsl(include_str!("splat_skin.wgsl").into()),
        });
        let sort_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("render-wgpu splat sort"),
            source: wgpu::ShaderSource::Wgsl(include_str!("splat_sort.wgsl").into()),
        });
        let draw_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("render-wgpu splat draw"),
            source: wgpu::ShaderSource::Wgsl(include_str!("splat_draw.wgsl").into()),
        });
        let skin_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("render-wgpu splat skin"),
            entries: &[
                uniform(0, false),
                storage(1, true, compute),
                storage(2, true, compute),
                storage(3, true, compute),
                storage(4, false, compute),
                storage(5, false, compute),
                storage(6, false, compute),
                storage(7, false, compute),
            ],
        });
        let sort_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("render-wgpu splat sort"),
            entries: &[
                uniform(0, true),
                storage(1, true, compute),
                storage(2, true, compute),
                storage(3, true, compute),
                storage(4, false, compute),
                storage(5, false, compute),
                storage(6, false, compute),
            ],
        });
        let draw_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("render-wgpu splat draw"),
            entries: &[
                storage(0, true, wgpu::ShaderStages::VERTEX),
                storage(1, true, wgpu::ShaderStages::VERTEX),
            ],
        });
        let compute_pipeline = |layout: &wgpu::BindGroupLayout, module, entry: &str| {
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(entry),
                bind_group_layouts: &[Some(layout)],
                immediate_size: 0,
            });
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(entry),
                layout: Some(&pipeline_layout),
                module,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        let buffer = |label, size, usage| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage,
                mapped_at_creation: false,
            })
        };
        Self {
            skin_pipeline: compute_pipeline(&skin_layout, &skin_shader, "cs_skin"),
            arguments_pipeline: compute_pipeline(&skin_layout, &skin_shader, "cs_arguments"),
            count_pipeline: compute_pipeline(&sort_layout, &sort_shader, "cs_sort_count"),
            scan_pipeline: compute_pipeline(&sort_layout, &sort_shader, "cs_sort_scan"),
            scatter_pipeline: compute_pipeline(&sort_layout, &sort_shader, "cs_sort_scatter"),
            draw_pipeline_layout: device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("render-wgpu splat draw"),
                bind_group_layouts: &[Some(&draw_layout)],
                immediate_size: 0,
            }),
            skin_layout,
            sort_layout,
            draw_layout,
            draw_shader,
            draw_pipelines: Vec::new(),
            frame: buffer(
                "render-wgpu splat frame",
                std::mem::size_of::<FrameUniform>() as u64,
                wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            ),
            sort_uniform: buffer(
                "render-wgpu splat sort passes",
                SORT_UNIFORM_STRIDE * MAX_SORT_PASSES,
                wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            ),
            counter: buffer(
                "render-wgpu splat counter",
                32,
                wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::INDIRECT
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::COPY_SRC,
            ),
            rest: Vec::new(),
            rest_buffer: None,
            assets: Vec::new(),
            instances: Vec::new(),
            instance_buffer: None,
            palette_buffer: None,
            buffers: None,
            sort_bits: 24,
            skin_timer: PassTimer::new(gpu, "splat-skin"),
            sort_timer: PassTimer::new(gpu, "splat-sort"),
            draw_timer: PassTimer::new(gpu, "splat-draw"),
        }
    }

    pub fn add_asset(&mut self, asset: &SplatAsset) -> SplatAssetId {
        let (low, high) = asset.bounds;
        self.assets.push(AssetRange {
            first: self.rest.len() as u32,
            count: asset.rest.len() as u32,
            joints: asset.joints.len() as u32,
            center: (low + high) * 0.5,
            radius: (high - low).length() * 0.5,
        });
        self.rest.extend_from_slice(&asset.rest);
        self.rest_buffer = None;
        self.buffers = None;
        SplatAssetId(self.assets.len() - 1)
    }

    pub fn set_instances(&mut self, instances: Vec<SplatInstance>) {
        let capacity = self.capacity_of(&instances);
        if self.buffers.as_ref().is_some_and(|b| b.capacity < capacity)
            || self.instances.len() != instances.len()
        {
            self.buffers = None;
        }
        self.instances = instances;
    }

    pub fn set_sort_bits(&mut self, bits: u32) {
        self.sort_bits = (bits / 8).clamp(2, 4) * 8;
        for timer in [&mut self.sort_timer].into_iter().flatten() {
            timer.restart();
        }
    }

    fn capacity_of(&self, instances: &[SplatInstance]) -> u32 {
        instances
            .iter()
            .map(|instance| self.assets[instance.asset.0].count)
            .sum()
    }

    pub fn drawing(&self) -> bool {
        !self.instances.is_empty()
    }

    /// The splats the last frame drew, read back waiting for the GPU.
    pub fn visible(&self, gpu: &Gpu) -> u32 {
        let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("render-wgpu splat visible"),
            size: 4,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        encoder.copy_buffer_to_buffer(&self.counter, 0, &readback, 0, Some(4));
        gpu.queue.submit([encoder.finish()]);
        readback.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
        let Ok(mapped) = readback.slice(..).get_mapped_range() else {
            return 0;
        };
        u32::from_le_bytes(mapped[..4].try_into().unwrap())
    }

    pub fn timings(&self) -> Vec<GpuPassTiming> {
        [
            (&self.skin_timer, "splat-skin"),
            (&self.sort_timer, "splat-sort"),
            (&self.draw_timer, "splat-draw"),
        ]
        .into_iter()
        .map(|(timer, pass)| {
            timer
                .as_ref()
                .map_or_else(|| crate::timing::untimed(pass), PassTimer::readout)
        })
        .collect()
    }

    /// Upload this frame's instances and palettes and (re)build whatever
    /// buffers the splat count outgrew.
    fn upload(&mut self, gpu: &Gpu, eye: Vec3) {
        let device = &gpu.device;
        let storage_buffer = |label, size: u64, extra| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: size.max(16),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | extra,
                mapped_at_creation: false,
            })
        };
        if self.rest_buffer.is_none() {
            let buffer = storage_buffer(
                "render-wgpu splat rest",
                std::mem::size_of_val(self.rest.as_slice()) as u64,
                wgpu::BufferUsages::empty(),
            );
            gpu.queue
                .write_buffer(&buffer, 0, bytemuck::cast_slice(&self.rest));
            self.rest_buffer = Some(buffer);
        }
        let mut palette = Vec::new();
        let gpu_instances: Vec<GpuInstance> = self
            .instances
            .iter()
            .map(|instance| {
                let asset = &self.assets[instance.asset.0];
                let first_bone = palette.len() as u32;
                for j in 0..asset.joints as usize {
                    palette.push(
                        instance
                            .palette
                            .get(j)
                            .copied()
                            .unwrap_or(Mat4::IDENTITY)
                            .to_cols_array(),
                    );
                }
                let relative = Mat4::from_translation(-eye) * instance.transform;
                GpuInstance {
                    model: relative.to_cols_array(),
                    palette: first_bone,
                    first: asset.first,
                    count: asset.count,
                    pad: 0,
                }
            })
            .collect();
        let palette_bytes: &[u8] = bytemuck::cast_slice(&palette);
        let instance_bytes: &[u8] = bytemuck::cast_slice(&gpu_instances);
        if self
            .palette_buffer
            .as_ref()
            .is_none_or(|b| b.size() < palette_bytes.len() as u64)
        {
            self.palette_buffer = Some(storage_buffer(
                "render-wgpu splat palette",
                palette_bytes.len() as u64,
                wgpu::BufferUsages::empty(),
            ));
            self.buffers = None;
        }
        if self
            .instance_buffer
            .as_ref()
            .is_none_or(|b| b.size() < instance_bytes.len() as u64)
        {
            self.instance_buffer = Some(storage_buffer(
                "render-wgpu splat instances",
                instance_bytes.len() as u64,
                wgpu::BufferUsages::empty(),
            ));
            self.buffers = None;
        }
        let (Some(rest), Some(instances), Some(palette_buffer)) = (
            self.rest_buffer.as_ref(),
            self.instance_buffer.as_ref(),
            self.palette_buffer.as_ref(),
        ) else {
            return;
        };
        gpu.queue.write_buffer(palette_buffer, 0, palette_bytes);
        gpu.queue.write_buffer(instances, 0, instance_bytes);
        if self.buffers.is_some() {
            return;
        }
        let capacity = self.capacity_of(&self.instances).max(1);
        let n = u64::from(capacity);
        let projected = storage_buffer(
            "render-wgpu splat projected",
            n * 48,
            wgpu::BufferUsages::empty(),
        );
        let keys = [0, 1]
            .map(|_| storage_buffer("render-wgpu splat keys", n * 4, wgpu::BufferUsages::empty()));
        let values = [0, 1].map(|_| {
            storage_buffer(
                "render-wgpu splat values",
                n * 4,
                wgpu::BufferUsages::empty(),
            )
        });
        let block_counts = storage_buffer(
            "render-wgpu splat block counts",
            SORT_BINS * 4 * u64::from(capacity.div_ceil(SORT_BLOCK)),
            wgpu::BufferUsages::empty(),
        );
        let skin = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("render-wgpu splat skin"),
            layout: &self.skin_layout,
            entries: &entries(&[
                self.frame.as_entire_binding(),
                rest.as_entire_binding(),
                instances.as_entire_binding(),
                palette_buffer.as_entire_binding(),
                projected.as_entire_binding(),
                keys[0].as_entire_binding(),
                values[0].as_entire_binding(),
                self.counter.as_entire_binding(),
            ]),
        });
        let sort = [(0, 1), (1, 0)].map(|(from, to)| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("render-wgpu splat sort"),
                layout: &self.sort_layout,
                entries: &entries(&[
                    wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &self.sort_uniform,
                        offset: 0,
                        size: wgpu::BufferSize::new(16),
                    }),
                    wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &self.counter,
                        offset: 0,
                        size: wgpu::BufferSize::new(16),
                    }),
                    keys[from].as_entire_binding(),
                    values[from].as_entire_binding(),
                    block_counts.as_entire_binding(),
                    keys[to].as_entire_binding(),
                    values[to].as_entire_binding(),
                ]),
            })
        });
        // An even pass count leaves the order in the first value buffer.
        let draw = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("render-wgpu splat draw"),
            layout: &self.draw_layout,
            entries: &entries(&[projected.as_entire_binding(), values[0].as_entire_binding()]),
        });
        self.buffers = Some(Buffers {
            capacity,
            skin,
            sort,
            draw,
        });
    }

    /// Skin, project and sort this frame's splats for `camera`, before the
    /// splat draw pass, in its encoder.
    pub fn encode_compute(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        camera: &CameraMatrices,
        viewport: [f32; 2],
    ) {
        for timer in [
            &mut self.skin_timer,
            &mut self.sort_timer,
            &mut self.draw_timer,
        ]
        .into_iter()
        .flatten()
        {
            timer.collect(gpu);
        }
        self.upload(gpu, camera.eye);
        let Some(buffers) = self.buffers.as_ref() else {
            return;
        };
        let view =
            (camera.view.as_dmat4() * DMat4::from_translation(camera.eye.as_dvec3())).as_mat4();
        // The key's depth range: every instance's rest bounds, with room for
        // the pose to reach past them.
        let forward = -Vec3::new(
            camera.view.x_axis.z,
            camera.view.y_axis.z,
            camera.view.z_axis.z,
        );
        let (mut near, mut far) = (f32::MAX, 0.0f32);
        for instance in &self.instances {
            let asset = &self.assets[instance.asset.0];
            let center = instance.transform.transform_point3(asset.center);
            let scale = instance.transform.x_axis.truncate().length();
            let radius = asset.radius * scale * 1.5;
            let depth = (center - camera.eye).dot(forward);
            near = near.min(depth - radius);
            far = far.max(depth + radius);
        }
        let near = near.max(0.05);
        let far = far.max(near + 0.01);
        let uniform = FrameUniform {
            view: view.to_cols_array(),
            projection: camera.projection.to_cols_array(),
            viewport,
            depth_near: near,
            depth_far: far,
            key_max: if self.sort_bits >= 32 {
                u32::MAX
            } else {
                (1u32 << self.sort_bits) - 1
            },
            capacity: buffers.capacity,
            pad: [0; 2],
        };
        gpu.queue
            .write_buffer(&self.frame, 0, bytemuck::bytes_of(&uniform));
        let passes = self.sort_bits / 4;
        let mut shifts = vec![0u8; (SORT_UNIFORM_STRIDE * MAX_SORT_PASSES) as usize];
        for pass in 0..passes {
            let at = (u64::from(pass) * SORT_UNIFORM_STRIDE) as usize;
            shifts[at..at + 4].copy_from_slice(&(pass * 4).to_le_bytes());
        }
        gpu.queue.write_buffer(&self.sort_uniform, 0, &shifts);

        encoder.clear_buffer(&self.counter, 0, Some(4));
        let widest = self
            .instances
            .iter()
            .map(|instance| self.assets[instance.asset.0].count)
            .max()
            .unwrap_or(0);
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("render-wgpu splat skin"),
                timestamp_writes: self.skin_timer.as_ref().and_then(PassTimer::compute_writes),
            });
            pass.set_bind_group(0, &buffers.skin, &[]);
            pass.set_pipeline(&self.skin_pipeline);
            pass.dispatch_workgroups(widest.div_ceil(SKIN_GROUP), self.instances.len() as u32, 1);
            pass.set_pipeline(&self.arguments_pipeline);
            pass.dispatch_workgroups(1, 1, 1);
        }
        {
            let blocks = buffers.capacity.div_ceil(SORT_BLOCK);
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("render-wgpu splat sort"),
                timestamp_writes: self.sort_timer.as_ref().and_then(PassTimer::compute_writes),
            });
            for p in 0..passes {
                let offset = (u64::from(p) * SORT_UNIFORM_STRIDE) as u32;
                pass.set_bind_group(0, &buffers.sort[(p % 2) as usize], &[offset]);
                pass.set_pipeline(&self.count_pipeline);
                pass.dispatch_workgroups(blocks, 1, 1);
                pass.set_pipeline(&self.scan_pipeline);
                pass.dispatch_workgroups(1, 1, 1);
                pass.set_pipeline(&self.scatter_pipeline);
                pass.dispatch_workgroups(blocks, 1, 1);
            }
        }
        for timer in [&mut self.skin_timer, &mut self.sort_timer]
            .into_iter()
            .flatten()
        {
            timer.resolve(encoder);
        }
    }

    /// Build the draw pipeline for `format` before the pass that draws.
    pub fn prepare_draw(&mut self, device: &wgpu::Device, format: ColorTarget) {
        if self.draw_pipelines.iter().any(|(f, _)| *f == format) {
            return;
        }
        let premultiplied = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        };
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("render-wgpu splat draw"),
            layout: Some(&self.draw_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &self.draw_shader,
                entry_point: Some("vs_splat"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: format.multisample(),
            fragment: Some(wgpu::FragmentState {
                module: &self.draw_shader,
                entry_point: Some("fs_splat"),
                compilation_options: Default::default(),
                targets: &format.targets(
                    wgpu::ColorTargetState {
                        format: format.format,
                        blend: Some(wgpu::BlendState {
                            color: premultiplied,
                            alpha: premultiplied,
                        }),
                        write_mask: wgpu::ColorWrites::ALL,
                    },
                    crate::motion::MotionWrite::None,
                ),
            }),
            multiview_mask: None,
            cache: None,
        });
        self.draw_pipelines.push((format, pipeline));
    }

    pub fn draw_writes(&self) -> Option<wgpu::RenderPassTimestampWrites<'_>> {
        self.draw_timer.as_ref().and_then(PassTimer::render_writes)
    }

    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>, format: ColorTarget) {
        let (Some(buffers), Some((_, pipeline))) = (
            self.buffers.as_ref(),
            self.draw_pipelines.iter().find(|(f, _)| *f == format),
        ) else {
            return;
        };
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &buffers.draw, &[]);
        pass.draw_indirect(&self.counter, ARGUMENTS_OFFSET);
    }

    pub fn resolve_draw(&mut self, encoder: &mut wgpu::CommandEncoder) {
        if let Some(timer) = &mut self.draw_timer {
            timer.resolve(encoder);
        }
    }

    pub fn submitted(&mut self) {
        for timer in [
            &mut self.skin_timer,
            &mut self.sort_timer,
            &mut self.draw_timer,
        ]
        .into_iter()
        .flatten()
        {
            timer.submitted();
        }
    }
}
