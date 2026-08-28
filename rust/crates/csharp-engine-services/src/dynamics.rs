use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::c_void,
    sync::Arc,
};

use core_ids::EntityId;
use core_math::Vec3;
use core_space::{GlobalPosition, WorldOrigin};
use csharp_engine_abi::*;
use engine_spatial::{
    rigid_body_component_mass_properties, RigidBodyAction, RigidBodyContactReadout,
    RigidBodyService, RigidBodyStepRequest, VoxelCollisionScene,
};
use entity_state::{
    replace_rigid_body_states, EntityAuthoringService, EntityDefinition, EntityLifecycle,
    EntityState, EntityTransform, Quat, RigidBodyComponent, RigidBodyInertiaPolicy, RigidBodyShape,
    RigidBodyStateReplacement, TransformComponent,
};

use crate::composition::{
    borrowed_slice, native_quat, native_quat_value, native_vec3, native_vec3_value,
    CsharpEngineServicesError, ABI_OK,
};
use crate::spatial::SpatialCollisionSource;

/// A retained Engine dynamics world. The EntityState, collision scene and
/// RigidBodyService remain one Engine-owned aggregate; C# receives only typed
/// handles and stable readouts.
pub(crate) struct RuntimeDynamicsBridge {
    worlds: BTreeMap<u64, WorldSlot>,
    bodies: BTreeMap<u64, BodySlot>,
    collision_source: SpatialCollisionSource,
    next_world: u64,
    next_body: u64,
    next_entity: u64,
}

enum WorldSlot {
    Active(DynamicsWorld),
    Tombstoned,
}

struct DynamicsWorld {
    entities: EntityState,
    scene: Arc<VoxelCollisionScene>,
    bound_spatial_session: Option<NativeSpatialSessionHandle>,
    bodies: BTreeMap<u64, EntityId>,
    service: RigidBodyService,
    gravity: Vec3,
    last_contacts: BTreeMap<EntityId, BodyContactSummary>,
    last_contact_receipts: Vec<RigidBodyContactReadout>,
}

#[derive(Clone, Copy, Default)]
struct BodyContactSummary {
    count: u32,
    latest: NativeDynamicsContactFact,
}

enum BodySlot {
    Active { world: u64, entity: EntityId },
    Tombstoned,
}

