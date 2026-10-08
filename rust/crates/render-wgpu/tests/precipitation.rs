//! Precipitation around the camera (`precipitation.wgsl`): drops 0 draws
//! nothing, more drops cover more of the view, the drops fall with time and
//! hold with it, the pass is timed, and no rain falls under a roof (the sky
//! occlusion layer). Same harness as `tests/screenshots.rs`.

mod support;

use render_model::*;
use render_wgpu::RendererOptions;
use support::*;

/// A dark ground lit by a hemisphere fill, so pale drops stand out, with
/// the sky's occlusion layer (an ambient light whose shadow is the open sky).
fn scene() -> Harness {
    let mut harness = Harness::new(RendererOptions {
        default_world_lights: false,
        shadows: true,
        ..RendererOptions::default()
    });
    harness.apply(vec![
        RenderDiff::SetBackgroundColor {
            color: [0.05, 0.05, 0.07, 1.0],
        },
        RenderDiff::DefineMaterial {
            material: material("material/ground", [0.2, 0.2, 0.2, 1.0], None),
        },
        static_mesh(
            "mesh/ground",
            box_mesh([-80.0, -1.0, -80.0], [80.0, 0.0, 80.0], |_| 0),
            "material/ground",
        ),
        instance(20, None, "mesh/ground", transform([0.0; 3], 0.0, [1.0; 3])),
        RenderDiff::CreateLight {
            handle: RenderHandle::new(10),
            parent: None,
            light: LightDescriptor::Hemisphere {
                color: [0.6, 0.6, 0.7],
                ground_color: [0.3, 0.3, 0.3],
                intensity: 0.5,
                enabled: true,
            },
        },
        RenderDiff::CreateLight {
            handle: RenderHandle::new(11),
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
    ]);
    harness
}

fn rain(drops: u32) -> RenderDiff {
    RenderDiff::SetPrecipitation {
        precipitation: Some(PrecipitationDescriptor {
            drops,
            shape: PrecipitationShape::Streak,
            velocity: [1.0, -14.0, 0.5],
            size: 0.03,
            streak_seconds: 0.05,
            color: [2.0, 2.0, 2.2, 0.8],
            additive: false,
            radius: 15.0,
            height: 10.0,
        }),
    }
}

/// From head height, looking out across the ground, at time `time`.
fn look(harness: &mut Harness, time: f64) -> Vec<u8> {
    harness.renderer.set_animation_time(time);
    harness.render(&camera([0.0, 1.7, 0.0], 0.0, -10.0)).1
}

/// How many pixels differ from `bare`.
fn changed(bare: &[u8], with: &[u8]) -> usize {
    bare.as_chunks::<4>()
        .0
        .iter()
        .zip(with.as_chunks::<4>().0)
        .filter(|(a, b)| {
            (0..3)
                .map(|channel| (i32::from(a[channel]) - i32::from(b[channel])).abs())
                .sum::<i32>()
                > 6
        })
        .count()
}

#[test]
fn no_drops_draw_as_without_precipitation() {
    let mut harness = scene();
    let bare = look(&mut harness, 3.0);
    harness.apply(vec![rain(0)]);
    assert_eq!(look(&mut harness, 3.0), bare);
    harness.apply(vec![RenderDiff::SetPrecipitation {
        precipitation: None,
    }]);
    assert_eq!(look(&mut harness, 3.0), bare);
}

#[test]
fn more_drops_fall_thicker_and_fall_with_time() {
    let mut harness = scene();
    let bare = look(&mut harness, 3.0);
    harness.apply(vec![rain(4_000)]);
    let light = changed(&bare, &look(&mut harness, 3.0));
    harness.apply(vec![rain(40_000)]);
    let heavy_frame = look(&mut harness, 3.0);
    let heavy = changed(&bare, &heavy_frame);
    let total = (WIDTH * HEIGHT) as usize;
    assert!(light > total / 400, "light rain shows: {light} of {total}");
    assert!(
        heavy > light * 3,
        "heavy rain covers more: {light} then {heavy}"
    );
    assert_eq!(
        look(&mut harness, 3.0),
        heavy_frame,
        "a held time draws the same"
    );
    assert_ne!(
        look(&mut harness, 3.4),
        heavy_frame,
        "the drops fall with time"
    );
    let passes = harness.renderer.gpu_readout().passes;
    assert!(passes.iter().any(|timing| timing.pass == "precipitation"));
}

#[test]
fn no_rain_falls_under_a_roof() {
    let open = {
        let mut harness = scene();
        let bare = look(&mut harness, 3.0);
        harness.apply(vec![rain(40_000)]);
        changed(&bare, &look(&mut harness, 3.0))
    };
    let roofed = {
        let mut harness = scene();
        // A wide low roof over the camera and the ground it looks at.
        harness.apply(vec![
            static_mesh(
                "mesh/roof",
                box_mesh([-40.0, 3.0, -40.0], [40.0, 3.5, 10.0], |_| 0),
                "material/ground",
            ),
            instance(21, None, "mesh/roof", transform([0.0; 3], 0.0, [1.0; 3])),
        ]);
        let bare = look(&mut harness, 3.0);
        harness.apply(vec![rain(40_000)]);
        changed(&bare, &look(&mut harness, 3.0))
    };
    assert!(
        roofed * 10 < open,
        "under a roof almost no rain shows: open {open}, roofed {roofed}"
    );
}
