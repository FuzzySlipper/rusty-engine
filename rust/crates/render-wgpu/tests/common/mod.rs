//! Shared fixture harness for the view and effect screenshot tests: scenes
//! go through `PresentationWorld` as the runtime applies them, and frames are
//! compared with `tests/screenshots/` at the tolerance `screenshots.rs`
//! documents.

#![allow(dead_code, reason = "each test file uses part of the harness")]

use std::{borrow::Cow, collections::HashMap, path::PathBuf};

use render_host_contracts::{
    RendererCameraPose, RendererCameraProjection, RendererCompositionCamera,
    RendererCompositionView, RendererViewComposition, RendererViewTarget, RendererViewport,
};
use render_model::*;
use render_presentation::PresentationWorld;
use render_wgpu::{
    decode_png_rgba, encode_png, Gpu, OffscreenTarget, Renderer, RendererOptions, ResourceSource,
};

#[derive(Default)]
pub struct Resources(pub HashMap<String, Vec<u8>>);

impl ResourceSource for Resources {
    fn bytes(&self, identity: &str) -> Option<Cow<'_, [u8]>> {
        self.0
            .get(identity)
            .map(|bytes| Cow::Borrowed(bytes.as_slice()))
    }
}

impl Resources {
    /// Admit an RGBA8 image as a resource-backed texture, as the runtime does.
    /// Returns the define op and the content hash.
    pub fn texture(
        &mut self,
        id: &str,
        width: u32,
        height: u32,
        rgba: &[u8],
        filter: TextureFilter,
    ) -> (RenderDiff, String) {
        let png = encode_png(width, height, rgba).expect("encode fixture texture");
        let texture = TextureDescriptor::admit_png_rgba8_resource(
            id.to_owned(),
            &png,
            filter,
            TextureWrap::Clamp,
            1,
        )
        .expect("admit fixture texture");
        let payload = texture.payload.as_ref().expect("resource payload");
        if let TexturePayloadSource::Resource { resource } = &payload.source {
            self.0.insert(resource.clone(), png);
        }
        let hash = payload.content_hash.clone();
        (RenderDiff::DefineTexture { texture }, hash)
    }
}

pub const WIDTH: u32 = 320;
pub const HEIGHT: u32 = 180;
pub const CHANNEL_TOLERANCE: u8 = 12;
pub const DIFFERING_PIXEL_FRACTION: f64 = 0.002;

pub struct Harness {
    pub gpu: Gpu,
    pub world: PresentationWorld,
    pub renderer: Renderer,
    pub target: OffscreenTarget,
    pub resources: Resources,
}

/// One device per test binary. The Vulkan loader crashes in
/// `vkSetDebugUtilsObjectNameEXT` when test threads create devices at the same
/// time (seen on RADV); the runtime creates one device, as this does.
pub fn gpu() -> Gpu {
    static GPU: std::sync::OnceLock<Gpu> = std::sync::OnceLock::new();
    GPU.get_or_init(|| Gpu::headless().expect("view tests need a wgpu adapter"))
        .clone()
}

impl Harness {
    pub fn new(options: RendererOptions) -> Self {
        let gpu = gpu();
        Self {
            renderer: Renderer::new(&gpu, options),
            target: OffscreenTarget::new(&gpu, WIDTH, HEIGHT, 4),
            world: PresentationWorld::default(),
            resources: Resources::default(),
            gpu,
        }
    }

    pub fn apply(&mut self, ops: Vec<RenderDiff>) {
        let delta = self
            .world
            .apply(RenderFrameDiff {
                publication: None,
                ops,
            })
            .expect("retained model admits the fixture");
        let issues = self.renderer.apply(&delta, &self.resources);
        assert!(
            issues.is_empty(),
            "renderer skipped fixture ops: {issues:?}"
        );
    }

    pub fn composition(&mut self, time: f64) -> (render_wgpu::FrameStats, Vec<u8>) {
        let stats = self.renderer.render_view_composition(&self.target, time);
        (stats, self.target.read_rgba(&self.gpu))
    }

    pub fn single(&mut self, camera: &RendererCompositionCamera) -> Vec<u8> {
        self.renderer.render_offscreen(camera, &self.target);
        self.target.read_rgba(&self.gpu)
    }
}

