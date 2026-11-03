//! The ray scene: acceleration structures of the shown world, for the
//! ray-traced settings to trace against.
//!
//! It is a reading of the retained tables, the rows the world pass draws:
//! no product API and no second scene. It exists only while something
//! asks for it ([`Renderer::set_ray_scene`]); otherwise nothing is built.
//!
//! - **BLAS:** one per drawn index range of a mesh (a part's mesh group),
//!   built from the mesh's own buffers (position at offset 0 of the 48-byte
//!   vertex). A range drawn with an opaque material is `OPAQUE`; a masked
//!   one is not, so a ray's candidate loop can alpha-test it. A replaced
//!   payload (a voxel chunk) or a new skinned pose is new geometry and
//!   builds a new BLAS; the old one is dropped once the TLAS stops using it.
//!   wgpu 30 has no refit, so a skinned mesh rebuilds per pose. Other BLAS
//!   are compacted once built. Builds and compactions are budgeted per frame;
//!   a range waiting for its first build is not in the TLAS yet.
//! - **TLAS:** one instance per shown, triangle, scene-layer part that
//!   casts or is opaque, on wgpu's safe path. The 24-bit custom data is the
//!   part id; the mask says what the instance is to a ray ([`MASK_SHADOW`],
//!   [`MASK_GI`]). Scattered copies are not in it.
//!
//! Morph targets, product `displace` stages and wind are vertex-stage
//! deformations: rays see rest geometry.
//!
//! Ray queries cost some devices startup time and memory to ask for, so a
//! device has them only when made for a ray-traced setting
//! ([`Gpu::headless_with_ray_queries`]); otherwise the ray scene is refused
//! as `NoRayQueries`.
//!
//! This is the only module of the crate allowed unsafe code (approved by
//! Patch on #10308): the experimental ray-query opt-in and wgpu-hal calls.
//! Every `unsafe` block states the invariant it relies on, and
//! `tests/unsafe_code.rs` fails on unsafe code anywhere else in the crate.

#![allow(unsafe_code)]

use std::collections::HashMap;
use std::sync::{Arc, Weak};
use std::time::Instant;

use crate::tables::{CpuGeometry, MeshRef, PartId, Topology};
use crate::{Gpu, Renderer, SettingRefusal};

/// What a device needs for the ray-traced settings: ray queries, and the
/// binding arrays a ray's hit reads its material through.
const FEATURES: wgpu::Features = wgpu::Features::EXPERIMENTAL_RAY_QUERY
    .union(wgpu::Features::TEXTURE_BINDING_ARRAY)
    .union(wgpu::Features::BUFFER_BINDING_ARRAY)
    .union(wgpu::Features::SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING)
    .union(wgpu::Features::PARTIALLY_BOUND_BINDING_ARRAY);

/// Instance mask bit: the part casts shadows.
pub(crate) const MASK_SHADOW: u8 = 1;
/// Instance mask bit: the part is opaque, so indirect light bounces off it.
pub(crate) const MASK_GI: u8 = 2;

/// Triangles built into new BLAS per frame; the rest wait. A skinned mesh
/// whose new pose waits keeps its last BLAS meanwhile.
const BUILD_TRIANGLES_PER_FRAME: u64 = 1 << 20;
/// BLAS compacted per frame.
const COMPACTIONS_PER_FRAME: usize = 256;
/// The bytes of the interleaved vertex the meshes share (`pipelines.rs`).
const VERTEX_BYTES: u64 = (crate::pipelines::VERTEX_FLOATS * 4) as u64;
/// The fewest instances a TLAS is made for; it grows by doubling.
const MIN_TLAS_INSTANCES: u32 = 256;

