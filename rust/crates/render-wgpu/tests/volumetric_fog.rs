//! Volumetric fog (`volumetric_fog.wgsl`, `finish_pass.wgsl`): off, or on
//! with nothing to light, draws exactly as without it; a medium hazes the
//! view and the sky; fog in the shadow of a wall scatters less of the sun
//! than fog in the open (light shafts); a glowing fog volume brightens where
//! it stands and goes when removed; the pass is timed and reported. Same
//! harness as `tests/screenshots.rs`.

mod support;

use render_model::*;
use render_wgpu::RendererOptions;
use support::*;

fn options(quality: VolumetricFogQuality) -> RendererOptions {
    RendererOptions {
        default_world_lights: false,
        shadows: true,
        volumetric_fog: quality,
        ..RendererOptions::default()
    }
}

/// A grey ground under a low sun from ahead of the camera, which casts, a
/// dim fill, and a dark background.
fn scene(quality: VolumetricFogQuality) -> Harness {
    let mut harness = Harness::new(options(quality));
    harness.apply(vec![
        RenderDiff::SetBackgroundColor {
            color: [0.02, 0.03, 0.05, 1.0],
        },
        RenderDiff::DefineMaterial {
            material: material("material/ground", [0.4, 0.4, 0.4, 1.0], None),
        },
        static_mesh(
            "mesh/ground",
            box_mesh([-60.0, -1.0, -60.0], [60.0, 0.0, 60.0], |_| 0),
            "material/ground",
        ),
        instance(20, None, "mesh/ground", transform([0.0; 3], 0.0, [1.0; 3])),
        RenderDiff::CreateLight {
            handle: RenderHandle::new(10),
            parent: None,
            light: LightDescriptor::Directional {
                color: [1.0, 0.95, 0.85],
                intensity: 3.0,
                enabled: true,
                // Ahead of the camera and low, so the camera looks into the
                // light through the fog.
                direction: [0.0, -0.35, 1.0],
                range: Some(80.0),
                shadow_intent: LightShadowIntent::Requested,
                shadow: Default::default(),
            },
        },
        RenderDiff::CreateLight {
            handle: RenderHandle::new(11),
            parent: None,
            light: LightDescriptor::Ambient {
                color: [0.4, 0.45, 0.55],
                intensity: 0.2,
                enabled: true,
                range: None,
                shadow_intent: LightShadowIntent::Disabled,
                shadow: Default::default(),
            },
        },
    ]);
    harness
}

fn medium(density: f32) -> RenderDiff {
    RenderDiff::SetVolumetricFog {
        fog: VolumetricFogDescriptor {
            density,
            albedo: [0.9, 0.9, 0.9],
            anisotropy: 0.6,
            base_height: 0.0,
            falloff_height: 0.0,
            distance: 60.0,
            ambient: 1.0,
        },
    }
}

/// From head height looking toward the low sun (yaw 0 faces -Z, where it
/// shines from), slightly down.
fn look(harness: &mut Harness) -> Vec<u8> {
    harness.render(&camera([0.0, 1.7, 0.0], 0.0, -4.0)).1
}

