//! The water field worker: one thread that owns every field's grid.
//!
//! Each field's pending work is an ordered queue of segments, each a set of
//! changes (sources, edges, bed edits, policy, drawing) followed by the
//! admitted time that runs under them. The bridge appends to the queue as a
//! call makes changes and as it finishes; the worker takes a field's whole
//! queue and runs it in order, so a change always takes effect at the admitted
//! boundary it was made at, however late the worker gets to it.
//!
//! The queue is bounded two ways. Time whose work would pass the field's
//! backlog (owed seconds at the measured cost of a simulated second) is
//! dropped from the oldest segments and reported, their changes carried
//! forward. And past [`MAX_PENDING_SEGMENTS`] the oldest two segments merge
//! into one span of time, the later one's changes deferred to the end of it
//! and counted as coalesced, so pending memory stays bounded even before a
//! first advance has measured anything. The worker in turn stops spending
//! time on a field's batch once it has spent the backlog in wall time, and
//! drops the rest of that batch's time.

use std::{
    collections::{BTreeMap, VecDeque},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Condvar, Mutex,
    },
    thread::JoinHandle,
    time::Instant,
};

use render_projection::{water_surface_mesh, WaterSurfaceCells, WaterSurfaceMesh};
use svc_water::{Boundary, Side, WaterGrid};

use super::{refused, Policy, TIME_EPSILON};
use crate::CsharpEngineServicesError;

/// Segments a field may have waiting before the oldest merge.
pub(super) const MAX_PENDING_SEGMENTS: usize = 16;

/// Volumes since creation, m³.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(super) struct Totals {
    pub(super) inflow: f64,
    pub(super) outflow: f64,
    pub(super) source: f64,
    pub(super) sink: f64,
}

/// A complete field, published at an admitted step boundary and never
/// changed after.
#[derive(Debug)]
pub(super) struct Snapshot {
    pub(super) incarnation: u64,
    pub(super) sequence: u64,
    pub(super) time: f64,
    pub(super) bed: Arc<[f32]>,
    pub(super) depth: Vec<f32>,
    pub(super) velocity: Vec<[f32; 2]>,
    pub(super) volume: f64,
    pub(super) totals: Totals,
    pub(super) wet_cells: u32,
    /// Admitted seconds the worker dropped past its wall-time budget.
    pub(super) dropped_seconds: f64,
    pub(super) steps: u32,
    pub(super) step_microseconds: u64,
    pub(super) load: f32,
    /// Built by the worker while the field is drawn, at that depth.
    pub(super) surface: Option<(f32, Option<WaterSurfaceMesh>)>,
}

/// Changes made between two admitted intervals, in effect from the next.
/// Each kind keeps only its latest value per cell, edge cell or field, so
/// merging two never grows past one field's worth.
#[derive(Debug, Default)]
pub(super) struct Changes {
    pub(super) sources: Option<Vec<f32>>,
    pub(super) boundaries: BTreeMap<(usize, usize), Boundary>,
    pub(super) bed: BTreeMap<usize, f32>,
    pub(super) incarnation: Option<u64>,
    pub(super) policy: Option<Policy>,
    pub(super) draw: Option<Option<f32>>,
}

impl Changes {
    /// These changes followed by `later` ones.
    fn absorb(&mut self, later: Changes) {
        if later.sources.is_some() {
            self.sources = later.sources;
        }
        self.boundaries.extend(later.boundaries);
        self.bed.extend(later.bed);
        self.incarnation = later.incarnation.or(self.incarnation);
        self.policy = later.policy.or(self.policy);
        self.draw = later.draw.or(self.draw);
    }

    fn is_empty(&self) -> bool {
        self.sources.is_none()
            && self.boundaries.is_empty()
            && self.bed.is_empty()
            && self.incarnation.is_none()
            && self.policy.is_none()
            && self.draw.is_none()
    }
}

/// Changes, then the admitted time that runs under them.
#[derive(Debug, Default)]
pub(super) struct Segment {
    pub(super) changes: Changes,
    pub(super) seconds: f64,
}

/// A field's work waiting for the worker.
#[derive(Debug, Default)]
pub(super) struct Pending {
    /// A new field's grid, until the worker takes it.
    pub(super) grid: Option<Box<WaterGrid>>,
    pub(super) segments: VecDeque<Segment>,
    /// Admitted seconds dropped here for the backlog, and changes deferred
    /// by merging segments.
    pub(super) dropped: f64,
    pub(super) coalesced: u64,
}

impl Pending {
    /// Changes made now, in effect from the next admitted time.
    pub(super) fn change(&mut self, apply: impl FnOnce(&mut Changes)) {
        match self.segments.back_mut() {
            Some(last) if last.seconds == 0.0 => apply(&mut last.changes),
            _ => {
                let mut segment = Segment::default();
                apply(&mut segment.changes);
                self.segments.push_back(segment);
            }
        }
    }

