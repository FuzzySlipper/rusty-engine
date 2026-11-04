//! The ray scene: acceleration structures of the shown world, for the
//! ray-traced settings to trace against.
//!
//! It is a reading of the retained tables, the rows the world pass draws:
//! no product API and no second scene. It exists only while something
//! asks for it ([`Renderer::set_ray_scene`]); otherwise nothing is built.
//!
//! - **BLAS:** one per drawn index range of a mesh (a part's mesh group),
//!   built from the mesh's own buffers (position at offset 0 of the 48-byte
//!   vertex) on wgpu's safe path. A range drawn with an opaque material is
//!   `OPAQUE`; a masked one is not, so a ray's candidate loop can alpha-test
//!   it. A replaced payload (a voxel chunk) or a new skinned pose is new
//!   geometry and builds a new BLAS (wgpu 30 has no refit). Other BLAS are
//!   compacted once built. Builds and compactions are budgeted per frame; a
//!   range waiting for its first build is not in the TLAS yet.
//! - **TLAS:** rebuilt each frame at the first world view, through
//!   wgpu-hal from an instance buffer: wgpu's safe path re-serialises and
//!   re-validates every instance on the CPU, which scatter copies make
//!   unaffordable. The CPU writes one row per shown, triangle, scene-layer
//!   part that casts or is opaque; a compute pass (`ray_instances.wgsl`)
//!   appends a row per part of the level each scatter copy draws at, for
//!   the copies within the ray reach of the eye (the sun's shadow reach,
//!   [`DEFAULT_REACH`] without one). A build takes rows for the copies the
//!   one before counted; the first waits for its count and builds again.
//!   The 24-bit custom data is the part id;
//!   the mask says what the instance is to a ray ([`MASK_SHADOW`],
//!   [`MASK_GI`]). Rows past the last written are zero: inactive.
//! - **Wind:** a part or copy whose material sways leans as the vertex
//!   stage leans it, as a shear of its instance by height above its
//!   origin. The per-vertex flutter, morph targets and product `displace`
//!   stages are vertex-stage deformations: rays see rest geometry.
//!
//! wgpu does not track what a hal-built TLAS references, so the ray scene
//! keeps every BLAS, instance buffer and scratch buffer a submitted build
//! used until the GPU has finished that work (`Retired`), and waits for the
//! GPU before it is dropped.
//!
//! Ray queries cost some devices startup time and memory to ask for, so a
//! device has them only when made for a ray-traced setting
//! ([`Gpu::headless_with_ray_queries`]); otherwise the ray scene is refused
//! as `NoRayQueries`. It builds on Vulkan only.
//!
//! This is the only module of the crate allowed unsafe code (approved by
//! Patch on #10308): the experimental ray-query opt-in and wgpu-hal calls.
//! Every `unsafe` block states the invariant it relies on, and
//! `tests/unsafe_code.rs` fails on unsafe code anywhere else in the crate.

#![allow(unsafe_code)]

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::{Arc, Weak};
use std::time::Instant;

use crate::tables::{CpuGeometry, MaterialRef, MeshRef, PartId, Topology};
use crate::{Gpu, Renderer, SettingRefusal};

type Vulkan = wgpu::hal::api::Vulkan;

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

/// How far from the eye scatter copies are in the TLAS without a shadowing
/// sun to take the reach from (`shadows.rs`).
pub(crate) const DEFAULT_REACH: f32 = crate::shadows::DIRECTIONAL_SHADOW_DISTANCE;

/// Triangles built into new BLAS per frame; the rest wait, the longest
/// waiting first, and the first always builds however large. A skinned mesh
/// whose new pose waits keeps its last BLAS meanwhile.
const BUILD_TRIANGLES_PER_FRAME: u64 = 1 << 20;
/// BLAS compacted per frame.
const COMPACTIONS_PER_FRAME: usize = 256;
/// The bytes of the interleaved vertex the meshes share (`pipelines.rs`).
const VERTEX_BYTES: u64 = (crate::pipelines::VERTEX_FLOATS * 4) as u64;
/// `VkAccelerationStructureInstanceKHR`: a 3×4 transform, custom data and
/// mask, offset and flags, and the BLAS's device address.
const INSTANCE_WORDS: usize = 16;
const INSTANCE_BYTES: u64 = (INSTANCE_WORDS * 4) as u64;
/// The fewest rows the instance buffer and TLAS are made for; they grow to
/// the rows a frame needed, and half again.
const MIN_INSTANCES: u32 = 4096;
const WORKGROUP: u32 = 64;
/// Chunks a dispatch row holds (the limit on a dimension).
const ROW_CHUNKS: u32 = 65_535;
/// `Params` in `ray_instances.wgsl`: four vectors.
const PARAMS_BYTES: u64 = 64;
/// `Run` in `ray_instances.wgsl`: eight vectors.
const RUN_WORDS: usize = 32;
/// Rows written, copies in reach, rows past the buffer.
const COUNTERS: usize = 3;

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
    // The traced shadows bind a fifth group, of five more storage buffers
    // and the mask's texture (`ray_shadows.rs`).
    let limits = &mut descriptor.required_limits;
    limits.max_bind_groups = limits.max_bind_groups.max(5).min(granted.max_bind_groups);
    limits.max_storage_buffers_per_shader_stage = (limits.max_storage_buffers_per_shader_stage + 5)
        .min(granted.max_storage_buffers_per_shader_stage);
    limits.max_sampled_textures_per_shader_stage = (limits.max_sampled_textures_per_shader_stage
        + 1)
    .min(granted.max_sampled_textures_per_shader_stage);
    descriptor
        .required_limits
        .max_binding_array_sampler_elements_per_shader_stage =
        granted.max_binding_array_sampler_elements_per_shader_stage;
    // SAFETY: `ExperimentalFeatures::enabled` asks the caller to accept that
    // experimental APIs may hold bugs and to report them. The renderer uses
    // the experimental ray queries only through this module, only on a
    // device that advertises them, and keeps what a built TLAS references
    // alive (see `RayScene`).
    descriptor.experimental_features = unsafe { wgpu::ExperimentalFeatures::enabled() };
}

/// `BLAS_INPUT` on a device with ray queries, for the buffers meshes draw
/// from, and `COPY_SRC`: the traced shadows copy masked meshes' uvs
/// (`ray_shadows.rs`). Nothing otherwise.
pub(crate) fn mesh_input(device: &wgpu::Device) -> wgpu::BufferUsages {
    if device
        .features()
        .contains(wgpu::Features::EXPERIMENTAL_RAY_QUERY)
    {
        wgpu::BufferUsages::BLAS_INPUT | wgpu::BufferUsages::COPY_SRC
    } else {
        wgpu::BufferUsages::empty()
    }
}

