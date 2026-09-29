//! The Engine's wgpu renderer over the retained `PresentationWorld`.
//!
//! The runtime applies the deltas `PresentationWorld::apply` returns (a fresh
//! renderer applies the world's snapshot frame first), installs the committed
//! camera composition, then renders it into an offscreen target or a window
//! surface. Output image jobs render their frozen frame in isolation. There is
//! no serialization, realizer layer or mirrored contract between the retained
//! model and the GPU.
//!
//! Inside, the renderer is typed handle-keyed tables (see [`tables`]) and a
//! fixed pass pipeline (see `frame` and `composition`): apply deltas,
//! propagate dirty transforms, upload changed rows, then per view pass build
//! the draw list and encode it, then present or read back. It has no scheduler, plugin registration, generic
//! query layer or second retained world, and it neither validates nor
//! sequences product mutations.
//!
//! Only this crate may depend on wgpu (`scripts/dependency_boundary_check.py`);
//! no wgpu type appears in its public API.

#![forbid(unsafe_code)]

mod animated;
mod apply;
mod batch;
mod camera;
mod capture;
mod compose;
mod composition;
mod convert;
mod effects;
mod export;
mod frame;
mod ghost;
mod glb;
mod gpu;
mod labels;
mod particles;
mod pick;
mod pipelines;
mod primitives;
mod resources;
mod shadows;
mod surface;
mod tables;
mod target;
mod video;
mod voxel;
#[cfg(feature = "web-overlay")]
pub mod web;

use std::collections::HashMap;

pub use animated::AnimationFact;
pub use apply::ApplyIssue;
pub use camera::CameraSampleReadout;
pub use composition::{DrawnCamera, TargetReadout, TargetStatus, ViewCompositionReadout};
pub use export::export_glb;
pub use frame::FrameStats;
pub use ghost::GhostPlateReadout;
pub use gpu::{AdapterSummary, Gpu, GpuError};
pub use particles::EntityPositions;
pub use resources::{decode_png_rgba, encode_png, NoResources, ResourceSource};
pub use surface::{PresentSkip, WindowSurface};
pub use target::OffscreenTarget;
pub use video::{VideoFact, VideoFailure};

use pipelines::{Layouts, Pipelines};
use tables::{Builtin, GpuMesh, GpuTexture, Tables};

/// The Three lane's default world rig (`createNeutralLights([5, 8, 6])`):
/// a hemisphere light and a key light, unless the product disables it.
pub(crate) const NEUTRAL_HEMISPHERE_INTENSITY: f32 = 2.4;
/// Hemisphere ground colour 0x263238, sRGB.
pub(crate) const NEUTRAL_GROUND_SRGB: [f32; 3] = [38.0 / 255.0, 50.0 / 255.0, 56.0 / 255.0];
pub(crate) const NEUTRAL_KEY_INTENSITY: f32 = 2.2;
pub(crate) const NEUTRAL_KEY_POSITION: [f32; 3] = [5.0, 8.0, 6.0];
/// The viewmodel rig's key light, in camera-local coordinates
/// (`createNeutralLights([2, 3, 2])`).
pub(crate) const NEUTRAL_VIEWMODEL_KEY_POSITION: [f32; 3] = [2.0, 3.0, 2.0];
/// Clear colour with no background or sky selected: 0x101820, sRGB.
pub(crate) const DEFAULT_CLEAR_SRGB: [f32; 3] = [16.0 / 255.0, 24.0 / 255.0, 32.0 / 255.0];

pub(crate) fn srgb_to_linear(value: f32) -> f32 {
    if value <= 0.040_45 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

/// Host choices that are not part of the retained model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RendererOptions {
    /// `RustyEngineProductDefaultWorldLights`: the neutral rig lights the world
    /// unless the product disables it.
    pub default_world_lights: bool,
    /// `RustyEngineProductDefaultViewmodelLights`: the neutral rig lights the
    /// viewmodel layer unless the product disables it.
    pub default_viewmodel_lights: bool,
    /// Render shadow maps for world lights whose `shadow_intent` requests
    /// them (Three's `lighting.shadows.enabled` host option). Off by
    /// default; C# products do not enable it.
    pub shadows: bool,
}

