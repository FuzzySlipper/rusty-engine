//! The animated instances of the committed scene and the one evaluation of
//! each one's pose.
//!
//! [`AnimatedScene`] observes the renderer changes that say what an animated
//! instance plays (its creation, playback commands, pose controls, the
//! animation controllers projected onto it), each at the Engine time it
//! takes effect. When a call settles, [`AnimatedScene::settle`] evaluates
//! every pose that can have changed at the call's Engine time and returns
//! the changed ones as [`RenderDiff::SetEvaluatedPose`]: realizers draw
//! those and never evaluate a pose themselves. The Engine services read the
//! same evaluation for joint reads, ragdolls and severed parts.
//!
//! **Time.** Poses sample the committed time a call reaches and hold
//! between calls. A playback command takes effect at the time its call
//! starts.
//!
//! **Cost.** A pose is evaluated only when it can change: while a clip
//! plays or fades, while a controller drives it, after a command, or when
//! the instance moves and its controls read its placement. An unchanged
//! pose is not sent again; a held one is only observed again at each call's
//! time and placement, so joint reads and spawns see the last call's motion.
//!
//! **Controllers** (`PresentationOp::Animation`) publish the controller's
//! state with per-clip phases each update; a controlled instance samples
//! `frozen_pose` of that state, each clip advanced by its speed over the
//! Engine time since the update. A controller replaces direct playback on
//! its target until destroyed (the first by handle, if several target it).
//!
//! **Placement.** An instance's placement is composed through the retained
//! world ([`PresentationWorld::node_link`]); a node that hangs from an
//! animated parent's joint is placed at that joint as this evaluation left
//! it, so parents are evaluated before their children.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use glam::{Mat4, Vec3};
use render_model::{
    AnimatedMeshAsset, AnimatedMeshPlaybackCommand, AnimatedMeshPlaybackTimeline, AnimatedMeshPose,
    AnimationLoopMode, ClipAction, EvaluatedIkChain, EvaluatedPiece, EvaluatedPose, IkChainReach,
    JointCut, PlaybackState, PosedJoint, RenderDiff, RenderFrameDiff, RenderHandle, Transform,
};
use render_presentation::{
    AnimationProjectionOp, PresentationFrameDiff, PresentationOp, PresentationWorld,
};

use crate::{
    posed_joints, transform_matrix, transform_of, ControllerPlayback, Hierarchy, PoseModel,
};

/// The animated instances of the committed scene: what each plays, and
/// its pose as last evaluated.
#[derive(Clone, Default)]
pub struct AnimatedScene {
    instances: BTreeMap<RenderHandle, Instance>,
    /// Each animation controller's target and projected state, by
    /// projection handle.
    controllers: BTreeMap<u64, (RenderHandle, ControllerPlayback)>,
    /// The controllers driving each target; the first poses it.
    driven: BTreeMap<RenderHandle, BTreeSet<u64>>,
    /// The animated mesh assets defined, by id.
    assets: BTreeMap<String, Arc<AnimatedMeshAsset>>,
    /// Each source entity's realization generation: counts the instances
    /// created for it.
    generations: BTreeMap<u64, u64>,
}

#[derive(Clone)]
struct Instance {
    asset: String,
    /// Its parent node, when that is an animated instance too.
    parent: Option<RenderHandle>,
    object_id: Option<u64>,
    generation: u64,
    /// The decoded mesh, once [`AnimatedScene::settle`] resolved it.
    model: Option<Arc<PoseModel>>,
    playback: PlaybackState,
    /// The `once` clip whose end is reported, until any new command.
    completion: Option<Completing>,
    /// The product's pose controls, clip layers, ragdoll layers and cuts.
    controls: AnimatedMeshPose,
    /// A command, control or model changed since the last evaluation.
    dirty: bool,
    /// The Engine time and placement the pose was last evaluated at.
    posed_at: f64,
    posed_world: Mat4,
    /// Each node's cut, while the controls cut the instance.
    owners: Vec<Option<usize>>,
    /// The pose last evaluated, and the one before it, each with the time
    /// and placement it was evaluated at.
    latest: Option<Evaluation>,
    previous: Option<Evaluation>,
}

