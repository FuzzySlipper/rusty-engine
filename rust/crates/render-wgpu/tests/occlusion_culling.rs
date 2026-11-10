//! Occlusion culling of opaque parts (#10698) on a real headless device: a
//! town of crates behind a wall draws the picture it draws without the
//! test, frame by frame while the eye walks past the wall's end, turns
//! fast and the wall goes away, and the cull reports what it left undrawn.
//! Needs a wgpu adapter: CI uses `WGPU_BACKEND=vulkan` with llvmpipe.

mod support;

use render_host_contracts::RendererCompositionCamera;
use render_model::*;
use render_wgpu::RendererOptions;
use support::*;

const PLASTER: &str = "material/occlusion-plaster";
const SCREEN: &str = "material/occlusion-screen";
/// Crate meshes, each in its own colour, so a missing crate shows.
const CRATES: [(&str, &str, [f32; 4]); 4] = [
    (
        "static-mesh/occlusion-crate-a",
        "material/occlusion-a",
        [0.8, 0.3, 0.2, 1.0],
    ),
    (
        "static-mesh/occlusion-crate-b",
        "material/occlusion-b",
        [0.2, 0.7, 0.3, 1.0],
    ),
    (
        "static-mesh/occlusion-crate-c",
        "material/occlusion-c",
        [0.25, 0.35, 0.9, 1.0],
    ),
    (
        "static-mesh/occlusion-crate-d",
        "material/occlusion-d",
        [0.9, 0.8, 0.2, 1.0],
    ),
];
const WALL: &str = "static-mesh/occlusion-wall";
const FLOOR: &str = "static-mesh/occlusion-floor";
const WALL_HANDLE: u64 = 20;
/// Crates in rows behind the wall, all hidden from the start.
const HIDDEN_ROWS: u64 = 4;
const HIDDEN_COLUMNS: u64 = 6;
/// Crates in front of the wall, seen from the start.
const FRONT: u64 = 3;

fn options(occlusion: bool) -> RendererOptions {
    RendererOptions {
        default_world_lights: false,
        gpu_culling: true,
        occlusion_culling: occlusion,
        ..RendererOptions::default()
    }
}

/// A floor, a wall 12 m wide and 4 m tall 6 m ahead of the start, a grid
/// of crates behind it and a few before it, in four colours.
fn town(wall_material: &str) -> Vec<RenderDiff> {
    let mut screen = material(SCREEN, [0.5, 0.5, 0.55, 1.0], None);
    screen.alpha_mode = MaterialAlphaModeDescriptor::Mask { cutoff: 0.5 };
    let mut ops = vec![
        RenderDiff::DefineMaterial {
            material: material(PLASTER, [0.6, 0.55, 0.5, 1.0], None),
        },
        RenderDiff::DefineMaterial { material: screen },
        static_mesh(
            FLOOR,
            box_mesh([-40.0, -0.2, -40.0], [40.0, 0.0, 40.0], |_| 0),
            PLASTER,
        ),
        static_mesh(
            WALL,
            box_mesh([-6.0, 0.0, -0.2], [6.0, 4.0, 0.2], |_| 0),
            wall_material,
        ),
        instance(10, None, FLOOR, transform([0.0; 3], 0.0, [1.0; 3])),
        instance(
            WALL_HANDLE,
            None,
            WALL,
            transform([0.0, 0.0, -6.0], 0.0, [1.0; 3]),
        ),
    ];
    for (mesh, id, color) in CRATES {
        ops.push(RenderDiff::DefineMaterial {
            material: material(id, color, None),
        });
        ops.push(static_mesh(
            mesh,
            box_mesh([-0.5, 0.0, -0.5], [0.5, 1.0, 0.5], |_| 0),
            id,
        ));
    }
    let mut crate_at = |handle: u64, at: [f32; 3]| {
        let mesh = CRATES[handle as usize % CRATES.len()].0;
        ops.push(instance(handle, None, mesh, transform(at, 15.0, [1.0; 3])));
    };
    for row in 0..HIDDEN_ROWS {
        for column in 0..HIDDEN_COLUMNS {
            crate_at(
                100 + row * HIDDEN_COLUMNS + column,
                [column as f32 * 1.8 - 4.5, 0.0, -9.0 - row as f32 * 2.5],
            );
        }
    }
    for index in 0..FRONT {
        crate_at(200 + index, [index as f32 * 2.5 - 2.5, 0.0, -3.0]);
    }
    ops
}

fn eye(position: [f64; 3], yaw: f64) -> RendererCompositionCamera {
    camera(position, yaw, -4.0)
}

/// The share of pixels differing by more than `tolerance` levels.
fn differing(a: &[u8], b: &[u8], tolerance: u8) -> f64 {
    let count = a
        .as_chunks::<4>()
        .0
        .iter()
        .zip(b.as_chunks::<4>().0)
        .filter(|(a, b)| {
            a.iter()
                .zip(b.iter())
                .any(|(a, b)| a.abs_diff(*b) > tolerance)
        })
        .count();
    count as f64 / f64::from(WIDTH * HEIGHT)
}

