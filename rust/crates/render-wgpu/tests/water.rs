//! The water feature (#9540): a blended material tinted by the depth of the
//! scene behind it, foam along the shore, ripples that move with
//! presentation time, and blended parts that cast no shadow unless asked.

mod common;

use common::*;
use render_host_contracts::RendererCompositionCamera;
use render_model::*;
use render_wgpu::RendererOptions;

fn directional(direction: [f32; 3], shadow: bool) -> RenderDiff {
    RenderDiff::CreateLight {
        handle: RenderHandle::new(77),
        parent: None,
        light: LightDescriptor::Directional {
            color: [1.0; 3],
            intensity: 3.0,
            enabled: true,
            direction,
            range: None,
            shadow_intent: if shadow {
                LightShadowIntent::Requested
            } else {
                LightShadowIntent::Disabled
            },
            shadow: Default::default(),
        },
    }
}

fn water(shoreline_width: f32, foam_threshold: f32) -> MaterialWaterDescriptor {
    MaterialWaterDescriptor {
        shallow_color: [0.3, 0.9, 0.8],
        deep_color: [0.0, 0.05, 0.3],
        depth_scale: 1.0,
        shoreline_width,
        foam_threshold,
        foam_scroll: [0.0; 2],
        normal_scroll_a: [0.0; 2],
        normal_scroll_b: [0.0; 2],
        wave_scale: 4.0,
        foam_texture: None,
        ripple_texture: None,
    }
}

/// A blended, glossy quad 8 m across at the origin: water when `water` is
/// set, a plain tinted sheet otherwise.
fn sheet(water: Option<MaterialWaterDescriptor>, alpha: f32) -> Vec<RenderDiff> {
    let mut ops = coloured_mesh("sheet", floor(8.0), [1.0, 1.0, 1.0, alpha]);
    if let RenderDiff::DefineMaterial { material } = &mut ops[0] {
        material.alpha_mode = MaterialAlphaModeDescriptor::Blend;
        material.roughness = 0.1;
        material.water = water;
    }
    ops
}

/// A shallow floor under the sheet's left half, a deep one under its right,
/// lit from above; the camera looks down over the sheet from its near edge.
fn shore(water: Option<MaterialWaterDescriptor>) -> Harness {
    let mut harness = Harness::new(RendererOptions {
        default_world_lights: false,
        ..RendererOptions::default()
    });
    let mut ops = vec![RenderDiff::SetBackgroundColor {
        color: [0.0, 0.0, 0.0, 1.0],
    }];
    ops.extend(coloured_mesh("floor", floor(4.0), [0.6, 0.6, 0.6, 1.0]));
    ops.extend(sheet(water, 0.3));
    ops.push(instance(
        1,
        None,
        "floor",
        transform([-2.0, -0.3, -4.0], 0.0, 1.0),
    ));
    ops.push(instance(
        2,
        None,
        "floor",
        transform([2.0, -3.0, -4.0], 0.0, 1.0),
    ));
    ops.push(instance(
        3,
        None,
        "sheet",
        transform([0.0, 0.0, -4.0], 0.0, 1.0),
    ));
    ops.push(directional([0.1, -1.0, -0.2], false));
    harness.apply(ops);
    harness
}

fn view() -> RendererCompositionCamera {
    camera("eye", [0.0, 2.5, 0.5], 0.0, -40.0)
}

/// The mean colour of a region of the frame.
fn mean(frame: &[u8], x: std::ops::Range<u32>, y: std::ops::Range<u32>) -> [f32; 3] {
    let mut sum = [0f32; 3];
    let mut count = 0.0;
    for y in y {
        for x in x.clone() {
            let pixel = pixel(frame, WIDTH, x, y);
            for channel in 0..3 {
                sum[channel] += f32::from(pixel[channel]);
            }
            count += 1.0;
        }
    }
    sum.map(|value| value / count)
}

/// The sheet fills most of the frame: the shallow floor shows through its
/// left part, higher in the frame; the deep one through its right, lower.
const SHALLOW: (std::ops::Range<u32>, std::ops::Range<u32>) =
    (WIDTH / 16..WIDTH * 2 / 5, HEIGHT * 3 / 10..HEIGHT * 11 / 20);
const DEEP: (std::ops::Range<u32>, std::ops::Range<u32>) = (
    WIDTH * 11 / 20..WIDTH * 4 / 5,
    HEIGHT * 11 / 20..HEIGHT * 17 / 20,
);

