//! Keeps a spatial session's air graph ([`AirGraph`]) on a worker thread:
//! chunks are built round the points its readers focus on, rebuilt where
//! the session's collision changes, and dropped as the focus moves away.
//! Readers take the latest graph published; nothing waits for a build.

use std::collections::BTreeSet;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, Weak};
use std::time::{Duration, Instant};

use engine_spatial::{
    air_chunk_distance, air_chunks_within, AirChunk, AirChunkKey, AirGraph, AirSceneChange,
    AirSceneWatch, VoxelCollisionScene,
};

/// Chunks within this many metres of a focus point are built...
pub const AIR_BUILD_METRES: f64 = 64.0;
/// ...and kept until every focus point is this far away.
pub const AIR_DROP_METRES: f64 = 80.0;
/// The most chunks with both air and solid one graph keeps; the furthest
/// from every focus point go first.
pub const AIR_MIXED_CHUNKS: usize = 512;
/// The most focus points kept; the one focused on least recently goes.
const FOCUS_POINTS: usize = 8;
/// A focus within this many metres of a kept one moves it.
const FOCUS_MERGE_METRES: f64 = 16.0;
/// A kept focus point moving less than this does not wake the worker.
const FOCUS_SETTLE_METRES: f64 = 4.0;

/// What the keeper's worker has done, for measurement.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct AirKeeperStats {
    /// Chunks built where none was.
    pub builds: u64,
    /// Chunks built again after a collision change.
    pub rebuilds: u64,
    pub build_seconds_total: f64,
    pub build_seconds_max: f64,
    pub rebuild_seconds_total: f64,
    /// From the latest collision change reaching built chunks to the graph
    /// with all of them rebuilt.
    pub last_change_seconds: Option<f64>,
    pub dropped: u64,
}

/// Keeps one session's graph: owned by the session's spatial service,
/// told of each scene it publishes.
pub struct AirKeeper {
    shared: Arc<Shared>,
    watch: AirSceneWatch,
    thread: Option<std::thread::JoinHandle<()>>,
}

/// A reader of one session's graph.
#[derive(Clone)]
pub struct AirReader {
    shared: Arc<Shared>,
}

struct Shared {
    inbox: Mutex<Inbox>,
    /// The worker waits on it for work; readers wait on it for idleness.
    wake: Condvar,
    latest: Mutex<Arc<AirGraph>>,
}

#[derive(Default)]
struct Inbox {
    scene: Weak<VoxelCollisionScene>,
    /// A scene arrived since the worker last looked.
    fresh: bool,
    changes: Vec<(AirSceneChange, Instant)>,
    focus: Vec<Focus>,
    focus_moved: bool,
    /// Focus calls so far, to tell the least recent.
    focus_count: u64,
    /// The worker has nothing left to build.
    idle: bool,
    stop: bool,
    stats: AirKeeperStats,
}

#[derive(Clone, Copy)]
struct Focus {
    point: [f64; 3],
    used: u64,
}

impl AirKeeper {
    /// A keeper for a session whose scene is `scene`. Nothing is built until
    /// a reader focuses somewhere; without a worker thread, nothing is.
    pub fn new(scene: &Arc<VoxelCollisionScene>) -> Self {
        let shared = Arc::new(Shared {
            inbox: Mutex::new(Inbox::default()),
            wake: Condvar::new(),
            latest: Mutex::new(Arc::new(AirGraph::default())),
        });
        let worker = Arc::clone(&shared);
        let thread = std::thread::Builder::new()
            .name("rusty-air-graph".to_owned())
            .spawn(move || {
                crate::worker_priority::below_normal();
                work(&worker);
            })
            .ok();
        let mut keeper = Self {
            shared,
            watch: AirSceneWatch::default(),
            thread,
        };
        keeper.scene_changed(scene);
        keeper
    }

    /// The session published `scene`.
    pub fn scene_changed(&mut self, scene: &Arc<VoxelCollisionScene>) {
        let change = self.watch.observe(scene);
        let mut inbox = lock(&self.shared.inbox);
        inbox.scene = Arc::downgrade(scene);
        inbox.fresh = true;
        if change != AirSceneChange::None {
            inbox.changes.push((change, Instant::now()));
            inbox.idle = false;
        }
        drop(inbox);
        self.shared.wake.notify_all();
    }

