//! Heightfield colliders: a product's height function as static collision in
//! a session. Tiles are sampled off the update by the product, as a drawn
//! heightfield's are, and ride deferred collision batches (`static_deferred`)
//! into the scene, so movement, queries, contacts, Dynamics and navigation
//! read them as any static collision, and a world-origin rebase moves them
//! with the rest. `svc_mesh::heightfield_collider` keeps which tiles are wanted.

use std::collections::BTreeMap;

use csharp_engine_abi::*;
use engine_spatial::{
    StaticMeshAssetId, StaticMeshColliderInstance, StaticMeshInstanceId, StaticMeshTransform,
};
use svc_mesh::heightfield_collider::{CarriedTile, ColliderLayout, ColliderTiles};

use super::{borrowed_slice, static_deferred, RuntimeSpatialBridge, SpatialSession, ABI_OK};
use crate::CsharpEngineServicesError;

/// Tile collision takes its ids from here up, out of a product's way: each
/// collider has a span of them.
const FIRST_TILE_ID: u64 = 1 << 63;
const TILE_IDS_BITS: u32 = 40;

fn refused(message: impl Into<String>) -> CsharpEngineServicesError {
    CsharpEngineServicesError::new("CSHARP_HEIGHTFIELD_COLLIDER", message)
}

struct HeightfieldCollider {
    tiles: ColliderTiles,
    anchor: core_space::GlobalPosition,
}

/// A collider's handle and the tiles a deferred batch carries for it.
type BatchTiles = (u64, Vec<CarriedTile>);

/// A session's colliders, and the tiles each deferred batch carries.
#[derive(Default)]
pub(super) struct SessionColliders {
    colliders: BTreeMap<u64, HeightfieldCollider>,
    in_batch: BTreeMap<u64, Vec<BatchTiles>>,
}

impl SessionColliders {
    pub(super) fn has_batch(&self) -> bool {
        self.colliders
            .values()
            .any(|collider| collider.tiles.has_batch())
    }

    /// Deferred batch `number` installed, or was refused.
    pub(super) fn finished(&mut self, number: u64, installed: bool) {
        for (handle, tiles) in self.in_batch.remove(&number).into_iter().flatten() {
            if let Some(collider) = self.colliders.get_mut(&handle) {
                collider.tiles.finished(&tiles, installed);
            }
        }
    }
}

impl SpatialSession {
    /// Hand the tiles sampled so far to the deferred batches, in the frame
    /// the session stands in now: a rebase installs earlier batches first.
    pub(super) fn flush_heightfield_colliders(&mut self) {
        if !self.heightfield_colliders.has_batch() {
            return;
        }
        let frame = self.world_origin.origin().cell();
        let mut batch = static_deferred::StaticBatch {
            assets: Vec::new(),
            instances: Vec::new(),
            removed_assets: Vec::new(),
            removed_instances: Vec::new(),
        };
        let mut carried = Vec::new();
        for (handle, collider) in &mut self.heightfield_colliders.colliders {
            let tiles = collider.tiles.take_batch();
            let place: [f64; 3] = std::array::from_fn(|axis| {
                (i128::from(collider.anchor.cell()[axis]) - i128::from(frame[axis])) as f64
                    + collider.anchor.offset()[axis]
            });
            for tile in tiles.geometry {
                batch.instances.push(StaticMeshColliderInstance {
                    id: StaticMeshInstanceId(tile.id),
                    asset: StaticMeshAssetId(tile.id),
                    transform: StaticMeshTransform {
                        translation: [
                            place[0] + tile.origin[0],
                            place[1],
                            place[2] + tile.origin[1],
                        ],
                        rotation: [0.0, 0.0, 0.0, 1.0],
                        scale: [1.0; 3],
                    },
                });
                batch.assets.push((tile.id, tile.positions, tile.triangles));
            }
            batch
                .removed_instances
                .extend(tiles.removed.iter().copied().map(StaticMeshInstanceId));
            batch
                .removed_assets
                .extend(tiles.removed.into_iter().map(StaticMeshAssetId));
            carried.push((*handle, tiles.tiles));
        }
        // Tiles of holes alone have nothing to carry.
        if batch.assets.is_empty() && batch.removed_assets.is_empty() {
            return;
        }
        let number = self
            .static_deferred
            .push(static_deferred::DeferredBatch::Collision(batch));
        self.heightfield_colliders.in_batch.insert(number, carried);
        self.start_deferred();
    }
}

