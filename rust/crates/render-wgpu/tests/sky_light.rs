//! The sky's light (`sky_light.rs`): the background (a clear colour, a sky
//! panorama or two blended) lighting the world through the standard shader,
//! off by default; while clouds or a backdrop draw, built from a capture of
//! them around the camera. Same harness as `tests/screenshots.rs`.

mod support;

use render_model::*;
use render_wgpu::RendererOptions;
use support::*;

/// Only the sky lights the world: no default lights.
fn unlit_world() -> Harness {
    Harness::new(RendererOptions {
        default_world_lights: false,
        ..RendererOptions::default()
    })
}

fn pixel(rgba: &[u8], (x, y): (u32, u32)) -> [i32; 3] {
    let at = ((y * WIDTH + x) * 4) as usize;
    [rgba[at], rgba[at + 1], rgba[at + 2]].map(i32::from)
}

fn srgb(linear: f32) -> i32 {
    let encoded = if linear <= 0.003_130_8 {
        linear * 12.92
    } else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    };
    (encoded.clamp(0.0, 1.0) * 255.0).round() as i32
}

fn sky_light(intensity: f32) -> RenderDiff {
    RenderDiff::SetSkyLight {
        sky_light: (intensity > 0.0).then_some(SkyLightDescriptor { intensity }),
    }
}

/// A wide slab whose top is the floor at y = 0, with `material` (rough
/// dielectric unless changed).
fn slab(harness: &mut Harness, color: [f32; 4], roughness: f32, metalness: f32) {
    let mut surface = material("material/slab", color, None);
    surface.roughness = roughness;
    surface.metalness = metalness;
    harness.apply(vec![
        RenderDiff::DefineMaterial { material: surface },
        static_mesh(
            "mesh/slab",
            box_mesh([-20.0, -1.0, -40.0], [20.0, 0.0, 5.0], |_| 0),
            "material/slab",
        ),
        instance(
            1,
            None,
            "mesh/slab",
            transform([0.0, 0.0, 0.0], 0.0, [1.0; 3]),
        ),
    ]);
}

/// Looking down at the floor from a metre above it.
fn over_the_floor() -> render_host_contracts::RendererCompositionCamera {
    camera([0.0, 1.0, 0.0], 0.0, -20.0)
}

/// A point on the floor, below the view's centre.
const FLOOR: (u32, u32) = (WIDTH / 2, HEIGHT * 3 / 4);

#[test]
fn a_clear_colour_lights_the_world_as_a_uniform_sky_and_off_changes_nothing() {
    let sky = [0.2, 0.4, 0.8];
    let albedo = 0.8;
    let render = |intensity: f32| {
        let mut harness = unlit_world();
        harness.apply(vec![RenderDiff::SetBackgroundColor {
            color: [sky[0], sky[1], sky[2], 1.0],
        }]);
        slab(&mut harness, [albedo, albedo, albedo, 1.0], 0.9, 0.0);
        harness.apply(vec![sky_light(intensity)]);
        pixel(&harness.render(&over_the_floor()).1, FLOOR)
    };
    // Unlit without it.
    assert_eq!(render(0.0), [0, 0, 0]);
    // A uniform sky of radiance L gives irradiance πL: the floor shows its
    // albedo times the sky (plus a little sky reflection).
    let lit = render(1.0);
    for channel in 0..3 {
        let expected = srgb(albedo * sky[channel]);
        assert!(
            (lit[channel] - expected).abs() <= 12,
            "channel {channel}: {} against {expected}",
            lit[channel]
        );
    }
    let half = render(0.5);
    assert!(half[2] < lit[2] - 20, "{half:?} {lit:?}");

    // Turning it off returns the plain image exactly.
    let mut harness = unlit_world();
    harness.apply(vec![RenderDiff::SetBackgroundColor {
        color: [sky[0], sky[1], sky[2], 1.0],
    }]);
    slab(&mut harness, [albedo, albedo, albedo, 1.0], 0.9, 0.0);
    let plain = harness.render(&over_the_floor()).1;
    harness.apply(vec![sky_light(1.0)]);
    assert_ne!(harness.render(&over_the_floor()).1, plain);
    harness.apply(vec![sky_light(0.0)]);
    assert_eq!(harness.render(&over_the_floor()).1, plain);
}

