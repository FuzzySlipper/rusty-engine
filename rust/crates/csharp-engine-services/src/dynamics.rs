mod anchor;
mod chain;
mod errors;
mod ragdoll;
use errors::{clear_receipt, refuse};

use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::c_void,
};

use core_math::Vec3;
use csharp_engine_abi::*;
use engine_spatial::{
    rigid_body_component_mass_properties, DynamicsAction, DynamicsBodyId, DynamicsBodyInput,
    DynamicsBodyOutput, DynamicsError, DynamicsMassProperties, DynamicsShape, DynamicsSolver,
    DynamicsTether, DynamicsTetherEndpoint, VoxelCollisionScene,
};
use entity_state::{
    EntityTransform, Quat, RigidBodyComponent, RigidBodyInertiaPolicy, RigidBodyShape,
};

use crate::composition::{
    borrowed_slice, native_quat, native_quat_value, native_vec3, native_vec3_value,
    CsharpEngineServicesError, ABI_OK,
};
use crate::spatial::SpatialCollisionSource;

/// Engine-owned Dynamics worlds. Each world keeps one live solver; C# holds
/// typed world and body handles, and a body handle is also its solver identity.
pub(crate) struct RuntimeDynamicsBridge {
    worlds: BTreeMap<u64, WorldSlot>,
    bodies: BTreeMap<u64, BodySlot>,
    collision_source: SpatialCollisionSource,
    next_world: u64,
    next_body: u64,
    /// Backing for the latest borrowed step/read or world result. Returned
    /// pointers stay valid until the next call on this bridge.
    body_facts: Vec<NativeDynamicsBodyFact>,
    contacts: Vec<NativeDynamicsContact>,
    operation_diagnostics: crate::operation_diagnostics::OperationDiagnostics,
    ragdolls: BTreeMap<u64, ragdoll::RagdollSlot>,
    next_ragdoll: u64,
    /// Ragdoll joint identities, counting down so they stay clear of the
    /// product's own.
    next_internal_joint: u64,
    /// Backing for the latest borrowed ragdoll result.
    ragdoll_bones: Box<[NativeTransform]>,
    /// The sibling Animation bridge ragdolls pose (`bind_appearance`).
    appearance: Option<*mut crate::appearance::RuntimeAppearanceBridge>,
}

#[allow(
    clippy::large_enum_variant,
    reason = "world slots retain live Engine world state inline so lifecycle transitions do not add allocation ownership"
)]
enum WorldSlot {
    Active(DynamicsWorld),
    Tombstoned,
}

struct DynamicsWorld {
    solver: DynamicsSolver,
    /// Publication identity of the Spatial scene last bound as the static
    /// environment. Holding the scene itself would force every voxel edit to
    /// copy it.
    bound_scene: Option<u64>,
    /// Authored shape, mass and material per body; pose and velocity live in
    /// the solver.
    bodies: BTreeMap<u64, RigidBodyComponent>,
    contacts: BTreeMap<u64, BodyContactSummary>,
    invalidated_tethers: BTreeSet<u64>,
    chains: BTreeMap<u64, chain::DynamicsChain>,
    invalidated_chains: BTreeSet<u64>,
}

#[derive(Clone, Copy, Default)]
struct BodyContactSummary {
    count: u32,
    latest: NativeDynamicsContactFact,
}

enum BodySlot {
    Active { world: u64 },
    Tombstoned,
}

fn solver_error(code: &'static str) -> impl Fn(DynamicsError) -> CsharpEngineServicesError {
    move |error| CsharpEngineServicesError::new(code, error.code())
}

impl RuntimeDynamicsBridge {
    fn tether_endpoint(
        &self,
        world: u64,
        body: NativeDynamicsBodyHandle,
        local: NativeVec3,
    ) -> Result<DynamicsTetherEndpoint, CsharpEngineServicesError> {
        self.world_body(world, body.value, "CSHARP_DYNAMICS_TETHER")?;
        Ok(DynamicsTetherEndpoint::Body {
            body: DynamicsBodyId(body.value),
            local_anchor: vec3_f64(native_vec3_value(local)),
        })
    }

    fn set_tether(
        &mut self,
        world: u64,
        config: NativeDynamicsTetherConfig,
        first: DynamicsTetherEndpoint,
        second: DynamicsTetherEndpoint,
    ) -> Result<(), CsharpEngineServicesError> {
        let world = self.active_world_mut(world)?;
        world
            .solver
            .set_tether(DynamicsTether {
                id: config.id,
                first,
                second,
                maximum_length: f64::from(config.maximum_length),
                target_length: f64::from(config.target_length),
                reel_speed: f64::from(config.reel_speed),
                contacts_enabled: config.contacts_enabled,
            })
            .map_err(|error| CsharpEngineServicesError::new(error.code(), error.code()))?;
        world.invalidated_tethers.remove(&config.id);
        Ok(())
    }

    fn set_fixed_tether(
        &mut self,
        request: NativeDynamicsFixedTetherRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let endpoint =
            self.tether_endpoint(request.world.value, request.body, request.local_anchor)?;
        self.set_tether(
            request.world.value,
            request.config,
            DynamicsTetherEndpoint::Fixed(vec3_f64(native_vec3_value(request.world_anchor))),
            endpoint,
        )
    }

    fn set_body_tether(
        &mut self,
        request: NativeDynamicsBodyTetherRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let first =
            self.tether_endpoint(request.world.value, request.first, request.first_anchor)?;
        let second =
            self.tether_endpoint(request.world.value, request.second, request.second_anchor)?;
        self.set_tether(request.world.value, request.config, first, second)
    }

    fn remove_tether(
        &mut self,
        request: NativeDynamicsTetherRequest,
    ) -> Result<NativeDynamicsTetherReleaseReceipt, CsharpEngineServicesError> {
        let world = self.active_world_mut(request.world.value)?;
        world.invalidated_tethers.remove(&request.id);
        Ok(NativeDynamicsTetherReleaseReceipt {
            released: world.solver.remove_tether(request.id),
        })
    }

    fn read_tether(
        &self,
        request: NativeDynamicsTetherRequest,
    ) -> Result<NativeDynamicsTetherReadout, CsharpEngineServicesError> {
        let world = self.active_world(request.world.value)?;
        let Some(definition) = world.solver.tether(request.id) else {
            return Ok(NativeDynamicsTetherReadout {
                invalidated: world.invalidated_tethers.contains(&request.id),
                ..Default::default()
            });
        };
        let mut result = NativeDynamicsTetherReadout {
            present: true,
            maximum_length: definition.maximum_length as f32,
            target_length: definition.target_length as f32,
            ..Default::default()
        };
        if let Some(readout) = world
            .solver
            .tether_readouts()
            .iter()
            .find(|readout| readout.id == request.id)
        {
            result.simulated = true;
            result.first = native_vec3(vec3_f32(readout.first));
            result.second = native_vec3(vec3_f32(readout.second));
            result.distance = readout.distance as f32;
            result.slack_distance = (result.maximum_length - result.distance).max(0.0);
            result.taut = readout.taut;
            result.caught = readout.caught;
            result.force_proxy = readout.force_proxy as f32;
        }
        Ok(result)
    }

    pub(crate) fn new(collision_source: SpatialCollisionSource) -> Self {
        Self {
            worlds: BTreeMap::new(),
            bodies: BTreeMap::new(),
            collision_source,
            next_world: 1,
            next_body: 1,
            body_facts: Vec::new(),
            contacts: Vec::new(),
            operation_diagnostics: Default::default(),
            ragdolls: BTreeMap::new(),
            next_ragdoll: 1,
            next_internal_joint: u64::MAX,
            ragdoll_bones: Box::new([]),
            appearance: None,
        }
    }

