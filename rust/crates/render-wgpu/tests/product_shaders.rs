//! Product shaders (#9099): a material naming a WGSL module that shades its
//! surface in place of the standard shade stage, beside standard materials,
//! in the opaque, blend and shadow passes; their standard-feature variants;
//! redefinition; a shader that does not compose; its own textures and the
//! presentation time (#9126); keywords and a caster stage (#9127); a
//! displace stage that moves the vertices and their shadow (#9541), and a
//! displaced normal that lights them smoothly (#9905), and a displacement
//! bound that grows what culls them (#9906).

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
        displacement_bound: 0.0,
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
                range: None,
                shadow_intent: LightShadowIntent::Requested,
                shadow: Default::default(),
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
                shadow: Default::default(),
                range: None,
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
        displacement_bound: 0.0,
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
        displacement_bound: 0.0,
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
            displacement_bound: 0.0,
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
        displacement_bound: 0.0,
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

/// Lifts the box by its first parameter, scaled by a wave of presentation
/// time, in the image and the shadow alike.
const RISING: &str = "#import rusty::types::{Surface, Vertex}
#import rusty::view::frame
#import rusty::material::material
#import rusty::shade::standard_shade

fn displace(vertex: Vertex) -> vec3<f32> {
    let lift = material.parameters[0].x * (0.5 + 0.5 * sin(frame.time.x));
    return vertex.world_position + vec3<f32>(0.0, lift, 0.0);
}

fn shade(surface: Surface) -> vec4<f32> {
    return standard_shade(surface);
}
";

/// The first row from the top of the image with the right box's unlit
/// faces (black; the background above them is not): its front face when it
/// stands, its underside once it rises above the eye.
fn top_of_right_box(rgba: &[u8]) -> u32 {
    (0..HEIGHT)
        .find(|&y| (WIDTH / 2 + 10..WIDTH - 10).any(|x| pixel(rgba, x, y)[0] < 8))
        .expect("the right box in view")
}

#[test]
fn a_displace_stage_moves_the_vertices_with_time_and_the_shadow_follows() {
    let mut harness = Harness::new(RendererOptions {
        default_world_lights: false,
        shadows: true,
        ..RendererOptions::default()
    });
    let mut rising = material("material/right", [0.8, 0.8, 0.8, 1.0], None);
    rising.shader = Some(MaterialShaderDescriptor {
        shader: "shader/rising".to_owned(),
        parameters: [[0.6, 0.0, 0.0, 0.0], [0.0; 4], [0.0; 4], [0.0; 4]],
        textures: [None, None],
        displacement_bound: 0.0,
    });
    harness.apply(vec![shader("shader/rising", RISING)]);
    scene(&mut harness, rising);
    let view = camera(VIEW.0, VIEW.1, VIEW.2);
    let mut at = |time: f64| {
        harness.renderer.set_animation_time(time);
        harness.render(&view).1
    };
    // sin is -1 at 3π/2 (no lift) and 1 at π/2 (the whole lift).
    let grounded = at(std::f64::consts::FRAC_PI_2 * 3.0);
    let lifted = at(std::f64::consts::FRAC_PI_2);
    let held = at(std::f64::consts::FRAC_PI_2);
    assert!(
        top_of_right_box(&lifted) + 6 < top_of_right_box(&grounded),
        "the box rises from row {} to {}",
        top_of_right_box(&grounded),
        top_of_right_box(&lifted)
    );
    assert_eq!(held, lifted, "a held time draws the same");
    // Nothing else changes between frames: the shadow maps redraw for the
    // time alone, and the lifted box throws a different shadow.
    assert!(
        shadowed(&grounded) > 500,
        "the box casts {}",
        shadowed(&grounded)
    );
    let grounded_shadow: Vec<(u32, u32)> = (0..WIDTH)
        .flat_map(|x| (0..HEIGHT).map(move |y| (x, y)))
        .filter(|&(x, y)| pixel(&grounded, x, y)[0] < 60)
        .collect();
    let lifted_shadow: Vec<(u32, u32)> = (0..WIDTH)
        .flat_map(|x| (0..HEIGHT).map(move |y| (x, y)))
        .filter(|&(x, y)| pixel(&lifted, x, y)[0] < 60)
        .collect();
    assert_ne!(
        grounded_shadow, lifted_shadow,
        "the shadow follows the lift"
    );
}

