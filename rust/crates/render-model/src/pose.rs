//! Product pose controls on one animated instance: two-bone IK constraints
//! and joint overrides, applied after clip sampling, and the pose the Engine
//! evaluated from them for realizers to draw ([`EvaluatedPose`]).
//!
//! Joints are indices into the admitted rig's joint list
//! ([`crate::AnimationRigSignature::joints`], sorted by joint id).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnimatedMeshPose {
    /// Clips laid over the playback's clip pose in order, before IK.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub clip_layers: Vec<ClipLayer>,
    /// Solved in order after the clip pose, before any override.
    #[serde(default)]
    pub two_bone_ik: Vec<TwoBoneIkConstraint>,
    /// Applied after IK, parents before children; one joint's overrides
    /// apply in list order.
    #[serde(default)]
    pub overrides: Vec<JointOverride>,
    /// Applied in order after the overrides, before the layers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub twists: Vec<JointTwist>,
    /// Applied in order after the twists, each mixed over the pose below
    /// it as one pose.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub layers: Vec<PlacedPoseLayer>,
    /// Joints whose subtree is cut off the instance and drawn apart as a
    /// severed piece. The skeleton is evaluated as without them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cuts: Vec<JointCut>,
}

/// A joint's subtree cut off its instance: the triangles weighted to the
/// joint and its descendants leave the skinned mesh and are drawn as one
/// rigid piece, posed as they were at the cut and placed by `placement`.
/// A triangle goes with the piece when two of its corners have most of their
/// weight there; the nearest cut above a joint takes it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct JointCut {
    pub joint: u32,
    /// Every rig joint at the moment of the cut, in the instance's space, in
    /// rig order.
    pub pose: Vec<crate::Transform>,
    /// Where the cut joint is now, in the scene: its rotation, translation
    /// and the scale it was drawn with at the cut.
    pub placement: crate::Transform,
}

/// One clip sampled at a time the product gives and laid over the pose
/// below it, by `weight` (0 to 1) times each joint's weight. A clip changes
/// only the joints and properties (translation, rotation, scale) it has
/// channels for. `Override` moves them toward the clip's values; `Additive`
/// adds the clip's change from its pose at `reference_time` (translation
/// added, rotation applied after the joint's, scale multiplied). `time` and
/// `reference_time` are fractions of the clip's duration when `normalized`,
/// otherwise seconds, clamped to the clip. Without `joint_weights` every
/// joint takes the layer's weight; with them, only the joints listed, each
/// by its own weight.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClipLayer {
    /// A clip descriptor id the mesh plays (its own or a clip pack's).
    pub clip: String,
    pub time: f32,
    #[serde(default)]
    pub normalized: bool,
    pub weight: f32,
    #[serde(default)]
    pub blend: ClipLayerBlend,
    #[serde(default)]
    pub reference_time: f32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub joint_weights: Vec<JointWeight>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ClipLayerBlend {
    #[default]
    Override,
    Additive,
}

/// One joint's weight in a [`ClipLayer`], 0 to 1.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct JointWeight {
    pub joint: u32,
    pub weight: f32,
}

/// The space a pose value is given in. `Model` is the animated instance's
/// own space (its glTF scene root); `World` is the scene's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PoseSpace {
    Local,
    Model,
    World,
}

/// One joint's rotation, translation and/or scale over the pose below it,
/// mixed in by `weight`. A `Local` value replaces the joint's local value, or
/// with `additive` composes onto it (rotation after the clip's, translation
/// added, scale multiplied). A `Model` or `World` value moves the joint
/// toward that placement and its descendants follow; the joint keeps its
/// evaluated scale. `scale` is the joint's uniform local scale in every
/// space, applied after the layers about the joint as drawn, its
/// descendants following: 0 collapses the joint's subtree into it. Each
/// override's weight is its own, and one joint's overrides apply in list
/// order across spaces.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct JointOverride {
    pub joint: u32,
    pub space: PoseSpace,
    #[serde(default)]
    pub additive: bool,
    /// `[x, y, z, w]`, normalized when applied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotation: Option<[f32; 4]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub translation: Option<[f32; 3]>,
    /// Uniform, 0 or more.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<f32>,
    pub weight: f32,
}

/// Rotates `root` and `mid` so `end` reaches `target`, bending in the plane
/// through `root`, `target` and `pole`. `mid` must descend from `root` and
/// `end` from `mid`; joints between them follow. A target beyond reach
/// straightens the chain toward it. `space` is `Model` or `World`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TwoBoneIkConstraint {
    pub root: u32,
    pub mid: u32,
    pub end: u32,
    pub space: PoseSpace,
    pub target: [f32; 3],
    pub pole: [f32; 3],
    pub weight: f32,
}