/// Over the shallow floor the water keeps its shallow colour and shows the
/// floor; over the deep one it turns toward its deep colour and opaque. A
/// plain blended sheet draws both halves alike.
#[test]
fn water_tints_by_the_depth_of_what_lies_behind_it() {
    let plain = shore(None).single(&view());
    let (plain_shallow, plain_deep) = (
        mean(&plain, SHALLOW.0, SHALLOW.1),
        mean(&plain, DEEP.0, DEEP.1),
    );
    assert!(
        (plain_shallow[1] - plain_deep[1]).abs() < 12.0,
        "a plain sheet draws alike over both floors: {plain_shallow:?} {plain_deep:?}"
    );
    let tinted = shore(Some(water(0.01, 1.0))).single(&view());
    let (shallow, deep) = (
        mean(&tinted, SHALLOW.0, SHALLOW.1),
        mean(&tinted, DEEP.0, DEEP.1),
    );
    assert!(
        shallow[1] > deep[1] + 40.0,
        "the shallow side keeps its green: {shallow:?} against {deep:?}"
    );
    assert!(
        deep[2] > deep[0] + 15.0 && deep[1] < 80.0,
        "the deep side turns dark blue: {deep:?}"
    );
}

/// Within the shoreline width the surface foams: the shallow side turns
/// toward white when the foam threshold is low, and not when the shoreline
/// is narrower than the water there.
#[test]
fn foam_covers_the_water_within_the_shoreline_width() {
    let clear = shore(Some(water(0.01, 1.0))).single(&view());
    let foamed = shore(Some(water(1.0, 0.0))).single(&view());
    let (clear_shallow, foamed_shallow) = (
        mean(&clear, SHALLOW.0, SHALLOW.1),
        mean(&foamed, SHALLOW.0, SHALLOW.1),
    );
    let (clear_deep, foamed_deep) = (mean(&clear, DEEP.0, DEEP.1), mean(&foamed, DEEP.0, DEEP.1));
    assert!(
        foamed_shallow[0] > clear_shallow[0] + 35.0,
        "foam whitens the shallows: {foamed_shallow:?} against {clear_shallow:?}"
    );
    assert!(
        (foamed_deep[0] - clear_deep[0]).abs() < 12.0,
        "the deep side, below the shoreline width, stays: {foamed_deep:?} against {clear_deep:?}"
    );
}

/// Without normal maps the surface ripples procedurally with presentation
/// time: two times draw differently, a held time the same.
#[test]
fn ripples_move_with_presentation_time() {
    let mut harness = shore(Some(water(0.01, 1.0)));
    let mut at = |time: f64| {
        harness.renderer.set_animation_time(time);
        harness.single(&view())
    };
    let first = at(0.0);
    let later = at(0.4);
    let held = at(0.4);
    assert_ne!(first, later, "the ripples move");
    assert_eq!(held, later, "a held time draws the same");
}

/// A blended box throws no shadow; one that asks for a translucent shadow
/// casts as an opaque one would.
#[test]
fn blended_parts_cast_no_shadow_unless_they_ask() {
    let render = |translucent_shadow: bool| {
        let mut harness = Harness::new(RendererOptions {
            default_world_lights: false,
            shadows: true,
            ..RendererOptions::default()
        });
        let mut ops = vec![RenderDiff::SetBackgroundColor {
            color: [0.0, 0.0, 0.0, 1.0],
        }];
        ops.extend(coloured_mesh("floor", floor(12.0), [0.7, 0.7, 0.7, 1.0]));
        let mut glass = coloured_mesh("glass", cube(), [1.0, 1.0, 1.0, 0.4]);
        if let RenderDiff::DefineMaterial { material } = &mut glass[0] {
            material.alpha_mode = MaterialAlphaModeDescriptor::Blend;
            material.translucent_shadow = translucent_shadow;
        }
        ops.extend(glass);
        ops.push(instance(
            1,
            None,
            "floor",
            transform([0.0, -1.0, -4.0], 0.0, 1.0),
        ));
        ops.push(instance(
            2,
            None,
            "glass",
            transform([0.0, 1.0, -4.0], 0.0, 1.5),
        ));
        ops.push(directional([0.6, -1.0, 0.0], true));
        harness.apply(ops);
        harness.single(&camera("eye", [0.0, 2.0, 0.5], 0.0, -35.0))
    };
    let dark = |frame: &[u8]| {
        (0..WIDTH)
            .flat_map(|x| (HEIGHT / 3..HEIGHT).map(move |y| (x, y)))
            .filter(|&(x, y)| pixel(frame, WIDTH, x, y)[0] < 40)
            .count()
    };
    let (none, cast) = (dark(&render(false)), dark(&render(true)));
    assert!(
        cast > none + 200,
        "the translucent shadow darkens {cast} pixels against {none}"
    );
}
