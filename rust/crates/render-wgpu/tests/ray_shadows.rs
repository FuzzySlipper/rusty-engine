//! The sun's shadows traced as rays (`RendererOptions::ray_traced_shadows`,
//! `ray_shadows.rs`): a box's shadow on the ground falls where its cascades
//! put it, a masked card's transparent texels let the sun through, and the
//! sun's cascades are not drawn meanwhile. A device made without ray
//! queries refuses the setting and draws the cascades.

mod support;

use render_model::*;
use render_wgpu::{Gpu, OffscreenTarget, RendererOptions, SettingRefusal};
use support::*;

const GROUND: u64 = 1;
const BOX: u64 = 2;
const CARD: u64 = 3;
const GLASS: u64 = 4;
const SUN: u64 = 90;
const SKY: u64 = 91;
/// A sun wide enough for its penumbra to span pixels at this size.
const WIDE_SUN_DEGREES: f32 = 8.0;

fn sun() -> RenderDiff {
    RenderDiff::CreateLight {
        handle: RenderHandle::new(SUN),
        parent: None,
        light: LightDescriptor::Directional {
            color: [1.0; 3],
            intensity: 3.0,
            enabled: true,
            // From the left, so the shadows fall to the right of what casts.
            direction: [0.8, -1.0, 0.0],
            range: None,
            shadow_intent: LightShadowIntent::Requested,
            shadow: Default::default(),
        },
    }
}

/// A ground plane, a box on it, and a thin masked card facing the sun
/// right of the box, whose texture's rows have the alphas `card` (top,
/// bottom; none: no card).
fn scene(harness: &mut Harness, card: Option<[u8; 2]>) {
    let mut ops = vec![
        RenderDiff::DefineMaterial {
            material: material("material/ground", [0.7, 0.7, 0.7, 1.0], None),
        },
        static_mesh(
            "mesh/ground",
            box_mesh([-20.0, -0.2, -30.0], [20.0, 0.0, 10.0], |_| 0),
            "material/ground",
        ),
        instance(
            GROUND,
            None,
            "mesh/ground",
            transform([0.0; 3], 0.0, [1.0; 3]),
        ),
        static_mesh(
            "mesh/box",
            box_mesh([-0.5, 0.0, -0.5], [0.5, 1.0, 0.5], |_| 0),
            "material/ground",
        ),
        instance(
            BOX,
            None,
            "mesh/box",
            transform([-1.5, 0.0, -6.0], 0.0, [1.0; 3]),
        ),
        sun(),
        RenderDiff::CreateLight {
            handle: RenderHandle::new(SKY),
            parent: None,
            light: LightDescriptor::Ambient {
                color: [1.0; 3],
                intensity: 0.3,
                enabled: true,
                shadow_intent: LightShadowIntent::Disabled,
                shadow: Default::default(),
                range: None,
            },
        },
    ];
    if let Some([top, bottom]) = card {
        let texture = harness.resources.texture(
            "texture/card",
            2,
            2,
            &[
                [255, 255, 255, top].repeat(2),
                [255, 255, 255, bottom].repeat(2),
            ]
            .concat(),
            TextureWrap::Clamp,
        );
        ops.extend([
            RenderDiff::DefineTexture { texture },
            RenderDiff::DefineMaterial {
                material: RenderMaterialDescriptor {
                    alpha_mode: MaterialAlphaModeDescriptor::Mask { cutoff: 0.5 },
                    double_sided: true,
                    ..material("material/card", [0.9, 0.9, 0.9, 1.0], Some("texture/card"))
                },
            },
            static_mesh(
                "mesh/card",
                box_mesh([-0.02, 0.2, -0.6], [0.02, 1.4, 0.6], |_| 0),
                "material/card",
            ),
            instance(
                CARD,
                None,
                "mesh/card",
                transform([1.5, 0.0, -6.0], 0.0, [1.0; 3]),
            ),
        ]);
    }
    harness.apply(ops);
}

