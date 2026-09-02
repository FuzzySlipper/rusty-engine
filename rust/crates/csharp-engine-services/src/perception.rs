use std::{collections::BTreeMap, ffi::c_void};

use csharp_engine_abi::*;
use engine_spatial::{
    SpatialPerceptionObserver, SpatialPerceptionPairKind, SpatialPerceptionQuery,
    SpatialPerceptionService, SpatialPerceptionTarget,
};

use crate::{
    composition::{borrowed_slice, ABI_OK},
    spatial::{entity_state, native_array, RuntimeSpatialBridge, SpatialCollisionSource},
    CsharpEngineServicesError,
};

struct PerceptionReadoutLeaseBacking {
    _pairs: Box<[NativePerceptionPair]>,
    _aggregates: Box<[NativePerceptionAggregate]>,
}

/// Named C# perception bridge. The scene remains owned by Spatial; this bridge only retains
/// copied readout leases until generated C# releases them.
pub(crate) struct RuntimePerceptionBridge {
    collision_source: SpatialCollisionSource,
    readout_leases: BTreeMap<u64, PerceptionReadoutLeaseBacking>,
    next_readout_lease: u64,
}

impl RuntimePerceptionBridge {
    pub(crate) fn new(spatial: &RuntimeSpatialBridge) -> Self {
        Self {
            collision_source: spatial.collision_source(),
            readout_leases: BTreeMap::new(),
            next_readout_lease: 1,
        }
    }

    fn query(
        &mut self,
        request: &NativePerceptionQueryRequest,
    ) -> Result<NativePerceptionReadoutLease, CsharpEngineServicesError> {
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
        let entities = entity_state(occluders)?;
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
                    entities: &entities,
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
        let lease = self.next_readout_lease;
        self.next_readout_lease = lease.checked_add(1).ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_PERCEPTION",
                "perception readout lease handles exhausted",
            )
        })?;
        let result = NativePerceptionReadoutLease {
            handle: NativePerceptionReadoutLeaseHandle { value: lease },
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
        self.readout_leases.insert(
            lease,
            PerceptionReadoutLeaseBacking {
                _pairs: pairs,
                _aggregates: aggregates,
            },
        );
        Ok(result)
    }

    fn destroy_readout_lease(&mut self, handle: NativePerceptionReadoutLeaseHandle) -> bool {
        self.readout_leases.remove(&handle.value).is_some()
    }
}

pub(crate) fn api(bridge: &mut RuntimePerceptionBridge) -> NativePerceptionApi {
    NativePerceptionApi {
        context: (bridge as *mut RuntimePerceptionBridge).cast(),
        query_visibility: query_perception,
        destroy_readout_lease,
    }
}

unsafe extern "C" fn query_perception(
    context: *mut c_void,
    request: *const NativePerceptionQueryRequest,
    result: *mut NativePerceptionReadoutLease,
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

unsafe extern "C" fn destroy_readout_lease(
    context: *mut c_void,
    handle: NativePerceptionReadoutLeaseHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimePerceptionBridge>() };
    if bridge.destroy_readout_lease(handle) {
        ABI_OK
    } else {
        0
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
    fn native_query_copies_typed_pairs_and_aggregates_and_releases_lease() {
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
        let mut result = NativePerceptionReadoutLease::default();
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
            unsafe {
                (perception_api.destroy_readout_lease)(perception_api.context, result.handle)
            },
            ABI_OK
        );
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
        let mut first = NativePerceptionReadoutLease::default();
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
        assert_eq!(
            unsafe { (perception_api.destroy_readout_lease)(perception_api.context, first.handle) },
            ABI_OK
        );

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
        let mut continued = NativePerceptionReadoutLease::default();
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
