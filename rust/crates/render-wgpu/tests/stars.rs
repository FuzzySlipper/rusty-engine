//! The star field (`SetStars`; `stars.wgsl`): stars appear brightest first
//! as the limiting magnitude rises, turn about a pole that stands still,
//! keep their light as they move and across resolutions, and lie under the
//! clouds and behind the world; the band adds over the sky; off draws
//! nothing. Same harness as `tests/screenshots.rs`.

mod support;

use render_model::*;
use render_wgpu::{OffscreenTarget, RendererOptions};
use support::*;

const NIGHT: [u8; 4] = [8, 10, 20, 255];
const CATALOGUE: &str = "stars/fixture";
const BAND: &str = "texture/band";

/// A star at least this much brighter than the sky in its green channel
/// is lit.
const LIT: i32 = 12;

/// A deterministic catalogue: `count` stars spread over the sphere with
/// magnitudes from -1 to 6.5, more of them faint, and a pole star of
/// magnitude 0 at the catalogue's +Y.
fn catalogue(count: usize) -> Vec<Star> {
    let mut state = 0x2545_f491_u32;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        state as f32 / u32::MAX as f32
    };
    let mut stars = vec![Star {
        direction: [0.0, 1.0, 0.0],
        magnitude: 0.0,
        color: [1.0; 3],
    }];
    for _ in 0..count {
        let z = next() * 2.0 - 1.0;
        let angle = next() * std::f32::consts::TAU;
        let ring = (1.0 - z * z).sqrt();
        // Faint stars are many: magnitude rises with the square root.
        let magnitude = -1.0 + 7.5 * next().sqrt();
        stars.push(Star {
            direction: [ring * angle.cos(), z, ring * angle.sin()],
            magnitude,
            color: [1.0, 0.95, 0.9],
        });
    }
    stars
}

/// A dark night panorama, the catalogue as a resource and a band texture
/// lit along the catalogue's equator.
fn scene(stars: &[Star]) -> Harness {
    let mut harness = Harness::new(RendererOptions {
        default_world_lights: false,
        ..RendererOptions::default()
    });
    let (width, height) = (64, 32);
    let sky: Vec<u8> = (0..width * height)
        .flat_map(|texel| {
            if texel / width < height / 2 {
                NIGHT
            } else {
                [20, 18, 18, 255]
            }
        })
        .collect();
    let panorama =
        harness
            .resources
            .texture("texture/sky", width, height, &sky, TextureWrap::Clamp);
    let band: Vec<u8> = (0..width * height)
        .flat_map(|texel| {
            let row = texel / width;
            if (14..18).contains(&row) {
                [90, 80, 110, 255]
            } else {
                [0, 0, 0, 255]
            }
        })
        .collect();
    let band = harness
        .resources
        .texture(BAND, width, height, &band, TextureWrap::Repeat);
    harness
        .resources
        .insert(CATALOGUE, pack_star_catalogue(stars));
    harness.apply(vec![
        RenderDiff::DefineTexture { texture: panorama },
        RenderDiff::DefineTexture { texture: band },
        RenderDiff::SetSkyBackground {
            background: Some(SkyBackgroundDescriptor {
                texture: "texture/sky".to_owned(),
                blend: None,
            }),
        },
    ]);
    harness
}

fn field(orientation: [f32; 4], limit: f32, band: f32) -> RenderDiff {
    RenderDiff::SetStars {
        stars: Some(StarsDescriptor {
            catalogue: CATALOGUE.to_owned(),
            band: Some(BAND.to_owned()),
            orientation,
            limiting_magnitude: limit,
            brightness: 1.0,
            band_strength: band,
            twinkle: 0.0,
        }),
    }
}

const IDENTITY: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

fn quaternion(axis: [f32; 3], degrees: f32) -> [f32; 4] {
    let half = degrees.to_radians() / 2.0;
    let length = axis.iter().map(|value| value * value).sum::<f32>().sqrt();
    let [x, y, z] = axis.map(|value| value / length * half.sin());
    [x, y, z, half.cos()]
}

fn multiply(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let [ax, ay, az, aw] = a;
    let [bx, by, bz, bw] = b;
    [
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
        aw * bw - ax * bx - ay * by - az * bz,
    ]
}

fn green(rgba: &[u8], width: usize, (x, y): (usize, usize)) -> i32 {
    i32::from(rgba[(y * width + x) * 4 + 1])
}

/// The pixels lit above `sky` (same size images).
fn lit(rgba: &[u8], sky: &[u8]) -> Vec<usize> {
    (0..rgba.len() / 4)
        .filter(|&pixel| i32::from(rgba[pixel * 4 + 1]) > i32::from(sky[pixel * 4 + 1]) + LIT)
        .collect()
}

fn up(harness: &mut Harness, pitch: f64) -> Vec<u8> {
    harness.render(&camera([0.0, 1.0, 0.0], 0.0, pitch)).1
}