/// Turns `joint` until it has rolled `share` of how far `to` has rolled
/// relative to `from` since the clip pose, about the line from `from` to
/// `to`, after IK and overrides: a forearm twist joint taking part of the
/// hand's roll, so a rolled hand does not twist the whole turn into the
/// wrist. A turn inherited from a twist joint above it counts toward its
/// share, so twists apply parents first. The joint turns about its own
/// origin and its descendants follow, except `to` and its descendants, which
/// stay where IK and overrides put them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct JointTwist {
    pub joint: u32,
    pub from: u32,
    pub to: u32,
    pub share: f32,
}

/// An animated instance's pose as the Engine evaluated it once for every
/// realizer: each GLB node's transform in the instance's space, column-major,
/// in the mesh's node order. A realizer skins from it, places rigid parts
/// and joint-attached children by it, and never evaluates the pose again.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvaluatedPose {
    /// The Engine presentation time the pose was evaluated at.
    pub seconds: f64,
    pub nodes: std::sync::Arc<[[f32; 16]]>,
    /// One per cut ([`JointCut`]), in the pose controls' order: the piece
    /// the cut severs, every node placed as that piece draws it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pieces: Vec<EvaluatedPiece>,
    /// Each two-bone IK chain as its solve saw it, in the controls' order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ik: Vec<EvaluatedIkChain>,
}

/// A severed piece of an [`EvaluatedPose`]: the cut joint, and every node of
/// the mesh placed where the piece is (in the instance's space). A realizer
/// draws the nodes the cut owns, and the skinned triangles weighted to them,
/// by these.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvaluatedPiece {
    pub joint: u32,
    pub nodes: std::sync::Arc<[[f32; 16]]>,
}

/// One two-bone IK chain of an [`EvaluatedPose`]: the target in the
/// instance's space, the chain's end node, and whether the target lay out of
/// reach so the solve straightened the chain toward it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvaluatedIkChain {
    pub target: [f32; 3],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<u32>,
    pub straightened: bool,
}

/// One evaluated joint, in its instance's space and in the scene.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PosedJoint {
    pub model: crate::Transform,
    pub world: crate::Transform,
}

/// One two-bone IK chain of an evaluated pose, from the solve that ran: its
/// target, the end joint as drawn (after overrides and layers), and how far
/// the end is from the target in the scene.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IkChainReach {
    pub target_model: [f32; 3],
    pub target_world: [f32; 3],
    pub end_model: [f32; 3],
    pub end_world: [f32; 3],
    /// Scene distance from the drawn end joint to the target.
    pub shortfall: f32,
    /// The target lay beyond the chain's reach, so the solve straightened
    /// the chain toward it.
    pub straightened: bool,
}

/// A set of joints placed in the world together (a ragdoll's bones), mixed
/// over the pose below by one `weight`. The mix is of each placed joint's
/// transform relative to its parent, so the placed joints blend as one pose:
/// weight 0 leaves the pose below unchanged, and bones keep their length at
/// any weight. Joints the layer does not place keep their transform relative
/// to their parent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlacedPoseLayer {
    pub weight: f32,
    pub joints: Vec<PlacedJoint>,
}

