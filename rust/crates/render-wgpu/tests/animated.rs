//! Animated mesh fixtures (#8788): GLBs admitted by `asset-import` as the
//! runtime admits them, posed on the Engine timeline, skinned, with a joint
//! attachment, completion facts and bounds inspection.

mod support;

use std::path::PathBuf;

use asset_import::{import_animated_glb_asset, ImportContext, SourceUri};
use render_model::*;
use render_wgpu::{AnimationFact, RendererOptions};
use support::*;

const BODY: u64 = 1;
const WEAPON: u64 = 2;

fn fixture(path: &str) -> Vec<u8> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures");
    std::fs::read(root.join(path)).unwrap_or_else(|error| panic!("read {path}: {error}"))
}

/// Admit a GLB and hold its runtime resource; returns the define op.
fn admit(harness: &mut Harness, path: &str) -> AnimatedMeshAsset {
    let bytes = fixture(path);
    let imported = import_animated_glb_asset(
        &SourceUri::RelativePath(path.to_owned()),
        &bytes,
        &ImportContext::default(),
    );
    let imported = imported
        .assets
        .unwrap_or_else(|| panic!("{path} is admitted: {:?}", imported.diagnostics));
    // The runtime serves the admitted bytes under their content identity.
    let hash = imported
        .animated_mesh
        .content_hash
        .clone()
        .expect("content hash");
    harness.resources.insert(
        &format!(
            "animated-mesh-resource/{}",
            hash.trim_start_matches("sha256:")
        ),
        imported.runtime_resource_bytes,
    );
    imported.animated_mesh
}

fn animated(
    handle: u64,
    parent: Option<u64>,
    asset: &AnimatedMeshAsset,
    transform: Transform,
    playback: Option<AnimatedMeshPlaybackCommand>,
) -> RenderDiff {
    RenderDiff::CreateAnimatedMeshInstance {
        handle: RenderHandle::new(handle),
        parent: parent.map(RenderHandle::new),
        instance: AnimatedMeshInstanceDescriptor {
            inspection: AnimatedMeshInspection::default(),
            asset: asset.asset.clone(),
            transform,
            visible: true,
            material_overrides: Vec::new(),
            playback,
            metadata: RenderMetadata {
                source_entity: Some(handle),
                ..RenderMetadata::default()
            },
        },
    }
}

fn sample(clip: &str, normalized_time: f32) -> AnimatedMeshPlaybackCommand {
    AnimatedMeshPlaybackCommand::Sample {
        clip: clip.to_owned(),
        normalized_time,
    }
}

fn play(clip: &str, r#loop: AnimationLoopMode) -> AnimatedMeshPlaybackCommand {
    AnimatedMeshPlaybackCommand::Play {
        clip: clip.to_owned(),
        r#loop,
        speed: 1.0,
        weight: 1.0,
        restart: true,
        fade_seconds: None,
        start_offset_seconds: None,
        start_paused: false,
    }
}

/// The joint attachment fixture's camera: from (4, 3, 6) toward (0, 1.5, 0).
fn character_view() -> render_host_contracts::RendererCompositionCamera {
    camera([4.0, 3.0, 6.0], -33.690_067, -11.750_674)
}

fn character_scene(harness: &mut Harness) {
    let body = admit(harness, "csharp-joint-attachments/content/body.glb");
    let weapon = admit(harness, "csharp-joint-attachments/content/weapon.glb");
    harness.apply(vec![
        RenderDiff::DefineAnimatedMesh {
            asset: body.clone(),
        },
        RenderDiff::DefineAnimatedMesh {
            asset: weapon.clone(),
        },
        animated(
            BODY,
            None,
            &body,
            Transform::IDENTITY,
            Some(sample("run", 0.5)),
        ),
        animated(
            WEAPON,
            Some(BODY),
            &weapon,
            Transform {
                translation: [0.0, 0.0, 0.0],
                rotation: [0.0, 0.0, 0.0, 1.0],
                scale: [0.01; 3],
            },
            None,
        ),
        RenderDiff::SetParentJoint {
            handle: RenderHandle::new(WEAPON),
            joint: Some("RightHand".to_owned()),
        },
    ]);
}

