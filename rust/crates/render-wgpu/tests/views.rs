//! View fixtures: camera composition (split primary views, an offscreen target
//! presented as an inset), the viewmodel pass, camera motion on the host
//! clock, and output captures. Scenes go through `PresentationWorld` as the
//! runtime applies them; references live in `tests/screenshots/` and use the
//! tolerance `screenshots.rs` documents.
//!
//! `RENDER_WGPU_BLESS=1 cargo test -p render-wgpu --test views` rewrites the
//! references. Needs a wgpu adapter: CI uses `WGPU_BACKEND=vulkan` with
//! llvmpipe.

use std::path::PathBuf;

use render_host_contracts::{
    RenderOutputJob, RenderOutputOperation, RenderOutputPose, RendererCameraInterpolation,
    RendererCameraMotion, RendererCameraPose, RendererCameraProjection, RendererCompositionCamera,
    RendererCompositionPresentation, RendererCompositionTarget, RendererCompositionView,
    RendererPrimaryDestination, RendererPrimaryDestinationKind, RendererTargetColor,
    RendererTargetDepth, RendererTargetSampling, RendererViewComposition, RendererViewTarget,
    RendererViewport, RENDERER_VIEW_COMPOSITION_SCHEMA_VERSION,
};
use render_model::*;
use render_presentation::PresentationWorld;
use render_wgpu::{
    decode_png_rgba, encode_png, Gpu, NoResources, OffscreenTarget, Renderer, RendererOptions,
    TargetStatus,
};

const WIDTH: u32 = 320;
const HEIGHT: u32 = 180;
const CHANNEL_TOLERANCE: u8 = 12;
const DIFFERING_PIXEL_FRACTION: f64 = 0.002;

struct Harness {
    gpu: Gpu,
    world: PresentationWorld,
    renderer: Renderer,
    target: OffscreenTarget,
}

impl Harness {
    fn new(options: RendererOptions) -> Self {
        let gpu = Gpu::headless().expect("view tests need a wgpu adapter");
        Self {
            renderer: Renderer::new(&gpu, options),
            target: OffscreenTarget::new(&gpu, WIDTH, HEIGHT),
            world: PresentationWorld::default(),
            gpu,
        }
    }

    fn apply(&mut self, ops: Vec<RenderDiff>) {
        let delta = self
            .world
            .apply(RenderFrameDiff {
                schema_version: RENDER_FRAME_SCHEMA_VERSION,
                publication: None,
                ops,
            })
            .expect("retained model admits the fixture");
        let issues = self.renderer.apply(&delta, &NoResources);
        assert!(
            issues.is_empty(),
            "renderer skipped fixture ops: {issues:?}"
        );
    }

    fn composition(&mut self, time: f64) -> (render_wgpu::FrameStats, Vec<u8>) {
        let stats = self.renderer.render_view_composition(&self.target, time);
        (stats, self.target.read_rgba(&self.gpu))
    }

    fn single(&mut self, camera: &RendererCompositionCamera) -> Vec<u8> {
        self.renderer.render_offscreen(camera, &self.target);
        self.target.read_rgba(&self.gpu)
    }
}

fn camera(id: &str, position: [f64; 3], yaw: f64, pitch: f64) -> RendererCompositionCamera {
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
    }
}

fn viewport(x: f64, y: f64, width: f64, height: f64) -> RendererViewport {
    RendererViewport {
        x,
        y,
        width,
        height,
    }
}

fn primary_view(
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
    }
}

fn composition(
    cameras: Vec<RendererCompositionCamera>,
    views: Vec<RendererCompositionView>,
) -> RendererViewComposition {
    RendererViewComposition {
        schema_version: RENDERER_VIEW_COMPOSITION_SCHEMA_VERSION,
        cameras,
        targets: Vec::new(),
        views,
        presentations: Vec::new(),
    }
}

fn transform(translation: [f32; 3], yaw_degrees: f32, scale: f32) -> Transform {
    let half = yaw_degrees.to_radians() * 0.5;
    Transform {
        translation,
        rotation: [0.0, half.sin(), 0.0, half.cos()],
        scale: [scale; 3],
    }
}