/// A panorama red above the horizon and blue below.
fn red_sky_blue_ground(harness: &mut Harness, id: &str) -> RenderDiff {
    let (width, height) = (64, 32);
    let rgba: Vec<u8> = (0..width * height)
        .flat_map(|index| {
            if index / width < height / 2 {
                [230, 40, 30, 255]
            } else {
                [30, 40, 230, 255]
            }
        })
        .collect();
    RenderDiff::DefineTexture {
        texture: harness
            .resources
            .texture(id, width, height, &rgba, TextureWrap::Clamp),
    }
}

fn panorama(harness: &mut Harness) {
    let texture = red_sky_blue_ground(harness, "texture/panorama");
    harness.apply(vec![
        texture,
        RenderDiff::SetSkyBackground {
            background: Some(SkyBackgroundDescriptor {
                texture: "texture/panorama".to_owned(),
                blend: None,
            }),
        },
    ]);
}

#[test]
fn a_panorama_lights_the_floor_from_the_sky_and_a_mirror_reflects_it() {
    let render = |roughness: f32, metalness: f32| {
        let mut harness = unlit_world();
        panorama(&mut harness);
        slab(&mut harness, [0.8, 0.8, 0.8, 1.0], roughness, metalness);
        harness.apply(vec![sky_light(1.0)]);
        pixel(&harness.render(&over_the_floor()).1, FLOOR)
    };
    // A rough floor takes its light from the red sky above, not the blue
    // ground below it.
    let [red, _, blue] = render(0.9, 0.0);
    assert!(red > blue + 40, "rough floor {red} {blue}");
    // A mirror floor reflects the sky above the horizon in front.
    let [red, _, blue] = render(0.0, 1.0);
    assert!(red > 150 && blue < 80, "mirror floor {red} {blue}");
}

#[test]
fn a_blend_reaches_the_light_over_a_few_frames() {
    let mut harness = unlit_world();
    let solid = |harness: &mut Harness, id: &str, color: [u8; 4]| {
        let rgba: Vec<u8> = (0..8 * 4).flat_map(|_| color).collect();
        harness
            .resources
            .texture(id, 8, 4, &rgba, TextureWrap::Clamp)
    };
    let red = solid(&mut harness, "texture/red", [220, 30, 30, 255]);
    let blue = solid(&mut harness, "texture/blue", [30, 30, 220, 255]);
    let blend = |amount: f32| RenderDiff::SetSkyBackground {
        background: Some(SkyBackgroundDescriptor {
            texture: "texture/red".to_owned(),
            blend: Some(SkyBackgroundBlend {
                texture: "texture/blue".to_owned(),
                amount,
            }),
        }),
    };
    harness.apply(vec![
        RenderDiff::DefineTexture { texture: red },
        RenderDiff::DefineTexture { texture: blue },
        blend(0.0),
    ]);
    slab(&mut harness, [0.8, 0.8, 0.8, 1.0], 0.9, 0.0);
    harness.apply(vec![sky_light(1.0)]);
    // The first build is in its first frame.
    let [r, _, b] = pixel(&harness.render(&over_the_floor()).1, FLOOR);
    assert!(r > b + 40, "{r} {b}");
    // A new blend is built over the next frames while the last light holds.
    harness.apply(vec![blend(1.0)]);
    let [r, _, b] = pixel(&harness.render(&over_the_floor()).1, FLOOR);
    assert!(r > b + 40, "still the last light: {r} {b}");
    for _ in 0..4 {
        harness.render(&over_the_floor());
    }
    let [r, _, b] = pixel(&harness.render(&over_the_floor()).1, FLOOR);
    assert!(b > r + 40, "the new light: {r} {b}");
}

