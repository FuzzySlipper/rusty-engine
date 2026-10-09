//! The backdrop (`SetBackdrop`, `RenderLayer::Backdrop`; `frame.rs`
//! `encode_backdrop`): a range in the backdrop at 1:1000 draws as the same
//! range at world scale would, beyond the world camera's far plane, fogged
//! at its world-equivalent distance, at any turn and as the camera walks;
//! world geometry always covers it; an origin rebase that moves the camera
//! and the anchor together leaves it where it was; without a link or
//! without content nothing changes. Same harness as `tests/screenshots.rs`.

mod support;

use render_host_contracts::{
    RendererCameraProjection, RendererCompositionView, RendererViewComposition, RendererViewTarget,
    RendererViewport,
};
use render_model::*;
use render_wgpu::RendererOptions;
use support::*;

/// World metres per backdrop unit.
const SCALE: f64 = 1000.0;

/// A sky-blue clear, a sun from ahead and above and an ambient fill, and
/// fog that hides about two thirds of what stands 4.5 km away.
fn scene() -> Harness {
    let mut harness = Harness::new(RendererOptions {
        default_world_lights: false,
        ..RendererOptions::default()
    });
    harness.apply(vec![
        RenderDiff::SetBackgroundColor {
            color: [0.45, 0.6, 0.85, 1.0],
        },
        RenderDiff::SetFog {
            fog: Some(FogDescriptor::Exponential {
                color: [0.6, 0.7, 0.85],
                density: 1.0 / 4000.0,
            }),
        },
        RenderDiff::CreateLight {
            handle: RenderHandle::new(10),
            parent: None,
            light: LightDescriptor::Directional {
                color: [1.0, 0.95, 0.85],
                intensity: 2.0,
                enabled: true,
                direction: [0.4, -0.6, 0.7],
                range: None,
                shadow_intent: LightShadowIntent::Disabled,
                shadow: Default::default(),
            },
        },
        RenderDiff::CreateLight {
            handle: RenderHandle::new(11),
            parent: None,
            light: LightDescriptor::Ambient {
                color: [0.5, 0.55, 0.65],
                intensity: 0.4,
                enabled: true,
                range: None,
                shadow_intent: LightShadowIntent::Disabled,
                shadow: Default::default(),
            },
        },
        RenderDiff::DefineMaterial {
            material: material("material/rock", [0.55, 0.45, 0.35, 1.0], None),
        },
    ]);
    harness
}

fn link(anchor: [f64; 3]) -> RenderDiff {
    RenderDiff::SetBackdrop {
        camera: None,
        backdrop: Some(BackdropDescriptor {
            anchor,
            origin: [0.0; 3],
            scale: SCALE,
        }),
    }
}

fn placed(handle: u64, asset: &str, layer: RenderLayer) -> RenderDiff {
    RenderDiff::CreateStaticMeshInstance {
        handle: RenderHandle::new(handle),
        parent: None,
        instance: StaticMeshInstanceDescriptor {
            asset: asset.to_owned(),
            transform: transform([0.0; 3], 0.0, [1.0; 3]),
            visible: true,
            material_overrides: Vec::new(),
            metadata: RenderMetadata::default(),
            layer,
            shadow_casting: Default::default(),
        },
    }
}

/// A range 1.5 km either side, 500 m high, 4 to 5 km ahead (-Z), with a
/// spur nearer on the left: in backdrop units in the backdrop layer, or at
/// world scale in the scene.
fn range(layer: RenderLayer) -> Vec<RenderDiff> {
    let s = if layer == RenderLayer::Backdrop {
        1.0
    } else {
        SCALE as f32
    };
    let mut ops = Vec::new();
    for (index, (min, max)) in [
        ([-1.5, 0.0, -5.0], [1.5, 0.5, -4.0]),
        ([-1.4, 0.0, -2.5], [-0.8, 0.25, -2.0]),
    ]
    .into_iter()
    .enumerate()
    {
        let asset = format!("mesh/range-{index}");
        ops.push(static_mesh(
            &asset,
            box_mesh(
                [min[0] * s, min[1] * s, min[2] * s],
                [max[0] * s, max[1] * s, max[2] * s],
                |_| 0,
            ),
            "material/rock",
        ));
        ops.push(placed(30 + index as u64, &asset, layer));
    }
    ops
}

