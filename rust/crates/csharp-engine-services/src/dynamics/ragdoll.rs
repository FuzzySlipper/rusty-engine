//! Limited joints between Dynamics bodies, and ragdolls: bodies per bone of
//! an animation instance, spawned at its reported pose and placing its bones
//! through world-space pose overrides after every step.

use engine_spatial::{
    DynamicsJoint, DynamicsJointFrame, DynamicsJointLimit, DynamicsPlacement, RagdollBone,
    RagdollLink, RagdollMaterial, RagdollShape, RagdollSpawn,
};
use render_model::{JointOverride, PoseSpace};

use super::*;

/// A bone without an end joint.
const NO_END_JOINT: u32 = u32::MAX;

pub(super) struct Ragdoll {
    pub(super) world: u64,
    instance: u64,
    bones: Vec<RagdollBone>,
    /// The rig joint each bone places.
    joints: Vec<u32>,
    links: Vec<u64>,
    blend: f32,
}

pub(super) enum RagdollSlot {
    Active(Ragdoll),
    Tombstoned,
}

fn ragdoll_error(message: impl Into<String>) -> CsharpEngineServicesError {
    CsharpEngineServicesError::new("CSHARP_RAGDOLL", message)
}

fn blend(value: f32) -> Result<f32, CsharpEngineServicesError> {
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        Ok(value)
    } else {
        Err(ragdoll_error("blend must be between 0 and 1"))
    }
}

fn limit(limits: NativeDynamicsJointLimits) -> DynamicsJointLimit {
    let (min, max) = (f64::from(limits.min_angle), f64::from(limits.max_angle));
    match limits.kind {
        NativeDynamicsJointKind::Hinge => DynamicsJointLimit::Hinge { min, max },
        NativeDynamicsJointKind::Cone => DynamicsJointLimit::Cone {
            swing: f64::from(limits.swing_angle),
            twist_min: min,
            twist_max: max,
        },
    }
}

fn placement(joint: &render_model::PosedJoint) -> DynamicsPlacement {
    DynamicsPlacement {
        translation: joint.world.translation.map(f64::from),
        rotation: joint.world.rotation.map(f64::from),
    }
}

impl RuntimeDynamicsBridge {
    /// The sibling Animation bridge, for ragdolls. Refreshed when the
    /// NativeEngineApi is assembled; used only within a product call.
    pub(crate) fn bind_appearance(
        &mut self,
        appearance: &mut crate::appearance::RuntimeAppearanceBridge,
    ) {
        self.appearance = Some(appearance as *mut crate::appearance::RuntimeAppearanceBridge);
    }

    fn appearance(
        &mut self,
    ) -> Result<&mut crate::appearance::RuntimeAppearanceBridge, CsharpEngineServicesError> {
        let appearance = self
            .appearance
            .ok_or_else(|| ragdoll_error("ragdolls need the Animation service"))?;
        // SAFETY: EngineServiceSet owns both bridges and refreshes this
        // pointer whenever it hands out the function tables.
        Ok(unsafe { &mut *appearance })
    }

    fn set_joint(
        &mut self,
        request: NativeDynamicsJointRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        for body in [request.first, request.second] {
            self.world_body(request.world.value, body.value, "CSHARP_DYNAMICS_JOINT")?;
        }
        let frame = |anchor: NativeVec3, rotation: NativeQuat| DynamicsJointFrame {
            anchor: vec3_f64(native_vec3_value(anchor)),
            rotation: quat_f64(native_quat_value(rotation)),
        };
        self.active_world_mut(request.world.value)?
            .solver
            .set_joint(DynamicsJoint {
                id: request.id,
                first: DynamicsBodyId(request.first.value),
                second: DynamicsBodyId(request.second.value),
                first_frame: frame(request.first_anchor, request.first_rotation),
                second_frame: frame(request.second_anchor, request.second_rotation),
                limit: limit(request.limits),
                damping: f64::from(request.limits.damping),
                contacts_enabled: request.contacts_enabled,
            })
            .map_err(solver_error("CSHARP_DYNAMICS_JOINT"))
    }

    fn remove_joint(
        &mut self,
        request: NativeDynamicsJointRemoveRequest,
    ) -> Result<NativeDynamicsJointReleaseReceipt, CsharpEngineServicesError> {
        Ok(NativeDynamicsJointReleaseReceipt {
            released: self
                .active_world_mut(request.world.value)?
                .solver
                .remove_joint(request.id),
        })
    }