/// A `once` clip whose end is reported: started `offset` seconds in, so a
/// clip started at or past its end reports nothing.
#[derive(Clone)]
struct Completing {
    clip: String,
    offset: f64,
}

#[derive(Clone)]
struct Evaluation {
    seconds: f64,
    world: Mat4,
    pose: EvaluatedPose,
    /// The nodes before the placed layers (a ragdoll's), when there are any.
    unlayered: Option<Arc<[[f32; 16]]>>,
}

/// What one settle produced.
#[derive(Debug, Default)]
pub struct Settled {
    /// The changed poses, for realizers ([`RenderDiff::SetEvaluatedPose`]).
    pub poses: RenderFrameDiff,
    /// `once` clips that played to their end, in evaluation order.
    pub completions: Vec<Completion>,
}

/// A `once` clip of a source entity's instance played to its end.
#[derive(Debug, Clone, PartialEq)]
pub struct Completion {
    pub object_id: u64,
    pub generation: u64,
    pub clip: String,
}

/// One evaluation of an instance's rig joints.
#[derive(Debug, Clone, PartialEq)]
pub struct PosedJoints {
    /// The Engine time it was evaluated at.
    pub seconds: f64,
    /// The instance's placement.
    pub world: Transform,
    /// Each rig joint, in rig order.
    pub joints: Vec<PosedJoint>,
}

/// An instance's evaluated joints, as joint reads, ragdolls and severed
/// parts read them.
#[derive(Debug, Clone, PartialEq)]
pub struct JointReading {
    pub latest: PosedJoints,
    /// `latest`'s joints before its placed layers (a ragdoll's), when it
    /// has any: the pose a driven ragdoll aims for.
    pub unlayered: Option<Vec<PosedJoint>>,
    /// The evaluation before `latest`, of the same instance, at an earlier
    /// time.
    pub previous: Option<PosedJoints>,
    /// The rig at rest, placed as `latest` is.
    pub rest: Vec<PosedJoint>,
    /// Each two-bone IK chain of `latest`, in the controls' order.
    pub ik_chains: Vec<IkChainReach>,
}

impl AnimatedScene {
    /// Observes a renderer frame's changes, which take effect at `now`.
    pub fn observe_frame(&mut self, frame: &RenderFrameDiff, now: f64) {
        for op in &frame.ops {
            match op {
                RenderDiff::DefineAnimatedMesh { asset } => {
                    self.assets
                        .insert(asset.asset.clone(), Arc::new(asset.clone()));
                    self.unresolve(&asset.asset);
                }
                RenderDiff::ReleaseAnimatedMesh { asset } => {
                    self.assets.remove(asset);
                    self.unresolve(asset);
                }
                RenderDiff::CreateAnimatedMeshInstance {
                    handle,
                    parent,
                    instance,
                } => {
                    self.forget(*handle);
                    let object_id = instance.metadata.source_entity;
                    let generation = object_id.map_or(0, |object| {
                        let next = self.generations.entry(object).or_insert(0);
                        *next += 1;
                        *next
                    });
                    let mut created = Instance {
                        asset: instance.asset.clone(),
                        parent: parent.filter(|parent| self.instances.contains_key(parent)),
                        object_id,
                        generation,
                        model: None,
                        playback: PlaybackState::default(),
                        completion: None,
                        controls: AnimatedMeshPose::default(),
                        dirty: true,
                        posed_at: f64::NAN,
                        posed_world: Mat4::IDENTITY,
                        owners: Vec::new(),
                        latest: None,
                        previous: None,
                    };
                    if let Some(playback) = &instance.playback {
                        created.command(playback, now);
                    }
                    // A fade a retained baseline carries keeps its own start.
                    if let Some(fade) = &instance.playback_fade {
                        created.playback.fade = Some(fade.clone());
                    }
                    self.instances.insert(*handle, created);
                }
                RenderDiff::SetAnimatedMeshPlayback { handle, playback } => {
                    if let Some(instance) = self.instances.get_mut(handle) {
                        instance.command(playback, now);
                    }
                }
                RenderDiff::SetAnimatedMeshPose { handle, pose } => {
                    if let Some(instance) = self.instances.get_mut(handle) {
                        if &instance.controls != pose {
                            instance.controls = pose.clone();
                            instance.dirty = true;
                        }
                    }
                }
                RenderDiff::Destroy { handle } => self.forget(*handle),
                _ => {}
            }
        }
    }

