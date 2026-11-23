//! The sound model playback and `Audio.QueryArrival` share: how loud and how
//! bright a sound arrives through a spatial session's collision.
//!
//! A sound reaches a point the louder of two ways. Straight through, it falls
//! off with distance and loses level and treble to the solid it crosses.
//! Through open air (a route over the session's air graph round the
//! solid), it falls off with the path's length, unfiltered. The listener's
//! space adds reverb by how enclosed it is. Entity boxes take no part.

use std::sync::Arc;

use engine_spatial::{AirGraph, AirRoute, Enclosure, VoxelCollisionScene, ENCLOSURE_METRES};
use render_presentation::AudioRolloff;

/// The collision a sound travels through.
pub use engine_spatial::VoxelCollisionScene as Scene;

/// The acoustic space playback hears through: a session's collision, its
/// air graph, and how much reverb its spaces add.
#[derive(Clone)]
pub struct AcousticWorld {
    pub scene: Arc<Scene>,
    pub air: crate::air::AirReader,
    pub reverb_level: f32,
}

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
    pub around: Option<AroundPath>,
}

/// An open path round solid, as an arrival reads it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AroundPath {
    pub metres: f64,
    /// A point about three metres along it from the sound.
    pub near_source: [f64; 3],
    /// A point about three metres back along it from where it is heard:
    /// the way it arrives.
    pub near_point: [f64; 3],
}

impl AroundPath {
    /// A route from the sound to the point.
    fn from_sound(route: &AirRoute) -> Self {
        Self {
            metres: route.metres,
            near_source: route.near_start(),
            near_point: route.near_end(),
        }
    }

