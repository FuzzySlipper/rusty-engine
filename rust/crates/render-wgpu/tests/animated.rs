//! Animated mesh fixtures (#8788): GLBs admitted by `asset-import` as the
//! runtime admits them, posed on the Engine timeline, skinned, with a joint
//! attachment, completion facts and bounds inspection.

mod support;

use std::f32::consts::FRAC_1_SQRT_2;
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
        bytes,
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
            playback_fade: None,
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
            layer: RenderLayer::Scene,
            shadow_casting: Default::default(),
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
            viewport_anchor: None,
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
                received_seconds: None,
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

/// The joint attachment character over a floor, lit by a directional light
/// that requests a shadow.
fn shadowed_character(harness: &mut Harness) {
    character_scene(harness);
    harness.apply(vec![
        RenderDiff::DefineMaterial {
            material: material("material/floor", [0.75, 0.75, 0.72, 1.0], None),
        },
        static_mesh(
            "mesh/floor",
            box_mesh([-4.0, -0.1, -4.0], [4.0, 0.0, 4.0], |_| 0),
            "material/floor",
        ),
        instance(20, None, "mesh/floor", Transform::IDENTITY),
        // A directional light casts from its node, so the node sits above the
        // character, back along the light's direction.
        group(23, None, transform([2.5, 4.0, 1.5], 0.0, [1.0; 3])),
        RenderDiff::CreateLight {
            handle: RenderHandle::new(21),
            parent: Some(RenderHandle::new(23)),
            light: LightDescriptor::Directional {
                color: [1.0, 0.97, 0.9],
                intensity: 2.5,
                enabled: true,
                direction: [-0.5, -1.0, -0.3],
                range: None,
                shadow_intent: LightShadowIntent::Requested,
                shadow: Default::default(),
            },
        },
        RenderDiff::CreateLight {
            handle: RenderHandle::new(22),
            parent: None,
            light: LightDescriptor::Ambient {
                color: [1.0; 3],
                intensity: 0.35,
                enabled: true,
                shadow_intent: LightShadowIntent::Disabled,
                shadow: Default::default(),
                range: None,
            },
        },
    ]);
}

/// Pixels the shadow darkens: the same frame drawn with and without shadows.
fn shadowed_pixels(lit: &[u8], unshadowed: &[u8]) -> Vec<usize> {
    lit.as_chunks::<4>()
        .0
        .iter()
        .zip(unshadowed.as_chunks::<4>().0)
        .enumerate()
        .filter(|(_, (lit, plain))| {
            let sum = |pixel: &[u8; 4]| pixel[..3].iter().map(|&c| u32::from(c)).sum::<u32>();
            sum(plain) > sum(lit) + 30
        })
        .map(|(index, _)| index)
        .collect()
}

#[test]
fn a_skinned_character_and_its_held_weapon_cast_shadows_that_follow_the_pose() {
    let mut shadowed = Harness::new(RendererOptions {
        default_world_lights: false,
        shadows: true,
        ..RendererOptions::default()
    });
    let mut plain = Harness::new(RendererOptions {
        default_world_lights: false,
        ..RendererOptions::default()
    });
    shadowed_character(&mut shadowed);
    shadowed_character(&mut plain);
    // From above, so the shadows of the body, arms and weapon fall in view.
    let view = camera([0.5, 5.5, 3.5], 0.0, -58.0);
    let (first, pixels) = shadowed.render(&view);
    // A sun casts through four cascades.
    assert_eq!(shadowed.renderer.table_counts().shadow_layers, 4);
    assert!(first.shadow_draws > 0);
    assert_screenshot("animated-shadows", &pixels);
    let running = shadowed_pixels(&pixels, &plain.render(&view).1);
    assert!(running.len() > 100, "{} shadowed pixels", running.len());

    // The weapon on the hand casts too: hiding it removes its shadow.
    let weapon_shown = |visible| RenderDiff::Update {
        handle: RenderHandle::new(WEAPON),
        transform: None,
        material: None,
        visible: Some(visible),
        metadata: None,
    };
    for harness in [&mut shadowed, &mut plain] {
        harness.apply(vec![weapon_shown(false)]);
    }
    let (unarmed_stats, pixels) = shadowed.render(&view);
    assert!(unarmed_stats.shadow_draws > 0 && unarmed_stats.shadow_draws < first.shadow_draws);
    let unarmed = shadowed_pixels(&pixels, &plain.render(&view).1);
    assert!(
        unarmed.len() < running.len(),
        "{} shadowed pixels without the weapon, {} with it",
        unarmed.len(),
        running.len()
    );

    // Another pose re-renders the maps, and the shadow takes the new pose.
    for harness in [&mut shadowed, &mut plain] {
        harness.apply(vec![
            weapon_shown(true),
            RenderDiff::SetAnimatedMeshPlayback {
                handle: RenderHandle::new(BODY),
                playback: sample("run", 0.0),
            },
        ]);
    }
    let (posed, pixels) = shadowed.render(&view);
    assert!(posed.shadow_draws > 0, "{posed:?}");
    let other = shadowed_pixels(&pixels, &plain.render(&view).1);
    let changed = running
        .iter()
        .filter(|pixel| other.binary_search(pixel).is_err())
        .count();
    assert!(
        changed > running.len() / 10,
        "{changed} of {} shadowed pixels moved with the pose",
        running.len()
    );
}

/// The admitted rig's joint index of `name` (joints sorted by id).
fn rig_joint(asset: &AnimatedMeshAsset, name: &str) -> u32 {
    asset
        .rig
        .as_ref()
        .and_then(|rig| rig.joints.iter().position(|joint| joint.id == name))
        .unwrap_or_else(|| panic!("rig joint {name}")) as u32
}

