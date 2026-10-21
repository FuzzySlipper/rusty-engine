//! Severed parts: a joint and its descendants cut off an animation
//! instance and spawned as one Dynamics body. The instance stops drawing the
//! subtree; the piece is drawn from the same mesh, posed as it was at the
//! cut, wherever its body is after every step.

use engine_spatial::{RagdollBone, RagdollMaterial, RagdollSpawn};
use render_model::{JointCut, Transform};

use super::ragdoll::{bone_spec, placement, ragdoll_call};
use super::*;

pub(super) struct SeveredPart {
    pub(super) world: u64,
    instance: u64,
    joint: u32,
    bone: RagdollBone,
    /// Every rig joint at the cut, in the instance's space.
    pose: Vec<Transform>,
    /// The cut joint's scale in the scene at the cut.
    scale: [f32; 3],
}

pub(super) enum SeveredSlot {
    Active(SeveredPart),
    Tombstoned,
}

fn severed_error(message: impl Into<String>) -> CsharpEngineServicesError {
    CsharpEngineServicesError::new("CSHARP_SEVERED_PART", message)
}

impl RuntimeDynamicsBridge {
    fn create_severed_part(
        &mut self,
        request: NativeDynamicsSeveredPartRequest,
    ) -> Result<NativeDynamicsSeveredPartHandle, CsharpEngineServicesError> {
        self.active_world(request.world.value)?;
        let appearance = self.appearance()?;
        let source =
            appearance.ragdoll_source(request.instance.value, "CSHARP_SEVERED_PART_POSE")?;
        let (tip, shape, mass) = bone_spec(&source, &request.bone, "CSHARP_SEVERED_PART")?;
        let joint = request.bone.joint;
        if appearance.joint_cut_off(request.instance.value, joint)? {
            return Err(severed_error(format!("joint {joint} is cut off already")));
        }
        let at = |joints: &[render_model::PosedJoint]| vec![placement(&joints[joint as usize])];
        let spawn = RagdollSpawn {
            rest: at(&source.rest),
            current: at(&source.latest.joints),
            previous: source.previous.as_ref().map(|previous| {
                (
                    at(&previous.joints),
                    source.latest.seconds - previous.seconds,
                )
            }),
        };
        let bone = RagdollBone {
            body: DynamicsBodyId(Self::allocate(&mut self.next_body)),
            tip,
            shape,
            mass,
        };
        let material = RagdollMaterial {
            collision_groups: request.collision_groups,
            collision_mask: request.collision_mask,
            friction: f64::from(request.friction),
            restitution: f64::from(request.restitution),
            linear_damping: f64::from(request.linear_damping),
            angular_damping: f64::from(request.angular_damping),
        };
        let solver = &mut self.active_world_mut(request.world.value)?.solver;
        solver
            .insert_ragdoll(&[bone], &[], material, &spawn)
            .map_err(solver_error("CSHARP_SEVERED_PART"))?;
        // The product's push, over the motion the pose had.
        let body = solver.body(bone.body).expect("inserted above");
        let add = |moving: [f64; 3], push: NativeVec3| {
            let push = vec3_f64(native_vec3_value(push));
            [0, 1, 2].map(|axis| moving[axis] + push[axis])
        };
        if let Err(error) = solver.set_body_motion(
            bone.body,
            body.translation,
            body.rotation,
            add(body.linear_velocity, request.linear_velocity),
            add(body.angular_velocity, request.angular_velocity),
            false,
        ) {
            solver.remove_body(bone.body);
            return Err(solver_error("CSHARP_SEVERED_PART")(error));
        }
        let handle = Self::allocate(&mut self.next_severed_part);
        self.severed_parts.insert(
            handle,
            SeveredSlot::Active(SeveredPart {
                world: request.world.value,
                instance: request.instance.value,
                joint,
                bone,
                pose: source
                    .latest
                    .joints
                    .iter()
                    .map(|joint| joint.model)
                    .collect(),
                scale: source.latest.joints[joint as usize].world.scale,
            }),
        );
        self.place_severed_part(handle)?;
        Ok(NativeDynamicsSeveredPartHandle { value: handle })
    }