#[test]
fn a_skinned_character_samples_its_clip_and_carries_a_weapon_on_its_hand() {
    let mut harness = Harness::new(RendererOptions::default());
    character_scene(&mut harness);
    let (first, pixels) = harness.render(&character_view());
    assert_eq!(harness.renderer.table_counts().animated_instances, 2);
    assert!(first.draws >= 2);
    assert_screenshot("animated-attachment", &pixels);

    // A sampled pose is held: nothing is re-skinned or re-uploaded.
    harness.renderer.set_animation_time(5.0);
    let (held, _) = harness.render(&character_view());
    assert_eq!(held.parts_uploaded, 0);
}

#[test]
fn repeating_playback_advances_on_the_engine_timeline_and_the_weapon_follows() {
    let mut harness = Harness::new(RendererOptions::default());
    character_scene(&mut harness);
    harness.apply(vec![RenderDiff::SetAnimatedMeshPlayback {
        handle: RenderHandle::new(BODY),
        playback: play("run", AnimationLoopMode::Repeat),
    }]);
    let (_, start) = harness.render(&character_view());
    // Display time alone does not move it: the same Engine time renders the
    // same pose.
    let (still, again) = harness.render(&character_view());
    assert_eq!(still.parts_uploaded, 0);
    assert_eq!(start, again);
    harness.renderer.set_animation_time(0.2);
    let (moved, later) = harness.render(&character_view());
    // The body's parts and the attached weapon's row are rewritten.
    assert!(moved.parts_uploaded >= 2, "{moved:?}");
    // Posing rewrites rows, not the drawn set: no instance ids upload.
    assert_eq!(moved.instances_uploaded, 0, "{moved:?}");
    assert_ne!(start, later);
}

/// The character drawn into an offscreen target, presented over the primary.
fn character_composition() -> render_host_contracts::RendererViewComposition {
    use render_host_contracts::*;
    let full = RendererViewport {
        x: 0.0,
        y: 0.0,
        width: 1.0,
        height: 1.0,
    };
    let mut camera = character_view();
    camera.id = "character".to_owned();
    RendererViewComposition {
        cameras: vec![camera],
        targets: vec![RendererCompositionTarget {
            id: "portrait".to_owned(),
            revision: 1,
            width: 96,
            height: 96,
            color: RendererTargetColor::Rgba8Srgb,
            depth: RendererTargetDepth::Depth24,
            sampling: RendererTargetSampling::Nearest,
        }],
        views: vec![RendererCompositionView {
            id: "portrait-view".to_owned(),
            camera_id: "character".to_owned(),
            target: RendererViewTarget::Offscreen {
                target_id: "portrait".to_owned(),
                target_revision: 1,
            },
            viewport: full,
            order: 0,
        }],
        presentations: vec![RendererCompositionPresentation {
            id: "inset".to_owned(),
            source_target_id: "portrait".to_owned(),
            source_target_revision: 1,
            destination: RendererPrimaryDestination {
                kind: RendererPrimaryDestinationKind::Primary,
                viewport: full,
            },
            order: 1,
        }],
    }
}

fn compose(harness: &mut Harness, time: f64) -> (render_wgpu::FrameStats, Vec<u8>) {
    let stats = harness
        .renderer
        .render_view_composition(&harness.target, time);
    (stats, harness.target.read_rgba(&harness.gpu))
}

