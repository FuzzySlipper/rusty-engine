//! Ray-traced ambient occlusion (`AmbientOcclusionPath::RayTraced`,
//! `ray_shadows.rs`): short rays darken where surfaces meet, see an occluder
//! just outside the view that the screen-space path cannot, pass through a
//! masked material's clear texels, and leave open ground alone. A device
//! that cannot trace takes the screen-space path and says why.

mod support;

use render_model::*;
use render_wgpu::{AmbientOcclusion, AmbientOcclusionPath, Gpu, RendererOptions, SettingRefusal};
use support::*;

const GROUND: u64 = 1;
const BOX: u64 = 2;
const CARD: u64 = 3;
const SKY: u64 = 90;
/// Rows at the image's bottom: the ground beside the occluder below the view.
const BOTTOM_ROWS: u32 = 16;

fn options(path: AmbientOcclusionPath) -> RendererOptions {
    RendererOptions {
        default_world_lights: false,
        ambient_occlusion: AmbientOcclusion {
            path,
            strength: 1.0,
            ..AmbientOcclusion::default()
        },
        ..RendererOptions::default()
    }
}

/// Ground under an even sky: only occlusion shades it.
fn ground() -> Vec<RenderDiff> {
    vec![
        RenderDiff::DefineMaterial {
            material: material("material/ground", [0.7, 0.7, 0.7, 1.0], None),
        },
        static_mesh(
            "mesh/ground",
            box_mesh([-20.0, -0.2, -30.0], [20.0, 0.0, 10.0], |_| 0),
            "material/ground",
        ),
        instance(
            GROUND,
            None,
            "mesh/ground",
            transform([0.0; 3], 0.0, [1.0; 3]),
        ),
        RenderDiff::CreateLight {
            handle: RenderHandle::new(SKY),
            parent: None,
            light: LightDescriptor::Ambient {
                color: [1.0; 3],
                intensity: 1.0,
                enabled: true,
                shadow_intent: LightShadowIntent::Disabled,
                shadow: Default::default(),
                range: None,
            },
        },
    ]
}

/// A box from `min` to `max`.
fn solid(min: [f32; 3], max: [f32; 3]) -> Vec<RenderDiff> {
    vec![
        static_mesh("mesh/box", box_mesh(min, max, |_| 0), "material/ground"),
        instance(BOX, None, "mesh/box", transform([0.0; 3], 0.0, [1.0; 3])),
    ]
}

/// A masked wall 4 m ahead whose texture's alpha is `alpha`.
fn card(harness: &mut Harness, alpha: u8) -> Vec<RenderDiff> {
    let texture = harness.resources.texture(
        "texture/card",
        2,
        2,
        &[255, 255, 255, alpha].repeat(4),
        TextureWrap::Clamp,
    );
    vec![
        RenderDiff::DefineTexture { texture },
        RenderDiff::DefineMaterial {
            material: RenderMaterialDescriptor {
                alpha_mode: MaterialAlphaModeDescriptor::Mask { cutoff: 0.5 },
                ..material("material/card", [0.7, 0.7, 0.7, 1.0], Some("texture/card"))
            },
        },
        static_mesh(
            "mesh/card",
            box_mesh([-2.0, 0.0, -4.1], [2.0, 1.5, -4.0], |_| 0),
            "material/card",
        ),
        instance(CARD, None, "mesh/card", transform([0.0; 3], 0.0, [1.0; 3])),
    ]
}

fn luma(rgba: &[u8]) -> Vec<f32> {
    rgba.as_chunks::<4>()
        .0
        .iter()
        .map(|pixel| pixel[..3].iter().map(|&c| f32::from(c)).sum::<f32>() / 3.0)
        .collect()
}

/// How much darker `shaded` is than `open`, on average over `rows`.
fn darkening(open: &[f32], shaded: &[f32], rows: std::ops::Range<u32>) -> f32 {
    let range = (rows.start * WIDTH) as usize..(rows.end * WIDTH) as usize;
    let sum: f32 = open[range.clone()]
        .iter()
        .zip(&shaded[range.clone()])
        .map(|(open, shaded)| open - shaded)
        .sum();
    sum / range.len() as f32
}

