//! Candidate (c): raw wgpu over the `PresentationWorld` baseline delta.
//!
//! The shape is the one #8783 proposes: typed tables keyed by retained ids,
//! filled by `apply`, and a fixed pass sequence (sky, opaque world, readback).
//! Only the families the room study uses are realized: textures, materials,
//! static meshes, static mesh instances and the equirectangular sky.

use std::collections::{BTreeMap, HashMap};

use bootstrap_fixture::{camera_forward_up, neutral_light, write_png, Fixture, HEIGHT, WIDTH};
use glam::{Mat4, Quat, Vec3};
use render_model::{
    MeshPayloadSource, RenderDiff, RenderHandle, RenderMaterialDescriptor, StaticMeshAsset,
    StaticMeshInstanceDescriptor, TextureDescriptor, TextureFilter, TextureWrap,
};
use wgpu::util::DeviceExt;

const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const MAX_INSTANCES: u64 = 4096;

const SHADER: &str = r#"
struct Frame {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    camera: vec4<f32>,
    hemi_sky: vec4<f32>,     // rgb * intensity
    hemi_ground: vec4<f32>,  // rgb * intensity
    key_dir: vec4<f32>,      // direction toward the light
    key_color: vec4<f32>,    // rgb * intensity
};
struct Material { color: vec4<f32> };

@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var<storage, read> models: array<mat4x4<f32>>;
@group(1) @binding(0) var<uniform> material: Material;
@group(1) @binding(1) var albedo: texture_2d<f32>;
@group(1) @binding(2) var albedo_sampler: sampler;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) uv: vec2<f32>,
};

@vertex
fn vs_world(@location(0) position: vec3<f32>, @location(1) normal: vec3<f32>,
            @location(2) uv: vec2<f32>, @builtin(instance_index) instance: u32) -> VsOut {
    let model = models[instance];
    var out: VsOut;
    out.clip = frame.view_proj * model * vec4<f32>(position, 1.0);
    out.normal = (model * vec4<f32>(normal, 0.0)).xyz;
    out.uv = uv;
    return out;
}

const PI: f32 = 3.14159265;

@fragment
fn fs_world(in: VsOut) -> @location(0) vec4<f32> {
    let n = normalize(in.normal);
    let base = material.color * textureSample(albedo, albedo_sampler, in.uv);
    let hemi = mix(frame.hemi_ground.rgb, frame.hemi_sky.rgb, 0.5 * n.y + 0.5);
    let key = frame.key_color.rgb * max(dot(n, frame.key_dir.xyz), 0.0);
    return vec4<f32>(base.rgb * (hemi + key) / PI, 1.0);
}

struct SkyOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) ndc: vec2<f32>,
};

@vertex
fn vs_sky(@builtin(vertex_index) index: u32) -> SkyOut {
    let xy = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u)) * 2.0 - 1.0;
    var out: SkyOut;
    out.clip = vec4<f32>(xy, 1.0, 1.0);
    out.ndc = xy;
    return out;
}

@fragment
fn fs_sky(in: SkyOut) -> @location(0) vec4<f32> {
    let far = frame.inv_view_proj * vec4<f32>(in.ndc, 1.0, 1.0);
    let dir = normalize(far.xyz / far.w - frame.camera.xyz);
    let u = atan2(dir.z, dir.x) / (2.0 * PI) + 0.5;
    let v = asin(clamp(dir.y, -1.0, 1.0)) / PI + 0.5;
    return textureSample(albedo, albedo_sampler, vec2<f32>(u, 1.0 - v));
}
"#;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct FrameUniform {
    view_proj: [[f32; 4]; 4],
    inv_view_proj: [[f32; 4]; 4],
    camera: [f32; 4],
    hemi_sky: [f32; 4],
    hemi_ground: [f32; 4],
    key_dir: [f32; 4],
    key_color: [f32; 4],
}

/// The runtime owns these; the backend only borrows them.
struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
}

struct GpuTexture {
    view: wgpu::TextureView,
    sampler: wgpu::Sampler,
}

struct GpuMaterial {
    bind_group: wgpu::BindGroup,
}

struct GpuMesh {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    /// (material slot, first index, index count)
    groups: Vec<(u16, u32, u32)>,
    slots: HashMap<u16, String>,
}

struct Instance {
    asset: String,
    model: Mat4,
    visible: bool,
    overrides: HashMap<u16, String>,
}