/// A joint's world-space placement in a [`PlacedPoseLayer`].
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlacedJoint {
    pub joint: u32,
    /// `[x, y, z, w]`, normalized when applied.
    pub rotation: [f32; 4],
    pub translation: [f32; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimatedMeshPoseError {
    InvalidWeight,
    InvalidRotation,
    InvalidTranslation,
    /// A negative or non-finite joint scale.
    InvalidScale,
    /// `additive` outside `Local` space.
    AdditiveOutsideLocal,
    /// An IK constraint in `Local` space.
    IkSpace,
    /// IK joints that are not three distinct joints.
    IkJoints,
    /// A twist measured from a joint to itself, or with a non-finite share.
    InvalidTwist,
    InvalidTarget,
    InvalidTime,
}

impl AnimatedMeshPose {
    pub fn validate(&self) -> Result<(), AnimatedMeshPoseError> {
        let weight = |value: f32| value.is_finite() && (0.0..=1.0).contains(&value);
        for layer in &self.clip_layers {
            if !weight(layer.weight)
                || !layer.joint_weights.iter().all(|joint| weight(joint.weight))
            {
                return Err(AnimatedMeshPoseError::InvalidWeight);
            }
            if !layer.time.is_finite() || !layer.reference_time.is_finite() {
                return Err(AnimatedMeshPoseError::InvalidTime);
            }
        }
        for ik in &self.two_bone_ik {
            if !weight(ik.weight) {
                return Err(AnimatedMeshPoseError::InvalidWeight);
            }
            if ik.space == PoseSpace::Local {
                return Err(AnimatedMeshPoseError::IkSpace);
            }
            if ik.root == ik.mid || ik.mid == ik.end || ik.root == ik.end {
                return Err(AnimatedMeshPoseError::IkJoints);
            }
            if !ik
                .target
                .iter()
                .chain(&ik.pole)
                .all(|value| value.is_finite())
            {
                return Err(AnimatedMeshPoseError::InvalidTarget);
            }
        }
        for layer in &self.layers {
            if !weight(layer.weight) {
                return Err(AnimatedMeshPoseError::InvalidWeight);
            }
            for joint in &layer.joints {
                let length = joint
                    .rotation
                    .iter()
                    .map(|value| value * value)
                    .sum::<f32>();
                if !length.is_finite() || length < 1e-12 {
                    return Err(AnimatedMeshPoseError::InvalidRotation);
                }
                if !joint.translation.iter().all(|value| value.is_finite()) {
                    return Err(AnimatedMeshPoseError::InvalidTranslation);
                }
            }
        }
        if self
            .twists
            .iter()
            .any(|twist| twist.from == twist.to || !twist.share.is_finite())
        {
            return Err(AnimatedMeshPoseError::InvalidTwist);
        }
        for joint in &self.overrides {
            if !weight(joint.weight) {
                return Err(AnimatedMeshPoseError::InvalidWeight);
            }
            if joint.additive && joint.space != PoseSpace::Local {
                return Err(AnimatedMeshPoseError::AdditiveOutsideLocal);
            }
            if let Some(rotation) = joint.rotation {
                let length = rotation.iter().map(|value| value * value).sum::<f32>();
                if !length.is_finite() || length < 1e-12 {
                    return Err(AnimatedMeshPoseError::InvalidRotation);
                }
            }
            if joint
                .translation
                .is_some_and(|translation| !translation.iter().all(|value| value.is_finite()))
            {
                return Err(AnimatedMeshPoseError::InvalidTranslation);
            }
            if joint
                .scale
                .is_some_and(|scale| !scale.is_finite() || scale < 0.0)
            {
                return Err(AnimatedMeshPoseError::InvalidScale);
            }
        }
        Ok(())
    }

    /// Whether evaluating this pose reads the instance's world placement.
    pub fn reads_world(&self) -> bool {
        !self.layers.is_empty()
            || !self.cuts.is_empty()
            || self
                .two_bone_ik
                .iter()
                .any(|ik| ik.space == PoseSpace::World)
            || self
                .overrides
                .iter()
                .any(|joint| joint.space == PoseSpace::World)
    }

    /// Whether it changes the clip pose at all.
    pub fn controls(&self) -> bool {
        !self.two_bone_ik.is_empty()
            || !self.overrides.is_empty()
            || !self.twists.is_empty()
            || !self.layers.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn joint(space: PoseSpace, additive: bool) -> JointOverride {
        JointOverride {
            joint: 0,
            space,
            additive,
            rotation: Some([0.0, 0.0, 0.0, 1.0]),
            translation: None,
            scale: None,
            weight: 1.0,
        }
    }

    #[test]
    fn additive_is_local_only_and_ik_needs_a_placed_space() {
        let pose = |overrides, two_bone_ik| AnimatedMeshPose {
            twists: Vec::new(),
            clip_layers: Vec::new(),
            two_bone_ik,
            overrides,
            layers: Vec::new(),
            cuts: Vec::new(),
        };
        assert_eq!(
            pose(vec![joint(PoseSpace::Local, true)], vec![]).validate(),
            Ok(())
        );
        assert_eq!(
            pose(vec![joint(PoseSpace::World, true)], vec![]).validate(),
            Err(AnimatedMeshPoseError::AdditiveOutsideLocal)
        );
        let ik = TwoBoneIkConstraint {
            root: 0,
            mid: 1,
            end: 2,
            space: PoseSpace::Local,
            target: [0.0; 3],
            pole: [0.0, 0.0, 1.0],
            weight: 1.0,
        };
        assert_eq!(
            pose(vec![], vec![ik.clone()]).validate(),
            Err(AnimatedMeshPoseError::IkSpace)
        );
        let repeated = TwoBoneIkConstraint {
            space: PoseSpace::Model,
            end: 0,
            ..ik
        };
        assert_eq!(
            pose(vec![], vec![repeated]).validate(),
            Err(AnimatedMeshPoseError::IkJoints)
        );
    }
}