#[test]
fn the_brightest_stars_come_first_as_the_limit_rises() {
    let stars = catalogue(3000);
    let mut harness = scene(&stars);
    let sky = up(&mut harness, 50.0);
    let mut previous: Vec<usize> = Vec::new();
    let mut counts = Vec::new();
    for limit in [1.0, 2.0, 3.0, 4.0, 5.0, 6.0] {
        harness.apply(vec![field(IDENTITY, limit, 0.0)]);
        let shown = lit(&up(&mut harness, 50.0), &sky);
        // What showed still shows.
        let kept = previous
            .iter()
            .filter(|pixel| shown.contains(pixel))
            .count();
        assert!(
            kept * 10 >= previous.len() * 9,
            "at {limit}: {kept} of {} kept",
            previous.len()
        );
        counts.push(shown.len());
        previous = shown;
    }
    assert!(
        counts.windows(2).all(|pair| pair[1] > pair[0]) && counts[0] > 0,
        "more stars as the limit rises: {counts:?}"
    );
    assert!(counts[5] > counts[0] * 8, "{counts:?}");
}

#[test]
fn the_field_turns_about_a_pole_that_stands_still() {
    // The pole and one other star; the pole tilted 40° up toward -Z, the
    // field spun about it.
    let stars = [
        Star {
            direction: [0.0, 1.0, 0.0],
            magnitude: -1.0,
            color: [1.0; 3],
        },
        Star {
            direction: [0.3, 0.95, 0.0],
            magnitude: -1.0,
            color: [1.0; 3],
        },
    ];
    let tilt = quaternion([1.0, 0.0, 0.0], -50.0);
    let render = |spin: f32| {
        let mut harness = scene(&stars);
        let orientation = multiply(tilt, quaternion([0.0, 1.0, 0.0], spin));
        harness.apply(vec![field(orientation, 6.0, 0.0)]);
        up(&mut harness, 40.0)
    };
    let sky = {
        let mut harness = scene(&stars);
        up(&mut harness, 40.0)
    };
    // The centre of the view is the pole.
    let pole = (WIDTH as usize / 2, HEIGHT as usize / 2);
    let (first, later) = (render(0.0), render(60.0));
    for image in [&first, &later] {
        assert!(green(image, WIDTH as usize, pole) > green(&sky, WIDTH as usize, pole) + 100);
    }
    let (first, later) = (lit(&first, &sky), lit(&later, &sky));
    let moved: Vec<_> = first
        .iter()
        .filter(|pixel| !later.contains(pixel))
        .collect();
    assert!(!moved.is_empty(), "the other star wheels");
    let pole_pixel = pole.1 * WIDTH as usize + pole.0;
    assert!(first.contains(&pole_pixel) && later.contains(&pole_pixel));
}

#[test]
fn a_star_keeps_its_light_as_it_moves_and_across_resolutions() {
    let stars = [Star {
        direction: [0.0, 0.0, -1.0],
        magnitude: 1.0,
        color: [1.0; 3],
    }];
    // The star's light summed over the view, less the sky's.
    let total = |harness: &mut Harness, width: u32, height: u32, degrees: f32| {
        harness.target = OffscreenTarget::new(&harness.gpu, width, height, 4);
        let view = camera([0.0, 1.0, 0.0], 0.0, 0.0);
        harness.renderer.render_offscreen(&view, &harness.target);
        let sky = harness.target.read_rgba(&harness.gpu);
        harness.apply(vec![field(quaternion([0.0, 1.0, 0.0], degrees), 6.0, 0.0)]);
        harness.renderer.render_offscreen(&view, &harness.target);
        let lit = harness.target.read_rgba(&harness.gpu);
        harness.apply(vec![RenderDiff::SetStars { stars: None }]);
        lit.iter()
            .zip(&sky)
            .skip(1)
            .step_by(4)
            .map(|(a, b)| f64::from(*a) - f64::from(*b))
            .sum::<f64>()
    };
    let mut harness = scene(&stars);
    // A tenth of a pixel at a time across two pixels.
    let pixel_degrees = 60.0 / HEIGHT as f32;
    let totals: Vec<f64> = (0..20)
        .map(|step| {
            total(
                &mut harness,
                WIDTH,
                HEIGHT,
                step as f32 * 0.1 * pixel_degrees,
            )
        })
        .collect();
    let (least, most) = totals
        .iter()
        .fold((f64::MAX, f64::MIN), |(least, most), value| {
            (least.min(*value), most.max(*value))
        });
    assert!(
        least > 100.0 && most < least * 1.15,
        "as it moves: {least} to {most}"
    );
    let twice = total(&mut harness, WIDTH * 2, HEIGHT * 2, 0.0);
    assert!(
        (twice - totals[0]).abs() < totals[0] * 0.2,
        "at twice the resolution: {twice} against {}",
        totals[0]
    );
}