/// Why the ray-traced settings cannot draw on this device, if they cannot,
/// and the detail.
pub(crate) fn refusal(gpu: &Gpu) -> Option<(SettingRefusal, String)> {
    let info = gpu.adapter.get_info();
    if gpu.is_software() {
        return Some((
            SettingRefusal::SoftwareAdapter,
            format!("{} is a software adapter", info.name),
        ));
    }
    if gpu.device.features().contains(FEATURES) && info.backend == wgpu::Backend::Vulkan {
        return None;
    }
    let missing = FEATURES - gpu.adapter.features();
    Some((
        SettingRefusal::NoRayQueries,
        if !missing.is_empty() {
            format!("{} has no {missing:?}", info.name)
        } else if info.backend != wgpu::Backend::Vulkan {
            format!(
                "the ray scene builds on Vulkan only, not {:?}",
                info.backend
            )
        } else {
            format!("the device on {} was made without ray queries: no ray-traced setting was selected when it was made", info.name)
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
    /// How far from the eye scatter copies are in the TLAS, in metres.
    pub reach: f32,
    /// The parts' instances the CPU wrote, and from the last read-back
    /// build: the scatter copies within the reach and the instances written
    /// for them (a copy's level may have several parts), and those past
    /// the instance buffer, which grows for the next frame.
    pub part_instances: u32,
    pub copies_in_reach: u32,
    pub copy_instances: u32,
    pub dropped_instances: u32,
    /// The rows the instance buffer and TLAS hold, and the rows the last
    /// build took: the parts', and the copies' the frame before needed
    /// with a margin. Rows past those written are inactive.
    pub capacity: u32,
    pub built_rows: u32,
    /// BLAS built and TLAS builds since the ray scene was made.
    pub blas_builds: u64,
    pub tlas_builds: u64,
    /// CPU milliseconds the last BLAS builds and the last TLAS build took
    /// to encode and submit, and all builds together since it was made.
    pub blas_cpu_ms: f64,
    pub tlas_cpu_ms: f64,
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

/// A shown part's instance: its BLAS, transform (3×4, row-major), mask and
/// its material's wind bend (0: no wind).
struct Instance {
    key: BlasKey,
    part: PartId,
    transform: [f32; 12],
    mask: u8,
    bend: f32,
}

/// What a submitted TLAS build may still be reading, kept until the GPU
/// has finished the work submitted before it was retired.
enum Retired {
    Blas(#[allow(dead_code, reason = "held until the GPU is done")] wgpu::Blas),
    Buffer(#[allow(dead_code, reason = "held until the GPU is done")] wgpu::Buffer),
    Tlas(#[allow(dead_code, reason = "held until the GPU is done")] wgpu::Tlas),
}

/// The scatter runs as the instance pass reads them.
struct Runs {
    runs: wgpu::Buffer,
    chunks: wgpu::Buffer,
    level_parts: wgpu::Buffer,
    chunk_count: u32,
}

/// The instance buffer, the TLAS built from it and the scratch the build
/// uses, all for `capacity` rows.
struct Built {
    instances: wgpu::Buffer,
    tlas: wgpu::Tlas,
    scratch: wgpu::Buffer,
    capacity: u32,
}

pub(crate) struct RayScene {
    device: wgpu::Device,
    blases: HashMap<BlasKey, BlasEntry>,
    built: Option<Built>,
    /// The parts' instances from the last walk of the parts.
    instances: Vec<Instance>,
    /// The shown masked parts that cast, from the last walk
    /// (`ray_shadows.rs`).
    masked: Vec<crate::ray_shadows::Masked>,
    /// Counts the TLAS made, so what binds it knows when it is another.
    generation: u64,
    /// The TLAS has been built, so a ray may trace it.
    traceable: bool,
    /// The scatter runs' tables, made again when the parts or the BLAS
    /// change.
    runs: Option<Runs>,
    runs_stale: bool,
    pipeline: Option<wgpu::ComputePipeline>,
    params: wgpu::Buffer,
    counters: wgpu::Buffer,
    counters_readback: wgpu::Buffer,
    pending_counters: Option<Receiver<Result<(), wgpu::BufferAsyncError>>>,
    /// The last read-back counters, and whether any have been: the first
    /// build waits for its own.
    counted: [u32; COUNTERS],
    counted_once: bool,
    /// The parts changed since the last walk (or ranges still wait).
    stale: bool,
    /// The TLAS was built this frame.
    built_this_frame: bool,
    /// The reach asked for in place of the sun's (tools and tests).
    reach_override: Option<f32>,
    reach: f32,
    part_rows: u32,
    built_rows: u32,
    retired: Vec<(Arc<AtomicBool>, Retired)>,
    /// The TLAS build (instance pass and build) and the BLAS builds.
    timer: Option<crate::timing::PassTimer>,
    blas_timer: Option<crate::timing::PassTimer>,
    probe: Option<wgpu::ComputePipeline>,
    waiting: u32,
    /// Since which walk each range waiting for a build has waited.
    waiting_since: HashMap<BlasKey, u64>,
    walks: u64,
    blas_builds: u64,
    tlas_builds: u64,
    blas_cpu_ms: f64,
    tlas_cpu_ms: f64,
    build_cpu_ms_total: f64,
}

impl RayScene {
    fn new(gpu: &Gpu) -> Self {
        let device = &gpu.device;
        let buffer = |label, size, usage| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage,
                mapped_at_creation: false,
            })
        };
        let counter_bytes = (COUNTERS * 4) as u64;
        Self {
            device: device.clone(),
            blases: HashMap::new(),
            built: None,
            instances: Vec::new(),
            masked: Vec::new(),
            generation: 0,
            traceable: false,
            runs: None,
            runs_stale: true,
            pipeline: None,
            params: buffer(
                "render-wgpu ray instances params",
                PARAMS_BYTES,
                wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            ),
            counters: buffer(
                "render-wgpu ray instances counters",
                counter_bytes,
                wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::COPY_SRC,
            ),
            counters_readback: buffer(
                "render-wgpu ray instances counters readback",
                counter_bytes,
                wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            ),
            pending_counters: None,
            counted: [0; COUNTERS],
            counted_once: false,
            stale: true,
            built_this_frame: false,
            reach_override: None,
            reach: DEFAULT_REACH,
            part_rows: 0,
            built_rows: 0,
            retired: Vec::new(),
            timer: crate::timing::PassTimer::new(gpu, "ray-scene"),
            blas_timer: crate::timing::PassTimer::new(gpu, "ray-blas"),
            probe: None,
            waiting: 0,
            waiting_since: HashMap::new(),
            walks: 0,
            blas_builds: 0,
            tlas_builds: 0,
            blas_cpu_ms: 0.0,
            tlas_cpu_ms: 0.0,
            build_cpu_ms_total: 0.0,
        }
    }

    /// The shown masked parts that cast, from the last walk.
    pub fn masked(&self) -> &[crate::ray_shadows::Masked] {
        &self.masked
    }

    /// Counts the walks of the parts.
    pub fn walks(&self) -> u64 {
        self.walks
    }

    /// The TLAS to trace and its generation, once one has been built.
    pub fn traced(&self) -> Option<(&wgpu::Tlas, u64)> {
        let built = self.built.as_ref().filter(|_| self.traceable)?;
        Some((&built.tlas, self.generation))
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
            reach: self.reach,
            part_instances: self.part_rows,
            copies_in_reach: self.counted[1],
            copy_instances: self.counted[0] - self.counted[2],
            dropped_instances: self.counted[2],
            capacity: self.built.as_ref().map_or(0, |built| built.capacity),
            built_rows: self.built_rows,
            blas_builds: self.blas_builds,
            tlas_builds: self.tlas_builds,
            blas_cpu_ms: self.blas_cpu_ms,
            tlas_cpu_ms: self.tlas_cpu_ms,
            build_cpu_ms_total: self.build_cpu_ms_total,
        }
    }

    /// The TLAS build's timing, then the BLAS builds'.
    pub fn timings(&self) -> [crate::GpuPassTiming; 2] {
        [
            self.timer.as_ref().map_or_else(
                || crate::timing::untimed("ray-scene"),
                crate::timing::PassTimer::readout,
            ),
            self.blas_timer.as_ref().map_or_else(
                || crate::timing::untimed("ray-blas"),
                crate::timing::PassTimer::readout,
            ),
        ]
    }

    /// Keep `retired` until the GPU has finished every submission so far.
    fn retire(&mut self, queue: &wgpu::Queue, retired: Retired) {
        let done = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&done);
        queue.on_submitted_work_done(move || flag.store(true, Ordering::Release));
        self.retired.push((done, retired));
    }

    /// Take the counters of the last read-back build, without waiting.
    fn collect_counters(&mut self) {
        let Some(receiver) = &self.pending_counters else {
            return;
        };
        match receiver.try_recv() {
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) | Ok(Err(_)) => self.pending_counters = None,
            Ok(Ok(())) => {
                self.pending_counters = None;
                if let Ok(mapped) = self.counters_readback.slice(..).get_mapped_range() {
                    let words: &[u32] = bytemuck::cast_slice(&mapped);
                    self.counted.copy_from_slice(&words[..COUNTERS]);
                }
                self.counters_readback.unmap();
            }
        }
    }
}

impl Drop for RayScene {
    /// The GPU may still be building or tracing with what the ray scene
    /// holds, which wgpu does not know of: wait for it first.
    fn drop(&mut self) {
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
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
    /// Build the ray scene and keep it current, or with `false` drop it
    /// unless a ray-traced setting draws: for tools and tests.
    #[doc(hidden)]
    pub fn set_ray_scene(&mut self, wanted: bool) {
        self.ray_scene_wanted = wanted;
        self.keep_ray_scene();
    }

    /// Make or drop the ray scene, and what the ray-traced settings bind
    /// beside it, as they and the tools ask.
    fn keep_ray_scene(&mut self) {
        let shadows = self.ray_shadows_drawn();
        if !(self.ray_scene_wanted || shadows) {
            self.ray_scene = None;
        } else if self.ray_scene.is_none() && refusal(&self.gpu).is_none() {
            self.ray_scene = Some(RayScene::new(&self.gpu));
        }
        if !shadows || self.ray_scene.is_none() {
            self.ray_shadows = None;
        } else if self.ray_shadows.is_none() {
            self.ray_shadows = Some(crate::ray_shadows::RayShadows::new(&self.gpu));
        }
    }

    /// Take scatter copies into the TLAS within `reach` metres of the eye,
    /// or with `None` within the sun's shadow reach: for tools and tests.
    #[doc(hidden)]
    pub fn set_ray_reach(&mut self, reach: Option<f32>) {
        if let Some(scene) = &mut self.ray_scene {
            scene.reach_override = reach;
        }
    }

    /// The ray scene's readout while it is asked for.
    pub(crate) fn ray_scene_readout(&self) -> Option<RaySceneReadout> {
        if !(self.ray_scene_wanted || self.ray_shadows_drawn()) {
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
                reach: 0.0,
                part_instances: 0,
                copies_in_reach: 0,
                copy_instances: 0,
                dropped_instances: 0,
                capacity: 0,
                built_rows: 0,
                blas_builds: 0,
                tlas_builds: 0,
                blas_cpu_ms: 0.0,
                tlas_cpu_ms: 0.0,
                build_cpu_ms_total: 0.0,
            },
        })
    }

    /// Bring the ray scene's BLAS up to date with the parts, after
    /// `prepare` has uploaded them: `changed` when parts were added,
    /// removed or moved, or scatter runs changed. The TLAS is built at the
    /// frame's first world view, from its eye.
    pub(crate) fn prepare_ray_scene(&mut self, changed: bool) {
        self.keep_ray_scene();
        let Some(mut scene) = self.ray_scene.take() else {
            return;
        };
        for timer in [&mut scene.timer, &mut scene.blas_timer]
            .into_iter()
            .flatten()
        {
            timer.collect(&self.gpu);
        }
        scene.collect_counters();
        scene
            .retired
            .retain(|(done, _)| !done.load(Ordering::Acquire));
        scene.built_this_frame = false;
        scene.stale |= changed;
        let mut builds: Vec<BlasKey> = Vec::new();
        if std::mem::take(&mut scene.stale) {
            builds = self.walk_parts(&mut scene);
            scene.runs_stale = true;
        }
        if compact(&self.gpu, &mut scene) > 0 {
            scene.runs_stale = true;
        }
        if !builds.is_empty() {
            self.build_blases(&mut scene, &builds);
            scene.runs_stale = true;
        }
        self.ray_scene = Some(scene);
        self.prepare_ray_shadows();
    }

    /// Gather the shown parts' instances, and the BLAS to build this frame
    /// within the budget: new ranges, and ranges whose geometry changed. A
    /// scatter run's level parts need their BLAS, but their instances are
    /// the copies'.
    fn walk_parts(&self, scene: &mut RayScene) -> Vec<BlasKey> {
        let parts = &self.tables.parts;
        let mut instances = Vec::new();
        let mut masked = Vec::new();
        let mut used = HashSet::new();
        // Ranges needing a build, with their triangles and since when.
        let mut wanted: Vec<(BlasKey, u64, u64)> = Vec::new();
        scene.walks += 1;
        let walk = scene.walks;
        for (id, part) in parts.meta.iter().enumerate() {
            let Some(part) = part else { continue };
            let state = &parts.state[id];
            let scattered = state.scatter_level.is_some();
            if !state.shown
                || state.layer != render_model::RenderLayer::Scene
                || state.class.lines
                || state
                    .class
                    .features
                    .contains(crate::shaders::Features::IMPOSTOR)
                || (scattered && state.class.blend)
                || part.wireframe
                || part.index_count < 3
                || id >= 1 << 24
            {
                continue;
            }
            let mask = part_mask(state);
            if mask == 0 {
                continue;
            }
            let Some(mesh) = self.mesh(&part.mesh) else {
                continue;
            };
            if mesh.topology != Topology::Triangles {
                continue;
            }
            let key = blas_key(part, state);
            let current = scene
                .blases
                .get(&key)
                .is_some_and(|entry| ptr_eq(&entry.source, &mesh.cpu));
            if !current && used.insert(key.clone()) {
                let since = *scene.waiting_since.entry(key.clone()).or_insert(walk);
                wanted.push((key.clone(), u64::from(part.index_count / 3), since));
            } else {
                used.insert(key.clone());
            }
            if mask & MASK_SHADOW != 0
                && state
                    .class
                    .features
                    .contains(crate::shaders::Features::MASK)
            {
                masked.push(crate::ray_shadows::Masked {
                    part: id as PartId,
                    mesh: part.mesh.clone(),
                    first_index: part.first_index,
                    index_count: part.index_count,
                });
            }
            if scattered {
                continue;
            }
            let row = id * crate::tables::PART_ROW_FLOATS;
            let model = &parts.gpu[row..row + 16];
            instances.push(Instance {
                key,
                part: id as PartId,
                transform: std::array::from_fn(|index| model[(index % 4) * 4 + index / 4]),
                mask,
                bend: self.wind_bend(&part.material, state.class.features),
            });
        }
        // Ranges no shown part draws any more: their BLAS go once the GPU is
        // done with the builds that used them.
        let gone: Vec<BlasKey> = scene
            .blases
            .keys()
            .filter(|key| !used.contains(*key))
            .cloned()
            .collect();
        for key in gone {
            if let Some(entry) = scene.blases.remove(&key) {
                scene.retire(&self.gpu.queue, Retired::Blas(entry.blas));
            }
        }
        scene.waiting_since.retain(|key, _| used.contains(key));
        let builds: Vec<BlasKey> = scheduled(&mut wanted, BUILD_TRIANGLES_PER_FRAME)
            .into_iter()
            .map(|(key, ..)| key)
            .collect();
        for key in &builds {
            scene.waiting_since.remove(key);
        }
        scene.waiting = wanted.len() as u32;
        scene.instances = instances;
        scene.masked = masked;
        scene.stale = scene.waiting > 0;
        builds
    }

    /// A part's material's wind bend, when it sways (`rusty::wind`).
    fn wind_bend(&self, material: &MaterialRef, features: crate::shaders::Features) -> f32 {
        if !features.contains(crate::shaders::Features::WIND) {
            return 0.0;
        }
        match material {
            MaterialRef::Retained(id) => self
                .tables
                .materials
                .get(*id)
                .and_then(|row| row.descriptor.wind)
                .map_or(0.0, |wind| wind.bend),
            _ => 0.0,
        }
    }

    /// Encode and submit this frame's BLAS builds (wgpu's safe path).
    fn build_blases(&self, scene: &mut RayScene, builds: &[BlasKey]) {
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
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("render-wgpu ray scene blas"),
        });
        // Empty passes carry the timer's stamps around the builds.
        if let Some(timer) = &scene.blas_timer {
            drop(encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("render-wgpu ray blas begin"),
                timestamp_writes: timer.compute_writes_between(true, false),
            }));
        }
        encoder.build_acceleration_structures(entries.iter(), std::iter::empty());
        if let Some(timer) = &mut scene.blas_timer {
            drop(encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("render-wgpu ray blas end"),
                timestamp_writes: timer.compute_writes_between(false, true),
            }));
            timer.resolve(&mut encoder);
        }
        gpu.queue.submit([encoder.finish()]);
        if let Some(timer) = &mut scene.blas_timer {
            timer.submitted();
        }
        drop(entries);
        for (key, mesh, _, blas, bytes, compactable) in made {
            if compactable {
                blas.prepare_compaction_async(|_| {});
            }
            let replaced = scene.blases.insert(
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
            if let Some(old) = replaced {
                scene.retire(&gpu.queue, Retired::Blas(old.blas));
            }
            scene.blas_builds += 1;
        }
        scene.blas_cpu_ms = started.elapsed().as_secs_f64() * 1e3;
        scene.build_cpu_ms_total += scene.blas_cpu_ms;
    }

    /// Build the TLAS from `eye` once a frame, at its first world view: the
    /// parts' rows from the CPU, the copies' from the instance pass, then
    /// the build through wgpu-hal.
    pub(crate) fn build_ray_tlas(&mut self, eye: glam::Vec3) {
        let Some(mut scene) = self.ray_scene.take() else {
            return;
        };
        if !scene.built_this_frame {
            scene.built_this_frame = true;
            self.encode_tlas(&mut scene, eye);
            if !scene.counted_once {
                // The first build has no count of the copies in reach to
                // take rows for, so it dropped them: wait for the count it
                // made and build again with room for them, so the first
                // frame traced has its copies too.
                let _ = self.gpu.device.poll(wgpu::PollType::wait_indefinitely());
                scene.collect_counters();
                scene.counted_once = true;
                if scene.counted[0] > 0 {
                    self.encode_tlas(&mut scene, eye);
                }
            }
        }
        self.ray_scene = Some(scene);
    }

    fn encode_tlas(&mut self, scene: &mut RayScene, eye: glam::Vec3) {
        let started = Instant::now();
        scene.reach = scene
            .reach_override
            .unwrap_or_else(|| self.sun_shadow_reach());
        if scene.runs_stale {
            scene.runs = self.scatter_tables(scene);
            scene.runs_stale = false;
        }
        // The parts' rows, leaning in the wind where they sway.
        let wind = crate::frame::wind_uniform(self.tables.wind);
        let time = self.presentation_time as f32;
        let mut rows: Vec<u32> = Vec::with_capacity(scene.instances.len() * INSTANCE_WORDS);
        for instance in &scene.instances {
            let Some(address) = scene
                .blases
                .get(&instance.key)
                .and_then(|entry| entry.blas.handle())
            else {
                continue;
            };
            let transform = leaning(instance.transform, instance.bend, wind, time);
            rows.extend(transform.map(f32::to_bits));
            rows.extend_from_slice(&[
                instance.part | (u32::from(instance.mask) << 24),
                0,
                address as u32,
                (address >> 32) as u32,
            ]);
        }
        let part_rows = (rows.len() / INSTANCE_WORDS) as u32;
        scene.part_rows = part_rows;
        // Room for the parts and the copies the last read-back build wrote
        // (and those it dropped), and half again.
        let needed = u64::from(part_rows) + u64::from(scene.counted[0]);
        let limit = self.gpu.device.limits().max_tlas_instance_count;
        let fits = scene
            .built
            .as_ref()
            .is_some_and(|built| u64::from(built.capacity) >= needed);
        if !fits {
            let capacity = ((needed * 3 / 2).max(u64::from(MIN_INSTANCES)))
                .next_power_of_two()
                .min(u64::from(limit)) as u32;
            if let Some(old) = scene.built.take() {
                scene.retire(&self.gpu.queue, Retired::Buffer(old.instances));
                scene.retire(&self.gpu.queue, Retired::Tlas(old.tlas));
                scene.retire(&self.gpu.queue, Retired::Buffer(old.scratch));
            }
            scene.built = made_for(&self.gpu, capacity);
            scene.generation += 1;
            scene.traceable = false;
        }
        if scene.pipeline.is_none() {
            let module = self
                .layouts
                .shaders
                .module(
                    &self.gpu.device,
                    crate::shaders::Entry::RayInstances,
                    crate::shaders::Features::default(),
                )
                .expect("the ray instances compose");
            scene.pipeline = Some(self.gpu.device.create_compute_pipeline(
                &wgpu::ComputePipelineDescriptor {
                    label: Some("render-wgpu ray instances"),
                    layout: None,
                    module: &module,
                    entry_point: Some("cs_instances"),
                    compilation_options: Default::default(),
                    cache: None,
                },
            ));
        }
        let Some(built) = scene.built.as_mut() else {
            return;
        };
        // The build takes the rows the copies needed the frame before, and an
        // eighth more: a build's cost follows its rows, written or inactive.
        // A frame needing more drops the rest (counted), and the next takes
        // them.
        let copy_rows = scene.counted[0];
        let rows_built = (u64::from(part_rows)
            + u64::from(copy_rows)
            + u64::from(copy_rows / 8)
            + u64::from(WORKGROUP * 16))
        .min(u64::from(built.capacity)) as u32;
        scene.built_rows = rows_built;
        let part_rows = part_rows.min(rows_built);
        let queue = &self.gpu.queue;
        let device = &self.gpu.device;
        if part_rows > 0 {
            queue.write_buffer(
                &built.instances,
                0,
                bytemuck::cast_slice(&rows[..part_rows as usize * INSTANCE_WORDS]),
            );
        }
        let mut params = [0u32; 16];
        params[..4].copy_from_slice(&[eye.x, eye.y, eye.z, scene.reach].map(f32::to_bits));
        params[4..8].copy_from_slice(&wind.map(f32::to_bits));
        let chunks = scene.runs.as_ref().map_or(0, |runs| runs.chunk_count);
        params[8..12].copy_from_slice(&[chunks, ROW_CHUNKS, part_rows, rows_built]);
        params[12] = time.to_bits();
        queue.write_buffer(&scene.params, 0, bytemuck::cast_slice(&params));

        // The copies' rows, after the cleared rest of the buffer.
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("render-wgpu ray instances"),
        });
        encoder.clear_buffer(&scene.counters, 0, None);
        if rows_built > part_rows {
            encoder.clear_buffer(
                &built.instances,
                u64::from(part_rows) * INSTANCE_BYTES,
                Some(u64::from(rows_built - part_rows) * INSTANCE_BYTES),
            );
        }
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("render-wgpu ray instances"),
                timestamp_writes: scene
                    .timer
                    .as_ref()
                    .and_then(|timer| timer.compute_writes_between(true, false)),
            });
            if let (Some(runs), Some(pipeline)) = (&scene.runs, &scene.pipeline) {
                if runs.chunk_count > 0 {
                    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("render-wgpu ray instances"),
                        layout: &pipeline.get_bind_group_layout(0),
                        entries: &[
                            entry(0, &scene.params),
                            entry(1, &self.parts_buffer),
                            entry(2, self.scatter_copies.buffer()),
                            entry(3, &runs.runs),
                            entry(4, &runs.chunks),
                            entry(5, &runs.level_parts),
                            entry(6, &built.instances),
                            entry(7, &scene.counters),
                        ],
                    });
                    pass.set_pipeline(pipeline);
                    pass.set_bind_group(0, &bind_group, &[]);
                    pass.dispatch_workgroups(
                        runs.chunk_count.min(ROW_CHUNKS),
                        runs.chunk_count.div_ceil(ROW_CHUNKS),
                        1,
                    );
                }
            }
        }
        let written = encoder.finish();

        // The build, recorded through wgpu-hal in an encoder of its own: an
        // encoder takes wgpu's commands or the raw ones, not both.
        let mut raw = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("render-wgpu ray tlas"),
        });
        let recorded = record_tlas_build(&mut raw, built, rows_built);
        if recorded {
            // SAFETY: `record_tlas_build` built this TLAS, and only it, in
            // `raw`, from BLAS built in earlier submissions
            // (`build_blases`).
            unsafe {
                raw.mark_acceleration_structures_built(
                    std::iter::empty::<&wgpu::Blas>(),
                    [&built.tlas],
                )
            };
        }
        let raw = raw.finish();

        // The timer's end and the counters' read-back.
        let mut after = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("render-wgpu ray tlas end"),
        });
        if let Some(timer) = &mut scene.timer {
            drop(after.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("render-wgpu ray tlas end"),
                timestamp_writes: timer.compute_writes_between(false, true),
            }));
            timer.resolve(&mut after);
        }
        let read_back = scene.pending_counters.is_none();
        if read_back {
            after.copy_buffer_to_buffer(&scene.counters, 0, &scene.counters_readback, 0, None);
        }
        queue.submit([written, raw, after.finish()]);
        if let Some(timer) = &mut scene.timer {
            timer.submitted();
        }
        if read_back {
            let (sender, receiver) = std::sync::mpsc::channel();
            scene
                .counters_readback
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |result| {
                    let _ = sender.send(result);
                });
            scene.pending_counters = Some(receiver);
        }
        if recorded {
            scene.tlas_builds += 1;
            scene.traceable = true;
        }
        scene.tlas_cpu_ms = started.elapsed().as_secs_f64() * 1e3;
        scene.build_cpu_ms_total += scene.tlas_cpu_ms;
    }

    /// How far the sun's cascades reach from the camera: the farthest of
    /// the shadowing directional lights' (`shadows.rs`), those rays trace
    /// in place of their cascades included, or the default.
    fn sun_shadow_reach(&self) -> f32 {
        let traced = self
            .shadow_candidates
            .iter()
            .filter_map(|candidate| candidate.ray_reach);
        self.shadow_candidates
            .iter()
            .flat_map(|candidate| &candidate.layers)
            .filter_map(|layer| match layer {
                crate::shadows::LayerSource::Cascade {
                    distance,
                    backdrop: false,
                    ..
                } => Some(*distance),
                _ => None,
            })
            .chain(traced)
            .reduce(f32::max)
            .unwrap_or(DEFAULT_REACH)
    }

    /// The scatter runs of the scene layer as the instance pass reads them:
    /// each run's copies, levels and thinning, its level parts' BLAS
    /// addresses, masks and wind bends, and a chunk per 64 copies.
    fn scatter_tables(&self, scene: &RayScene) -> Option<Runs> {
        let parts = &self.tables.parts;
        let live = |id: PartId| {
            let state = &parts.state[id as usize];
            parts.meta[id as usize].is_some()
                && state.shown
                && state.layer == render_model::RenderLayer::Scene
                && !state.class.blend
        };
        let mut handles: Vec<_> = self.tables.scatter_runs.keys().copied().collect();
        handles.sort_unstable();
        let mut runs: Vec<u32> = Vec::new();
        let mut chunks: Vec<u32> = Vec::new();
        let mut level_parts: Vec<u32> = Vec::new();
        let mut run_count = 0u32;
        for handle in handles {
            let run = &self.tables.scatter_runs[&handle];
            if !live(run.part) || run.copies.count == 0 {
                continue;
            }
            let mut level_first = [0u32; 4];
            let mut level_count = [0u32; 4];
            for (level, ids) in run.levels.iter().take(4).enumerate() {
                level_first[level] = (level_parts.len() / 4) as u32;
                for &id in ids.iter().filter(|id| live(**id) && **id < 1 << 24) {
                    let state = &parts.state[id as usize];
                    let Some(part) = parts.meta[id as usize].as_ref() else {
                        continue;
                    };
                    let address = scene
                        .blases
                        .get(&blas_key(part, state))
                        .and_then(|entry| entry.blas.handle())
                        .unwrap_or(0);
                    let mut mask = part_mask(state);
                    if !run.casts {
                        mask &= !MASK_SHADOW;
                    }
                    level_parts.extend_from_slice(&[
                        address as u32,
                        (address >> 32) as u32,
                        id | (u32::from(mask) << 24),
                        self.wind_bend(&part.material, state.class.features)
                            .to_bits(),
                    ]);
                    level_count[level] += 1;
                }
            }
            let mut starts = [0.0f32; 4];
            for (level, start) in run.starts.iter().take(4).enumerate() {
                starts[level] = *start;
            }
            let mut thin_distance = [0.0f32; 4];
            let mut thin_share = [1.0f32; 4];
            for (index, [distance, share]) in run.thinning.iter().take(4).enumerate() {
                thin_distance[index] = *distance;
                thin_share[index] = *share;
            }
            let index = run_count;
            run_count += 1;
            runs.extend_from_slice(&[
                run.copies.first,
                run.copies.count,
                run.part,
                run.levels.len().min(4) as u32,
            ]);
            runs.extend(starts.map(f32::to_bits));
            runs.extend(
                [
                    run.band,
                    run.max_scale.max(1.0),
                    run.radius,
                    run.thinning.len().min(4) as f32,
                ]
                .map(f32::to_bits),
            );
            runs.extend(thin_distance.map(f32::to_bits));
            runs.extend(thin_share.map(f32::to_bits));
            runs.extend(level_first);
            runs.extend(level_count);
            runs.extend_from_slice(&[u32::from(run.last_mesh_level).min(3), 0, 0, 0]);
            for first in (0..run.copies.count).step_by(WORKGROUP as usize) {
                chunks.extend_from_slice(&[index, first]);
            }
        }
        debug_assert_eq!(runs.len(), run_count as usize * RUN_WORDS);
        if run_count == 0 {
            return None;
        }
        let device = &self.gpu.device;
        let upload = |label, words: &[u32]| {
            crate::gpu::buffer_init(
                device,
                &wgpu::util::BufferInitDescriptor {
                    label: Some(label),
                    contents: bytemuck::cast_slice(words),
                    usage: wgpu::BufferUsages::STORAGE,
                },
            )
        };
        Some(Runs {
            runs: upload("render-wgpu ray instances runs", &runs),
            level_parts: upload("render-wgpu ray instances parts", &level_parts),
            chunk_count: (chunks.len() / 2) as u32,
            chunks: upload("render-wgpu ray instances chunks", &chunks),
        })
    }

    /// Trace `rays` against the ray scene and return each one's closest
    /// hit, waiting for the GPU: for tools and tests. Empty while there is
    /// no ray scene or it has not been built.
    #[doc(hidden)]
    pub fn trace_rays(&mut self, rays: &[ProbeRay]) -> Vec<Option<ProbeHit>> {
        let Some(scene) = self.ray_scene.as_mut() else {
            return Vec::new();
        };
        let (Some(built), false) = (scene.built.as_ref(), rays.is_empty()) else {
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
                    resource: built.tlas.as_binding(),
                },
                entry(1, &input),
                entry(2, &output),
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

fn entry(binding: u32, buffer: &wgpu::Buffer) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry {
        binding,
        resource: buffer.as_entire_binding(),
    }
}