    /// Observes a presentation frame's animation controllers, which take
    /// effect at `now`.
    pub fn observe_presentation(&mut self, frame: &PresentationFrameDiff, now: f64) {
        for op in &frame.ops {
            let PresentationOp::Animation { op, .. } = op else {
                continue;
            };
            let target = match op {
                AnimationProjectionOp::Create { handle, descriptor } => {
                    // A baseline's state took effect when it was received; a
                    // live one takes effect now.
                    let playback = ControllerPlayback::new(
                        descriptor.controller.clone(),
                        descriptor.received_seconds.unwrap_or(now),
                    );
                    // A replaced controller's former target poses without it.
                    if let Some((replaced, _)) = self
                        .controllers
                        .insert(handle.raw(), (descriptor.target, playback))
                    {
                        self.undrive(replaced, handle.raw());
                        self.touch(replaced);
                    }
                    self.driven
                        .entry(descriptor.target)
                        .or_default()
                        .insert(handle.raw());
                    descriptor.target
                }
                AnimationProjectionOp::Update { handle, controller } => {
                    let Some((target, playback)) = self.controllers.get_mut(&handle.raw()) else {
                        continue;
                    };
                    *playback = ControllerPlayback::new(controller.clone(), now);
                    *target
                }
                AnimationProjectionOp::Destroy { handle } => {
                    let Some((target, _)) = self.controllers.remove(&handle.raw()) else {
                        continue;
                    };
                    self.undrive(target, handle.raw());
                    target
                }
            };
            self.touch(target);
        }
    }

    /// Evaluates every pose that can have changed at `now`, the Engine time
    /// the settled call reached, with each instance placed in `world` (the
    /// committed world with the call's changes). `model` decodes an asset's
    /// mesh; an instance whose mesh it cannot decode is not posed.
    pub fn settle(
        &mut self,
        now: f64,
        world: &PresentationWorld,
        model: &dyn Fn(&AnimatedMeshAsset) -> Option<Arc<PoseModel>>,
    ) -> Settled {
        let mut settled = Settled::default();
        if self.instances.is_empty() {
            return settled;
        }
        // An instance whose node left the world (under a destroyed or
        // replaced ancestor) is gone.
        self.instances
            .retain(|handle, _| world.node_link(*handle).is_some_and(|link| link.animated));
        // Parents before children, so a child hanging from a parent's joint
        // is placed by the parent's new pose.
        let mut order: Vec<(usize, RenderHandle)> = self
            .instances
            .keys()
            .map(|handle| (depth(world, *handle), *handle))
            .collect();
        order.sort_unstable();
        let mut ops = Vec::new();
        for (_, handle) in order {
            let placement = self.placement(world, handle).unwrap_or(Mat4::IDENTITY);
            let Self {
                instances,
                controllers,
                driven,
                assets,
                ..
            } = self;
            let Some(instance) = instances.get_mut(&handle) else {
                continue;
            };
            if instance.model.is_none() {
                instance.model = assets.get(&instance.asset).and_then(|asset| model(asset));
            }
            let Some(decoded) = instance.model.as_deref() else {
                continue;
            };
            let driving = driven.get(&handle).into_iter().flatten();
            // A controller's clips bind to its target's mesh when either
            // changes, which leaves the target dirty.
            if instance.dirty {
                for key in driving.clone() {
                    if let Some((_, playback)) = controllers.get_mut(key) {
                        playback.resolve(&|clip| decoded.clip_index(clip));
                    }
                }
            }
            let controller = driving
                .clone()
                .next()
                .and_then(|key| controllers.get(key))
                .map(|(_, playback)| playback);
            if let Some(pose) = instance.evaluate(now, placement, controller, &mut settled) {
                ops.push(RenderDiff::SetEvaluatedPose { handle, pose });
            }
        }
        settled.poses = RenderFrameDiff::from_ops(ops);
        settled
    }