/// Typed handle-keyed tables and a fixed pass pipeline.
struct Backend {
    world_pipeline: wgpu::RenderPipeline,
    sky_pipeline: wgpu::RenderPipeline,
    material_layout: wgpu::BindGroupLayout,
    frame_buffer: wgpu::Buffer,
    model_buffer: wgpu::Buffer,
    frame_bind_group: wgpu::BindGroup,
    fallback: GpuTexture,
    textures: HashMap<String, GpuTexture>,
    materials: HashMap<String, GpuMaterial>,
    meshes: HashMap<String, GpuMesh>,
    instances: BTreeMap<RenderHandle, Instance>,
    sky: Option<(String, wgpu::BindGroup)>,
}

impl Backend {
    fn new(gpu: &Gpu) -> Self {
        let device = &gpu.device;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("bootstrap-raw"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let frame_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("frame"),
            entries: &[
                buffer_entry(0, wgpu::BufferBindingType::Uniform),
                buffer_entry(1, wgpu::BufferBindingType::Storage { read_only: true }),
            ],
        });
        let material_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("material"),
            entries: &[
                buffer_entry(0, wgpu::BufferBindingType::Uniform),
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("world"),
            bind_group_layouts: &[Some(&frame_layout), Some(&material_layout)],
            immediate_size: 0,
        });
        let vertex_attributes =
            wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2];
        let world_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("world"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_world"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: 32,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &vertex_attributes,
                })],
            },
            primitive: wgpu::PrimitiveState {
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_world"),
                compilation_options: Default::default(),
                targets: &[Some(COLOR_FORMAT.into())],
            }),
            multiview_mask: None,
            cache: None,
        });
        let sky_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sky"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_sky"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: Default::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_sky"),
                compilation_options: Default::default(),
                targets: &[Some(COLOR_FORMAT.into())],
            }),
            multiview_mask: None,
            cache: None,
        });
        let frame_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("frame"),
            size: std::mem::size_of::<FrameUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let model_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("models"),
            size: MAX_INSTANCES * 64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let frame_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("frame"),
            layout: &frame_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: frame_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: model_buffer.as_entire_binding(),
                },
            ],
        });
        let fallback = upload_texture(
            gpu,
            "fallback-white",
            1,
            1,
            &[255; 4],
            TextureFilter::Nearest,
            TextureWrap::Repeat,
        );
        Self {
            world_pipeline,
            sky_pipeline,
            material_layout,
            frame_buffer,
            model_buffer,
            frame_bind_group,
            fallback,
            textures: HashMap::new(),
            materials: HashMap::new(),
            meshes: HashMap::new(),
            instances: BTreeMap::new(),
            sky: None,
        }
    }

    /// Apply one `PresentationWorld` delta to the tables.
    fn apply(&mut self, gpu: &Gpu, fixture: &Fixture, ops: &[RenderDiff]) -> Result<(), String> {
        for op in ops {
            match op {
                RenderDiff::DefineTexture { texture } => {
                    self.define_texture(gpu, fixture, texture)?
                }
                RenderDiff::DefineMaterial { material } => self.define_material(gpu, material),
                RenderDiff::DefineStaticMesh { asset } => self.define_mesh(gpu, asset)?,
                RenderDiff::CreateStaticMeshInstance {
                    handle, instance, ..
                } => self.create_instance(*handle, instance),
                RenderDiff::SetSkyBackground { background } => {
                    self.sky = background.as_ref().map(|sky| {
                        let texture = self.textures.get(&sky.texture).unwrap_or(&self.fallback);
                        let bind_group = self.material_bind_group(gpu, [1.0; 4], texture);
                        (sky.texture.clone(), bind_group)
                    });
                }
                // Sprites and atlases are a separate family, excluded for
                // every candidate in this spike.
                _ => {}
            }
        }
        Ok(())
    }

    fn define_texture(
        &mut self,
        gpu: &Gpu,
        fixture: &Fixture,
        texture: &TextureDescriptor,
    ) -> Result<(), String> {
        let image = fixture.texture_pixels(texture)?;
        let uploaded = upload_texture(
            gpu,
            &texture.id,
            image.width,
            image.height,
            &image.pixels,
            texture.filter,
            texture.wrap,
        );
        self.textures.insert(texture.id.clone(), uploaded);
        Ok(())
    }

    fn define_material(&mut self, gpu: &Gpu, material: &RenderMaterialDescriptor) {
        let texture = material
            .texture
            .as_ref()
            .and_then(|id| self.textures.get(id))
            .unwrap_or(&self.fallback);
        let color = std::array::from_fn(|i| material.color[i] * material.texture_tint[i]);
        let bind_group = self.material_bind_group(gpu, color, texture);
        self.materials
            .insert(material.id.clone(), GpuMaterial { bind_group });
    }

    fn material_bind_group(
        &self,
        gpu: &Gpu,
        color: [f32; 4],
        texture: &GpuTexture,
    ) -> wgpu::BindGroup {
        let uniform = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("material"),
                contents: bytemuck::cast_slice(&color),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("material"),
            layout: &self.material_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&texture.sampler),
                },
            ],
        })
    }

    fn define_mesh(&mut self, gpu: &Gpu, asset: &StaticMeshAsset) -> Result<(), String> {
        let MeshPayloadSource::Inline {
            positions,
            normals,
            uvs,
            indices,
            ..
        } = &asset.payload.source
        else {
            return Err(format!(
                "{}: only inline payloads are realized",
                asset.asset
            ));
        };
        let vertex_count = positions.len() / 3;
        let mut interleaved = Vec::with_capacity(vertex_count * 8);
        for v in 0..vertex_count {
            interleaved.extend_from_slice(&positions[v * 3..v * 3 + 3]);
            interleaved.extend_from_slice(&normals[v * 3..v * 3 + 3]);
            match uvs {
                Some(uvs) => interleaved.extend_from_slice(&uvs[v * 2..v * 2 + 2]),
                None => interleaved.extend_from_slice(&[0.0, 0.0]),
            }
        }
        let vertices = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&asset.asset),
                contents: bytemuck::cast_slice(&interleaved),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let index_buffer = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&asset.asset),
                contents: bytemuck::cast_slice(indices),
                usage: wgpu::BufferUsages::INDEX,
            });
        self.meshes.insert(
            asset.asset.clone(),
            GpuMesh {
                vertices,
                indices: index_buffer,
                groups: asset
                    .payload
                    .groups
                    .iter()
                    .map(|group| (group.material_slot, group.start, group.count))
                    .collect(),
                slots: asset
                    .material_slots
                    .iter()
                    .map(|slot| (slot.slot, slot.material.clone()))
                    .collect(),
            },
        );
        Ok(())
    }

    fn create_instance(&mut self, handle: RenderHandle, instance: &StaticMeshInstanceDescriptor) {
        let transform = &instance.transform;
        self.instances.insert(
            handle,
            Instance {
                asset: instance.asset.clone(),
                model: Mat4::from_scale_rotation_translation(
                    Vec3::from(transform.scale),
                    Quat::from_array(transform.rotation),
                    Vec3::from(transform.translation),
                ),
                visible: instance.visible,
                overrides: instance
                    .material_overrides
                    .iter()
                    .map(|slot| (slot.slot, slot.material.clone()))
                    .collect(),
            },
        );
    }

    /// Fixed pass pipeline: frame constants, instance rows, sky, opaque world.
    fn render(&self, gpu: &Gpu, camera: &bootstrap_fixture::Camera, target: &RenderTarget) {
        let (forward, up) = camera_forward_up(camera);
        let eye = Vec3::from(camera.position);
        let view = Mat4::look_to_rh(eye, Vec3::from(forward), Vec3::from(up));
        let projection = Mat4::perspective_rh(
            camera.fov_y_degrees.to_radians(),
            WIDTH as f32 / HEIGHT as f32,
            camera.near,
            camera.far,
        );
        let view_proj = projection * view;
        let key_dir = Vec3::from(neutral_light::KEY_POSITION).normalize();
        let scaled = |rgb: [f32; 3], intensity: f32| {
            [
                rgb[0] * intensity,
                rgb[1] * intensity,
                rgb[2] * intensity,
                0.0,
            ]
        };
        let uniform = FrameUniform {
            view_proj: view_proj.to_cols_array_2d(),
            inv_view_proj: view_proj.inverse().to_cols_array_2d(),
            camera: [eye.x, eye.y, eye.z, 1.0],
            hemi_sky: scaled(
                neutral_light::HEMISPHERE_SKY,
                neutral_light::HEMISPHERE_INTENSITY,
            ),
            hemi_ground: scaled(
                neutral_light::HEMISPHERE_GROUND,
                neutral_light::HEMISPHERE_INTENSITY,
            ),
            key_dir: [key_dir.x, key_dir.y, key_dir.z, 0.0],
            key_color: scaled(neutral_light::KEY_COLOR, neutral_light::KEY_INTENSITY),
        };
        gpu.queue
            .write_buffer(&self.frame_buffer, 0, bytemuck::bytes_of(&uniform));

        let visible: Vec<&Instance> = self.instances.values().filter(|i| i.visible).collect();
        let models: Vec<[[f32; 4]; 4]> =
            visible.iter().map(|i| i.model.to_cols_array_2d()).collect();
        gpu.queue
            .write_buffer(&self.model_buffer, 0, bytemuck::cast_slice(&models));

        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("world"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target.color_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &target.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, &self.frame_bind_group, &[]);
            if let Some((_, sky)) = &self.sky {
                pass.set_pipeline(&self.sky_pipeline);
                pass.set_bind_group(1, sky, &[]);
                pass.draw(0..3, 0..1);
            }
            pass.set_pipeline(&self.world_pipeline);
            for (row, instance) in visible.iter().enumerate() {
                let Some(mesh) = self.meshes.get(&instance.asset) else {
                    continue;
                };
                pass.set_vertex_buffer(0, mesh.vertices.slice(..));
                pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
                for (slot, start, count) in &mesh.groups {
                    let material = instance
                        .overrides
                        .get(slot)
                        .or_else(|| mesh.slots.get(slot))
                        .and_then(|id| self.materials.get(id));
                    let Some(material) = material else { continue };
                    pass.set_bind_group(1, &material.bind_group, &[]);
                    let row = row as u32;
                    pass.draw_indexed(*start..start + count, 0, row..row + 1);
                }
            }
        }
        target.copy_to_readback(&mut encoder);
        gpu.queue.submit([encoder.finish()]);
    }
}