fn options(ray_traced_shadows: bool) -> RendererOptions {
    RendererOptions {
        default_world_lights: false,
        shadows: true,
        ray_traced_shadows,
        ..RendererOptions::default()
    }
}

fn frame(harness: &mut Harness) -> Vec<u8> {
    harness.render(&camera([0.0, 4.0, 0.0], 0.0, -40.0)).1
}

/// Ground pixels in shadow in the left half (the box's) and the right half
/// (the card's) of the image: much darker than the lit ground.
fn shadowed(rgba: &[u8]) -> [usize; 2] {
    let luma = |pixel: &[u8; 4]| u32::from(pixel[0]) + u32::from(pixel[1]) + u32::from(pixel[2]);
    let pixels = rgba.as_chunks::<4>().0;
    let lit = pixels.iter().map(luma).max().unwrap_or(0);
    let mut counts = [0; 2];
    for (index, pixel) in pixels.iter().enumerate() {
        let x = index as u32 % WIDTH;
        if luma(pixel) * 2 < lit {
            counts[usize::from(x >= WIDTH / 2)] += 1;
        }
    }
    counts
}

fn near(traced: usize, cascades: usize, what: &str) {
    let (low, high) = (cascades * 3 / 4, cascades * 5 / 4);
    assert!(
        (low..=high).contains(&traced),
        "{what}: {traced} traced shadow pixels, {cascades} with cascades"
    );
}

#[test]
fn rays_trace_the_suns_shadows_through_masked_texels_in_place_of_its_cascades() {
    let gpu = Gpu::headless_with_ray_queries().expect("a device");
    let render = |ray_traced: bool, card: Option<[u8; 2]>| {
        let mut harness = Harness::on(gpu.clone(), options(ray_traced));
        scene(&mut harness, card);
        // The first frame builds the BLAS, the next traces them.
        frame(&mut harness);
        let image = frame(&mut harness);
        (harness, image)
    };
    const OPAQUE: Option<[u8; 2]> = Some([255, 255]);
    let (harness, traced) = render(true, OPAQUE);
    let refused = harness.renderer.settings_readout().ray_traced_shadows;
    let report = harness.renderer.shadow_report();
    drop(harness);
    if gpu.is_software() {
        assert_eq!(refused, Some(SettingRefusal::SoftwareAdapter));
        assert!(report.layers > 0, "the cascades draw");
        return;
    }
    assert_eq!(refused, None, "a GPU made with ray queries traces them");
    assert_eq!(report.layers, 0, "no cascades while rays trace the sun");
    assert_eq!(report.rendered_layers, 0);
    let (_, cascades) = render(false, OPAQUE);
    let [box_cascades, card_cascades] = shadowed(&cascades);
    let [box_traced, card_traced] = shadowed(&traced);
    assert!(box_cascades > 200, "the box casts: {box_cascades}");
    near(box_traced, box_cascades, "the box");
    near(card_traced, card_cascades, "the opaque card");

    // Where the card's texture is transparent, the sun passes: a clear
    // card casts nothing, and one clear in its top half casts about half
    // its shadow.
    let card = |card| shadowed(&render(true, card).1)[1];
    let (clear, half, none) = (card(Some([0, 0])), card(Some([0, 255])), card(None));
    assert!(card_traced > none + 200, "{card_traced} vs {none}");
    assert!(
        clear.abs_diff(none) < 20,
        "a clear card casts nothing: {clear} vs {none}"
    );
    let cast = (card_traced - none) as f32;
    assert!(
        (0.3..0.7).contains(&((half - none) as f32 / cast)),
        "a half-clear card casts {half} of {card_traced}, {none} without it"
    );
    blended_parts_trace_their_own_shadow(&gpu);
    a_temporal_stage_gathers_a_penumbra(&gpu);
    refused_without_ray_queries();
}