    fn active_severed_part(&self, handle: u64) -> Result<&SeveredPart, CsharpEngineServicesError> {
        match self.severed_parts.get(&handle) {
            Some(SeveredSlot::Active(part)) => Ok(part),
            Some(SeveredSlot::Tombstoned) => Err(tombstoned("severed part")),
            None => Err(unknown("severed part", handle)),
        }
    }

    /// Cut the part's joint off its instance and place the piece at its
    /// body. A part whose instance is gone is released.
    fn place_severed_part(&mut self, handle: u64) -> Result<(), CsharpEngineServicesError> {
        let part = self.active_severed_part(handle)?;
        let world = self.active_world(part.world)?;
        let Some(at) = world.solver.ragdoll_placements(&[part.bone])[0] else {
            return Ok(());
        };
        let cut = JointCut {
            joint: part.joint,
            pose: part.pose.clone(),
            placement: Transform {
                translation: at.translation.map(|value| value as f32),
                rotation: at.rotation.map(|value| value as f32),
                scale: part.scale,
            },
        };
        let instance = part.instance;
        if !self
            .appearance()?
            .set_joint_cut(instance, handle, Some(cut))?
        {
            self.release_severed_part(handle)?;
        }
        Ok(())
    }

    /// Before a step of `world`, release the parts whose instance is gone.
    pub(super) fn release_orphaned_parts_of(
        &mut self,
        world: u64,
    ) -> Result<(), CsharpEngineServicesError> {
        if self.appearance.is_none() {
            return Ok(());
        }
        for handle in self.severed_parts_of(world) {
            let SeveredSlot::Active(part) = &self.severed_parts[&handle] else {
                continue;
            };
            let instance = part.instance;
            if !self.appearance()?.animation_instance_lives(instance)? {
                self.release_severed_part(handle)?;
            }
        }
        Ok(())
    }

    /// After a step of `world`, its parts are placed again.
    pub(super) fn place_severed_parts_of(
        &mut self,
        world: u64,
    ) -> Result<(), CsharpEngineServicesError> {
        for handle in self.severed_parts_of(world) {
            self.place_severed_part(handle)?;
        }
        Ok(())
    }

    pub(super) fn severed_parts_of(&self, world: u64) -> Vec<u64> {
        self.severed_parts
            .iter()
            .filter_map(|(handle, slot)| match slot {
                SeveredSlot::Active(part) if part.world == world => Some(*handle),
                _ => None,
            })
            .collect()
    }

    /// Remove a part's body and give its joint back to the instance.
    pub(super) fn release_severed_part(
        &mut self,
        handle: u64,
    ) -> Result<(), CsharpEngineServicesError> {
        let Some(SeveredSlot::Active(part)) =
            self.severed_parts.insert(handle, SeveredSlot::Tombstoned)
        else {
            return Ok(());
        };
        if let Some(WorldSlot::Active(world)) = self.worlds.get_mut(&part.world) {
            world.solver.remove_body(part.bone.body);
        }
        if self.appearance.is_some() {
            self.appearance()?
                .set_joint_cut(part.instance, handle, None)?;
        }
        Ok(())
    }

    fn destroy_severed_part(
        &mut self,
        handle: NativeDynamicsSeveredPartHandle,
    ) -> Result<(), CsharpEngineServicesError> {
        match self.severed_parts.get(&handle.value) {
            Some(SeveredSlot::Active(_)) => self.release_severed_part(handle.value),
            Some(SeveredSlot::Tombstoned) => Ok(()),
            None => Err(unknown("severed part", handle.value)),
        }
    }

