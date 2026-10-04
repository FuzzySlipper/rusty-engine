//! Chunk meshing, and collision that follows reconstructed surfaces.
//!
//! A chunk whose materials are all drawn as cubes keeps the established
//! cuboid collider. When any material is reconstructed, each chunk collider
//! is derived from what is drawn: a cuboid for every cube-material voxel and
//! for every reconstructed voxel no surface passes through (all 26
//! neighbours solid), plus the chunk's reconstructed surface triangles, each
//! owned by the voxel that produced it.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Instant;

use core_space::{ChunkCoord, LocalVoxelCoord, VoxelCoord};
use svc_collision::ChunkSurfaceCollider;
use svc_mesh::SurfaceMeshOptions;
use svc_spatial::VoxelWorld;

use crate::{build_mesh_chunk, CollisionSceneError, VoxelMeshChunk};

/// Below this many chunks per thread, meshing stays on the calling thread.
const CHUNKS_PER_THREAD: usize = 4;

/// Mesh `coordinates` (all resident and non-empty), in parallel when there
/// are enough of them. Returns the meshes in input order and the summed
/// per-chunk meshing time in microseconds.
pub(crate) fn mesh_chunks(
    world: &VoxelWorld,
    coordinates: &[ChunkCoord],
    options: &SurfaceMeshOptions,
) -> Result<(Vec<Arc<VoxelMeshChunk>>, u64), CollisionSceneError> {
    let mesh_slice =
        |slice: &[ChunkCoord]| -> Result<(Vec<Arc<VoxelMeshChunk>>, u64), CollisionSceneError> {
            let mut meshes = Vec::with_capacity(slice.len());
            let mut microseconds = 0_u64;
            for coordinate in slice {
                let started = Instant::now();
                meshes.push(Arc::new(build_mesh_chunk(world, *coordinate, options)?));
                microseconds = microseconds.saturating_add(started.elapsed().as_micros() as u64);
            }
            Ok((meshes, microseconds))
        };
    let threads = std::thread::available_parallelism()
        .map_or(1, |threads| threads.get())
        .min(coordinates.len() / CHUNKS_PER_THREAD);
    if threads <= 1 {
        return mesh_slice(coordinates);
    }
    let per_thread = coordinates.len().div_ceil(threads);
    let results: Vec<_> = std::thread::scope(|scope| {
        let handles: Vec<_> = coordinates
            .chunks(per_thread)
            .map(|slice| scope.spawn(move || mesh_slice(slice)))
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().expect("chunk meshing does not panic"))
            .collect()
    });
    let mut meshes = Vec::with_capacity(coordinates.len());
    let mut microseconds = 0_u64;
    for result in results {
        let (slice, time) = result?;
        meshes.extend(slice);
        microseconds = microseconds.saturating_add(time);
    }
    Ok((meshes, microseconds))
}

