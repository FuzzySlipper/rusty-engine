//! The sound model playback and `Audio.QueryArrival` share: how loud and how
//! bright a sound arrives through a spatial session's collision.
//!
//! A sound reaches a point the louder of two ways. Straight through, it falls
//! off with distance and loses level and treble to the solid it crosses.
//! Through open air (an [`AirSearch`] round the solid), it falls off with
//! the path's length, unfiltered. The listener's space adds reverb by how
//! enclosed it is. Entity boxes take no part.

use engine_spatial::{AirReach, AirSearch, Enclosure, VoxelCollisionScene, ENCLOSURE_METRES};
use render_presentation::AudioRolloff;

/// The collision a sound travels through.
pub use engine_spatial::VoxelCollisionScene as Scene;

/// Crossing any solid costs this much level.
pub const OCCLUSION_DECIBELS: f32 = 6.0;
/// And each metre of it this much more.
pub const OCCLUSION_DECIBELS_PER_METRE: f32 = 10.0;
/// The cutoff of an unfiltered sound.
pub const OPEN_CUTOFF_HZ: f32 = 20_000.0;
/// The cutoff halves for each this many metres of solid crossed...
pub const CUTOFF_HALVING_METRES: f32 = 0.25;
/// ...down to this.
pub const FLOOR_CUTOFF_HZ: f32 = 250.0;
/// The most listener points one query answers.
pub const QUERY_POINTS: usize = 64;
/// How far either side of the straight line the two side rays of an
/// occluded measurement run, in metres.
const SIDE_RAY_METRES: f64 = 0.3;
/// Reverb feedback in the smallest and the largest enclosed spaces.
const SMALL_SPACE_FEEDBACK: f64 = 0.55;
const LARGE_SPACE_FEEDBACK: f64 = 0.92;

/// The way a sound reaches a point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrivalPath {
    /// Beyond its range.
    Silent,
    /// Straight, through nothing.
    Open,
    /// Straight, through solid.
    Occluded,
    /// By an open path round the solid.
    Around,
}

/// The straight line from a source to a point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Direct {
    pub distance: f64,
    /// The amplitude left after the solid crossed: one when nothing is.
    pub transmission: f32,
    /// The cutoff the solid leaves; `None` when nothing is crossed.
    pub low_pass_hz: Option<f32>,
    /// Rays cast to measure it.
    pub rays: u32,
}

/// How a sound arrives at one point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Arrival {
    pub path: ArrivalPath,
    /// The amplitude it arrives at, as playback plays it, before the
    /// source's own volume and its bus.
    pub gain: f32,
    pub low_pass_hz: Option<f32>,
    /// The length of the path it takes.
    pub metres: f64,
    /// The open path, when it takes one.
    pub around: Option<AirReach>,
}

/// The level and cutoff left after crossing `solid_metres` of solid.
pub fn transmission(solid_metres: f64) -> (f32, Option<f32>) {
    if solid_metres <= 0.0 {
        return (1.0, None);
    }
    let solid = solid_metres as f32;
    let decibels = OCCLUSION_DECIBELS + OCCLUSION_DECIBELS_PER_METRE * solid;
    let cutoff =
        (OPEN_CUTOFF_HZ * 0.5_f32.powf(solid / CUTOFF_HALVING_METRES)).max(FLOOR_CUTOFF_HZ);
    (10.0_f32.powf(-decibels / 20.0), Some(cutoff))
}

/// Measures the straight line from `source` to `point`. A clear centre ray
/// is the whole measurement; through solid, two side rays
/// [`SIDE_RAY_METRES`] either side join it and the three are averaged, so
/// a sound at a wall's edge is muffled by part rather than all at once.
pub fn direct(scene: &VoxelCollisionScene, source: [f64; 3], point: [f64; 3]) -> Direct {
    let distance = length(sub(point, source));
    let centre = scene.solid_metres(source, point);
    if centre <= 0.0 {
        return Direct {
            distance,
            transmission: 1.0,
            low_pass_hz: None,
            rays: 1,
        };
    }
    let side = side(sub(point, source));
    let mut gains = 0.0;
    let mut log_cutoffs = 0.0;
    for solid in [
        centre,
        scene.solid_metres(add(source, side), add(point, side)),
        scene.solid_metres(sub(source, side), sub(point, side)),
    ] {
        let (gain, cutoff) = transmission(solid);
        gains += gain;
        log_cutoffs += cutoff.unwrap_or(OPEN_CUTOFF_HZ).ln();
    }
    let cutoff = (log_cutoffs / 3.0).exp();
    Direct {
        distance,
        transmission: gains / 3.0,
        low_pass_hz: (cutoff < OPEN_CUTOFF_HZ * 0.999).then_some(cutoff),
        rays: 3,
    }
}