impl RuntimeDynamicsBridge {
    pub(crate) fn new(collision_source: SpatialCollisionSource) -> Self {
        Self {
            worlds: BTreeMap::new(),
            bodies: BTreeMap::new(),
            collision_source,
            next_world: 1,
            next_body: 1,
            next_entity: 1,
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
        let scene = Arc::new(
            VoxelCollisionScene::from_solid_voxels(1.0, 8, std::iter::empty::<[i64; 3]>())
                .map_err(|error| {
                    CsharpEngineServicesError::new("CSHARP_DYNAMICS_WORLD", error.to_string())
                })?,
        );
        let value = Self::allocate(&mut self.next_world, "world")?;
        self.worlds.insert(
            value,
            WorldSlot::Active(DynamicsWorld {
                entities: EntityState::from_definitions(std::iter::empty::<EntityDefinition>())
                    .map_err(|error| {
                        CsharpEngineServicesError::new("CSHARP_DYNAMICS_WORLD", error.to_string())
                    })?,
                scene,
                bound_spatial_session: None,
                bodies: BTreeMap::new(),
                service: RigidBodyService::default(),
                gravity,
                last_contacts: BTreeMap::new(),
                last_contact_receipts: Vec::new(),
            }),
        );
        Ok(NativeDynamicsWorldHandle { value })
    }

    fn destroy_world(
        &mut self,
        handle: NativeDynamicsWorldHandle,
    ) -> Result<(), CsharpEngineServicesError> {
        let body_handles = match self.worlds.get(&handle.value) {
            Some(WorldSlot::Active(world)) => world.bodies.keys().copied().collect::<Vec<_>>(),
            Some(WorldSlot::Tombstoned) => return Ok(()),
            None => return Err(unknown("world", handle.value)),
        };
        self.worlds.insert(handle.value, WorldSlot::Tombstoned);
        for body in body_handles {
            self.bodies.insert(body, BodySlot::Tombstoned);
        }
        Ok(())
    }

    /// Bind an immutable Engine-owned collision projection snapshot at an
    /// explicit product update boundary. Spatial replacement publishes a new
    /// snapshot; a world therefore changes environment only after another
    /// successful bind, never during solver publication.
    fn bind_world_collision(
        &mut self,
        request: NativeDynamicsWorldCollisionBindingRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let scene = self.collision_source.scene(request.spatial_session)?;
        let world = self.active_world_mut(request.world.value)?;
        world.scene = scene;
        world.bound_spatial_session = Some(request.spatial_session);
        world.last_contacts.clear();
        world.last_contact_receipts.clear();
        Ok(())
    }

    fn rebase_world_origin(
        &mut self,
        request: NativeDynamicsRebaseWorldOriginRequest,
    ) -> Result<NativeDynamicsRebaseWorldOriginReceipt, CsharpEngineServicesError> {
        let latest_scene = self.collision_source.scene(request.spatial_session)?;
        let (bound_session, current_scene, body_members, entity_revision, solver_generation) =
            self.rebase_snapshot(request.world.value)?;
        if bound_session != Some(request.spatial_session) {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_DYNAMICS_REBASE",
                "world was not bound to the supplied spatial session",
            ));
        }
        if entity_revision != request.expected_entity_revision {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_DYNAMICS_REBASE",
                "dynamics entity revision was stale",
            ));
        }
        if solver_generation != request.expected_solver_generation {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_DYNAMICS_REBASE",
                "dynamics solver generation was stale",
            ));
        }
        validate_rebase_receipt(&request.receipt)?;
        validate_scene_before(current_scene.as_ref(), &request.receipt)?;
        validate_scene_after(latest_scene.as_ref(), &request.receipt)?;
        self.validate_body_handles(request.world.value, &body_members)?;

        let candidate = {
            let world = self.active_world(request.world.value)?;
            let body_entities = body_members.values().copied().collect::<BTreeSet<_>>();
            let state_entities = world
                .entities
                .rigid_bodies()
                .map(|(entity, _)| entity)
                .collect::<BTreeSet<_>>();
            if body_entities != state_entities {
                return Err(CsharpEngineServicesError::new(
                    "CSHARP_DYNAMICS_REBASE",
                    "dynamics body mapping did not match active rigid-body state",
                ));
            }
            let replacements = body_entities
                .iter()
                .copied()
                .map(|entity| rebase_body_replacement(&world.entities, entity, &request.receipt))
                .collect::<Result<Vec<_>, _>>()?;
            let mut candidate = world.entities.clone();
            replace_rigid_body_states(&mut candidate, replacements).map_err(|error| {
                CsharpEngineServicesError::new("CSHARP_DYNAMICS_REBASE", error.code())
            })?;
            candidate
        };
        let receipt = NativeDynamicsRebaseWorldOriginReceipt {
            entity_revision_before: entity_revision,
            entity_revision_after: candidate.revision(),
            solver_generation,
            body_count: u32::try_from(body_members.len()).map_err(|_| {
                CsharpEngineServicesError::new("CSHARP_DYNAMICS_REBASE", "body count exceeded u32")
            })?,
            contact_count: u32::try_from(
                self.active_world(request.world.value)?
                    .last_contact_receipts
                    .len(),
            )
            .map_err(|_| {
                CsharpEngineServicesError::new(
                    "CSHARP_DYNAMICS_REBASE",
                    "contact count exceeded u32",
                )
            })?,
        };

        // There are no fallible operations after this point. The body owners,
        // solver generation, and last contact facts remain intact while the
        // state and scene change as one committed Dynamics-world snapshot.
        let world = self.active_world_mut(request.world.value)?;
        world.entities = candidate;
        world.scene = latest_scene;
        Ok(receipt)
    }

    fn create_body(
        &mut self,
        request: &NativeDynamicsCreateBodyRequest,
    ) -> Result<NativeDynamicsBodyHandle, CsharpEngineServicesError> {
        self.create_body_with_config(request.world, cuboid_body_config(request.body)?)
    }

    fn create_sphere_body(
        &mut self,
        request: &NativeDynamicsCreateSphereBodyRequest,
    ) -> Result<NativeDynamicsBodyHandle, CsharpEngineServicesError> {
        self.create_body_with_config(request.world, sphere_body_config(request.body)?)
    }

    fn create_cuboid_body(
        &mut self,
        request: &NativeDynamicsCreateCuboidBodyRequest,
    ) -> Result<NativeDynamicsBodyHandle, CsharpEngineServicesError> {
        self.create_body_with_config(request.world, cuboid_body_properties_config(request.body)?)
    }

    fn create_sphere_body_with_properties(
        &mut self,
        request: &NativeDynamicsCreateSphereBodyPropertiesRequest,
    ) -> Result<NativeDynamicsBodyHandle, CsharpEngineServicesError> {
        self.create_body_with_config(request.world, sphere_body_properties_config(request.body)?)
    }

    fn create_capsule_body(
        &mut self,
        request: &NativeDynamicsCreateCapsuleBodyRequest,
    ) -> Result<NativeDynamicsBodyHandle, CsharpEngineServicesError> {
        self.create_body_with_config(request.world, capsule_body_config(request.body)?)
    }

    fn create_body_with_config(
        &mut self,
        world_handle: NativeDynamicsWorldHandle,
        config: BodyConfig,
    ) -> Result<NativeDynamicsBodyHandle, CsharpEngineServicesError> {
        let entity_value = Self::allocate(&mut self.next_entity, "entity")?;
        let body_handle = Self::allocate(&mut self.next_body, "body")?;
        let entity = EntityId::new(entity_value);
        let world = self.active_world_mut(world_handle.value)?;
        let mut candidate = world.entities.clone();
        insert_body(&mut candidate, entity, config)?;
        world.entities = candidate;
        world.bodies.insert(body_handle, entity);
        self.bodies.insert(
            body_handle,
            BodySlot::Active {
                world: world_handle.value,
                entity,
            },
        );
        Ok(NativeDynamicsBodyHandle { value: body_handle })
    }

    fn destroy_body(
        &mut self,
        handle: NativeDynamicsBodyHandle,
    ) -> Result<(), CsharpEngineServicesError> {
        let (world_handle, entity) = match self.bodies.get(&handle.value) {
            Some(BodySlot::Active { world, entity }) => (*world, *entity),
            Some(BodySlot::Tombstoned) => return Ok(()),
            None => return Err(unknown("body", handle.value)),
        };
        let world = self.active_world_mut(world_handle)?;
        let mut candidate = world.entities.clone();
        let revision = candidate.revision();
        EntityAuthoringService
            .destroy(&mut candidate, revision, entity)
            .map_err(|error| {
                CsharpEngineServicesError::new("CSHARP_DYNAMICS_DESTROY", error.to_string())
            })?;
        world.entities = candidate;
        world.bodies.remove(&handle.value);
        world.last_contacts.remove(&entity);
        world
            .last_contact_receipts
            .retain(|contact| contact.first != entity && contact.second != Some(entity));
        self.bodies.insert(handle.value, BodySlot::Tombstoned);
        Ok(())
    }

    fn step(
        &mut self,
        request: &NativeDynamicsStepRequest,
    ) -> Result<NativeDynamicsStepReceipt, CsharpEngineServicesError> {
        let steps = u8::try_from(request.steps).map_err(|_| {
            CsharpEngineServicesError::new("CSHARP_DYNAMICS_STEP", "steps exceeded Engine u8 limit")
        })?;
        let actions =
            unsafe { borrowed_slice(request.actions, request.actions_len, "dynamics actions") }?;
        let active = actions
            .iter()
            .map(|action| {
                let (world, entity) = self.active_body(action.body.value)?;
                if world != request.world.value {
                    return Err(CsharpEngineServicesError::new(
                        "CSHARP_DYNAMICS_BODY",
                        "action body belonged to another world",
                    ));
                }
                Ok(RigidBodyAction {
                    entity,
                    force: native_vec3_value(action.force),
                    torque: native_vec3_value(action.torque),
                    impulse: native_vec3_value(action.impulse),
                    torque_impulse: native_vec3_value(action.torque_impulse),
                    wake: action.wake,
                })
            })
            .collect::<Result<Vec<_>, CsharpEngineServicesError>>()?;
        if active.iter().any(|action| {
            !finite_vec3(action.force)
                || !finite_vec3(action.torque)
                || !finite_vec3(action.impulse)
                || !finite_vec3(action.torque_impulse)
        }) {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_DYNAMICS_STEP",
                "action contained a non-finite vector",
            ));
        }
        let world = self.active_world_mut(request.world.value)?;
        let receipt = world
            .service
            .step(
                &mut world.entities,
                world.scene.as_ref(),
                RigidBodyStepRequest {
                    step_seconds: request.step_seconds,
                    steps,
                    gravity: world.gravity,
                    actions: active,
                },
            )
            .map_err(|error| {
                CsharpEngineServicesError::new("CSHARP_DYNAMICS_STEP", error.code())
            })?;
        world.last_contacts = contacts_by_body(&receipt);
        world.last_contact_receipts = receipt.contacts.clone();
        let output = NativeDynamicsStepReceipt {
            generation: receipt.generation,
            body_count: u32::try_from(receipt.bodies_considered).map_err(|_| {
                CsharpEngineServicesError::new("CSHARP_DYNAMICS_STEP", "body count exceeded u32")
            })?,
            contact_count: u32::try_from(receipt.contacts.len()).map_err(|_| {
                CsharpEngineServicesError::new("CSHARP_DYNAMICS_STEP", "contact count exceeded u32")
            })?,
        };
        Ok(output)
    }

    fn read(
        &mut self,
        request: NativeDynamicsReadRequest,
    ) -> Result<NativeDynamicsReadout, CsharpEngineServicesError> {
        let (world_handle, entity) = self.active_body(request.body.value)?;
        let world = self.active_world_mut(world_handle)?;
        read_entity(world, entity)
    }

    fn reset(
        &mut self,
        request: NativeDynamicsResetRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let (world_handle, entity) = self.active_body(request.body.value)?;
        let transform = checked_transform(request.transform)?;
        let world = self.active_world_mut(world_handle)?;
        let before = world.entities.rigid_body(entity).copied().ok_or_else(|| {
            CsharpEngineServicesError::new("CSHARP_DYNAMICS_RESET", "body lacked dynamics state")
        })?;
        let mut body = before;
        body.linear_velocity = native_vec3_value(request.linear_velocity);
        body.angular_velocity = native_vec3_value(request.angular_velocity);
        body.sleeping = request.sleeping;
        entity_state::validate_rigid_body(&body).map_err(|error| {
            CsharpEngineServicesError::new("CSHARP_DYNAMICS_RESET", error.code())
        })?;
        let replacement = RigidBodyStateReplacement {
            entity,
            expected_transform_revision: world
                .entities
                .component_revision::<TransformComponent>(entity)
                .map_err(|error| {
                    CsharpEngineServicesError::new("CSHARP_DYNAMICS_RESET", error.to_string())
                })?,
            expected_rigid_body_revision: world
                .entities
                .component_revision::<RigidBodyComponent>(entity)
                .map_err(|error| {
                    CsharpEngineServicesError::new("CSHARP_DYNAMICS_RESET", error.to_string())
                })?,
            transform: TransformComponent::from_transform(transform),
            rigid_body: body,
        };
        replace_rigid_body_states(&mut world.entities, vec![replacement]).map_err(|error| {
            CsharpEngineServicesError::new("CSHARP_DYNAMICS_RESET", error.code())
        })?;
        world.last_contacts.remove(&entity);
        world
            .last_contact_receipts
            .retain(|contact| contact.first != entity && contact.second != Some(entity));
        Ok(())
    }

    fn read_body_at(
        &mut self,
        request: NativeDynamicsBodyAtRequest,
    ) -> Result<NativeDynamicsBodyAtReceipt, CsharpEngineServicesError> {
        let world = self.active_world_mut(request.world.value)?;
        let body = world
            .bodies
            .iter()
            .nth(request.index as usize)
            .map(|(handle, entity)| (*handle, *entity));
        let Some((handle, entity)) = body else {
            return Ok(NativeDynamicsBodyAtReceipt::default());
        };
        Ok(NativeDynamicsBodyAtReceipt {
            present: true,
            body: NativeDynamicsBodyReference { value: handle },
            readout: read_entity(world, entity)?,
        })
    }

    fn read_contact_at(
        &mut self,
        request: NativeDynamicsContactAtRequest,
    ) -> Result<NativeDynamicsContactAtReceipt, CsharpEngineServicesError> {
        let world = self.active_world_mut(request.world.value)?;
        let Some(contact) = world
            .last_contact_receipts
            .get(request.index as usize)
            .copied()
        else {
            return Ok(NativeDynamicsContactAtReceipt::default());
        };
        let body_for = |entity| {
            world
                .bodies
                .iter()
                .find_map(|(handle, current)| (*current == entity).then_some(*handle))
                .unwrap_or(0)
        };
        Ok(NativeDynamicsContactAtReceipt {
            present: true,
            environment: contact.second.is_none(),
            first: NativeDynamicsBodyReference {
                value: body_for(contact.first),
            },
            second: NativeDynamicsBodyReference {
                value: contact.second.map_or(0, body_for),
            },
            impulse: native_vec3(contact.impulse),
            impulse_magnitude: contact.impulse_magnitude,
        })
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

    fn replace_body_with_config(
        &mut self,
        body_handle: NativeDynamicsBodyHandle,
        config: BodyConfig,
    ) -> Result<NativeDynamicsBodyHandle, CsharpEngineServicesError> {
        let (world_handle, old_entity) = self.active_body(body_handle.value)?;
        let entity_value = Self::allocate(&mut self.next_entity, "entity")?;
        let new_handle = Self::allocate(&mut self.next_body, "body")?;
        let new_entity = EntityId::new(entity_value);
        let world = self.active_world_mut(world_handle)?;
        let mut candidate = world.entities.clone();
        let revision = candidate.revision();
        EntityAuthoringService
            .destroy(&mut candidate, revision, old_entity)
            .map_err(|error| {
                CsharpEngineServicesError::new("CSHARP_DYNAMICS_REPLACE", error.to_string())
            })?;
        insert_body(&mut candidate, new_entity, config)?;
        world.entities = candidate;
        world.bodies.remove(&body_handle.value);
        world.bodies.insert(new_handle, new_entity);
        world.last_contacts.remove(&old_entity);
        world
            .last_contact_receipts
            .retain(|contact| contact.first != old_entity && contact.second != Some(old_entity));
        self.bodies.insert(body_handle.value, BodySlot::Tombstoned);
        self.bodies.insert(
            new_handle,
            BodySlot::Active {
                world: world_handle,
                entity: new_entity,
            },
        );
        Ok(NativeDynamicsBodyHandle { value: new_handle })
    }

    fn update_body(
        &mut self,
        request: NativeDynamicsUpdateBodyRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let (world_handle, entity) = self.active_body(request.body.value)?;
        let world = self.active_world_mut(world_handle)?;
        let shape = world
            .entities
            .rigid_body(entity)
            .copied()
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_DYNAMICS_UPDATE",
                    "body lacked dynamics state",
                )
            })?
            .shape;
        let body = body_with_properties(shape, request.properties)?;
        replace_body_component(&mut world.entities, entity, body, "CSHARP_DYNAMICS_UPDATE")?;
        world.last_contacts.remove(&entity);
        world
            .last_contact_receipts
            .retain(|contact| contact.first != entity && contact.second != Some(entity));
        Ok(())
    }

    fn read_world(
        &mut self,
        request: NativeDynamicsWorldReadRequest,
    ) -> Result<NativeDynamicsWorldReadout, CsharpEngineServicesError> {
        let world = self.active_world_mut(request.world.value)?;
        let readout = world.service.readout();
        Ok(NativeDynamicsWorldReadout {
            generation: readout.map_or(0, |value| value.generation),
            entity_revision: world.entities.revision(),
            body_count: u32::try_from(world.bodies.len()).map_err(|_| {
                CsharpEngineServicesError::new("CSHARP_DYNAMICS_WORLD", "body count exceeded u32")
            })?,
            contact_count: u32::try_from(world.last_contact_receipts.len()).map_err(|_| {
                CsharpEngineServicesError::new(
                    "CSHARP_DYNAMICS_WORLD",
                    "contact count exceeded u32",
                )
            })?,
        })
    }

    fn active_world_mut(
        &mut self,
        handle: u64,
    ) -> Result<&mut DynamicsWorld, CsharpEngineServicesError> {
        match self.worlds.get_mut(&handle) {
            Some(WorldSlot::Active(world)) => Ok(world),
            Some(WorldSlot::Tombstoned) => Err(CsharpEngineServicesError::new(
                "CSHARP_DYNAMICS_WORLD",
                "world handle was tombstoned",
            )),
            None => Err(unknown("world", handle)),
        }
    }

    fn active_world(&self, handle: u64) -> Result<&DynamicsWorld, CsharpEngineServicesError> {
        match self.worlds.get(&handle) {
            Some(WorldSlot::Active(world)) => Ok(world),
            Some(WorldSlot::Tombstoned) => Err(CsharpEngineServicesError::new(
                "CSHARP_DYNAMICS_WORLD",
                "world handle was tombstoned",
            )),
            None => Err(unknown("world", handle)),
        }
    }

    fn rebase_snapshot(
        &self,
        handle: u64,
    ) -> Result<
        (
            Option<NativeSpatialSessionHandle>,
            Arc<VoxelCollisionScene>,
            BTreeMap<u64, EntityId>,
            u64,
            u64,
        ),
        CsharpEngineServicesError,
    > {
        let world = self.active_world(handle)?;
        Ok((
            world.bound_spatial_session,
            Arc::clone(&world.scene),
            world.bodies.clone(),
            world.entities.revision(),
            world
                .service
                .readout()
                .map_or(0, |readout| readout.generation),
        ))
    }

    fn validate_body_handles(
        &self,
        world: u64,
        body_members: &BTreeMap<u64, EntityId>,
    ) -> Result<(), CsharpEngineServicesError> {
        for (handle, entity) in body_members {
            match self.bodies.get(handle) {
                Some(BodySlot::Active {
                    world: active_world,
                    entity: active_entity,
                }) if *active_world == world && *active_entity == *entity => {}
                _ => {
                    return Err(CsharpEngineServicesError::new(
                        "CSHARP_DYNAMICS_REBASE",
                        "dynamics body handle mapping was stale",
                    ));
                }
            }
        }
        Ok(())
    }

    fn active_body(&self, handle: u64) -> Result<(u64, EntityId), CsharpEngineServicesError> {
        match self.bodies.get(&handle) {
            Some(BodySlot::Active { world, entity }) => Ok((*world, *entity)),
            Some(BodySlot::Tombstoned) => Err(CsharpEngineServicesError::new(
                "CSHARP_DYNAMICS_BODY",
                "body handle was tombstoned",
            )),
            None => Err(unknown("body", handle)),
        }
    }
}