#[test]
fn clouds_cover_the_field_and_the_world_hides_it() {
    let stars = catalogue(3000);
    // Up into the sky, with an overcast or not.
    let shown = |overcast: bool| {
        let mut harness = scene(&stars);
        let sky = up(&mut harness, 60.0);
        harness.apply(vec![field(IDENTITY, 6.0, 0.0)]);
        if overcast {
            harness.apply(vec![RenderDiff::SetClouds {
                clouds: Some(CloudsDescriptor {
                    coverage: 1.0,
                    drift: [0.0, 0.0],
                    altitude: 1500.0,
                    scale: 600.0,
                    color: [0.2; 3],
                    thickness: 0.0,
                    kind: CloudKind::Stratus,
                    twilight_degrees: 0.0,
                }),
            }]);
        }
        let image = up(&mut harness, 60.0);
        lit(&image, &sky).len()
    };
    let (clear, overcast) = (shown(false), shown(true));
    assert!(
        clear > 100 && overcast * 4 < clear,
        "clear {clear}, overcast {overcast}"
    );
    // Level at the horizon: stars show below it over the panorama, and a
    // ground hides them.
    let below = |ground: bool| {
        let mut harness = scene(&stars);
        if ground {
            harness.apply(vec![
                RenderDiff::DefineMaterial {
                    material: material("material/ground", [0.1, 0.1, 0.1, 1.0], None),
                },
                static_mesh(
                    "mesh/ground",
                    box_mesh([-90.0, -1.0, -90.0], [90.0, 0.0, 90.0], |_| 0),
                    "material/ground",
                ),
                instance(20, None, "mesh/ground", transform([0.0; 3], 0.0, [1.0; 3])),
            ]);
        }
        let sky = up(&mut harness, 0.0);
        harness.apply(vec![field(IDENTITY, 6.0, 0.0)]);
        let image = up(&mut harness, 0.0);
        let horizon = HEIGHT as usize / 2 + 4;
        lit(&image, &sky)
            .into_iter()
            .filter(|pixel| pixel / WIDTH as usize > horizon)
            .count()
    };
    assert!(below(false) > 20, "{}", below(false));
    assert_eq!(below(true), 0);
}

#[test]
fn the_band_adds_over_the_sky_and_off_draws_nothing() {
    let stars = catalogue(10);
    let mut harness = scene(&stars);
    // Level, the catalogue's equator along the horizon.
    let sky = up(&mut harness, 5.0);
    harness.apply(vec![field(IDENTITY, -5.0, 1.0)]);
    let band = up(&mut harness, 5.0);
    let row = |rgba: &[u8], y: usize| -> i32 {
        (0..WIDTH as usize)
            .map(|x| green(rgba, WIDTH as usize, (x, y)))
            .sum::<i32>()
            / WIDTH as i32
    };
    let horizon = HEIGHT as usize / 2 + 14;
    assert!(
        row(&band, horizon) > row(&sky, horizon) + 20,
        "{} {}",
        row(&band, horizon),
        row(&sky, horizon)
    );
    assert_eq!(
        row(&band, 10),
        row(&sky, 10),
        "away from the band the sky is as it was"
    );
    harness.apply(vec![RenderDiff::SetStars { stars: None }]);
    assert_eq!(up(&mut harness, 5.0), sky);
}

#[test]
fn the_moons_whole_disc_hides_the_stars_and_band_behind_it() {
    // A star straight ahead behind the moon, one 10° to its right, and the
    // band along the horizon behind it too; the moon new (dark) or a
    // crescent lit on its far side.
    let stars = [
        Star {
            direction: [0.0, 0.0, -1.0],
            magnitude: 0.0,
            color: [1.0; 3],
        },
        Star {
            direction: [0.17, 0.0, -0.98],
            magnitude: 0.0,
            color: [1.0; 3],
        },
    ];
    let render = |phase: f32, field_on: bool| {
        let mut harness = scene(&stars);
        harness.apply(vec![
            // The sky's sun to the left, so a crescent's lit limb is away
            // from the star to the right.
            RenderDiff::SetAtmosphere {
                atmosphere: Some(AtmosphereDescriptor {
                    sky_sun_direction: [-1.0, -0.3, 0.0],
                    sky_sun_color: [1.0; 3],
                    sky_sun_intensity: 1.0,
                    ..Default::default()
                }),
            },
            RenderDiff::SetMoon {
                moon: Some(MoonDescriptor {
                    direction: [0.0, 0.0, -1.0],
                    radius_degrees: 4.0,
                    color: [1.0; 3],
                    radiance: 1.0,
                    phase,
                    earthshine: 0.0,
                    halo: 0.0,
                    light_intensity: 0.0,
                }),
            },
        ]);
        if field_on {
            harness.apply(vec![field(IDENTITY, 6.0, 1.0)]);
        }
        harness.render(&camera([0.0, 0.0, 0.0], 0.0, 0.0)).1
    };
    let centre = (WIDTH as usize / 2, HEIGHT as usize / 2);
    // 10° right of centre at the harness's 60° view.
    let beside = (centre.0 + 52, centre.1);
    for phase in [0.0, 0.15] {
        let (with, without) = (render(phase, true), render(phase, false));
        assert_eq!(
            green(&with, WIDTH as usize, centre),
            green(&without, WIDTH as usize, centre),
            "phase {phase}: the star and band behind the moon are hidden"
        );
        assert!(
            green(&with, WIDTH as usize, beside) > green(&without, WIDTH as usize, beside) + 50,
            "phase {phase}: the star beside it shows"
        );
    }
}
