use std::ffi::c_void;

use csharp_engine_abi::*;
use engine_spatial::{
    SpatialOcclusionCollider, SpatialPerceptionObserver, SpatialPerceptionPairKind,
    SpatialPerceptionQuery, SpatialPerceptionService, SpatialPerceptionTarget,
};

use crate::{
    composition::{borrowed_slice, ABI_OK},
    spatial::{native_array, RuntimeSpatialBridge, SpatialCollisionSource},
    CsharpEngineServicesError,
};

/// Named C# perception bridge. The scene remains owned by Spatial; this bridge
/// only keeps its latest borrowed readout until the next call.
pub(crate) struct RuntimePerceptionBridge {
    collision_source: SpatialCollisionSource,
    borrowed: crate::operation_diagnostics::BorrowedResult,
}

impl RuntimePerceptionBridge {
    pub(crate) fn new(spatial: &RuntimeSpatialBridge) -> Self {
        Self {
            collision_source: spatial.collision_source(),
            borrowed: Default::default(),
        }
    }

    fn query(
        &mut self,
        request: &NativePerceptionQueryRequest,
    ) -> Result<NativePerceptionReadoutResult, CsharpEngineServicesError> {
        let observers = unsafe {
            borrowed_slice(
                request.observers,
                request.observers_len,
                "perception observers",
            )
        }?;
        let targets =
            unsafe { borrowed_slice(request.targets, request.targets_len, "perception targets") }?;
        let occluders = unsafe {
            borrowed_slice(
                request.occluders,
                request.occluders_len,
                "perception occluders",
            )
        }?;
        let scene = self.collision_source.scene(request.session)?;
        let projection_identity = self.collision_source.cursor_identity(request.session)?;
        if request.expected_projection_identity != 0
            && request.expected_projection_identity != projection_identity
        {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_PERCEPTION_STALE_CURSOR",
                "perception continuation projection identity changed",
            ));
        }
        let pair_cursor = usize::try_from(request.pair_cursor).map_err(|_| {
            CsharpEngineServicesError::new("CSHARP_PERCEPTION", "perception pair cursor overflow")
        })?;
        let page_size = usize::try_from(request.page_size).map_err(|_| {
            CsharpEngineServicesError::new("CSHARP_PERCEPTION", "perception page size overflow")
        })?;
        // Disabled colliders do not block sight.
        let occluders = occluders
            .iter()
            .filter(|value| value.enabled)
            .map(|value| SpatialOcclusionCollider {
                entity: core_ids::EntityId::new(value.entity),
                min: native_array(value.min),
                max: native_array(value.max),
            })
            .collect::<Vec<_>>();
        let observers = observers
            .iter()
            .map(|value| SpatialPerceptionObserver {
                entity: core_ids::EntityId::new(value.entity),
                origin: native_array(value.origin),
                forward: native_array(value.forward),
                maximum_distance: value.maximum_distance,
                minimum_facing_cosine: value.minimum_facing_cosine,
                evidence: value.evidence,
            })
            .collect::<Vec<_>>();
        let targets = targets
            .iter()
            .map(|value| SpatialPerceptionTarget {
                entity: core_ids::EntityId::new(value.entity),
                center: native_array(value.center),
            })
            .collect::<Vec<_>>();
        let readout = SpatialPerceptionService
            .evaluate_page(
                SpatialPerceptionQuery {
                    scene: scene.as_ref(),
                    occluders: &occluders,
                    observers: &observers,
                    targets: &targets,
                },
                pair_cursor,
                page_size,
            )
            .map_err(|error| {
                CsharpEngineServicesError::new("CSHARP_PERCEPTION", error.to_string())
            })?;
        let pairs = readout
            .pairs
            .iter()
            .copied()
            .map(native_pair)
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let aggregates = readout
            .aggregates
            .iter()
            .copied()
            .map(|value| NativePerceptionAggregate {
                target: value.target.raw(),
                visible_observer_count: value.visible_observer_count,
                evidence_total: value.evidence_total,
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let result = NativePerceptionReadoutResult {
            pairs: pointer_or_null(&pairs),
            pairs_len: pairs.len(),
            aggregates: pointer_or_null(&aggregates),
            aggregates_len: aggregates.len(),
            pair_total: checked_count(readout.pair_total, "pairs")?,
            has_next_pair_cursor: readout.next_pair_cursor.is_some(),
            next_pair_cursor: readout
                .next_pair_cursor
                .map(|value| checked_count(value, "pair cursor"))
                .transpose()?
                .unwrap_or_default(),
            projection_identity,
            selected_observers: checked_count(readout.selected_observers, "observers")?,
            selected_targets: checked_count(readout.selected_targets, "targets")?,
            selection_comparisons: readout.selection_comparisons as u64,
            distance_rejects: checked_count(readout.distance_rejects, "distance rejects")?,
            facing_rejects: checked_count(readout.facing_rejects, "facing rejects")?,
            visibility_casts: checked_count(readout.visibility_casts, "visibility casts")?,
            occlusion_rejects: checked_count(readout.occlusion_rejects, "occlusion rejects")?,
        };
        self.borrowed.hold((pairs, aggregates));
        Ok(result)
    }
}

