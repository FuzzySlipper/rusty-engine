//! The rendered-frame route: `GET /__rusty/product/runtime/frames`.
//!
//! When the runtime renders in process, a viewer pulls the frames it draws
//! one request at a time: `?after=<sequence>` answers with the latest frame
//! newer than `sequence`, waiting for one when there is none yet, and `204 No
//! Content` when none arrives in time. There is no history and no replay; a
//! viewer that starts with `after=0` gets the latest frame. Because a viewer
//! asks for the next frame only when it can take one, a slow viewer skips
//! frames instead of queueing them in socket buffers.
//!
//! `width` and `height` state the size the viewer shows, in its pixels. The
//! renderer draws at the most recent viewer's size; other viewers scale what
//! they receive. `cssWidth` states the same width in CSS pixels, so
//! `width / cssWidth` is the viewer's device pixel ratio: CSS-pixel
//! presentation (labels, pixel-sized sprites) keeps its size at that ratio.
//!
//! A frame is a fixed little-endian header followed by its payload:
//!
//! | offset | size | field |
//! |---|---|---|
//! | 0 | 4 | magic `RSF1` |
//! | 4 | 4 | header length in bytes (40) |
//! | 8 | 8 | sequence, from 1, increasing per published frame |
//! | 16 | 8 | Engine simulation step the frame shows |
//! | 24 | 4 | width in pixels |
//! | 28 | 4 | height in pixels |
//! | 32 | 1 | payload format: 1 JPEG, 2 RGBA8 sRGB rows top first |
//! | 33 | 1 | flags: bit 0 set while the simulation is held; bit 1 set while a video clip covers the frame |
//! | 34 | 2 | reserved, zero |
//! | 36 | 4 | payload length in bytes |
//!
//! A reader skips header bytes past the fields it knows, so later fields
//! extend the header without a new magic.

use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

pub const PRODUCT_DEV_FRAMES_PATH: &str = "/__rusty/product/runtime/frames";
/// A tool's capture: `?format=png|rgba[&width=W&height=H]`. It draws one
/// frame at its own size and never counts as a viewer.
pub const PRODUCT_DEV_FRAME_CAPTURE_PATH: &str = "/__rusty/product/runtime/frames/capture";
/// How long one request waits for a newer frame before answering 204.
pub const FRAME_REQUEST_WAIT: Duration = Duration::from_secs(1);
pub(crate) const FRAME_MAGIC: &[u8; 4] = b"RSF1";
/// Byte offsets of the little-endian header fields, and the header length.
/// The browser's frame view reads the same offsets, emitted with the wire
/// contracts.
pub(crate) mod header {
    pub const MAGIC: usize = 0;
    /// u32: the header length, so later fields can extend it.
    pub const HEADER_BYTES: usize = 4;
    /// u64
    pub const SEQUENCE: usize = 8;
    /// u64
    pub const STEP: usize = 16;
    /// u32
    pub const WIDTH: usize = 24;
    /// u32
    pub const HEIGHT: usize = 28;
    /// u8: a [`super::ProductDevFrameFormat`].
    pub const FORMAT: usize = 32;
    /// u8: [`super::FLAG_HELD`] and [`super::FLAG_VIDEO`].
    pub const FLAGS: usize = 33;
    /// u32
    pub const PAYLOAD_BYTES: usize = 36;
    pub const LEN: usize = 40;
}
pub(crate) const FLAG_HELD: u8 = 1;
/// The page shows such a frame above its UI, as a browser's video element
/// covered the page.
pub(crate) const FLAG_VIDEO: u8 = 2;
/// Largest size a viewer may ask for, per side.
const MAX_FRAME_SIDE: u32 = 4096;
/// Size rendered before any viewer states one.
const DEFAULT_FRAME_SIZE: (u32, u32) = (1280, 720);
/// A viewer is still watching this long after its last request ended.
const VIEWER_GRACE: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductDevFrameFormat {
    Jpeg = 1,
    Rgba8 = 2,
    /// Lossless; captures only.
    Png = 3,
}

/// One rendered frame, before the stream numbers it.
pub struct ProductDevFrame {
    pub width: u32,
    pub height: u32,
    pub format: ProductDevFrameFormat,
    pub held: bool,
    /// A playing video clip covers the frame.
    pub video: bool,
    pub step: u64,
    pub payload: Vec<u8>,
}

/// The latest rendered frame and the viewers asking for newer ones. The
/// runtime's renderer publishes into it; the host's frame route reads it.
#[derive(Default)]
pub struct ProductDevFrameStream {
    state: Mutex<FrameState>,
    changed: Condvar,
    demand_waker: Mutex<Option<Box<dyn Fn() + Send + Sync>>>,
}