/// How a sound with this range and rolloff arrives, from its straight line
/// and the open path to the point, if any.
pub fn arrival(
    direct: &Direct,
    around: Option<AirReach>,
    max_distance: f32,
    rolloff: AudioRolloff,
) -> Arrival {
    if direct.distance >= f64::from(max_distance) {
        return Arrival {
            path: ArrivalPath::Silent,
            gain: 0.0,
            low_pass_hz: None,
            metres: direct.distance,
            around: None,
        };
    }
    let straight = rolloff.gain(max_distance, direct.distance as f32) * direct.transmission;
    if direct.low_pass_hz.is_none() && direct.transmission >= 1.0 {
        return Arrival {
            path: ArrivalPath::Open,
            gain: straight,
            low_pass_hz: None,
            metres: direct.distance,
            around: None,
        };
    }
    if let Some(reach) = around {
        let gain = rolloff.gain(max_distance, reach.metres as f32);
        if gain > straight {
            return Arrival {
                path: ArrivalPath::Around,
                gain,
                low_pass_hz: None,
                metres: reach.metres,
                around: Some(reach),
            };
        }
    }
    Arrival {
        path: ArrivalPath::Occluded,
        gain: straight,
        low_pass_hz: direct.low_pass_hz,
        metres: direct.distance,
        around: None,
    }
}

/// The reverb send and feedback a listener's space gives, `level` scaling
/// the send: none in the open, rising with how enclosed it is, ringing
/// longer as its walls are further off.
pub fn space_reverb(enclosure: Enclosure, level: f32) -> (f32, f64) {
    let size = (enclosure.mean_free_metres / ENCLOSURE_METRES).clamp(0.0, 1.0);
    (
        level * enclosure.closed as f32,
        SMALL_SPACE_FEEDBACK + (LARGE_SPACE_FEEDBACK - SMALL_SPACE_FEEDBACK) * size,
    )
}

/// What one query found.
pub struct QueryOutcome {
    pub arrivals: Vec<(Arrival, [f64; 3])>,
    pub cells_searched: u32,
    pub rays_cast: u32,
    pub truncated: bool,
}

/// How a sound made at `source` arrives at each of `points`, with the way
/// it arrives from at each. An open-air search runs only when a point within
/// range is occluded.
pub fn query(
    scene: &VoxelCollisionScene,
    source: [f64; 3],
    range: f32,
    rolloff: AudioRolloff,
    points: &[[f64; 3]],
) -> QueryOutcome {
    let directs: Vec<Direct> = points
        .iter()
        .map(|point| {
            if length(sub(*point, source)) >= f64::from(range) {
                // Out of range is silent however it would go.
                Direct {
                    distance: length(sub(*point, source)),
                    transmission: 1.0,
                    low_pass_hz: None,
                    rays: 0,
                }
            } else {
                direct(scene, source, *point)
            }
        })
        .collect();
    let mut rays: u32 = directs.iter().map(|direct| direct.rays).sum();
    let occluded: Vec<[f64; 3]> = points
        .iter()
        .zip(&directs)
        .filter(|(_, direct)| direct.distance < f64::from(range) && direct.transmission < 1.0)
        .map(|(point, _)| *point)
        .collect();
    let search =
        (!occluded.is_empty()).then(|| AirSearch::run(scene, source, f64::from(range), &occluded));
    let arrivals = points
        .iter()
        .zip(&directs)
        .map(|(point, direct)| {
            let around = (direct.transmission < 1.0)
                .then(|| search.as_ref()?.reach(scene, *point))
                .flatten();
            let arrival = arrival(direct, around, range, rolloff);
            let from = arrival.around.map_or(source, |reach| reach.near_point);
            (arrival, from)
        })
        .collect();
    if let Some(search) = &search {
        rays = rays.saturating_add(search.segments);
    }
    QueryOutcome {
        arrivals,
        cells_searched: search
            .as_ref()
            .map_or(0, |search| search.settled_cells() as u32),
        rays_cast: rays,
        truncated: search.as_ref().is_some_and(|search| search.truncated),
    }
}

/// How one playing sound is heard: where from, and what is left of it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Heard {
    /// Where an open path makes it heard from: the listener moved along the
    /// way the path leaves it by the path's length. `None` hears it at its
    /// emitter.
    pub heard_at: Option<[f32; 3]>,
    pub transmission: f32,
    pub low_pass_hz: Option<f32>,
    pub path: ArrivalPath,
}