/// The cuboid voxels of one chunk of a session with reconstructed materials:
/// cube materials, and reconstructed voxels no surface passes through: none
/// touches empty space or a different non-occluding material.
/// Noncollidable materials contribute nothing; a reconstructed voxel beside a
/// noncollidable solid that occludes keeps its cuboid, since no surface is
/// drawn between them.
pub(crate) fn collider_cubes(
    world: &VoxelWorld,
    noncollidable: &BTreeSet<u16>,
    options: &SurfaceMeshOptions,
    coordinate: ChunkCoord,
) -> Vec<VoxelCoord> {
    let grid = world.grid();
    let Some(chunk) = world.get(coordinate) else {
        return Vec::new();
    };
    let size = grid.chunk_dims().to_array().map(|value| value as usize);
    let origin = grid.chunk_origin_voxel(coordinate);
    // Materials of the chunk and a one-voxel halo; `u16::MAX` is empty.
    const EMPTY: u16 = u16::MAX;
    let halo = size.map(|value| value + 2);
    let mut materials = vec![EMPTY; halo[0] * halo[1] * halo[2]];
    let index = |x: usize, y: usize, z: usize| (z * halo[1] + y) * halo[0] + x;
    for dz in -1..=1_i64 {
        for dy in -1..=1_i64 {
            for dx in -1..=1_i64 {
                let neighbour =
                    ChunkCoord::new(coordinate.x + dx, coordinate.y + dy, coordinate.z + dz);
                let Some(source) = world.get(neighbour) else {
                    continue;
                };
                // The neighbour's voxels within one voxel of this chunk.
                let range = |offset: i64, extent: usize| match offset {
                    -1 => (extent as u32 - 1)..extent as u32,
                    0 => 0..extent as u32,
                    _ => 0..1,
                };
                let at = |offset: i64, local: u32, extent: usize| {
                    (offset * extent as i64 + i64::from(local) + 1) as usize
                };
                for lz in range(dz, size[2]) {
                    for ly in range(dy, size[1]) {
                        for lx in range(dx, size[0]) {
                            let local = LocalVoxelCoord::new(lx, ly, lz);
                            if let Some(material) =
                                source.get(local).and_then(|value| value.material())
                            {
                                materials[index(
                                    at(dx, lx, size[0]),
                                    at(dy, ly, size[1]),
                                    at(dz, lz, size[2]),
                                )] = material.raw();
                            }
                        }
                    }
                }
            }
        }
    }
    let mut cubes = Vec::new();
    for (local, value) in chunk.iter() {
        let Some(material) = value.material() else {
            continue;
        };
        let slot = material.raw();
        if noncollidable.contains(&slot) {
            continue;
        }
        let voxel = VoxelCoord::new(
            origin.x + i64::from(local.x),
            origin.y + i64::from(local.y),
            origin.z + i64::from(local.z),
        );
        if !options.surface(slot).mode.is_reconstructed() {
            cubes.push(voxel);
            continue;
        }
        let [x, y, z] = [local.x, local.y, local.z].map(|value| value as usize + 1);
        let mut interior = true;
        let mut beside_noncollidable = false;
        for dz in 0..3 {
            for dy in 0..3 {
                for dx in 0..3 {
                    let neighbour = materials[index(x + dx - 1, y + dy - 1, z + dz - 1)];
                    // The surface also meets a different non-occluding material.
                    let see_through =
                        neighbour != slot && options.non_occluding.contains(&neighbour);
                    if neighbour == EMPTY || see_through {
                        interior = false;
                    } else if noncollidable.contains(&neighbour)
                        && [dx, dy, dz].iter().filter(|offset| **offset != 1).count() == 1
                    {
                        beside_noncollidable = true;
                    }
                }
            }
        }
        if interior || beside_noncollidable {
            cubes.push(voxel);
        }
    }
    cubes
}

/// The reconstructed triangles one chunk draws, owned by their voxels,
/// without noncollidable materials.
pub(crate) fn collider_surface(
    world: &VoxelWorld,
    noncollidable: &BTreeSet<u16>,
    coordinate: ChunkCoord,
    mesh: &VoxelMeshChunk,
) -> Option<ChunkSurfaceCollider> {
    let grid = world.grid();
    let size = grid.chunk_dims().to_array().map(|value| value as usize);
    let origin = grid.chunk_origin_voxel(coordinate);
    {
        let base = grid.voxel_min_world(origin);
        let mut surface = ChunkSurfaceCollider::default();
        for group in mesh
            .groups
            .iter()
            .filter(|group| group.surface_mode.is_reconstructed())
        {
            for triangle in (group.start / 3)..((group.start + group.count) / 3) {
                let owner = mesh.triangle_owners[triangle as usize] as usize;
                let local = [
                    owner % size[0],
                    (owner / size[0]) % size[1],
                    owner / (size[0] * size[1]),
                ];
                if noncollidable.contains(&group.material_slot) {
                    continue;
                }
                let first = surface.positions.len() as u32;
                for corner in 0..3 {
                    let vertex = mesh.indices[triangle as usize * 3 + corner] as usize * 3;
                    surface.positions.push([
                        base.x + f64::from(mesh.positions[vertex]),
                        base.y + f64::from(mesh.positions[vertex + 1]),
                        base.z + f64::from(mesh.positions[vertex + 2]),
                    ]);
                }
                surface.triangles.push([first, first + 1, first + 2]);
                surface.owners.push(VoxelCoord::new(
                    origin.x + local[0] as i64,
                    origin.y + local[1] as i64,
                    origin.z + local[2] as i64,
                ));
            }
        }
        (!surface.triangles.is_empty()).then_some(surface)
    }
}

/// Identity of a noncollidable material set, mixed into a surface collider's
/// key so a participation change replaces the retained triangles even when
/// the drawn mesh is unchanged. The empty set is 0, leaving the key the mesh's.
pub(crate) fn noncollidable_key(materials: &BTreeSet<u16>) -> u64 {
    materials.iter().fold(0, |key, material| {
        (key ^ u64::from(*material).wrapping_add(1)).wrapping_mul(0x100000001b3)
    })
}
