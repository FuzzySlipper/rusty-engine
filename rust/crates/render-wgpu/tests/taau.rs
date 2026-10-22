//! The temporal upscale stage, TAAU (`upscale.rs`): `Antialiasing::Temporal`
//! jitters the primary views and gathers their frames into a history at
//! output resolution. Its edges come close to 4× MSAA's at native scale,
//! and closer to the native picture than bilinear at half scale; a still
//! view settles; a moved part leaves no trail; a software adapter refuses
//! it and draws without it. Needs a wgpu adapter: CI uses
//! `WGPU_BACKEND=vulkan` with llvmpipe.

mod support;

use std::path::PathBuf;

use asset_import::{import_animated_glb_asset, ImportContext, SourceUri};
use render_host_contracts::RendererCompositionCamera;
use render_model::*;
use render_wgpu::{
    FrameResolution, FrameStats, OffscreenTarget, RendererOptions, SettingRefusal, UpscaleMethod,
};
use support::*;

const MATERIAL: &str = "material/taau-plaster";
const CRATE: &str = "static-mesh/taau-crate";
const FLOOR: &str = "static-mesh/taau-floor";
/// The crate the ghosting test moves.
const MOVED: u64 = 102;

fn scene() -> Harness {
    let mut harness = Harness::new(RendererOptions::default());
    let mut ops = vec![
        RenderDiff::DefineMaterial {
            material: material(MATERIAL, [0.8, 0.75, 0.7, 1.0], None),
        },
        static_mesh(
            FLOOR,
            box_mesh([-8.0, -0.2, -8.0], [8.0, 0.0, 8.0], |_| 0),
            MATERIAL,
        ),
        static_mesh(
            CRATE,
            box_mesh([-0.5, 0.0, -0.5], [0.5, 1.0, 0.5], |_| 0),
            MATERIAL,
        ),
        instance(10, None, FLOOR, transform([0.0; 3], 0.0, [1.0; 3])),
        RenderDiff::SetBackgroundColor {
            color: [0.05, 0.1, 0.3, 1.0],
        },
    ];
    for index in 0..6_u64 {
        ops.push(instance(
            100 + index,
            None,
            CRATE,
            transform(
                [index as f32 * 1.6 - 4.0, 0.0, -2.0 - (index % 2) as f32],
                (index * 25) as f32 + 10.0,
                [1.0; 3],
            ),
        ));
    }
    harness.apply(ops);
    harness
}

fn view() -> RendererCompositionCamera {
    camera([0.0, 2.0, 5.0], 0.0, -15.0)
}

/// Draw with `antialiasing` at `render_scale` on a target sized as a host
/// sizes it, `frames` times: the last frame.
fn settle(
    harness: &mut Harness,
    antialiasing: Antialiasing,
    render_scale: f32,
    frames: usize,
) -> (FrameStats, Vec<u8>) {
    harness.apply(vec![RenderDiff::SetRendererSettings {
        settings: RendererSettingsDescriptor {
            antialiasing,
            render_scale,
            ..RendererSettingsDescriptor::DEFAULT
        },
    }]);
    let samples = harness.renderer.samples();
    harness.target = OffscreenTarget::new(&harness.gpu, WIDTH, HEIGHT, samples);
    let mut last = harness.render(&view());
    for _ in 1..frames {
        last = harness.render(&view());
    }
    last
}

/// Mean absolute difference per channel over the pixels where `reference`
/// has an edge (a neighbour differs by more than 24 levels): where
/// anti-aliasing shows.
fn edge_difference(reference: &[u8], picture: &[u8]) -> f64 {
    let at =
        |rgba: &[u8], x: u32, y: u32, c: usize| f64::from(rgba[((y * WIDTH + x) * 4) as usize + c]);
    let (mut sum, mut count) = (0.0, 0usize);
    for y in 1..HEIGHT - 1 {
        for x in 1..WIDTH - 1 {
            let edge = (0..3).any(|c| {
                (at(reference, x + 1, y, c) - at(reference, x, y, c)).abs() > 24.0
                    || (at(reference, x, y + 1, c) - at(reference, x, y, c)).abs() > 24.0
            });
            if edge {
                for c in 0..3 {
                    sum += (at(reference, x, y, c) - at(picture, x, y, c)).abs();
                }
                count += 3;
            }
        }
    }
    assert!(count > 300, "the scene has edges: {count}");
    sum / count as f64
}

