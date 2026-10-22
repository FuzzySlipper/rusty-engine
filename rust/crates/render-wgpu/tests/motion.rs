//! Motion vectors (`motion.rs`): while inspection shows the motion target
//! (`OverlayShown::motion`), the primary views draw each pixel's motion
//! since the previous frame, and the frame says when its history resets.
//! The motion view encodes x and y as red and green, `0.5 + pixels / 32`,
//! so these tests read the motion back from the picture.

mod common;

use common::*;
use render_host_contracts::{
    RendererCameraInterpolation, RendererCameraMotion, RendererCompositionCamera,
};
use render_model::*;
use render_presentation::{
    ParticleAnchor, ParticleColorKey, ParticleEmitterDescriptor, ParticleProjectionOp,
    ParticleScalarKey, ParticleVisual, PresentationFrameDiff, PresentationOp, PresentationOpMeta,
};
use render_wgpu::{FrameStats, OverlayShown, RendererOptions};

/// The motion view's range: `compose.wgsl` `MOTION_VIEW_RANGE`.
const RANGE: f32 = 16.0;
/// The camera's focal length in pixels: 90 / tan(30°), for the fixture
/// camera's 60° field of view over 180 rows.
const FOCAL_PIXELS: f32 = 155.884_57;

fn shown() -> OverlayShown {
    OverlayShown {
        motion: true,
        ..OverlayShown::default()
    }
}

fn harness() -> Harness {
    let mut harness = Harness::new(RendererOptions::default());
    harness.renderer.set_overlay_shown(shown());
    harness
}

/// The motion at a pixel, render pixels to the right and down.
fn motion(rgba: &[u8], x: u32, y: u32) -> [f32; 2] {
    let [red, green, ..] = pixel(rgba, WIDTH, x, y);
    [red, green].map(|value| (f32::from(value) / 255.0 - 0.5) * 2.0 * RANGE)
}

/// Whether a pixel has no motion of its own (`NO_MOTION`, shown blue): the
/// sky behind everything, and water.
fn no_motion(rgba: &[u8], x: u32, y: u32) -> bool {
    pixel(rgba, WIDTH, x, y)[..3] == [0, 0, 255]
}

/// Pixels whose motion along `axis` is beyond `limit` (positive) or below
/// `-limit` (negative): (beyond, below).
fn moving(rgba: &[u8], axis: usize, limit: f32) -> (usize, usize) {
    let mut counts = (0, 0);
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            if no_motion(rgba, x, y) {
                continue;
            }
            let value = motion(rgba, x, y)[axis];
            if value > limit {
                counts.0 += 1;
            } else if value < -limit {
                counts.1 += 1;
            }
        }
    }
    counts
}

fn draw(harness: &mut Harness, camera: &RendererCompositionCamera) -> (FrameStats, Vec<u8>) {
    let stats = harness.renderer.render_offscreen(camera, &harness.target);
    (stats, harness.target.read_rgba(&harness.gpu))
}

fn assert_close(found: [f32; 2], expected: [f32; 2], tolerance: f32, what: &str) {
    assert!(
        (found[0] - expected[0]).abs() <= tolerance && (found[1] - expected[1]).abs() <= tolerance,
        "{what}: {found:?}, expected {expected:?}"
    );
}

/// A cube one metre across at eye height, `distance` metres ahead of the
/// eye at the origin, under group 5.
fn cube_ahead(distance: f32) -> Vec<RenderDiff> {
    let mut ops = vec![RenderDiff::SetBackgroundColor {
        color: [0.0, 0.0, 0.0, 1.0],
    }];
    ops.extend(coloured_mesh("cube", cube(), [0.7, 0.6, 0.5, 1.0]));
    ops.push(RenderDiff::Create {
        handle: RenderHandle::new(5),
        parent: None,
        node: RenderNode::new(Geometry::Group),
    });
    ops.push(instance(
        6,
        Some(5),
        "cube",
        transform([0.0, 0.0, -distance], 0.0, 1.0),
    ));
    ops
}

fn eye(yaw: f64) -> RendererCompositionCamera {
    camera("eye", [0.0; 3], yaw, 0.0)
}

fn move_group(harness: &mut Harness, x: f32) {
    harness.apply(vec![RenderDiff::Update {
        handle: RenderHandle::new(5),
        transform: Some(transform([x, 0.0, 0.0], 0.0, 1.0)),
        material: None,
        visible: None,
        metadata: None,
    }]);
}