/// The BLAS a part's index range and material take.
fn blas_key(part: &crate::tables::Part, state: &crate::tables::PartState) -> BlasKey {
    BlasKey {
        mesh: part.mesh.clone(),
        first_index: part.first_index,
        index_count: part.index_count,
        opaque: !state.class.blend && !state.class.features.caster_fragment(),
    }
}

/// What a part is to a ray: it casts (honouring `shadowCasting`), and it
/// is opaque.
fn part_mask(state: &crate::tables::PartState) -> u8 {
    let mut mask = 0;
    if state.casts_shadows && state.class.shadow {
        mask |= MASK_SHADOW;
    }
    if !state.class.blend {
        mask |= MASK_GI;
    }
    mask
}

/// `transform` (3×4, row-major, its origin in the last column) leaning in
/// the wind as `rusty::wind` leans a vertex: each point moves along the
/// wind by its height above the origin times the lean, a shear of the
/// linear columns. `wind` is the frame's (x, z, strength, gustiness).
fn leaning(mut transform: [f32; 12], bend: f32, wind: [f32; 4], time: f32) -> [f32; 12] {
    if bend <= 0.0 || wind[2] <= 0.0 {
        return transform;
    }
    let place = transform[3] * 0.37 + transform[11] * 0.61;
    let gust = 0.5 + 0.5 * (time * 0.9 + place).sin() * (time * 2.3 + place * 1.7).sin();
    let lean = wind[2] * (1.0 + (gust - 1.0) * wind[3]) * bend;
    for column in 0..3 {
        let height = transform[4 + column];
        transform[column] += wind[0] * lean * height;
        transform[8 + column] += wind[1] * lean * height;
    }
    transform
}

