//! The finish stage (`rusty::finish`): exposure, tone mapping and distance
//! fog over everything drawn in the world, never the background. Same
//! harness, tolerance and blessing as `tests/screenshots.rs`
//! (`RENDER_WGPU_BLESS=1` rewrites the references).

mod support;

use render_model::*;
use render_wgpu::RendererOptions;
use support::*;

const BACKGROUND: [f32; 4] = [0.35, 0.4, 0.5, 1.0];
const CENTER: (usize, usize) = (WIDTH as usize / 2, HEIGHT as usize / 2);
const CORNER: (usize, usize) = (2, 2);
/// Channel difference allowed between two renders that should agree.
const SAME: i32 = 1;

fn pixel(rgba: &[u8], (x, y): (usize, usize)) -> [i32; 3] {
    let at = (y * WIDTH as usize + x) * 4;
    [rgba[at], rgba[at + 1], rgba[at + 2]].map(i32::from)
}

fn distance(a: [i32; 3], b: [i32; 3]) -> i32 {
    (0..3).map(|i| (a[i] - b[i]).abs()).max().unwrap()
}

/// One box straight ahead of a camera at the origin looking down -z, before
/// a plain background.
fn box_ahead(harness: &mut Harness, color: [f32; 4]) {
    harness.apply(vec![
        RenderDiff::SetBackgroundColor { color: BACKGROUND },
        RenderDiff::DefineMaterial {
            material: material("material/box", color, None),
        },
        static_mesh(
            "mesh/box",
            box_mesh([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5], |_| 0),
            "material/box",
        ),
        instance(
            1,
            None,
            "mesh/box",
            transform([0.0, 0.0, -5.0], 30.0, [1.0; 3]),
        ),
    ]);
}

fn move_box(harness: &mut Harness, z: f32) {
    harness.apply(vec![RenderDiff::Update {
        handle: RenderHandle::new(1),
        transform: Some(transform([0.0, 0.0, z], 30.0, [z.abs() / 5.0; 3])),
        material: None,
        visible: None,
        metadata: None,
    }]);
}

fn fog(harness: &mut Harness, fog: Option<FogDescriptor>) {
    harness.apply(vec![RenderDiff::SetFog { fog }]);
}

