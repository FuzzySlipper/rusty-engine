//! The sun's shadows traced as rays (`RendererOptions::ray_traced_shadows`,
//! `rusty::ray`): the mask a world view traces, and what the rays bind as
//! group 4 beside the ray scene's TLAS (`ray_scene.rs`). Ambient occlusion's
//! `RayTraced` path (`ambient_occlusion.rs`) binds the same group for its
//! pass (`ray_mask.wgsl` `cs_occlusion`).
//!
//! - **Mask:** a world view draws its opaque depth once more at full size
//!   (the occlusion pre-pass's pipelines) and traces one ray per pixel from
//!   it (`ray_mask.wgsl`); the world pass reads the result. Rays traced in
//!   the world pass's fragments cost 97 to 225 ms at 1440p on an RX 9070 XT
//!   against 9 to 17 ms for the cascades. With the mask and a ray left in
//!   every pipeline for fragments it did not hold, the pass still took 36
//!   to 64 ms, and 7 to 14 ms with none in the opaque pipelines. So only
//!   the blended pipelines hold one (`RAY_TRACE`), for what the mask does
//!   not hold.
//! - **Masked geometry:** a ray that meets a masked material tests the base
//!   texture's alpha where it hit, through the sampler its material binds
//!   and a voxel surface's tiling, times the part's and the vertices'
//!   colour alpha, against the cutoff, as the shadow pass's `fs_shadow`
//!   does. The hit triangle's uvs come from a pool holding each masked
//!   mesh's vertices and each masked range's indices, copied on the GPU
//!   once per mesh geometry; the pool is laid out again when it would
//!   overflow or holds more stale vertices than live.
//! - **Parts:** a row per part id with its range's first index and its
//!   mesh's first vertex in the pool, its base texture's slot, its uv
//!   transform, cutoff and colour alpha. A row is written when it changes;
//!   only masked parts' rows are read, and a part that stops being masked
//!   has its row cleared.
//! - **Textures:** the masked materials' base textures, at most
//!   [`RAY_TEXTURES`]; a material past them tests its colour alpha alone.
//!   Triplanar and stochastic sampling are not followed: a ray reads the
//!   base texture at the mesh's uv.
//! - **Shape:** under a temporal stage each ray leans within the sun's disc
//!   by a blue noise (`BLUE_NOISE`) turned each frame, so the stage gathers a soft
//!   penumbra; otherwise it aims at the sun's centre (hard shadows: MSAA
//!   has nothing to gather noise with).
//!
//! The sun's cascades are not drawn while rays trace its shadows
//! (`frame.rs` `choose_shadows`): its light row's shadow lane reads
//! [`RAY_SUN`].

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Weak};

use crate::tables::{CpuGeometry, GpuMesh, MaterialRef, MeshRef, PartId};
use crate::Renderer;

/// Base textures a ray reads masks from, at most (`ray.wgsl`
/// `ray_textures`).
pub(crate) const RAY_TEXTURES: u32 = 256;
/// A light row's shadow lane for the sun whose shadows are traced as rays
/// (`rusty::types::RAY_SUN`).
pub(crate) const RAY_SUN: f32 = 1_048_576.0;
/// `RayPart` in `ray.wgsl`: eight vectors.
const PART_WORDS: usize = 32;
/// `ray_blue_noise` in `ray.wgsl`: a 64×64 tile, two bytes a texel, made by
/// void and cluster.
const BLUE_NOISE: &[u8; 8192] = include_bytes!("blue_noise.bin");
const PART_BYTES: u64 = (PART_WORDS * 4) as u64;
/// A part's texture slot when it has none (`ray.wgsl` `NO_TEXTURE`).
const NO_TEXTURE: u32 = u32::MAX;
/// `RayPart.slots.w` (`ray.wgsl` `PART_MASKED`, `PART_VOXEL`).
const PART_MASKED: u32 = 1;
const PART_VOXEL: u32 = 2;
/// The ray's start off the surface along its normal at the camera, and how
/// much it grows a metre away, metres: past self-intersection with the
/// surface's own triangles at the depths the world pass draws.
const NORMAL_OFFSET: f32 = 0.02;
const NORMAL_OFFSET_PER_METRE: f32 = 0.0002;
/// The sun's angular radius when no atmosphere sets one: the real sun's.
const SUN_RADIUS_DEGREES: f32 = 0.27;
/// The pool's first size, vertices and indices.
const POOL_VERTICES: u64 = 1 << 14;
const POOL_INDICES: u64 = 1 << 16;
const VERTEX_BYTES: u64 = (crate::pipelines::VERTEX_FLOATS * 4) as u64;

/// A shown part whose material is masked, as the ray scene's walk found it.
#[derive(Clone, PartialEq)]
pub(crate) struct Masked {
    pub part: PartId,
    pub mesh: MeshRef,
    pub first_index: u32,
    pub index_count: u32,
}

