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
