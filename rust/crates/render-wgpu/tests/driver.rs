//! The scene driver against a real headless device: applied changes land
//! with the step they reach, a drawn frame and a tool capture report what
//! they showed, an undrawn scene still ends clips on Engine time, and a
//! joint-reporting instance reports with each applied call, and a running
//! world's frames draw on past its committed time between calls. One
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
    Gpu, NoResources, OffscreenTarget, PresentationLead, RendererOptions, ResourceSource,
    SceneChange, SceneDriver, SceneState, VideoFact,
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
        lead: None,
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
            viewmodel_fov_y_degrees: 0.0,
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
    let capture = scene.capture(Some((40, 20)), false);
    assert_eq!(
        (capture.width, capture.height, capture.rgba.len()),
        (40, 20, 40 * 20 * 4)
    );
    assert_eq!((capture.step, capture.held, capture.sequence), (3, true, 1));
    assert!(capture.cameras[0].observer, "it draws what an output draws");
    let default = scene.capture(None, false);
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

    // The player's video-option choices survive a rebaseline, which builds
    // a new renderer.
    let mut player = render_model::RendererSettingsOverrides::default();
    player
        .choose("antialiasing", &serde_json::json!("off"))
        .unwrap();
    scene.set_player_settings(player);
    scene.rebaseline([], &NoResources, &|_| None, state(4, true));
    let readout = scene.settings_readout();
    assert_eq!(readout.player, player);
    assert_eq!(
        readout.requested.antialiasing,
        render_model::Antialiasing::Off
    );
    drop(scene);

    // A running world's frames draw on past its committed time between
    // calls, so what moves with presentation time moves every frame: from
    // what admission already owed, never behind the frame before, never a
    // step or more ahead. A held world draws its committed time exactly
    // (#9912).
    let running = SceneDriver::new(gpu.clone(), RendererOptions::default());
    const STEP: f64 = 1.0 / 60.0;
    let lead = PresentationLead {
        owed_seconds: 0.25 * STEP,
        rate: 1.0,
        step_seconds: STEP,
        observed_ns: 1,
    };
    let drawn = |driver: &SceneDriver| {
        let ((), shown) = driver.draw(|renderer, now| {
            renderer.render_view_composition(&target, now);
        });
        shown.presentation_seconds
    };
    let apply = |step: u64, lead: Option<PresentationLead>| {
        running.apply(
            [],
            &NoResources,
            &|_| None,
            SceneState {
                lead,
                ..state(step, lead.is_none())
            },
        );
    };
    apply(6, Some(lead));
    let (committed, next) = (6.0 / 60.0, 6.0 / 60.0 + STEP);
    let first = drawn(&running);
    assert!(
        first >= committed + lead.owed_seconds && first < next,
        "{first} runs on from {committed} plus what was owed"
    );
    std::thread::sleep(Duration::from_millis(4));
    let later = drawn(&running);
    assert!(
        later > first && later <= next,
        "{later} moves on from {first}"
    );
    std::thread::sleep(Duration::from_millis(30));
    assert_eq!(
        drawn(&running),
        next,
        "never a step past the committed time"
    );
    // The next step's call owes nothing yet: frames go on from where they
    // drew.
    apply(
        7,
        Some(PresentationLead {
            owed_seconds: 0.0,
            observed_ns: 2,
            ..lead
        }),
    );
    assert!(drawn(&running) >= next);
    apply(7, None);
    assert_eq!(drawn(&running), 7.0 / 60.0, "held, the committed time");
    std::thread::sleep(Duration::from_millis(4));
    assert_eq!(drawn(&running), 7.0 / 60.0, "and it stays");
    apply(8, Some(lead));
    running.set_simulation(true, 8);
    assert_eq!(drawn(&running), 8.0 / 60.0, "paused, the committed time");
    // A call without a new host observation (a debug command between
    // updates) brings the same lead: the clock runs on from its anchor
    // rather than being set back to the owed time.
    let resumed = PresentationLead {
        owed_seconds: 0.0,
        observed_ns: 3,
        ..lead
    };
    apply(9, Some(resumed));
    let start = drawn(&running);
    std::thread::sleep(Duration::from_millis(4));
    apply(9, Some(resumed));
    std::thread::sleep(Duration::from_millis(4));
    let after = drawn(&running);
    assert!(
        after - start >= 0.007,
        "{after} ran on from {start} across a call with no new observation"
    );
    drop(running);

    // Undrawn, a clip still ends on Engine time: waiting advances the scene
    // to the latest call, and the completion reaches the Engine (#8871).
    let unwatched = SceneDriver::new(gpu.clone(), RendererOptions::default());
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
    drop(unwatched);
}
