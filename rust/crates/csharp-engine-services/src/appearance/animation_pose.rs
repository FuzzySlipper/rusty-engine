//! Product pose controls on animation instances (`SetPose`), the admitted
//! rig's joints (`ReadJoints`), and the joints the renderer reports back
//! (`ReadJointPose`).
//!
//! A pose is retained on its instance and reaches the renderer when the call
//! settles, as one `SetAnimatedMeshPose` per changed instance or target. The
//! renderer stays the only pose evaluator: a reporting instance is posed
//! when each call is applied and its joints arrive before the next call.

use super::*;

/// The joints a renderer evaluated for one animated object.
#[derive(Debug, Clone, PartialEq)]
pub struct JointPoseReport {
    pub object_id: u64,
    pub generation: u64,
    /// The Engine time the pose was evaluated at.
    pub seconds: f64,
    pub world: Transform,
    /// Rig order (`ReadJoints`).
    pub joints: Vec<PosedJoint>,
    /// The rest pose, when the report carries it.
    pub rest: Option<Vec<PosedJoint>>,
}

/// The latest report of one object, with the rest pose its renderer last
/// reported.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ReportedJointPose {
    pub(crate) latest: JointPoseReport,
    /// The report before `latest`, of the same realization.
    pub(crate) previous: Option<JointPoseReport>,
    pub(crate) rest: Option<Vec<PosedJoint>>,
}

/// What a ragdoll spawns from: an instance's rig size and its latest
/// reports.
pub(crate) struct RagdollSource {
    pub(crate) joints: usize,
    pub(crate) rest: Vec<PosedJoint>,
    pub(crate) latest: JointPoseReport,
    pub(crate) previous: Option<JointPoseReport>,
}

const NO_PARENT: u32 = u32::MAX;

/// The admitted rig of an animated asset.
fn asset_rig<'a>(
    resources: &'a RenderResourceRegistry,
    asset: &str,
) -> Option<&'a AnimationRigSignature> {
    resources
        .iter()
        .filter(|resource| resource.kind() == CsharpRenderResourceKind::AnimatedMesh)
        .filter_map(CsharpRenderResource::animated_mesh)
        .find(|mesh| mesh.asset == asset)
        .and_then(|mesh| mesh.rig.as_ref())
}

/// Each rig joint's parent index.
fn rig_parents(joints: &[AnimationRigJoint]) -> Vec<u32> {
    let index: BTreeMap<&str, u32> = joints
        .iter()
        .enumerate()
        .map(|(position, joint)| (joint.id.as_str(), position as u32))
        .collect();
    joints
        .iter()
        .map(|joint| {
            joint
                .parent
                .as_deref()
                .and_then(|parent| index.get(parent).copied())
                .unwrap_or(NO_PARENT)
        })
        .collect()
}

fn descends(parents: &[u32], joint: u32, ancestor: u32) -> bool {
    let mut cursor = parents[joint as usize];
    while cursor != NO_PARENT {
        if cursor == ancestor {
            return true;
        }
        cursor = parents[cursor as usize];
    }
    false
}

fn pose_error(message: impl Into<String>) -> CsharpEngineServicesError {
    CsharpEngineServicesError::new("CSHARP_ANIMATION_POSE", message)
}

fn pose_space(space: NativePoseSpace) -> PoseSpace {
    match space {
        NativePoseSpace::Local => PoseSpace::Local,
        NativePoseSpace::Model => PoseSpace::Model,
        NativePoseSpace::World => PoseSpace::World,
    }
}

fn vec3(value: NativeVec3) -> [f32; 3] {
    [value.x, value.y, value.z]
}

fn native_transform_of(value: &Transform) -> NativeTransform {
    let [x, y, z] = value.translation;
    let [qx, qy, qz, qw] = value.rotation;
    let [sx, sy, sz] = value.scale;
    NativeTransform {
        translation: NativeVec3 { x, y, z },
        rotation: NativeQuat {
            x: qx,
            y: qy,
            z: qz,
            w: qw,
        },
        scale: NativeVec3 {
            x: sx,
            y: sy,
            z: sz,
        },
    }
}

