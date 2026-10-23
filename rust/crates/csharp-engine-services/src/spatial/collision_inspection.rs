//! Each Spatial session's colliders for a collision inspection
//! ([`crate::collision_inspection`]).
use super::*;
use crate::collision_inspection::{CollisionLineKind, CollisionSource, Region, Wires};

impl RuntimeSpatialBridge {
    /// Every session's character capsule, last step's obstacle boxes and
    /// active triggers meeting `region`.
    pub(crate) fn inspect_primitives(&self, region: Region, wires: &mut Wires) {
        for (&id, session) in &self.sessions {
            if let (Some(receipt), Some(config)) = (
                &session.last_character_receipt,
                session.last_character_config,
            ) {
                let height = match receipt.stance.accepted {
                    CharacterStance::Standing => config.shape.standing_height,
                    CharacterStance::Crouched => config.shape.crouched_height,
                };
                let radius = f64::from(config.shape.radius);
                let half_height = (f64::from(height) * 0.5 - radius).max(0.0);
                let center = receipt.transform_after.translation;
                let center = [center.x, center.y, center.z].map(f64::from);
                let reach = [radius, half_height + radius, radius];
                let bounds = (
                    std::array::from_fn(|axis| center[axis] - reach[axis]),
                    std::array::from_fn(|axis| center[axis] + reach[axis]),
                );
                if region.meets(bounds.0, bounds.1) {
                    wires.primitive(
                        Some(id),
                        CollisionSource::Character,
                        bounds,
                        CollisionLineKind::Character,
                        |pen| pen.capsule(half_height, radius, [0.0, 0.0, 0.0, 1.0], center),
                    );
                }
            }
            for obstacle in &session.last_character_obstacles {
                let center = obstacle.center.to_array();
                let half = [
                    obstacle.half_extents.x,
                    obstacle.half_extents.y,
                    obstacle.half_extents.z,
                ];
                let bounds = (
                    std::array::from_fn(|axis| center[axis] - half[axis]),
                    std::array::from_fn(|axis| center[axis] + half[axis]),
                );
                if region.meets(bounds.0, bounds.1) {
                    wires.primitive(
                        Some(id),
                        CollisionSource::Obstacle {
                            entity: obstacle.id,
                        },
                        bounds,
                        CollisionLineKind::Obstacle,
                        |pen| pen.wire_box(bounds.0, bounds.1),
                    );
                }
            }
            for trigger in session.triggers.active_geometry() {
                let bounds = (
                    [trigger.min.x, trigger.min.y, trigger.min.z].map(f64::from),
                    [trigger.max.x, trigger.max.y, trigger.max.z].map(f64::from),
                );
                if region.meets(bounds.0, bounds.1) {
                    wires.primitive(
                        Some(id),
                        CollisionSource::Trigger {
                            entity: trigger.entity.raw(),
                        },
                        bounds,
                        CollisionLineKind::Trigger,
                        |pen| pen.wire_box(bounds.0, bounds.1),
                    );
                }
            }
        }
    }