pub(crate) fn api(bridge: &mut RuntimePerceptionBridge) -> NativePerceptionApi {
    NativePerceptionApi {
        context: (bridge as *mut RuntimePerceptionBridge).cast(),
        query_visibility: query_perception,
    }
}

unsafe extern "C" fn query_perception(
    context: *mut c_void,
    request: *const NativePerceptionQueryRequest,
    result: *mut NativePerceptionReadoutResult,
) -> i32 {
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimePerceptionBridge>() };
    match bridge.query(unsafe { &*request }) {
        Ok(value) => {
            unsafe { *result = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}

fn native_pair(value: engine_spatial::SpatialPerceptionPair) -> NativePerceptionPair {
    NativePerceptionPair {
        observer: value.observer.raw(),
        target: value.target.raw(),
        distance: value.distance,
        facing_cosine: value.facing_cosine,
        kind: match value.kind {
            SpatialPerceptionPairKind::Visible => NativePerceptionPairKind::Visible,
            SpatialPerceptionPairKind::FacingRejected => NativePerceptionPairKind::FacingRejected,
            SpatialPerceptionPairKind::Occluded => NativePerceptionPairKind::Occluded,
        },
        evidence: value.evidence,
    }
}

fn pointer_or_null<T>(values: &[T]) -> *const T {
    if values.is_empty() {
        std::ptr::null()
    } else {
        values.as_ptr()
    }
}

fn checked_count(value: usize, field: &'static str) -> Result<u32, CsharpEngineServicesError> {
    u32::try_from(value).map_err(|_| {
        CsharpEngineServicesError::new("CSHARP_PERCEPTION", format!("{field} exceeded u32"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spatial;
    use std::sync::Arc;

    #[test]
    fn native_query_copies_typed_pairs_and_aggregates() {
        let mut spatial_bridge = RuntimeSpatialBridge::new();
        let mut perception_bridge = RuntimePerceptionBridge::new(&spatial_bridge);
        let spatial_api = spatial::api(&mut spatial_bridge);
        let mut session = NativeSpatialSessionHandle::default();
        assert_eq!(
            unsafe {
                (spatial_api.create_session)(
                    spatial_api.context,
                    NativeSpatialSessionConfig {
                        collision_voxel_size: 1.0,
                        collision_chunk_size: 8,
                        voxel_surface_mode: NativeVoxelSurfaceMode::GreedyCubes,
                    },
                    &mut session,
                )
            },
            ABI_OK
        );

        let observers = [NativePerceptionObserver {
            entity: 1,
            origin: NativeVec3::default(),
            forward: NativeVec3 {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            },
            maximum_distance: 10.0,
            minimum_facing_cosine: 0.5,
            evidence: 0.75,
        }];
        let targets = [NativePerceptionTarget {
            entity: 2,
            center: NativeVec3 {
                x: 4.0,
                y: 0.0,
                z: 0.0,
            },
        }];
        let occluders: [NativeSpatialEntityCollider; 0] = [];
        let request = NativePerceptionQueryRequest {
            session,
            observers: observers.as_ptr(),
            observers_len: observers.len(),
            targets: targets.as_ptr(),
            targets_len: targets.len(),
            occluders: std::ptr::null(),
            occluders_len: occluders.len(),
            expected_projection_identity: 0,
            pair_cursor: 0,
            page_size: 64,
        };
        let perception_api = api(&mut perception_bridge);
        let mut result = NativePerceptionReadoutResult::default();
        assert_eq!(
            unsafe {
                (perception_api.query_visibility)(perception_api.context, &request, &mut result)
            },
            ABI_OK
        );
        assert_eq!(result.pairs_len, 1);
        assert_eq!(result.aggregates_len, 1);
        assert_eq!(result.selected_observers, 1);
        assert_eq!(result.selected_targets, 1);
        assert_eq!(result.visibility_casts, 1);
        assert_eq!(result.occlusion_rejects, 0);
        let pairs = unsafe { std::slice::from_raw_parts(result.pairs, result.pairs_len) };
        assert_eq!(pairs[0].kind, NativePerceptionPairKind::Visible);
        let aggregates =
            unsafe { std::slice::from_raw_parts(result.aggregates, result.aggregates_len) };
        assert_eq!(aggregates[0].target, 2);
        assert_eq!(aggregates[0].visible_observer_count, 1);
        assert_eq!(
            unsafe { (spatial_api.destroy_session)(spatial_api.context, session) },
            ABI_OK
        );
    }

    #[test]
    fn native_visibility_is_blocked_only_by_enabled_occluder_rows() {
        let mut spatial_bridge = RuntimeSpatialBridge::new();
        let mut perception_bridge = RuntimePerceptionBridge::new(&spatial_bridge);
        let spatial_api = spatial::api(&mut spatial_bridge);
        let mut session = NativeSpatialSessionHandle::default();
        assert_eq!(
            unsafe {
                (spatial_api.create_session)(
                    spatial_api.context,
                    NativeSpatialSessionConfig {
                        collision_voxel_size: 1.0,
                        collision_chunk_size: 8,
                        voxel_surface_mode: NativeVoxelSurfaceMode::GreedyCubes,
                    },
                    &mut session,
                )
            },
            ABI_OK
        );
        let observers = [NativePerceptionObserver {
            entity: 1,
            origin: NativeVec3::default(),
            forward: NativeVec3 {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            },
            maximum_distance: 10.0,
            minimum_facing_cosine: 0.5,
            evidence: 1.0,
        }];
        let targets = [NativePerceptionTarget {
            entity: 2,
            center: NativeVec3 {
                x: 4.0,
                y: 0.0,
                z: 0.0,
            },
        }];
        let cube = |entity, x: f32, enabled| NativeSpatialEntityCollider {
            entity,
            min: NativeVec3 {
                x: x - 0.5,
                y: -0.5,
                z: -0.5,
            },
            max: NativeVec3 {
                x: x + 0.5,
                y: 0.5,
                z: 0.5,
            },
            enabled,
            ..Default::default()
        };
        let perception_api = api(&mut perception_bridge);
        let kind = |occluders: &[NativeSpatialEntityCollider]| {
            let request = NativePerceptionQueryRequest {
                session,
                observers: observers.as_ptr(),
                observers_len: observers.len(),
                targets: targets.as_ptr(),
                targets_len: targets.len(),
                occluders: occluders.as_ptr(),
                occluders_len: occluders.len(),
                expected_projection_identity: 0,
                pair_cursor: 0,
                page_size: 64,
            };
            let mut result = NativePerceptionReadoutResult::default();
            assert_eq!(
                unsafe {
                    (perception_api.query_visibility)(perception_api.context, &request, &mut result)
                },
                ABI_OK
            );
            let pair = unsafe { *result.pairs };
            pair.kind
        };

        // The target's own box never hides it.
        assert_eq!(
            kind(&[cube(2, 4.0, true)]),
            NativePerceptionPairKind::Visible
        );
        assert_eq!(
            kind(&[cube(2, 4.0, true), cube(5, 2.0, true)]),
            NativePerceptionPairKind::Occluded
        );
        assert_eq!(
            kind(&[cube(2, 4.0, true), cube(5, 2.0, false)]),
            NativePerceptionPairKind::Visible
        );
        assert_eq!(
            unsafe { (spatial_api.destroy_session)(spatial_api.context, session) },
            ABI_OK
        );
    }

    #[test]
    fn native_visibility_reports_a_retained_static_mesh_as_occluded() {
        let mut spatial_bridge = RuntimeSpatialBridge::new();
        let spatial_api = spatial::api(&mut spatial_bridge);
        let mut session = NativeSpatialSessionHandle::default();
        assert_eq!(
            unsafe {
                (spatial_api.create_session)(
                    spatial_api.context,
                    NativeSpatialSessionConfig {
                        collision_voxel_size: 1.0,
                        collision_chunk_size: 8,
                        voxel_surface_mode: NativeVoxelSurfaceMode::GreedyCubes,
                    },
                    &mut session,
                )
            },
            ABI_OK
        );
        let mut scene = engine_spatial::VoxelCollisionScene::from_solid_voxels(1.0, 8, [])
            .expect("empty scene");
        let asset = engine_spatial::StaticMeshColliderAsset::new(
            engine_spatial::StaticMeshAssetId(17),
            vec![[2.0, -1.0, -1.0], [2.0, 1.0, -1.0], [2.0, 0.0, 1.0]],
            vec![[0, 1, 2]],
        )
        .expect("valid retained mesh");
        scene
            .replace_static_mesh_colliders(
                [asset],
                [engine_spatial::StaticMeshColliderInstance {
                    id: engine_spatial::StaticMeshInstanceId(23),
                    asset: engine_spatial::StaticMeshAssetId(17),
                    transform: engine_spatial::StaticMeshTransform::IDENTITY,
                }],
            )
            .expect("retained mesh projection");
        spatial_bridge.publish_scene(session, Arc::new(scene));

        let mut perception_bridge = RuntimePerceptionBridge::new(&spatial_bridge);
        let perception_api = api(&mut perception_bridge);
        let observers = [NativePerceptionObserver {
            entity: 1,
            origin: NativeVec3::default(),
            forward: NativeVec3 {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            },
            maximum_distance: 10.0,
            minimum_facing_cosine: 0.5,
            evidence: 1.0,
        }];
        let targets = [NativePerceptionTarget {
            entity: 2,
            center: NativeVec3 {
                x: 4.0,
                y: 0.0,
                z: 0.0,
            },
        }];
        let mut result = NativePerceptionReadoutResult::default();
        assert_eq!(
            unsafe {
                (perception_api.query_visibility)(
                    perception_api.context,
                    &NativePerceptionQueryRequest {
                        session,
                        observers: observers.as_ptr(),
                        observers_len: observers.len(),
                        targets: targets.as_ptr(),
                        targets_len: targets.len(),
                        occluders: std::ptr::null(),
                        occluders_len: 0,
                        expected_projection_identity: 0,
                        pair_cursor: 0,
                        page_size: 1,
                    },
                    &mut result,
                )
            },
            ABI_OK
        );
        assert_eq!(result.pairs_len, 1);
        assert_eq!(result.aggregates_len, 0);
        assert_eq!(result.occlusion_rejects, 1);
        let pairs = unsafe { std::slice::from_raw_parts(result.pairs, result.pairs_len) };
        assert_eq!(pairs[0].kind, NativePerceptionPairKind::Occluded);
        assert_eq!(
            unsafe { (spatial_api.destroy_session)(spatial_api.context, session) },
            ABI_OK
        );
    }

    #[test]
    fn continuation_rejects_a_rebuilt_projection_that_reuses_its_local_version() {
        let mut spatial_bridge = RuntimeSpatialBridge::new();
        let mut perception_bridge = RuntimePerceptionBridge::new(&spatial_bridge);
        let spatial_api = spatial::api(&mut spatial_bridge);
        let mut session = NativeSpatialSessionHandle::default();
        assert_eq!(
            unsafe {
                (spatial_api.create_session)(
                    spatial_api.context,
                    NativeSpatialSessionConfig {
                        collision_voxel_size: 1.0,
                        collision_chunk_size: 8,
                        voxel_surface_mode: NativeVoxelSurfaceMode::GreedyCubes,
                    },
                    &mut session,
                )
            },
            ABI_OK
        );
        let observers = [NativePerceptionObserver {
            entity: 1,
            origin: NativeVec3::default(),
            forward: NativeVec3 {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            },
            maximum_distance: 10.0,
            minimum_facing_cosine: 0.5,
            evidence: 1.0,
        }];
        let targets = [
            NativePerceptionTarget {
                entity: 2,
                center: NativeVec3 {
                    x: 2.0,
                    y: 0.0,
                    z: 0.0,
                },
            },
            NativePerceptionTarget {
                entity: 3,
                center: NativeVec3 {
                    x: 3.0,
                    y: 0.0,
                    z: 0.0,
                },
            },
        ];
        let mut request = NativePerceptionQueryRequest {
            session,
            observers: observers.as_ptr(),
            observers_len: observers.len(),
            targets: targets.as_ptr(),
            targets_len: targets.len(),
            occluders: std::ptr::null(),
            occluders_len: 0,
            expected_projection_identity: 0,
            pair_cursor: 0,
            page_size: 1,
        };
        let perception_api = api(&mut perception_bridge);
        let mut first = NativePerceptionReadoutResult::default();
        assert_eq!(
            unsafe {
                (perception_api.query_visibility)(perception_api.context, &request, &mut first)
            },
            ABI_OK
        );
        assert!(first.has_next_pair_cursor);
        let initial_local_version = spatial_bridge
            .collision_source()
            .scene(session)
            .unwrap()
            .projection_version();
        assert_eq!(initial_local_version, 1);

        spatial_bridge.publish_scene(
            session,
            Arc::new(engine_spatial::VoxelCollisionScene::from_solid_voxels(1.0, 8, []).unwrap()),
        );
        assert_eq!(
            spatial_bridge
                .collision_source()
                .scene(session)
                .unwrap()
                .projection_version(),
            initial_local_version,
            "rebuilt scenes reset their projection-local counter",
        );
        request.expected_projection_identity = first.projection_identity;
        request.pair_cursor = first.next_pair_cursor;
        let mut continued = NativePerceptionReadoutResult::default();
        assert_eq!(
            unsafe {
                (perception_api.query_visibility)(perception_api.context, &request, &mut continued)
            },
            0,
            "publication identity must reject a cursor from the replaced scene",
        );
        assert_eq!(
            unsafe { (spatial_api.destroy_session)(spatial_api.context, session) },
            ABI_OK
        );
    }
}