const CENTRE: (u32, u32) = (WIDTH / 2, HEIGHT / 2);

#[test]
fn the_world_moves_against_a_turning_camera_and_holds_still_with_it() {
    let mut harness = harness();
    harness.apply(cube_ahead(3.0));
    let (first, still) = draw(&mut harness, &eye(0.0));
    let first = first.motion.expect("the inspection draws motion");
    assert!(first.history_reset, "the first frame has no history");
    assert_eq!(first.jitter_phase, None, "no temporal stage, no jitter");
    assert_eq!(moving(&still, 0, 0.2), (0, 0), "nothing moved yet");
    let (second, _) = draw(&mut harness, &eye(0.0));
    assert!(!second.motion.unwrap().history_reset);
    // Turning 2° right moves the cube at the centre left by the focal
    // length times tan 2°; the background has no motion of its own.
    let (_, turned) = draw(&mut harness, &eye(2.0));
    let expected = -FOCAL_PIXELS * 2.0_f32.to_radians().tan();
    assert_close(
        motion(&turned, CENTRE.0, CENTRE.1),
        [expected, 0.0],
        0.4,
        "the cube against the turn",
    );
    assert!(no_motion(&turned, 4, 4), "the background");
    let (_, held) = draw(&mut harness, &eye(2.0));
    assert_close(
        motion(&held, CENTRE.0, CENTRE.1),
        [0.0, 0.0],
        0.2,
        "a camera that holds",
    );
}

#[test]
fn a_moved_part_moves_once_and_then_holds() {
    let mut harness = harness();
    harness.apply(cube_ahead(3.0));
    draw(&mut harness, &eye(0.0));
    move_group(&mut harness, 0.2);
    let (_, moved) = draw(&mut harness, &eye(0.0));
    // The cube's face is 2.5 m away: 0.2 m across is that share of the
    // focal length.
    assert_close(
        motion(&moved, CENTRE.0, CENTRE.1),
        [FOCAL_PIXELS * 0.2 / 2.5, 0.0],
        0.5,
        "the moved cube",
    );
    assert!(no_motion(&moved, 4, 4), "the background");
    // Not moved again: its previous transform catches up.
    let (_, held) = draw(&mut harness, &eye(0.0));
    assert_eq!(moving(&held, 0, 0.2), (0, 0), "nothing moves");
}

#[test]
fn the_viewmodel_moves_with_the_camera() {
    let mut harness = harness();
    let mut ops = cube_ahead(4.0);
    ops.push(RenderDiff::CreateStaticMeshInstance {
        handle: RenderHandle::new(9),
        parent: None,
        instance: StaticMeshInstanceDescriptor {
            asset: "mesh/cube".to_owned(),
            transform: transform([0.0, -0.45, -1.2], 0.0, 0.3),
            visible: true,
            material_overrides: Vec::new(),
            metadata: RenderMetadata::default(),
            layer: RenderLayer::Viewmodel,
            shadow_casting: Default::default(),
        },
    });
    harness.apply(ops);
    draw(&mut harness, &eye(0.0));
    let (_, turned) = draw(&mut harness, &eye(2.0));
    // The viewmodel cube sits below the centre, the world cube at it.
    let held = motion(&turned, CENTRE.0, CENTRE.1 + 50);
    assert_close(held, [0.0, 0.0], 0.2, "the viewmodel");
    assert!(
        motion(&turned, CENTRE.0, CENTRE.1)[0] < -4.0,
        "the world turns"
    );
}

