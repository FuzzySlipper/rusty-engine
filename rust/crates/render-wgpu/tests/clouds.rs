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
    clouds_sized(coverage, 600.0)
}

fn clouds_sized(coverage: f32, scale: f32) -> RenderDiff {
    RenderDiff::SetClouds {
        clouds: Some(CloudsDescriptor {
            coverage,
            drift: [12.0, 4.0],
            altitude: 1500.0,
            scale,
            color: [1.0; 3],
        }),
    }
}

/// Clouds small enough that several cast shade within the view of the ground.
const SMALL_CLOUD: f32 = 12.0;

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

/// The sky with a wide grey ground plane under it, lit only by the sun.
fn ground(toward: [f32; 3], color: [f32; 3]) -> Harness {
    let mut harness = sky(toward, color);
    harness.apply(vec![
        RenderDiff::DefineMaterial {
            material: material("material/ground", [0.6, 0.6, 0.6, 1.0], None),
        },
        static_mesh(
            "mesh/ground",
            box_mesh([-400.0, -1.0, -400.0], [400.0, 0.0, 400.0], |_| 0),
            "material/ground",
        ),
        instance(20, None, "mesh/ground", transform([0.0; 3], 0.0, [1.0; 3])),
    ]);
    harness
}

/// The view down onto the ground from above, at presentation time `time`.
fn look_down(harness: &mut Harness, time: f64) -> Vec<u8> {
    harness.renderer.set_animation_time(time);
    harness.render(&camera([0.0, 30.0, 0.0], 0.0, -80.0)).1
}

/// The ground pixels' mean brightness and its spread (standard deviation).
fn brightness(rgba: &[u8]) -> (f64, f64) {
    let values: Vec<f64> = rgba
        .as_chunks::<4>()
        .0
        .iter()
        .map(|pixel| (f64::from(pixel[0]) + f64::from(pixel[1]) + f64::from(pixel[2])) / 3.0)
        .collect();
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let spread = (values
        .iter()
        .map(|value| (value - mean).powi(2))
        .sum::<f64>()
        / values.len() as f64)
        .sqrt();
    (mean, spread)
}

#[test]
fn clouds_shade_the_ground_by_coverage_and_their_shadows_drift() {
    let mut harness = ground(NOON, NOON_COLOR);
    let clear = brightness(&look_down(&mut harness, 2.0));
    harness.apply(vec![clouds_sized(0.5, SMALL_CLOUD)]);
    let broken_frame = look_down(&mut harness, 2.0);
    let broken = brightness(&broken_frame);
    harness.apply(vec![clouds_sized(1.0, SMALL_CLOUD)]);
    let overcast = brightness(&look_down(&mut harness, 2.0));
    assert!(
        overcast.0 < clear.0 * 0.5,
        "a full overcast dims the sunlit ground: clear {clear:?}, overcast {overcast:?}"
    );
    assert!(
        broken.0 < clear.0 && broken.0 > overcast.0,
        "a broken sky dims it part way: clear {clear:?}, broken {broken:?}, overcast {overcast:?}"
    );
    assert!(
        broken.1 > clear.1 + 4.0 && broken.1 > overcast.1 + 4.0,
        "a broken sky casts patches of shade: spread clear {:.1}, broken {:.1}, overcast {:.1}",
        clear.1,
        broken.1,
        overcast.1
    );
    harness.apply(vec![clouds_sized(0.5, SMALL_CLOUD)]);
    assert_eq!(
        look_down(&mut harness, 2.0),
        broken_frame,
        "a held time shades the same"
    );
    assert_ne!(
        look_down(&mut harness, 40.0),
        broken_frame,
        "the shadows drift with the clouds"
    );
}

#[test]
fn an_overcast_greys_the_panorama() {
    let mut harness = sky(NOON, NOON_COLOR);
    // Below the cloud plane's horizon fade, where only the panorama shows.
    let view = |harness: &mut Harness| harness.render(&camera([0.0, 1.0, 0.0], 0.0, 2.0)).1;
    let bare = view(&mut harness);
    harness.apply(vec![clouds(1.0)]);
    let grey = view(&mut harness);
    let blue = |rgba: &[u8]| {
        let pixel = &rgba[((HEIGHT as usize / 2 - 20) * WIDTH as usize) * 4..][..4];
        i32::from(pixel[2]) - i32::from(pixel[0])
    };
    assert!(
        blue(&grey) < blue(&bare) / 2,
        "under a full overcast the sky's blue fades toward grey: {} then {}",
        blue(&bare),
        blue(&grey)
    );
}

