//! Product shaders (#9099): a material naming a WGSL module that shades its
//! surface in place of the standard shade stage, beside standard materials,
//! in the opaque, blend and shadow passes; their standard-feature variants;
//! redefinition; a shader that does not compose; its own textures and the
//! presentation time (#9126); keywords and a caster stage (#9127).

mod support;

use render_model::*;
use render_wgpu::RendererOptions;
use support::*;

/// Shades with its first parameter, through the standard finish; with a
/// normal map, with its second.
const FLAT: &str = "#import rusty::types::Surface
#import rusty::material::material
#import rusty::finish::finish

fn shade(surface: Surface) -> vec4<f32> {
#ifdef NORMAL_MAP
    let colour = material.parameters[1];
#else
    let colour = material.parameters[0];
#endif
    return finish(vec4<f32>(colour.rgb, colour.a * surface.base.a), surface.world_position);
}
";

fn shader(id: &str, source: &str) -> RenderDiff {
    RenderDiff::DefineShader {
        shader: ShaderDescriptor {
            id: id.to_owned(),
            path: format!("shaders/{}.wgsl", id.trim_start_matches("shader/")),
            source: source.to_owned(),
            keywords: Vec::new(),
        },
    }
}

fn shaded(
    id: &str,
    color: [f32; 4],
    first: [f32; 4],
    second: [f32; 4],
) -> RenderMaterialDescriptor {
    let mut descriptor = material(id, color, None);
    descriptor.shader = Some(MaterialShaderDescriptor {
        shader: "shader/flat".to_owned(),
        parameters: [first, second, [0.0; 4], [0.0; 4]],
        textures: [None, None],
    });
    descriptor
}

fn pixel(rgba: &[u8], x: u32, y: u32) -> [u8; 4] {
    let at = ((y * WIDTH + x) * 4) as usize;
    rgba[at..at + 4].try_into().unwrap()
}

/// A floor, a standard box on the left and a product-shaded box on the
/// right, under a sun that casts.
fn scene(harness: &mut Harness, right: RenderMaterialDescriptor) {
    let right_id = right.id.clone();
    harness.apply(vec![
        shader("shader/flat", FLAT),
        RenderDiff::DefineMaterial {
            material: material("material/plain", [0.8, 0.8, 0.8, 1.0], None),
        },
        RenderDiff::DefineMaterial { material: right },
        static_mesh(
            "mesh/floor",
            box_mesh([-6.0, -1.0, -8.0], [6.0, 0.0, 2.0], |_| 0),
            "material/plain",
        ),
        static_mesh(
            "mesh/left",
            box_mesh([-0.5, 0.0, -0.5], [0.5, 1.0, 0.5], |_| 0),
            "material/plain",
        ),
        static_mesh(
            "mesh/right",
            box_mesh([-0.5, 0.0, -0.5], [0.5, 1.0, 0.5], |_| 0),
            &right_id,
        ),
        instance(1, None, "mesh/floor", Transform::IDENTITY),
        instance(
            2,
            None,
            "mesh/left",
            transform([-1.2, 0.0, -3.0], 0.0, [1.0; 3]),
        ),
        instance(
            3,
            None,
            "mesh/right",
            transform([1.2, 0.0, -3.0], 0.0, [1.0; 3]),
        ),
        // Directional shadows cast from the light's node, behind the boxes.
        group(9, None, transform([0.0, 2.0, -6.0], 0.0, [1.0; 3])),
        RenderDiff::CreateLight {
            handle: RenderHandle::new(10),
            parent: Some(RenderHandle::new(9)),
            light: LightDescriptor::Directional {
                color: [1.0; 3],
                intensity: 2.0,
                enabled: true,
                direction: [0.0, -0.7, 1.0],
                shadow_intent: LightShadowIntent::Requested,
            },
        },
    ]);
}

const VIEW: ([f64; 3], f64, f64) = ([0.0, 1.5, 1.5], 0.0, -20.0);