impl RuntimeAppearanceBridge {
    fn set_animation_pose(
        &mut self,
        request: &NativeAnimationPoseRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let two_bone_ik = unsafe {
            borrowed_slice(
                request.two_bone_ik,
                request.two_bone_ik_len,
                "animation pose IK",
            )?
        };
        let overrides = unsafe {
            borrowed_slice(
                request.overrides,
                request.overrides_len,
                "animation pose overrides",
            )?
        };
        let pose = AnimatedMeshPose {
            two_bone_ik: two_bone_ik
                .iter()
                .map(|ik| TwoBoneIkConstraint {
                    root: ik.root,
                    mid: ik.mid,
                    end: ik.end,
                    space: pose_space(ik.space),
                    target: vec3(ik.target),
                    pole: vec3(ik.pole),
                    weight: ik.weight,
                })
                .collect(),
            overrides: overrides
                .iter()
                .map(|joint| JointOverride {
                    joint: joint.joint,
                    space: pose_space(joint.space),
                    additive: joint.additive,
                    rotation: joint.override_rotation.then_some([
                        joint.rotation.x,
                        joint.rotation.y,
                        joint.rotation.z,
                        joint.rotation.w,
                    ]),
                    translation: joint
                        .override_translation
                        .then_some(vec3(joint.translation)),
                    weight: joint.weight,
                })
                .collect(),
            layers: Vec::new(),
            report_joints: request.report_joints,
        };
        pose.validate()
            .map_err(|error| pose_error(format!("invalid pose: {error:?}")))?;
        let state = &mut *self.staged_mut()?.state;
        let instance = state
            .animation_instances
            .get(&request.instance.value)
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_ANIMATION_INSTANCE",
                    "animation instance is not live",
                )
            })?;
        if pose.controls() {
            let rig = asset_rig(&state.render_resources, &instance.asset)
                .ok_or_else(|| pose_error("pose controls need a skinned animated mesh"))?;
            let parents = rig_parents(&rig.joints);
            let joints = parents.len() as u32;
            let named = pose
                .two_bone_ik
                .iter()
                .flat_map(|ik| [ik.root, ik.mid, ik.end])
                .chain(pose.overrides.iter().map(|joint| joint.joint));
            if let Some(joint) = named.into_iter().find(|joint| *joint >= joints) {
                return Err(pose_error(format!(
                    "joint {joint} is outside the rig's {joints} joints"
                )));
            }
            if let Some(ik) = pose.two_bone_ik.iter().find(|ik| {
                !descends(&parents, ik.mid, ik.root) || !descends(&parents, ik.end, ik.mid)
            }) {
                return Err(pose_error(format!(
                    "IK joint {} must descend from {} and {} from {}",
                    ik.mid, ik.root, ik.end, ik.mid
                )));
            }
        }
        state
            .animation_instances
            .get_mut(&request.instance.value)
            .expect("checked above")
            .pose = pose;
        Ok(())
    }

    pub(crate) fn read_animation_joints(
        &mut self,
        resource: NativeRenderResourceHandle,
    ) -> Result<NativeAnimationJointInfoResult, CsharpEngineServicesError> {
        let joints = self
            .resource(resource.value)?
            .animated_mesh()
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_ANIMATION_RESOURCE",
                    "resource is not an animated mesh",
                )
            })?
            .rig
            .as_ref()
            .map(|rig| rig.joints.clone())
            .unwrap_or_default();
        let parents = rig_parents(&joints);
        let readout = joints
            .iter()
            .zip(&parents)
            .map(|(joint, parent)| NativeAnimationJointInfo {
                id: NativeUtf8Slice {
                    bytes: joint.id.as_ptr(),
                    len: joint.id.len(),
                },
                parent: *parent,
            })
            .collect::<Box<[_]>>();
        let result = NativeAnimationJointInfoResult {
            joints: readout.as_ptr(),
            joints_len: readout.len(),
        };
        self.borrowed.hold((joints, readout));
        Ok(result)
    }

    fn read_animation_joint_pose(
        &mut self,
        instance: NativeAnimationInstanceHandle,
    ) -> Result<NativeAnimationJointPoseResult, CsharpEngineServicesError> {
        let object = self
            .staged_ref()?
            .state
            .animation_instances
            .get(&instance.value)
            .map(|instance| instance.object_id)
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_ANIMATION_INSTANCE",
                    "animation instance is not live",
                )
            })?;
        let Some(reported) = self.joint_poses.get(&object) else {
            return Ok(NativeAnimationJointPoseResult {
                joints: std::ptr::null(),
                joints_len: 0,
                world: native_transform_of(&Transform::IDENTITY),
                seconds: 0.0,
                reported: false,
            });
        };
        let joints = reported
            .latest
            .joints
            .iter()
            .map(|joint| NativeJointPose {
                model: native_transform_of(&joint.model),
                world: native_transform_of(&joint.world),
            })
            .collect::<Box<[_]>>();
        let result = NativeAnimationJointPoseResult {
            joints: joints.as_ptr(),
            joints_len: joints.len(),
            world: native_transform_of(&reported.latest.world),
            seconds: reported.latest.seconds,
            reported: true,
        };
        self.borrowed.hold(joints);
        Ok(result)
    }

    /// The reports a ragdoll of `instance` spawns from, during a call.
    pub(crate) fn ragdoll_source(
        &self,
        instance: u64,
    ) -> Result<RagdollSource, CsharpEngineServicesError> {
        let state = &self.staged_ref()?.state;
        let instance = state.animation_instances.get(&instance).ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_ANIMATION_INSTANCE",
                "animation instance is not live",
            )
        })?;
        let joints =
            asset_rig(&state.render_resources, &instance.asset).map_or(0, |rig| rig.joints.len());
        let unreported = || {
            CsharpEngineServicesError::new(
                "CSHARP_RAGDOLL_POSE",
                "the instance has no reported pose: set ReportJoints on it a call before",
            )
        };
        let reported = self
            .joint_poses
            .get(&instance.object_id)
            .ok_or_else(unreported)?;
        let rest = reported.rest.clone().ok_or_else(unreported)?;
        if reported.latest.joints.len() != joints || rest.len() != joints {
            return Err(unreported());
        }
        Ok(RagdollSource {
            joints,
            rest,
            latest: reported.latest.clone(),
            previous: reported.previous.clone(),
        })
    }

    /// The layer a ragdoll places its bones with, over the product's pose
    /// controls; `None` releases them. Nothing for an instance that is gone.
    pub(crate) fn set_ragdoll_layer(
        &mut self,
        instance: u64,
        layer: Option<PlacedPoseLayer>,
    ) -> Result<(), CsharpEngineServicesError> {
        let state = &mut *self.staged_mut()?.state;
        if let Some(instance) = state.animation_instances.get_mut(&instance) {
            instance.ragdoll = layer;
        }
        Ok(())
    }

    /// Joint poses the renderer reported since the last call, oldest first.
    /// Only live instances' objects are kept.
    pub(crate) fn ingest_joint_poses(
        &mut self,
        reports: impl IntoIterator<Item = JointPoseReport>,
    ) {
        for report in reports {
            let live = self
                .state
                .animation_instances
                .values()
                .any(|instance| instance.object_id == report.object_id);
            if !live {
                continue;
            }
            let rest = report.rest.clone();
            let entry =
                self.joint_poses
                    .entry(report.object_id)
                    .or_insert_with(|| ReportedJointPose {
                        latest: report.clone(),
                        previous: None,
                        rest: None,
                    });
            if rest.is_some() {
                entry.rest = rest;
            }
            let earlier = std::mem::replace(&mut entry.latest, report);
            entry.previous = (earlier.generation == entry.latest.generation
                && earlier.seconds < entry.latest.seconds)
                .then_some(earlier);
        }
        let state = &self.state;
        self.joint_poses.retain(|object, _| {
            state
                .animation_instances
                .values()
                .any(|instance| instance.object_id == *object)
        });
    }
}