struct RenderTarget {
    color: wgpu::Texture,
    color_view: wgpu::TextureView,
    depth_view: wgpu::TextureView,
    readback: wgpu::Buffer,
    padded_row: u32,
}

impl RenderTarget {
    fn new(gpu: &Gpu) -> Self {
        let size = wgpu::Extent3d {
            width: WIDTH,
            height: HEIGHT,
            depth_or_array_layers: 1,
        };
        let color = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("offscreen-color"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: COLOR_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let depth = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("offscreen-depth"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let padded_row = (WIDTH * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: u64::from(padded_row * HEIGHT),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        Self {
            color_view: color.create_view(&Default::default()),
            depth_view: depth.create_view(&Default::default()),
            color,
            readback,
            padded_row,
        }
    }

    fn copy_to_readback(&self, encoder: &mut wgpu::CommandEncoder) {
        encoder.copy_texture_to_buffer(
            self.color.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &self.readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(self.padded_row),
                    rows_per_image: Some(HEIGHT),
                },
            },
            self.color.size(),
        );
    }

    fn read(&self, gpu: &Gpu) -> Vec<u8> {
        let slice = self.readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, |result| result.expect("map readback"));
        gpu.device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("poll device");
        let mapped = slice.get_mapped_range().expect("mapped range");
        let mut pixels = Vec::with_capacity((WIDTH * HEIGHT * 4) as usize);
        for row in mapped.chunks(self.padded_row as usize) {
            pixels.extend_from_slice(&row[..(WIDTH * 4) as usize]);
        }
        drop(mapped);
        self.readback.unmap();
        pixels
    }
}