#[test]
fn rays_darken_what_meets_on_screen_or_off_and_pass_clear_texels() {
    let gpu = Gpu::headless_with_ray_queries().expect("a device");
    let draw = |path, scene: &dyn Fn(&mut Harness) -> Vec<RenderDiff>, view: &_| {
        let mut harness = Harness::on(gpu.clone(), options(path));
        let ops = scene(&mut harness);
        harness.apply(ops);
        harness.render(view);
        let picture = luma(&harness.render(view).1);
        (harness, picture)
    };
    let (harness, _) = draw(
        AmbientOcclusionPath::RayTraced,
        &|_| ground(),
        &camera([0.0, 2.0, 0.0], 0.0, -45.0),
    );
    let readout = harness.renderer.settings_readout();
    let path = harness.renderer.gpu_readout().ambient_occlusion.path;
    drop(harness);
    if gpu.is_software() {
        assert_eq!(
            readout.ambient_occlusion,
            Some(SettingRefusal::SoftwareAdapter)
        );
        assert_eq!(
            readout.effective.ambient_occlusion.mode,
            AmbientOcclusionMode::ScreenSpace
        );
        assert_eq!(path, AmbientOcclusionPath::Raster);
        return;
    }
    assert_eq!(readout.ambient_occlusion, None);
    assert_eq!(path, AmbientOcclusionPath::RayTraced);

    // Open ground takes nothing.
    let down = camera([0.0, 2.0, 0.0], 0.0, -45.0);
    let (_, open) = draw(AmbientOcclusionPath::Off, &|_| ground(), &down);
    let (_, plain) = draw(AmbientOcclusionPath::RayTraced, &|_| ground(), &down);
    let flat = darkening(&open, &plain, 0..HEIGHT);
    assert!(flat.abs() < 0.5, "open ground darkens by {flat}");

    // A wall's face, far from the ground, sees open air.
    let tall = |_: &mut Harness| [ground(), solid([-8.0, 0.0, -4.1], [8.0, 7.0, -4.0])].concat();
    let facing = camera([0.0, 4.0, 0.0], 0.0, 0.0);
    let (_, without) = draw(AmbientOcclusionPath::Off, &tall, &facing);
    let (_, with) = draw(AmbientOcclusionPath::RayTraced, &tall, &facing);
    let face = darkening(&without, &with, 0..HEIGHT / 2);
    assert!(face.abs() < 0.5, "an open wall darkens by {face}");

    // A box's foot darkens.
    let foot = |_: &mut Harness| [ground(), solid([-1.0, 0.0, -3.5], [1.0, 1.5, -2.5])].concat();
    let (_, without) = draw(AmbientOcclusionPath::Off, &foot, &down);
    let (_, with) = draw(AmbientOcclusionPath::RayTraced, &foot, &down);
    let contact = darkening(&without, &with, 0..HEIGHT);
    assert!(contact > 0.3, "the foot darkens by {contact}");

    // A box just below the view's bottom edge: the ground beside it darkens
    // under rays; the screen-space pass cannot see it.
    let below = |_: &mut Harness| [ground(), solid([-4.0, 0.0, -0.2], [4.0, 0.5, 1.0])].concat();
    let rows = HEIGHT - BOTTOM_ROWS..HEIGHT;
    let (_, without) = draw(AmbientOcclusionPath::Off, &below, &down);
    let (_, rays) = draw(AmbientOcclusionPath::RayTraced, &below, &down);
    let (_, screen) = draw(AmbientOcclusionPath::Raster, &below, &down);
    let (traced, seen) = (
        darkening(&without, &rays, rows.clone()),
        darkening(&without, &screen, rows),
    );
    assert!(
        traced > 1.0 && traced > seen * 3.0,
        "the ground beside what is off screen: rays {traced}, screen space {seen}"
    );

    // A clear masked wall occludes nothing; an opaque one does.
    let wall = |alpha| move |harness: &mut Harness| [ground(), card(harness, alpha)].concat();
    let ahead = camera([0.0, 2.0, 0.0], 0.0, -30.0);
    let (_, open) = draw(AmbientOcclusionPath::RayTraced, &|_| ground(), &ahead);
    let (_, clear) = draw(AmbientOcclusionPath::RayTraced, &wall(0), &ahead);
    let (_, solid_wall) = draw(AmbientOcclusionPath::Off, &wall(255), &ahead);
    let (_, shaded_wall) = draw(AmbientOcclusionPath::RayTraced, &wall(255), &ahead);
    let through = darkening(&open, &clear, 0..HEIGHT);
    let cast = darkening(&solid_wall, &shaded_wall, 0..HEIGHT);
    assert!(through.abs() < 0.5, "a clear wall darkens by {through}");
    assert!(cast > 0.3, "an opaque wall darkens by {cast}");
    refused_without_ray_queries();
}

/// A device made without ray queries takes the screen-space path and says
/// why. In the test above, so the binary's devices are made one at a time.
fn refused_without_ray_queries() {
    let gpu = Gpu::headless().expect("a device");
    let mut harness = Harness::on(gpu, options(AmbientOcclusionPath::RayTraced));
    harness.apply(ground());
    harness.render(&camera([0.0, 2.0, 0.0], 0.0, -45.0));
    let readout = harness.renderer.settings_readout();
    assert_eq!(
        readout.ambient_occlusion,
        Some(SettingRefusal::NoRayQueries)
    );
    assert_eq!(
        readout.effective.ambient_occlusion.mode,
        AmbientOcclusionMode::ScreenSpace
    );
    assert_eq!(
        harness.renderer.gpu_readout().ambient_occlusion.path,
        AmbientOcclusionPath::Raster
    );
}