#[derive(Default)]
struct FrameState {
    sequence: u64,
    latest: Option<Arc<[u8]>>,
    size: Option<(u32, u32)>,
    pixel_ratio: Option<f32>,
    waiting: usize,
    last_request: Option<Instant>,
}

impl FrameState {
    fn watched(&self) -> bool {
        self.waiting > 0
            || self
                .last_request
                .is_some_and(|last| last.elapsed() < VIEWER_GRACE)
    }
}

impl ProductDevFrameStream {
    pub fn new() -> Arc<Self> {
        Arc::default()
    }

    /// Called when viewers start watching or ask for a new size, so the
    /// renderer need not poll for demand.
    pub fn set_demand_waker(&self, waker: impl Fn() + Send + Sync + 'static) {
        *self.demand_waker.lock().expect("frame waker lock") = Some(Box::new(waker));
    }

    /// The size to render, or `None` while nobody watches.
    pub fn wanted_size(&self) -> Option<(u32, u32)> {
        let state = self.state();
        state
            .watched()
            .then(|| state.size.unwrap_or(DEFAULT_FRAME_SIZE))
    }

    /// The most recent viewer's device pixel ratio; 1 until one states it.
    pub fn wanted_pixel_ratio(&self) -> f32 {
        self.state().pixel_ratio.unwrap_or(1.0)
    }

    /// Records the device pixel ratio a viewer's request stated.
    pub fn set_viewer_pixel_ratio(&self, ratio: f32) {
        let mut state = self.state();
        if state.pixel_ratio == Some(ratio) {
            return;
        }
        state.pixel_ratio = Some(ratio);
        drop(state);
        if let Some(waker) = &*self.demand_waker.lock().expect("frame waker lock") {
            waker();
        }
    }

    /// Numbers `frame`, makes it the latest, and returns its sequence.
    pub fn publish(&self, frame: ProductDevFrame) -> u64 {
        let mut state = self.state();
        state.sequence += 1;
        let sequence = state.sequence;
        state.latest = Some(encode_frame(sequence, &frame));
        drop(state);
        self.changed.notify_all();
        sequence
    }

    /// The latest encoded frame (header and payload) newer than `after`,
    /// waiting up to `timeout` for one. `size` is the size this viewer shows.
    pub fn next_after(
        &self,
        after: u64,
        size: Option<(u32, u32)>,
        timeout: Duration,
    ) -> Option<Arc<[u8]>> {
        let mut state = self.state();
        let demand_changed = !state.watched() || size.is_some_and(|size| state.size != Some(size));
        if size.is_some() {
            state.size = size;
        }
        state.waiting += 1;
        if demand_changed {
            drop(state);
            if let Some(waker) = &*self.demand_waker.lock().expect("frame waker lock") {
                waker();
            }
            state = self.state();
        }
        let (mut state, _) = self
            .changed
            .wait_timeout_while(state, timeout, |state| state.sequence <= after)
            .expect("frame stream lock");
        state.waiting -= 1;
        state.last_request = Some(Instant::now());
        (state.sequence > after)
            .then(|| state.latest.clone())
            .flatten()
    }

    fn state(&self) -> MutexGuard<'_, FrameState> {
        self.state.lock().expect("frame stream lock")
    }
}

fn encode_frame(sequence: u64, frame: &ProductDevFrame) -> Arc<[u8]> {
    let mut bytes = vec![0; header::LEN + frame.payload.len()];
    let mut put = |offset: usize, field: &[u8]| {
        bytes[offset..offset + field.len()].copy_from_slice(field);
    };
    put(header::MAGIC, FRAME_MAGIC);
    put(header::HEADER_BYTES, &(header::LEN as u32).to_le_bytes());
    put(header::SEQUENCE, &sequence.to_le_bytes());
    put(header::STEP, &frame.step.to_le_bytes());
    put(header::WIDTH, &frame.width.to_le_bytes());
    put(header::HEIGHT, &frame.height.to_le_bytes());
    put(header::FORMAT, &[frame.format as u8]);
    let flags = if frame.held { FLAG_HELD } else { 0 } | if frame.video { FLAG_VIDEO } else { 0 };
    put(header::FLAGS, &[flags]);
    put(
        header::PAYLOAD_BYTES,
        &(frame.payload.len() as u32).to_le_bytes(),
    );
    put(header::LEN, &frame.payload);
    bytes.into()
}

/// A tool's capture request: the frame's size, or the output's when `None`,
/// and its payload format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProductDevCaptureRequest {
    pub size: Option<(u32, u32)>,
    /// [`ProductDevFrameFormat::Png`] or [`ProductDevFrameFormat::Rgba8`].
    pub format: ProductDevFrameFormat,
}

/// One captured frame, numbered by capture rather than by the stream, with
/// the drawn cameras as the inspection commands report them.
pub struct ProductDevCapture {
    pub sequence: u64,
    pub frame: ProductDevFrame,
    pub cameras: serde_json::Value,
}