    pub fn reader(&self) -> AirReader {
        AirReader {
            shared: Arc::clone(&self.shared),
        }
    }
}

impl Drop for AirKeeper {
    /// Stops the worker without waiting: a chunk it is building finishes
    /// and is thrown away.
    fn drop(&mut self) {
        lock(&self.shared.inbox).stop = true;
        self.shared.wake.notify_all();
        drop(self.thread.take());
    }
}

impl AirReader {
    /// The latest graph built.
    pub fn latest(&self) -> Arc<AirGraph> {
        Arc::clone(&lock(&self.shared.latest))
    }

    /// Asks for the graph to be built round `point`.
    pub fn focus(&self, point: [f64; 3]) {
        let mut inbox = lock(&self.shared.inbox);
        inbox.focus_count += 1;
        let used = inbox.focus_count;
        let near = inbox
            .focus
            .iter_mut()
            .find(|focus| distance(focus.point, point) <= FOCUS_MERGE_METRES);
        let moved = match near {
            Some(focus) => {
                focus.used = used;
                if distance(focus.point, point) > FOCUS_SETTLE_METRES {
                    focus.point = point;
                    true
                } else {
                    false
                }
            }
            None => {
                if inbox.focus.len() >= FOCUS_POINTS {
                    let oldest = inbox
                        .focus
                        .iter()
                        .enumerate()
                        .min_by_key(|(_, focus)| focus.used)
                        .map(|(index, _)| index)
                        .expect("full");
                    inbox.focus.remove(oldest);
                }
                inbox.focus.push(Focus { point, used });
                true
            }
        };
        if moved {
            inbox.focus_moved = true;
            inbox.idle = false;
            drop(inbox);
            self.shared.wake.notify_all();
        }
    }

    /// Waits until the worker has nothing left to build, at most `timeout`;
    /// whether it got there.
    pub fn wait_idle(&self, timeout: Duration) -> bool {
        let inbox = lock(&self.shared.inbox);
        let (inbox, _) = self
            .shared
            .wake
            .wait_timeout_while(inbox, timeout, |inbox| !inbox.idle && !inbox.stop)
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        inbox.idle
    }

    pub fn stats(&self) -> AirKeeperStats {
        lock(&self.shared.inbox).stats
    }
}

/// The worker: builds until stopped, sleeping while nothing is wanted.
fn work(shared: &Shared) {
    let mut state = Work::default();
    loop {
        if !state.step(shared) {
            let mut inbox = lock(&shared.inbox);
            if inbox.stop {
                return;
            }
            if inbox.changes.is_empty() && !inbox.focus_moved && !inbox.fresh {
                inbox.idle = true;
                shared.wake.notify_all();
                inbox = shared
                    .wake
                    .wait_while(inbox, |inbox| {
                        !inbox.stop
                            && inbox.changes.is_empty()
                            && !inbox.focus_moved
                            && !inbox.fresh
                    })
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
            }
            if inbox.stop {
                return;
            }
        }
    }
}

#[derive(Default)]
struct Work {
    graph: Arc<AirGraph>,
    /// Built chunks a collision change reached, with when it came.
    dirty: Vec<(AirChunkKey, Instant)>,
}