fn read_entity(
    world: &DynamicsWorld,
    entity: EntityId,
) -> Result<NativeDynamicsReadout, CsharpEngineServicesError> {
    let view = world.entities.view(entity).map_err(|error| {
        CsharpEngineServicesError::new("CSHARP_DYNAMICS_READ", error.to_string())
    })?;
    let transform = view.transform.ok_or_else(|| {
        CsharpEngineServicesError::new("CSHARP_DYNAMICS_READ", "body lacked a transform")
    })?;
    let body = world.entities.rigid_body(entity).ok_or_else(|| {
        CsharpEngineServicesError::new("CSHARP_DYNAMICS_READ", "body lacked dynamics state")
    })?;
    let properties = rigid_body_component_mass_properties(*body);
    let policy = match body.inertia {
        RigidBodyInertiaPolicy::DeriveFromShapeAndMass => {
            NativeDynamicsMassPolicyKind::DeriveFromShapeAndMass
        }
        RigidBodyInertiaPolicy::Explicit { .. } => NativeDynamicsMassPolicyKind::Explicit,
    };
    let contact = world
        .last_contacts
        .get(&entity)
        .copied()
        .unwrap_or_default();
    Ok(NativeDynamicsReadout {
        transform: native_transform(transform.transform()),
        linear_velocity: native_vec3(body.linear_velocity),
        angular_velocity: native_vec3(body.angular_velocity),
        sleeping: body.sleeping,
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
    })
}

