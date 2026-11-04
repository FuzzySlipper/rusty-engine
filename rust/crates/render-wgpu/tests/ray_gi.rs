//! Ray-traced global illumination (`ray_gi.rs`): rays bring the world's
//! indirect light in place of the probes and the sky's light. A sunlit red
//! wall colours the floor beside it, a sky reaches what it can see and not
//! what it cannot, an emissive part lights what faces it, and a device or a
//! frame that cannot run it refuses, drawing as before.

mod support;

use render_model::*;
#[cfg(feature = "dlss")]
use render_wgpu::OffscreenTarget;
use render_wgpu::{Gpu, Guide, OverlayShown, RendererOptions, SettingRefusal};
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

/// A lamp behind the low wall of the bounce scenes.
fn lamp() -> RenderDiff {
    RenderDiff::CreateLight {
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
        ops.push(lamp());
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

/// The floor's light beside a high wall lit from behind a low one, through a
/// card of `alpha` between the lamp and that wall.
fn floor_through_card(gpu: &Gpu, alpha: u8) -> f32 {
    let mut harness = Harness::on(gpu.clone(), options(GlobalIllumination::RayTraced));
    harness.renderer.set_ray_gi_without_denoiser(true);
    let texture = harness.resources.texture(
        "texture/card",
        2,
        2,
        &[8, 8, 8, alpha].repeat(4),
        TextureWrap::Clamp,
    );
    let mut ops = floor();
    ops.push(RenderDiff::DefineTexture { texture });
    ops.push(RenderDiff::DefineMaterial {
        material: material("material/wall", [0.8, 0.8, 0.8, 1.0], None),
    });
    ops.push(RenderDiff::DefineMaterial {
        material: RenderMaterialDescriptor {
            alpha_mode: MaterialAlphaModeDescriptor::Mask { cutoff: 0.5 },
            ..material("material/card", [1.0; 4], Some("texture/card"))
        },
    });
    for (handle, asset, surface, min, max) in [
        (
            WALL,
            "mesh/low",
            "material/wall",
            [-6.0, 0.0, -3.1],
            [6.0, 1.2, -3.0],
        ),
        (
            WALL + 1,
            "mesh/high",
            "material/wall",
            [-6.0, 0.0, -7.1],
            [6.0, 6.0, -7.0],
        ),
        (
            WALL + 2,
            "mesh/card",
            "material/card",
            [-6.0, 0.0, -6.1],
            [6.0, 6.0, -6.0],
        ),
    ] {
        ops.push(static_mesh(asset, box_mesh(min, max, |_| 0), surface));
        ops.push(instance(
            handle,
            None,
            asset,
            transform([0.0; 3], 0.0, [1.0; 3]),
        ));
    }
    ops.push(lamp());
    harness.apply(ops);
    let view = camera([0.0, 1.0, 0.0], 0.0, -40.0);
    for _ in 0..SETTLING_FRAMES {
        harness.render(&view);
    }
    let picture = harness.render(&view).1;
    mean(&picture[picture.len() / 3 * 2 / 4 * 4..])[1]
}

#[test]
fn a_cut_out_passes_light_and_paths_where_clear_and_stops_them_where_opaque() {
    let gpu = Gpu::headless_with_ray_queries().expect("a device");
    if render_wgpu_cannot_trace(&gpu) {
        return;
    }
    // The wall behind a card takes the lamp through it (a visibility ray),
    // and the floor sees the wall through it (a path's ray).
    let (clear, opaque) = (floor_through_card(&gpu, 0), floor_through_card(&gpu, 255));
    assert!(
        clear > 2.0 && clear > opaque * 3.0,
        "the floor took {clear} through clear texels and {opaque} through opaque ones"
    );
}

fn mirror_material() -> RenderDiff {
    RenderDiff::DefineMaterial {
        material: RenderMaterialDescriptor {
            metalness: 1.0,
            roughness: 0.0,
            ..material("material/mirror", [0.9, 0.9, 0.9, 1.0], None)
        },
    }
}

fn solid(handle: u64, asset: &str, surface: &str, min: [f32; 3], max: [f32; 3]) -> Vec<RenderDiff> {
    vec![
        static_mesh(asset, box_mesh(min, max, |_| 0), surface),
        instance(handle, None, asset, transform([0.0; 3], 0.0, [1.0; 3])),
    ]
}

fn red() -> RenderDiff {
    RenderDiff::DefineMaterial {
        material: material("material/red", [0.9, 0.05, 0.05, 1.0], None),
    }
}

/// `scene`'s diffuse guide once paths have walked it, or the world's own.
fn diffuse_guide(gpu: &Gpu, setting: GlobalIllumination, scene: Vec<RenderDiff>) -> Vec<u8> {
    let mut harness = Harness::on(gpu.clone(), options(setting));
    harness.renderer.set_ray_gi_without_denoiser(true);
    harness.renderer.set_overlay_shown(OverlayShown {
        guide: Some(Guide::DiffuseAlbedo),
        ..OverlayShown::default()
    });
    harness.apply(scene);
    let view = camera([0.0, 1.5, 0.0], 0.0, -20.0);
    for _ in 0..4 {
        harness.render(&view);
    }
    harness.render(&view).1
}

#[test]
fn a_mirror_gives_way_in_the_guides_to_what_its_chain_ends_on() {
    let gpu = Gpu::headless_with_ray_queries().expect("a device");
    if render_wgpu_cannot_trace(&gpu) {
        return;
    }
    let floor_of = |rgba: &[u8]| mean(&rgba[rgba.len() / 3 * 2 / 4 * 4..]);
    let mirror_floor = || {
        let mut ops = vec![mirror_material(), red()];
        ops.extend(solid(
            FLOOR,
            "mesh/mirror",
            "material/mirror",
            [-40.0, -0.2, -40.0],
            [40.0, 0.0, 40.0],
        ));
        ops
    };
    // One mirror: the floor shows the red wall ahead.
    let one = || {
        let mut ops = mirror_floor();
        ops.extend(solid(
            WALL,
            "mesh/red",
            "material/red",
            [-40.0, 0.0, -8.2],
            [40.0, 30.0, -8.0],
        ));
        ops
    };
    // Two: the floor shows the wall ahead, a mirror showing the red wall
    // behind the camera.
    let two = || {
        let mut ops = mirror_floor();
        ops.extend(solid(
            WALL,
            "mesh/wall-mirror",
            "material/mirror",
            [-40.0, 0.0, -8.2],
            [40.0, 30.0, -8.0],
        ));
        ops.extend(solid(
            WALL + 1,
            "mesh/red",
            "material/red",
            [-40.0, 0.0, 8.0],
            [40.0, 30.0, 8.2],
        ));
        ops
    };
    let world = floor_of(&diffuse_guide(&gpu, GlobalIllumination::Off, one()));
    assert!(
        world[0] < 2.0,
        "a metal has no diffuse albedo of its own: {world:?}"
    );
    for (chain, scene) in [("one mirror", one()), ("two mirrors", two())] {
        let guide = floor_of(&diffuse_guide(&gpu, GlobalIllumination::RayTraced, scene));
        assert!(
            guide[0] > guide[1] + 60.0,
            "the floor's diffuse guide through {chain}: {guide:?}"
        );
    }
}

/// The motion view's range (`compose.wgsl` `MOTION_VIEW_RANGE`).
const MOTION_RANGE: f32 = 16.0;

/// The mean x motion, render pixels, where the picture's lower third moved.
fn moved_x(rgba: &[u8]) -> (usize, f32) {
    let (mut count, mut sum) = (0, 0.0);
    for pixel in rgba[rgba.len() / 3 * 2 / 4 * 4..].as_chunks::<4>().0 {
        let x = (f32::from(pixel[0]) / 255.0 * 2.0 - 1.0) * MOTION_RANGE;
        if x.abs() > 1.0 {
            count += 1;
            sum += x;
        }
    }
    (count, sum / count.max(1) as f32)
}

/// A box moving right above a mirror floor: the motion Ray Reconstruction
/// reads under it.
fn reflection_motion(gpu: &Gpu, setting: GlobalIllumination) -> (usize, f32) {
    let mut harness = Harness::on(gpu.clone(), options(setting));
    harness.renderer.set_ray_gi_without_denoiser(true);
    harness.renderer.set_overlay_shown(OverlayShown {
        motion: true,
        ..OverlayShown::default()
    });
    let mut ops = vec![mirror_material(), red()];
    ops.extend(solid(
        FLOOR,
        "mesh/mirror",
        "material/mirror",
        [-40.0, -0.2, -40.0],
        [40.0, 0.0, 40.0],
    ));
    ops.push(static_mesh(
        "mesh/box",
        box_mesh([-1.5, 1.0, -7.0], [1.5, 6.0, -6.0], |_| 0),
        "material/red",
    ));
    ops.push(instance(
        WALL,
        None,
        "mesh/box",
        transform([0.0; 3], 0.0, [1.0; 3]),
    ));
    harness.apply(ops);
    let view = camera([0.0, 1.5, 0.0], 0.0, -25.0);
    harness.render(&view);
    harness.render(&view);
    let mut picture = Vec::new();
    for step in 1..4 {
        harness.apply(vec![RenderDiff::Update {
            handle: RenderHandle::new(WALL),
            transform: Some(transform([step as f32 * 0.2, 0.0, 0.0], 0.0, [1.0; 3])),
            material: None,
            visible: None,
            metadata: None,
        }]);
        picture = harness.render(&view).1;
    }
    moved_x(&picture)
}

#[test]
fn a_mirror_moves_with_what_it_reflects_in_the_guides() {
    let gpu = Gpu::headless_with_ray_queries().expect("a device");
    if render_wgpu_cannot_trace(&gpu) {
        return;
    }
    let (still, _) = reflection_motion(&gpu, GlobalIllumination::Off);
    let (moved, x) = reflection_motion(&gpu, GlobalIllumination::RayTraced);
    assert_eq!(still, 0, "the world's own motion holds the floor still");
    assert!(moved > 50, "{moved} pixels of the reflection moved");
    assert!(x > 1.0, "the reflection moved right with the box: {x}");
}

/// The red a moving picture differs by from a still one, under the box seen
/// directly: what its reflection trails or smears.
#[cfg(feature = "dlss")]
fn reflection_error(moving: &[u8], still: &[u8]) -> u64 {
    let start = (RR_SIZE.0 * RR_SIZE.1 * 2 / 5 * 4) as usize;
    moving[start..]
        .as_chunks::<4>()
        .0
        .iter()
        .zip(still[start..].as_chunks::<4>().0)
        .map(|(moving, still)| u64::from(moving[0].abs_diff(still[0])))
        .sum()
}

/// The size Ray Reconstruction's mirror scene draws at, and the most its
/// reflection may differ moving: 39,000 on an RTX 3080, 48,500 with a mirror
/// that does not give way in the guides.
#[cfg(feature = "dlss")]
const RR_SIZE: (u32, u32) = (960, 540);
#[cfg(feature = "dlss")]
const RR_REFLECTION_ERROR: u64 = 44_000;

/// A red box over a mirror floor before a mirror wall under Ray
/// Reconstruction, at `x`; moving there at `step` a frame, or still. The
/// settled picture, written for the eye beside the Engine's evidence.
#[cfg(feature = "dlss")]
fn reconstructed_mirrors(gpu: &Gpu, x: f32, step: f32, name: &str) -> Vec<u8> {
    const FRAMES: u32 = 48;
    let mut harness = Harness::on(
        gpu.clone(),
        RendererOptions {
            default_world_lights: false,
            ..RendererOptions::default().with_settings(&RendererSettingsDescriptor {
                antialiasing: Antialiasing::Dlss,
                global_illumination: GlobalIllumination::RayTraced,
                ..RendererSettingsDescriptor::DEFAULT
            })
        },
    );
    let mut ops = vec![mirror_material(), red(), sun()];
    ops.extend(solid(
        FLOOR,
        "mesh/mirror",
        "material/mirror",
        [-40.0, -0.2, -40.0],
        [40.0, 0.0, 40.0],
    ));
    ops.extend(solid(
        WALL + 1,
        "mesh/wall",
        "material/mirror",
        [-40.0, 0.0, -12.2],
        [40.0, 30.0, -12.0],
    ));
    ops.push(static_mesh(
        "mesh/box",
        box_mesh([-1.0, 1.0, -7.0], [1.0, 3.0, -6.0], |_| 0),
        "material/red",
    ));
    ops.push(instance(
        WALL,
        None,
        "mesh/box",
        transform([0.0; 3], 0.0, [1.0; 3]),
    ));
    harness.apply(ops);
    harness.target = OffscreenTarget::new(gpu, RR_SIZE.0, RR_SIZE.1, harness.renderer.samples());
    let view = camera([0.0, 1.5, 0.0], 0.0, -25.0);
    let mut picture = Vec::new();
    for frame in 0..FRAMES {
        let at = x - step * (FRAMES - 1 - frame) as f32;
        harness.apply(vec![RenderDiff::Update {
            handle: RenderHandle::new(WALL),
            transform: Some(transform([at, 0.0, 0.0], 0.0, [1.0; 3])),
            material: None,
            visible: None,
            metadata: None,
        }]);
        let (stats, drawn) = harness.render(&view);
        picture = drawn;
        if frame + 1 == FRAMES {
            assert_eq!(stats.upscale, Some(render_wgpu::UpscaleMethod::DlssRr));
        }
    }
    let mut file = format!("P6 {} {} 255\n", RR_SIZE.0, RR_SIZE.1).into_bytes();
    for pixel in picture.as_chunks::<4>().0 {
        file.extend(&pixel[..3]);
    }
    std::fs::write(
        std::env::temp_dir().join(format!("ray-gi-{name}.ppm")),
        file,
    )
    .expect("the picture writes");
    picture
}

/// Ray Reconstruction keeps a moving reflection where it is: what a mirror
/// shows of a moving box lands on what it shows of a box standing there.
/// Needs DLSS: run on a GPU with Ray Reconstruction
/// (docs/verification.md#gpu-verification).
#[cfg(feature = "dlss")]
#[test]
#[ignore = "needs DLSS Ray Reconstruction"]
fn a_moving_reflection_stays_on_its_surface_under_ray_reconstruction() {
    let gpu = Gpu::headless_with_ray_queries().expect("a device");
    let moving = reconstructed_mirrors(&gpu, 1.0, 0.2, "moving");
    let still = reconstructed_mirrors(&gpu, 1.0, 0.0, "still");
    let error = reflection_error(&moving, &still);
    eprintln!("reflection error {error}");
    assert!(error < RR_REFLECTION_ERROR, "reflection error {error}");
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