/// The pooled vertices and indices of the masked ranges.
struct Pool {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    /// Vertices and indices placed, live or stale.
    vertex_end: u64,
    index_end: u64,
    /// Each mesh's geometry it holds: its source, first vertex and count.
    meshes: HashMap<MeshRef, (Weak<CpuGeometry>, u32, u64)>,
    /// Each masked range's first index, by mesh, first index and count.
    ranges: HashMap<(MeshRef, u32, u32), u32>,
}

impl Pool {
    fn new(device: &wgpu::Device, vertices: u64, indices: u64) -> Self {
        let buffer = |label, size: u64| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: size.max(16),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        Self {
            vertices: buffer("render-wgpu ray vertices", vertices * VERTEX_BYTES),
            indices: buffer("render-wgpu ray indices", indices * 4),
            vertex_end: 0,
            index_end: 0,
            meshes: HashMap::new(),
            ranges: HashMap::new(),
        }
    }

    /// Where `masked`'s indices and its mesh's vertices are, copying what is
    /// not held yet; none when the pool has no room.
    fn place(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        masked: &Masked,
        mesh: &GpuMesh,
    ) -> Option<(u32, u32)> {
        // A skinned mesh keeps the pose it was placed in: a hit reads its uvs
        // and colours, which every pose shares.
        let skinned = matches!(masked.mesh, MeshRef::AnimatedSkinned(..));
        let held = self
            .meshes
            .get(&masked.mesh)
            .filter(|(source, ..)| skinned || std::ptr::eq(source.as_ptr(), Arc::as_ptr(&mesh.cpu)))
            .map(|(_, first, _)| *first);
        let first_vertex = match held {
            Some(first) => first,
            None => {
                let count = mesh.vertices.size() / VERTEX_BYTES;
                if (self.vertex_end + count) * VERTEX_BYTES > self.vertices.size() {
                    return None;
                }
                encoder.copy_buffer_to_buffer(
                    &mesh.vertices,
                    0,
                    &self.vertices,
                    self.vertex_end * VERTEX_BYTES,
                    count * VERTEX_BYTES,
                );
                let first = self.vertex_end as u32;
                self.vertex_end += count;
                self.meshes.insert(
                    masked.mesh.clone(),
                    (Arc::downgrade(&mesh.cpu), first, count),
                );
                // The mesh's ranges index its former geometry.
                self.ranges.retain(|(held, ..), _| *held != masked.mesh);
                first
            }
        };
        let range = (masked.mesh.clone(), masked.first_index, masked.index_count);
        if let Some(first) = self.ranges.get(&range) {
            return Some((*first, first_vertex));
        }
        let count = u64::from(masked.index_count);
        if (self.index_end + count) * 4 > self.indices.size() {
            return None;
        }
        encoder.copy_buffer_to_buffer(
            &mesh.indices,
            u64::from(masked.first_index) * 4,
            &self.indices,
            self.index_end * 4,
            count * 4,
        );
        let first = self.index_end as u32;
        self.index_end += count;
        self.ranges.insert(range, first);
        Some((first, first_vertex))
    }
}

/// A world view size's mask: the opaque depth the pre-pass draws, the
/// per-pixel result the mask pass writes from it (`ray_mask.wgsl`), and
/// the world pipelines' group that reads it.
struct Mask {
    size: (u32, u32),
    depth: wgpu::TextureView,
    params: wgpu::Buffer,
    pass_group: wgpu::BindGroup,
    view: wgpu::TextureView,
    /// The world pipelines' group, and what it was made from (`Bound`).
    world: Option<(Bound, wgpu::BindGroup)>,
}

/// What a group binds: the changes to the pool, rows and textures, and the
/// TLAS (`RayScene::traced`).
type Bound = (u64, u64);

pub(crate) struct RayShadows {
    pool: Pool,
    parts: wgpu::Buffer,
    /// Each pooled part's row as last written.
    rows: HashMap<PartId, [u32; PART_WORDS]>,
    /// The emissive parts among them, for global illumination's light tiles.
    emissive: Vec<crate::ray_gi::EmissivePart>,
    /// Each slot's texture id, in slot order.
    textures: Vec<String>,
    params: wgpu::Buffer,
    blue_noise: wgpu::Buffer,
    /// The scene generation and ray scene walk the rows were laid out for.
    prepared: Option<(u64, u64)>,
    /// Counts the changes to what the groups bind but the TLAS.
    changes: u64,
    /// What the groups bind now; none before a TLAS is built.
    bound: Option<Bound>,
    /// The mask pass's group 4: a cleared mask in place of a view's, which
    /// the pass writes.
    trace_group: Option<(Bound, wgpu::BindGroup)>,
    unbound_mask: wgpu::TextureView,
    pass_layout: wgpu::BindGroupLayout,
    pipeline: Option<wgpu::ComputePipeline>,
    /// Ambient occlusion's pass: its group 0's layout, its pipeline, what
    /// a view's pass reads, and the view's group.
    occlusion_layout: wgpu::BindGroupLayout,
    occlusion_pipeline: Option<wgpu::ComputePipeline>,
    occlusion_params: wgpu::Buffer,
    occlusion_group: Option<wgpu::BindGroup>,
    masks: Vec<Mask>,
    timer: Option<crate::timing::PassTimer>,
    frames: u32,
}

