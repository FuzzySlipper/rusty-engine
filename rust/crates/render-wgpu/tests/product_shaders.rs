//! Product shaders (#9099): a material naming a WGSL module that shades its
//! surface in place of the standard shade stage, beside standard materials,
//! in the opaque, blend and shadow passes; their standard-feature variants;
//! redefinition; and a shader that does not compose.

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
