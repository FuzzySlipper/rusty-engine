//! A city of static meshes (#10697): 400 meshes of five material slots each,
//! drawing 120 shared materials, placed 1,414 times, under a sun that casts.
//! Every static mesh draws from the static pool (`mesh_pool.rs`), which lays
//! itself out anew once released meshes hold most of it.
//!
//! The draw cost bench is ignored; run it with
//! `cargo test --release -p render-wgpu --test static_batching -- --ignored --nocapture`.

mod support;

use std::time::Instant;

use render_host_contracts::{RendererCameraProjection, RendererCompositionCamera};
use render_model::*;
use render_wgpu::{OffscreenTarget, RendererOptions};
use support::*;

const MESHES: usize = 400;
const SLOTS: u16 = 5;
const MATERIALS: usize = 120;
const TEXTURES: usize = 30;
const PLACED: u64 = 1_414;
/// Metres between placed models on the city's grid.
const SPACING: f32 = 9.0;

fn city(options: RendererOptions) -> Harness {
    let mut harness = Harness::new(options);
    let ops = city_ops(&mut harness.resources);
    harness.apply(ops);
    harness
}

fn box_slot(normal: [f32; 3]) -> u16 {
    // Walls, roof and floor in five slots.
    match normal {
        [x, _, _] if x > 0.5 => 0,
        [x, _, _] if x < -0.5 => 1,
        [_, y, _] if y > 0.5 => 2,
        [_, y, _] if y < -0.5 => 3,
        _ => 4,
    }
}

/// Mesh `index` of the city, `grown` metres taller.
fn building(index: usize, grown: f32) -> RenderDiff {
    let (width, height, depth) = (
        3.0 + (index % 6) as f32,
        3.0 + (index % 5) as f32 * 2.0 + grown,
        3.0 + (index % 3) as f32 * 2.0,
    );
    RenderDiff::DefineStaticMesh {
        asset: StaticMeshAsset {
            asset: format!("mesh/{index}"),
            payload: box_mesh(
                // Clear of the ground and of each other: no coplanar faces,
                // whose depth ties draw order would settle.
                [-width * 0.5, 0.01, -depth * 0.5],
                [width * 0.5, height, depth * 0.5],
                box_slot,
            ),
            material_slots: (0..SLOTS)
                .map(|slot| MeshMaterialSlot {
                    slot,
                    material: format!(
                        "material/{}",
                        (index * 7 + usize::from(slot) * 31) % MATERIALS
                    ),
                })
                .collect(),
            collision: MeshCollisionPolicy::VisualOnly,
            deferred: false,
        },
    }
}

/// The mesh placed model `index` draws.
fn placed_mesh(index: u64) -> usize {
    (index as usize * 13) % MESHES
}

fn city_ops(resources: &mut Resources) -> Vec<RenderDiff> {
    let mut ops = vec![
        RenderDiff::CreateLight {
            handle: RenderHandle::new(1),
            parent: None,
            light: LightDescriptor::Directional {
                color: [1.0, 0.95, 0.85],
                intensity: 2.5,
                enabled: true,
                direction: [0.4, -0.7, 0.3],
                range: Some(400.0),
                shadow_intent: LightShadowIntent::Requested,
                shadow: Default::default(),
            },
        },
        RenderDiff::DefineMaterial {
            material: material("material/ground", [0.4, 0.5, 0.3, 1.0], None),
        },
        static_mesh(
            "mesh/ground",
            box_mesh([-400.0, -1.0, -400.0], [400.0, 0.0, 400.0], |_| 0),
            "material/ground",
        ),
        instance(2, None, "mesh/ground", Transform::IDENTITY),
    ];
    for index in 0..TEXTURES {
        let shade = 120 + (index * 4) as u8;
        let rgba = checker(32, [shade, 90, 60, 255], [60, shade, 90, 255]);
        let texture = resources.texture(
            &format!("texture/{index}"),
            32,
            32,
            &rgba,
            TextureWrap::Repeat,
        );
        ops.push(RenderDiff::DefineTexture { texture });
    }
    for index in 0..MATERIALS {
        let tint = 0.6 + (index % 5) as f32 * 0.1;
        ops.push(RenderDiff::DefineMaterial {
            material: material(
                &format!("material/{index}"),
                [tint, 1.0, tint, 1.0],
                Some(&format!("texture/{}", index % TEXTURES)),
            ),
        });
    }
    ops.extend((0..MESHES).map(|index| building(index, 0.0)));
    let side = (PLACED as f32).sqrt().ceil() as u64;
    for index in 0..PLACED {
        let (x, z) = ((index % side) as f32, (index / side) as f32);
        ops.push(instance(
            100 + index,
            None,
            &format!("mesh/{}", placed_mesh(index)),
            transform(
                [
                    (x - side as f32 * 0.5) * SPACING,
                    0.0,
                    (z - side as f32 * 0.5) * SPACING,
                ],
                (index % 4) as f32 * 90.0,
                [1.0; 3],
            ),
        ));
    }
    ops
}