    fn allocate(counter: &mut u64, kind: &'static str) -> Result<u64, CsharpEngineServicesError> {
        let value = *counter;
        if value == 0 {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_DYNAMICS_HANDLE",
                format!("{kind} handles exhausted"),
            ));
        }
        *counter = counter.checked_add(1).unwrap_or(0);
        Ok(value)
    }

    fn create_world(
        &mut self,
        config: NativeDynamicsWorldConfig,
    ) -> Result<NativeDynamicsWorldHandle, CsharpEngineServicesError> {
        let gravity = native_vec3_value(config.gravity);
        if !finite_vec3(gravity) {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_DYNAMICS_WORLD",
                "gravity was not finite",
            ));
        }
        let value = Self::allocate(&mut self.next_world, "world")?;
        self.worlds.insert(
            value,
            WorldSlot::Active(DynamicsWorld {
                solver: DynamicsSolver::new(vec3_f64(gravity)),
                bound_scene: None,
                bodies: BTreeMap::new(),
                contacts: BTreeMap::new(),
                invalidated_tethers: BTreeSet::new(),
                chains: BTreeMap::new(),
                invalidated_chains: BTreeSet::new(),
            }),
        );
        Ok(NativeDynamicsWorldHandle { value })
    }

    fn destroy_world(
        &mut self,
        handle: NativeDynamicsWorldHandle,
    ) -> Result<(), CsharpEngineServicesError> {
        let bodies = match self.worlds.get(&handle.value) {
            Some(WorldSlot::Active(world)) => world.bodies.keys().copied().collect::<Vec<_>>(),
            Some(WorldSlot::Tombstoned) => return Ok(()),
            None => return Err(unknown("world", handle.value)),
        };
        let ragdolls: Vec<u64> = self
            .ragdolls
            .iter()
            .filter_map(|(ragdoll, slot)| match slot {
                ragdoll::RagdollSlot::Active(active) if active.world == handle.value => {
                    Some(*ragdoll)
                }
                _ => None,
            })
            .collect();
        for ragdoll in ragdolls {
            self.release_ragdoll(ragdoll)?;
        }
        self.worlds.insert(handle.value, WorldSlot::Tombstoned);
        for body in bodies {
            self.bodies.insert(body, BodySlot::Tombstoned);
        }
        Ok(())
    }

    /// Bind the Spatial session's current collision scene as the world's
    /// static environment. Only the chunks and mesh instances that changed
    /// since the last bind are replaced in the solver.
    fn bind_world_collision(
        &mut self,
        request: NativeDynamicsWorldCollisionBindingRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let identity = self
            .collision_source
            .cursor_identity(request.spatial_session)?;
        let scene = self.collision_source.scene(request.spatial_session)?;
        let world = self.active_world_mut(request.world.value)?;
        world.bind_scene(identity, &scene);
        Ok(())
    }

    fn rebase_world_origin(
        &mut self,
        request: NativeDynamicsRebaseWorldOriginRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let identity = self
            .collision_source
            .cursor_identity(request.spatial_session)?;
        let scene = self.collision_source.scene(request.spatial_session)?;
        let receipt = request.receipt;
        let delta = [
            receipt.origin_before_cell_x,
            receipt.origin_before_cell_y,
            receipt.origin_before_cell_z,
        ]
        .into_iter()
        .zip([
            receipt.origin_after_cell_x,
            receipt.origin_after_cell_y,
            receipt.origin_after_cell_z,
        ])
        .map(|(before, after)| (i128::from(before) - i128::from(after)) as f64);
        let delta = <[f64; 3]>::try_from(delta.collect::<Vec<_>>()).expect("three axes");
        let world = self.active_world_mut(request.world.value)?;
        world.solver.translate(delta);
        world.bind_scene(identity, &scene);
        Ok(())
    }

    fn create_body(
        &mut self,
        request: &NativeDynamicsCreateBodyRequest,
    ) -> Result<NativeDynamicsBodyHandle, CsharpEngineServicesError> {
        self.create_body_with_config(request.world.value, cuboid_body_config(request.body)?)
    }

    fn create_sphere_body(
        &mut self,
        request: &NativeDynamicsCreateSphereBodyRequest,
    ) -> Result<NativeDynamicsBodyHandle, CsharpEngineServicesError> {
        self.create_body_with_config(request.world.value, sphere_body_config(request.body)?)
    }

    fn create_cuboid_body(
        &mut self,
        request: &NativeDynamicsCreateCuboidBodyRequest,
    ) -> Result<NativeDynamicsBodyHandle, CsharpEngineServicesError> {
        self.create_body_with_config(
            request.world.value,
            cuboid_body_properties_config(request.body)?,
        )
    }

    fn create_sphere_body_with_properties(
        &mut self,
        request: &NativeDynamicsCreateSphereBodyPropertiesRequest,
    ) -> Result<NativeDynamicsBodyHandle, CsharpEngineServicesError> {
        self.create_body_with_config(
            request.world.value,
            sphere_body_properties_config(request.body)?,
        )
    }

    fn create_capsule_body(
        &mut self,
        request: &NativeDynamicsCreateCapsuleBodyRequest,
    ) -> Result<NativeDynamicsBodyHandle, CsharpEngineServicesError> {
        self.create_body_with_config(request.world.value, capsule_body_config(request.body)?)
    }

    fn create_body_with_config(
        &mut self,
        world_handle: u64,
        config: BodyConfig,
    ) -> Result<NativeDynamicsBodyHandle, CsharpEngineServicesError> {
        self.active_world(world_handle)?;
        let handle = Self::allocate(&mut self.next_body, "body")?;
        let world = self.active_world_mut(world_handle)?;
        world
            .solver
            .insert_body(body_input(handle, config.transform, &config.body))
            .map_err(solver_error("CSHARP_DYNAMICS_BODY"))?;
        world.bodies.insert(handle, config.body);
        self.bodies.insert(
            handle,
            BodySlot::Active {
                world: world_handle,
            },
        );
        Ok(NativeDynamicsBodyHandle { value: handle })
    }

    fn destroy_body(
        &mut self,
        handle: NativeDynamicsBodyHandle,
    ) -> Result<(), CsharpEngineServicesError> {
        let world = match self.bodies.get(&handle.value) {
            Some(BodySlot::Active { world }) => *world,
            Some(BodySlot::Tombstoned) => return Ok(()),
            None => return Err(unknown("body", handle.value)),
        };
        self.remove_body(world, handle.value)
    }

    /// Remove a body, the ropes attached to it and any chain anchored on it.
    /// Product-authored ropes and chains it held report as invalidated.
    fn remove_body(
        &mut self,
        world_handle: u64,
        handle: u64,
    ) -> Result<(), CsharpEngineServicesError> {
        let world = self.active_world_mut(world_handle)?;
        let anchored = world
            .chains
            .iter()
            .filter(|(_, chain)| chain.anchor.body() == Some(DynamicsBodyId(handle)))
            .map(|(id, _)| *id)
            .collect::<Vec<_>>();
        let mut released = Vec::new();
        for id in anchored {
            let chain = world.chains.remove(&id).expect("listed chain");
            released.extend(world.remove_chain_bodies(&chain));
            world.invalidated_chains.insert(id);
        }
        let ropes = world.solver.remove_body(DynamicsBodyId(handle));
        world.invalidated_tethers.extend(ropes);
        world.bodies.remove(&handle);
        world.contacts.remove(&handle);
        released.push(handle);
        for handle in released {
            self.bodies.insert(handle, BodySlot::Tombstoned);
        }
        Ok(())
    }

    fn step(
        &mut self,
        request: &NativeDynamicsStepRequest,
    ) -> Result<NativeDynamicsStepReceipt, CsharpEngineServicesError> {
        let actions =
            unsafe { borrowed_slice(request.actions, request.actions_len, "dynamics actions") }?;
        let actions = self.step_actions(request.world.value, actions)?;
        self.execute_step(
            request.world.value,
            request.step_seconds,
            request.steps,
            &actions,
        )
    }

    /// Performs one step and copies the listed bodies' facts, in the caller's
    /// order.
    fn step_and_read(
        &mut self,
        request: &NativeDynamicsStepAndReadRequest,
    ) -> Result<NativeDynamicsStepAndReadResult, CsharpEngineServicesError> {
        let actions = unsafe {
            borrowed_slice(
                request.actions,
                request.actions_len,
                "dynamics step/read actions",
            )
        }?;
        let bodies = unsafe {
            borrowed_slice(
                request.bodies,
                request.bodies_len,
                "dynamics step/read bodies",
            )
        }?;
        let actions = self.step_actions(request.world.value, actions)?;
        for body in bodies {
            self.world_body(request.world.value, body.value, "CSHARP_DYNAMICS_BODY")?;
        }
        let receipt = self.execute_step(
            request.world.value,
            request.step_seconds,
            request.steps,
            &actions,
        )?;
        let world = match self.worlds.get(&request.world.value) {
            Some(WorldSlot::Active(world)) => world,
            _ => return Err(unknown("world", request.world.value)),
        };
        self.body_facts.clear();
        self.body_facts
            .extend(bodies.iter().map(|body| NativeDynamicsBodyFact {
                body: NativeDynamicsBodyReference { value: body.value },
                readout: world.readout(body.value),
            }));
        Ok(NativeDynamicsStepAndReadResult {
            bodies: self.body_facts.as_ptr(),
            bodies_len: self.body_facts.len(),
            generation: receipt.generation,
            body_count: receipt.body_count,
            contact_count: receipt.contact_count,
        })
    }

    fn step_actions(
        &self,
        world: u64,
        actions: &[NativeDynamicsAction],
    ) -> Result<Vec<DynamicsAction>, CsharpEngineServicesError> {
        actions
            .iter()
            .map(|action| {
                self.world_body(world, action.body.value, "CSHARP_DYNAMICS_BODY")?;
                Ok(DynamicsAction {
                    body: DynamicsBodyId(action.body.value),
                    force: vec3_f64(native_vec3_value(action.force)),
                    torque: vec3_f64(native_vec3_value(action.torque)),
                    impulse: vec3_f64(native_vec3_value(action.impulse)),
                    torque_impulse: vec3_f64(native_vec3_value(action.torque_impulse)),
                    wake: action.wake,
                })
            })
            .collect()
    }

    fn execute_step(
        &mut self,
        world_handle: u64,
        step_seconds: f32,
        steps: u32,
        actions: &[DynamicsAction],
    ) -> Result<NativeDynamicsStepReceipt, CsharpEngineServicesError> {
        let world = self.active_world_mut(world_handle)?;
        let receipt = world
            .solver
            .step(f64::from(step_seconds), steps, actions)
            .map_err(solver_error("CSHARP_DYNAMICS_STEP"))?;
        world.contacts = contacts_by_body(world.solver.contacts());
        self.place_ragdolls_of(world_handle)?;
        let links = receipt.rope_link_count as u32;
        let substeps = receipt.rope_substeps as u32;
        let iterations = receipt.rope_iterations as u32;
        Ok(NativeDynamicsStepReceipt {
            rope_substeps: substeps,
            rope_iterations: iterations,
            rope_link_count: links,
            rope_solver_link_steps: steps * substeps * iterations * links,
            generation: receipt.generation,
            body_count: receipt.body_count as u32,
            contact_count: receipt.contact_count as u32,
        })
    }

    fn read(
        &mut self,
        request: NativeDynamicsReadRequest,
    ) -> Result<NativeDynamicsReadout, CsharpEngineServicesError> {
        let world = self.active_body(request.body.value)?;
        Ok(self.active_world(world)?.readout(request.body.value))
    }

    fn reset(
        &mut self,
        request: NativeDynamicsResetRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let world = self.active_body(request.body.value)?;
        let transform = checked_transform(request.transform)?;
        let world = self.active_world_mut(world)?;
        world
            .solver
            .set_body_motion(
                DynamicsBodyId(request.body.value),
                vec3_f64(transform.translation),
                quat_f64(transform.rotation),
                vec3_f64(native_vec3_value(request.linear_velocity)),
                vec3_f64(native_vec3_value(request.angular_velocity)),
                request.sleeping,
            )
            .map_err(solver_error("CSHARP_DYNAMICS_RESET"))
    }

    fn replace_body(
        &mut self,
        request: NativeDynamicsReplaceBodyRequest,
    ) -> Result<NativeDynamicsBodyHandle, CsharpEngineServicesError> {
        self.replace_body_with_config(request.body, cuboid_body_config(request.replacement)?)
    }

    fn replace_cuboid_body(
        &mut self,
        request: NativeDynamicsReplaceCuboidBodyRequest,
    ) -> Result<NativeDynamicsBodyHandle, CsharpEngineServicesError> {
        self.replace_body_with_config(
            request.body,
            cuboid_body_properties_config(request.replacement)?,
        )
    }

    fn replace_sphere_body(
        &mut self,
        request: NativeDynamicsReplaceSphereBodyRequest,
    ) -> Result<NativeDynamicsBodyHandle, CsharpEngineServicesError> {
        self.replace_body_with_config(
            request.body,
            sphere_body_properties_config(request.replacement)?,
        )
    }

    fn replace_capsule_body(
        &mut self,
        request: NativeDynamicsReplaceCapsuleBodyRequest,
    ) -> Result<NativeDynamicsBodyHandle, CsharpEngineServicesError> {
        self.replace_body_with_config(request.body, capsule_body_config(request.replacement)?)
    }

    /// Destroy a body and create its replacement under a new handle. Ropes
    /// attached to the old body are invalidated like any other destroy.
    fn replace_body_with_config(
        &mut self,
        body: NativeDynamicsBodyHandle,
        config: BodyConfig,
    ) -> Result<NativeDynamicsBodyHandle, CsharpEngineServicesError> {
        let world = self.active_body(body.value)?;
        let replacement = self.create_body_with_config(world, config)?;
        self.remove_body(world, body.value)?;
        Ok(replacement)
    }

    fn update_body(
        &mut self,
        request: NativeDynamicsUpdateBodyRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let world = self.active_body(request.body.value)?;
        let world = self.active_world_mut(world)?;
        let handle = request.body.value;
        let shape = world.bodies[&handle].shape;
        let body = body_with_properties(shape, request.properties)?;
        let pose = world
            .solver
            .body(DynamicsBodyId(handle))
            .expect("admitted body");
        let mut input = body_input(handle, EntityTransform::IDENTITY, &body);
        input.translation = pose.translation;
        input.rotation = pose.rotation;
        world
            .solver
            .replace_body(input)
            .map_err(solver_error("CSHARP_DYNAMICS_UPDATE"))?;
        world.bodies.insert(handle, body);
        world.contacts.remove(&handle);
        Ok(())
    }

    fn read_world(
        &mut self,
        request: NativeDynamicsWorldReadRequest,
    ) -> Result<NativeDynamicsWorldResult, CsharpEngineServicesError> {
        let world = match self.worlds.get(&request.world.value) {
            Some(WorldSlot::Active(world)) => world,
            Some(WorldSlot::Tombstoned) => return Err(tombstoned("world")),
            None => return Err(unknown("world", request.world.value)),
        };
        self.body_facts.clear();
        self.body_facts
            .extend(world.bodies.keys().map(|&body| NativeDynamicsBodyFact {
                body: NativeDynamicsBodyReference { value: body },
                readout: world.readout(body),
            }));
        self.contacts.clear();
        self.contacts.extend(
            world
                .solver
                .contacts()
                .iter()
                .map(|contact| NativeDynamicsContact {
                    environment: contact.second.is_none(),
                    first: NativeDynamicsBodyReference {
                        value: contact.first.0,
                    },
                    second: NativeDynamicsBodyReference {
                        value: contact.second.map_or(0, |body| body.0),
                    },
                    impulse: native_vec3(vec3_f32(contact.impulse)),
                    impulse_magnitude: contact.impulse_magnitude as f32,
                }),
        );
        Ok(NativeDynamicsWorldResult {
            bodies: self.body_facts.as_ptr(),
            bodies_len: self.body_facts.len(),
            contacts: self.contacts.as_ptr(),
            contacts_len: self.contacts.len(),
            generation: world.solver.generation(),
        })
    }

    fn active_world_mut(
        &mut self,
        handle: u64,
    ) -> Result<&mut DynamicsWorld, CsharpEngineServicesError> {
        match self.worlds.get_mut(&handle) {
            Some(WorldSlot::Active(world)) => Ok(world),
            Some(WorldSlot::Tombstoned) => Err(tombstoned("world")),
            None => Err(unknown("world", handle)),
        }
    }

    fn active_world(&self, handle: u64) -> Result<&DynamicsWorld, CsharpEngineServicesError> {
        match self.worlds.get(&handle) {
            Some(WorldSlot::Active(world)) => Ok(world),
            Some(WorldSlot::Tombstoned) => Err(tombstoned("world")),
            None => Err(unknown("world", handle)),
        }
    }

    /// The world owning a live body handle.
    fn active_body(&self, handle: u64) -> Result<u64, CsharpEngineServicesError> {
        match self.bodies.get(&handle) {
            Some(BodySlot::Active { world }) => Ok(*world),
            Some(BodySlot::Tombstoned) => Err(tombstoned("body")),
            None => Err(unknown("body", handle)),
        }
    }

    /// Require a live body in `world`.
    fn world_body(
        &self,
        world: u64,
        handle: u64,
        code: &'static str,
    ) -> Result<(), CsharpEngineServicesError> {
        if self.active_body(handle)? != world {
            return Err(CsharpEngineServicesError::new(
                code,
                "body belonged to another world",
            ));
        }
        Ok(())
    }
}

