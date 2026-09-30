//! The render thread against a real headless device: frames reach an
//! attached viewer, follow applied steps, and stop once a held scene has
//! been drawn, and an unwatched scene still reports clip ends. One test, so
//! the binary opens one device (parallel devices can crash the Vulkan
//! loader).

use std::borrow::Cow;
use std::time::{Duration, Instant};

use product_dev_host::ProductDevFrameStream;
use render_host_contracts::{
    RendererCameraProjection, RendererCompositionCamera, RendererCompositionView,
    RendererViewTarget, RendererViewport,
};
use render_presentation::{video_frame, VideoClipRef, VideoPlaybackHandle, VideoProjectionOp};
use render_stream::{
    FrameStreamer, Gpu, RendererCameraPose, RendererOptions, RendererViewComposition,
    ResourceSource, SceneDriver, SceneState, StreamFormat, VideoFact,
};
use runtime_publication::RuntimePublication;

fn state(step: u64, held: bool) -> SceneState {
    SceneState {
        elapsed_seconds: step as f64 / 60.0,
        world_revision: 0,
        step,
        held,
    }
}

struct NoResources;

impl ResourceSource for NoResources {
    fn bytes(&self, _identity: &str) -> Option<Cow<'_, [u8]>> {
        None
    }
}

/// `render-video`'s synthetic fixture: 1.5 s of VP9 at 10 fps.
const CLIP: &[u8] = include_bytes!("../../render-video/tests/fixtures/testsrc.webm");
const CLIP_RESOURCE: &str = "content/video/testsrc.webm";

struct Clip;

impl ResourceSource for Clip {
    fn bytes(&self, identity: &str) -> Option<Cow<'_, [u8]>> {
        (identity == CLIP_RESOURCE).then_some(Cow::Borrowed(CLIP))
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
    let gpu = Gpu::headless().expect("a headless adapter");
    let scene = SceneDriver::new(gpu.clone(), RendererOptions::default());
    let streamer = FrameStreamer::start(scene.clone(), StreamFormat::Rgba8, frames.clone())
        .expect("the render thread starts");
    let wait = Duration::from_secs(5);
    // A new viewer gets the held scene: with no composition, the clear colour.
    let first = frames.next_after(0, Some((64, 32)), wait).unwrap();
    let head = header(&first);
    assert_eq!((head.width, head.height, head.format), (64, 32, 2));
    assert!(head.held);
    let pixel = &first[40..44];
    assert_eq!(pixel, &[16, 24, 32, 255], "the default clear colour");

    // Running: each applied step draws a frame that shows that step.
    let mut running = Vec::new();
    for step in 1..=10 {
        scene.apply(&[], &NoResources, &|_| None, state(step, false));
        let after = running
            .last()
            .map_or(head.sequence, |last: &Header| last.sequence);
        if let Some(frame) = frames.next_after(after, None, wait) {
            let shown = header(&frame);
            assert_eq!(
                shown.step, step,
                "a frame shows the step it was applied with"
            );
            running.push(shown);
        }
    }
    assert!(
        running.len() >= 5,
        "only {} frames while running",
        running.len()
    );
    assert!(!running.last().unwrap().held);
    let last = running.last().unwrap();

    // Held again: one frame shows the change, then nothing until the next one.
    // A lifecycle change no call published (pause) draws once.
    scene.set_simulation(true, 8);
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

    // A held call's step lands with its changes: one frame, at the new
    // step, and no second frame for the same change.
    scene.apply(&[], &NoResources, &|_| None, state(9, true));
    let stepped = header(&frames.next_after(held.sequence, None, wait).unwrap());
    assert_eq!((stepped.step, stepped.held), (9, true));
    assert!(frames
        .next_after(stepped.sequence, None, Duration::from_millis(300))
        .is_none());
    let held = stepped;

    // On demand: a change waits for a request, which draws exactly one frame.
    streamer.set_on_demand(true);
    scene.apply(&[], &NoResources, &|_| None, state(10, true));
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

