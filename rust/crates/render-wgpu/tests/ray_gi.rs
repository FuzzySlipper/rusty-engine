//! Ray-traced global illumination (`ray_gi.rs`): rays bring the world's
//! indirect light in place of the probes and the sky's light. A sunlit red
//! wall colours the floor beside it, a sky reaches what it can see and not
//! what it cannot, an emissive part lights what faces it, and a device or a
//! frame that cannot run it refuses, drawing as before.

mod support;

use render_model::*;
use render_wgpu::{Gpu, RendererOptions, SettingRefusal};
use support::*;

const FLOOR: u64 = 1;
const WALL: u64 = 2;
const SUN: u64 = 90;
const SKY: u64 = 91;
const LAMP: u64 = 92;
/// Frames that fill the world cache and let the temporal stage gather them.
const SETTLING_FRAMES: u32 = 48;

fn options(global_illumination: GlobalIllumination) -> RendererOptions {
    RendererOptions {
        default_world_lights: false,
        ..RendererOptions::default().with_settings(&RendererSettingsDescriptor {
            antialiasing: Antialiasing::Temporal,
            shadows: true,
            global_illumination,
            ..RendererSettingsDescriptor::DEFAULT
        })
    }
}

fn floor() -> Vec<RenderDiff> {
    vec![
        RenderDiff::DefineMaterial {
            material: material("material/floor", [0.8, 0.8, 0.8, 1.0], None),
        },
        static_mesh(
            "mesh/floor",
            box_mesh([-20.0, -0.2, -20.0], [20.0, 0.0, 20.0], |_| 0),
            "material/floor",
        ),
        instance(
            FLOOR,
            None,
            "mesh/floor",
            transform([0.0; 3], 0.0, [1.0; 3]),
        ),
    ]
}

fn sun() -> RenderDiff {
    RenderDiff::CreateLight {
        handle: RenderHandle::new(SUN),
        parent: None,
        light: LightDescriptor::Directional {
            color: [1.0; 3],
            intensity: 1.0,
            enabled: true,
            direction: [-1.0, -1.0, 0.0],
            range: None,
            shadow_intent: LightShadowIntent::Disabled,
            shadow: Default::default(),
        },
    }
}

/// A red wall at x = -2, facing the sun.
fn red_wall() -> Vec<RenderDiff> {
    vec![
        RenderDiff::DefineMaterial {
            material: material("material/wall", [0.9, 0.05, 0.05, 1.0], None),
        },
        static_mesh(
            "mesh/wall",
            box_mesh([-2.2, 0.0, -20.0], [-2.0, 6.0, 20.0], |_| 0),
            "material/wall",
        ),
        instance(WALL, None, "mesh/wall", transform([0.0; 3], 0.0, [1.0; 3])),
    ]
}

/// The mean red, green and blue of `rgba`.
fn mean(rgba: &[u8]) -> [f32; 3] {
    let pixels = rgba.as_chunks::<4>().0;
    let mut sum = [0.0; 3];
    for pixel in pixels {
        for channel in 0..3 {
            sum[channel] += f32::from(pixel[channel]);
        }
    }
    sum.map(|channel| channel / pixels.len() as f32)
}

/// The picture once the frames settled, and the harness that drew it.
fn settled(
    gpu: &Gpu,
    global_illumination: GlobalIllumination,
    scene: Vec<RenderDiff>,
    view: &render_host_contracts::RendererCompositionCamera,
) -> (Harness, Vec<u8>) {
    let mut harness = Harness::on(gpu.clone(), options(global_illumination));
    harness.renderer.set_ray_gi_without_denoiser(true);
    harness.apply(scene);
    for _ in 0..SETTLING_FRAMES {
        harness.render(view);
    }
    let picture = harness.render(view).1;
    (harness, picture)
}

