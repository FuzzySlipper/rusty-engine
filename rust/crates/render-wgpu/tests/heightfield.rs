//! Heightfield terrain (#9842) as the services draw it: rings of tiles from
//! `svc_mesh::heightfield_tiles`, projected by `HeightfieldProjector`, each
//! finer ring geomorphing onto the next. The focus walks across a ring's
//! region boundary, where tiles join and leave the finer ring: the picture
//! changes no more at that step than at an ordinary one, and no tile seam
//! or ring join ever shows the background through the ground. Same harness
//! as `tests/screenshots.rs`.

mod support;
mod terrain;

use render_model::*;
use render_wgpu::RendererOptions;
use support::*;
use terrain::Terrain;

const BACKGROUND: [f32; 4] = [1.0, 0.0, 1.0, 1.0];

/// Rough ground: the fine ring's half-metre cells show bumps the coarse
/// ring's 2 m cells cannot, so a swap between them would show.
fn ground(x: f64, z: f64) -> f32 {
    (0.6 * (x * 1.9).sin() * (z * 1.3).cos() + 0.4 * (x / 7.0).sin() + 0.3 * (z / 5.0).cos()) as f32
}

fn scene() -> Harness {
    let mut harness = Harness::new(RendererOptions {
        default_world_lights: false,
        ..RendererOptions::default()
    });
    harness.apply(vec![
        RenderDiff::SetBackgroundColor { color: BACKGROUND },
        RenderDiff::DefineMaterial {
            material: material("material/ground", [0.8, 0.8, 0.8, 1.0], None),
        },
        RenderDiff::CreateLight {
            handle: RenderHandle::new(900),
            parent: None,
            light: LightDescriptor::Directional {
                color: [1.0; 3],
                intensity: 3.0,
                enabled: true,
                direction: [-0.6, -0.5, 0.3],
                range: None,
                shadow_intent: LightShadowIntent::Disabled,
                shadow: Default::default(),
            },
        },
        RenderDiff::CreateLight {
            handle: RenderHandle::new(901),
            parent: None,
            light: LightDescriptor::Ambient {
                color: [1.0; 3],
                intensity: 0.5,
                enabled: true,
                shadow_intent: LightShadowIntent::Disabled,
                shadow: Default::default(),
                range: None,
            },
        },
    ]);
    harness
}

/// Looking down on the ground about (16, 0) from 30 m up, so the frame
/// holds the fine ring's morph band, its edge and the coarse ring beyond.
fn view(harness: &mut Harness) -> Vec<u8> {
    harness.render(&camera([16.0, 30.0, 18.0], 0.0, -60.0)).1
}

fn background_pixels(pixels: &[u8]) -> usize {
    pixels
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|p| p[0] > 200 && p[1] < 40 && p[2] > 200)
        .count()
}

fn mean_difference(a: &[u8], b: &[u8]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(a, b)| f64::from(a.abs_diff(*b)))
        .sum::<f64>()
        / a.len() as f64
}

#[test]
fn a_focus_crossing_a_ring_boundary_changes_the_picture_no_more_than_an_ordinary_step() {
    let mut harness = scene();
    let mut terrain = Terrain::new(ground);
    // The fine ring's region tiles are 16 m: crossing x = 16 shifts its
    // region a column, which joins at the morph's end, fully coarse.
    let mut frames = Vec::new();
    for x in [15.5, 15.75, 15.95, 16.15, 16.35] {
        let ops = terrain.settle([x, 0.0]);
        harness.apply(ops);
        frames.push(view(&mut harness));
    }
    for (step, frame) in frames.iter().enumerate() {
        assert_eq!(background_pixels(frame), 0, "a seam shows at step {step}");
    }
    let differences: Vec<f64> = frames
        .windows(2)
        .map(|pair| mean_difference(&pair[0], &pair[1]))
        .collect();
    let (before, crossing, after) = (differences[1], differences[2], differences[3]);
    assert!(
        crossing <= 2.0 * before.max(after) + 0.05,
        "the crossing step pops: {differences:?}"
    );
    assert!(
        differences.iter().all(|difference| *difference < 1.0),
        "{differences:?}"
    );
}