fn validate_rebase_receipt(
    receipt: &NativeWorldOriginCommitReceipt,
) -> Result<(), CsharpEngineServicesError> {
    let expected_after = receipt.revision_before.checked_add(1).ok_or_else(|| {
        CsharpEngineServicesError::new(
            "CSHARP_DYNAMICS_REBASE",
            "world-origin receipt revision was exhausted",
        )
    })?;
    if receipt.revision_after != expected_after {
        return Err(CsharpEngineServicesError::new(
            "CSHARP_DYNAMICS_REBASE",
            "world-origin receipt did not advance exactly one revision",
        ));
    }
    if !receipt.local_envelope.is_finite() || receipt.local_envelope <= 0.0 {
        return Err(CsharpEngineServicesError::new(
            "CSHARP_DYNAMICS_REBASE",
            "world-origin receipt local envelope was invalid",
        ));
    }
    Ok(())
}

fn validate_scene_before(
    scene: &VoxelCollisionScene,
    receipt: &NativeWorldOriginCommitReceipt,
) -> Result<(), CsharpEngineServicesError> {
    validate_scene(
        scene,
        WorldOrigin::new([
            receipt.origin_before_cell_x,
            receipt.origin_before_cell_y,
            receipt.origin_before_cell_z,
        ]),
        receipt.revision_before,
        receipt,
        "before",
    )
}