impl DynamicsWorld {
    fn bind_scene(&mut self, identity: u64, scene: &VoxelCollisionScene) {
        if self.bound_scene == Some(identity) {
            return;
        }
        scene.bind_dynamics_environment(&mut self.solver);
        self.bound_scene = Some(identity);
    }

    fn body_output(&self, handle: u64) -> DynamicsBodyOutput {
        self.solver
            .body(DynamicsBodyId(handle))
            .expect("a live body handle has a solver body")
    }

    fn readout(&self, handle: u64) -> NativeDynamicsReadout {
        let output = self.body_output(handle);
        let body = &self.bodies[&handle];
        let properties = rigid_body_component_mass_properties(*body);
        let policy = match body.inertia {
            RigidBodyInertiaPolicy::DeriveFromShapeAndMass => {
                NativeDynamicsMassPolicyKind::DeriveFromShapeAndMass
            }
            RigidBodyInertiaPolicy::Explicit { .. } => NativeDynamicsMassPolicyKind::Explicit,
        };
        let contact = self.contacts.get(&handle).copied().unwrap_or_default();
        NativeDynamicsReadout {
            transform: native_transform(output_transform(&output)),
            linear_velocity: native_vec3(vec3_f32(output.linear_velocity)),
            angular_velocity: native_vec3(vec3_f32(output.angular_velocity)),
            sleeping: output.sleeping,
            mass_properties: NativeMassProperties {
                available: properties.is_some(),
                mass: body.mass,
                principal_inertia: properties.map_or(NativeVec3::default(), |value| {
                    native_vec3(value.principal_inertia)
                }),
                policy,
                center_of_mass: properties.map_or(NativeVec3::default(), |value| {
                    native_vec3(value.center_of_mass)
                }),
                principal_inertia_local_frame: properties.map_or(NativeQuat::default(), |value| {
                    native_quat(value.principal_inertia_local_frame)
                }),
            },
            contact_count: contact.count,
            first_contact: contact.latest,
        }
    }
}

fn output_transform(output: &DynamicsBodyOutput) -> EntityTransform {
    EntityTransform {
        translation: vec3_f32(output.translation),
        rotation: Quat::new(
            output.rotation[0] as f32,
            output.rotation[1] as f32,
            output.rotation[2] as f32,
            output.rotation[3] as f32,
        ),
        scale: Vec3::ONE,
    }
}

fn body_input(id: u64, transform: EntityTransform, body: &RigidBodyComponent) -> DynamicsBodyInput {
    DynamicsBodyInput {
        id: DynamicsBodyId(id),
        translation: vec3_f64(transform.translation),
        rotation: quat_f64(transform.rotation),
        shape: match body.shape {
            RigidBodyShape::Sphere { radius } => DynamicsShape::Sphere {
                radius: f64::from(radius),
            },
            RigidBodyShape::Cuboid { half_extents } => DynamicsShape::Cuboid {
                half_extents: vec3_f64(half_extents),
            },
            RigidBodyShape::CapsuleY {
                half_height,
                radius,
            } => DynamicsShape::CapsuleY {
                half_height: f64::from(half_height),
                radius: f64::from(radius),
            },
        },
        mass: f64::from(body.mass),
        mass_properties: match body.inertia {
            RigidBodyInertiaPolicy::DeriveFromShapeAndMass => None,
            RigidBodyInertiaPolicy::Explicit {
                center_of_mass,
                principal_inertia,
                principal_inertia_local_frame,
            } => Some(DynamicsMassProperties {
                center_of_mass: vec3_f64(center_of_mass),
                principal_inertia: vec3_f64(principal_inertia),
                principal_inertia_local_frame: quat_f64(principal_inertia_local_frame),
            }),
        },
        linear_velocity: vec3_f64(body.linear_velocity),
        angular_velocity: vec3_f64(body.angular_velocity),
        locked_translation_axes: body.locked_translation_axes,
        locked_rotation_axes: body.locked_rotation_axes,
        linear_damping: f64::from(body.linear_damping),
        angular_damping: f64::from(body.angular_damping),
        gravity_scale: f64::from(body.gravity_scale),
        friction: f64::from(body.friction),
        restitution: f64::from(body.restitution),
        collision_groups: body.collision_groups,
        collision_mask: body.collision_mask,
        enabled: body.enabled,
        sleeping: body.sleeping,
        continuous_collision: body.continuous_collision,
    }
}

fn vec3_f64(value: Vec3) -> [f64; 3] {
    [f64::from(value.x), f64::from(value.y), f64::from(value.z)]
}

fn vec3_f32(value: [f64; 3]) -> Vec3 {
    Vec3::new(value[0] as f32, value[1] as f32, value[2] as f32)
}

fn quat_f64(value: Quat) -> [f64; 4] {
    [
        f64::from(value.x),
        f64::from(value.y),
        f64::from(value.z),
        f64::from(value.w),
    ]
}

fn unknown(kind: &str, value: u64) -> CsharpEngineServicesError {
    CsharpEngineServicesError::new(
        "CSHARP_DYNAMICS_HANDLE",
        format!("unknown {kind} handle {value}"),
    )
}

fn tombstoned(kind: &str) -> CsharpEngineServicesError {
    CsharpEngineServicesError::new(
        "CSHARP_DYNAMICS_HANDLE",
        format!("{kind} handle was destroyed"),
    )
}

struct BodyConfig {
    transform: EntityTransform,
    body: RigidBodyComponent,
}

fn cuboid_body_config(
    value: NativeDynamicsBodyConfig,
) -> Result<BodyConfig, CsharpEngineServicesError> {
    let transform = checked_transform(value.transform)?;
    let shape = RigidBodyShape::Cuboid {
        half_extents: native_vec3_value(value.half_extents),
    };
    body_config_with_properties(transform, shape, value.properties)
}

fn sphere_body_config(
    value: NativeDynamicsSphereBodyConfig,
) -> Result<BodyConfig, CsharpEngineServicesError> {
    let transform = checked_transform(value.transform)?;
    body_config(
        transform,
        RigidBodyShape::Sphere {
            radius: value.radius,
        },
        value.mass,
        value.mass_policy,
        value.axis_locks,
        value.gravity_scale,
    )
}

fn cuboid_body_properties_config(
    value: NativeDynamicsCuboidBodyConfig,
) -> Result<BodyConfig, CsharpEngineServicesError> {
    let transform = checked_transform(value.transform)?;
    body_config_with_properties(
        transform,
        RigidBodyShape::Cuboid {
            half_extents: native_vec3_value(value.half_extents),
        },
        value.properties,
    )
}

fn sphere_body_properties_config(
    value: NativeDynamicsSphereBodyPropertiesConfig,
) -> Result<BodyConfig, CsharpEngineServicesError> {
    let transform = checked_transform(value.transform)?;
    body_config_with_properties(
        transform,
        RigidBodyShape::Sphere {
            radius: value.radius,
        },
        value.properties,
    )
}

fn capsule_body_config(
    value: NativeDynamicsCapsuleBodyConfig,
) -> Result<BodyConfig, CsharpEngineServicesError> {
    let transform = checked_transform(value.transform)?;
    body_config_with_properties(
        transform,
        RigidBodyShape::CapsuleY {
            half_height: value.half_height,
            radius: value.radius,
        },
        value.properties,
    )
}

fn rigid_body_inertia_policy(
    value: NativeDynamicsMassPolicy,
) -> Result<RigidBodyInertiaPolicy, CsharpEngineServicesError> {
    Ok(match value.kind {
        NativeDynamicsMassPolicyKind::DeriveFromShapeAndMass => {
            RigidBodyInertiaPolicy::DeriveFromShapeAndMass
        }
        NativeDynamicsMassPolicyKind::Explicit => RigidBodyInertiaPolicy::Explicit {
            center_of_mass: native_vec3_value(value.explicit.center_of_mass),
            principal_inertia: native_vec3_value(value.explicit.principal_inertia),
            principal_inertia_local_frame: native_quat_value(
                value.explicit.principal_inertia_local_frame,
            ),
        },
    })
}

fn body_config(
    transform: EntityTransform,
    shape: RigidBodyShape,
    mass: f32,
    mass_policy: NativeDynamicsMassPolicy,
    axis_locks: NativeAxisLocks,
    gravity_scale: f32,
) -> Result<BodyConfig, CsharpEngineServicesError> {
    let mut body = RigidBodyComponent::dynamic(shape, mass);
    body.inertia = rigid_body_inertia_policy(mass_policy)?;
    body.locked_translation_axes = [
        axis_locks.translation_x,
        axis_locks.translation_y,
        axis_locks.translation_z,
    ];
    body.locked_rotation_axes = [
        axis_locks.rotation_x,
        axis_locks.rotation_y,
        axis_locks.rotation_z,
    ];
    body.gravity_scale = gravity_scale;
    Ok(BodyConfig { transform, body })
}

fn body_config_with_properties(
    transform: EntityTransform,
    shape: RigidBodyShape,
    properties: NativeDynamicsBodyProperties,
) -> Result<BodyConfig, CsharpEngineServicesError> {
    Ok(BodyConfig {
        transform,
        body: body_with_properties(shape, properties)?,
    })
}

fn body_with_properties(
    shape: RigidBodyShape,
    properties: NativeDynamicsBodyProperties,
) -> Result<RigidBodyComponent, CsharpEngineServicesError> {
    let mut body = RigidBodyComponent::dynamic(shape, properties.mass);
    body.inertia = rigid_body_inertia_policy(properties.mass_policy)?;
    body.linear_velocity = native_vec3_value(properties.linear_velocity);
    body.angular_velocity = native_vec3_value(properties.angular_velocity);
    body.locked_translation_axes = [
        properties.axis_locks.translation_x,
        properties.axis_locks.translation_y,
        properties.axis_locks.translation_z,
    ];
    body.locked_rotation_axes = [
        properties.axis_locks.rotation_x,
        properties.axis_locks.rotation_y,
        properties.axis_locks.rotation_z,
    ];
    body.linear_damping = properties.linear_damping;
    body.angular_damping = properties.angular_damping;
    body.gravity_scale = properties.gravity_scale;
    body.friction = properties.friction;
    body.restitution = properties.restitution;
    body.collision_groups = properties.collision_groups;
    body.collision_mask = properties.collision_mask;
    body.enabled = properties.enabled;
    body.sleeping = properties.sleeping;
    body.continuous_collision = properties.continuous_collision;
    Ok(body)
}

fn contacts_by_body(
    contacts: &[engine_spatial::DynamicsContact],
) -> BTreeMap<u64, BodyContactSummary> {
    let mut summary = BTreeMap::new();
    for contact in contacts {
        let impulse = vec3_f32(contact.impulse);
        let magnitude = contact.impulse_magnitude as f32;
        record_contact(
            &mut summary,
            contact.first.0,
            contact.second.is_none(),
            impulse,
            magnitude,
        );
        if let Some(second) = contact.second {
            record_contact(
                &mut summary,
                second.0,
                false,
                Vec3::new(-impulse.x, -impulse.y, -impulse.z),
                magnitude,
            );
        }
    }
    summary
}

