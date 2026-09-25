use super::*;

fn error(message: &str) -> CsharpEngineServicesError {
    CsharpEngineServicesError::new("CSHARP_DYNAMICS_ANCHOR", message)
}

impl RuntimeDynamicsBridge {
    pub(super) fn observe_anchor(
        &self,
        request: NativeDynamicsObserveAnchorRequest,
    ) -> Result<NativeDynamicsAnchorObservation, CsharpEngineServicesError> {
        let (world, _) = self.active_body(request.body.value)?;
        if world != request.world.value {
            return Err(error("anchor belonged to another world"));
        }
        self.refresh_anchor(NativeDynamicsRefreshAnchorRequest {
            world: request.world,
            anchor: NativeDynamicsAnchorObservation {
                world_identity: world,
                body: NativeDynamicsBodyReference {
                    value: request.body.value,
                },
                local_anchor: request.local_anchor,
                ..Default::default()
            },
        })
    }

    pub(super) fn refresh_anchor(
        &self,
        request: NativeDynamicsRefreshAnchorRequest,
    ) -> Result<NativeDynamicsAnchorObservation, CsharpEngineServicesError> {
        let world = self.active_world(request.world.value)?;
        if request.anchor.world_identity != request.world.value {
            return Err(error("anchor belonged to another world"));
        }
        let local = native_vec3_value(request.anchor.local_anchor);
        if !finite_vec3(local) {
            return Err(error("nonfinite local anchor"));
        }
        let mut observation = NativeDynamicsAnchorObservation {
            world_identity: request.world.value,
            body: request.anchor.body,
            local_anchor: request.anchor.local_anchor,
            entity_revision: world.entities.revision(),
            solver_generation: world
                .service
                .readout()
                .map_or(0, |readout| readout.generation),
            ..Default::default()
        };
        let Ok((owner, entity)) = self.active_body(request.anchor.body.value) else {
            return Ok(observation);
        };
        if owner != request.world.value {
            return Err(error("anchor belonged to another world"));
        }
        let body = world
            .entities
            .rigid_body(entity)
            .ok_or_else(|| error("missing anchor body"))?;
        if !body.enabled {
            return Ok(observation);
        }
        let response = engine_spatial::observe_rigid_body_anchor(&world.entities, entity, local)
            .map_err(|failure| error(failure.code()))?;
        observation.valid = true;
        observation.point = native_vec3(response.point);
        observation.point_velocity = native_vec3(response.point_velocity);
        observation.center_of_mass = native_vec3(response.center_of_mass);
        observation.response_x = native_vec3(response.response[0]);
        observation.response_y = native_vec3(response.response[1]);
        observation.response_z = native_vec3(response.response[2]);
        Ok(observation)
    }

    pub(super) fn step_with_reactions(
        &mut self,
        request: &NativeDynamicsStepWithReactionsRequest,
    ) -> Result<NativeDynamicsStepReceipt, CsharpEngineServicesError> {
        if request.actions_len.saturating_add(request.reactions_len)
            > engine_spatial::MAX_DYNAMICS_ACTIONS
        {
            return Err(error("reaction/action work budget exceeded"));
        }
        let actions =
            unsafe { borrowed_slice(request.actions, request.actions_len, "dynamics actions") }?;
        let reactions = unsafe {
            borrowed_slice(request.reactions, request.reactions_len, "anchor reactions")
        }?;
        let mut active = self.validate_step_actions(request.world.value, request.steps, actions)?;
        let world = self.active_world(request.world.value)?;
        let generation = world
            .service
            .readout()
            .map_or(0, |readout| readout.generation);
        let mut proposals = BTreeSet::new();
        for reaction in reactions.iter().filter(|reaction| reaction.present) {
            if !proposals.insert((reaction.source_identity, reaction.source_generation)) {
                return Err(error("duplicate anchor reaction in one update"));
            }
            let observation = reaction.anchor;
            if !observation.valid
                || observation.world_identity != request.world.value
                || observation.entity_revision != world.entities.revision()
                || observation.solver_generation != generation
            {
                return Err(error("stale or invalid anchor reaction"));
            }
            let (owner, entity) = self.active_body(observation.body.value)?;
            if owner != request.world.value {
                return Err(error("reaction belonged to another world"));
            }
            let body = world
                .entities
                .rigid_body(entity)
                .ok_or_else(|| error("missing reaction body"))?;
            if !body.enabled {
                return Err(error("reaction body disabled"));
            }
            let impulse = native_vec3_value(reaction.impulse);
            let point = native_vec3_value(observation.point);
            if !finite_vec3(impulse)
                || !finite_vec3(point)
                || !reaction.maximum_impulse.is_finite()
                || reaction.maximum_impulse < 0.0
                || impulse.length() > reaction.maximum_impulse * (1.0 + 1e-5) + 1e-6
            {
                return Err(error("reaction exceeded the character impulse bound"));
            }
            let torque_impulse =
                (point - native_vec3_value(observation.center_of_mass)).cross(impulse);
            if !finite_vec3(torque_impulse) {
                return Err(error("reaction torque out of range"));
            }
            active.push(RigidBodyAction {
                entity,
                impulse,
                torque_impulse,
                force: Vec3::ZERO,
                torque: Vec3::ZERO,
                wake: true,
            });
        }
        self.execute_step(
            request.world.value,
            request.step_seconds,
            request.steps,
            active,
        )
    }
}

pub(super) unsafe extern "C" fn observe_anchor(
    context: *mut c_void,
    request: NativeDynamicsObserveAnchorRequest,
    output: *mut NativeDynamicsAnchorObservation,
) -> i32 {
    if context.is_null() || output.is_null() {
        return 0;
    }
    match unsafe { &*context.cast::<RuntimeDynamicsBridge>() }.observe_anchor(request) {
        Ok(value) => {
            unsafe { *output = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}
pub(super) unsafe extern "C" fn refresh_anchor(
    context: *mut c_void,
    request: NativeDynamicsRefreshAnchorRequest,
    output: *mut NativeDynamicsAnchorObservation,
) -> i32 {
    if context.is_null() || output.is_null() {
        return 0;
    }
    match unsafe { &*context.cast::<RuntimeDynamicsBridge>() }.refresh_anchor(request) {
        Ok(value) => {
            unsafe { *output = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}
pub(super) unsafe extern "C" fn step_with_reactions(
    context: *mut c_void,
    request: *const NativeDynamicsStepWithReactionsRequest,
    output: *mut NativeDynamicsStepReceipt,
) -> i32 {
    if context.is_null() || request.is_null() || output.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }
        .step_with_reactions(unsafe { &*request })
    {
        Ok(value) => {
            unsafe { *output = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}
