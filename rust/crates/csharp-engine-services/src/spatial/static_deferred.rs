//! Deferred static collision residency (#10328) and spatial content
//! residency (#10361): a batch's inputs are copied when the product makes it,
//! then prepared on a worker thread, one batch at a time in the order they
//! were made, and installed at the start of the first call after it is
//! prepared.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

use csharp_engine_abi::NativeCollisionResidencyReadout;
use engine_spatial::{
    StaticMeshAssetId, StaticMeshBaked, StaticMeshColliderAsset, StaticMeshColliderInstance,
    StaticMeshCollisionProjection, StaticMeshInstanceId, StaticMeshResidencyPlan,
};

use super::{ComposedCells, ContentAdmission, ContentBatch};

/// An asset's id, vertices and triangles, copied from a request.
pub(crate) type CopiedAsset = (u64, Vec<[f64; 3]>, Vec<[u32; 3]>);

/// One deferred collision batch as the product made it, its geometry copied.
pub(crate) struct StaticBatch {
    pub(crate) assets: Vec<CopiedAsset>,
    pub(crate) instances: Vec<StaticMeshColliderInstance>,
    pub(crate) removed_assets: Vec<StaticMeshAssetId>,
    pub(crate) removed_instances: Vec<StaticMeshInstanceId>,
}

pub(crate) enum DeferredBatch {
    Collision(StaticBatch),
    Content(ContentBatch),
}

/// What a worker has done of its batch, for the readout.
#[derive(Default)]
pub(crate) struct Progress {
    pub(crate) planned_instances: AtomicUsize,
    pub(crate) baked_instances: AtomicUsize,
    pub(crate) planned_cells: AtomicUsize,
    pub(crate) composed_cells: AtomicUsize,
}

/// What a worker makes of a batch.
pub(crate) enum Prepared {
    Collision(StaticMeshResidencyPlan, StaticMeshBaked),
    Content(Box<ContentAdmission>),
}

/// Why a batch was refused, and the composed cells a content batch took
/// with it: `None` when they were lost with its worker.
pub(crate) struct Refused {
    pub(crate) reason: String,
    pub(crate) cells: Option<ComposedCells>,
}

impl Refused {
    pub(crate) fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
            cells: None,
        }
    }
}

pub(crate) type Baked = Result<Prepared, Refused>;

/// A batch taken from its worker: its number, whether it took the
/// session's composed cells and what came of it.
pub(crate) struct Taken {
    pub(crate) number: u64,
    pub(crate) content: bool,
    pub(crate) baked: Baked,
}

struct Baking {
    number: u64,
    content: bool,
    /// The ids a content batch places, which a later deferred collision
    /// batch may not remove or replace.
    placing: Vec<u64>,
    progress: Arc<Progress>,
    worker: Option<JoinHandle<Baked>>,
    /// Made here when no thread could be spawned.
    ready: Option<Baked>,
}

/// A session's deferred batches.
#[derive(Default)]
pub(crate) struct StaticDeferred {
    waiting: VecDeque<(u64, DeferredBatch)>,
    baking: Option<Baking>,
    last_batch: u64,
    completed_batch: u64,
    refused_batches: u64,
    last_refused_batch: u64,
}

impl StaticDeferred {
    pub(crate) fn is_empty(&self) -> bool {
        self.baking.is_none() && self.waiting.is_empty()
    }

    /// Queue a batch; returns its number.
    pub(crate) fn push(&mut self, batch: DeferredBatch) -> u64 {
        self.last_batch += 1;
        self.waiting.push_back((self.last_batch, batch));
        self.last_batch
    }