fn joint_poses(harness: &mut Harness) -> Vec<(f64, Transform, Vec<PosedJoint>, bool)> {
    harness
        .renderer
        .take_animation_facts()
        .into_iter()
        .filter_map(|fact| match fact {
            AnimationFact::JointPose {
                object_id: BODY,
                seconds,
                world,
                joints,
                rest,
                ..
            } => Some((seconds, world, joints, rest.is_some())),
            _ => None,
        })
        .collect()
}

fn world_point(world: &Transform, local: [f32; 3]) -> [f32; 3] {
    let [x, y, z, w] = world.rotation;
    let scaled = [
        local[0] * world.scale[0],
        local[1] * world.scale[1],
        local[2] * world.scale[2],
    ];
    // v + 2w(q×v) + 2q×(q×v)
    let q = [x, y, z];
    let cross = |a: [f32; 3], b: [f32; 3]| {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };
    let t = cross(q, scaled).map(|value| value * 2.0);
    let u = cross(q, t);
    [0, 1, 2].map(|i| scaled[i] + w * t[i] + u[i] + world.translation[i])
}

fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f32>().sqrt()
}

/// Pose controls on the skinned character: a two-bone IK constraint brings
/// the right hand (and the weapon on it) to a world target, its joints are
/// reported as drawn, and an additive finger override blends by weight.
#[test]
fn pose_controls_reach_a_world_target_and_report_the_drawn_joints() {
    let mut harness = Harness::new(RendererOptions::default());
    character_scene(&mut harness);
    let body = admit(&mut harness, "csharp-joint-attachments/content/body.glb");
    let (arm, forearm, hand, finger) = (
        rig_joint(&body, "RightArm"),
        rig_joint(&body, "RightForeArm"),
        rig_joint(&body, "RightHand"),
        rig_joint(&body, "RightHandIndex1"),
    );
    harness.apply(vec![move_body(2.0)]);
    let view = character_view();
    let (_, before) = harness.render(&view);
    let reach = |target: [f32; 3], curl: f32| RenderDiff::SetAnimatedMeshPose {
        handle: RenderHandle::new(BODY),
        pose: AnimatedMeshPose {
            clip_layers: Vec::new(),
            two_bone_ik: vec![TwoBoneIkConstraint {
                root: arm,
                mid: forearm,
                end: hand,
                space: PoseSpace::World,
                target,
                pole: [2.0, 1.0, -3.0],
                weight: 1.0,
            }],
            overrides: vec![JointOverride {
                joint: finger,
                space: PoseSpace::Local,
                additive: true,
                rotation: Some([0.0, 0.0, FRAC_1_SQRT_2, FRAC_1_SQRT_2]),
                translation: None,
                weight: curl,
            }],
            layers: Vec::new(),
            report_joints: true,
            cuts: Vec::new(),
        },
    };

    harness.apply(vec![reach([1.6, 1.4, 0.5], 0.0)]);
    let (_, reached) = harness.render(&view);
    assert_ne!(before, reached, "the arm moved");
    let reports = joint_poses(&mut harness);
    assert_eq!(reports.len(), 1, "one report per posing");
    let (_, world, joints, first) = &reports[0];
    assert!(*first, "the first report carries the rest pose");
    let hand_world = world_point(world, joints[hand as usize].model.translation);
    assert_eq!(
        hand_world.map(|v| (v * 1e3).round()),
        joints[hand as usize]
            .world
            .translation
            .map(|v| (v * 1e3).round())
    );
    assert!(
        distance(hand_world, [1.6, 1.4, 0.5]) < 1e-3,
        "the reported hand {hand_world:?} is at the target"
    );
    let open_finger = joints[finger as usize].model.rotation;

    // A moving target: each posing reports again, without the rest pose.
    harness.renderer.set_animation_time(0.5);
    harness.apply(vec![reach([1.5, 1.6, 0.6], 1.0)]);
    harness.render(&view);
    let reports = joint_poses(&mut harness);
    let (seconds, _, joints, first) = &reports[0];
    assert_eq!((*seconds, *first), (0.5, false));
    let hand_world = joints[hand as usize].world.translation;
    assert!(
        distance(hand_world, [1.5, 1.6, 0.6]) < 1e-3,
        "{hand_world:?}"
    );
    assert_ne!(
        joints[finger as usize].model.rotation, open_finger,
        "the curl weight bends the finger"
    );

    // Clearing the controls restores the clip pose exactly.
    harness.apply(vec![RenderDiff::SetAnimatedMeshPose {
        handle: RenderHandle::new(BODY),
        pose: AnimatedMeshPose::default(),
    }]);
    let (_, cleared) = harness.render(&view);
    assert_eq!(cleared, before);
    assert!(joint_poses(&mut harness).is_empty(), "reporting stopped");
}