    fn create_ragdoll(
        &mut self,
        request: &NativeDynamicsRagdollRequest,
    ) -> Result<NativeDynamicsRagdollHandle, CsharpEngineServicesError> {
        let bones = unsafe { borrowed_slice(request.bones, request.bones_len, "ragdoll bones") }?;
        let links = unsafe { borrowed_slice(request.links, request.links_len, "ragdoll links") }?;
        let blend = blend(request.blend)?;
        self.active_world(request.world.value)?;
        let source = self.appearance()?.ragdoll_source(request.instance.value)?;
        let joint = |index: u32| {
            source.rest.get(index as usize).ok_or_else(|| {
                ragdoll_error(format!(
                    "joint {index} is outside the rig's {} joints",
                    source.joints
                ))
            })
        };
        if bones.is_empty() {
            return Err(ragdoll_error("a ragdoll needs a bone"));
        }
        if links
            .iter()
            .any(|link| link.parent as usize >= bones.len() || link.child as usize >= bones.len())
        {
            return Err(ragdoll_error(
                "a link names a bone the ragdoll does not have",
            ));
        }
        let mut specs = Vec::with_capacity(bones.len());
        for bone in bones {
            let rest = placement(joint(bone.joint)?);
            let tip = if bone.end_joint == NO_END_JOINT {
                [0.0, f64::from(bone.length), 0.0]
            } else {
                rest.local_point(placement(joint(bone.end_joint)?).translation)
            };
            let shape = match bone.shape {
                NativeDynamicsRagdollShape::Capsule => RagdollShape::Capsule {
                    radius: f64::from(bone.radius),
                },
                NativeDynamicsRagdollShape::Box => RagdollShape::Box {
                    half_width: f64::from(bone.radius),
                    half_depth: f64::from(bone.half_depth),
                },
            };
            specs.push((tip, shape, f64::from(bone.mass)));
        }
        let placements = |joints: &[render_model::PosedJoint]| {
            bones
                .iter()
                .map(|bone| placement(&joints[bone.joint as usize]))
                .collect::<Vec<_>>()
        };
        let spawn = RagdollSpawn {
            rest: placements(&source.rest),
            current: placements(&source.latest.joints),
            previous: source.previous.as_ref().map(|previous| {
                (
                    placements(&previous.joints),
                    source.latest.seconds - previous.seconds,
                )
            }),
        };
        let mut solver_bones = Vec::with_capacity(bones.len());
        for (tip, shape, mass) in specs {
            solver_bones.push(RagdollBone {
                body: DynamicsBodyId(Self::allocate(&mut self.next_body, "body")?),
                tip,
                shape,
                mass,
            });
        }
        let mut solver_links = Vec::with_capacity(links.len());
        for link in links {
            let id = self.next_internal_joint;
            self.next_internal_joint -= 1;
            solver_links.push(RagdollLink {
                joint: id,
                parent: link.parent as usize,
                child: link.child as usize,
                axis: vec3_f64(native_vec3_value(link.axis)),
                limit: limit(link.limits),
                damping: f64::from(link.limits.damping),
            });
        }
        let material = RagdollMaterial {
            collision_groups: request.collision_groups,
            collision_mask: request.collision_mask,
            friction: f64::from(request.friction),
            restitution: f64::from(request.restitution),
            linear_damping: f64::from(request.linear_damping),
            angular_damping: f64::from(request.angular_damping),
        };
        self.active_world_mut(request.world.value)?
            .solver
            .insert_ragdoll(&solver_bones, &solver_links, material, &spawn)
            .map_err(solver_error("CSHARP_RAGDOLL"))?;
        let handle = Self::allocate(&mut self.next_ragdoll, "ragdoll")?;
        self.ragdolls.insert(
            handle,
            RagdollSlot::Active(Ragdoll {
                world: request.world.value,
                instance: request.instance.value,
                bones: solver_bones,
                joints: bones.iter().map(|bone| bone.joint).collect(),
                links: solver_links.iter().map(|link| link.joint).collect(),
                blend,
            }),
        );
        self.place_ragdoll(handle)?;
        Ok(NativeDynamicsRagdollHandle { value: handle })
    }

    fn active_ragdoll(&self, handle: u64) -> Result<&Ragdoll, CsharpEngineServicesError> {
        match self.ragdolls.get(&handle) {
            Some(RagdollSlot::Active(ragdoll)) => Ok(ragdoll),
            Some(RagdollSlot::Tombstoned) => Err(tombstoned("ragdoll")),
            None => Err(unknown("ragdoll", handle)),
        }
    }