    /// The clip actions `handle` plays at `now`, resolved against `model`:
    /// the first controller driving it, or its direct playback, under any
    /// fade. A node that is not animated plays nothing.
    pub fn actions(&self, handle: RenderHandle, model: &PoseModel, now: f64) -> Vec<ClipAction> {
        let index = |clip: &str| model.clip_index(clip);
        let duration = |clip: usize| model.clip_duration(clip);
        let controller = self.controller_of(handle).map(|playback| {
            let mut playback = playback.clone();
            playback.resolve(&index);
            playback.actions(now, &duration)
        });
        let Some(instance) = self.instances.get(&handle) else {
            return controller.unwrap_or_default();
        };
        let mut state = instance.playback.clone();
        state.resolve(&index);
        state.actions(now, &duration, controller).0
    }

    /// `handle` posed by `clip` at `normalized_time` (a fraction of the
    /// clip; a clip the mesh does not play leaves it at rest) under its own
    /// controls, placed as `world` places it: an image of it in that pose.
    /// `None` for a node that is not an animated instance whose mesh a
    /// settle decoded.
    pub fn sample_clip(
        &self,
        handle: RenderHandle,
        clip: &str,
        normalized_time: f32,
        world: &PresentationWorld,
    ) -> Option<EvaluatedPose> {
        let instance = self.instances.get(&handle)?;
        let model = instance.model.as_deref()?;
        let actions: Vec<ClipAction> = model
            .clip_index(clip)
            .map(|index| (index, model.clip_duration(index) * normalized_time, 1.0))
            .into_iter()
            .collect();
        let placement = self.placement(world, handle).unwrap_or(Mat4::IDENTITY);
        let (nodes, pieces, ik, _) = evaluated(model, &actions, &instance.controls, placement);
        Some(EvaluatedPose {
            seconds: world.elapsed_seconds(),
            nodes: nodes.into(),
            pieces,
            ik,
        })
    }

    /// `handle`'s joints as last evaluated, with the evaluation before them
    /// and the rest pose; `None` before its first evaluation.
    pub fn joints(&self, handle: RenderHandle) -> Option<JointReading> {
        let instance = self.instances.get(&handle)?;
        let model = instance.model.as_deref()?;
        let latest = instance.latest.as_ref()?;
        let posed = |evaluation: &Evaluation| PosedJoints {
            seconds: evaluation.seconds,
            world: transform_of(evaluation.world),
            joints: posed_joints(
                &model.joints,
                &matrices(&evaluation.pose.nodes),
                evaluation.world,
            ),
        };
        let world = latest.world;
        let ik_chains = latest
            .pose
            .ik
            .iter()
            .map(|chain| {
                let target = Vec3::from_array(chain.target);
                let end = chain
                    .end
                    .and_then(|node| latest.pose.nodes.get(node as usize))
                    .map_or(Vec3::ZERO, |end| {
                        Mat4::from_cols_array(end).w_axis.truncate()
                    });
                let (target_world, end_world) =
                    (world.transform_point3(target), world.transform_point3(end));
                IkChainReach {
                    target_model: chain.target,
                    target_world: target_world.to_array(),
                    end_model: end.to_array(),
                    end_world: end_world.to_array(),
                    shortfall: end_world.distance(target_world),
                    straightened: chain.straightened,
                }
            })
            .collect();
        Some(JointReading {
            latest: posed(latest),
            unlayered: latest
                .unlayered
                .as_ref()
                .map(|nodes| posed_joints(&model.joints, &matrices(nodes), latest.world)),
            previous: instance.previous.as_ref().map(posed),
            rest: posed_joints(&model.joints, &model.rest_nodes(), world),
            ik_chains,
        })
    }

    /// Drops what `asset` was decoded into: its instances decode it again,
    /// and re-pose, at the next settle.
    fn unresolve(&mut self, asset: &str) {
        for instance in self.instances.values_mut() {
            if instance.asset == asset {
                instance.model = None;
                instance.dirty = true;
            }
        }
    }

