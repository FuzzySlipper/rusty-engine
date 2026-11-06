//! The moons (`SetMoon`; `sky.wgsl` `fs_moon`): each a disc of its own beside the
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
        slot: 0,
        moon: Some(MoonDescriptor {
            direction: [0.0, up.sin(), -up.cos()],
            radius_degrees: 4.0,
            color: [1.0, 0.95, 0.85],
            radiance,
            phase,
            earthshine,
            halo: 0.0,
            light_intensity: 0.0,
            ..Default::default()
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
    harness.apply(vec![RenderDiff::SetMoon {
        slot: 0,
        moon: None,
    }]);
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
                    pixel_metres: 0.0,
                    shade_steps: 0,
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
                slot: 0,
                moon: Some(MoonDescriptor {
                    direction: [0.0, up.sin(), -up.cos()],
                    radius_degrees: 4.0,
                    color: [1.0; 3],
                    radiance: 1.0,
                    phase: 1.0,
                    earthshine: 0.0,
                    halo: 0.0,
                    light_intensity: 0.0,
                    ..Default::default()
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
                    pixel_metres: 0.0,
                    shade_steps: 0,
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
                    pixel_metres: 0.0,
                    shade_steps: 0,
                }),
            },
        ]);
        if moon_on {
            harness.apply(vec![RenderDiff::SetMoon {
                slot: 0,
                moon: Some(MoonDescriptor {
                    direction: [0.0, 0.3, 1.0],
                    radius_degrees: 4.0,
                    color: [1.0; 3],
                    radiance: 1.0,
                    phase: 1.0,
                    earthshine: 0.0,
                    halo: 0.5,
                    light_intensity: 0.0,
                    ..Default::default()
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
                    pixel_metres: 0.0,
                    shade_steps: 0,
                }),
            },
            RenderDiff::SetMoon {
                slot: 0,
                moon: Some(MoonDescriptor {
                    direction: [side * up.cos() * 0.7, up.sin(), -up.cos() * 0.7],
                    radius_degrees: 1.0,
                    color: [0.85, 0.9, 1.0],
                    radiance: 1.0,
                    phase: 1.0,
                    earthshine: 0.0,
                    halo: 0.0,
                    light_intensity: light,
                    ..Default::default()
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

/// A moon in `slot` 5° up, `offset` degrees to the right of straight ahead, of
/// this radius and colour, full and bright.
fn moon_at(slot: u32, offset: f32, radius_degrees: f32, color: [f32; 3]) -> RenderDiff {
    let (up, right) = (MOON_UP.to_radians(), offset.to_radians());
    RenderDiff::SetMoon {
        slot,
        moon: Some(MoonDescriptor {
            direction: [right.sin() * up.cos(), up.sin(), -right.cos() * up.cos()],
            radius_degrees,
            color,
            radiance: 1.0,
            phase: 1.0,
            ..Default::default()
        }),
    }
}

const RED: [f32; 3] = [1.0, 0.1, 0.1];
const BLUE: [f32; 3] = [0.1, 0.2, 1.0];

/// Each moon has its own size, colour, phase and place, and setting one
/// leaves the others: slot 0 keeps its meaning for a product with one moon.
#[test]
fn moons_in_their_slots_stand_apart_and_move_on_their_own() {
    let mut harness = scene(NIGHT);
    let sky = pixel(&look(&mut harness), CENTRE);
    harness.apply(vec![moon_at(0, -8.0, 3.0, RED), moon_at(1, 8.0, 2.0, BLUE)]);
    let both = look(&mut harness);
    // 8° across is 24 pixels at this view.
    let (left, right) = ((CENTRE.0 - 24, CENTRE.1), (CENTRE.0 + 24, CENTRE.1));
    let [r, _, b] = pixel(&both, left);
    assert!(r > b + 60, "the red moon: {:?}", pixel(&both, left));
    let [r, _, b] = pixel(&both, right);
    assert!(b > r + 60, "the blue moon: {:?}", pixel(&both, right));
    assert!(same(pixel(&both, CENTRE), sky), "the sky between them");

    harness.apply(vec![moon_at(1, 0.0, 2.0, BLUE)]);
    let moved = look(&mut harness);
    assert!(same(pixel(&moved, right), sky), "the blue moon left");
    assert!(above(pixel(&moved, CENTRE), sky, 40), "and stands ahead");
    assert_eq!(pixel(&moved, left), pixel(&both, left), "the red one stays");

    harness.apply(vec![RenderDiff::SetMoon {
        slot: 0,
        moon: None,
    }]);
    let cleared = look(&mut harness);
    assert!(same(pixel(&cleared, left), sky), "slot 0 is gone");
    assert_eq!(
        pixel(&cleared, CENTRE),
        pixel(&moved, CENTRE),
        "slot 1 stays"
    );
}

/// A higher slot stands in front, its whole disc hiding the moon behind it,
/// the dark part of a new moon too; the sum of two added discs never shows.
#[test]
fn a_higher_slot_passes_in_front_of_a_lower_one() {
    let render = |back: u32, front: u32| {
        let mut harness = scene(NIGHT);
        harness.apply(vec![
            moon_at(back, 0.0, 4.0, RED),
            moon_at(front, 0.0, 2.0, BLUE),
        ]);
        look(&mut harness)
    };
    let blue_ahead = render(0, 1);
    let [r, _, b] = pixel(&blue_ahead, CENTRE);
    assert!(b > r + 60, "the higher slot's blue shows: {r} {b}");
    // Beyond the small disc, the larger one shows around it.
    let ring = (CENTRE.0 + 9, CENTRE.1);
    let [r, _, b] = pixel(&blue_ahead, ring);
    assert!(r > b + 60, "the red ring shows around it: {r} {b}");

    let red_ahead = render(1, 0);
    let [r, _, b] = pixel(&red_ahead, CENTRE);
    assert!(
        r > b + 60,
        "red in the higher slot covers the blue: {r} {b}"
    );

    let mut harness = scene(NIGHT);
    let sky = pixel(&look(&mut harness), CENTRE);
    let RenderDiff::SetMoon {
        moon: Some(new), ..
    } = moon_at(1, 0.0, 2.0, BLUE)
    else {
        unreachable!("a set moon");
    };
    harness.apply(vec![
        moon_at(0, 0.0, 4.0, RED),
        RenderDiff::SetMoon {
            slot: 1,
            moon: Some(MoonDescriptor { phase: 0.0, ..new }),
        },
    ]);
    assert!(
        same(pixel(&look(&mut harness), CENTRE), sky),
        "a new moon's dark disc hides the moon behind it"
    );
}

/// Four quadrants a texel each: red and green over blue and white.
fn quadrants(harness: &mut Harness, filter: TextureFilter) -> RenderDiff {
    let rgba: Vec<u8> = [
        [255, 0, 0, 255],
        [0, 255, 0, 255],
        [0, 0, 255, 255],
        [255, 255, 255, 255],
    ]
    .concat();
    let mut texture = harness
        .resources
        .texture("texture/moon", 2, 2, &rgba, TextureWrap::Clamp);
    texture.filter = filter;
    RenderDiff::DefineTexture { texture }
}

fn textured(uv_min: [f32; 2], uv_max: [f32; 2], phase: f32) -> RenderDiff {
    let RenderDiff::SetMoon {
        moon: Some(moon), ..
    } = moon_at(0, 0.0, 4.0, [1.0; 3])
    else {
        unreachable!("a set moon");
    };
    RenderDiff::SetMoon {
        slot: 0,
        moon: Some(MoonDescriptor {
            phase,
            texture: Some("texture/moon".to_owned()),
            uv_min,
            uv_max,
            ..moon
        }),
    }
}

/// A texture stands upright on the disc, sampled with its own filter so
/// nearest stays hard; a region picks a frame of an atlas, and `phase` 1 draws
/// it as it is while a lesser phase still shades it.
#[test]
fn a_moons_texture_draws_upright_with_its_filter_and_a_chosen_frame() {
    // 4° is 12 pixels at this view: the quadrants' centres are 5 away.
    let (top_left, top_right, bottom_left, bottom_right) = (
        (CENTRE.0 - 5, CENTRE.1 - 5),
        (CENTRE.0 + 5, CENTRE.1 - 5),
        (CENTRE.0 - 5, CENTRE.1 + 5),
        (CENTRE.0 + 5, CENTRE.1 + 5),
    );
    let mut harness = scene(NIGHT);
    let define = quadrants(&mut harness, TextureFilter::Nearest);
    harness.apply(vec![define, textured([0.0; 2], [0.0; 2], 1.0)]);
    let whole = look(&mut harness);
    // A moon adds over the sky, so each texel lands on the night's colour.
    let [red, green, blue] = [[255, 12, 25], [10, 255, 25], [10, 12, 255]];
    assert_eq!(pixel(&whole, top_left), red, "red top left");
    assert_eq!(pixel(&whole, top_right), green, "green top right");
    assert_eq!(pixel(&whole, bottom_left), blue, "blue below it");
    assert_eq!(pixel(&whole, bottom_right), [255; 3], "white bottom right");
    // Nearest: the colour holds up to the seam, with nothing between.
    let seam = |x| pixel(&whole, (x, top_left.1));
    assert_eq!(seam(CENTRE.0 - 1), red, "left of the seam");
    assert_eq!(seam(CENTRE.0 + 1), green, "right of the seam");

    harness.apply(vec![textured([0.5, 0.5], [1.0, 1.0], 1.0)]);
    let frame = look(&mut harness);
    assert_eq!(
        pixel(&frame, top_left),
        [255; 3],
        "the frame fills the disc"
    );
    assert_eq!(pixel(&frame, bottom_right), [255; 3], "all of it");

    harness.apply(vec![textured([0.5, 0.5], [1.0, 1.0], 0.5)]);
    let shaded = look(&mut harness);
    let (lit, dark) = (pixel(&shaded, top_right), pixel(&shaded, top_left));
    assert!(
        lit[0] > dark[0] + 60,
        "the phase shades it: {lit:?} {dark:?}"
    );
}

/// The moons' light on the clouds is one light: two moons side by side give
/// what one moon of their summed intensity gives from there.
#[test]
fn two_moons_light_the_clouds_as_their_sum() {
    let lit = |moons: Vec<RenderDiff>| {
        let mut harness = scene(NIGHT);
        harness.apply(vec![RenderDiff::SetClouds {
            clouds: Some(CloudsDescriptor {
                coverage: 0.7,
                drift: [0.0, 0.0],
                altitude: 1500.0,
                scale: 1500.0,
                color: [1.0; 3],
                thickness: 0.0,
                kind: CloudKind::Cumulus,
                twilight_degrees: 0.0,
                pixel_metres: 0.0,
                shade_steps: 0,
            }),
        }]);
        harness.apply(moons);
        look(&mut harness)
    };
    // No disc: only their light reaches the frame.
    let RenderDiff::SetMoon {
        moon: Some(moon), ..
    } = moon_at(0, 0.0, 0.0, [1.0; 3])
    else {
        unreachable!("a set moon");
    };
    let shining = |slot, light_intensity| RenderDiff::SetMoon {
        slot,
        moon: Some(MoonDescriptor {
            light_intensity,
            ..moon.clone()
        }),
    };
    let one = lit(vec![shining(0, 0.6)]);
    let two = lit(vec![shining(0, 0.25), shining(1, 0.35)]);
    let dim = lit(vec![shining(0, 0.25)]);
    assert_eq!(one, two, "their light adds as one");
    assert_ne!(dim, two, "and each gives its own");
}
