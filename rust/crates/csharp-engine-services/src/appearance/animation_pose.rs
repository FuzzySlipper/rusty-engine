//! Product pose controls on animation instances (`SetPose`), the admitted
//! rig's joints (`ReadJoints`), the joints the Engine evaluated
//! (`ReadJointPose`), and joints sampled at simulation time
//! (`SampleJointPose`).
//!
//! A pose is retained on its instance and reaches the scene when the call
//! settles, as one `SetAnimatedMeshPose` per changed instance or target.
//! The scene's poses are evaluated once as each call settles
//! ([`::animation_pose::AnimatedScene`], kept with the appearance state),
//! for realizers to draw and for joint reads, ragdolls and severed parts to
//! read. A sample poses the mesh here through the same evaluator, so it
//! answers within the call.

use super::*;

/// What a ragdoll or a severed part spawns from: an instance's rig size and
/// its evaluated joints.
pub(crate) struct RagdollSource {
    pub(crate) joints: usize,
    pub(crate) rest: Vec<PosedJoint>,
    pub(crate) latest: ::animation_pose::PosedJoints,
    pub(crate) previous: Option<::animation_pose::PosedJoints>,
    /// `latest` before its ragdoll layer, when it has one.
    pub(crate) unlayered: Option<Vec<PosedJoint>>,
}

const NO_PARENT: u32 = u32::MAX;

/// Evaluates the poses a settled call can have changed, at the time it
/// reached, with its instances placed in `world` (the committed world with
/// the call's changes). Returns the changed poses and the `once` clips that
/// ended.
pub(crate) fn evaluate_poses(
    call: &mut RuntimeAppearanceCall,
    world: &render_presentation::PresentationWorld,
) -> ::animation_pose::Settled {
    let state = &mut *call.state;
    let (poses, resources) = (&mut state.poses, &state.render_resources);
    poses.settle(world.elapsed_seconds(), world, &|asset| {
        decoded_model(resources, asset)
    })
}

/// The decoded mesh of an animated mesh asset a renderer change defines.
fn decoded_model(
    resources: &RenderResourceRegistry,
    asset: &AnimatedMeshAsset,
) -> Option<Arc<::animation_pose::PoseModel>> {
    let identity = ::animation_pose::resource_identity(
        "animated-mesh-resource",
        asset.content_hash.as_deref(),
    )?;
    let body = resources.by_identity(&identity).or_else(|| {
        resources
            .recently_released()
            .find(|resource| resource.identity() == identity)
    })?;
    body.pose_model(|pack| resources.by_identity(pack).map(|pack| pack.shared_bytes()))
        .ok()
}

/// The admitted rig of the mesh an instance plays.
fn instance_rig<'a>(
    state: &'a RuntimeAppearanceData,
    instance: &AnimationInstance,
) -> Option<&'a AnimationRigSignature> {
    instance_mesh(
        &state.animated_appearances,
        &state.render_resources,
        instance.appearance,
    )?
    .rig
    .as_ref()
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

fn native_vec3([x, y, z]: [f32; 3]) -> NativeVec3 {
    NativeVec3 { x, y, z }
}