#[test]
fn a_product_shader_shades_its_material_beside_standard_ones_and_casts_as_they_do() {
    let render = |right: RenderMaterialDescriptor| {
        let mut harness = Harness::new(RendererOptions {
            default_world_lights: false,
            shadows: true,
            ..RendererOptions::default()
        });
        scene(&mut harness, right);
        harness.render(&camera(VIEW.0, VIEW.1, VIEW.2)).1
    };
    let standard = render(material("material/right", [0.8, 0.8, 0.8, 1.0], None));
    let product = render(shaded(
        "material/right",
        [0.8, 0.8, 0.8, 1.0],
        [0.0, 1.0, 0.0, 1.0],
        [0.0; 4],
    ));
    let (left, right) = (
        (WIDTH / 4 + 25, HEIGHT / 2),
        (WIDTH * 3 / 4 - 25, HEIGHT / 2),
    );
    // The standard box and the floor are untouched; the product box is its
    // parameter colour, unlit.
    assert_eq!(
        pixel(&product, left.0, left.1),
        pixel(&standard, left.0, left.1)
    );
    assert_eq!(pixel(&product, right.0, right.1), [0, 255, 0, 255]);
    assert_ne!(pixel(&standard, right.0, right.1), [0, 255, 0, 255]);
    // It casts through the standard caster: its shadow on the floor in
    // front of it is the standard box's.
    let (shadow, open) = ((right.0, HEIGHT * 3 / 5), (right.0, HEIGHT - 10));
    assert_eq!(
        pixel(&product, shadow.0, shadow.1),
        pixel(&standard, shadow.0, shadow.1)
    );
    assert!(pixel(&product, shadow.0, shadow.1)[0] + 60 < pixel(&product, open.0, open.1)[0]);
    assert_screenshot("product-shader", &product);
}

#[test]
fn a_product_shader_blends_compiles_per_standard_feature_and_follows_redefinition() {
    let mut harness = Harness::new(RendererOptions {
        default_world_lights: false,
        ..RendererOptions::default()
    });
    // Blended: half the parameter's alpha over the floor.
    let mut glass = shaded(
        "material/right",
        [1.0, 1.0, 1.0, 0.5],
        [1.0, 0.0, 0.0, 1.0],
        [0.0; 4],
    );
    glass.alpha_mode = MaterialAlphaModeDescriptor::Blend;
    scene(&mut harness, glass);
    let view = camera(VIEW.0, VIEW.1, VIEW.2);
    let right = (WIDTH * 3 / 4 - 25, HEIGHT / 2);
    let blended = pixel(&harness.render(&view).1, right.0, right.1);
    assert!(blended[0] > 150 && blended[0] < 255, "{blended:?}");

    // A normal map makes the same shader compile its NORMAL_MAP branch.
    let mut data = harness.resources.texture(
        "texture/flat-normal",
        1,
        1,
        &[128, 128, 255, 255],
        TextureWrap::Repeat,
    );
    if let Some(payload) = data.payload.as_mut() {
        payload.color_space = TextureColorSpace::Linear;
    }
    let mut mapped = shaded(
        "material/right",
        [1.0; 4],
        [1.0, 0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0, 1.0],
    );
    mapped.normal_map = Some(MaterialNormalMapDescriptor {
        texture: data.id.clone(),
        scale: 1.0,
    });
    harness.apply(vec![
        RenderDiff::DefineTexture { texture: data },
        RenderDiff::DefineMaterial { material: mapped },
    ]);
    assert_eq!(
        pixel(&harness.render(&view).1, right.0, right.1),
        [0, 0, 255, 255]
    );

    // Redefining the shader (a content reload) recompiles what it shades.
    harness.apply(vec![shader(
        "shader/flat",
        &FLAT.replace("colour.rgb", "vec3<f32>(1.0, 1.0, 0.0)"),
    )]);
    assert_eq!(
        pixel(&harness.render(&view).1, right.0, right.1),
        [255, 255, 0, 255]
    );
}

#[test]
fn a_product_shader_that_does_not_compose_is_reported_by_line_and_shades_as_standard() {
    let mut harness = Harness::new(RendererOptions {
        default_world_lights: false,
        ..RendererOptions::default()
    });
    let broken = FLAT.replace("material.parameters[0]", "missing_value");
    let frame = harness
        .world
        .apply(RenderFrameDiff {
            publication: None,
            ops: vec![
                shader("shader/flat", &broken),
                RenderDiff::DefineMaterial {
                    material: shaded(
                        "material/right",
                        [0.8, 0.8, 0.8, 1.0],
                        [0.0, 1.0, 0.0, 1.0],
                        [0.0; 4],
                    ),
                },
            ],
        })
        .unwrap();
    let issues = harness.renderer.apply(&frame, &harness.resources);
    assert_eq!(issues.len(), 1, "{issues:?}");
    assert_eq!(issues[0].op, "defineMaterial");
    assert!(
        issues[0].detail.contains("shaders/flat.wgsl"),
        "{}",
        issues[0].detail
    );
    assert!(issues[0].detail.contains(":9:"), "{}", issues[0].detail);
    // It still draws, with the standard shade stage.
    harness.apply(vec![
        RenderDiff::DefineMaterial {
            material: material("material/plain", [0.8, 0.8, 0.8, 1.0], None),
        },
        static_mesh(
            "mesh/right",
            box_mesh([-0.5, 0.0, -0.5], [0.5, 1.0, 0.5], |_| 0),
            "material/right",
        ),
        instance(
            3,
            None,
            "mesh/right",
            transform([1.2, 0.0, -3.0], 0.0, [1.0; 3]),
        ),
        RenderDiff::CreateLight {
            handle: RenderHandle::new(10),
            parent: None,
            light: LightDescriptor::Ambient {
                color: [1.0; 3],
                intensity: std::f32::consts::PI,
                enabled: true,
                shadow_intent: LightShadowIntent::Disabled,
            },
        },
    ]);
    let right = (WIDTH * 3 / 4 - 25, HEIGHT / 2);
    let drawn = pixel(
        &harness.render(&camera(VIEW.0, VIEW.1, VIEW.2)).1,
        right.0,
        right.1,
    );
    assert_eq!(
        drawn[0], drawn[1],
        "grey, not the parameter's green: {drawn:?}"
    );
}

