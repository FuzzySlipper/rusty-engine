use std::{collections::BTreeMap, path::PathBuf, time::Instant};

use engine_spatial::{
    MaterialVoxel, SurfaceMeshOptions, SurfaceMode, VoxelCollisionScene, VoxelEdit,
    VoxelEditService, VoxelEditTransaction,
};
use render_model::{MaterialUvStrategy, RenderDiff, RenderMaterialDescriptor, Transform};
use render_projection::{VoxelProjectionInstance, VoxelRenderProjector};

fn main() {
    println!("mode,scenario,elapsed_us,encoded_bytes,ops,replacements,destroys,rebuilt,reused");
    for mode in [
        SurfaceMode::GreedyCubes,
        SurfaceMode::MarchingCubes,
        SurfaceMode::DualContouring,
    ] {
        measure(
            mode,
            "one_cell",
            &[VoxelEdit::Clear {
                address: [31, 4, 31],
            }],
        );
        let bounded = (28..32)
            .flat_map(|x| {
                (28..32).flat_map(move |z| {
                    (5..9).map(move |y| VoxelEdit::Set {
                        address: [x, y, z],
                        material_slot: 1,
                    })
                })
            })
            .collect::<Vec<_>>();
        measure(mode, "bounded_4x4x4", &bounded);
    }
}

fn measure(mode: SurfaceMode, scenario: &str, edits: &[VoxelEdit]) {
    let options = SurfaceMeshOptions {
        mode,
        ..SurfaceMeshOptions::default()
    };
    let materials = BTreeMap::from([(1, material())]);

    let full_scene = fixture(options);
    let mut full_projector = VoxelRenderProjector::with_publication_stream(format!(
        "benchmark:{}:{scenario}:full",
        mode.as_str()
    ));
    let full_base = project(&mut full_projector, &full_scene, "terrain-v1", &materials);
    let full_started = Instant::now();
    let full_candidate = edited_fixture(options, edits);
    let full = project(
        &mut full_projector,
        &full_candidate,
        "terrain-v2",
        &materials,
    );
    print_row(
        mode,
        &format!("{scenario}_whole_replacement"),
        full_started.elapsed().as_micros(),
        &full.frame,
        full_candidate.mesh_chunks().len(),
        0,
    );
    write_frame_pair(mode, scenario, "whole", &full_base.frame, &full.frame);

    let mut scene = fixture(options);
    let mut projector = VoxelRenderProjector::with_publication_stream(format!(
        "benchmark:{}:{scenario}:incremental",
        mode.as_str()
    ));
    let incremental_base = project(&mut projector, &scene, "terrain-v1", &materials);
    let incremental_started = Instant::now();
    let expected_revision = scene.source_revision();
    let receipt = VoxelEditService::apply(
        &mut scene,
        VoxelEditTransaction {
            expected_revision,
            edits,
        },
    )
    .expect("representative edit must remain within production limits");
    let incremental = project(&mut projector, &scene, "terrain-v1", &materials);
    print_row(
        mode,
        &format!("{scenario}_incremental"),
        incremental_started.elapsed().as_micros(),
        &incremental.frame,
        receipt.rebuilt_mesh_chunks,
        receipt.reused_mesh_chunks,
    );
    write_frame_pair(
        mode,
        scenario,
        "incremental",
        &incremental_base.frame,
        &incremental.frame,
    );
}

fn write_frame_pair(
    mode: SurfaceMode,
    scenario: &str,
    path: &str,
    base: &render_model::RenderFrameDiff,
    update: &render_model::RenderFrameDiff,
) {
    let Ok(directory) = std::env::var("VOXEL_BENCH_FRAME_DIR") else {
        return;
    };
    let output = PathBuf::from(directory).join(format!("{}-{scenario}-{path}.json", mode.as_str()));
    let bytes = serde_json::to_vec(&serde_json::json!({
        "base": base,
        "update": update,
    }))
    .expect("benchmark frame pair JSON");
    std::fs::write(output, bytes).expect("write benchmark frame pair");
}

fn fixture(options: SurfaceMeshOptions) -> VoxelCollisionScene {
    VoxelCollisionScene::from_material_voxels_with_mesh_options(
        1.0,
        16,
        fixture_voxels()
            .into_iter()
            .map(|(address, material_slot)| MaterialVoxel {
                address,
                material_slot,
            }),
        options,
    )
    .expect("representative terrain fixture")
}

fn fixture_voxels() -> BTreeMap<[i64; 3], u16> {
    (0..64)
        .flat_map(|x| {
            (0..64).flat_map(move |z| {
                let height = 3 + ((x * 17 + z * 31 + (x ^ z)) % 5);
                (0..=height).map(move |y| ([x, y, z], 1))
            })
        })
        .collect()
}

fn edited_fixture(options: SurfaceMeshOptions, edits: &[VoxelEdit]) -> VoxelCollisionScene {
    let mut voxels = fixture_voxels();
    for edit in edits {
        match *edit {
            VoxelEdit::Set {
                address,
                material_slot,
            } => {
                voxels.insert(address, material_slot);
            }
            VoxelEdit::Clear { address } => {
                voxels.remove(&address);
            }
        }
    }
    VoxelCollisionScene::from_material_voxels_with_mesh_options(
        1.0,
        16,
        voxels
            .into_iter()
            .map(|(address, material_slot)| MaterialVoxel {
                address,
                material_slot,
            }),
        options,
    )
    .expect("post-edit whole-scene fixture")
}

fn project(
    projector: &mut VoxelRenderProjector,
    scene: &VoxelCollisionScene,
    asset_id: &str,
    materials: &BTreeMap<u16, RenderMaterialDescriptor>,
) -> render_projection::VoxelProjectionResult {
    projector
        .project(
            &[VoxelProjectionInstance {
                instance_id: "terrain".to_string(),
                asset_id: asset_id.to_string(),
                transform: Transform::IDENTITY,
                scene,
            }],
            materials,
        )
        .expect("representative projection")
}

fn print_row(
    mode: SurfaceMode,
    scenario: &str,
    elapsed_us: u128,
    frame: &render_model::RenderFrameDiff,
    rebuilt: usize,
    reused: usize,
) {
    let encoded_bytes = serde_json::to_vec(frame).expect("frame JSON").len();
    let replacements = frame
        .ops
        .iter()
        .filter(|operation| matches!(operation, RenderDiff::ReplaceMeshPayload { .. }))
        .count();
    let destroys = frame
        .ops
        .iter()
        .filter(|operation| matches!(operation, RenderDiff::Destroy { .. }))
        .count();
    println!(
        "{},{scenario},{elapsed_us},{encoded_bytes},{},{replacements},{destroys},{rebuilt},{reused}",
        mode.as_str(),
        frame.ops.len(),
    );
}

fn material() -> RenderMaterialDescriptor {
    RenderMaterialDescriptor {
        schema_version: 2,
        id: "voxel-material/1".to_string(),
        color: [0.45, 0.7, 0.35, 1.0],
        texture: None,
        roughness: 1.0,
        texture_tint: [1.0; 4],
        emission_color: [0.0; 3],
        emission_intensity: 0.0,
        uv_strategy: MaterialUvStrategy::Flat,
        alpha_mode: Default::default(),
        double_sided: false,
        voxel_surface: None,
    }
}
