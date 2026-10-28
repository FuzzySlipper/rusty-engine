//! The one evaluation of a scene's animated poses (`AnimatedScene`), on the
//! joint attachment fixture's character admitted as the runtime admits it:
//! what it evaluates is what `PoseModel` samples at simulation time, it
//! follows playback, controllers, controls and placement, and it evaluates
//! only when a pose can change.

use std::collections::HashMap;
use std::f32::consts::FRAC_1_SQRT_2;
use std::path::PathBuf;
use std::sync::Arc;

use animation_pose::{AnimatedScene, ClipSample, Completion, JointReading, PoseModel};
use asset_import::{import_animated_glb_asset, ImportContext, SourceUri};
use render_model::*;
use render_presentation::{
    AnimationControllerClipPhase, AnimationControllerProjectionState,
    AnimationProjectionDescriptor, AnimationProjectionHandle, AnimationProjectionOp,
    AnimationTransitionState, PresentationFrameDiff, PresentationOp, PresentationOpMeta,
    PresentationWorld, ResolvedAnimationMotion,
};

const BODY: u64 = 1;
const WEAPON: u64 = 2;
const BODY_GLB: &str = "csharp-joint-attachments/content/body.glb";

fn fixture(path: &str) -> Vec<u8> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures");
    std::fs::read(root.join(path)).unwrap_or_else(|error| panic!("read {path}: {error}"))
}

/// The committed scene as the Engine services keep it: the retained world,
/// its evaluated poses, and the resources the meshes decode from.
struct Scene {
    world: PresentationWorld,
    poses: AnimatedScene,
    resources: HashMap<String, Vec<u8>>,
    /// The poses each settle sent, by instance.
    sent: Vec<(u64, EvaluatedPose)>,
    completions: Vec<Completion>,
}

impl Scene {
    fn new() -> Self {
        Self {
            world: PresentationWorld::default(),
            poses: AnimatedScene::default(),
            resources: HashMap::new(),
            sent: Vec::new(),
            completions: Vec::new(),
        }
    }

    /// Admit a GLB and hold its runtime resource.
    fn admit(&mut self, path: &str) -> AnimatedMeshAsset {
        let bytes = fixture(path);
        let imported = import_animated_glb_asset(
            &SourceUri::RelativePath(path.to_owned()),
            &bytes,
            &ImportContext::default(),
        );
        let imported = imported
            .assets
            .unwrap_or_else(|| panic!("{path} is admitted: {:?}", imported.diagnostics));
        let hash = imported.animated_mesh.content_hash.clone().unwrap();
        self.resources.insert(
            format!(
                "animated-mesh-resource/{}",
                hash.trim_start_matches("sha256:")
            ),
            bytes,
        );
        imported.animated_mesh
    }

    /// One call's changes, taking effect at the time it starts, then the
    /// poses it changed, evaluated where it ends.
    fn call(&mut self, ops: Vec<RenderDiff>, advance: f64) {
        let frame = RenderFrameDiff::from_ops(ops);
        self.poses
            .observe_frame(&frame, self.world.elapsed_seconds());
        self.world.apply(frame).expect("the world admits the call");
        self.settle(advance);
    }

    fn presentation(&mut self, op: AnimationProjectionOp, advance: f64) {
        let frame = PresentationFrameDiff {
            ops: vec![PresentationOp::Animation {
                meta: PresentationOpMeta::new(0),
                op,
            }],
            ..PresentationFrameDiff::default()
        };
        self.poses
            .observe_presentation(&frame, self.world.elapsed_seconds());
        self.world
            .apply_presentation(frame)
            .expect("the world admits the call");
        self.settle(advance);
    }

    fn settle(&mut self, advance: f64) {
        self.world.advance_elapsed(advance);
        let resources = &self.resources;
        let settled = self
            .poses
            .settle(self.world.elapsed_seconds(), &self.world, &|asset| {
                decoded(resources, asset)
            });
        self.completions.extend(settled.completions);
        for op in &settled.poses.ops {
            if let RenderDiff::SetEvaluatedPose { handle, pose } = op {
                self.sent.push((handle.raw(), pose.clone()));
            }
        }
        self.world
            .apply(settled.poses)
            .expect("the world retains the poses");
    }

