//! Owned background preparation; publication remains on the session callback lane.
use crate::{
    PreparedVoxelChunkResidency, VoxelChunkLeaseRegistry, VoxelChunkResidencyApplyError,
    VoxelChunkResidencyOperation, VoxelChunkResidencyService, VoxelChunkResidencyTransaction,
    VoxelCollisionScene, VoxelSourceRevision,
};
use std::{
    sync::{
        mpsc::{self, Receiver, TryRecvError},
        Arc,
    },
    thread::{self, JoinHandle},
};

/// One in-flight transaction. The immutable scene snapshot and copied inputs
/// cross the worker boundary; no callback, native pointer or managed lease does.
/// Dropping this owner joins its worker, so session teardown cannot leave work
/// retaining a retired scene. Ordinary polling never waits for preparation.
pub struct VoxelResidencyPreparation {
    worker: Option<JoinHandle<()>>,
    result: Receiver<Result<PreparedVoxelChunkResidency, VoxelChunkResidencyApplyError>>,
}

pub enum VoxelPreparationPoll {
    Pending,
    Ready(Box<Result<PreparedVoxelChunkResidency, VoxelChunkResidencyApplyError>>),
    WorkerFailed,
}

impl VoxelResidencyPreparation {
    pub fn start(
        scene: Arc<VoxelCollisionScene>,
        leases: VoxelChunkLeaseRegistry,
        expected_revision: VoxelSourceRevision,
        operations: Vec<VoxelChunkResidencyOperation>,
    ) -> std::io::Result<Self> {
        let (sender, result) = mpsc::channel();
        let worker = thread::Builder::new()
            .name("voxel-preparation".into())
            .spawn(move || {
                let prepared = VoxelChunkResidencyService::prepare(
                    &scene,
                    &leases,
                    VoxelChunkResidencyTransaction {
                        expected_scene_source_revision: expected_revision,
                        operations: &operations,
                    },
                );
                let _ = sender.send(prepared);
            })?;
        Ok(Self {
            worker: Some(worker),
            result,
        })
    }

    pub fn poll(&mut self) -> VoxelPreparationPoll {
        match self.result.try_recv() {
            Ok(result) => VoxelPreparationPoll::Ready(Box::new(result)),
            Err(TryRecvError::Empty) => VoxelPreparationPoll::Pending,
            Err(TryRecvError::Disconnected) => VoxelPreparationPoll::WorkerFailed,
        }
    }
}
impl Drop for VoxelResidencyPreparation {
    fn drop(&mut self) {
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