fn mean_difference(a: &[u8], b: &[u8]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(a, b)| f64::from(a.abs_diff(*b)))
        .sum::<f64>()
        / a.len() as f64
}

#[test]
fn native_taa_smooths_edges_toward_msaa_and_settles() {
    let mut harness = scene();
    if harness.gpu.is_software() {
        return;
    }
    let (_, msaa) = settle(&mut harness, Antialiasing::Msaa4, 1.0, 2);
    let (_, aliased) = settle(&mut harness, Antialiasing::Off, 1.0, 2);
    let (stats, taa) = settle(&mut harness, Antialiasing::Temporal, 1.0, 40);
    assert_eq!(stats.upscale, Some(UpscaleMethod::Taau));
    assert_eq!(harness.renderer.samples(), 1, "TAA in MSAA's place");
    let motion = stats.motion.expect("the stage draws motion");
    assert!(motion.jitter_phase.is_some() && !motion.history_reset);
    let (off, temporal) = (
        edge_difference(&msaa, &aliased),
        edge_difference(&msaa, &taa),
    );
    assert!(
        temporal < off * 0.75,
        "TAA's edges ({temporal:.2}) nearer 4x MSAA's than none ({off:.2})"
    );
    // A still view settles: the next frame barely differs.
    let (_, next) = harness.render(&view());
    let shimmer = mean_difference(&taa, &next);
    assert!(shimmer < 0.75, "a still view settles: {shimmer:.3}");
}

#[test]
fn half_scale_taau_is_nearer_the_native_picture_than_bilinear() {
    let mut harness = scene();
    if harness.gpu.is_software() {
        return;
    }
    let (_, native) = settle(&mut harness, Antialiasing::Msaa4, 1.0, 2);
    let (_, bilinear) = settle(&mut harness, Antialiasing::Msaa4, 0.5, 2);
    let (stats, taau) = settle(&mut harness, Antialiasing::Temporal, 0.5, 64);
    assert_eq!(stats.upscale, Some(UpscaleMethod::Taau));
    assert_eq!(
        stats.resolution,
        FrameResolution {
            render: (WIDTH / 2, HEIGHT / 2),
            output: (WIDTH, HEIGHT),
        }
    );
    let readout = harness.renderer.gpu_readout();
    assert_eq!(readout.upscale.method, Some(UpscaleMethod::Taau));
    assert_eq!(readout.upscale.resolution, stats.resolution);
    let (spatial, temporal) = (
        edge_difference(&native, &bilinear),
        edge_difference(&native, &taau),
    );
    assert!(
        temporal < spatial * 0.85,
        "TAAU's edges ({temporal:.2}) nearer native than bilinear's ({spatial:.2})"
    );
}

#[test]
fn a_moved_part_leaves_no_trail() {
    let mut harness = scene();
    if harness.gpu.is_software() {
        return;
    }
    settle(&mut harness, Antialiasing::Temporal, 0.67, 32);
    harness.apply(vec![RenderDiff::Update {
        handle: RenderHandle::new(MOVED),
        transform: Some(transform([-0.8, 0.0, -2.0], 35.0, [1.0; 3])),
        material: None,
        visible: Some(false),
        metadata: None,
    }]);
    // The crate is gone: two frames later its pixels show what lies behind.
    harness.render(&view());
    let (_, after) = harness.render(&view());
    let mut fresh = scene();
    fresh.apply(vec![RenderDiff::Update {
        handle: RenderHandle::new(MOVED),
        transform: None,
        material: None,
        visible: Some(false),
        metadata: None,
    }]);
    let (_, without) = settle(&mut fresh, Antialiasing::Temporal, 0.67, 32);
    let trail = mean_difference(&after, &without);
    assert!(trail < 2.0, "a hidden crate's trail: {trail:.3}");
}

#[test]
fn a_software_adapter_refuses_the_temporal_stage_and_still_draws() {
    let mut harness = scene();
    let (stats, picture) = settle(&mut harness, Antialiasing::Temporal, 0.5, 2);
    let readout = harness.renderer.settings_readout();
    assert_eq!(readout.requested.antialiasing, Antialiasing::Temporal);
    assert!(picture.chunks(4).any(|p| p[0] > 100), "the crates drew");
    if harness.gpu.is_software() {
        assert_eq!(readout.antialiasing, Some(SettingRefusal::SoftwareAdapter));
        assert_eq!(readout.effective.antialiasing, Antialiasing::Off);
        assert_eq!(stats.upscale, Some(UpscaleMethod::Bilinear));
        assert_eq!(stats.motion, None, "no jitter or motion without the stage");
    } else {
        assert_eq!(readout.antialiasing, None);
        assert_eq!(readout.effective.antialiasing, Antialiasing::Temporal);
        assert_eq!(stats.upscale, Some(UpscaleMethod::Taau));
    }
}