fn validate_scene_after(
    scene: &VoxelCollisionScene,
    receipt: &NativeWorldOriginCommitReceipt,
) -> Result<(), CsharpEngineServicesError> {
    validate_scene(
        scene,
        WorldOrigin::new([
            receipt.origin_after_cell_x,
            receipt.origin_after_cell_y,
            receipt.origin_after_cell_z,
        ]),
        receipt.revision_after,
        receipt,
        "after",
    )
}

fn validate_scene(
    scene: &VoxelCollisionScene,
    expected_origin: WorldOrigin,
    expected_rebase_revision: u64,
    receipt: &NativeWorldOriginCommitReceipt,
    phase: &'static str,
) -> Result<(), CsharpEngineServicesError> {
    if scene.world_origin() != expected_origin
        || scene.rebase_revision() != expected_rebase_revision
        || scene.source_revision().raw() != receipt.voxel_source_revision
        || scene.static_mesh_collision_revision() != receipt.static_mesh_revision
    {
        return Err(CsharpEngineServicesError::new(
            "CSHARP_DYNAMICS_REBASE",
            format!("world-origin receipt did not match the {phase} collision scene"),
        ));
    }
    Ok(())
}

fn rebase_body_replacement(
    state: &EntityState,
    entity: EntityId,
    receipt: &NativeWorldOriginCommitReceipt,
) -> Result<RigidBodyStateReplacement, CsharpEngineServicesError> {
    if state.lifecycle(entity) != Some(EntityLifecycle::Active) {
        return Err(CsharpEngineServicesError::new(
            "CSHARP_DYNAMICS_REBASE",
            "dynamics body was not active",
        ));
    }
    if state.transform_parent(entity).is_some() {
        return Err(CsharpEngineServicesError::new(
            "CSHARP_DYNAMICS_REBASE",
            "dynamics body was parented",
        ));
    }
    let transform = state.transform(entity).copied().ok_or_else(|| {
        CsharpEngineServicesError::new("CSHARP_DYNAMICS_REBASE", "dynamics body lacked a transform")
    })?;
    if transform.scale != Vec3::ONE {
        return Err(CsharpEngineServicesError::new(
            "CSHARP_DYNAMICS_REBASE",
            "dynamics body transform scale was not unit",
        ));
    }
    let rigid_body = state.rigid_body(entity).copied().ok_or_else(|| {
        CsharpEngineServicesError::new("CSHARP_DYNAMICS_REBASE", "dynamics body lacked rigid state")
    })?;
    let global = GlobalPosition::from_local(
        WorldOrigin::new([
            receipt.origin_before_cell_x,
            receipt.origin_before_cell_y,
            receipt.origin_before_cell_z,
        ]),
        transform.translation.to_array(),
    )
    .map_err(|error| CsharpEngineServicesError::new("CSHARP_DYNAMICS_REBASE", error.to_string()))?;
    let local = global
        .local(
            WorldOrigin::new([
                receipt.origin_after_cell_x,
                receipt.origin_after_cell_y,
                receipt.origin_after_cell_z,
            ]),
            receipt.local_envelope,
        )
        .map_err(|error| {
            CsharpEngineServicesError::new("CSHARP_DYNAMICS_REBASE", error.to_string())
        })?;
    Ok(RigidBodyStateReplacement {
        entity,
        expected_transform_revision: state
            .component_revision::<TransformComponent>(entity)
            .map_err(|error| {
                CsharpEngineServicesError::new("CSHARP_DYNAMICS_REBASE", error.to_string())
            })?,
        expected_rigid_body_revision: state
            .component_revision::<RigidBodyComponent>(entity)
            .map_err(|error| {
                CsharpEngineServicesError::new("CSHARP_DYNAMICS_REBASE", error.to_string())
            })?,
        transform: TransformComponent::from_transform(EntityTransform {
            translation: Vec3::new(local[0], local[1], local[2]),
            rotation: transform.rotation,
            scale: transform.scale,
        }),
        rigid_body,
    })
}

