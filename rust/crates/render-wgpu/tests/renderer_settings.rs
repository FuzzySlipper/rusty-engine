//! `RenderDiff::SetRendererSettings` realized by the renderer: the options it
//! changes, the sample count hosts size their targets by, and the readout a
//! product gets back.

mod support;

use render_model::{
    AmbientOcclusionMode, AmbientOcclusionSettings, RenderDiff, RendererSettingsDescriptor,
    RendererSettingsOverrides,
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
        render_scale: 1.0,
        vsync: false,
        clustered_lighting: false,
        gpu_culling: false,
        volumetric_fog: render_model::VolumetricFogQuality::Off,
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
fn the_players_choices_hold_over_every_product_request() {
    let mut harness = Harness::new(RendererOptions::default());
    let mut player = RendererSettingsOverrides::default();
    player
        .choose("antialiasing", &serde_json::json!("off"))
        .unwrap();
    player
        .choose("ambientOcclusion", &serde_json::json!("disabled"))
        .unwrap();
    harness.renderer.set_player_settings(player);
    assert_eq!(harness.renderer.samples(), 1, "the choice applies at once");

    // The product asks for 4x and screen-space occlusion: the player's
    // choices stay, the rest of the request is taken.
    harness.apply(vec![RenderDiff::SetRendererSettings {
        settings: RendererSettingsDescriptor {
            antialiasing: 4,
            ..settings()
        },
    }]);
    let readout = harness.renderer.settings_readout();
    assert_eq!(readout.product.antialiasing, 4);
    assert_eq!(readout.requested.antialiasing, 1);
    assert_eq!(
        readout.requested.ambient_occlusion.mode,
        AmbientOcclusionMode::Disabled
    );
    assert_eq!(
        readout.requested.shadow_budget,
        Some(2),
        "not chosen: the product's"
    );
    assert_eq!(readout.player, player);
    assert_eq!(harness.renderer.samples(), 1);

    // Forgetting the choices restores the product's request.
    harness
        .renderer
        .set_player_settings(RendererSettingsOverrides::default());
    let readout = harness.renderer.settings_readout();
    assert_eq!(readout.requested, readout.product);
    assert_eq!(harness.renderer.samples(), 4);
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
fn every_admitted_sample_count_draws_or_is_refused_to_four() {
    let mut harness = Harness::new(RendererOptions::default());
    harness.apply(vec![RenderDiff::SetBackgroundColor {
        color: [0.2, 0.4, 0.6, 1.0],
    }]);
    let view = camera([0.0, 1.0, 4.0], 0.0, 0.0);
    for count in RendererSettingsDescriptor::SAMPLE_COUNTS {
        harness.apply(vec![RenderDiff::SetRendererSettings {
            settings: RendererSettingsDescriptor {
                antialiasing: count,
                ..RendererSettingsDescriptor::DEFAULT
            },
        }]);
        // What the host sizes its target by never fails to create or draw.
        let samples = harness.renderer.samples();
        harness.target = OffscreenTarget::new(&harness.gpu, WIDTH, HEIGHT, samples);
        let (_, pixels) = harness.render(&view);
        let center = ((HEIGHT / 2) * WIDTH + WIDTH / 2) as usize * 4;
        assert!(pixels[center + 2] > pixels[center], "{count} samples drew");
        let readout = harness.renderer.settings_readout();
        assert_eq!(readout.effective.antialiasing, samples);
        assert_eq!(
            readout.antialiasing.is_some(),
            samples != count,
            "{count} samples: refused only when the device draws another count"
        );
    }
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

#[test]
fn a_display_without_an_immediate_mode_keeps_vsync_on() {
    let mut harness = Harness::new(RendererOptions::default());
    harness.apply(vec![RenderDiff::SetRendererSettings {
        settings: settings(),
    }]);
    assert!(!harness.renderer.vsync(), "the request turns vsync off");
    assert_eq!(
        harness.renderer.settings_readout().vsync,
        None,
        "no display seen yet, nothing to refuse"
    );

    // wgpu resolves AutoNoVsync through Immediate and Mailbox to Fifo, so
    // a display with Fifo alone waits for its refresh whatever was asked.
    harness
        .renderer
        .set_display_present_modes(&[wgpu::PresentMode::Fifo]);
    let readout = harness.renderer.settings_readout();
    assert!(readout.effective.vsync, "frames wait for the display");
    assert_eq!(readout.vsync, Some(SettingRefusal::VsyncOnly));
    assert!(!readout.requested.vsync, "the request is reported as made");

    harness
        .renderer
        .set_display_present_modes(&[wgpu::PresentMode::Fifo, wgpu::PresentMode::Mailbox]);
    let readout = harness.renderer.settings_readout();
    assert!(!readout.effective.vsync);
    assert_eq!(readout.vsync, None);

    let mut on = settings();
    on.vsync = true;
    harness.apply(vec![RenderDiff::SetRendererSettings { settings: on }]);
    harness
        .renderer
        .set_display_present_modes(&[wgpu::PresentMode::Fifo]);
    assert_eq!(
        harness.renderer.settings_readout().vsync,
        None,
        "vsync asked for is what the display does"
    );
}
