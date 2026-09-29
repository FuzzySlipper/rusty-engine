//! Frames the runtime renders with `render-wgpu`, streamed to the browser
//! shell through the development host's frame route.
//!
//! The runtime applies each product call's renderer publications here, on its
//! own thread, as it commits them. A render thread draws the committed scene
//! offscreen while a viewer is attached, reads it back, encodes it and hands
//! it to [`ProductDevFrameStream`]. It draws as soon as a change is applied:
//! every step while the simulation runs, and once per change while it is
//! held, so the last frame stays on screen and inspection can still redraw
//! it.
//!
//! Inspection can replace the primary views' camera with an observer pose,
//! switch to on-demand drawing (a frame only when one is requested), and
//! request a frame and wait for it. Each drawn frame's facts (sequence,
//! simulation step, size, observer, composition) are kept for the runtime's
//! presentation observation.
//!
//! Only this crate may depend on the frame encoder
//! (`scripts/dependency_boundary_check.py`).

#![forbid(unsafe_code)]

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use product_dev_host::{ProductDevFrame, ProductDevFrameFormat, ProductDevFrameStream};
pub use render_host_contracts::{RendererCameraPose, RendererViewComposition};
pub use render_wgpu::{AnimationFact, EntityPositions, RendererOptions, ResourceSource};
use render_wgpu::{Gpu, OffscreenTarget, Renderer};
use runtime_publication::RuntimePublication;

/// Size drawn for an explicit request while no viewer states one.
const UNWATCHED_SIZE: (u32, u32) = (1280, 720);
/// How long an idle render thread sleeps before rechecking for viewers.
const IDLE_WAIT: Duration = Duration::from_millis(250);
/// JPEG quality: 88 KB per 1280x720 Doom frame, without visible blocking.
const JPEG_QUALITY: u8 = 80;
/// Frames the rolling statistics cover.
const STATS_WINDOW: usize = 120;

/// How frames travel. JPEG is the default; raw RGBA exists to measure what
/// the encoder saves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamFormat {
    Jpeg,
    Rgba8,
}

/// The runtime's handle on the streamed renderer. Dropping it stops the
/// render thread.
pub struct FrameStreamer {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

struct Shared {
    gpu: Gpu,
    options: RendererOptions,
    epoch: Instant,
    scene: Mutex<Scene>,
    wake: Condvar,
    frames: Arc<ProductDevFrameStream>,
}

struct Scene {
    renderer: Renderer,
    /// Something changed since the last frame was drawn.
    dirty: bool,
    held: bool,
    step: u64,
    /// Engine presentation time the last applied call reached.
    elapsed_seconds: f64,
    animation_facts: Vec<AnimationFact>,
    skipped_ops: BTreeMap<&'static str, u64>,
    last_skip: Option<String>,
    stats: VecDeque<FrameCost>,
    stop: bool,
    /// Counts renderers built; a rebaseline replaces the renderer.
    renderer_id: u64,
    /// The retained world revision the last applied publications reached.
    world_revision: u64,
    composition: Option<Arc<RendererViewComposition>>,
    /// Counts installed compositions on the current renderer.
    composition_revision: u64,
    observer: Option<RendererCameraPose>,
    /// Draw only on request, not on change.
    on_demand: bool,
    /// Frame requests made, and the latest one a drawn frame answered.
    requested: u64,
    answered: u64,
    last_drawn: Option<DrawnFrame>,
}

/// The Engine state applied publications bring the scene to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SceneState {
    /// Engine presentation time the call reached.
    pub elapsed_seconds: f64,
    /// Retained world revision the publications reach.
    pub world_revision: u64,
    /// Simulation step the scene shows.
    pub step: u64,
    /// Paused, or inspection time held.
    pub held: bool,
}

/// What one drawn and published frame showed.
#[derive(Debug, Clone)]
pub struct DrawnFrame {
    /// The frame's sequence on the frame route.
    pub sequence: u64,
    pub step: u64,
    pub held: bool,
    pub width: u32,
    pub height: u32,
    pub drawn_at: Instant,
    /// Changes with each renderer the streamer builds.
    pub renderer_id: u64,
    /// The retained world revision the frame shows.
    pub world_revision: u64,
    pub composition: Option<Arc<RendererViewComposition>>,
    pub composition_revision: u64,
    pub observer: Option<RendererCameraPose>,
}

/// The inspection state and the last drawn frame.
#[derive(Debug, Clone)]
pub struct StreamInspection {
    pub on_demand: bool,
    pub held: bool,
    pub observer: Option<RendererCameraPose>,
    pub composition: Option<Arc<RendererViewComposition>>,
    /// A change or request is waiting for a frame.
    pub pending: bool,
    pub last_drawn: Option<DrawnFrame>,
}