/// An instance buffer, a TLAS and a scratch buffer for `capacity` rows;
/// none when the device has no room for them.
fn made_for(gpu: &Gpu, capacity: u32) -> Option<Built> {
    let device = &gpu.device;
    let flags = wgpu::AccelerationStructureFlags::PREFER_FAST_TRACE;
    let scratch_size = tlas_scratch_bytes(device, capacity, flags)?;
    let instances = crate::gpu::allocating(device, || {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("render-wgpu ray instances"),
            size: u64::from(capacity) * INSTANCE_BYTES,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::TLAS_INPUT
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    })
    .ok()?;
    let tlas = crate::gpu::allocating(device, || {
        device.create_tlas(&wgpu::CreateTlasDescriptor {
            label: Some("render-wgpu ray scene"),
            max_instances: capacity,
            flags,
            update_mode: wgpu::AccelerationStructureUpdateMode::Build,
        })
    })
    .ok()?;
    let scratch = scratch_buffer(device, scratch_size)?;
    Some(Built {
        instances,
        tlas,
        scratch,
        capacity,
    })
}

/// The scratch bytes a TLAS of `capacity` instances needs to build.
fn tlas_scratch_bytes(
    device: &wgpu::Device,
    capacity: u32,
    flags: wgpu::AccelerationStructureFlags,
) -> Option<u64> {
    use wgpu::hal::Device as _;
    let entries = wgpu::hal::AccelerationStructureEntries::Instances(
        wgpu::hal::AccelerationStructureInstances::<wgpu::hal::vulkan::Buffer> {
            buffer: None,
            offset: 0,
            count: capacity,
        },
    );
    // SAFETY: `as_hal` hands out the device while the guard lives; the
    // size query only reads the descriptor (no buffer is named, as the
    // query allows) and creates, uses or destroys nothing.
    unsafe {
        let hal = device.as_hal::<Vulkan>()?;
        let sizes = hal.get_acceleration_structure_build_sizes(
            &wgpu::hal::GetAccelerationStructureBuildSizesDescriptor {
                entries: &entries,
                flags,
            },
        );
        Some(sizes.build_scratch_size.max(4))
    }
}