/// Rolls a grid into waves along x and z, and lights it with a normal by
/// keyword: `ANALYTIC` the wave's own slope from a displaced normal stage,
/// `DERIVATIVE` each triangle's plane from screen derivatives, neither the
/// mesh's undisplaced normal.
const ROLLING: &str = "#import rusty::types::{Surface, Vertex}
#import rusty::shade::standard_shade

const HEIGHT: f32 = 0.4;
const ALONG: vec2<f32> = vec2<f32>(1.5, 0.7);

fn displace(vertex: Vertex) -> vec3<f32> {
    let p = vertex.world_position;
    return p + vec3<f32>(0.0, HEIGHT * sin(dot(ALONG, p.xz)), 0.0);
}

#ifdef ANALYTIC
fn displace_normal(vertex: Vertex) -> vec3<f32> {
    let slope = HEIGHT * cos(dot(ALONG, vertex.world_position.xz)) * ALONG;
    return vec3<f32>(-slope.x, 1.0, -slope.y);
}
#endif

fn shade(surface: Surface) -> vec4<f32> {
    var lit = surface;
#ifdef DERIVATIVE
    let plane = cross(dpdx(surface.world_position), dpdy(surface.world_position));
    lit.normal = normalize(plane * sign(dot(plane, surface.normal)));
#endif
    return standard_shade(lit);
}
";

/// A flat grid of 1 m cells from `-half` to `half` over x and z, facing up.
fn grid(half: i32) -> MeshPayloadDescriptor {
    let (mut positions, mut normals, mut uvs, mut indices) = (vec![], vec![], vec![], vec![]);
    let side = (2 * half + 1) as u32;
    for z in -half..=half {
        for x in -half..=half {
            positions.extend_from_slice(&[x as f32, 0.0, z as f32]);
            normals.extend_from_slice(&[0.0, 1.0, 0.0]);
            uvs.extend_from_slice(&[x as f32, z as f32]);
        }
    }
    for row in 0..side - 1 {
        for column in 0..side - 1 {
            let at = row * side + column;
            indices.extend_from_slice(&[at, at + side, at + 1, at + 1, at + side, at + side + 1]);
        }
    }
    let count = indices.len() as u32;
    payload(positions, normals, uvs, indices, &[(0, count)])
}

/// The grey level of each pixel in the middle of the image, which the
/// rolled grid covers.
fn grey_rows(rgba: &[u8]) -> Vec<Vec<i32>> {
    (HEIGHT / 4..HEIGHT * 3 / 4)
        .map(|y| {
            (WIDTH / 4..WIDTH * 3 / 4)
                .map(|x| {
                    let [r, g, b, _] = pixel(rgba, x, y);
                    (r as i32 + g as i32 + b as i32) / 3
                })
                .collect()
        })
        .collect()
}

/// Steps between neighbouring pixels along a row larger than a smooth
/// gradient makes: a faceted surface's shading jumps at its triangles' edges.
fn facet_edges(rows: &[Vec<i32>]) -> usize {
    rows.iter()
        .flat_map(|row| row.windows(2))
        .filter(|pair| (pair[0] - pair[1]).abs() > 6)
        .count()
}

fn grey_range(rows: &[Vec<i32>]) -> i32 {
    let all = || rows.iter().flatten();
    all().max().unwrap() - all().min().unwrap()
}

