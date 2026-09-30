//! The runtime's frames streamed to the browser shell through the
//! development host's frame route.
//!
//! The runtime applies each product call's renderer changes to a
//! [`SceneDriver`] (render-wgpu). The stream's render thread
//! ([`FrameStreamer`]) draws the committed scene offscreen while a viewer is
//! attached, reads it back, encodes it and hands it to
//! [`ProductDevFrameStream`]. It draws as soon as a change is applied: every
//! step while the simulation runs, and once per change while it is held, so
//! the last frame stays on screen and inspection can still redraw it.
//!
//! Inspection can switch to on-demand drawing (a frame only when one is
//! requested) and request a frame and wait for it. Each drawn frame's facts
//! (sequence, simulation step, size, observer, composition) are kept for the
//! runtime's presentation observation.
//!
//! Only this crate may depend on the frame encoder
//! (`scripts/dependency_boundary_check.py`).

#![forbid(unsafe_code)]

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime};

use product_dev_host::{ProductDevFrame, ProductDevFrameFormat, ProductDevFrameStream};
use render_host_contracts::{RendererCameraPose, RendererViewComposition};
use render_wgpu::{Capture, DrawnCamera, OffscreenTarget, SceneDriver};

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

/// The stream's render thread over a [`SceneDriver`]. Dropping it stops the
/// thread.
pub struct FrameStreamer {
    driver: Arc<SceneDriver>,
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

/// What the stream keeps beside the driver's scene. Lock order: the
/// driver's scene, then this.
struct Shared {
    state: Mutex<StreamState>,
    /// Signals a drawn frame to [`FrameStreamer::draw_now`].
    drawn: Condvar,
}

struct StreamState {
    /// The size the stream last drew at, for a capture that states none.
    output_size: Option<(u32, u32)>,
    stats: VecDeque<FrameCost>,
    stop: bool,
    /// Draw only on request, not on change.
    on_demand: bool,
    /// Frame requests made, and the latest one a drawn frame answered.
    requested: u64,
    answered: u64,
    last_drawn: Option<DrawnFrame>,
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
    /// Changes with each renderer the driver builds.
    pub renderer_id: u64,
    /// The retained world revision the frame shows.
    pub world_revision: u64,
    pub composition: Option<Arc<RendererViewComposition>>,
    pub composition_revision: u64,
    pub observer: Option<RendererCameraPose>,
    /// How each of `composition`'s cameras drew this frame: its sampled
    /// pose, or the observer's where the observer replaced it in primary
    /// views (`Renderer::drawn_cameras`).
    pub cameras: Vec<DrawnCamera>,
    /// A playing video clip covered the frame.
    pub video: bool,
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
    published_at: SystemTime,
    step: u64,
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
    /// When each recent frame was published, and the step it showed.
    pub shown: Vec<(SystemTime, u64)>,
    pub skipped_ops: BTreeMap<&'static str, u64>,
    pub last_skip: Option<String>,
}

impl FrameStreamer {
    /// Starts the render thread over `driver`. Frames go to `frames`
    /// whenever a viewer is attached there.
    pub fn start(
        driver: Arc<SceneDriver>,
        format: StreamFormat,
        frames: Arc<ProductDevFrameStream>,
    ) -> Result<Self, String> {
        let waker = Arc::downgrade(&driver);
        frames.set_demand_waker(move || {
            if let Some(driver) = waker.upgrade() {
                driver.mark_changed();
            }
        });
        let shared = Arc::new(Shared {
            state: Mutex::new(StreamState {
                output_size: None,
                stats: VecDeque::with_capacity(STATS_WINDOW),
                stop: false,
                on_demand: false,
                requested: 0,
                answered: 0,
                last_drawn: None,
            }),
            drawn: Condvar::new(),
        });
        let thread_driver = Arc::clone(&driver);
        let thread_shared = Arc::clone(&shared);
        let thread = std::thread::Builder::new()
            .name("rusty-render-stream".to_owned())
            .spawn(move || render_loop(&thread_driver, &thread_shared, &frames, format))
            .map_err(|error| format!("could not start the render thread: {error}"))?;
        Ok(Self {
            driver,
            shared,
            thread: Some(thread),
        })
    }

    /// On demand, changes wait for [`Self::draw_now`] instead of drawing.
    pub fn set_on_demand(&self, on_demand: bool) {
        self.shared.state().on_demand = on_demand;
        self.driver.notify();
    }

    /// Draws a frame of the current scene, even on demand or with no viewer,
    /// and waits up to `timeout` for it to be published.
    pub fn draw_now(&self, timeout: Duration) -> Option<DrawnFrame> {
        let request = {
            let mut state = self.shared.state();
            state.requested += 1;
            state.requested
        };
        self.driver.notify();
        let (state, _) = self
            .shared
            .drawn
            .wait_timeout_while(self.shared.state(), timeout, |state| {
                state.answered < request && !state.stop
            })
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        (state.answered >= request)
            .then(|| state.last_drawn.clone())
            .flatten()
    }

