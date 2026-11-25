//! Fog regions (`fog_regions.rs`, `fog_region.wgsl`, `volumetric_fog.wgsl`):
//! a stirrer crossing a dense region clears a hole that trails behind it
//! and fills back in once it has gone; readings between output frames leave
//! what the output draws as it was; the region and its step are reported; a
//! renderer built afresh from the same scene (a snapshot drawn offline, a
//! rebaseline) and seeded with the evolved cells shows the same wake.
//! Same harness as
//! `tests/volumetric_fog.rs`.

mod support;

use render_model::*;
use render_wgpu::{
    CommittedCall, FogField, NoResources, OffscreenTarget, Renderer, RendererOptions, SceneChange,
    SceneDriver, SceneState,
};
use support::*;

/// Output frames a second.
const RATE: f64 = 30.0;
const SETTLE_SECONDS: f32 = 2.0;

fn options() -> RendererOptions {
    RendererOptions {
        default_world_lights: false,
        shadows: true,
        volumetric_fog: VolumetricFogQuality::Low,
        ..RendererOptions::default()
    }
}

/// A dark floor and back wall, a dim fill and a point light over the
/// region, which glows faintly too.
fn scene() -> Harness {
    let mut harness = Harness::new(options());
    harness.apply(vec![
        RenderDiff::SetBackgroundColor {
            color: [0.0, 0.0, 0.0, 1.0],
        },
        RenderDiff::DefineMaterial {
            material: material("material/rock", [0.05, 0.05, 0.05, 1.0], None),
        },
        static_mesh(
            "mesh/rock",
            box_mesh([-40.0, -1.0, -40.0], [40.0, 0.0, 40.0], |_| 0),
            "material/rock",
        ),
        instance(20, None, "mesh/rock", transform([0.0; 3], 0.0, [1.0; 3])),
        RenderDiff::CreateLight {
            handle: RenderHandle::new(10),
            parent: None,
            light: LightDescriptor::Point {
                color: [1.0, 0.85, 0.6],
                intensity: 40.0,
                enabled: true,
                position: [0.0, 3.5, -8.0],
                range: Some(20.0),
                decay: 2.0,
                shadow_intent: LightShadowIntent::Disabled,
                shadow: Default::default(),
            },
        },
    ]);
    harness
}

/// A sump of mist 16 m across, 3 m high and 2 m deep, 10 m ahead of the
/// camera (about 80% opaque through), drifting slowly to +x, without noise
/// so a hole is all that changes it.
fn region() -> RenderDiff {
    RenderDiff::SetFogRegion {
        id: 1,
        region: FogRegionDescriptor {
            center: [0.0, 1.5, -10.0],
            half_extents: [8.0, 1.5, 1.0],
            yaw_degrees: 0.0,
            cell_size: 0.25,
            density: 0.8,
            albedo: [0.7, 0.7, 0.7],
            emission: [0.05, 0.05, 0.06],
            edge: 0.1,
            noise_scale: 0.0,
            noise_strength: 0.0,
            current: [0.2, 0.0, 0.0],
            settle_seconds: SETTLE_SECONDS,
        },
    }
}

/// A body crossing the sump along +x through its middle at 4 m/s, at `x`.
fn stirrer(x: f32) -> RenderDiff {
    RenderDiff::SetFogStirrer {
        id: 3,
        stirrer: FogStirrerDescriptor {
            position: [x, 1.5, -10.0],
            velocity: [4.0, 0.0, 0.0],
            radius: 1.0,
            push: 0.8,
            clear: 1.0,
        },
    }
}

/// From head height looking straight at the sump's middle.
fn look(harness: &mut Harness, time: f64) -> Vec<u8> {
    harness.renderer.set_presentation_time(time);
    harness.render(&camera([0.0, 1.5, 0.0], 0.0, 0.0)).1
}

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

/// Mean luminance of a patch around the column where the sump's middle row
/// shows world `x` (10 m ahead).
fn at(rgba: &[u8], x: f32) -> f64 {
    let half_width = 10.0 * (30f64.to_radians().tan() * f64::from(WIDTH) / f64::from(HEIGHT));
    let column = (f64::from(WIDTH) * (0.5 + f64::from(x) / (2.0 * half_width))) as u32;
    let mut total = 0.0;
    let mut count = 0.0;
    for y in HEIGHT / 2 - 6..HEIGHT / 2 + 6 {
        for x in column - 4..column + 4 {
            let index = ((y * WIDTH + x) * 4) as usize;
            total += 0.2126 * f64::from(rgba[index])
                + 0.7152 * f64::from(rgba[index + 1])
                + 0.0722 * f64::from(rgba[index + 2]);
            count += 1.0;
        }
    }
    total / count
}

