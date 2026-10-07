//! Harness for the scene family fixtures (`tests/scene.rs`): scenes applied
//! through `PresentationWorld`, rendered offscreen and compared with
//! reference PNGs in `tests/screenshots/`, with the same tolerance and
//! blessing as `tests/screenshots.rs`.

#![allow(dead_code, reason = "each test binary uses a subset")]

use std::{borrow::Cow, collections::HashMap, path::PathBuf};

use render_host_contracts::{
    RendererCameraPose, RendererCameraProjection, RendererCompositionCamera,
};
use render_model::*;
use render_presentation::PresentationWorld;
use render_wgpu::{
    decode_png_rgba, encode_png, FrameStats, Gpu, OffscreenTarget, Renderer, RendererOptions,
    ResourceSource,
};

pub const WIDTH: u32 = 320;
pub const HEIGHT: u32 = 180;
const CHANNEL_TOLERANCE: u8 = 12;
const DIFFERING_PIXEL_FRACTION: f64 = 0.002;

#[derive(Default)]
pub struct Resources(HashMap<String, Vec<u8>>);

impl ResourceSource for Resources {
    fn bytes(&self, identity: &str) -> Option<Cow<'_, [u8]>> {
        self.0
            .get(identity)
            .map(|bytes| Cow::Borrowed(bytes.as_slice()))
    }
}

impl Resources {
    /// Hold resource bytes under their runtime identity.
    pub fn insert(&mut self, identity: &str, bytes: Vec<u8>) {
        self.0.insert(identity.to_owned(), bytes);
    }

    /// Admit an RGBA8 image as a resource-backed texture, as the runtime does.
    pub fn texture(
        &mut self,
        id: &str,
        width: u32,
        height: u32,
        rgba: &[u8],
        wrap: TextureWrap,
    ) -> TextureDescriptor {
        let png = encode_png(width, height, rgba).expect("encode fixture texture");
        let texture = TextureDescriptor::admit_png_rgba8_resource(
            id.to_owned(),
            &png,
            TextureFilter::Nearest,
            wrap,
            1,
        )
        .expect("admit fixture texture");
        if let Some(TexturePayloadSource::Resource { resource }) =
            texture.payload.as_ref().map(|payload| &payload.source)
        {
            self.0.insert(resource.clone(), png);
        }
        texture
    }
}

pub struct Harness {
    pub gpu: Gpu,
    pub world: PresentationWorld,
    pub renderer: Renderer,
    pub target: OffscreenTarget,
    pub resources: Resources,
}

/// One device per test binary, as `common::gpu` does: the Vulkan loader can
/// crash when test threads create devices at the same time (seen on RADV in
/// the animated fixtures).
pub fn gpu() -> Gpu {
    static GPU: std::sync::OnceLock<Gpu> = std::sync::OnceLock::new();
    GPU.get_or_init(|| Gpu::headless().expect("screenshot tests need a wgpu adapter"))
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

    /// Apply ops through the retained model and hand the renderer its delta.
    /// Returns the delta the renderer received.
    pub fn apply(&mut self, ops: Vec<RenderDiff>) -> RenderFrameDiff {
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
        delta
    }

    pub fn render(&mut self, camera: &RendererCompositionCamera) -> (FrameStats, Vec<u8>) {
        let stats = self.renderer.render_offscreen(camera, &self.target);
        (stats, self.target.read_rgba(&self.gpu))
    }
}

pub fn camera(
    position: [f64; 3],
    yaw_degrees: f64,
    pitch_degrees: f64,
) -> RendererCompositionCamera {
    RendererCompositionCamera {
        id: "fixture-camera".to_owned(),
        pose: RendererCameraPose {
            position,
            pitch_degrees,
            yaw_degrees,
        },
        basis: None,
        projection: RendererCameraProjection::Perspective {
            fov_y_degrees: 60.0,
            near: 0.1,
            far: 100.0,
        },
        motion: None,
        viewmodel_fov_y_degrees: 0.0,
    }
}

pub fn transform(translation: [f32; 3], yaw_degrees: f32, scale: [f32; 3]) -> Transform {
    let half = yaw_degrees.to_radians() * 0.5;
    Transform {
        translation,
        rotation: [0.0, half.sin(), 0.0, half.cos()],
        scale,
    }
}

/// An inline triangle mesh with position, normal and uv, one group per
/// `(slot, index count)` in order.
pub fn payload(
    positions: Vec<f32>,
    normals: Vec<f32>,
    uvs: Vec<f32>,
    indices: Vec<u32>,
    groups: &[(u16, u32)],
) -> MeshPayloadDescriptor {
    let (mut min, mut max) = ([f32::MAX; 3], [f32::MIN; 3]);
    for point in positions.as_chunks::<3>().0 {
        for axis in 0..3 {
            min[axis] = min[axis].min(point[axis]);
            max[axis] = max[axis].max(point[axis]);
        }
    }
    let mut start = 0;
    let groups = groups
        .iter()
        .map(|(slot, count)| {
            let group = MeshGroupDescriptor {
                material_slot: *slot,
                start,
                count: *count,
            };
            start += count;
            group
        })
        .collect();
    MeshPayloadDescriptor {
        texture_space: None,
        distance_field: None,
        layout: MeshBufferLayout {
            vertex_count: (positions.len() / 3) as u32,
            index_count: indices.len() as u32,
            index_width: MeshIndexWidth::U32,
            attributes: [
                MeshAttributeName::Position,
                MeshAttributeName::Normal,
                MeshAttributeName::Uv,
            ]
            .into_iter()
            .map(|name| MeshAttribute {
                components: if name == MeshAttributeName::Uv { 2 } else { 3 },
                name,
                kind: MeshAttributeKind::F32,
            })
            .collect(),
        },
        groups,
        bounds: MeshBoundsDescriptor { min, max },
        source: MeshPayloadSource::Inline {
            positions,
            normals,
            uvs: Some(uvs),
            colors: None,
            indices,
        },
        provenance: MeshProvenance::Generated,
    }
}

