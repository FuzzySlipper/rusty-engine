//! The scene driver against a real headless device: applied changes land
//! with the step they reach, a drawn frame and a tool capture report what
//! they showed, and an undrawn scene still ends clips on Engine time. One
//! test, so the binary opens one device (parallel devices can crash the
//! Vulkan loader).

use std::borrow::Cow;
use std::time::{Duration, Instant};

use render_host_contracts::{
    RendererCameraPose, RendererCameraProjection, RendererCompositionCamera,
    RendererCompositionView, RendererViewComposition, RendererViewTarget, RendererViewport,
};
use render_presentation::{video_frame, VideoClipRef, VideoPlaybackHandle, VideoProjectionOp};
use render_wgpu::{
    Gpu, NoResources, OffscreenTarget, RendererOptions, ResourceSource, SceneChange, SceneDriver,
    SceneState, VideoFact,
};

/// `render-video`'s synthetic fixture: 1.5 s of VP9 at 10 fps.
const CLIP: &[u8] = include_bytes!("../../render-video/tests/fixtures/testsrc.webm");
const CLIP_RESOURCE: &str = "content/video/testsrc.webm";

struct Clip;

impl ResourceSource for Clip {
    fn bytes(&self, identity: &str) -> Option<Cow<'_, [u8]>> {
        (identity == CLIP_RESOURCE).then_some(Cow::Borrowed(CLIP))
    }
}

fn state(step: u64, held: bool) -> SceneState {
    SceneState {
        elapsed_seconds: step as f64 / 60.0,
        world_revision: step,
        step,
        held,
    }
}

fn composition() -> RendererViewComposition {
    RendererViewComposition {
        cameras: vec![RendererCompositionCamera {
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
        }],
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
            viewport_anchor: None,
        }],
        presentations: Vec::new(),
    }
}

#[test]
fn the_driver_reports_what_it_drew_and_ends_clips_undrawn() {
    let gpu = Gpu::headless().expect("a headless adapter");
    let scene = SceneDriver::new(gpu.clone(), RendererOptions::default());
    assert!(scene.view_state().changed, "a new scene is drawn once");

    // A call's changes land with the step and revision it reached.
    let composition = composition();
    scene.apply(
        [SceneChange::ViewComposition(&composition)],
        &NoResources,
        &|_| None,
        state(3, true),
    );
    let target = OffscreenTarget::new(&gpu, 64, 32, 4);
    let (video, shown) =
        scene.draw(|renderer, now| renderer.render_view_composition(&target, now).video);
    assert!(!video);
    assert_eq!((shown.step, shown.held, shown.world_revision), (3, true, 3));
    assert_eq!(shown.composition_revision, 1);
    assert_eq!(shown.cameras.len(), 1);
    assert!(!shown.cameras[0].observer);
    assert!(!scene.view_state().changed, "a draw consumes the change");

    // The observer replaces the product camera in the primary view.
    let observer = RendererCameraPose {
        position: [3.0, 2.0, 1.0],
        pitch_degrees: -20.0,
        yaw_degrees: 45.0,
    };
    scene.set_observer(Some(observer));
    assert!(scene.view_state().changed);
    let ((), shown) = scene.draw(|renderer, now| {
        renderer.render_view_composition(&target, now);
    });
    assert_eq!(shown.observer, Some(observer));
    assert!(shown.cameras[0].observer);

    // A tool's capture draws at its own size and counts its own sequence;
    // with no size and no window it draws at 1280x720.
    let capture = scene.capture(Some((40, 20)));
    assert_eq!(
        (capture.width, capture.height, capture.rgba.len()),
        (40, 20, 40 * 20 * 4)
    );
    assert_eq!((capture.step, capture.held, capture.sequence), (3, true, 1));
    assert!(capture.cameras[0].observer, "it draws what an output draws");
    let default = scene.capture(None);
    assert_eq!(
        (default.width, default.height, default.sequence),
        (1280, 720, 2)
    );

    // wait_for_change answers at once when ready, and wakes on a change.
    assert_eq!(
        scene.wait_for_change(Duration::from_secs(5), |_| Some(1)),
        Some(1)
    );
    let started = Instant::now();
    assert_eq!(
        scene.wait_for_change(Duration::from_millis(50), |changed| changed.then_some(())),
        None
    );
    assert!(started.elapsed() >= Duration::from_millis(50));
    scene.mark_changed();
    assert_eq!(
        scene.wait_for_change(Duration::from_secs(5), |changed| changed.then_some(())),
        Some(())
    );
    drop(scene);

    // Undrawn, a clip still ends on Engine time: waiting advances the scene
    // to the latest call, and the completion reaches the Engine (#8871).
    let unwatched = SceneDriver::new(gpu, RendererOptions::default());
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
        [SceneChange::Presentation(&play)],
        &Clip,
        &|_| None,
        state(1, false),
    );
    assert!(unwatched.take_video_facts().is_empty());
    for step in 2..=120 {
        unwatched.apply([], &Clip, &|_| None, state(step, false));
    }
    let never = |_| None::<()>;
    unwatched.wait_for_change(Duration::from_millis(1), never);
    assert_eq!(
        unwatched.take_video_facts(),
        [VideoFact::Completed { handle }]
    );
}
