//! The Engine's wgpu renderer over the retained `PresentationWorld`.
//!
//! The runtime applies the deltas `PresentationWorld::apply` returns (a fresh
//! renderer applies the world's snapshot frame first), then renders a view
//! into an offscreen target or a window surface. There is no serialization,
//! realizer layer or mirrored contract between the retained model and the GPU.
//!
//! Inside, the renderer is typed handle-keyed tables (see [`tables`]) and a
//! fixed pass pipeline (see [`frame`]): apply deltas, propagate dirty
//! transforms, upload changed rows, build the view's draw list, encode passes,
//! present or read back. It has no scheduler, plugin registration, generic
//! query layer or second retained world, and it neither validates nor
//! sequences product mutations.
//!
//! Only this crate may depend on wgpu (`scripts/dependency_boundary_check.py`);
//! no wgpu type appears in its public API.

#![forbid(unsafe_code)]

mod apply;
mod frame;
mod gpu;
mod pipelines;
mod primitives;
mod resources;
mod surface;
pub mod tables;
mod target;

use std::collections::HashMap;

pub use apply::ApplyIssue;
pub use frame::FrameStats;
pub use gpu::{AdapterSummary, Gpu, GpuError};
pub use resources::{decode_png_rgba, encode_png, NoResources, ResourceSource};
pub use surface::{PresentSkip, WindowSurface};
pub use target::OffscreenTarget;

use pipelines::{Layouts, Pipelines};
use tables::{Builtin, GpuMesh, GpuTexture, Tables};

/// The Three lane's default world rig (`createNeutralLights([5, 8, 6])`):
/// a hemisphere light and a key light, unless the product disables it.
pub(crate) const NEUTRAL_HEMISPHERE_INTENSITY: f32 = 2.4;
/// Hemisphere ground colour 0x263238, sRGB.
pub(crate) const NEUTRAL_GROUND_SRGB: [f32; 3] = [38.0 / 255.0, 50.0 / 255.0, 56.0 / 255.0];
pub(crate) const NEUTRAL_KEY_INTENSITY: f32 = 2.2;
pub(crate) const NEUTRAL_KEY_POSITION: [f32; 3] = [5.0, 8.0, 6.0];
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
}

impl Default for RendererOptions {
    fn default() -> Self {
        Self {
            default_world_lights: true,
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
    light_count: u32,
    frame_bind_group: wgpu::BindGroup,
    sky_bind_group: Option<wgpu::BindGroup>,
}

/// Initial storage sizes; both grow by doubling.
const INITIAL_PARTS_BYTES: u64 = 64 * 1024;
const INITIAL_LIGHTS_BYTES: u64 = 4 * 1024;

impl Renderer {
    pub fn new(gpu: &Gpu, options: RendererOptions) -> Self {
        let device = &gpu.device;
        let layouts = Layouts::new(device);
        let frame_buffer = frame::frame_uniform_buffer(device);
        let parts_buffer = frame::storage_buffer(device, "render-wgpu parts", INITIAL_PARTS_BYTES);
        let lights_buffer =
            frame::storage_buffer(device, "render-wgpu lights", INITIAL_LIGHTS_BYTES);
        let frame_bind_group = frame::frame_bind_group(
            device,
            &layouts.frame,
            &frame_buffer,
            &parts_buffer,
            &lights_buffer,
        );
        let white = white_texture(gpu);
        let (unlit_material, lit_fallback_material) =
            apply::builtin_materials(device, &layouts.material, &white);
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
            light_count: 0,
            frame_bind_group,
            sky_bind_group: None,
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

    /// Change host options; lights are re-derived on the next render.
    pub fn set_options(&mut self, options: RendererOptions) {
        self.options = options;
        self.tables.lights_dirty = true;
    }

    /// Retained table sizes, for diagnostics and tests.
    pub fn table_counts(&self) -> TableCounts {
        TableCounts {
            textures: self.tables.textures.len(),
            materials: self.tables.materials.len(),
            static_meshes: self.tables.static_meshes.len(),
            nodes: self.tables.nodes.len(),
            parts: self.tables.parts.meta.iter().flatten().count(),
            lights: self.tables.lights.len(),
            atlases: self.tables.atlases.len(),
        }
    }
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
        view: texture.create_view(&Default::default()),
        sampler: gpu.device.create_sampler(&Default::default()),
    }
}
