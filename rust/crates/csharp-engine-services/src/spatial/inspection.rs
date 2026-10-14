use super::*;

impl RuntimeSpatialBridge {
    fn read_map(
        &mut self,
        request: &NativeSpatialMapRequest,
    ) -> Result<NativeSpatialMapResult, CsharpEngineServicesError> {
        let count = request
            .columns
            .checked_mul(request.rows)
            .ok_or_else(|| spatial_error("CSHARP_SPATIAL_MAP", "map cell count overflows u32"))?;
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
        {
            return Err(spatial_error(
                "CSHARP_SPATIAL_MAP",
                "invalid map bounds or scale",
            ));
        }
        // Each Y interval lies between its two values in either order; the
        // collision query, the collider overlap and the support filter all
        // read the ordered pair.
        let ordered = |a: f64, b: f64| (a.min(b), a.max(b));
        let (collision_min_y, collision_max_y) =
            ordered(request.collision_min_y, request.collision_max_y);
        let (navigation_min_y, navigation_max_y) =
            ordered(request.navigation_min_y, request.navigation_max_y);
        let entities =
            unsafe { borrowed_slice(request.entities, request.entities_len, "map colliders") }?;
        let records = filtered_entities(entities, NativeSpatialQueryFilter::default(), &[]);
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
                if y >= navigation_min_y && y <= navigation_max_y {
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
                    collision_min_y,
                    z0 + f64::from(row) * request.cell_size,
                ];
                let max = [
                    min[0] + request.cell_size,
                    collision_max_y,
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
                    let [x, y, z] =
                        nav.grid_position([(min[0] + max[0]) * 0.5, 0.0, (min[2] + max[2]) * 0.5]);
                    let sample = nav
                        .projection
                        .grid()
                        .world_to_voxel(core_space::WorldPos::new(x, y, z))
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
        let result = NativeSpatialMapResult {
            cells: cells.as_ptr(),
            cells_len: cells.len(),
            projection_identity: identity,
            source_revision: session.scene.source_revision().raw(),
            collision_revision: session.scene.static_mesh_collision_revision(),
            navigation_revision: session.navigation_revision,
            navigation_present: session.navigation.is_some(),
        };
        self.borrowed.hold(cells);
        Ok(result)
    }
}

pub(super) unsafe extern "C" fn read_map(
    context: *mut c_void,
    request: *const NativeSpatialMapRequest,
    result: *mut NativeSpatialMapResult,
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
        // Copied, as the generated caller does: the next call replaces it.
        let cells = unsafe { std::slice::from_raw_parts(closed.cells, closed.cells_len) }.to_vec();
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
        assert_eq!(cells[0].dynamic_collision_count, 1); // The earlier copy is independent.

        // A map is as large as the product asks for.
        request.columns = 64;
        request.rows = 64;
        let large = bridge.read_map(&request).unwrap();
        let large_cells = unsafe { std::slice::from_raw_parts(large.cells, large.cells_len) };
        assert_eq!(large_cells.len(), 4096);
        assert!(large_cells[1].static_collision);
        assert_eq!(large_cells[0].navigation_samples, 2);

        // Each Y interval reads the same in either order, for retained
        // collision, supplied colliders and navigation supports alike.
        let shelf = NativeSpatialEntityCollider {
            entity: 18,
            min: NativeVec3 {
                x: 0.1,
                y: 1.0,
                z: 0.1,
            },
            max: NativeVec3 {
                x: 0.9,
                y: 1.2,
                z: 0.9,
            },
            enabled: true,
            ..Default::default()
        };
        request.columns = 2;
        request.entities = &shelf;
        let read = |bridge: &mut RuntimeSpatialBridge, request: &NativeSpatialMapRequest| {
            let result = bridge.read_map(request).unwrap();
            format!("{:?}", unsafe {
                std::slice::from_raw_parts(result.cells, result.cells_len)
            })
        };
        let ordered = read(&mut bridge, &request);
        assert!(ordered.contains("first_dynamic_entity: 18"));
        assert!(ordered.contains("navigation_samples: 2"));
        assert!(ordered.contains("static_collision: true"));
        let reversed = NativeSpatialMapRequest {
            collision_min_y: request.collision_max_y,
            collision_max_y: request.collision_min_y,
            navigation_min_y: request.navigation_max_y,
            navigation_max_y: request.navigation_min_y,
            ..request
        };
        assert_eq!(read(&mut bridge, &reversed), ordered);

        request.columns = 0;
        assert!(bridge.read_map(&request).is_err());
    }
}
