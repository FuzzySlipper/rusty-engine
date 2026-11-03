//! Scatter copies in the ray scene (`ray_scene.rs`, `ray_instances.wgsl`):
//! the copies within the ray reach of the eye are TLAS instances written on
//! the GPU, leaning in the wind as the world pass leans them; copies beyond
//! the reach are not, until the reach takes them in.

mod common;

use common::*;
use render_model::*;
use render_wgpu::{Gpu, ProbeHit, ProbeRay, RaySceneReadout, Renderer, RendererOptions};

const POST: &str = "post";
const PATCH: u64 = 10;
/// Copies along x, 2 m apart, 6 m ahead; and one 300 m ahead.
const NEAR: [f32; 3] = [-2.0, 0.0, 2.0];
const FAR_Z: f32 = -300.0;
const ROW_Z: f32 = -6.0;
/// The wind blows along +x without gusts, the posts bend this much: at
/// height h above its origin a post leans `STRENGTH * BEND * h` along x.
const STRENGTH: f32 = 0.5;
const BEND: f32 = 1.0;
/// The fixture camera's focal length in pixels: 90 / tan(30°).
const FOCAL_PIXELS: f32 = 155.884_57;

/// A post standing on its origin, 0.3 m across and 1 m tall.
fn post() -> MeshPayloadDescriptor {
    let mut payload = cube();
    let MeshPayloadSource::Inline { positions, .. } = &mut payload.source else {
        unreachable!()
    };
    for point in positions.as_chunks_mut::<3>().0 {
        point[0] *= 0.3;
        point[1] += 0.5;
        point[2] *= 0.3;
    }
    payload.bounds = MeshBoundsDescriptor {
        min: [-0.15, 0.0, -0.15],
        max: [0.15, 1.0, 0.15],
    };
    payload
}

fn copy(x: f32, z: f32) -> ScatterInstance {
    ScatterInstance {
        translation: [x, 0.0, z],
        rotation: [0.0, 0.0, 0.0, 1.0],
        scale: 1.0,
        tint: [1.0; 3],
        rank: 0.5,
    }
}

fn scene(harness: &mut Harness, wind: bool) {
    let mut ops = coloured_mesh(POST, post(), [0.9, 0.9, 0.9, 1.0]);
    ops.push(RenderDiff::SetBackgroundColor {
        color: [0.0, 0.0, 0.0, 1.0],
    });
    if let Some(RenderDiff::DefineMaterial { material }) = ops.first_mut() {
        material.wind = Some(MaterialWindDescriptor {
            bend: BEND,
            flutter: 0.0,
        });
    }
    ops.push(RenderDiff::SetWind {
        wind: wind.then_some(WindDescriptor {
            direction: [1.0, 0.0],
            strength: STRENGTH,
            gust: 0.0,
        }),
    });
    let mut copies: Vec<ScatterInstance> = NEAR.iter().map(|x| copy(*x, ROW_Z)).collect();
    copies.push(copy(0.0, FAR_Z));
    ops.push(RenderDiff::CreateScatterPatch {
        handle: RenderHandle::new(PATCH),
        parent: None,
        patch: ScatterPatchDescriptor {
            asset: format!("mesh/{POST}"),
            material_overrides: Vec::new(),
            instances: copies,
            fade: ScatterFade::default(),
            shadow_casting: ShadowCasting::Cast,
            lods: Vec::new(),
            lod_band: 0.0,
            thinning: Default::default(),
        },
    });
    harness.apply(ops);
}

fn frame(harness: &mut Harness) {
    harness.renderer.set_animation_time(3.0);
    harness.single(&camera("eye", [0.0, 0.5, 0.0], 0.0, 0.0));
}

fn readout(renderer: &Renderer) -> RaySceneReadout {
    renderer
        .gpu_readout()
        .ray_scene
        .expect("the ray scene is asked for")
}

/// A ray along -z at height `y` and across offset `x`, from the eye's side
/// of the row.
fn along(x: f32, y: f32, z: f32) -> ProbeRay {
    ProbeRay {
        origin: [x, y, z],
        direction: [0.0, 0.0, -1.0],
        reach: 1000.0,
        mask: 0xff,
    }
}

