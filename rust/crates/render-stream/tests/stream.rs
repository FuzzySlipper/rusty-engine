//! The render thread against a real headless device: frames reach an
//! attached viewer, follow applied steps, and stop once a held scene has
//! been drawn. One test, so the binary opens one
//! device (parallel devices can crash the Vulkan loader).

use std::borrow::Cow;
use std::time::Duration;

use product_dev_host::ProductDevFrameStream;
use render_stream::{FrameStreamer, RendererOptions, ResourceSource, StreamFormat};

struct NoResources;

impl ResourceSource for NoResources {
    fn bytes(&self, _identity: &str) -> Option<Cow<'_, [u8]>> {
        None
    }
}

struct Header {
    sequence: u64,
    step: u64,
    width: u32,
    height: u32,
    format: u8,
    held: bool,
}

fn header(frame: &[u8]) -> Header {
    assert_eq!(&frame[..4], b"RSF1");
    let u32_at = |at: usize| u32::from_le_bytes(frame[at..at + 4].try_into().unwrap());
    let u64_at = |at: usize| u64::from_le_bytes(frame[at..at + 8].try_into().unwrap());
    assert_eq!(u32_at(36) as usize, frame.len() - u32_at(4) as usize);
    Header {
        sequence: u64_at(8),
        step: u64_at(16),
        width: u32_at(24),
        height: u32_at(28),
        format: frame[32],
        held: frame[33] & 1 == 1,
    }
}

#[test]
fn frames_follow_viewers_and_simulation_time() {
    let frames = ProductDevFrameStream::new();
    let streamer = FrameStreamer::start(
        RendererOptions::default(),
        StreamFormat::Rgba8,
        frames.clone(),
    )
    .expect("a headless adapter");
    let wait = Duration::from_secs(5);
    // A new viewer gets the held scene: with no composition, the clear colour.
    let first = frames.next_after(0, Some((64, 32)), wait).unwrap();
    let head = header(&first);
    assert_eq!((head.width, head.height, head.format), (64, 32, 2));
    assert!(head.held);
    let pixel = &first[40..44];
    assert_eq!(pixel, &[16, 24, 32, 255], "the default clear colour");

    // Running: each applied step draws a frame.
    streamer.set_simulation(false, 7);
    let mut running = Vec::new();
    for step in 1..=10 {
        streamer.apply(&[], &NoResources, &|_| None, f64::from(step) / 60.0, 0);
        let after = running.last().map_or(head.sequence, |last: &Header| last.sequence);
        if let Some(frame) = frames.next_after(after, None, wait) {
            running.push(header(&frame));
        }
    }
    assert!(running.len() >= 5, "only {} frames while running", running.len());
    let last = running.last().unwrap();
    assert!(!last.held && last.step == 7);
    assert!(running.windows(2).all(|pair| pair[0].sequence < pair[1].sequence));

    // Held again: one frame shows the change, then nothing until the next one.
    streamer.set_simulation(true, 8);
    let mut after = last.sequence;
    let held = loop {
        let head = header(&frames.next_after(after, None, wait).unwrap());
        after = head.sequence;
        if head.held {
            break head;
        }
    };
    assert_eq!(held.step, 8);
    assert!(frames
        .next_after(after, None, Duration::from_millis(400))
        .is_none());

    // On demand: a change waits for a request, which draws exactly one frame.
    streamer.set_on_demand(true);
    streamer.apply(&[], &NoResources, &|_| None, 1.0, 0);
    assert!(frames
        .next_after(held.sequence, None, Duration::from_millis(300))
        .is_none());
    assert!(streamer.inspection().pending);
    let drawn = streamer.draw_now(wait).expect("a requested frame");
    assert!(drawn.sequence > held.sequence && drawn.held);
    let shown = header(&frames.next_after(held.sequence, None, wait).unwrap());
    assert_eq!(shown.sequence, drawn.sequence);
    let inspection = streamer.inspection();
    assert!(inspection.on_demand && !inspection.pending);
    assert_eq!(inspection.last_drawn.unwrap().sequence, drawn.sequence);
    drop(streamer);
}