/// A build's scratch buffer of `size` bytes: made by wgpu-hal, which aligns
/// it as acceleration structure builds require (wgpu has no scratch
/// usage), and handed to wgpu, which frees it once dropped.
fn scratch_buffer(device: &wgpu::Device, size: u64) -> Option<wgpu::Buffer> {
    use wgpu::hal::Device as _;
    // SAFETY: the hal device makes a buffer the size and usage asked for;
    // the guard is dropped before wgpu takes the buffer. wgpu takes it as a
    // storage buffer of the same size, which it never binds: only the
    // hal-recorded builds use it (`record_tlas_build`), and the ray scene
    // keeps it until the GPU has finished them (`Retired`).
    unsafe {
        let raw = {
            let hal = device.as_hal::<Vulkan>()?;
            hal.create_buffer(&wgpu::hal::BufferDescriptor {
                label: Some("render-wgpu ray tlas scratch"),
                size,
                usage: wgpu::BufferUses::ACCELERATION_STRUCTURE_SCRATCH,
                memory_flags: wgpu::hal::MemoryFlags::empty(),
            })
            .ok()?
        };
        Some(device.create_buffer_from_hal::<Vulkan>(
            raw,
            &wgpu::BufferDescriptor {
                label: Some("render-wgpu ray tlas scratch"),
                size,
                usage: wgpu::BufferUsages::STORAGE,
                mapped_at_creation: false,
            },
        ))
    }
}