    /// A tool's capture ([`SceneDriver::capture`]), at `size` or else the
    /// stream's last frame size.
    pub fn capture(&self, size: Option<(u32, u32)>) -> Capture {
        let output_size = self.shared.state().output_size;
        self.driver.capture(size.or(output_size))
    }

    pub fn inspection(&self) -> StreamInspection {
        let scene = self.driver.view_state();
        let state = self.shared.state();
        StreamInspection {
            on_demand: state.on_demand,
            held: scene.held,
            observer: scene.observer,
            composition: scene.composition,
            pending: scene.changed || state.requested > state.answered,
            last_drawn: state.last_drawn.clone(),
        }
    }

    pub fn stats(&self) -> StreamStats {
        let (skipped_ops, last_skip) = self.driver.skipped_ops();
        let state = self.shared.state();
        let costs = &state.stats;
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
        let adapter = self.driver.gpu().adapter_summary();
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
            shown: costs
                .iter()
                .map(|cost| (cost.published_at, cost.step))
                .collect(),
            skipped_ops,
            last_skip,
        }
    }
}

impl Drop for FrameStreamer {
    fn drop(&mut self) {
        self.shared.state().stop = true;
        self.driver.notify();
        self.shared.drawn.notify_all();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Shared {
    fn state(&self) -> MutexGuard<'_, StreamState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// What the render thread does next.
enum Next {
    Stop,
    /// Draw at this size, answering requests up to this one.
    Draw((u32, u32), u64),
}

fn render_loop(
    driver: &SceneDriver,
    shared: &Shared,
    frames: &ProductDevFrameStream,
    format: StreamFormat,
) {
    let mut target: Option<OffscreenTarget> = None;
    let mut pixels = Vec::new();
    loop {
        // A running simulation applies a change every step, so frames follow
        // the simulation rate; a held scene draws once per change. On
        // demand, only requests draw.
        let next = driver.wait_for_change(IDLE_WAIT, |changed| {
            let state = shared.state();
            if state.stop {
                return Some(Next::Stop);
            }
            let wanted = frames.wanted_size();
            if state.requested > state.answered {
                return Some(Next::Draw(
                    wanted.unwrap_or(UNWATCHED_SIZE),
                    state.requested,
                ));
            }
            wanted
                .filter(|_| changed && !state.on_demand)
                .map(|size| Next::Draw(size, state.requested))
        });
        let (size, request) = match next {
            None => continue,
            Some(Next::Stop) => return,
            Some(Next::Draw(size, request)) => (size, request),
        };
        let target = match &mut target {
            Some(target) => {
                target.resize(driver.gpu(), size.0, size.1);
                target
            }
            None => target.insert(OffscreenTarget::new(driver.gpu(), size.0, size.1)),
        };
        let started = Instant::now();
        let pixel_ratio = frames.wanted_pixel_ratio();
        let (video, shown) = driver.draw(|renderer, now| {
            renderer.set_pixel_ratio(pixel_ratio);
            renderer.render_view_composition(target, now).video
        });
        let rendered = Instant::now();
        target.read_rgba_into(driver.gpu(), &mut pixels);
        let read = Instant::now();
        let (width, height) = target.size();
        let (format, payload) = match format {
            StreamFormat::Jpeg => (
                ProductDevFrameFormat::Jpeg,
                encode_jpeg(&pixels, width, height),
            ),
            StreamFormat::Rgba8 => (ProductDevFrameFormat::Rgba8, pixels.clone()),
        };
        let encoded = Instant::now();
        let mut cost = FrameCost {
            at: started,
            published_at: SystemTime::UNIX_EPOCH,
            step: shown.step,
            render_ms: ms(rendered - started),
            readback_ms: ms(read - rendered),
            encode_ms: ms(encoded - read),
            bytes: payload.len(),
        };
        let sequence = frames.publish(ProductDevFrame {
            width,
            height,
            format,
            held: shown.held,
            video,
            step: shown.step,
            payload,
        });
        cost.published_at = SystemTime::now();
        let drawn = DrawnFrame {
            sequence,
            step: shown.step,
            held: shown.held,
            width,
            height,
            drawn_at: started,
            renderer_id: shown.renderer_id,
            world_revision: shown.world_revision,
            composition: shown.composition,
            composition_revision: shown.composition_revision,
            observer: shown.observer,
            cameras: shown.cameras,
            video,
        };
        let mut state = shared.state();
        state.output_size = Some(size);
        if state.stats.len() == STATS_WINDOW {
            state.stats.pop_front();
        }
        state.stats.push_back(cost);
        state.answered = state.answered.max(request);
        state.last_drawn = Some(drawn);
        drop(state);
        shared.drawn.notify_all();
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