impl RayShadows {
    pub fn new(gpu: &crate::Gpu) -> Self {
        let device = &gpu.device;
        let unbound_mask = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("render-wgpu ray mask unbound"),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: MASK_FORMAT,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&Default::default());
        Self {
            pool: Pool::new(device, POOL_VERTICES, POOL_INDICES),
            parts: parts_buffer(device, 1),
            rows: HashMap::new(),
            emissive: Vec::new(),
            textures: Vec::new(),
            params: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("render-wgpu ray shadows params"),
                size: 16,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            blue_noise: crate::gpu::buffer_init(
                device,
                &wgpu::util::BufferInitDescriptor {
                    label: Some("render-wgpu ray blue noise"),
                    contents: bytemuck::cast_slice(&blue_noise_words()),
                    usage: wgpu::BufferUsages::STORAGE,
                },
            ),
            prepared: None,
            changes: 0,
            bound: None,
            trace_group: None,
            unbound_mask,
            pass_layout: pass_layout(device),
            pipeline: None,
            occlusion_layout: occlusion_layout(device),
            occlusion_pipeline: None,
            occlusion_params: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("render-wgpu ray occlusion params"),
                size: MASK_PARAMS_BYTES,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            occlusion_group: None,
            masks: Vec::new(),
            timer: crate::timing::PassTimer::new(gpu, "ray-mask"),
            frames: 0,
        }
    }

    /// Whether a TLAS is built for this frame's rays.
    pub fn traceable(&self) -> bool {
        self.bound.is_some()
    }

    /// The group of the world pipelines that trace the sun's shadows in a
    /// view of `size`, once `prepare_ray_mask` has made it.
    pub fn world_group(&self, size: (u32, u32)) -> Option<&wgpu::BindGroup> {
        let mask = self.masks.iter().find(|mask| mask.size == size)?;
        mask.world
            .as_ref()
            .filter(|(bound, _)| Some(*bound) == self.bound)
            .map(|(_, group)| group)
    }

    /// What rays bind beside a pass's own groups, once a TLAS is built.
    pub fn trace_group(&self) -> Option<&wgpu::BindGroup> {
        self.trace_group.as_ref().map(|(_, group)| group)
    }

    pub fn emissive(&self) -> &[crate::ray_gi::EmissivePart] {
        &self.emissive
    }

    /// The depth a view of `size` draws its pre-pass into.
    pub fn mask_depth(&self, size: (u32, u32)) -> Option<&wgpu::TextureView> {
        let mask = self.masks.iter().find(|mask| mask.size == size)?;
        Some(&mask.depth)
    }

    pub fn timing(&self) -> crate::GpuPassTiming {
        self.timer.as_ref().map_or_else(
            || crate::timing::untimed("ray-mask"),
            crate::timing::PassTimer::readout,
        )
    }

    pub fn timer(&self) -> Option<&crate::timing::PassTimer> {
        self.timer.as_ref()
    }

    /// Trace a view's occlusion over `region`, as `prepare_ray_occlusion`
    /// readied it. Returns its workgroups.
    pub fn encode_occlusion(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        region: crate::frame::PixelRect,
        timestamp_writes: Option<wgpu::ComputePassTimestampWrites<'_>>,
    ) -> u32 {
        let (Some(pipeline), Some(group), Some((_, trace))) = (
            &self.occlusion_pipeline,
            &self.occlusion_group,
            &self.trace_group,
        ) else {
            return 0;
        };
        let workgroups = (
            region.width.div_ceil(MASK_WORKGROUP),
            region.height.div_ceil(MASK_WORKGROUP),
        );
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("render-wgpu ray occlusion"),
            timestamp_writes,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, group, &[]);
        pass.set_bind_group(4, trace, &[]);
        pass.dispatch_workgroups(workgroups.0, workgroups.1, 1);
        workgroups.0 * workgroups.1
    }

    /// After the view's encoder is submitted.
    pub fn submitted(&mut self) {
        if let Some(timer) = &mut self.timer {
            timer.submitted();
        }
    }
}

/// The mask: the distance to the pixel's surface, negative in shadow.
const MASK_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R32Float;
/// `MaskParams` in `ray_mask.wgsl`.
const MASK_PARAMS_BYTES: u64 = 64 + 16 + 16 + 16;
const MASK_WORKGROUP: u32 = 8;

/// [`BLUE_NOISE`] as `ray_blue_noise` reads it: a word a texel.
fn blue_noise_words() -> Vec<u32> {
    BLUE_NOISE
        .as_chunks::<2>()
        .0
        .iter()
        .map(|[x, y]| u32::from(*x) | u32::from(*y) << 8)
        .collect()
}

