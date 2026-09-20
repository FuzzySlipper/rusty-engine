use super::*;

const MAX_MAP_CELLS: u32 = 1024;

impl RuntimeSpatialBridge {
    fn read_map(
        &mut self,
        request: &NativeSpatialMapRequest,
    ) -> Result<NativeSpatialMapLease, CsharpEngineServicesError> {
        let count = request
            .columns
            .checked_mul(request.rows)
            .filter(|count| *count > 0 && *count <= MAX_MAP_CELLS)
            .ok_or_else(|| spatial_error("CSHARP_SPATIAL_MAP", "map requires 1..1024 cells"))?;
        let x0 = f64::from(request.origin.x);
        let z0 = f64::from(request.origin.z);
        let x1 = x0 + f64::from(request.columns) * request.cell_size;
        let z1 = z0 + f64::from(request.rows) * request.cell_size;
        if ![
            x0,
            z0,
            x1,
            z1,
            request.cell_size,
            request.collision_min_y,
            request.collision_max_y,
            request.navigation_min_y,
            request.navigation_max_y,
        ]
        .iter()
        .all(|value| value.is_finite())
            || request.cell_size <= 0.0
            || x1 <= x0
            || z1 <= z0
            || request.collision_min_y >= request.collision_max_y
            || request.navigation_min_y > request.navigation_max_y
        {
            return Err(spatial_error(
                "CSHARP_SPATIAL_MAP",
                "invalid map bounds or scale",
            ));
        }
        let entities =
            unsafe { borrowed_slice(request.entities, request.entities_len, "map colliders") }?;
        let records = filtered_entities(entities, NativeSpatialQueryFilter::default(), &[])?;
        let identity = self.collision_source.cursor_identity(request.session)?;
        let session = self
            .sessions
            .get(&request.session.value)
            .ok_or_else(|| spatial_error("CSHARP_SPATIAL_MAP", "unknown spatial session"))?;
        let mut columns: BTreeMap<(i64, i64), Vec<(f64, bool)>> = BTreeMap::new();
        if let Some(nav) = &session.navigation {
            for cell in nav.projection.walkable_cells() {
                let center = nav.cell_center(cell);
                let y = f64::from(center.y);
                if y >= request.navigation_min_y && y <= request.navigation_max_y {
                    let [x, _, z] = cell.to_array();
                    columns
                        .entry((x, z))
                        .or_default()
                        .push((y, nav.traversal.is_allowed(cell)));
                }
            }
        }
        let mut cells = Vec::with_capacity(count as usize);
        for row in 0..request.rows {
            for column in 0..request.columns {
                let min = [
                    x0 + f64::from(column) * request.cell_size,
                    request.collision_min_y,
                    z0 + f64::from(row) * request.cell_size,
                ];
                let max = [
                    min[0] + request.cell_size,
                    request.collision_max_y,
                    min[2] + request.cell_size,
                ];
                let mut cell = NativeSpatialMapCell {
                    static_collision: session.scene.aabb_overlaps_solid(min, max),
                    ..Default::default()
                };
                for record in records
                    .iter()
                    .filter(|record| record.enabled && !record.trigger)
                {
                    let (lo, hi) = collider_bounds(*record);
                    if aabb_overlaps(min, max, lo, hi) {
                        cell.dynamic_collision_count += 1;
                        if cell.first_dynamic_entity == 0
                            || record.entity < cell.first_dynamic_entity
                        {
                            cell.first_dynamic_entity = record.entity;
                        }
                    }
                }
                if let Some(nav) = &session.navigation {
                    let sample = nav
                        .projection
                        .grid()
                        .world_to_voxel(core_space::WorldPos::new(
                            (min[0] + max[0]) * 0.5,
                            0.0,
                            (min[2] + max[2]) * 0.5,
                        ))
                        .to_array();
                    if let Some(supports) = columns.get(&(sample[0], sample[2])) {
                        cell.minimum_support_y =
                            supports.iter().map(|s| s.0).fold(f64::INFINITY, f64::min);
                        cell.maximum_support_y = supports
                            .iter()
                            .map(|s| s.0)
                            .fold(f64::NEG_INFINITY, f64::max);
                        cell.navigation_samples =
                            checked_u32(supports.len(), "map navigation samples")?;
                        cell.navigation_allowed_samples = checked_u32(
                            supports.iter().filter(|s| s.1).count(),
                            "map allowed samples",
                        )?;
                    }
                }
                cells.push(cell);
            }
        }
        let cells = cells.into_boxed_slice();
        let handle = self.next_map_lease;
        self.next_map_lease = handle
            .checked_add(1)
            .ok_or_else(|| spatial_error("CSHARP_SPATIAL_MAP", "map lease handles exhausted"))?;
        let result = NativeSpatialMapLease {
            handle: NativeSpatialMapLeaseHandle { value: handle },
            cells: cells.as_ptr(),
            cells_len: cells.len(),
            projection_identity: identity,
            source_revision: session.scene.source_revision().raw(),
            collision_revision: session.scene.static_mesh_collision_revision(),
            navigation_revision: session.navigation_revision,
            navigation_present: session.navigation.is_some(),
        };
        self.map_leases.insert(handle, cells);
        Ok(result)
    }
}