fn hit_patch(hit: Option<ProbeHit>, what: &str) -> f32 {
    let hit = hit.unwrap_or_else(|| panic!("{what}: no hit"));
    assert_eq!(hit.node, Some(RenderHandle::new(PATCH)), "{what}");
    hit.distance
}

#[test]
fn copies_within_the_reach_lean_in_the_wind_and_the_reach_takes_in_more() {
    let gpu = Gpu::headless_with_ray_queries().expect("a device");
    if gpu.is_software() {
        // The software adapter refuses the ray scene (`ray_scene.rs`).
        return;
    }
    let mut harness = Harness::on(gpu, RendererOptions::default());
    harness.renderer.set_ray_scene(true);
    scene(&mut harness, true);
    // The counters read back a frame later.
    for _ in 0..3 {
        frame(&mut harness);
    }
    let near = readout(&harness.renderer);
    assert!(near.refused.is_none(), "{near:?}");
    assert_eq!(near.reach, 100.0, "the default reach");
    assert_eq!(near.copies_in_reach, NEAR.len() as u32, "{near:?}");
    assert_eq!(near.copy_instances, NEAR.len() as u32, "one part each");
    assert_eq!(near.dropped_instances, 0);

    // 0.9 m up, a post leans 0.45 m along +x: the ray at its leaning centre
    // hits its front face (the face at z + 0.15); the one at its standing
    // centre passes it.
    let lean = STRENGTH * BEND * 0.9;
    let hits = harness.renderer.trace_rays(&[
        along(NEAR[1] + lean, 0.9, 0.0),
        along(NEAR[1], 0.9, 0.0),
        // At its foot it stands where it was.
        along(NEAR[1], 0.05, 0.0),
        // The far copy is beyond the reach.
        along(0.0, 0.5, ROW_Z - 1.0),
    ]);
    let face = -(ROW_Z + 0.15);
    assert!((hit_patch(hits[0], "the leaning post") - face).abs() < 1e-3);
    assert_eq!(hits[1], None, "where the post stood");
    // The world pass draws it there: level with the eye at 0.9 m, the middle
    // row shows the post around x 0.45 at its face's depth, not around 0.
    let picture = harness.single(&camera("level", [0.0, 0.9, 0.0], 0.0, 0.0));
    let column = |x: f32| (WIDTH as f32 / 2.0 + FOCAL_PIXELS * x / face).round() as u32;
    let lit = |x: f32| {
        pixel(&picture, WIDTH, column(x), HEIGHT / 2)[..3]
            .iter()
            .any(|c| *c > 40)
    };
    assert!(lit(NEAR[1] + lean), "the drawn post leans");
    assert!(!lit(NEAR[1]), "nothing drawn where it stood");
    assert!((hit_patch(hits[2], "the post's foot") - face).abs() < 1e-3);
    assert_eq!(hits[3], None, "beyond the reach");

    // A reach of 500 m takes the far copy in; its instance leans too.
    harness.renderer.set_ray_reach(Some(500.0));
    for _ in 0..3 {
        frame(&mut harness);
    }
    let far = readout(&harness.renderer);
    assert_eq!(far.reach, 500.0);
    assert_eq!(far.copies_in_reach, NEAR.len() as u32 + 1, "{far:?}");
    let hits = harness
        .renderer
        .trace_rays(&[along(lean, 0.9, ROW_Z - 1.0), along(0.0, 0.9, ROW_Z - 1.0)]);
    let far_face = -(FAR_Z + 0.15) + (ROW_Z - 1.0);
    assert!((hit_patch(hits[0], "the far post") - far_face).abs() < 1e-2);
    assert_eq!(hits[1], None, "where the far post stood");

    // Without wind the posts stand.
    harness.apply(vec![RenderDiff::SetWind { wind: None }]);
    frame(&mut harness);
    let hits = harness.renderer.trace_rays(&[along(NEAR[1], 0.9, 0.0)]);
    assert!((hit_patch(hits[0], "the standing post") - face).abs() < 1e-3);
}