pub fn camera(id: &str, position: [f64; 3], yaw: f64, pitch: f64) -> RendererCompositionCamera {
    RendererCompositionCamera {
        id: id.to_owned(),
        pose: RendererCameraPose {
            position,
            pitch_degrees: pitch,
            yaw_degrees: yaw,
        },
        basis: None,
        projection: RendererCameraProjection::Perspective {
            fov_y_degrees: 60.0,
            near: 0.05,
            far: 100.0,
        },
        motion: None,
        viewmodel_fov_y_degrees: 0.0,
    }
}

pub fn viewport(x: f64, y: f64, width: f64, height: f64) -> RendererViewport {
    RendererViewport {
        x,
        y,
        width,
        height,
    }
}

pub fn primary_view(
    id: &str,
    camera: &str,
    area: RendererViewport,
    order: u64,
) -> RendererCompositionView {
    RendererCompositionView {
        id: id.to_owned(),
        camera_id: camera.to_owned(),
        target: RendererViewTarget::Primary,
        viewport: area,
        order,
        viewport_anchor: None,
    }
}

pub fn composition(
    cameras: Vec<RendererCompositionCamera>,
    views: Vec<RendererCompositionView>,
) -> RendererViewComposition {
    RendererViewComposition {
        cameras,
        targets: Vec::new(),
        views,
        presentations: Vec::new(),
    }
}

pub fn transform(translation: [f32; 3], yaw_degrees: f32, scale: f32) -> Transform {
    let half = yaw_degrees.to_radians() * 0.5;
    Transform {
        translation,
        rotation: [0.0, half.sin(), 0.0, half.cos()],
        scale: [scale; 3],
    }
}

pub fn payload(positions: Vec<f32>, normals: Vec<f32>, indices: Vec<u32>) -> MeshPayloadDescriptor {
    let (mut min, mut max) = ([f32::MAX; 3], [f32::MIN; 3]);
    for point in positions.as_chunks::<3>().0 {
        for axis in 0..3 {
            min[axis] = min[axis].min(point[axis]);
            max[axis] = max[axis].max(point[axis]);
        }
    }
    let attribute = |name, components| MeshAttribute {
        name,
        components,
        kind: MeshAttributeKind::F32,
    };
    MeshPayloadDescriptor {
        texture_space: None,
        distance_field: None,
        layout: MeshBufferLayout {
            vertex_count: (positions.len() / 3) as u32,
            index_count: indices.len() as u32,
            index_width: MeshIndexWidth::U32,
            attributes: vec![
                attribute(MeshAttributeName::Position, 3),
                attribute(MeshAttributeName::Normal, 3),
            ],
        },
        groups: vec![MeshGroupDescriptor {
            material_slot: 0,
            start: 0,
            count: indices.len() as u32,
        }],
        bounds: MeshBoundsDescriptor { min, max },
        source: MeshPayloadSource::Inline {
            positions,
            normals,
            uvs: None,
            colors: None,
            indices,
        },
        provenance: MeshProvenance::Generated,
        layer_weights: false,
        layer_palette: Vec::new(),
        vertex_occlusion: false,
    }
}