    /// Every session's voxel chunks and static meshes meeting `region`,
    /// nearest its centre first, within the lines left.
    pub(crate) fn inspect_projections(&self, region: Region, wires: &mut Wires) {
        for (&id, session) in &self.sessions {
            let slope = session.last_character_config.map_or_else(
                || {
                    CharacterControllerConfig::default()
                        .surface
                        .maximum_slope_radians
                },
                |config| config.surface.maximum_slope_radians,
            );
            let slope_cos = f64::from(slope.min(std::f32::consts::FRAC_PI_2).cos());
            let base = session.content.base.as_ref().map(|(_, content)| content);
            for collider in
                session
                    .scene
                    .inspect_collision_region(region.min, region.max, wires.budget)
            {
                let source = match collider.source {
                    engine_spatial::InspectedColliderSource::VoxelChunk(chunk) => {
                        CollisionSource::VoxelChunk(chunk.to_array())
                    }
                    engine_spatial::InspectedColliderSource::StaticMesh {
                        instance, asset, ..
                    } => {
                        let placed = session
                            .content
                            .placed
                            .get(&instance.0)
                            .filter(|placed| placed.asset() == Some(asset))
                            .map(|placed| &placed.content);
                        let based = base
                            .filter(|base| base.asset() == Some(asset) && instance.0 == asset.0);
                        match (based, placed) {
                            (Some(content), _) | (None, Some(content)) => {
                                CollisionSource::ContentArtifact {
                                    path: content.path.clone(),
                                    instance: instance.0,
                                    base: based.is_some(),
                                }
                            }
                            (None, None) => CollisionSource::StaticMesh {
                                instance: instance.0,
                                asset: asset.0,
                            },
                        }
                    }
                };
                wires.projected(id, source, &collider, slope_cos);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision_inspection::{CollisionInspection, Region, Wires};

    fn vec3(x: f32, y: f32, z: f32) -> NativeVec3 {
        NativeVec3 { x, y, z }
    }

    /// A session with a 4x4 voxel floor under y = 1, a wall triangle and a
    /// floor triangle of one static mesh, and a character stepped beside an
    /// obstacle box.
    fn session() -> (RuntimeSpatialBridge, NativeSpatialSessionHandle) {
        let mut bridge = RuntimeSpatialBridge::new();
        let session = bridge
            .create(NativeSpatialSessionConfig {
                collision_voxel_size: 1.0,
                collision_chunk_size: 8,
                voxel_surface_mode: NativeVoxelSurfaceMode::GreedyCubes,
            })
            .unwrap();
        let mut scene = VoxelCollisionScene::from_solid_voxels(
            1.0,
            8,
            (0..4).flat_map(|x| (0..4).map(move |z| [x, 0, z])),
        )
        .unwrap();
        let asset = StaticMeshColliderAsset::new(
            StaticMeshAssetId(7),
            vec![
                [0.0, 1.0, 3.0],
                [0.0, 3.0, 3.0],
                [2.0, 1.0, 3.0],
                [2.0, 1.0, 2.0],
            ],
            // A wall facing -z, then a floor facing up.
            vec![[0, 1, 2], [0, 2, 3]],
        )
        .unwrap();
        scene
            .replace_static_mesh_colliders(
                [asset],
                [StaticMeshColliderInstance {
                    id: StaticMeshInstanceId(9),
                    asset: StaticMeshAssetId(7),
                    transform: StaticMeshTransform::IDENTITY,
                }],
            )
            .unwrap();
        let scene = Arc::new(scene);
        bridge.sessions.get_mut(&session.value).unwrap().scene = Arc::clone(&scene);
        bridge.publish_scene(session, scene);
        let obstacles = [NativeCharacterObstacle {
            entity: 5,
            transform: NativeTransform {
                translation: vec3(3.0, 1.5, 0.5),
                rotation: NativeQuat {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                    w: 1.0,
                },
                scale: vec3(1.0, 1.0, 1.0),
            },
            bounds_min: vec3(-0.5, -0.5, -0.5),
            bounds_max: vec3(0.5, 0.5, 0.5),
            collision_enabled: true,
            linear_velocity: NativeVec3::default(),
            angular_velocity: NativeVec3::default(),
        }];
        let config = bridge.default_character_controller_config();
        bridge
            .propose_character(NativeCharacterStepRequest {
                tether: NativeCharacterTetherRequest::default(),
                session,
                position: vec3(1.0, 2.0, 1.0),
                motion: NativeCharacterMotion {
                    stance: NativeCharacterStance::Standing,
                    fall_origin_y: 2.0,
                    peak_y: 2.0,
                    ..Default::default()
                },
                support: NativeCharacterSupport::default(),
                obstacles: obstacles.as_ptr(),
                obstacles_len: obstacles.len(),
                mesh_instances: std::ptr::null(),
                mesh_instances_len: 0,
                config,
                command: NativeCharacterControllerCommand {
                    movement: Default::default(),
                    planar_intent: NativeVec2::default(),
                    heading_yaw_radians: 0.0,
                    jump_pressed: false,
                    jump_held: false,
                    crouch_requested: false,
                    external_velocity: NativeVec3::default(),
                    external_impulse: NativeVec3::default(),
                    step_seconds: 1.0 / 60.0,
                    sequence: 1,
                },
            })
            .unwrap();
        (bridge, session)
    }

    fn inspect(
        bridge: &RuntimeSpatialBridge,
        center: [f64; 3],
        radius: f64,
        lines: usize,
    ) -> CollisionInspection {
        let region = Region::around(center, radius);
        let mut inspection = CollisionInspection::default();
        let mut wires = Wires {
            inspection: &mut inspection,
            budget: lines,
        };
        bridge.inspect_primitives(region, &mut wires);
        bridge.inspect_projections(region, &mut wires);
        inspection
    }

    #[test]
    fn an_inspection_draws_each_collider_near_the_point_and_lists_it() {
        let (bridge, session) = session();
        let inspection = inspect(&bridge, [1.0, 1.0, 1.0], 4.0, usize::MAX);
        let sources: Vec<&CollisionSource> = inspection
            .colliders
            .iter()
            .map(|collider| &collider.source)
            .collect();
        assert_eq!(
            sources,
            [
                &CollisionSource::Character,
                &CollisionSource::Obstacle { entity: 5 },
                &CollisionSource::VoxelChunk([0, 0, 0]),
                &CollisionSource::StaticMesh {
                    instance: 9,
                    asset: 7
                },
            ]
        );
        assert!(inspection
            .colliders
            .iter()
            .all(|collider| collider.session == Some(session.value) && collider.omitted == 0));
        assert_eq!(
            inspection.lines.len(),
            inspection
                .colliders
                .iter()
                .map(|collider| collider.lines)
                .sum::<usize>()
        );
        // The obstacle's box is its bounds at its translation.
        assert_eq!(inspection.colliders[1].min, [2.5, 1.0, 0.0]);
        assert_eq!(inspection.colliders[1].max, [3.5, 2.0, 1.0]);
        // The wall is steeper than the slope limit, the floor is not.
        let mesh: Vec<bool> = inspection
            .lines
            .iter()
            .filter(|line| line.kind == CollisionLineKind::StaticMesh)
            .map(|line| line.steep)
            .collect();
        assert_eq!(mesh, [true, true, true, false, false, false]);
        assert!(inspection
            .lines
            .iter()
            .any(|line| line.kind == CollisionLineKind::Voxel));
    }

    #[test]
    fn an_inspection_leaves_out_what_is_far_and_says_what_the_cap_cut() {
        let (bridge, _) = session();
        let far = inspect(&bridge, [40.0, 1.0, 40.0], 4.0, usize::MAX);
        assert!(far.colliders.is_empty() && far.lines.is_empty());
        // Room for the character only: everything after it is left out and
        // still listed.
        let character = inspect(&bridge, [1.0, 1.0, 1.0], 4.0, usize::MAX).colliders[0].lines;
        let cut = inspect(&bridge, [1.0, 1.0, 1.0], 4.0, character);
        assert_eq!(cut.colliders.len(), 4);
        assert_eq!(cut.lines.len(), character);
        assert_eq!(cut.colliders[1].omitted, 1);
        assert!(cut.colliders[2].omitted > 0 && cut.colliders[3].omitted == 2);
        assert_eq!(cut.omitted(), 1 + cut.colliders[2].omitted + 2);
    }
}
