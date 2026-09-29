//! View fixtures: camera composition (split primary views, an offscreen target
//! presented as an inset), the viewmodel pass, camera motion on the host
//! clock, and output captures. Scenes go through `PresentationWorld` as the
//! runtime applies them; references live in `tests/screenshots/` and use the
//! tolerance `screenshots.rs` documents.
//!
//! `RENDER_WGPU_BLESS=1 cargo test -p render-wgpu --test views` rewrites the
//! references. Needs a wgpu adapter: CI uses `WGPU_BACKEND=vulkan` with
//! llvmpipe.

mod common;

use common::*;
use render_host_contracts::{
    RenderOutputJob, RenderOutputOperation, RenderOutputPose, RendererCameraInterpolation,
    RendererCameraMotion, RendererCameraProjection, RendererCompositionPresentation,
    RendererCompositionTarget, RendererCompositionView, RendererPrimaryDestination,
    RendererPrimaryDestinationKind, RendererTargetColor, RendererTargetDepth,
    RendererTargetSampling, RendererViewTarget,
};
use render_model::*;
use render_presentation::PresentationWorld;
use render_wgpu::{decode_png_rgba, NoResources, RendererOptions, TargetStatus};

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