#[derive(Debug, Clone, Copy)]
struct FrameCost {
    at: Instant,
    render_ms: f64,
    readback_ms: f64,
    encode_ms: f64,
    bytes: usize,
}

/// Rolling costs of the recent frames, for diagnostics and evidence.
#[derive(Debug, Clone, PartialEq)]
pub struct StreamStats {
    pub adapter: String,
    pub frames: usize,
    pub frames_per_second: f64,
    pub render_ms: f64,
    pub readback_ms: f64,
    pub encode_ms: f64,
    pub bytes_per_frame: f64,
    pub bytes_per_second: f64,
    pub skipped_ops: BTreeMap<&'static str, u64>,
    pub last_skip: Option<String>,
}

impl FrameStreamer {
    /// Opens a headless device and starts the render thread. Frames go to
    /// `frames` whenever a viewer is attached there.
    pub fn start(
        options: RendererOptions,
        format: StreamFormat,
        frames: Arc<ProductDevFrameStream>,
    ) -> Result<Self, String> {
        let gpu = Gpu::headless().map_err(|error| error.to_string())?;
        let shared = Arc::new(Shared {
            scene: Mutex::new(Scene {
                renderer: Renderer::new(&gpu, options),
                dirty: true,
                held: true,
                step: 0,
                elapsed_seconds: 0.0,
                animation_facts: Vec::new(),
                skipped_ops: BTreeMap::new(),
                last_skip: None,
                stats: VecDeque::with_capacity(STATS_WINDOW),
                stop: false,
                renderer_id: 1,
                world_revision: 0,
                composition: None,
                composition_revision: 0,
                observer: None,
                on_demand: false,
                requested: 0,
                answered: 0,
                last_drawn: None,
            }),
            gpu,
            options,
            epoch: Instant::now(),
            wake: Condvar::new(),
            frames,
        });
        let waker = Arc::downgrade(&shared);
        shared.frames.set_demand_waker(move || {
            if let Some(shared) = waker.upgrade() {
                shared.scene().dirty = true;
                shared.wake.notify_all();
            }
        });
        let thread_shared = Arc::clone(&shared);
        let thread = std::thread::Builder::new()
            .name("rusty-render-stream".to_owned())
            .spawn(move || render_loop(&thread_shared, format))
            .map_err(|error| format!("could not start the render thread: {error}"))?;
        Ok(Self {
            shared,
            thread: Some(thread),
        })
    }

    /// Applies one committed call's renderer publications in order and moves
    /// effects and animation to the Engine presentation time the call
    /// reached. Other publications are not the renderer's. `state` is what
    /// the call brought the Engine to; it lands with the publications, so no
    /// frame shows them under the previous step.
    pub fn apply(
        &self,
        publications: &[RuntimePublication],
        resources: &dyn ResourceSource,
        entities: EntityPositions<'_>,
        state: SceneState,
    ) {
        let now = self.shared.now();
        let mut scene = self.shared.scene();
        if scene.apply(publications, resources, entities, state, now) {
            drop(scene);
            self.shared.wake.notify_all();
        }
    }

    /// Replaces the renderer with one built from a complete baseline, after
    /// a product call's renderer work was lost or the world was replaced.
    /// The render thread never sees the empty renderer in between.
    pub fn rebaseline(
        &self,
        baseline: &[RuntimePublication],
        resources: &dyn ResourceSource,
        entities: EntityPositions<'_>,
        state: SceneState,
    ) {
        let now = self.shared.now();
        let mut scene = self.shared.scene();
        scene.renderer = Renderer::new(&self.shared.gpu, self.shared.options);
        let observer = scene.observer;
        scene.renderer.set_observer(observer);
        scene.renderer_id += 1;
        scene.composition = None;
        scene.composition_revision = 0;
        scene.elapsed_seconds = state.elapsed_seconds;
        scene.renderer.set_animation_time(state.elapsed_seconds);
        scene.animation_facts.clear();
        scene.apply(baseline, resources, entities, state, now);
        scene.dirty = true;
        drop(scene);
        self.shared.wake.notify_all();
    }

    /// Whether the simulation is held, and the step the scene shows, for a
    /// lifecycle change no product call published (pause, time mode). A held
    /// scene is drawn once per change instead of continuously.
    pub fn set_simulation(&self, held: bool, step: u64) {
        let mut scene = self.shared.scene();
        if scene.held != held || scene.step != step {
            scene.held = held;
            scene.step = step;
            scene.dirty = true;
            drop(scene);
            self.shared.wake.notify_all();
        }
    }