/// The fog's froxels are sampled at points that cycle every this many
/// frames, so frames compared are a whole number of cycles apart.
const JITTER_CYCLE: u32 = 16;

/// Draws `cycles` jitter cycles of frames from `start`, moving the stirrer
/// along with them when `body` gives where it starts; the last frame, and
/// the time after it.
fn run(harness: &mut Harness, start: f64, cycles: u32, body: Option<f32>) -> (Vec<u8>, f64) {
    let frames = cycles * JITTER_CYCLE;
    let mut last = Vec::new();
    for frame in 0..frames {
        let time = start + f64::from(frame) / RATE;
        if let Some(from) = body {
            harness.apply(vec![stirrer(from + 4.0 * (time - start) as f32)]);
        }
        last = look(harness, time);
    }
    (last, start + f64::from(frames) / RATE)
}

#[test]
fn a_crossing_body_clears_a_wake_that_fills_back_in() {
    let mut harness = scene();
    if harness.renderer.settings_readout().volumetric_fog.is_some() {
        // A software adapter draws no volumetric fog, and so no regions.
        return;
    }
    let dark = look(&mut harness, 0.0);
    harness.apply(vec![region()]);
    let (rest, time) = run(&mut harness, 1.0 / RATE, 2, None);
    keep("region-rest", &rest);
    assert!(
        at(&rest, -3.0) > at(&dark, -3.0) + 10.0,
        "the sump's mist shows: {:.1} over {:.1}",
        at(&rest, -3.0),
        at(&dark, -3.0)
    );
    let readout = harness.renderer.gpu_readout();
    assert_eq!(
        (
            readout.volumetric_fog.regions,
            readout.volumetric_fog.region_cells
        ),
        (1, 64 * 12 * 8)
    );
    assert!(readout.passes.iter().any(|pass| pass.pass == "fog-regions"));

    // From x = -6 for 48 frames (1.6 s, to x = 0.4): it passed x = -2 0.6 s
    // ago.
    let (passing, time) = run(&mut harness, time, 3, Some(-6.0));
    keep("region-passing", &passing);
    assert_eq!(harness.renderer.gpu_readout().volumetric_fog.stirrers, 1);
    let behind = at(&passing, -2.0);
    let ahead = at(&passing, 4.0);
    assert!(
        behind < at(&rest, -2.0) - 8.0,
        "the wake behind the body is clearer: {behind:.1} against {:.1} at rest",
        at(&rest, -2.0)
    );
    assert!(
        (ahead - at(&rest, 4.0)).abs() < 4.0,
        "the mist ahead of it is as it was: {ahead:.1} against {:.1}",
        at(&rest, 4.0)
    );

    harness.apply(vec![RenderDiff::RemoveFogStirrer { id: 3 }]);
    let (soon, time) = run(&mut harness, time, 1, None);
    // About six settle times.
    let (settled, time) = run(&mut harness, time, 24, None);
    keep("region-settled", &settled);
    let gap = |frame: &[u8]| (at(frame, -2.0) - at(&rest, -2.0)).abs();
    assert!(
        gap(&settled) < 3.0 && gap(&settled) < gap(&soon),
        "the wake fills back in: {:.1} after it went, {:.1} later",
        gap(&soon),
        gap(&settled)
    );

    harness.apply(vec![RenderDiff::RemoveFogRegion { id: 1 }]);
    look(&mut harness, time);
    assert_eq!(harness.renderer.gpu_readout().volumetric_fog.regions, 0);
}

#[test]
fn readings_between_output_frames_do_not_move_a_regions_cells() {
    let mut alone = scene();
    if alone.renderer.settings_readout().volumetric_fog.is_some() {
        return;
    }
    let fog = vec![region(), stirrer(-2.0)];
    alone.apply(fog.clone());
    let moments: Vec<_> = (0..6)
        .map(|frame| {
            (
                1.0 + f64::from(frame) / RATE,
                camera([0.0, 1.5, 0.0], 0.0, 0.0),
            )
        })
        .collect();
    let expected = output_frames_around_captures(&mut alone, &moments, None);
    let mut captured = scene();
    captured.apply(fog);
    let frames = output_frames_around_captures(&mut captured, &moments, Some((200, 120)));
    for (frame, (expected, drawn)) in expected.iter().zip(&frames).enumerate() {
        assert!(
            expected == drawn,
            "frame {frame}: a reading moved the region's cells"
        );
    }
}

/// The wake's darkest density factor in `field`'s one region.
fn emptiest(field: &FogField) -> f32 {
    field.regions[0]
        .data
        .iter()
        .map(|cell| cell[3])
        .fold(f32::INFINITY, f32::min)
}