fn record_contact(
    contacts: &mut BTreeMap<u64, BodyContactSummary>,
    body: u64,
    environment: bool,
    impulse: Vec3,
    impulse_magnitude: f32,
) {
    let entry = contacts.entry(body).or_default();
    if entry.count == 0 {
        entry.latest = NativeDynamicsContactFact {
            present: true,
            environment,
            impulse: native_vec3(impulse),
            impulse_magnitude,
        };
    }
    entry.count = entry.count.saturating_add(1);
}

fn checked_transform(value: NativeTransform) -> Result<EntityTransform, CsharpEngineServicesError> {
    let transform = EntityTransform {
        translation: native_vec3_value(value.translation),
        rotation: native_quat_value(value.rotation),
        scale: native_vec3_value(value.scale),
    };
    if !finite_vec3(transform.translation)
        || !finite_vec3(transform.scale)
        || !finite_quat(transform.rotation)
        || (transform.rotation.norm_squared() - 1.0).abs() > 0.001
        || transform.scale != Vec3::ONE
    {
        return Err(CsharpEngineServicesError::new(
            "CSHARP_DYNAMICS_TRANSFORM",
            "transform must be finite with unit scale",
        ));
    }
    Ok(transform)
}

fn finite_vec3(value: Vec3) -> bool {
    value.x.is_finite() && value.y.is_finite() && value.z.is_finite()
}
fn finite_quat(value: Quat) -> bool {
    value.x.is_finite() && value.y.is_finite() && value.z.is_finite() && value.w.is_finite()
}

fn native_transform(value: EntityTransform) -> NativeTransform {
    NativeTransform {
        translation: native_vec3(value.translation),
        rotation: native_quat(value.rotation),
        scale: native_vec3(value.scale),
    }
}

unsafe extern "C" fn create_world(
    context: *mut c_void,
    config: NativeDynamicsWorldConfig,
    handle: *mut NativeDynamicsWorldHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    clear_receipt(operation_error);
    if context.is_null() || handle.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() };
    match bridge.create_world(config) {
        Ok(value) => {
            unsafe { *handle = value };
            ABI_OK
        }
        Err(error) => refuse(context, &error, operation_error),
    }
}

unsafe extern "C" fn destroy_world(
    context: *mut c_void,
    handle: NativeDynamicsWorldHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    clear_receipt(operation_error);
    if context.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }.destroy_world(handle) {
        Ok(()) => ABI_OK,
        Err(error) => refuse(context, &error, operation_error),
    }
}

unsafe extern "C" fn create_body(
    context: *mut c_void,
    request: *const NativeDynamicsCreateBodyRequest,
    handle: *mut NativeDynamicsBodyHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    clear_receipt(operation_error);
    if context.is_null() || request.is_null() || handle.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }.create_body(unsafe { &*request })
    {
        Ok(value) => {
            unsafe { *handle = value };
            ABI_OK
        }
        Err(error) => refuse(context, &error, operation_error),
    }
}

unsafe extern "C" fn create_sphere_body(
    context: *mut c_void,
    request: *const NativeDynamicsCreateSphereBodyRequest,
    handle: *mut NativeDynamicsBodyHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    clear_receipt(operation_error);
    if context.is_null() || request.is_null() || handle.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }
        .create_sphere_body(unsafe { &*request })
    {
        Ok(value) => {
            unsafe { *handle = value };
            ABI_OK
        }
        Err(error) => refuse(context, &error, operation_error),
    }
}

unsafe extern "C" fn create_cuboid_body(
    context: *mut c_void,
    request: *const NativeDynamicsCreateCuboidBodyRequest,
    handle: *mut NativeDynamicsBodyHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    clear_receipt(operation_error);
    if context.is_null() || request.is_null() || handle.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }
        .create_cuboid_body(unsafe { &*request })
    {
        Ok(value) => {
            unsafe { *handle = value };
            ABI_OK
        }
        Err(error) => refuse(context, &error, operation_error),
    }
}

unsafe extern "C" fn create_sphere_body_with_properties(
    context: *mut c_void,
    request: *const NativeDynamicsCreateSphereBodyPropertiesRequest,
    handle: *mut NativeDynamicsBodyHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    clear_receipt(operation_error);
    if context.is_null() || request.is_null() || handle.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }
        .create_sphere_body_with_properties(unsafe { &*request })
    {
        Ok(value) => {
            unsafe { *handle = value };
            ABI_OK
        }
        Err(error) => refuse(context, &error, operation_error),
    }
}

unsafe extern "C" fn create_capsule_body(
    context: *mut c_void,
    request: *const NativeDynamicsCreateCapsuleBodyRequest,
    handle: *mut NativeDynamicsBodyHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    clear_receipt(operation_error);
    if context.is_null() || request.is_null() || handle.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }
        .create_capsule_body(unsafe { &*request })
    {
        Ok(value) => {
            unsafe { *handle = value };
            ABI_OK
        }
        Err(error) => refuse(context, &error, operation_error),
    }
}

unsafe extern "C" fn bind_world_collision(
    context: *mut c_void,
    request: NativeDynamicsWorldCollisionBindingRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    clear_receipt(operation_error);
    if context.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }.bind_world_collision(request) {
        Ok(()) => ABI_OK,
        Err(error) => refuse(context, &error, operation_error),
    }
}

unsafe extern "C" fn rebase_world_origin(
    context: *mut c_void,
    request: NativeDynamicsRebaseWorldOriginRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    clear_receipt(operation_error);
    if context.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }.rebase_world_origin(request) {
        Ok(()) => ABI_OK,
        Err(error) => refuse(context, &error, operation_error),
    }
}

unsafe extern "C" fn destroy_body(
    context: *mut c_void,
    handle: NativeDynamicsBodyHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    clear_receipt(operation_error);
    if context.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }.destroy_body(handle) {
        Ok(()) => ABI_OK,
        Err(error) => refuse(context, &error, operation_error),
    }
}

unsafe extern "C" fn step(
    context: *mut c_void,
    request: *const NativeDynamicsStepRequest,
    receipt: *mut NativeDynamicsStepReceipt,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    clear_receipt(operation_error);
    if context.is_null() || request.is_null() || receipt.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }.step(unsafe { &*request }) {
        Ok(value) => {
            unsafe { *receipt = value };
            ABI_OK
        }
        Err(error) => refuse(context, &error, operation_error),
    }
}

unsafe extern "C" fn step_and_read(
    context: *mut c_void,
    request: *const NativeDynamicsStepAndReadRequest,
    result: *mut NativeDynamicsStepAndReadResult,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    clear_receipt(operation_error);
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }
        .step_and_read(unsafe { &*request })
    {
        Ok(value) => {
            unsafe { *result = value };
            ABI_OK
        }
        Err(error) => refuse(context, &error, operation_error),
    }
}

unsafe extern "C" fn read(
    context: *mut c_void,
    request: NativeDynamicsReadRequest,
    readout: *mut NativeDynamicsReadout,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    clear_receipt(operation_error);
    if context.is_null() || readout.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }.read(request) {
        Ok(value) => {
            unsafe { *readout = value };
            ABI_OK
        }
        Err(error) => refuse(context, &error, operation_error),
    }
}

unsafe extern "C" fn reset(
    context: *mut c_void,
    request: NativeDynamicsResetRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    clear_receipt(operation_error);
    if context.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }.reset(request) {
        Ok(()) => ABI_OK,
        Err(error) => refuse(context, &error, operation_error),
    }
}

unsafe extern "C" fn update_body(
    context: *mut c_void,
    request: NativeDynamicsUpdateBodyRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    clear_receipt(operation_error);
    if context.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }.update_body(request) {
        Ok(()) => ABI_OK,
        Err(error) => refuse(context, &error, operation_error),
    }
}

unsafe extern "C" fn read_world(
    context: *mut c_void,
    request: NativeDynamicsWorldReadRequest,
    readout: *mut NativeDynamicsWorldResult,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    clear_receipt(operation_error);
    if context.is_null() || readout.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }.read_world(request) {
        Ok(value) => {
            unsafe { *readout = value };
            ABI_OK
        }
        Err(error) => refuse(context, &error, operation_error),
    }
}

unsafe extern "C" fn replace_body(
    context: *mut c_void,
    request: NativeDynamicsReplaceBodyRequest,
    handle: *mut NativeDynamicsBodyHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    clear_receipt(operation_error);
    if context.is_null() || handle.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }.replace_body(request) {
        Ok(value) => {
            unsafe { *handle = value };
            ABI_OK
        }
        Err(error) => refuse(context, &error, operation_error),
    }
}

unsafe extern "C" fn replace_cuboid_body(
    context: *mut c_void,
    request: NativeDynamicsReplaceCuboidBodyRequest,
    handle: *mut NativeDynamicsBodyHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    clear_receipt(operation_error);
    if context.is_null() || handle.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }.replace_cuboid_body(request) {
        Ok(value) => {
            unsafe { *handle = value };
            ABI_OK
        }
        Err(error) => refuse(context, &error, operation_error),
    }
}

unsafe extern "C" fn replace_sphere_body(
    context: *mut c_void,
    request: NativeDynamicsReplaceSphereBodyRequest,
    handle: *mut NativeDynamicsBodyHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    clear_receipt(operation_error);
    if context.is_null() || handle.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }.replace_sphere_body(request) {
        Ok(value) => {
            unsafe { *handle = value };
            ABI_OK
        }
        Err(error) => refuse(context, &error, operation_error),
    }
}

unsafe extern "C" fn replace_capsule_body(
    context: *mut c_void,
    request: NativeDynamicsReplaceCapsuleBodyRequest,
    handle: *mut NativeDynamicsBodyHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    clear_receipt(operation_error);
    if context.is_null() || handle.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }.replace_capsule_body(request) {
        Ok(value) => {
            unsafe { *handle = value };
            ABI_OK
        }
        Err(error) => refuse(context, &error, operation_error),
    }
}

pub(crate) fn api(bridge: &mut RuntimeDynamicsBridge) -> NativeDynamicsApi {
    NativeDynamicsApi {
        context: (bridge as *mut RuntimeDynamicsBridge).cast(),
        observe_anchor: anchor::observe_anchor,
        step_with_reactions: anchor::step_with_reactions,
        configure_ropes: chain::configure_ropes,
        set_chain_length: chain::set_chain_length,
        create_fixed_chain: chain::create_fixed_chain,
        create_body_chain: chain::create_body_chain,
        read_chain: chain::read_chain,
        read_chain_point: chain::read_chain_point,
        remove_chain: chain::remove_chain,
        set_fixed_tether,
        set_body_tether,
        remove_tether,
        read_tether,
        set_joint: ragdoll::set_joint,
        remove_joint: ragdoll::remove_joint,
        create_ragdoll: ragdoll::create_ragdoll,
        destroy_ragdoll: ragdoll::destroy_ragdoll,
        set_ragdoll_blend: ragdoll::set_ragdoll_blend,
        apply_ragdoll_impulse: ragdoll::apply_ragdoll_impulse,
        read_ragdoll: ragdoll::read_ragdoll,
        create_world,
        destroy_world,
        create_body,
        create_sphere_body,
        create_cuboid_body,
        create_sphere_body_with_properties,
        create_capsule_body,
        bind_world_collision,
        rebase_world_origin,
        destroy_body,
        step,
        step_and_read,
        read,
        reset,
        update_body,
        read_world,
        replace_body,
        replace_cuboid_body,
        replace_sphere_body,
        replace_capsule_body,
    }
}

