//! Projection cost for a one-chunk edit in a 256-chunk voxel scene (#8797).
//!
//! Times only `VoxelRenderProjector::project`; the voxel edit runs outside
//! the timed region. Run with `--release`.

use std::{collections::BTreeMap, time::Instant};

use engine_spatial::{MaterialVoxel, VoxelCollisionScene, VoxelEdit, VoxelEditService};
use render_model::{MaterialUvStrategy, RenderDiff, RenderMaterialDescriptor, Transform};
use render_projection::{voxel_material_id, VoxelProjectionInstance, VoxelRenderProjector};

const CHUNK_SIZE: u32 = 8;
const CHUNKS_PER_SIDE: i64 = 16;
const ITERATIONS: usize = 200;
/// An empty interior cell of chunk [5, 0, 5], away from every chunk boundary.
const EDITED_CELL: [i64; 3] = [44, 4, 44];

fn main() {
    let scene = fixture();
    assert_eq!(scene.mesh_chunks().len(), 256);
    let materials = BTreeMap::from([(1, material())]);
    let mut scene = scene;
    let mut projector = VoxelRenderProjector::new();
    let baseline = project(&mut projector, &scene, &materials);

    let mut unchanged = Vec::with_capacity(ITERATIONS);
    for _ in 0..ITERATIONS {
        let started = Instant::now();
        let result = project(&mut projector, &scene, &materials);
        unchanged.push(started.elapsed().as_nanos());
        assert!(result.frame.ops.is_empty());
    }

    let mut edited = Vec::with_capacity(ITERATIONS);
    let mut replacements = 0;
    for iteration in 0..ITERATIONS {
        let edit = if iteration % 2 == 0 {
            VoxelEdit::Set {
                address: EDITED_CELL,
                material_slot: 1,
            }
        } else {
            VoxelEdit::Clear {
                address: EDITED_CELL,
            }
        };
        let receipt = VoxelEditService::apply(&mut scene, &[edit]).expect("one-cell edit");
        assert_eq!(receipt.dirty_mesh_chunks, vec![[5, 0, 5]]);
        let started = Instant::now();
        let result = project(&mut projector, &scene, &materials);
        edited.push(started.elapsed().as_nanos());
        replacements = result
            .frame
            .ops
            .iter()
            .filter(|operation| matches!(operation, RenderDiff::ReplaceMeshPayload { .. }))
            .count();
    }

    // A fresh attachment projects the edited world from a new projector.
    if let Ok(path) = std::env::var("VOXEL_BASELINE_FRAMES") {
        let attached = project(&mut VoxelRenderProjector::new(), &scene, &materials);
        let frames = serde_json::json!({ "initial": baseline.frame, "attached": attached.frame });
        std::fs::write(path, serde_json::to_vec(&frames).expect("frame JSON"))
            .expect("write baseline frames");
    }

    println!("chunks,baseline_ops,scenario,median_us,p95_us,replacements");
    let (median, p95) = summary(&mut unchanged);
    println!(
        "256,{},unchanged_refresh,{median:.1},{p95:.1},0",
        baseline.frame.ops.len()
    );
    let (median, p95) = summary(&mut edited);
    println!(
        "256,{},one_chunk_edit,{median:.1},{p95:.1},{replacements}",
        baseline.frame.ops.len()
    );
}

fn summary(samples: &mut [u128]) -> (f64, f64) {
    samples.sort_unstable();
    let at = |fraction: f64| {
        let index = ((samples.len() - 1) as f64 * fraction).round() as usize;
        samples[index] as f64 / 1_000.0
    };
    (at(0.5), at(0.95))
}

/// 16 × 16 columns of 8³ chunks, one chunk high: 256 mesh chunks.
fn fixture() -> VoxelCollisionScene {
    let side = CHUNKS_PER_SIDE * i64::from(CHUNK_SIZE);
    VoxelCollisionScene::from_material_voxels(
        1.0,
        CHUNK_SIZE,
        (0..side).flat_map(|x| {
            (0..side).flat_map(move |z| {
                let height = 2 + ((x * 17 + z * 31 + (x ^ z)) % 4);
                (0..=height).map(move |y| MaterialVoxel {
                    state: 0,
                    address: [x, y, z],
                    material_slot: 1,
                })
            })
        }),
    )
    .expect("256-chunk terrain fixture")
}

fn project(
    projector: &mut VoxelRenderProjector,
    scene: &VoxelCollisionScene,
    materials: &BTreeMap<u16, RenderMaterialDescriptor>,
) -> render_projection::VoxelProjectionResult {
    projector
        .project(
            &[VoxelProjectionInstance {
                instance_id: "terrain".to_string(),
                asset_id: "voxel-object/terrain".to_string(),
                transform: Transform::IDENTITY,
                scene,
            }],
            materials,
        )
        .expect("terrain projection")
}

fn material() -> RenderMaterialDescriptor {
    RenderMaterialDescriptor {
        texture_transform: None,
        stochastic_tiling: None,
        terrain_layers: None,
        shader: None,
        id: voxel_material_id(1),
        color: [0.45, 0.7, 0.35, 1.0],
        texture: None,
        roughness: 1.0,
        metalness: 0.0,
        texture_tint: [1.0; 4],
        emission_color: [0.0; 3],
        emission_intensity: 0.0,
        uv_strategy: MaterialUvStrategy::Flat,
        alpha_mode: Default::default(),
        double_sided: false,
        voxel_surface: None,
        normal_map: None,
        triplanar: None,
        emission_map: Default::default(),
        occlusion_map: Default::default(),
        unlit: false,
        flat_shading: false,
        wind: None,
        water: None,
        translucent_shadow: false,
    }
}