fn transform_value(value: NativeTransform) -> Transform {
    Transform {
        translation: vec3(value.translation),
        rotation: [
            value.rotation.x,
            value.rotation.y,
            value.rotation.z,
            value.rotation.w,
        ],
        scale: vec3(value.scale),
    }
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
            clip_layers: Vec::new(),
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
                    scale: joint.override_scale.then_some(joint.scale),
                    weight: joint.weight,
                })
                .collect(),
            layers: Vec::new(),
            cuts: Vec::new(),
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
            let rig = instance_rig(state, instance)
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

    fn set_animation_layers(
        &mut self,
        request: &NativeAnimationLayersRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let layers =
            unsafe { borrowed_slice(request.layers, request.layers_len, "animation clip layers")? };
        let weights = unsafe {
            borrowed_slice(
                request.joint_weights,
                request.joint_weights_len,
                "animation layer joint weights",
            )?
        };
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
        let mesh = instance_mesh(
            &state.animated_appearances,
            &state.render_resources,
            instance.appearance,
        )
        .ok_or_else(|| pose_error("clip layers need an animated mesh"))?;
        let clips: Vec<&str> = mesh_clips(mesh).map(|(_, clip)| clip.id.as_str()).collect();
        let joints = mesh.rig.as_ref().map_or(0, |rig| rig.joints.len());
        let clip_layers = layers
            .iter()
            .map(|layer| {
                let clip = clips.get(layer.clip as usize).ok_or_else(|| {
                    pose_error(format!(
                        "clip {} is outside the mesh's {} clips",
                        layer.clip,
                        clips.len()
                    ))
                })?;
                let start = layer.joint_weights_start as usize;
                let mask = start
                    .checked_add(layer.joint_weights_len as usize)
                    .and_then(|end| weights.get(start..end))
                    .ok_or_else(|| pose_error("a layer's joint weights lie outside the list"))?;
                if let Some(joint) = mask.iter().find(|joint| joint.joint as usize >= joints) {
                    return Err(pose_error(format!(
                        "joint {} is outside the rig's {joints} joints",
                        joint.joint
                    )));
                }
                Ok(ClipLayer {
                    clip: (*clip).to_owned(),
                    time: layer.time,
                    normalized: layer.normalized,
                    weight: layer.weight,
                    blend: if layer.additive {
                        ClipLayerBlend::Additive
                    } else {
                        ClipLayerBlend::Override
                    },
                    reference_time: layer.reference_time,
                    joint_weights: mask
                        .iter()
                        .map(|joint| JointWeight {
                            joint: joint.joint,
                            weight: joint.weight,
                        })
                        .collect(),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        AnimatedMeshPose {
            clip_layers: clip_layers.clone(),
            ..AnimatedMeshPose::default()
        }
        .validate()
        .map_err(|error| pose_error(format!("invalid clip layers: {error:?}")))?;
        state
            .animation_instances
            .get_mut(&request.instance.value)
            .expect("checked above")
            .clip_layers = clip_layers;
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
        let reading = self.joint_reading(instance.value)?;
        let Some(reading) = reading else {
            return Ok(NativeAnimationJointPoseResult {
                joints: std::ptr::null(),
                joints_len: 0,
                ik_chains: std::ptr::null(),
                ik_chains_len: 0,
                world: native_transform_of(&Transform::IDENTITY),
                seconds: 0.0,
                reported: false,
            });
        };
        let joints = reading
            .latest
            .joints
            .iter()
            .map(|joint| NativeJointPose {
                model: native_transform_of(&joint.model),
                world: native_transform_of(&joint.world),
            })
            .collect::<Box<[_]>>();
        let ik_chains = reading
            .ik_chains
            .iter()
            .map(|chain| NativeIkChainReach {
                target_model: native_vec3(chain.target_model),
                target_world: native_vec3(chain.target_world),
                end_model: native_vec3(chain.end_model),
                end_world: native_vec3(chain.end_world),
                shortfall: chain.shortfall,
                straightened: chain.straightened,
            })
            .collect::<Box<[_]>>();
        let result = NativeAnimationJointPoseResult {
            joints: joints.as_ptr(),
            joints_len: joints.len(),
            ik_chains: ik_chains.as_ptr(),
            ik_chains_len: ik_chains.len(),
            world: native_transform_of(&reading.latest.world),
            seconds: reading.latest.seconds,
            reported: true,
        };
        self.borrowed.hold((joints, ik_chains));
        Ok(result)
    }

    /// The joints the Engine last evaluated for a live instance, during a
    /// call; `None` while its node has no evaluated pose (it is not in the
    /// published scene, or was published this call).
    fn joint_reading(
        &self,
        instance: u64,
    ) -> Result<Option<::animation_pose::JointReading>, CsharpEngineServicesError> {
        let state = &self.staged_ref()?.state;
        let instance = state.animation_instances.get(&instance).ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_ANIMATION_INSTANCE",
                "animation instance is not live",
            )
        })?;
        Ok(
            instance_target(&state.projector, &state.appearances, instance)
                .and_then(|target| state.poses.joints(target)),
        )
    }

    /// The decoded pose model of an animated mesh resource, kept with the
    /// resource.
    fn pose_model(
        &self,
        resource: u64,
    ) -> Result<Arc<::animation_pose::PoseModel>, CsharpEngineServicesError> {
        let resources = &self.staged_ref()?.state.render_resources;
        let body = resources.resource(resource)?;
        if body.animated_mesh().is_none() {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_ANIMATION_RESOURCE",
                "resource is not an animated mesh",
            ));
        }
        body.pose_model(|identity| {
            resources
                .by_identity(identity)
                .map(|pack| pack.shared_bytes())
        })
        .map_err(|error| pose_error(format!("the mesh could not be posed: {error}")))
    }

    fn sample_animation_joint_pose(
        &mut self,
        request: &NativeAnimationPoseSampleRequest,
    ) -> Result<NativeAnimationPoseSampleResult, CsharpEngineServicesError> {
        let state = &self.staged_ref()?.state;
        let instance = state
            .animation_instances
            .get(&request.instance.value)
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_ANIMATION_INSTANCE",
                    "animation instance is not live",
                )
            })?;
        let resource = *state
            .animated_appearances
            .get(&instance.appearance)
            .ok_or_else(|| pose_error("the instance's appearance is not animated"))?;
        let mut controls = AnimatedMeshPose::default();
        if request.apply_pose_controls {
            controls = instance.pose.clone();
            controls.layers.extend(instance.ragdoll.iter().cloned());
        }
        if request.use_current_playback {
            // What it plays includes its clip layers.
            controls.clip_layers.clone_from(&instance.clip_layers);
            let staged = self.staged_ref()?;
            let state = &staged.state;
            let target = instance_target(
                &state.projector,
                &state.appearances,
                &state.animation_instances[&request.instance.value],
            )
            .ok_or_else(|| {
                pose_error("the instance is not in the published scene, so it plays nothing")
            })?;
            let now = staged.clock.1;
            let model = self.pose_model(resource)?;
            let state = &self.staged_ref()?.state;
            let actions = state.poses.actions(target, &model, now);
            let world = transform_value(request.placement);
            let joints = model.sample_actions(&actions, &controls, &world);
            return Ok(self.hold_joints(&joints));
        }
        self.sample_joints(
            resource,
            &NativeAnimationModelPoseSampleRequest {
                resource: NativeRenderResourceHandle { value: resource },
                clip: request.clip,
                time: request.time,
                blend_clip: request.blend_clip,
                blend_time: request.blend_time,
                blend_weight: request.blend_weight,
                normalized: request.normalized,
                placement: request.placement,
            },
            &controls,
        )
    }

    fn sample_animation_model_joint_pose(
        &mut self,
        request: &NativeAnimationModelPoseSampleRequest,
    ) -> Result<NativeAnimationPoseSampleResult, CsharpEngineServicesError> {
        self.sample_joints(
            request.resource.value,
            request,
            &AnimatedMeshPose::default(),
        )
    }

    /// The joints of `resource`'s mesh sampled as `request` says, under
    /// `controls`.
    fn sample_joints(
        &mut self,
        resource: u64,
        request: &NativeAnimationModelPoseSampleRequest,
        controls: &AnimatedMeshPose,
    ) -> Result<NativeAnimationPoseSampleResult, CsharpEngineServicesError> {
        let clip = unsafe { borrowed_utf8(request.clip.bytes, request.clip.len, "sample clip")? };
        let blend_clip = unsafe {
            borrowed_utf8(
                request.blend_clip.bytes,
                request.blend_clip.len,
                "sample blend clip",
            )?
        };
        let model = self.pose_model(resource)?;
        let blend = if blend_clip.is_empty() {
            0.0
        } else {
            request.blend_weight.clamp(0.0, 1.0)
        };
        let sample = |clip, time, weight| ::animation_pose::ClipSample {
            clip,
            time,
            normalized: request.normalized,
            weight,
        };
        let samples: Vec<_> = [
            (!clip.is_empty()).then(|| sample(clip, request.time, 1.0 - blend)),
            (!blend_clip.is_empty()).then(|| sample(blend_clip, request.blend_time, blend)),
        ]
        .into_iter()
        .flatten()
        .collect();
        let world = transform_value(request.placement);
        let joints = model
            .sample(&samples, controls, &world)
            .map_err(pose_error)?;
        Ok(self.hold_joints(&joints))
    }

    /// Sampled joints as a borrowed result.
    fn hold_joints(&mut self, joints: &[PosedJoint]) -> NativeAnimationPoseSampleResult {
        let joints = joints
            .iter()
            .map(|joint| NativeJointPose {
                model: native_transform_of(&joint.model),
                world: native_transform_of(&joint.world),
            })
            .collect::<Box<[_]>>();
        let result = NativeAnimationPoseSampleResult {
            joints: joints.as_ptr(),
            joints_len: joints.len(),
        };
        self.borrowed.hold(joints);
        result
    }

    /// The evaluated joints a ragdoll or a severed part of `instance` spawns
    /// from, during a call; without them, refused with `unposed`.
    pub(crate) fn ragdoll_source(
        &self,
        instance: u64,
        unposed: &'static str,
    ) -> Result<RagdollSource, CsharpEngineServicesError> {
        let joints = {
            let state = &self.staged_ref()?.state;
            state
                .animation_instances
                .get(&instance)
                .and_then(|instance| instance_rig(state, instance))
                .map_or(0, |rig| rig.joints.len())
        };
        let reading = self.joint_reading(instance)?.ok_or_else(|| {
            CsharpEngineServicesError::new(
                unposed,
                "the instance has no evaluated pose: publish it in a call before",
            )
        })?;
        Ok(RagdollSource {
            joints,
            rest: reading.rest,
            latest: reading.latest,
            previous: reading.previous,
            unlayered: reading.unlayered,
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

    /// Whether `joint` of `instance` is cut off already: it or a joint
    /// above it is cut.
    pub(crate) fn joint_cut_off(
        &self,
        instance: u64,
        joint: u32,
    ) -> Result<bool, CsharpEngineServicesError> {
        let state = &self.staged_ref()?.state;
        let Some(instance) = state.animation_instances.get(&instance) else {
            return Ok(false);
        };
        let parents =
            instance_rig(state, instance).map_or_else(Vec::new, |rig| rig_parents(&rig.joints));
        Ok(instance.cuts.values().any(|cut| {
            cut.joint == joint
                || ((joint as usize) < parents.len() && descends(&parents, joint, cut.joint))
        }))
    }

    /// The joint a severed part cuts off `instance`; `None` releases it.
    /// Returns whether the instance lives.
    pub(crate) fn set_joint_cut(
        &mut self,
        instance: u64,
        part: u64,
        cut: Option<JointCut>,
    ) -> Result<bool, CsharpEngineServicesError> {
        let state = &mut *self.staged_mut()?.state;
        let Some(instance) = state.animation_instances.get_mut(&instance) else {
            return Ok(false);
        };
        match cut {
            Some(cut) => instance.cuts.insert(part, cut),
            None => instance.cuts.remove(&part),
        };
        Ok(true)
    }

    /// Whether an animation instance lives, during a call.
    pub(crate) fn animation_instance_lives(
        &self,
        instance: u64,
    ) -> Result<bool, CsharpEngineServicesError> {
        Ok(self
            .staged_ref()?
            .state
            .animation_instances
            .contains_key(&instance))
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
        pose.clip_layers.clone_from(&instance.clip_layers);
        pose.layers.extend(instance.ragdoll.iter().cloned());
        pose.cuts.extend(instance.cuts.values().cloned());
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
    let frame = RenderFrameDiff::from_ops(ops);
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

pub(crate) unsafe extern "C" fn set_animation_layers(
    context: *mut c_void,
    request: *const NativeAnimationLayersRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        if request.is_null() {
            return 0;
        }
        animation_void(context, |bridge| {
            bridge.set_animation_layers(unsafe { &*request })
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

pub(crate) unsafe extern "C" fn sample_animation_joint_pose(
    context: *mut c_void,
    request: *const NativeAnimationPoseSampleRequest,
    result: *mut NativeAnimationPoseSampleResult,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        if request.is_null() {
            return 0;
        }
        animation_result(context, result, |bridge| {
            bridge.sample_animation_joint_pose(unsafe { &*request })
        })
    })
}

pub(crate) unsafe extern "C" fn sample_animation_model_joint_pose(
    context: *mut c_void,
    request: *const NativeAnimationModelPoseSampleRequest,
    result: *mut NativeAnimationPoseSampleResult,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        if request.is_null() {
            return 0;
        }
        animation_result(context, result, |bridge| {
            bridge.sample_animation_model_joint_pose(unsafe { &*request })
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

/// Ends the open call as the service set ends one: its renderer changes
/// reach `world`, Engine time moves on by `advance`, and the scene's poses
/// are evaluated there.
#[cfg(test)]
pub(crate) fn end_call_posed(
    bridge: &mut RuntimeAppearanceBridge,
    world: &mut render_presentation::PresentationWorld,
    advance: f64,
) {
    let call = bridge.take_staged_call();
    settle_call_posed(bridge, call, world, advance);
}

/// [`end_call_posed`] for a call already taken from the bridge.
#[cfg(test)]
pub(crate) fn settle_call_posed(
    bridge: &mut RuntimeAppearanceBridge,
    mut call: RuntimeAppearanceCall,
    world: &mut render_presentation::PresentationWorld,
    advance: f64,
) {
    for output in std::mem::take(&mut call.outputs) {
        match output {
            RuntimeAppearanceCallOutput::Frame(frame) => {
                world.apply(frame).expect("the world admits the call");
            }
            RuntimeAppearanceCallOutput::Presentation(frame) => {
                world
                    .apply_presentation(frame)
                    .expect("the world admits the call");
            }
        }
    }
    world.advance_elapsed(advance);
    let settled = evaluate_poses(&mut call, world);
    world
        .apply(settled.poses)
        .expect("the world retains the poses");
    bridge.commit(call);
}

/// Publishes the joint attachment fixture's character (object 7) at
/// `height` in the open call.
#[cfg(test)]
pub(crate) fn place_character_for_test(
    bridge: &mut RuntimeAppearanceBridge,
    instance: NativeAnimationInstanceHandle,
    height: f32,
) {
    let appearance =
        bridge.staged_ref().unwrap().state.animation_instances[&instance.value].appearance;
    let mut fact = super::tests::appearance_fact(NativeAppearanceHandle { value: appearance });
    fact.transform.translation.y = height;
    unsafe { bridge.stage_snapshot(&fact, 1) }.expect("snapshot");
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

/// The joints the open call cuts off `instance`, by part.
#[cfg(test)]
pub(crate) fn cuts_for_test(
    bridge: &RuntimeAppearanceBridge,
    instance: NativeAnimationInstanceHandle,
) -> Option<Vec<JointCut>> {
    bridge
        .staged
        .as_ref()
        .expect("an open call")
        .state
        .animation_instances
        .get(&instance.value)
        .map(|instance| instance.cuts.values().cloned().collect())
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

    fn utf8(value: &str) -> NativeUtf8Slice {
        NativeUtf8Slice {
            bytes: value.as_ptr(),
            len: value.len(),
        }
    }

    fn placed_at(x: f32) -> NativeTransform {
        NativeTransform {
            translation: NativeVec3 { x, y: 0.0, z: 0.0 },
            rotation: NativeQuat {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 1.0,
            },
            scale: NativeVec3 {
                x: 1.0,
                y: 1.0,
                z: 1.0,
            },
        }
    }

    /// Samples and copies the joints before the next call on the bridge.
    fn sampled(
        bridge: &mut RuntimeAppearanceBridge,
        request: &NativeAnimationPoseSampleRequest,
    ) -> Result<Vec<NativeJointPose>, CsharpEngineServicesError> {
        let result = bridge.sample_animation_joint_pose(request)?;
        Ok(unsafe { std::slice::from_raw_parts(result.joints, result.joints_len) }.to_vec())
    }

    fn hand_x(joints: &[NativeJointPose], hand: u32) -> (f32, f32) {
        let joint = joints[hand as usize];
        (joint.model.translation.x, joint.world.translation.x)
    }

    #[test]
    fn a_sampled_pose_needs_no_renderer_and_follows_the_clip_controls_and_placement() {
        let mut bridge = RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), {
            let mut content = BTreeMap::new();
            content.insert(
                "body.glb".to_owned(),
                crate::ContentBytes::from(CHARACTER_GLB),
            );
            content
        });
        bridge.begin_call();
        let (instance, joints) = character_for_test(&mut bridge);
        let hand = joints["RightHand"];
        let resource = NativeRenderResourceHandle {
            value: bridge.staged_ref().unwrap().state.animated_appearances[&bridge
                .staged_ref()
                .unwrap()
                .state
                .animation_instances[&instance.value]
                .appearance],
        };
        let request = |clip: &'static str, time: f32| NativeAnimationPoseSampleRequest {
            instance,
            clip: utf8(clip),
            time,
            blend_clip: utf8(""),
            blend_time: 0.0,
            blend_weight: 0.0,
            normalized: true,
            placement: placed_at(2.0),
            apply_pose_controls: true,
            use_current_playback: false,
        };
        let early = sampled(&mut bridge, &request("run", 0.1)).unwrap();
        let late = sampled(&mut bridge, &request("run", 0.6)).unwrap();
        assert_eq!(early.len(), joints.len(), "one per rig joint");
        assert_ne!(hand_x(&early, hand), hand_x(&late, hand), "the clip moves");
        let (model, world) = hand_x(&late, hand);
        assert!((world - (model + 2.0)).abs() < 1e-5, "{model} {world}");
        // Sampling twice is the same pose: nothing is retained.
        let again = sampled(&mut bridge, &request("run", 0.6)).unwrap();
        assert_eq!(hand_x(&again, hand), hand_x(&late, hand));
        // A blend with weight 0 is the first clip; weight 1 the second.
        let blended = |weight| NativeAnimationPoseSampleRequest {
            blend_clip: utf8("run"),
            blend_time: 0.1,
            blend_weight: weight,
            ..request("run", 0.6)
        };
        let none = sampled(&mut bridge, &blended(0.0)).unwrap();
        let full = sampled(&mut bridge, &blended(1.0)).unwrap();
        assert_eq!(hand_x(&none, hand), hand_x(&late, hand));
        assert_eq!(hand_x(&full, hand), hand_x(&early, hand));
        // The rest pose, by resource without an instance.
        let rest = bridge
            .sample_animation_model_joint_pose(&NativeAnimationModelPoseSampleRequest {
                resource,
                clip: utf8(""),
                time: 0.0,
                blend_clip: utf8(""),
                blend_time: 0.0,
                blend_weight: 0.0,
                normalized: true,
                placement: placed_at(2.0),
            })
            .unwrap();
        let rest = unsafe { std::slice::from_raw_parts(rest.joints, rest.joints_len) }.to_vec();
        assert_ne!(hand_x(&rest, hand), hand_x(&early, hand));

        // The instance's controls apply on request: a hand placed in the
        // scene lands there.
        let target = NativeVec3 {
            x: 5.0,
            y: 1.0,
            z: 0.5,
        };
        bridge
            .set_animation_pose(&NativeAnimationPoseRequest {
                instance,
                two_bone_ik: std::ptr::null(),
                two_bone_ik_len: 0,
                overrides: &NativeJointOverride {
                    joint: hand,
                    space: NativePoseSpace::World,
                    additive: false,
                    override_rotation: false,
                    rotation: NativeQuat::default(),
                    override_translation: true,
                    translation: target,
                    override_scale: false,
                    scale: 1.0,
                    weight: 1.0,
                },
                overrides_len: 1,
            })
            .unwrap();
        let controlled = sampled(&mut bridge, &request("run", 0.6)).unwrap();
        let placed = controlled[hand as usize].world.translation;
        assert!(
            (placed.x - target.x).abs() < 1e-4 && (placed.y - target.y).abs() < 1e-4,
            "{placed:?}"
        );
        let uncontrolled = sampled(
            &mut bridge,
            &NativeAnimationPoseSampleRequest {
                apply_pose_controls: false,
                ..request("run", 0.6)
            },
        )
        .unwrap();
        assert_eq!(hand_x(&uncontrolled, hand), hand_x(&late, hand));

        // A joint scaled to 0 reads back with its scale.
        bridge
            .set_animation_pose(&NativeAnimationPoseRequest {
                instance,
                two_bone_ik: std::ptr::null(),
                two_bone_ik_len: 0,
                overrides: &NativeJointOverride {
                    joint: hand,
                    space: NativePoseSpace::Local,
                    additive: false,
                    override_rotation: false,
                    rotation: NativeQuat::default(),
                    override_translation: false,
                    translation: NativeVec3::default(),
                    override_scale: true,
                    scale: 0.0,
                    weight: 1.0,
                },
                overrides_len: 1,
            })
            .unwrap();
        let collapsed = sampled(&mut bridge, &request("run", 0.6)).unwrap();
        let hand_pose = collapsed[hand as usize].model;
        assert_eq!(
            [hand_pose.scale.x, hand_pose.scale.y, hand_pose.scale.z],
            [0.0; 3]
        );
        assert!(hand_pose.rotation.w.is_finite());
        assert_eq!(hand_x(&collapsed, hand), hand_x(&late, hand));

        let refused = sampled(&mut bridge, &request("fly", 0.5)).unwrap_err();
        assert!(refused.to_string().contains("fly"), "{refused}");
    }

    /// Sampling current playback reads what the renderer will draw for the
    /// call: commands anchored at the call's start on the presentation
    /// timeline, posed at its end, across calls and through a cross-fade.
    #[test]
    fn current_playback_is_sampled_at_the_time_the_call_is_drawn() {
        let mut bridge = RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), {
            let mut content = BTreeMap::new();
            content.insert(
                "body.glb".to_owned(),
                crate::ContentBytes::from(CHARACTER_GLB),
            );
            content
        });
        let play = |bridge: &mut RuntimeAppearanceBridge,
                    instance,
                    clip: &'static str,
                    fade: Option<f32>| {
            bridge
                .set_animation_playback(&NativeAnimationPlaybackRequest {
                    instance,
                    kind: NativeAnimationPlaybackKind::Play,
                    clip: utf8(clip),
                    loop_mode: NativeAnimationLoopMode::Repeat,
                    speed: 1.0,
                    weight: 1.0,
                    restart: true,
                    fade_seconds: fade.unwrap_or(0.0),
                    has_fade: fade.is_some(),
                    normalized_time: 0.0,
                })
                .unwrap();
        };
        let explicit = |instance, clip, time, blend_clip, blend_time, blend_weight| {
            NativeAnimationPoseSampleRequest {
                instance,
                clip: utf8(clip),
                time,
                blend_clip: utf8(blend_clip),
                blend_time,
                blend_weight,
                normalized: false,
                placement: placed_at(0.0),
                apply_pose_controls: false,
                use_current_playback: false,
            }
        };
        let current = |instance| NativeAnimationPoseSampleRequest {
            use_current_playback: true,
            ..explicit(instance, "", 0.0, "", 0.0, 0.0)
        };
        let same = |a: &[NativeJointPose], b: &[NativeJointPose]| {
            a.len() == b.len()
                && a.iter().zip(b).all(|(a, b)| {
                    let (a, b) = (a.model.translation, b.model.translation);
                    (a.x - b.x).abs() < 1e-6 && (a.y - b.y).abs() < 1e-6 && (a.z - b.z).abs() < 1e-6
                })
        };
        let end_call = |bridge: &mut RuntimeAppearanceBridge| {
            let call = bridge.take_staged_call();
            bridge.commit(call);
        };

        // The first call runs from 1.0 to 1.25: idle starts at 1.0.
        bridge.set_presentation_clock(1.0, 0.25);
        bridge.begin_call();
        let (instance, _) = character_for_test(&mut bridge);
        play(&mut bridge, instance, "idle", None);
        let now = sampled(&mut bridge, &current(instance)).unwrap();
        let idle = sampled(&mut bridge, &explicit(instance, "idle", 0.25, "", 0.0, 0.0)).unwrap();
        assert!(same(&now, &idle), "idle 0.25 s in at the call's end");
        end_call(&mut bridge);

        // The next call, 1.25 to 1.5, cross-fades to run over a second.
        bridge.set_presentation_clock(1.25, 0.25);
        bridge.begin_call();
        let still = sampled(&mut bridge, &current(instance)).unwrap();
        let idle = sampled(&mut bridge, &explicit(instance, "idle", 0.5, "", 0.0, 0.0)).unwrap();
        assert!(same(&still, &idle), "the playback carries across calls");
        play(&mut bridge, instance, "run", Some(1.0));
        let fading = sampled(&mut bridge, &current(instance)).unwrap();
        let blend = sampled(
            &mut bridge,
            &explicit(instance, "idle", 0.5, "run", 0.25, 0.25),
        )
        .unwrap();
        assert!(same(&fading, &blend), "a quarter through the fade");
        assert!(!same(&fading, &idle));
        end_call(&mut bridge);

        // A call with no admitted time samples where the last one drew.
        bridge.set_presentation_clock(1.5, 0.0);
        bridge.begin_call();
        let held = sampled(&mut bridge, &current(instance)).unwrap();
        assert!(same(&held, &blend));
        end_call(&mut bridge);
    }

    const CLIP_PACK_GLB: &[u8] =
        include_bytes!("../../../../../fixtures/csharp-joint-attachments/content/body.clips.glb");

    /// A skeleton-only clip pack's clips join the mesh's own in `ReadClips`
    /// and play as clip layers over the playback: an override on the arm at
    /// half weight and an additive nod, sampled as the instance plays.
    #[test]
    fn clip_layers_from_a_skeleton_only_pack_lay_over_the_playback() {
        let mut bridge = RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), {
            let mut content = BTreeMap::new();
            content.insert(
                "body.glb".to_owned(),
                crate::ContentBytes::from(CHARACTER_GLB),
            );
            content.insert(
                "body.clips.glb".to_owned(),
                crate::ContentBytes::from(CLIP_PACK_GLB),
            );
            content
        });
        bridge.set_presentation_clock(0.0, 0.25);
        bridge.begin_call();
        let resource = bridge
            .open_animated_mesh(&NativeAnimatedMeshResourceRequest {
                path: utf8("body.glb"),
            })
            .unwrap();
        let pack = bridge
            .open_animation_clip_pack(&NativeAnimationClipPackResourceRequest {
                path: utf8("body.clips.glb"),
            })
            .expect("a pack without a mesh is admitted");
        bridge
            .associate_animation_clip_pack(&NativeAnimationClipPackAssociationRequest {
                primary_mesh: resource,
                clip_pack: pack,
                producer: utf8("fixture"),
                license: utf8("CC0"),
            })
            .expect("the pack shares the body's rig");
        let mut clips = NativeAnimationClipInfoResult {
            clips: std::ptr::null(),
            clips_len: 0,
        };
        let mut error: NativeOperationErrorReceipt = unsafe { std::mem::zeroed() };
        let context = (&mut bridge as *mut RuntimeAppearanceBridge).cast();
        assert_eq!(
            unsafe { super::read_animation_clips(context, resource, &mut clips, &mut error) },
            ABI_OK
        );
        let clips: Vec<(String, u32)> =
            unsafe { std::slice::from_raw_parts(clips.clips, clips.clips_len) }
                .iter()
                .map(|clip| {
                    let id = unsafe { std::slice::from_raw_parts(clip.id.bytes, clip.id.len) };
                    (String::from_utf8(id.to_vec()).unwrap(), clip.clip_pack)
                })
                .collect();
        let expected: Vec<(String, u32)> = [
            ("idle", 0),
            ("run", 0),
            ("jump", 0),
            ("wave", 1),
            ("nod", 1),
            ("leap", 1),
        ]
        .map(|(id, pack)| (id.to_owned(), pack))
        .to_vec();
        assert_eq!(clips, expected, "own clips, then the pack's");
        let joints = joint_index(&mut bridge, resource);
        let appearance = bridge
            .create_animated_mesh_appearance(NativeAnimatedMeshAppearanceRequest { resource })
            .unwrap();
        let instance = bridge
            .create_animation_instance(NativeAnimationInstanceRequest {
                appearance,
                object_id: 7,
            })
            .unwrap();
        let fact = super::super::tests::appearance_fact(appearance);
        unsafe { bridge.stage_snapshot(&fact, 1) }.unwrap();
        bridge
            .set_animation_playback(&NativeAnimationPlaybackRequest {
                instance,
                kind: NativeAnimationPlaybackKind::Play,
                clip: utf8("run"),
                loop_mode: NativeAnimationLoopMode::Repeat,
                speed: 1.0,
                weight: 1.0,
                restart: true,
                fade_seconds: 0.0,
                has_fade: false,
                normalized_time: 0.0,
            })
            .unwrap();
        let current = NativeAnimationPoseSampleRequest {
            instance,
            clip: utf8(""),
            time: 0.0,
            blend_clip: utf8(""),
            blend_time: 0.0,
            blend_weight: 0.0,
            normalized: false,
            placement: placed_at(0.0),
            apply_pose_controls: false,
            use_current_playback: true,
        };
        let rotation = |joints: &[NativeJointPose], joint: u32| {
            let q = joints[joint as usize].model.rotation;
            [q.x, q.y, q.z, q.w]
        };
        let base = sampled(&mut bridge, &current).unwrap();
        let (arm, head, leg) = (joints["RightArm"], joints["Head"], joints["LeftUpLeg"]);
        let layers = [
            NativeAnimationClipLayer {
                clip: 3,
                time: 0.25,
                normalized: true,
                weight: 0.5,
                additive: false,
                reference_time: 0.0,
                joint_weights_start: 0,
                joint_weights_len: 2,
            },
            NativeAnimationClipLayer {
                clip: 4,
                time: 0.5,
                normalized: true,
                weight: 1.0,
                additive: true,
                reference_time: 0.0,
                joint_weights_start: 0,
                joint_weights_len: 0,
            },
        ];
        let arms = [
            NativeAnimationJointWeight {
                joint: arm,
                weight: 1.0,
            },
            NativeAnimationJointWeight {
                joint: joints["RightForeArm"],
                weight: 1.0,
            },
        ];
        let set = |bridge: &mut RuntimeAppearanceBridge, layers: &[NativeAnimationClipLayer]| {
            bridge.set_animation_layers(&NativeAnimationLayersRequest {
                instance,
                layers: layers.as_ptr(),
                layers_len: layers.len(),
                joint_weights: arms.as_ptr(),
                joint_weights_len: arms.len(),
            })
        };
        set(&mut bridge, &layers).unwrap();
        let layered = sampled(&mut bridge, &current).unwrap();
        assert_ne!(
            rotation(&layered, arm),
            rotation(&base, arm),
            "the arm waves"
        );
        assert_ne!(
            rotation(&layered, head),
            rotation(&base, head),
            "the head nods"
        );
        assert_eq!(
            rotation(&layered, leg),
            rotation(&base, leg),
            "the run's legs"
        );
        // The layers reach the renderer with the instance's pose as the
        // call settles.
        let call = bridge.take_staged_call();
        let sent = call.render_ops().into_iter().rev().find_map(|op| match op {
            RenderDiff::SetAnimatedMeshPose { pose, .. } => Some(pose),
            _ => None,
        });
        assert_eq!(sent.map(|pose| pose.clip_layers.len()), Some(2));
        bridge.commit(call);

        bridge.set_presentation_clock(0.25, 0.25);
        bridge.begin_call();
        let refused = set(
            &mut bridge,
            &[NativeAnimationClipLayer {
                clip: 6,
                ..layers[0]
            }],
        )
        .unwrap_err();
        assert!(
            refused.to_string().contains("outside the mesh's 6 clips"),
            "{refused}"
        );
        set(&mut bridge, &[]).unwrap();
        let cleared = sampled(&mut bridge, &current).unwrap();
        let run_now = sampled(
            &mut bridge,
            &NativeAnimationPoseSampleRequest {
                clip: utf8("run"),
                time: 0.5,
                use_current_playback: false,
                ..current
            },
        )
        .unwrap();
        assert_eq!(rotation(&cleared, arm), rotation(&run_now, arm), "cleared");
    }

    /// A decoded pose model lives with its resource: released with it, and
    /// decoded afresh when the model is opened again.
    #[test]
    fn a_sampled_model_is_released_with_its_resource() {
        let mut bridge = RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), {
            let mut content = BTreeMap::new();
            content.insert(
                "body.glb".to_owned(),
                crate::ContentBytes::from(CHARACTER_GLB),
            );
            content
        });
        let path = b"body.glb";
        let open = |bridge: &mut RuntimeAppearanceBridge| {
            bridge
                .open_animated_mesh(&NativeAnimatedMeshResourceRequest {
                    path: NativeUtf8Slice {
                        bytes: path.as_ptr(),
                        len: path.len(),
                    },
                })
                .expect("admitted animated GLB")
        };
        let sample = |bridge: &mut RuntimeAppearanceBridge, resource| {
            let result = bridge
                .sample_animation_model_joint_pose(&NativeAnimationModelPoseSampleRequest {
                    resource,
                    clip: utf8("run"),
                    time: 0.5,
                    blend_clip: utf8(""),
                    blend_time: 0.0,
                    blend_weight: 0.0,
                    normalized: true,
                    placement: placed_at(0.0),
                })
                .expect("a sample");
            assert!(result.joints_len > 0);
        };
        let end_call = |bridge: &mut RuntimeAppearanceBridge| {
            let call = bridge.take_staged_call();
            bridge.commit(call);
        };
        bridge.begin_call();
        let first = open(&mut bridge);
        sample(&mut bridge, first);
        let decoded = Arc::downgrade(&bridge.pose_model(first.value).unwrap());
        sample(&mut bridge, first);
        assert!(
            Arc::ptr_eq(
                &decoded.upgrade().unwrap(),
                &bridge.pose_model(first.value).unwrap()
            ),
            "a live resource keeps its decoded model"
        );
        end_call(&mut bridge);

        bridge.begin_call();
        bridge.destroy_resource(first).unwrap();
        end_call(&mut bridge);
        // Released bodies stay servable for the call after their release.
        for _ in 0..2 {
            bridge.begin_call();
            end_call(&mut bridge);
        }
        assert!(
            decoded.upgrade().is_none(),
            "the decoded model went with its resource"
        );

        bridge.begin_call();
        let again = open(&mut bridge);
        sample(&mut bridge, again);
        end_call(&mut bridge);
    }

    #[test]
    fn a_pose_is_checked_against_the_rig_sent_once_when_it_changes_and_its_joints_read_back() {
        let mut content_resources = BTreeMap::new();
        content_resources.insert(
            "body.glb".to_owned(),
            crate::ContentBytes::from(CHARACTER_GLB),
        );
        let mut bridge =
            RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), content_resources);
        let mut world = render_presentation::PresentationWorld::default();
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
        let unposed = bridge.read_animation_joint_pose(instance).unwrap();
        assert!(!unposed.reported, "no settle has posed it yet");
        let call = bridge.take_staged_call();
        let sent: Vec<_> = call
            .render_ops()
            .into_iter()
            .filter(|op| matches!(op, RenderDiff::SetAnimatedMeshPose { .. }))
            .collect();
        assert_eq!(sent.len(), 1);
        settle_call_posed(&mut bridge, call, &mut world, 0.0);

        // Rewritten unchanged each update, it is not sent again.
        bridge.begin_call();
        bridge.set_animation_pose(&request(&chain)).unwrap();
        let call = bridge.take_staged_call();
        assert!(!call
            .render_ops()
            .iter()
            .any(|op| matches!(op, RenderDiff::SetAnimatedMeshPose { .. })));
        settle_call_posed(&mut bridge, call, &mut world, 0.0);

        // The pose the first settle evaluated reads back in later calls, with
        // the IK chain's target and how far the drawn hand is from it.
        bridge.begin_call();
        let pose = bridge.read_animation_joint_pose(instance).unwrap();
        assert!(pose.reported);
        assert_eq!((pose.joints_len, pose.seconds), (joints.len(), 0.0));
        let read = unsafe { *pose.joints.add(hand as usize) };
        assert!(
            (read.world.translation.x - read.model.translation.x).abs() < 1e-5,
            "placed at the origin"
        );
        assert_eq!(pose.ik_chains_len, 1);
        let chain = unsafe { *pose.ik_chains };
        assert_eq!(
            (
                chain.target_world.x,
                chain.target_world.y,
                chain.target_world.z
            ),
            (0.3, 1.4, 0.4)
        );
        let reached = (chain.end_world.x - read.world.translation.x).abs() < 1e-5
            && (chain.end_world.y - read.world.translation.y).abs() < 1e-5;
        assert!(reached, "the chain ends at the drawn hand");

        // Destroying the instance clears the controls it left on the target;
        // its pose goes with it.
        bridge.destroy_animation_instance(instance).unwrap();
        assert_eq!(pose_ops(&bridge), [AnimatedMeshPose::default()]);
        assert!(bridge.read_animation_joint_pose(instance).is_err());
        end_call_posed(&mut bridge, &mut world, 0.25);
    }
}
