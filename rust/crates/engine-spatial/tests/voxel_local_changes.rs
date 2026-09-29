//! Local voxel edits and residency changes update the scene in place. These
//! tests check that the result matches a scene built in one pass, and that a
//! change touches only its neighbourhood.
use std::collections::BTreeMap;

use engine_spatial::{
    DynamicsBodyId, DynamicsBodyInput, DynamicsShape, DynamicsSolver, MaterialVoxel,
    VoxelChunkIdentity, VoxelChunkPayload, VoxelChunkResidencyOperation,
    VoxelChunkResidencyService, VoxelCollisionScene, VoxelEdit, VoxelEditService,
};

const CHUNK: i64 = 8;

/// Deterministic pseudo-random sequence for repeatable workloads.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self, bound: i64) -> i64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.0 >> 33) % bound as u64) as i64
    }
}

fn terrain(chunks_x: i64, chunks_z: i64) -> VoxelCollisionScene {
    let mut voxels = Vec::new();
    for x in 0..chunks_x * CHUNK {
        for z in 0..chunks_z * CHUNK {
            let height = 2 + (x * 3 + z * 5) % 5;
            for y in 0..height {
                voxels.push(MaterialVoxel {
                    state: 0,
                    address: [x, y, z],
                    material_slot: 1 + ((x + z) % 3) as u16,
                });
            }
        }
    }
    VoxelCollisionScene::from_material_voxels(1.0, CHUNK as u32, voxels).unwrap()
}

/// The same resident chunks and voxels, admitted in one residency change to
/// an empty scene: every chunk is built from scratch.
fn rebuilt(scene: &VoxelCollisionScene) -> VoxelCollisionScene {
    let mut fresh =
        VoxelCollisionScene::from_solid_voxels(scene.voxel_size(), scene.chunk_size(), []).unwrap();
    fresh.set_noncollidable_materials(scene.noncollidable_materials().clone());
    let volume = (CHUNK * CHUNK * CHUNK) as usize;
    let mut payloads: BTreeMap<[i64; 3], VoxelChunkPayload> = scene
        .resident_chunk_coordinates()
        .into_iter()
        .map(|chunk| {
            let mut payload = VoxelChunkPayload::new([CHUNK as u32; 3], vec![0; volume]);
            payload.states = vec![0; volume];
            (chunk, payload)
        })
        .collect();
    for voxel in scene.material_voxels() {
        let chunk = voxel.address.map(|axis| axis.div_euclid(CHUNK));
        let [x, y, z] = voxel.address.map(|axis| axis.rem_euclid(CHUNK));
        let index = (x + CHUNK * (y + CHUNK * z)) as usize;
        let payload = payloads.get_mut(&chunk).expect("resident chunk");
        payload.material_slots[index] = voxel.material_slot;
        payload.states[index] = voxel.state;
    }
    let operations: Vec<_> = payloads
        .into_iter()
        .map(|(chunk, payload)| VoxelChunkResidencyOperation::Admit {
            chunk: VoxelChunkIdentity::from_array(chunk),
            payload,
        })
        .collect();
    if !operations.is_empty() {
        VoxelChunkResidencyService::apply(&mut fresh, &operations).unwrap();
    }
    fresh
}