#[test]
fn an_ambient_lights_sky_layer_shades_the_sky_light_under_a_roof() {
    let render = |roofed: bool| {
        let mut harness = Harness::new(RendererOptions {
            default_world_lights: false,
            shadows: true,
            ..RendererOptions::default()
        });
        harness.apply(vec![RenderDiff::SetBackgroundColor {
            color: [0.5, 0.5, 0.5, 1.0],
        }]);
        slab(&mut harness, [0.8, 0.8, 0.8, 1.0], 0.9, 0.0);
        // A faint ambient light whose sky layer says where the sky is open.
        harness.apply(vec![RenderDiff::CreateLight {
            handle: RenderHandle::new(40),
            parent: None,
            light: LightDescriptor::Ambient {
                color: [1.0, 1.0, 1.0],
                intensity: 0.001,
                enabled: true,
                shadow_intent: LightShadowIntent::Requested,
                shadow: Default::default(),
                range: None,
            },
        }]);
        if roofed {
            harness.apply(vec![
                static_mesh(
                    "mesh/roof",
                    box_mesh([-6.0, 3.0, -12.0], [6.0, 3.5, 2.0], |_| 0),
                    "material/slab",
                ),
                instance(
                    2,
                    None,
                    "mesh/roof",
                    transform([0.0, 0.0, 0.0], 0.0, [1.0; 3]),
                ),
            ]);
        }
        harness.apply(vec![sky_light(1.0)]);
        pixel(&harness.render(&over_the_floor()).1, FLOOR)[1]
    };
    let open = render(false);
    let roofed = render(true);
    assert!(roofed + 40 < open, "roofed {roofed} open {open}");
}

#[test]
fn a_mirror_reflects_the_part_of_the_sky_it_faces() {
    // Four quarters of longitude in four colours, as the sky pass draws
    // them: the mirror floor must show the colour of the sky it faces.
    let palette = [[220, 30, 30], [30, 220, 30], [30, 30, 220], [220, 220, 220]];
    let (width, height) = (64, 32);
    let rgba: Vec<u8> = (0..width * height)
        .flat_map(|index| {
            let [r, g, b] = palette[((index % width) * 4 / width) as usize];
            [r, g, b, 255]
        })
        .collect();
    let nearest = |seen: [i32; 3]| {
        (0..4)
            .min_by_key(|&index| {
                let colour = palette[index];
                (0..3)
                    .map(|c| (seen[c] - colour[c] as i32).pow(2))
                    .sum::<i32>()
            })
            .unwrap()
    };
    // Each quarter's centre: u 0.125 is yaw -45, 0.375 yaw 45, 0.625 yaw
    // 135 and 0.875 yaw -135 (yaw 0 faces -Z, positive turns toward +X).
    for (yaw, quarter) in [(-45.0, 0), (45.0, 1), (135.0, 2), (-135.0, 3)] {
        let mut harness = unlit_world();
        let texture =
            harness
                .resources
                .texture("texture/quarters", width, height, &rgba, TextureWrap::Clamp);
        harness.apply(vec![
            RenderDiff::DefineTexture { texture },
            RenderDiff::SetSkyBackground {
                background: Some(SkyBackgroundDescriptor {
                    texture: "texture/quarters".to_owned(),
                    blend: None,
                }),
            },
        ]);
        slab(&mut harness, [0.8, 0.8, 0.8, 1.0], 0.0, 1.0);
        harness.apply(vec![sky_light(1.0)]);
        let pixels = harness.render(&camera([0.0, 1.0, 0.0], yaw, -20.0)).1;
        let floor = pixel(&pixels, FLOOR);
        let sky = pixel(&pixels, (WIDTH / 2, HEIGHT / 8));
        assert_eq!(nearest(sky), quarter, "yaw {yaw}: sky {sky:?}");
        assert_eq!(nearest(floor), quarter, "yaw {yaw}: floor {floor:?}");
    }
}

/// A UV sphere of radius 1 about the origin.
fn sphere() -> MeshPayloadDescriptor {
    let (rings, segments) = (16u32, 32u32);
    let (mut positions, mut normals, mut uvs, mut indices) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for ring in 0..=rings {
        let theta = std::f32::consts::PI * ring as f32 / rings as f32;
        for segment in 0..=segments {
            let phi = 2.0 * std::f32::consts::PI * segment as f32 / segments as f32;
            let normal = [
                theta.sin() * phi.cos(),
                theta.cos(),
                theta.sin() * phi.sin(),
            ];
            positions.extend_from_slice(&normal);
            normals.extend_from_slice(&normal);
            uvs.extend_from_slice(&[segment as f32 / segments as f32, ring as f32 / rings as f32]);
        }
    }
    let row = segments + 1;
    for ring in 0..rings {
        for segment in 0..segments {
            let a = ring * row + segment;
            let b = a + row;
            indices.extend_from_slice(&[a, a + 1, b, a + 1, b + 1, b]);
        }
    }
    let count = indices.len() as u32;
    payload(positions, normals, uvs, indices, &[(0, count)])
}