impl Default for RendererOptions {
    fn default() -> Self {
        Self {
            default_world_lights: true,
            default_viewmodel_lights: true,
            shadows: false,
        }
    }
}

pub struct Renderer {
    gpu: Gpu,
    options: RendererOptions,
    layouts: Layouts,
    pipelines: Vec<Pipelines>,
    tables: Tables,
    white: GpuTexture,
    builtins: HashMap<Builtin, GpuMesh>,
    unlit_material: wgpu::BindGroup,
    lit_fallback_material: wgpu::BindGroup,
    frame_buffer: wgpu::Buffer,
    parts_buffer: wgpu::Buffer,
    lights_buffer: wgpu::Buffer,
    lights: frame::LightRanges,
    /// Part ids: the caster list, then the world and viewmodel view lists.
    instances_buffer: wgpu::Buffer,
    casters: batch::DrawList,
    /// The caster ids are in the instance buffer.
    casters_uploaded: bool,
    /// Per view layer (world, viewmodel): the last draw list.
    views: [Option<frame::ViewCache>; 2],
    shadows: shadows::ShadowMaps,
    frame_bind_group: wgpu::BindGroup,
    caster_bind_group: wgpu::BindGroup,
    sky_bind_group: Option<wgpu::BindGroup>,
    /// The Engine presentation timeline animated poses advance on.
    animation_time: f64,
    animation_facts: Vec<animated::AnimationFact>,
    /// Realization generation per source entity, as the runtime reads it.
    animation_generations: HashMap<u64, u64>,
    ghosts: HashMap<render_presentation::GhostPlateHandle, ghost::GhostPlate>,
    ghost_pipelines: ghost::GhostPipelines,
    /// Counts applied deltas; offscreen composition targets re-render when
    /// it moves past the value they were drawn at.
    scene_generation: u64,
    compose: compose::Compose,
    composition: composition::ViewComposition,
    effects: effects::Effects,
    particles: particles::Particles,
    labels: labels::Labels,
    video: video::Video,
}

/// Initial storage sizes; both grow by doubling.
const INITIAL_PARTS_BYTES: u64 = 64 * 1024;
const INITIAL_LIGHTS_BYTES: u64 = 4 * 1024;
const INITIAL_INSTANCES_BYTES: u64 = 16 * 1024;

impl Renderer {
    pub fn new(gpu: &Gpu, options: RendererOptions) -> Self {
        let device = &gpu.device;
        let layouts = Layouts::new(device);
        let frame_buffer = frame::frame_uniform_buffer(device);
        let parts_buffer = frame::storage_buffer(device, "render-wgpu parts", INITIAL_PARTS_BYTES);
        let lights_buffer =
            frame::storage_buffer(device, "render-wgpu lights", INITIAL_LIGHTS_BYTES);
        let instances_buffer =
            frame::storage_buffer(device, "render-wgpu instances", INITIAL_INSTANCES_BYTES);
        let shadows = shadows::ShadowMaps::new(device, &layouts.shadow_layer);
        let frame_bind_group = frame::frame_bind_group(
            device,
            &layouts.frame,
            frame::FrameBindings {
                frame: &frame_buffer,
                parts: &parts_buffer,
                lights: &lights_buffer,
                instances: &instances_buffer,
                shadows: &shadows,
            },
        );
        let caster_bind_group = frame::caster_bind_group(
            device,
            &layouts.casters,
            &parts_buffer,
            &instances_buffer,
            &shadows,
        );
        let white = white_texture(gpu);
        let (unlit_material, lit_fallback_material) =
            apply::builtin_materials(device, &layouts.material, &white);
        let effects = effects::Effects::new(device, &layouts.frame);
        let mut renderer = Self {
            gpu: gpu.clone(),
            options,
            layouts,
            pipelines: Vec::new(),
            tables: Tables::new(),
            white,
            builtins: HashMap::new(),
            unlit_material,
            lit_fallback_material,
            frame_buffer,
            parts_buffer,
            lights_buffer,
            lights: Default::default(),
            instances_buffer,
            casters: batch::DrawList::default(),
            casters_uploaded: false,
            views: Default::default(),
            shadows,
            frame_bind_group,
            caster_bind_group,
            sky_bind_group: None,
            animation_time: 0.0,
            animation_facts: Vec::new(),
            animation_generations: HashMap::new(),
            ghosts: HashMap::new(),
            ghost_pipelines: ghost::GhostPipelines::new(device),
            scene_generation: 0,
            compose: compose::Compose::new(device),
            composition: Default::default(),
            effects,
            particles: Default::default(),
            labels: Default::default(),
            video: video::Video::new(device),
        };
        for kind in [
            Builtin::Cube,
            Builtin::Sphere,
            Builtin::Quad,
            Builtin::Point,
        ] {
            let geometry = primitives::builtin(kind);
            let count = geometry.indices.len() as u32;
            let mesh = renderer.upload_vertices(
                "render-wgpu builtin",
                &geometry.vertices,
                &geometry.indices,
                tables::Topology::Triangles,
                vec![(0, 0, count)],
                Default::default(),
            );
            renderer.builtins.insert(kind, mesh);
        }
        renderer
    }