    /// The ids the content batches not yet installed place.
    pub(crate) fn placing(&self) -> impl Iterator<Item = u64> + '_ {
        let waiting = self.waiting.iter().flat_map(|(_, batch)| match batch {
            DeferredBatch::Content(batch) => batch.placing(),
            DeferredBatch::Collision(_) => Vec::new(),
        });
        self.baking
            .iter()
            .flat_map(|baking| baking.placing.iter().copied())
            .chain(waiting)
    }

    /// The next batch to prepare, when none is being prepared.
    pub(crate) fn next(&mut self) -> Option<(u64, DeferredBatch)> {
        if self.baking.is_some() {
            return None;
        }
        self.waiting.pop_front()
    }

    /// Prepare batch `number` with `work` on a worker thread. A `content`
    /// batch has taken the session's composed cells.
    pub(crate) fn spawn(
        &mut self,
        number: u64,
        content: bool,
        placing: Vec<u64>,
        work: impl FnOnce(&Progress) -> Baked + Send + 'static,
    ) {
        let progress = Arc::new(Progress::default());
        let work = {
            let progress = Arc::clone(&progress);
            move || work(&progress)
        };
        let (worker, ready) = match std::thread::Builder::new()
            .name("rusty-collision-residency".to_owned())
            .spawn(work)
        {
            Ok(worker) => (Some(worker), None),
            // No thread to spare: the batch is lost with it, so say so.
            Err(error) => (
                None,
                Some(Err(Refused::new(format!("no worker thread: {error}")))),
            ),
        };
        self.baking = Some(Baking {
            number,
            content,
            placing,
            progress,
            worker,
            ready,
        });
    }

    /// Whether the batch being prepared is done, so [`Self::take`] will not
    /// wait.
    pub(crate) fn is_baked(&self) -> bool {
        self.baking.as_ref().is_some_and(|baking| {
            baking.ready.is_some() || baking.worker.as_ref().is_some_and(JoinHandle::is_finished)
        })
    }

    /// The batch being prepared and what came of it, waiting for it if it
    /// is still being prepared.
    pub(crate) fn take(&mut self) -> Option<Taken> {
        let mut baking = self.baking.take()?;
        let baked = match (baking.ready.take(), baking.worker.take()) {
            (Some(result), _) => result,
            (None, Some(worker)) => worker
                .join()
                .unwrap_or_else(|_| Err(Refused::new("the worker panicked"))),
            (None, None) => unreachable!("a batch baking has a worker or a result"),
        };
        Some(Taken {
            number: baking.number,
            content: baking.content,
            baked,
        })
    }

    /// A taken batch installed (`refused`: or was refused).
    pub(crate) fn finish(&mut self, number: u64, refused: bool) {
        self.completed_batch = number;
        if refused {
            self.refused_batches += 1;
            self.last_refused_batch = number;
        }
    }

    pub(crate) fn readout(
        &self,
        static_collision_revision: u64,
    ) -> NativeCollisionResidencyReadout {
        let load = |counter: fn(&Progress) -> &AtomicUsize| {
            self.baking.as_ref().map_or(0, |baking| {
                counter(&baking.progress).load(Ordering::Relaxed) as u64
            })
        };
        NativeCollisionResidencyReadout {
            completed_batch: self.completed_batch,
            pending_batches: (self.waiting.len() + usize::from(self.baking.is_some())) as u32,
            baked_instances: load(|progress| &progress.baked_instances),
            planned_instances: load(|progress| &progress.planned_instances),
            composed_cells: load(|progress| &progress.composed_cells),
            planned_cells: load(|progress| &progress.planned_cells),
            refused_batches: self.refused_batches,
            last_refused_batch: self.last_refused_batch,
            static_collision_revision,
        }
    }
}

/// Validate a collision batch's assets, plan it against `projection` and
/// bake it.
pub(crate) fn bake(
    projection: &StaticMeshCollisionProjection,
    batch: StaticBatch,
    progress: &Progress,
) -> Baked {
    let assets = batch
        .assets
        .into_iter()
        .map(|(id, positions, triangles)| {
            StaticMeshColliderAsset::new(StaticMeshAssetId(id), positions, triangles)
                .map_err(|error| Refused::new(format!("asset {id}: {error:?}")))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let plan = projection
        .plan_residency(
            assets,
            batch.instances,
            batch.removed_assets,
            batch.removed_instances,
        )
        .map_err(|error| Refused::new(format!("{error:?}")))?;
    progress
        .planned_instances
        .store(plan.instance_count(), Ordering::Relaxed);
    let instances = plan
        .bake(&progress.baked_instances)
        .map_err(|error| Refused::new(format!("{error:?}")))?;
    Ok(Prepared::Collision(plan, instances))
}
