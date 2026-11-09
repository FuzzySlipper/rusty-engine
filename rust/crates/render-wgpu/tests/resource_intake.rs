//! What the renderer's intake of a location-sized batch costs on the thread
//! that commits it (#10693): PNG textures, the materials sampling them,
//! packed static meshes and their instances, as a product's location
//! appearance publishes them in one call. Ignored; run with
//! `cargo test --release -p render-wgpu --test resource_intake -- --ignored --nocapture`.

mod support;

use std::time::Instant;

use render_model::*;
use render_wgpu::{encode_png, RendererOptions};
use support::*;

/// A location's batch: texture sizes, meshes and their vertex counts.
struct Batch {
    name: &'static str,
    textures: Vec<(u32, u32)>,
    meshes: Vec<u32>,
    instances: u64,
}

fn roadside() -> Batch {
    let mut textures = vec![(64, 64); 30];
    textures.extend([(64, 128); 10]);
    textures.extend([(128, 128); 6]);
    textures.extend([(256, 256); 2]);
    let mut meshes = vec![600; 20];
    meshes.push(40_000);
    Batch {
        name: "roadside",
        textures,
        meshes,
        instances: 60,
    }
}

fn city() -> Batch {
    let mut textures = vec![(64, 64); 200];
    textures.extend([(64, 128); 80]);
    textures.extend([(128, 128); 50]);
    textures.extend([(256, 256); 20]);
    let mut meshes = vec![1_200; 140];
    meshes.push(150_000);
    Batch {
        name: "city",
        textures,
        meshes,
        instances: 600,
    }
}

/// Paletted-looking art: blocks of a few colours with a little noise, so
/// it compresses as Daggerfall's textures do rather than as noise.
fn art(width: u32, height: u32, seed: u32) -> Vec<u8> {
    let mut state = seed.wrapping_mul(2_654_435_761).max(1);
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        state
    };
    let palette: Vec<[u8; 4]> = (0..8)
        .map(|_| {
            let v = next();
            [v as u8, (v >> 8) as u8, (v >> 16) as u8, 255]
        })
        .collect();
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            let block = ((x / 4) * 7 + (y / 4) * 13 + seed) as usize;
            let mut texel = palette[block % palette.len()];
            if next() % 5 == 0 {
                texel = palette[(next() as usize) % palette.len()];
            }
            rgba.extend_from_slice(&texel);
        }
    }
    rgba
}

/// A grid of `vertices` (about) with uvs, its quads split between `slots`.
fn grid(vertices: u32, slots: u16, offset: f32) -> MeshPayloadDescriptor {
    let side = (vertices as f32).sqrt().ceil() as u32;
    let (mut positions, mut normals, mut uvs, mut indices) = (vec![], vec![], vec![], vec![]);
    for z in 0..side {
        for x in 0..side {
            positions.extend_from_slice(&[
                offset + x as f32 * 0.1,
                ((x * 31 + z * 17) % 7) as f32 * 0.01,
                z as f32 * 0.1,
            ]);
            normals.extend_from_slice(&[0.0, 1.0, 0.0]);
            uvs.extend_from_slice(&[x as f32 / 4.0, z as f32 / 4.0]);
        }
    }
    let quads_per_slot = ((side - 1) * (side - 1)).div_ceil(u32::from(slots));
    let mut groups = Vec::new();
    let mut quad = 0;
    for slot in 0..slots {
        let mut count = 0;
        while quad < (side - 1) * (side - 1) && count < quads_per_slot * 6 {
            let (x, z) = (quad % (side - 1), quad / (side - 1));
            let a = z * side + x;
            indices.extend_from_slice(&[a, a + side, a + 1, a + 1, a + side, a + side + 1]);
            count += 6;
            quad += 1;
        }
        groups.push((slot, count));
    }
    payload(positions, normals, uvs, indices, &groups)
}

fn ms(since: Instant) -> f64 {
    since.elapsed().as_secs_f64() * 1000.0
}

/// A running world: a ground and `locations` locations already drawn.
fn world(locations: u32) -> Harness {
    let mut harness = Harness::new(RendererOptions::default());
    harness.apply(vec![
        RenderDiff::DefineMaterial {
            material: material("material/ground", [0.4, 0.5, 0.3, 1.0], None),
        },
        static_mesh(
            "mesh/ground",
            box_mesh([-50.0, -1.0, -50.0], [50.0, 0.0, 50.0], |_| 0),
            "material/ground",
        ),
        instance(1, None, "mesh/ground", Transform::IDENTITY),
    ]);
    harness.render(&camera([0.0, 5.0, 20.0], 0.0, -10.0));
    for index in 0..locations {
        intake(&mut harness, &roadside(), 100 + index, false, false);
    }
    harness
}