/// Each eye's frame from a harness with occlusion as asked, the cull's
/// readout after each.
fn walk(
    occlusion: bool,
    ops: &[RenderDiff],
    eyes: &[RendererCompositionCamera],
) -> (Vec<Vec<u8>>, Harness) {
    let mut harness = Harness::new(options(occlusion));
    harness.apply(ops.to_vec());
    let frames = eyes.iter().map(|eye| harness.render(eye).1).collect();
    (frames, harness)
}

#[test]
fn crates_behind_a_wall_are_left_undrawn_and_the_picture_stays() {
    let ops = town(PLASTER);
    let start = vec![eye([0.0, 1.6, 0.0], 0.0); 6];
    let (expected, _) = walk(false, &ops, &start);
    let (culled, harness) = walk(true, &ops, &start);
    let readout = harness.renderer.gpu_readout();
    if let Some(reason) = &readout.gpu_culling.refused {
        eprintln!("GPU culling refused on this device: {reason}");
        assert!(!readout.occlusion.parts);
        return;
    }
    assert!(readout.occlusion.enabled && readout.occlusion.parts);
    let culling = &readout.gpu_culling;
    eprintln!("{culling:?}");
    assert!(culling.occlusion);
    assert_eq!(
        culling.occluded,
        (HIDDEN_ROWS * HIDDEN_COLUMNS) as u32,
        "every crate behind the wall is left undrawn"
    );
    // The floor, the wall and the crates before it.
    assert_eq!(culling.visible, FRONT as u32 + 2);
    assert_eq!(culling.occluders, FRONT as u32 + 2);
    for (frame, (a, b)) in expected.iter().zip(&culled).enumerate() {
        assert_eq!(
            differing(a, b, 0),
            0.0,
            "frame {frame}: the picture differs with occlusion culling"
        );
    }
    // Each step reports its own GPU time.
    for name in [
        "occluder-select",
        "occlusion-depth",
        "occlusion",
        "gpu-cull",
    ] {
        let timing = readout
            .passes
            .iter()
            .find(|pass| pass.pass == name)
            .unwrap_or_else(|| panic!("{name} is a timed pass"));
        if readout.timestamps {
            assert!(
                timing.timed_frames > 0 && timing.median_gpu_ms.is_finite(),
                "{name}: {timing:?}"
            );
        }
    }
}

#[test]
fn walking_past_the_wall_and_turning_fast_never_shows_a_hole() {
    // Sideways past the wall's end in long steps (from 7 m across, the
    // crates behind it show past its end), a fast turn toward the hidden
    // crates and away, then back: every crate that comes into view draws in
    // the frame it does.
    let mut eyes = Vec::new();
    for step in 0..8 {
        eyes.push(eye([f64::from(step) * 1.5, 1.6, 0.0], 0.0));
    }
    for yaw in [-35.0, 40.0, 180.0, -20.0, 0.0] {
        eyes.push(eye([10.5, 1.6, -2.0], yaw));
    }
    for step in (0..8).rev() {
        eyes.push(eye([f64::from(step) * 1.5, 1.6, -1.0], 20.0));
    }
    let ops = town(PLASTER);
    let (expected, _) = walk(false, &ops, &eyes);
    let (culled, harness) = walk(true, &ops, &eyes);
    if harness.renderer.gpu_readout().gpu_culling.refused.is_some() {
        return;
    }
    for (frame, (a, b)) in expected.iter().zip(&culled).enumerate() {
        assert_eq!(
            differing(a, b, 0),
            0.0,
            "frame {frame} ({:?}): a crate is missing or extra with occlusion culling",
            eyes[frame].pose
        );
    }
}

#[test]
fn hidden_crates_draw_in_the_frame_their_wall_goes() {
    let ops = town(PLASTER);
    let view = eye([0.0, 1.6, 0.0], 0.0);
    let (mut harness, mut reference) = (Harness::new(options(true)), Harness::new(options(false)));
    harness.apply(ops.clone());
    reference.apply(ops);
    for _ in 0..3 {
        harness.render(&view);
    }
    let gone = vec![RenderDiff::Update {
        handle: RenderHandle::new(WALL_HANDLE),
        transform: None,
        material: None,
        visible: Some(false),
        metadata: None,
    }];
    harness.apply(gone.clone());
    reference.apply(gone);
    let (culled, expected) = (harness.render(&view).1, reference.render(&view).1);
    assert_eq!(differing(&expected, &culled, 0), 0.0);
}

#[test]
fn a_masked_wall_hides_no_parts() {
    // A masked material keeps the samples its alpha covers, which the world
    // pass may share out differently from a depth pass: it never occludes.
    let ops = town(SCREEN);
    let start = vec![eye([0.0, 1.6, 0.0], 0.0); 3];
    let (expected, _) = walk(false, &ops, &start);
    let (culled, harness) = walk(true, &ops, &start);
    let culling = harness.renderer.gpu_readout().gpu_culling;
    if culling.refused.is_some() {
        return;
    }
    assert_eq!(culling.occluded, 0);
    // Every crate is in view, so every one occludes next frame; the wall
    // never does.
    assert_eq!(
        culling.occluders,
        (HIDDEN_ROWS * HIDDEN_COLUMNS + FRONT) as u32 + 1,
        "the floor and the crates"
    );
    for (a, b) in expected.iter().zip(&culled) {
        assert_eq!(differing(a, b, 0), 0.0);
    }
}