#[test]
fn a_displaced_normal_lights_a_rolled_grid_smoothly() {
    let render = |keywords: &[&str]| {
        let mut harness = Harness::new(RendererOptions {
            default_world_lights: false,
            ..RendererOptions::default()
        });
        let mut rolled = material("material/rolled", [0.8, 0.8, 0.8, 1.0], None);
        rolled.shader = Some(MaterialShaderDescriptor {
            shader: "shader/rolling".to_owned(),
            parameters: [[0.0; 4]; 4],
            textures: [None, None],
            displacement_bound: 0.0,
        });
        harness.apply(vec![
            RenderDiff::DefineShader {
                shader: ShaderDescriptor {
                    id: "shader/rolling".to_owned(),
                    path: "shaders/rolling.wgsl".to_owned(),
                    source: ROLLING.to_owned(),
                    keywords: keywords.iter().map(|keyword| keyword.to_string()).collect(),
                },
            },
            RenderDiff::DefineMaterial { material: rolled },
            static_mesh("mesh/grid", grid(8), "material/rolled"),
            instance(
                1,
                None,
                "mesh/grid",
                transform([0.0, 0.0, -5.0], 0.0, [1.0; 3]),
            ),
            RenderDiff::CreateLight {
                handle: RenderHandle::new(2),
                parent: None,
                light: LightDescriptor::Directional {
                    color: [1.0; 3],
                    intensity: 2.0,
                    enabled: true,
                    direction: [0.8, -0.5, -0.3],
                    range: None,
                    shadow_intent: LightShadowIntent::Disabled,
                    shadow: Default::default(),
                },
            },
        ]);
        let image = harness.render(&camera([0.0, 3.0, 0.0], 0.0, -35.0)).1;
        grey_rows(&image)
    };
    let mesh = render(&[]);
    let derivative = render(&["DERIVATIVE"]);
    let analytic = render(&["ANALYTIC"]);
    let difference = |a: &[Vec<i32>], b: &[Vec<i32>]| {
        let pixels = a.iter().flatten().zip(b.iter().flatten());
        pixels.map(|(a, b)| (a - b).abs()).sum::<i32>() / a.iter().flatten().count() as i32
    };
    // The derivative normal jumps at every triangle's edge; the displaced
    // normal varies smoothly across them.
    assert!(
        facet_edges(&derivative) > 200,
        "the derivative normal shows {} facet edges",
        facet_edges(&derivative)
    );
    assert!(
        facet_edges(&analytic) * 20 < facet_edges(&derivative),
        "the displaced normal shows {} facet edges against {}",
        facet_edges(&analytic),
        facet_edges(&derivative)
    );
    // It lights the waves as their slope does; the mesh's normal leaves
    // the rolled grid evenly lit.
    assert!(
        grey_range(&mesh) < 10,
        "the mesh normal lights the grid evenly: {}",
        grey_range(&mesh)
    );
    assert!(
        grey_range(&analytic) > 60,
        "the displaced normal lights the slopes: {}",
        grey_range(&analytic)
    );
    assert!(
        difference(&analytic, &derivative) * 2 < difference(&mesh, &derivative),
        "the displaced normal follows the slope: {} from the derivative's, the mesh's {}",
        difference(&analytic, &derivative),
        difference(&mesh, &derivative)
    );
}

/// Moves the box by its first parameter: from where its mesh stands, out of
/// view, to above the floor in view.
const CARRIED: &str = "#import rusty::types::{Surface, Vertex}
#import rusty::material::material
#import rusty::shade::standard_shade

fn displace(vertex: Vertex) -> vec3<f32> {
    return vertex.world_position + material.parameters[0].xyz;
}

fn shade(surface: Surface) -> vec4<f32> {
    return standard_shade(surface);
}
";

/// Pixels of the carried box (red, lit or not).
fn red(rgba: &[u8]) -> usize {
    rgba.as_chunks::<4>()
        .0
        .iter()
        .filter(|[r, g, b, _]| {
            *r > 40 && u16::from(*g) * 3 < u16::from(*r) && u16::from(*b) * 3 < u16::from(*r)
        })
        .count()
}

