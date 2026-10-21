//! Depth order far from the origin: the world pass places each vertex
//! relative to the camera, so two surfaces a few millimetres apart keep their
//! order kilometres from the origin.

mod common;

use common::*;
use render_host_contracts::{
    RendererCameraPose, RendererCameraProjection, RendererCompositionCamera,
};
use render_model::*;
use render_wgpu::RendererOptions;

/// The scene stands this far below the origin, as an underground level might.
const DEPTH_BELOW_ORIGIN: f32 = 2026.0;
/// How far the dark wall stands behind the emissive disc.
const WALL_BEHIND: f32 = 0.0025;
/// The disc's radius, centre and distance in front of the camera (+X).
const DISC_RADIUS: f32 = 1.8;
const DISC_X: f32 = 47.5;
const DISC_CENTRE: [f32; 2] = [2.5, -3.0];
const CAMERA: [f64; 3] = [30.5, -0.4, 0.0];

/// A disc in the plane x = `x`, facing -X, as a triangle fan.
fn disc(x: f32, radius: f32, segments: u32) -> MeshPayloadDescriptor {
    let mut positions = vec![x, 0.0, 0.0];
    for segment in 0..segments {
        let angle = std::f32::consts::TAU * segment as f32 / segments as f32;
        positions.extend_from_slice(&[x, radius * angle.sin(), radius * angle.cos()]);
    }
    let count = segments + 1;
    let indices = (1..count)
        .flat_map(|i| [0, i, i % (count - 1) + 1])
        .collect();
    payload(positions, [-1.0, 0.0, 0.0].repeat(count as usize), indices)
}

/// A square in the plane x = `x`, facing -X, as two triangles.
fn square(x: f32, half: f32) -> MeshPayloadDescriptor {
    payload(
        vec![
            x, -half, -half, x, half, -half, x, half, half, x, -half, half,
        ],
        [-1.0, 0.0, 0.0].repeat(4),
        vec![0, 1, 2, 0, 2, 3],
    )
}

fn material(ops: &mut [RenderDiff], emission: [f32; 3]) {
    if let RenderDiff::DefineMaterial { material } = &mut ops[0] {
        material.emission_color = emission;
        material.emission_intensity = 2.0;
        material.double_sided = true;
    }
}

fn placed(handle: u64, mesh: &str, translation: [f32; 3], scale: [f32; 3]) -> RenderDiff {
    let mut op = instance(handle, None, mesh, transform(translation, 0.0, 1.0));
    if let RenderDiff::CreateStaticMeshInstance { instance, .. } = &mut op {
        instance.transform.scale = scale;
    }
    op
}

/// A large emissive disc 18 m away and 2 km below the origin, a dark wall
/// 2.5 mm behind it and a pillar near the camera hiding part of it: the disc
/// draws whole and even wherever the pillar leaves it in view, as it does
/// with no wall behind it at all.
#[test]
fn an_emissive_disc_holds_over_a_wall_just_behind_it_far_from_the_origin() {
    let render = |wall: bool| -> Vec<u8> {
        let mut harness = Harness::new(RendererOptions {
            default_world_lights: false,
            ..RendererOptions::default()
        });
        let mut ops = vec![RenderDiff::SetBackgroundColor {
            color: [0.0, 0.0, 0.0, 1.0],
        }];
        let mut glow = coloured_mesh("disc", disc(0.0, DISC_RADIUS, 48), [0.0, 0.0, 0.0, 1.0]);
        material(&mut glow, [0.3, 0.9, 1.0]);
        ops.extend(glow);
        let mut back = coloured_mesh("wall", square(0.0, 4.0), [0.5, 0.5, 0.5, 1.0]);
        material(&mut back, [0.0; 3]);
        ops.extend(back);
        let mut pillar = coloured_mesh("pillar", cube(), [0.5, 0.5, 0.5, 1.0]);
        material(&mut pillar, [0.1, 0.08, 0.05]);
        ops.extend(pillar);
        let centre = |x: f32| [x, DISC_CENTRE[0] - DEPTH_BELOW_ORIGIN, DISC_CENTRE[1]];
        ops.push(placed(1, "disc", centre(DISC_X), [1.0; 3]));
        if wall {
            ops.push(placed(2, "wall", centre(DISC_X + WALL_BEHIND), [1.0; 3]));
        }
        // The pillar 4.5 m out, over a fifth of the disc on screen.
        ops.push(placed(
            3,
            "pillar",
            [35.0, -DEPTH_BELOW_ORIGIN, -0.05],
            [0.8, 8.0, 0.8],
        ));
        harness.apply(ops);
        harness.single(&RendererCompositionCamera {
            id: "eye".to_owned(),
            pose: RendererCameraPose {
                position: [
                    CAMERA[0],
                    CAMERA[1] - f64::from(DEPTH_BELOW_ORIGIN),
                    CAMERA[2],
                ],
                pitch_degrees: 8.0,
                yaw_degrees: 80.0,
            },
            basis: None,
            projection: RendererCameraProjection::Perspective {
                fov_y_degrees: 30.0,
                near: 0.05,
                far: 2400.0,
            },
            motion: None,
            viewmodel_fov_y_degrees: 0.0,
        })
    };
    let alone = render(false);
    let over_wall = render(true);
    let bright = |rgba: &[u8], x: u32, y: u32| pixel(rgba, WIDTH, x, y)[2] > 160;
    let (mut disc, mut pillar, mut darkened) = (0, 0, 0);
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let reference = pixel(&alone, WIDTH, x, y);
            if bright(&alone, x, y) {
                disc += 1;
                if !bright(&over_wall, x, y) {
                    darkened += 1;
                }
            } else if reference[0] > 8 {
                pillar += 1;
            }
        }
    }
    assert!(disc > 2000, "the disc covers {disc} pixels");
    assert!(pillar > 1000, "the pillar covers {pillar} pixels");
    assert_eq!(
        darkened, 0,
        "{darkened} of the disc's {disc} pixels show the wall 2.5 mm behind it"
    );
}
