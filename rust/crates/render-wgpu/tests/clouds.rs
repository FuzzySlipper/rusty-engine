//! The sky's cloud layer (`sky.wgsl` `fs_clouds`): it drifts over the
//! panorama with presentation time, takes the sun's colour and direction,
//! reports its pass, and coverage 0 draws the sky exactly as without it.
//! Same harness as `tests/screenshots.rs`.

mod support;

use render_model::*;
use render_wgpu::RendererOptions;
use support::*;

const NOON: [f32; 3] = [0.3, 1.0, -0.4];
const NOON_COLOR: [f32; 3] = [1.0, 0.98, 0.92];
const DUSK: [f32; 3] = [0.2, 0.08, -1.0];
const DUSK_COLOR: [f32; 3] = [1.0, 0.45, 0.15];

/// A panorama of a plain blue sky over a dark ground, and a sun toward
/// `toward` in `color`.
fn sky(toward: [f32; 3], color: [f32; 3]) -> Harness {
    let mut harness = Harness::new(RendererOptions {
        default_world_lights: false,
        ..RendererOptions::default()
    });
    let (width, height) = (64, 32);
    let rgba: Vec<u8> = (0..width * height)
        .flat_map(|texel| {
            if texel / width < height / 2 {
                [90, 140, 220, 255]
            } else {
                [40, 40, 40, 255]
            }
        })
        .collect();
    let texture =
        harness
            .resources
            .texture("texture/sky", width, height, &rgba, TextureWrap::Clamp);
    harness.apply(vec![
        RenderDiff::DefineTexture { texture },
        RenderDiff::SetSkyBackground {
            background: Some(SkyBackgroundDescriptor {
                texture: "texture/sky".to_owned(),
                blend: None,
            }),
        },
        RenderDiff::CreateLight {
            handle: RenderHandle::new(10),
            parent: None,
            light: LightDescriptor::Directional {
                color,
                intensity: 1.0,
                enabled: true,
                direction: toward.map(|value| -value),
                range: None,
                shadow_intent: LightShadowIntent::Disabled,
                shadow: Default::default(),
            },
        },
    ]);
    harness
}

fn clouds(coverage: f32) -> RenderDiff {
    RenderDiff::SetClouds {
        clouds: Some(CloudsDescriptor {
            coverage,
            drift: [12.0, 4.0],
            altitude: 1500.0,
            scale: 600.0,
            color: [1.0; 3],
        }),
    }
}

/// The view up into the sky, at presentation time `time`.
fn look_up(harness: &mut Harness, time: f64) -> Vec<u8> {
    harness.renderer.set_animation_time(time);
    harness.render(&camera([0.0, 1.0, 0.0], 0.0, 35.0)).1
}

/// Whether the clouds changed a pixel only a little: a thin edge, mostly
/// the sky behind.
fn thin(bare: &[u8], with: &[u8]) -> bool {
    let change: i32 = (0..3)
        .map(|channel| (i32::from(bare[channel]) - i32::from(with[channel])).abs())
        .sum();
    change < 60
}

/// The sky pixels the clouds covered, as mean (r, g, b), and how many there
/// were.
fn clouded(bare: &[u8], with: &[u8]) -> ([f64; 3], usize) {
    let (mut sum, mut count) = ([0.0; 3], 0);
    for (a, b) in bare.as_chunks::<4>().0.iter().zip(with.as_chunks::<4>().0) {
        if a[..3] != b[..3] && !thin(a, b) {
            for channel in 0..3 {
                sum[channel] += f64::from(b[channel]);
            }
            count += 1;
        }
    }
    (sum.map(|value| value / count.max(1) as f64), count)
}

#[test]
fn coverage_zero_draws_the_sky_as_without_clouds() {
    let mut harness = sky(NOON, NOON_COLOR);
    let bare = look_up(&mut harness, 2.0);
    harness.apply(vec![clouds(0.0)]);
    assert_eq!(look_up(&mut harness, 2.0), bare);
    harness.apply(vec![RenderDiff::SetClouds { clouds: None }]);
    assert_eq!(look_up(&mut harness, 2.0), bare);
}

#[test]
fn clouds_cover_the_sky_by_their_coverage_and_drift_with_time() {
    let mut harness = sky(NOON, NOON_COLOR);
    let bare = look_up(&mut harness, 2.0);
    harness.apply(vec![clouds(0.3)]);
    let (_, few) = clouded(&bare, &look_up(&mut harness, 2.0));
    harness.apply(vec![clouds(0.8)]);
    let first = look_up(&mut harness, 2.0);
    let (_, many) = clouded(&bare, &first);
    let total = (WIDTH * HEIGHT) as usize;
    assert!(few > total / 40, "some sky is cloudy: {few} of {total}");
    assert!(
        many > few * 2,
        "more coverage, more cloud: {few} then {many}"
    );
    assert_eq!(
        look_up(&mut harness, 2.0),
        first,
        "a held time draws the same"
    );
    assert_ne!(
        look_up(&mut harness, 40.0),
        first,
        "the layer drifts with time"
    );
    let passes = harness.renderer.gpu_readout().passes;
    assert!(passes.iter().any(|timing| timing.pass == "clouds"));
}

#[test]
fn dusk_warms_the_clouds_the_noon_sun_leaves_white() {
    let mean = |toward, color| {
        let mut harness = sky(toward, color);
        let bare = look_up(&mut harness, 2.0);
        harness.apply(vec![clouds(0.7)]);
        let frame = look_up(&mut harness, 2.0);
        clouded(&bare, &frame).0
    };
    let noon = mean(NOON, NOON_COLOR);
    let dusk = mean(DUSK, DUSK_COLOR);
    assert!(
        dusk[0] > dusk[2] && noon[2] > noon[0],
        "dusk clouds take the sun's red, noon ones stay white over a blue sky: noon {noon:?}, dusk {dusk:?}"
    );
    assert!(
        noon[0] + noon[1] + noon[2] > dusk[0] + dusk[1] + dusk[2],
        "the low sun lights them less: noon {noon:?}, dusk {dusk:?}"
    );
}
