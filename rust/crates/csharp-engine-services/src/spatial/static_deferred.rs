//! Deferred static collision residency (#10328): a batch's geometry is copied
//! when the product makes it, then validated, planned and baked on a worker
//! thread, one batch at a time in the order they were made, and installed at
//! the start of the first call after it is baked.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

use csharp_engine_abi::NativeCollisionResidencyReadout;
use engine_spatial::{
    StaticMeshAssetId, StaticMeshBaked, StaticMeshColliderAsset, StaticMeshColliderInstance,
    StaticMeshCollisionProjection, StaticMeshInstanceId, StaticMeshResidencyPlan,
};

/// An asset's id, vertices and triangles, copied from a request.
pub(crate) type CopiedAsset = (u64, Vec<[f64; 3]>, Vec<[u32; 3]>);

/// One deferred batch as the product made it, its geometry copied.
pub(crate) struct StaticBatch {
    pub(crate) assets: Vec<CopiedAsset>,
    pub(crate) instances: Vec<StaticMeshColliderInstance>,
    pub(crate) removed_assets: Vec<StaticMeshAssetId>,
    pub(crate) removed_instances: Vec<StaticMeshInstanceId>,
}

/// What a worker makes of a batch: its plan and baked instances, or why it
/// was refused.
type Baked = Result<(StaticMeshResidencyPlan, StaticMeshBaked), String>;

struct Baking {
    number: u64,
    planned: Arc<AtomicUsize>,
    baked: Arc<AtomicUsize>,
    worker: Option<JoinHandle<Baked>>,
    /// Made here when no thread could be spawned.
    ready: Option<Baked>,
}

/// A session's deferred static collision batches.
#[derive(Default)]
pub(crate) struct StaticDeferred {
    waiting: VecDeque<(u64, StaticBatch)>,
    baking: Option<Baking>,
    last_batch: u64,
    completed_batch: u64,
    refused_batches: u64,
}

impl StaticDeferred {
    pub(crate) fn is_empty(&self) -> bool {
        self.baking.is_none() && self.waiting.is_empty()
    }

    /// Queue a batch; returns its number.
    pub(crate) fn push(&mut self, batch: StaticBatch) -> u64 {
        self.last_batch += 1;
        self.waiting.push_back((self.last_batch, batch));
        self.last_batch
    }

    /// Start baking the next batch, planned against `projection`, when none
    /// is baking.
    pub(crate) fn start(&mut self, projection: &StaticMeshCollisionProjection) {
        if self.baking.is_some() {
            return;
        }
        let Some((number, batch)) = self.waiting.pop_front() else {
            return;
        };
        let planned = Arc::new(AtomicUsize::new(0));
        let baked = Arc::new(AtomicUsize::new(0));
        let projection = projection.clone();
        let work = {
            let (planned, baked) = (Arc::clone(&planned), Arc::clone(&baked));
            move || bake(&projection, batch, &planned, &baked)
        };
        let (worker, ready) = match std::thread::Builder::new()
            .name("rusty-collision-residency".to_owned())
            .spawn(work)
        {
            Ok(worker) => (Some(worker), None),
            // No thread to spare: the batch is lost with it, so say so.
            Err(error) => (None, Some(Err(format!("no worker thread: {error}")))),
        };
        self.baking = Some(Baking {
            number,
            planned,
            baked,
            worker,
            ready,
        });
    }

    /// Whether the batch baking is done, so [`Self::take`] will not wait.
    pub(crate) fn is_baked(&self) -> bool {
        self.baking.as_ref().is_some_and(|baking| {
            baking.ready.is_some() || baking.worker.as_ref().is_some_and(JoinHandle::is_finished)
        })
    }

    /// The batch baking and what came of it, waiting for it if it is still
    /// baking.
    pub(crate) fn take(&mut self) -> Option<(u64, Baked)> {
        let mut baking = self.baking.take()?;
        let result = match (baking.ready.take(), baking.worker.take()) {
            (Some(result), _) => result,
            (None, Some(worker)) => worker
                .join()
                .unwrap_or_else(|_| Err("the worker panicked".to_owned())),
            (None, None) => unreachable!("a batch baking has a worker or a result"),
        };
        Some((baking.number, result))
    }

    /// A taken batch installed (`refused`: or was refused).
    pub(crate) fn finish(&mut self, number: u64, refused: bool) {
        self.completed_batch = number;
        self.refused_batches += u64::from(refused);
    }

    pub(crate) fn readout(
        &self,
        static_collision_revision: u64,
    ) -> NativeCollisionResidencyReadout {
        let (baked, planned) = self.baking.as_ref().map_or((0, 0), |baking| {
            (
                baking.baked.load(Ordering::Relaxed),
                baking.planned.load(Ordering::Relaxed),
            )
        });
        NativeCollisionResidencyReadout {
            completed_batch: self.completed_batch,
            pending_batches: (self.waiting.len() + usize::from(self.baking.is_some())) as u32,
            baked_instances: baked as u64,
            planned_instances: planned as u64,
            refused_batches: self.refused_batches,
            static_collision_revision,
        }
    }
}

/// Validate a batch's assets, plan it against `projection` and bake it.
fn bake(
    projection: &StaticMeshCollisionProjection,
    batch: StaticBatch,
    planned: &AtomicUsize,
    baked: &AtomicUsize,
) -> Baked {
    let assets = batch
        .assets
        .into_iter()
        .map(|(id, positions, triangles)| {
            StaticMeshColliderAsset::new(StaticMeshAssetId(id), positions, triangles)
                .map_err(|error| format!("asset {id}: {error:?}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let plan = projection
        .plan_residency(
            assets,
            batch.instances,
            batch.removed_assets,
            batch.removed_instances,
        )
        .map_err(|error| format!("{error:?}"))?;
    planned.store(plan.instance_count(), Ordering::Relaxed);
    let instances = plan.bake(baked).map_err(|error| format!("{error:?}"))?;
    Ok((plan, instances))
}
