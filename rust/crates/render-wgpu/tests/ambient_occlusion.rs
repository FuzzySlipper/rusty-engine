//! Screen-space ambient occlusion (#9510) on a real headless device: the two
//! paths darken the ambient light where geometry meets, agree with each
//! other, leave open surfaces alone, fall back cleanly, and report their
//! GPU time through the pass timers. Same harness, tolerance and blessing as
//! `tests/screenshots.rs` (`RENDER_WGPU_BLESS=1` rewrites the references).
//! Needs a wgpu adapter: CI uses `WGPU_BACKEND=vulkan` with llvmpipe.

mod support;

use render_host_contracts::RendererCompositionCamera;
use render_model::*;
use render_wgpu::{AmbientOcclusion, AmbientOcclusionPath, RendererOptions};
use support::*;

const FLOOR: &str = "static-mesh/ao-floor";
const CRATE: &str = "static-mesh/ao-crate";
const WALL: &str = "static-mesh/ao-wall";
const MATERIAL: &str = "material/ao-plaster";

fn options(path: AmbientOcclusionPath, strength: f32) -> RendererOptions {
    RendererOptions {
        ambient_occlusion: AmbientOcclusion { path, strength },
        ..RendererOptions::default()
    }
}

/// A floor, a wall along its back edge and two crates: the corners between
/// them are where occlusion shows.
fn corner_scene() -> Harness {
    let mut harness = Harness::new(options(AmbientOcclusionPath::Off, 1.0));
    harness.apply(vec![
        RenderDiff::DefineMaterial {
            material: material(MATERIAL, [0.45, 0.42, 0.4, 1.0], None),
        },
        static_mesh(
            FLOOR,
            box_mesh([-6.0, -0.2, -6.0], [6.0, 0.0, 6.0], |_| 0),
            MATERIAL,
        ),
        static_mesh(
            WALL,
            box_mesh([-6.0, 0.0, -2.2], [6.0, 2.5, -2.0], |_| 0),
            MATERIAL,
        ),
        static_mesh(
            CRATE,
            box_mesh([-0.5, 0.0, -0.5], [0.5, 1.0, 0.5], |_| 0),
            MATERIAL,
        ),
        instance(10, None, FLOOR, transform([0.0; 3], 0.0, [1.0; 3])),
        instance(11, None, WALL, transform([0.0; 3], 0.0, [1.0; 3])),
        instance(
            12,
            None,
            CRATE,
            transform([-0.6, 0.0, -1.4], 20.0, [1.0; 3]),
        ),
        instance(
            13,
            None,
            CRATE,
            transform([1.4, 0.0, -0.2], -35.0, [1.0; 3]),
        ),
    ]);
    harness
}

fn view() -> RendererCompositionCamera {
    camera([0.4, 2.2, 4.0], 0.0, -22.0)
}

/// Per-pixel luminance differences `with - without`, in 8-bit units.
fn differences(without: &[u8], with: &[u8]) -> Vec<i32> {
    without
        .as_chunks::<4>()
        .0
        .iter()
        .zip(with.as_chunks::<4>().0)
        .map(|(a, b)| {
            let luminance = |p: &[u8; 4]| i32::from(p[0]) + i32::from(p[1]) + i32::from(p[2]);
            (luminance(b) - luminance(a)) / 3
        })
        .collect()
}

fn render_with(harness: &mut Harness, options: RendererOptions) -> Vec<u8> {
    harness.renderer.set_options(options);
    harness.render(&view()).1
}