fn buffer_entry(binding: u32, ty: wgpu::BufferBindingType) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn upload_texture(
    gpu: &Gpu,
    label: &str,
    width: u32,
    height: u32,
    pixels: &[u8],
    filter: TextureFilter,
    wrap: TextureWrap,
) -> GpuTexture {
    let texture = gpu.device.create_texture_with_data(
        &gpu.queue,
        &wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        },
        wgpu::util::TextureDataOrder::LayerMajor,
        pixels,
    );
    let filter = match filter {
        TextureFilter::Nearest => wgpu::FilterMode::Nearest,
        _ => wgpu::FilterMode::Linear,
    };
    let address = match wrap {
        TextureWrap::Repeat => wgpu::AddressMode::Repeat,
        _ => wgpu::AddressMode::ClampToEdge,
    };
    let sampler = gpu.device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some(label),
        address_mode_u: address,
        address_mode_v: address,
        mag_filter: filter,
        min_filter: filter,
        ..Default::default()
    });
    GpuTexture {
        view: texture.create_view(&Default::default()),
        sampler,
    }
}

fn runtime_gpu() -> Gpu {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .expect("adapter");
    eprintln!("adapter: {:?}", adapter.get_info());
    let (device, queue) =
        pollster::block_on(adapter.request_device(&Default::default())).expect("device");
    Gpu { device, queue }
}

fn main() -> Result<(), String> {
    let fixture = Fixture::load(&Fixture::default_dir())?;
    let gpu = runtime_gpu();
    let mut backend = Backend::new(&gpu);
    backend.apply(&gpu, &fixture, &fixture.baseline())?;
    let target = RenderTarget::new(&gpu);
    backend.render(&gpu, &fixture.camera, &target);
    let pixels = target.read(&gpu);
    write_png(std::path::Path::new("out/raw.png"), WIDTH, HEIGHT, &pixels)?;
    eprintln!(
        "raw: {} textures, {} materials, {} meshes, {} instances",
        backend.textures.len(),
        backend.materials.len(),
        backend.meshes.len(),
        backend.instances.len()
    );
    Ok(())
}