    fn read_severed_part(
        &mut self,
        handle: NativeDynamicsSeveredPartHandle,
    ) -> Result<NativeDynamicsSeveredPartReadout, CsharpEngineServicesError> {
        let instance = self.active_severed_part(handle.value)?.instance;
        if !self.appearance()?.animation_instance_lives(instance)? {
            self.release_severed_part(handle.value)?;
        }
        let part = self.active_severed_part(handle.value)?;
        let world = self.active_world(part.world)?;
        let body = world
            .solver
            .body(part.bone.body)
            .ok_or_else(|| severed_error("the part's body is gone"))?;
        let at = world.solver.ragdoll_placements(&[part.bone])[0]
            .ok_or_else(|| severed_error("the part's body is gone"))?;
        let [x, y, z, w] = at.rotation.map(|value| value as f32);
        Ok(NativeDynamicsSeveredPartReadout {
            placement: NativeTransform {
                translation: native_vec3(vec3_f32(at.translation)),
                rotation: NativeQuat { x, y, z, w },
                scale: native_vec3(Vec3::new(part.scale[0], part.scale[1], part.scale[2])),
            },
            resting: body.sleeping,
        })
    }
}

pub(super) unsafe extern "C" fn create_severed_part(
    context: *mut c_void,
    request: NativeDynamicsSeveredPartRequest,
    handle: *mut NativeDynamicsSeveredPartHandle,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    ragdoll_call(context, receipt, handle, |bridge| {
        bridge.create_severed_part(request)
    })
}

pub(super) unsafe extern "C" fn destroy_severed_part(
    context: *mut c_void,
    handle: NativeDynamicsSeveredPartHandle,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    let mut done = ();
    ragdoll_call(context, receipt, &mut done, |bridge| {
        bridge.destroy_severed_part(handle)
    })
}