    /// Drops a destroyed instance and the animated instances under it.
    fn forget(&mut self, handle: RenderHandle) {
        let mut gone = vec![handle];
        while let Some(handle) = gone.pop() {
            self.instances.remove(&handle);
            gone.extend(
                self.instances
                    .iter()
                    .filter(|(_, instance)| instance.parent == Some(handle))
                    .map(|(child, _)| *child),
            );
        }
    }

    fn touch(&mut self, handle: RenderHandle) {
        if let Some(instance) = self.instances.get_mut(&handle) {
            instance.dirty = true;
        }
    }

    /// The first of the controllers driving `handle`, if any.
    fn controller_of(&self, handle: RenderHandle) -> Option<&ControllerPlayback> {
        let key = self.driven.get(&handle)?.first()?;
        self.controllers.get(key).map(|(_, playback)| playback)
    }

    fn undrive(&mut self, target: RenderHandle, controller: u64) {
        if let Some(controllers) = self.driven.get_mut(&target) {
            controllers.remove(&controller);
            if controllers.is_empty() {
                self.driven.remove(&target);
            }
        }
    }

    /// `handle`'s placement in the scene, composed through its parents, at
    /// the joint of any animated parent it hangs from.
    fn placement(&self, world: &PresentationWorld, handle: RenderHandle) -> Option<Mat4> {
        let mut placed = Mat4::IDENTITY;
        let mut cursor = handle;
        loop {
            let link = world.node_link(cursor)?;
            let local = link.transform.map_or(Mat4::IDENTITY, transform_matrix);
            let joint = match (link.parent, link.parent_joint) {
                (Some(parent), Some(joint)) => self.joint(parent, joint),
                _ => None,
            };
            placed = joint.unwrap_or(Mat4::IDENTITY) * local * placed;
            match link.parent {
                Some(parent) => cursor = parent,
                None => return Some(placed),
            }
        }
    }

    /// A named joint of an animated instance as last evaluated, in the
    /// instance's space: where a child attached to it hangs.
    fn joint(&self, handle: RenderHandle, joint: &str) -> Option<Mat4> {
        let instance = self.instances.get(&handle)?;
        let node = *instance.model.as_ref()?.attach.get(joint)?;
        let pose = &instance.latest.as_ref()?.pose;
        let piece = instance
            .owners
            .get(node)
            .copied()
            .flatten()
            .and_then(|cut| pose.pieces.get(cut));
        let nodes = piece.map_or(&pose.nodes, |piece| &piece.nodes);
        nodes.get(node).map(Mat4::from_cols_array)
    }
}

impl Instance {
    /// Applies a playback command that takes effect at `now`. A refused
    /// command leaves the playback as it was.
    fn command(&mut self, command: &AnimatedMeshPlaybackCommand, now: f64) {
        self.completion = None;
        self.dirty = true;
        if self.playback.apply(command, now).is_err() {
            return;
        }
        match command {
            AnimatedMeshPlaybackCommand::Play {
                clip,
                r#loop: AnimationLoopMode::Once,
                start_offset_seconds,
                start_paused: false,
                ..
            } => {
                self.completion = Some(Completing {
                    clip: clip.clone(),
                    offset: start_offset_seconds.unwrap_or(0.0),
                });
            }
            AnimatedMeshPlaybackCommand::Resume => {
                if let AnimatedMeshPlaybackTimeline::Playing {
                    clip,
                    r#loop: AnimationLoopMode::Once,
                    ..
                } = &self.playback.playback.timeline
                {
                    self.completion = Some(Completing {
                        clip: clip.clone(),
                        offset: 0.0,
                    });
                }
            }
            _ => {}
        }
    }

    /// Whether the pose moves with time.
    fn varies(&self) -> bool {
        if self.playback.fade.is_some() {
            return true;
        }
        match &self.playback.playback.timeline {
            // A `once` clip settles once its completion is reported.
            AnimatedMeshPlaybackTimeline::Playing { r#loop, .. } => {
                *r#loop != AnimationLoopMode::Once || self.completion.is_some()
            }
            _ => false,
        }
    }