/// From `position` toward `yaw` degrees (0 faces -Z), a little up, seeing
/// `far` metres.
fn look(harness: &mut Harness, position: [f64; 3], yaw: f64, far: f64) -> Vec<u8> {
    let mut camera = camera(position, yaw, 3.0);
    camera.projection = RendererCameraProjection::Perspective {
        fov_y_degrees: 60.0,
        near: 0.1,
        far,
    };
    harness.render(&camera).1
}

/// The share of pixels that differ by more than `tolerance` summed over
/// the channels.
fn changed(a: &[u8], b: &[u8], tolerance: i32) -> f64 {
    let (pixels, changed) = a.as_chunks::<4>().0.iter().zip(b.as_chunks::<4>().0).fold(
        (0, 0),
        |(pixels, changed), (a, b)| {
            let difference: i32 = (0..3)
                .map(|channel| (i32::from(a[channel]) - i32::from(b[channel])).abs())
                .sum();
            (pixels + 1, changed + usize::from(difference > tolerance))
        },
    );
    changed as f64 / pixels as f64
}

/// Writes `rgba` under `RUSTY_BACKDROP_TEST_IMAGES` when it is set, to look
/// at.
fn keep(name: &str, rgba: &[u8]) {
    if let Some(directory) = std::env::var_os("RUSTY_BACKDROP_TEST_IMAGES") {
        let png = render_wgpu::encode_png(WIDTH, HEIGHT, rgba).expect("png");
        std::fs::write(
            std::path::Path::new(&directory).join(format!("{name}.png")),
            png,
        )
        .expect("write");
    }
}

/// The world camera sees 100 m; the world-scale range needs 10 km.
const NEAR_FIELD: f64 = 100.0;
const FAR_FIELD: f64 = 10_000.0;

#[test]
fn without_a_link_or_without_content_nothing_changes() {
    let mut harness = scene();
    let bare = look(&mut harness, [0.0, 1.0, 0.0], 0.0, NEAR_FIELD);
    harness.apply(vec![link([0.0; 3])]);
    assert_eq!(
        look(&mut harness, [0.0, 1.0, 0.0], 0.0, NEAR_FIELD),
        bare,
        "a link with nothing in the backdrop draws nothing"
    );
    assert_eq!(harness.renderer.gpu_readout().backdrop, None);

    let mut unlinked = scene();
    unlinked.apply(range(RenderLayer::Backdrop));
    assert_eq!(
        look(&mut unlinked, [0.0, 1.0, 0.0], 0.0, NEAR_FIELD),
        bare,
        "without a link the backdrop's parts draw nowhere, the world included"
    );
}

#[test]
fn the_backdrop_draws_beyond_the_far_plane_as_the_world_would_at_any_turn_and_walk() {
    for (position, yaw) in [
        ([0.0, 1.0, 0.0], 0.0),
        ([0.0, 1.0, 0.0], 25.0),
        ([0.0, 1.0, -100.0], 0.0),
        ([300.0, 40.0, -900.0], -20.0),
    ] {
        let mut world = scene();
        let bare = look(&mut world, position, yaw, FAR_FIELD);
        world.apply(range(RenderLayer::Scene));
        let expected = look(&mut world, position, yaw, FAR_FIELD);

        let mut backdrop = scene();
        backdrop.apply(range(RenderLayer::Backdrop));
        backdrop.apply(vec![link([0.0; 3])]);
        let drawn = look(&mut backdrop, position, yaw, NEAR_FIELD);
        let name = format!("walk-{}-{yaw}", position[2].abs());
        keep(&format!("{name}-world"), &expected);
        keep(&format!("{name}-backdrop"), &drawn);
        assert!(
            changed(&bare, &expected, 6) > 0.05,
            "the range is in view from {position:?} at {yaw}°"
        );
        let differs = changed(&expected, &drawn, 6);
        assert!(
            differs < 0.003,
            "from {position:?} at {yaw}° the backdrop draws as the world-scale range, fog included: {:.2}% differ",
            differs * 100.0
        );

        // Walking 100 m moves the backdrop's eye 0.1 units.
        let readout = backdrop
            .renderer
            .gpu_readout()
            .backdrop
            .expect("the backdrop drew");
        for (axis, value) in readout.eye.iter().enumerate() {
            let expected = (position[axis] / SCALE) as f32;
            assert!(
                (value - expected).abs() < 1e-5,
                "eye {:?} for {position:?}",
                readout.eye
            );
        }
        assert!(readout.near > 0.0 && readout.near < readout.far);
        assert!(
            backdrop
                .renderer
                .gpu_readout()
                .passes
                .iter()
                .any(|pass| pass.pass == "backdrop"),
            "the backdrop's pass reports its cost"
        );
    }
}