#[test]
fn a_sunlit_wall_colours_the_floor_beside_it() {
    let gpu = Gpu::headless_with_ray_queries().expect("a device");
    if render_wgpu_cannot_trace(&gpu) {
        return;
    }
    let scene = || {
        let mut ops = floor();
        ops.extend(red_wall());
        ops.push(sun());
        ops
    };
    let view = camera([1.0, 1.5, 0.0], -90.0, -35.0);
    let (_, direct) = settled(&gpu, GlobalIllumination::Off, scene(), &view);
    let (harness, lit) = settled(&gpu, GlobalIllumination::RayTraced, scene(), &view);
    let readout = harness.renderer.gpu_readout();
    let gi = readout.ray_gi.expect("it drew");
    assert!(gi.world_cache_cells > 0, "{gi:?}");
    assert_eq!(gi.light_sources, 1);
    // The floor: the picture's lower third.
    let floor_of = |rgba: &[u8]| mean(&rgba[rgba.len() / 3 * 2 / 4 * 4..]);
    let (direct, lit) = (floor_of(&direct), floor_of(&lit));
    assert!(
        lit[0] - lit[1] > direct[0] - direct[1] + 4.0,
        "the floor beside the wall went from {direct:?} to {lit:?}"
    );
}

#[test]
fn a_miss_meets_the_sky_and_a_closed_box_does_not() {
    let gpu = Gpu::headless_with_ray_queries().expect("a device");
    if render_wgpu_cannot_trace(&gpu) {
        return;
    }
    let sky = RenderDiff::CreateLight {
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
    };
    let view = camera([0.0, 1.5, 0.0], 0.0, -45.0);
    let mut open = floor();
    open.push(sky.clone());
    let (_, open) = settled(&gpu, GlobalIllumination::RayTraced, open, &view);
    // A lid and four walls around the camera.
    let mut closed = floor();
    closed.push(sky);
    closed.push(RenderDiff::DefineMaterial {
        material: material("material/box", [0.8, 0.8, 0.8, 1.0], None),
    });
    for (index, (min, max)) in [
        ([-6.0, 4.0, -6.0], [6.0, 4.2, 6.0]),
        ([-6.2, 0.0, -6.2], [-6.0, 4.0, 6.2]),
        ([6.0, 0.0, -6.2], [6.2, 4.0, 6.2]),
        ([-6.2, 0.0, -6.2], [6.2, 4.0, -6.0]),
        ([-6.2, 0.0, 6.0], [6.2, 4.0, 6.2]),
    ]
    .into_iter()
    .enumerate()
    {
        let asset = format!("mesh/box-{index}");
        closed.push(static_mesh(
            &asset,
            box_mesh(min, max, |_| 0),
            "material/box",
        ));
        closed.push(instance(
            10 + index as u64,
            None,
            &asset,
            transform([0.0; 3], 0.0, [1.0; 3]),
        ));
    }
    let (_, closed) = settled(&gpu, GlobalIllumination::RayTraced, closed, &view);
    let (open, closed) = (mean(&open)[1], mean(&closed)[1]);
    assert!(open > 60.0, "open sky lit the floor to {open}");
    assert!(closed < 4.0, "the closed box lit its floor to {closed}");
}

#[test]
fn an_emissive_part_lights_what_faces_it() {
    let gpu = Gpu::headless_with_ray_queries().expect("a device");
    if render_wgpu_cannot_trace(&gpu) {
        return;
    }
    let mut scene = floor();
    scene.push(RenderDiff::DefineMaterial {
        material: RenderMaterialDescriptor {
            emission_color: [1.0; 3],
            emission_intensity: 8.0,
            ..material("material/lamp", [0.0, 0.0, 0.0, 1.0], None)
        },
    });
    scene.push(static_mesh(
        "mesh/lamp",
        box_mesh([-1.0, 1.0, -5.0], [1.0, 1.2, -3.0], |_| 0),
        "material/lamp",
    ));
    scene.push(instance(
        LAMP,
        None,
        "mesh/lamp",
        transform([0.0; 3], 0.0, [1.0; 3]),
    ));
    let view = camera([0.0, 1.5, 0.0], 0.0, -45.0);
    let (harness, lit) = settled(&gpu, GlobalIllumination::RayTraced, scene, &view);
    assert_eq!(
        harness
            .renderer
            .gpu_readout()
            .ray_gi
            .expect("it drew")
            .light_sources,
        1
    );
    assert!(
        mean(&lit)[1] > 10.0,
        "the lamp lit the floor to {:?}",
        mean(&lit)
    );
}