/// Record the TLAS build from the first `rows` of `built.instances` (the
/// rows past those written are zero, inactive) in `encoder`, through
/// wgpu-hal. Returns whether it was recorded (a Vulkan device).
fn record_tlas_build(encoder: &mut wgpu::CommandEncoder, built: &mut Built, rows: u32) -> bool {
    use wgpu::hal::CommandEncoder as _;
    let Built {
        instances,
        tlas,
        scratch,
        capacity,
    } = built;
    let rows = rows.min(*capacity);
    // The hal handles, one `as_hal` guard at a time: a guard holds the
    // device's snatch lock, which must not be taken twice on a thread.
    // SAFETY: each handle stays valid after its guard drops: `built` owns
    // the resources for the whole call, and wgpu takes a resource's raw
    // handle only when it is destroyed or dropped, which the ray scene
    // never does to a resource a build may still use (`Retired`).
    let handles = unsafe {
        (
            instances
                .as_hal::<Vulkan>()
                .map(|guard| &*guard as *const _),
            tlas.as_hal::<Vulkan>().map(|guard| &*guard as *const _),
            scratch.as_hal::<Vulkan>().map(|guard| &*guard as *const _),
        )
    };
    let (Some(instances), Some(tlas), Some(scratch)) = handles else {
        return false;
    };
    // SAFETY:
    // - Handles: valid while `built` lives (above).
    // - Instance buffer: written in an earlier command buffer of the same
    //   submission (the CPU's rows by `write_buffer`, the copies' by the
    //   instance pass, the rest cleared), which wgpu leaves in the copy or
    //   storage state it tracks; the barrier below makes those writes
    //   visible to the build and the one after it returns the buffer to
    //   those states, so wgpu's later barriers order the next frame's
    //   writes after this build's reads.
    // - BLAS: every row names the device address of a BLAS the ray scene
    //   holds (`RayScene::blases`), built in an earlier submission; a BLAS
    //   it lets go is retired until the GPU has finished all work submitted
    //   so far (`Retired`), so no TLAS a submitted command reads names a
    //   destroyed BLAS. A row whose address is 0 is inactive.
    // - TLAS and scratch: made for `capacity` instances, at least `rows`,
    //   with the flags the build uses (`made_for`); the barrier first orders the build after
    //   any earlier reads of the TLAS and the previous build's scratch use,
    //   and the barrier after it makes the TLAS readable by shaders. Both
    //   are retired, not dropped, while a submitted build may use them.
    // - `encoder` takes no wgpu command while the raw one records.
    unsafe {
        let (instances, tlas, scratch): (
            &wgpu::hal::vulkan::Buffer,
            &wgpu::hal::vulkan::AccelerationStructure,
            &wgpu::hal::vulkan::Buffer,
        ) = (&*instances, &*tlas, &*scratch);
        encoder.as_hal_mut::<Vulkan, _, _>(|raw| {
            let Some(raw) = raw else {
                return false;
            };
            let written = wgpu::BufferUses::COPY_DST | wgpu::BufferUses::STORAGE_READ_WRITE;
            raw.transition_buffers(
                [
                    wgpu::hal::BufferBarrier {
                        buffer: instances,
                        usage: wgpu::hal::StateTransition {
                            from: written,
                            to: wgpu::BufferUses::TOP_LEVEL_ACCELERATION_STRUCTURE_INPUT,
                        },
                    },
                    wgpu::hal::BufferBarrier {
                        buffer: scratch,
                        usage: wgpu::hal::StateTransition {
                            from: wgpu::BufferUses::ACCELERATION_STRUCTURE_SCRATCH,
                            to: wgpu::BufferUses::ACCELERATION_STRUCTURE_SCRATCH,
                        },
                    },
                ]
                .into_iter(),
            );
            raw.place_acceleration_structure_barrier(wgpu::hal::AccelerationStructureBarrier {
                usage: wgpu::hal::StateTransition {
                    from: wgpu::hal::AccelerationStructureUses::SHADER_INPUT,
                    to: wgpu::hal::AccelerationStructureUses::BUILD_OUTPUT,
                },
            });
            let entries = wgpu::hal::AccelerationStructureEntries::Instances(
                wgpu::hal::AccelerationStructureInstances {
                    buffer: Some(instances),
                    offset: 0,
                    count: rows,
                },
            );
            raw.build_acceleration_structures(
                1,
                [wgpu::hal::BuildAccelerationStructureDescriptor {
                    entries: &entries,
                    mode: wgpu::hal::AccelerationStructureBuildMode::Build,
                    flags: wgpu::AccelerationStructureFlags::PREFER_FAST_TRACE,
                    source_acceleration_structure: None,
                    destination_acceleration_structure: tlas,
                    scratch_buffer: scratch,
                    scratch_buffer_offset: 0,
                }],
            );
            raw.place_acceleration_structure_barrier(wgpu::hal::AccelerationStructureBarrier {
                usage: wgpu::hal::StateTransition {
                    from: wgpu::hal::AccelerationStructureUses::BUILD_OUTPUT,
                    to: wgpu::hal::AccelerationStructureUses::SHADER_INPUT,
                },
            });
            raw.transition_buffers(
                [wgpu::hal::BufferBarrier {
                    buffer: instances,
                    usage: wgpu::hal::StateTransition {
                        from: wgpu::BufferUses::TOP_LEVEL_ACCELERATION_STRUCTURE_INPUT,
                        to: written,
                    },
                }]
                .into_iter(),
            );
            true
        })
    }
}