/// Draws a capture; the runtime supplies it for either render output.
pub type ProductDevFrameCapture =
    Arc<dyn Fn(ProductDevCaptureRequest) -> Result<ProductDevCapture, String> + Send + Sync>;

/// The capture's header and payload, in the streamed frame's layout.
pub(crate) fn encode_capture(capture: &ProductDevCapture) -> Arc<[u8]> {
    encode_frame(capture.sequence, &capture.frame)
}

pub(crate) fn capture_request(path: &str) -> Result<ProductDevCaptureRequest, &'static str> {
    let query = match path.split_once('?') {
        Some((PRODUCT_DEV_FRAME_CAPTURE_PATH, query)) => query,
        None if path == PRODUCT_DEV_FRAME_CAPTURE_PATH => "",
        _ => return Err("unknown capture route"),
    };
    let (mut format, mut width, mut height) = (None, None, None);
    for pair in query.split('&').filter(|pair| !pair.is_empty()) {
        let (name, value) = pair.split_once('=').ok_or("malformed capture query")?;
        let repeated = match name {
            "format" => format
                .replace(match value {
                    "png" => ProductDevFrameFormat::Png,
                    "rgba" => ProductDevFrameFormat::Rgba8,
                    _ => return Err("capture format is png or rgba"),
                })
                .is_some(),
            "width" | "height" => {
                let side = value
                    .parse::<u32>()
                    .ok()
                    .filter(|value| (1..=MAX_FRAME_SIDE).contains(value))
                    .ok_or("frame size must be 1..=4096 pixels per side")?;
                let slot = if name == "width" {
                    &mut width
                } else {
                    &mut height
                };
                slot.replace(side).is_some()
            }
            _ => return Err("unknown capture query parameter"),
        };
        if repeated {
            return Err("repeated capture query parameter");
        }
    }
    let size = match (width, height) {
        (Some(width), Some(height)) => Some((width, height)),
        (None, None) => None,
        _ => return Err("capture query needs both width and height"),
    };
    Ok(ProductDevCaptureRequest {
        size,
        format: format.unwrap_or(ProductDevFrameFormat::Png),
    })
}

/// A viewer's frame request: `?after=N[&width=W&height=H[&cssWidth=C]]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct FrameRequest {
    pub after: u64,
    pub size: Option<(u32, u32)>,
    /// `width / cssWidth`, when the viewer stated its CSS width.
    pub pixel_ratio: Option<f32>,
}