    /// Admitted time, run under every change made before it. Then the queue
    /// is held to `max_backlog` worker seconds at `load` worker seconds per
    /// simulated second, and to [`MAX_PENDING_SEGMENTS`].
    pub(super) fn admit(&mut self, seconds: f64, load: f64, max_backlog: f64) {
        match self.segments.back_mut() {
            Some(last) => last.seconds += seconds,
            None => self.segments.push_back(Segment {
                changes: Changes::default(),
                seconds,
            }),
        }
        if load > 0.0 {
            let mut over = self.seconds() - max_backlog / load;
            while over > TIME_EPSILON {
                let alone = self.segments.len() == 1;
                let first = self.segments.front_mut().expect("time is pending");
                if first.seconds > over || alone {
                    let cut = over.min(first.seconds);
                    first.seconds -= cut;
                    self.dropped += cut;
                    break;
                }
                let first = self.segments.pop_front().expect("time is pending");
                over -= first.seconds;
                self.dropped += first.seconds;
                let next = self.segments.front_mut().expect("more than one segment");
                let mut changes = first.changes;
                changes.absorb(std::mem::take(&mut next.changes));
                next.changes = changes;
            }
        }
        while self.segments.len() > MAX_PENDING_SEGMENTS {
            let first = self.segments.pop_front().expect("segments are pending");
            let second = self.segments.pop_front().expect("segments are pending");
            let deferred = second.changes;
            match self.segments.front_mut() {
                Some(next) => {
                    let mut changes = deferred;
                    changes.absorb(std::mem::take(&mut next.changes));
                    next.changes = changes;
                }
                None => self.segments.push_front(Segment {
                    changes: deferred,
                    seconds: 0.0,
                }),
            }
            self.segments.push_front(Segment {
                changes: first.changes,
                seconds: first.seconds + second.seconds,
            });
            self.coalesced += 1;
        }
    }

    pub(super) fn seconds(&self) -> f64 {
        self.segments.iter().map(|segment| segment.seconds).sum()
    }
}

/// What the bridge and the worker share for one field.
#[derive(Debug)]
pub(super) struct FieldShared {
    pub(super) pending: Mutex<Pending>,
    pub(super) published: Mutex<Arc<Snapshot>>,
    /// Worker seconds per simulated second, as the last advance measured
    /// (f64 bits; 0 until measured).
    load: AtomicU64,
}

impl FieldShared {
    pub(super) fn new(grid: Box<WaterGrid>, first: Arc<Snapshot>) -> Self {
        Self {
            pending: Mutex::new(Pending {
                grid: Some(grid),
                ..Pending::default()
            }),
            published: Mutex::new(first),
            load: AtomicU64::new(0f64.to_bits()),
        }
    }

    pub(super) fn load(&self) -> f64 {
        f64::from_bits(self.load.load(Ordering::Relaxed))
    }
}

/// A field as its worker holds it.
pub(super) struct WorkerField {
    grid: Box<WaterGrid>,
    policy: Policy,
    bed: Arc<[f32]>,
    incarnation: u64,
    load: f64,
    dropped: f64,
    totals: Totals,
    sequence: u64,
    drawn_depth: Option<f32>,
}

impl WorkerField {
    pub(super) fn new(grid: Box<WaterGrid>, policy: Policy) -> Self {
        let bed = grid.bed().into();
        Self {
            grid,
            policy,
            bed,
            incarnation: 0,
            load: 0.0,
            dropped: 0.0,
            totals: Totals::default(),
            sequence: 0,
            drawn_depth: None,
        }
    }

    fn apply(&mut self, changes: Changes) {
        if let Some(rates) = changes.sources {
            self.grid.sources_mut().copy_from_slice(&rates);
        }
        for ((side, cell), boundary) in changes.boundaries {
            self.grid
                .set_boundary(Side::ALL[side], cell..cell + 1, boundary);
        }
        if !changes.bed.is_empty() {
            self.grid.edit_bed(changes.bed);
            self.bed = self.grid.bed().into();
        }
        if let Some(incarnation) = changes.incarnation {
            self.incarnation = incarnation;
        }
        if let Some(policy) = changes.policy {
            self.grid.set_params(policy.flow());
            self.policy = policy;
        }
        if let Some(draw) = changes.draw {
            self.drawn_depth = draw;
        }
    }

