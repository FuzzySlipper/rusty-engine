//! Water on a device that binds only WebGPU's guaranteed 16 sampled textures
//! per stage (R10183-1): no copy of the opaque scene's colour, so a water
//! material that asks to absorb and refract draws as water that does not,
//! and the GPU readout says why, with the water reflections setting off.
//! Its own test binary: it makes a device of its own.

mod common;

use common::*;
use render_model::*;
use render_wgpu::{Gpu, RendererOptions};

/// A floor under a blended water sheet, lit from above, on `gpu`.
fn scene(gpu: &Gpu, water: MaterialWaterDescriptor) -> Harness {
    let mut harness = Harness::on(
        gpu.clone(),
        RendererOptions {
            default_world_lights: false,
            ..RendererOptions::default()
        },
    );
    let mut ops = vec![RenderDiff::SetBackgroundColor {
        color: [0.0, 0.0, 0.0, 1.0],
    }];
    ops.extend(coloured_mesh("floor", floor(4.0), [0.6, 0.6, 0.6, 1.0]));
    let mut sheet = coloured_mesh("sheet", floor(8.0), [1.0, 1.0, 1.0, 0.3]);
    if let RenderDiff::DefineMaterial { material } = &mut sheet[0] {
        material.alpha_mode = MaterialAlphaModeDescriptor::Blend;
        material.roughness = 0.1;
        material.water = Some(water);
    }
    ops.extend(sheet);
    ops.push(instance(
        1,
        None,
        "floor",
        transform([0.0, -1.0, -4.0], 0.0, 1.0),
    ));
    ops.push(instance(
        2,
        None,
        "sheet",
        transform([0.0, 0.0, -4.0], 0.0, 1.0),
    ));
    ops.push(RenderDiff::CreateLight {
        handle: RenderHandle::new(77),
        parent: None,
        light: LightDescriptor::Directional {
            color: [1.0; 3],
            intensity: 3.0,
            enabled: true,
            direction: [0.1, -1.0, -0.2],
            range: None,
            shadow_intent: LightShadowIntent::Disabled,
            shadow: Default::default(),
        },
    });
    harness.apply(ops);
    harness
}

fn plain_water() -> MaterialWaterDescriptor {
    MaterialWaterDescriptor {
        shallow_color: [0.3, 0.9, 0.8],
        deep_color: [0.0, 0.05, 0.3],
        depth_scale: 1.0,
        shoreline_width: 0.01,
        foam_threshold: 1.0,
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

#[test]
fn absorbing_water_falls_back_and_reports_why_on_a_sixteen_texture_device() {
    let gpu = Gpu::headless_with_sampled_texture_limit(16).expect("a wgpu adapter");
    let absorbing = MaterialWaterDescriptor {
        absorption_color: [0.2, 0.6, 0.9],
        absorption_distance: 1.0,
        refraction: 0.5,
        ..plain_water()
    };
    let eye = camera("eye", [0.0, 2.5, 0.5], 0.0, -40.0);
    let mut harness = scene(&gpu, absorbing);
    let fallback = harness.single(&eye);
    let readout = harness.renderer.gpu_readout();
    let refused = readout.water.refused.expect("the readout says why");
    assert!(refused.contains("16"), "{refused}");
    // Reflections stay off: the refusal does not depend on them.
    let settings = harness.renderer.settings_readout();
    assert_eq!(
        settings.requested.water_reflections,
        WaterReflectionsQuality::Off
    );
    assert_eq!(settings.water_reflections, None);
    // It draws as water that does not read the scene.
    let plain = scene(&gpu, plain_water()).single(&eye);
    assert_eq!(fallback, plain, "absorbing water draws as plain water");
}