fn unknown(kind: &str, value: u64) -> CsharpEngineServicesError {
    CsharpEngineServicesError::new(
        "CSHARP_DYNAMICS_HANDLE",
        format!("unknown {kind} handle {value}"),
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
    body_config(
        transform,
        shape,
        value.mass,
        value.mass_policy,
        value.axis_locks,
        value.gravity_scale,
    )
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
    entity_state::validate_rigid_body(&body)
        .map_err(|error| CsharpEngineServicesError::new("CSHARP_DYNAMICS_BODY", error.code()))?;
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
    entity_state::validate_rigid_body(&body)
        .map_err(|error| CsharpEngineServicesError::new("CSHARP_DYNAMICS_BODY", error.code()))?;
    Ok(body)
}

fn replace_body_component(
    entities: &mut EntityState,
    entity: EntityId,
    body: RigidBodyComponent,
    code: &'static str,
) -> Result<(), CsharpEngineServicesError> {
    let transform = entities
        .view(entity)
        .map_err(|error| CsharpEngineServicesError::new(code, error.to_string()))?
        .transform
        .ok_or_else(|| CsharpEngineServicesError::new(code, "body lacked a transform"))?;
    let replacement = RigidBodyStateReplacement {
        entity,
        expected_transform_revision: entities
            .component_revision::<TransformComponent>(entity)
            .map_err(|error| CsharpEngineServicesError::new(code, error.to_string()))?,
        expected_rigid_body_revision: entities
            .component_revision::<RigidBodyComponent>(entity)
            .map_err(|error| CsharpEngineServicesError::new(code, error.to_string()))?,
        transform,
        rigid_body: body,
    };
    replace_rigid_body_states(entities, vec![replacement])
        .map(|_| ())
        .map_err(|error| CsharpEngineServicesError::new(code, error.code()))
}

fn contacts_by_body(
    receipt: &engine_spatial::RigidBodyStepReceipt,
) -> BTreeMap<EntityId, BodyContactSummary> {
    let mut contacts = BTreeMap::new();
    for contact in &receipt.contacts {
        record_contact(
            &mut contacts,
            contact.first,
            contact.second.is_none(),
            contact.impulse,
            contact.impulse_magnitude,
        );
        if let Some(second) = contact.second {
            record_contact(
                &mut contacts,
                second,
                false,
                Vec3::new(-contact.impulse.x, -contact.impulse.y, -contact.impulse.z),
                contact.impulse_magnitude,
            );
        }
    }
    contacts
}

fn record_contact(
    contacts: &mut BTreeMap<EntityId, BodyContactSummary>,
    entity: EntityId,
    environment: bool,
    impulse: Vec3,
    impulse_magnitude: f32,
) {
    let entry = contacts.entry(entity).or_default();
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

fn insert_body(
    state: &mut EntityState,
    entity: EntityId,
    config: BodyConfig,
) -> Result<(), CsharpEngineServicesError> {
    EntityAuthoringService
        .admit(
            state,
            state.revision(),
            [
                EntityDefinition::new(entity, format!("dynamics-body-{}", entity.raw()))
                    .with_full_transform(config.transform),
            ],
        )
        .map_err(|error| {
            CsharpEngineServicesError::new("CSHARP_DYNAMICS_BODY", error.to_string())
        })?;
    let revision = state
        .component_revision::<RigidBodyComponent>(entity)
        .map_err(|error| {
            CsharpEngineServicesError::new("CSHARP_DYNAMICS_BODY", error.to_string())
        })?;
    EntityAuthoringService
        .attach_component(state, revision, entity, config.body)
        .map_err(|error| {
            CsharpEngineServicesError::new("CSHARP_DYNAMICS_BODY", error.to_string())
        })?;
    Ok(())
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
) -> i32 {
    if context.is_null() || handle.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() };
    match bridge.create_world(config) {
        Ok(value) => {
            unsafe { *handle = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn destroy_world(context: *mut c_void, handle: NativeDynamicsWorldHandle) -> i32 {
    if context.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }.destroy_world(handle) {
        Ok(()) => ABI_OK,
        Err(_) => 0,
    }
}

unsafe extern "C" fn create_body(
    context: *mut c_void,
    request: *const NativeDynamicsCreateBodyRequest,
    handle: *mut NativeDynamicsBodyHandle,
) -> i32 {
    if context.is_null() || request.is_null() || handle.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }.create_body(unsafe { &*request })
    {
        Ok(value) => {
            unsafe { *handle = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn create_sphere_body(
    context: *mut c_void,
    request: *const NativeDynamicsCreateSphereBodyRequest,
    handle: *mut NativeDynamicsBodyHandle,
) -> i32 {
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
        Err(_) => 0,
    }
}

unsafe extern "C" fn create_cuboid_body(
    context: *mut c_void,
    request: *const NativeDynamicsCreateCuboidBodyRequest,
    handle: *mut NativeDynamicsBodyHandle,
) -> i32 {
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
        Err(_) => 0,
    }
}

unsafe extern "C" fn create_sphere_body_with_properties(
    context: *mut c_void,
    request: *const NativeDynamicsCreateSphereBodyPropertiesRequest,
    handle: *mut NativeDynamicsBodyHandle,
) -> i32 {
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
        Err(_) => 0,
    }
}

unsafe extern "C" fn create_capsule_body(
    context: *mut c_void,
    request: *const NativeDynamicsCreateCapsuleBodyRequest,
    handle: *mut NativeDynamicsBodyHandle,
) -> i32 {
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
        Err(_) => 0,
    }
}

unsafe extern "C" fn bind_world_collision(
    context: *mut c_void,
    request: NativeDynamicsWorldCollisionBindingRequest,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }.bind_world_collision(request) {
        Ok(()) => ABI_OK,
        Err(_) => 0,
    }
}

unsafe extern "C" fn rebase_world_origin(
    context: *mut c_void,
    request: NativeDynamicsRebaseWorldOriginRequest,
    receipt: *mut NativeDynamicsRebaseWorldOriginReceipt,
) -> i32 {
    if context.is_null() || receipt.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }.rebase_world_origin(request) {
        Ok(value) => {
            unsafe { *receipt = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn destroy_body(context: *mut c_void, handle: NativeDynamicsBodyHandle) -> i32 {
    if context.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }.destroy_body(handle) {
        Ok(()) => ABI_OK,
        Err(_) => 0,
    }
}

unsafe extern "C" fn step(
    context: *mut c_void,
    request: *const NativeDynamicsStepRequest,
    receipt: *mut NativeDynamicsStepReceipt,
) -> i32 {
    if context.is_null() || request.is_null() || receipt.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }.step(unsafe { &*request }) {
        Ok(value) => {
            unsafe { *receipt = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn read(
    context: *mut c_void,
    request: NativeDynamicsReadRequest,
    readout: *mut NativeDynamicsReadout,
) -> i32 {
    if context.is_null() || readout.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }.read(request) {
        Ok(value) => {
            unsafe { *readout = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn reset(context: *mut c_void, request: NativeDynamicsResetRequest) -> i32 {
    if context.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }.reset(request) {
        Ok(()) => ABI_OK,
        Err(_) => 0,
    }
}

unsafe extern "C" fn update_body(
    context: *mut c_void,
    request: NativeDynamicsUpdateBodyRequest,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }.update_body(request) {
        Ok(()) => ABI_OK,
        Err(_) => 0,
    }
}

unsafe extern "C" fn read_world(
    context: *mut c_void,
    request: NativeDynamicsWorldReadRequest,
    readout: *mut NativeDynamicsWorldReadout,
) -> i32 {
    if context.is_null() || readout.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }.read_world(request) {
        Ok(value) => {
            unsafe { *readout = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn read_body_at(
    context: *mut c_void,
    request: NativeDynamicsBodyAtRequest,
    receipt: *mut NativeDynamicsBodyAtReceipt,
) -> i32 {
    if context.is_null() || receipt.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }.read_body_at(request) {
        Ok(value) => {
            unsafe { *receipt = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn read_contact_at(
    context: *mut c_void,
    request: NativeDynamicsContactAtRequest,
    receipt: *mut NativeDynamicsContactAtReceipt,
) -> i32 {
    if context.is_null() || receipt.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }.read_contact_at(request) {
        Ok(value) => {
            unsafe { *receipt = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn replace_body(
    context: *mut c_void,
    request: NativeDynamicsReplaceBodyRequest,
    handle: *mut NativeDynamicsBodyHandle,
) -> i32 {
    if context.is_null() || handle.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }.replace_body(request) {
        Ok(value) => {
            unsafe { *handle = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn replace_cuboid_body(
    context: *mut c_void,
    request: NativeDynamicsReplaceCuboidBodyRequest,
    handle: *mut NativeDynamicsBodyHandle,
) -> i32 {
    if context.is_null() || handle.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }.replace_cuboid_body(request) {
        Ok(value) => {
            unsafe { *handle = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn replace_sphere_body(
    context: *mut c_void,
    request: NativeDynamicsReplaceSphereBodyRequest,
    handle: *mut NativeDynamicsBodyHandle,
) -> i32 {
    if context.is_null() || handle.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }.replace_sphere_body(request) {
        Ok(value) => {
            unsafe { *handle = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn replace_capsule_body(
    context: *mut c_void,
    request: NativeDynamicsReplaceCapsuleBodyRequest,
    handle: *mut NativeDynamicsBodyHandle,
) -> i32 {
    if context.is_null() || handle.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }.replace_capsule_body(request) {
        Ok(value) => {
            unsafe { *handle = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}

pub(crate) fn api(bridge: &mut RuntimeDynamicsBridge) -> NativeDynamicsApi {
    NativeDynamicsApi {
        context: (bridge as *mut RuntimeDynamicsBridge).cast(),
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
        read,
        reset,
        update_body,
        read_world,
        read_body_at,
        read_contact_at,
        replace_body,
        replace_cuboid_body,
        replace_sphere_body,
        replace_capsule_body,
    }
}

#[cfg(test)]
mod tests {
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
            mass: 2.0,
            mass_policy: NativeDynamicsMassPolicy::default(),
            axis_locks: NativeAxisLocks::default(),
            gravity_scale: 0.0,
        }
    }

    #[test]
    fn bridge_preserves_step_atomicity_replacement_tombstones_and_disposal_orders() {
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
        assert!(bridge
            .step(&NativeDynamicsStepRequest {
                world,
                step_seconds: ONE_SIXTIETH_SECOND,
                steps: 256,
                actions: std::ptr::null(),
                actions_len: 0
            })
            .is_err());
        let unchanged = bridge.read(NativeDynamicsReadRequest { body }).unwrap();
        assert_eq!(unchanged.sleeping, driven.sleeping);
        assert_eq!(
            [
                unchanged.transform.translation.x,
                unchanged.transform.translation.y,
                unchanged.transform.translation.z,
                unchanged.transform.rotation.x,
                unchanged.transform.rotation.y,
                unchanged.transform.rotation.z,
                unchanged.transform.rotation.w,
                unchanged.linear_velocity.x,
                unchanged.linear_velocity.y,
                unchanged.linear_velocity.z,
                unchanged.angular_velocity.x,
                unchanged.angular_velocity.y,
                unchanged.angular_velocity.z,
                unchanged.mass_properties.mass,
                unchanged.mass_properties.principal_inertia.x,
                unchanged.mass_properties.principal_inertia.y,
                unchanged.mass_properties.principal_inertia.z
            ],
            [
                driven.transform.translation.x,
                driven.transform.translation.y,
                driven.transform.translation.z,
                driven.transform.rotation.x,
                driven.transform.rotation.y,
                driven.transform.rotation.z,
                driven.transform.rotation.w,
                driven.linear_velocity.x,
                driven.linear_velocity.y,
                driven.linear_velocity.z,
                driven.angular_velocity.x,
                driven.angular_velocity.y,
                driven.angular_velocity.z,
                driven.mass_properties.mass,
                driven.mass_properties.principal_inertia.x,
                driven.mass_properties.principal_inertia.y,
                driven.mass_properties.principal_inertia.z
            ],
        );
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
        assert_eq!(
            bridge
                .read_world(NativeDynamicsWorldReadRequest { world })
                .unwrap()
                .body_count,
            1
        );
        let first = bridge
            .read_body_at(NativeDynamicsBodyAtRequest { world, index: 0 })
            .unwrap();
        assert!(
            first.present
                && first.body.value == cuboid.value
                && first.readout.mass_properties.available
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
    fn bind_world_collision_uses_spatial_projection_snapshots_atomically() {
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
                        reserved: 0,
                    },
                    &mut session,
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
            unsafe { (spatial_api.replace_collision)(spatial_api.context, &request, &mut replace) },
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
        let indexed = bridge
            .read_contact_at(NativeDynamicsContactAtRequest { world, index: 0 })
            .unwrap();
        assert!(
            indexed.present
                && indexed.environment
                && indexed.first.value == body.value
                && indexed.second.value == 0
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
    fn rebase_world_origin_updates_bound_dynamics_atomically() {
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
                        reserved: 0,
                    },
                    &mut session,
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

        let mut origin = NativeWorldOriginReadout::default();
        assert_eq!(
            unsafe {
                (world_origin_api.read)(
                    world_origin_api.context,
                    NativeWorldOriginReadRequest { session },
                    &mut origin,
                )
            },
            ABI_OK
        );
        let prepare = NativeWorldOriginPrepareRequest {
            session,
            expected_origin_revision: origin.revision,
            expected_voxel_source_revision: origin.voxel_source_revision,
            expected_static_mesh_revision: origin.static_mesh_revision,
            target_cell_x: 5,
            target_cell_y: 0,
            target_cell_z: 0,
            entities: std::ptr::null(),
            entities_len: 0,
        };
        let mut prepared = NativeWorldOriginPreparedHandle::default();
        assert_eq!(
            unsafe {
                (world_origin_api.prepare)(world_origin_api.context, &prepare, &mut prepared)
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
                )
            },
            ABI_OK
        );

        let before_world = bridge
            .read_world(NativeDynamicsWorldReadRequest { world })
            .unwrap();
        let before_body = bridge.read(NativeDynamicsReadRequest { body }).unwrap();
        let before_contact = bridge
            .read_contact_at(NativeDynamicsContactAtRequest { world, index: 0 })
            .unwrap();
        let request = NativeDynamicsRebaseWorldOriginRequest {
            world,
            spatial_session: session,
            receipt,
            expected_entity_revision: before_world.entity_revision,
            expected_solver_generation: before_world.generation,
        };
        let rebase = bridge.rebase_world_origin(request).unwrap();
        let after_world = bridge
            .read_world(NativeDynamicsWorldReadRequest { world })
            .unwrap();
        let after_body = bridge.read(NativeDynamicsReadRequest { body }).unwrap();
        let after_contact = bridge
            .read_contact_at(NativeDynamicsContactAtRequest { world, index: 0 })
            .unwrap();
        assert_eq!(after_world.generation, before_world.generation);
        assert_eq!(
            after_world.entity_revision,
            before_world.entity_revision + 1
        );
        assert_eq!(rebase.entity_revision_before, before_world.entity_revision);
        assert_eq!(rebase.entity_revision_after, after_world.entity_revision);
        assert_eq!(rebase.solver_generation, after_world.generation);
        assert_eq!(rebase.body_count, after_world.body_count);
        assert_eq!(rebase.contact_count, after_world.contact_count);
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
        assert!(after_contact.present && after_contact.environment);
        assert_eq!(after_contact.first.value, before_contact.first.value);
        assert_eq!(
            after_contact.impulse_magnitude,
            before_contact.impulse_magnitude
        );

        let preserved = |bridge: &mut RuntimeDynamicsBridge| {
            let world_readout = bridge
                .read_world(NativeDynamicsWorldReadRequest { world })
                .unwrap();
            let body_readout = bridge.read(NativeDynamicsReadRequest { body }).unwrap();
            let contact = bridge
                .read_contact_at(NativeDynamicsContactAtRequest { world, index: 0 })
                .unwrap();
            (
                world_readout.entity_revision,
                world_readout.generation,
                body_readout.transform.translation.x,
                body_readout.linear_velocity.x,
                body_readout.angular_velocity.y,
                body_readout.sleeping,
                contact.present,
                contact.first.value,
                contact.impulse_magnitude,
            )
        };
        let expected = preserved(&mut bridge);
        let current = NativeDynamicsRebaseWorldOriginRequest {
            expected_entity_revision: after_world.entity_revision,
            expected_solver_generation: after_world.generation,
            ..request
        };
        assert!(bridge
            .rebase_world_origin(NativeDynamicsRebaseWorldOriginRequest {
                spatial_session: NativeSpatialSessionHandle { value: u64::MAX },
                ..current
            })
            .is_err());
        assert_eq!(preserved(&mut bridge), expected);
        assert!(bridge
            .rebase_world_origin(NativeDynamicsRebaseWorldOriginRequest {
                receipt: NativeWorldOriginCommitReceipt {
                    revision_after: receipt.revision_before,
                    ..receipt
                },
                ..current
            })
            .is_err());
        assert_eq!(preserved(&mut bridge), expected);
        assert!(bridge
            .rebase_world_origin(NativeDynamicsRebaseWorldOriginRequest {
                expected_entity_revision: current.expected_entity_revision - 1,
                ..current
            })
            .is_err());
        assert_eq!(preserved(&mut bridge), expected);
        assert!(bridge
            .rebase_world_origin(NativeDynamicsRebaseWorldOriginRequest {
                expected_solver_generation: current.expected_solver_generation + 1,
                ..current
            })
            .is_err());
        assert_eq!(preserved(&mut bridge), expected);
    }
}
