//! GPU visibility and indirect drawing (#5912) on a real headless device:
//! a scene of many crates spread past the frustum draws the same picture
//! whether the CPU or the GPU culls it, the GPU path keeps its candidates
//! across camera moves and rebuilds them on a regroup, and the cull reports
//! its counts and time. Needs a wgpu adapter: CI uses `WGPU_BACKEND=vulkan`
//! with llvmpipe.

mod support;

use render_host_contracts::RendererCompositionCamera;
use render_model::*;
use render_wgpu::RendererOptions;
use support::*;

const MATERIAL: &str = "material/cull-plaster";
const GLASS: &str = "material/cull-glass";
const CRATE: &str = "static-mesh/cull-crate";
const PANE: &str = "static-mesh/cull-pane";
const FLOOR: &str = "static-mesh/cull-floor";
/// Crates in a wide grid; most lie outside any one view.
const CRATES: u64 = 400;

fn options(gpu: bool) -> RendererOptions {
    RendererOptions {
        gpu_culling: gpu,
        ..RendererOptions::default()
    }
}

fn wide_scene() -> Harness {
    let mut harness = Harness::new(options(false));
    let mut glass = material(GLASS, [0.4, 0.7, 0.9, 0.5], None);
    glass.alpha_mode = MaterialAlphaModeDescriptor::Blend;
    let mut ops = vec![
        RenderDiff::DefineMaterial {
            material: material(MATERIAL, [0.6, 0.55, 0.5, 1.0], None),
        },
        RenderDiff::DefineMaterial { material: glass },
        static_mesh(
            FLOOR,
            box_mesh([-40.0, -0.2, -40.0], [40.0, 0.0, 40.0], |_| 0),
            MATERIAL,
        ),
        static_mesh(
            CRATE,
            box_mesh([-0.5, 0.0, -0.5], [0.5, 1.0, 0.5], |_| 0),
            MATERIAL,
        ),
        static_mesh(
            PANE,
            box_mesh([-0.6, 0.0, -0.05], [0.6, 1.2, 0.05], |_| 0),
            GLASS,
        ),
        instance(10, None, FLOOR, transform([0.0; 3], 0.0, [1.0; 3])),
    ];
    for index in 0..CRATES {
        let column = (index % 20) as f32;
        let row = (index / 20) as f32;
        ops.push(instance(
            100 + index,
            None,
            CRATE,
            transform(
                [column * 3.0 - 28.5, 0.0, row * 3.0 - 28.5],
                (index * 37 % 360) as f32,
                [1.0; 3],
            ),
        ));
    }
    // A few blended panes among the crates: they stay on the CPU list.
    for index in 0..6 {
        ops.push(instance(
            600 + index,
            None,
            PANE,
            transform([index as f32 * 2.0 - 5.0, 0.0, -1.0], 0.0, [1.0; 3]),
        ));
    }
    harness.apply(ops);
    harness
}

fn view(yaw: f64) -> RendererCompositionCamera {
    camera([0.0, 2.5, 6.0], yaw, -18.0)
}

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
fn gpu_culling_draws_the_cpu_picture_across_camera_moves() {
    let mut harness = wide_scene();
    for yaw in [0.0, 90.0, 215.0] {
        harness.renderer.set_options(options(false));
        let (cpu_stats, cpu) = harness.render(&view(yaw));
        harness.renderer.set_options(options(true));
        let (gpu_stats, gpu) = harness.render(&view(yaw));
        let readout = harness.renderer.gpu_readout().gpu_culling;
        if let Some(reason) = &readout.refused {
            eprintln!("GPU culling refused on this device: {reason}");
            assert!(!readout.enabled);
            assert_eq!(cpu, gpu, "a refusal draws from the CPU list as before");
            return;
        }
        assert!(readout.enabled, "yaw {yaw}");
        assert_eq!(
            readout.candidates,
            CRATES as u32 + 1,
            "every crate and the floor are candidates"
        );
        assert!(readout.batches >= 2, "{} batches", readout.batches);
        let fraction = differing(&cpu, &gpu);
        assert!(
            fraction < 0.002,
            "yaw {yaw}: {:.2}% of pixels differ between CPU and GPU culling",
            fraction * 100.0
        );
        // The GPU path draws every candidate batch; the CPU path only the
        // culled ones, but each batch is still one draw per mesh and
        // material run.
        assert!(gpu_stats.draws >= 2 && cpu_stats.draws >= 2);
    }
    assert_screenshot("gpu-culling", &harness.render(&view(215.0)).1);
}

#[test]
fn the_candidates_survive_moves_and_follow_regroups_and_the_cull_is_timed() {
    let mut harness = wide_scene();
    harness.renderer.set_options(options(true));
    for _ in 0..20 {
        harness.render(&view(0.0));
    }
    let readout = harness.renderer.gpu_readout();
    let culling = &readout.gpu_culling;
    let adapter = harness.gpu.adapter_summary();
    eprintln!(
        "{} ({}): {culling:?}, {:?}",
        adapter.name,
        adapter.backend,
        readout.passes.iter().find(|pass| pass.pass == "gpu-cull")
    );
    if culling.refused.is_some() {
        return;
    }
    assert!(culling.visible > 0 && culling.visible < culling.candidates);
    // A camera move re-culls without a new candidate list: only the blended
    // panes, sorted for the camera, upload again.
    let (stats, _) = harness.render(&view(45.0));
    assert!(
        stats.instances_uploaded <= 6,
        "a camera move uploads only the blended parts: {}",
        stats.instances_uploaded
    );
    // Hiding a crate regroups: the candidates rebuild and upload again.
    harness.apply(vec![RenderDiff::Update {
        handle: RenderHandle::new(100),
        transform: None,
        material: None,
        visible: Some(false),
        metadata: None,
    }]);
    let (stats, _) = harness.render(&view(45.0));
    assert!(
        stats.instances_uploaded > 0,
        "a regroup uploads the candidates"
    );
    assert_eq!(
        harness.renderer.gpu_readout().gpu_culling.candidates,
        CRATES as u32,
        "one crate fewer"
    );
    let timing = readout
        .passes
        .iter()
        .find(|pass| pass.pass == "gpu-cull")
        .expect("the cull is a timed pass");
    if readout.timestamps {
        assert!(timing.timed_frames > 0);
        assert!(timing.median_gpu_ms.is_finite());
    }
}
