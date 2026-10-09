//! Cloud regions and volumetric clouds (`clouds.wgsl`, `sky.wgsl`
//! `fs_clouds_volumetric`): a region raises the sky's cloud where it stands
//! and nowhere else, is gone when removed, and shades the ground under it;
//! volumetric clouds draw where the flat layer does, but differently, and
//! off draws the flat layer exactly. Same harness as `tests/screenshots.rs`.

mod support;

use render_model::*;
use render_wgpu::RendererOptions;
use support::*;

const NOON: [f32; 3] = [0.3, 1.0, -0.4];

/// A plain blue panorama over a dark ground, a grey ground plane, and a
/// noon sun that casts on it.
fn scene(clouds: VolumetricCloudsQuality) -> Harness {
    let mut harness = Harness::new(RendererOptions {
        default_world_lights: false,
        volumetric_clouds: clouds,
        ..RendererOptions::default()
    });
    let (width, height) = (64, 32);
    let rgba: Vec<u8> = (0..width * height)
        .flat_map(|texel| {
            if texel / width < height / 2 {
                [90, 140, 220, 255]
            } else {
                [40, 40, 40, 255]
            }
        })
        .collect();
    let texture =
        harness
            .resources
            .texture("texture/sky", width, height, &rgba, TextureWrap::Clamp);
    harness.apply(vec![
        RenderDiff::DefineTexture { texture },
        RenderDiff::SetSkyBackground {
            background: Some(SkyBackgroundDescriptor {
                texture: "texture/sky".to_owned(),
                blend: None,
            }),
        },
        RenderDiff::DefineMaterial {
            material: material("material/ground", [0.5, 0.5, 0.5, 1.0], None),
        },
        static_mesh(
            "mesh/ground",
            box_mesh([-400.0, -1.0, -400.0], [400.0, 0.0, 400.0], |_| 0),
            "material/ground",
        ),
        instance(20, None, "mesh/ground", transform([0.0; 3], 0.0, [1.0; 3])),
        RenderDiff::CreateLight {
            handle: RenderHandle::new(10),
            parent: None,
            light: LightDescriptor::Directional {
                color: [1.0, 0.98, 0.92],
                intensity: 1.0,
                enabled: true,
                direction: NOON.map(|value| -value),
                range: None,
                shadow_intent: LightShadowIntent::Disabled,
                shadow: Default::default(),
            },
        },
    ]);
    harness
}

fn layer(coverage: f32) -> RenderDiff {
    RenderDiff::SetClouds {
        clouds: Some(CloudsDescriptor {
            coverage,
            drift: [0.0, 0.0],
            altitude: 1500.0,
            scale: 600.0,
            color: [1.0; 3],
        }),
    }
}

/// A storm over (x, z), `radius` metres across.
fn region(id: u32, center: [f32; 2], radius: f32) -> RenderDiff {
    RenderDiff::SetCloudRegion {
        id,
        region: CloudRegionDescriptor {
            center,
            radius,
            coverage: 1.0,
            darkness: 0.5,
            drift: [0.0, 0.0],
        },
    }
}

/// Up into the sky toward `yaw` degrees (0 faces -Z).
fn look_up(harness: &mut Harness, yaw: f64) -> Vec<u8> {
    harness.renderer.set_animation_time(2.0);
    harness.render(&camera([0.0, 1.0, 0.0], yaw, 25.0)).1
}

/// The share of pixels that changed against `bare`.
fn changed(bare: &[u8], with: &[u8]) -> f64 {
    let (pixels, changed) = bare
        .as_chunks::<4>()
        .0
        .iter()
        .zip(with.as_chunks::<4>().0)
        .fold((0, 0), |(pixels, changed), (a, b)| {
            let difference: i32 = (0..3)
                .map(|channel| (i32::from(a[channel]) - i32::from(b[channel])).abs())
                .sum();
            (pixels + 1, changed + usize::from(difference > 6))
        });
    changed as f64 / pixels as f64
}

/// Writes `rgba` under `RUSTY_CLOUD_TEST_IMAGES` when it is set, to look at.
fn keep(name: &str, rgba: &[u8]) {
    if let Some(directory) = std::env::var_os("RUSTY_CLOUD_TEST_IMAGES") {
        let png = render_wgpu::encode_png(WIDTH, HEIGHT, rgba).expect("png");
        std::fs::write(
            std::path::Path::new(&directory).join(format!("{name}.png")),
            png,
        )
        .expect("write");
    }
}