    /// Draws the primary views from `pose` instead of the product's cameras,
    /// or from the product's cameras again with `None`.
    pub fn set_observer(&self, pose: Option<RendererCameraPose>) {
        let mut scene = self.shared.scene();
        scene.observer = pose;
        scene.renderer.set_observer(pose);
        scene.dirty = true;
        drop(scene);
        self.shared.wake.notify_all();
    }

    /// On demand, changes wait for [`Self::draw_now`] instead of drawing.
    pub fn set_on_demand(&self, on_demand: bool) {
        let mut scene = self.shared.scene();
        scene.on_demand = on_demand;
        drop(scene);
        self.shared.wake.notify_all();
    }

    /// Draws a frame of the current scene, even on demand or with no viewer,
    /// and waits up to `timeout` for it to be published.
    pub fn draw_now(&self, timeout: Duration) -> Option<DrawnFrame> {
        let mut scene = self.shared.scene();
        scene.requested += 1;
        let request = scene.requested;
        self.shared.wake.notify_all();
        let (scene, _) = self
            .shared
            .wake
            .wait_timeout_while(scene, timeout, |scene| {
                scene.answered < request && !scene.stop
            })
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        (scene.answered >= request)
            .then(|| scene.last_drawn.clone())
            .flatten()
    }

    pub fn inspection(&self) -> StreamInspection {
        let scene = self.shared.scene();
        StreamInspection {
            on_demand: scene.on_demand,
            held: scene.held,
            observer: scene.observer,
            composition: scene.composition.clone(),
            pending: scene.dirty || scene.requested > scene.answered,
            last_drawn: scene.last_drawn.clone(),
        }
    }

    /// Animation facts the drawn frames produced since the last call.
    pub fn take_animation_facts(&self) -> Vec<AnimationFact> {
        std::mem::take(&mut self.shared.scene().animation_facts)
    }

    pub fn stats(&self) -> StreamStats {
        let scene = self.shared.scene();
        let costs = &scene.stats;
        let median = |value: fn(&FrameCost) -> f64| {
            let mut values: Vec<f64> = costs.iter().map(value).collect();
            values.sort_by(f64::total_cmp);
            values.get(values.len() / 2).copied().unwrap_or(0.0)
        };
        let span = match (costs.front(), costs.back()) {
            (Some(first), Some(last)) if costs.len() > 1 => {
                last.at.duration_since(first.at).as_secs_f64()
            }
            _ => 0.0,
        };
        let total_bytes: usize = costs.iter().skip(1).map(|cost| cost.bytes).sum();
        let adapter = self.shared.gpu.adapter_summary();
        StreamStats {
            adapter: format!("{} ({})", adapter.name, adapter.backend),
            frames: costs.len(),
            frames_per_second: if span > 0.0 {
                (costs.len() - 1) as f64 / span
            } else {
                0.0
            },
            render_ms: median(|cost| cost.render_ms),
            readback_ms: median(|cost| cost.readback_ms),
            encode_ms: median(|cost| cost.encode_ms),
            bytes_per_frame: median(|cost| cost.bytes as f64),
            bytes_per_second: if span > 0.0 {
                total_bytes as f64 / span
            } else {
                0.0
            },
            skipped_ops: scene.skipped_ops.clone(),
            last_skip: scene.last_skip.clone(),
        }
    }
}