#[test]
fn swaying_vegetation_moves_by_the_wind_between_frames() {
    let mut harness = harness();
    let mut ops = vec![RenderDiff::SetBackgroundColor {
        color: [0.0, 0.0, 0.0, 1.0],
    }];
    let mut post = cube();
    let MeshPayloadSource::Inline { positions, .. } = &mut post.source else {
        unreachable!()
    };
    // A post 0.2 m across and 2 m tall standing on its origin.
    for point in positions.as_chunks_mut::<3>().0 {
        point[0] *= 0.2;
        point[1] = (point[1] + 0.5) * 2.0;
        point[2] *= 0.2;
    }
    post.bounds = MeshBoundsDescriptor {
        min: [-0.1, 0.0, -0.1],
        max: [0.1, 2.0, 0.1],
    };
    let mut swaying = coloured_mesh("post", post, [0.9, 0.9, 0.9, 1.0]);
    if let RenderDiff::DefineMaterial { material } = &mut swaying[0] {
        material.wind = Some(MaterialWindDescriptor {
            bend: 0.25,
            flutter: 0.0,
        });
    }
    ops.extend(swaying);
    ops.push(instance(
        6,
        None,
        "post",
        transform([0.0, -1.0, -3.0], 0.0, 1.0),
    ));
    ops.push(RenderDiff::SetWind {
        wind: Some(WindDescriptor {
            direction: [1.0, 0.0],
            strength: 2.0,
            gust: 1.0,
        }),
    });
    harness.apply(ops);
    let view = eye(0.0);
    harness.renderer.set_animation_time(3.0);
    draw(&mut harness, &view);
    harness.renderer.set_animation_time(3.1);
    let (_, swayed) = draw(&mut harness, &view);
    // The post's foot stands still; its top leans with the gusts.
    let foot = (CENTRE.1 as f32 + FOCAL_PIXELS * 0.9 / 2.9) as u32;
    let top = (CENTRE.1 as f32 - FOCAL_PIXELS * 0.9 / 2.9) as u32;
    let base = motion(&swayed, CENTRE.0, foot);
    assert!(base[0].abs() < 0.3, "the foot holds: {base:?}");
    let (right, left) = moving(&swayed, 0, 0.5);
    assert!(
        right + left > 20,
        "the top sways: {right} right, {left} left"
    );
    assert!(
        motion(&swayed, CENTRE.0, top)[0].abs() > 0.5 || right + left > 40,
        "the top sways"
    );
    // A held time holds the sway.
    let (_, held) = draw(&mut harness, &view);
    assert_eq!(moving(&held, 0, 0.3), (0, 0), "a held time holds");
}

/// A swaying post `width` across and `height` tall standing on its origin,
/// unlit white, bending `bend` per metre in a gusting wind along +x, as
/// node 6 at `at`.
fn swaying_post(width: f32, height: f32, bend: f32, at: [f32; 3]) -> Vec<RenderDiff> {
    let mut ops = vec![RenderDiff::SetBackgroundColor {
        color: [0.0, 0.0, 0.0, 1.0],
    }];
    let mut post = cube();
    let MeshPayloadSource::Inline { positions, .. } = &mut post.source else {
        unreachable!()
    };
    for point in positions.as_chunks_mut::<3>().0 {
        point[0] *= width;
        point[1] = (point[1] + 0.5) * height;
        point[2] *= width;
    }
    post.bounds = MeshBoundsDescriptor {
        min: [-width / 2.0, 0.0, -width / 2.0],
        max: [width / 2.0, height, width / 2.0],
    };
    let mut swaying = coloured_mesh("post", post, [1.0, 1.0, 1.0, 1.0]);
    if let RenderDiff::DefineMaterial { material } = &mut swaying[0] {
        material.unlit = true;
        material.wind = Some(MaterialWindDescriptor { bend, flutter: 0.0 });
    }
    ops.extend(swaying);
    ops.push(instance(6, None, "post", transform(at, 0.0, 1.0)));
    ops.push(RenderDiff::SetWind {
        wind: Some(WindDescriptor {
            direction: [1.0, 0.0],
            strength: 2.0,
            gust: 1.0,
        }),
    });
    ops
}

/// The coverage-weighted mean x of row `y` of a picture of a white shape
/// on black.
fn centroid(rgba: &[u8], y: u32) -> f32 {
    let (mut sum, mut weight) = (0.0, 0.0);
    for x in 0..WIDTH {
        let value = f32::from(pixel(rgba, WIDTH, x, y)[0]) / 255.0;
        sum += value * x as f32;
        weight += value;
    }
    assert!(weight > 1.0, "row {y} shows the post");
    sum / weight
}