#[test]
fn both_paths_darken_the_corners_and_agree_with_each_other() {
    let mut harness = corner_scene();
    let off = render_with(&mut harness, options(AmbientOcclusionPath::Off, 1.0));
    let compute = render_with(&mut harness, options(AmbientOcclusionPath::Compute, 1.0));
    let compute_readout = harness.renderer.gpu_readout();
    let raster = render_with(&mut harness, options(AmbientOcclusionPath::Raster, 1.0));
    let raster_readout = harness.renderer.gpu_readout();
    assert_eq!(
        raster_readout.ambient_occlusion,
        AmbientOcclusionPath::Raster
    );
    assert_eq!(
        raster_readout.workgroups, 0,
        "the raster path dispatches nothing"
    );
    match &compute_readout.compute_refused {
        None => {
            assert_eq!(
                compute_readout.ambient_occlusion,
                AmbientOcclusionPath::Compute
            );
            assert_eq!(
                compute_readout.workgroups,
                (WIDTH / 2).div_ceil(16) * (HEIGHT / 2).div_ceil(16),
                "one 16×16 workgroup per half-resolution tile"
            );
        }
        Some(reason) => {
            eprintln!("compute path refused on this adapter: {reason}");
            assert_eq!(
                compute_readout.ambient_occlusion,
                AmbientOcclusionPath::Raster,
                "a refused compute path falls back to the raster path"
            );
        }
    }
    assert_eq!(
        compute_readout.occlusion_texture,
        (WIDTH / 2, HEIGHT / 2),
        "half the target"
    );

    for (name, with) in [("compute", &compute), ("raster", &raster)] {
        let deltas = differences(&off, with);
        let pixels = deltas.len() as f64;
        let darkened = deltas.iter().filter(|delta| **delta < -8).count() as f64 / pixels;
        let brightened = deltas.iter().filter(|delta| **delta > 4).count() as f64 / pixels;
        let mean = deltas.iter().map(|delta| f64::from(*delta)).sum::<f64>() / pixels;
        assert!(
            darkened > 0.02,
            "{name}: {:.2}% of pixels darkened; corners should show",
            darkened * 100.0
        );
        assert!(
            brightened < 0.001,
            "{name}: {:.3}% of pixels brightened; occlusion only darkens",
            brightened * 100.0
        );
        assert!(
            mean > -30.0,
            "{name}: mean change {mean:.1}; open surfaces should keep their light"
        );
        eprintln!(
            "{name}: {:.1}% of pixels darkened by more than 8, mean change {mean:.2}",
            darkened * 100.0
        );
    }

    // The same occlusion from either path, within the screenshot tolerance.
    let disagreeing = compute
        .as_chunks::<4>()
        .0
        .iter()
        .zip(raster.as_chunks::<4>().0)
        .filter(|(a, b)| a.iter().zip(b.iter()).any(|(a, b)| a.abs_diff(*b) > 12))
        .count() as f64
        / f64::from(WIDTH * HEIGHT);
    assert!(
        disagreeing < 0.002,
        "{:.2}% of pixels differ between the compute and raster paths",
        disagreeing * 100.0
    );
    assert_screenshot("ambient-occlusion-off", &off);
    assert_screenshot("ambient-occlusion", &compute);
}

#[test]
fn strength_scales_the_occlusion_and_zero_is_off() {
    let mut harness = corner_scene();
    let off = render_with(&mut harness, options(AmbientOcclusionPath::Off, 1.0));
    let zero = render_with(&mut harness, options(AmbientOcclusionPath::Raster, 0.0));
    assert_eq!(off, zero, "strength 0 draws exactly without occlusion");
    assert_eq!(
        harness.renderer.gpu_readout().ambient_occlusion,
        AmbientOcclusionPath::Off,
        "strength 0 runs no occlusion passes"
    );
    let half = render_with(&mut harness, options(AmbientOcclusionPath::Raster, 0.5));
    let full = render_with(&mut harness, options(AmbientOcclusionPath::Raster, 1.0));
    let sum = |with: &[u8]| -> i64 {
        differences(&off, with)
            .iter()
            .map(|delta| i64::from(*delta))
            .sum()
    };
    let (half_sum, full_sum) = (sum(&half), sum(&full));
    assert!(
        full_sum < half_sum && half_sum < 0,
        "{half_sum} vs {full_sum}"
    );
    let ratio = half_sum as f64 / full_sum as f64;
    assert!(
        (0.35..0.65).contains(&ratio),
        "half strength darkens about half as much: {half_sum} vs {full_sum}"
    );
}

#[test]
fn the_passes_are_timed_where_the_device_has_timestamp_queries() {
    let mut harness = corner_scene();
    for path in [AmbientOcclusionPath::Compute, AmbientOcclusionPath::Raster] {
        harness.renderer.set_options(options(path, 1.0));
        // Stamps read back a frame or more after they are written.
        for _ in 0..30 {
            harness.render(&view());
        }
        let readout = harness.renderer.gpu_readout();
        let adapter = harness.gpu.adapter_summary();
        // Evidence for the candidate's Den note.
        eprintln!(
            "{} ({}): {:?} path, timestamps {}, {:?}, limits {:?}",
            adapter.name,
            adapter.backend,
            readout.ambient_occlusion,
            readout.timestamps,
            readout.passes,
            readout.limits
        );
        assert_eq!(readout.passes.len(), 3, "pre-pass, occlusion, blur");
        for pass in &readout.passes {
            if readout.timestamps {
                assert!(pass.timed_frames > 0, "{}: no timed frame", pass.pass);
                assert!(
                    pass.median_gpu_ms.is_finite() && pass.median_gpu_ms >= 0.0,
                    "{}: {}",
                    pass.pass,
                    pass.median_gpu_ms
                );
            } else {
                assert_eq!(
                    pass.timed_frames, 0,
                    "{}: untimed without queries",
                    pass.pass
                );
            }
        }
    }
}