/// Its own two textures, alternating every half second of presentation time.
const BLINK: &str = "#import rusty::types::Surface
#import rusty::material::{product_map_a, product_sampler_a, product_map_b, product_sampler_b}
#import rusty::view::frame
#import rusty::finish::finish

fn shade(surface: Surface) -> vec4<f32> {
    let a = textureSample(product_map_a, product_sampler_a, surface.uv);
    let b = textureSample(product_map_b, product_sampler_b, surface.uv);
    let colour = select(a, b, fract(frame.time.x) >= 0.5);
    return finish(vec4<f32>(colour.rgb, 1.0), surface.world_position);
}
";

#[test]
fn a_product_shader_samples_its_own_textures_and_animates_with_presentation_time() {
    let mut harness = Harness::new(RendererOptions {
        default_world_lights: false,
        ..RendererOptions::default()
    });
    let red =
        harness
            .resources
            .texture("texture/red", 1, 1, &[255, 0, 0, 255], TextureWrap::Repeat);
    let blue =
        harness
            .resources
            .texture("texture/blue", 1, 1, &[0, 0, 255, 255], TextureWrap::Repeat);
    let mut blink = material("material/right", [1.0; 4], None);
    blink.shader = Some(MaterialShaderDescriptor {
        shader: "shader/blink".to_owned(),
        parameters: [[0.0; 4]; 4],
        textures: [Some(red.id.clone()), Some(blue.id.clone())],
    });
    harness.apply(vec![
        RenderDiff::DefineTexture { texture: red },
        RenderDiff::DefineTexture { texture: blue },
        shader("shader/blink", BLINK),
    ]);
    scene(&mut harness, blink);
    let view = camera(VIEW.0, VIEW.1, VIEW.2);
    let right = (WIDTH * 3 / 4 - 25, HEIGHT / 2);
    // No material update between frames: only the Engine's time moves.
    harness.renderer.set_animation_time(10.25);
    assert_eq!(
        pixel(&harness.render(&view).1, right.0, right.1),
        [255, 0, 0, 255]
    );
    harness.renderer.set_animation_time(10.75);
    assert_eq!(
        pixel(&harness.render(&view).1, right.0, right.1),
        [0, 0, 255, 255]
    );
    harness.renderer.set_animation_time(11.25);
    assert_eq!(
        pixel(&harness.render(&view).1, right.0, right.1),
        [255, 0, 0, 255]
    );
    // Each texture keeps its slot: with only B, A samples white.
    let mut only_b = material("material/right", [1.0; 4], None);
    only_b.shader = Some(MaterialShaderDescriptor {
        shader: "shader/blink".to_owned(),
        parameters: [[0.0; 4]; 4],
        textures: [None, Some("texture/blue".to_owned())],
    });
    harness.apply(vec![RenderDiff::DefineMaterial { material: only_b }]);
    assert_eq!(
        pixel(&harness.render(&view).1, right.0, right.1),
        [255, 255, 255, 255]
    );
    harness.renderer.set_animation_time(11.75);
    assert_eq!(
        pixel(&harness.render(&view).1, right.0, right.1),
        [0, 0, 255, 255]
    );
}

/// With DISSOLVE, the left part of each face (uv.x below the first
/// parameter) is cut away, in its image and its shadow alike.
const DISSOLVE: &str = "#import rusty::types::{Surface, Caster}
#import rusty::material::material
#import rusty::shade::standard_shade