/// A unit box centred on the origin with per-face normals.
pub fn cube() -> MeshPayloadDescriptor {
    let (mut positions, mut normals, mut indices) = (vec![], vec![], vec![]);
    let faces: [([f32; 3], [f32; 3], [f32; 3]); 6] = [
        ([1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]),
        ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
        ([0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, -1.0]),
        ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        ([0.0, 0.0, -1.0], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
    ];
    for (normal, u, v) in faces {
        let base = (positions.len() / 3) as u32;
        for (su, sv) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            for axis in 0..3 {
                positions.push((normal[axis] + u[axis] * su + v[axis] * sv) * 0.5);
            }
            normals.extend_from_slice(&normal);
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    payload(positions, normals, indices)
}

pub fn floor(size: f32) -> MeshPayloadDescriptor {
    let h = size * 0.5;
    payload(
        vec![-h, 0.0, h, h, 0.0, h, h, 0.0, -h, -h, 0.0, -h],
        [0.0, 1.0, 0.0].repeat(4),
        vec![0, 1, 2, 0, 2, 3],
    )
}

/// A lit material and a static mesh drawing `payload` with it.
pub fn coloured_mesh(
    name: &str,
    payload: MeshPayloadDescriptor,
    color: [f32; 4],
) -> Vec<RenderDiff> {
    let material = format!("material/{name}");
    vec![
        RenderDiff::DefineMaterial {
            material: RenderMaterialDescriptor {
                texture_transform: None,
                stochastic_tiling: None,
                terrain_layers: None,
                shader: None,
                id: material.clone(),
                color,
                texture: None,
                roughness: 0.8,
                metalness: 0.0,
                texture_tint: [1.0; 4],
                emission_color: [0.0; 3],
                emission_intensity: 0.0,
                uv_strategy: MaterialUvStrategy::Flat,
                alpha_mode: MaterialAlphaModeDescriptor::Opaque,
                double_sided: false,
                voxel_surface: None,
                normal_map: None,
                triplanar: None,
                emission_map: Default::default(),
                occlusion_map: Default::default(),
                unlit: false,
                flat_shading: false,
                keep_dry: false,
                wind: None,
                water: None,
                translucent_shadow: false,
            },
        },
        RenderDiff::DefineStaticMesh {
            asset: StaticMeshAsset {
                asset: format!("mesh/{name}"),
                payload,
                material_slots: vec![MeshMaterialSlot { slot: 0, material }],
                collision: MeshCollisionPolicy::VisualOnly,
            },
        },
    ]
}

pub fn instance(handle: u64, parent: Option<u64>, mesh: &str, transform: Transform) -> RenderDiff {
    RenderDiff::CreateStaticMeshInstance {
        handle: RenderHandle::new(handle),
        parent: parent.map(RenderHandle::new),
        instance: StaticMeshInstanceDescriptor {
            asset: format!("mesh/{mesh}"),
            transform,
            visible: true,
            material_overrides: Vec::new(),
            metadata: RenderMetadata::default(),
            layer: RenderLayer::Scene,
            shadow_casting: Default::default(),
        },
    }
}

/// A floor and three coloured boxes around (0, 0, -4).
pub fn room() -> Vec<RenderDiff> {
    let mut ops = vec![RenderDiff::SetBackgroundColor {
        color: [0.02, 0.03, 0.06, 1.0],
    }];
    ops.extend(coloured_mesh("floor", floor(10.0), [0.45, 0.45, 0.5, 1.0]));
    ops.extend(coloured_mesh("red", cube(), [0.85, 0.12, 0.1, 1.0]));
    ops.extend(coloured_mesh("green", cube(), [0.15, 0.75, 0.2, 1.0]));
    ops.extend(coloured_mesh("blue", cube(), [0.15, 0.3, 0.9, 1.0]));
    ops.extend([
        instance(1, None, "floor", transform([0.0, -0.5, -4.0], 0.0, 1.0)),
        instance(10, None, "red", transform([-1.3, 0.0, -4.0], 30.0, 1.0)),
        instance(11, None, "green", transform([0.0, 0.25, -5.2], 0.0, 1.5)),
        instance(12, None, "blue", transform([1.3, 0.0, -3.6], -20.0, 1.0)),
    ]);
    ops
}

pub fn assert_screenshot(name: &str, width: u32, height: u32, rgba: &[u8]) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let reference = root.join("tests/screenshots").join(format!("{name}.png"));
    let encoded = encode_png(width, height, rgba).expect("encode screenshot");
    if std::env::var_os("RENDER_WGPU_BLESS").is_some() {
        std::fs::write(&reference, encoded).unwrap();
        return;
    }
    let expected = std::fs::read(&reference).unwrap_or_else(|_| {
        panic!(
            "missing {}; run with RENDER_WGPU_BLESS=1",
            reference.display()
        )
    });
    let (expected_width, expected_height, expected) =
        decode_png_rgba(&expected).expect("decode reference");
    assert_eq!(
        (expected_width, expected_height),
        (width, height),
        "{name}: reference size"
    );
    let differing = expected
        .as_chunks::<4>()
        .0
        .iter()
        .zip(rgba.as_chunks::<4>().0)
        .filter(|(a, b)| {
            a.iter()
                .zip(b.iter())
                .any(|(a, b)| a.abs_diff(*b) > CHANNEL_TOLERANCE)
        })
        .count();
    let fraction = differing as f64 / f64::from(width * height);
    if fraction > DIFFERING_PIXEL_FRACTION {
        let out = root.join("../../../target/render-wgpu-screenshots");
        std::fs::create_dir_all(&out).unwrap();
        std::fs::write(out.join(format!("{name}.png")), encoded).unwrap();
        panic!(
            "{name}: {:.2}% of pixels differ (limit {:.2}%); actual written to {}",
            fraction * 100.0,
            DIFFERING_PIXEL_FRACTION * 100.0,
            out.display()
        );
    }
}

pub fn pixel(rgba: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let start = ((y * width + x) * 4) as usize;
    rgba[start..start + 4].try_into().unwrap()
}