/// The lighting fixture's own panoramas (`fixtures/csharp-lighting-sky`).
fn fixture_sky(harness: &mut Harness, amount: f32) {
    for (id, png) in [
        (
            "texture/day",
            &include_bytes!("../../../../fixtures/csharp-lighting-sky/content/day.png")[..],
        ),
        (
            "texture/night",
            &include_bytes!("../../../../fixtures/csharp-lighting-sky/content/night.png")[..],
        ),
    ] {
        let (width, height, rgba) = render_wgpu::decode_png_rgba(png).expect("fixture panorama");
        let texture = harness
            .resources
            .texture(id, width, height, &rgba, TextureWrap::Clamp);
        harness.apply(vec![RenderDiff::DefineTexture { texture }]);
    }
    harness.apply(vec![RenderDiff::SetSkyBackground {
        background: Some(SkyBackgroundDescriptor {
            texture: "texture/day".to_owned(),
            blend: Some(SkyBackgroundBlend {
                texture: "texture/night".to_owned(),
                amount,
            }),
        }),
    }]);
}

/// A mirror metal sphere on the left and a rough dielectric on the right,
/// under the fixture's sky blended by `amount`, lit by the sky alone.
fn spheres_under(amount: f32) -> Vec<u8> {
    let mut harness = unlit_world();
    fixture_sky(&mut harness, amount);
    let mut metal = material("material/metal", [0.95, 0.95, 0.95, 1.0], None);
    metal.metalness = 1.0;
    metal.roughness = 0.05;
    let mut rough = material("material/rough", [0.8, 0.8, 0.8, 1.0], None);
    rough.roughness = 0.9;
    harness.apply(vec![
        RenderDiff::DefineMaterial { material: metal },
        RenderDiff::DefineMaterial { material: rough },
        static_mesh("mesh/metal", sphere(), "material/metal"),
        static_mesh("mesh/rough", sphere(), "material/rough"),
        instance(
            1,
            None,
            "mesh/metal",
            transform([-1.2, 0.0, -4.0], 0.0, [1.0; 3]),
        ),
        instance(
            2,
            None,
            "mesh/rough",
            transform([1.2, 0.0, -4.0], 0.0, [1.0; 3]),
        ),
        sky_light(1.0),
    ]);
    harness.render(&camera([0.0, 0.0, 0.0], 0.0, 0.0)).1
}

#[test]
fn metal_and_rough_spheres_take_the_fixture_skys_light_by_day_and_night() {
    let day = spheres_under(0.0);
    let night = spheres_under(1.0);
    assert_screenshot("sky-light-day", &day);
    assert_screenshot("sky-light-night", &night);
    // The spheres' centres (about 70 pixels either side of the middle).
    let metal = (WIDTH / 2 - 70, HEIGHT / 2);
    let rough = (WIDTH / 2 + 70, HEIGHT / 2);
    let brightness = |image: &[u8], at| pixel(image, at).iter().sum::<i32>();
    for at in [metal, rough] {
        assert!(
            brightness(&day, at) > brightness(&night, at) + 60,
            "day {:?} night {:?}",
            pixel(&day, at),
            pixel(&night, at)
        );
    }
    // The mirror shows the sky's detail; the rough sphere smooths it: down
    // a column through each, the metal varies more.
    let spread = |image: &[u8], x: u32| {
        let column: Vec<i32> = (HEIGHT / 2 - 25..HEIGHT / 2 + 25)
            .map(|y| brightness(image, (x, y)))
            .collect();
        column.iter().max().unwrap() - column.iter().min().unwrap()
    };
    assert!(
        spread(&day, metal.0) > spread(&day, rough.0),
        "metal {} rough {}",
        spread(&day, metal.0),
        spread(&day, rough.0)
    );
}