fn shade(surface: Surface) -> vec4<f32> {
#ifdef DISSOLVE
    if surface.uv.x < material.parameters[0].x {
        discard;
    }
#endif
    return standard_shade(surface);
}

fn cast_shadow(caster: Caster) {
#ifdef DISSOLVE
    if caster.uv.x < material.parameters[0].x {
        discard;
    }
#endif
}
";

#[test]
fn a_keyword_variant_dissolves_a_material_and_its_caster_stage_cuts_the_shadow_alike() {
    let render = |keywords: &[&str]| {
        let mut harness = Harness::new(RendererOptions {
            default_world_lights: false,
            shadows: true,
            ..RendererOptions::default()
        });
        let mut dissolving = material("material/right", [0.8, 0.8, 0.8, 1.0], None);
        dissolving.shader = Some(MaterialShaderDescriptor {
            shader: "shader/dissolve".to_owned(),
            parameters: [[0.6, 0.0, 0.0, 0.0], [0.0; 4], [0.0; 4], [0.0; 4]],
            textures: [None, None],
        });
        harness.apply(vec![RenderDiff::DefineShader {
            shader: ShaderDescriptor {
                id: "shader/dissolve".to_owned(),
                path: "shaders/dissolve.wgsl".to_owned(),
                source: DISSOLVE.to_owned(),
                keywords: keywords.iter().map(|keyword| keyword.to_string()).collect(),
            },
        }]);
        scene(&mut harness, dissolving);
        harness.render(&camera(VIEW.0, VIEW.1, VIEW.2)).1
    };
    let (whole, dissolved) = (render(&[]), render(&["DISSOLVE"]));
    let (whole_shadow, dissolved_shadow) = (shadowed(&whole), shadowed(&dissolved));
    assert!(whole_shadow > 500, "the whole box casts {whole_shadow}");
    assert!(
        dissolved_shadow * 10 < whole_shadow * 7,
        "the dissolved box casts {dissolved_shadow} of {whole_shadow}"
    );
    // The image is cut too: background shows where the box was.
    let right = (WIDTH * 3 / 4 - 25, HEIGHT / 2);
    assert_ne!(
        pixel(&whole, right.0 - 12, right.1),
        pixel(&dissolved, right.0 - 12, right.1)
    );
    assert_screenshot("product-shader-dissolve", &dissolved);
}

/// Dark floor pixels in the right box's shadow, in front of it.
fn shadowed(rgba: &[u8]) -> usize {
    (HEIGHT / 2 + 10..HEIGHT * 3 / 4)
        .flat_map(|y| (WIDTH / 2 + 10..WIDTH - 10).map(move |x| (x, y)))
        .filter(|(x, y)| pixel(rgba, *x, *y)[0] < 60)
        .count()
}

/// Only its shadow dissolves, with presentation time: the left part of each
/// face (uv.x below the second's fraction) casts none.
const FADING_SHADOW: &str = "#import rusty::types::{Surface, Caster}
#import rusty::view::frame
#import rusty::shade::standard_shade

fn shade(surface: Surface) -> vec4<f32> {
    return standard_shade(surface);
}

fn cast_shadow(caster: Caster) {
    if caster.uv.x < fract(frame.time.x) {
        discard;
    }
}
";

#[test]
fn a_caster_stage_reads_presentation_time_and_the_shadow_follows_it() {
    let mut harness = Harness::new(RendererOptions {
        default_world_lights: false,
        shadows: true,
        ..RendererOptions::default()
    });
    let mut fading = material("material/right", [0.8, 0.8, 0.8, 1.0], None);
    fading.shader = Some(MaterialShaderDescriptor {
        shader: "shader/fading".to_owned(),
        parameters: [[0.0; 4]; 4],
        textures: [None, None],
    });
    harness.apply(vec![shader("shader/fading", FADING_SHADOW)]);
    scene(&mut harness, fading);
    let view = camera(VIEW.0, VIEW.1, VIEW.2);
    let mut at = |time: f64| {
        harness.renderer.set_animation_time(time);
        harness.render(&view).1
    };
    // Nothing else changes between frames: the maps redraw for time alone.
    let whole = at(10.0);
    let faded = at(10.6);
    let held = at(10.6);
    let again = at(11.0);
    assert!(
        shadowed(&whole) > 500,
        "the whole box casts {}",
        shadowed(&whole)
    );
    assert!(
        shadowed(&faded) * 10 < shadowed(&whole) * 7,
        "the faded box casts {} of {}",
        shadowed(&faded),
        shadowed(&whole)
    );
    assert!(held == faded, "a held time draws the same");
    assert!(again == whole, "the shadow follows time back");
}