/// Whether the cloud seen toward the sun from a point is the cloud that
/// shades it: from a camera 300 m up (the sky and `cloud_light` measure the
/// layer's altitude alike), at points across the layer, the sky straight
/// toward an oblique sun is clouded where a small plate at that point is in
/// the cloud's shade (wisps aside).
#[test]
fn the_cloud_toward_the_sun_is_the_cloud_that_shades_an_elevated_point() {
    let toward = [0.6, 0.8, 0.0];
    let height = 300.0;
    let plate = |harness: &mut Harness, at: [f64; 3]| {
        harness.apply(vec![RenderDiff::Update {
            handle: RenderHandle::new(21),
            transform: Some(transform(at.map(|value| value as f32), 0.0, [1.0; 3])),
            material: None,
            visible: None,
            metadata: None,
        }]);
    };
    let mut harness = sky(toward, NOON_COLOR);
    harness.apply(vec![
        RenderDiff::DefineMaterial {
            material: material("material/plate", [0.8, 0.8, 0.8, 1.0], None),
        },
        static_mesh(
            "mesh/plate",
            box_mesh([-0.5, -0.1, -0.5], [0.5, 0.0, 0.5], |_| 0),
            "material/plate",
        ),
        instance(21, None, "mesh/plate", transform([0.0; 3], 0.0, [1.0; 3])),
    ]);
    harness.renderer.set_animation_time(2.0);
    let centre = |rgba: &[u8]| -> f64 {
        let at = (((HEIGHT / 2) * WIDTH + WIDTH / 2) * 4) as usize;
        (f64::from(rgba[at]) + f64::from(rgba[at + 1]) + f64::from(rgba[at + 2])) / 3.0
    };
    let toward_sun =
        |harness: &mut Harness, at: [f64; 3]| centre(&harness.render(&camera(at, 90.0, 53.13)).1);
    let down_on = |harness: &mut Harness, at: [f64; 3]| {
        centre(
            &harness
                .render(&camera([at[0], at[1] + 2.0, at[2]], 0.0, -89.9))
                .1,
        )
    };
    let points: Vec<[f64; 3]> = (0..6)
        .flat_map(|i| (0..5).map(move |j| [f64::from(i) * 97.0, height, f64::from(j) * 113.0]))
        .collect();
    let mut bare = Vec::new();
    for at in &points {
        plate(&mut harness, *at);
        bare.push((toward_sun(&mut harness, *at), down_on(&mut harness, *at)));
    }
    harness.apply(vec![clouds_sized(0.5, 200.0)]);
    let (mut agree, mut decisive) = (0, 0);
    for (at, (sky_bare, plate_bare)) in points.iter().zip(&bare) {
        plate(&mut harness, *at);
        let clouded = toward_sun(&mut harness, *at) - sky_bare;
        let shade = down_on(&mut harness, *at) / plate_bare.max(1.0);
        // A thin cloud already brightens the sky well past its shade's
        // dimming, and a wisp of the octaves the shade leaves out casts
        // none; between clear and clouded either way is left out.
        let sky_says = if clouded > 50.0 {
            Some(true)
        } else if clouded.abs() < 8.0 {
            Some(false)
        } else {
            None
        };
        let ground_says = if shade < 0.95 {
            Some(true)
        } else if shade > 0.97 {
            Some(false)
        } else {
            None
        };
        if let (Some(sky_says), Some(ground_says)) = (sky_says, ground_says) {
            decisive += 1;
            agree += usize::from(sky_says == ground_says);
        }
    }
    assert!(
        decisive >= 20,
        "enough points are clearly clouded or clear: {decisive}"
    );
    assert!(
        agree == decisive,
        "the sky toward the sun and the shade agree at {agree} of {decisive} points"
    );
}