#[test]
fn fog_fades_world_geometry_toward_its_colour_by_distance_but_not_the_background() {
    let mut harness = Harness::new(RendererOptions::default());
    box_ahead(&mut harness, [0.9, 0.3, 0.2, 1.0]);
    let view = camera([0.0, 0.0, 0.0], 0.0, 0.0);
    let fog_color = [BACKGROUND[0], BACKGROUND[1], BACKGROUND[2]];
    // The box scales with its distance, so it covers the centre at each.
    let unfogged = |harness: &mut Harness, z: f32| {
        move_box(harness, z);
        let (_, rgba) = harness.render(&view);
        (pixel(&rgba, CENTER), pixel(&rgba, CORNER))
    };
    fog(&mut harness, None);
    let (near, background) = unfogged(&mut harness, -5.0);
    let (middle, _) = unfogged(&mut harness, -20.0);
    let (far, _) = unfogged(&mut harness, -40.0);

    // Linear: none before start, full beyond end.
    fog(
        &mut harness,
        Some(FogDescriptor::Linear {
            color: fog_color,
            start: 10.0,
            end: 30.0,
        }),
    );
    let fogged = |harness: &mut Harness, z: f32| {
        move_box(harness, z);
        let (_, rgba) = harness.render(&view);
        (pixel(&rgba, CENTER), pixel(&rgba, CORNER))
    };
    let (linear_near, linear_background) = fogged(&mut harness, -5.0);
    let (linear_middle, _) = fogged(&mut harness, -20.0);
    let (linear_far, _) = fogged(&mut harness, -40.0);
    assert!(
        distance(linear_near, near) <= SAME,
        "{linear_near:?} {near:?}"
    );
    assert!(distance(linear_background, background) <= SAME);
    assert!(distance(linear_far, background) <= SAME, "{linear_far:?}");
    assert!(
        distance(far, background) > 20,
        "unfogged far box shows: {far:?}"
    );
    for channel in 0..3 {
        let (low, high) = if middle[channel] < background[channel] {
            (middle[channel], background[channel])
        } else {
            (background[channel], middle[channel])
        };
        assert!(
            (low..=high).contains(&linear_middle[channel]),
            "halfway fog lies between the box and the fog: {linear_middle:?} {middle:?} {background:?}"
        );
    }
    assert!(distance(linear_middle, middle) > 10 && distance(linear_middle, background) > 10);
    // A row fanned across the view from 6 m to 36 m, each the same size on
    // screen: clear, then fading, then gone into the background.
    let distances = [6.0_f32, 12.0, 18.0, 24.0, 30.0, 36.0];
    let mut row = vec![RenderDiff::Update {
        handle: RenderHandle::new(1),
        transform: None,
        material: None,
        visible: Some(false),
        metadata: None,
    }];
    row.extend(distances.iter().enumerate().map(|(index, distance)| {
        let angle = (index as f32 * 9.0 - 22.5).to_radians();
        instance(
            10 + index as u64,
            None,
            "mesh/box",
            transform(
                [
                    distance * angle.sin(),
                    -0.1 * distance,
                    -distance * angle.cos(),
                ],
                30.0,
                [distance * 0.12; 3],
            ),
        )
    }));
    harness.apply(row);
    let (_, rgba) = harness.render(&view);
    assert_screenshot("fog-linear", &rgba);
    let mut cleanup: Vec<RenderDiff> = (0..distances.len())
        .map(|index| RenderDiff::Destroy {
            handle: RenderHandle::new(10 + index as u64),
        })
        .collect();
    cleanup.push(RenderDiff::Update {
        handle: RenderHandle::new(1),
        transform: None,
        material: None,
        visible: Some(true),
        metadata: None,
    });
    harness.apply(cleanup);

    // Exponential squared stays clearer than exponential at short range and
    // the same density.
    let at_ten = |harness: &mut Harness, fog_descriptor| {
        fog(harness, Some(fog_descriptor));
        move_box(harness, -10.0);
        let (_, rgba) = harness.render(&view);
        pixel(&rgba, CENTER)
    };
    fog(&mut harness, None);
    move_box(&mut harness, -10.0);
    let (_, rgba) = harness.render(&view);
    let clear = pixel(&rgba, CENTER);
    let exponential = at_ten(
        &mut harness,
        FogDescriptor::Exponential {
            color: fog_color,
            density: 0.05,
        },
    );
    let squared = at_ten(
        &mut harness,
        FogDescriptor::ExponentialSquared {
            color: fog_color,
            density: 0.05,
        },
    );
    assert!(
        distance(squared, clear) < distance(exponential, clear),
        "clear {clear:?}, exponential {exponential:?}, squared {squared:?}"
    );
    assert!(distance(exponential, clear) > 10);
}

/// Boxes under a strong white sun, before a plain background.
fn bright_boxes(harness: &mut Harness) {
    let colors = [
        [1.0, 1.0, 1.0, 1.0],
        [1.0, 0.45, 0.1, 1.0],
        [0.15, 0.35, 1.0, 1.0],
    ];
    let mut ops = vec![
        RenderDiff::SetBackgroundColor { color: BACKGROUND },
        RenderDiff::CreateLight {
            handle: RenderHandle::new(90),
            parent: None,
            light: LightDescriptor::Directional {
                color: [1.0, 1.0, 1.0],
                intensity: 9.0,
                enabled: true,
                direction: [-0.3, -0.5, -1.0],
                range: None,
                shadow_intent: LightShadowIntent::Disabled,
                shadow: Default::default(),
            },
        },
        RenderDiff::CreateLight {
            handle: RenderHandle::new(91),
            parent: None,
            light: LightDescriptor::Ambient {
                color: [1.0, 1.0, 1.0],
                intensity: 0.6,
                enabled: true,
                shadow_intent: LightShadowIntent::Disabled,
                shadow: Default::default(),
            },
        },
    ];
    for (index, color) in colors.into_iter().enumerate() {
        let id = format!("material/box-{index}");
        let mesh = format!("mesh/box-{index}");
        ops.push(RenderDiff::DefineMaterial {
            material: material(&id, color, None),
        });
        ops.push(static_mesh(
            &mesh,
            box_mesh([-0.6, -0.6, -0.6], [0.6, 0.6, 0.6], |_| 0),
            &id,
        ));
        let x = index as f32 * 1.7 - 1.7;
        ops.push(instance(
            index as u64 + 1,
            None,
            &mesh,
            transform([x, 0.0, -5.0], 25.0, [1.0; 3]),
        ));
    }
    harness.apply(ops);
}

