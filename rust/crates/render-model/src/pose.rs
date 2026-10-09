//! Product pose controls on one animated instance: two-bone IK constraints
//! and joint overrides, applied by the renderer after clip sampling and
//! before skinning, and whether the evaluated joints are reported back.
//!
//! Joints are indices into the admitted rig's joint list
//! ([`crate::AnimationRigSignature::joints`], sorted by joint id).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnimatedMeshPose {
    /// Solved in order after the clip pose, before any override.
    #[serde(default)]
    pub two_bone_ik: Vec<TwoBoneIkConstraint>,
    /// Applied after IK, parents before children; one joint's overrides
    /// apply in list order.
    #[serde(default)]
    pub overrides: Vec<JointOverride>,
    /// Report the evaluated joints after each committed call.
    #[serde(default)]
    pub report_joints: bool,
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

/// One joint's rotation and/or translation over the pose below it, mixed in
/// by `weight`. A `Local` value replaces the joint's local value, or with
/// `additive` composes onto it (rotation after the clip's, translation
/// added). A `Model` or `World` value places the joint there and its
/// descendants follow; the joint keeps its evaluated scale.
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

/// One evaluated joint, in its instance's space and in the scene.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PosedJoint {
    pub model: crate::Transform,
    pub world: crate::Transform,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimatedMeshPoseError {
    InvalidWeight,
    InvalidRotation,
    InvalidTranslation,
    /// `additive` outside `Local` space.
    AdditiveOutsideLocal,
    /// An IK constraint in `Local` space.
    IkSpace,
    /// IK joints that are not three distinct joints.
    IkJoints,
    InvalidTarget,
}

impl AnimatedMeshPose {
    pub fn validate(&self) -> Result<(), AnimatedMeshPoseError> {
        let weight = |value: f32| value.is_finite() && (0.0..=1.0).contains(&value);
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
        }
        Ok(())
    }

    /// Whether evaluating this pose reads the instance's world placement.
    pub fn reads_world(&self) -> bool {
        self.two_bone_ik
            .iter()
            .any(|ik| ik.space == PoseSpace::World)
            || self
                .overrides
                .iter()
                .any(|joint| joint.space == PoseSpace::World)
    }

    /// Whether it changes the clip pose at all.
    pub fn controls(&self) -> bool {
        !self.two_bone_ik.is_empty() || !self.overrides.is_empty()
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
            weight: 1.0,
        }
    }

    #[test]
    fn additive_is_local_only_and_ik_needs_a_placed_space() {
        let pose = |overrides, two_bone_ik| AnimatedMeshPose {
            two_bone_ik,
            overrides,
            report_joints: false,
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
