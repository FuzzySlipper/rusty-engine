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

/// Whether the device refuses volumetric fog: a software adapter does (a
/// GPU-only feature) and draws the scene's analytic fog alone. Each test
/// then checks the refusal and that nothing volumetric draws.
fn refused_on_software(harness: &mut Harness, fog: Vec<RenderDiff>) -> bool {
    let refusal = harness.renderer.settings_readout().volumetric_fog;
    if refusal.is_none() {
        return false;
    }
    assert_eq!(refusal, Some(render_wgpu::SettingRefusal::SoftwareAdapter));
    let bare = look(harness);
    harness.apply(fog);
    assert_eq!(look(harness), bare, "refused, the fog draws nothing");
    assert_eq!(harness.renderer.gpu_readout().volumetric_fog.grid, None);
    true
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
    if refused_on_software(&mut harness, vec![medium(0.04)]) {
        return;
    }
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
    if refused_on_software(&mut scene(VolumetricFogQuality::Low), vec![medium(0.03)]) {
        return;
    }
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
    if refused_on_software(&mut harness, vec![medium(0.02)]) {
        return;
    }
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

#[test]
fn volumetric_fog_replaces_the_analytic_fog_as_far_as_it_reaches() {
    // A wall 20 m ahead on the left, inside the grid's 60 m reach, and one
    // 95 m ahead on the right, beyond it (the camera sees 100 m), under a
    // scene with both the analytic fog and a volumetric medium.
    let walls = vec![
        RenderDiff::DefineMaterial {
            material: material("material/wall", [0.6, 0.6, 0.6, 1.0], None),
        },
        static_mesh(
            "mesh/near",
            box_mesh([-60.0, 0.0, -21.0], [0.0, 30.0, -20.0], |_| 0),
            "material/wall",
        ),
        static_mesh(
            "mesh/far",
            box_mesh([0.0, 0.0, -96.0], [400.0, 200.0, -95.0], |_| 0),
            "material/wall",
        ),
        instance(30, None, "mesh/near", transform([0.0; 3], 0.0, [1.0; 3])),
        instance(31, None, "mesh/far", transform([0.0; 3], 0.0, [1.0; 3])),
    ];
    let analytic = RenderDiff::SetFog {
        // Red, to tell it from the volumetric fog's grey.
        fog: Some(FogDescriptor::Exponential {
            color: [0.9, 0.1, 0.1],
            density: 0.03,
        }),
    };
    let render = |fog: Vec<RenderDiff>| {
        let mut harness = scene(VolumetricFogQuality::Low);
        harness.apply(walls.clone());
        harness.apply(fog);
        let image = look(&mut harness);
        let refused = harness.renderer.settings_readout().volumetric_fog.is_some();
        (image, refused)
    };
    let (both, refused) = render(vec![analytic.clone(), medium(0.02)]);
    let (analytic_only, _) = render(vec![analytic.clone()]);
    if refused {
        // A software adapter refuses the volumetric fog: the analytic fog
        // draws alone, all the way.
        assert_eq!(both, analytic_only, "refused, the analytic fog alone");
        return;
    }
    let (volumetric_only, _) = render(vec![medium(0.02)]);
    keep("both", &both);
    keep("volumetric-only", &volumetric_only);
    let near = (70..110, 20..WIDTH / 2 - 20);
    let far = (70..110, WIDTH / 2 + 20..WIDTH - 20);
    // Redness: red over green, which the red analytic fog raises.
    let red = |rgba: &[u8], (rows, columns): &(std::ops::Range<u32>, std::ops::Range<u32>)| {
        let (mut sum, mut count) = (0.0, 0.0);
        for y in rows.clone() {
            for x in columns.clone() {
                let at = ((y * WIDTH + x) * 4) as usize;
                sum += f64::from(rgba[at]) - f64::from(rgba[at + 1]);
                count += 1.0;
            }
        }
        sum / count
    };
    assert!(
        (red(&both, &near) - red(&volumetric_only, &near)).abs() < 1.5,
        "within the grid the analytic fog adds nothing: both {:.2}, volumetric {:.2}, analytic {:.2}",
        red(&both, &near),
        red(&volumetric_only, &near),
        red(&analytic_only, &near)
    );
    assert!(
        red(&both, &far) > red(&volumetric_only, &far) + 5.0,
        "beyond it the analytic fog takes over: both {:.2}, volumetric {:.2}",
        red(&both, &far),
        red(&volumetric_only, &far)
    );
}

/// The mean absolute difference of two images, per channel.
fn mean_difference(a: &[u8], b: &[u8]) -> f64 {
    let total: u64 = a
        .as_chunks::<4>()
        .0
        .iter()
        .zip(b.as_chunks::<4>().0)
        .map(|(a, b)| (0..3).map(|c| u64::from(a[c].abs_diff(b[c]))).sum::<u64>())
        .sum();
    total as f64 / (a.len() / 4 * 3) as f64
}

/// From `position`, toward the low sun (yaw 0 faces -Z), slightly down.
fn look_from(harness: &mut Harness, position: [f64; 3]) -> Vec<u8> {
    harness.render(&camera(position, 0.0, -4.0)).1
}

#[test]
fn a_still_view_gathers_and_holds_and_a_cut_starts_afresh() {
    let mut harness = scene(VolumetricFogQuality::High);
    if refused_on_software(&mut harness, vec![medium(0.03)]) {
        return;
    }
    harness.apply(vec![medium(0.03)]);
    let frames: Vec<Vec<u8>> = (0..40)
        .map(|_| look_from(&mut harness, [0.0, 1.7, 0.0]))
        .collect();
    // The first frame samples cell centres; the history then gathers the
    // jittered points across each cell, so the image moves off it...
    let gathered = mean_difference(&frames[0], &frames[39]);
    assert!(
        gathered > 0.01,
        "the history gathers more than the centres: {gathered:.3}"
    );
    // ...and holds steady while the view does: the jitter's flicker is
    // filtered away.
    let flicker = (10..39)
        .map(|frame| mean_difference(&frames[frame], &frames[frame + 1]))
        .fold(0.0f64, f64::max);
    assert!(
        flicker < 0.1,
        "a still view holds steady: at most {flicker:.3} levels from frame to frame"
    );
    // A cut (30 m in one frame, half the grid's reach) drops the history:
    // the first frame there is a fresh renderer's.
    let cut = look_from(&mut harness, [30.0, 1.7, 0.0]);
    let mut fresh = scene(VolumetricFogQuality::High);
    fresh.apply(vec![medium(0.03)]);
    assert_eq!(
        cut,
        look_from(&mut fresh, [30.0, 1.7, 0.0]),
        "no history crosses a cut"
    );
}

#[test]
fn shafts_through_a_slatted_wall_hold_under_camera_motion() {
    // Slats 0.5 m wide with 0.5 m gaps, 20 m ahead, between the camera and
    // the low sun: shafts of lit fog stream through the gaps.
    let mut slats = vec![
        RenderDiff::DefineMaterial {
            material: material("material/slat", [0.3, 0.25, 0.2, 1.0], None),
        },
        static_mesh(
            "mesh/slat",
            box_mesh([0.0, 0.0, -20.5], [0.5, 12.0, -20.0], |_| 0),
            "material/slat",
        ),
    ];
    for index in 0..60 {
        slats.push(instance(
            100 + index,
            None,
            "mesh/slat",
            transform([index as f32 - 30.0, 0.0, 0.0], 0.0, [1.0; 3]),
        ));
    }
    let setup = || {
        let mut harness = scene(VolumetricFogQuality::High);
        harness.apply(slats.clone());
        harness
    };
    if refused_on_software(&mut setup(), vec![medium(0.04)]) {
        return;
    }
    let mut clear = setup();
    let unfogged = look_from(&mut clear, [0.3, 1.7, 0.0]);
    // Strafing 3 cm a frame for 40 frames, from x 0.3 - 1.2 to 0.3.
    let mut moving = setup();
    moving.apply(vec![medium(0.04)]);
    let mut last = Vec::new();
    for frame in 0..=40 {
        let x = 0.3 - 0.03 * f64::from(40 - frame);
        last = look_from(&mut moving, [x, 1.7, 0.0]);
    }
    // The same pose held still for as long.
    let mut still = setup();
    still.apply(vec![medium(0.04)]);
    let mut settled = Vec::new();
    for _ in 0..=40 {
        settled = look_from(&mut still, [0.3, 1.7, 0.0]);
    }
    keep("shafts-moving", &last);
    keep("shafts-still", &settled);
    let trailing = mean_difference(&last, &settled);
    assert!(
        trailing < 2.0,
        "moving, the shafts match the still view without trails: {trailing:.3} levels apart"
    );
    // The fog's own light along a row in the air before the wall varies
    // across the shafts and the shadowed stripes between them.
    let row = HEIGHT / 2 - 10;
    let added: Vec<f64> = (40..WIDTH - 40)
        .map(|x| {
            let at = ((row * WIDTH + x) * 4) as usize;
            f64::from(last[at]) - f64::from(unfogged[at])
        })
        .collect();
    let mean = added.iter().sum::<f64>() / added.len() as f64;
    let spread =
        (added.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / added.len() as f64).sqrt();
    assert!(
        spread > 2.0,
        "the shafts stand out from the shadowed fog between them: spread {spread:.2} about {mean:.2}"
    );
}