/// Each IK chain of a reporting pose is reported from the solve that ran:
/// a reachable target with no shortfall, one beyond reach straightened toward
/// with the shortfall the drawn hand leaves.
#[test]
fn a_reported_pose_carries_each_ik_chains_target_and_shortfall() {
    let mut harness = Harness::new(RendererOptions::default());
    character_scene(&mut harness);
    let body = admit(&mut harness, "csharp-joint-attachments/content/body.glb");
    let (arm, forearm, hand) = (
        rig_joint(&body, "RightArm"),
        rig_joint(&body, "RightForeArm"),
        rig_joint(&body, "RightHand"),
    );
    harness.apply(vec![move_body(2.0)]);
    let reach = |target: [f32; 3]| RenderDiff::SetAnimatedMeshPose {
        handle: RenderHandle::new(BODY),
        pose: AnimatedMeshPose {
            clip_layers: Vec::new(),
            two_bone_ik: vec![TwoBoneIkConstraint {
                root: arm,
                mid: forearm,
                end: hand,
                space: PoseSpace::World,
                target,
                pole: [2.0, 1.0, -3.0],
                weight: 1.0,
            }],
            overrides: Vec::new(),
            layers: Vec::new(),
            report_joints: true,
            cuts: Vec::new(),
        },
    };
    let chains = |harness: &mut Harness| {
        harness
            .renderer
            .take_animation_facts()
            .into_iter()
            .find_map(|fact| match fact {
                AnimationFact::JointPose {
                    joints, ik_chains, ..
                } => Some((joints, ik_chains)),
                _ => None,
            })
            .expect("a joint report")
    };
    let view = character_view();

    harness.apply(vec![reach([1.6, 1.4, 0.5])]);
    harness.render(&view);
    let (_, reached) = chains(&mut harness);
    assert_eq!(reached.len(), 1);
    assert!(!reached[0].straightened);
    assert!(reached[0].shortfall < 1e-3, "{:?}", reached[0]);
    assert!(distance(reached[0].target_world, [1.6, 1.4, 0.5]) < 1e-4);

    let far = [6.0, 1.4, 0.5];
    harness.apply(vec![reach(far)]);
    harness.render(&view);
    let (joints, beyond) = chains(&mut harness);
    let chain = beyond[0];
    assert!(chain.straightened);
    let drawn = joints[hand as usize].world.translation;
    assert!(distance(chain.end_world, drawn) < 1e-4);
    assert!(
        (chain.shortfall - distance(drawn, far)).abs() < 1e-4 && chain.shortfall > 1.0,
        "{chain:?}"
    );
}