#[test]
fn a_moved_swaying_post_moves_from_where_the_wind_swayed_it() {
    // The post moves 1.5 m across while the gust changes: the motion near
    // its top is what the two pictures show, the sway the previous frame
    // drew at the post's previous place.
    let (from, to) = ([0.0, -3.0, -20.0], [1.5, -3.0, -20.0]);
    let mut pictures = Harness::new(RendererOptions::default());
    let mut motions = harness();
    let view = eye(0.0);
    let mut frames = Vec::new();
    for harness in [&mut pictures, &mut motions] {
        harness.apply(swaying_post(0.6, 6.0, 0.1, from));
        harness.renderer.set_animation_time(3.0);
        let before = draw(harness, &view).1;
        harness.apply(vec![RenderDiff::Update {
            handle: RenderHandle::new(6),
            transform: Some(transform(to, 0.0, 1.0)),
            material: None,
            visible: None,
            metadata: None,
        }]);
        harness.renderer.set_animation_time(3.05);
        let after = draw(harness, &view).1;
        frames.push((before, after));
    }
    let (before, after) = &frames[0];
    let moved = &frames[1].1;
    // Rows from just under the post's top (3 m up at 20 m) to its middle.
    let top = (CENTRE.1 as f32 - FOCAL_PIXELS * 3.0 / 20.0).ceil() as u32 + 2;
    for y in [top, top + 6, top + 12] {
        let now = centroid(after, y);
        let expected = now - centroid(before, y);
        let found = motion(moved, now.round() as u32, y)[0];
        assert!(
            (found - expected).abs() < 0.5,
            "row {y}: motion {found}, the pictures moved {expected}"
        );
    }
}

#[test]
fn scatter_copies_move_under_their_patch() {
    let mut harness = harness();
    let mut ops = cube_ahead(3.0);
    // The same cube as two scattered copies under group 5, either side.
    ops.push(RenderDiff::CreateScatterPatch {
        handle: RenderHandle::new(7),
        parent: Some(RenderHandle::new(5)),
        patch: ScatterPatchDescriptor {
            asset: "mesh/cube".to_owned(),
            material_overrides: Vec::new(),
            instances: [-1.4, 1.4]
                .map(|x| ScatterInstance {
                    translation: [x, 0.0, -3.0],
                    rotation: [0.0, 0.0, 0.0, 1.0],
                    scale: 0.6,
                    tint: [1.0; 3],
                    rank: 0.5,
                })
                .to_vec(),
            fade: ScatterFade::default(),
            shadow_casting: ShadowCasting::None,
            lods: Vec::new(),
            lod_band: 0.0,
            thinning: Default::default(),
        },
    });
    harness.apply(ops);
    draw(&mut harness, &eye(0.0));
    move_group(&mut harness, 0.2);
    let (_, moved) = draw(&mut harness, &eye(0.0));
    // A copy's face is 2.7 m away.
    let copy = (CENTRE.0 as f32 + FOCAL_PIXELS * 1.4 / 2.7) as u32;
    assert_close(
        motion(&moved, copy, CENTRE.1),
        [FOCAL_PIXELS * 0.2 / 2.7, 0.0],
        0.6,
        "a copy",
    );
    let (_, held) = draw(&mut harness, &eye(0.0));
    assert_eq!(moving(&held, 0, 0.2), (0, 0), "nothing moves");
}

#[test]
fn particles_move_from_where_they_were_drawn() {
    let mut harness = harness();
    harness.apply(vec![RenderDiff::SetBackgroundColor {
        color: [0.0, 0.0, 0.0, 1.0],
    }]);
    let descriptor = ParticleEmitterDescriptor {
        anchor: ParticleAnchor::World {
            position: [0.0, 0.0, -3.0],
        },
        visual: ParticleVisual::Cube,
        size_mode: Default::default(),
        blend: Default::default(),
        softness_metres: 0.0,
        rate_per_second: 0.0,
        burst_count: 24,
        lifetime_seconds: [5.0, 5.0],
        // Rising, so each moves up the screen.
        velocity_min: [-0.2, 2.0, -0.2],
        velocity_max: [0.2, 2.5, 0.2],
        acceleration: [0.0, 0.0, 0.0],
        size_curve: vec![ParticleScalarKey {
            age: 0.0,
            value: 0.15,
        }],
        color_curve: vec![ParticleColorKey {
            age: 0.0,
            color: [1.0, 0.9, 0.3, 1.0],
        }],
        flipbook_frames_per_second: 0.0,
        seed: 4,
        max_particles: 64,
        visible: true,
        collision: None,
        backdrop: false,
    };
    let issues = harness.renderer.apply_presentation(
        &PresentationFrameDiff::from_ops(vec![PresentationOp::Particle {
            meta: PresentationOpMeta::new(0),
            op: ParticleProjectionOp::Emit {
                signal_id: "rise".to_owned(),
                descriptor,
            },
        }]),
        &harness.resources,
        &|_| None,
    );
    assert!(issues.is_empty(), "{issues:?}");
    let view = eye(0.0);
    harness.renderer.advance_effects(0.05, &|_| None);
    draw(&mut harness, &view);
    harness.renderer.advance_effects(0.05, &|_| None);
    let (_, risen) = draw(&mut harness, &view);
    let (down, up) = moving(&risen, 1, 1.0);
    assert!(up > 20 && down == 0, "{up} rising, {down} falling");
    // Held: no Engine time, no motion.
    harness.renderer.advance_effects(0.0, &|_| None);
    let (_, held) = draw(&mut harness, &view);
    assert_eq!(moving(&held, 1, 0.3), (0, 0), "held particles hold");
}