/// The joint attachment fixture's body, admitted as the runtime admits it.
fn admit_body(harness: &mut Harness) -> AnimatedMeshAsset {
    let path = "csharp-joint-attachments/content/body.glb";
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures");
    let bytes = std::fs::read(root.join(path)).expect("the fixture body");
    let imported = import_animated_glb_asset(
        &SourceUri::RelativePath(path.to_owned()),
        &bytes,
        &ImportContext::default(),
    )
    .assets
    .expect("the body is admitted");
    let hash = imported.animated_mesh.content_hash.clone().expect("hash");
    harness.resources.insert(
        &format!(
            "animated-mesh-resource/{}",
            hash.trim_start_matches("sha256:")
        ),
        bytes,
    );
    imported.animated_mesh
}

/// The body running along x at `speed` metres a second, at Engine time
/// `time`.
fn place_runner(harness: &mut Harness, time: f64, speed: f32) {
    harness.at(time);
    harness.apply(vec![RenderDiff::Update {
        handle: RenderHandle::new(RUNNER),
        transform: Some(Transform {
            translation: [-1.5 + speed * time as f32, 0.0, 0.0],
            rotation: [
                0.0,
                std::f32::consts::FRAC_1_SQRT_2,
                0.0,
                std::f32::consts::FRAC_1_SQRT_2,
            ],
            scale: [1.0; 3],
        }),
        material: None,
        visible: None,
        metadata: None,
    }]);
}

const RUNNER: u64 = 500;

/// Beside the runner's path, framing the whole body.
fn runner_view() -> RendererCompositionCamera {
    camera([0.0, 1.0, 5.0], 0.0, -2.0)
}

#[test]
fn a_running_skinned_character_leaves_no_trail() {
    let mut harness = scene();
    if harness.gpu.is_software() {
        return;
    }
    let body = admit_body(&mut harness);
    harness.apply(vec![
        RenderDiff::DefineAnimatedMesh {
            asset: body.clone(),
        },
        RenderDiff::CreateAnimatedMeshInstance {
            handle: RenderHandle::new(RUNNER),
            parent: None,
            instance: AnimatedMeshInstanceDescriptor {
                playback_fade: None,
                inspection: AnimatedMeshInspection::default(),
                asset: body.asset.clone(),
                transform: Transform::IDENTITY,
                visible: true,
                material_overrides: Vec::new(),
                playback: Some(AnimatedMeshPlaybackCommand::Play {
                    clip: "run".to_owned(),
                    r#loop: AnimationLoopMode::Repeat,
                    speed: 1.0,
                    weight: 1.0,
                    restart: true,
                    fade_seconds: None,
                    start_offset_seconds: None,
                    start_paused: false,
                }),
                metadata: RenderMetadata::default(),
                layer: RenderLayer::Scene,
                shadow_casting: Default::default(),
            },
        },
    ]);
    const STEP: f64 = 1.0 / 60.0;
    const SPEED: f32 = 1.5;
    harness.apply(vec![RenderDiff::SetRendererSettings {
        settings: RendererSettingsDescriptor {
            antialiasing: Antialiasing::Temporal,
            render_scale: 0.67,
            ..RendererSettingsDescriptor::DEFAULT
        },
    }]);
    harness.target = OffscreenTarget::new(&harness.gpu, WIDTH, HEIGHT, harness.renderer.samples());
    let view = runner_view();
    let mut last = Vec::new();
    for frame in 1..=48 {
        place_runner(&mut harness, f64::from(frame) * STEP, SPEED);
        last = harness.render(&view).1;
    }
    // The same moment drawn afresh, with 4x MSAA and no history.
    let reference = {
        let mut renderer = render_wgpu::Renderer::new(&harness.gpu, RendererOptions::default());
        let issues = renderer.apply(&harness.world.snapshot().frame, &harness.resources);
        assert!(issues.is_empty(), "{issues:?}");
        let msaa = RenderFrameDiff {
            publication: None,
            ops: vec![RenderDiff::SetRendererSettings {
                settings: RendererSettingsDescriptor::DEFAULT,
            }],
        };
        assert!(renderer.apply(&msaa, &harness.resources).is_empty());
        let target = OffscreenTarget::new(&harness.gpu, WIDTH, HEIGHT, 4);
        renderer.render_offscreen(&view, &target);
        target.read_rgba(&harness.gpu)
    };
    let difference = mean_difference(&last, &reference);
    assert!(
        difference < 3.0,
        "the runner against a fresh frame: {difference:.3}"
    );
}

