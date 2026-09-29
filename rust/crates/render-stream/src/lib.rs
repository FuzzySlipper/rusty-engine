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
//! Only this crate may depend on the frame encoder
//! (`scripts/dependency_boundary_check.py`).

#![forbid(unsafe_code)]

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use product_dev_host::{ProductDevFrame, ProductDevFrameFormat, ProductDevFrameStream};
pub use render_wgpu::{AnimationFact, EntityPositions, RendererOptions, ResourceSource};
use render_wgpu::{Gpu, OffscreenTarget, Renderer};
use runtime_publication::RuntimePublication;

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

    /// Applies one committed call's renderer publications in order, then
    /// moves effects and animation to the Engine presentation time the call
    /// reached. Other publications are not the renderer's.
    pub fn apply(
        &self,
        publications: &[RuntimePublication],
        resources: &dyn ResourceSource,
        entities: EntityPositions<'_>,
        elapsed_seconds: f64,
    ) {
        let now = self.shared.now();
        let mut scene = self.shared.scene();
        let mut applied = false;
        for publication in publications {
            let issues = match publication {
                RuntimePublication::Frame(frame) => scene.renderer.apply(frame, resources),
                RuntimePublication::Presentation(frame) => {
                    scene
                        .renderer
                        .apply_presentation(frame, resources, entities)
                }
                RuntimePublication::ViewComposition(composition) => {
                    scene.renderer.set_view_composition(composition, now);
                    Vec::new()
                }
                _ => continue,
            };
            applied = true;
            scene.record_issues(issues);
        }
        let advanced = elapsed_seconds - scene.elapsed_seconds;
        if advanced > 0.0 {
            let issues = scene.renderer.advance_effects(advanced, entities);
            scene.record_issues(issues);
            scene.renderer.set_animation_time(elapsed_seconds);
            applied = true;
        }
        scene.elapsed_seconds = elapsed_seconds;
        if applied {
            scene.dirty = true;
            drop(scene);
            self.shared.wake.notify_all();
        }
    }

    /// Replaces the renderer with one built from a complete baseline, after
    /// a product call's renderer work was lost or the world was replaced.
    pub fn rebaseline(
        &self,
        baseline: &[RuntimePublication],
        resources: &dyn ResourceSource,
        entities: EntityPositions<'_>,
        elapsed_seconds: f64,
    ) {
        {
            let mut scene = self.shared.scene();
            scene.renderer = Renderer::new(&self.shared.gpu, self.shared.options);
            scene.elapsed_seconds = elapsed_seconds;
            scene.renderer.set_animation_time(elapsed_seconds);
            scene.animation_facts.clear();
        }
        self.apply(baseline, resources, entities, elapsed_seconds);
    }

    /// Whether the simulation is held, and the step the scene shows. A held
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
            if let Some(size) = shared.frames.wanted_size().filter(|_| scene.dirty) {
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
        shared.frames.publish(ProductDevFrame {
            width,
            height,
            format,
            held,
            step,
            payload,
        });
        let mut scene = shared.scene();
        if scene.stats.len() == STATS_WINDOW {
            scene.stats.pop_front();
        }
        scene.stats.push_back(cost);
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
