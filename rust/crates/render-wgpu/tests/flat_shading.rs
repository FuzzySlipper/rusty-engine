//! Flat shading: a material that shades each triangle from its own plane.

mod common;

use common::*;
use render_model::*;
use render_wgpu::RendererOptions;

fn directional(direction: [f32; 3]) -> RenderDiff {
    RenderDiff::CreateLight {
        handle: RenderHandle::new(77),
        parent: None,
        light: LightDescriptor::Directional {
            color: [1.0; 3],
            intensity: 3.0,
            enabled: true,
            direction,
            range: None,
            shadow_intent: LightShadowIntent::Disabled,
            shadow: Default::default(),
        },
    }
}

/// A lat/long sphere with smooth normals: every vertex normal points out
/// from the centre, so the standard shader grades it continuously; flat
/// shading takes each triangle's own plane instead.
fn smooth_sphere(rings: u32, segments: u32) -> MeshPayloadDescriptor {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    for ring in 0..=rings {
        let theta = std::f32::consts::PI * ring as f32 / rings as f32;
        for segment in 0..=segments {
            let phi = std::f32::consts::TAU * segment as f32 / segments as f32;
            let normal = [
                theta.sin() * phi.cos(),
                theta.cos(),
                theta.sin() * phi.sin(),
            ];
            positions.extend_from_slice(&normal);
            normals.extend_from_slice(&normal);
        }
    }
    let mut indices = Vec::new();
    for ring in 0..rings {
        for segment in 0..segments {
            let a = ring * (segments + 1) + segment;
            let b = a + segments + 1;
            indices.extend_from_slice(&[a, b, a + 1, a + 1, b, b + 1]);
        }
    }
    payload(positions, normals, indices)
}

/// Flat shading turns a smooth low-poly sphere into facets: along one row of
/// the lit sphere the smooth one grades through many shades, the flat one
/// steps through a few, and a material without the flag draws as before.
#[test]
fn flat_shading_facets_a_smooth_low_poly_sphere() {
    let render = |flat: bool| -> Vec<u8> {
        let mut harness = Harness::new(RendererOptions {
            default_world_lights: false,
            ..RendererOptions::default()
        });
        let mut ops = vec![RenderDiff::SetBackgroundColor {
            color: [0.0, 0.0, 0.0, 1.0],
        }];
        let mut sphere = coloured_mesh("sphere", smooth_sphere(8, 12), [0.8, 0.8, 0.8, 1.0]);
        if let RenderDiff::DefineMaterial { material } = &mut sphere[0] {
            material.flat_shading = flat;
        }
        ops.extend(sphere);
        ops.push(instance(
            1,
            None,
            "sphere",
            transform([0.0, 0.0, -3.0], 0.0, 1.0),
        ));
        ops.push(directional([-0.6, -0.5, -0.6]));
        harness.apply(ops);
        harness.single(&camera("eye", [0.0, 0.0, 0.0], 0.0, 0.0))
    };
    let shades = |frame: &[u8]| {
        let row = HEIGHT / 2;
        let mut values: Vec<u8> = (0..WIDTH)
            .map(|x| pixel(frame, WIDTH, x, row)[0])
            .filter(|&red| red > 8)
            .collect();
        values.sort_unstable();
        values.dedup();
        values.len()
    };
    let smooth = render(false);
    let flat = render(true);
    let (smooth_shades, flat_shades) = (shades(&smooth), shades(&flat));
    assert!(
        smooth_shades >= 24,
        "the smooth sphere grades through {smooth_shades} shades"
    );
    assert!(
        flat_shades * 2 < smooth_shades,
        "the flat sphere steps through {flat_shades} shades against {smooth_shades}"
    );
    assert_ne!(smooth, flat, "flat shading changes the image");
}