fn tone(harness: &mut Harness, operator: ToneMappingOperator, exposure: f32) -> Vec<u8> {
    harness.apply(vec![RenderDiff::SetToneMapping {
        tone_mapping: ToneMappingDescriptor { operator, exposure },
    }]);
    harness.render(&camera([0.0, 0.0, 0.0], 0.0, 0.0)).1
}

#[test]
fn tone_mapping_compresses_highlights_scales_by_exposure_and_leaves_the_background() {
    let mut harness = Harness::new(RendererOptions {
        default_world_lights: false,
        ..RendererOptions::default()
    });
    bright_boxes(&mut harness);
    // The orange box's face toward the sun.
    let orange = CENTER;
    let none = tone(&mut harness, ToneMappingOperator::None, 1.0);
    let neutral = tone(&mut harness, ToneMappingOperator::Neutral, 1.0);
    let aces = tone(&mut harness, ToneMappingOperator::AcesFilmic, 1.0);
    let aces_dim = tone(&mut harness, ToneMappingOperator::AcesFilmic, 0.25);
    assert_screenshot("tone-mapping-neutral", &neutral);
    assert_screenshot("tone-mapping-aces", &aces);

    let clipped = pixel(&none, orange);
    assert!(
        clipped[0] == 255,
        "without tone mapping the sunlit face clips: {clipped:?}"
    );
    for (name, frame) in [("neutral", &neutral), ("aces", &aces)] {
        let [r, g, b] = pixel(frame, orange);
        assert!(
            r < 255 && r > g && g > b,
            "{name} keeps the highlight's hue: {:?}",
            [r, g, b]
        );
        assert!(
            distance(pixel(frame, CORNER), pixel(&none, CORNER)) <= SAME,
            "{name} leaves the background"
        );
    }
    let [bright, dim] = [pixel(&aces, orange), pixel(&aces_dim, orange)];
    assert!(
        (0..3).all(|channel| dim[channel] < bright[channel]),
        "exposure 0.25 is darker: {dim:?} {bright:?}"
    );

    // Back to none reproduces the untouched frame.
    let again = tone(&mut harness, ToneMappingOperator::None, 1.0);
    assert_eq!(again, none);
}

/// A box straight ahead glowing far past white.
fn glowing_box(harness: &mut Harness) {
    let mut glow = material("material/glow", [1.0, 0.8, 0.5, 1.0], None);
    glow.emission_color = [1.0, 0.8, 0.5];
    glow.emission_intensity = 6.0;
    harness.apply(vec![
        RenderDiff::SetBackgroundColor { color: BACKGROUND },
        RenderDiff::DefineMaterial { material: glow },
        static_mesh(
            "mesh/glow",
            box_mesh([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5], |_| 0),
            "material/glow",
        ),
        instance(
            1,
            None,
            "mesh/glow",
            transform([0.0, 0.0, -5.0], 0.0, [1.0; 3]),
        ),
    ]);
}

#[test]
fn bloom_spreads_bright_light_past_its_surface_and_off_changes_nothing() {
    let view = camera([0.0, 0.0, 0.0], 0.0, 0.0);
    let render = |bloom: Option<BloomDescriptor>| {
        let mut harness = Harness::new(RendererOptions::default());
        glowing_box(&mut harness);
        harness.apply(vec![RenderDiff::SetBloom { bloom }]);
        harness.render(&view).1
    };
    let plain = render(None);
    let glowing = render(Some(BloomDescriptor {
        threshold: 1.0,
        intensity: 1.0,
    }));
    // Beside the box's right edge (it spans about 57 pixels at 5 m), and far
    // off in the corner.
    let beside = (CENTER.0 + 40, CENTER.1);
    assert_eq!(
        pixel(&plain, beside),
        pixel(&plain, CORNER),
        "background beside the box"
    );
    assert!(
        distance(pixel(&glowing, beside), pixel(&plain, beside)) > 15,
        "the glow reaches past the box: {:?} {:?}",
        pixel(&glowing, beside),
        pixel(&plain, beside)
    );
    assert!(distance(pixel(&glowing, CORNER), pixel(&plain, CORNER)) <= SAME);
    // Zero intensity draws what no bloom draws.
    let zero = render(Some(BloomDescriptor {
        threshold: 1.0,
        intensity: 0.0,
    }));
    assert_eq!(zero, plain);
}