/// Compact the BLAS whose builds have completed, within the budget, and
/// return how many were: the TLAS must take them in place of the originals.
fn compact(gpu: &Gpu, scene: &mut RayScene) -> usize {
    let mut swapped = Vec::new();
    for entry in scene.blases.values_mut() {
        if swapped.len() == COMPACTIONS_PER_FRAME {
            break;
        }
        if entry.compaction == Compaction::Preparing && entry.blas.ready_for_compaction() {
            let compacted = gpu.queue.compact_blas(&entry.blas);
            swapped.push(std::mem::replace(&mut entry.blas, compacted));
            entry.compaction = Compaction::Compacted;
        }
    }
    let count = swapped.len();
    for old in swapped {
        scene.retire(&gpu.queue, Retired::Blas(old));
    }
    count
}

/// The ranges from `wanted` (each with its triangles and the walk it has
/// waited since) to build this frame: the longest waiting first, while they
/// fit `budget`, and the first whatever its size, so a range larger than the
/// budget builds alone and one rebuilt every frame (a posing character)
/// cannot hold back a range waiting longer. The rest stay in `wanted`.
fn scheduled<K>(wanted: &mut Vec<(K, u64, u64)>, budget: u64) -> Vec<(K, u64, u64)> {
    wanted.sort_by_key(|(_, _, since)| *since);
    let mut left = budget;
    let mut taken = 0;
    for (index, (_, triangles, _)) in wanted.iter().enumerate() {
        if index > 0 && *triangles > left {
            break;
        }
        left = left.saturating_sub(*triangles);
        taken += 1;
    }
    wanted.drain(..taken).collect()
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
        let hal = device.as_hal::<Vulkan>()?;
        let sizes = hal.get_acceleration_structure_build_sizes(
            &wgpu::hal::GetAccelerationStructureBuildSizesDescriptor {
                entries: &entries,
                flags,
            },
        );
        Some(sizes.acceleration_structure_size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_range_larger_than_the_budget_builds_alone() {
        let mut wanted = vec![("huge", 1_048_577, 1), ("small", 10, 1)];
        let first = scheduled(&mut wanted, 1_048_576);
        assert_eq!(first, [("huge", 1_048_577, 1)]);
        assert_eq!(scheduled(&mut wanted, 1_048_576), [("small", 10, 1)]);
    }

    #[test]
    fn a_range_rebuilt_every_frame_does_not_hold_back_one_waiting_longer() {
        // A 600k-triangle skinned range, first in part order, has a new pose
        // every walk; a 500k range waits from the first walk. Together they
        // pass the budget, so one builds a walk.
        let budget = 1_048_576;
        let mut waiting = vec![("pose", 600_000, 1), ("chunk", 500_000, 1)];
        let first: Vec<_> = scheduled(&mut waiting, budget)
            .into_iter()
            .map(|(key, ..)| key)
            .collect();
        assert_eq!(first, ["pose"]);
        // Built, the pose changes again and waits from walk 2; the chunk,
        // waiting since walk 1, builds first.
        waiting.insert(0, ("pose", 600_000, 2));
        let second: Vec<_> = scheduled(&mut waiting, budget)
            .into_iter()
            .map(|(key, ..)| key)
            .collect();
        assert_eq!(second, ["chunk"]);
        assert_eq!(waiting, [("pose", 600_000, 2)]);
    }

    #[test]
    fn a_swaying_transform_leans_by_height_along_the_wind() {
        // A unit transform at (2, 1, 3); the wind blows along +x at
        // strength 0.5 without gusts, and the material bends 0.2.
        let transform = [1.0, 0.0, 0.0, 2.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 1.0, 3.0];
        let leant = leaning(transform, 0.2, [1.0, 0.0, 0.5, 0.0], 7.0);
        // A point 2 up from the origin moves 0.5 × 0.2 × 2 along x.
        let point = [0.0f32, 2.0, 0.0];
        let x = leant[0] * point[0] + leant[1] * point[1] + leant[2] * point[2] + leant[3];
        assert!((x - (2.0 + 0.2)).abs() < 1e-6, "{x}");
        assert_eq!(leant[3], 2.0, "the origin stays");
        assert_eq!(
            leaning(transform, 0.0, [1.0, 0.0, 0.5, 0.0], 7.0),
            transform
        );
    }
}
