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