    fn joints(&self, handle: u64) -> JointReading {
        self.poses
            .joints(RenderHandle::new(handle))
            .expect("the instance is posed")
    }

    fn model(&self, asset: &AnimatedMeshAsset) -> Arc<PoseModel> {
        decoded(&self.resources, asset).expect("the mesh decodes")
    }
}

fn decoded(
    resources: &HashMap<String, Vec<u8>>,
    asset: &AnimatedMeshAsset,
) -> Option<Arc<PoseModel>> {
    let hash = asset.content_hash.as_deref()?;
    let bytes = resources.get(&format!(
        "animated-mesh-resource/{}",
        hash.trim_start_matches("sha256:")
    ))?;
    PoseModel::decode(asset, bytes, |identity| resources.get(identity))
        .ok()
        .map(Arc::new)
}

fn animated(
    handle: u64,
    parent: Option<u64>,
    asset: &AnimatedMeshAsset,
    playback: Option<AnimatedMeshPlaybackCommand>,
) -> RenderDiff {
    RenderDiff::CreateAnimatedMeshInstance {
        handle: RenderHandle::new(handle),
        parent: parent.map(RenderHandle::new),
        instance: AnimatedMeshInstanceDescriptor {
            playback_fade: None,
            inspection: AnimatedMeshInspection::default(),
            asset: asset.asset.clone(),
            transform: Transform::IDENTITY,
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

fn play(clip: &str, r#loop: AnimationLoopMode, fade: Option<f32>) -> AnimatedMeshPlaybackCommand {
    started(clip, r#loop, fade, None, false)
}

fn started(
    clip: &str,
    r#loop: AnimationLoopMode,
    fade: Option<f32>,
    offset: Option<f64>,
    paused: bool,
) -> AnimatedMeshPlaybackCommand {
    AnimatedMeshPlaybackCommand::Play {
        clip: clip.to_owned(),
        r#loop,
        speed: 1.0,
        weight: 1.0,
        restart: true,
        fade_seconds: fade,
        start_offset_seconds: offset,
        start_paused: paused,
    }
}

fn playback(handle: u64, command: AnimatedMeshPlaybackCommand) -> RenderDiff {
    RenderDiff::SetAnimatedMeshPlayback {
        handle: RenderHandle::new(handle),
        playback: command,
    }
}

fn controls(handle: u64, pose: AnimatedMeshPose) -> RenderDiff {
    RenderDiff::SetAnimatedMeshPose {
        handle: RenderHandle::new(handle),
        pose,
    }
}

fn move_to(handle: u64, x: f32) -> RenderDiff {
    RenderDiff::Update {
        handle: RenderHandle::new(handle),
        transform: Some(Transform {
            translation: [x, 0.0, 0.0],
            ..Transform::IDENTITY
        }),
        material: None,
        visible: None,
        metadata: None,
    }
}

/// The admitted rig's joint index of `name` (joints sorted by id).
fn rig_joint(asset: &AnimatedMeshAsset, name: &str) -> u32 {
    asset
        .rig
        .as_ref()
        .and_then(|rig| rig.joints.iter().position(|joint| joint.id == name))
        .unwrap_or_else(|| panic!("rig joint {name}")) as u32
}

fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f32>().sqrt()
}

fn assert_same(sampled: &[PosedJoint], evaluated: &[PosedJoint], case: &str) {
    assert_eq!(sampled.len(), evaluated.len(), "{case}");
    for (index, (sampled, evaluated)) in sampled.iter().zip(evaluated).enumerate() {
        for (a, b) in [
            (sampled.model.translation, evaluated.model.translation),
            (sampled.world.translation, evaluated.world.translation),
        ] {
            assert!(distance(a, b) < 1e-5, "{case}: joint {index}: {a:?} {b:?}");
        }
        let dot: f32 = (0..4)
            .map(|i| sampled.world.rotation[i] * evaluated.world.rotation[i])
            .sum();
        assert!(dot.abs() > 1.0 - 1e-5, "{case}: joint {index} rotation");
    }
}

/// A pose sampled at simulation time (`PoseModel`, as the Engine services
/// sample one) is the pose the scene evaluates and every realizer draws, for
/// the same clip time, controls and placement.
#[test]
fn the_evaluated_pose_is_the_pose_sampled_at_simulation_time() {
    let mut scene = Scene::new();
    let body = scene.admit(BODY_GLB);
    let finger = rig_joint(&body, "RightHandIndex1");
    let pose = AnimatedMeshPose {
        overrides: vec![JointOverride {
            joint: finger,
            space: PoseSpace::Local,
            additive: true,
            rotation: Some([0.0, 0.0, FRAC_1_SQRT_2, FRAC_1_SQRT_2]),
            translation: None,
            scale: None,
            weight: 0.5,
        }],
        ..AnimatedMeshPose::default()
    };
    scene.call(
        vec![
            RenderDiff::DefineAnimatedMesh {
                asset: body.clone(),
            },
            animated(BODY, None, &body, None),
        ],
        0.0,
    );
    let model = scene.model(&body);
    for (x, time) in [(2.0, 0.5), (-1.0, 0.8)] {
        scene.call(
            vec![
                move_to(BODY, x),
                playback(BODY, sample("run", time)),
                controls(BODY, pose.clone()),
            ],
            0.0,
        );
        let evaluated = scene.joints(BODY);
        assert_eq!(evaluated.latest.world.translation[0], x);
        let sampled = model
            .sample(
                &[ClipSample {
                    clip: "run",
                    time,
                    normalized: true,
                    weight: 1.0,
                }],
                &pose,
                &evaluated.latest.world,
            )
            .unwrap();
        assert_same(&sampled, &evaluated.latest.joints, "sampled run");
    }
}

/// What an instance plays now (`AnimatedScene::actions`, which the Engine
/// samples an instance's current playback with) is what the scene
/// evaluates: direct playback, a cross-fade and a controller in a
/// transition, each command in effect from the start of its call.
#[test]
fn current_playback_is_what_the_scene_evaluates() {
    let mut scene = Scene::new();
    let body = scene.admit(BODY_GLB);
    scene.call(
        vec![
            RenderDiff::DefineAnimatedMesh {
                asset: body.clone(),
            },
            animated(BODY, None, &body, Some(sample("run", 0.5))),
            move_to(BODY, 1.0),
        ],
        1.0,
    );
    let model = scene.model(&body);
    let check = |scene: &Scene, case: &str| {
        let now = scene.world.elapsed_seconds();
        let evaluated = scene.joints(BODY);
        assert_eq!(evaluated.latest.seconds, now, "{case}");
        let actions = scene.poses.actions(RenderHandle::new(BODY), &model, now);
        let sampled = model.sample_actions(
            &actions,
            &AnimatedMeshPose::default(),
            &evaluated.latest.world,
        );
        assert_same(&sampled, &evaluated.latest.joints, case);
        actions.len()
    };

    // Direct playback, from 1.0 to 1.4.
    scene.call(
        vec![playback(
            BODY,
            play("idle", AnimationLoopMode::Repeat, None),
        )],
        0.4,
    );
    assert_eq!(check(&scene, "playing"), 1);

    // A cross-fade to another clip, from 1.4, evaluated partway.
    scene.call(
        vec![playback(
            BODY,
            play("run", AnimationLoopMode::Repeat, Some(0.5)),
        )],
        0.2,
    );
    assert_eq!(
        check(&scene, "cross-fading"),
        2,
        "the prior clip fades under the new one"
    );

    // A controller halfway through a transition replaces direct playback
    // (the fade still under it), from 1.6 to 1.75.
    let motion = |clip: &str| ResolvedAnimationMotion {
        clip_a: clip.to_owned(),
        clip_b: None,
        blend_weight_milli: 0,
        speed_milli: 1250,
    };
    scene.presentation(
        AnimationProjectionOp::Create {
            handle: AnimationProjectionHandle::new(1),
            descriptor: AnimationProjectionDescriptor {
                received_seconds: None,
                target: RenderHandle::new(BODY),
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
                },
            },
        },
        0.15,
    );
    assert_eq!(
        check(&scene, "controlled"),
        3,
        "the fading idle under the controller's jump and idle"
    );
}

/// A clip pack that carries only the character's skeleton plays its clips
/// as clip layers over the playback: an override on the right arm at half
/// weight and an additive nod. The scene evaluates what `PoseModel` samples
/// for the same layers.
#[test]
fn clip_layers_from_a_skeleton_only_pack_evaluate_as_sampled() {
    let mut scene = Scene::new();
    let mut body = scene.admit(BODY_GLB);
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
    scene.resources.insert(
        format!("clip-pack-resource/{}", hash.trim_start_matches("sha256:")),
        pack_bytes,
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
    let pose = AnimatedMeshPose {
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
        ..AnimatedMeshPose::default()
    };
    scene.call(
        vec![
            RenderDiff::DefineAnimatedMesh {
                asset: body.clone(),
            },
            animated(BODY, None, &body, Some(sample("run", 0.5))),
            controls(BODY, pose.clone()),
        ],
        0.0,
    );
    let evaluated = scene.joints(BODY);
    let model = scene.model(&body);
    let run = [ClipSample {
        clip: "run",
        time: 0.5,
        normalized: true,
        weight: 1.0,
    }];
    let world = &evaluated.latest.world;
    let sampled = model.sample(&run, &pose, world).unwrap();
    let plain = model
        .sample(&run, &AnimatedMeshPose::default(), world)
        .unwrap();
    assert_same(&sampled, &evaluated.latest.joints, "layered");
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

/// A `once` clip reports its end once, when an evaluation finds it ended,
/// and then holds; one started at or past its end reports nothing; a resumed
/// one reports again.
#[test]
fn a_once_clip_completes_once_then_holds() {
    let mut scene = Scene::new();
    let body = scene.admit(BODY_GLB);
    scene.call(
        vec![
            RenderDiff::DefineAnimatedMesh {
                asset: body.clone(),
            },
            animated(BODY, None, &body, None),
        ],
        1.0,
    );
    // `run` lasts 0.667 s.
    scene.call(
        vec![playback(BODY, play("run", AnimationLoopMode::Once, None))],
        0.3,
    );
    assert!(scene.completions.is_empty());
    scene.call(Vec::new(), 0.4);
    assert_eq!(
        std::mem::take(&mut scene.completions),
        [Completion {
            object_id: BODY,
            generation: 1,
            clip: "run".to_owned(),
        }]
    );
    let sent = scene.sent.len();
    scene.call(Vec::new(), 1.0);
    assert!(scene.completions.is_empty());
    assert_eq!(scene.sent.len(), sent, "a finished clip holds its pose");

    scene.call(
        vec![playback(
            BODY,
            started("run", AnimationLoopMode::Once, None, Some(2.0), false),
        )],
        0.1,
    );
    scene.call(Vec::new(), 1.0);
    assert!(scene.completions.is_empty(), "started past its end");

    scene.call(
        vec![playback(
            BODY,
            started("run", AnimationLoopMode::Once, None, None, true),
        )],
        0.5,
    );
    assert!(scene.completions.is_empty(), "paused");
    scene.call(
        vec![playback(BODY, AnimatedMeshPlaybackCommand::Resume)],
        1.0,
    );
    assert_eq!(scene.completions.len(), 1, "resumed to its end");
}

/// A held pose is evaluated once and not sent again; a moved instance reads
/// where it is with the same pose, the evaluation before it kept for
/// velocities.
#[test]
fn a_held_pose_is_sent_once_and_a_moved_instance_reads_where_it_is() {
    let mut scene = Scene::new();
    let body = scene.admit(BODY_GLB);
    scene.call(
        vec![
            RenderDiff::DefineAnimatedMesh {
                asset: body.clone(),
            },
            RenderDiff::Create {
                handle: RenderHandle::new(10),
                parent: None,
                node: RenderNode::new(Geometry::Group),
            },
            animated(BODY, Some(10), &body, Some(sample("run", 0.5))),
        ],
        0.25,
    );
    assert_eq!(scene.sent.len(), 1, "created, so evaluated");
    let first = scene.joints(BODY);
    assert!(first.previous.is_none());
    scene.call(Vec::new(), 0.25);
    assert_eq!(scene.sent.len(), 1, "a sampled pose holds");
    let held = scene.joints(BODY);
    assert_eq!(
        held.latest.seconds, 0.5,
        "observed again at the call's time"
    );
    assert_eq!(held.latest.joints, first.latest.joints, "the same pose");

    // Moving the instance, then its parent: the pose is unchanged and not
    // sent, but its joints read where it now is.
    let mut placed = held;
    for (handle, x, world_x) in [(BODY, 3.0, 3.0), (10, 5.0, 8.0)] {
        scene.call(vec![move_to(handle, x)], 0.25);
        assert_eq!(scene.sent.len(), 1, "moving handle {handle} sends nothing");
        let reading = scene.joints(BODY);
        assert_eq!(reading.latest.world.translation[0], world_x);
        let previous = reading.previous.as_ref().expect("the evaluation before");
        assert_eq!(previous.seconds, placed.latest.seconds);
        let shift = world_x - placed.latest.world.translation[0];
        for (joint, before) in reading.latest.joints.iter().zip(&placed.latest.joints) {
            assert_eq!(joint.model, before.model, "the pose is unchanged");
            assert!(
                (joint.world.translation[0] - before.world.translation[0] - shift).abs() < 1e-4
            );
        }
        placed = reading;
    }

    // A repeating clip is evaluated as each call reaches a new time.
    scene.call(
        vec![playback(BODY, play("run", AnimationLoopMode::Repeat, None))],
        0.1,
    );
    scene.call(Vec::new(), 0.1);
    assert_eq!(scene.sent.len(), 3);
    let (seconds, poses): (Vec<f64>, Vec<&EvaluatedPose>) = scene.sent[1..]
        .iter()
        .map(|(_, pose)| (pose.seconds, pose))
        .unzip();
    assert!(seconds[1] > seconds[0]);
    assert_ne!(poses[0].nodes, poses[1].nodes, "the running pose moved");
}

/// The largest joint speed in the scene between an instance's two latest
/// observations: what a ragdoll or severed part spawned from it moves with.
fn joint_speed(reading: &JointReading) -> f32 {
    let previous = reading.previous.as_ref().expect("an observation before");
    let seconds = (reading.latest.seconds - previous.seconds) as f32;
    reading
        .latest
        .joints
        .iter()
        .zip(&previous.joints)
        .map(|(now, before)| distance(now.world.translation, before.world.translation) / seconds)
        .fold(0.0, f32::max)
}

/// A character moved and then held still spawns at rest: its last call is
/// what its joints moved by, not the move before (R10222-3).
#[test]
fn a_character_held_still_after_moving_reads_no_motion() {
    let mut scene = Scene::new();
    let body = scene.admit(BODY_GLB);
    scene.call(
        vec![
            RenderDiff::DefineAnimatedMesh {
                asset: body.clone(),
            },
            animated(BODY, None, &body, Some(sample("run", 0.5))),
        ],
        0.25,
    );
    scene.call(vec![move_to(BODY, 3.0)], 0.25);
    let moving = joint_speed(&scene.joints(BODY));
    assert!((moving - 12.0).abs() < 1e-3, "3 m in 0.25 s: {moving}");
    let sent = scene.sent.len();
    scene.call(Vec::new(), 0.25);
    let reading = scene.joints(BODY);
    assert_eq!(reading.latest.seconds, 0.75);
    assert!(joint_speed(&reading) < 1e-5, "{}", joint_speed(&reading));
    assert_eq!(scene.sent.len(), sent, "held, so not sent again");
}

/// A `once` clip that played to its end and holds its last pose reads no
/// motion from the call after it ended.
#[test]
fn a_finished_once_clip_reads_no_motion_while_it_holds() {
    let mut scene = Scene::new();
    let body = scene.admit(BODY_GLB);
    scene.call(
        vec![
            RenderDiff::DefineAnimatedMesh {
                asset: body.clone(),
            },
            animated(
                BODY,
                None,
                &body,
                Some(play("run", AnimationLoopMode::Once, None)),
            ),
        ],
        0.25,
    );
    scene.call(Vec::new(), 0.25);
    assert!(joint_speed(&scene.joints(BODY)) > 0.0, "running");
    // `run` lasts 0.667 s: it ends in this call.
    scene.call(Vec::new(), 0.5);
    assert_eq!(scene.completions.len(), 1);
    let sent = scene.sent.len();
    scene.call(Vec::new(), 0.25);
    let reading = scene.joints(BODY);
    assert_eq!(reading.latest.seconds, 1.25);
    assert!(joint_speed(&reading) < 1e-5, "{}", joint_speed(&reading));
    assert_eq!(scene.sent.len(), sent, "a finished clip holds its pose");
}

/// A child on a joint is placed at its animated parent's joint as this
/// settle evaluated it: the parent is evaluated first.
#[test]
fn a_child_on_a_joint_hangs_from_its_parents_evaluated_joint() {
    let mut scene = Scene::new();
    let body = scene.admit(BODY_GLB);
    let weapon = scene.admit("csharp-joint-attachments/content/weapon.glb");
    let hand = rig_joint(&body, "RightHand") as usize;
    scene.call(
        vec![
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
                Some(play("run", AnimationLoopMode::Repeat, None)),
            ),
            animated(WEAPON, Some(BODY), &weapon, None),
            RenderDiff::SetParentJoint {
                handle: RenderHandle::new(WEAPON),
                joint: Some("RightHand".to_owned()),
            },
            move_to(BODY, 2.0),
        ],
        0.0,
    );
    for advance in [0.2, 0.3] {
        scene.call(Vec::new(), advance);
        let hand = scene.joints(BODY).latest.joints[hand].world;
        let weapon = scene.joints(WEAPON).latest.world;
        assert!(
            distance(hand.translation, weapon.translation) < 1e-4,
            "the weapon {weapon:?} hangs from the hand {hand:?}"
        );
    }
}

/// A cut joint's subtree is placed as one piece: in place, its nodes are
/// where the pose has them; a child on a joint of the piece goes with it.
#[test]
fn a_cut_piece_carries_its_nodes_and_the_children_on_them() {
    let mut scene = Scene::new();
    let body = scene.admit(BODY_GLB);
    let weapon = scene.admit("csharp-joint-attachments/content/weapon.glb");
    let (arm, hand) = (rig_joint(&body, "RightArm"), rig_joint(&body, "RightHand"));
    scene.call(
        vec![
            RenderDiff::DefineAnimatedMesh {
                asset: body.clone(),
            },
            RenderDiff::DefineAnimatedMesh {
                asset: weapon.clone(),
            },
            animated(BODY, None, &body, Some(sample("run", 0.5))),
            animated(WEAPON, Some(BODY), &weapon, None),
            RenderDiff::SetParentJoint {
                handle: RenderHandle::new(WEAPON),
                joint: Some("RightHand".to_owned()),
            },
        ],
        0.0,
    );
    let whole = scene.joints(BODY).latest.joints;
    let held = scene.joints(WEAPON).latest.world;
    let cut = |offset: [f32; 3]| {
        let mut placement = whole[arm as usize].world;
        for (axis, delta) in offset.iter().enumerate() {
            placement.translation[axis] += delta;
        }
        controls(
            BODY,
            AnimatedMeshPose {
                cuts: vec![JointCut {
                    joint: arm,
                    pose: whole.iter().map(|joint| joint.model).collect(),
                    placement,
                }],
                ..AnimatedMeshPose::default()
            },
        )
    };
    scene.call(vec![cut([0.0; 3])], 0.0);
    let (_, posed) = scene.sent.last().expect("the cut is sent");
    assert_eq!(posed.pieces.len(), 1);
    let piece = &posed.pieces[0];
    assert_eq!(piece.joint, arm);
    // The piece in place: the weapon stays where it was held.
    let in_place = scene.joints(WEAPON).latest.world;
    assert!(distance(in_place.translation, held.translation) < 1e-4);
    // The skeleton keeps its own pose.
    assert_eq!(
        scene.joints(BODY).latest.joints[hand as usize],
        whole[hand as usize]
    );

    // Moved, the weapon goes with the piece.
    scene.call(vec![cut([0.0, 0.0, 1.0])], 0.0);
    let moved = scene.joints(WEAPON).latest.world;
    assert!(
        distance(moved.translation, held.translation) > 0.99,
        "{moved:?} from {held:?}"
    );
    assert!((moved.translation[2] - held.translation[2] - 1.0).abs() < 1e-3);
}
