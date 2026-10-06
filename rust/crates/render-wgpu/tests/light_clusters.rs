//! Clustered forward shading (#5938) on a real headless device: shading a
//! scene of many ranged point lights from each fragment's cluster draws the
//! same picture as looping over every light, the lighting loop stays
//! available, and the binning reports its counts and GPU time. Needs a wgpu
//! adapter: CI uses `WGPU_BACKEND=vulkan` with llvmpipe.

mod support;

use render_host_contracts::RendererCompositionCamera;
use render_model::*;
use render_wgpu::RendererOptions;
use support::*;

const MATERIAL: &str = "material/cluster-plaster";
const CRATE: &str = "static-mesh/cluster-crate";
const FLOOR: &str = "static-mesh/cluster-floor";
/// Ranged point lights in the scene; several per screen tile.
const POINT_LIGHTS: u64 = 60;

fn options(clustered: bool) -> RendererOptions {
    RendererOptions {
        clustered_lighting: clustered,
        ..RendererOptions::default()
    }
}

/// A dark floor and a row of crates under a grid of small warm point
/// lights, a cool directional light and one unbounded point light.
fn lit_scene() -> Harness {
    let mut harness = Harness::new(RendererOptions {
        default_world_lights: false,
        ..options(false)
    });
    let mut ops = vec![
        RenderDiff::DefineMaterial {
            material: material(MATERIAL, [0.5, 0.48, 0.45, 1.0], None),
        },
        static_mesh(
            FLOOR,
            box_mesh([-8.0, -0.2, -8.0], [8.0, 0.0, 8.0], |_| 0),
            MATERIAL,
        ),
        static_mesh(
            CRATE,
            box_mesh([-0.4, 0.0, -0.4], [0.4, 0.8, 0.4], |_| 0),
            MATERIAL,
        ),
        instance(10, None, FLOOR, transform([0.0; 3], 0.0, [1.0; 3])),
    ];
    for index in 0..12 {
        ops.push(instance(
            20 + index,
            None,
            CRATE,
            transform(
                [index as f32 * 1.2 - 6.6, 0.0, -2.0],
                (index * 30) as f32,
                [1.0; 3],
            ),
        ));
    }
    for index in 0..POINT_LIGHTS {
        let column = (index % 10) as f32;
        let row = (index / 10) as f32;
        ops.push(RenderDiff::CreateLight {
            handle: RenderHandle::new(100 + index),
            parent: None,
            light: LightDescriptor::Point {
                color: [1.0, 0.7 + 0.05 * row, 0.4],
                intensity: 1.5,
                enabled: true,
                position: [column * 1.5 - 6.75, 0.6, row * 1.4 - 4.0],
                range: Some(2.5),
                decay: 2.0,
                shadow_intent: LightShadowIntent::Disabled,
                shadow: Default::default(),
            },
        });
    }
    ops.push(RenderDiff::CreateLight {
        handle: RenderHandle::new(200),
        parent: None,
        light: LightDescriptor::Directional {
            color: [0.4, 0.5, 0.8],
            intensity: 0.6,
            enabled: true,
            direction: [0.3, -1.0, -0.2],
            range: None,
            shadow_intent: LightShadowIntent::Disabled,
            shadow: Default::default(),
        },
    });
    // A point light without a range lights everything: a global light.
    ops.push(RenderDiff::CreateLight {
        handle: RenderHandle::new(201),
        parent: None,
        light: LightDescriptor::Point {
            color: [0.2, 0.3, 0.5],
            intensity: 4.0,
            enabled: true,
            position: [0.0, 6.0, 4.0],
            range: None,
            decay: 2.0,
            shadow_intent: LightShadowIntent::Disabled,
            shadow: Default::default(),
        },
    });
    harness.apply(ops);
    harness
}

fn view() -> RendererCompositionCamera {
    camera([0.0, 3.0, 5.5], 0.0, -28.0)
}

fn render_with(harness: &mut Harness, clustered: bool) -> Vec<u8> {
    harness.renderer.set_options(RendererOptions {
        default_world_lights: false,
        ..options(clustered)
    });
    harness.render(&view()).1
}

/// Pixels whose channels differ by more than the screenshot tolerance.
fn differing(a: &[u8], b: &[u8]) -> f64 {
    let count = a
        .as_chunks::<4>()
        .0
        .iter()
        .zip(b.as_chunks::<4>().0)
        .filter(|(a, b)| a.iter().zip(b.iter()).any(|(a, b)| a.abs_diff(*b) > 12))
        .count();
    count as f64 / f64::from(WIDTH * HEIGHT)
}

#[test]
fn clustered_shading_draws_the_looped_picture() {
    let mut harness = lit_scene();
    let looped = render_with(&mut harness, false);
    let loop_readout = harness.renderer.gpu_readout();
    assert!(!loop_readout.light_clusters.enabled);
    let clustered = render_with(&mut harness, true);
    let readout = harness.renderer.gpu_readout().light_clusters;
    if let Some(reason) = &readout.refused {
        eprintln!("clustering refused on this device: {reason}");
        assert!(!readout.enabled);
        assert_eq!(looped, clustered, "a refusal loops as before");
        return;
    }
    assert!(readout.enabled);
    assert_eq!(readout.grid, [16, 9, 24]);
    // The scene is lit: the floor under a lamp is warmer than the far
    // corner, so an all-dark or all-global picture would not pass.
    let lit_pixels = looped
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|p| p[0] > 60)
        .count();
    assert!(lit_pixels > 1000, "{lit_pixels} lit pixels");
    let fraction = differing(&looped, &clustered);
    assert!(
        fraction < 0.002,
        "{:.2}% of pixels differ between looped and clustered shading",
        fraction * 100.0
    );
    assert_screenshot("light-clusters", &clustered);
}

#[test]
fn the_binning_reports_its_lists_and_time() {
    let mut harness = lit_scene();
    harness.renderer.set_options(RendererOptions {
        default_world_lights: false,
        ..options(true)
    });
    // Stats and stamps read back a frame or more after they are written.
    for _ in 0..30 {
        harness.render(&view());
    }
    let readout = harness.renderer.gpu_readout();
    let clusters = &readout.light_clusters;
    let adapter = harness.gpu.adapter_summary();
    eprintln!(
        "{} ({}): {clusters:?}, {:?}",
        adapter.name,
        adapter.backend,
        readout
            .passes
            .iter()
            .find(|pass| pass.pass == "light-clusters")
    );
    if clusters.refused.is_some() {
        return;
    }
    assert_eq!(
        clusters.global_lights, 2,
        "the directional light and the unbounded point light"
    );
    assert!(
        clusters.binned_lights > POINT_LIGHTS as u32,
        "each ranged light lands in several clusters: {}",
        clusters.binned_lights
    );
    assert_eq!(clusters.overflowed_clusters, 0);
    let timing = readout
        .passes
        .iter()
        .find(|pass| pass.pass == "light-clusters")
        .expect("the binning is a timed pass");
    if readout.timestamps {
        assert!(timing.timed_frames > 0);
        assert!(timing.median_gpu_ms.is_finite() && timing.median_gpu_ms >= 0.0);
    }
}