impl RuntimeSpatialBridge {
    fn collider_session(
        &mut self,
        handle: NativeHeightfieldColliderHandle,
    ) -> Result<&mut SpatialSession, CsharpEngineServicesError> {
        self.sessions
            .values_mut()
            .find(|session| {
                session
                    .heightfield_colliders
                    .colliders
                    .contains_key(&handle.value)
            })
            .ok_or_else(|| refused("C# used a heightfield collider that does not live"))
    }

    fn collider(
        &mut self,
        handle: NativeHeightfieldColliderHandle,
    ) -> Result<&mut ColliderTiles, CsharpEngineServicesError> {
        let session = self.collider_session(handle)?;
        let collider = session
            .heightfield_colliders
            .colliders
            .get_mut(&handle.value);
        Ok(&mut collider.expect("found in the session").tiles)
    }

    pub(super) fn create_collider(
        &mut self,
        request: NativeHeightfieldColliderCreateRequest,
    ) -> Result<NativeHeightfieldColliderHandle, CsharpEngineServicesError> {
        let layout = ColliderLayout {
            tile_cells: request.tile_cells,
            cell_size: request.cell_size,
            reach: request.reach,
        };
        if !layout.valid() {
            return Err(refused(
                "a heightfield collider's tiles have 1 to 256 cells, of a finite size above 0, within a finite reach of 0 or more",
            ));
        }
        let anchor = core_space::GlobalPosition::new(
            [
                request.anchor.cell_x,
                request.anchor.cell_y,
                request.anchor.cell_z,
            ],
            [
                request.anchor.offset_x,
                request.anchor.offset_y,
                request.anchor.offset_z,
            ],
        )
        .map_err(|_| refused("a heightfield collider's anchor must be a finite world position"))?;
        let value = self.next_heightfield_collider;
        let first_id = FIRST_TILE_ID + (value << TILE_IDS_BITS);
        let session = self.session_mut(request.session)?;
        session.heightfield_colliders.colliders.insert(
            value,
            HeightfieldCollider {
                tiles: ColliderTiles::new(layout, first_id),
                anchor,
            },
        );
        self.next_heightfield_collider += 1;
        Ok(NativeHeightfieldColliderHandle { value })
    }

    /// Its tiles leave the collision scene with the next batch.
    pub(super) fn destroy_collider(
        &mut self,
        handle: NativeHeightfieldColliderHandle,
    ) -> Result<(), CsharpEngineServicesError> {
        self.collider(handle)?.set_focuses(Vec::new());
        let session = self.collider_session(handle)?;
        session.flush_heightfield_colliders();
        session
            .heightfield_colliders
            .colliders
            .remove(&handle.value);
        Ok(())
    }

    pub(super) fn set_focuses(
        &mut self,
        request: &NativeHeightfieldColliderFocusRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let focuses = unsafe {
            borrowed_slice(
                request.focuses,
                request.focuses_len,
                "heightfield collider focuses",
            )?
        };
        if !focuses
            .iter()
            .all(|focus| focus.x.is_finite() && focus.z.is_finite())
        {
            return Err(refused("a heightfield collider's focuses must be finite"));
        }
        let points = focuses.iter().map(|focus| [focus.x, focus.z]).collect();
        self.collider(request.collider)?.set_focuses(points);
        Ok(())
    }

    pub(super) fn read_requests(
        &mut self,
        request: NativeHeightfieldColliderRequestsRequest,
    ) -> Result<NativeHeightfieldRequestsResult, CsharpEngineServicesError> {
        let tiles = self.collider(request.collider)?;
        let layout = *tiles.layout();
        let requests: Vec<NativeHeightfieldTileRequest> = tiles
            .requests(request.max_requests as usize)
            .into_iter()
            .map(|waiting| {
                let origin = layout.origin(waiting.tile);
                NativeHeightfieldTileRequest {
                    tile_cells: layout.tile_cells,
                    ring: 0,
                    x: waiting.tile.0,
                    z: waiting.tile.1,
                    revision: waiting.revision,
                    origin_x: origin[0],
                    origin_z: origin[1],
                    cell_size: layout.cell_size,
                }
            })
            .collect();
        let result = NativeHeightfieldRequestsResult {
            requests: requests.as_ptr(),
            requests_len: requests.len(),
        };
        self.borrowed.hold(requests);
        Ok(result)
    }

    pub(super) fn commit_tile(
        &mut self,
        request: &NativeHeightfieldColliderTileCommit,
    ) -> Result<NativeHeightfieldCommitReceipt, CsharpEngineServicesError> {
        let heights = unsafe {
            borrowed_slice(
                request.heights,
                request.heights_len,
                "heightfield collider heights",
            )?
        };
        let tiles = self.collider(request.collider)?;
        let expected = tiles.layout().height_side().pow(2);
        if heights.len() != expected {
            return Err(refused(format!(
                "a tile has {expected} heights (tile cells + 3, squared), not {}",
                heights.len()
            )));
        }
        if heights.iter().any(|height| height.is_infinite()) {
            return Err(refused("heights are finite or NaN"));
        }
        Ok(NativeHeightfieldCommitReceipt {
            accepted: tiles.commit((request.x, request.z), request.revision, heights),
        })
    }