/// Applies `batch` as one product call does; `deferred` defers its textures
/// and meshes, and then takes them in as 60 Hz updates would.
fn intake(harness: &mut Harness, batch: &Batch, round: u32, report: bool, deferred: bool) {
    let view = camera([0.0, 5.0, 20.0], 0.0, -10.0);
    let prefix = format!("loc{round}");
    let mut texture_ops = Vec::new();
    let mut png_bytes = 0usize;
    let mut decode_alone = 0.0;
    for (index, (width, height)) in batch.textures.iter().enumerate() {
        let id = format!("texture/{prefix}-{index}");
        let rgba = art(*width, *height, index as u32 + round * 1000);
        let png = encode_png(*width, *height, &rgba).expect("encode");
        png_bytes += png.len();
        let started = Instant::now();
        let _ = render_wgpu::decode_png_rgba(&png).expect("decode");
        decode_alone += ms(started);
        let mut texture =
            harness
                .resources
                .texture(&id, *width, *height, &rgba, TextureWrap::Repeat);
        texture.deferred = deferred;
        texture_ops.push(RenderDiff::DefineTexture { texture });
    }
    let material_ops: Vec<RenderDiff> = (0..batch.textures.len())
        .map(|index| RenderDiff::DefineMaterial {
            material: material(
                &format!("material/{prefix}-{index}"),
                [1.0; 4],
                Some(&format!("texture/{prefix}-{index}")),
            ),
        })
        .collect();
    let slots = 6u16;
    let payloads: Vec<MeshPayloadDescriptor> = batch
        .meshes
        .iter()
        .enumerate()
        .map(|(index, vertices)| grid(*vertices, slots, index as f32))
        .collect();
    let packed = pack_mesh_resources(&payloads, 64 << 20).expect("pack meshes");
    for resource in &packed.resources {
        harness
            .resources
            .insert(&resource.resource, resource.bytes.clone());
    }
    let vertices: u32 = batch.meshes.iter().sum();
    let mesh_ops: Vec<RenderDiff> = packed
        .payloads
        .into_iter()
        .enumerate()
        .map(|(index, payload)| RenderDiff::DefineStaticMesh {
            asset: StaticMeshAsset {
                asset: format!("mesh/{prefix}-{index}"),
                payload,
                material_slots: (0..slots)
                    .map(|slot| MeshMaterialSlot {
                        slot,
                        material: format!(
                            "material/{prefix}-{}",
                            (index * slots as usize + slot as usize) % batch.textures.len()
                        ),
                    })
                    .collect(),
                collision: MeshCollisionPolicy::VisualOnly,
                deferred,
            },
        })
        .collect();
    let instance_ops: Vec<RenderDiff> = (0..batch.instances)
        .map(|index| {
            instance(
                u64::from(round) * 10_000 + 100 + index,
                None,
                &format!("mesh/{prefix}-{}", index as usize % batch.meshes.len()),
                transform(
                    [
                        (index % 20) as f32 * 3.0 - 30.0,
                        0.0,
                        -((index / 20) as f32) * 3.0,
                    ],
                    0.0,
                    [1.0; 3],
                ),
            )
        })
        .collect();

    // Each kind, through the retained model as the runtime applies it, in
    // the one call that publishes them.
    harness.renderer.set_preparation_batch(u64::from(round) + 1);
    let mut row = Vec::new();
    let mut call = 0.0;
    for (kind, ops) in [
        ("textures", texture_ops),
        ("materials", material_ops),
        ("meshes", mesh_ops),
        ("instances", instance_ops),
    ] {
        let count = ops.len();
        let frame = RenderFrameDiff {
            publication: None,
            ops,
        };
        let started = Instant::now();
        let delta = harness.world.apply(frame).expect("retained model");
        let world = ms(started);
        let started = Instant::now();
        let issues = harness.renderer.apply(&delta, &harness.resources);
        let renderer = ms(started);
        assert!(issues.is_empty(), "{issues:?}");
        call += world + renderer;
        row.push(format!("{kind} {count} {:.1}", world + renderer));
    }
    // Later updates take in what workers finished, until the batch is drawn.
    let published = Instant::now();
    let (mut updates, mut worst_take, mut takes) = (0, 0.0f64, 0.0);
    while deferred && harness.renderer.preparation_readout().pending > 0 {
        std::thread::sleep(std::time::Duration::from_millis(16));
        updates += 1;
        let started = Instant::now();
        let issues = harness.renderer.take_prepared();
        let took = ms(started);
        assert!(issues.is_empty(), "{issues:?}");
        worst_take = worst_take.max(took);
        takes += took;
    }
    let ready = ms(published);
    let started = Instant::now();
    harness.render(&view);
    let first_draw = ms(started);
    if !report {
        return;
    }
    println!(
        "{} {} round {round}: {} textures ({} KiB png, decode alone {decode_alone:.1} ms), {} meshes ({vertices} vertices), {} instances",
        if deferred { "deferred" } else { "immediate" },
        batch.name,
        batch.textures.len(),
        png_bytes / 1024,
        batch.meshes.len(),
        batch.instances
    );
    println!("  the call: {call:.1} ms ({})", row.join(", "));
    if deferred {
        println!(
            "  drawn after {updates} updates ({ready:.0} ms): worst take-in {worst_take:.1} ms, all take-ins {takes:.1} ms"
        );
    }
    println!("  first draw {first_draw:.1} ms");
}

#[test]
#[ignore = "a timing bench: run explicitly with --release"]
fn location_intake_cost() {
    let adapter = gpu().adapter_summary().name;
    println!("adapter: {adapter}");
    for locations in [0, 40] {
        println!("-- a world of {locations} locations");
        for deferred in [false, true] {
            let mut harness = world(locations);
            for round in 0..2 {
                intake(&mut harness, &roadside(), round * 2, true, deferred);
                intake(&mut harness, &city(), round * 2 + 1, true, deferred);
            }
        }
    }
}