/// Add ray queries, the binding arrays and their limits to `descriptor`
/// when a GPU (not a software adapter) has them all, for a device made
/// while a ray-traced setting is selected ([`Gpu::headless_with_ray_queries`]).
/// llvmpipe advertises ray queries too; the ray-traced settings refuse it,
/// so its device is made as before.
pub(crate) fn request(adapter: &wgpu::Adapter, descriptor: &mut wgpu::DeviceDescriptor<'_>) {
    if adapter.get_info().device_type == wgpu::DeviceType::Cpu
        || !adapter.features().contains(FEATURES)
    {
        return;
    }
    let granted = adapter.limits();
    descriptor.required_features |= FEATURES;
    descriptor.required_limits = std::mem::take(&mut descriptor.required_limits)
        .using_acceleration_structure_values(granted.clone());
    descriptor
        .required_limits
        .max_binding_array_elements_per_shader_stage =
        granted.max_binding_array_elements_per_shader_stage;
    descriptor
        .required_limits
        .max_binding_array_sampler_elements_per_shader_stage =
        granted.max_binding_array_sampler_elements_per_shader_stage;
    // SAFETY: `ExperimentalFeatures::enabled` asks the caller to accept that
    // experimental APIs may hold bugs and to report them. The renderer uses
    // the experimental ray queries only through this module, only on a
    // device that advertises them, and keeps the BLAS a built TLAS
    // references alive (see `RayScene`).
    descriptor.experimental_features = unsafe { wgpu::ExperimentalFeatures::enabled() };
}

/// `BLAS_INPUT` on a device with ray queries, for the buffers meshes draw
/// from; nothing otherwise.
pub(crate) fn mesh_input(device: &wgpu::Device) -> wgpu::BufferUsages {
    if device
        .features()
        .contains(wgpu::Features::EXPERIMENTAL_RAY_QUERY)
    {
        wgpu::BufferUsages::BLAS_INPUT
    } else {
        wgpu::BufferUsages::empty()
    }
}

/// Why the ray-traced settings cannot draw on this device, if they cannot,
/// and the detail.
pub(crate) fn refusal(gpu: &Gpu) -> Option<(SettingRefusal, String)> {
    if gpu.is_software() {
        return Some((
            SettingRefusal::SoftwareAdapter,
            format!("{} is a software adapter", gpu.adapter.get_info().name),
        ));
    }
    if gpu.device.features().contains(FEATURES) {
        return None;
    }
    let name = gpu.adapter.get_info().name;
    let missing = FEATURES - gpu.adapter.features();
    Some((
        SettingRefusal::NoRayQueries,
        if missing.is_empty() {
            format!("the device on {name} was made without ray queries: no ray-traced setting was selected when it was made")
        } else {
            format!("{name} has no {missing:?}")
        },
    ))
}

/// The ray scene's state, for diagnostics and evidence
/// ([`crate::GpuReadout::ray_scene`]).
#[derive(Debug, Clone, PartialEq)]
pub struct RaySceneReadout {
    /// Why it is not built on this device, and the detail.
    pub refused: Option<(SettingRefusal, String)>,
    /// BLAS held, and the triangles in them.
    pub blases: u32,
    pub triangles: u64,
    /// The bytes the BLAS were built at, by the driver's size query; a
    /// compacted BLAS takes less, which wgpu does not report.
    pub blas_bytes: u64,
    /// BLAS compacted so far, of those held.
    pub compacted: u32,
    /// Index ranges waiting for their first build (over the frame budget).
    pub waiting: u32,
    /// Instances in the TLAS.
    pub tlas_instances: u32,
    /// BLAS built and TLAS builds since the ray scene was made.
    pub blas_builds: u64,
    pub tlas_builds: u64,
    /// CPU milliseconds the last build took to encode and submit, and all
    /// builds together since the ray scene was made.
    pub build_cpu_ms: f64,
    pub build_cpu_ms_total: f64,
}

/// What a BLAS holds: one index range of one mesh, and whether it is
/// opaque to rays.
#[derive(Clone, PartialEq, Eq, Hash)]
struct BlasKey {
    mesh: MeshRef,
    first_index: u32,
    index_count: u32,
    opaque: bool,
}