#[test]
fn an_advancing_pose_redraws_a_cached_composition_target_and_a_held_pose_reuses_it() {
    let mut harness = Harness::new(RendererOptions::default());
    character_scene(&mut harness);
    harness
        .renderer
        .set_view_composition(&character_composition(), 0.0);
    let (first, start) = compose(&mut harness, 0.0);
    assert_eq!(first.offscreen_views, 1, "a new target draws once");

    // The sampled pose holds, so Engine time alone leaves the target current.
    harness.renderer.set_animation_time(1.0);
    let (held, same) = compose(&mut harness, 1.0);
    assert_eq!(held.offscreen_views, 0, "a held pose reuses the target");
    assert_eq!(same, start);

    harness.apply(vec![RenderDiff::SetAnimatedMeshPlayback {
        handle: RenderHandle::new(BODY),
        playback: play("run", AnimationLoopMode::Repeat),
    }]);
    let (_, playing) = compose(&mut harness, 2.0);
    // Repeating playback moves the pose on Engine time alone: no delta and no
    // republished composition, yet the target redraws the new pose.
    harness.renderer.set_animation_time(1.2);
    let (moved, later) = compose(&mut harness, 3.0);
    assert_eq!(moved.offscreen_views, 1, "{moved:?}");
    assert_ne!(later, playing);
    assert_eq!(
        harness.renderer.view_composition_readout().targets[0].status,
        render_wgpu::TargetStatus::Current
    );
}

#[test]
fn a_once_clip_reports_its_natural_completion_then_holds_and_reports_bounds() {
    let mut harness = Harness::new(RendererOptions::default());
    character_scene(&mut harness);
    let view = character_view();
    harness.render(&view);
    harness.renderer.set_animation_time(1.0);
    harness.apply(vec![RenderDiff::SetAnimatedMeshPlayback {
        handle: RenderHandle::new(BODY),
        playback: play("run", AnimationLoopMode::Once),
    }]);
    harness.renderer.set_animation_time(1.3);
    harness.render(&view);
    assert!(harness.renderer.take_animation_facts().is_empty());
    // `run` lasts 0.667 s.
    harness.renderer.set_animation_time(1.7);
    harness.render(&view);
    assert_eq!(
        harness.renderer.take_animation_facts(),
        vec![AnimationFact::NaturalCompletion {
            object_id: BODY,
            generation: 1,
            clip: "run".to_owned(),
        }]
    );
    harness.renderer.set_animation_time(3.0);
    let (held, _) = harness.render(&view);
    assert_eq!(held.parts_uploaded, 0, "a finished once clip holds");
    assert!(harness.renderer.take_animation_facts().is_empty());

    // A changed bounds request reports the posed, skinned world bounds.
    harness.apply(vec![RenderDiff::SetAnimatedMeshInspection {
        handle: RenderHandle::new(BODY),
        inspection: AnimatedMeshInspection {
            bounds_request: 7,
            ..AnimatedMeshInspection::default()
        },
    }]);
    harness.render(&view);
    let facts = harness.renderer.take_animation_facts();
    let [AnimationFact::MeshInspection {
        object_id: BODY,
        generation: 1,
        request: 7,
        bounds: Some((min, max)),
    }] = facts.as_slice()
    else {
        panic!("expected one bounds observation, got {facts:?}");
    };
    // The admitted bind-pose bounds are x ±1.81 (arms out), y 0..3.77. The
    // posed character stands on the ground below that height with its arms
    // in.
    assert!(
        min[1] > -0.05 && (3.0..3.77).contains(&max[1]),
        "{min:?} {max:?}"
    );
    assert!(max[0] - min[0] < 3.0, "{min:?} {max:?}");
}