    /// Place the ragdoll's bones over its instance's clips from its bodies.
    fn place_ragdoll(&mut self, handle: u64) -> Result<(), CsharpEngineServicesError> {
        let ragdoll = self.active_ragdoll(handle)?;
        let world = self.active_world(ragdoll.world)?;
        let overrides = world
            .solver
            .ragdoll_placements(&ragdoll.bones)
            .into_iter()
            .zip(&ragdoll.joints)
            .filter_map(|(placement, joint)| {
                let placement = placement?;
                Some(JointOverride {
                    joint: *joint,
                    space: PoseSpace::World,
                    additive: false,
                    rotation: Some(placement.rotation.map(|value| value as f32)),
                    translation: Some(placement.translation.map(|value| value as f32)),
                    weight: ragdoll.blend,
                })
            })
            .collect();
        let instance = ragdoll.instance;
        self.appearance()?
            .set_ragdoll_overrides(instance, overrides)
    }

    /// After a step of `world`, its ragdolls place their bones again.
    pub(super) fn place_ragdolls_of(
        &mut self,
        world: u64,
    ) -> Result<(), CsharpEngineServicesError> {
        let handles: Vec<u64> = self
            .ragdolls
            .iter()
            .filter_map(|(handle, slot)| match slot {
                RagdollSlot::Active(ragdoll) if ragdoll.world == world => Some(*handle),
                _ => None,
            })
            .collect();
        for handle in handles {
            self.place_ragdoll(handle)?;
        }
        Ok(())
    }

    /// Remove a ragdoll's bodies and joints and release its bones to the
    /// clips. A ragdoll of a destroyed world only releases its bones.
    pub(super) fn release_ragdoll(&mut self, handle: u64) -> Result<(), CsharpEngineServicesError> {
        let Some(RagdollSlot::Active(ragdoll)) =
            self.ragdolls.insert(handle, RagdollSlot::Tombstoned)
        else {
            return Ok(());
        };
        if let Some(WorldSlot::Active(world)) = self.worlds.get_mut(&ragdoll.world) {
            for link in &ragdoll.links {
                world.solver.remove_joint(*link);
            }
            for bone in &ragdoll.bones {
                world.solver.remove_body(bone.body);
            }
        }
        if self.appearance.is_some() {
            self.appearance()?
                .set_ragdoll_overrides(ragdoll.instance, Vec::new())?;
        }
        Ok(())
    }

    fn destroy_ragdoll(
        &mut self,
        handle: NativeDynamicsRagdollHandle,
    ) -> Result<(), CsharpEngineServicesError> {
        match self.ragdolls.get(&handle.value) {
            Some(RagdollSlot::Active(_)) => self.release_ragdoll(handle.value),
            Some(RagdollSlot::Tombstoned) => Ok(()),
            None => Err(unknown("ragdoll", handle.value)),
        }
    }

    fn set_ragdoll_blend(
        &mut self,
        request: NativeDynamicsRagdollBlendRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let value = blend(request.blend)?;
        self.active_ragdoll(request.ragdoll.value)?;
        if let Some(RagdollSlot::Active(ragdoll)) = self.ragdolls.get_mut(&request.ragdoll.value) {
            ragdoll.blend = value;
        }
        self.place_ragdoll(request.ragdoll.value)
    }

    fn apply_ragdoll_impulse(
        &mut self,
        request: NativeDynamicsRagdollImpulseRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let ragdoll = self.active_ragdoll(request.ragdoll.value)?;
        let body = ragdoll
            .bones
            .get(request.bone as usize)
            .ok_or_else(|| ragdoll_error(format!("the ragdoll has no bone {}", request.bone)))?
            .body;
        let world = ragdoll.world;
        self.active_world_mut(world)?
            .solver
            .apply_impulse_at_point(
                body,
                vec3_f64(native_vec3_value(request.point)),
                vec3_f64(native_vec3_value(request.impulse)),
            )
            .map_err(solver_error("CSHARP_RAGDOLL"))
    }

