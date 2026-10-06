//! Screenshot fixtures: small retained scenes applied through
//! `PresentationWorld`, rendered offscreen and compared with reference PNGs in
//! `tests/screenshots/`.
//!
//! Tolerance: a pixel differs when any channel differs by more than
//! `CHANNEL_TOLERANCE`; a scene passes when at most `DIFFERING_PIXEL_FRACTION`
//! of its pixels differ. That absorbs rasterization and filtering differences
//! between adapters (llvmpipe in CI, hardware locally) without hiding a wrong
//! transform, material, light or sky.
//!
//! `RENDER_WGPU_BLESS=1 cargo test -p render-wgpu --test screenshots` rewrites
//! the references; failures write `target/render-wgpu-screenshots/<name>.png`.
//! Needs a wgpu adapter: CI uses `WGPU_BACKEND=vulkan` with llvmpipe.

use std::{borrow::Cow, collections::HashMap, path::PathBuf};

use render_host_contracts::{
    RendererCameraPose, RendererCameraProjection, RendererCompositionCamera,
};
use render_model::*;
use render_presentation::PresentationWorld;
use render_wgpu::{
    decode_png_rgba, encode_png, Gpu, OffscreenTarget, Renderer, RendererOptions, ResourceSource,
};

const WIDTH: u32 = 320;
const HEIGHT: u32 = 180;
const CHANNEL_TOLERANCE: u8 = 12;
const DIFFERING_PIXEL_FRACTION: f64 = 0.002;
/// Scenes that draw lines: Vulkan lets implementations rasterize
/// multisampled lines differently (llvmpipe and RADV disagree along them),
/// so these compare with a wider limit. Triangles still meet the default.
const LINE_SCENE_PIXEL_FRACTION: f64 = 0.01;

#[derive(Default)]
struct Resources(HashMap<String, Vec<u8>>);

impl ResourceSource for Resources {
    fn bytes(&self, identity: &str) -> Option<Cow<'_, [u8]>> {
        self.0
            .get(identity)
            .map(|bytes| Cow::Borrowed(bytes.as_slice()))
    }
}

impl Resources {
    /// Admit an RGBA8 image as a resource-backed texture, as the runtime does.
    fn texture(
        &mut self,
        id: &str,
        width: u32,
        height: u32,
        rgba: &[u8],
        wrap: TextureWrap,
    ) -> RenderDiff {
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
        RenderDiff::DefineTexture { texture }
    }
}

struct Harness {
    gpu: Gpu,
    world: PresentationWorld,
    renderer: Renderer,
    target: OffscreenTarget,
    resources: Resources,
}

impl Harness {
    fn new(options: RendererOptions) -> Self {
        // One device per test binary: parallel device creation crashes the
        // Vulkan loader (`vkSetDebugUtilsObjectNameEXT`, seen on RADV).
        static GPU: std::sync::OnceLock<Gpu> = std::sync::OnceLock::new();
        let gpu = GPU
            .get_or_init(|| Gpu::headless().expect("screenshot tests need a wgpu adapter"))
            .clone();
        Self {
            renderer: Renderer::new(&gpu, options),
            target: OffscreenTarget::new(&gpu, WIDTH, HEIGHT, 4),
            world: PresentationWorld::default(),
            resources: Resources::default(),
            gpu,
        }
    }

    /// As [`Self::apply`], returning the ops the renderer reported.
    fn apply_reporting(&mut self, ops: Vec<RenderDiff>) -> Vec<render_wgpu::ApplyIssue> {
        let delta = self
            .world
            .apply(RenderFrameDiff {
                publication: None,
                ops,
            })
            .expect("retained model admits the fixture");
        self.renderer.apply(&delta, &self.resources)
    }