/// Floor pixels in shadow (dark grey; the box's lit faces are red).
fn floor_shadow(rgba: &[u8]) -> usize {
    rgba.as_chunks::<4>()
        .0
        .iter()
        .filter(|[r, g, b, _]| *r < 60 && r.abs_diff(*g) < 8 && r.abs_diff(*b) < 8)
        .count()
}

#[test]
fn a_displacement_bound_keeps_a_carried_part_drawn_and_shadowed() {
    // The box's mesh stands 30 m to the right; its displace stage carries it
    // back onto the floor, left of the middle of the view.
    const CARRY: [f32; 4] = [-31.5, 0.0, 0.0, 0.0];
    let carried = |bound: f32| {
        let mut descriptor = material("material/carried", [1.0, 0.0, 0.0, 1.0], None);
        descriptor.shader = Some(MaterialShaderDescriptor {
            shader: "shader/carried".to_owned(),
            parameters: [CARRY, [0.0; 4], [0.0; 4], [0.0; 4]],
            textures: [None, None],
            displacement_bound: bound,
        });
        RenderDiff::DefineMaterial {
            material: descriptor,
        }
    };
    let mut harness = Harness::new(RendererOptions {
        default_world_lights: false,
        shadows: true,
        ..RendererOptions::default()
    });
    harness.apply(vec![
        shader("shader/carried", CARRIED),
        RenderDiff::DefineMaterial {
            material: material("material/plain", [0.8, 0.8, 0.8, 1.0], None),
        },
        carried(0.0),
        static_mesh(
            "mesh/floor",
            box_mesh([-6.0, -1.0, -8.0], [6.0, 0.0, 2.0], |_| 0),
            "material/plain",
        ),
        static_mesh(
            "mesh/box",
            box_mesh([-0.5, 0.0, -0.5], [0.5, 1.0, 0.5], |_| 0),
            "material/carried",
        ),
        instance(1, None, "mesh/floor", Transform::IDENTITY),
        instance(
            2,
            None,
            "mesh/box",
            transform([30.0, 0.0, -3.0], 0.0, [1.0; 3]),
        ),
        RenderDiff::CreateLight {
            handle: RenderHandle::new(3),
            parent: None,
            light: LightDescriptor::Directional {
                color: [1.0; 3],
                intensity: 2.0,
                enabled: true,
                // From the right, behind the camera: the faces in view are
                // lit, and the shadow falls back and to the left.
                direction: [-1.0, -1.0, -0.6],
                range: None,
                shadow_intent: LightShadowIntent::Requested,
                shadow: Default::default(),
            },
        },
    ]);
    let view = camera(VIEW.0, VIEW.1, VIEW.2);
    for gpu_culling in [false, true] {
        harness.renderer.set_options(RendererOptions {
            default_world_lights: false,
            shadows: true,
            gpu_culling,
            ..RendererOptions::default()
        });
        harness.apply(vec![carried(0.0)]);
        let unbounded = harness.render(&view).1;
        if gpu_culling {
            if let Some(reason) = harness.renderer.gpu_readout().gpu_culling.refused {
                eprintln!("GPU culling refused on this device: {reason}");
                return;
            }
        }
        // Redefining the material regrows its parts' bounds, and back.
        harness.apply(vec![carried(31.0)]);
        let bounded = harness.render(&view).1;
        harness.apply(vec![carried(0.0)]);
        let again = harness.render(&view).1;
        assert_eq!(
            (red(&unbounded), floor_shadow(&unbounded)),
            (0, 0),
            "gpu culling {gpu_culling}: culled by its mesh's bounds, unseen and casting nothing"
        );
        assert!(
            red(&bounded) > 300,
            "gpu culling {gpu_culling}: drawn within its bound: {} pixels",
            red(&bounded)
        );
        assert!(
            floor_shadow(&bounded) > 80,
            "gpu culling {gpu_culling}: its shadow on the floor: {} pixels",
            floor_shadow(&bounded)
        );
        assert_eq!(again, unbounded, "gpu culling {gpu_culling}: culled again");
    }
}