/// Group 0 of the mask pass (`ray_mask.wgsl`).
fn pass_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    let compute = wgpu::ShaderStages::COMPUTE;
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("render-wgpu ray mask"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: compute,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Depth,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: compute,
                ty: wgpu::BindingType::StorageTexture {
                    access: wgpu::StorageTextureAccess::WriteOnly,
                    format: MASK_FORMAT,
                    view_dimension: wgpu::TextureViewDimension::D2,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: compute,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    })
}

fn parts_buffer(device: &wgpu::Device, parts: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("render-wgpu ray parts"),
        size: parts.max(1) * PART_BYTES,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

/// Group 0 of ambient occlusion's pass (`ray_mask.wgsl` `cs_occlusion`).
fn occlusion_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    let compute = wgpu::ShaderStages::COMPUTE;
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("render-wgpu ray occlusion"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: compute,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Depth,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: compute,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: compute,
                ty: wgpu::BindingType::StorageTexture {
                    access: wgpu::StorageTextureAccess::WriteOnly,
                    format: crate::ambient_occlusion::OCCLUSION_FORMAT,
                    view_dimension: wgpu::TextureViewDimension::D2,
                },
                count: None,
            },
        ],
    })
}

/// `MaskParams` in `ray_mask.wgsl`.
fn mask_params(
    view_proj: &glam::Mat4,
    eye: glam::Vec3,
    toward: glam::Vec3,
    radius: f32,
    viewport: crate::frame::PixelRect,
) -> [u32; (MASK_PARAMS_BYTES / 4) as usize] {
    let mut params = [0u32; (MASK_PARAMS_BYTES / 4) as usize];
    params[..16].copy_from_slice(&view_proj.inverse().to_cols_array().map(f32::to_bits));
    params[16..19].copy_from_slice(&eye.to_array().map(f32::to_bits));
    params[20..23].copy_from_slice(&toward.to_array().map(f32::to_bits));
    params[23] = radius.to_bits();
    params[24..28].copy_from_slice(&[viewport.x, viewport.y, viewport.width, viewport.height]);
    params
}

/// Group 4 of the world pipelines that trace the sun's shadows
/// (`ray.wgsl`).
pub(crate) fn layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    // The world pass's fragments and the mask pass trace.
    let fragment = wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::COMPUTE;
    let storage = |binding| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: fragment,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: true },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    };
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("render-wgpu ray shadows"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: fragment,
                ty: wgpu::BindingType::AccelerationStructure {
                    vertex_return: false,
                },
                count: None,
            },
            storage(1),
            storage(2),
            storage(3),
            wgpu::BindGroupLayoutEntry {
                binding: 4,
                visibility: fragment,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: std::num::NonZeroU32::new(RAY_TEXTURES),
            },
            wgpu::BindGroupLayoutEntry {
                binding: 5,
                visibility: fragment,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: std::num::NonZeroU32::new(RAY_TEXTURES),
            },
            // Storage, not uniform: a group with a binding array may hold
            // no uniform buffer.
            storage(6),
            wgpu::BindGroupLayoutEntry {
                binding: 7,
                visibility: fragment,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            storage(8),
        ],
    })
}