/// A still crate before a striped wall sliding across the view, and a
/// viewmodel cube in the lower right corner, `shown` or not, drawn with TAAU
/// for `frames` frames: the last frame.
fn crate_before_a_sliding_wall(shown: bool, frames: u32) -> Vec<u8> {
    const WALL: u64 = 700;
    let mut harness = scene();
    let mut ops = vec![
        RenderDiff::DefineMaterial {
            material: material("material/taau-stripe", [0.95, 0.9, 0.2, 1.0], None),
        },
        static_mesh(
            "static-mesh/taau-stripe",
            box_mesh([-0.1, 0.0, -0.1], [0.1, 4.0, 0.1], |_| 0),
            "material/taau-stripe",
        ),
        group(WALL, None, transform([0.0, 0.0, -6.0], 0.0, [1.0; 3])),
        instance(
            600,
            None,
            CRATE,
            transform([-0.6, 1.0, 1.5], 20.0, [1.0; 3]),
        ),
        RenderDiff::CreateStaticMeshInstance {
            handle: RenderHandle::new(601),
            parent: None,
            instance: StaticMeshInstanceDescriptor {
                asset: CRATE.to_owned(),
                transform: transform([0.35, -0.3, -1.0], 30.0, [0.15; 3]),
                visible: shown,
                material_overrides: Vec::new(),
                metadata: RenderMetadata::default(),
                layer: RenderLayer::Viewmodel,
                shadow_casting: Default::default(),
            },
        },
        RenderDiff::SetRendererSettings {
            settings: RendererSettingsDescriptor {
                antialiasing: Antialiasing::Temporal,
                render_scale: 0.67,
                ..RendererSettingsDescriptor::DEFAULT
            },
        },
    ];
    for stripe in 0..24_u64 {
        ops.push(instance(
            610 + stripe,
            Some(WALL),
            "static-mesh/taau-stripe",
            transform([stripe as f32 * 0.5 - 6.0, 0.0, 0.0], 0.0, [1.0; 3]),
        ));
    }
    harness.apply(ops);
    harness.target = OffscreenTarget::new(&harness.gpu, WIDTH, HEIGHT, harness.renderer.samples());
    let view = camera([0.0, 1.5, 5.0], 0.0, 0.0);
    let mut last = Vec::new();
    for frame in 0..frames {
        harness.apply(vec![RenderDiff::Update {
            handle: RenderHandle::new(WALL),
            transform: Some(transform([frame as f32 * 0.4, 0.0, -6.0], 0.0, [1.0; 3])),
            material: None,
            visible: None,
            metadata: None,
        }]);
        last = harness.render(&view).1;
    }
    last
}

/// The viewmodel draws against a depth of its own, so the world's depth
/// still dilates the world's motion: a still crate's edges keep their own
/// (no) motion over the wall sliding behind them whether a viewmodel draws
/// or not, and the picture away from the viewmodel is the same.
#[test]
fn a_viewmodel_leaves_the_worlds_depth_aware_dilation_alone() {
    if scene().gpu.is_software() {
        return;
    }
    let without = crate_before_a_sliding_wall(false, 24);
    let with = crate_before_a_sliding_wall(true, 24);
    // Away from the viewmodel, in the lower right quarter.
    let mut differing = 0;
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            if x >= WIDTH / 2 && y >= HEIGHT / 2 {
                continue;
            }
            let at = ((y * WIDTH + x) * 4) as usize;
            if (0..3).any(|c| without[at + c].abs_diff(with[at + c]) > 6) {
                differing += 1;
            }
        }
    }
    assert!(
        differing <= 4,
        "{differing} pixels away from the viewmodel differ with it drawn"
    );
}