    /// Runs `segments` in order: each one's changes, then its time, until
    /// the batch has spent the field's backlog in wall time; time past that
    /// is dropped, changes still apply. Returns the snapshot to publish.
    pub(super) fn run(&mut self, segments: VecDeque<Segment>) -> Snapshot {
        let started = Instant::now();
        let mut advanced = 0.0;
        let mut steps = 0;
        for segment in segments {
            self.apply(segment.changes);
            let mut owed = segment.seconds;
            while owed > TIME_EPSILON {
                if started.elapsed().as_secs_f64() > self.policy.max_backlog {
                    self.dropped += owed;
                    break;
                }
                let dt = self
                    .grid
                    .stable_step(self.policy.courant)
                    .min(self.policy.max_step)
                    .min(owed as f32);
                let report = self.grid.step(dt, self.policy.threads);
                self.totals.inflow += report.boundary_inflow.iter().sum::<f64>();
                self.totals.outflow += report.boundary_outflow.iter().sum::<f64>();
                self.totals.source += report.source_volume;
                self.totals.sink += report.sink_volume;
                owed -= f64::from(dt);
                advanced += f64::from(dt);
                steps += 1;
            }
        }
        self.snapshot(steps, started, advanced)
    }

    /// The field as it stands; the advance's wall time and load include
    /// building the snapshot (and the drawn surface's mesh).
    fn snapshot(&mut self, steps: u32, started: Instant, advanced: f64) -> Snapshot {
        self.sequence += 1;
        let grid = &self.grid;
        let surface = self.drawn_depth.map(|drawn_depth| {
            (
                drawn_depth,
                water_surface_mesh(WaterSurfaceCells {
                    width: grid.width(),
                    length: grid.length(),
                    cell_size: grid.cell_size(),
                    bed: grid.bed(),
                    depth: grid.depth(),
                    drawn_depth,
                }),
            )
        });
        let depth = grid.depth().to_vec();
        let velocity = grid.velocity().to_vec();
        let elapsed = started.elapsed();
        if advanced > 0.0 {
            self.load = elapsed.as_secs_f64() / advanced;
        }
        Snapshot {
            incarnation: self.incarnation,
            sequence: self.sequence,
            time: grid.time(),
            bed: self.bed.clone(),
            depth,
            velocity,
            volume: grid.total_volume(),
            totals: self.totals,
            wet_cells: (0..grid.cell_count()).filter(|&i| grid.is_wet(i)).count() as u32,
            dropped_seconds: self.dropped,
            steps,
            step_microseconds: elapsed.as_micros() as u64,
            load: self.load as f32,
            surface,
        }
    }
}

/// Fields and the signal the worker waits on.
#[derive(Default)]
struct Registry {
    fields: Mutex<BTreeMap<u64, (Arc<FieldShared>, Policy)>>,
    wake: Mutex<bool>,
    signal: Condvar,
    stop: AtomicBool,
}

/// The worker thread; dropping it stops and joins the thread.
pub(super) struct Worker {
    registry: Arc<Registry>,
    thread: Option<JoinHandle<()>>,
}

impl Worker {
    pub(super) fn spawn() -> Result<Self, CsharpEngineServicesError> {
        let registry = Arc::new(Registry::default());
        let shared = registry.clone();
        let thread = std::thread::Builder::new()
            .name("rusty-water-field".to_owned())
            .spawn(move || run(&shared))
            .map_err(|error| refused(format!("the water field worker did not start: {error}")))?;
        Ok(Self {
            registry,
            thread: Some(thread),
        })
    }

    pub(super) fn register(&self, field: u64, shared: Arc<FieldShared>, policy: Policy) {
        self.registry
            .fields
            .lock()
            .expect("water field registry")
            .insert(field, (shared, policy));
        self.wake();
    }

    pub(super) fn retire(&self, field: u64) {
        self.registry
            .fields
            .lock()
            .expect("water field registry")
            .remove(&field);
        self.wake();
    }