unsafe extern "C" fn set_fixed_tether(
    context: *mut c_void,
    request: NativeDynamicsFixedTetherRequest,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    if receipt.is_null() {
        return 0;
    }
    unsafe { *receipt = std::mem::zeroed() };
    if context.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }.set_fixed_tether(request) {
        Ok(()) => ABI_OK,
        Err(error) => {
            unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }
                .operation_diagnostics
                .retain(&error, receipt);
            0
        }
    }
}
unsafe extern "C" fn set_body_tether(
    context: *mut c_void,
    request: NativeDynamicsBodyTetherRequest,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    if receipt.is_null() {
        return 0;
    }
    unsafe { *receipt = std::mem::zeroed() };
    if context.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }.set_body_tether(request) {
        Ok(()) => ABI_OK,
        Err(error) => {
            unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }
                .operation_diagnostics
                .retain(&error, receipt);
            0
        }
    }
}
unsafe extern "C" fn remove_tether(
    context: *mut c_void,
    request: NativeDynamicsTetherRequest,
    result: *mut NativeDynamicsTetherReleaseReceipt,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    if receipt.is_null() {
        return 0;
    }
    unsafe { *receipt = std::mem::zeroed() };
    if context.is_null() || result.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }.remove_tether(request) {
        Ok(value) => {
            unsafe { *result = value };
            ABI_OK
        }
        Err(error) => {
            unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }
                .operation_diagnostics
                .retain(&error, receipt);
            0
        }
    }
}
unsafe extern "C" fn read_tether(
    context: *mut c_void,
    request: NativeDynamicsTetherRequest,
    output: *mut NativeDynamicsTetherReadout,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    if receipt.is_null() {
        return 0;
    }
    unsafe { *receipt = std::mem::zeroed() };
    if context.is_null() || output.is_null() {
        return 0;
    }
    match unsafe { &*context.cast::<RuntimeDynamicsBridge>() }.read_tether(request) {
        Ok(value) => {
            unsafe {
                *output = value;
            }
            ABI_OK
        }
        Err(error) => {
            unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }
                .operation_diagnostics
                .retain(&error, receipt);
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use core_ids::EntityId;
    use entity_state::{EntityDefinition, EntityState};

    use super::*;

    const ONE_SIXTIETH_SECOND: f32 = 1.0 / 60.0;

    fn transform(translation: NativeVec3) -> NativeTransform {
        NativeTransform {
            translation,
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

    fn body_config(translation: NativeVec3) -> NativeDynamicsBodyConfig {
        NativeDynamicsBodyConfig {
            transform: transform(translation),
            half_extents: NativeVec3 {
                x: 0.5,
                y: 0.5,
                z: 0.5,
            },
            properties: NativeDynamicsBodyProperties {
                mass: 2.0,
                mass_policy: NativeDynamicsMassPolicy::default(),
                linear_velocity: NativeVec3::default(),
                angular_velocity: NativeVec3::default(),
                axis_locks: NativeAxisLocks::default(),
                linear_damping: 0.0,
                angular_damping: 0.0,
                gravity_scale: 0.0,
                friction: 0.5,
                restitution: 0.0,
                collision_groups: u32::MAX,
                collision_mask: u32::MAX,
                enabled: true,
                sleeping: false,
                continuous_collision: false,
            },
        }
    }

    fn chain_config(id: u64, bead_count: u32) -> NativeDynamicsChainConfig {
        let mut properties = body_config(NativeVec3::default()).properties;
        properties.mass = 1.0;
        properties.gravity_scale = 1.0;
        properties.continuous_collision = true;
        NativeDynamicsChainConfig {
            id,
            bead_count,
            link_length: 0.5,
            radius: 0.15,
            properties,
        }
    }

    #[test]
    fn chain_creation_removal_and_anchor_invalidation() {
        let spatial = crate::spatial::RuntimeSpatialBridge::new();
        let mut bridge = RuntimeDynamicsBridge::new(spatial.collision_source());
        let world = bridge
            .create_world(NativeDynamicsWorldConfig {
                gravity: NativeVec3::default(),
            })
            .unwrap();
        let body = bridge
            .create_body(&NativeDynamicsCreateBodyRequest {
                world,
                body: body_config(NativeVec3::default()),
            })
            .unwrap();
        let request = NativeDynamicsBodyChainRequest {
            world,
            body,
            local_anchor: NativeVec3::default(),
            end: NativeVec3 {
                x: 2.0,
                y: 0.0,
                z: 0.0,
            },
            config: chain_config(8, 4),
        };
        let mut invalid = request;
        invalid.config.bead_count = 0;
        assert!(bridge.create_body_chain(invalid).is_err());
        invalid = request;
        invalid.config.properties.mass = -1.0;
        assert!(bridge.create_body_chain(invalid).is_err());
        // A refused chain leaves no beads behind.
        assert_eq!(bridge.active_world(world.value).unwrap().bodies.len(), 1);
        assert_eq!(
            bridge
                .active_world(world.value)
                .unwrap()
                .solver
                .tether_count(),
            0
        );
        bridge.create_body_chain(request).unwrap();
        assert!(bridge.create_body_chain(request).is_err());
        let query = NativeDynamicsChainRequest { world, id: 8 };
        assert_eq!(bridge.read_chain(query).unwrap().point_count, 5);
        assert_eq!(bridge.active_world(world.value).unwrap().bodies.len(), 5);
        let point = bridge
            .read_chain_point(NativeDynamicsChainPointRequest {
                world,
                id: 8,
                index: 4,
            })
            .unwrap();
        assert!(point.present);
        assert_eq!(point.position.x, 2.0);
        bridge.destroy_body(body).unwrap();
        assert!(bridge.read_chain(query).unwrap().invalidated);
        assert_eq!(bridge.active_world(world.value).unwrap().bodies.len(), 0);
        assert_eq!(
            bridge
                .active_world(world.value)
                .unwrap()
                .solver
                .tether_count(),
            0
        );
        bridge.remove_chain(query).unwrap();
        assert!(!bridge.read_chain(query).unwrap().invalidated);
        bridge
            .create_fixed_chain(NativeDynamicsFixedChainRequest {
                world,
                anchor: NativeVec3::default(),
                end: request.end,
                config: request.config,
            })
            .unwrap();
        let receipt = bridge.remove_chain(query).unwrap();
        assert!(receipt.released);
        assert_eq!(receipt.removed_bodies, 4);
        assert!(!bridge.remove_chain(query).unwrap().released);
        assert_eq!(bridge.active_world(world.value).unwrap().bodies.len(), 0);
    }

    #[test]
    fn short_chain_collides_with_terrain_and_repeats_exactly() {
        let run = || {
            let spatial = crate::spatial::RuntimeSpatialBridge::new();
            let mut bridge = RuntimeDynamicsBridge::new(spatial.collision_source());
            let world = bridge
                .create_world(NativeDynamicsWorldConfig {
                    gravity: NativeVec3 {
                        x: 0.0,
                        y: -9.81,
                        z: 0.0,
                    },
                })
                .unwrap();
            let ground = (-6..7).flat_map(|x| (-2..3).map(move |z| [x, -1, z]));
            bridge.active_world_mut(world.value).unwrap().bind_scene(
                u64::MAX,
                &VoxelCollisionScene::from_solid_voxels(1.0, 8, ground).unwrap(),
            );
            bridge
                .create_fixed_chain(NativeDynamicsFixedChainRequest {
                    world,
                    anchor: NativeVec3 {
                        x: 0.0,
                        y: 2.0,
                        z: 0.0,
                    },
                    end: NativeVec3 {
                        x: 4.0,
                        y: 2.0,
                        z: 0.0,
                    },
                    config: chain_config(1, 8),
                })
                .unwrap();
            let mut touched_ground = false;
            for _ in 0..300 {
                bridge
                    .step(&NativeDynamicsStepRequest {
                        world,
                        step_seconds: ONE_SIXTIETH_SECOND,
                        steps: 1,
                        actions: std::ptr::null(),
                        actions_len: 0,
                    })
                    .unwrap();
                touched_ground |= bridge
                    .active_world(world.value)
                    .unwrap()
                    .solver
                    .contacts()
                    .iter()
                    .any(|contact| contact.second.is_none());
                for index in 1..=8 {
                    let point = bridge
                        .read_chain_point(NativeDynamicsChainPointRequest {
                            world,
                            id: 1,
                            index,
                        })
                        .unwrap();
                    assert!(
                        point.position.y >= 0.13,
                        "bead passed through terrain: {}",
                        point.position.y
                    );
                }
            }
            assert!(touched_ground);
            assert!(
                bridge
                    .read_chain(NativeDynamicsChainRequest { world, id: 1 })
                    .unwrap()
                    .simulated
            );
            (0..=8)
                .map(|index| {
                    let point = bridge
                        .read_chain_point(NativeDynamicsChainPointRequest {
                            world,
                            id: 1,
                            index,
                        })
                        .unwrap()
                        .position;
                    [point.x, point.y, point.z]
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn chain_length_controls_reel_the_links() {
        let spatial = crate::spatial::RuntimeSpatialBridge::new();
        let mut bridge = RuntimeDynamicsBridge::new(spatial.collision_source());
        let world = bridge
            .create_world(NativeDynamicsWorldConfig {
                gravity: NativeVec3::default(),
            })
            .unwrap();
        bridge
            .create_fixed_chain(NativeDynamicsFixedChainRequest {
                world,
                anchor: NativeVec3::default(),
                end: NativeVec3 {
                    x: 0.0,
                    y: -0.5,
                    z: 0.0,
                },
                config: chain_config(0, 1),
            })
            .unwrap();
        let control = NativeDynamicsChainLengthRequest {
            world,
            id: 0,
            target_length: 0.25,
            reel_speed: 0.25,
        };
        assert!(bridge
            .set_chain_length(NativeDynamicsChainLengthRequest {
                reel_speed: -1.0,
                ..control
            })
            .is_err());
        bridge.set_chain_length(control).unwrap();
        bridge
            .step(&NativeDynamicsStepRequest {
                world,
                step_seconds: ONE_SIXTIETH_SECOND,
                steps: 1,
                actions: std::ptr::null(),
                actions_len: 0,
            })
            .unwrap();
        let query = NativeDynamicsChainRequest { world, id: 0 };
        let shortened = bridge.read_chain(query).unwrap();
        assert!((shortened.effective_length - (0.5 - 0.25 * ONE_SIXTIETH_SECOND)).abs() < 1e-6);
        assert_eq!(shortened.target_length, 0.25);
        bridge
            .set_chain_length(NativeDynamicsChainLengthRequest {
                target_length: 0.75,
                ..control
            })
            .unwrap();
        bridge
            .step(&NativeDynamicsStepRequest {
                world,
                step_seconds: ONE_SIXTIETH_SECOND,
                steps: 1,
                actions: std::ptr::null(),
                actions_len: 0,
            })
            .unwrap();
        assert!((bridge.read_chain(query).unwrap().effective_length - 0.5).abs() < 1e-6);
    }

    #[test]
    fn chain_suppresses_adjacent_contacts_and_respects_self_collision_groups() {
        for self_collision in [true, false] {
            let spatial = crate::spatial::RuntimeSpatialBridge::new();
            let mut bridge = RuntimeDynamicsBridge::new(spatial.collision_source());
            let world = bridge
                .create_world(NativeDynamicsWorldConfig {
                    gravity: NativeVec3::default(),
                })
                .unwrap();
            let mut config = chain_config(1, 3);
            config.properties.collision_groups = 1;
            config.properties.collision_mask = if self_collision { u32::MAX } else { !1 };
            bridge
                .create_fixed_chain(NativeDynamicsFixedChainRequest {
                    world,
                    anchor: NativeVec3::default(),
                    end: NativeVec3 {
                        x: 0.03,
                        y: 0.0,
                        z: 0.0,
                    },
                    config,
                })
                .unwrap();
            bridge
                .step(&NativeDynamicsStepRequest {
                    world,
                    step_seconds: ONE_SIXTIETH_SECOND,
                    steps: 1,
                    actions: std::ptr::null(),
                    actions_len: 0,
                })
                .unwrap();
            let state = bridge.active_world(world.value).unwrap();
            let beads = state.chains[&1]
                .bodies
                .iter()
                .map(|bead| DynamicsBodyId(*bead))
                .collect::<Vec<_>>();
            for contact in state.solver.contacts() {
                assert_ne!((contact.first, contact.second), (beads[0], Some(beads[1])));
                assert_ne!((contact.first, contact.second), (beads[1], Some(beads[2])));
            }
            assert_eq!(
                state
                    .solver
                    .contacts()
                    .iter()
                    .any(|contact| contact.first == beads[0] && contact.second == Some(beads[2])),
                self_collision
            );
        }
    }

    #[test]
    fn anchor_observation_resolves_angular_velocity_and_reactions_apply_at_the_point() {
        let spatial = crate::spatial::RuntimeSpatialBridge::new();
        let mut bridge = RuntimeDynamicsBridge::new(spatial.collision_source());
        let world = bridge
            .create_world(NativeDynamicsWorldConfig {
                gravity: NativeVec3::default(),
            })
            .unwrap();
        let mut config = body_config(NativeVec3 {
            x: 3.0,
            y: 0.0,
            z: 0.0,
        });
        config.properties.linear_velocity.x = 1.0;
        config.properties.angular_velocity.z = 2.0;
        let body = bridge
            .create_body(&NativeDynamicsCreateBodyRequest {
                world,
                body: config,
            })
            .unwrap();
        let anchor = bridge
            .observe_anchor(NativeDynamicsObserveAnchorRequest {
                world,
                body,
                local_anchor: NativeVec3 {
                    x: 0.0,
                    y: 1.0,
                    z: 0.0,
                },
            })
            .unwrap();
        assert!(anchor.valid);
        assert_eq!(anchor.point.x, 3.0);
        assert_eq!(anchor.point.y, 1.0);
        assert_eq!(anchor.point_velocity.x, -1.0);
        let reaction = NativeDynamicsAnchorReaction {
            present: true,
            anchor,
            impulse: NativeVec3 {
                x: 2.0,
                y: 0.0,
                z: 0.0,
            },
        };
        let request = NativeDynamicsStepWithReactionsRequest {
            world,
            step_seconds: ONE_SIXTIETH_SECOND,
            steps: 1,
            actions: std::ptr::null(),
            actions_len: 0,
            reactions: &reaction,
            reactions_len: 1,
        };
        bridge.step_with_reactions(&request).unwrap();
        let after = bridge.read(NativeDynamicsReadRequest { body }).unwrap();
        assert!((after.linear_velocity.x - 2.0).abs() < 1e-5);
        assert!((after.angular_velocity.z + 4.0).abs() < 1e-4);
        // A reaction is an ordinary impulse: repeating it applies it again.
        bridge.step_with_reactions(&request).unwrap();
        let again = bridge.read(NativeDynamicsReadRequest { body }).unwrap();
        assert!((again.linear_velocity.x - 3.0).abs() < 1e-5);
        bridge.destroy_body(body).unwrap();
        assert!(bridge.step_with_reactions(&request).is_err());
    }

    #[test]
    fn character_reaction_and_dynamic_body_exchange_equal_and_opposite_momentum() {
        use engine_spatial::{
            CharacterControllerCommand, CharacterControllerConfig, CharacterControllerService,
            CharacterTetherRequest,
        };
        use entity_state::CharacterMotionComponent;
        let spatial = crate::spatial::RuntimeSpatialBridge::new();
        let mut bridge = RuntimeDynamicsBridge::new(spatial.collision_source());
        let world = bridge
            .create_world(NativeDynamicsWorldConfig {
                gravity: NativeVec3::default(),
            })
            .unwrap();
        let mut config = body_config(NativeVec3::default());
        config.properties.mass = 80.0;
        config.properties.linear_velocity.x = 1.0;
        let body = bridge
            .create_body(&NativeDynamicsCreateBodyRequest {
                world,
                body: config,
            })
            .unwrap();
        let anchor = bridge
            .observe_anchor(NativeDynamicsObserveAnchorRequest {
                world,
                body,
                local_anchor: NativeVec3::default(),
            })
            .unwrap();
        let mut motion = CharacterMotionComponent::at_rest(-3.0);
        motion.external_velocity = Vec3::new(0.0, -10.0, 0.0);
        let entity = EntityId::new(1);
        let mut entities =
            EntityState::from_definitions([EntityDefinition::new(entity, "character")
                .with_transform(Vec3::new(0.0, -3.0, 0.0))
                .with_character_motion(motion)])
            .unwrap();
        let mut controller = CharacterControllerConfig::default();
        controller.vertical.gravity = 0.0;
        controller.external_motion.external_decay_per_second = 0.0;
        controller.external_motion.authored_mass = 80.0;
        controller.external_motion.maximum_dynamic_impulse = 80.0;
        let tether = CharacterTetherRequest {
            anchor_id: anchor.body.value,
            anchor_point: native_vec3_value(anchor.point),
            anchor_velocity: native_vec3_value(anchor.point_velocity),
            anchor_response: [
                native_vec3_value(anchor.response_x),
                native_vec3_value(anchor.response_y),
                native_vec3_value(anchor.response_z),
            ],
            ..CharacterTetherRequest::fixed(1, Vec3::ZERO, 3.0)
        };
        let scene = VoxelCollisionScene::from_solid_voxels(1.0, 8, []).unwrap();
        let receipt = CharacterControllerService::default()
            .step(
                &mut entities,
                &scene,
                entity,
                &controller,
                CharacterControllerCommand {
                    tether: Some(tether),
                    ..CharacterControllerCommand::idle(ONE_SIXTIETH_SECOND, 1)
                },
            )
            .unwrap();
        let reaction = NativeDynamicsAnchorReaction {
            present: true,
            anchor,
            impulse: native_vec3(receipt.tether.reaction_impulse),
        };
        bridge
            .step_with_reactions(&NativeDynamicsStepWithReactionsRequest {
                world,
                step_seconds: ONE_SIXTIETH_SECOND,
                steps: 1,
                actions: std::ptr::null(),
                actions_len: 0,
                reactions: &reaction,
                reactions_len: 1,
            })
            .unwrap();
        let body_after = bridge.read(NativeDynamicsReadRequest { body }).unwrap();
        let character_delta = (receipt.motion_after.controlled_velocity
            + receipt.motion_after.external_velocity
            - motion.external_velocity)
            * 80.0;
        let body_delta =
            (native_vec3_value(body_after.linear_velocity) - Vec3::new(1.0, 0.0, 0.0)) * 80.0;
        assert!((character_delta + body_delta).length() < 0.001);
        assert!(receipt.tether.saturated && receipt.tether.unresolved);
    }

    #[test]
    fn light_dynamic_anchor_does_not_create_catch_energy() {
        use engine_spatial::{
            CharacterControllerCommand, CharacterControllerConfig, CharacterControllerService,
            CharacterTetherRequest,
        };
        use entity_state::CharacterMotionComponent;
        let spatial = crate::spatial::RuntimeSpatialBridge::new();
        let mut bridge = RuntimeDynamicsBridge::new(spatial.collision_source());
        let world = bridge
            .create_world(NativeDynamicsWorldConfig {
                gravity: NativeVec3::default(),
            })
            .unwrap();
        let mut config = body_config(NativeVec3::default());
        config.properties.mass = 2.0;
        config.properties.continuous_collision = true;
        config.properties.linear_velocity.x = 1.0;
        let body = bridge
            .create_body(&NativeDynamicsCreateBodyRequest {
                world,
                body: config,
            })
            .unwrap();
        let mut motion = CharacterMotionComponent::at_rest(-3.0);
        motion.external_velocity = Vec3::new(0.0, -10.0, 0.0);
        let entity = EntityId::new(1);
        let mut entities =
            EntityState::from_definitions([EntityDefinition::new(entity, "character")
                .with_transform(Vec3::new(0.0, -3.0, 0.0))
                .with_character_motion(motion)])
            .unwrap();
        let mut controller = CharacterControllerConfig::default();
        controller.vertical.gravity = 0.0;
        controller.external_motion.external_decay_per_second = 0.0;
        controller.external_motion.authored_mass = 80.0;
        controller.external_motion.maximum_dynamic_impulse = 500.0;
        let mut service = CharacterControllerService::default();
        for tick in 1..=240 {
            let anchor = bridge
                .observe_anchor(NativeDynamicsObserveAnchorRequest {
                    world,
                    body,
                    local_anchor: NativeVec3::default(),
                })
                .unwrap();
            let tether = CharacterTetherRequest {
                anchor_id: anchor.body.value,
                anchor_point: native_vec3_value(anchor.point),
                anchor_velocity: native_vec3_value(anchor.point_velocity),
                anchor_response: [
                    native_vec3_value(anchor.response_x),
                    native_vec3_value(anchor.response_y),
                    native_vec3_value(anchor.response_z),
                ],
                ..CharacterTetherRequest::fixed(1, Vec3::ZERO, 3.0)
            };
            let scene = VoxelCollisionScene::from_solid_voxels(1.0, 8, []).unwrap();
            let receipt = service
                .step(
                    &mut entities,
                    &scene,
                    entity,
                    &controller,
                    CharacterControllerCommand {
                        tether: Some(tether),
                        ..CharacterControllerCommand::idle(ONE_SIXTIETH_SECOND, tick)
                    },
                )
                .unwrap();
            let reaction = NativeDynamicsAnchorReaction {
                present: true,
                anchor,
                impulse: native_vec3(receipt.tether.reaction_impulse),
            };
            bridge
                .step_with_reactions(&NativeDynamicsStepWithReactionsRequest {
                    world,
                    step_seconds: ONE_SIXTIETH_SECOND,
                    steps: 1,
                    actions: std::ptr::null(),
                    actions_len: 0,
                    reactions: &reaction,
                    reactions_len: 1,
                })
                .unwrap();
            let body_after = bridge.read(NativeDynamicsReadRequest { body }).unwrap();
            let character_velocity =
                receipt.motion_after.controlled_velocity + receipt.motion_after.external_velocity;
            let energy = 0.5 * 80.0 * character_velocity.length_squared()
                + native_vec3_value(body_after.linear_velocity).length_squared();
            assert!(energy <= 4001.01, "catch created energy: {energy}");
            let momentum =
                character_velocity * 80.0 + native_vec3_value(body_after.linear_velocity) * 2.0;
            assert!((momentum - Vec3::new(2.0, -800.0, 0.0)).length() < 0.02);
        }
    }

    #[test]
    fn tether_bridge_steps_and_invalidates_when_anchor_is_destroyed() {
        let spatial = crate::spatial::RuntimeSpatialBridge::new();
        let mut bridge = RuntimeDynamicsBridge::new(spatial.collision_source());
        let world = bridge
            .create_world(NativeDynamicsWorldConfig {
                gravity: NativeVec3 {
                    x: 0.0,
                    y: -9.81,
                    z: 0.0,
                },
            })
            .unwrap();
        let mut config = body_config(NativeVec3 {
            x: 0.0,
            y: -3.0,
            z: 0.0,
        });
        config.properties.gravity_scale = 1.0;
        let body = bridge
            .create_body(&NativeDynamicsCreateBodyRequest {
                world,
                body: config,
            })
            .unwrap();
        bridge
            .set_fixed_tether(NativeDynamicsFixedTetherRequest {
                world,
                body,
                local_anchor: NativeVec3::default(),
                world_anchor: NativeVec3::default(),
                config: NativeDynamicsTetherConfig {
                    id: 7,
                    maximum_length: 3.0,
                    target_length: 3.0,
                    reel_speed: 0.25,
                    contacts_enabled: true,
                },
            })
            .unwrap();
        let query = NativeDynamicsTetherRequest { world, id: 7 };
        assert!(!bridge.read_tether(query).unwrap().simulated);
        bridge
            .execute_step(world.value, ONE_SIXTIETH_SECOND, 1, &[])
            .unwrap();
        let readout = bridge.read_tether(query).unwrap();
        assert!(readout.present && readout.simulated && readout.caught && !readout.invalidated);
        assert!((readout.force_proxy - 19.62).abs() < 0.01);
        bridge.destroy_body(body).unwrap();
        let invalid = bridge.read_tether(query).unwrap();
        assert!(!invalid.present && invalid.invalidated);
        bridge
            .execute_step(world.value, ONE_SIXTIETH_SECOND, 1, &[])
            .unwrap();
        bridge.remove_tether(query).unwrap();
        assert!(!bridge.read_tether(query).unwrap().invalidated);
    }

    #[test]
    fn fast_bodies_step_without_a_motion_cap_and_shapes_share_validation() {
        let spatial = crate::spatial::RuntimeSpatialBridge::new();
        let mut bridge = RuntimeDynamicsBridge::new(spatial.collision_source());
        let world = bridge
            .create_world(NativeDynamicsWorldConfig {
                gravity: NativeVec3::default(),
            })
            .unwrap();
        let mut config = body_config(NativeVec3::default());
        config.properties.linear_velocity.x = 120.0; // Two units per step.
        let step = NativeDynamicsStepRequest {
            world,
            step_seconds: ONE_SIXTIETH_SECOND,
            steps: 1,
            actions: std::ptr::null(),
            actions_len: 0,
        };
        for continuous in [false, true] {
            config.properties.continuous_collision = continuous;
            let body = bridge
                .create_body(&NativeDynamicsCreateBodyRequest {
                    world,
                    body: config,
                })
                .unwrap();
            bridge.step(&step).unwrap();
            let moved = bridge.read(NativeDynamicsReadRequest { body }).unwrap();
            assert!((moved.transform.translation.x - 2.0).abs() < 1e-4);
            bridge.destroy_body(body).unwrap();
        }

        let body = bridge
            .create_body(&NativeDynamicsCreateBodyRequest {
                world,
                body: config,
            })
            .unwrap();
        config.properties.mass = -1.0;
        let generic_error = bridge
            .replace_body(NativeDynamicsReplaceBodyRequest {
                body,
                replacement: config,
            })
            .unwrap_err();
        let shape_error = bridge
            .create_cuboid_body(&NativeDynamicsCreateCuboidBodyRequest {
                world,
                body: NativeDynamicsCuboidBodyConfig {
                    transform: config.transform,
                    half_extents: config.half_extents,
                    properties: config.properties,
                },
            })
            .unwrap_err();
        assert_eq!(generic_error.code(), shape_error.code());
        assert_eq!(generic_error.to_string(), shape_error.to_string());
        // A refused replacement leaves the original body in place.
        assert!(bridge.read(NativeDynamicsReadRequest { body }).is_ok());
    }

    #[test]
    fn bridge_steps_resets_replaces_and_disposes_in_either_order() {
        let spatial = crate::spatial::RuntimeSpatialBridge::new();
        let mut bridge = RuntimeDynamicsBridge::new(spatial.collision_source());
        let world = bridge
            .create_world(NativeDynamicsWorldConfig {
                gravity: NativeVec3::default(),
            })
            .unwrap();
        let body = bridge
            .create_body(&NativeDynamicsCreateBodyRequest {
                world,
                body: body_config(NativeVec3 {
                    x: 0.0,
                    y: 2.0,
                    z: 0.0,
                }),
            })
            .unwrap();
        let initial = bridge.read(NativeDynamicsReadRequest { body }).unwrap();
        assert!(initial.mass_properties.principal_inertia.x > 0.0);
        let actions = [NativeDynamicsAction {
            body,
            force: NativeVec3 {
                x: 2.0,
                y: 0.0,
                z: 0.0,
            },
            torque: NativeVec3 {
                x: 0.0,
                y: 0.0,
                z: 1.0,
            },
            impulse: NativeVec3::default(),
            torque_impulse: NativeVec3::default(),
            wake: true,
        }];
        bridge
            .step(&NativeDynamicsStepRequest {
                world,
                step_seconds: ONE_SIXTIETH_SECOND,
                steps: 1,
                actions: actions.as_ptr(),
                actions_len: actions.len(),
            })
            .unwrap();
        let driven = bridge.read(NativeDynamicsReadRequest { body }).unwrap();
        assert!(driven.linear_velocity.x > 0.0 && driven.angular_velocity.z > 0.0);
        bridge
            .reset(NativeDynamicsResetRequest {
                body,
                transform: transform(NativeVec3 {
                    x: 3.0,
                    y: 2.0,
                    z: 0.0,
                }),
                linear_velocity: NativeVec3::default(),
                angular_velocity: NativeVec3::default(),
                sleeping: false,
            })
            .unwrap();
        assert_eq!(
            bridge
                .read(NativeDynamicsReadRequest { body })
                .unwrap()
                .transform
                .translation
                .x,
            3.0
        );
        let replacement = bridge
            .replace_body(NativeDynamicsReplaceBodyRequest {
                body,
                replacement: body_config(NativeVec3::default()),
            })
            .unwrap();
        assert!(bridge.read(NativeDynamicsReadRequest { body }).is_err());
        bridge.destroy_body(body).unwrap();
        bridge.destroy_body(replacement).unwrap();
        bridge.destroy_world(world).unwrap();

        let parent_first_world = bridge
            .create_world(NativeDynamicsWorldConfig {
                gravity: NativeVec3::default(),
            })
            .unwrap();
        let parent_first_body = bridge
            .create_body(&NativeDynamicsCreateBodyRequest {
                world: parent_first_world,
                body: body_config(NativeVec3::default()),
            })
            .unwrap();
        bridge.destroy_world(parent_first_world).unwrap();
        bridge.destroy_body(parent_first_body).unwrap();
    }

    /// Copies one borrowed world result, as the generated managed caller does.
    fn world_facts(
        bridge: &mut RuntimeDynamicsBridge,
        world: NativeDynamicsWorldHandle,
    ) -> (u64, Vec<NativeDynamicsBodyFact>, Vec<NativeDynamicsContact>) {
        let result = bridge
            .read_world(NativeDynamicsWorldReadRequest { world })
            .unwrap();
        (
            result.generation,
            borrowed_copy(result.bodies, result.bodies_len),
            borrowed_copy(result.contacts, result.contacts_len),
        )
    }

    fn borrowed_copy<T: Copy>(pointer: *const T, len: usize) -> Vec<T> {
        match len {
            0 => Vec::new(),
            len => unsafe { std::slice::from_raw_parts(pointer, len) }.to_vec(),
        }
    }

    #[test]
    fn step_and_read_returns_ordered_borrowed_result_that_the_next_call_replaces() {
        let spatial = crate::spatial::RuntimeSpatialBridge::new();
        let mut bridge = RuntimeDynamicsBridge::new(spatial.collision_source());
        let world = bridge
            .create_world(NativeDynamicsWorldConfig {
                gravity: NativeVec3::default(),
            })
            .unwrap();
        let first = bridge
            .create_body(&NativeDynamicsCreateBodyRequest {
                world,
                body: body_config(NativeVec3::default()),
            })
            .unwrap();
        let second = bridge
            .create_body(&NativeDynamicsCreateBodyRequest {
                world,
                body: body_config(NativeVec3 {
                    x: 2.0,
                    y: 0.0,
                    z: 0.0,
                }),
            })
            .unwrap();
        let actions = [NativeDynamicsAction {
            body: first,
            force: NativeVec3 {
                x: 3.0,
                y: 0.0,
                z: 0.0,
            },
            torque: NativeVec3::default(),
            impulse: NativeVec3::default(),
            torque_impulse: NativeVec3::default(),
            wake: true,
        }];
        let step = |bridge: &mut RuntimeDynamicsBridge,
                    actions: &[NativeDynamicsAction],
                    bodies: &[NativeDynamicsBodyHandle]| {
            let result = bridge
                .step_and_read(&NativeDynamicsStepAndReadRequest {
                    world,
                    step_seconds: ONE_SIXTIETH_SECOND,
                    steps: 1,
                    actions: actions.as_ptr(),
                    actions_len: actions.len(),
                    bodies: bodies.as_ptr(),
                    bodies_len: bodies.len(),
                })
                .unwrap();
            (
                result.generation,
                borrowed_copy(result.bodies, result.bodies_len),
            )
        };

        let (ordinary_generation, ordinary) = step(&mut bridge, &actions, &[second, first]);
        assert_eq!(ordinary.len(), 2);
        assert_eq!(ordinary[0].body.value, second.value);
        assert_eq!(ordinary[1].body.value, first.value);
        assert!(ordinary[1].readout.linear_velocity.x > 0.0);

        // An empty selection still steps exactly once.
        let (empty_generation, empty) = step(&mut bridge, &[], &[]);
        assert!(empty.is_empty());
        assert_eq!(empty_generation, ordinary_generation + 1);

        // A larger selection grows the bridge buffer inside the one call; the
        // step is not repeated to fit the result.
        let many = [first, second, first, second, first, second, first, second];
        let (grown_generation, grown) = step(&mut bridge, &[], &many);
        assert_eq!(grown_generation, empty_generation + 1);
        assert_eq!(grown.len(), many.len());
        for (fact, body) in grown.iter().zip(many) {
            assert_eq!(fact.body.value, body.value);
        }
        assert_eq!(bridge.body_facts.len(), many.len());
    }

    #[test]
    fn bridge_exposes_full_dynamic_shape_properties_replacement_and_bounded_world_readouts() {
        let spatial = crate::spatial::RuntimeSpatialBridge::new();
        let mut bridge = RuntimeDynamicsBridge::new(spatial.collision_source());
        let world = bridge
            .create_world(NativeDynamicsWorldConfig {
                gravity: NativeVec3::default(),
            })
            .unwrap();
        let properties = NativeDynamicsBodyProperties {
            mass: 4.0,
            mass_policy: NativeDynamicsMassPolicy::default(),
            linear_velocity: NativeVec3::default(),
            angular_velocity: NativeVec3::default(),
            axis_locks: NativeAxisLocks {
                translation_x: true,
                translation_y: false,
                translation_z: false,
                rotation_x: false,
                rotation_y: true,
                rotation_z: false,
            },
            linear_damping: 0.2,
            angular_damping: 0.3,
            gravity_scale: 0.5,
            friction: 0.8,
            restitution: 0.4,
            collision_groups: 2,
            collision_mask: 4,
            enabled: true,
            sleeping: false,
            continuous_collision: true,
        };
        let generic = NativeDynamicsBodyConfig {
            properties,
            ..body_config(NativeVec3::default())
        };
        let shape_specific = NativeDynamicsCuboidBodyConfig {
            transform: generic.transform,
            half_extents: generic.half_extents,
            properties,
        };
        assert_eq!(
            cuboid_body_config(generic).unwrap().body,
            cuboid_body_properties_config(shape_specific).unwrap().body,
        );
        let cuboid = bridge
            .create_cuboid_body(&NativeDynamicsCreateCuboidBodyRequest {
                world,
                body: NativeDynamicsCuboidBodyConfig {
                    transform: transform(NativeVec3::default()),
                    half_extents: NativeVec3 {
                        x: 0.25,
                        y: 0.5,
                        z: 0.75,
                    },
                    properties,
                },
            })
            .unwrap();
        let (_, bodies, _) = world_facts(&mut bridge, world);
        assert_eq!(bodies.len(), 1);
        assert!(
            bodies[0].body.value == cuboid.value && bodies[0].readout.mass_properties.available
        );
        bridge
            .update_body(NativeDynamicsUpdateBodyRequest {
                body: cuboid,
                properties: NativeDynamicsBodyProperties {
                    sleeping: true,
                    ..properties
                },
            })
            .unwrap();
        assert!(
            bridge
                .read(NativeDynamicsReadRequest { body: cuboid })
                .unwrap()
                .sleeping
        );
        let capsule = bridge
            .create_capsule_body(&NativeDynamicsCreateCapsuleBodyRequest {
                world,
                body: NativeDynamicsCapsuleBodyConfig {
                    transform: transform(NativeVec3 {
                        x: 2.0,
                        y: 0.0,
                        z: 0.0,
                    }),
                    half_height: 0.75,
                    radius: 0.25,
                    properties,
                },
            })
            .unwrap();
        assert!(
            !bridge
                .read(NativeDynamicsReadRequest { body: capsule })
                .unwrap()
                .mass_properties
                .available
        );
        let sphere = bridge
            .replace_sphere_body(NativeDynamicsReplaceSphereBodyRequest {
                body: cuboid,
                replacement: NativeDynamicsSphereBodyPropertiesConfig {
                    transform: transform(NativeVec3::default()),
                    radius: 0.5,
                    properties,
                },
            })
            .unwrap();
        assert!(bridge
            .read(NativeDynamicsReadRequest { body: cuboid })
            .is_err());
        assert!(
            bridge
                .read(NativeDynamicsReadRequest { body: sphere })
                .unwrap()
                .mass_properties
                .available
        );
        bridge.destroy_body(cuboid).unwrap();
        bridge.destroy_body(sphere).unwrap();
        bridge.destroy_body(capsule).unwrap();
        bridge.destroy_world(world).unwrap();
    }

    #[test]
    fn bind_world_collision_uses_the_spatial_scene() {
        let mut spatial = crate::spatial::RuntimeSpatialBridge::new();
        let mut bridge = RuntimeDynamicsBridge::new(spatial.collision_source());
        let spatial_api = crate::spatial::api(&mut spatial);
        let mut session = NativeSpatialSessionHandle::default();
        assert_eq!(
            unsafe {
                (spatial_api.create_session)(
                    spatial_api.context,
                    NativeSpatialSessionConfig {
                        collision_voxel_size: 1.0,
                        collision_chunk_size: 8,
                        voxel_surface_mode: NativeVoxelSurfaceMode::GreedyCubes,
                    },
                    &mut session,
                    std::ptr::null_mut(),
                )
            },
            ABI_OK
        );

        let vertices = [
            NativeVec3 {
                x: -10.0,
                y: 0.0,
                z: -10.0,
            },
            NativeVec3 {
                x: 10.0,
                y: 0.0,
                z: -10.0,
            },
            NativeVec3 {
                x: 10.0,
                y: 0.0,
                z: 10.0,
            },
            NativeVec3 {
                x: -10.0,
                y: 0.0,
                z: 10.0,
            },
        ];
        let assets = [NativeStaticMeshAsset {
            id: 1,
            mesh_resource: NativeMeshResourceReference { value: 0 },
            first_vertex: 0,
            vertex_count: vertices.len() as u32,
            first_triangle: 0,
            triangle_count: 2,
        }];
        let triangles = [
            NativeTriangle { a: 0, b: 1, c: 2 },
            NativeTriangle { a: 0, b: 2, c: 3 },
        ];
        let instances = [NativeStaticMeshInstance {
            id: 1,
            asset: 1,
            transform: transform(NativeVec3::default()),
        }];
        let request = NativeCollisionReplaceRequest {
            session,
            assets: assets.as_ptr(),
            assets_len: assets.len(),
            vertices: vertices.as_ptr(),
            vertices_len: vertices.len(),
            triangles: triangles.as_ptr(),
            triangles_len: triangles.len(),
            instances: instances.as_ptr(),
            instances_len: instances.len(),
        };
        let mut replace = NativeCollisionReplaceReceipt::default();
        assert_eq!(
            unsafe {
                (spatial_api.replace_collision)(
                    spatial_api.context,
                    &request,
                    &mut replace,
                    std::ptr::null_mut(),
                )
            },
            ABI_OK
        );
        assert_eq!(replace.instance_count, 1);

        let world = bridge
            .create_world(NativeDynamicsWorldConfig {
                gravity: NativeVec3::default(),
            })
            .unwrap();
        let body = bridge
            .create_body(&NativeDynamicsCreateBodyRequest {
                world,
                body: body_config(NativeVec3 {
                    x: 0.0,
                    y: 0.4,
                    z: 0.0,
                }),
            })
            .unwrap();
        assert_eq!(
            bridge
                .step(&NativeDynamicsStepRequest {
                    world,
                    step_seconds: ONE_SIXTIETH_SECOND,
                    steps: 1,
                    actions: std::ptr::null(),
                    actions_len: 0,
                })
                .unwrap()
                .contact_count,
            0
        );

        bridge
            .bind_world_collision(NativeDynamicsWorldCollisionBindingRequest {
                world,
                spatial_session: session,
            })
            .unwrap();
        let contact = bridge
            .step(&NativeDynamicsStepRequest {
                world,
                step_seconds: ONE_SIXTIETH_SECOND,
                steps: 1,
                actions: std::ptr::null(),
                actions_len: 0,
            })
            .unwrap();
        assert!(contact.contact_count > 0);
        let indexed = world_facts(&mut bridge, world).2[0];
        assert!(
            indexed.environment && indexed.first.value == body.value && indexed.second.value == 0
        );

        let readout = bridge.read(NativeDynamicsReadRequest { body }).unwrap();
        assert!(bridge
            .bind_world_collision(NativeDynamicsWorldCollisionBindingRequest {
                world,
                spatial_session: NativeSpatialSessionHandle { value: u64::MAX },
            })
            .is_err());
        let after_rejected_bind = bridge.read(NativeDynamicsReadRequest { body }).unwrap();
        assert_eq!(
            [
                after_rejected_bind.transform.translation.x,
                after_rejected_bind.transform.translation.y,
                after_rejected_bind.transform.translation.z,
                after_rejected_bind.linear_velocity.x,
                after_rejected_bind.linear_velocity.y,
                after_rejected_bind.linear_velocity.z,
            ],
            [
                readout.transform.translation.x,
                readout.transform.translation.y,
                readout.transform.translation.z,
                readout.linear_velocity.x,
                readout.linear_velocity.y,
                readout.linear_velocity.z,
            ]
        );
        assert!(
            bridge
                .step(&NativeDynamicsStepRequest {
                    world,
                    step_seconds: ONE_SIXTIETH_SECOND,
                    steps: 1,
                    actions: std::ptr::null(),
                    actions_len: 0,
                })
                .unwrap()
                .contact_count
                > 0
        );
    }

    #[test]
    fn rebase_world_origin_moves_bodies_ropes_and_environment_together() {
        let mut spatial = crate::spatial::RuntimeSpatialBridge::new();
        let mut bridge = RuntimeDynamicsBridge::new(spatial.collision_source());
        let spatial_api = crate::spatial::api(&mut spatial);
        let world_origin_api = crate::world_origin::api(&mut spatial);
        let mut session = NativeSpatialSessionHandle::default();
        assert_eq!(
            unsafe {
                (spatial_api.create_session)(
                    spatial_api.context,
                    NativeSpatialSessionConfig {
                        collision_voxel_size: 1.0,
                        collision_chunk_size: 8,
                        voxel_surface_mode: NativeVoxelSurfaceMode::GreedyCubes,
                    },
                    &mut session,
                    std::ptr::null_mut(),
                )
            },
            ABI_OK
        );

        let vertices = [
            NativeVec3 {
                x: -10.0,
                y: 0.0,
                z: -10.0,
            },
            NativeVec3 {
                x: 10.0,
                y: 0.0,
                z: -10.0,
            },
            NativeVec3 {
                x: 10.0,
                y: 0.0,
                z: 10.0,
            },
            NativeVec3 {
                x: -10.0,
                y: 0.0,
                z: 10.0,
            },
        ];
        let assets = [NativeStaticMeshAsset {
            id: 1,
            mesh_resource: NativeMeshResourceReference { value: 0 },
            first_vertex: 0,
            vertex_count: vertices.len() as u32,
            first_triangle: 0,
            triangle_count: 2,
        }];
        let triangles = [
            NativeTriangle { a: 0, b: 1, c: 2 },
            NativeTriangle { a: 0, b: 2, c: 3 },
        ];
        let instances = [NativeStaticMeshInstance {
            id: 1,
            asset: 1,
            transform: transform(NativeVec3::default()),
        }];
        let mut collision = NativeCollisionReplaceReceipt::default();
        assert_eq!(
            unsafe {
                (spatial_api.replace_collision)(
                    spatial_api.context,
                    &NativeCollisionReplaceRequest {
                        session,
                        assets: assets.as_ptr(),
                        assets_len: assets.len(),
                        vertices: vertices.as_ptr(),
                        vertices_len: vertices.len(),
                        triangles: triangles.as_ptr(),
                        triangles_len: triangles.len(),
                        instances: instances.as_ptr(),
                        instances_len: instances.len(),
                    },
                    &mut collision,
                    std::ptr::null_mut(),
                )
            },
            ABI_OK
        );

        let world = bridge
            .create_world(NativeDynamicsWorldConfig {
                gravity: NativeVec3::default(),
            })
            .unwrap();
        let body = bridge
            .create_body(&NativeDynamicsCreateBodyRequest {
                world,
                body: body_config(NativeVec3 {
                    x: 0.0,
                    y: 0.4,
                    z: 0.0,
                }),
            })
            .unwrap();
        bridge
            .bind_world_collision(NativeDynamicsWorldCollisionBindingRequest {
                world,
                spatial_session: session,
            })
            .unwrap();
        let step = bridge
            .step(&NativeDynamicsStepRequest {
                world,
                step_seconds: ONE_SIXTIETH_SECOND,
                steps: 1,
                actions: std::ptr::null(),
                actions_len: 0,
            })
            .unwrap();
        assert!(step.contact_count > 0);
        bridge
            .set_fixed_tether(NativeDynamicsFixedTetherRequest {
                world,
                body,
                local_anchor: NativeVec3::default(),
                world_anchor: NativeVec3 {
                    x: 0.0,
                    y: 2.0,
                    z: 0.0,
                },
                config: NativeDynamicsTetherConfig {
                    id: 71,
                    maximum_length: 3.0,
                    target_length: 3.0,
                    reel_speed: 0.0,
                    contacts_enabled: false,
                },
            })
            .unwrap();

        let prepare = NativeWorldOriginPrepareRequest {
            session,
            target_cell_x: 5,
            target_cell_y: 0,
            target_cell_z: 0,
            entities: std::ptr::null(),
            entities_len: 0,
            exclude_outside_envelope: false,
        };
        let mut prepared = NativeWorldOriginPreparedHandle::default();
        assert_eq!(
            unsafe {
                (world_origin_api.prepare)(
                    world_origin_api.context,
                    &prepare,
                    &mut prepared,
                    std::ptr::null_mut(),
                )
            },
            ABI_OK
        );
        let mut receipt = NativeWorldOriginCommitReceipt::default();
        assert_eq!(
            unsafe {
                (world_origin_api.commit)(
                    world_origin_api.context,
                    NativeWorldOriginCommitRequest { prepared },
                    &mut receipt,
                    std::ptr::null_mut(),
                )
            },
            ABI_OK
        );

        let (before_generation, _, before_contacts) = world_facts(&mut bridge, world);
        let before_body = bridge.read(NativeDynamicsReadRequest { body }).unwrap();
        let before_contact = before_contacts[0];
        bridge
            .rebase_world_origin(NativeDynamicsRebaseWorldOriginRequest {
                world,
                spatial_session: session,
                receipt,
            })
            .unwrap();
        let rebased_tether = bridge
            .active_world(world.value)
            .unwrap()
            .solver
            .tether(71)
            .unwrap();
        assert_eq!(
            rebased_tether.first,
            DynamicsTetherEndpoint::Fixed([-5.0, 2.0, 0.0])
        );
        assert_eq!(rebased_tether.maximum_length, 3.0);
        assert_eq!(
            rebased_tether.second,
            DynamicsTetherEndpoint::Body {
                body: DynamicsBodyId(body.value),
                local_anchor: [0.0; 3]
            }
        );
        let (after_generation, _, after_contacts) = world_facts(&mut bridge, world);
        let after_body = bridge.read(NativeDynamicsReadRequest { body }).unwrap();
        let after_contact = after_contacts[0];
        assert_eq!(after_generation, before_generation);
        assert_eq!(
            after_body.transform.translation.x,
            before_body.transform.translation.x - 5.0
        );
        assert_eq!(
            after_body.transform.translation.y,
            before_body.transform.translation.y
        );
        assert_eq!(
            after_body.transform.rotation.w,
            before_body.transform.rotation.w
        );
        assert_eq!(after_body.linear_velocity.x, before_body.linear_velocity.x);
        assert_eq!(
            after_body.angular_velocity.y,
            before_body.angular_velocity.y
        );
        assert_eq!(after_body.sleeping, before_body.sleeping);
        assert!(after_contact.environment);
        assert_eq!(after_contact.first.value, before_contact.first.value);
        assert_eq!(
            after_contact.impulse_magnitude,
            before_contact.impulse_magnitude
        );

        // The rebased floor is still under the body.
        let step = bridge
            .step(&NativeDynamicsStepRequest {
                world,
                step_seconds: ONE_SIXTIETH_SECOND,
                steps: 1,
                actions: std::ptr::null(),
                actions_len: 0,
            })
            .unwrap();
        assert!(step.contact_count > 0);
        let stepped = bridge.read(NativeDynamicsReadRequest { body }).unwrap();
        assert!(
            (stepped.transform.translation.x - after_body.transform.translation.x).abs() < 0.01
        );
        assert!(bridge
            .rebase_world_origin(NativeDynamicsRebaseWorldOriginRequest {
                world,
                spatial_session: NativeSpatialSessionHandle { value: u64::MAX },
                receipt,
            })
            .is_err());
    }
}
