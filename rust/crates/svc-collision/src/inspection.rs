//! The projection's collider geometry in a region, as it collides, for
//! inspection drawing and readouts. Reading it changes nothing.

use core_space::{ChunkCoord, WorldPos};
use parry3d_f64::bounding_volume::Aabb;
use parry3d_f64::shape::{Compound, TriMesh};

use crate::static_mesh::{StaticMeshAssetId, StaticMeshInstanceId};
use crate::{world_to_point, CollisionProjection, WorldAabb};

/// Wire lines a cuboid and a triangle draw as.
pub const CUBOID_LINES: usize = 12;
pub const TRIANGLE_LINES: usize = 3;

/// Which retained collider a part belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InspectedColliderSource {
    /// One resident voxel chunk's cuboids and reconstructed surface.
    VoxelChunk(ChunkCoord),
    StaticMesh {
        instance: StaticMeshInstanceId,
        asset: StaticMeshAssetId,
        geometry_hash: u64,
    },
}

/// One collider meeting a region: its whole bounds, and its parts that meet
/// the region, in the projection's coordinate frame.
#[derive(Debug, Clone, PartialEq)]
pub struct InspectedCollider {
    pub source: InspectedColliderSource,
    pub min: [f64; 3],
    pub max: [f64; 3],
    /// Cuboids, as min and max corners.
    pub boxes: Vec<[[f64; 3]; 2]>,
    pub triangles: Vec<[[f64; 3]; 3]>,
    /// Parts meeting the region left out because the line budget ran out.
    pub omitted: usize,
}

impl CollisionProjection {
    /// The voxel chunks and static-mesh instances whose bounds meet the box
    /// `min..max`, nearest its centre first, each with its parts (cuboids
    /// and triangles) that meet it, while they draw within `budget` wire
    /// lines in all ([`CUBOID_LINES`], [`TRIANGLE_LINES`]). A collider whose
    /// bounds are unknown is read from its parts.
    pub fn inspect_region(
        &self,
        min: WorldPos,
        max: WorldPos,
        mut budget: usize,
    ) -> Vec<InspectedCollider> {
        let region = WorldAabb { min, max };
        let query = Aabb::new(world_to_point(min), world_to_point(max));
        let center = [
            (min.x + max.x) * 0.5,
            (min.y + max.y) * 0.5,
            (min.z + max.z) * 0.5,
        ];
        let mut found: Vec<(f64, InspectedColliderSource, WorldAabb)> = Vec::new();
        for (coord, collider) in &self.chunks {
            let bounds = collider.bounds.or_else(|| {
                let shapes = collider
                    .cubes
                    .iter()
                    .map(|cubes| local_bounds(*cubes.local_aabb()));
                let surface = collider
                    .surface
                    .iter()
                    .map(|surface| local_bounds(surface.shape.local_aabb()));
                shapes.chain(surface).reduce(union)
            });
            if let Some(bounds) = bounds.filter(|bounds| bounds.intersects(region)) {
                found.push((
                    distance(center, bounds),
                    InspectedColliderSource::VoxelChunk(*coord),
                    bounds,
                ));
            }
        }
        for (instance, asset, geometry_hash, shape, bounds) in self.static_meshes.character_shapes()
        {
            let bounds = bounds.unwrap_or_else(|| local_bounds(shape.compute_local_aabb()));
            if bounds.intersects(region) {
                found.push((
                    distance(center, bounds),
                    InspectedColliderSource::StaticMesh {
                        instance,
                        asset,
                        geometry_hash,
                    },
                    bounds,
                ));
            }
        }
        found.sort_by(|left, right| left.0.total_cmp(&right.0));
        found
            .into_iter()
            .map(|(_, source, bounds)| {
                let mut inspected = InspectedCollider {
                    source,
                    min: [bounds.min.x, bounds.min.y, bounds.min.z],
                    max: [bounds.max.x, bounds.max.y, bounds.max.z],
                    boxes: Vec::new(),
                    triangles: Vec::new(),
                    omitted: 0,
                };
                match source {
                    InspectedColliderSource::VoxelChunk(coord) => {
                        let collider = &self.chunks[&coord];
                        if let Some(cubes) = &collider.cubes {
                            read_cuboids(cubes, &query, &mut budget, &mut inspected);
                        }
                        if let Some(surface) = &collider.surface {
                            read_triangles(&surface.shape, &query, &mut budget, &mut inspected);
                        }
                    }
                    InspectedColliderSource::StaticMesh { instance, .. } => {
                        if let Some(mesh) = self.static_meshes.instance_shape(instance).as_trimesh()
                        {
                            read_triangles(mesh, &query, &mut budget, &mut inspected);
                        }
                    }
                }
                inspected
            })
            .collect()
    }
}

fn read_cuboids(
    cubes: &Compound,
    query: &Aabb,
    budget: &mut usize,
    inspected: &mut InspectedCollider,
) {
    for index in cubes.bvh().intersect_aabb(query) {
        let (pose, shape) = &cubes.shapes()[index as usize];
        let Some(cuboid) = shape.as_cuboid() else {
            continue;
        };
        if *budget < CUBOID_LINES {
            inspected.omitted += 1;
            continue;
        }
        *budget -= CUBOID_LINES;
        let (middle, half) = (pose.translation, cuboid.half_extents);
        inspected
            .boxes
            .push([(middle - half).to_array(), (middle + half).to_array()]);
    }
}