#[test]
fn a_fresh_renderer_seeded_with_the_evolved_cells_shows_the_same_wake() {
    let mut harness = scene();
    if harness.renderer.settings_readout().volumetric_fog.is_some() {
        return;
    }
    harness.apply(vec![region()]);
    let (_, time) = run(&mut harness, 1.0 / RATE, 2, None);
    // (The pictures themselves differ by the fog's temporal filtering: a
    // fresh renderer's first frame samples cell centres with no history.)
    let (_, time) = run(&mut harness, time, 3, Some(-6.0));
    let field = harness.renderer.read_fog_field();
    assert_eq!(field.regions.len(), 1);
    assert!(
        emptiest(&field) < 0.2,
        "the live wake is in the cells: {}",
        emptiest(&field)
    );
    // A renderer built from the retained scene, as a snapshot is drawn, and
    // one more frame at the time the live one reached: unseeded, every cell
    // is at rest and the wake is gone; seeded, the cells are the live ones
    // and the wake is where it was.
    let fresh = |seed: Option<FogField>| {
        let mut renderer = Renderer::new(&harness.gpu, harness.renderer.options());
        let issues = renderer.apply(&harness.world.snapshot().frame, &harness.resources);
        assert!(issues.is_empty(), "{issues:?}");
        if let Some(seed) = seed {
            renderer.seed_fog_field(seed);
        }
        renderer.set_presentation_time(time);
        renderer.render_offscreen(&camera([0.0, 1.5, 0.0], 0.0, 0.0), &harness.target);
        (
            renderer.read_fog_field(),
            harness.target.read_rgba(&harness.gpu),
        )
    };
    let (rest, unseeded) = fresh(None);
    assert_eq!(emptiest(&rest), 1.0, "unseeded, every cell is at rest");
    let (seeded_field, seeded) = fresh(Some(field.clone()));
    assert_eq!(seeded_field, field, "seeded, the cells are the live ones");
    keep("region-fresh-unseeded", &unseeded);
    keep("region-fresh-seeded", &seeded);
    assert!(
        at(&seeded, -2.0) < at(&unseeded, -2.0) - 8.0,
        "the seeded renderer draws the wake: {:.1} against {:.1} at rest",
        at(&seeded, -2.0),
        at(&unseeded, -2.0)
    );
    // A seed for a region the scene does not hold at that place starts
    // nothing: a moved region starts at rest.
    let mut moved = field;
    moved.regions[0].center[0] += 1.0;
    assert_eq!(emptiest(&fresh(Some(moved)).0), 1.0);
}

#[test]
fn a_rebaseline_keeps_the_regions_cells() {
    // The binary's one device (`support::gpu`): a device made beside
    // tests that draw can crash the Vulkan loader (#11065).
    let gpu = gpu();
    let driver = SceneDriver::new(gpu.clone(), options());
    if driver.settings_readout().volumetric_fog.is_some() {
        return;
    }
    let state = |step: u64| SceneState {
        elapsed_seconds: step as f64 / RATE,
        world_revision: step,
        step,
        held: false,
        lead: None,
        preparation_batch: 0,
    };
    let call = |ops: Vec<RenderDiff>, step| CommittedCall {
        changes: vec![
            SceneChange::Frame(RenderFrameDiff {
                publication: None,
                ops,
            }),
            SceneChange::ViewComposition(composition_of(camera([0.0, 1.5, 0.0], 0.0, 0.0))),
        ],
        resources: Box::new(NoResources),
        entities: Default::default(),
        state: state(step),
    };
    let target = OffscreenTarget::new(&gpu, WIDTH, HEIGHT, 1);
    driver.apply(call(vec![region()], 0));
    for frame in 0..48u32 {
        let time = f64::from(frame) / RATE;
        driver.apply(call(
            vec![stirrer(-6.0 + 4.0 * time as f32)],
            u64::from(frame),
        ));
        driver.draw(|renderer, _| {
            renderer.set_presentation_time(time);
            renderer.render_view_composition(&target, time);
        });
    }
    let field = driver.fog_field();
    assert!(emptiest(&field) < 0.2, "the wake is in the cells");
    // The baseline holds the region but no stirrer: a new renderer, built
    // from it, starts from the old one's cells.
    driver.rebaseline(call(vec![region()], 48));
    driver.draw(|renderer, _| {
        renderer.set_presentation_time(48.0 / RATE);
        renderer.render_view_composition(&target, 48.0 / RATE);
    });
    assert_eq!(driver.fog_field(), field);
}