pub(super) unsafe extern "C" fn read_map(
    context: *mut c_void,
    request: *const NativeSpatialMapRequest,
    result: *mut NativeSpatialMapLease,
) -> i32 {
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeSpatialBridge>() }.read_map(unsafe { &*request }) {
        Ok(value) => {
            unsafe { *result = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}

pub(super) unsafe extern "C" fn destroy_map_lease(
    context: *mut c_void,
    handle: NativeSpatialMapLeaseHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    i32::from(
        unsafe { &mut *context.cast::<RuntimeSpatialBridge>() }
            .map_leases
            .remove(&handle.value)
            .is_some(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_observes_door_changes_without_mutating_navigation_and_preserves_layers() {
        let mut bridge = RuntimeSpatialBridge::new();
        let session = bridge
            .create(NativeSpatialSessionConfig {
                collision_voxel_size: 1.0,
                collision_chunk_size: 8,
                voxel_surface_mode: NativeVoxelSurfaceMode::GreedyCubes,
            })
            .unwrap();
        let nav_cells = [
            NativePlanarNavCell { x: 0, y: 0, z: 0 },
            NativePlanarNavCell { x: 0, y: 2, z: 0 },
        ];
        bridge
            .replace_navigation(&NativeNavigationReplaceRequest {
                session,
                config: NativePlanarNavConfig {
                    grid_id: 1,
                    cell_size: 1.0,
                    chunk_size: 8,
                    max_step_cells: 1,
                },
                cells: nav_cells.as_ptr(),
                cells_len: nav_cells.len(),
            })
            .unwrap();
        let scene = Arc::new(VoxelCollisionScene::from_solid_voxels(1.0, 8, [[1, 1, 0]]).unwrap());
        bridge.session_mut(session).unwrap().scene = scene.clone();
        bridge.publish_scene(session, scene);
        let mut door = NativeSpatialEntityCollider {
            entity: 17,
            min: NativeVec3 {
                x: 0.1,
                y: 0.0,
                z: 0.1,
            },
            max: NativeVec3 {
                x: 0.9,
                y: 2.0,
                z: 0.9,
            },
            enabled: true,
            ..Default::default()
        };
        let mut request = NativeSpatialMapRequest {
            session,
            origin: NativeVec3::default(),
            cell_size: 1.0,
            columns: 2,
            rows: 1,
            collision_min_y: 0.1,
            collision_max_y: 1.8,
            navigation_min_y: 0.0,
            navigation_max_y: 3.0,
            entities: &door,
            entities_len: 1,
        };
        let closed = bridge.read_map(&request).unwrap();
        let cells = unsafe { std::slice::from_raw_parts(closed.cells, closed.cells_len) };
        assert_eq!(cells[0].first_dynamic_entity, 17);
        assert_eq!(cells[0].navigation_samples, 2);
        assert_eq!(cells[0].navigation_allowed_samples, 2);
        assert!(cells[1].static_collision);
        assert_eq!(cells[1].navigation_samples, 0); // No false floor outside admitted navigation.
        assert!(cells[0].maximum_support_y > cells[0].minimum_support_y);
        door.enabled = false;
        request.entities = &door;
        let open = bridge.read_map(&request).unwrap();
        let open_cells = unsafe { std::slice::from_raw_parts(open.cells, open.cells_len) };
        assert_eq!(open_cells[0].dynamic_collision_count, 0);
        assert_eq!(open.navigation_revision, closed.navigation_revision);
        assert_eq!(open.projection_identity, closed.projection_identity);
        assert_eq!(cells[0].dynamic_collision_count, 1); // Earlier copied lease remains independent.
        assert!(bridge.map_leases.remove(&closed.handle.value).is_some());
        assert!(bridge.map_leases.remove(&open.handle.value).is_some());
        request.columns = 1025;
        assert!(bridge.read_map(&request).is_err());
        assert!(bridge.map_leases.is_empty());
    }
}