/// Send each instance's pose to its current target when either changed.
/// Runs as the call settles, after every other write of the call.
pub(super) fn settle_poses(
    staged: &mut RuntimeAppearanceCall,
) -> Result<(), CsharpEngineServicesError> {
    let mut ops = Vec::new();
    let state = &mut *staged.state;
    let handles: Vec<u64> = state.animation_instances.keys().copied().collect();
    for handle in handles {
        let instance = &state.animation_instances[&handle];
        let Some(target) = state.projector.object_handle(instance.object_id) else {
            continue;
        };
        let mut pose = instance.pose.clone();
        pose.layers.extend(instance.ragdoll.iter().cloned());
        let changed = match &instance.pose_sent {
            Some((sent, sent_pose)) if *sent == target => *sent_pose != pose,
            // A new target starts without controls.
            _ => pose != AnimatedMeshPose::default(),
        };
        if !changed {
            continue;
        }
        ops.push(RenderDiff::SetAnimatedMeshPose {
            handle: target,
            pose: pose.clone(),
        });
        state
            .animation_instances
            .get_mut(&handle)
            .expect("listed above")
            .pose_sent = Some((target, pose));
    }
    if ops.is_empty() {
        return Ok(());
    }
    let frame = RenderFrameDiff::try_from_ops(ops)
        .map_err(|error| pose_error(format!("animation pose frame is invalid: {error:?}")))?;
    push_extra_frame(staged, frame);
    Ok(())
}