pub(crate) fn frame_request(path: &str) -> Result<FrameRequest, &'static str> {
    let query = match path.split_once('?') {
        Some((PRODUCT_DEV_FRAMES_PATH, query)) => query,
        None if path == PRODUCT_DEV_FRAMES_PATH => "",
        _ => return Err("unknown frame route"),
    };
    let (mut after, mut width, mut height, mut css_width) = (None, None, None, None);
    for pair in query.split('&').filter(|pair| !pair.is_empty()) {
        let (name, value) = pair.split_once('=').ok_or("malformed frame query")?;
        let value = value
            .parse::<u64>()
            .map_err(|_| "frame query values are integers")?;
        let slot = match name {
            "after" => &mut after,
            "width" => &mut width,
            "height" => &mut height,
            "cssWidth" => &mut css_width,
            _ => return Err("unknown frame query parameter"),
        };
        if slot.replace(value).is_some() {
            return Err("repeated frame query parameter");
        }
    }
    let side = |value: u64| {
        u32::try_from(value)
            .ok()
            .filter(|value| (1..=MAX_FRAME_SIDE).contains(value))
            .ok_or("frame size must be 1..=4096 pixels per side")
    };
    let size = match (width, height) {
        (Some(width), Some(height)) => Some((side(width)?, side(height)?)),
        (None, None) => None,
        _ => return Err("frame query needs both width and height"),
    };
    let pixel_ratio = match (size, css_width) {
        (Some((width, _)), Some(css)) => Some(width as f32 / side(css)? as f32),
        (None, Some(_)) => return Err("cssWidth needs width and height"),
        (_, None) => None,
    };
    Ok(FrameRequest {
        after: after.unwrap_or(0),
        size,
        pixel_ratio,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_capture_request_states_a_whole_size_and_png_or_rgba() {
        let parse =
            |query: &str| capture_request(&format!("{PRODUCT_DEV_FRAME_CAPTURE_PATH}{query}"));
        assert_eq!(
            parse(""),
            Ok(ProductDevCaptureRequest {
                size: None,
                format: ProductDevFrameFormat::Png,
            })
        );
        assert_eq!(
            parse("?format=rgba&width=640&height=360"),
            Ok(ProductDevCaptureRequest {
                size: Some((640, 360)),
                format: ProductDevFrameFormat::Rgba8,
            })
        );
        assert!(parse("?format=jpeg").is_err());
        assert!(parse("?width=640").is_err());
        assert!(parse("?width=0&height=1").is_err());
        assert!(parse("?width=5000&height=1").is_err());
        assert!(parse("?format=png&format=png").is_err());
        assert!(parse("?after=3").is_err());
    }

    fn frame(step: u64) -> ProductDevFrame {
        ProductDevFrame {
            width: 2,
            height: 1,
            format: ProductDevFrameFormat::Rgba8,
            held: step.is_multiple_of(2),
            video: step.is_multiple_of(3),
            step,
            payload: vec![step as u8; 8],
        }
    }

    fn sequence(frame: &[u8]) -> u64 {
        u64::from_le_bytes(frame[8..16].try_into().unwrap())
    }

    #[test]
    fn a_frame_is_its_header_then_its_payload() {
        let bytes = encode_frame(7, &frame(4));
        assert_eq!(&bytes[..4], b"RSF1");
        assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), 40);
        assert_eq!(sequence(&bytes), 7);
        assert_eq!(u64::from_le_bytes(bytes[16..24].try_into().unwrap()), 4);
        assert_eq!(u32::from_le_bytes(bytes[24..28].try_into().unwrap()), 2);
        assert_eq!(u32::from_le_bytes(bytes[28..32].try_into().unwrap()), 1);
        assert_eq!(bytes[32], 2);
        assert_eq!(bytes[33], FLAG_HELD);
        assert_eq!(encode_frame(8, &frame(3))[33], FLAG_VIDEO);
        assert_eq!(u32::from_le_bytes(bytes[36..40].try_into().unwrap()), 8);
        assert_eq!(&bytes[40..], &[4; 8]);
    }

    #[test]
    fn a_viewer_gets_the_latest_frame_then_only_newer_ones() {
        let stream = ProductDevFrameStream::new();
        assert_eq!(stream.wanted_size(), None);
        stream.publish(frame(1));
        stream.publish(frame(2));
        let first = stream
            .next_after(0, Some((64, 32)), Duration::ZERO)
            .unwrap();
        assert_eq!(sequence(&first), 2);
        assert_eq!(stream.wanted_size(), Some((64, 32)));
        assert!(stream.next_after(2, None, Duration::ZERO).is_none());
        stream.publish(frame(3));
        stream.publish(frame(4));
        let skipped = stream.next_after(2, None, Duration::ZERO).unwrap();
        assert_eq!(sequence(&skipped), 4);
    }

    #[test]
    fn a_waiting_request_takes_the_next_frame_and_wakes_the_renderer() {
        let stream = ProductDevFrameStream::new();
        let woken = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter = Arc::clone(&woken);
        let publisher = Arc::clone(&stream);
        stream.set_demand_waker(move || {
            counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            publisher.publish(frame(9));
        });
        let next = stream.next_after(0, None, Duration::from_secs(5)).unwrap();
        assert_eq!(sequence(&next), 1);
        assert_eq!(stream.wanted_size(), Some(DEFAULT_FRAME_SIZE));
        // Watching already, at the same size: no second wake.
        assert!(stream.next_after(1, None, Duration::ZERO).is_none());
        assert_eq!(woken.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn frame_query_takes_after_and_both_sides_or_neither() {
        assert_eq!(
            frame_request(PRODUCT_DEV_FRAMES_PATH),
            Ok(FrameRequest {
                after: 0,
                size: None,
                pixel_ratio: None,
            })
        );
        assert_eq!(
            frame_request("/__rusty/product/runtime/frames?after=12&width=1280&height=720"),
            Ok(FrameRequest {
                after: 12,
                size: Some((1280, 720)),
                pixel_ratio: None,
            })
        );
        assert_eq!(
            frame_request(
                "/__rusty/product/runtime/frames?after=3&width=2560&height=1440&cssWidth=1280"
            ),
            Ok(FrameRequest {
                after: 3,
                size: Some((2560, 1440)),
                pixel_ratio: Some(2.0),
            })
        );
        for bad in [
            "/__rusty/product/runtime/frames?cssWidth=1280",
            "/__rusty/product/runtime/frames?width=1280&height=720&cssWidth=0",
            "/__rusty/product/runtime/frames?width=1280",
            "/__rusty/product/runtime/frames?width=0&height=720",
            "/__rusty/product/runtime/frames?width=5000&height=720",
            "/__rusty/product/runtime/frames?after=1&after=2",
            "/__rusty/product/runtime/frames?after=-1",
            "/__rusty/product/runtime/frames?depth=1",
        ] {
            assert!(frame_request(bad).is_err(), "{bad}");
        }
    }
}
