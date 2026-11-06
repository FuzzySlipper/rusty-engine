//! The sky's cloud layer (`sky.wgsl` `fs_clouds`): it drifts over the
//! panorama or a clear colour with presentation time, takes the sun's colour
//! and direction, reports its pass, and coverage 0 draws the sky exactly as
//! without it.
//! Through a twilight the sky's sun lights it from below the horizon,
//! reddening, flat and volumetric alike. Under a motion target the clouds
//! move with their drift and the camera's turn. Same harness as
//! `tests/screenshots.rs`.

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
    let mut harness = panorama(VolumetricCloudsQuality::Off, NOON_SKY);
    harness.apply(vec![RenderDiff::CreateLight {
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
    }]);
    harness
}

/// The panoramas' sky above the horizon: a noon blue and a dusk's darker one.
const NOON_SKY: [u8; 4] = [90, 140, 220, 255];
const DUSK_SKY: [u8; 4] = [60, 70, 120, 255];

/// A panorama of a plain `color` sky over a dark ground, and no light.
fn panorama(clouds: VolumetricCloudsQuality, color: [u8; 4]) -> Harness {
    let mut harness = Harness::new(RendererOptions {
        default_world_lights: false,
        volumetric_clouds: clouds,
        ..RendererOptions::default()
    });
    let (width, height) = (64, 32);
    let rgba: Vec<u8> = (0..width * height)
        .flat_map(|texel| {
            if texel / width < height / 2 {
                color
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
            thickness: 0.0,
            kind: CloudKind::Cumulus,
            twilight_degrees: 0.0,
            pixel_metres: 0.0,
            shade_steps: 0,
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

/// The sky's sun placed by the atmosphere `elevation` degrees up (below the
/// horizon when negative), straight ahead of `look_up`, apart from any
/// light: the scene has none, so nothing on the ground is lit by it.
fn sky_sun(elevation: f32) -> RenderDiff {
    let radians = elevation.to_radians();
    RenderDiff::SetAtmosphere {
        atmosphere: Some(AtmosphereDescriptor {
            sky_sun_direction: [0.0, radians.sin(), -radians.cos()],
            sky_sun_color: [1.0, 0.75, 0.5],
            sky_sun_intensity: 1.0,
            ..Default::default()
        }),
    }
}

fn twilight_clouds(twilight_degrees: f32) -> RenderDiff {
    RenderDiff::SetClouds {
        clouds: Some(CloudsDescriptor {
            coverage: 0.55,
            drift: [0.0, 0.0],
            altitude: 1500.0,
            scale: 600.0,
            color: [1.0; 3],
            thickness: 0.0,
            kind: CloudKind::Cumulus,
            twilight_degrees,
            pixel_metres: 0.0,
            shade_steps: 0,
        }),
    }
}

#[test]
#[ignore = "hardware-only volumetric-cloud twilight visual test; run scripts/gpu-lane-temporal-tests.sh"]
fn a_sun_below_the_horizon_lights_the_clouds_through_the_twilight_reddening_flat_and_volumetric_alike(
) {
    const TWILIGHT: f32 = 8.0;
    require_hardware_adapter(&gpu(), "volumetric-cloud twilight lighting");
    // Up into a dusk sky toward the sun with the clouds' twilight, several
    // frames so the volumetric history settles.
    let render = |quality, elevation: f32, twilight: f32| {
        let mut harness = panorama(quality, DUSK_SKY);
        harness.renderer.set_animation_time(2.0);
        let view = camera([0.0, 1.0, 0.0], 0.0, 25.0);
        let bare = harness.render(&view).1;
        harness.apply(vec![sky_sun(elevation), twilight_clouds(twilight)]);
        let mut image = Vec::new();
        for _ in 0..4 {
            image = harness.render(&view).1;
        }
        let refused = harness
            .renderer
            .settings_readout()
            .volumetric_clouds
            .is_some();
        (!refused).then_some((bare, image))
    };
    for quality in [VolumetricCloudsQuality::Off, VolumetricCloudsQuality::High] {
        // Each mode covers different pixels and integrates different cloud
        // heights, attenuation and sky contributions. Check its own lighting
        // response through twilight.
        let mean = |elevation, twilight| {
            let (bare, image) = render(quality, elevation, twilight)
                .expect("the hardware adapter draws the selected cloud mode");
            clouded(&bare, &image).0
        };
        let sum = |color: [f64; 3]| color[0] + color[1] + color[2];
        let unlit = mean(-3.0, 0.0);
        let horizon = mean(0.0, TWILIGHT);
        let below = mean(-3.0, TWILIGHT);
        let deeper = mean(-6.0, TWILIGHT);
        // Without a twilight a sun under the horizon lights no cloud; with
        // one it does, warm, and redder as it sinks.
        assert!(
            sum(below) > sum(unlit) + 120.0 && sum(deeper) > sum(unlit) + 60.0,
            "{quality:?}: lit below the horizon: {below:?} {deeper:?}, unlit {unlit:?}"
        );
        assert!(below[0] > below[2] + 30.0, "{quality:?}: warm: {below:?}");
        let red = |color: [f64; 3]| color[0] / color[1];
        assert!(
            red(horizon) < red(below) && red(below) < red(deeper),
            "{quality:?}: redder as it sinks: {horizon:?} {below:?} {deeper:?}"
        );
        // Past the twilight the sun is gone, as without one.
        assert_eq!(
            render(quality, -12.0, TWILIGHT),
            render(quality, -12.0, 0.0),
            "{quality:?}: gone past the twilight"
        );
        // While it is up a twilight lights the clouds at least as much.
        assert!(sum(mean(5.0, TWILIGHT)) >= sum(mean(5.0, 0.0)) - 3.0);
    }
}

#[test]
fn the_sky_sun_lights_no_ground() {
    let mut harness = ground(NOON, NOON_COLOR);
    let lit = look_down(&mut harness, 2.0);
    harness.apply(vec![sky_sun(-3.0)]);
    assert_eq!(look_down(&mut harness, 2.0), lit);
}

#[test]
fn a_cloud_region_under_a_clear_twilight_layer_catches_the_sunset_as_under_a_broken_one() {
    // A storm region ahead, under a twilight layer of `coverage` (None: no
    // layer, so the default one), with the sky's sun `elevation` degrees up.
    let render = |quality, coverage: Option<f32>, elevation: f32| {
        let mut harness = panorama(quality, DUSK_SKY);
        harness.renderer.set_animation_time(2.0);
        let mut ops = vec![
            sky_sun(elevation),
            RenderDiff::SetCloudRegion {
                id: 1,
                region: CloudRegionDescriptor {
                    center: [0.0, -6000.0],
                    radius: 5000.0,
                    coverage: 1.0,
                    darkness: 0.0,
                    drift: [0.0, 0.0],
                    kind: CloudKind::Cumulus,
                    thickness: 0.0,
                },
            },
        ];
        if let Some(coverage) = coverage {
            ops.push(RenderDiff::SetClouds {
                clouds: Some(CloudsDescriptor {
                    coverage,
                    ..match twilight_clouds(8.0) {
                        RenderDiff::SetClouds {
                            clouds: Some(clouds),
                        } => clouds,
                        _ => unreachable!(),
                    }
                }),
            });
        }
        harness.apply(ops);
        let view = camera([0.0, 1.0, 0.0], 0.0, 25.0);
        let mut image = Vec::new();
        for _ in 0..4 {
            image = harness.render(&view).1;
        }
        let refused = harness
            .renderer
            .settings_readout()
            .volumetric_clouds
            .is_some();
        (!refused).then_some(image)
    };
    // The mean colour of the view's middle, inside the region.
    let middle = |image: &[u8]| {
        let mut sum = [0.0f64; 3];
        let mut count = 0.0;
        for y in 50..130u32 {
            for x in 100..220u32 {
                let at = ((y * WIDTH + x) * 4) as usize;
                for channel in 0..3 {
                    sum[channel] += f64::from(image[at + channel]);
                }
                count += 1.0;
            }
        }
        sum.map(|value| value / count)
    };
    for quality in [VolumetricCloudsQuality::Off, VolumetricCloudsQuality::High] {
        if render(quality, Some(0.0), 0.0).is_none() {
            continue;
        }
        for elevation in [0.0, -3.0] {
            let clear = middle(&render(quality, Some(0.0), elevation).unwrap());
            let broken = middle(&render(quality, Some(0.3), elevation).unwrap());
            let unlit = middle(&render(quality, None, elevation).unwrap());
            assert!(
                (0..3).all(|channel| (clear[channel] - broken[channel]).abs() < 6.0),
                "{quality:?} at {elevation}°: under a clear layer {clear:.1?}, under a broken one {broken:.1?}"
            );
            if elevation < 0.0 {
                assert!(
                    clear[0] > unlit[0] + 20.0,
                    "{quality:?} at {elevation}°: the twilight lights it: {clear:.1?}, without {unlit:.1?}"
                );
            }
        }
    }
}

/// The motion view's range (`compose.wgsl` `MOTION_VIEW_RANGE`) and the
/// fixture camera's focal length in pixels, 90 / tan(30°).
const MOTION_RANGE: f32 = 16.0;
const FOCAL_PIXELS: f32 = 155.884_57;

/// Each pixel's motion in the motion view, render pixels right and down.
fn motions(rgba: &[u8]) -> Vec<[f32; 2]> {
    rgba.as_chunks::<4>()
        .0
        .iter()
        .map(|[red, green, ..]| {
            [red, green].map(|value| (f32::from(*value) / 255.0 - 0.5) * 2.0 * MOTION_RANGE)
        })
        .collect()
}

/// The motion up into the sky at `time`, looking `yaw` degrees right.
fn motion_up(harness: &mut Harness, yaw: f64, time: f64) -> Vec<[f32; 2]> {
    harness.renderer.set_animation_time(time);
    motions(&harness.render(&camera([0.0, 1.0, 0.0], yaw, 35.0)).1)
}

/// A layer 300 m up drifting 12 m/s east (right of a camera looking north).
fn low_drifting(coverage: f32) -> RenderDiff {
    RenderDiff::SetClouds {
        clouds: Some(CloudsDescriptor {
            coverage,
            drift: [12.0, 0.0],
            altitude: 300.0,
            scale: 200.0,
            color: [1.0; 3],
            thickness: 0.0,
            kind: CloudKind::Cumulus,
            twilight_degrees: 0.0,
            pixel_metres: 0.0,
            shade_steps: 0,
        }),
    }
}

#[test]
fn drifting_clouds_move_on_a_still_camera_and_turn_with_the_sky() {
    let mut harness = sky(NOON, NOON_COLOR);
    harness
        .renderer
        .set_overlay_shown(render_wgpu::OverlayShown {
            motion: true,
            ..Default::default()
        });
    harness.apply(vec![low_drifting(0.0)]);
    motion_up(&mut harness, 0.0, 2.0);
    // No cover: the sky holds as the time runs on.
    let bare = motion_up(&mut harness, 0.0, 4.0);
    let still = |motion: &[f32; 2]| motion[0].abs() < 0.2 && motion[1].abs() < 0.2;
    assert!(bare.iter().all(still), "a clear sky holds");
    harness.apply(vec![low_drifting(0.5)]);
    motion_up(&mut harness, 0.0, 4.0);
    let drifted = motion_up(&mut harness, 0.0, 6.0);
    // At the centre, looking 35° up at a layer 299 m above the eye, the
    // cloud stands 427 m ahead: 24 m of drift east is that share of the
    // focal length to the right.
    let full = FOCAL_PIXELS * 24.0 / (299.0 / 35.0_f32.to_radians().tan());
    let centre = ((HEIGHT / 2) * WIDTH + WIDTH / 2) as usize;
    let rightward = drifted.iter().filter(|motion| motion[0] > 1.0).count();
    let leftward = drifted.iter().filter(|motion| motion[0] < -0.3).count();
    let clear = drifted.iter().filter(|motion| still(motion)).count();
    assert!(
        rightward > drifted.len() / 10 && leftward == 0,
        "the clouds drift right: {rightward} right, {leftward} left"
    );
    assert!(clear > drifted.len() / 20, "the gaps hold: {clear}");
    assert!(
        drifted[centre][0] <= full * 1.05,
        "at most a full cloud's drift at the centre: {:?}, full {full}",
        drifted[centre]
    );
    let densest = drifted
        .iter()
        .skip(centre - WIDTH as usize * 5)
        .take(WIDTH as usize * 10)
        .map(|motion| motion[0])
        .fold(0.0, f32::max);
    assert!(
        densest > full * 0.6,
        "a dense cloud near the centre row drifts nearly in full: {densest}, full {full}"
    );
    // A turn with the time held: the clouds and the clear sky turn alike.
    let turned = motion_up(&mut harness, 2.0, 6.0);
    let spread = turned
        .iter()
        .skip(centre - 2)
        .take(5)
        .map(|motion| motion[0])
        .collect::<Vec<_>>();
    let expected = -FOCAL_PIXELS * 2.0_f32.to_radians().tan() * 35.0_f32.to_radians().cos();
    for value in spread {
        assert!(
            (value - expected).abs() < 1.0,
            "turned at the centre: {value}, about {expected}"
        );
    }
}

fn pixel(rgba: &[u8], (x, y): (usize, usize)) -> [i32; 3] {
    let at = (y * WIDTH as usize + x) * 4;
    [rgba[at] as i32, rgba[at + 1] as i32, rgba[at + 2] as i32]
}

/// The largest difference of any channel.
fn distance(a: [i32; 3], b: [i32; 3]) -> i32 {
    (0..3)
        .map(|channel| (a[channel] - b[channel]).abs())
        .max()
        .unwrap_or(0)
}

/// A daylit panorama with a coverage of cloud in cells of `pixel_metres`
/// and `shade_steps` shades, at the altitude and size a product would give
/// them, drifting.
fn pixel_sky(pixel_metres: f32, shade_steps: u32) -> Harness {
    let mut harness = panorama(VolumetricCloudsQuality::Off, NOON_SKY);
    harness.apply(vec![
        RenderDiff::SetAtmosphere {
            atmosphere: Some(AtmosphereDescriptor {
                sky_sun_direction: NOON,
                sky_sun_color: NOON_COLOR,
                sky_sun_intensity: 1.5,
                ..Default::default()
            }),
        },
        RenderDiff::SetClouds {
            clouds: Some(CloudsDescriptor {
                coverage: 0.6,
                drift: [12.0, 4.0],
                altitude: 1500.0,
                scale: 1600.0,
                color: [1.0; 3],
                thickness: 0.0,
                kind: CloudKind::Cumulus,
                twilight_degrees: 0.0,
                pixel_metres,
                shade_steps,
            }),
        },
    ]);
    harness
}

/// The pixel a view turned to `yaw` and `pitch` degrees shows `direction` at,
/// if it shows it (the fixture camera's 60° view).
fn seen_at(direction: [f32; 3], yaw: f32, pitch: f32) -> Option<(usize, usize)> {
    let (yaw, pitch) = (yaw.to_radians(), pitch.to_radians());
    let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let forward = [
        yaw.sin() * pitch.cos(),
        pitch.sin(),
        -yaw.cos() * pitch.cos(),
    ];
    let right = [yaw.cos(), 0.0, yaw.sin()];
    let up = [
        right[1] * forward[2] - right[2] * forward[1],
        right[2] * forward[0] - right[0] * forward[2],
        right[0] * forward[1] - right[1] * forward[0],
    ];
    let depth = dot(direction, forward);
    if depth <= 0.0 {
        return None;
    }
    let half = HEIGHT as f32 / 2.0;
    let focal = half / 30f32.to_radians().tan();
    let x = WIDTH as f32 / 2.0 + dot(direction, right) / depth * focal;
    let y = half - dot(direction, up) / depth * focal;
    (x >= 0.0 && y >= 0.0 && x < WIDTH as f32 && y < HEIGHT as f32)
        .then_some((x as usize, y as usize))
}

/// Directions over the sky between two elevations, degrees.
fn sky_directions(low: f32, high: f32) -> Vec<[f32; 3]> {
    let mut directions = Vec::new();
    for row in 0..24 {
        let elevation = (low + (high - low) * row as f32 / 23.0).to_radians();
        for column in 0..48 {
            let azimuth = ((column as f32 - 24.0) * 1.1).to_radians();
            directions.push([
                azimuth.sin() * elevation.cos(),
                elevation.sin(),
                -azimuth.cos() * elevation.cos(),
            ]);
        }
    }
    directions
}

/// The share of `directions` that draw differently (by more than rounding)
/// in the two frames, among those both show.
fn changed_share(
    first: (&[u8], f32, f32),
    second: (&[u8], f32, f32),
    directions: &[[f32; 3]],
) -> f32 {
    let (mut seen, mut changed) = (0, 0);
    for &direction in directions {
        if let (Some(a), Some(b)) = (
            seen_at(direction, first.1, first.2),
            seen_at(direction, second.1, second.2),
        ) {
            seen += 1;
            // Whole pixels place a direction to within one, so a sample by a
            // cell's edge may land either side: it moved only if nothing
            // around it matches.
            let color = pixel(first.0, a);
            let held = (-1..=1).any(|dy: isize| {
                (-1..=1).any(|dx: isize| {
                    let x = b.0.saturating_add_signed(dx).min(WIDTH as usize - 1);
                    let y = b.1.saturating_add_signed(dy).min(HEIGHT as usize - 1);
                    distance(color, pixel(second.0, (x, y))) <= 6
                })
            });
            if !held {
                changed += 1;
            }
        }
    }
    assert!(seen > 100, "the views share the sky: {seen}");
    changed as f32 / seen as f32
}

/// The share of a frame's neighbouring pixels that are the same colour.
fn flat_share(rgba: &[u8]) -> f32 {
    let (mut same, mut total) = (0, 0);
    for y in 10..HEIGHT as usize - 10 {
        for x in 10..WIDTH as usize - 10 {
            total += 1;
            if pixel(rgba, (x, y)) == pixel(rgba, (x + 1, y)) {
                same += 1;
            }
        }
    }
    same as f32 / total as f32
}

/// Overhead, cells draw one flat colour each where the smooth layer changes
/// from pixel to pixel.
#[test]
fn pixel_clouds_draw_each_cell_one_colour() {
    let view = camera([0.0, 0.0, 0.0], 0.0, 70.0);
    let smooth = pixel_sky(0.0, 0).render(&view).1;
    let cells = pixel_sky(160.0, 0).render(&view).1;
    let (smooth, cells) = (flat_share(&smooth), flat_share(&cells));
    assert!(cells > 0.8, "cells are flat: {cells}");
    assert!(cells > smooth + 0.2, "{cells} against smooth {smooth}");
}

/// Cells belong to the sky, not the screen: a direction draws the same
/// through a turn of the camera, up high and down toward the horizon.
#[test]
fn pixel_clouds_hold_their_cells_as_the_camera_turns() {
    let mut harness = pixel_sky(160.0, 4);
    let (yaw, turned) = (0.0, 9.0);
    for (pitch, low, high) in [(60.0, 45.0, 75.0), (12.0, 2.0, 18.0)] {
        let first = harness
            .render(&camera([0.0, 0.0, 0.0], yaw as f64, pitch as f64))
            .1;
        let second = harness
            .render(&camera([0.0, 0.0, 0.0], turned as f64, pitch as f64))
            .1;
        let share = changed_share(
            (&first, yaw, pitch),
            (&second, turned, pitch),
            &sky_directions(low, high),
        );
        assert!(share < 0.02, "pitch {pitch}: {share} of the sky moved");
    }
}

/// With the camera held, a drifting cell keeps its colour as it travels:
/// the sky near the horizon changes only where a cell's edge passed.
#[test]
fn pixel_clouds_do_not_shimmer_near_the_horizon_as_they_drift() {
    let view = camera([0.0, 0.0, 0.0], 0.0, 12.0);
    let mut harness = pixel_sky(160.0, 4);
    let mut previous = harness.render(&view).1;
    let directions = sky_directions(1.0, 12.0);
    for step in 1..=6 {
        // Five seconds: 60 metres of drift, enough to flip a cell too small
        // for the view, and well under a pixel of a merged one.
        harness.at(f64::from(step) * 5.0);
        let next = harness.render(&view).1;
        let share = changed_share((&previous, 0.0, 12.0), (&next, 0.0, 12.0), &directions);
        assert!(share < 0.003, "{share} of the low sky flickered");
        previous = next;
    }
}

/// Shades limit the layer to a few levels: far fewer colours in one frame
/// than the smooth layer has.
#[test]
fn shade_steps_limit_the_clouds_to_a_few_shades() {
    let colours = |rgba: &[u8]| {
        let mut seen = std::collections::BTreeSet::new();
        for y in (10..HEIGHT as usize - 10).step_by(3) {
            for x in (10..WIDTH as usize - 10).step_by(3) {
                seen.insert(pixel(rgba, (x, y)));
            }
        }
        seen.len()
    };
    let view = camera([0.0, 0.0, 0.0], 0.0, 80.0);
    let smooth = colours(&pixel_sky(160.0, 0).render(&view).1);
    let stepped = colours(&pixel_sky(160.0, 3).render(&view).1);
    assert!(stepped * 2 < smooth, "{stepped} against {smooth} colours");
}

/// A clear colour and no light, so the clouds show the sky's ambient alone.
fn clear_sky(color: [f32; 4]) -> Harness {
    let mut harness = Harness::new(RendererOptions {
        default_world_lights: false,
        ..RendererOptions::default()
    });
    harness.apply(vec![RenderDiff::SetBackgroundColor { color }]);
    harness
}

#[test]
fn clouds_draw_over_a_clear_colour_and_take_its_light() {
    let red = [0.6, 0.05, 0.05, 1.0];
    let mut harness = clear_sky(red);
    let bare = look_up(&mut harness, 2.0);
    harness.apply(vec![clouds(0.8)]);
    let reddish = look_up(&mut harness, 2.0);
    let (mean, count) = clouded(&bare, &reddish);
    assert!(count > 1000, "the layer draws: {count} pixels");
    assert!(
        harness
            .renderer
            .gpu_readout()
            .passes
            .iter()
            .any(|timing| timing.pass == "clouds"),
        "the pass is timed"
    );
    assert_ne!(
        look_up(&mut harness, 40.0),
        reddish,
        "the layer drifts with time"
    );

    let mut harness = clear_sky([0.05, 0.05, 0.6, 1.0]);
    let bare = look_up(&mut harness, 2.0);
    harness.apply(vec![clouds(0.8)]);
    let (blue, _) = clouded(&bare, &look_up(&mut harness, 2.0));
    assert!(
        mean[0] > blue[0] && mean[2] < blue[2],
        "the clear colour lights them: over red {mean:?}, over blue {blue:?}"
    );
}

#[test]
fn clouds_fade_into_a_clear_colour_at_the_horizon() {
    let color = [0.3, 0.32, 0.36, 1.0];
    let mut harness = clear_sky(color);
    let level = camera([0.0, 1.0, 0.0], 0.0, 0.0);
    harness.renderer.set_animation_time(2.0);
    let bare = harness.render(&level).1;
    harness.apply(vec![clouds(1.0)]);
    let frame = harness.render(&level).1;
    assert_ne!(frame, bare, "the layer shows above the horizon");
    // The rows at the horizon and below it hold the clear colour.
    let row = (WIDTH * 4) as usize;
    let horizon = (HEIGHT / 2 - 1) as usize * row;
    let most = bare[horizon..]
        .iter()
        .zip(&frame[horizon..])
        .map(|(a, b)| a.abs_diff(*b))
        .max();
    assert!(most <= Some(1), "the fade ends on the colour: {most:?}");
}

#[test]
fn no_clouds_leave_a_clear_colour_as_it_was() {
    let color = [0.2, 0.3, 0.5, 1.0];
    let mut harness = clear_sky(color);
    let bare = look_up(&mut harness, 2.0);
    harness.apply(vec![clouds(0.0)]);
    assert_eq!(look_up(&mut harness, 2.0), bare, "coverage 0");
    harness.apply(vec![clouds(0.8)]);
    assert_ne!(look_up(&mut harness, 2.0), bare);
    harness.apply(vec![RenderDiff::SetClouds { clouds: None }]);
    assert_eq!(look_up(&mut harness, 2.0), bare, "cleared");
}