    // Each frame records the camera it drew from: the product's camera,
    // then the observer that replaced it in the primary view (#8841).
    let product = RendererCompositionCamera {
        id: "main".to_owned(),
        pose: RendererCameraPose {
            position: [0.0, 1.0, 4.0],
            pitch_degrees: -10.0,
            yaw_degrees: 0.0,
        },
        basis: None,
        projection: RendererCameraProjection::Perspective {
            fov_y_degrees: 60.0,
            near: 0.1,
            far: 100.0,
        },
        motion: None,
    };
    let composition = RendererViewComposition {
        cameras: vec![product],
        targets: Vec::new(),
        views: vec![RendererCompositionView {
            id: "main".to_owned(),
            camera_id: "main".to_owned(),
            target: RendererViewTarget::Primary,
            viewport: RendererViewport {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            },
            order: 0,
        }],
        presentations: Vec::new(),
    };
    scene.apply(
        &[RuntimePublication::ViewComposition(composition)],
        &NoResources,
        &|_| None,
        state(11, true),
    );
    let own = streamer.draw_now(wait).expect("a requested frame");
    let near = |actual: [f64; 3], expected: [f64; 3]| {
        actual
            .iter()
            .zip(expected)
            .all(|(actual, expected)| (actual - expected).abs() < 1e-3)
    };
    assert_eq!(own.cameras.len(), 1);
    assert!(!own.cameras[0].observer);
    assert!(near(own.cameras[0].pose.position, [0.0, 1.0, 4.0]));
    assert!((own.cameras[0].pose.pitch_degrees + 10.0).abs() < 1e-3);
    let observer = RendererCameraPose {
        position: [3.0, 2.0, 1.0],
        pitch_degrees: -20.0,
        yaw_degrees: 45.0,
    };
    scene.set_observer(Some(observer));
    let observed = streamer.draw_now(wait).expect("a requested frame");
    assert!(observed.cameras[0].observer);
    assert!(near(observed.cameras[0].pose.position, observer.position));
    assert!((observed.cameras[0].pose.yaw_degrees - 45.0).abs() < 1e-3);
    assert!(observed.cameras[0].offscreen.is_none(), "no offscreen view");

    // A tool's capture draws at its own size and publishes nothing to the
    // stream; without a size it takes the stream's last frame size.
    let latest = streamer.inspection().last_drawn.unwrap().sequence;
    let capture = scene.capture(Some((40, 20)));
    assert_eq!(
        (capture.width, capture.height, capture.rgba.len()),
        (40, 20, 40 * 20 * 4)
    );
    assert_eq!(
        (capture.step, capture.held, capture.sequence),
        (11, true, 1)
    );
    assert!(
        capture.cameras[0].observer,
        "it draws what the stream draws"
    );
    let default = scene.capture(None);
    assert_eq!(
        (default.width, default.height),
        (observed.width, observed.height)
    );
    assert_eq!(default.sequence, 2);
    assert!(
        frames
            .next_after(latest, None, Duration::from_millis(300))
            .is_none(),
        "a capture publishes no frame"
    );
    drop(streamer);

    // Unwatched, nothing draws, but a clip still ends on Engine time and its
    // completion reaches the Engine (#8871).
    let unwatched_frames = ProductDevFrameStream::new();
    let unwatched = SceneDriver::new(gpu, RendererOptions::default());
    let unwatched_streamer = FrameStreamer::start(
        unwatched.clone(),
        StreamFormat::Rgba8,
        unwatched_frames.clone(),
    )
    .expect("the render thread starts");
    let handle = VideoPlaybackHandle::new(7);
    let play = video_frame([VideoProjectionOp::Play {
        handle,
        clip: VideoClipRef {
            asset: CLIP_RESOURCE.to_owned(),
            content_hash: "sha256:fixture".to_owned(),
            media_type: "video/webm".to_owned(),
        },
    }]);
    unwatched.apply(
        &[RuntimePublication::Presentation(play)],
        &Clip,
        &|_| None,
        state(1, false),
    );
    assert!(unwatched.take_video_facts().is_empty());
    for step in 2..=120 {
        unwatched.apply(&[], &Clip, &|_| None, state(step, false));
    }
    let deadline = Instant::now() + wait;
    let facts = loop {
        let facts = unwatched.take_video_facts();
        if !facts.is_empty() || Instant::now() > deadline {
            break facts;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(facts, [VideoFact::Completed { handle }]);
    assert!(
        unwatched_streamer.inspection().last_drawn.is_none(),
        "no frame was drawn"
    );
    drop(unwatched_streamer);
}