#[test]
fn world_geometry_always_covers_the_backdrop() {
    // A backdrop pane 2 m (world-equivalent) in front of the eye, across
    // the whole view, and a world wall 5 m away across the left half: the
    // wall is drawn exactly as without the pane, though the pane is nearer.
    let pane = vec![
        static_mesh(
            "mesh/pane",
            box_mesh([-0.05, -0.05, -0.0021], [0.05, 0.05, -0.002], |_| 0),
            "material/rock",
        ),
        placed(40, "mesh/pane", RenderLayer::Backdrop),
        link([0.0, 1.0, 0.0]),
    ];
    let wall = vec![
        RenderDiff::DefineMaterial {
            material: material("material/wall", [0.2, 0.7, 0.3, 1.0], None),
        },
        static_mesh(
            "mesh/wall",
            box_mesh([-20.0, -10.0, -5.5], [0.0, 10.0, -5.0], |_| 0),
            "material/wall",
        ),
        instance(41, None, "mesh/wall", transform([0.0; 3], 0.0, [1.0; 3])),
    ];
    let mut walled = scene();
    walled.apply(wall.clone());
    let bare = look(&mut walled, [0.0, 1.0, 0.0], 0.0, NEAR_FIELD);
    walled.apply(pane);
    let covered = look(&mut walled, [0.0, 1.0, 0.0], 0.0, NEAR_FIELD);
    keep("covered", &covered);
    let left = |rgba: &[u8]| -> Vec<u8> {
        rgba.as_chunks::<4>()
            .0
            .iter()
            .enumerate()
            .filter(|(index, _)| (*index as u32 % WIDTH) < WIDTH / 2 - 4)
            .flat_map(|(_, pixel)| *pixel)
            .collect()
    };
    assert_eq!(left(&covered), left(&bare), "the wall covers the pane");
    assert!(
        changed(&bare, &covered, 6) > 0.3,
        "the pane fills the open half"
    );
}

#[test]
fn an_origin_rebase_that_moves_the_camera_and_the_anchor_together_keeps_the_backdrop() {
    let render = |offset: [f64; 3]| {
        let mut harness = scene();
        harness.apply(range(RenderLayer::Backdrop));
        harness.apply(vec![link(offset)]);
        let image = look(
            &mut harness,
            [offset[0], 1.0 + offset[1], -100.0 + offset[2]],
            10.0,
            NEAR_FIELD,
        );
        (image, harness.renderer.gpu_readout().backdrop)
    };
    let (before, at) = render([0.0; 3]);
    let (after, rebased) = render([-2048.0, 0.0, 4096.0]);
    assert_eq!(at, rebased, "the backdrop camera is where it was");
    assert!(
        changed(&before, &after, 6) < 0.001,
        "and the backdrop draws as it did"
    );
}

#[test]
fn removing_the_link_removes_the_backdrop() {
    let mut harness = scene();
    let bare = look(&mut harness, [0.0, 1.0, 0.0], 0.0, NEAR_FIELD);
    harness.apply(range(RenderLayer::Backdrop));
    harness.apply(vec![link([0.0; 3])]);
    assert_ne!(look(&mut harness, [0.0, 1.0, 0.0], 0.0, NEAR_FIELD), bare);
    harness.apply(vec![RenderDiff::SetBackdrop {
        camera: None,
        backdrop: None,
    }]);
    assert_eq!(look(&mut harness, [0.0, 1.0, 0.0], 0.0, NEAR_FIELD), bare);
}