impl Work {
    /// Takes what the inbox holds and builds one chunk; whether there was
    /// one to build.
    fn step(&mut self, shared: &Shared) -> bool {
        let (scene, changes, focus) = {
            let mut inbox = lock(&shared.inbox);
            if inbox.stop {
                return false;
            }
            inbox.fresh = false;
            inbox.focus_moved = false;
            (
                inbox.scene.clone(),
                std::mem::take(&mut inbox.changes),
                inbox
                    .focus
                    .iter()
                    .map(|focus| focus.point)
                    .collect::<Vec<_>>(),
            )
        };
        for (change, at) in changes {
            match change {
                AirSceneChange::None => {}
                AirSceneChange::Everything { moved: true } => {
                    let keys: Vec<_> = self.graph.chunk_keys().collect();
                    if !keys.is_empty() {
                        self.publish(shared, self.graph.without(&keys));
                    }
                    self.dirty.clear();
                }
                change => {
                    for key in self.graph.chunk_keys() {
                        if change.reaches(key) && !self.dirty.iter().any(|held| held.0 == key) {
                            self.dirty.push((key, at));
                        }
                    }
                }
            }
        }
        let away = |key: AirChunkKey| {
            focus
                .iter()
                .map(|point| air_chunk_distance(key, *point))
                .fold(f64::INFINITY, f64::min)
        };

        // Drop what every focus point has left behind.
        let gone: Vec<AirChunkKey> = self
            .graph
            .chunk_keys()
            .filter(|key| away(*key) > AIR_DROP_METRES)
            .collect();
        if !gone.is_empty() {
            self.dirty.retain(|(key, _)| !gone.contains(key));
            lock(&shared.inbox).stats.dropped += gone.len() as u64;
            self.publish(shared, self.graph.without(&gone));
        }

        // A dirty chunk first, nearest first; then the nearest unbuilt.
        self.dirty
            .sort_by(|left, right| away(left.0).total_cmp(&away(right.0)));
        let rebuilding = !self.dirty.is_empty();
        let next = if rebuilding {
            Some(self.dirty[0].0)
        } else {
            let mixed = self.graph.mixed_chunk_count();
            let furthest_mixed = (mixed >= AIR_MIXED_CHUNKS).then(|| {
                self.graph
                    .chunk_keys()
                    .filter(|key| {
                        self.graph
                            .chunk(*key)
                            .is_some_and(|chunk| !chunk.is_open() && !chunk.is_solid())
                    })
                    .map(away)
                    .fold(0.0, f64::max)
            });
            let wanted: BTreeSet<AirChunkKey> = focus
                .iter()
                .flat_map(|point| air_chunks_within(*point, AIR_BUILD_METRES))
                .filter(|key| self.graph.chunk(*key).is_none())
                .collect();
            wanted
                .into_iter()
                .map(|key| (away(key), key))
                .filter(|(reach, _)| furthest_mixed.is_none_or(|furthest| *reach < furthest))
                .min_by(|left, right| left.0.total_cmp(&right.0).then(left.1.cmp(&right.1)))
                .map(|(_, key)| key)
        };
        let Some(key) = next else {
            return false;
        };
        let Some(scene) = scene.upgrade() else {
            // The scene is changing; its next publication brings it back.
            return false;
        };
        let started = Instant::now();
        let chunk = AirChunk::build(&scene, key);
        drop(scene);
        let seconds = started.elapsed().as_secs_f64();
        let mut next = self.graph.with_chunk(key, Arc::new(chunk));
        // Keep the nearest mixed chunks within the cap.
        let mixed = next.mixed_chunk_count();
        if mixed > AIR_MIXED_CHUNKS {
            let mut furthest: Vec<(f64, AirChunkKey)> = next
                .chunk_keys()
                .filter(|key| {
                    next.chunk(*key)
                        .is_some_and(|chunk| !chunk.is_open() && !chunk.is_solid())
                })
                .map(|key| (away(key), key))
                .collect();
            furthest.sort_by(|left, right| right.0.total_cmp(&left.0));
            let gone: Vec<AirChunkKey> = furthest
                .into_iter()
                .take(mixed - AIR_MIXED_CHUNKS)
                .map(|(_, key)| key)
                .collect();
            lock(&shared.inbox).stats.dropped += gone.len() as u64;
            next = next.without(&gone);
        }
        self.publish(shared, next);
        let mut inbox = lock(&shared.inbox);
        if rebuilding {
            let (_, changed_at) = self.dirty.remove(0);
            inbox.stats.rebuilds += 1;
            inbox.stats.rebuild_seconds_total += seconds;
            if self.dirty.is_empty() {
                inbox.stats.last_change_seconds = Some(changed_at.elapsed().as_secs_f64());
            }
        } else {
            inbox.stats.builds += 1;
            inbox.stats.build_seconds_total += seconds;
        }
        inbox.stats.build_seconds_max = inbox.stats.build_seconds_max.max(seconds);
        true
    }

    fn publish(&mut self, shared: &Shared, graph: AirGraph) {
        let graph = Arc::new(graph);
        self.graph = Arc::clone(&graph);
        *lock(&shared.latest) = graph;
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn distance(left: [f64; 3], right: [f64; 3]) -> f64 {
    let difference = [left[0] - right[0], left[1] - right[1], left[2] - right[2]];
    (difference[0] * difference[0] + difference[1] * difference[1] + difference[2] * difference[2])
        .sqrt()
}
