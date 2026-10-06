//! Render scale: below 1, the primary passes draw into an internal target at
//! that fraction of the output size and are upscaled bilinearly into the
//! output, so the picture is the full-size picture resampled, the screen-space
//! passes shrink with it, and scale 1 draws directly again. Needs a wgpu
//! adapter: CI uses `WGPU_BACKEND=vulkan` with llvmpipe.

mod support;

use render_host_contracts::RendererCompositionCamera;
use render_model::*;
use render_wgpu::{FrameStats, RendererOptions};
use support::*;

const MATERIAL: &str = "material/scale-plaster";
const CRATE: &str = "static-mesh/scale-crate";
const FLOOR: &str = "static-mesh/scale-floor";

fn scene() -> Harness {
    let mut harness = Harness::new(RendererOptions::default());
    let mut ops = vec![
        RenderDiff::DefineMaterial {
            material: material(MATERIAL, [0.6, 0.55, 0.5, 1.0], None),
        },
        static_mesh(
            FLOOR,
            box_mesh([-8.0, -0.2, -8.0], [8.0, 0.0, 8.0], |_| 0),
            MATERIAL,
        ),
        static_mesh(
            CRATE,
            box_mesh([-0.5, 0.0, -0.5], [0.5, 1.0, 0.5], |_| 0),
            MATERIAL,
        ),
        instance(10, None, FLOOR, transform([0.0; 3], 0.0, [1.0; 3])),
        RenderDiff::SetBackgroundColor {
            color: [0.2, 0.3, 0.5, 1.0],
        },
    ];
    for index in 0..6_u64 {
        ops.push(instance(
            100 + index,
            None,
            CRATE,
            transform(
                [index as f32 * 1.6 - 4.0, 0.0, -2.0 - (index % 2) as f32],
                (index * 25) as f32,
                [1.0; 3],
            ),
        ));
    }
    harness.apply(ops);
    harness
}

fn view() -> RendererCompositionCamera {
    camera([0.0, 2.0, 5.0], 0.0, -15.0)
}

fn settings(render_scale: f32) -> RendererSettingsDescriptor {
    RendererSettingsDescriptor {
        render_scale,
        ambient_occlusion: AmbientOcclusionSettings {
            mode: AmbientOcclusionMode::ScreenSpace,
            ..AmbientOcclusionSettings::DEFAULT
        },
        ..RendererSettingsDescriptor::DEFAULT
    }
}

fn render_at(harness: &mut Harness, render_scale: f32) -> (FrameStats, Vec<u8>) {
    harness.apply(vec![RenderDiff::SetRendererSettings {
        settings: settings(render_scale),
    }]);
    harness.render(&view())
}

/// Box-filter `rgba` (WIDTH × HEIGHT) down by `factor`.
fn downsample(rgba: &[u8], factor: u32) -> Vec<f64> {
    let (width, height) = (WIDTH / factor, HEIGHT / factor);
    let mut out = Vec::with_capacity((width * height * 3) as usize);
    for y in 0..height {
        for x in 0..width {
            for channel in 0..3 {
                let mut sum = 0.0;
                for dy in 0..factor {
                    for dx in 0..factor {
                        let index = (((y * factor + dy) * WIDTH + x * factor + dx) * 4) as usize;
                        sum += f64::from(rgba[index + channel as usize]);
                    }
                }
                out.push(sum / f64::from(factor * factor));
            }
        }
    }
    out
}

#[test]
fn a_half_scale_frame_is_the_full_frame_resampled() {
    let mut harness = scene();
    let (full_stats, full) = render_at(&mut harness, 1.0);
    let full_occlusion = harness.renderer.gpu_readout().ambient_occlusion.texture;
    assert_eq!(
        full_occlusion,
        (WIDTH / 2, HEIGHT / 2),
        "half-resolution occlusion of the output"
    );

    let (half_stats, half) = render_at(&mut harness, 0.5);
    let readout = harness.renderer.settings_readout();
    assert_eq!(readout.effective.render_scale, 0.5);
    assert_eq!(half.len(), full.len(), "the output keeps its size");
    // The screen-space passes draw at the scaled size.
    assert_eq!(
        harness.renderer.gpu_readout().ambient_occlusion.texture,
        (WIDTH / 4, HEIGHT / 4)
    );
    assert_eq!(half_stats.draws, full_stats.draws, "the same draws, scaled");
    let (coarse_full, coarse_half) = (downsample(&full, 4), downsample(&half, 4));
    let mean = coarse_full
        .iter()
        .zip(&coarse_half)
        .map(|(a, b)| (a - b).abs())
        .sum::<f64>()
        / coarse_full.len() as f64;
    assert!(
        mean < 6.0,
        "mean channel difference {mean:.2} between the full and the upscaled half-scale frame"
    );
    assert_ne!(
        full, half,
        "the half-scale frame is resampled, not the same frame"
    );

    // Back at scale 1 the renderer draws into the output directly again.
    let (_, again) = render_at(&mut harness, 1.0);
    assert_eq!(again, full);
    assert_eq!(
        harness.renderer.gpu_readout().ambient_occlusion.texture,
        (WIDTH / 2, HEIGHT / 2)
    );
}

#[test]
fn the_scale_must_be_within_half_and_one() {
    for (scale, ok) in [
        (0.25, false),
        (0.5, true),
        (0.75, true),
        (1.0, true),
        (1.5, false),
    ] {
        assert_eq!(
            RenderDiff::SetRendererSettings {
                settings: settings(scale),
            }
            .validate()
            .is_ok(),
            ok,
            "scale {scale}"
        );
    }
    assert!(RenderDiff::SetRendererSettings {
        settings: settings(f32::NAN),
    }
    .validate()
    .is_err());
}