#[test]
fn auto_exposure_brings_bright_and_dim_worlds_toward_middle_grey_over_presentation_time() {
    let view = camera([0.0, 0.0, 0.0], 0.0, 0.0);
    let auto = AutoExposureDescriptor {
        speed: 2.0,
        min_exposure: 0.01,
        max_exposure: 100.0,
    };
    let lit = |harness: &mut Harness, color: f32| {
        // The box fills the view.
        harness.apply(vec![RenderDiff::DefineMaterial {
            material: material("material/box", [color, color, color, 1.0], None),
        }]);
    };
    let centre = |harness: &mut Harness, seconds: f64| {
        harness.renderer.set_animation_time(seconds);
        pixel(&harness.render(&view).1, CENTER)[0]
    };
    let mut harness = Harness::new(RendererOptions::default());
    box_ahead(&mut harness, [1.0; 4]);
    move_box(&mut harness, -0.7);
    let mut fixed = Harness::new(RendererOptions::default());
    box_ahead(&mut fixed, [1.0; 4]);
    move_box(&mut fixed, -0.7);
    harness.apply(vec![RenderDiff::SetAutoExposure {
        auto_exposure: Some(auto),
    }]);
    // Bright and dim worlds differ by 20× without it.
    lit(&mut fixed, 1.0);
    let fixed_bright = centre(&mut fixed, 0.0);
    lit(&mut fixed, 0.05);
    let fixed_dim = centre(&mut fixed, 0.0);
    assert!(fixed_bright > fixed_dim + 100, "{fixed_bright} {fixed_dim}");
    // The first frame takes its exposure at once.
    lit(&mut harness, 1.0);
    let bright = centre(&mut harness, 0.0);
    // The world dims: held time keeps the exposure, so the view goes dark.
    lit(&mut harness, 0.05);
    let held = centre(&mut harness, 0.0);
    assert!(held + 50 < bright, "{held} {bright}");
    // A second later the exposure has come most of the way, and after ten
    // the dim world reads as the bright one did.
    let later = centre(&mut harness, 1.0);
    let settled = centre(&mut harness, 10.0);
    assert!(
        held < later && later < settled + 3,
        "{held} {later} {settled}"
    );
    assert!((settled - bright).abs() <= 6, "{settled} {bright}");
}

#[test]
fn the_world_its_bloom_and_exposure_and_its_finish_are_timed() {
    let view = camera([0.0, 0.0, 0.0], 0.0, 0.0);
    let mut harness = Harness::new(RendererOptions::default());
    glowing_box(&mut harness);
    harness.apply(vec![
        RenderDiff::SetBloom {
            bloom: Some(BloomDescriptor {
                threshold: 1.0,
                intensity: 1.0,
            }),
        },
        RenderDiff::SetAutoExposure {
            auto_exposure: Some(AutoExposureDescriptor {
                speed: 1.0,
                min_exposure: 0.1,
                max_exposure: 10.0,
            }),
        },
    ]);
    // Stamps read back a frame or more after they are written.
    for frame in 0..30 {
        harness.renderer.set_animation_time(f64::from(frame) / 60.0);
        harness.render(&view);
    }
    let readout = harness.renderer.gpu_readout();
    eprintln!("timestamps {}, {:?}", readout.timestamps, readout.passes);
    for name in ["world", "bloom-exposure", "finish"] {
        let pass = readout
            .passes
            .iter()
            .find(|pass| pass.pass == name)
            .unwrap_or_else(|| panic!("{name} is reported"));
        if readout.timestamps {
            assert!(pass.timed_frames > 0, "{name}: no timed frame");
            assert!(pass.median_gpu_ms.is_finite() && pass.median_gpu_ms >= 0.0);
        } else {
            assert_eq!(pass.timed_frames, 0, "{name}: untimed without queries");
        }
    }
}