#[test]
fn an_output_capture_samples_the_requested_pose() {
    use render_host_contracts::{RenderOutputJob, RenderOutputOperation, RenderOutputPose};
    let mut harness = Harness::new(RendererOptions::default());
    character_scene(&mut harness);
    let capture = |harness: &Harness, normalized_time: f64| {
        let job = RenderOutputJob {
            id: 1,
            source: RenderHandle::new(BODY),
            frame: harness
                .world
                .capture_output_scene(RenderHandle::new(BODY), true)
                .expect("capture the body subtree"),
            operation: RenderOutputOperation::Image {
                camera: Box::new(character_view()),
                width: 96,
                height: 96,
                background: [0.0; 4],
                use_camera_background: false,
                exposure: 1.0,
                aces_filmic: false,
                samples: 1,
                pose: Some(RenderOutputPose {
                    handle: RenderHandle::new(BODY),
                    clip: "run".to_owned(),
                    normalized_time,
                }),
            },
        };
        harness
            .renderer
            .capture_image(&job, &harness.resources)
            .expect("pose capture")
    };
    let early = capture(&harness, 0.1);
    let late = capture(&harness, 0.6);
    assert_ne!(early, late, "the two poses differ");
    assert_eq!(
        early,
        capture(&harness, 0.1),
        "a pose capture is deterministic"
    );
}

#[test]
fn a_controller_drives_its_target_on_the_engine_timeline_and_blends_a_transition() {
    use render_presentation::{
        AnimationControllerClipPhase, AnimationControllerProjectionState,
        AnimationProjectionDescriptor, AnimationProjectionHandle, AnimationProjectionOp,
        AnimationTransitionState, PresentationFrameDiff, PresentationOp, PresentationOpMeta,
        ResolvedAnimationMotion,
    };
    const NO_ENTITIES: &dyn Fn(u64) -> Option<[f32; 3]> = &|_| None;
    let motion = |clip: &str| ResolvedAnimationMotion {
        clip_a: clip.to_owned(),
        clip_b: None,
        blend_weight_milli: 0,
        speed_milli: 1000,
    };
    let state = |tick: u64, transition: Option<AnimationTransitionState>| {
        AnimationControllerProjectionState {
            entity: BODY,
            graph_id: "graph/character".to_owned(),
            graph_version: 1,
            state_id: "run".to_owned(),
            revision: tick,
            controller_tick: tick,
            phase_seconds: 0.0,
            clip_phases: vec![
                AnimationControllerClipPhase {
                    clip: "run".to_owned(),
                    time_seconds: 0.1,
                },
                AnimationControllerClipPhase {
                    clip: "idle".to_owned(),
                    time_seconds: 0.0,
                },
            ],
            motion: motion("run"),
            transition,
            transition_fact: None,
        }
    };
    let frame = |op| PresentationFrameDiff {
        ops: vec![PresentationOp::Animation {
            meta: PresentationOpMeta::new(0),
            op,
        }],
        ..PresentationFrameDiff::default()
    };
    let mut harness = Harness::new(RendererOptions::default());
    character_scene(&mut harness);
    let view = character_view();
    let (_, sampled) = harness.render(&view);
    let handle = AnimationProjectionHandle::new(1);
    let issues = harness.renderer.apply_presentation(
        &frame(AnimationProjectionOp::Create {
            handle,
            descriptor: AnimationProjectionDescriptor {
                target: RenderHandle::new(BODY),
                asset: "graph/character".to_owned(),
                content_hash: "sha256:controller-fixture".to_owned(),
                tick_duration_millis: 16,
                controller: state(1, None),
            },
        }),
        &harness.resources,
        NO_ENTITIES,
    );
    assert!(issues.is_empty(), "{issues:?}");
    let (_, run) = harness.render(&view);
    assert_ne!(run, sampled, "the controller replaced the sampled pose");
    let (still, again) = harness.render(&view);
    assert_eq!(
        (still.parts_uploaded, &again),
        (0, &run),
        "no Engine time, no motion"
    );
    harness.renderer.set_animation_time(0.25);
    let (_, later) = harness.render(&view);
    assert_ne!(later, run, "the clip advanced with Engine time");

    // Halfway through a transition to idle, both clips contribute.
    harness.renderer.apply_presentation(
        &frame(AnimationProjectionOp::Update {
            handle,
            controller: state(
                2,
                Some(AnimationTransitionState {
                    transition_id: "to-idle".to_owned(),
                    from_state_id: "run".to_owned(),
                    to_state_id: "idle".to_owned(),
                    elapsed_ticks: 5,
                    duration_ticks: 10,
                    target_motion: motion("idle"),
                }),
            ),
        }),
        &harness.resources,
        NO_ENTITIES,
    );
    let (blended, mid) = harness.render(&view);
    assert!(blended.parts_uploaded > 0);
    assert_ne!(mid, later);

    // Destroying the controller leaves the instance's own playback.
    harness.renderer.apply_presentation(
        &frame(AnimationProjectionOp::Destroy { handle }),
        &harness.resources,
        NO_ENTITIES,
    );
    let (_, released) = harness.render(&view);
    assert_eq!(released, sampled, "back to the sampled direct playback");
}

