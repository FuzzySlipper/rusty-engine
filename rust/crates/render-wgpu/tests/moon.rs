//! The moon (`SetMoon`; `sky.wgsl` `fs_moon`): a disc of its own beside the
//! sun, lit by its phase with the lit limb toward the sky's sun, a pale disc
//! over a day sky, covered by the clouds and the world, blooming at a
//! radiance, and changing nothing else. Same harness as
//! `tests/screenshots.rs`.

mod support;

use render_model::*;
use render_wgpu::RendererOptions;
use support::*;

/// Skies above the horizon: a night's, a day's.
const NIGHT: [u8; 4] = [10, 12, 25, 255];
const DAY: [u8; 4] = [90, 140, 220, 255];

/// The moon 5° up straight ahead, 4° across: its centre's pixel and its
/// radius in pixels at the harness's 60° view.
const MOON_UP: f32 = 5.0;
const CENTRE: (usize, usize) = (160, 76);
const RADIUS_PIXELS: usize = 10;

/// A panorama of a plain `sky` over a dark ground, no light, and the sky's
/// sun off to the right below the horizon, so a moon ahead has its lit limb
/// to the right.
fn scene(sky: [u8; 4]) -> Harness {
    let mut harness = Harness::new(RendererOptions {
        default_world_lights: false,
        ..RendererOptions::default()
    });
    let (width, height) = (64, 32);
    let rgba: Vec<u8> = (0..width * height)
        .flat_map(|texel| {
            if texel / width < height / 2 {
                sky
            } else {
                [30, 30, 30, 255]
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
        RenderDiff::SetAtmosphere {
            atmosphere: Some(AtmosphereDescriptor {
                sky_sun_direction: [1.0, -0.2, 0.0],
                sky_sun_color: [1.0, 0.8, 0.6],
                sky_sun_intensity: 1.0,
                ..Default::default()
            }),
        },
    ]);
    harness
}

fn moon(phase: f32, radiance: f32, earthshine: f32) -> RenderDiff {
    let up = MOON_UP.to_radians();
    RenderDiff::SetMoon {
        moon: Some(MoonDescriptor {
            direction: [0.0, up.sin(), -up.cos()],
            radius_degrees: 4.0,
            color: [1.0, 0.95, 0.85],
            radiance,
            phase,
            earthshine,
            halo: 0.0,
            light_intensity: 0.0,
        }),
    }
}

fn look(harness: &mut Harness) -> Vec<u8> {
    harness.render(&camera([0.0, 1.0, 0.0], 0.0, 0.0)).1
}

fn pixel(rgba: &[u8], (x, y): (usize, usize)) -> [i32; 3] {
    let at = (y * WIDTH as usize + x) * 4;
    [rgba[at], rgba[at + 1], rgba[at + 2]].map(i32::from)
}

/// Brighter than `sky` by more than `margin` in every channel.
fn above(color: [i32; 3], sky: [i32; 3], margin: i32) -> bool {
    (0..3).all(|channel| color[channel] > sky[channel] + margin)
}

fn same(a: [i32; 3], b: [i32; 3]) -> bool {
    (0..3).all(|channel| (a[channel] - b[channel]).abs() <= 1)
}

/// Points across the disc: left of centre, centre, right of centre, and
/// near the right rim.
fn across() -> [(usize, usize); 4] {
    let (x, y) = CENTRE;
    let half = RADIUS_PIXELS * 6 / 10;
    [
        (x - half, y),
        (x, y),
        (x + half, y),
        (x + RADIUS_PIXELS - 2, y),
    ]
}

#[test]
fn its_phase_lights_the_limb_toward_the_sun_and_earthshine_the_rest() {
    let render = |op: Option<RenderDiff>| {
        let mut harness = scene(NIGHT);
        if let Some(op) = op {
            harness.apply(vec![op]);
        }
        look(&mut harness)
    };
    let sky = render(None);
    let [left, centre, right, rim] = across();
    let sky_at = |at| pixel(&sky, at);
    let full = render(Some(moon(1.0, 1.0, 0.0)));
    for at in [left, centre, right, rim] {
        assert!(
            above(pixel(&full, at), sky_at(at), 100),
            "full at {at:?}: {:?}",
            pixel(&full, at)
        );
    }
    let half = render(Some(moon(0.5, 1.0, 0.0)));
    assert!(
        above(pixel(&half, right), sky_at(right), 100),
        "half, right: {:?}",
        pixel(&half, right)
    );
    assert!(
        same(pixel(&half, left), sky_at(left)),
        "half, left: {:?}",
        pixel(&half, left)
    );
    let crescent = render(Some(moon(0.15, 1.0, 0.0)));
    assert!(
        above(pixel(&crescent, rim), sky_at(rim), 60),
        "crescent, rim: {:?}",
        pixel(&crescent, rim)
    );
    assert!(
        same(pixel(&crescent, centre), sky_at(centre)),
        "crescent, centre: {:?}",
        pixel(&crescent, centre)
    );
    // A new moon is unseen, unless earthshine lights its face.
    assert_eq!(render(Some(moon(0.0, 1.0, 0.0))), sky);
    let shine = render(Some(moon(0.0, 1.0, 0.15)));
    assert!(
        above(pixel(&shine, centre), sky_at(centre), 5),
        "earthshine: {:?}",
        pixel(&shine, centre)
    );
    // Taken away, the sky is as it was.
    let mut harness = scene(NIGHT);
    harness.apply(vec![moon(1.0, 1.0, 0.0)]);
    look(&mut harness);
    harness.apply(vec![RenderDiff::SetMoon { moon: None }]);
    assert_eq!(look(&mut harness), sky);
}

#[test]
fn by_day_a_half_moon_is_a_pale_disc_over_the_sky() {
    let mut harness = scene(DAY);
    let sky = look(&mut harness);
    harness.apply(vec![moon(0.5, 0.4, 0.0)]);
    let day = look(&mut harness);
    let [left, _, right, _] = across();
    assert!(
        above(pixel(&day, right), pixel(&sky, right), 20),
        "lit: {:?} over {:?}",
        pixel(&day, right),
        pixel(&sky, right)
    );
    assert!(
        same(pixel(&day, left), pixel(&sky, left)),
        "dark half shows the sky: {:?}",
        pixel(&day, left)
    );
}

#[test]
fn clouds_and_the_world_cover_it_and_its_bloom() {
    let render = |moon_on: bool, cover: bool, overcast: bool, bloom: bool| {
        let mut harness = scene(NIGHT);
        if moon_on {
            harness.apply(vec![moon(1.0, 40.0, 0.0)]);
        }
        if bloom {
            harness.apply(vec![RenderDiff::SetBloom {
                bloom: Some(BloomDescriptor {
                    threshold: 1.0,
                    intensity: 1.0,
                }),
            }]);
        }
        if overcast {
            harness.apply(vec![RenderDiff::SetClouds {
                clouds: Some(CloudsDescriptor {
                    coverage: 1.0,
                    drift: [0.0, 0.0],
                    altitude: 1500.0,
                    scale: 600.0,
                    color: [1.0; 3],
                    thickness: 0.0,
                    kind: CloudKind::Stratus,
                    twilight_degrees: 0.0,
                }),
            }]);
        }
        if cover {
            // A ridge 30 m ahead, rising well past the moon.
            harness.apply(vec![
                RenderDiff::DefineMaterial {
                    material: material("material/ridge", [0.2, 0.25, 0.2, 1.0], None),
                },
                static_mesh(
                    "mesh/ridge",
                    box_mesh([-40.0, -5.0, -31.0], [40.0, 12.0, -30.0], |_| 0),
                    "material/ridge",
                ),
                instance(20, None, "mesh/ridge", transform([0.0; 3], 0.0, [1.0; 3])),
            ]);
        }
        look(&mut harness)
    };
    // Open, it blooms past its rim.
    let beside = (CENTRE.0 + RADIUS_PIXELS + 8, CENTRE.1);
    let open = render(true, false, false, true);
    assert!(
        above(
            pixel(&open, beside),
            pixel(&render(false, false, false, true), beside),
            15
        ),
        "it blooms: {:?}",
        pixel(&open, beside)
    );
    // Behind the ridge neither it nor its bloom shows.
    assert_eq!(
        render(true, true, false, true),
        render(false, true, false, true)
    );
}

#[test]
fn an_overcast_covers_it_as_much_as_it_covers_the_sky() {
    // High in the sky, where the flat layer is whole (toward the horizon it
    // fades into the panorama, over the moon as over the sun).
    let render = |moon_on: bool, overcast: bool| {
        let mut harness = scene(NIGHT);
        let up = 40.0f32.to_radians();
        if moon_on {
            harness.apply(vec![RenderDiff::SetMoon {
                moon: Some(MoonDescriptor {
                    direction: [0.0, up.sin(), -up.cos()],
                    radius_degrees: 4.0,
                    color: [1.0; 3],
                    radiance: 1.0,
                    phase: 1.0,
                    earthshine: 0.0,
                    halo: 0.0,
                    light_intensity: 0.0,
                }),
            }]);
        }
        if overcast {
            harness.apply(vec![RenderDiff::SetClouds {
                clouds: Some(CloudsDescriptor {
                    coverage: 1.0,
                    drift: [0.0, 0.0],
                    altitude: 1500.0,
                    scale: 600.0,
                    color: [1.0; 3],
                    thickness: 0.0,
                    kind: CloudKind::Stratus,
                    twilight_degrees: 0.0,
                }),
            }]);
        }
        harness.render(&camera([0.0, 1.0, 0.0], 0.0, 40.0)).1
    };
    let centre = (160, 90);
    let gain = |overcast| {
        pixel(&render(true, overcast), centre)[1] - pixel(&render(false, overcast), centre)[1]
    };
    let (open, covered) = (gain(false), gain(true));
    assert!(
        open > 150 && covered * 3 < open,
        "the overcast takes most of it: {covered} of {open}"
    );
}

#[test]
fn it_changes_nothing_else() {
    // Behind the camera it is unseen, and the haze and the clouds keep the
    // sky's sun.
    let render = |moon_on: bool| {
        let mut harness = scene(DAY);
        harness.apply(vec![
            RenderDiff::SetAtmosphere {
                atmosphere: Some(AtmosphereDescriptor {
                    haze_color: [1.0, 0.5, 0.2],
                    haze_exponent: 4.0,
                    sun_radius_degrees: 2.0,
                    sky_sun_direction: [0.3, 0.2, -1.0],
                    sky_sun_color: [1.0, 0.8, 0.6],
                    sky_sun_intensity: 1.0,
                    ..Default::default()
                }),
            },
            RenderDiff::SetFog {
                fog: Some(FogDescriptor::Exponential {
                    color: [0.5, 0.6, 0.7],
                    density: 0.02,
                }),
            },
            RenderDiff::SetClouds {
                clouds: Some(CloudsDescriptor {
                    coverage: 0.5,
                    drift: [0.0, 0.0],
                    altitude: 1500.0,
                    scale: 600.0,
                    color: [1.0; 3],
                    thickness: 0.0,
                    kind: CloudKind::Cumulus,
                    twilight_degrees: 0.0,
                }),
            },
        ]);
        if moon_on {
            harness.apply(vec![RenderDiff::SetMoon {
                moon: Some(MoonDescriptor {
                    direction: [0.0, 0.3, 1.0],
                    radius_degrees: 4.0,
                    color: [1.0; 3],
                    radiance: 1.0,
                    phase: 1.0,
                    earthshine: 0.0,
                    halo: 0.5,
                    light_intensity: 0.0,
                }),
            }]);
        }
        harness.renderer.set_animation_time(2.0);
        harness.render(&camera([0.0, 1.0, 0.0], 0.0, 10.0)).1
    };
    assert_eq!(render(true), render(false));
}

#[test]
fn its_light_lights_the_clouds_at_night_toward_it_and_none_leaves_them_as_they_were() {
    // Broken cumulus at midnight (the sky's sun far below the horizon), a
    // full moon 60° up to the right (`side` 1) or left (-1), flat and
    // volumetric; mean colour of the left and right halves of the view up
    // into the clouds.
    let render = |quality, light: f32, side: f32| {
        let mut harness = Harness::new(RendererOptions {
            default_world_lights: false,
            volumetric_clouds: quality,
            ..RendererOptions::default()
        });
        let (width, height) = (64, 32);
        let sky: Vec<u8> = (0..width * height)
            .flat_map(|texel| {
                if texel / width < height / 2 {
                    NIGHT
                } else {
                    [10, 10, 10, 255]
                }
            })
            .collect();
        let texture =
            harness
                .resources
                .texture("texture/sky", width, height, &sky, TextureWrap::Clamp);
        let up = 60.0f32.to_radians();
        harness.apply(vec![
            RenderDiff::DefineTexture { texture },
            RenderDiff::SetSkyBackground {
                background: Some(SkyBackgroundDescriptor {
                    texture: "texture/sky".to_owned(),
                    blend: None,
                }),
            },
            RenderDiff::SetAtmosphere {
                atmosphere: Some(AtmosphereDescriptor {
                    sky_sun_direction: [0.0, -0.7, 0.7],
                    sky_sun_color: [1.0; 3],
                    sky_sun_intensity: 1.0,
                    ..Default::default()
                }),
            },
            RenderDiff::SetClouds {
                clouds: Some(CloudsDescriptor {
                    coverage: 0.55,
                    drift: [0.0, 0.0],
                    altitude: 1500.0,
                    scale: 600.0,
                    color: [1.0; 3],
                    thickness: 0.0,
                    kind: CloudKind::Cumulus,
                    twilight_degrees: 8.0,
                }),
            },
            RenderDiff::SetMoon {
                moon: Some(MoonDescriptor {
                    direction: [side * up.cos() * 0.7, up.sin(), -up.cos() * 0.7],
                    radius_degrees: 1.0,
                    color: [0.85, 0.9, 1.0],
                    radiance: 1.0,
                    phase: 1.0,
                    earthshine: 0.0,
                    halo: 0.0,
                    light_intensity: light,
                }),
            },
        ]);
        harness.renderer.set_animation_time(2.0);
        let view = camera([0.0, 1.0, 0.0], 0.0, 30.0);
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
    let mean = |image: &[u8], columns: std::ops::Range<usize>| {
        let mut sum = 0.0;
        let mut count = 0.0;
        for y in 20..140usize {
            for x in columns.clone() {
                sum += f64::from(image[(y * WIDTH as usize + x) * 4 + 2]);
                count += 1.0;
            }
        }
        sum / count
    };
    let (left, right) = (0..WIDTH as usize / 2, WIDTH as usize / 2..WIDTH as usize);
    for quality in [VolumetricCloudsQuality::Off, VolumetricCloudsQuality::High] {
        let Some(dark) = render(quality, 0.0, 1.0) else {
            continue;
        };
        // No light leaves them as a moon never lit them.
        assert_eq!(
            dark,
            render(quality, 0.0, -1.0).expect("drawn"),
            "{quality:?}"
        );
        let (on_right, on_left) = (
            render(quality, 0.3, 1.0).expect("drawn"),
            render(quality, 0.3, -1.0).expect("drawn"),
        );
        let gain = |lit: &[u8], columns: std::ops::Range<usize>| {
            mean(lit, columns.clone()) - mean(&dark, columns)
        };
        let whole = 0..WIDTH as usize;
        assert!(
            gain(&on_right, whole.clone()) > 8.0,
            "{quality:?}: the moon lights the clouds: {:.1}",
            gain(&on_right, whole)
        );
        // Each half takes more of its light with the moon on its side.
        assert!(
            gain(&on_right, right.clone()) > gain(&on_left, right.clone())
                && gain(&on_left, left.clone()) > gain(&on_right, left.clone()),
            "{quality:?}: toward the moon: right {:.1} against {:.1}, left {:.1} against {:.1}",
            gain(&on_right, right.clone()),
            gain(&on_left, right.clone()),
            gain(&on_left, left.clone()),
            gain(&on_right, left.clone())
        );
    }
}