    fn read_ragdoll(
        &mut self,
        handle: NativeDynamicsRagdollHandle,
    ) -> Result<NativeDynamicsRagdollResult, CsharpEngineServicesError> {
        let ragdoll = self.active_ragdoll(handle.value)?;
        let world = self.active_world(ragdoll.world)?;
        let resting = ragdoll.bones.iter().all(|bone| {
            world
                .solver
                .body(bone.body)
                .is_some_and(|body| body.sleeping)
        });
        let bones = world
            .solver
            .ragdoll_placements(&ragdoll.bones)
            .into_iter()
            .map(|placement| {
                let placement = placement.unwrap_or(DynamicsPlacement {
                    translation: [0.0; 3],
                    rotation: [0.0, 0.0, 0.0, 1.0],
                });
                NativeTransform {
                    translation: native_vec3(vec3_f32(placement.translation)),
                    rotation: {
                        let [x, y, z, w] = placement.rotation.map(|value| value as f32);
                        NativeQuat { x, y, z, w }
                    },
                    scale: NativeVec3 {
                        x: 1.0,
                        y: 1.0,
                        z: 1.0,
                    },
                }
            })
            .collect::<Box<[_]>>();
        let result = NativeDynamicsRagdollResult {
            bones: bones.as_ptr(),
            bones_len: bones.len(),
            resting,
            blend: ragdoll.blend,
        };
        self.ragdoll_bones = bones;
        Ok(result)
    }
}

fn ragdoll_call<T>(
    context: *mut c_void,
    receipt: *mut NativeOperationErrorReceipt,
    output: *mut T,
    call: impl FnOnce(&mut RuntimeDynamicsBridge) -> Result<T, CsharpEngineServicesError>,
) -> i32 {
    clear_receipt(receipt);
    if context.is_null() || output.is_null() {
        return 0;
    }
    match call(unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }) {
        Ok(value) => {
            unsafe { *output = value };
            ABI_OK
        }
        Err(error) => refuse(context, &error, receipt),
    }
}

pub(super) unsafe extern "C" fn set_joint(
    context: *mut c_void,
    request: NativeDynamicsJointRequest,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    let mut done = ();
    ragdoll_call(context, receipt, &mut done, |bridge| {
        bridge.set_joint(request)
    })
}

pub(super) unsafe extern "C" fn remove_joint(
    context: *mut c_void,
    request: NativeDynamicsJointRemoveRequest,
    result: *mut NativeDynamicsJointReleaseReceipt,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    ragdoll_call(context, receipt, result, |bridge| {
        bridge.remove_joint(request)
    })
}

pub(super) unsafe extern "C" fn create_ragdoll(
    context: *mut c_void,
    request: *const NativeDynamicsRagdollRequest,
    handle: *mut NativeDynamicsRagdollHandle,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    if request.is_null() {
        clear_receipt(receipt);
        return 0;
    }
    ragdoll_call(context, receipt, handle, |bridge| {
        bridge.create_ragdoll(unsafe { &*request })
    })
}

pub(super) unsafe extern "C" fn destroy_ragdoll(
    context: *mut c_void,
    handle: NativeDynamicsRagdollHandle,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    let mut done = ();
    ragdoll_call(context, receipt, &mut done, |bridge| {
        bridge.destroy_ragdoll(handle)
    })
}

pub(super) unsafe extern "C" fn set_ragdoll_blend(
    context: *mut c_void,
    request: NativeDynamicsRagdollBlendRequest,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    let mut done = ();
    ragdoll_call(context, receipt, &mut done, |bridge| {
        bridge.set_ragdoll_blend(request)
    })
}

pub(super) unsafe extern "C" fn apply_ragdoll_impulse(
    context: *mut c_void,
    request: NativeDynamicsRagdollImpulseRequest,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    let mut done = ();
    ragdoll_call(context, receipt, &mut done, |bridge| {
        bridge.apply_ragdoll_impulse(request)
    })
}