#[test]
fn invalid_links_are_refused_by_the_model() {
    let mut backdrop = BackdropDescriptor {
        anchor: [0.0; 3],
        origin: [0.0; 3],
        scale: 1000.0,
    };
    assert!(RenderDiff::SetBackdrop {
        camera: None,
        backdrop: Some(backdrop)
    }
    .validate()
    .is_ok());
    backdrop.scale = 0.0;
    assert!(RenderDiff::SetBackdrop {
        camera: None,
        backdrop: Some(backdrop)
    }
    .validate()
    .is_err());
    backdrop.scale = 1.0;
    backdrop.anchor[1] = f64::NAN;
    assert!(RenderDiff::SetBackdrop {
        camera: None,
        backdrop: Some(backdrop)
    }
    .validate()
    .is_err());
}

#[test]
fn each_view_takes_its_own_cameras_link() {
    // Two cameras side by side looking the same way: the left one's view has
    // a 1:1000 link, the right one's none, so only the left draws the range.
    let mut harness = scene();
    harness.apply(range(RenderLayer::Backdrop));
    let mut left = camera([0.0, 1.0, 0.0], 0.0, 3.0);
    left.id = "left".to_owned();
    let mut right = left.clone();
    right.id = "right".to_owned();
    let view = |id: &str, x: f64| RendererCompositionView {
        id: id.to_owned(),
        camera_id: id.to_owned(),
        target: RendererViewTarget::Primary,
        viewport: RendererViewport {
            x,
            y: 0.0,
            width: 0.5,
            height: 1.0,
        },
        order: if x == 0.0 { 0 } else { 1 },
        viewport_anchor: None,
    };
    harness.renderer.set_view_composition(
        &RendererViewComposition {
            cameras: vec![left, right],
            targets: Vec::new(),
            views: vec![view("left", 0.0), view("right", 0.5)],
            presentations: Vec::new(),
        },
        0.0,
    );
    let draw = |harness: &mut Harness| {
        harness
            .renderer
            .render_view_composition(&harness.target, 0.0);
        harness.target.read_rgba(&harness.gpu)
    };
    let bare = draw(&mut harness);
    harness.apply(vec![RenderDiff::SetBackdrop {
        camera: Some("left".to_owned()),
        backdrop: Some(BackdropDescriptor {
            anchor: [0.0; 3],
            origin: [0.0; 3],
            scale: SCALE,
        }),
    }]);
    let linked = draw(&mut harness);
    keep("views", &linked);
    let half = |rgba: &[u8], right: bool| -> Vec<u8> {
        rgba.as_chunks::<4>()
            .0
            .iter()
            .enumerate()
            .filter(|(index, _)| ((*index as u32 % WIDTH) >= WIDTH / 2) == right)
            .flat_map(|(_, pixel)| *pixel)
            .collect()
    };
    assert!(
        changed(&half(&bare, false), &half(&linked, false), 6) > 0.02,
        "the left camera's view draws the range"
    );
    assert_eq!(
        half(&bare, true),
        half(&linked, true),
        "the right camera's view, with no link, draws none"
    );
    // Every view's link (no camera) reaches the right view too; the left
    // keeps its own, so a link at another scale moves only the right.
    harness.apply(vec![RenderDiff::SetBackdrop {
        camera: None,
        backdrop: Some(BackdropDescriptor {
            anchor: [0.0; 3],
            origin: [0.0; 3],
            scale: SCALE * 2.0,
        }),
    }]);
    let both = draw(&mut harness);
    assert_eq!(
        half(&both, false),
        half(&linked, false),
        "the left keeps its own link"
    );
    assert!(
        changed(&half(&bare, true), &half(&both, true), 6) > 0.01,
        "the right takes every view's link"
    );
}