fn luminance(rgba: &[u8], rows: std::ops::Range<u32>, columns: std::ops::Range<u32>) -> f64 {
    let mut total = 0.0;
    let mut count = 0.0;
    for y in rows {
        for x in columns.clone() {
            let at = ((y * WIDTH + x) * 4) as usize;
            total += 0.2126 * f64::from(rgba[at])
                + 0.7152 * f64::from(rgba[at + 1])
                + 0.0722 * f64::from(rgba[at + 2]);
            count += 1.0;
        }
    }
    total / count
}

#[test]
fn a_region_clouds_the_sky_where_it_stands_and_goes_when_removed() {
    let mut harness = scene(VolumetricCloudsQuality::Off);
    harness.apply(vec![layer(0.0)]);
    let (ahead, behind) = (look_up(&mut harness, 0.0), look_up(&mut harness, 180.0));
    // A storm 4 km ahead (-Z), 3 km across: under no layer coverage.
    harness.apply(vec![region(1, [0.0, -4000.0], 3000.0)]);
    let (stormy, clear) = (look_up(&mut harness, 0.0), look_up(&mut harness, 180.0));
    keep("ahead", &ahead);
    keep("stormy", &stormy);
    assert!(
        changed(&ahead, &stormy) > 0.05,
        "the storm clouds the sky ahead"
    );
    assert!(changed(&behind, &clear) < 0.01, "and nowhere else");
    harness.apply(vec![RenderDiff::RemoveCloudRegion { id: 1 }]);
    assert_eq!(
        look_up(&mut harness, 0.0),
        ahead,
        "removed, the sky is clear"
    );
}

#[test]
fn a_region_with_no_layer_still_draws_and_shades_the_ground_under_it() {
    let mut harness = scene(VolumetricCloudsQuality::Off);
    // Looking down on the ground from 60 m: the region stands where the
    // sun's rays from the view's left side cross the layer, 1500 m up
    // toward the sun, so it shades that side.
    let down = camera([0.0, 60.0, 0.0], 0.0, -70.0);
    let bare = harness.render(&down).1;
    let [x, y, z] = NOON;
    let rise = 1500.0 / y;
    harness.apply(vec![region(2, [-130.0 + x * rise, z * rise], 120.0)]);
    let shaded = harness.render(&down).1;
    let left = luminance(&shaded, 40..140, 0..80) - luminance(&bare, 40..140, 0..80);
    let right = luminance(&shaded, 40..140, 240..320) - luminance(&bare, 40..140, 240..320);
    assert!(left < -5.0, "the ground under the storm darkens: {left:.2}");
    assert!(right.abs() < 1.0, "the open ground does not: {right:.2}");
}

#[test]
fn volumetric_clouds_draw_where_the_flat_layer_does_but_differently_and_off_is_flat() {
    let render = |quality| {
        let mut harness = scene(quality);
        let bare = look_up(&mut harness, 0.0);
        harness.apply(vec![layer(0.55)]);
        (bare, look_up(&mut harness, 0.0), harness)
    };
    let (bare, flat, _) = render(VolumetricCloudsQuality::Off);
    let (bare_volumetric, volumetric, harness) = render(VolumetricCloudsQuality::Low);
    assert_eq!(
        bare, bare_volumetric,
        "without clouds the setting changes nothing"
    );
    assert!(changed(&bare, &flat) > 0.1, "the flat layer clouds the sky");
    assert!(
        changed(&bare, &volumetric) > 0.1,
        "so do the volumetric clouds"
    );
    assert!(
        changed(&flat, &volumetric) > 0.05,
        "but they draw differently"
    );
    assert!(
        harness
            .renderer
            .gpu_readout()
            .passes
            .iter()
            .any(|pass| pass.pass == "clouds"),
        "the volumetric clouds are timed as the clouds pass"
    );
}

#[test]
fn invalid_regions_are_refused_by_the_model() {
    let mut region = CloudRegionDescriptor {
        center: [0.0, 0.0],
        radius: 100.0,
        coverage: 0.5,
        darkness: 0.0,
        drift: [0.0, 0.0],
    };
    assert!(RenderDiff::SetCloudRegion { id: 1, region }
        .validate()
        .is_ok());
    region.radius = 0.0;
    assert!(RenderDiff::SetCloudRegion { id: 1, region }
        .validate()
        .is_err());
    region.radius = 100.0;
    region.coverage = 1.5;
    assert!(RenderDiff::SetCloudRegion { id: 1, region }
        .validate()
        .is_err());
}