#[test]
fn falling_rain_moves_down() {
    let mut harness = harness();
    harness.apply(vec![
        RenderDiff::SetBackgroundColor {
            color: [0.05, 0.05, 0.07, 1.0],
        },
        RenderDiff::SetPrecipitation {
            precipitation: Some(PrecipitationDescriptor {
                drops: 20_000,
                shape: PrecipitationShape::Streak,
                velocity: [0.0, -4.0, 0.0],
                size: 0.03,
                streak_seconds: 0.05,
                color: [2.0, 2.0, 2.2, 0.9],
                additive: false,
                radius: 6.0,
                height: 6.0,
            }),
        },
    ]);
    let view = eye(0.0);
    harness.renderer.set_animation_time(3.0);
    draw(&mut harness, &view);
    harness.renderer.set_animation_time(3.02);
    let (_, fallen) = draw(&mut harness, &view);
    let (down, up) = moving(&fallen, 1, 0.5);
    assert!(down > 50, "drops fall: {down} pixels");
    assert!(up < down / 50, "none rise: {up} against {down}");
}

#[test]
fn the_history_resets_on_a_cut_a_resize_and_a_settings_change() {
    let mut harness = harness();
    harness.apply(cube_ahead(3.0));
    let reset = |harness: &mut Harness, camera: &RendererCompositionCamera| {
        draw(harness, camera)
            .0
            .motion
            .expect("drawing motion")
            .history_reset
    };
    let view = eye(0.0);
    assert!(reset(&mut harness, &view), "the first frame");
    assert!(!reset(&mut harness, &view));
    let mut cut = view.clone();
    cut.motion = Some(RendererCameraMotion {
        sample_id: "cut".to_owned(),
        sample_time_seconds: 1.0,
        delay_seconds: 0.0,
        interpolation: RendererCameraInterpolation::Pose,
        cut: true,
    });
    assert!(reset(&mut harness, &cut), "a camera cut");
    assert!(!reset(&mut harness, &view));
    harness
        .target
        .resize(&harness.gpu, WIDTH / 2, HEIGHT / 2, 4);
    assert!(reset(&mut harness, &view), "a resize");
    assert!(!reset(&mut harness, &view));
    harness.apply(vec![RenderDiff::SetRendererSettings {
        settings: RendererSettingsDescriptor {
            shadows: true,
            ..RendererSettingsDescriptor::DEFAULT
        },
    }]);
    assert!(reset(&mut harness, &view), "a settings change");
    assert!(!reset(&mut harness, &view));
    harness.renderer.set_observer(Some(view.pose));
    assert!(reset(&mut harness, &view), "the observer comes");
}

#[test]
fn drawing_motion_leaves_the_picture_and_nothing_draws_it_unasked() {
    let mut harness = Harness::new(RendererOptions::default());
    harness.apply(cube_ahead(3.0));
    let view = eye(0.0);
    let (stats, picture) = draw(&mut harness, &view);
    assert_eq!(stats.motion, None, "no stage and no inspection: no motion");
    harness.renderer.set_overlay_shown(shown());
    let (_, motion_view) = draw(&mut harness, &view);
    assert_ne!(motion_view, picture, "the motion view replaces the picture");
    harness.renderer.set_overlay_shown(OverlayShown::default());
    let (stats, again) = draw(&mut harness, &view);
    assert_eq!(stats.motion, None);
    assert_eq!(again, picture, "the picture is as it was");
}