/// A glass slab half way up the box catches its shadow: the mask holds the
/// ground below it, so its fragments trace for themselves.
fn blended_parts_trace_their_own_shadow(gpu: &Gpu) {
    let render = |ray_traced: bool| {
        let mut harness = Harness::on(gpu.clone(), options(ray_traced));
        scene(&mut harness, None);
        harness.apply(vec![
            RenderDiff::DefineMaterial {
                material: RenderMaterialDescriptor {
                    alpha_mode: MaterialAlphaModeDescriptor::Blend,
                    ..material("material/glass", [0.8, 0.8, 0.8, 0.95], None)
                },
            },
            static_mesh(
                "mesh/glass",
                box_mesh([-1.0, 0.5, -7.0], [1.0, 0.52, -5.0], |_| 0),
                "material/glass",
            ),
            instance(
                GLASS,
                None,
                "mesh/glass",
                transform([0.0; 3], 0.0, [1.0; 3]),
            ),
        ]);
        frame(&mut harness);
        let [left, right] = shadowed(&frame(&mut harness));
        left + right
    };
    near(render(true), render(false), "the shadow on the glass");
}

/// Under a temporal stage each ray leans within the sun's disc, so a wide
/// sun's shadow edge settles into shades between lit and shadowed; with
/// multisampling every ray aims at its centre and the edge stays hard.
fn a_temporal_stage_gathers_a_penumbra(gpu: &Gpu) {
    let edge = |antialiasing: Antialiasing, frames: usize| {
        let mut harness = Harness::on(gpu.clone(), options(true));
        scene(&mut harness, None);
        harness.apply(vec![
            RenderDiff::SetAtmosphere {
                atmosphere: Some(AtmosphereDescriptor {
                    sun_radius_degrees: WIDE_SUN_DEGREES,
                    ..AtmosphereDescriptor::default()
                }),
            },
            RenderDiff::SetRendererSettings {
                settings: RendererSettingsDescriptor {
                    antialiasing,
                    shadows: true,
                    ray_traced_shadows: true,
                    ..RendererSettingsDescriptor::DEFAULT
                },
            },
        ]);
        let samples = harness.renderer.samples();
        harness.target = OffscreenTarget::new(&harness.gpu, WIDTH, HEIGHT, samples);
        let mut picture = frame(&mut harness);
        for _ in 1..frames {
            picture = frame(&mut harness);
        }
        between(&picture)
    };
    let (hard, soft) = (
        edge(Antialiasing::Msaa4, 2),
        edge(Antialiasing::Temporal, 64),
    );
    assert!(
        soft > hard * 2,
        "{soft} penumbra pixels under the temporal stage, {hard} with multisampling"
    );
}

/// Pixels of the left half (the box's shadow) in the middle of the range
/// from the darkest to the brightest there.
fn between(rgba: &[u8]) -> usize {
    let luma = |pixel: &[u8; 4]| u32::from(pixel[0]) + u32::from(pixel[1]) + u32::from(pixel[2]);
    let left: Vec<u32> = rgba
        .as_chunks::<4>()
        .0
        .iter()
        .enumerate()
        .filter(|(index, _)| (*index as u32 % WIDTH) < WIDTH / 2)
        .map(|(_, pixel)| luma(pixel))
        .collect();
    let (low, high) = (
        left.iter().copied().min().unwrap_or(0),
        left.iter().copied().max().unwrap_or(0),
    );
    let range = high - low;
    left.iter()
        .filter(|luma| (low + range / 4..high - range / 4).contains(luma))
        .count()
}

/// A device made without ray queries refuses the setting and keeps the
/// sun's cascades. In the test above, so the binary's devices are made one
/// at a time.
fn refused_without_ray_queries() {
    let gpu = Gpu::headless().expect("a device");
    let mut harness = Harness::on(gpu, options(true));
    scene(&mut harness, None);
    frame(&mut harness);
    let refused = harness.renderer.settings_readout().ray_traced_shadows;
    assert_eq!(refused, Some(SettingRefusal::NoRayQueries));
    assert!(
        harness.renderer.shadow_report().layers > 0,
        "the cascades draw"
    );
    let [box_shadow, _] = shadowed(&frame(&mut harness));
    assert!(box_shadow > 200, "the box casts: {box_shadow}");
}