struct BlasEntry {
    blas: wgpu::Blas,
    /// The geometry it was built from: a mesh whose geometry is another
    /// (a replaced payload, a new pose) builds again.
    source: Weak<CpuGeometry>,
    triangles: u64,
    bytes: u64,
    compaction: Compaction,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Compaction {
    /// Rebuilt per pose: never compacted.
    Never,
    /// Built; compaction asked for once the build completes.
    Preparing,
    Compacted,
}

/// A shown part's instance: its BLAS, transform (3×4, row-major) and mask.
struct Instance {
    key: BlasKey,
    part: PartId,
    transform: [f32; 12],
    mask: u8,
}

pub(crate) struct RayScene {
    blases: HashMap<BlasKey, BlasEntry>,
    tlas: Option<wgpu::Tlas>,
    /// The instances from the last walk of the parts.
    instances: Vec<Instance>,
    /// The parts changed since the last walk (or ranges still wait).
    stale: bool,
    /// The TLAS holds other instances or BLAS than the last walk found.
    tlas_dirty: bool,
    timer: Option<crate::timing::PassTimer>,
    probe: Option<wgpu::ComputePipeline>,
    waiting: u32,
    blas_builds: u64,
    tlas_builds: u64,
    build_cpu_ms: f64,
    build_cpu_ms_total: f64,
}

impl RayScene {
    fn new(gpu: &Gpu) -> Self {
        Self {
            blases: HashMap::new(),
            tlas: None,
            instances: Vec::new(),
            stale: true,
            tlas_dirty: true,
            timer: crate::timing::PassTimer::new(gpu, "ray-scene"),
            probe: None,
            waiting: 0,
            blas_builds: 0,
            tlas_builds: 0,
            build_cpu_ms: 0.0,
            build_cpu_ms_total: 0.0,
        }
    }

    pub fn readout(&self) -> RaySceneReadout {
        RaySceneReadout {
            refused: None,
            blases: self.blases.len() as u32,
            triangles: self.blases.values().map(|entry| entry.triangles).sum(),
            blas_bytes: self.blases.values().map(|entry| entry.bytes).sum(),
            compacted: self
                .blases
                .values()
                .filter(|entry| entry.compaction == Compaction::Compacted)
                .count() as u32,
            waiting: self.waiting,
            tlas_instances: self
                .instances
                .iter()
                .filter(|instance| self.blases.contains_key(&instance.key))
                .count() as u32,
            blas_builds: self.blas_builds,
            tlas_builds: self.tlas_builds,
            build_cpu_ms: self.build_cpu_ms,
            build_cpu_ms_total: self.build_cpu_ms_total,
        }
    }

    pub fn timing(&self) -> crate::GpuPassTiming {
        self.timer.as_ref().map_or_else(
            || crate::timing::untimed("ray-scene"),
            crate::timing::PassTimer::readout,
        )
    }
}

/// A ray for [`Renderer::trace_rays`]: from `origin` along `direction`, up
/// to `reach`, seeing the instances whose mask shares a bit with `mask`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProbeRay {
    pub origin: [f32; 3],
    pub direction: [f32; 3],
    pub reach: f32,
    pub mask: u8,
}

/// The closest hit of a [`ProbeRay`]: how far along it, and the node whose
/// part it hit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProbeHit {
    pub distance: f32,
    pub node: Option<render_model::RenderHandle>,
    pub front_face: bool,
}

impl Renderer {
    /// Build the ray scene and keep it current, or with `false` drop it.
    /// The ray-traced settings ask for it; tools and tests may too.
    #[doc(hidden)]
    pub fn set_ray_scene(&mut self, wanted: bool) {
        if !wanted {
            self.ray_scene = None;
        } else if self.ray_scene.is_none() && refusal(&self.gpu).is_none() {
            self.ray_scene = Some(RayScene::new(&self.gpu));
        }
        self.ray_scene_wanted = wanted;
    }

