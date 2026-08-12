use std::{collections::BTreeMap, time::Instant};

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
    project(&mut full_projector, &full_scene, "terrain-v1", &materials);
    let full_started = Instant::now();
    let full_candidate = fixture(options);
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
        full_scene.mesh_chunks().len(),
        0,
    );

    let mut scene = fixture(options);
    let mut projector = VoxelRenderProjector::with_publication_stream(format!(
        "benchmark:{}:{scenario}:incremental",
        mode.as_str()
    ));
    project(&mut projector, &scene, "terrain-v1", &materials);
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
}

fn fixture(options: SurfaceMeshOptions) -> VoxelCollisionScene {
    let voxels = (0..64)
        .flat_map(|x| {
            (0..64).flat_map(move |z| {
                let height = 3 + ((x * 17 + z * 31 + (x ^ z)) % 5);
                (0..=height).map(move |y| MaterialVoxel {
                    address: [x, y, z],
                    material_slot: 1,
                })
            })
        })
        .collect::<Vec<_>>();
    VoxelCollisionScene::from_material_voxels_with_mesh_options(1.0, 16, voxels, options)
        .expect("representative terrain fixture")
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
        voxel_surface: None,
    }
}