impl Drop for FrameStreamer {
    fn drop(&mut self) {
        self.shared.scene().stop = true;
        self.shared.wake.notify_all();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Shared {
    fn scene(&self) -> MutexGuard<'_, Scene> {
        self.scene.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// The presentation clock camera motion is sampled on.
    fn now(&self) -> f64 {
        self.epoch.elapsed().as_secs_f64()
    }
}

impl Scene {
    /// Applies publications and the state they reach under the caller's
    /// lock. Returns whether anything changed.
    fn apply(
        &mut self,
        publications: &[RuntimePublication],
        resources: &dyn ResourceSource,
        entities: EntityPositions<'_>,
        state: SceneState,
        now: f64,
    ) -> bool {
        let mut applied = false;
        for publication in publications {
            let issues = match publication {
                RuntimePublication::Frame(frame) => self.renderer.apply(frame, resources),
                RuntimePublication::Presentation(frame) => {
                    self.renderer.apply_presentation(frame, resources, entities)
                }
                RuntimePublication::ViewComposition(composition) => {
                    self.renderer.set_view_composition(composition, now);
                    self.composition = Some(Arc::new(composition.clone()));
                    self.composition_revision += 1;
                    Vec::new()
                }
                _ => continue,
            };
            applied = true;
            self.record_issues(issues);
        }
        let advanced = state.elapsed_seconds - self.elapsed_seconds;
        if advanced > 0.0 {
            let issues = self.renderer.advance_effects(advanced, entities);
            self.record_issues(issues);
            self.renderer.set_animation_time(state.elapsed_seconds);
            applied = true;
        }
        self.elapsed_seconds = state.elapsed_seconds;
        self.world_revision = state.world_revision;
        if (self.step, self.held) != (state.step, state.held) {
            (self.step, self.held) = (state.step, state.held);
            applied = true;
        }
        self.dirty |= applied;
        applied
    }

    fn record_issues(&mut self, issues: Vec<render_wgpu::ApplyIssue>) {
        for issue in issues {
            *self.skipped_ops.entry(issue.op).or_default() += 1;
            self.last_skip = Some(format!("{}: {}", issue.op, issue.detail));
        }
    }
}

fn render_loop(shared: &Shared, format: StreamFormat) {
    let mut target: Option<OffscreenTarget> = None;
    let mut pixels = Vec::new();
    loop {
        let mut scene = shared.scene();
        let size = loop {
            if scene.stop {
                return;
            }
            // A running simulation applies a change every step, so frames
            // follow the simulation rate; a held scene draws once per change.
            // On demand, only requests draw.
            let wanted = shared.frames.wanted_size();
            if scene.requested > scene.answered {
                break wanted.unwrap_or(UNWATCHED_SIZE);
            }
            if let Some(size) = wanted.filter(|_| scene.dirty && !scene.on_demand) {
                break size;
            }
            scene = shared
                .wake
                .wait_timeout(scene, IDLE_WAIT)
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .0;
        };
        let target = match &mut target {
            Some(target) => {
                target.resize(&shared.gpu, size.0, size.1);
                target
            }
            None => target.insert(OffscreenTarget::new(&shared.gpu, size.0, size.1)),
        };
        let started = Instant::now();
        scene.dirty = false;
        let request = scene.requested;
        let mut drawn = DrawnFrame {
            sequence: 0,
            step: scene.step,
            held: scene.held,
            width: 0,
            height: 0,
            drawn_at: started,
            renderer_id: scene.renderer_id,
            world_revision: scene.world_revision,
            composition: scene.composition.clone(),
            composition_revision: scene.composition_revision,
            observer: scene.observer,
        };
        scene.renderer.render_view_composition(target, shared.now());
        let facts = scene.renderer.take_animation_facts();
        scene.animation_facts.extend(facts);
        let (held, step) = (scene.held, scene.step);
        drop(scene);
        let rendered = Instant::now();
        target.read_rgba_into(&shared.gpu, &mut pixels);
        let read = Instant::now();
        let (width, height) = target.size();
        let (format, payload) = match format {
            StreamFormat::Jpeg => (ProductDevFrameFormat::Jpeg, encode_jpeg(&pixels, width, height)),
            StreamFormat::Rgba8 => (ProductDevFrameFormat::Rgba8, pixels.clone()),
        };
        let encoded = Instant::now();
        let cost = FrameCost {
            at: started,
            render_ms: ms(rendered - started),
            readback_ms: ms(read - rendered),
            encode_ms: ms(encoded - read),
            bytes: payload.len(),
        };
        drawn.sequence = shared.frames.publish(ProductDevFrame {
            width,
            height,
            format,
            held,
            step,
            payload,
        });
        (drawn.width, drawn.height) = (width, height);
        let mut scene = shared.scene();
        if scene.stats.len() == STATS_WINDOW {
            scene.stats.pop_front();
        }
        scene.stats.push_back(cost);
        scene.answered = scene.answered.max(request);
        scene.last_drawn = Some(drawn);
        drop(scene);
        shared.wake.notify_all();
    }
}

fn encode_jpeg(rgba: &[u8], width: u32, height: u32) -> Vec<u8> {
    let mut jpeg = Vec::with_capacity(rgba.len() / 16);
    // OffscreenTarget sides fit u16: the frame route bounds them to 4096.
    jpeg_encoder::Encoder::new(&mut jpeg, JPEG_QUALITY)
        .encode(
            rgba,
            width as u16,
            height as u16,
            jpeg_encoder::ColorType::Rgba,
        )
        .expect("a JPEG encodes into memory");
    jpeg
}

fn ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_encodes_as_one_baseline_jpeg() {
        let rgba: Vec<u8> = (0..64 * 32).flat_map(|i| [i as u8, 24, 32, 255]).collect();
        let jpeg = encode_jpeg(&rgba, 64, 32);
        assert_eq!(&jpeg[..2], &[0xFF, 0xD8], "start of image");
        assert_eq!(&jpeg[jpeg.len() - 2..], &[0xFF, 0xD9], "end of image");
        assert!(jpeg.len() < rgba.len());
    }
}