    /// The ray scene's readout while it is asked for.
    pub(crate) fn ray_scene_readout(&self) -> Option<RaySceneReadout> {
        if !self.ray_scene_wanted {
            return None;
        }
        Some(match (&self.ray_scene, refusal(&self.gpu)) {
            (Some(scene), _) => scene.readout(),
            (None, refused) => RaySceneReadout {
                refused,
                blases: 0,
                triangles: 0,
                blas_bytes: 0,
                compacted: 0,
                waiting: 0,
                tlas_instances: 0,
                blas_builds: 0,
                tlas_builds: 0,
                build_cpu_ms: 0.0,
                build_cpu_ms_total: 0.0,
            },
        })
    }

    /// Bring the ray scene up to date with the parts, after `prepare` has
    /// uploaded them: `changed` when parts were added, removed or moved.
    pub(crate) fn prepare_ray_scene(&mut self, changed: bool) {
        let Some(mut scene) = self.ray_scene.take() else {
            return;
        };
        if let Some(timer) = &mut scene.timer {
            timer.collect(&self.gpu);
        }
        scene.stale |= changed;
        let mut builds: Vec<BlasKey> = Vec::new();
        if std::mem::take(&mut scene.stale) {
            builds = self.walk_parts(&mut scene);
        }
        let compactions = compact(&self.gpu, &mut scene);
        if !builds.is_empty() || compactions > 0 {
            scene.tlas_dirty = true;
        }
        if scene.tlas_dirty {
            self.build(&mut scene, &builds);
        }
        self.ray_scene = Some(scene);
    }

    /// Gather the shown parts' instances, and the BLAS to build this frame
    /// within the budget: new ranges, and ranges whose geometry changed.
    fn walk_parts(&self, scene: &mut RayScene) -> Vec<BlasKey> {
        let parts = &self.tables.parts;
        let mut instances = Vec::new();
        let mut used = std::collections::HashSet::new();
        let mut builds = Vec::new();
        let mut budget = BUILD_TRIANGLES_PER_FRAME;
        scene.waiting = 0;
        for (id, part) in parts.meta.iter().enumerate() {
            let Some(part) = part else { continue };
            let state = &parts.state[id];
            if !state.shown
                || state.layer != render_model::RenderLayer::Scene
                || state.copies.is_some()
                || state.scatter_level.is_some()
                || state.class.lines
                || part.wireframe
                || part.index_count < 3
                || id >= 1 << 24
            {
                continue;
            }
            let mut mask = 0;
            if state.casts_shadows && state.class.shadow {
                mask |= MASK_SHADOW;
            }
            if !state.class.blend {
                mask |= MASK_GI;
            }
            if mask == 0 {
                continue;
            }
            let Some(mesh) = self.mesh(&part.mesh) else {
                continue;
            };
            if mesh.topology != Topology::Triangles {
                continue;
            }
            let key = BlasKey {
                mesh: part.mesh.clone(),
                first_index: part.first_index,
                index_count: part.index_count,
                opaque: !state.class.blend && !state.class.features.caster_fragment(),
            };
            let current = scene
                .blases
                .get(&key)
                .is_some_and(|entry| ptr_eq(&entry.source, &mesh.cpu));
            if !current && used.insert(key.clone()) {
                let triangles = u64::from(part.index_count / 3);
                if triangles <= budget {
                    budget -= triangles;
                    builds.push(key.clone());
                } else {
                    scene.waiting += 1;
                }
            } else {
                used.insert(key.clone());
            }
            let row = id * crate::tables::PART_ROW_FLOATS;
            let model = &parts.gpu[row..row + 16];
            instances.push(Instance {
                key,
                part: id as PartId,
                transform: std::array::from_fn(|index| model[(index % 4) * 4 + index / 4]),
                mask,
            });
        }
        // Ranges no shown part draws any more: their BLAS go once the TLAS
        // is rebuilt without them.
        scene.blases.retain(|key, _| used.contains(key));
        scene.instances = instances;
        scene.stale = scene.waiting > 0;
        scene.tlas_dirty = true;
        builds
    }