pub(crate) unsafe extern "C" fn set_animation_pose(
    context: *mut c_void,
    request: *const NativeAnimationPoseRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        if request.is_null() {
            return 0;
        }
        animation_void(context, |bridge| {
            bridge.set_animation_pose(unsafe { &*request })
        })
    })
}

pub(crate) unsafe extern "C" fn read_animation_joints(
    context: *mut c_void,
    resource: NativeRenderResourceHandle,
    result: *mut NativeAnimationJointInfoResult,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        animation_result(context, result, |bridge| {
            bridge.read_animation_joints(resource)
        })
    })
}

pub(crate) unsafe extern "C" fn read_animation_joint_pose(
    context: *mut c_void,
    instance: NativeAnimationInstanceHandle,
    result: *mut NativeAnimationJointPoseResult,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        animation_result(context, result, |bridge| {
            bridge.read_animation_joint_pose(instance)
        })
    })
}

/// The joint attachment fixture's character as object 7, published in the
/// open call, with its rig joint indices by name.
#[cfg(test)]
pub(crate) fn character_for_test(
    bridge: &mut RuntimeAppearanceBridge,
) -> (NativeAnimationInstanceHandle, BTreeMap<String, u32>) {
    let path = b"body.glb";
    let resource = bridge
        .open_animated_mesh(&NativeAnimatedMeshResourceRequest {
            path: NativeUtf8Slice {
                bytes: path.as_ptr(),
                len: path.len(),
            },
        })
        .expect("admitted animated GLB");
    let joints = bridge.read_animation_joints(resource).expect("rig joints");
    // SAFETY: the result borrows bridge storage until the next call.
    let joints = unsafe { std::slice::from_raw_parts(joints.joints, joints.joints_len) }
        .iter()
        .enumerate()
        .map(|(index, joint)| {
            let id = unsafe { std::slice::from_raw_parts(joint.id.bytes, joint.id.len) };
            (String::from_utf8(id.to_vec()).unwrap(), index as u32)
        })
        .collect();
    let appearance = bridge
        .create_animated_mesh_appearance(NativeAnimatedMeshAppearanceRequest { resource })
        .expect("animated appearance");
    let instance = bridge
        .create_animation_instance(NativeAnimationInstanceRequest {
            appearance,
            object_id: 7,
        })
        .expect("animation instance");
    let fact = super::tests::appearance_fact(appearance);
    unsafe { bridge.stage_snapshot(&fact, 1) }.expect("snapshot");
    (instance, joints)
}