/// Redefining a live animated asset keeps its textures and materials: the
/// previous definition is retired before the replacement's ids are reused
/// (#8788 review).
#[test]
fn redefining_a_live_animated_asset_keeps_its_resources_and_image() {
    let mut harness = Harness::new(RendererOptions::default());
    character_scene(&mut harness);
    let view = character_view();
    let (before, pixels_before) = harness.render(&view);
    let counts_before = harness.renderer.table_counts();
    let body = admit(&mut harness, "csharp-joint-attachments/content/body.glb");
    harness.apply(vec![RenderDiff::DefineAnimatedMesh { asset: body }]);
    let (after, pixels_after) = harness.render(&view);
    assert_eq!(harness.renderer.table_counts(), counts_before);
    assert_eq!(before.draws, after.draws);
    assert!(
        pixels_before == pixels_after,
        "an identical redefinition changed {} bytes",
        pixels_before
            .iter()
            .zip(&pixels_after)
            .filter(|(a, b)| a != b)
            .count()
    );
}

#[test]
fn clip_and_joint_indices_are_resolved_again_when_the_asset_changes() {
    let mut harness = Harness::new(RendererOptions::default());
    character_scene(&mut harness);
    harness.apply(vec![RenderDiff::SetAnimatedMeshPlayback {
        handle: RenderHandle::new(BODY),
        playback: play("run", AnimationLoopMode::Repeat),
    }]);
    harness.renderer.set_animation_time(0.3);
    let view = character_view();
    let (_, playing) = harness.render(&view);
    // Each definition resolves its clips afresh (their index order is not
    // stable across definitions); the playing clip and the weapon's joint
    // follow by identity. (The retained model refuses to release an asset a
    // live instance uses, so redefinition is the change to follow.)
    for _ in 0..3 {
        let body = admit(&mut harness, "csharp-joint-attachments/content/body.glb");
        harness.apply(vec![RenderDiff::DefineAnimatedMesh { asset: body }]);
        assert_eq!(harness.render(&view).1, playing);
    }
}

#[test]
fn a_held_attachment_follows_its_moved_parent_by_the_resolved_joint() {
    // The sampled pose holds; only the body moves.
    let moved = Transform {
        translation: [0.4, 0.0, -0.3],
        rotation: [0.0, 0.3826834, 0.0, 0.9238795],
        scale: [1.0; 3],
    };
    let mut harness = Harness::new(RendererOptions::default());
    character_scene(&mut harness);
    let view = character_view();
    harness.render(&view);
    harness.apply(vec![RenderDiff::Update {
        handle: RenderHandle::new(BODY),
        transform: Some(moved),
        material: None,
        visible: None,
        metadata: None,
    }]);
    let (_, after_move) = harness.render(&view);

    // The same scene built at the moved transform from the start.
    let mut reference = Harness::new(RendererOptions::default());
    character_scene(&mut reference);
    reference.apply(vec![RenderDiff::Update {
        handle: RenderHandle::new(BODY),
        transform: Some(moved),
        material: None,
        visible: None,
        metadata: None,
    }]);
    assert_eq!(reference.render(&view).1, after_move);
}