impl Renderer {
    /// Lay the masked parts' geometry, rows and textures out for the rays,
    /// after `prepare_ray_scene`, when the scene changed.
    pub(crate) fn prepare_ray_shadows(&mut self) {
        let Some(mut shadows) = self.ray_shadows.take() else {
            return;
        };
        let Some(scene) = &self.ray_scene else {
            self.ray_shadows = Some(shadows);
            return;
        };
        let state = (self.scene_generation, scene.walks());
        if shadows.prepared == Some(state) {
            self.ray_shadows = Some(shadows);
            return;
        }
        shadows.prepared = Some(state);
        let masked = scene.masked();
        let device = &self.gpu.device;
        // Meshes and ranges no masked part draws any more are stale; lay
        // the pool out again once they outnumber the live.
        let drawn: HashSet<(MeshRef, u32, u32)> = masked
            .iter()
            .map(|masked| (masked.mesh.clone(), masked.first_index, masked.index_count))
            .collect();
        let meshes: HashSet<&MeshRef> = masked.iter().map(|masked| &masked.mesh).collect();
        let pool = &mut shadows.pool;
        pool.ranges.retain(|range, _| drawn.contains(range));
        pool.meshes.retain(|mesh, _| meshes.contains(mesh));
        let live: u64 = pool.meshes.values().map(|(.., count)| count).sum();
        if pool.vertex_end > POOL_VERTICES.max(2 * live) {
            let sizes = (pool.vertices.size() / VERTEX_BYTES, pool.indices.size() / 4);
            *pool = Pool::new(device, sizes.0, sizes.1);
            shadows.changes += 1;
        }
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("render-wgpu ray shadows pool"),
        });
        let mut placed = self.place_masked(&mut shadows.pool, &mut encoder, masked);
        if placed.is_none() {
            // Out of room: lay it all out again in twice what it needs.
            let vertices: u64 = meshes
                .iter()
                .filter_map(|mesh| self.mesh(mesh))
                .map(|mesh| mesh.vertices.size() / VERTEX_BYTES)
                .sum();
            let indices: u64 = drawn.iter().map(|(_, _, count)| u64::from(*count)).sum();
            shadows.pool = Pool::new(
                device,
                (vertices * 2).max(POOL_VERTICES),
                (indices * 2).max(POOL_INDICES),
            );
            shadows.changes += 1;
            encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render-wgpu ray shadows pool"),
            });
            placed = self.place_masked(&mut shadows.pool, &mut encoder, masked);
        }
        let rows = self.pooled_rows(&mut shadows, placed.unwrap_or_default());
        self.write_ray_rows(&mut shadows, rows);
        self.gpu.queue.submit([encoder.finish()]);
        self.ray_shadows = Some(shadows);
    }

    /// Each masked part's place in `pool`; none when the pool has no room
    /// for them all.
    fn place_masked<'a>(
        &self,
        pool: &mut Pool,
        encoder: &mut wgpu::CommandEncoder,
        masked: &'a [Masked],
    ) -> Option<Vec<(&'a Masked, u32, u32)>> {
        let mut placed = Vec::with_capacity(masked.len());
        for entry in masked {
            let Some(mesh) = self.mesh(&entry.mesh) else {
                continue;
            };
            let (first_index, first_vertex) = pool.place(encoder, entry, mesh)?;
            placed.push((entry, first_index, first_vertex));
        }
        Some(placed)
    }

    /// The pooled parts' rows, and the texture slots they take (the group
    /// is made again when those change). Masked parts take theirs first:
    /// without one they lose their cut-out, and the others a colour they
    /// keep as a mean.
    fn pooled_rows(
        &self,
        shadows: &mut RayShadows,
        mut placed: Vec<(&Masked, u32, u32)>,
    ) -> HashMap<PartId, [u32; PART_WORDS]> {
        let parts = &self.tables.parts;
        let mut rows = HashMap::with_capacity(placed.len());
        let mut textures: Vec<String> = Vec::new();
        let mut emissive = Vec::new();
        placed.sort_by_key(|(entry, ..)| {
            !parts.state[entry.part as usize]
                .class
                .features
                .contains(crate::shaders::Features::MASK)
        });
        for (entry, first_index, first_vertex) in placed {
            let Some(Some(part)) = parts.meta.get(entry.part as usize) else {
                continue;
            };
            let at = entry.part as usize * crate::tables::PART_ROW_FLOATS;
            let colour = &parts.gpu[at + 28..at + 32];
            let emission = &parts.gpu[at + 32..at + 35];
            let (mut row, mean) = match &part.material {
                MaterialRef::Retained(material) => {
                    let Some(row) = self.tables.materials.get(*material) else {
                        continue;
                    };
                    let size = row
                        .descriptor
                        .texture
                        .as_ref()
                        .and_then(|texture| self.tables.textures.get(texture))
                        .map(|texture| texture.size);
                    let params = crate::apply::MaterialParams::of(&row.descriptor, size);
                    let slot = match &row.descriptor.texture {
                        Some(texture) if self.tables.textures.contains_key(texture) => {
                            match textures.iter().position(|held| held == texture) {
                                Some(slot) => slot as u32,
                                None if textures.len() < RAY_TEXTURES as usize => {
                                    textures.push(texture.clone());
                                    textures.len() as u32 - 1
                                }
                                None => NO_TEXTURE,
                            }
                        }
                        _ => NO_TEXTURE,
                    };
                    let [a, b, c, d, e, f] = row
                        .maps
                        .base
                        .unwrap_or(crate::glb::UvTransform::IDENTITY)
                        .0
                        .map(f32::to_bits);
                    let (tile, rect) =
                        params
                            .voxel_surface
                            .map_or(([0.0; 4], [0.0; 4]), |surface| {
                                (
                                    [
                                        surface.tile_scale[0],
                                        surface.tile_scale[1],
                                        surface.tile_origin[0],
                                        surface.tile_origin[1],
                                    ],
                                    [
                                        surface.uv_min[0],
                                        surface.uv_min[1],
                                        surface.uv_max[0],
                                        surface.uv_max[1],
                                    ],
                                )
                            });
                    let kind = u32::from(params.alpha_cutoff.is_some()) * PART_MASKED
                        + u32::from(params.voxel_surface.is_some()) * PART_VOXEL;
                    let mut row_words = [0u32; PART_WORDS];
                    row_words[..4].copy_from_slice(&[first_index, first_vertex, slot, kind]);
                    row_words[4..8].copy_from_slice(&[
                        a,
                        b,
                        c,
                        params.alpha_cutoff.unwrap_or(0.0).to_bits(),
                    ]);
                    row_words[8..11].copy_from_slice(&[d, e, f]);
                    row_words[12..16].copy_from_slice(&tile.map(f32::to_bits));
                    row_words[16..20].copy_from_slice(&rect.map(f32::to_bits));
                    row_words[23] = params.roughness.to_bits();
                    row_words[27] = params.metalness.to_bits();
                    let mean = (slot == NO_TEXTURE)
                        .then(|| self.tables.material_means.get(&row.descriptor.id))
                        .flatten()
                        .copied()
                        .unwrap_or([1.0; 3]);
                    (row_words, mean)
                }
                // Lit but untextured, or flat: no texture, full roughness.
                MaterialRef::LitFallback | MaterialRef::Unlit => {
                    let mut row_words = [0u32; PART_WORDS];
                    row_words[..4].copy_from_slice(&[first_index, first_vertex, NO_TEXTURE, 0]);
                    row_words[23] = 1.0f32.to_bits();
                    (row_words, [1.0; 3])
                }
            };
            row[11] = colour[3].to_bits();
            for channel in 0..3 {
                row[20 + channel] = (colour[channel] * mean[channel]).to_bits();
                row[24 + channel] = emission[channel].to_bits();
            }
            // A part scattered in copies is found by rays alone: its row has
            // no one transform to sample a light from.
            if emission.iter().any(|channel| *channel > 0.0)
                && parts.state[entry.part as usize].scatter_level.is_none()
            {
                let triangles = entry.index_count / 3;
                row[28] = triangles;
                let model = &parts.gpu[at..at + 16];
                emissive.push(crate::ray_gi::EmissivePart {
                    part: entry.part,
                    rows: std::array::from_fn(|axis| {
                        std::array::from_fn(|column| model[column * 4 + axis])
                    }),
                    triangles,
                });
            }
            rows.insert(entry.part, row);
        }
        shadows.emissive = emissive;
        if textures != shadows.textures {
            shadows.textures = textures;
            shadows.changes += 1;
        }
        rows
    }

    /// Write the rows that changed, and clear those of parts no longer
    /// masked.
    fn write_ray_rows(&self, shadows: &mut RayShadows, rows: HashMap<PartId, [u32; PART_WORDS]>) {
        let queue = &self.gpu.queue;
        let needed = rows.keys().max().map_or(1, |part| u64::from(*part) + 1);
        if needed * PART_BYTES > shadows.parts.size() {
            // A new buffer starts cleared; every row is written into it.
            shadows.parts = parts_buffer(&self.gpu.device, needed.next_power_of_two());
            shadows.rows.clear();
            shadows.changes += 1;
        }
        for (part, row) in &rows {
            if shadows.rows.get(part) != Some(row) {
                queue.write_buffer(
                    &shadows.parts,
                    u64::from(*part) * PART_BYTES,
                    bytemuck::cast_slice(row),
                );
            }
        }
        for part in shadows.rows.keys().filter(|part| !rows.contains_key(part)) {
            queue.write_buffer(
                &shadows.parts,
                u64::from(*part) * PART_BYTES,
                bytemuck::cast_slice(&[0u32; PART_WORDS]),
            );
        }
        shadows.rows = rows;
    }

    /// This frame's rays' shape (`soft` under a temporal stage), after the
    /// frame's TLAS is built (`ray_scene.rs`), and what the groups bind.
    pub(crate) fn bind_ray_shadows(&mut self, soft: bool) {
        let (Some(shadows), Some(scene)) = (&mut self.ray_shadows, &self.ray_scene) else {
            return;
        };
        shadows.bound = scene
            .traced()
            .map(|(_, generation)| (shadows.changes, generation));
        let radius = self
            .tables
            .atmosphere
            .as_ref()
            .map_or(SUN_RADIUS_DEGREES, |atmosphere| {
                atmosphere.sun_radius_degrees
            });
        let spread = if soft {
            radius.max(SUN_RADIUS_DEGREES).to_radians().tan()
        } else {
            0.0
        };
        shadows.frames = (shadows.frames + 1) % (1 << 20);
        // Without a temporal stage to gather it, noise that changes each
        // frame would flicker.
        let params = [
            spread,
            if soft { shadows.frames as f32 } else { 0.0 },
            NORMAL_OFFSET,
            NORMAL_OFFSET_PER_METRE,
        ];
        self.gpu
            .queue
            .write_buffer(&shadows.params, 0, bytemuck::cast_slice(&params));
        if let Some(timer) = &mut shadows.timer {
            timer.collect(&self.gpu);
        }
    }

    /// Group 4 of the world pipelines or the mask pass, over `mask`.
    fn ray_group(&self, shadows: &RayShadows, mask: &wgpu::TextureView) -> Option<wgpu::BindGroup> {
        let layout = self.layouts.ray.as_ref()?;
        let (tlas, _) = self.ray_scene.as_ref()?.traced()?;
        // Every slot holds a view and its sampler, as its material binds
        // them; those past the textures repeat white.
        let (mut views, mut samplers): (Vec<&wgpu::TextureView>, Vec<&wgpu::Sampler>) = shadows
            .textures
            .iter()
            .map(|texture| {
                self.tables
                    .textures
                    .get(texture)
                    .unwrap_or(&self.white)
                    .material_binding()
            })
            .unzip();
        let (white_view, white_sampler) = self.white.material_binding();
        views.resize(RAY_TEXTURES as usize, white_view);
        samplers.resize(RAY_TEXTURES as usize, white_sampler);
        fn entry(binding: u32, buffer: &wgpu::Buffer) -> wgpu::BindGroupEntry<'_> {
            wgpu::BindGroupEntry {
                binding,
                resource: buffer.as_entire_binding(),
            }
        }
        Some(
            self.gpu
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("render-wgpu ray shadows"),
                    layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: tlas.as_binding(),
                        },
                        entry(1, &shadows.parts),
                        entry(2, &shadows.pool.vertices),
                        entry(3, &shadows.pool.indices),
                        wgpu::BindGroupEntry {
                            binding: 4,
                            resource: wgpu::BindingResource::TextureViewArray(&views),
                        },
                        wgpu::BindGroupEntry {
                            binding: 5,
                            resource: wgpu::BindingResource::SamplerArray(&samplers),
                        },
                        entry(6, &shadows.params),
                        wgpu::BindGroupEntry {
                            binding: 7,
                            resource: wgpu::BindingResource::TextureView(mask),
                        },
                        entry(8, &shadows.blue_noise),
                    ],
                }),
        )
    }

    /// Ready the group 4 of a compute pass that traces. Whether it can.
    pub(crate) fn refresh_ray_trace_group(&mut self) -> bool {
        let Some(mut shadows) = self.ray_shadows.take() else {
            return false;
        };
        self.refresh_trace_group(&mut shadows);
        let ready = shadows.bound.is_some() && shadows.trace_group.is_some();
        self.ray_shadows = Some(shadows);
        ready
    }

    /// The mask pass's and ambient occlusion's group 4, made again when
    /// what it binds changed.
    fn refresh_trace_group(&self, shadows: &mut RayShadows) {
        let Some(bound) = shadows.bound else {
            return;
        };
        if shadows
            .trace_group
            .as_ref()
            .is_none_or(|(at, _)| *at != bound)
        {
            shadows.trace_group = self
                .ray_group(shadows, &shadows.unbound_mask)
                .map(|group| (bound, group));
        }
    }

    /// Ready a view's ray-traced occlusion: its group over the pre-pass's
    /// `depth` and the `occlusion` the pass writes, and what the pass reads
    /// for `region` of them.
    pub(crate) fn prepare_ray_occlusion(
        &mut self,
        view_proj: &glam::Mat4,
        eye: glam::Vec3,
        radius: f32,
        region: crate::frame::PixelRect,
        depth: &wgpu::TextureView,
        occlusion: &wgpu::TextureView,
    ) {
        let Some(mut shadows) = self.ray_shadows.take() else {
            return;
        };
        self.refresh_trace_group(&mut shadows);
        let device = &self.gpu.device;
        if shadows.occlusion_pipeline.is_none() {
            let module = self
                .layouts
                .shaders
                .module(device, crate::shaders::Entry::RayMask, Default::default())
                .expect("the ray mask composes");
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("render-wgpu ray occlusion"),
                bind_group_layouts: &[
                    Some(&shadows.occlusion_layout),
                    None,
                    None,
                    None,
                    self.layouts.ray.as_ref(),
                ],
                immediate_size: 0,
            });
            shadows.occlusion_pipeline = Some(device.create_compute_pipeline(
                &wgpu::ComputePipelineDescriptor {
                    label: Some("render-wgpu ray occlusion"),
                    layout: Some(&layout),
                    module: &module,
                    entry_point: Some("cs_occlusion"),
                    compilation_options: Default::default(),
                    cache: None,
                },
            ));
        }
        let params = mask_params(view_proj, eye, glam::Vec3::ZERO, radius, region);
        self.gpu
            .queue
            .write_buffer(&shadows.occlusion_params, 0, bytemuck::cast_slice(&params));
        shadows.occlusion_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("render-wgpu ray occlusion"),
            layout: &shadows.occlusion_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(depth),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: shadows.occlusion_params.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(occlusion),
                },
            ],
        }));
        self.ray_shadows = Some(shadows);
    }

    /// Make a world view of `size` its mask and its groups, and write what
    /// its mask pass reads: the pre-pass's camera, the sun (`toward`) and
    /// the viewport. Returns whether the view traces.
    pub(crate) fn prepare_ray_mask(
        &mut self,
        size: (u32, u32),
        view_proj: &glam::Mat4,
        eye: glam::Vec3,
        toward: glam::Vec3,
        viewport: crate::frame::PixelRect,
    ) -> bool {
        let Some(mut shadows) = self.ray_shadows.take() else {
            return false;
        };
        let device = &self.gpu.device;
        if !shadows.masks.iter().any(|mask| mask.size == size) {
            let texture = |label, format, usage| {
                device
                    .create_texture(&wgpu::TextureDescriptor {
                        label: Some(label),
                        size: wgpu::Extent3d {
                            width: size.0,
                            height: size.1,
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format,
                        usage,
                        view_formats: &[],
                    })
                    .create_view(&Default::default())
            };
            let depth = texture(
                "render-wgpu ray mask depth",
                crate::target::DEPTH_FORMAT,
                wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            );
            let view = texture(
                "render-wgpu ray mask",
                MASK_FORMAT,
                wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            );
            let params = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("render-wgpu ray mask params"),
                size: MASK_PARAMS_BYTES,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let pass_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("render-wgpu ray mask"),
                layout: &shadows.pass_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&depth),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: params.as_entire_binding(),
                    },
                ],
            });
            shadows.masks.push(Mask {
                size,
                depth,
                params,
                pass_group,
                view,
                world: None,
            });
        }
        let ready = match shadows.bound {
            None => false,
            Some(bound) => {
                self.refresh_trace_group(&mut shadows);
                let at = shadows
                    .masks
                    .iter()
                    .position(|mask| mask.size == size)
                    .expect("made above");
                if shadows.masks[at]
                    .world
                    .as_ref()
                    .is_none_or(|(was, _)| *was != bound)
                {
                    let group = self.ray_group(&shadows, &shadows.masks[at].view);
                    shadows.masks[at].world = group.map(|group| (bound, group));
                }
                let params = mask_params(view_proj, eye, toward, 0.0, viewport);
                self.gpu.queue.write_buffer(
                    &shadows.masks[at].params,
                    0,
                    bytemuck::cast_slice(&params),
                );
                shadows.trace_group.is_some() && shadows.masks[at].world.is_some()
            }
        };
        if ready && shadows.pipeline.is_none() {
            let module = self
                .layouts
                .shaders
                .module(device, crate::shaders::Entry::RayMask, Default::default())
                .expect("the ray mask composes");
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("render-wgpu ray mask"),
                bind_group_layouts: &[
                    Some(&shadows.pass_layout),
                    None,
                    None,
                    None,
                    self.layouts.ray.as_ref(),
                ],
                immediate_size: 0,
            });
            shadows.pipeline = Some(device.create_compute_pipeline(
                &wgpu::ComputePipelineDescriptor {
                    label: Some("render-wgpu ray mask"),
                    layout: Some(&layout),
                    module: &module,
                    entry_point: Some("cs_mask"),
                    compilation_options: Default::default(),
                    cache: None,
                },
            ));
        }
        self.ray_shadows = Some(shadows);
        ready
    }

    /// Trace a world view's mask from the depth its pre-pass drew, then
    /// read the pass's time.
    pub(crate) fn encode_ray_mask(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        size: (u32, u32),
        viewport: crate::frame::PixelRect,
    ) {
        let Some(shadows) = self.ray_shadows.as_mut() else {
            return;
        };
        let (Some(mask), Some(pipeline), Some((_, trace))) = (
            shadows.masks.iter().find(|mask| mask.size == size),
            &shadows.pipeline,
            &shadows.trace_group,
        ) else {
            return;
        };
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("render-wgpu ray mask"),
                timestamp_writes: shadows
                    .timer
                    .as_ref()
                    .and_then(|timer| timer.compute_writes_between(false, true)),
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &mask.pass_group, &[]);
            pass.set_bind_group(4, trace, &[]);
            pass.dispatch_workgroups(
                viewport.width.div_ceil(MASK_WORKGROUP),
                viewport.height.div_ceil(MASK_WORKGROUP),
                1,
            );
        }
        if let Some(timer) = &mut shadows.timer {
            timer.resolve(encoder);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Neighbours of blue noise differ by more than white noise's third of
    /// the range (0.39 here), on both channels, and each channel holds every value
    /// evenly: what a temporal stage gathers without clumps.
    #[test]
    fn the_noise_is_blue_and_even() {
        const SIZE: usize = 64;
        let texels = BLUE_NOISE.as_chunks::<2>().0;
        for channel in 0..2 {
            let value = |x: usize, y: usize| {
                f32::from(texels[(y % SIZE) * SIZE + x % SIZE][channel]) / 255.0
            };
            let mut difference = 0.0;
            for y in 0..SIZE {
                for x in 0..SIZE {
                    difference += (value(x, y) - value(x + 1, y)).abs();
                    difference += (value(x, y) - value(x, y + 1)).abs();
                }
            }
            let mean = difference / (2 * SIZE * SIZE) as f32;
            assert!(
                mean > 0.37,
                "channel {channel}: neighbours differ by {mean}"
            );
            let mut counts = [0u32; 256];
            texels
                .iter()
                .for_each(|texel| counts[usize::from(texel[channel])] += 1);
            assert!(counts.iter().all(|count| *count == 16), "channel {channel}");
        }
    }
}