/// A street-level view from the city's edge across it, turning with `frame`.
fn view(frame: u32) -> RendererCompositionCamera {
    let mut camera = camera([0.0, 8.0, 190.0], f64::from(frame) * 0.25 - 15.0, -5.0);
    camera.projection = RendererCameraProjection::Perspective {
        fov_y_degrees: 60.0,
        near: 0.1,
        far: 1500.0,
    };
    camera
}

fn median(values: &mut [f64]) -> f64 {
    values.sort_by(f64::total_cmp);
    values[values.len() / 2]
}

#[test]
#[ignore = "a timing bench: run explicitly with --release"]
fn city_draw_cost() {
    let adapter = gpu().adapter_summary().name;
    println!("adapter: {adapter}");
    for (name, options) in [
        (
            "shadows",
            RendererOptions {
                default_world_lights: false,
                shadows: true,
                ..RendererOptions::default()
            },
        ),
        (
            "no shadows",
            RendererOptions {
                default_world_lights: false,
                ..RendererOptions::default()
            },
        ),
    ] {
        let mut harness = city(options);
        harness.target = OffscreenTarget::new(&harness.gpu, 1280, 720, 4);
        // Pipelines compile and lists settle.
        for frame in 0..10 {
            harness.render(&view(frame));
        }
        let (mut turning, mut still, mut lists) = (Vec::new(), Vec::new(), Vec::new());
        let mut stats = None;
        for frame in 0..120 {
            let started = Instant::now();
            let drawn = harness
                .renderer
                .render_offscreen(&view(10 + frame), &harness.target);
            turning.push(started.elapsed().as_secs_f64() * 1000.0);
            lists.push(f64::from(drawn.cpu_batch_us) / 1000.0);
            stats = Some(drawn);
            // Wait for the GPU outside the timing.
            harness.target.read_rgba(&harness.gpu);
        }
        let mut pixels = Vec::new();
        for _ in 0..60 {
            let started = Instant::now();
            harness
                .renderer
                .render_offscreen(&view(130), &harness.target);
            still.push(started.elapsed().as_secs_f64() * 1000.0);
            pixels = harness.target.read_rgba(&harness.gpu);
        }
        // The still frame, for comparing builds.
        let path = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("static_batching_{}.png", name.replace(' ', "_")));
        let png = render_wgpu::encode_png(1280, 720, &pixels).expect("encode the still frame");
        std::fs::write(&path, png).expect("write the still frame");
        println!("{name}: still frame {}", path.display());
        let stats = stats.expect("frames drawn");
        println!(
            "{name}: turning {:.2} ms (lists {:.2} ms), still {:.2} ms (median CPU per frame); draws {}, shadow draws {} in {} layers, draw calls {}, pipeline binds {}, instances {}",
            median(&mut turning),
            median(&mut lists),
            median(&mut still),
            stats.draws,
            stats.shadow_draws,
            stats.shadow_layers,
            stats.draw_calls,
            stats.pipeline_binds,
            stats.instances,
        );
    }
}

fn differing(a: &[u8], b: &[u8]) -> usize {
    a.as_chunks::<4>()
        .0
        .iter()
        .zip(b.as_chunks::<4>().0)
        .filter(|(a, b)| a != b)
        .count()
}

#[test]
fn a_city_draws_as_a_fresh_renderer_does_after_its_pool_is_laid_out_anew() {
    let mut harness = city(RendererOptions {
        default_world_lights: false,
        shadows: true,
        ..RendererOptions::default()
    });
    let view = view(40);
    let (stats, before) = harness.render(&view);
    assert_eq!(
        differing(&before, &harness.render_fresh(&view).1),
        0,
        "the city draws as a fresh renderer draws it"
    );
    assert!(
        stats.draws > 1_000,
        "every material slot of every mesh in view draws: {stats:?}"
    );
    // Three quarters of the meshes go, which lays the pool out anew; one
    // that stays is defined again, taller.
    let gone = |mesh: usize| mesh < MESHES * 3 / 4;
    let mut ops: Vec<RenderDiff> = (0..PLACED)
        .filter(|index| gone(placed_mesh(*index)))
        .map(|index| RenderDiff::Destroy {
            handle: RenderHandle::new(100 + index),
        })
        .collect();
    ops.extend(
        (0..MESHES)
            .filter(|mesh| gone(*mesh))
            .map(|mesh| RenderDiff::ReleaseStaticMesh {
                asset: format!("mesh/{mesh}"),
            }),
    );
    harness.apply(ops);
    harness.apply(vec![building(MESHES - 1, 6.0)]);
    let after = harness.render(&view).1;
    assert_eq!(
        differing(&after, &harness.render_fresh(&view).1),
        0,
        "what stays draws from its new place in the pool"
    );
    assert!(
        differing(&after, &before) > 1_000,
        "most of the city is gone"
    );
}