#[test]
fn the_sky_light_build_is_timed_whole_and_spread_over_frames() {
    let timing = |harness: &Harness| {
        harness
            .renderer
            .gpu_readout()
            .passes
            .into_iter()
            .find(|pass| pass.pass == "sky-light")
            .expect("the sky light is reported")
    };
    // A first build is whole in its frame; later frames build nothing.
    let mut harness = unlit_world();
    fixture_sky(&mut harness, 0.0);
    slab(&mut harness, [0.8, 0.8, 0.8, 1.0], 0.5, 0.0);
    harness.apply(vec![sky_light(1.0)]);
    for _ in 0..5 {
        harness.render(&over_the_floor());
    }
    eprintln!(
        "{}: whole first build {:?}",
        harness.gpu.adapter_summary().name,
        timing(&harness)
    );
    // A blend moving every frame keeps a build going.
    for frame in 0..60 {
        harness.apply(vec![RenderDiff::SetSkyBackground {
            background: Some(SkyBackgroundDescriptor {
                texture: "texture/day".to_owned(),
                blend: Some(SkyBackgroundBlend {
                    texture: "texture/night".to_owned(),
                    amount: (frame as f32 / 60.0).fract(),
                }),
            }),
        }]);
        harness.render(&over_the_floor());
    }
    let readout = harness.renderer.gpu_readout();
    let pass = readout
        .passes
        .iter()
        .find(|pass| pass.pass == "sky-light")
        .expect("the sky light is reported");
    eprintln!(
        "{} (timestamps {}): sky light per frame while blending {pass:?}",
        harness.gpu.adapter_summary().name,
        readout.timestamps
    );
    if readout.timestamps {
        assert!(pass.timed_frames > 0 && pass.median_gpu_ms.is_finite());
    }
}

/// A blue panorama sky over a dark ground, the sky's light on, a mirror
/// floor, and (with `sun`) a sun from behind the camera.
fn mirrored(sun: bool) -> Harness {
    let mut harness = unlit_world();
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
            .texture("texture/blue", width, height, &rgba, TextureWrap::Clamp);
    harness.apply(vec![
        RenderDiff::DefineTexture { texture },
        RenderDiff::SetSkyBackground {
            background: Some(SkyBackgroundDescriptor {
                texture: "texture/blue".to_owned(),
                blend: None,
            }),
        },
        sky_light(1.0),
    ]);
    if sun {
        harness.apply(vec![RenderDiff::CreateLight {
            handle: RenderHandle::new(10),
            parent: None,
            light: LightDescriptor::Directional {
                color: [1.0, 0.95, 0.85],
                intensity: 2.0,
                enabled: true,
                direction: [0.2, -0.6, -0.7],
                range: None,
                shadow_intent: LightShadowIntent::Disabled,
                shadow: Default::default(),
            },
        }]);
    }
    harness
}

/// A dark range 4 to 6 km ahead, 800 m high and 6 km wide, in the backdrop
/// at 1:1000.
fn backdrop_range() -> Vec<RenderDiff> {
    vec![
        RenderDiff::DefineMaterial {
            material: material("material/range", [0.3, 0.22, 0.15, 1.0], None),
        },
        static_mesh(
            "mesh/range",
            box_mesh([-3.0, 0.0, -6.0], [3.0, 0.8, -4.0], |_| 0),
            "material/range",
        ),
        RenderDiff::CreateStaticMeshInstance {
            handle: RenderHandle::new(30),
            parent: None,
            instance: StaticMeshInstanceDescriptor {
                asset: "mesh/range".to_owned(),
                transform: transform([0.0; 3], 0.0, [1.0; 3]),
                visible: true,
                material_overrides: Vec::new(),
                metadata: RenderMetadata::default(),
                layer: RenderLayer::Backdrop,
                shadow_casting: Default::default(),
            },
        },
        RenderDiff::SetBackdrop {
            camera: None,
            backdrop: Some(BackdropDescriptor {
                anchor: [0.0; 3],
                origin: [0.0; 3],
                scale: 1000.0,
            }),
        },
    ]
}