#[test]
fn colour_grading_warms_cools_desaturates_and_steepens_the_world_but_not_the_background() {
    let view = camera([0.0, 0.0, 0.0], 0.0, 0.0);
    let render = |color: [f32; 4], grading: Option<ColorGradingDescriptor>| {
        let mut harness = Harness::new(RendererOptions::default());
        box_ahead(&mut harness, color);
        harness.apply(vec![RenderDiff::SetColorGrading {
            color_grading: grading,
        }]);
        harness.render(&view).1
    };
    let grade = |temperature, tint, contrast, saturation| {
        Some(ColorGradingDescriptor {
            temperature,
            tint,
            contrast,
            saturation,
        })
    };
    let grey = [0.5, 0.5, 0.5, 1.0];
    let plain = render(grey, None);
    // Neutral grading draws what no grading draws.
    let neutral = render(grey, grade(0.0, 0.0, 0.0, 0.0));
    assert!(distance(pixel(&neutral, CENTER), pixel(&plain, CENTER)) <= SAME);
    let [r, g, b] = pixel(&plain, CENTER);
    let [warm_r, _, warm_b] = pixel(&render(grey, grade(0.8, 0.0, 0.0, 0.0)), CENTER);
    assert!(
        warm_r > r + 5 && warm_b + 5 < b,
        "warm {warm_r} {warm_b} from {r} {b}"
    );
    let [cool_r, _, cool_b] = pixel(&render(grey, grade(-0.8, 0.0, 0.0, 0.0)), CENTER);
    assert!(
        cool_r + 5 < r && cool_b > b + 5,
        "cool {cool_r} {cool_b} from {r} {b}"
    );
    let [_, magenta_g, _] = pixel(&render(grey, grade(0.0, 0.8, 0.0, 0.0)), CENTER);
    assert!(magenta_g + 5 < g, "tint {magenta_g} from {g}");
    // No saturation turns a red box grey.
    let red = [0.8, 0.1, 0.1, 1.0];
    let [gr, gg, gb] = pixel(&render(red, grade(0.0, 0.0, 0.0, -1.0)), CENTER);
    assert!(
        gr.abs_diff(gg) <= 1 && gg.abs_diff(gb) <= 1,
        "{gr} {gg} {gb}"
    );
    // More contrast takes a dark box darker and a bright one brighter.
    let dark = [0.05, 0.05, 0.05, 1.0];
    let bright = [1.0; 4];
    let steeper = grade(0.0, 0.0, 0.5, 0.0);
    assert!(pixel(&render(dark, steeper), CENTER)[0] < pixel(&render(dark, None), CENTER)[0]);
    assert!(pixel(&render(bright, steeper), CENTER)[0] > pixel(&render(bright, None), CENTER)[0]);
    // The background is never graded.
    let warm = render(grey, grade(0.8, 0.0, 0.5, 0.5));
    assert_eq!(pixel(&warm, CORNER), pixel(&plain, CORNER));
}

/// A directional light travelling along `direction`.
fn sun(harness: &mut Harness, direction: [f32; 3]) {
    harness.apply(vec![RenderDiff::CreateLight {
        handle: RenderHandle::new(50),
        parent: None,
        light: LightDescriptor::Directional {
            color: [1.0, 0.9, 0.7],
            intensity: 2.0,
            enabled: true,
            direction,
            range: None,
            shadow_intent: LightShadowIntent::Disabled,
            shadow: Default::default(),
        },
    }]);
}