    pub(super) fn wake(&self) {
        *self.registry.wake.lock().expect("water field wake") = true;
        self.registry.signal.notify_one();
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.registry.stop.store(true, Ordering::Relaxed);
        self.wake();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// The worker loop: wait to be woken, then run every field's pending
/// segments in order and publish what changed.
fn run(registry: &Registry) {
    let mut fields: BTreeMap<u64, WorkerField> = BTreeMap::new();
    loop {
        {
            let mut wake = registry.wake.lock().expect("water field wake");
            while !*wake && !registry.stop.load(Ordering::Relaxed) {
                wake = registry.signal.wait(wake).expect("water field wake");
            }
            *wake = false;
        }
        if registry.stop.load(Ordering::Relaxed) {
            return;
        }
        let live: Vec<(u64, Arc<FieldShared>, Policy)> = registry
            .fields
            .lock()
            .expect("water field registry")
            .iter()
            .map(|(id, (shared, policy))| (*id, shared.clone(), *policy))
            .collect();
        fields.retain(|id, _| live.iter().any(|(live, _, _)| live == id));
        for (id, shared, policy) in live {
            let (grid, segments) = {
                let mut pending = shared.pending.lock().expect("water field pending");
                (pending.grid.take(), std::mem::take(&mut pending.segments))
            };
            if let Some(grid) = grid {
                fields.insert(id, WorkerField::new(grid, policy));
            }
            let Some(field) = fields.get_mut(&id) else {
                continue;
            };
            if segments.is_empty()
                || segments
                    .iter()
                    .all(|s| s.seconds == 0.0 && s.changes.is_empty())
            {
                continue;
            }
            let snapshot = field.run(segments);
            shared.load.store(field.load.to_bits(), Ordering::Relaxed);
            *shared.published.lock().expect("water field slot") = Arc::new(snapshot);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::water_field::Policy;
    use csharp_engine_abi::NativeWaterFieldPolicy;

    fn policy() -> Policy {
        Policy::of(NativeWaterFieldPolicy::default()).unwrap()
    }

    fn closed_cell() -> WorkerField {
        WorkerField::new(Box::new(WaterGrid::new(1, 1, 1.0, vec![0.0])), policy())
    }

    fn rain(rate: f32) -> Changes {
        Changes {
            sources: Some(vec![rate]),
            ..Changes::default()
        }
    }

    /// R9884-2: rain for one second, then none for one: the second's rain
    /// stops at its boundary whether the worker ran between or not.
    #[test]
    fn forcing_takes_effect_at_its_admitted_boundary_however_late_the_worker_runs() {
        let mut queued = Pending::default();
        queued.change(|changes| changes.absorb(rain(1.0)));
        queued.admit(1.0, 0.0, 1.0);
        queued.change(|changes| changes.absorb(rain(0.0)));
        queued.admit(1.0, 0.0, 1.0);
        let mut worker = closed_cell();
        let at_once = worker.run(std::mem::take(&mut queued.segments));

        let mut separated = closed_cell();
        for rate in [1.0, 0.0] {
            let mut pending = Pending::default();
            pending.change(|changes| changes.absorb(rain(rate)));
            pending.admit(1.0, 0.0, 1.0);
            separated.run(std::mem::take(&mut pending.segments));
        }
        let one_by_one = separated.snapshot(0, Instant::now(), 0.0);
        assert!((at_once.volume - 1.0).abs() < 1.0e-6, "{}", at_once.volume);
        assert_eq!(at_once.depth, one_by_one.depth);
        assert!((at_once.time - 2.0).abs() < 1.0e-6);
    }

    /// R9884-3: a worker away while every update changes the forcing: the
    /// queue stays within its segments and its backlog, reports what it
    /// dropped and deferred, and keeps the latest forcing.
    #[test]
    fn pending_work_stays_bounded_while_forcing_changes_every_update() {
        const CELLS: usize = 64 * 64;
        const UPDATES: usize = 10_000;
        const STEP: f64 = 1.0 / 60.0;
        for load in [0.0, 0.5] {
            let mut pending = Pending::default();
            for update in 0..UPDATES {
                pending.change(|changes| {
                    changes.sources = Some(vec![update as f32; CELLS]);
                });
                pending.admit(STEP, load, 1.0);
                assert!(pending.segments.len() <= MAX_PENDING_SEGMENTS);
            }
            let last = pending
                .segments
                .iter()
                .rev()
                .find_map(|s| s.changes.sources.as_ref());
            assert_eq!(last.map(|rates| rates[0]), Some((UPDATES - 1) as f32));
            if load > 0.0 {
                assert!(
                    pending.seconds() * load <= 1.0 + 1.0e-9,
                    "{}",
                    pending.seconds()
                );
                assert!(pending.dropped > 0.0);
                let kept = pending.seconds() + pending.dropped;
                assert!((kept - UPDATES as f64 * STEP).abs() < 1.0e-6);
            } else {
                // Unmeasured, nothing drops: time is kept and forcing deferred.
                assert!((pending.seconds() - UPDATES as f64 * STEP).abs() < 1.0e-6);
                assert!(pending.coalesced > 0);
            }
        }
    }

    /// A batch spends at most the backlog in wall time; the rest of its time
    /// is dropped and reported, its changes still applied.
    #[test]
    fn a_batch_stops_at_its_wall_time_budget() {
        let mut field = WorkerField::new(
            Box::new(WaterGrid::new(64, 64, 1.0, vec![0.0; 64 * 64])),
            Policy {
                max_backlog: 0.0,
                ..policy()
            },
        );
        let mut pending = Pending::default();
        pending.admit(30.0, 0.0, 1.0);
        pending.change(|changes| changes.sources = Some(vec![0.5; 64 * 64]));
        pending.admit(30.0, 0.0, 1.0);
        let snapshot = field.run(std::mem::take(&mut pending.segments));
        assert!(
            snapshot.dropped_seconds > 59.0,
            "{}",
            snapshot.dropped_seconds
        );
        assert_eq!(field.grid.sources_mut()[0], 0.5);
    }
}
