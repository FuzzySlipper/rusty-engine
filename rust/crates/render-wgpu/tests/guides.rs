//! Guide targets (`motion.rs`, `world.wgsl` `OpaqueOut`): while inspection
//! shows a guide (`OverlayShown::guide`), the opaque world writes diffuse
//! albedo, specular albedo, and the normal with the roughness beside the
//! motion target, and the frame shows the one asked for. Blended parts
//! write none; without the view nothing draws them.

mod common;

use common::*;
use render_model::*;
use render_wgpu::{Guide, OverlayShown, RendererOptions};

const RED: [f32; 4] = [0.8, 0.1, 0.1, 1.0];
const GOLD: [f32; 4] = [1.0, 0.8, 0.3, 1.0];

/// A floor, a rough red box on the left, a smooth gold metal box on the
/// right and a blended glass pane in front of the red box, from an eye 1 m
/// up looking 10° down.
fn scene(harness: &mut Harness) {
    let mut ops = vec![RenderDiff::SetBackgroundColor {
        color: [0.2, 0.3, 0.5, 1.0],
    }];
    let mut ground = coloured_mesh("floor", floor(60.0), [0.5, 0.5, 0.5, 1.0]);
    if let Some(RenderDiff::DefineMaterial { material }) = ground.first_mut() {
        material.roughness = 0.3;
    }
    ops.extend(ground);
    let mut red = coloured_mesh("red", cube(), RED);
    let mut gold = coloured_mesh("gold", cube(), GOLD);
    let mut glass = coloured_mesh("glass", cube(), [0.2, 0.9, 0.2, 0.3]);
    for (ops, roughness, metalness, blend) in [
        (&mut red, 0.9, 0.0, false),
        (&mut gold, 0.2, 1.0, false),
        (&mut glass, 0.1, 0.0, true),
    ] {
        if let Some(RenderDiff::DefineMaterial { material }) = ops.first_mut() {
            material.roughness = roughness;
            material.metalness = metalness;
            if blend {
                material.alpha_mode = MaterialAlphaModeDescriptor::Blend;
            }
        }
    }
    ops.extend(red);
    ops.extend(gold);
    ops.extend(glass);
    ops.push(instance(
        1,
        None,
        "floor",
        transform([0.0, 0.0, -20.0], 0.0, 1.0),
    ));
    ops.push(instance(
        2,
        None,
        "red",
        transform([-1.0, 0.5, -4.0], 0.0, 1.0),
    ));
    ops.push(instance(
        3,
        None,
        "gold",
        transform([1.0, 0.5, -4.0], 0.0, 1.0),
    ));
    let mut pane = transform([-1.0, 0.5, -3.0], 0.0, 1.0);
    pane.scale = [1.2, 1.2, 0.05];
    ops.push(instance(4, None, "glass", pane));
    harness.apply(ops);
}

fn view(harness: &mut Harness, guide: Option<Guide>) -> Vec<u8> {
    harness.renderer.set_overlay_shown(OverlayShown {
        guide,
        ..OverlayShown::default()
    });
    harness.single(&camera("eye", [0.0, 1.0, 0.0], 0.0, -10.0))
}

/// The screen pixel of a world point seen by the fixture camera.
fn seen(point: [f32; 3]) -> (u32, u32) {
    let focal = (HEIGHT as f32 / 2.0) / 30f32.to_radians().tan();
    // Into the camera's frame: turned up by the 10° it looks down.
    let turn = 10f32.to_radians();
    let (dx, dy, dz) = (point[0], point[1] - 1.0, point[2]);
    let y = dy * turn.cos() - dz * turn.sin();
    let z = dy * turn.sin() + dz * turn.cos();
    let depth = -z;
    (
        (WIDTH as f32 / 2.0 + focal * dx / depth).round() as u32,
        (HEIGHT as f32 / 2.0 - focal * y / depth).round() as u32,
    )
}