fn read_triangles(
    mesh: &TriMesh,
    query: &Aabb,
    budget: &mut usize,
    inspected: &mut InspectedCollider,
) {
    for index in mesh.bvh().intersect_aabb(query) {
        if *budget < TRIANGLE_LINES {
            inspected.omitted += 1;
            continue;
        }
        *budget -= TRIANGLE_LINES;
        let triangle = mesh.triangle(index);
        inspected
            .triangles
            .push([triangle.a, triangle.b, triangle.c].map(|point| point.to_array()));
    }
}

fn local_bounds(aabb: Aabb) -> WorldAabb {
    WorldAabb {
        min: WorldPos::new(aabb.mins.x, aabb.mins.y, aabb.mins.z),
        max: WorldPos::new(aabb.maxs.x, aabb.maxs.y, aabb.maxs.z),
    }
}

fn union(left: WorldAabb, right: WorldAabb) -> WorldAabb {
    WorldAabb {
        min: WorldPos::new(
            left.min.x.min(right.min.x),
            left.min.y.min(right.min.y),
            left.min.z.min(right.min.z),
        ),
        max: WorldPos::new(
            left.max.x.max(right.max.x),
            left.max.y.max(right.max.y),
            left.max.z.max(right.max.z),
        ),
    }
}

/// How far `point` is from the box; 0 inside it.
fn distance(point: [f64; 3], bounds: WorldAabb) -> f64 {
    let min = [bounds.min.x, bounds.min.y, bounds.min.z];
    let max = [bounds.max.x, bounds.max.y, bounds.max.z];
    (0..3)
        .map(|axis| {
            (min[axis] - point[axis])
                .max(point[axis] - max[axis])
                .max(0.0)
        })
        .map(|gap| gap * gap)
        .sum::<f64>()
        .sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{StaticMeshColliderAsset, StaticMeshColliderInstance, StaticMeshTransform};
    use core_space::{ChunkDims, GridId, LocalVoxelCoord, VoxelGridSpec};
    use core_voxel::VoxelValue;
    use svc_spatial::VoxelWorld;
    use svc_volume::VoxelChunk;

    fn projection() -> CollisionProjection {
        let spec = VoxelGridSpec::new(GridId::new(0), 1.0, ChunkDims::cubic(8).unwrap()).unwrap();
        let mut world = VoxelWorld::new(spec);
        for (coord, local) in [
            (ChunkCoord::new(0, 0, 0), LocalVoxelCoord::new(1, 0, 1)),
            (ChunkCoord::new(6, 0, 0), LocalVoxelCoord::new(0, 0, 0)),
        ] {
            let mut chunk = VoxelChunk::from_spec(&spec);
            chunk.set(local, VoxelValue::solid_raw(1)).unwrap();
            world.insert(coord, chunk);
        }
        world.drain_dirty();
        let mut projection = CollisionProjection::build(&world);
        let asset = StaticMeshColliderAsset::new(
            StaticMeshAssetId(41),
            vec![[0.0, 0.0, 3.0], [2.0, 0.0, 3.0], [0.0, 2.0, 3.0]],
            vec![[0, 1, 2]],
        )
        .unwrap();
        projection
            .replace_static_meshes(
                [asset],
                [StaticMeshColliderInstance {
                    id: StaticMeshInstanceId(9),
                    asset: StaticMeshAssetId(41),
                    transform: StaticMeshTransform::IDENTITY,
                }],
            )
            .unwrap();
        projection
    }

    #[test]
    fn a_region_reads_the_colliders_meeting_it_nearest_first_within_the_budget() {
        let projection = projection();
        let region = |budget| {
            projection.inspect_region(
                WorldPos::new(0.0, -1.0, 0.0),
                WorldPos::new(3.0, 2.0, 3.0),
                budget,
            )
        };
        let read = region(16);
        assert_eq!(read.len(), 2, "the far chunk is left out");
        assert_eq!(
            read[0].source,
            InspectedColliderSource::VoxelChunk(ChunkCoord::new(0, 0, 0))
        );
        assert_eq!(read[0].boxes, [[[1.0, 0.0, 1.0], [2.0, 1.0, 2.0]]]);
        assert_eq!(
            read[1].source,
            InspectedColliderSource::StaticMesh {
                instance: StaticMeshInstanceId(9),
                asset: StaticMeshAssetId(41),
                geometry_hash: read[1].source_hash(),
            }
        );
        assert_eq!(
            read[1].triangles,
            [[[0.0, 0.0, 3.0], [2.0, 0.0, 3.0], [0.0, 2.0, 3.0]]]
        );
        let cut = region(CUBOID_LINES + TRIANGLE_LINES - 1);
        assert_eq!((cut[0].boxes.len(), cut[0].omitted), (1, 0));
        assert_eq!((cut[1].triangles.len(), cut[1].omitted), (0, 1));
    }

    impl InspectedCollider {
        fn source_hash(&self) -> u64 {
            match self.source {
                InspectedColliderSource::StaticMesh { geometry_hash, .. } => geometry_hash,
                InspectedColliderSource::VoxelChunk(_) => 0,
            }
        }
    }
}