    /// Encode this frame's BLAS builds and the TLAS build, and submit them.
    fn build(&self, scene: &mut RayScene, builds: &[BlasKey]) {
        let started = Instant::now();
        let gpu = &self.gpu;
        let device = &gpu.device;
        // Make each BLAS to build, with its geometry's size.
        let mut made = Vec::new();
        for key in builds {
            let Some(mesh) = self.mesh(&key.mesh) else {
                continue;
            };
            let size = wgpu::BlasTriangleGeometrySizeDescriptor {
                vertex_format: wgpu::VertexFormat::Float32x3,
                vertex_count: (mesh.vertices.size() / VERTEX_BYTES) as u32,
                index_format: Some(wgpu::IndexFormat::Uint32),
                index_count: Some(key.index_count),
                flags: if key.opaque {
                    wgpu::AccelerationStructureGeometryFlags::OPAQUE
                } else {
                    wgpu::AccelerationStructureGeometryFlags::empty()
                },
            };
            let compactable = !matches!(key.mesh, MeshRef::AnimatedSkinned(..));
            let flags = if compactable {
                wgpu::AccelerationStructureFlags::PREFER_FAST_TRACE
                    | wgpu::AccelerationStructureFlags::ALLOW_COMPACTION
            } else {
                wgpu::AccelerationStructureFlags::PREFER_FAST_TRACE
            };
            let Ok(blas) = crate::gpu::allocating(device, || {
                device.create_blas(
                    &wgpu::CreateBlasDescriptor {
                        label: Some("render-wgpu ray scene"),
                        flags,
                        update_mode: wgpu::AccelerationStructureUpdateMode::Build,
                    },
                    wgpu::BlasGeometrySizeDescriptors::Triangles {
                        descriptors: vec![size.clone()],
                    },
                )
            }) else {
                continue;
            };
            let bytes = built_bytes(device, &size, flags).unwrap_or(0);
            made.push((key, mesh, size, blas, bytes, compactable));
        }
        let entries: Vec<wgpu::BlasBuildEntry<'_>> = made
            .iter()
            .map(|(key, mesh, size, blas, ..)| wgpu::BlasBuildEntry {
                blas,
                geometry: wgpu::BlasGeometries::TriangleGeometries(vec![
                    wgpu::BlasTriangleGeometry {
                        size,
                        vertex_buffer: &mesh.vertices,
                        first_vertex: 0,
                        vertex_stride: VERTEX_BYTES,
                        index_buffer: Some(&mesh.indices),
                        first_index: Some(key.first_index),
                        transform_buffer: None,
                        transform_buffer_offset: None,
                    },
                ]),
            })
            .collect();
        // The new BLAS replace the old in the instances the TLAS takes.
        let mut fresh: HashMap<&BlasKey, &wgpu::Blas> = HashMap::new();
        for (key, _, _, blas, ..) in &made {
            fresh.insert(*key, blas);
        }
        let shown: Vec<(&Instance, &wgpu::Blas)> = scene
            .instances
            .iter()
            .filter_map(|instance| {
                let blas = fresh
                    .get(&instance.key)
                    .copied()
                    .or_else(|| scene.blases.get(&instance.key).map(|entry| &entry.blas))?;
                Some((instance, blas))
            })
            .collect();
        let count = shown.len() as u32;
        let capacity = scene
            .tlas
            .as_ref()
            .map_or(0, |tlas| tlas.get().len() as u32);
        if count > capacity || scene.tlas.is_none() {
            let instances = count.next_power_of_two().max(MIN_TLAS_INSTANCES);
            scene.tlas = crate::gpu::allocating(device, || {
                device.create_tlas(&wgpu::CreateTlasDescriptor {
                    label: Some("render-wgpu ray scene"),
                    max_instances: instances,
                    flags: wgpu::AccelerationStructureFlags::PREFER_FAST_TRACE,
                    update_mode: wgpu::AccelerationStructureUpdateMode::Build,
                })
            })
            .ok();
        }
        let Some(tlas) = scene.tlas.as_mut() else {
            return;
        };
        let length = tlas.get().len();
        if let Some(slots) = tlas.get_mut_slice(0..length) {
            for (slot, (instance, blas)) in slots.iter_mut().zip(&shown) {
                *slot = Some(wgpu::TlasInstance::new(
                    blas,
                    instance.transform,
                    instance.part,
                    instance.mask,
                ));
            }
            for slot in &mut slots[shown.len()..] {
                *slot = None;
            }
        }
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("render-wgpu ray scene"),
        });
        // Empty passes carry the timer's stamps around the builds.
        if let Some(timer) = &scene.timer {
            drop(encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("render-wgpu ray scene begin"),
                timestamp_writes: timer.compute_writes_between(true, false),
            }));
        }
        encoder.build_acceleration_structures(entries.iter(), std::iter::once(&*tlas));
        if let Some(timer) = &mut scene.timer {
            drop(encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("render-wgpu ray scene end"),
                timestamp_writes: timer.compute_writes_between(false, true),
            }));
            timer.resolve(&mut encoder);
        }
        gpu.queue.submit([encoder.finish()]);
        if let Some(timer) = &mut scene.timer {
            timer.submitted();
        }
        drop(entries);
        drop(shown);
        for (key, mesh, _, blas, bytes, compactable) in made {
            if compactable {
                blas.prepare_compaction_async(|_| {});
            }
            scene.blases.insert(
                key.clone(),
                BlasEntry {
                    blas,
                    source: Arc::downgrade(&mesh.cpu),
                    triangles: u64::from(key.index_count / 3),
                    bytes,
                    compaction: if compactable {
                        Compaction::Preparing
                    } else {
                        Compaction::Never
                    },
                },
            );
            scene.blas_builds += 1;
        }
        scene.tlas_builds += 1;
        scene.tlas_dirty = false;
        scene.build_cpu_ms = started.elapsed().as_secs_f64() * 1e3;
        scene.build_cpu_ms_total += scene.build_cpu_ms;
    }

    /// Trace `rays` against the ray scene and return each one's closest
    /// hit, waiting for the GPU: for tools and tests. Empty while there is
    /// no ray scene or it has not been built.
    #[doc(hidden)]
    pub fn trace_rays(&mut self, rays: &[ProbeRay]) -> Vec<Option<ProbeHit>> {
        let Some(scene) = self.ray_scene.as_mut() else {
            return Vec::new();
        };
        let (Some(tlas), false) = (scene.tlas.as_ref(), rays.is_empty()) else {
            return Vec::new();
        };
        let device = &self.gpu.device;
        let pipeline = scene.probe.get_or_insert_with(|| {
            let module = self
                .layouts
                .shaders
                .module(
                    device,
                    crate::shaders::Entry::RayProbe,
                    crate::shaders::Features::default(),
                )
                .expect("the ray probe composes");
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("render-wgpu ray probe"),
                layout: None,
                module: &module,
                entry_point: Some("cs_trace"),
                compilation_options: Default::default(),
                cache: None,
            })
        });
        let floats: Vec<f32> = rays
            .iter()
            .flat_map(|ray| {
                [
                    ray.origin[0],
                    ray.origin[1],
                    ray.origin[2],
                    ray.reach,
                    ray.direction[0],
                    ray.direction[1],
                    ray.direction[2],
                    f32::from(ray.mask),
                ]
            })
            .collect();
        let input = crate::gpu::buffer_init(
            device,
            &wgpu::util::BufferInitDescriptor {
                label: Some("render-wgpu ray probe rays"),
                contents: bytemuck::cast_slice(&floats),
                usage: wgpu::BufferUsages::STORAGE,
            },
        );
        let bytes = rays.len() as u64 * 16;
        let output = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("render-wgpu ray probe hits"),
            size: bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("render-wgpu ray probe readback"),
            size: bytes,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("render-wgpu ray probe"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: tlas.as_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: input.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: output.as_entire_binding(),
                },
            ],
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups((rays.len() as u32).div_ceil(64), 1, 1);
        }
        encoder.copy_buffer_to_buffer(&output, 0, &readback, 0, bytes);
        self.gpu.queue.submit([encoder.finish()]);
        let slice = readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
        let Ok(mapped) = slice.get_mapped_range() else {
            return Vec::new();
        };
        let words: Vec<u32> = bytemuck::cast_slice(&mapped).to_vec();
        drop(mapped);
        readback.unmap();
        words
            .as_chunks::<4>()
            .0
            .iter()
            .map(|hit| {
                let distance = f32::from_bits(hit[0]);
                (distance >= 0.0).then(|| ProbeHit {
                    distance,
                    node: self.part_node(hit[1]),
                    front_face: hit[3] != 0,
                })
            })
            .collect()
    }

    /// The node a part belongs to.
    fn part_node(&self, part: PartId) -> Option<render_model::RenderHandle> {
        self.tables
            .nodes
            .iter()
            .find(|(_, node)| node.parts.contains(&part))
            .map(|(handle, _)| *handle)
    }
}