/// One playing spatial sound to hear.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sounding {
    pub position: [f32; 3],
    pub max_distance: f32,
    pub rolloff: AudioRolloff,
}

/// A listener search is rebuilt at most this often while the listener moves
/// or the scene changes.
const LISTENER_SEARCH_SECONDS: f64 = 0.25;
/// Searches cover ranges in steps of this many metres, so small range
/// changes reuse one.
const SEARCH_RANGE_STEP: f64 = 8.0;

/// The listener's side of playback acoustics: one open-air search from the
/// listener, shared by every occluded sound since a path is as long either
/// way, and the listener's space. The search runs on its own worker thread,
/// off the update; sounds are heard by the latest one finished.
#[derive(Default)]
pub struct ListenerAcoustics {
    search: Option<ListenerSearch>,
    worker: Option<SearchWorker>,
    /// A search was sent to the worker and has not come back.
    searching: bool,
    space: Option<(SceneKey, [i64; 3], Enclosure)>,
}

/// The listener search's worker thread: it takes a search, runs it and
/// hands it back, and ends when its owner drops.
struct SearchWorker {
    jobs: Option<std::sync::mpsc::Sender<SearchJob>>,
    done: std::sync::mpsc::Receiver<ListenerSearch>,
    thread: Option<std::thread::JoinHandle<()>>,
}

struct SearchJob {
    scene: std::sync::Arc<VoxelCollisionScene>,
    root: [f64; 3],
    range: f64,
    at: f64,
}

impl SearchWorker {
    fn start() -> Option<Self> {
        let (jobs, inbox) = std::sync::mpsc::channel::<SearchJob>();
        let (outbox, done) = std::sync::mpsc::channel();
        let thread = std::thread::Builder::new()
            .name("rusty-audio-acoustics".to_owned())
            .spawn(move || {
                crate::worker_priority::below_normal();
                while let Ok(job) = inbox.recv() {
                    let search = AirSearch::run(&job.scene, job.root, job.range, &[]);
                    let finished = ListenerSearch {
                        search,
                        scene: SceneKey::of(&job.scene),
                        cell: air_cell(job.root),
                        range: job.range,
                        at: job.at,
                    };
                    if outbox.send(finished).is_err() {
                        break;
                    }
                }
            })
            .ok()?;
        Some(Self {
            jobs: Some(jobs),
            done,
            thread: Some(thread),
        })
    }
}