fn at(picture: &[u8], point: [f32; 3]) -> [u8; 3] {
    let (x, y) = seen(point);
    let [r, g, b, _] = pixel(picture, WIDTH, x, y);
    [r, g, b]
}

fn near(found: [u8; 3], expected: [f32; 3], what: &str) {
    for axis in 0..3 {
        let value = expected[axis] * 255.0;
        assert!(
            (f32::from(found[axis]) - value).abs() <= 4.0,
            "{what}: {found:?}, expected {expected:?}"
        );
    }
}

#[test]
fn guides_show_albedo_reflectance_normals_and_roughness_of_the_opaque_world() {
    let mut harness = Harness::new(RendererOptions::default());
    scene(&mut harness);
    let picture = view(&mut harness, None);

    // Albedo without lighting: the red box's colour; a metal reflects no
    // diffuse light; the glass pane in front of the red box writes none.
    let diffuse = view(&mut harness, Some(Guide::DiffuseAlbedo));
    let red_face = [-1.2, 0.3, -3.5];
    let gold_face = [1.0, 0.5, -3.5];
    near(
        at(&diffuse, red_face),
        [0.8, 0.1, 0.1],
        "the red box behind the glass",
    );
    near(at(&diffuse, gold_face), [0.0, 0.0, 0.0], "the gold metal");
    near(at(&diffuse, [0.0, 0.0, -2.0]), [0.5, 0.5, 0.5], "the floor");

    // Specular albedo: a metal reflects its colour; the floor reflects more
    // at grazing angles (far away) than looking down at it (near).
    let specular = view(&mut harness, Some(Guide::SpecularAlbedo));
    let gold = at(&specular, gold_face);
    assert!(
        gold[0] > gold[2] + 60 && gold[0] > 150,
        "the gold reflects gold: {gold:?}"
    );
    let steep = at(&specular, [0.0, 0.0, -2.0])[1];
    let grazing = at(&specular, [0.0, 0.0, -30.0])[1];
    assert!(grazing > steep + 5, "grazing {grazing} over steep {steep}");

    // Normals: the boxes' faces toward the eye along +z, the floor up.
    let normals = view(&mut harness, Some(Guide::Normal));
    near(
        at(&normals, gold_face),
        [0.5, 0.5, 1.0],
        "the face toward the eye",
    );
    near(at(&normals, [0.0, 0.0, -2.0]), [0.5, 1.0, 0.5], "the floor");

    // Roughness as grey.
    let roughness = view(&mut harness, Some(Guide::Roughness));
    near(at(&roughness, red_face), [0.9; 3], "the rough red box");
    near(at(&roughness, gold_face), [0.2; 3], "the smooth gold");

    // Nothing drew where the sky is open: the guides stay clear.
    let [sky_x, sky_y] = [WIDTH / 2, 2];
    assert_eq!(pixel(&diffuse, WIDTH, sky_x, sky_y)[..3], [0, 0, 0]);

    // Without the view the frame draws as before.
    assert_eq!(view(&mut harness, None), picture);
}

#[test]
fn guides_draw_under_a_temporal_stage() {
    let mut harness = Harness::new(RendererOptions {
        samples: 1,
        temporal: true,
        ..RendererOptions::default()
    });
    scene(&mut harness);
    let diffuse = view(&mut harness, Some(Guide::DiffuseAlbedo));
    near(at(&diffuse, [0.0, 0.0, -2.0]), [0.5, 0.5, 0.5], "the floor");
}

/// A box 4 m ahead of an eye at its height, over the background.
fn lone_box(harness: &mut Harness, material: RenderMaterialDescriptor) {
    let id = material.id.clone();
    harness.apply(vec![
        RenderDiff::SetBackgroundColor {
            color: [0.2, 0.3, 0.5, 1.0],
        },
        RenderDiff::DefineMaterial { material },
        RenderDiff::DefineStaticMesh {
            asset: StaticMeshAsset {
                asset: "mesh/box".to_owned(),
                payload: cube(),
                material_slots: vec![MeshMaterialSlot {
                    slot: 0,
                    material: id,
                }],
                collision: MeshCollisionPolicy::VisualOnly,
            },
        },
        instance(1, None, "box", transform([0.0, 1.0, -4.0], 0.0, 1.0)),
    ]);
}