#[test]
fn a_lamp_lights_past_the_wall_that_hides_it_by_its_bounce() {
    let gpu = Gpu::headless_with_ray_queries().expect("a device");
    if render_wgpu_cannot_trace(&gpu) {
        return;
    }
    // A lamp behind a low wall, a high wall beyond them both: only what the
    // high wall throws back reaches the floor before the low one.
    let scene = || {
        let mut ops = floor();
        ops.push(RenderDiff::DefineMaterial {
            material: material("material/wall", [0.8, 0.8, 0.8, 1.0], None),
        });
        for (handle, asset, min, max) in [
            (WALL, "mesh/low", [-6.0, 0.0, -3.1], [6.0, 1.2, -3.0]),
            (WALL + 1, "mesh/high", [-6.0, 0.0, -7.1], [6.0, 6.0, -7.0]),
        ] {
            ops.push(static_mesh(
                asset,
                box_mesh(min, max, |_| 0),
                "material/wall",
            ));
            ops.push(instance(
                handle,
                None,
                asset,
                transform([0.0; 3], 0.0, [1.0; 3]),
            ));
        }
        ops.push(RenderDiff::CreateLight {
            handle: RenderHandle::new(LAMP),
            parent: None,
            light: LightDescriptor::Point {
                color: [1.0; 3],
                intensity: 40.0,
                enabled: true,
                position: [0.0, 0.8, -5.0],
                range: None,
                decay: 2.0,
                shadow_intent: LightShadowIntent::Requested,
                shadow: Default::default(),
            },
        });
        ops
    };
    let view = camera([0.0, 1.0, 0.0], 0.0, -40.0);
    let lit_floor = |rgba: &[u8]| mean(&rgba[rgba.len() / 3 * 2 / 4 * 4..])[1];
    let (_, direct) = settled(&gpu, GlobalIllumination::Off, scene(), &view);
    let (_, bounced) = settled(&gpu, GlobalIllumination::RayTraced, scene(), &view);
    let (direct, bounced) = (lit_floor(&direct), lit_floor(&bounced));
    assert!(
        direct < 0.5 && bounced > 2.0,
        "the floor before the low wall went from {direct} to {bounced}"
    );
}

/// On a device that cannot trace, global illumination refuses, and the
/// frame draws as it would without it.
fn render_wgpu_cannot_trace(gpu: &Gpu) -> bool {
    let mut harness = Harness::on(gpu.clone(), options(GlobalIllumination::RayTraced));
    harness.renderer.set_ray_gi_without_denoiser(true);
    harness
        .renderer
        .settings_readout()
        .global_illumination
        .is_some()
}

#[test]
fn without_ray_reconstruction_it_refuses_and_the_probes_draw() {
    let gpu = Gpu::headless_with_ray_queries().expect("a device");
    let scene = || {
        let mut ops = floor();
        ops.push(sun());
        ops
    };
    let view = camera([0.0, 1.5, 0.0], 0.0, -45.0);
    let draw = |setting| {
        let mut harness = Harness::on(gpu.clone(), options(setting));
        harness.apply(scene());
        harness.render(&view);
        let picture = harness.render(&view).1;
        (harness, picture)
    };
    let (asked, picture) = draw(GlobalIllumination::RayTraced);
    let (_, plain) = draw(GlobalIllumination::Off);
    let readout = asked.renderer.settings_readout();
    assert_eq!(
        readout.global_illumination,
        Some(if gpu.is_software() {
            SettingRefusal::SoftwareAdapter
        } else {
            SettingRefusal::NeedsDlss
        })
    );
    assert_eq!(
        readout.effective.global_illumination,
        GlobalIllumination::Off
    );
    assert!(asked.renderer.gpu_readout().ray_gi.is_none());
    assert_eq!(picture, plain, "a refused setting draws as without it");
}
