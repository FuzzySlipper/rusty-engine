//! The water feature (#9540): a blended material tinted by the depth of the
//! scene behind it, foam along the shore, ripples that move with
//! presentation time at the material's scale, and blended parts that cast
//! no shadow unless asked; reading the opaque scene's colour, water that
//! absorbs and refracts what shows through it, and the view's reflection in
//! still water.

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
        ripple_scale: 1.0,
        absorption_color: [1.0; 3],
        absorption_distance: 0.0,
        refraction: 0.0,
    }
}

/// A blended, glossy quad 8 m across at the origin: water when `water` is
/// set, a plain tinted sheet otherwise; `texture` names a base texture.
fn sheet(
    water: Option<MaterialWaterDescriptor>,
    alpha: f32,
    texture: Option<&str>,
) -> Vec<RenderDiff> {
    let mut ops = coloured_mesh("sheet", floor(8.0), [1.0, 1.0, 1.0, alpha]);
    if let RenderDiff::DefineMaterial { material } = &mut ops[0] {
        material.alpha_mode = MaterialAlphaModeDescriptor::Blend;
        material.roughness = 0.1;
        material.water = water;
        material.texture = texture.map(str::to_owned);
    }
    ops
}

/// A shallow floor under the sheet's left half, a deep one under its right,
/// lit from above; the camera looks down over the sheet from its near edge.
fn shore(water: Option<MaterialWaterDescriptor>) -> Harness {
    shore_textured(water, None)
}