    /// Evaluates the pose at `now` when it can have changed, and records the
    /// placement when the instance moved. Returns a pose that changed.
    fn evaluate(
        &mut self,
        now: f64,
        world: Mat4,
        controller: Option<&ControllerPlayback>,
        settled: &mut Settled,
    ) -> Option<EvaluatedPose> {
        let model = Arc::clone(self.model.as_ref()?);
        let index = |clip: &str| model.clip_index(clip);
        let duration = |clip: usize| model.clip_duration(clip);
        if self.dirty {
            self.playback.resolve(&index);
            // A `once` clip started at or past its end reports nothing.
            if let Some(completing) = &self.completion {
                let ended = model
                    .clip_index(&completing.clip)
                    .is_some_and(|clip| completing.offset >= f64::from(duration(clip)));
                if ended {
                    self.completion = None;
                }
            }
        }
        let moved = world != self.posed_world;
        let needed = self.dirty
            || (moved && self.controls.reads_world())
            || ((controller.is_some() || self.varies()) && self.posed_at != now);
        if !needed {
            // A held pose is observed again at this call's time and where the
            // instance now is, with the same pose: its joints read in the
            // scene follow its placement, and what spawns from it moves as
            // the last call moved it, not as an earlier one did.
            let observed = self
                .latest
                .as_ref()
                .is_some_and(|latest| moved || latest.seconds < now);
            if observed {
                if let Some(latest) = self.latest.clone() {
                    self.record(Evaluation {
                        seconds: now,
                        world,
                        pose: latest.pose,
                        unlayered: latest.unlayered,
                    });
                }
            }
            self.posed_world = world;
            return None;
        }
        let (actions, finished) = self.playback.actions(
            now,
            &duration,
            controller.map(|controller| controller.actions(now, &duration)),
        );
        let (nodes, pieces, ik, unlayered) = evaluated(&model, &actions, &self.controls, world);
        self.owners = if self.controls.cuts.is_empty() {
            Vec::new()
        } else {
            let joints: Vec<u32> = self.controls.cuts.iter().map(|cut| cut.joint).collect();
            node_cuts(&model.rig, &model.joints, &joints)
        };
        self.dirty = false;
        self.posed_at = now;
        self.posed_world = world;
        if finished {
            if let (Some(completing), Some(object_id)) = (self.completion.take(), self.object_id) {
                settled.completions.push(Completion {
                    object_id,
                    generation: self.generation,
                    clip: completing.clip,
                });
            }
        }
        let unchanged = self.latest.as_ref().is_some_and(|latest| {
            *latest.pose.nodes == *nodes && latest.pose.pieces == pieces && latest.pose.ik == ik
        });
        let pose = match (&self.latest, unchanged) {
            // An unchanged pose keeps its first evaluation's time and
            // storage, and is not sent again.
            (Some(latest), true) => latest.pose.clone(),
            _ => EvaluatedPose {
                seconds: now,
                nodes: nodes.into(),
                pieces,
                ik,
            },
        };
        self.record(Evaluation {
            seconds: now,
            world,
            pose: pose.clone(),
            unlayered: unlayered.map(Into::into),
        });
        (!unchanged).then_some(pose)
    }

    /// Makes `evaluation` the latest; the one it replaces becomes the
    /// previous when it is earlier.
    fn record(&mut self, evaluation: Evaluation) {
        let earlier = self.latest.replace(evaluation);
        if let Some(earlier) = earlier {
            if self
                .latest
                .as_ref()
                .is_some_and(|latest| earlier.seconds < latest.seconds)
            {
                self.previous = Some(earlier);
            }
        }
    }
}

/// Nodes, severed pieces, IK chains, and the nodes before placed layers.
type Evaluated = (
    Vec<[f32; 16]>,
    Vec<EvaluatedPiece>,
    Vec<EvaluatedIkChain>,
    Option<Vec<[f32; 16]>>,
);