    /// A route from the point to the sound.
    fn to_sound(route: &AirRoute) -> Self {
        Self {
            metres: route.metres,
            near_source: route.near_end(),
            near_point: route.near_start(),
        }
    }
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
    around: Option<AroundPath>,
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
/// it arrives from at each. A route over `air` runs only when a point within
/// range is occluded.
pub fn query(
    scene: &VoxelCollisionScene,
    air: &AirGraph,
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
    let rays: u32 = directs.iter().map(|direct| direct.rays).sum();
    let occluded: Vec<usize> = directs
        .iter()
        .enumerate()
        .filter(|(_, direct)| direct.distance < f64::from(range) && direct.transmission < 1.0)
        .map(|(index, _)| index)
        .collect();
    let targets: Vec<[f64; 3]> = occluded.iter().map(|index| points[*index]).collect();
    let routes =
        (!targets.is_empty()).then(|| air.route(scene, source, &targets, f64::from(range)));
    let mut around = vec![None; points.len()];
    if let Some(routes) = &routes {
        for (index, route) in occluded.iter().zip(&routes.routes) {
            around[*index] = route.as_ref().ok().map(AroundPath::from_sound);
        }
    }
    let arrivals = directs
        .iter()
        .zip(around)
        .map(|(direct, around)| {
            let arrival = arrival(direct, around, range, rolloff);
            let from = arrival.around.map_or(source, |path| path.near_point);
            (arrival, from)
        })
        .collect();
    QueryOutcome {
        arrivals,
        cells_searched: routes
            .as_ref()
            .map_or(0, |routes| routes.ports_settled + routes.cells_marched),
        rays_cast: rays,
        truncated: routes.as_ref().is_some_and(|routes| {
            routes.incomplete
                || routes
                    .routes
                    .iter()
                    .any(|route| route == &Err(engine_spatial::AirRouteFailure::NotBuilt))
        }),
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

/// The listener's side of playback acoustics: routes from the listener to
/// its occluded sounds over the session's air graph, found on a worker
/// thread off the update and kept while nothing they were found from
/// changes, and the listener's space. Sounds are heard by the latest routes
/// found.
#[derive(Default)]
pub struct ListenerAcoustics {
    routes: Option<Routes>,
    worker: Option<RouteWorker>,
    /// Routes were asked of the worker and have not come back.
    routing: bool,
    space: Option<(SceneKey, [i64; 3], Enclosure)>,
}

/// Routes from the listener, by each occluded sound's air cell and range.
struct Routes {
    key: RouteKey,
    around: std::collections::HashMap<([i64; 3], u32), Option<AroundPath>>,
}

/// What a set of listener routes was found from: the scene and graph, and
/// the air cells of the listener and of each occluded sound.
#[derive(Debug, Clone, PartialEq)]
struct RouteKey {
    scene: SceneKey,
    air: u64,
    listener: [i64; 3],
    sounds: Vec<([i64; 3], u32)>,
}

struct RouteJob {
    key: RouteKey,
    scene: Arc<VoxelCollisionScene>,
    air: Arc<AirGraph>,
    listener: [f64; 3],
    targets: Vec<[f64; 3]>,
}

impl RouteJob {
    fn run(self) -> Routes {
        let range = self
            .key
            .sounds
            .iter()
            .map(|(_, range)| f64::from(f32::from_bits(*range)))
            .fold(0.0, f64::max);
        let routes = self
            .air
            .route(&self.scene, self.listener, &self.targets, range);
        Routes {
            around: self
                .key
                .sounds
                .iter()
                .copied()
                .zip(
                    routes
                        .routes
                        .iter()
                        .map(|route| route.as_ref().ok().map(AroundPath::to_sound)),
                )
                .collect(),
            key: self.key,
        }
    }
}

/// The listener routes' worker thread: it takes a job, routes it and hands
/// the routes back, and ends when its owner drops.
struct RouteWorker {
    jobs: Option<std::sync::mpsc::Sender<RouteJob>>,
    done: std::sync::mpsc::Receiver<Routes>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl RouteWorker {
    fn start() -> Option<Self> {
        let (jobs, inbox) = std::sync::mpsc::channel::<RouteJob>();
        let (outbox, done) = std::sync::mpsc::channel();
        let thread = std::thread::Builder::new()
            .name("rusty-audio-acoustics".to_owned())
            .spawn(move || {
                crate::worker_priority::below_normal();
                while let Ok(job) = inbox.recv() {
                    if outbox.send(job.run()).is_err() {
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

impl Drop for RouteWorker {
    fn drop(&mut self) {
        self.jobs = None;
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// One scene's collision as it stands: replaced scenes and edits differ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SceneKey(usize, u64, u64);

impl SceneKey {
    fn of(scene: &Arc<VoxelCollisionScene>) -> Self {
        Self(
            Arc::as_ptr(scene) as usize,
            scene.projection_version(),
            scene.static_mesh_collision_revision(),
        )
    }
}

impl ListenerAcoustics {
    /// Whether routes are being found on the worker.
    pub fn routing(&mut self) -> bool {
        self.collect_routes();
        self.routing
    }

    fn collect_routes(&mut self) {
        let Some(worker) = &self.worker else {
            return;
        };
        while let Ok(routes) = worker.done.try_recv() {
            self.routes = Some(routes);
            self.routing = false;
        }
    }

    fn start_routes(&mut self, job: RouteJob) {
        if self.worker.is_none() {
            self.worker = RouteWorker::start();
        }
        match self.worker.as_ref().and_then(|worker| worker.jobs.as_ref()) {
            Some(jobs) => match jobs.send(job) {
                Ok(()) => self.routing = true,
                Err(std::sync::mpsc::SendError(job)) => self.routes = Some(job.run()),
            },
            // No worker thread: route here.
            None => self.routes = Some(job.run()),
        }
    }
}

impl ListenerAcoustics {
    /// The listener's space reverb (send, feedback), sampled again when the
    /// listener changes cell or the scene changes.
    pub fn space(
        &mut self,
        scene: &Arc<VoxelCollisionScene>,
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

    /// How each of `sounds` is heard at `listener`: straight, or round
    /// solid by the latest routes found over `air`.
    pub fn hear(
        &mut self,
        scene: &Arc<VoxelCollisionScene>,
        air: &Arc<AirGraph>,
        listener: [f32; 3],
        sounds: &[Sounding],
    ) -> Vec<Heard> {
        let at = widen(listener);
        let directs: Vec<Direct> = sounds
            .iter()
            .map(|sound| direct(scene, widen(sound.position), at))
            .collect();
        let occluded: Vec<usize> = directs
            .iter()
            .enumerate()
            .filter(|(_, direct)| direct.transmission < 1.0)
            .map(|(index, _)| index)
            .collect();
        self.collect_routes();
        let mut around_by_sound = vec![None; sounds.len()];
        if !occluded.is_empty() {
            let sound_keys: Vec<([i64; 3], u32)> = occluded
                .iter()
                .map(|index| {
                    (
                        air_cell(widen(sounds[*index].position)),
                        sounds[*index].max_distance.to_bits(),
                    )
                })
                .collect();
            let key = RouteKey {
                scene: SceneKey::of(scene),
                air: air.revision(),
                listener: air_cell(at),
                sounds: sound_keys.clone(),
            };
            let current = self.routes.as_ref().is_some_and(|routes| routes.key == key);
            if !current && !self.routing {
                self.start_routes(RouteJob {
                    key,
                    scene: Arc::clone(scene),
                    air: Arc::clone(air),
                    listener: at,
                    targets: occluded
                        .iter()
                        .map(|index| widen(sounds[*index].position))
                        .collect(),
                });
            }
            if let Some(routes) = &self.routes {
                for (index, sound_key) in occluded.iter().zip(sound_keys) {
                    around_by_sound[*index] = routes.around.get(&sound_key).copied().flatten();
                }
            }
        }
        sounds
            .iter()
            .zip(&directs)
            .zip(around_by_sound)
            .map(|((sound, direct), around)| {
                let arrival = arrival(direct, around, sound.max_distance, sound.rolloff);
                match (arrival.path, arrival.around) {
                    (ArrivalPath::Around, Some(path)) => {
                        let leaves = sub(path.near_point, at);
                        let leaves = if length(leaves) > 1e-6 {
                            leaves
                        } else {
                            sub(widen(sound.position), at)
                        };
                        let size = length(leaves).max(1e-9);
                        let heard_at = add(at, leaves.map(|value| value / size * path.metres));
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