pub(super) unsafe extern "C" fn read_severed_part(
    context: *mut c_void,
    handle: NativeDynamicsSeveredPartHandle,
    result: *mut NativeDynamicsSeveredPartReadout,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    ragdoll_call(context, receipt, result, |bridge| {
        bridge.read_severed_part(handle)
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Arc;

    use render_model::{PosedJoint, RenderDiff};

    use super::*;
    use crate::appearance::{character_for_test, cuts_for_test, JointPoseReport};

    const BODY_GLB: &[u8] =
        include_bytes!("../../../../../fixtures/csharp-joint-attachments/content/body.glb");
    const TICK: f32 = 1.0 / 60.0;
    const NO_END_JOINT: u32 = u32::MAX;

    /// Every joint at `height`, moving up at 0.6 m/s between two reports.
    fn report(joints: usize, seconds: f64, height: f32, rest: bool) -> JointPoseReport {
        let at = |height: f32| {
            let transform = Transform {
                translation: [0.0, height, 0.0],
                ..Transform::IDENTITY
            };
            vec![
                PosedJoint {
                    model: transform,
                    world: transform,
                };
                joints
            ]
        };
        JointPoseReport {
            object_id: 7,
            generation: 1,
            seconds,
            world: Transform::IDENTITY,
            joints: at(height),
            rest: rest.then(|| at(1.5)),
            ik_chains: Vec::new(),
        }
    }

    fn step(dynamics: &mut RuntimeDynamicsBridge, world: NativeDynamicsWorldHandle, steps: u32) {
        let actions: [NativeDynamicsAction; 0] = [];
        dynamics
            .step(&NativeDynamicsStepRequest {
                world,
                step_seconds: TICK,
                steps,
                actions: actions.as_ptr(),
                actions_len: 0,
            })
            .unwrap();
    }

    #[test]
    fn a_severed_part_cuts_its_joint_off_falls_as_a_body_and_is_released_with_its_instance() {
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
                gravity: NativeVec3 {
                    x: 0.0,
                    y: -9.81,
                    z: 0.0,
                },
            })
            .unwrap();
        let bone = |joint: &str| NativeDynamicsRagdollBone {
            joint: joints[joint],
            end_joint: NO_END_JOINT,
            length: 0.25,
            shape: NativeDynamicsRagdollShape::Capsule,
            radius: 0.1,
            half_depth: 0.0,
            mass: 4.0,
        };
        let request = |joint: &str| NativeDynamicsSeveredPartRequest {
            world,
            instance,
            bone: bone(joint),
            linear_velocity: NativeVec3 {
                x: 2.0,
                y: 0.0,
                z: 0.0,
            },
            angular_velocity: NativeVec3::default(),
            collision_groups: 1,
            collision_mask: u32::MAX,
            friction: 0.5,
            restitution: 0.0,
            linear_damping: 0.0,
            angular_damping: 0.0,
        };
        let unreported = dynamics.create_severed_part(request("Head")).unwrap_err();
        assert_eq!(unreported.code(), "CSHARP_SEVERED_PART_POSE");
        let call = appearance.take_staged_call();
        appearance.commit(call);
        appearance.ingest_joint_poses([
            report(joints.len(), 1.0, 1.5, true),
            report(joints.len(), 1.0 + 1.0 / 60.0, 1.51, false),
        ]);

        appearance.begin_call();
        dynamics.bind_appearance(&mut appearance);
        let head = dynamics.create_severed_part(request("Head")).unwrap();
        // Cut where the joint was drawn, moving as the pose moved plus the push.
        let cuts = cuts_for_test(&appearance, instance).unwrap();
        assert_eq!(cuts.len(), 1);
        assert_eq!(
            (cuts[0].joint, cuts[0].pose.len()),
            (joints["Head"], joints.len())
        );
        assert!((cuts[0].placement.translation[1] - 1.51).abs() < 1e-5);
        let SeveredSlot::Active(part) = &dynamics.severed_parts[&head.value] else {
            unreachable!()
        };
        let velocity = dynamics
            .active_world(world.value)
            .unwrap()
            .solver
            .body(part.bone.body)
            .unwrap()
            .linear_velocity;
        assert!((velocity[0] - 2.0).abs() < 1e-6 && (velocity[1] - 0.6).abs() < 1e-3);
        // The head, or a joint under it, is cut off already; an arm is not.
        let again = dynamics.create_severed_part(request("Head")).unwrap_err();
        assert_eq!(again.code(), "CSHARP_SEVERED_PART");
        let arm = dynamics.create_severed_part(request("LeftArm")).unwrap();
        assert_eq!(cuts_for_test(&appearance, instance).unwrap().len(), 2);

        // A step moves the piece; the call sends the cuts as the pose.
        step(&mut dynamics, world, 10);
        let read = dynamics.read_severed_part(head).unwrap();
        assert!(read.placement.translation.x > 0.3, "{read:?}");
        assert!(!read.resting);
        let call = appearance.take_staged_call();
        let sent = call
            .render_ops()
            .into_iter()
            .find_map(|op| match op {
                RenderDiff::SetAnimatedMeshPose { pose, .. } => Some(pose),
                _ => None,
            })
            .expect("the cuts are sent");
        assert_eq!(sent.cuts.len(), 2);
        appearance.commit(call);

        // Disposing a part gives its joint back and removes its body.
        appearance.begin_call();
        dynamics.bind_appearance(&mut appearance);
        dynamics.destroy_severed_part(arm).unwrap();
        assert_eq!(cuts_for_test(&appearance, instance).unwrap().len(), 1);
        assert!(dynamics.read_severed_part(arm).is_err());
        let bodies = |dynamics: &RuntimeDynamicsBridge| {
            dynamics
                .active_world(world.value)
                .unwrap()
                .solver
                .body_count()
        };
        assert_eq!(bodies(&dynamics), 1);

        // Destroying a world releases its parts.
        let other = dynamics
            .create_world(NativeDynamicsWorldConfig {
                gravity: NativeVec3::default(),
            })
            .unwrap();
        let arm = dynamics
            .create_severed_part(NativeDynamicsSeveredPartRequest {
                world: other,
                ..request("LeftArm")
            })
            .unwrap();
        dynamics.destroy_world(other).unwrap();
        assert_eq!(cuts_for_test(&appearance, instance).unwrap().len(), 1);
        assert!(dynamics.read_severed_part(arm).is_err());

        // Destroying the instance releases the other part's body.
        let mut receipt = crate::operation_diagnostics::empty_receipt();
        let status = unsafe {
            crate::appearance::destroy_animation_instance(
                (&mut appearance as *mut crate::appearance::RuntimeAppearanceBridge).cast(),
                instance,
                &mut receipt,
            )
        };
        assert_eq!(status, ABI_OK);
        step(&mut dynamics, world, 1);
        assert_eq!(bodies(&dynamics), 0);
        assert!(dynamics.read_severed_part(head).is_err());
        dynamics.destroy_severed_part(head).unwrap();
    }
}