    pub(super) fn invalidate(
        &mut self,
        request: NativeHeightfieldColliderInvalidateRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let bounds = [request.min_x, request.min_z, request.max_x, request.max_z];
        if !bounds.iter().all(|value| value.is_finite())
            || request.min_x > request.max_x
            || request.min_z > request.max_z
        {
            return Err(refused(
                "an invalidated rectangle must be finite and ordered",
            ));
        }
        self.collider(request.collider)?.invalidate(
            [request.min_x, request.min_z],
            [request.max_x, request.max_z],
        );
        Ok(())
    }

    pub(super) fn read(
        &mut self,
        request: NativeHeightfieldColliderReadRequest,
    ) -> Result<NativeHeightfieldColliderReadout, CsharpEngineServicesError> {
        if !(request.x.is_finite() && request.z.is_finite() && request.reach.is_finite())
            || request.reach < 0.0
        {
            return Err(refused(
                "a ground read needs a finite point and a finite reach of 0 or more",
            ));
        }
        let tiles = self.collider(request.collider)?;
        let counts = tiles.counts();
        Ok(NativeHeightfieldColliderReadout {
            ready: tiles.ready([request.x, request.z], request.reach),
            resident_tiles: counts.live,
            pending_tiles: counts.pending,
            refused_tiles: counts.refused,
        })
    }
}

/// Run `work` on the bridge behind `context`, reporting a refusal.
unsafe fn operate<T>(
    context: *mut std::ffi::c_void,
    output: *mut T,
    error: *mut NativeOperationErrorReceipt,
    work: impl FnOnce(&mut RuntimeSpatialBridge) -> Result<T, CsharpEngineServicesError>,
) -> i32 {
    crate::operation_diagnostics::clear_receipt(error);
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeSpatialBridge>() };
    match work(bridge) {
        Ok(value) => {
            if !output.is_null() {
                unsafe { *output = value };
            }
            ABI_OK
        }
        Err(refusal) => {
            bridge.operation_diagnostics.retain(&refusal, error);
            0
        }
    }
}

pub(super) unsafe extern "C" fn create(
    context: *mut std::ffi::c_void,
    request: NativeHeightfieldColliderCreateRequest,
    result: *mut NativeHeightfieldColliderHandle,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    unsafe {
        operate(context, result, error, |bridge| {
            bridge.create_collider(request)
        })
    }
}

pub(super) unsafe extern "C" fn destroy(
    context: *mut std::ffi::c_void,
    collider: NativeHeightfieldColliderHandle,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    unsafe {
        operate(context, std::ptr::null_mut(), error, |bridge| {
            bridge.destroy_collider(collider)
        })
    }
}

pub(super) unsafe extern "C" fn set_focuses(
    context: *mut std::ffi::c_void,
    request: *const NativeHeightfieldColliderFocusRequest,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if request.is_null() {
        return 0;
    }
    unsafe {
        operate(context, std::ptr::null_mut(), error, |bridge| {
            bridge.set_focuses(&*request)
        })
    }
}

pub(super) unsafe extern "C" fn read_requests(
    context: *mut std::ffi::c_void,
    request: NativeHeightfieldColliderRequestsRequest,
    result: *mut NativeHeightfieldRequestsResult,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    unsafe {
        operate(context, result, error, |bridge| {
            bridge.read_requests(request)
        })
    }
}

pub(super) unsafe extern "C" fn commit_tile(
    context: *mut std::ffi::c_void,
    request: *const NativeHeightfieldColliderTileCommit,
    receipt: *mut NativeHeightfieldCommitReceipt,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if request.is_null() {
        return 0;
    }
    unsafe {
        operate(context, receipt, error, |bridge| {
            bridge.commit_tile(&*request)
        })
    }
}

pub(super) unsafe extern "C" fn invalidate(
    context: *mut std::ffi::c_void,
    request: NativeHeightfieldColliderInvalidateRequest,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    unsafe {
        operate(context, std::ptr::null_mut(), error, |bridge| {
            bridge.invalidate(request)
        })
    }
}

pub(super) unsafe extern "C" fn read(
    context: *mut std::ffi::c_void,
    request: NativeHeightfieldColliderReadRequest,
    readout: *mut NativeHeightfieldColliderReadout,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    unsafe { operate(context, readout, error, |bridge| bridge.read(request)) }
}