#[test]
fn atmospheric_fog_thins_with_height_and_turns_toward_the_haze_near_the_sun() {
    let view = camera([0.0, 0.0, 0.0], 0.0, 0.0);
    let thick = FogDescriptor::Exponential {
        color: [0.2, 0.3, 0.6],
        density: 0.2,
    };
    // The box straight ahead, fogged, with the sun travelling along
    // `sun_direction` (if any) and the atmosphere set.
    let render = |fogged: bool,
                  sun_direction: Option<[f32; 3]>,
                  atmosphere: Option<AtmosphereDescriptor>| {
        let mut harness = Harness::new(RendererOptions::default());
        box_ahead(&mut harness, [0.8, 0.8, 0.8, 1.0]);
        fog(&mut harness, fogged.then_some(thick));
        if let Some(direction) = sun_direction {
            sun(&mut harness, direction);
        }
        harness.apply(vec![RenderDiff::SetAtmosphere { atmosphere }]);
        pixel(&harness.render(&view).1, CENTER)
    };
    let atmosphere = |base: f32, falloff: f32, exponent: f32| AtmosphereDescriptor {
        fog_base_height: base,
        fog_falloff_height: falloff,
        haze_color: [1.0, 0.5, 0.1],
        haze_exponent: exponent,
        ..Default::default()
    };
    let clear = render(false, None, None);
    let fogged = render(true, None, None);
    // Zero values draw today's fog exactly.
    assert_eq!(
        render(true, None, Some(AtmosphereDescriptor::default())),
        fogged
    );
    // Fog based far below the camera is thin air at its height; based above
    // it, thicker.
    let thin = render(true, None, Some(atmosphere(-10.0, 2.0, 0.0)));
    let thicker = render(true, None, Some(atmosphere(4.0, 2.0, 0.0)));
    assert!(
        distance(thin, clear) * 4 < distance(fogged, clear),
        "{thin:?} {fogged:?} {clear:?}"
    );
    assert!(
        distance(thicker, clear) > distance(fogged, clear),
        "{thicker:?} {fogged:?}"
    );
    // Looking into the sun (it travels toward the camera), the fog turns
    // toward the haze colour; with the sun behind, it does not.
    let toward = Some([0.0, 0.0, 1.0]);
    let behind = Some([0.0, 0.0, -1.0]);
    let [r, _, b] = render(true, toward, None);
    let [hazy_r, _, hazy_b] = render(true, toward, Some(atmosphere(0.0, 0.0, 4.0)));
    assert!(
        hazy_r > r + 10 && hazy_b + 10 < b,
        "{hazy_r} {hazy_b} from {r} {b}"
    );
    assert_eq!(
        render(true, behind, Some(atmosphere(0.0, 0.0, 4.0))),
        render(true, behind, None)
    );
}

#[test]
fn the_sun_draws_its_disc_and_halo_over_the_clear_colour_only_when_set() {
    let view = camera([0.0, 0.0, 0.0], 0.0, 0.0);
    // Nothing in the world: only the background.
    let render = |atmosphere: Option<AtmosphereDescriptor>| {
        let mut harness = Harness::new(RendererOptions::default());
        harness.apply(vec![RenderDiff::SetBackgroundColor { color: BACKGROUND }]);
        // Straight ahead of the camera.
        sun(&mut harness, [0.0, 0.0, 1.0]);
        harness.apply(vec![RenderDiff::SetAtmosphere { atmosphere }]);
        harness.render(&view).1
    };
    let plain = render(None);
    let disc = render(Some(AtmosphereDescriptor {
        sun_radius_degrees: 3.0,
        ..Default::default()
    }));
    let halo = render(Some(AtmosphereDescriptor {
        sun_halo: 0.5,
        ..Default::default()
    }));
    let off = render(Some(AtmosphereDescriptor::default()));
    assert_eq!(off, plain);
    // The disc is the sun's colour at the centre and leaves the corner.
    assert!(pixel(&disc, CENTER)[0] > 240, "{:?}", pixel(&disc, CENTER));
    assert_eq!(pixel(&disc, CORNER), pixel(&plain, CORNER));
    // The halo brightens around the centre, fading outward.
    let near = (CENTER.0 + 12, CENTER.1);
    let far = (CENTER.0 + 60, CENTER.1);
    let gain = |at| distance(pixel(&halo, at), pixel(&plain, at));
    assert!(
        gain(near) > gain(far) && gain(near) > 10,
        "{} {}",
        gain(near),
        gain(far)
    );
}