/// Every node of `model` posed by `actions` and `controls` with the
/// instance placed by `world`, column-major in the instance's space, with
/// the severed pieces of the controls' cuts, the IK chains and, when the
/// controls place layers, the nodes before them.
fn evaluated(
    model: &PoseModel,
    actions: &[ClipAction],
    controls: &AnimatedMeshPose,
    world: Mat4,
) -> Evaluated {
    let mut solves = Vec::new();
    let mut unlayered = Vec::new();
    let nodes = model.evaluate(
        actions,
        controls,
        world,
        Some(&mut solves),
        Some(&mut unlayered),
    );
    let pieces = if controls.cuts.is_empty() {
        Vec::new()
    } else {
        let rig = rig_indices(&model.joints);
        let world_inverse = world.inverse();
        controls
            .cuts
            .iter()
            .map(|cut| EvaluatedPiece {
                joint: cut.joint,
                nodes: (0..nodes.len())
                    .map(|node| {
                        piece_matrix(&model.rig, &nodes, &rig, cut, world_inverse, node)
                            .to_cols_array()
                    })
                    .collect(),
            })
            .collect()
    };
    let ik = solves
        .iter()
        .map(|solve| EvaluatedIkChain {
            target: solve.target.to_array(),
            end: solve.end.map(|node| node as u32),
            straightened: solve.straightened,
        })
        .collect();
    let nodes = nodes.iter().map(Mat4::to_cols_array).collect();
    let unlayered =
        (!controls.layers.is_empty()).then(|| unlayered.iter().map(Mat4::to_cols_array).collect());
    (nodes, pieces, ik, unlayered)
}

/// How many nodes `handle` hangs under.
fn depth(world: &PresentationWorld, handle: RenderHandle) -> usize {
    let mut depth = 0;
    let mut cursor = world.node_link(handle).and_then(|link| link.parent);
    while let Some(parent) = cursor {
        depth += 1;
        cursor = world.node_link(parent).and_then(|link| link.parent);
    }
    depth
}

fn matrices(nodes: &[[f32; 16]]) -> Vec<Mat4> {
    nodes.iter().map(Mat4::from_cols_array).collect()
}

/// Each node's cut: the nearest cut joint (`joints`, rig joint indices) at
/// or above it. `rig` maps rig joints to nodes.
pub fn node_cuts<H: Hierarchy + ?Sized>(
    model: &H,
    rig: &[Option<usize>],
    joints: &[u32],
) -> Vec<Option<usize>> {
    let mut cut_at: BTreeMap<usize, usize> = BTreeMap::new();
    for (cut, joint) in joints.iter().enumerate() {
        if let Some(node) = rig.get(*joint as usize).copied().flatten() {
            cut_at.entry(node).or_insert(cut);
        }
    }
    let mut owners = vec![None; model.node_count()];
    for &node in model.order() {
        owners[node] = cut_at
            .get(&node)
            .copied()
            .or_else(|| model.parent(node).and_then(|parent| owners[parent]));
    }
    owners
}

/// Each rig joint's index, by its node.
fn rig_indices(rig: &[Option<usize>]) -> BTreeMap<usize, usize> {
    rig.iter()
        .enumerate()
        .filter_map(|(index, node)| Some(((*node)?, index)))
        .collect()
}

/// Where a node is drawn as part of a severed piece, in the instance's space
/// (`world_inverse` undoes the instance's placement): its nearest rig joint
/// as it was at the cut, relative to the cut joint and placed where that
/// joint is now, and the node as it is now relative to that joint.
fn piece_matrix<H: Hierarchy + ?Sized>(
    model: &H,
    pose: &[Mat4],
    rig: &BTreeMap<usize, usize>,
    cut: &JointCut,
    world_inverse: Mat4,
    node: usize,
) -> Mat4 {
    let frozen = |joint: usize| cut.pose.get(joint).map(transform_matrix);
    let mut joint = node;
    while !rig.contains_key(&joint) {
        match model.parent(joint) {
            Some(parent) => joint = parent,
            None => break,
        }
    }
    let base = world_inverse
        * transform_matrix(&cut.placement)
        * frozen(cut.joint as usize)
            .unwrap_or(Mat4::IDENTITY)
            .inverse();
    let at_cut = rig
        .get(&joint)
        .and_then(|index| frozen(*index))
        .unwrap_or(Mat4::IDENTITY);
    let relative = match (pose.get(joint), pose.get(node)) {
        (Some(above), Some(posed)) if joint != node => above.inverse() * *posed,
        _ => Mat4::IDENTITY,
    };
    base * at_cut * relative
}
