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
    RendererTargetSampling, RendererViewComposition, RendererViewTarget, RendererViewport,
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
                viewport_anchor: None,
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
    assert!(pixels.as_chunks::<4>().0.iter().all(|p| *p == background));
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
fn a_root_mesh_instance_in_the_viewmodel_layer_draws_camera_local() {
    let mut harness = Harness::new(RendererOptions::default());
    let mut ops = room();
    ops.push(instance(
        20,
        None,
        "red",
        transform([0.0, 0.0, -1.3], 0.0, 1.5),
    ));
    // No group parent: the instance itself carries the layer.
    let mut held = instance(31, None, "green", transform([0.35, -0.28, -0.9], 35.0, 0.3));
    let RenderDiff::CreateStaticMeshInstance {
        instance: descriptor,
        ..
    } = &mut held
    else {
        unreachable!()
    };
    descriptor.layer = RenderLayer::Viewmodel;
    ops.push(held);
    harness.apply(ops);
    let eye = camera("eye", [0.0, 0.0, 0.0], 0.0, 0.0);
    let frame = harness.single(&eye);
    let (x, y) = (WIDTH * 3 / 4 - 8, HEIGHT * 3 / 4);
    let [r, g, ..] = pixel(&frame, WIDTH, x, y);
    assert!(
        g > 60 && g > r,
        "the green box draws over the nearer red wall: {:?}",
        [r, g]
    );
    // At the world origin it would sit behind the camera's wall instead.
    let [r, g, ..] = pixel(&frame, WIDTH, WIDTH / 2, HEIGHT / 3);
    assert!(r > g, "the wall still fills the rest of the view");
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

fn close(actual: &[f64; 3], expected: [f64; 3]) -> bool {
    actual
        .iter()
        .zip(expected)
        .all(|(actual, expected)| (actual - expected).abs() < 1e-3)
}

#[test]
fn drawn_cameras_report_the_sampled_and_observer_poses_each_view_drew_from() {
    let mut harness = Harness::new(RendererOptions::default());
    harness.apply(room());
    // "moving" draws the main view and an offscreen inset; "top" only the
    // inset. Motion puts "moving" halfway between two samples.
    let sample = |x: f64, id: &str, time: f64| {
        let mut moving = camera("moving", [x, 0.6, 1.0], 20.0, -6.0);
        moving.motion = Some(RendererCameraMotion {
            sample_id: id.to_owned(),
            sample_time_seconds: time,
            delay_seconds: 0.1,
            interpolation: RendererCameraInterpolation::Pose,
            cut: false,
        });
        let mut view = composition(
            vec![moving, camera("top", [0.0, 7.0, -4.3], 0.0, -90.0)],
            vec![
                primary_view("main", "moving", viewport(0.0, 0.0, 1.0, 1.0), 0),
                RendererCompositionView {
                    id: "inset-moving".to_owned(),
                    camera_id: "moving".to_owned(),
                    target: RendererViewTarget::Offscreen {
                        target_id: "inset".to_owned(),
                        target_revision: 1,
                    },
                    viewport: viewport(0.0, 0.0, 0.5, 1.0),
                    order: 0,
                    viewport_anchor: None,
                },
                RendererCompositionView {
                    id: "inset-top".to_owned(),
                    camera_id: "top".to_owned(),
                    target: RendererViewTarget::Offscreen {
                        target_id: "inset".to_owned(),
                        target_revision: 1,
                    },
                    viewport: viewport(0.5, 0.0, 0.5, 1.0),
                    order: 1,
                    viewport_anchor: None,
                },
            ],
        );
        view.targets.push(RendererCompositionTarget {
            id: "inset".to_owned(),
            revision: 1,
            width: 64,
            height: 32,
            color: RendererTargetColor::Rgba8Srgb,
            depth: RendererTargetDepth::Depth24,
            sampling: RendererTargetSampling::Nearest,
        });
        view
    };
    harness
        .renderer
        .set_view_composition(&sample(0.0, "1", 0.0), 0.0);
    harness
        .renderer
        .set_view_composition(&sample(1.0, "2", 0.1), 0.1);

    // Halfway through the motion, the drawn pose is the sampled one, not the
    // descriptor's latest position.
    let (_, halfway) = harness.composition(0.15);
    let drawn = harness.renderer.drawn_cameras();
    assert_eq!(drawn.len(), 2);
    assert!(!drawn[0].observer && drawn[0].offscreen.is_none());
    assert!(close(&drawn[0].pose.position, [0.5, 0.6, 1.0]), "{drawn:?}");
    assert!((drawn[0].pose.yaw_degrees - 20.0).abs() < 1e-3);
    assert!((drawn[0].pose.pitch_degrees + 6.0).abs() < 1e-3);
    assert!(close(&drawn[1].pose.position, [0.0, 7.0, -4.3]));
    // The pixels are that viewpoint's.
    let mut expected = sample(0.5, "3", 0.0);
    expected.cameras[0].motion = None;
    let mut reference = Harness::new(RendererOptions::default());
    reference.apply(room());
    reference.renderer.set_view_composition(&expected, 0.0);
    assert_eq!(reference.composition(0.0).1, halfway);

    // An observer replaces "moving" in the primary view only: the report
    // says so, and keeps the sampled pose its offscreen view still drew.
    let observer = camera("observer", [2.0, 1.5, 2.0], 35.0, -20.0);
    harness.renderer.set_observer(Some(observer.pose));
    let (_, observed) = harness.composition(0.15);
    assert_ne!(observed, halfway);
    let drawn = harness.renderer.drawn_cameras();
    assert!(drawn[0].observer);
    assert!(close(&drawn[0].pose.position, [2.0, 1.5, 2.0]));
    assert!((drawn[0].pose.yaw_degrees - 35.0).abs() < 1e-3);
    assert!((drawn[0].pose.pitch_degrees + 20.0).abs() < 1e-3);
    let (offscreen, _) = drawn[0].offscreen.as_ref().expect("offscreen pose");
    assert!(close(&offscreen.position, [0.5, 0.6, 1.0]));
    // "top" is offscreen only: the observer does not touch it.
    assert!(!drawn[1].observer && drawn[1].offscreen.is_none());
    assert!(close(&drawn[1].pose.position, [0.0, 7.0, -4.3]));

    // A different observer viewpoint is a different report.
    let other = camera("observer", [-2.0, 1.5, 2.0], -35.0, -20.0);
    harness.renderer.set_observer(Some(other.pose));
    harness.composition(0.15);
    assert_ne!(harness.renderer.drawn_cameras()[0].pose, drawn[0].pose);
}

#[test]
fn an_observer_pose_replaces_the_primary_view_camera_until_it_is_cleared() {
    let mut harness = Harness::new(RendererOptions::default());
    harness.apply(room());
    let product = camera("player", [0.0, 0.6, 3.0], 0.0, -6.0);
    harness.renderer.set_view_composition(
        &composition(
            vec![product.clone()],
            vec![primary_view(
                "main",
                "player",
                viewport(0.0, 0.0, 1.0, 1.0),
                0,
            )],
        ),
        0.0,
    );
    let (_, own) = harness.composition(0.0);
    let observer = camera("observer", [2.0, 1.5, 2.0], 35.0, -20.0);
    harness.renderer.set_observer(Some(observer.pose));
    let (_, observed) = harness.composition(0.0);
    assert_ne!(observed, own);
    assert_eq!(observed, harness.single(&observer));
    harness.renderer.set_observer(None);
    let (_, restored) = harness.composition(0.0);
    assert_eq!(restored, own);
}

#[test]
fn an_anchored_view_draws_at_its_anchor_rect_until_the_anchor_goes() {
    let mut harness = Harness::new(RendererOptions::default());
    harness.apply(room());
    let player = camera("player", [0.0, 0.6, 3.0], 0.0, -6.0);
    let at = |area: RendererViewport, anchor: Option<&str>| {
        let mut view = primary_view("main", "player", area, 0);
        view.viewport_anchor = anchor.map(str::to_owned);
        composition(vec![player.clone()], vec![view])
    };
    let left = viewport(0.0, 0.0, 0.5, 1.0);
    let inset = viewport(0.5, 0.25, 0.4, 0.5);
    harness.renderer.set_view_composition(&at(inset, None), 0.0);
    let (_, at_inset) = harness.composition(0.0);
    harness.renderer.set_view_composition(&at(left, None), 0.0);
    let (_, at_left) = harness.composition(0.0);

    harness
        .renderer
        .set_view_composition(&at(left, Some("hero")), 0.0);
    let (_, unreported) = harness.composition(0.0);
    assert_eq!(
        unreported, at_left,
        "an unreported anchor keeps the viewport"
    );
    harness
        .renderer
        .set_viewport_anchors([("hero".to_owned(), inset)].into());
    let (_, anchored) = harness.composition(0.0);
    assert_eq!(anchored, at_inset);
    // A later composition keeps following the anchor.
    harness
        .renderer
        .set_view_composition(&at(left, Some("hero")), 0.0);
    assert_eq!(harness.composition(0.0).1, at_inset);
    harness.renderer.set_viewport_anchors(Default::default());
    assert_eq!(harness.composition(0.0).1, at_left);
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
    // A pose names an animated instance; this box is not one (pose captures
    // of animated meshes: tests/animated.rs).
    let refused = harness.renderer.capture_image(
        &capture_job(&harness.world, image(1, Some(pose))),
        &NoResources,
    );
    assert!(refused.unwrap_err().contains("unknown animated mesh"));
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

#[test]
fn a_partly_transparent_capture_background_keeps_its_colour() {
    let mut harness = Harness::new(RendererOptions::default());
    harness.apply(room());
    let corner = |alpha: f32, harness: &Harness| {
        let mut operation = image(1, None);
        if let RenderOutputOperation::Image { background, .. } = &mut operation {
            *background = [0.25, 0.1, 0.05, alpha];
        }
        let png = harness
            .renderer
            .capture_image(&capture_job(&harness.world, operation), &NoResources)
            .expect("capture");
        let (width, _, rgba) = decode_png_rgba(&png).expect("a PNG");
        pixel(&rgba, width, 2, 2)
    };
    let opaque = corner(1.0, &harness);
    let half = corner(0.5, &harness);
    // Straight alpha: only alpha changes with the background's opacity.
    assert_eq!(opaque[3], 255);
    assert!(half[3].abs_diff(128) <= 1, "{half:?}");
    for channel in 0..3 {
        assert!(
            opaque[channel].abs_diff(half[channel]) <= 1,
            "{opaque:?} vs {half:?}"
        );
    }
}

#[test]
fn a_capture_keeps_the_scene_bloom_and_sky_light_but_its_own_exposure() {
    let mut harness = Harness::new(RendererOptions::default());
    harness.apply(room());
    let capture = |harness: &Harness| {
        let png = harness
            .renderer
            .capture_image(&capture_job(&harness.world, image(1, None)), &NoResources)
            .expect("capture");
        decode_png_rgba(&png).expect("a PNG").2
    };
    let plain = capture(&harness);
    // Auto exposure would scale the capture's exposure: it is left out.
    harness.apply(vec![RenderDiff::SetAutoExposure {
        auto_exposure: Some(AutoExposureDescriptor {
            speed: 1.0,
            min_exposure: 4.0,
            max_exposure: 8.0,
        }),
    }]);
    assert_eq!(capture(&harness), plain);
    // Bloom is the scene's light, kept as its fog is.
    harness.apply(vec![RenderDiff::SetBloom {
        bloom: Some(BloomDescriptor {
            threshold: 0.0,
            intensity: 4.0,
        }),
    }]);
    let bloomed = capture(&harness);
    assert_ne!(bloomed, plain);
    // So is the sky's light.
    harness.apply(vec![RenderDiff::SetSkyLight {
        sky_light: Some(SkyLightDescriptor { intensity: 2.0 }),
    }]);
    assert_ne!(capture(&harness), bloomed);
}

/// A dark background and a box at `at`, glowing by `emission` (0 for a
/// plain grey box).
fn lone_box(name: &str, handle: u64, at: [f32; 3], emission: f32) -> Vec<RenderDiff> {
    let mut ops = coloured_mesh(name, cube(), [0.6, 0.6, 0.6, 1.0]);
    if let RenderDiff::DefineMaterial { material } = &mut ops[0] {
        material.emission_color = [1.0, 0.8, 0.5];
        material.emission_intensity = emission;
    }
    ops.push(instance(handle, None, name, transform(at, 0.0, 1.5)));
    ops
}

/// Two side-by-side views of one target: the left looks down -z, the right
/// down +z.
fn back_to_back() -> RendererViewComposition {
    composition(
        vec![
            camera("ahead", [0.0, 0.0, 0.0], 0.0, 0.0),
            camera("behind", [0.0, 0.0, 0.0], 180.0, 0.0),
        ],
        vec![
            primary_view("left", "ahead", viewport(0.0, 0.0, 0.5, 1.0), 0),
            primary_view("right", "behind", viewport(0.5, 0.0, 0.5, 1.0), 1),
        ],
    )
}

fn half(rgba: &[u8], right: bool) -> Vec<[u8; 4]> {
    let columns = if right {
        WIDTH / 2..WIDTH
    } else {
        0..WIDTH / 2
    };
    (0..HEIGHT)
        .flat_map(|y| columns.clone().map(move |x| (x, y)))
        .map(|(x, y)| pixel(rgba, WIDTH, x, y))
        .collect()
}

fn largest_difference(a: &[[u8; 4]], b: &[[u8; 4]]) -> u8 {
    a.iter()
        .zip(b)
        .flat_map(|(a, b)| (0..3).map(move |c| a[c].abs_diff(b[c])))
        .max()
        .unwrap_or(0)
}

#[test]
fn a_view_blooms_only_its_own_viewport() {
    let mut harness = Harness::new(RendererOptions::default());
    harness.apply(vec![RenderDiff::SetBackgroundColor {
        color: [0.02, 0.03, 0.06, 1.0],
    }]);
    // Near the left view's right edge, so its glow would cross into the
    // right view.
    harness.apply(lone_box("glow", 1, [0.6, 0.0, -3.0], 8.0));
    harness.renderer.set_view_composition(&back_to_back(), 0.0);
    let (_, plain) = harness.composition(0.0);
    harness.apply(vec![RenderDiff::SetBloom {
        bloom: Some(BloomDescriptor {
            threshold: 1.0,
            intensity: 4.0,
        }),
    }]);
    let (_, bloomed) = harness.composition(0.0);
    assert!(
        largest_difference(&half(&plain, false), &half(&bloomed, false)) > 20,
        "the left view glows"
    );
    assert!(
        largest_difference(&half(&plain, true), &half(&bloomed, true)) <= 1,
        "the right view, beside the glow, is untouched"
    );
}

#[test]
fn auto_exposure_measures_only_the_view_it_adapts_at() {
    let left = |bright_behind: bool| {
        let mut harness = Harness::new(RendererOptions::default());
        harness.apply(vec![
            RenderDiff::SetBackgroundColor {
                color: [0.02, 0.03, 0.06, 1.0],
            },
            RenderDiff::SetAutoExposure {
                auto_exposure: Some(AutoExposureDescriptor {
                    speed: 1.0,
                    min_exposure: 0.01,
                    max_exposure: 100.0,
                }),
            },
        ]);
        harness.apply(lone_box("ahead", 1, [0.0, 0.0, -3.0], 0.0));
        if bright_behind {
            // Only the right view sees it.
            harness.apply(lone_box("behind", 2, [0.0, 0.0, 3.0], 8.0));
        }
        harness.renderer.set_view_composition(&back_to_back(), 0.0);
        harness.composition(0.0);
        // A later frame adapts again, with the other view's last frame
        // still in the shared target.
        harness.renderer.set_animation_time(100.0);
        let (_, pixels) = harness.composition(0.0);
        (half(&pixels, false), half(&pixels, true))
    };
    let (alone, dark_right) = left(false);
    let (beside_bright, bright_right) = left(true);
    assert!(largest_difference(&dark_right, &bright_right) > 20);
    // The first world view (the left) sets the exposure from its own world.
    assert!(
        largest_difference(&alone, &beside_bright) <= 1,
        "the left view's exposure ignores the right view"
    );
}

#[test]
fn every_view_of_a_crowded_frame_blooms() {
    let mut harness = Harness::new(RendererOptions::default());
    harness.apply(vec![
        RenderDiff::SetBackgroundColor {
            color: [0.02, 0.03, 0.06, 1.0],
        },
        RenderDiff::SetBloom {
            bloom: Some(BloomDescriptor {
                threshold: 1.0,
                intensity: 4.0,
            }),
        },
    ]);
    harness.apply(lone_box("glow", 1, [0.0, 0.0, -3.0], 8.0));
    // Sixteen views of one camera, more post passes than a frame starts
    // with uniform slots for.
    let (columns, rows) = (4u32, 4u32);
    let views = (0..columns * rows)
        .map(|index| {
            let (column, row) = (index % columns, index / columns);
            primary_view(
                &format!("view-{index}"),
                "ahead",
                viewport(
                    f64::from(column) / f64::from(columns),
                    f64::from(row) / f64::from(rows),
                    1.0 / f64::from(columns),
                    1.0 / f64::from(rows),
                ),
                u64::from(index),
            )
        })
        .collect();
    harness.renderer.set_view_composition(
        &composition(vec![camera("ahead", [0.0, 0.0, 0.0], 0.0, 0.0)], views),
        0.0,
    );
    let (_, pixels) = harness.composition(0.0);
    let (width, height) = (WIDTH / columns, HEIGHT / rows);
    let cell = |column: u32, row: u32| -> Vec<[u8; 4]> {
        (0..height)
            .flat_map(|y| (0..width).map(move |x| (column * width + x, row * height + y)))
            .map(|(x, y)| pixel(&pixels, WIDTH, x, y))
            .collect()
    };
    let first = cell(0, 0);
    for row in 0..rows {
        for column in 0..columns {
            assert!(
                largest_difference(&first, &cell(column, row)) <= 2,
                "view at column {column}, row {row} draws as the first does"
            );
        }
    }
}

#[test]
fn a_viewmodel_field_of_view_narrower_than_the_camera_draws_the_viewmodel_larger() {
    let mut harness = Harness::new(RendererOptions::default());
    let mut ops = room();
    let mut root = RenderNode::new(Geometry::Group);
    root.layer = RenderLayer::Viewmodel;
    ops.push(RenderDiff::Create {
        handle: RenderHandle::new(30),
        parent: None,
        node: root,
    });
    // Centred, so a narrower field of view scales it about the middle of the
    // frame rather than pushing it off an edge.
    ops.push(instance(
        31,
        Some(30),
        "green",
        transform([0.0, 0.0, -1.2], 35.0, 0.3),
    ));
    harness.apply(ops);
    let green = |frame: &[u8]| {
        frame
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|[r, g, _, _]| *g > 60 && *g > *r)
            .count()
    };
    let eye = camera("eye", [0.0, 0.0, 0.0], 0.0, 0.0);
    let world_fov = green(&harness.single(&eye));
    let mut narrow = eye.clone();
    narrow.viewmodel_fov_y_degrees = 30.0;
    let narrow_fov = green(&harness.single(&narrow));
    assert!(world_fov > 0, "the viewmodel box is in view");
    assert!(
        narrow_fov > world_fov * 2,
        "a 30 degree viewmodel field of view magnifies the box about the centre, roughly four times its area at the camera's 60: {narrow_fov} vs {world_fov} pixels"
    );
    let mut wide = eye.clone();
    wide.viewmodel_fov_y_degrees = eye_fov(&eye);
    assert_eq!(
        green(&harness.single(&wide)),
        world_fov,
        "the camera's own field of view asked for explicitly draws the same"
    );
}

fn eye_fov(camera: &render_host_contracts::RendererCompositionCamera) -> f64 {
    match camera.projection {
        render_host_contracts::RendererCameraProjection::Perspective { fov_y_degrees, .. } => {
            fov_y_degrees
        }
        _ => unreachable!("the test camera is perspective"),
    }
}