fn assert_same_projections(scene: &VoxelCollisionScene, fresh: &VoxelCollisionScene) {
    assert_eq!(scene.solid_voxel_count(), fresh.solid_voxel_count());
    assert_eq!(scene.authority_hash(), fresh.authority_hash());
    // The hash is also what a scene built directly from the voxels reports.
    let direct = VoxelCollisionScene::from_material_voxels(
        scene.voxel_size(),
        scene.chunk_size(),
        scene.material_voxels(),
    )
    .unwrap();
    assert_eq!(scene.authority_hash(), direct.authority_hash());
    let meshes = |scene: &VoxelCollisionScene| {
        scene
            .mesh_chunks()
            .map(|chunk| (chunk.chunk, chunk.content_hash))
            .collect::<Vec<_>>()
    };
    assert_eq!(meshes(scene), meshes(fresh));
    assert_eq!(scene.collider_chunk_count(), fresh.collider_chunk_count());
    assert_eq!(scene.navigation_cell_count(), fresh.navigation_cell_count());
    assert_eq!(scene.navigation_hash(), fresh.navigation_hash());
    for x in (-4..40).step_by(3) {
        for z in (-4..40).step_by(5) {
            let origin = [x as f64 + 0.5, 20.0, z as f64 + 0.5];
            assert_eq!(
                scene.raycast(origin, [0.0, -1.0, 0.0], 40.0),
                fresh.raycast(origin, [0.0, -1.0, 0.0], 40.0),
                "ray at {origin:?}"
            );
        }
    }
}

#[test]
fn many_local_changes_match_a_scene_built_in_one_pass() {
    let mut scene = terrain(4, 4);
    let mut random = Lcg(7);
    for _ in 0..150 {
        let edits: Vec<_> = (0..1 + random.next(4))
            .map(|_| {
                let address = [random.next(34) - 1, random.next(9), random.next(34) - 1];
                match random.next(3) {
                    0 => VoxelEdit::Clear { address },
                    1 => VoxelEdit::Set {
                        address,
                        material_slot: 1 + random.next(4) as u16,
                    },
                    _ => VoxelEdit::SetState {
                        address,
                        material_slot: 2,
                        state: random.next(16) as u16,
                    },
                }
            })
            .collect();
        let _ = VoxelEditService::apply(&mut scene, &edits);
    }
    // Residency: unload two chunks, load a new one, replace another.
    let volume = (CHUNK * CHUNK * CHUNK) as usize;
    let mut stripes = vec![0u16; volume];
    for (index, slot) in stripes.iter_mut().enumerate() {
        if (index / (CHUNK * CHUNK) as usize).is_multiple_of(2) {
            *slot = 3;
        }
    }
    VoxelChunkResidencyService::apply(
        &mut scene,
        &[
            VoxelChunkResidencyOperation::Evict {
                chunk: VoxelChunkIdentity::new(1, 0, 1),
            },
            VoxelChunkResidencyOperation::Evict {
                chunk: VoxelChunkIdentity::new(3, 0, 3),
            },
            VoxelChunkResidencyOperation::Admit {
                chunk: VoxelChunkIdentity::new(4, 0, 0),
                payload: VoxelChunkPayload::new([CHUNK as u32; 3], stripes.clone()),
            },
            VoxelChunkResidencyOperation::Replace {
                chunk: VoxelChunkIdentity::new(2, 0, 2),
                payload: VoxelChunkPayload::new([CHUNK as u32; 3], stripes),
            },
        ],
    )
    .unwrap();
    for _ in 0..50 {
        let address = [random.next(40), random.next(9), random.next(34)];
        let _ = VoxelEditService::apply(
            &mut scene,
            &[VoxelEdit::Set {
                address,
                material_slot: 1,
            }],
        );
    }
    assert_same_projections(&scene, &rebuilt(&scene));
}

#[test]
fn noncollidable_materials_apply_to_edited_chunks() {
    let mut scene = terrain(2, 2);
    scene.set_noncollidable_materials([9].into());
    let top = |scene: &VoxelCollisionScene| {
        scene
            .raycast([3.5, 20.0, 3.5], [0.0, -1.0, 0.0], 40.0)
            .unwrap()
            .voxel
    };
    let floor = top(&scene);
    // A noncollidable voxel above the floor keeps its cell but not collision.
    let ghost = [3, floor[1] + 1, 3];
    VoxelEditService::apply(
        &mut scene,
        &[VoxelEdit::Set {
            address: ghost,
            material_slot: 9,
        }],
    )
    .unwrap();
    assert!(scene.material_voxels().iter().any(|v| v.address == ghost));
    assert_eq!(top(&scene), floor);
    assert_same_projections(&scene, &rebuilt(&scene));
    // Making the material collidable later updates the existing chunks.
    scene.set_noncollidable_materials(Default::default());
    assert_eq!(top(&scene), ghost);
    assert_same_projections(&scene, &rebuilt(&scene));
}