impl Drop for SearchWorker {
    fn drop(&mut self) {
        self.jobs = None;
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

struct ListenerSearch {
    search: AirSearch,
    scene: SceneKey,
    cell: [i64; 3],
    range: f64,
    /// Seconds on the caller's clock when it ran.
    at: f64,
}

/// One scene's collision as it stands: replaced scenes and edits differ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SceneKey(usize, u64, u64);

impl SceneKey {
    fn of(scene: &std::sync::Arc<VoxelCollisionScene>) -> Self {
        Self(
            std::sync::Arc::as_ptr(scene) as usize,
            scene.projection_version(),
            scene.static_mesh_collision_revision(),
        )
    }
}

impl ListenerAcoustics {
    /// The listener's space reverb (send, feedback), sampled again when the
    /// listener changes cell or the scene changes.
    pub fn space(
        &mut self,
        scene: &std::sync::Arc<VoxelCollisionScene>,
        listener: [f32; 3],
        level: f32,
    ) -> (f32, f64) {
        let key = SceneKey::of(scene);
        let at = widen(listener);
        let cell = air_cell(at);
        let enclosure = match self.space {
            Some((held, held_cell, enclosure)) if held == key && held_cell == cell => enclosure,
            _ => {
                let enclosure = scene.enclosure(at);
                self.space = Some((key, cell, enclosure));
                enclosure
            }
        };
        space_reverb(enclosure, level)
    }

    /// How each of `sounds` is heard at `listener`. `now` is seconds on any
    /// steady clock; the listener's search is rebuilt at most every
    /// quarter second as the listener moves.
    pub fn hear(
        &mut self,
        scene: &std::sync::Arc<VoxelCollisionScene>,
        listener: [f32; 3],
        sounds: &[Sounding],
        now: f64,
    ) -> Vec<Heard> {
        let at = widen(listener);
        self.collect_search();

        let directs: Vec<Direct> = sounds
            .iter()
            .map(|sound| direct(scene, widen(sound.position), at))
            .collect();
        let range = sounds
            .iter()
            .zip(&directs)
            .filter(|(_, direct)| direct.transmission < 1.0)
            .map(|(sound, _)| f64::from(sound.max_distance))
            .fold(0.0, f64::max);
        if range > 0.0 {
            let range = (range / SEARCH_RANGE_STEP).ceil() * SEARCH_RANGE_STEP;
            let key = SceneKey::of(scene);
            let cell = air_cell(at);
            let stale = match &self.search {
                None => true,
                Some(held) => {
                    held.range < range
                        || ((held.scene != key || held.cell != cell)
                            && now - held.at >= LISTENER_SEARCH_SECONDS)
                }
            };
            if stale && !self.searching {
                self.start_search(scene, at, range, now);
            }
        }
        sounds
            .iter()
            .zip(&directs)
            .map(|(sound, direct)| {
                let around = (direct.transmission < 1.0)
                    .then(|| {
                        let search = &self.search.as_ref()?.search;
                        search.reach(scene, widen(sound.position))
                    })
                    .flatten();
                let arrival = arrival(direct, around, sound.max_distance, sound.rolloff);
                match (arrival.path, arrival.around) {
                    (ArrivalPath::Around, Some(reach)) => {
                        let leaves = sub(reach.near_root, at);
                        let leaves = if length(leaves) > 1e-6 {
                            leaves
                        } else {
                            sub(widen(sound.position), at)
                        };
                        let size = length(leaves).max(1e-9);
                        let heard_at = add(at, leaves.map(|value| value / size * reach.metres));
                        Heard {
                            heard_at: Some(heard_at.map(|value| value as f32)),
                            transmission: 1.0,
                            low_pass_hz: None,
                            path: ArrivalPath::Around,
                        }
                    }
                    (ArrivalPath::Occluded, _) => Heard {
                        heard_at: None,
                        transmission: direct.transmission,
                        low_pass_hz: direct.low_pass_hz,
                        path: ArrivalPath::Occluded,
                    },
                    (path, _) => Heard {
                        heard_at: None,
                        transmission: 1.0,
                        low_pass_hz: None,
                        path,
                    },
                }
            })
            .collect()
    }
}

impl ListenerAcoustics {
    /// Whether a listener search is running on the worker.
    pub fn searching(&mut self) -> bool {
        self.collect_search();
        self.searching
    }

    /// Cells the latest finished listener search settled.
    pub fn searched_cells(&self) -> Option<usize> {
        self.search.as_ref().map(|held| held.search.settled_cells())
    }

    fn collect_search(&mut self) {
        let Some(worker) = &self.worker else {
            return;
        };
        while let Ok(finished) = worker.done.try_recv() {
            self.search = Some(finished);
            self.searching = false;
        }
    }

    fn start_search(
        &mut self,
        scene: &std::sync::Arc<VoxelCollisionScene>,
        root: [f64; 3],
        range: f64,
        now: f64,
    ) {
        if self.worker.is_none() {
            self.worker = SearchWorker::start();
        }
        let job = SearchJob {
            scene: std::sync::Arc::clone(scene),
            root,
            range,
            at: now,
        };
        match self.worker.as_ref().and_then(|worker| worker.jobs.as_ref()) {
            Some(jobs) if jobs.send(job).is_ok() => self.searching = true,
            // No worker thread: search here.
            _ => {
                let search = AirSearch::run(scene, root, range, &[]);
                self.search = Some(ListenerSearch {
                    search,
                    scene: SceneKey::of(scene),
                    cell: air_cell(root),
                    range,
                    at: now,
                });
            }
        }
    }
}

fn widen(value: [f32; 3]) -> [f64; 3] {
    value.map(f64::from)
}

fn air_cell(point: [f64; 3]) -> [i64; 3] {
    point.map(|value| (value / engine_spatial::AIR_CELL_METRES).floor() as i64)
}

/// A unit vector across the line `along`, level where it can be, scaled to
/// [`SIDE_RAY_METRES`].
fn side(along: [f64; 3]) -> [f64; 3] {
    let across = [-along[2], 0.0, along[0]];
    let across = if length(across) > 1e-9 {
        across
    } else {
        [1.0, 0.0, 0.0]
    };
    let size = length(across);
    across.map(|value| value / size * SIDE_RAY_METRES)
}

fn sub(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}

fn add(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [left[0] + right[0], left[1] + right[1], left[2] + right[2]]
}

fn length(value: [f64; 3]) -> f64 {
    (value[0] * value[0] + value[1] * value[1] + value[2] * value[2]).sqrt()
}

#[cfg(test)]
mod tests;