    /// Apply ops through the retained model and hand the renderer its delta.
    fn apply(&mut self, ops: Vec<RenderDiff>) {
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

    fn render(&mut self, camera: &RendererCompositionCamera) -> (render_wgpu::FrameStats, Vec<u8>) {
        let stats = self.renderer.render_offscreen(camera, &self.target);
        (stats, self.target.read_rgba(&self.gpu))
    }
}

fn camera(position: [f64; 3], yaw_degrees: f64, pitch_degrees: f64) -> RendererCompositionCamera {
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

fn transform(translation: [f32; 3], yaw_degrees: f32, scale: [f32; 3]) -> Transform {
    let half = yaw_degrees.to_radians() * 0.5;
    Transform {
        translation,
        rotation: [0.0, half.sin(), 0.0, half.cos()],
        scale,
    }
}

/// An inline, single-group triangle mesh with position, normal and uv.
fn payload(
    positions: Vec<f32>,
    normals: Vec<f32>,
    uvs: Vec<f32>,
    indices: Vec<u32>,
) -> MeshPayloadDescriptor {
    let (mut min, mut max) = ([f32::MAX; 3], [f32::MIN; 3]);
    for point in positions.as_chunks::<3>().0 {
        for axis in 0..3 {
            min[axis] = min[axis].min(point[axis]);
            max[axis] = max[axis].max(point[axis]);
        }
    }
    MeshPayloadDescriptor {
        texture_space: None,
        distance_field: None,
        layout: MeshBufferLayout {
            vertex_count: (positions.len() / 3) as u32,
            index_count: indices.len() as u32,
            index_width: MeshIndexWidth::U32,
            attributes: vec![
                MeshAttribute {
                    name: MeshAttributeName::Position,
                    components: 3,
                    kind: MeshAttributeKind::F32,
                },
                MeshAttribute {
                    name: MeshAttributeName::Normal,
                    components: 3,
                    kind: MeshAttributeKind::F32,
                },
                MeshAttribute {
                    name: MeshAttributeName::Uv,
                    components: 2,
                    kind: MeshAttributeKind::F32,
                },
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
            uvs: Some(uvs),
            colors: None,
            indices,
        },
        provenance: MeshProvenance::Generated,
    }
}

/// A floor plane of side `size` at y = 0, uv repeating `repeat` times.
fn floor(size: f32, repeat: f32) -> MeshPayloadDescriptor {
    let h = size * 0.5;
    payload(
        vec![-h, 0.0, h, h, 0.0, h, h, 0.0, -h, -h, 0.0, -h],
        [0.0, 1.0, 0.0].repeat(4),
        vec![0.0, repeat, repeat, repeat, repeat, 0.0, 0.0, 0.0],
        vec![0, 1, 2, 0, 2, 3],
    )
}

/// A unit box centred on the origin with per-face normals.
fn cube() -> MeshPayloadDescriptor {
    let (mut positions, mut normals, mut uvs, mut indices) = (vec![], vec![], vec![], vec![]);
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
        for (su, sv, uv) in [
            (-1.0, -1.0, [0.0, 1.0]),
            (1.0, -1.0, [1.0, 1.0]),
            (1.0, 1.0, [1.0, 0.0]),
            (-1.0, 1.0, [0.0, 0.0]),
        ] {
            for axis in 0..3 {
                positions.push((normal[axis] + u[axis] * su + v[axis] * sv) * 0.5);
            }
            normals.extend_from_slice(&normal);
            uvs.extend_from_slice(&uv);
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    payload(positions, normals, uvs, indices)
}

fn material(id: &str, color: [f32; 4], texture: Option<&str>) -> RenderMaterialDescriptor {
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
    }
}

fn static_mesh(asset: &str, payload: MeshPayloadDescriptor, material: &str) -> RenderDiff {
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

fn instance(handle: u64, parent: Option<u64>, asset: &str, transform: Transform) -> RenderDiff {
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

fn primitive(handle: u64, geometry: Geometry, color: [f32; 4], transform: Transform) -> RenderDiff {
    let mut node = RenderNode::new(geometry);
    node.material = Material {
        color,
        wireframe: false,
    };
    node.transform = transform;
    RenderDiff::Create {
        handle: RenderHandle::new(handle),
        parent: None,
        node,
    }
}

fn checker(size: u32, a: [u8; 4], b: [u8; 4]) -> Vec<u8> {
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
fn assert_screenshot(name: &str, rgba: &[u8]) {
    assert_screenshot_within(name, rgba, DIFFERING_PIXEL_FRACTION);
}

fn assert_screenshot_within(name: &str, rgba: &[u8], limit: f64) {
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
    if fraction > limit {
        let out = root.join("../../../target/render-wgpu-screenshots");
        std::fs::create_dir_all(&out).unwrap();
        std::fs::write(out.join(format!("{name}.png")), encoded).unwrap();
        panic!(
            "{name}: {:.2}% of pixels differ from the reference (limit {:.2}%); actual written to {}",
            fraction * 100.0,
            limit * 100.0,
            out.display()
        );
    }
}

#[test]
fn primitives_draw_unlit_over_the_background_colour() {
    let mut harness = Harness::new(RendererOptions::default());
    harness.apply(vec![
        RenderDiff::SetBackgroundColor {
            color: [0.02, 0.05, 0.12, 1.0],
        },
        primitive(
            1,
            Geometry::Cube,
            [0.9, 0.1, 0.1, 1.0],
            transform([-1.4, 0.0, -4.0], 35.0, [1.0; 3]),
        ),
        primitive(
            2,
            Geometry::Sphere,
            [0.1, 0.8, 0.2, 1.0],
            transform([0.0, 0.0, -4.0], 0.0, [1.0; 3]),
        ),
        primitive(
            3,
            Geometry::Quad,
            [0.2, 0.3, 1.0, 0.5],
            transform([1.4, 0.0, -4.0], -20.0, [1.0; 3]),
        ),
        primitive(
            4,
            Geometry::Line {
                a: [-2.0, -1.0, -4.0],
                b: [2.0, -1.0, -4.0],
            },
            [1.0, 0.9, 0.1, 1.0],
            Transform::IDENTITY,
        ),
        primitive(
            5,
            Geometry::Point,
            [1.0, 1.0, 1.0, 1.0],
            transform([0.0, 1.0, -4.0], 0.0, [1.0; 3]),
        ),
    ]);
    let (stats, pixels) = harness.render(&camera([0.0, 0.0, 0.0], 0.0, 0.0));
    assert_eq!(stats.draws, 5);
    assert_screenshot_within("primitives", &pixels, LINE_SCENE_PIXEL_FRACTION);
}

#[test]
fn textured_static_meshes_take_the_neutral_rig_and_instance_parameters() {
    let mut harness = Harness::new(RendererOptions::default());
    let checker = checker(8, [200, 200, 200, 255], [60, 60, 70, 255]);
    let texture = harness
        .resources
        .texture("texture/checker", 8, 8, &checker, TextureWrap::Repeat);
    let mut glowing = material("material/crate", [0.8, 0.6, 0.4, 1.0], None);
    glowing.roughness = 0.5;
    harness.apply(vec![
        texture,
        RenderDiff::DefineMaterial {
            material: material("material/floor", [1.0; 4], Some("texture/checker")),
        },
        RenderDiff::DefineMaterial { material: glowing },
        static_mesh("mesh/floor", floor(12.0, 6.0), "material/floor"),
        static_mesh("mesh/crate", cube(), "material/crate"),
        instance(10, None, "mesh/floor", Transform::IDENTITY),
        instance(
            11,
            None,
            "mesh/crate",
            transform([-1.2, 0.5, -1.0], 20.0, [1.0; 3]),
        ),
        instance(
            12,
            None,
            "mesh/crate",
            transform([1.2, 0.5, -1.5], -30.0, [1.0; 3]),
        ),
        RenderDiff::SetMaterialInstanceParameters {
            handle: RenderHandle::new(12),
            slot: 0,
            parameters: Some(MaterialInstanceParameters {
                base_color: None,
                texture_tint: [0.3, 0.3, 1.0, 1.0],
                emission: Some(MaterialInstanceEmission {
                    color: [0.1, 0.1, 0.6],
                    intensity: 1.0,
                }),
            }),
        },
    ]);
    let (stats, pixels) = harness.render(&camera([0.0, 1.8, 3.5], 0.0, -20.0));
    // Both crates share mesh and material: one instanced draw, with the
    // instance parameters in the second crate's row.
    assert_eq!((stats.draws, stats.instances, stats.lights), (2, 3, 2));
    assert_screenshot("lit-textured", &pixels);
}

#[test]
fn a_torch_lights_a_dark_room_when_the_default_rig_is_disabled() {
    let mut harness = Harness::new(RendererOptions {
        default_world_lights: false,
        ..RendererOptions::default()
    });
    harness.apply(vec![
        RenderDiff::DefineMaterial {
            material: material("material/stone", [0.7, 0.7, 0.7, 1.0], None),
        },
        static_mesh("mesh/floor", floor(10.0, 1.0), "material/stone"),
        static_mesh("mesh/wall", cube(), "material/stone"),
        instance(20, None, "mesh/floor", Transform::IDENTITY),
        instance(
            21,
            None,
            "mesh/wall",
            transform([0.0, 1.5, -3.0], 0.0, [8.0, 3.0, 0.4]),
        ),
        instance(
            22,
            None,
            "mesh/wall",
            transform([-3.0, 1.5, 0.0], 90.0, [8.0, 3.0, 0.4]),
        ),
        RenderDiff::CreateLight {
            handle: RenderHandle::new(23),
            parent: None,
            light: LightDescriptor::Point {
                color: [1.0, 0.6, 0.25],
                intensity: 6.0,
                enabled: true,
                position: [-1.5, 1.0, -1.8],
                range: Some(6.0),
                decay: 2.0,
                shadow_intent: LightShadowIntent::Disabled,
                shadow: Default::default(),
            },
        },
        RenderDiff::CreateLight {
            handle: RenderHandle::new(24),
            parent: None,
            light: LightDescriptor::Spot {
                color: [0.3, 0.5, 1.0],
                intensity: 20.0,
                enabled: true,
                position: [2.0, 3.0, 0.5],
                direction: [-0.3, -1.0, -0.4],
                range: None,
                decay: 2.0,
                outer_angle_radians: 0.5,
                penumbra: 0.3,
                shadow_intent: LightShadowIntent::Disabled,
                shadow: Default::default(),
            },
        },
        RenderDiff::CreateLight {
            handle: RenderHandle::new(25),
            parent: None,
            light: LightDescriptor::Ambient {
                color: [1.0, 1.0, 1.0],
                intensity: 0.05,
                enabled: true,
                shadow_intent: LightShadowIntent::Disabled,
                shadow: Default::default(),
                range: None,
            },
        },
    ]);
    let (stats, pixels) = harness.render(&camera([1.5, 1.6, 3.0], -20.0, -15.0));
    assert_eq!(stats.lights, 3);
    assert_screenshot("torch", &pixels);
}

#[test]
fn hierarchy_changes_upload_only_the_changed_subtree() {
    let mut harness = Harness::new(RendererOptions::default());
    harness.apply(vec![
        RenderDiff::DefineMaterial {
            material: material("material/red", [0.8, 0.2, 0.2, 1.0], None),
        },
        RenderDiff::DefineMaterial {
            material: material("material/blue", [0.2, 0.3, 0.8, 1.0], None),
        },
        static_mesh("mesh/red", cube(), "material/red"),
        static_mesh("mesh/blue", cube(), "material/blue"),
        primitive(
            30,
            Geometry::Group,
            [1.0; 4],
            transform([0.0, 0.0, -5.0], 30.0, [1.0; 3]),
        ),
        instance(
            31,
            Some(30),
            "mesh/red",
            transform([-1.5, 0.0, 0.0], 0.0, [1.0; 3]),
        ),
        instance(
            32,
            Some(30),
            "mesh/blue",
            transform([1.5, 0.0, 0.0], 0.0, [1.0; 3]),
        ),
        primitive(
            33,
            Geometry::Group,
            [1.0; 4],
            transform([0.0, 1.8, -5.0], 0.0, [1.0; 3]),
        ),
        instance(
            34,
            Some(33),
            "mesh/red",
            transform([0.0, 0.0, 0.0], 45.0, [0.6; 3]),
        ),
        instance(
            35,
            Some(34),
            "mesh/blue",
            transform([0.0, 1.0, 0.0], 0.0, [0.5; 3]),
        ),
    ]);
    let view = camera([0.0, 0.5, 0.0], 0.0, 0.0);
    let (first, _) = harness.render(&view);
    // Red and blue instances batch by mesh and material.
    assert_eq!(
        (first.draws, first.instances, first.parts_uploaded),
        (2, 4, 4)
    );

    harness.apply(vec![RenderDiff::Update {
        handle: RenderHandle::new(30),
        transform: Some(transform([0.0, -0.5, -5.0], 75.0, [1.0; 3])),
        material: None,
        visible: None,
        metadata: None,
    }]);
    let (moved, _) = harness.render(&view);
    // Only the parent's two children are re-uploaded.
    assert_eq!(
        (moved.draws, moved.instances, moved.parts_uploaded),
        (2, 4, 2)
    );

    harness.apply(vec![
        RenderDiff::Update {
            handle: RenderHandle::new(32),
            transform: None,
            material: None,
            visible: Some(false),
            metadata: None,
        },
        RenderDiff::Destroy {
            handle: RenderHandle::new(33),
        },
    ]);
    let (after, pixels) = harness.render(&view);
    assert_eq!(
        (after.draws, after.instances),
        (1, 1),
        "hidden child and destroyed subtree are not drawn"
    );
    let counts = harness.renderer.table_counts();
    assert_eq!(
        (counts.nodes, counts.parts),
        (3, 2),
        "destroy removes the whole subtree"
    );
    assert_screenshot("hierarchy", &pixels);
}

#[test]
fn equirectangular_sky_blends_two_panoramas() {
    let mut harness = Harness::new(RendererOptions::default());
    let (width, height) = (64, 32);
    let day: Vec<u8> = (0..width * height)
        .flat_map(|index| {
            let (u, v) = (index % width, index / width);
            if (44..52).contains(&u) {
                [220, 40, 40, 255]
            } else if v < height / 2 {
                [90, 150, 230, 255]
            } else {
                [110, 80, 50, 255]
            }
        })
        .collect();
    let night = checker(8, [10, 10, 40, 255], [30, 30, 80, 255]);
    let day = harness
        .resources
        .texture("texture/day", width, height, &day, TextureWrap::Clamp);
    let night = harness
        .resources
        .texture("texture/night", 8, 8, &night, TextureWrap::Repeat);
    harness.apply(vec![
        day,
        night,
        RenderDiff::SetSkyBackground {
            background: Some(SkyBackgroundDescriptor {
                texture: "texture/day".to_owned(),
                blend: Some(SkyBackgroundBlend {
                    texture: "texture/night".to_owned(),
                    amount: 0.25,
                }),
            }),
        },
    ]);
    // Yaw 180 faces +Z, where the marker stripe (u ≈ 0.75) sits.
    let (_, pixels) = harness.render(&camera([0.0, 0.0, 0.0], 180.0, 10.0));
    assert_screenshot("sky-blend", &pixels);
}

#[test]
fn resize_changes_the_readback_size() {
    let mut harness = Harness::new(RendererOptions::default());
    harness.apply(vec![RenderDiff::SetBackgroundColor {
        color: [1.0, 0.0, 0.0, 1.0],
    }]);
    harness.target.resize(&harness.gpu, 64, 48, 4);
    let (_, pixels) = harness.render(&camera([0.0; 3], 0.0, 0.0));
    assert_eq!(pixels.len(), 64 * 48 * 4);
    assert_eq!(
        &pixels[..4],
        &[255, 0, 0, 255],
        "linear red encodes to sRGB red"
    );
}

/// Blended parts draw back to front across every blend pipeline: switching
/// the farther panel to double-sided must not move it in front (#8783 review).
#[test]
fn transparent_parts_sort_by_distance_whatever_their_face_culling() {
    let capture = |far_double_sided: bool| {
        let mut harness = Harness::new(RendererOptions::default());
        let blended = |id: &str, color: [f32; 4], double_sided: bool| {
            let mut descriptor = material(id, color, None);
            descriptor.alpha_mode = MaterialAlphaModeDescriptor::Blend;
            descriptor.double_sided = double_sided;
            descriptor.emission_color = [color[0], color[1], color[2]];
            descriptor.emission_intensity = 0.5;
            RenderDiff::DefineMaterial {
                material: descriptor,
            }
        };
        let quad = || {
            payload(
                vec![-1., -1., 0., 1., -1., 0., 1., 1., 0., -1., 1., 0.],
                [0., 0., 1.].repeat(4),
                vec![0., 0., 1., 0., 1., 1., 0., 1.],
                vec![0, 1, 2, 0, 2, 3],
            )
        };
        harness.apply(vec![
            blended("material/far", [0.0, 0.0, 1.0, 0.5], far_double_sided),
            blended("material/near", [1.0, 0.0, 0.0, 0.5], false),
            static_mesh("mesh/far", quad(), "material/far"),
            static_mesh("mesh/near", quad(), "material/near"),
            instance(
                1,
                None,
                "mesh/far",
                transform([0.0, 0.0, 0.0], 0.0, [1.0; 3]),
            ),
            instance(
                2,
                None,
                "mesh/near",
                transform([0.0, 0.0, 1.0], 0.0, [1.0; 3]),
            ),
        ]);
        let (_, pixels) = harness.render(&camera([0.0, 0.0, 5.0], 0.0, 0.0));
        let center = ((HEIGHT / 2 * WIDTH + WIDTH / 2) * 4) as usize;
        [pixels[center], pixels[center + 1], pixels[center + 2]]
    };
    let single_sided = capture(false);
    let double_sided = capture(true);
    assert!(
        single_sided[0] > single_sided[2],
        "the nearer red panel blends over the farther blue one: {single_sided:?}"
    );
    assert_eq!(
        single_sided, double_sided,
        "face culling changed the blend order"
    );
}

/// The primary target is multisampled: a black unlit cube on white leaves
/// partially covered pixels along its edges. Single-sampled, every pixel would be exactly black or white.
#[test]
fn primary_targets_antialias_triangle_edges() {
    let mut harness = Harness::new(RendererOptions::default());
    harness.apply(vec![
        RenderDiff::SetBackgroundColor {
            color: [1.0, 1.0, 1.0, 1.0],
        },
        primitive(
            1,
            Geometry::Cube,
            [0.0, 0.0, 0.0, 1.0],
            transform([0.0, 0.0, -3.0], 30.0, [1.0; 3]),
        ),
    ]);
    let (_, pixels) = harness.render(&camera([0.0, 0.0, 0.0], 0.0, 0.0));
    let partial = pixels
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|pixel| pixel[0] > 8 && pixel[0] < 247)
        .count();
    assert!(
        partial > 100,
        "only {partial} partially covered edge pixels"
    );
}

/// `Material.wireframe` draws a primitive's triangle edges: the cube is
/// outlined, not filled.
#[test]
fn wireframe_primitives_draw_their_triangle_edges() {
    let lit_pixels = |wireframe: bool| {
        let mut harness = Harness::new(RendererOptions::default());
        let mut cube = RenderNode::new(Geometry::Cube);
        cube.material = Material {
            color: [1.0, 1.0, 1.0, 1.0],
            wireframe,
        };
        cube.transform = transform([0.0, 0.0, -3.0], 30.0, [1.0; 3]);
        harness.apply(vec![
            RenderDiff::SetBackgroundColor {
                color: [0.0, 0.0, 0.0, 1.0],
            },
            RenderDiff::Create {
                handle: RenderHandle::new(1),
                parent: None,
                node: cube,
            },
        ]);
        let (_, pixels) = harness.render(&camera([0.0, 0.0, 0.0], 0.0, 15.0));
        let lit = pixels
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|pixel| pixel[0] > 128)
            .count();
        (lit, pixels)
    };
    let (solid, _) = lit_pixels(false);
    let (outline, pixels) = lit_pixels(true);
    assert!(
        outline > 0 && outline * 2 < solid,
        "wireframe lit {outline} pixels, solid {solid}"
    );
    assert_screenshot_within("wireframe", &pixels, LINE_SCENE_PIXEL_FRACTION);
}

/// A view material on a static mesh is reported: no Engine producer sends
/// one (appearance changes recreate the instance). The rest of the update
/// applies.
#[test]
fn view_materials_on_static_meshes_are_reported_and_the_rest_applies() {
    let mut harness = Harness::new(RendererOptions::default());
    harness.apply(vec![
        RenderDiff::DefineMaterial {
            material: material("material/red", [0.8, 0.2, 0.2, 1.0], None),
        },
        static_mesh("mesh/red", cube(), "material/red"),
        instance(
            1,
            None,
            "mesh/red",
            transform([0.0, 0.0, -3.0], 0.0, [1.0; 3]),
        ),
    ]);
    let issues = harness.apply_reporting(vec![RenderDiff::Update {
        handle: RenderHandle::new(1),
        transform: None,
        material: Some(Material {
            color: [0.0, 1.0, 0.0, 1.0],
            wireframe: false,
        }),
        visible: Some(false),
        metadata: None,
    }]);
    assert_eq!(issues.len(), 1, "{issues:?}");
    assert_eq!(issues[0].op, "update");
    let (stats, _) = harness.render(&camera([0.0, 0.0, 0.0], 0.0, 0.0));
    assert_eq!(stats.draws, 0, "the visibility in the same update applied");
}
