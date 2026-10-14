//! Voxel authority works out to the edge of the coordinate bound and with any
//! nonzero material slot, and the first coordinate past the bound is refused
//! where it enters rather than while meshing.

use core_space::WorldPos;
use engine_spatial::{
    CharacterCapsule, CollisionSceneError, MaterialVoxel, SurfaceMeshOptions, SurfaceMode,
    VoxelAuthorityValidationError, VoxelChunkIdentity, VoxelChunkPayload,
    VoxelChunkResidencyOperation, VoxelChunkResidencyService, VoxelCollisionScene, VoxelEdit,
    VoxelEditApplyError, VoxelEditRejection, VoxelEditService, MAX_VOXEL_COORDINATE_ABS,
};
use svc_mesh::texture_mapping::voxel_surface_texture_basis;

const BOUND: i64 = MAX_VOXEL_COORDINATE_ABS;
const CHUNK: u32 = 4;
const WIDE_SLOTS: [u16; 3] = [4_096, 40_000, 65_535];

fn options(mode: SurfaceMode) -> SurfaceMeshOptions {
    SurfaceMeshOptions {
        mode,
        ..SurfaceMeshOptions::default()
    }
}

fn voxel(address: [i64; 3], material_slot: u16) -> MaterialVoxel {
    MaterialVoxel {
        state: 0,
        address,
        material_slot,
    }
}

fn ray_voxel(scene: &VoxelCollisionScene, origin: [f64; 3], direction: [f64; 3]) -> [i64; 3] {
    scene
        .raycast(origin, direction, 16.0)
        .unwrap_or_else(|| panic!("ray from {origin:?} along {direction:?} hits"))
        .voxel
}

#[test]
fn scenes_edits_residency_meshes_and_rays_work_at_the_coordinate_bound() {
    for mode in [SurfaceMode::GreedyCubes, SurfaceMode::DualContouring] {
        for sign in [1_i64, -1] {
            let edge = sign * BOUND;
            let inward = -sign;
            let corner = [edge, edge, edge];
            let mut scene = VoxelCollisionScene::from_material_voxels_with_mesh_options(
                1.0,
                CHUNK,
                [
                    voxel(corner, WIDE_SLOTS[0]),
                    voxel([edge + inward, edge, edge], WIDE_SLOTS[1]),
                ],
                options(mode),
            )
            .unwrap();
            VoxelEditService::apply(
                &mut scene,
                &[VoxelEdit::Set {
                    address: [edge, edge + inward, edge],
                    material_slot: WIDE_SLOTS[2],
                }],
            )
            .unwrap();

            // The outermost chunk along X whose every voxel is inside the bound,
            // with one voxel on its outer face.
            let chunk_x = sign * ((BOUND + 1) / i64::from(CHUNK) - 1);
            let outer_local = if sign > 0 { CHUNK - 1 } else { 0 };
            let outer_x = chunk_x * i64::from(CHUNK) + i64::from(outer_local);
            let mut slots = vec![0_u16; CHUNK.pow(3) as usize];
            slots[outer_local as usize] = WIDE_SLOTS[2];
            VoxelChunkResidencyService::apply(
                &mut scene,
                &[VoxelChunkResidencyOperation::Admit {
                    chunk: VoxelChunkIdentity::new(chunk_x, 0, 0),
                    payload: VoxelChunkPayload::new([CHUNK; 3], slots),
                }],
            )
            .unwrap();

            let mut slots_drawn = Vec::new();
            for mesh in scene.mesh_chunks() {
                assert!(
                    mesh.positions.iter().all(|value| value.is_finite()),
                    "{mode:?} {sign}"
                );
                for group in &mesh.groups {
                    slots_drawn.push(group.material_slot);
                    if mode != SurfaceMode::GreedyCubes {
                        continue;
                    }
                    // Every greedy tile coordinate is its absolute corner, exactly.
                    let direction = group.direction.expect("greedy faces have a direction");
                    for &index in &mesh.indices[group.start as usize..][..group.count as usize] {
                        let vertex = index as usize * 3;
                        let grid_point: [i64; 3] = std::array::from_fn(|axis| {
                            mesh.origin_voxel[axis] + mesh.positions[vertex + axis] as i64
                        });
                        let basis = voxel_surface_texture_basis(direction);
                        let exact = [basis.u, basis.v]
                            .map(|axis| grid_point[axis.axis().index()] * i64::from(axis.sign()));
                        let tile = &mesh.tile_coordinates[index as usize * 2..][..2];
                        assert_eq!([tile[0] as i64, tile[1] as i64], exact);
                    }
                }
            }
            for slot in WIDE_SLOTS {
                assert!(slots_drawn.contains(&slot), "{mode:?} {sign} draws {slot}");
            }

            let centre = |coordinate: i64| coordinate as f64 + 0.5;
            assert_eq!(
                ray_voxel(
                    &scene,
                    [
                        centre(edge) - 3.0 * inward as f64,
                        centre(edge),
                        centre(edge)
                    ],
                    [inward as f64, 0.0, 0.0],
                ),
                corner,
                "{mode:?} {sign}"
            );
            assert_eq!(
                ray_voxel(&scene, [centre(outer_x), 8.0, 0.5], [0.0, -1.0, 0.0]),
                [outer_x, 0, 0],
                "{mode:?} {sign}"
            );
        }
    }
}