#[test]
fn backdrop_particles_draw_only_in_the_backdrop_and_the_world_covers_them() {
    use render_presentation::{
        ParticleAnchor, ParticleColorKey, ParticleEmitterDescriptor, ParticleProjectionOp,
        ParticleScalarKey, ParticleSizeMode, ParticleVisual, PresentationFrameDiff, PresentationOp,
        PresentationOpMeta,
    };
    let no_entities: &dyn Fn(u64) -> Option<[f32; 3]> = &|_| None;
    // A still cloud of cubes 300 m across (0.3 units) 4 km out on each side,
    // as a backdrop emitter: the only thing in the backdrop.
    let plume = |x: f32, seed: u64| ParticleEmitterDescriptor {
        anchor: ParticleAnchor::World {
            position: [x, 0.3, -4.0],
        },
        visual: ParticleVisual::Cube,
        size_mode: ParticleSizeMode::World,
        blend: Default::default(),
        softness_metres: 0.0,
        rate_per_second: 0.0,
        burst_count: 16,
        lifetime_seconds: [100.0, 100.0],
        velocity_min: [0.0; 3],
        velocity_max: [0.0; 3],
        acceleration: [0.0; 3],
        size_curve: vec![
            ParticleScalarKey {
                age: 0.0,
                value: 0.3,
            },
            ParticleScalarKey {
                age: 1.0,
                value: 0.3,
            },
        ],
        color_curve: vec![
            ParticleColorKey {
                age: 0.0,
                color: [0.2, 0.2, 0.2, 1.0],
            },
            ParticleColorKey {
                age: 1.0,
                color: [0.2, 0.2, 0.2, 1.0],
            },
        ],
        flipbook_frames_per_second: 0.0,
        seed,
        max_particles: 64,
        visible: true,
        collision: None,
        backdrop: true,
    };
    let wall = vec![
        RenderDiff::DefineMaterial {
            material: material("material/wall", [0.2, 0.7, 0.3, 1.0], None),
        },
        static_mesh(
            "mesh/wall",
            box_mesh([-20.0, -10.0, -5.5], [0.0, 10.0, -5.0], |_| 0),
            "material/wall",
        ),
        instance(41, None, "mesh/wall", transform([0.0; 3], 0.0, [1.0; 3])),
    ];
    let mut harness = scene();
    harness.apply(wall);
    let bare = look(&mut harness, [0.0, 1.0, 0.0], 0.0, NEAR_FIELD);
    let ops: Vec<PresentationOp> = [plume(-0.6, 3), plume(0.6, 5)]
        .into_iter()
        .enumerate()
        .map(|(sequence, descriptor)| PresentationOp::Particle {
            meta: PresentationOpMeta::new(sequence as u32),
            op: ParticleProjectionOp::Emit {
                signal_id: format!("plume-{sequence}"),
                descriptor,
            },
        })
        .collect();
    let issues = harness.renderer.apply_presentation(
        &PresentationFrameDiff::try_from_ops(ops).expect("frame"),
        &harness.resources,
        no_entities,
    );
    assert!(issues.is_empty(), "{issues:?}");
    harness.renderer.advance_effects(0.01, no_entities);
    assert_eq!(
        look(&mut harness, [0.0, 1.0, 0.0], 0.0, NEAR_FIELD),
        bare,
        "unlinked, backdrop particles draw nowhere: not in the world"
    );
    harness.apply(vec![link([0.0; 3])]);
    let linked = look(&mut harness, [0.0, 1.0, 0.0], 0.0, NEAR_FIELD);
    keep("particles", &linked);
    let half = |rgba: &[u8], right: bool| -> Vec<u8> {
        rgba.as_chunks::<4>()
            .0
            .iter()
            .enumerate()
            .filter(|(index, _)| {
                let x = *index as u32 % WIDTH;
                if right {
                    x > WIDTH / 2 + 4
                } else {
                    x < WIDTH / 2 - 4
                }
            })
            .flat_map(|(_, pixel)| *pixel)
            .collect()
    };
    assert!(
        changed(&half(&bare, true), &half(&linked, true), 6) > 0.005,
        "linked, the open side shows the plume"
    );
    assert_eq!(
        half(&linked, false),
        half(&bare, false),
        "the wall covers the other"
    );
    assert!(
        harness.renderer.gpu_readout().backdrop.is_some(),
        "a particle-only backdrop draws"
    );
}