#[test]
fn one_cell_edit_in_a_large_world_touches_only_its_neighbourhood() {
    let mut scene = terrain(16, 16);
    let chunks = scene.mesh_chunks().len();
    assert_eq!(chunks, 256);
    let receipt = VoxelEditService::apply(
        &mut scene,
        &[VoxelEdit::Clear {
            address: [8 * 7 + 3, 1, 8 * 9 + 4],
        }],
    )
    .unwrap();
    // An interior cell: its own chunk only.
    assert_eq!(receipt.dirty_mesh_chunks, vec![[7, 0, 9]]);
    assert_eq!(receipt.rebuilt_mesh_chunks, 1);
    assert_eq!(receipt.reused_mesh_chunks, chunks - 1);
    // A corner cell also dirties its resident face neighbours.
    let receipt = VoxelEditService::apply(
        &mut scene,
        &[VoxelEdit::Clear {
            address: [8 * 7, 0, 8 * 9],
        }],
    )
    .unwrap();
    assert_eq!(
        receipt.dirty_mesh_chunks,
        vec![[6, 0, 9], [7, 0, 8], [7, 0, 9]]
    );
}

fn resting_ball(id: u64, at: [f64; 3]) -> DynamicsBodyInput {
    DynamicsBodyInput {
        id: DynamicsBodyId(id),
        translation: at,
        rotation: [0.0, 0.0, 0.0, 1.0],
        shape: DynamicsShape::Sphere { radius: 0.4 },
        mass: 1.0,
        mass_properties: None,
        linear_velocity: [0.0; 3],
        angular_velocity: [0.0; 3],
        locked_translation_axes: [false; 3],
        locked_rotation_axes: [false; 3],
        linear_damping: 0.0,
        angular_damping: 0.0,
        gravity_scale: 1.0,
        friction: 0.5,
        restitution: 0.0,
        collision_groups: u32::MAX,
        collision_mask: u32::MAX,
        enabled: true,
        sleeping: false,
        continuous_collision: false,
    }
}

#[test]
fn an_edit_keeps_distant_colliders_and_a_resting_body_asleep() {
    let mut scene = VoxelCollisionScene::from_solid_voxels(
        1.0,
        CHUNK as u32,
        (0..64).flat_map(|x| (0..64).map(move |z| [x, 0, z])),
    )
    .unwrap();
    let mut solver = DynamicsSolver::new([0.0, -9.81, 0.0]);
    assert_eq!(scene.bind_dynamics_environment(&mut solver).inserted, 64);
    solver
        .insert_body(resting_ball(1, [60.5, 1.45, 60.5]))
        .unwrap();
    for _ in 0..180 {
        solver.step(1.0 / 60.0, 1, &[]).unwrap();
    }
    assert!(solver.body(DynamicsBodyId(1)).unwrap().sleeping);

    // Dig one cell on the far side of the floor.
    VoxelEditService::apply(&mut scene, &[VoxelEdit::Clear { address: [3, 0, 3] }]).unwrap();
    let receipt = scene.bind_dynamics_environment(&mut solver);
    assert_eq!(
        (receipt.retained, receipt.inserted, receipt.removed),
        (63, 1, 1)
    );
    for _ in 0..30 {
        solver.step(1.0 / 60.0, 1, &[]).unwrap();
    }
    let ball = solver.body(DynamicsBodyId(1)).unwrap();
    assert!(ball.sleeping);
    assert!((ball.translation[1] - 1.4).abs() < 0.05);
}