fn payload(positions: Vec<f32>, normals: Vec<f32>, indices: Vec<u32>) -> MeshPayloadDescriptor {
    let (mut min, mut max) = ([f32::MAX; 3], [f32::MIN; 3]);
    for point in positions.chunks_exact(3) {
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
    }
}

/// A unit box centred on the origin with per-face normals.
fn cube() -> MeshPayloadDescriptor {
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

fn floor(size: f32) -> MeshPayloadDescriptor {
    let h = size * 0.5;
    payload(
        vec![-h, 0.0, h, h, 0.0, h, h, 0.0, -h, -h, 0.0, -h],
        [0.0, 1.0, 0.0].repeat(4),
        vec![0, 1, 2, 0, 2, 3],
    )
}

/// A lit material and a static mesh drawing `payload` with it.
fn coloured_mesh(name: &str, payload: MeshPayloadDescriptor, color: [f32; 4]) -> Vec<RenderDiff> {
    let material = format!("material/{name}");
    vec![
        RenderDiff::DefineMaterial {
            material: RenderMaterialDescriptor {
                schema_version: 1,
                id: material.clone(),
                color,
                texture: None,
                roughness: 0.8,
                texture_tint: [1.0; 4],
                emission_color: [0.0; 3],
                emission_intensity: 0.0,
                uv_strategy: MaterialUvStrategy::Flat,
                alpha_mode: MaterialAlphaModeDescriptor::Opaque,
                double_sided: false,
                voxel_surface: None,
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

fn instance(handle: u64, parent: Option<u64>, mesh: &str, transform: Transform) -> RenderDiff {
    RenderDiff::CreateStaticMeshInstance {
        handle: RenderHandle::new(handle),
        parent: parent.map(RenderHandle::new),
        instance: StaticMeshInstanceDescriptor {
            asset: format!("mesh/{mesh}"),
            transform,
            visible: true,
            material_overrides: Vec::new(),
            metadata: RenderMetadata::default(),
        },
    }
}

/// A floor and three coloured boxes around (0, 0, -4).
fn room() -> Vec<RenderDiff> {
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

fn assert_screenshot(name: &str, width: u32, height: u32, rgba: &[u8]) {
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
        .chunks_exact(4)
        .zip(rgba.chunks_exact(4))
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

fn pixel(rgba: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let start = ((y * width + x) * 4) as usize;
    rgba[start..start + 4].try_into().unwrap()
}

#[test]
fn composed_views_split_the_primary_and_present_an_offscreen_target() {
    let mut harness = Harness::new(RendererOptions::default());
    harness.apply(room());
    let mut top = camera("top", [0.0, 7.0, -4.3], 0.0, -90.0);
    top.projection = RendererCameraProjection::Orthographic {
        vertical_size: 6.0,
        near: 0.1,
        far: 20.0,
    };
    let mut view = composition(
        vec![
            camera("front", [0.0, 0.6, 1.0], 0.0, -6.0),
            camera("side", [4.5, 0.6, -4.2], -90.0, -6.0),
            top,
        ],
        vec![
            primary_view("left", "front", viewport(0.0, 0.0, 0.5, 1.0), 0),
            primary_view("right", "side", viewport(0.5, 0.0, 0.5, 1.0), 1),
            RendererCompositionView {
                id: "map".to_owned(),
                camera_id: "top".to_owned(),
                target: RendererViewTarget::Offscreen {
                    target_id: "minimap".to_owned(),
                    target_revision: 1,
                },
                viewport: viewport(0.0, 0.0, 1.0, 1.0),
                order: 0,
            },
        ],
    );
    view.targets.push(RendererCompositionTarget {
        id: "minimap".to_owned(),
        revision: 1,
        width: 64,
        height: 64,
        color: RendererTargetColor::Rgba8Srgb,
        depth: RendererTargetDepth::Depth24,
        sampling: RendererTargetSampling::Nearest,
    });
    view.presentations.push(RendererCompositionPresentation {
        id: "inset".to_owned(),
        source_target_id: "minimap".to_owned(),
        source_target_revision: 1,
        // Upper right: the viewport is bottom-left based.
        destination: RendererPrimaryDestination {
            kind: RendererPrimaryDestinationKind::Primary,
            viewport: viewport(0.78, 0.6, 0.2, 0.36),
        },
        order: 2,
    });
    harness.renderer.set_view_composition(&view, 0.0);
    let (stats, pixels) = harness.composition(0.0);
    assert_eq!(stats.offscreen_views, 1, "a new target draws once");
    assert_screenshot("composition", WIDTH, HEIGHT, &pixels);

    // Nothing changed: the target is presented as drawn.
    let readout = harness.renderer.view_composition_readout();
    assert_eq!(readout.targets[0].status, TargetStatus::Current);
    let (stats, again) = harness.composition(1.0);
    assert_eq!(stats.offscreen_views, 0, "a current target is not redrawn");
    assert_eq!(again, pixels);

    // A delta makes it stale, and the next frame redraws it.
    harness.apply(vec![RenderDiff::SetBackgroundColor {
        color: [0.3, 0.05, 0.05, 1.0],
    }]);
    let readout = harness.renderer.view_composition_readout();
    assert_eq!(readout.targets[0].status, TargetStatus::Stale);
    let (stats, _) = harness.composition(2.0);
    assert_eq!(stats.offscreen_views, 1);
    assert_eq!(
        harness.renderer.view_composition_readout().targets[0].last_refreshed_frame,
        Some(3)
    );
}

#[test]
fn a_frame_without_a_primary_view_is_only_the_clear() {
    let mut harness = Harness::new(RendererOptions::default());
    harness.apply(room());
    harness
        .renderer
        .set_view_composition(&composition(Vec::new(), Vec::new()), 0.0);
    let (stats, pixels) = harness.composition(0.0);
    assert_eq!(stats.draws, 0, "there is no fallback world pass");
    let background = pixel(&pixels, WIDTH, WIDTH / 2, HEIGHT / 2);
    assert!(pixels.chunks_exact(4).all(|p| p == background));
}

#[test]
fn the_viewmodel_draws_over_the_world_after_a_depth_break() {
    let mut harness = Harness::new(RendererOptions::default());
    let mut ops = room();
    // A wall right in front of the camera, nearer than the viewmodel box
    // would be in world space.
    ops.push(instance(
        20,
        None,
        "red",
        transform([0.0, 0.0, -1.3], 0.0, 1.5),
    ));
    let mut root = RenderNode::new(Geometry::Group);
    root.layer = RenderLayer::Viewmodel;
    ops.push(RenderDiff::Create {
        handle: RenderHandle::new(30),
        parent: None,
        node: root,
    });
    // Camera-local: lower right, in front of the eye.
    ops.push(instance(
        31,
        Some(30),
        "green",
        transform([0.35, -0.28, -0.9], 35.0, 0.3),
    ));
    harness.apply(ops);
    let eye = camera("eye", [0.0, 0.0, 0.0], 0.0, 0.0);
    let lit = harness.single(&eye);
    assert_screenshot("viewmodel", WIDTH, HEIGHT, &lit);

    // Viewmodel lights are their own rig: disabling it leaves the viewmodel
    // box unlit while the world keeps its rig.
    harness.renderer.set_options(RendererOptions {
        default_viewmodel_lights: false,
        ..RendererOptions::default()
    });
    let unlit = harness.single(&eye);
    let (x, y) = (WIDTH * 3 / 4 - 8, HEIGHT * 3 / 4);
    assert!(
        pixel(&lit, WIDTH, x, y)[1] > 60,
        "the rig lights the viewmodel"
    );
    assert!(
        pixel(&unlit, WIDTH, x, y)[1] < 8,
        "no viewmodel rig, no light"
    );
    assert_eq!(
        pixel(&lit, WIDTH, WIDTH / 2, HEIGHT / 3),
        pixel(&unlit, WIDTH, WIDTH / 2, HEIGHT / 3),
        "the world rig is unchanged"
    );
}

#[test]
fn camera_motion_interpolates_on_the_host_clock_and_holds_when_samples_stop() {
    let mut harness = Harness::new(RendererOptions::default());
    harness.apply(room());
    let sample = |id: &str, x: f64, time: f64| {
        let mut moving = camera("moving", [x, 0.6, 1.0], 0.0, -6.0);
        moving.motion = Some(RendererCameraMotion {
            sample_id: id.to_owned(),
            sample_time_seconds: time,
            delay_seconds: 0.1,
            interpolation: RendererCameraInterpolation::Pose,
            cut: false,
        });
        composition(
            vec![moving],
            vec![primary_view(
                "main",
                "moving",
                viewport(0.0, 0.0, 1.0, 1.0),
                0,
            )],
        )
    };
    harness
        .renderer
        .set_view_composition(&sample("1", 0.0, 0.0), 0.0);
    harness
        .renderer
        .set_view_composition(&sample("2", 1.0, 0.1), 0.1);
    // Delay 0.1: at presentation time 0.15 the source time is 0.05, halfway.
    let (_, halfway) = harness.composition(0.15);
    let expected = harness.single(&camera("still", [0.5, 0.6, 1.0], 0.0, -6.0));
    assert_eq!(halfway, expected);
    // No further samples, as when the simulation is held: the last pose
    // holds however far the presentation clock runs.
    let (_, held) = harness.composition(60.0);
    let expected = harness.single(&camera("still", [1.0, 0.6, 1.0], 0.0, -6.0));
    assert_eq!(held, expected);
    let cameras = harness.renderer.view_composition_readout().cameras;
    assert_eq!(cameras[0].sample_id.as_deref(), Some("2"));
}

fn capture_job(world: &PresentationWorld, operation: RenderOutputOperation) -> RenderOutputJob {
    RenderOutputJob {
        id: 1,
        source: RenderHandle::new(10),
        frame: world
            .capture_output_scene(RenderHandle::new(10), false)
            .expect("capture the red box's subtree"),
        operation,
    }
}

fn image(samples: u32, pose: Option<RenderOutputPose>) -> RenderOutputOperation {
    let mut eye = camera("capture", [-0.2, 0.9, -1.6], -25.0, -18.0);
    eye.projection = RendererCameraProjection::Perspective {
        fov_y_degrees: 45.0,
        near: 0.1,
        far: 20.0,
    };
    RenderOutputOperation::Image {
        camera: Box::new(eye),
        width: 160,
        height: 120,
        background: [0.0; 4],
        use_camera_background: false,
        exposure: 1.0,
        aces_filmic: false,
        samples,
        pose,
    }
}

#[test]
fn capture_image_writes_a_straight_alpha_png_of_the_frozen_subtree() {
    let mut harness = Harness::new(RendererOptions::default());
    harness.apply(room());
    let before = harness.renderer.table_counts();
    let png = harness
        .renderer
        .capture_image(&capture_job(&harness.world, image(4, None)), &NoResources)
        .expect("capture the red box");
    let (width, height, rgba) = decode_png_rgba(&png).expect("a PNG");
    assert_eq!((width, height), (160, 120));
    // Only the selected subtree: transparent where the box is not.
    assert_eq!(pixel(&rgba, width, 2, 2)[3], 0);
    let centre = pixel(&rgba, width, width / 2, height / 2);
    assert_eq!(centre[3], 255);
    assert!(
        centre[0] > centre[1] && centre[0] > centre[2],
        "red box: {centre:?}"
    );
    assert_screenshot("capture", width, height, &rgba);

    // The capture ran on its own tables; the live ones are untouched.
    assert_eq!(harness.renderer.table_counts(), before);

    let pose = RenderOutputPose {
        handle: RenderHandle::new(10),
        clip: "run".to_owned(),
        normalized_time: 0.5,
    };
    let refused = harness.renderer.capture_image(
        &capture_job(&harness.world, image(1, Some(pose))),
        &NoResources,
    );
    assert!(refused.unwrap_err().contains("#8788"));
    let glb = harness.renderer.capture_image(
        &capture_job(
            &harness.world,
            RenderOutputOperation::Glb {
                include_animations: false,
            },
        ),
        &NoResources,
    );
    assert!(glb.unwrap_err().contains("GLB"));
}