pub(super) unsafe extern "C" fn read_ragdoll(
    context: *mut c_void,
    handle: NativeDynamicsRagdollHandle,
    result: *mut NativeDynamicsRagdollResult,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    ragdoll_call(context, receipt, result, |bridge| {
        bridge.read_ragdoll(handle)
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Arc;

    use render_model::{PosedJoint, Transform};

    use super::*;
    use crate::appearance::{character_for_test, ragdoll_overrides_for_test, JointPoseReport};

    const BODY_GLB: &[u8] =
        include_bytes!("../../../../../fixtures/csharp-joint-attachments/content/body.glb");
    const TICK: f32 = 1.0 / 60.0;

    fn joint(translation: [f32; 3], rotation: [f32; 4]) -> PosedJoint {
        let transform = Transform {
            translation,
            rotation,
            scale: [1.0; 3],
        };
        PosedJoint {
            model: transform,
            world: transform,
        }
    }

    /// A standing pose: hips at 1 m, the left leg down, everything else at
    /// the origin; `drop` lowers the whole pose.
    fn pose(joints: &BTreeMap<String, u32>, drop: f32) -> Vec<PosedJoint> {
        let down = [0.0, 0.0, 1.0, 0.0];
        let mut pose = vec![joint([0.0; 3], [0.0, 0.0, 0.0, 1.0]); joints.len()];
        for (name, translation, rotation) in [
            ("Hips", [0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]),
            ("Spine", [0.0, 1.25, 0.0], [0.0, 0.0, 0.0, 1.0]),
            ("LeftUpLeg", [-0.1, 1.0, 0.0], down),
            ("LeftLeg", [-0.1, 0.55, 0.0], down),
        ] {
            pose[joints[name] as usize] = joint(
                [translation[0], translation[1] - drop, translation[2]],
                rotation,
            );
        }
        pose
    }

    fn limits(kind: NativeDynamicsJointKind, min: f32, max: f32) -> NativeDynamicsJointLimits {
        NativeDynamicsJointLimits {
            kind,
            min_angle: min,
            max_angle: max,
            swing_angle: 0.6,
            damping: 0.5,
        }
    }

    #[test]
    fn a_ragdoll_spawns_from_the_reported_pose_and_places_its_bones_until_released() {
        let mut appearance = crate::appearance::create(
            Default::default(),
            BTreeMap::from([("body.glb".to_owned(), Arc::<[u8]>::from(BODY_GLB))]),
        );
        appearance.begin_call();
        let (instance, joints) = character_for_test(&mut appearance);
        let call = appearance.take_staged_call();
        appearance.commit(call);
        // Two reports a tick apart: the pose falls at 1.2 m/s.
        let report = |seconds: f64, drop: f32, rest: bool| JointPoseReport {
            object_id: 7,
            generation: 1,
            seconds,
            world: Transform::IDENTITY,
            joints: pose(&joints, drop),
            rest: rest.then(|| pose(&joints, 0.0)),
        };
        appearance.ingest_joint_poses([
            report(1.0, 0.0, true),
            report(1.0 + 1.0 / 60.0, 0.02, false),
        ]);

        let spatial = crate::spatial::RuntimeSpatialBridge::new();
        let mut dynamics = RuntimeDynamicsBridge::new(spatial.collision_source());
        dynamics.bind_appearance(&mut appearance);
        appearance.begin_call();
        let world = dynamics
            .create_world(NativeDynamicsWorldConfig {
                gravity: NativeVec3 {
                    x: 0.0,
                    y: -9.81,
                    z: 0.0,
                },
            })
            .unwrap();
        let bone = |joint: &str, end: Option<&str>, length: f32| NativeDynamicsRagdollBone {
            joint: joints[joint],
            end_joint: end.map_or(NO_END_JOINT, |end| joints[end]),
            length,
            shape: NativeDynamicsRagdollShape::Capsule,
            radius: 0.08,
            half_depth: 0.0,
            mass: 5.0,
        };
        let bones = [
            bone("Hips", Some("Spine"), 0.0),
            bone("LeftUpLeg", Some("LeftLeg"), 0.0),
            bone("LeftLeg", None, 0.45),
        ];
        let links = [
            NativeDynamicsRagdollLink {
                parent: 0,
                child: 1,
                axis: NativeVec3 {
                    x: 0.0,
                    y: 1.0,
                    z: 0.0,
                },
                limits: limits(NativeDynamicsJointKind::Cone, -0.3, 0.3),
            },
            NativeDynamicsRagdollLink {
                parent: 1,
                child: 2,
                axis: NativeVec3 {
                    x: 1.0,
                    y: 0.0,
                    z: 0.0,
                },
                limits: limits(NativeDynamicsJointKind::Hinge, 0.0, 2.4),
            },
        ];
        let request = |instance| NativeDynamicsRagdollRequest {
            world,
            instance,
            bones: bones.as_ptr(),
            bones_len: bones.len(),
            links: links.as_ptr(),
            links_len: links.len(),
            collision_groups: 1,
            collision_mask: u32::MAX,
            friction: 0.8,
            restitution: 0.0,
            linear_damping: 0.0,
            angular_damping: 0.2,
            blend: 1.0,
        };
        let ragdoll = dynamics.create_ragdoll(&request(instance)).unwrap();

        // Spawned where the latest report was, falling as it fell.
        let placed = ragdoll_overrides_for_test(&appearance, instance);
        assert_eq!(placed.len(), 3);
        assert!(placed
            .iter()
            .all(|joint| joint.space == PoseSpace::World && joint.weight == 1.0));
        assert_eq!(placed[0].joint, joints["Hips"]);
        let hips = placed[0].translation.unwrap();
        assert!((hips[1] - 0.98).abs() < 1e-5, "{hips:?}");
        let solver = &dynamics.active_world(world.value).unwrap().solver;
        let RagdollSlot::Active(spawned) = &dynamics.ragdolls[&ragdoll.value] else {
            unreachable!()
        };
        let velocity = solver.body(spawned.bones[0].body).unwrap().linear_velocity;
        assert!((velocity[1] + 1.2).abs() < 1e-3, "{velocity:?}");

        // A step moves the bones; the blend weights them.
        let action: [NativeDynamicsAction; 0] = [];
        dynamics
            .step(&NativeDynamicsStepRequest {
                world,
                step_seconds: TICK,
                steps: 10,
                actions: action.as_ptr(),
                actions_len: 0,
            })
            .unwrap();
        let fallen = ragdoll_overrides_for_test(&appearance, instance);
        assert!(fallen[0].translation.unwrap()[1] < hips[1] - 0.1);
        dynamics
            .set_ragdoll_blend(NativeDynamicsRagdollBlendRequest {
                ragdoll,
                blend: 0.5,
            })
            .unwrap();
        assert!(ragdoll_overrides_for_test(&appearance, instance)
            .iter()
            .all(|joint| joint.weight == 0.5));
        let read = dynamics.read_ragdoll(ragdoll).unwrap();
        assert_eq!((read.bones_len, read.resting, read.blend), (3, false, 0.5));

        // The call sends them as the instance's pose.
        let call = appearance.take_staged_call();
        let sent = call
            .render_ops()
            .into_iter()
            .find_map(|op| match op {
                render_model::RenderDiff::SetAnimatedMeshPose { pose, .. } => Some(pose),
                _ => None,
            })
            .expect("the ragdoll's bones are sent");
        assert_eq!(sent.overrides.len(), 3);
        appearance.commit(call);

        // Disposing the ragdoll removes its bodies and releases the bones.
        appearance.begin_call();
        dynamics.bind_appearance(&mut appearance);
        dynamics.destroy_ragdoll(ragdoll).unwrap();
        assert!(ragdoll_overrides_for_test(&appearance, instance).is_empty());
        let solver = &dynamics.active_world(world.value).unwrap().solver;
        assert_eq!((solver.body_count(), solver.joint_count()), (0, 0));
        assert!(dynamics.read_ragdoll(ragdoll).is_err());
    }

    #[test]
    fn a_ragdoll_needs_a_reported_pose() {
        let mut appearance = crate::appearance::create(
            Default::default(),
            BTreeMap::from([("body.glb".to_owned(), Arc::<[u8]>::from(BODY_GLB))]),
        );
        appearance.begin_call();
        let (instance, joints) = character_for_test(&mut appearance);
        let spatial = crate::spatial::RuntimeSpatialBridge::new();
        let mut dynamics = RuntimeDynamicsBridge::new(spatial.collision_source());
        dynamics.bind_appearance(&mut appearance);
        let world = dynamics
            .create_world(NativeDynamicsWorldConfig {
                gravity: NativeVec3::default(),
            })
            .unwrap();
        let bones = [NativeDynamicsRagdollBone {
            joint: joints["Hips"],
            end_joint: NO_END_JOINT,
            length: 0.3,
            shape: NativeDynamicsRagdollShape::Box,
            radius: 0.15,
            half_depth: 0.1,
            mass: 10.0,
        }];
        let request = NativeDynamicsRagdollRequest {
            world,
            instance,
            bones: bones.as_ptr(),
            bones_len: 1,
            links: std::ptr::null(),
            links_len: 0,
            collision_groups: 1,
            collision_mask: u32::MAX,
            friction: 0.5,
            restitution: 0.0,
            linear_damping: 0.0,
            angular_damping: 0.0,
            blend: 1.0,
        };
        let unreported = dynamics.create_ragdoll(&request).unwrap_err();
        assert_eq!(unreported.code(), "CSHARP_RAGDOLL_POSE");
        assert_eq!(
            dynamics
                .active_world(world.value)
                .unwrap()
                .solver
                .body_count(),
            0
        );
    }
}
