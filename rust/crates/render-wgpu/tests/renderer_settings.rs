//! `RenderDiff::SetRendererSettings` realized by the renderer: the options it
//! changes, the sample count hosts size their targets by, and the readout a
//! product gets back.

mod support;

use render_model::{
    AmbientOcclusionMode, AmbientOcclusionSettings, RenderDiff, RendererSettingsDescriptor,
};
use render_wgpu::{OffscreenTarget, RendererOptions, SettingRefusal};
use support::{camera, Harness, HEIGHT, WIDTH};

fn settings() -> RendererSettingsDescriptor {
    RendererSettingsDescriptor {
        shadows: true,
        shadow_budget: Some(2),
        ambient_occlusion: AmbientOcclusionSettings {
            mode: AmbientOcclusionMode::ScreenSpace,
            strength: 0.5,
            radius: 2.0,
        },
        antialiasing: 1,
        vsync: false,
        clustered_lighting: false,
        gpu_culling: false,
    }
}

#[test]
fn settings_change_the_options_the_renderer_draws_with() {
    let mut harness = Harness::new(RendererOptions::default());
    assert_eq!(
        harness.renderer.samples(),
        4,
        "the default primary sample count"
    );
    assert_eq!(harness.renderer.shadow_report().budget, None);

    harness.apply(vec![RenderDiff::SetRendererSettings {
        settings: settings(),
    }]);
    let options = harness.renderer.options();
    assert!(options.shadows);
    assert_eq!(options.shadow_budget, Some(2));
    assert_eq!(options.ambient_occlusion.strength, 0.5);
    assert_eq!(options.ambient_occlusion.radius, 2.0);
    assert_eq!(harness.renderer.samples(), 1);
    assert!(!harness.renderer.vsync());
    assert_eq!(harness.renderer.shadow_report().budget, Some(2));

    let readout = harness.renderer.settings_readout();
    assert_eq!(readout.requested, settings());
    assert_eq!(
        readout.effective,
        settings(),
        "nothing asked for was refused"
    );
    assert_eq!(readout.antialiasing, None);
    assert_eq!(readout.ambient_occlusion, None);

    // A sample count no adapter multisamples at is refused back to 4; only
    // a host tool can ask for one, since the model admits 1, 2 and 4.
    harness.renderer.set_options(RendererOptions {
        samples: 3,
        ..harness.renderer.options()
    });
    assert_eq!(harness.renderer.samples(), 4);
    assert_eq!(
        harness.renderer.settings_readout().antialiasing,
        Some(SettingRefusal::UnsupportedSampleCount)
    );
}

#[test]
fn invalid_settings_are_refused_by_the_model() {
    let mut invalid = settings();
    invalid.ambient_occlusion.radius = 0.0;
    assert!(RenderDiff::SetRendererSettings { settings: invalid }
        .validate()
        .is_err());
    let mut invalid = settings();
    invalid.antialiasing = 3;
    assert!(RenderDiff::SetRendererSettings { settings: invalid }
        .validate()
        .is_err());
    assert!(RenderDiff::SetRendererSettings {
        settings: settings()
    }
    .validate()
    .is_ok());
}

#[test]
fn a_single_sample_primary_target_draws_the_same_background() {
    let mut harness = Harness::new(RendererOptions::default());
    harness.apply(vec![RenderDiff::SetBackgroundColor {
        color: [0.2, 0.4, 0.6, 1.0],
    }]);
    let view = camera([0.0, 1.0, 4.0], 0.0, 0.0);
    let (_, multisampled) = harness.render(&view);
    harness.target = OffscreenTarget::new(&harness.gpu, WIDTH, HEIGHT, 1);
    let (_, single) = harness.render(&view);
    let center = ((HEIGHT / 2) * WIDTH + WIDTH / 2) as usize * 4;
    assert_eq!(
        &multisampled[center..center + 3],
        &single[center..center + 3]
    );
    assert!(
        single[center + 2] > single[center],
        "the background is blue"
    );
}