/// The renderer's own inspection drawing on the character: its joint chain
/// from the pose it evaluated, and each IK chain's target and shortfall
/// segment, which it records for the overlay without a joint report.
#[test]
fn inspection_draws_the_joint_chain_and_ik_reach_of_an_instance() {
    use render_host_contracts::{
        RendererCompositionView, RendererViewComposition, RendererViewTarget, RendererViewport,
    };
    let mut harness = Harness::new(RendererOptions::default());
    character_scene(&mut harness);
    let body = admit(&mut harness, "csharp-joint-attachments/content/body.glb");
    let joint = |name| rig_joint(&body, name);
    harness.apply(vec![
        move_body(2.0),
        RenderDiff::SetAnimatedMeshPose {
            handle: RenderHandle::new(BODY),
            pose: AnimatedMeshPose {
                clip_layers: Vec::new(),
                two_bone_ik: vec![TwoBoneIkConstraint {
                    root: joint("RightArm"),
                    mid: joint("RightForeArm"),
                    end: joint("RightHand"),
                    space: PoseSpace::World,
                    target: [6.0, 1.4, 0.5],
                    pole: [2.0, 1.0, -3.0],
                    weight: 1.0,
                }],
                overrides: Vec::new(),
                layers: Vec::new(),
                report_joints: false,
                cuts: Vec::new(),
            },
        },
    ]);
    let view = character_view();
    harness.renderer.set_view_composition(
        &RendererViewComposition {
            cameras: vec![view.clone()],
            targets: Vec::new(),
            views: vec![RendererCompositionView {
                id: "main".to_owned(),
                camera_id: view.id.clone(),
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
        },
        0.0,
    );
    let draw = |harness: &mut Harness| {
        harness
            .renderer
            .render_view_composition(&harness.target, 0.0)
            .overlay_lines
    };
    let mut shown = render_wgpu::OverlayShown::default();
    shown.joints.insert(BODY);
    harness.renderer.set_overlay_shown(shown.clone());
    let joints = draw(&mut harness);
    // Each rig joint below another draws its bone and a three-line cross.
    let bones = body
        .rig
        .as_ref()
        .unwrap()
        .joints
        .iter()
        .filter(|joint| joint.parent.is_some())
        .count() as u32;
    assert_eq!(joints, bones * 4);
    shown.ik.insert(BODY);
    harness.renderer.set_overlay_shown(shown);
    // Posed again to record the chain, then drawn: a cross and a segment.
    assert_eq!(draw(&mut harness), joints + 4);
}

#[test]
fn a_controller_created_again_for_another_target_drives_only_that_one() {
    use render_presentation::{
        AnimationControllerClipPhase, AnimationControllerProjectionState,
        AnimationProjectionDescriptor, AnimationProjectionHandle, AnimationProjectionOp,
        PresentationFrameDiff, PresentationOp, PresentationOpMeta, ResolvedAnimationMotion,
    };
    const NO_ENTITIES: &dyn Fn(u64) -> Option<[f32; 3]> = &|_| None;
    const SECOND: u64 = 3;
    let handle = AnimationProjectionHandle::new(1);
    let create = |target: u64| PresentationFrameDiff {
        ops: vec![PresentationOp::Animation {
            meta: PresentationOpMeta::new(0),
            op: AnimationProjectionOp::Create {
                handle,
                descriptor: AnimationProjectionDescriptor {
                    received_seconds: None,
                    target: RenderHandle::new(target),
                    asset: "graph/character".to_owned(),
                    content_hash: "sha256:controller-fixture".to_owned(),
                    tick_duration_millis: 16,
                    controller: AnimationControllerProjectionState {
                        entity: BODY,
                        graph_id: "graph/character".to_owned(),
                        graph_version: 1,
                        state_id: "run".to_owned(),
                        revision: 1,
                        controller_tick: 1,
                        phase_seconds: 0.0,
                        clip_phases: vec![AnimationControllerClipPhase {
                            clip: "run".to_owned(),
                            time_seconds: 0.1,
                        }],
                        motion: ResolvedAnimationMotion {
                            clip_a: "run".to_owned(),
                            clip_b: None,
                            blend_weight_milli: 0,
                            speed_milli: 1000,
                        },
                        transition: None,
                        transition_fact: None,
                    },
                },
            },
        }],
        ..PresentationFrameDiff::default()
    };
    // Two characters side by side, each sampling its own clip.
    let scene = |harness: &mut Harness| {
        character_scene(harness);
        let body = admit(harness, "csharp-joint-attachments/content/body.glb");
        harness.apply(vec![animated(
            SECOND,
            None,
            &body,
            transform([-2.0, 0.0, 0.0], 0.0, [1.0; 3]),
            Some(sample("run", 0.5)),
        )]);
    };
    let view = character_view();
    let mut harness = Harness::new(RendererOptions::default());
    scene(&mut harness);
    let (_, sampled) = harness.render(&view);
    let issues =
        harness
            .renderer
            .apply_presentation(&create(BODY), &harness.resources, NO_ENTITIES);
    assert!(issues.is_empty(), "{issues:?}");
    let (_, first) = harness.render(&view);
    assert_ne!(first, sampled, "the controller drives the first character");

    // The same controller created for the second character: the first
    // returns to its own playback.
    harness
        .renderer
        .apply_presentation(&create(SECOND), &harness.resources, NO_ENTITIES);
    let (_, moved) = harness.render(&view);
    let mut expected = Harness::new(RendererOptions::default());
    scene(&mut expected);
    expected
        .renderer
        .apply_presentation(&create(SECOND), &expected.resources, NO_ENTITIES);
    let (_, only_second) = expected.render(&view);
    assert_ne!(moved, first);
    assert_eq!(moved, only_second, "only the second character is driven");
}

#[test]
fn a_redefined_asset_keeps_its_instances_drawing() {
    let mut harness = Harness::new(RendererOptions::default());
    character_scene(&mut harness);
    let (shown, before) = harness.render(&character_view());
    // A redefinition retires the old asset's materials before the new one
    // lands, which re-derives the instance's parts while it has no asset.
    let body = admit(&mut harness, "csharp-joint-attachments/content/body.glb");
    harness.apply(vec![RenderDiff::DefineAnimatedMesh { asset: body }]);
    let (again, after) = harness.render(&character_view());
    assert_eq!(again.draws, shown.draws);
    assert_eq!(before, after);
}

/// A pose sampled without a renderer (`animation_pose::PoseModel`, as the
/// Engine services sample at simulation time) is the pose the renderer draws
/// and reports for the same clip time, controls and placement.
#[test]
fn a_pose_sampled_without_the_renderer_matches_the_drawn_report() {
    let mut harness = Harness::new(RendererOptions::default());
    character_scene(&mut harness);
    let body = admit(&mut harness, "csharp-joint-attachments/content/body.glb");
    let finger = rig_joint(&body, "RightHandIndex1");
    let controls = AnimatedMeshPose {
        clip_layers: Vec::new(),
        two_bone_ik: Vec::new(),
        overrides: vec![JointOverride {
            joint: finger,
            space: PoseSpace::Local,
            additive: true,
            rotation: Some([0.0, 0.0, FRAC_1_SQRT_2, FRAC_1_SQRT_2]),
            translation: None,
            weight: 0.5,
        }],
        layers: Vec::new(),
        report_joints: true,
        cuts: Vec::new(),
    };
    let model = animation_pose::PoseModel::decode(
        &body,
        &fixture("csharp-joint-attachments/content/body.glb"),
        |_| None::<Vec<u8>>,
    )
    .expect("the body poses");
    let view = character_view();
    for (x, time) in [(2.0, 0.5), (-1.0, 0.8)] {
        harness.apply(vec![
            move_body(x),
            RenderDiff::SetAnimatedMeshPlayback {
                handle: RenderHandle::new(BODY),
                playback: sample("run", time),
            },
            RenderDiff::SetAnimatedMeshPose {
                handle: RenderHandle::new(BODY),
                pose: controls.clone(),
            },
        ]);
        harness.render(&view);
        let reports = joint_poses(&mut harness);
        let (_, world, drawn, _) = reports.last().expect("a report");
        let sampled = model
            .sample(
                &[animation_pose::ClipSample {
                    clip: "run",
                    time,
                    normalized: true,
                    weight: 1.0,
                }],
                &controls,
                world,
            )
            .unwrap();
        assert_eq!(sampled.len(), drawn.len());
        for (index, (sampled, drawn)) in sampled.iter().zip(drawn).enumerate() {
            for (a, b) in [
                (sampled.model.translation, drawn.model.translation),
                (sampled.world.translation, drawn.world.translation),
            ] {
                assert!(distance(a, b) < 1e-5, "joint {index}: {a:?} {b:?}");
            }
            let dot: f32 = (0..4)
                .map(|i| sampled.world.rotation[i] * drawn.world.rotation[i])
                .sum();
            assert!(dot.abs() > 1.0 - 1e-5, "joint {index} rotation");
        }
    }
}

/// What an instance plays, followed outside the renderer with
/// `animation_pose::PlaybackState` and `ControllerPlayback` from the same
/// commands at the same Engine times (as the services follow it to sample
/// an instance's current playback), poses as the renderer draws and reports
/// it: direct playback, a cross-fade and a controller in a transition.
#[test]
fn current_playback_followed_without_the_renderer_matches_the_drawn_report() {
    use render_presentation::{
        AnimationControllerClipPhase, AnimationControllerProjectionState,
        AnimationProjectionDescriptor, AnimationProjectionHandle, AnimationProjectionOp,
        AnimationTransitionState, PresentationFrameDiff, PresentationOp, PresentationOpMeta,
        ResolvedAnimationMotion,
    };
    const NO_ENTITIES: &dyn Fn(u64) -> Option<[f32; 3]> = &|_| None;
    let mut harness = Harness::new(RendererOptions::default());
    character_scene(&mut harness);
    let body = admit(&mut harness, "csharp-joint-attachments/content/body.glb");
    let model = animation_pose::PoseModel::decode(
        &body,
        &fixture("csharp-joint-attachments/content/body.glb"),
        |_| None::<Vec<u8>>,
    )
    .expect("the body poses");
    let report = AnimatedMeshPose {
        report_joints: true,
        ..AnimatedMeshPose::default()
    };
    harness.apply(vec![
        move_body(1.0),
        RenderDiff::SetAnimatedMeshPose {
            handle: RenderHandle::new(BODY),
            pose: report.clone(),
        },
    ]);
    // The scene's own playback, applied at Engine time 0.
    let mut followed = animation_pose::PlaybackState::default();
    followed.apply(&sample("run", 0.5), 0.0).unwrap();
    let mut controller: Option<animation_pose::ControllerPlayback> = None;
    let view = character_view();
    let check = |harness: &mut Harness,
                 followed: &mut animation_pose::PlaybackState,
                 controller: &Option<animation_pose::ControllerPlayback>,
                 now: f64,
                 case: &str| {
        harness.renderer.set_animation_time(now);
        harness.render(&view);
        let reports = joint_poses(harness);
        let (seconds, world, drawn, _) = reports.last().expect("a report");
        assert_eq!(*seconds, now, "{case}");
        let index = |clip: &str| model.clip_index(clip);
        let duration = |clip: usize| model.clip_duration(clip);
        followed.resolve(&index);
        let driving = controller.clone().map(|mut controller| {
            controller.resolve(&index);
            controller.actions(now, &duration)
        });
        let (actions, _) = followed.actions(now, &duration, driving);
        let sampled = model.sample_actions(&actions, &report, world);
        for (joint, (sampled, drawn)) in sampled.iter().zip(drawn).enumerate() {
            let (a, b) = (sampled.world.translation, drawn.world.translation);
            assert!(distance(a, b) < 1e-5, "{case}: joint {joint}: {a:?} {b:?}");
        }
        actions.len()
    };

    // Direct playback, applied at 1.0 and drawn at 1.4.
    harness.renderer.set_animation_time(1.0);
    let idle = play("idle", AnimationLoopMode::Repeat);
    harness.apply(vec![RenderDiff::SetAnimatedMeshPlayback {
        handle: RenderHandle::new(BODY),
        playback: idle.clone(),
    }]);
    followed.apply(&idle, 1.0).unwrap();
    assert_eq!(
        check(&mut harness, &mut followed, &controller, 1.4, "playing"),
        1
    );

    // A cross-fade to another clip, applied at 1.4 and drawn partway.
    let run = AnimatedMeshPlaybackCommand::Play {
        clip: "run".to_owned(),
        r#loop: AnimationLoopMode::Repeat,
        speed: 1.0,
        weight: 1.0,
        restart: true,
        fade_seconds: Some(0.5),
        start_offset_seconds: None,
        start_paused: false,
    };
    harness.apply(vec![RenderDiff::SetAnimatedMeshPlayback {
        handle: RenderHandle::new(BODY),
        playback: run.clone(),
    }]);
    followed.apply(&run, 1.4).unwrap();
    assert_eq!(
        check(
            &mut harness,
            &mut followed,
            &controller,
            1.6,
            "cross-fading"
        ),
        2,
        "the prior clip fades under the new one"
    );

    // A controller halfway through a transition replaces direct playback
    // (the fade still under it), applied at 1.6 and drawn at 1.75.
    let motion = |clip: &str| ResolvedAnimationMotion {
        clip_a: clip.to_owned(),
        clip_b: None,
        blend_weight_milli: 0,
        speed_milli: 1250,
    };
    let state = AnimationControllerProjectionState {
        entity: BODY,
        graph_id: "graph/character".to_owned(),
        graph_version: 1,
        state_id: "run".to_owned(),
        revision: 1,
        controller_tick: 1,
        phase_seconds: 0.0,
        clip_phases: vec![
            AnimationControllerClipPhase {
                clip: "jump".to_owned(),
                time_seconds: 0.2,
            },
            AnimationControllerClipPhase {
                clip: "idle".to_owned(),
                time_seconds: 0.1,
            },
        ],
        motion: motion("jump"),
        transition: Some(AnimationTransitionState {
            transition_id: "to-idle".to_owned(),
            from_state_id: "jump".to_owned(),
            to_state_id: "idle".to_owned(),
            elapsed_ticks: 3,
            duration_ticks: 10,
            target_motion: motion("idle"),
        }),
        transition_fact: None,
    };
    let issues = harness.renderer.apply_presentation(
        &PresentationFrameDiff {
            ops: vec![PresentationOp::Animation {
                meta: PresentationOpMeta::new(0),
                op: AnimationProjectionOp::Create {
                    handle: AnimationProjectionHandle::new(1),
                    descriptor: AnimationProjectionDescriptor {
                        received_seconds: None,
                        target: RenderHandle::new(BODY),
                        asset: "graph/character".to_owned(),
                        content_hash: "sha256:controller-fixture".to_owned(),
                        tick_duration_millis: 16,
                        controller: state.clone(),
                    },
                },
            }],
            ..PresentationFrameDiff::default()
        },
        &harness.resources,
        NO_ENTITIES,
    );
    assert!(issues.is_empty(), "{issues:?}");
    controller = Some(animation_pose::ControllerPlayback::new(state, 1.6));
    assert_eq!(
        check(&mut harness, &mut followed, &controller, 1.75, "controlled"),
        3,
        "the fading idle under the controller's jump and idle"
    );
}

/// A clip pack that carries only the character's skeleton plays its clips
/// as clip layers over the playback: an override on the right arm at half
/// weight and an additive nod. The renderer draws and reports the pose
/// `animation_pose::PoseModel` samples for the same layers.
#[test]
fn clip_layers_from_a_skeleton_only_pack_draw_what_pose_sampling_gives() {
    let mut harness = Harness::new(RendererOptions::default());
    character_scene(&mut harness);
    let mut body = admit(&mut harness, "csharp-joint-attachments/content/body.glb");
    let pack_path = "csharp-joint-attachments/content/body.clips.glb";
    let pack_bytes = fixture(pack_path);
    let pack = import_animated_glb_asset(
        &SourceUri::RelativePath(pack_path.to_owned()),
        &pack_bytes,
        &ImportContext::default(),
    )
    .assets
    .expect("a skeleton-only pack is admitted")
    .animated_mesh;
    let hash = pack.content_hash.clone().expect("content hash");
    harness.resources.insert(
        &format!("clip-pack-resource/{}", hash.trim_start_matches("sha256:")),
        pack_bytes.clone(),
    );
    body.clip_packs.push(AnimationClipPack {
        asset: format!("animation-clip-pack/{}", hash.trim_start_matches("sha256:")),
        runtime_format: pack.runtime_format,
        content_hash: hash.clone(),
        rig: pack.rig.clone().expect("the pack's rig"),
        clips: pack.clips.clone(),
        provenance: AnimationClipPackProvenance {
            producer: "fixture".to_owned(),
            source_hash: hash,
            target_hash: body.content_hash.clone().unwrap(),
            license: "CC0".to_owned(),
        },
    });
    let joint = |name: &str| rig_joint(&body, name);
    let controls = AnimatedMeshPose {
        clip_layers: vec![
            ClipLayer {
                clip: "wave".to_owned(),
                time: 0.25,
                normalized: true,
                weight: 0.5,
                blend: ClipLayerBlend::Override,
                reference_time: 0.0,
                joint_weights: vec![
                    JointWeight {
                        joint: joint("RightArm"),
                        weight: 1.0,
                    },
                    JointWeight {
                        joint: joint("RightForeArm"),
                        weight: 1.0,
                    },
                ],
            },
            ClipLayer {
                clip: "nod".to_owned(),
                time: 0.5,
                normalized: true,
                weight: 1.0,
                blend: ClipLayerBlend::Additive,
                reference_time: 0.0,
                joint_weights: Vec::new(),
            },
        ],
        report_joints: true,
        ..AnimatedMeshPose::default()
    };
    harness.apply(vec![
        RenderDiff::DefineAnimatedMesh {
            asset: body.clone(),
        },
        RenderDiff::SetAnimatedMeshPose {
            handle: RenderHandle::new(BODY),
            pose: controls.clone(),
        },
    ]);
    harness.render(&character_view());
    let reports = joint_poses(&mut harness);
    let (_, world, drawn, _) = reports.last().expect("a report");
    let model = animation_pose::PoseModel::decode(
        &body,
        &fixture("csharp-joint-attachments/content/body.glb"),
        |identity| {
            identity
                .ends_with(&body.clip_packs[0].content_hash[7..])
                .then(|| pack_bytes.clone())
        },
    )
    .expect("the body and its pack pose");
    let sampled = model
        .sample(
            &[animation_pose::ClipSample {
                clip: "run",
                time: 0.5,
                normalized: true,
                weight: 1.0,
            }],
            &controls,
            world,
        )
        .unwrap();
    let plain = model
        .sample(
            &[animation_pose::ClipSample {
                clip: "run",
                time: 0.5,
                normalized: true,
                weight: 1.0,
            }],
            &AnimatedMeshPose::default(),
            world,
        )
        .unwrap();
    for (index, (sampled, drawn)) in sampled.iter().zip(drawn).enumerate() {
        let (a, b) = (sampled.world.translation, drawn.world.translation);
        assert!(distance(a, b) < 1e-5, "joint {index}: {a:?} {b:?}");
    }
    let moved = |name: &str| {
        let index = joint(name) as usize;
        distance(
            sampled[index].world.translation,
            plain[index].world.translation,
        ) > 1e-3
    };
    assert!(moved("RightHand"), "the arm waves");
    let head = joint("Head") as usize;
    let turn: f32 = (0..4)
        .map(|i| sampled[head].world.rotation[i] * plain[head].world.rotation[i])
        .sum();
    assert!(turn.abs() < 0.999, "the head nods");
    assert!(!moved("LeftHand"), "the left arm stays with the run");
}

/// A renderer built from the retained world partway through a cross-fade,
/// a fade-out or a controller's update draws and reports the pose the live
/// renderer does: the fade keeps its start and prior playback, and the
/// controller state advances from when it took effect, not from the rebuild.
#[test]
fn a_renderer_rebuilt_from_the_retained_world_poses_as_the_live_one() {
    use render_presentation::{
        AnimationControllerClipPhase, AnimationControllerProjectionState,
        AnimationProjectionDescriptor, AnimationProjectionHandle, AnimationProjectionOp,
        PresentationFrameDiff, PresentationOp, PresentationOpMeta, ResolvedAnimationMotion,
    };
    const NO_ENTITIES: &dyn Fn(u64) -> Option<[f32; 3]> = &|_| None;
    let mut harness = Harness::new(RendererOptions::default());
    character_scene(&mut harness);
    harness.apply(vec![RenderDiff::SetAnimatedMeshPose {
        handle: RenderHandle::new(BODY),
        pose: AnimatedMeshPose {
            report_joints: true,
            ..AnimatedMeshPose::default()
        },
    }]);
    let view = character_view();
    // Both clocks at `seconds`: the retained world's and the renderer's.
    let at = |harness: &mut Harness, seconds: f64| {
        let behind = seconds - harness.world.elapsed_seconds();
        harness.world.advance_elapsed(behind);
        harness.renderer.set_animation_time(seconds);
    };
    let play = |clip: &str, fade: Option<f32>| RenderDiff::SetAnimatedMeshPlayback {
        handle: RenderHandle::new(BODY),
        playback: AnimatedMeshPlaybackCommand::Play {
            clip: clip.to_owned(),
            r#loop: AnimationLoopMode::Repeat,
            speed: 1.0,
            weight: 1.0,
            restart: true,
            fade_seconds: fade,
            start_offset_seconds: None,
            start_paused: false,
        },
    };
    let mut baseline: Vec<PresentationFrameDiff> = Vec::new();
    let compare = |harness: &mut Harness, baseline: &[PresentationFrameDiff], case: &str| {
        harness.render(&view);
        let live = joint_poses(harness).pop().expect("a live report").2;
        let mut fresh = render_wgpu::Renderer::new(&harness.gpu, harness.renderer.options());
        fresh.set_animation_time(harness.world.elapsed_seconds());
        let issues = fresh.apply(&harness.world.snapshot().frame, &harness.resources);
        assert!(issues.is_empty(), "{case}: {issues:?}");
        for frame in harness.world.with_retained_captures(baseline.to_vec()) {
            let issues = fresh.apply_presentation(&frame, &harness.resources, NO_ENTITIES);
            assert!(issues.is_empty(), "{case}: {issues:?}");
        }
        fresh.render_offscreen(&view, &harness.target);
        let rebuilt = fresh
            .take_animation_facts()
            .into_iter()
            .find_map(|fact| match fact {
                AnimationFact::JointPose {
                    object_id: BODY,
                    joints,
                    ..
                } => Some(joints),
                _ => None,
            })
            .expect("a rebuilt report");
        for (joint, (live, rebuilt)) in live.iter().zip(&rebuilt).enumerate() {
            let (a, b) = (live.world.translation, rebuilt.world.translation);
            assert!(distance(a, b) < 1e-5, "{case}: joint {joint}: {a:?} {b:?}");
        }
    };

    at(&mut harness, 1.0);
    harness.apply(vec![play("idle", None)]);
    at(&mut harness, 1.4);
    harness.apply(vec![play("run", Some(1.0))]);
    at(&mut harness, 1.9);
    compare(&mut harness, &baseline, "halfway through a cross-fade");

    harness.apply(vec![RenderDiff::SetAnimatedMeshPlayback {
        handle: RenderHandle::new(BODY),
        playback: AnimatedMeshPlaybackCommand::Stop {
            fade_seconds: Some(0.8),
        },
    }]);
    at(&mut harness, 2.2);
    compare(&mut harness, &baseline, "partway through a fade-out");

    harness.apply(vec![play("jump", None)]);
    let state = AnimationControllerProjectionState {
        entity: BODY,
        graph_id: "graph/character".to_owned(),
        graph_version: 1,
        state_id: "run".to_owned(),
        revision: 1,
        controller_tick: 1,
        phase_seconds: 0.0,
        clip_phases: vec![AnimationControllerClipPhase {
            clip: "run".to_owned(),
            time_seconds: 0.1,
        }],
        motion: ResolvedAnimationMotion {
            clip_a: "run".to_owned(),
            clip_b: None,
            blend_weight_milli: 0,
            speed_milli: 1000,
        },
        transition: None,
        transition_fact: None,
    };
    let create = PresentationFrameDiff {
        ops: vec![PresentationOp::Animation {
            meta: PresentationOpMeta::new(0),
            op: AnimationProjectionOp::Create {
                handle: AnimationProjectionHandle::new(1),
                descriptor: AnimationProjectionDescriptor {
                    target: RenderHandle::new(BODY),
                    asset: "graph/character".to_owned(),
                    content_hash: "sha256:controller-fixture".to_owned(),
                    tick_duration_millis: 16,
                    controller: state,
                    received_seconds: None,
                },
            },
        }],
        ..PresentationFrameDiff::default()
    };
    let delta = harness.world.apply_presentation(create.clone()).unwrap();
    let issues = harness
        .renderer
        .apply_presentation(&delta, &harness.resources, NO_ENTITIES);
    assert!(issues.is_empty(), "{issues:?}");
    // The mechanism's own baseline names the controller without a time.
    baseline.push(create);
    at(&mut harness, 2.7);
    compare(
        &mut harness,
        &baseline,
        "a controller updated before the rebuild",
    );
}

/// Pixels that differ between two frames.
fn differing_pixels(a: &[u8], b: &[u8]) -> usize {
    a.as_chunks::<4>()
        .0
        .iter()
        .zip(b.as_chunks::<4>().0)
        .filter(|(a, b)| a != b)
        .count()
}

/// A cut joint's subtree leaves the skinned body and is drawn as a piece
/// posed as it was at the cut and placed where the cut says; the weapon on a
/// cut hand goes with its piece, the skeleton keeps its own pose, a fresh
/// renderer draws the same, and clearing the cuts draws the whole character.
/// Two adjacent pieces (R10176-1): the forearm cut first, then the upper
/// arm. Upper-arm vertices partly weighted to the forearm stay rigid with
/// the upper-arm piece wherever the forearm's piece goes: sending the
/// forearm far away draws the upper arm in place as it draws without its
/// own cut, not stretched toward the forearm.
#[test]
fn adjacent_pieces_move_apart_without_stretching_each_other() {
    let mut harness = Harness::new(RendererOptions::default());
    character_scene(&mut harness);
    let body = admit(&mut harness, "csharp-joint-attachments/content/body.glb");
    let (forearm, arm) = (rig_joint(&body, "LeftForeArm"), rig_joint(&body, "LeftArm"));
    let view = character_view();
    let reporting = |cuts: Vec<JointCut>| RenderDiff::SetAnimatedMeshPose {
        handle: RenderHandle::new(BODY),
        pose: AnimatedMeshPose {
            report_joints: true,
            cuts,
            ..AnimatedMeshPose::default()
        },
    };
    harness.apply(vec![reporting(Vec::new())]);
    harness.render(&view);
    let (_, _, joints, _) = joint_poses(&mut harness).pop().expect("a report");
    let pose: Vec<Transform> = joints.iter().map(|joint| joint.model).collect();
    let cut = |joint: u32, offset: [f32; 3]| {
        let mut placement = joints[joint as usize].world;
        for (axis, delta) in offset.iter().enumerate() {
            placement.translation[axis] += delta;
        }
        JointCut {
            joint,
            pose: pose.clone(),
            placement,
        }
    };
    // Far behind the camera: the forearm's piece leaves the view.
    const GONE: [f32; 3] = [0.0, 0.0, 50.0];
    harness.apply(vec![reporting(vec![cut(forearm, GONE)])]);
    let (_, forearm_only) = harness.render(&view);
    harness.apply(vec![reporting(vec![
        cut(forearm, GONE),
        cut(arm, [0.0; 3]),
    ])]);
    let (_, both) = harness.render(&view);
    let changed = differing_pixels(&forearm_only, &both);
    assert!(
        changed * 1000 < both.len() / 4,
        "the upper arm's piece in place draws as the uncut upper arm: {changed} pixels changed"
    );
}

#[test]
fn a_cut_joint_draws_its_subtree_as_a_piece_where_it_is_placed() {
    let mut harness = Harness::new(RendererOptions::default());
    character_scene(&mut harness);
    let body = admit(&mut harness, "csharp-joint-attachments/content/body.glb");
    let (head, arm) = (rig_joint(&body, "Head"), rig_joint(&body, "RightArm"));
    let view = character_view();
    let reporting = |cuts: Vec<JointCut>| RenderDiff::SetAnimatedMeshPose {
        handle: RenderHandle::new(BODY),
        pose: AnimatedMeshPose {
            report_joints: true,
            cuts,
            ..AnimatedMeshPose::default()
        },
    };
    harness.apply(vec![reporting(Vec::new())]);
    let (_, whole) = harness.render(&view);
    let (_, _, joints, _) = joint_poses(&mut harness).pop().expect("a report");
    let pose: Vec<Transform> = joints.iter().map(|joint| joint.model).collect();
    let cut = |joint: u32, offset: [f32; 3]| {
        let mut placement = joints[joint as usize].world;
        for (axis, delta) in offset.iter().enumerate() {
            placement.translation[axis] += delta;
        }
        JointCut {
            joint,
            pose: pose.clone(),
            placement,
        }
    };
    harness.apply(vec![inspect(BODY, 1), inspect(WEAPON, 1)]);
    harness.render(&view);
    let facts = harness.renderer.take_animation_facts();
    let bounds_of = |facts: &[AnimationFact], handle: u64| {
        facts
            .iter()
            .find_map(|fact| match fact {
                AnimationFact::MeshInspection {
                    object_id,
                    bounds: Some(bounds),
                    ..
                } if *object_id == handle => Some(*bounds),
                _ => None,
            })
            .unwrap_or_else(|| panic!("bounds of {handle}: {facts:?}"))
    };
    let (body_whole, weapon_whole) = (bounds_of(&facts, BODY), bounds_of(&facts, WEAPON));

    // Cut where they are, the pieces draw the character as it was.
    harness.apply(vec![reporting(vec![
        cut(head, [0.0; 3]),
        cut(arm, [0.0; 3]),
    ])]);
    let (_, in_place) = harness.render(&view);
    let seam = differing_pixels(&whole, &in_place);
    assert!(
        seam * 1000 < whole.len() / 4,
        "a piece in place draws as before: {seam} pixels changed"
    );
    let (_, _, reported, _) = joint_poses(&mut harness).pop().expect("a report");
    assert_eq!(
        reported[head as usize].world, joints[head as usize].world,
        "the skeleton keeps its own pose"
    );

    // Moved away, the head leaves its place and the weapon goes with the arm.
    const AWAY: [f32; 3] = [1.5, -0.5, 0.0];
    harness.apply(vec![
        reporting(vec![cut(head, AWAY), cut(arm, [0.0, 0.0, 1.0])]),
        inspect(BODY, 2),
        inspect(WEAPON, 2),
    ]);
    let (_, moved) = harness.render(&view);
    assert!(
        differing_pixels(&whole, &moved) > 50 * seam.max(1),
        "the pieces moved"
    );
    let facts = harness.renderer.take_animation_facts();
    let (body_cut, weapon_cut) = (bounds_of(&facts, BODY), bounds_of(&facts, WEAPON));
    assert!(
        body_cut.1[0] > body_whole.1[0] + 1.0,
        "the drawn bounds reach the head: {body_cut:?} from {body_whole:?}"
    );
    for axis in 0..3 {
        let shift = [0.0, 0.0, 1.0][axis];
        assert!(
            (weapon_cut.0[axis] - weapon_whole.0[axis] - shift).abs() < 1e-3,
            "the weapon follows its hand's piece: {weapon_cut:?} from {weapon_whole:?}"
        );
    }
    let (_, fresh) = harness.render_fresh(&view);
    assert_eq!(fresh, moved, "a fresh renderer draws the same pieces");

    // Without cuts the character is whole again.
    harness.apply(vec![reporting(Vec::new())]);
    let (_, restored) = harness.render(&view);
    assert_eq!(restored, whole);
}