fn shore_textured(water: Option<MaterialWaterDescriptor>, texture: Option<&str>) -> Harness {
    let mut harness = Harness::new(RendererOptions {
        default_world_lights: false,
        ..RendererOptions::default()
    });
    let mut ops = vec![RenderDiff::SetBackgroundColor {
        color: [0.0, 0.0, 0.0, 1.0],
    }];
    if let Some(texture) = texture {
        // A solid red base texture, 2 x 2.
        let (define, _) = harness.resources.texture(
            texture,
            2,
            2,
            &[255, 40, 40, 255].repeat(4),
            TextureFilter::Linear,
        );
        ops.push(define);
    }
    ops.extend(coloured_mesh("floor", floor(4.0), [0.6, 0.6, 0.6, 1.0]));
    ops.extend(sheet(water, 0.3, texture));
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

/// A base texture colours the water as the material colour does: a red
/// texture keeps the shallows red-tinted and still turns deep over the deep
/// floor.
#[test]
fn water_takes_its_base_texture_into_the_tint() {
    let plain = shore(Some(water(0.01, 1.0))).single(&view());
    let red = shore_textured(Some(water(0.01, 1.0)), Some("texture/red")).single(&view());
    let (plain_shallow, red_shallow) = (
        mean(&plain, SHALLOW.0, SHALLOW.1),
        mean(&red, SHALLOW.0, SHALLOW.1),
    );
    assert!(
        red_shallow[1] < plain_shallow[1] - 30.0 && red_shallow[0] > red_shallow[1],
        "the red texture tints the shallows: {red_shallow:?} against {plain_shallow:?}"
    );
    let red_deep = mean(&red, DEEP.0, DEEP.1);
    assert!(
        red_deep[0] < red_shallow[0] - 30.0,
        "the deep side still darkens: {red_deep:?} against {red_shallow:?}"
    );
}

/// Water reflects its surroundings by Fresnel without a sky cube (#9540
/// review): black, foamless, non-metallic water under ambient or hemisphere
/// light alone shows that light's colour, far more at a grazing view than
/// looking straight down.
#[test]
fn water_reflects_ambient_and_hemisphere_light_by_fresnel() {
    let black = MaterialWaterDescriptor {
        shallow_color: [0.0; 3],
        deep_color: [0.0; 3],
        // Foam never rises above a threshold past 1.
        foam_threshold: 2.0,
        ..water(0.5, 2.0)
    };
    for (name, light) in [
        (
            "ambient",
            LightDescriptor::Ambient {
                color: [0.4, 0.6, 1.0],
                intensity: 1.5,
                enabled: true,
                shadow_intent: LightShadowIntent::Disabled,
                shadow: Default::default(),
                range: None,
            },
        ),
        (
            "hemisphere",
            LightDescriptor::Hemisphere {
                color: [0.4, 0.6, 1.0],
                ground_color: [0.1, 0.1, 0.1],
                intensity: 1.5,
                enabled: true,
            },
        ),
    ] {
        let mut harness = Harness::new(RendererOptions {
            default_world_lights: false,
            ..RendererOptions::default()
        });
        let mut ops = vec![
            RenderDiff::SetBackgroundColor {
                color: [0.0, 0.0, 0.0, 1.0],
            },
            RenderDiff::CreateLight {
                handle: RenderHandle::new(78),
                parent: None,
                light,
            },
        ];
        ops.extend(sheet(Some(black.clone()), 1.0, None));
        if let RenderDiff::DefineMaterial { material } = &mut ops[2] {
            material.metalness = 0.0;
        }
        ops.push(instance(
            3,
            None,
            "sheet",
            transform([0.0, 0.0, -4.0], 0.0, 1.0),
        ));
        harness.apply(ops);
        // The water's blue in the lower middle of the frame.
        let blue = |frame: &[u8]| -> f64 {
            let mut sum = 0.0;
            for y in HEIGHT * 3 / 5..HEIGHT * 4 / 5 {
                for x in WIDTH * 2 / 5..WIDTH * 3 / 5 {
                    sum += f64::from(pixel(frame, WIDTH, x, y)[2]);
                }
            }
            sum / f64::from(HEIGHT / 5 * WIDTH / 5)
        };
        let grazing = blue(&harness.single(&camera("eye", [0.0, 0.25, 0.0], 0.0, -6.0)));
        let steep = blue(&harness.single(&camera("eye", [0.0, 3.0, -4.0], 0.0, -89.0)));
        // Without the reflection both read black. Looking straight down the
        // water still reflects its few percent (lifted by the sRGB curve);
        // at a grazing view far more.
        assert!(grazing > 90.0, "{name}: grazing water reflects {grazing}");
        assert!(
            steep > 20.0 && steep + 30.0 < grazing,
            "{name}: straight down {steep}, grazing {grazing}"
        );
    }
}

/// The mean absolute difference between two frames over a region, per
/// channel value.
fn difference(a: &[u8], b: &[u8], x: std::ops::Range<u32>, y: std::ops::Range<u32>) -> f64 {
    let mut sum = 0.0;
    let mut count = 0.0;
    for y in y {
        for x in x.clone() {
            let (a, b) = (pixel(a, WIDTH, x, y), pixel(b, WIDTH, x, y));
            for channel in 0..3 {
                sum += f64::from(a[channel].abs_diff(b[channel]));
            }
            count += 3.0;
        }
    }
    sum / count
}

/// The whole sheet: both floors' regions.
const SHEET: (std::ops::Range<u32>, std::ops::Range<u32>) =
    (WIDTH / 16..WIDTH * 4 / 5, HEIGHT * 3 / 10..HEIGHT * 17 / 20);

/// The ripples follow the material's normal scale without a normal map
/// (#10182): at 0 the surface draws flat, the same at every time; at 0.5 it
/// strays from the flat surface about half as far as at 1.
#[test]
fn ripples_scale_by_the_ripple_scale_without_a_normal_map() {
    let at = |scale: f32, time: f64| {
        let mut harness = shore(Some(MaterialWaterDescriptor {
            ripple_scale: scale,
            ..water(0.01, 1.0)
        }));
        harness.renderer.set_animation_time(time);
        harness.single(&view())
    };
    let flat = at(0.0, 0.0);
    assert_eq!(flat, at(0.0, 0.4), "a flat surface does not move");
    let full = difference(&at(1.0, 0.0), &flat, SHEET.0, SHEET.1);
    let half = difference(&at(0.5, 0.0), &flat, SHEET.0, SHEET.1);
    assert!(full > 0.5, "full ripples stray from flat: {full}");
    assert!(
        half > full * 0.3 && half < full * 0.7,
        "half ripples stray about half as far: {half} against {full}"
    );
}

/// Water that keeps a share of each channel per metre (#10183): a bed seen
/// through it loses its red far more through the deep water than through
/// the shallows, and keeps more blue than red there; the same water without
/// absorption shows both floors alike.
#[test]
fn absorbing_water_tints_the_bed_more_with_depth() {
    // Dark, nearly clear water, so the bed shows through both sides.
    let clear = MaterialWaterDescriptor {
        shallow_color: [0.05; 3],
        deep_color: [0.05; 3],
        depth_scale: 100.0,
        ..water(0.01, 1.0)
    };
    let plain = shore(Some(clear.clone())).single(&view());
    let mut absorbing = shore(Some(MaterialWaterDescriptor {
        absorption_color: [0.2, 0.6, 0.9],
        absorption_distance: 1.0,
        ..clear
    }));
    let absorbed = absorbing.single(&view());
    assert_eq!(
        absorbing.renderer.gpu_readout().water.refused,
        None,
        "this device copies the scene's colour"
    );
    let (plain_shallow, plain_deep) = (
        mean(&plain, SHALLOW.0, SHALLOW.1),
        mean(&plain, DEEP.0, DEEP.1),
    );
    let (shallow, deep) = (
        mean(&absorbed, SHALLOW.0, SHALLOW.1),
        mean(&absorbed, DEEP.0, DEEP.1),
    );
    assert!(
        deep[0] < plain_deep[0] - 40.0,
        "the deep water absorbs the bed's red: {deep:?} against {plain_deep:?}"
    );
    assert!(
        shallow[0] > deep[0] + 30.0,
        "the shallows keep more of it: {shallow:?} against {deep:?}"
    );
    assert!(
        deep[2] > deep[0] + 20.0,
        "blue survives the depth better than red: {deep:?}"
    );
    assert!(
        (plain_shallow[0] - plain_deep[0]).abs() < 25.0,
        "without absorption both floors show alike: {plain_shallow:?} {plain_deep:?}"
    );
}

/// A striped bed under refracting water (#10183): the ripples shift the
/// stripes, and a flat surface (ripple scale 0) does not shift them, drawing
/// as the water that reads the scene without refracting does. Reading the
/// scene draws what the blend would: water that only reads it matches plain
/// water.
#[test]
fn refracting_water_shifts_the_bed_by_its_ripples() {
    let render = |water: MaterialWaterDescriptor| {
        let mut harness = Harness::new(RendererOptions {
            default_world_lights: false,
            ..RendererOptions::default()
        });
        let mut ops = vec![RenderDiff::SetBackgroundColor {
            color: [0.0, 0.0, 0.0, 1.0],
        }];
        // A dark bed studded with a grid of white blocks, their edges a
        // quarter metre apart.
        ops.extend(coloured_mesh("floor", floor(8.0), [0.05, 0.05, 0.05, 1.0]));
        ops.extend(coloured_mesh("block", cube(), [0.9, 0.9, 0.9, 1.0]));
        ops.push(instance(
            1,
            None,
            "floor",
            transform([0.0, -1.0, -4.0], 0.0, 1.0),
        ));
        for row in 0..16 {
            for column in 0..16 {
                let at = |index: i32| -4.0 + 0.5 * index as f32 + 0.25;
                ops.push(instance(
                    100 + (row * 16 + column) as u64,
                    None,
                    "block",
                    transform([at(column), -1.0, at(row) - 4.0], 0.0, 0.25),
                ));
            }
        }
        ops.extend(sheet(Some(water), 0.3, None));
        ops.push(instance(
            3,
            None,
            "sheet",
            transform([0.0, 0.0, -4.0], 0.0, 1.0),
        ));
        ops.push(directional([0.1, -1.0, -0.2], false));
        harness.apply(ops);
        harness.single(&view())
    };
    let clear = MaterialWaterDescriptor {
        shallow_color: [0.05; 3],
        deep_color: [0.05; 3],
        depth_scale: 100.0,
        ..water(0.01, 1.0)
    };
    let refracting = |ripple_scale: f32, refraction: f32| {
        render(MaterialWaterDescriptor {
            ripple_scale,
            refraction,
            // Reading the scene without refracting: absorption that keeps all.
            absorption_distance: 1.0,
            ..clear.clone()
        })
    };
    let still = refracting(1.0, 0.0);
    let shifted = refracting(1.0, 1.0);
    let flat_still = refracting(0.0, 0.0);
    let flat = refracting(0.0, 1.0);
    let moved = difference(&shifted, &still, SHEET.0, SHEET.1);
    assert!(moved > 4.0, "the ripples shift the stripes: {moved}");
    let unmoved = difference(&flat, &flat_still, SHEET.0, SHEET.1);
    assert!(
        unmoved < 0.5,
        "a flat surface does not shift them: {unmoved}"
    );
    let blended = render(clear);
    let composed = difference(&still, &blended, SHEET.0, SHEET.1);
    assert!(
        composed < 1.5,
        "water reading the scene draws as the blend does: {composed}"
    );
}

/// Still black water before a glowing red wall, in a scene with no light
/// but the wall's (#10184), seen from `eye` pitched by `pitch`.
fn wall_over_water(reflections: WaterReflectionsQuality) -> Harness {
    let mut harness = Harness::new(RendererOptions {
        default_world_lights: false,
        water_reflections: reflections,
        ..RendererOptions::default()
    });
    let mut ops = vec![RenderDiff::SetBackgroundColor {
        color: [0.0, 0.0, 0.0, 1.0],
    }];
    ops.extend(coloured_mesh("floor", floor(12.0), [0.2, 0.2, 0.2, 1.0]));
    let mut wall = coloured_mesh("wall", cube(), [1.0, 0.1, 0.1, 1.0]);
    if let RenderDiff::DefineMaterial { material } = &mut wall[0] {
        material.emission_color = [1.0, 0.1, 0.1];
        material.emission_intensity = 3.0;
    }
    ops.extend(wall);
    let black = MaterialWaterDescriptor {
        shallow_color: [0.0; 3],
        deep_color: [0.0; 3],
        ripple_scale: 0.0,
        foam_threshold: 2.0,
        ..water(0.01, 2.0)
    };
    let mut still = sheet(Some(black), 1.0, None);
    if let RenderDiff::DefineMaterial { material } = &mut still[0] {
        material.roughness = 0.05;
    }
    ops.extend(still);
    ops.push(instance(
        1,
        None,
        "floor",
        transform([0.0, -1.0, -6.0], 0.0, 1.0),
    ));
    ops.push(instance(
        2,
        None,
        "wall",
        transform([0.0, 1.0, -9.0], 0.0, 2.0),
    ));
    ops.push(instance(
        3,
        None,
        "sheet",
        transform([0.0, 0.0, -4.0], 0.0, 1.0),
    ));
    harness.apply(ops);
    harness
}

/// The red in a region of the frame, less its green.
fn redness(frame: &[u8], x: std::ops::Range<u32>, y: std::ops::Range<u32>) -> f32 {
    let [red, green, _] = mean(frame, x, y);
    red - green
}

/// Water mirrors the lit wall beyond it in screen space (#10184), below
/// the wall's foot where the water shows; off, the black water there stays
/// dark. Where the wall is above the view, the reflected rays leave it and
/// the water draws as without reflections. A software adapter refuses the
/// feature and draws as off.
#[test]
fn still_water_reflects_what_the_view_shows() {
    let eye = camera("eye", [0.0, 1.0, 0.0], 0.0, -8.0);
    let off = wall_over_water(WaterReflectionsQuality::Off).single(&eye);
    let mut harness = wall_over_water(WaterReflectionsQuality::Low);
    let on = harness.single(&eye);
    // The water between the eye and the wall's foot, the lower middle.
    let water = (WIDTH * 2 / 5..WIDTH * 3 / 5, HEIGHT * 3 / 5..HEIGHT * 4 / 5);
    if harness.gpu.is_software() {
        let readout = harness.renderer.settings_readout();
        assert_eq!(
            readout.water_reflections,
            Some(render_wgpu::SettingRefusal::SoftwareAdapter)
        );
        assert_eq!(on, off, "a refused reflection draws as off");
        return;
    }
    let (dark, mirrored) = (
        redness(&off, water.0.clone(), water.1.clone()),
        redness(&on, water.0, water.1),
    );
    assert!(dark < 10.0, "without reflections the water is dark: {dark}");
    assert!(
        mirrored > dark + 25.0,
        "the wall shows in the water: {mirrored} against {dark}"
    );
    // Looking down, the wall is above the view: nothing to reflect but
    // the standard reflection, so the frames match.
    let down = camera("eye", [0.0, 1.0, -1.0], 0.0, -70.0);
    let off_down = wall_over_water(WaterReflectionsQuality::Off).single(&down);
    let on_down = harness.single(&down);
    let all = (0..WIDTH, 0..HEIGHT);
    let stray = difference(&on_down, &off_down, all.0, all.1);
    assert!(
        stray < 0.5,
        "rays leaving the view fall back cleanly: {stray}"
    );
}

/// Rippled water seen at a grazing angle over a bright bed, with nothing
/// above it (#10184): ripples that tilt the reflected ray under the surface
/// must not find the bed, so reflections draw what they draw off.
#[test]
fn reflections_never_find_what_lies_under_the_surface() {
    let render = |reflections: WaterReflectionsQuality| {
        let mut harness = Harness::new(RendererOptions {
            default_world_lights: false,
            water_reflections: reflections,
            ..RendererOptions::default()
        });
        let mut ops = vec![RenderDiff::SetBackgroundColor {
            color: [0.0, 0.0, 0.0, 1.0],
        }];
        ops.extend(coloured_mesh("floor", floor(12.0), [0.9, 0.9, 0.9, 1.0]));
        ops.extend(sheet(
            Some(MaterialWaterDescriptor {
                ripple_scale: 2.0,
                ..water(0.01, 2.0)
            }),
            0.3,
            None,
        ));
        ops.push(instance(
            1,
            None,
            "floor",
            transform([0.0, -0.3, -4.0], 0.0, 1.0),
        ));
        ops.push(instance(
            3,
            None,
            "sheet",
            transform([0.0, 0.0, -4.0], 0.0, 1.0),
        ));
        ops.push(directional([0.1, -1.0, -0.2], false));
        harness.apply(ops);
        let frame = harness.single(&camera("eye", [0.0, 0.4, 0.0], 0.0, -8.0));
        (frame, harness.gpu.is_software())
    };
    let (off, _) = render(WaterReflectionsQuality::Off);
    let (on, software) = render(WaterReflectionsQuality::High);
    let all = (0..WIDTH, 0..HEIGHT);
    let stray = difference(&on, &off, all.0, all.1);
    assert!(
        software || stray < 0.5,
        "the bed under the water does not reflect: {stray}"
    );
}

/// The motion view's range (`compose.wgsl` `MOTION_VIEW_RANGE`).
const MOTION_RANGE: f32 = 16.0;

/// The motion at (x, y) in render pixels, and its reactive mask, as the
/// motion view shows them: `0.5 + px / 32` in red and green, `0.5 + mask /
/// 2` in blue.
fn motion_at(frame: &[u8], x: u32, y: u32) -> ([f32; 2], f32) {
    let [red, green, blue, _] = pixel(frame, WIDTH, x, y);
    (
        [red, green].map(|value| (f32::from(value) / 255.0 - 0.5) * 2.0 * MOTION_RANGE),
        (f32::from(blue) / 255.0 - 0.5) * 2.0,
    )
}

/// The motion view after the eye steps 0.1 m right from `view`.
fn stepped_motion(harness: &mut Harness) -> Vec<u8> {
    harness
        .renderer
        .set_overlay_shown(render_wgpu::OverlayShown {
            motion: true,
            ..Default::default()
        });
    harness.renderer.render_offscreen(&view(), &harness.target);
    let stepped = camera("eye", [0.1, 2.5, 0.5], 0.0, -40.0);
    harness.renderer.render_offscreen(&stepped, &harness.target);
    harness.target.read_rgba(&harness.gpu)
}

/// A water surface over a still bed takes its own motion, not the bed's:
/// as a plain opaque sheet in its place moves, and nearer than the deep
/// bed, so faster. It marks the reactive mask; nothing else does.
#[test]
fn a_water_surface_moves_as_itself_over_a_still_bed_and_marks_the_reactive_mask() {
    let water = stepped_motion(&mut shore(Some(water(0.0, 0.0))));
    let mut opaque = shore(None);
    if let RenderDiff::DefineMaterial { material } = &mut sheet(None, 1.0, None)[0] {
        opaque.apply(vec![RenderDiff::DefineMaterial {
            material: material.clone(),
        }]);
    }
    let surface = stepped_motion(&mut opaque);
    let mut bare = shore(None);
    bare.apply(vec![RenderDiff::Destroy {
        handle: RenderHandle::new(3),
    }]);
    let bed = stepped_motion(&mut bare);
    let mut faster = 0;
    for (x, y) in [(80, 60), (160, 110), (230, 130), (250, 140), (200, 150)] {
        let (moved, reactive) = motion_at(&water, x, y);
        let (sheet, plain) = motion_at(&surface, x, y);
        let (under, _) = motion_at(&bed, x, y);
        assert!(
            (moved[0] - sheet[0]).abs() < 0.3 && (moved[1] - sheet[1]).abs() < 0.3,
            "at {x}, {y} the water moves as the surface {sheet:?}: {moved:?}"
        );
        assert!(moved[0] < -0.5, "the step moves it left: {moved:?}");
        faster += usize::from(moved[0] < under[0] - 0.3);
        assert!(
            (reactive - 0.5).abs() < 0.02,
            "the water's mask: {reactive}"
        );
        assert!(plain.abs() < 0.02, "a plain sheet's mask: {plain}");
    }
    assert!(
        faster >= 3,
        "over the deep bed the surface moves faster than it"
    );
}
