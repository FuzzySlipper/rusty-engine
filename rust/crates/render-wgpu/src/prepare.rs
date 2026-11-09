//! Deferred preparation (#10693). A texture or static mesh defined
//! `deferred` is decoded and uploaded on a worker rather than in the call
//! that defines it, and taken in at the start of a later
//! [`Renderer::apply`](crate::Renderer::apply): a committed call, so every
//! reading of the scene shows it from the same call on. Until then what
//! draws it waits: a node naming a mesh being prepared, or a material
//! sampling a texture being prepared, draws nothing (`rebuild_parts`).
//!
//! Each call's deferred definitions belong to the batch the Engine numbered
//! for it ([`Renderer::set_preparation_batch`]); the readout says which
//! batches are done, as the services' deferred residency does.

use std::collections::HashMap;
use std::sync::{mpsc, Arc, Mutex};

use render_model::{StaticMeshAsset, TextureDescriptor};

use crate::probes::Thumb;
use crate::tables::{GpuMesh, GpuTexture};

/// Workers at most: decoding is the cost, and the product's thread and the
/// draw keep theirs.
const MAX_WORKERS: usize = 4;

/// What is waiting to be taken in.
pub(crate) enum Definition {
    Texture(TextureDescriptor),
    Mesh(Box<StaticMeshAsset>),
}

/// What a worker made of a definition.
pub(crate) enum Prepared {
    Texture { thumb: Thumb, texture: GpuTexture },
    Mesh(GpuMesh),
}

type Work = Box<dyn FnOnce() -> Result<Prepared, String> + Send>;

struct Job {
    id: String,
    generation: u64,
    work: Work,
}

struct Finished {
    id: String,
    generation: u64,
    result: Result<Prepared, String>,
}

struct Pending {
    generation: u64,
    batch: u64,
    definition: Definition,
}

/// The definitions being prepared, and the workers preparing them.
pub(crate) struct Preparation {
    /// By texture id or mesh asset: the latest deferred definition. A later
    /// definition or release replaces it, and an earlier one's work is
    /// dropped when it finishes.
    pending: HashMap<String, Pending>,
    jobs: Option<mpsc::Sender<Job>>,
    finished_sender: mpsc::Sender<Finished>,
    finished: mpsc::Receiver<Finished>,
    generation: u64,
    /// The batch the definitions now applied belong to.
    batch: u64,
    /// Definitions prepared and taken in, and those that failed.
    installed: u64,
    failed: u64,
}

impl Default for Preparation {
    fn default() -> Self {
        let (finished_sender, finished) = mpsc::channel();
        Self {
            pending: HashMap::new(),
            jobs: None,
            finished_sender,
            finished,
            generation: 0,
            batch: 0,
            installed: 0,
            failed: 0,
        }
    }
}

/// How far deferred preparation is.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PreparationReadout {
    /// Every batch numbered up to this one is drawn: its deferred textures
    /// and meshes were prepared and taken in, or failed.
    pub completed_batch: u64,
    /// Deferred textures and meshes not yet taken in.
    pub pending: u32,
    /// Taken in, and failed (the renderer's skipped ops name why).
    pub installed: u64,
    pub failed: u64,
}

impl Preparation {
    /// Whether `id` (a texture id or mesh asset) is being prepared.
    pub(crate) fn pending(&self, id: &str) -> bool {
        self.pending.contains_key(id)
    }

    pub(crate) fn is_idle(&self) -> bool {
        self.pending.is_empty()
    }

    pub(crate) fn set_batch(&mut self, batch: u64) {
        self.batch = batch;
    }

    /// Prepare `definition` with `work` on a worker. A definition still
    /// being prepared under the same id is replaced.
    pub(crate) fn start(&mut self, id: String, definition: Definition, work: Work) {
        self.generation += 1;
        let generation = self.generation;
        self.pending.insert(
            id.clone(),
            Pending {
                generation,
                batch: self.batch,
                definition,
            },
        );
        let job = Job {
            id,
            generation,
            work,
        };
        match self.workers() {
            Some(jobs) => {
                if let Err(mpsc::SendError(job)) = jobs.send(job) {
                    run(job, &self.finished_sender);
                }
            }
            // No thread to spare: prepared here, taken in as one that was.
            None => run(job, &self.finished_sender),
        }
    }

    /// `id` is defined at once, or released: whatever is being prepared
    /// for it is dropped.
    pub(crate) fn forget(&mut self, id: &str) {
        self.pending.remove(id);
    }

    /// What workers have finished that is still wanted, with its
    /// definition; with `wait`, until nothing is pending.
    pub(crate) fn take_finished(
        &mut self,
        wait: bool,
    ) -> Vec<(Definition, Result<Prepared, String>)> {
        let mut taken = Vec::new();
        loop {
            let finished = if wait && !self.pending.is_empty() {
                match self.finished.recv() {
                    Ok(finished) => finished,
                    Err(_) => break,
                }
            } else {
                match self.finished.try_recv() {
                    Ok(finished) => finished,
                    Err(_) => break,
                }
            };
            let current = self
                .pending
                .get(&finished.id)
                .is_some_and(|pending| pending.generation == finished.generation);
            if !current {
                continue;
            }
            let pending = self
                .pending
                .remove(&finished.id)
                .expect("a current definition is pending");
            match &finished.result {
                Ok(_) => self.installed += 1,
                Err(_) => self.failed += 1,
            }
            taken.push((pending.definition, finished.result));
        }
        taken
    }

    pub(crate) fn readout(&self) -> PreparationReadout {
        let completed_batch = self
            .pending
            .values()
            .map(|pending| pending.batch.saturating_sub(1))
            .min()
            .unwrap_or(self.batch);
        PreparationReadout {
            completed_batch,
            pending: self.pending.len() as u32,
            installed: self.installed,
            failed: self.failed,
        }
    }

    /// The workers' queue, started on first use.
    fn workers(&mut self) -> Option<&mpsc::Sender<Job>> {
        if self.jobs.is_none() {
            let (jobs, queue) = mpsc::channel::<Job>();
            let queue = Arc::new(Mutex::new(queue));
            let count = std::thread::available_parallelism()
                .map_or(1, |threads| threads.get() / 2)
                .clamp(1, MAX_WORKERS);
            let mut started = 0;
            for _ in 0..count {
                let queue = Arc::clone(&queue);
                let finished = self.finished_sender.clone();
                let spawned = std::thread::Builder::new()
                    .name("rusty-render-prepare".to_owned())
                    .spawn(move || loop {
                        let job = queue
                            .lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner())
                            .recv();
                        match job {
                            Ok(job) => run(job, &finished),
                            // The renderer is gone.
                            Err(_) => return,
                        }
                    });
                started += usize::from(spawned.is_ok());
            }
            if started == 0 {
                return None;
            }
            self.jobs = Some(jobs);
        }
        self.jobs.as_ref()
    }
}

fn run(job: Job, finished: &mpsc::Sender<Finished>) {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(job.work))
        .unwrap_or_else(|_| Err("preparing it panicked".to_owned()));
    // A renderer replaced since has no use for it.
    let _ = finished.send(Finished {
        id: job.id,
        generation: job.generation,
        result,
    });
}
