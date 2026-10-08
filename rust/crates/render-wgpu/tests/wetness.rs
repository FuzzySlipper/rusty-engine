//! Wet surfaces (`lighting.wgsl` `wetted`, `CameraView.SetWetness`): a wet
//! scene darkens and smooths what faces up, walls less, puddles gather on
//! flat ground, and wetness 0 draws exactly as dry. Same harness as
//! `tests/screenshots.rs`.

mod support;

use render_model::*;
use render_wgpu::RendererOptions;
use support::*;

/// A sunlit sandy ground with a wall standing on it, lit by a sun and a
/// hemisphere fill. Shadows are on, for the sky's occlusion layer.
fn scene() -> Harness {
    let mut harness = Harness::new(RendererOptions {
        default_world_lights: false,
        shadows: true,
        ..RendererOptions::default()
    });
    harness.apply(vec![
        RenderDiff::DefineMaterial {
            material: material("material/sand", [0.8, 0.7, 0.5, 1.0], None),
        },
        static_mesh(
            "mesh/ground",
            box_mesh([-60.0, -1.0, -60.0], [60.0, 0.0, 60.0], |_| 0),
            "material/sand",
        ),
        static_mesh(
            "mesh/wall",
            box_mesh([-4.0, 0.0, -10.0], [4.0, 6.0, -9.0], |_| 0),
            "material/sand",
        ),
        instance(20, None, "mesh/ground", transform([0.0; 3], 0.0, [1.0; 3])),
        instance(21, None, "mesh/wall", transform([0.0; 3], 0.0, [1.0; 3])),
        RenderDiff::CreateLight {
            handle: RenderHandle::new(10),
            parent: None,
            light: LightDescriptor::Directional {
                color: [1.0, 0.97, 0.9],
                intensity: 2.0,
                enabled: true,
                direction: [-0.3, -0.8, -0.5],
                range: None,
                shadow_intent: LightShadowIntent::Disabled,
                shadow: Default::default(),
            },
        },
        RenderDiff::CreateLight {
            handle: RenderHandle::new(11),
            parent: None,
            light: LightDescriptor::Hemisphere {
                color: [0.6, 0.7, 0.9],
                ground_color: [0.3, 0.25, 0.2],
                intensity: 0.6,
                enabled: true,
            },
        },
    ]);
    harness
}

fn wet(wetness: f32, puddles: f32) -> RenderDiff {
    RenderDiff::SetWetness {
        wetness: Some(WetnessDescriptor { wetness, puddles }),
    }
}

/// Toward the wall across the ground, from a little above it.
fn look(harness: &mut Harness) -> Vec<u8> {
    harness.render(&camera([0.0, 3.0, 6.0], 0.0, -12.0)).1
}

/// Mean brightness of a pixel rectangle (x0..x1, y0..y1), and its spread.
fn region(rgba: &[u8], x: std::ops::Range<u32>, y: std::ops::Range<u32>) -> (f64, f64) {
    let mut values = Vec::new();
    for row in y {
        for column in x.clone() {
            let at = ((row * WIDTH + column) * 4) as usize;
            values.push(
                (f64::from(rgba[at]) + f64::from(rgba[at + 1]) + f64::from(rgba[at + 2])) / 3.0,
            );
        }
    }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let spread = (values
        .iter()
        .map(|value| (value - mean).powi(2))
        .sum::<f64>()
        / values.len() as f64)
        .sqrt();
    (mean, spread)
}

/// The ground near the camera (bottom of the frame) and the wall's face
/// (middle of the frame).
fn ground(rgba: &[u8]) -> (f64, f64) {
    region(rgba, 20..WIDTH - 20, HEIGHT - 40..HEIGHT - 4)
}
fn wall(rgba: &[u8]) -> (f64, f64) {
    region(
        rgba,
        WIDTH / 2 - 30..WIDTH / 2 + 30,
        HEIGHT / 2 - 25..HEIGHT / 2 - 5,
    )
}

#[test]
fn zero_wetness_draws_as_dry() {
    let mut harness = scene();
    let dry = look(&mut harness);
    harness.apply(vec![wet(0.0, 1.0)]);
    assert_eq!(look(&mut harness), dry);
    harness.apply(vec![RenderDiff::SetWetness { wetness: None }]);
    assert_eq!(look(&mut harness), dry);
}

#[test]
fn wet_ground_darkens_more_than_wet_walls() {
    let mut harness = scene();
    let dry = look(&mut harness);
    harness.apply(vec![wet(1.0, 0.0)]);
    let soaked = look(&mut harness);
    let (ground_dry, ground_wet) = (ground(&dry).0, ground(&soaked).0);
    let (wall_dry, wall_wet) = (wall(&dry).0, wall(&soaked).0);
    assert!(
        ground_wet < ground_dry * 0.85,
        "soaked ground darkens: {ground_dry:.1} to {ground_wet:.1}"
    );
    assert!(
        wall_wet < wall_dry && wall_dry - wall_wet < ground_dry - ground_wet,
        "a wall darkens less than the ground: wall {wall_dry:.1} to {wall_wet:.1}, ground {ground_dry:.1} to {ground_wet:.1}"
    );
    harness.apply(vec![wet(0.5, 0.0)]);
    let damp = ground(&look(&mut harness)).0;
    assert!(
        damp < ground_dry && damp > ground_wet,
        "half wet is part way: {ground_dry:.1}, {damp:.1}, {ground_wet:.1}"
    );
}

#[test]
fn puddles_gather_in_patches_on_flat_ground() {
    let mut harness = scene();
    harness.apply(vec![wet(1.0, 0.0)]);
    let plain = ground(&look(&mut harness));
    harness.apply(vec![wet(1.0, 1.0)]);
    let puddled = ground(&look(&mut harness));
    assert!(
        puddled.1 > plain.1 + 2.0 && puddled.0 < plain.0,
        "puddles darken the ground in patches: plain {plain:?}, puddled {puddled:?}"
    );
}

#[test]
fn ground_under_a_roof_stays_dry() {
    let mut harness = scene();
    // The sky's occlusion layer (an ambient light whose shadow is the open
    // sky) and a roof over the ground left of the camera.
    harness.apply(vec![
        RenderDiff::CreateLight {
            handle: RenderHandle::new(12),
            parent: None,
            light: LightDescriptor::Ambient {
                color: [0.6, 0.7, 0.9],
                intensity: 0.1,
                enabled: true,
                range: Some(40.0),
                shadow_intent: LightShadowIntent::Requested,
                shadow: Default::default(),
            },
        },
        static_mesh(
            "mesh/roof",
            box_mesh([-12.0, 6.0, -8.0], [-1.0, 6.5, 8.0], |_| 0),
            "material/sand",
        ),
        instance(22, None, "mesh/roof", transform([0.0; 3], 0.0, [1.0; 3])),
    ]);
    let under = |rgba: &[u8]| region(rgba, 10..WIDTH / 2 - 60, HEIGHT - 40..HEIGHT - 4).0;
    let open = |rgba: &[u8]| region(rgba, WIDTH / 2 + 60..WIDTH - 10, HEIGHT - 40..HEIGHT - 4).0;
    let dry = look(&mut harness);
    harness.apply(vec![wet(1.0, 0.0)]);
    let soaked = look(&mut harness);
    let sheltered = (under(&dry) - under(&soaked)) / under(&dry);
    let exposed = (open(&dry) - open(&soaked)) / open(&dry);
    assert!(
        exposed > 0.15 && sheltered < exposed * 0.25,
        "rain darkens the open ground but not the ground under the roof: open {exposed:.3}, sheltered {sheltered:.3}"
    );
}