#[test]
fn the_first_coordinate_past_the_bound_is_refused_where_it_enters() {
    for sign in [1_i64, -1] {
        for axis in 0..3 {
            let mut address = [0; 3];
            address[axis] = sign * (BOUND + 1);
            let error = VoxelCollisionScene::from_material_voxels(1.0, CHUNK, [voxel(address, 1)])
                .unwrap_err();
            assert!(
                matches!(
                    error,
                    CollisionSceneError::InvalidMaterialVoxel(
                        VoxelAuthorityValidationError::CoordinateOutOfBounds { axis: refused, limit: BOUND, .. }
                    ) if refused == axis
                ),
                "{error:?}"
            );

            let mut scene =
                VoxelCollisionScene::from_material_voxels(1.0, CHUNK, [voxel([0, 0, 0], 1)])
                    .unwrap();
            let error = VoxelEditService::apply(
                &mut scene,
                &[VoxelEdit::Set {
                    address,
                    material_slot: 1,
                }],
            )
            .unwrap_err();
            assert!(
                matches!(
                    error,
                    VoxelEditApplyError::Rejected(VoxelEditRejection::CoordinateOutOfBounds {
                        edit_index: 0,
                        axis: refused,
                        limit: BOUND,
                        ..
                    }) if refused == axis
                ),
                "{error:?}"
            );
            assert_eq!(scene.solid_voxel_count(), 1);
        }
    }
}

/// Reconstructed colliders keep the cuboids of voxels no surface passes
/// through. A neighbour of the largest slot is solid like any other, so the
/// middle of a solid block still collides.
#[test]
fn the_largest_slot_is_solid_inside_a_reconstructed_block() {
    const EDGE: u32 = 8;
    let mut scene = VoxelCollisionScene::from_material_voxels_with_mesh_options(
        1.0,
        EDGE,
        [],
        options(SurfaceMode::DualContouring),
    )
    .unwrap();
    VoxelChunkResidencyService::apply(
        &mut scene,
        &[VoxelChunkResidencyOperation::Admit {
            chunk: VoxelChunkIdentity::new(0, 0, 0),
            payload: VoxelChunkPayload::new([EDGE; 3], vec![u16::MAX; EDGE.pow(3) as usize]),
        }],
    )
    .unwrap();
    let overlap = scene
        .character_capsule_overlap(CharacterCapsule {
            center: WorldPos::new(4.0, 4.0, 4.0),
            half_height: 0.3,
            radius: 0.3,
        })
        .unwrap();
    assert!(overlap.is_some());
}