fn plain(id: &str, color: [f32; 4]) -> RenderMaterialDescriptor {
    let mut ops = coloured_mesh("unused", cube(), color);
    let Some(RenderDiff::DefineMaterial { mut material }) = ops.drain(..1).next() else {
        unreachable!()
    };
    material.id = id.to_owned();
    material
}

#[test]
fn a_view_over_part_of_the_output_clears_its_guides_each_frame() {
    let mut harness = Harness::new(RendererOptions::default());
    lone_box(&mut harness, plain("material/red", RED));
    harness.renderer.set_overlay_shown(OverlayShown {
        guide: Some(Guide::DiffuseAlbedo),
        ..OverlayShown::default()
    });
    // One view over the left half: it does not cover the output, so it
    // clears its viewport rather than the whole target.
    harness.renderer.set_view_composition(
        &composition(
            vec![camera("eye", [0.0, 1.0, 0.0], 0.0, 0.0)],
            vec![primary_view("main", "eye", viewport(0.0, 0.0, 0.5, 1.0), 0)],
        ),
        0.0,
    );
    let (_, drawn) = harness.composition(0.0);
    let centre = (WIDTH / 4, HEIGHT / 2);
    let red = pixel(&drawn, WIDTH, centre.0, centre.1);
    assert!(red[0] > 150 && red[1] < 60, "the box's albedo: {red:?}");
    // Hidden, the box leaves nothing behind in the guides.
    harness.apply(vec![RenderDiff::Update {
        handle: RenderHandle::new(1),
        transform: None,
        material: None,
        visible: Some(false),
        metadata: None,
    }]);
    let (_, cleared) = harness.composition(0.1);
    assert_eq!(pixel(&cleared, WIDTH, centre.0, centre.1)[..3], [0, 0, 0]);
}

/// Shades a blue, smoother copy of its surface through the standard stage.
const BLUE_COPY: &str = "#import rusty::types::Surface
#import rusty::shade::standard_shade

fn shade(surface: Surface) -> vec4<f32> {
    var blue = surface;
    blue.base = vec4<f32>(0.0, 0.0, 1.0, surface.base.a);
    blue.roughness = 0.25;
    return standard_shade(blue);
}
";

#[test]
fn a_product_shader_changing_its_surface_gets_guides_of_what_it_shaded() {
    let mut harness = Harness::new(RendererOptions::default());
    let mut material = plain("material/shaded", RED);
    material.shader = Some(MaterialShaderDescriptor {
        shader: "shader/blue-copy".to_owned(),
        parameters: [[0.0; 4]; 4],
        textures: [None, None],
        displacement_bound: 0.0,
    });
    harness.apply(vec![RenderDiff::DefineShader {
        shader: ShaderDescriptor {
            id: "shader/blue-copy".to_owned(),
            path: "shaders/blue-copy.wgsl".to_owned(),
            source: BLUE_COPY.to_owned(),
            keywords: Vec::new(),
        },
    }]);
    lone_box(&mut harness, material);
    let look = |harness: &mut Harness, guide| {
        harness.renderer.set_overlay_shown(OverlayShown {
            guide: Some(guide),
            ..OverlayShown::default()
        });
        let picture = harness.single(&camera("eye", [0.0, 1.0, 0.0], 0.0, 0.0));
        let [r, g, b, _] = pixel(&picture, WIDTH, WIDTH / 2, HEIGHT / 2);
        [r, g, b]
    };
    near(
        look(&mut harness, Guide::DiffuseAlbedo),
        [0.0, 0.0, 1.0],
        "the blue it shaded",
    );
    near(
        look(&mut harness, Guide::Roughness),
        [0.25; 3],
        "the roughness it shaded",
    );
}