/// The ragdoll layer the open call holds for `instance`.
#[cfg(test)]
pub(crate) fn ragdoll_layer_for_test(
    bridge: &RuntimeAppearanceBridge,
    instance: NativeAnimationInstanceHandle,
) -> Option<PlacedPoseLayer> {
    bridge
        .staged
        .as_ref()
        .expect("an open call")
        .state
        .animation_instances[&instance.value]
        .ragdoll
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHARACTER_GLB: &[u8] =
        include_bytes!("../../../../../fixtures/csharp-joint-attachments/content/body.glb");

    fn joint_index(
        bridge: &mut RuntimeAppearanceBridge,
        resource: NativeRenderResourceHandle,
    ) -> BTreeMap<String, u32> {
        let joints = bridge.read_animation_joints(resource).expect("rig joints");
        // SAFETY: the result borrows bridge storage until the next call.
        let joints = unsafe { std::slice::from_raw_parts(joints.joints, joints.joints_len) };
        joints
            .iter()
            .enumerate()
            .map(|(index, joint)| {
                let id = unsafe { std::slice::from_raw_parts(joint.id.bytes, joint.id.len) };
                (String::from_utf8(id.to_vec()).unwrap(), index as u32)
            })
            .collect()
    }

    fn pose_ops(bridge: &RuntimeAppearanceBridge) -> Vec<AnimatedMeshPose> {
        bridge
            .staged
            .as_ref()
            .expect("staged call")
            .render_ops()
            .into_iter()
            .filter_map(|op| match op {
                RenderDiff::SetAnimatedMeshPose { pose, .. } => Some(pose),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_pose_is_checked_against_the_rig_sent_once_when_it_changes_and_its_joints_read_back() {
        let mut content_resources = BTreeMap::new();
        content_resources.insert("body.glb".to_owned(), Arc::from(CHARACTER_GLB));
        let mut bridge =
            RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), content_resources);
        let path = b"body.glb";
        bridge.begin_call();
        let resource = bridge
            .open_animated_mesh(&NativeAnimatedMeshResourceRequest {
                path: NativeUtf8Slice {
                    bytes: path.as_ptr(),
                    len: path.len(),
                },
            })
            .expect("admitted animated GLB");
        let joints = joint_index(&mut bridge, resource);
        let (arm, forearm, hand) = (
            joints["RightArm"],
            joints["RightForeArm"],
            joints["RightHand"],
        );
        let appearance = bridge
            .create_animated_mesh_appearance(NativeAnimatedMeshAppearanceRequest { resource })
            .expect("animated appearance");
        let instance = bridge
            .create_animation_instance(NativeAnimationInstanceRequest {
                appearance,
                object_id: 7,
            })
            .expect("animation instance");
        let fact = super::super::tests::appearance_fact(appearance);
        unsafe { bridge.stage_snapshot(&fact, 1) }.expect("snapshot");

        let ik = |root: u32, mid: u32, end: u32| NativeTwoBoneIk {
            root,
            mid,
            end,
            space: NativePoseSpace::World,
            target: NativeVec3 {
                x: 0.3,
                y: 1.4,
                z: 0.4,
            },
            pole: NativeVec3 {
                x: 0.0,
                y: 1.0,
                z: -2.0,
            },
            weight: 1.0,
        };
        let request = |chain: &[NativeTwoBoneIk]| NativeAnimationPoseRequest {
            instance,
            two_bone_ik: chain.as_ptr(),
            two_bone_ik_len: chain.len(),
            overrides: std::ptr::null(),
            overrides_len: 0,
            report_joints: true,
        };
        let backwards = [ik(hand, forearm, arm)];
        let refused = bridge
            .set_animation_pose(&request(&backwards))
            .expect_err("a chain that does not descend is refused");
        assert!(refused.to_string().contains("must descend"), "{refused}");
        let outside = [ik(arm, forearm, 9_999)];
        assert!(bridge.set_animation_pose(&request(&outside)).is_err());

        let chain = [ik(arm, forearm, hand)];
        bridge
            .set_animation_pose(&request(&chain))
            .expect("a valid pose");
        bridge
            .set_animation_pose(&request(&chain))
            .expect("written again");
        assert!(pose_ops(&bridge).is_empty(), "sent when the call settles");
        let call = bridge.take_staged_call();
        let sent: Vec<_> = call
            .render_ops()
            .into_iter()
            .filter(|op| matches!(op, RenderDiff::SetAnimatedMeshPose { .. }))
            .collect();
        assert_eq!(sent.len(), 1);
        bridge.commit(call);

        // Rewritten unchanged each update, it is not sent again.
        bridge.begin_call();
        bridge.set_animation_pose(&request(&chain)).unwrap();
        let call = bridge.take_staged_call();
        assert!(!call
            .render_ops()
            .iter()
            .any(|op| matches!(op, RenderDiff::SetAnimatedMeshPose { .. })));
        bridge.commit(call);

        // The renderer's report reads back before the next call.
        let joint = |x: f32| PosedJoint {
            model: Transform {
                translation: [x, 0.0, 0.0],
                ..Transform::IDENTITY
            },
            world: Transform {
                translation: [x + 1.0, 0.0, 0.0],
                ..Transform::IDENTITY
            },
        };
        bridge.ingest_joint_poses([
            JointPoseReport {
                object_id: 7,
                generation: 1,
                seconds: 0.5,
                world: Transform::IDENTITY,
                joints: vec![joint(1.0); joints.len()],
                rest: Some(vec![joint(0.0); joints.len()]),
            },
            JointPoseReport {
                object_id: 99,
                generation: 1,
                seconds: 0.5,
                world: Transform::IDENTITY,
                joints: Vec::new(),
                rest: None,
            },
        ]);
        assert_eq!(bridge.joint_poses.len(), 1, "only live instances are kept");
        bridge.begin_call();
        let pose = bridge.read_animation_joint_pose(instance).unwrap();
        assert!(pose.reported);
        assert_eq!((pose.joints_len, pose.seconds), (joints.len(), 0.5));
        let read = unsafe { *pose.joints.add(hand as usize) };
        assert_eq!(
            (read.model.translation.x, read.world.translation.x),
            (1.0, 2.0)
        );

        // Destroying the instance clears the controls it left on the target.
        bridge.destroy_animation_instance(instance).unwrap();
        assert_eq!(pose_ops(&bridge), [AnimatedMeshPose::default()]);
        let call = bridge.take_staged_call();
        bridge.commit(call);
        bridge.ingest_joint_poses([]);
        assert!(bridge.joint_poses.is_empty());
    }
}
