//! CPU pose evaluation for animated GLB meshes: clip sampling and blending,
//! pose controls (two-bone IK, joint overrides, placed layers) and
//! composition into instance-space node transforms.
//!
//! This is the one pose evaluator. The renderer poses what it draws through
//! it, and the Engine services sample poses at simulation time through it
//! ([`PoseModel`]) without a renderer, so a sampled pose and a drawn one of
//! the same clip time are the same pose. glam is this crate's private math;
//! callers outside the renderer read Engine transforms.

mod clip;
mod playback;
mod pose;
mod rig;

use std::collections::HashMap;

use render_model::{AnimatedMeshAsset, AnimatedMeshPose, PosedJoint, Transform};

pub use clip::{sample_locals, Channel, Clip, Interp, Path, Trs};
pub use playback::{wrap, ClipAction, ControllerPlayback, Fade, Playback, PlaybackState};
pub use pose::{compose, controlled, IkSolve};
pub use rig::{
    asset_clips, decode_clips, decode_nodes, joint_nodes, posed_joints, resource_identity,
    rig_nodes, transform_matrix, transform_of, Rig, RigNode,
};

/// A node hierarchy pose evaluation walks: each node's parent and rest
/// transform, and an order visiting parents before children.
pub trait Hierarchy {
    fn node_count(&self) -> usize;
    fn parent(&self, node: usize) -> Option<usize>;
    fn rest(&self, node: usize) -> Trs;
    fn name(&self, node: usize) -> Option<&str>;
    fn order(&self) -> &[usize];
    /// Every skin's joint nodes, skin by skin.
    fn skin_joints(&self) -> Vec<usize>;
}

/// One clip in a sample: its descriptor id, its time in seconds (or as a
/// fraction of its duration, when `normalized`) and its weight.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClipSample<'a> {
    pub clip: &'a str,
    pub time: f32,
    pub normalized: bool,
    pub weight: f32,
}

/// An animated mesh asset ready to pose without a renderer: its rig, the
/// clips it plays by descriptor id and its rig joints' nodes.
#[derive(Debug)]
pub struct PoseModel {
    rig: Rig,
    clips: Vec<Clip>,
    clip_ids: HashMap<String, usize>,
    joints: Vec<Option<usize>>,
}

impl PoseModel {
    /// Decodes `asset` from its GLB (`bytes`) and its clip packs
    /// (`pack_bytes`, by resource identity).
    pub fn decode<B: AsRef<[u8]>>(
        asset: &AnimatedMeshAsset,
        bytes: &[u8],
        pack_bytes: impl FnMut(&str) -> Option<B>,
    ) -> Result<Self, String> {
        let mut rig = Rig::decode(bytes)?;
        let own = std::mem::take(&mut rig.clips);
        let by_id = asset_clips(asset, &rig, own, pack_bytes)?;
        let joints = rig_nodes(asset, &rig);
        let mut clip_ids = HashMap::with_capacity(by_id.len());
        let clips = by_id
            .into_iter()
            .enumerate()
            .map(|(index, (id, clip))| {
                clip_ids.insert(id, index);
                clip
            })
            .collect();
        Ok(Self {
            rig,
            clips,
            clip_ids,
            joints,
        })
    }

    /// A clip's index in this model's clip table, by descriptor id.
    pub fn clip_index(&self, clip: &str) -> Option<usize> {
        self.clip_ids.get(clip).copied()
    }

    /// A clip's duration in seconds, by index.
    pub fn clip_duration(&self, clip: usize) -> f32 {
        self.clips[clip].duration
    }

    /// The rig joints posed by `samples` (blended over the rest pose) and
    /// then `controls`, with the instance placed by `world`: per joint in rig
    /// order, in the instance's space and in the scene, as the renderer
    /// reports a pose it draws. A sample naming an unknown clip is refused.
    pub fn sample(
        &self,
        samples: &[ClipSample<'_>],
        controls: &AnimatedMeshPose,
        world: &Transform,
    ) -> Result<Vec<PosedJoint>, String> {
        let actions = samples
            .iter()
            .map(|sample| {
                let index = self
                    .clip_index(sample.clip)
                    .ok_or_else(|| format!("the mesh plays no clip {:?}", sample.clip))?;
                let time = if sample.normalized {
                    self.clips[index].duration * sample.time
                } else {
                    sample.time
                };
                Ok((index, time, sample.weight))
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(self.sample_actions(&actions, controls, world))
    }

    /// The rig joints posed by clip actions (`(clip index, seconds,
    /// weight)`, as [`PlaybackState::actions`] gives them), then `controls`,
    /// placed by `world`.
    pub fn sample_actions(
        &self,
        actions: &[ClipAction],
        controls: &AnimatedMeshPose,
        world: &Transform,
    ) -> Vec<PosedJoint> {
        let world = transform_matrix(world);
        let locals = sample_locals(&self.rig, &self.clips, actions);
        let pose = controlled(&self.rig, &self.joints, locals, controls, world, None);
        posed_joints(&self.joints, &pose, world)
    }

    /// The rig joints at rest, placed by `world`.
    pub fn rest(&self, world: &Transform) -> Vec<PosedJoint> {
        let locals: Vec<Trs> = self.rig.nodes.iter().map(|node| node.rest).collect();
        posed_joints(
            &self.joints,
            &compose(&self.rig, &locals),
            transform_matrix(world),
        )
    }
}