/// Enough frames from `camera` for a capture and its build to land.
fn settle(
    harness: &mut Harness,
    camera: &render_host_contracts::RendererCompositionCamera,
) -> Vec<u8> {
    for _ in 0..8 {
        harness.render(camera);
    }
    harness.render(camera).1
}

/// The share of the pixels in rows `rows` (and the middle three quarters of
/// each) that differ by more than 24 summed over the channels.
fn changed_in(a: &[u8], b: &[u8], rows: std::ops::Range<u32>) -> f64 {
    let (mut pixels, mut changed) = (0, 0);
    for y in rows {
        for x in WIDTH / 8..WIDTH * 7 / 8 {
            let (p, q) = (pixel(a, (x, y)), pixel(b, (x, y)));
            let difference: i32 = (0..3).map(|c| (p[c] - q[c]).abs()).sum();
            pixels += 1;
            changed += usize::from(difference > 24);
        }
    }
    changed as f64 / pixels as f64
}

#[test]
fn a_mirror_floor_reflects_the_backdrop_on_the_horizon_and_the_clouds_overhead() {
    // A metre above the mirror looking 4° down: the floor's reflections
    // between about 2° and 10° above the horizon fall on the range.
    let level = camera([0.0, 1.0, 0.0], 0.0, -4.0);
    let mut plain = mirrored(true);
    slab(&mut plain, [1.0; 4], 0.0, 1.0);
    let open = settle(&mut plain, &level);
    let mut ranged = mirrored(true);
    slab(&mut ranged, [1.0; 4], 0.0, 1.0);
    ranged.apply(backdrop_range());
    let reflected = settle(&mut ranged, &level);
    // Rows 85 to 105 hold the floor just below the horizon.
    let share = changed_in(&open, &reflected, 85..105);
    assert!(
        share > 0.8,
        "the floor reflects the range: {:.1}%",
        share * 100.0
    );

    // Looking well down, the floor reflects the sky overhead: a cloud layer
    // there shows in it.
    let down = camera([0.0, 1.0, 0.0], 0.0, -60.0);
    let mut clear = mirrored(true);
    slab(&mut clear, [1.0; 4], 0.0, 1.0);
    let blue = settle(&mut clear, &down);
    let mut clouded = mirrored(true);
    slab(&mut clouded, [1.0; 4], 0.0, 1.0);
    clouded.apply(vec![RenderDiff::SetClouds {
        clouds: Some(CloudsDescriptor {
            coverage: 0.9,
            drift: [0.0, 0.0],
            altitude: 1500.0,
            scale: 600.0,
            color: [1.0; 3],
            thickness: 0.0,
            kind: CloudKind::Stratus,
        }),
    }]);
    let overcast = settle(&mut clouded, &down);
    let share = changed_in(&blue, &overcast, HEIGHT / 4..HEIGHT);
    assert!(
        share > 0.8,
        "the floor reflects the clouds: {:.1}%",
        share * 100.0
    );
}

#[test]
fn a_storm_overhead_darkens_the_sky_light_on_the_ground() {
    // No sun: the floor takes the sky's light alone.
    let render = |storm: bool| {
        let mut harness = mirrored(false);
        slab(&mut harness, [0.8, 0.8, 0.8, 1.0], 1.0, 0.0);
        if storm {
            harness.apply(vec![RenderDiff::SetCloudRegion {
                id: 1,
                region: CloudRegionDescriptor {
                    center: [0.0, 0.0],
                    radius: 20_000.0,
                    coverage: 1.0,
                    darkness: 1.0,
                    drift: [0.0, 0.0],
                    kind: CloudKind::Cumulonimbus,
                    thickness: 0.0,
                },
            }]);
        }
        pixel(&settle(&mut harness, &over_the_floor()), FLOOR)
    };
    let clear = render(false);
    let storm = render(true);
    let brightness = |rgb: [i32; 3]| rgb.iter().sum::<i32>();
    assert!(
        brightness(storm) < brightness(clear) * 2 / 3,
        "under the storm {storm:?}, clear {clear:?}"
    );
}