/// Writes `rgba` under `RUSTY_FOG_TEST_IMAGES` when it is set, to look at.
fn keep(name: &str, rgba: &[u8]) {
    if let Some(directory) = std::env::var_os("RUSTY_FOG_TEST_IMAGES") {
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
fn off_or_with_nothing_to_light_draws_exactly_as_without_it() {
    let mut plain = scene(VolumetricFogQuality::Off);
    let bare = look(&mut plain);
    plain.apply(vec![medium(0.05)]);
    assert_eq!(look(&mut plain), bare, "the setting off ignores the medium");

    let mut empty = scene(VolumetricFogQuality::Low);
    assert_eq!(
        look(&mut empty),
        bare,
        "no medium and no volumes draw no fog"
    );
    assert_eq!(
        empty.renderer.gpu_readout().volumetric_fog.grid,
        None,
        "nothing was lit"
    );
}

#[test]
fn a_medium_hazes_the_ground_and_the_sky_toward_the_sun() {
    let mut harness = scene(VolumetricFogQuality::Low);
    let bare = look(&mut harness);
    harness.apply(vec![medium(0.04)]);
    let fogged = look(&mut harness);
    let readout = harness.renderer.gpu_readout();
    assert_eq!(readout.volumetric_fog.grid, Some([96, 54, 48]));
    // The background above the horizon, looking into the sun, glows.
    let sky = (0..40, 100..220);
    assert!(
        luminance(&fogged, sky.0.clone(), sky.1.clone()) > luminance(&bare, sky.0, sky.1) + 4.0,
        "the sky behind the fog glows toward the sun"
    );
    assert!(
        readout
            .passes
            .iter()
            .any(|pass| pass.pass == "volumetric-fog"),
        "the pass reports its cost"
    );
}

#[test]
fn fog_in_a_walls_shadow_scatters_less_of_the_sun_than_fog_in_the_open() {
    // A tall wall across the view 30 m ahead, between the low sun and the
    // ground in front of it: everything between the camera and the wall is
    // in its shadow. What the fog adds to that ground, lit or shaded, is the
    // fog's own light.
    let wall = vec![
        RenderDiff::DefineMaterial {
            material: material("material/wall", [0.3, 0.3, 0.3, 1.0], None),
        },
        static_mesh(
            "mesh/wall",
            box_mesh([-60.0, 0.0, -31.0], [60.0, 40.0, -30.0], |_| 0),
            "material/wall",
        ),
        instance(30, None, "mesh/wall", transform([0.0; 3], 0.0, [1.0; 3])),
    ];
    let added = |walled: bool| {
        let mut harness = scene(VolumetricFogQuality::Low);
        if walled {
            harness.apply(wall.clone());
        }
        let clear = look(&mut harness);
        harness.apply(vec![medium(0.03)]);
        let fogged = look(&mut harness);
        keep(if walled { "walled" } else { "open" }, &fogged);
        // The near ground, below the horizon.
        luminance(&fogged, 120..180, 0..WIDTH) - luminance(&clear, 120..180, 0..WIDTH)
    };
    let (open, walled) = (added(false), added(true));
    assert!(
        open > walled * 2.0 && open > walled + 2.0,
        "fog in the sun adds more light than fog in the wall's shadow: open {open:.2}, walled {walled:.2}"
    );
}

#[test]
fn a_glowing_fog_volume_brightens_where_it_stands_and_goes_when_removed() {
    let mut harness = scene(VolumetricFogQuality::Low);
    let bare = look(&mut harness);
    harness.apply(vec![RenderDiff::SetFogVolume {
        id: 1,
        volume: FogVolumeDescriptor {
            shape: FogVolumeShape::Ellipsoid,
            center: [0.0, 1.5, -8.0],
            half_extents: [3.0, 2.0, 3.0],
            yaw_degrees: 0.0,
            density: 0.6,
            albedo: [0.8, 0.8, 0.8],
            emission: [2.0, 0.4, 1.2],
            edge: 0.3,
            noise_scale: 0.0,
            noise_strength: 0.0,
            noise_velocity: [0.0; 3],
        },
    }]);
    let glowing = look(&mut harness);
    keep("bare", &bare);
    keep("glowing", &glowing);
    let middle = (60..120, 120..200);
    assert!(
        luminance(&glowing, middle.0.clone(), middle.1.clone())
            > luminance(&bare, middle.0.clone(), middle.1.clone()) + 6.0,
        "the volume glows in the middle of the view"
    );
    assert_eq!(harness.renderer.gpu_readout().volumetric_fog.volumes, 1);
    harness.apply(vec![RenderDiff::RemoveFogVolume { id: 1 }]);
    assert_eq!(look(&mut harness), bare, "removed, nothing remains");
}

#[test]
fn invalid_fog_is_refused_by_the_model() {
    let mut fog = VolumetricFogDescriptor::DEFAULT;
    fog.distance = 2.0;
    assert!(RenderDiff::SetVolumetricFog { fog }.validate().is_err());
    let volume = FogVolumeDescriptor {
        shape: FogVolumeShape::Box,
        center: [0.0; 3],
        half_extents: [0.0, 1.0, 1.0],
        yaw_degrees: 0.0,
        density: 0.5,
        albedo: [1.0; 3],
        emission: [0.0; 3],
        edge: 0.2,
        noise_scale: 0.0,
        noise_strength: 0.0,
        noise_velocity: [0.0; 3],
    };
    assert!(RenderDiff::SetFogVolume { id: 1, volume }
        .validate()
        .is_err());
}