/// An axis-aligned box from `min` to `max` with per-face normals; each face's
/// uv spans 0..1. Faces are grouped by `slots(face normal)`, in face order.
pub fn box_mesh(
    min: [f32; 3],
    max: [f32; 3],
    slot: impl Fn([f32; 3]) -> u16,
) -> MeshPayloadDescriptor {
    let (mut positions, mut normals, mut uvs, mut indices) = (vec![], vec![], vec![], vec![]);
    let mut groups: Vec<(u16, u32)> = Vec::new();
    let faces: [([f32; 3], [f32; 3], [f32; 3]); 6] = [
        ([1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]),
        ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
        ([0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, -1.0]),
        ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        ([0.0, 0.0, -1.0], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
    ];
    let center: [f32; 3] = std::array::from_fn(|axis| (min[axis] + max[axis]) * 0.5);
    let half: [f32; 3] = std::array::from_fn(|axis| (max[axis] - min[axis]) * 0.5);
    for (normal, u, v) in faces {
        let base = (positions.len() / 3) as u32;
        for (su, sv, uv) in [
            (-1.0, -1.0, [0.0, 1.0]),
            (1.0, -1.0, [1.0, 1.0]),
            (1.0, 1.0, [1.0, 0.0]),
            (-1.0, 1.0, [0.0, 0.0]),
        ] {
            for axis in 0..3 {
                positions
                    .push(center[axis] + (normal[axis] + u[axis] * su + v[axis] * sv) * half[axis]);
            }
            normals.extend_from_slice(&normal);
            uvs.extend_from_slice(&uv);
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        let face_slot = slot(normal);
        match groups.last_mut() {
            Some((last, count)) if *last == face_slot => *count += 6,
            _ => groups.push((face_slot, 6)),
        }
    }
    payload(positions, normals, uvs, indices, &groups)
}

pub fn material(id: &str, color: [f32; 4], texture: Option<&str>) -> RenderMaterialDescriptor {
    RenderMaterialDescriptor {
        texture_transform: None,
        stochastic_tiling: None,
        terrain_layers: None,
        shader: None,
        id: id.to_owned(),
        color,
        texture: texture.map(str::to_owned),
        roughness: 0.9,
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
        wind: None,
        water: None,
        translucent_shadow: false,
    }
}

pub fn static_mesh(asset: &str, payload: MeshPayloadDescriptor, material: &str) -> RenderDiff {
    RenderDiff::DefineStaticMesh {
        asset: StaticMeshAsset {
            asset: asset.to_owned(),
            payload,
            material_slots: vec![MeshMaterialSlot {
                slot: 0,
                material: material.to_owned(),
            }],
            collision: MeshCollisionPolicy::VisualOnly,
        },
    }
}

pub fn instance(handle: u64, parent: Option<u64>, asset: &str, transform: Transform) -> RenderDiff {
    RenderDiff::CreateStaticMeshInstance {
        handle: RenderHandle::new(handle),
        parent: parent.map(RenderHandle::new),
        instance: StaticMeshInstanceDescriptor {
            asset: asset.to_owned(),
            transform,
            visible: true,
            material_overrides: Vec::new(),
            metadata: RenderMetadata::default(),
            layer: RenderLayer::Scene,
            shadow_casting: Default::default(),
        },
    }
}

pub fn group(handle: u64, parent: Option<u64>, transform: Transform) -> RenderDiff {
    let mut node = RenderNode::new(Geometry::Group);
    node.transform = transform;
    RenderDiff::Create {
        handle: RenderHandle::new(handle),
        parent: parent.map(RenderHandle::new),
        node,
    }
}

pub fn checker(size: u32, a: [u8; 4], b: [u8; 4]) -> Vec<u8> {
    (0..size * size)
        .flat_map(|index| {
            if (index % size + index / size).is_multiple_of(2) {
                a
            } else {
                b
            }
        })
        .collect()
}

/// Compare with the reference, or write it when blessing.
pub fn assert_screenshot(name: &str, rgba: &[u8]) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let reference = root.join("tests/screenshots").join(format!("{name}.png"));
    let encoded = encode_png(WIDTH, HEIGHT, rgba).expect("encode screenshot");
    if std::env::var_os("RENDER_WGPU_BLESS").is_some() {
        std::fs::create_dir_all(reference.parent().unwrap()).unwrap();
        std::fs::write(&reference, encoded).unwrap();
        return;
    }
    let expected = std::fs::read(&reference).unwrap_or_else(|_| {
        panic!(
            "missing {}; run with RENDER_WGPU_BLESS=1",
            reference.display()
        )
    });
    let (width, height, expected) = decode_png_rgba(&expected).expect("decode reference");
    assert_eq!((width, height), (WIDTH, HEIGHT), "{name}: reference size");
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
    let fraction = differing as f64 / f64::from(WIDTH * HEIGHT);
    if fraction > DIFFERING_PIXEL_FRACTION {
        let out = root.join("../../../target/render-wgpu-screenshots");
        std::fs::create_dir_all(&out).unwrap();
        std::fs::write(out.join(format!("{name}.png")), encoded).unwrap();
        panic!(
            "{name}: {:.2}% of pixels differ from the reference (limit {:.2}%); actual written to {}",
            fraction * 100.0,
            DIFFERING_PIXEL_FRACTION * 100.0,
            out.display()
        );
    }
}