/// Compact the BLAS whose builds have completed, within the budget, and
/// return how many were: the TLAS must take them in place of the originals.
fn compact(gpu: &Gpu, scene: &mut RayScene) -> usize {
    let mut compacted = 0;
    for entry in scene.blases.values_mut() {
        if compacted == COMPACTIONS_PER_FRAME {
            break;
        }
        if entry.compaction == Compaction::Preparing && entry.blas.ready_for_compaction() {
            entry.blas = gpu.queue.compact_blas(&entry.blas);
            entry.compaction = Compaction::Compacted;
            compacted += 1;
        }
    }
    compacted
}

/// Whether `source` still names `current`: the weak reference keeps the
/// allocation, so another geometry never takes its address.
fn ptr_eq(source: &Weak<CpuGeometry>, current: &Arc<CpuGeometry>) -> bool {
    std::ptr::eq(source.as_ptr(), Arc::as_ptr(current))
}

/// The bytes the driver says a BLAS of `size` takes when built with
/// `flags`, on Vulkan; wgpu does not report it.
fn built_bytes(
    device: &wgpu::Device,
    size: &wgpu::BlasTriangleGeometrySizeDescriptor,
    flags: wgpu::AccelerationStructureFlags,
) -> Option<u64> {
    use wgpu::hal::Device as _;
    let entries = wgpu::hal::AccelerationStructureEntries::Triangles(vec![
        wgpu::hal::AccelerationStructureTriangles::<wgpu::hal::vulkan::Buffer> {
            vertex_buffer: None,
            vertex_format: size.vertex_format,
            first_vertex: 0,
            vertex_count: size.vertex_count,
            vertex_stride: VERTEX_BYTES,
            indices: size.index_count.map(|count| {
                wgpu::hal::AccelerationStructureTriangleIndices {
                    format: wgpu::IndexFormat::Uint32,
                    buffer: None,
                    offset: 0,
                    count,
                }
            }),
            transform: None,
            flags: size.flags,
        },
    ]);
    // SAFETY: `as_hal` hands out the device while the guard lives; the
    // size query only reads the descriptor (no buffer is named, as the
    // query allows) and creates, uses or destroys nothing.
    unsafe {
        let hal = device.as_hal::<wgpu::hal::api::Vulkan>()?;
        let sizes = hal.get_acceleration_structure_build_sizes(
            &wgpu::hal::GetAccelerationStructureBuildSizesDescriptor {
                entries: &entries,
                flags,
            },
        );
        Some(sizes.acceleration_structure_size)
    }
}