fn requested_bounds(harness: &mut Harness, handle: u64, request: u32) -> ([f32; 3], [f32; 3]) {
    let facts = harness.renderer.take_animation_facts();
    match facts.as_slice() {
        [AnimationFact::MeshInspection {
            object_id,
            request: answered,
            bounds: Some(bounds),
            ..
        }] if *object_id == handle && *answered == request => *bounds,
        other => panic!("expected bounds for {handle} request {request}, got {other:?}"),
    }
}

fn inspect(handle: u64, request: u32) -> RenderDiff {
    RenderDiff::SetAnimatedMeshInspection {
        handle: RenderHandle::new(handle),
        inspection: AnimatedMeshInspection {
            bounds_request: request,
            ..AnimatedMeshInspection::default()
        },
    }
}

fn move_body(x: f32) -> RenderDiff {
    RenderDiff::Update {
        handle: RenderHandle::new(BODY),
        transform: Some(Transform {
            translation: [x, 0.0, 0.0],
            ..Transform::IDENTITY
        }),
        material: None,
        visible: None,
        metadata: None,
    }
}

/// A transform and a bounds request in the same delta report the new
/// placement, for the instance itself and for a child on its joint
/// (#8788 review).
#[test]
fn bounds_requested_with_a_move_report_the_moved_world_state() {
    let mut harness = Harness::new(RendererOptions::default());
    character_scene(&mut harness);
    let view = character_view();
    harness.render(&view);
    harness.apply(vec![inspect(BODY, 1)]);
    harness.render(&view);
    let (body_min, _) = requested_bounds(&mut harness, BODY, 1);

    harness.apply(vec![move_body(20.0), inspect(BODY, 2)]);
    harness.render(&view);
    let (moved_min, _) = requested_bounds(&mut harness, BODY, 2);
    assert!(
        (moved_min[0] - body_min[0] - 20.0).abs() < 1e-3,
        "same-delta bounds {moved_min:?} must include the move from {body_min:?}"
    );

    // The weapon hangs from the body's hand joint: its bounds follow both.
    harness.apply(vec![move_body(-5.0), inspect(WEAPON, 1)]);
    harness.render(&view);
    let same_delta = requested_bounds(&mut harness, WEAPON, 1);
    harness.apply(vec![inspect(WEAPON, 2)]);
    harness.render(&view);
    let next_frame = requested_bounds(&mut harness, WEAPON, 2);
    assert_eq!(
        same_delta, next_frame,
        "the joint child reported a stale placement"
    );
}

/// Inspection wireframe draws the posed instance's triangle edges.
#[test]
fn inspection_wireframe_outlines_the_posed_character() {
    let mut harness = Harness::new(RendererOptions::default());
    character_scene(&mut harness);
    let view = character_view();
    let (_, solid) = harness.render(&view);
    harness.apply(vec![RenderDiff::SetAnimatedMeshInspection {
        handle: RenderHandle::new(BODY),
        inspection: AnimatedMeshInspection {
            wireframe: true,
            ..AnimatedMeshInspection::default()
        },
    }]);
    let (_, outlined) = harness.render(&view);
    let changed = solid
        .as_chunks::<4>()
        .0
        .iter()
        .zip(outlined.as_chunks::<4>().0)
        .filter(|(a, b)| a != b)
        .count();
    assert!(changed > 1000, "wireframe changed only {changed} pixels");
    harness.apply(vec![RenderDiff::SetAnimatedMeshInspection {
        handle: RenderHandle::new(BODY),
        inspection: AnimatedMeshInspection::default(),
    }]);
    let (_, restored) = harness.render(&view);
    assert!(
        restored == solid,
        "clearing inspection restores the solid body"
    );
}
