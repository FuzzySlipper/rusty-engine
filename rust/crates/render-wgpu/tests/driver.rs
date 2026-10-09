//! The scene driver against a real headless device: applied changes land
//! with the step they reach, a drawn frame and a tool capture report what
//! they showed, an undrawn scene still ends clips on Engine time, and a
//! joint-reporting instance reports with each applied call. One
//! test, so the binary opens one device (parallel devices can crash the
//! Vulkan loader).

use std::borrow::Cow;
use std::time::{Duration, Instant};

use render_host_contracts::{
    RendererCameraPose, RendererCameraProjection, RendererCompositionCamera,
    RendererCompositionView, RendererViewComposition, RendererViewTarget, RendererViewport,
};
use render_model::{
    AnimatedMeshAsset, AnimatedMeshInstanceDescriptor, AnimatedMeshPlaybackCommand,
    AnimatedMeshPose, AnimationLoopMode, RenderDiff, RenderFrameDiff, RenderHandle, RenderLayer,
    RenderMetadata, Transform,
};
use render_presentation::{video_frame, VideoClipRef, VideoPlaybackHandle, VideoProjectionOp};
use render_wgpu::{
    AnimationFact, Gpu, NoResources, OffscreenTarget, RendererOptions, ResourceSource, SceneChange,
    SceneDriver, SceneState, VideoFact,
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
    assert_eq!(readout.requested.antialiasing, 1);
    drop(scene);

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

    // An instance that reports its joints reports each applied call's pose
    // with the call, drawn or not, so the Engine reads it before the next.
    let reporting = SceneDriver::new(gpu, RendererOptions::default());
    let body = Body::admit();
    let create = RenderFrameDiff::try_from_ops(vec![
        RenderDiff::DefineAnimatedMesh {
            asset: body.asset.clone(),
        },
        RenderDiff::CreateAnimatedMeshInstance {
            handle: RenderHandle::new(1),
            parent: None,
            instance: AnimatedMeshInstanceDescriptor {
                inspection: Default::default(),
                asset: body.asset.asset.clone(),
                transform: Transform::IDENTITY,
                visible: true,
                material_overrides: Vec::new(),
                playback: Some(AnimatedMeshPlaybackCommand::Play {
                    clip: "run".to_owned(),
                    r#loop: AnimationLoopMode::Repeat,
                    speed: 1.0,
                    weight: 1.0,
                    restart: true,
                    fade_seconds: None,
                    start_offset_seconds: None,
                    start_paused: false,
                }),
                metadata: RenderMetadata {
                    source_entity: Some(1),
                    ..RenderMetadata::default()
                },
                layer: RenderLayer::Scene,
                shadow_casting: Default::default(),
            },
        },
        RenderDiff::SetAnimatedMeshPose {
            handle: RenderHandle::new(1),
            pose: AnimatedMeshPose {
                report_joints: true,
                ..AnimatedMeshPose::default()
            },
        },
    ])
    .expect("a valid frame");
    reporting.apply(
        [SceneChange::Frame(&create)],
        &body,
        &|_| None,
        state(1, false),
    );
    let reported = |facts: Vec<AnimationFact>| {
        facts
            .into_iter()
            .filter_map(|fact| match fact {
                AnimationFact::JointPose {
                    seconds,
                    joints,
                    rest,
                    ..
                } => Some((seconds, joints, rest.is_some())),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let first = reported(reporting.take_animation_facts());
    assert_eq!(first.len(), 1);
    assert_eq!((first[0].0, first[0].2), (1.0 / 60.0, true));
    reporting.apply([], &body, &|_| None, state(2, false));
    let second = reported(reporting.take_animation_facts());
    assert_eq!(second.len(), 1);
    assert_eq!((second[0].0, second[0].2), (2.0 / 60.0, false));
    assert_ne!(first[0].1, second[0].1, "the running pose moved");
}

/// The joint attachment fixture's body, admitted as the runtime admits it.
struct Body {
    asset: AnimatedMeshAsset,
    identity: String,
    bytes: Vec<u8>,
}

impl Body {
    fn admit() -> Self {
        let path = "csharp-joint-attachments/content/body.glb";
        let source = std::fs::read(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../fixtures")
                .join(path),
        )
        .expect("the body fixture");
        let imported = asset_import::import_animated_glb_asset(
            &asset_import::SourceUri::RelativePath(path.to_owned()),
            &source,
            &asset_import::ImportContext::default(),
        )
        .assets
        .expect("the body is admitted");
        let hash = imported
            .animated_mesh
            .content_hash
            .clone()
            .expect("content hash");
        Self {
            identity: format!(
                "animated-mesh-resource/{}",
                hash.trim_start_matches("sha256:")
            ),
            asset: imported.animated_mesh,
            bytes: imported.runtime_resource_bytes,
        }
    }
}

impl ResourceSource for Body {
    fn bytes(&self, identity: &str) -> Option<Cow<'_, [u8]>> {
        (identity == self.identity).then_some(Cow::Borrowed(&self.bytes))
    }
}