    pub fn options(&self) -> RendererOptions {
        self.options
    }

    /// Change host options; lights (and shadow layers) are re-derived on the
    /// next render.
    pub fn set_options(&mut self, options: RendererOptions) {
        self.options = options;
        self.tables.lights_dirty = true;
    }

    /// Retained table sizes, for diagnostics and tests.
    pub fn mesh_memory(&self) -> MeshMemory {
        let mut memory = MeshMemory::default();
        let mut seen = std::collections::HashSet::new();
        let mut count = |mesh: &tables::GpuMesh| {
            memory.meshes += 1;
            memory.gpu_bytes += mesh.vertices.size() + mesh.indices.size();
            if seen.insert(std::sync::Arc::as_ptr(&mesh.cpu)) {
                memory.cpu_geometry_bytes +=
                    (mesh.cpu.positions.len() * 12 + mesh.cpu.indices.len() * 4) as u64;
            }
        };
        self.tables.static_meshes.values().for_each(&mut count);
        self.tables.payload_meshes.values().for_each(&mut count);
        self.tables
            .voxel_objects
            .values()
            .flat_map(|row| row.meshes.iter())
            .for_each(&mut count);
        self.for_each_animated_mesh(&mut count);
        memory
    }

    pub fn table_counts(&self) -> TableCounts {
        TableCounts {
            textures: self.tables.textures.len(),
            materials: self.tables.materials.len(),
            static_meshes: self.tables.static_meshes.len(),
            nodes: self.tables.nodes.len(),
            parts: self.tables.parts.meta.iter().flatten().count(),
            lights: self.tables.lights.len(),
            atlases: self.tables.atlases.len(),
            voxel_objects: self.tables.voxel_objects.len(),
            animated_meshes: self.tables.animated_assets.len(),
            animated_instances: self.tables.animated.len(),
            shadow_layers: self.shadows.layers as usize,
        }
    }
}

/// Mesh memory: the CPU geometry copies kept for picking and bounds, beside
/// the GPU vertex and index bytes of the same meshes (#8849).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MeshMemory {
    pub meshes: usize,
    pub cpu_geometry_bytes: u64,
    pub gpu_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableCounts {
    pub textures: usize,
    pub materials: usize,
    pub static_meshes: usize,
    pub nodes: usize,
    pub parts: usize,
    pub lights: usize,
    pub atlases: usize,
    pub voxel_objects: usize,
    pub animated_meshes: usize,
    pub animated_instances: usize,
    pub shadow_layers: usize,
}

/// A 1×1 white texture: untextured materials sample it.
fn white_texture(gpu: &Gpu) -> GpuTexture {
    use wgpu::util::DeviceExt;
    let texture = gpu.device.create_texture_with_data(
        &gpu.queue,
        &wgpu::TextureDescriptor {
            label: Some("render-wgpu white"),
            size: target::extent(1, 1),
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        },
        wgpu::util::TextureDataOrder::LayerMajor,
        &[255; 4],
    );
    GpuTexture {
        size: (1, 1),
        view: texture.create_view(&Default::default()),
        sampler: gpu.device.create_sampler(&Default::default()),
    }
}
