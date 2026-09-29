use super::*;

fn error(message: &str) -> CsharpEngineServicesError {
    CsharpEngineServicesError::new("CSHARP_DYNAMICS_ANCHOR", message)
}

impl RuntimeDynamicsBridge {
    pub(super) fn observe_anchor(
        &self,
        request: NativeDynamicsObserveAnchorRequest,
    ) -> Result<NativeDynamicsAnchorObservation, CsharpEngineServicesError> {
        self.world_body(
            request.world.value,
            request.body.value,
            "CSHARP_DYNAMICS_ANCHOR",
        )?;
        let local = native_vec3_value(request.local_anchor);
        if !finite_vec3(local) {
            return Err(error("nonfinite local anchor"));
        }
        let mut observation = NativeDynamicsAnchorObservation {
            body: NativeDynamicsBodyReference {
                value: request.body.value,
            },
            local_anchor: request.local_anchor,
            ..Default::default()
        };
        let Some(response) = self
            .active_world(request.world.value)?
            .solver
            .observe_anchor(DynamicsBodyId(request.body.value), vec3_f64(local))
        else {
            return Ok(observation);
        };
        observation.valid = true;
        observation.point = native_vec3(vec3_f32(response.point));
        observation.point_velocity = native_vec3(vec3_f32(response.point_velocity));
        observation.center_of_mass = native_vec3(vec3_f32(response.center_of_mass));
        observation.response_x = native_vec3(vec3_f32(response.response[0]));
        observation.response_y = native_vec3(vec3_f32(response.response[1]));
        observation.response_z = native_vec3(vec3_f32(response.response[2]));
        Ok(observation)
    }

    /// Step with the product's actions plus each reaction as an impulse at its
    /// observed anchor point.
    pub(super) fn step_with_reactions(
        &mut self,
        request: &NativeDynamicsStepWithReactionsRequest,
    ) -> Result<NativeDynamicsStepReceipt, CsharpEngineServicesError> {
        let actions =
            unsafe { borrowed_slice(request.actions, request.actions_len, "dynamics actions") }?;
        let reactions = unsafe {
            borrowed_slice(request.reactions, request.reactions_len, "anchor reactions")
        }?;
        let mut actions = self.step_actions(request.world.value, actions)?;
        for reaction in reactions.iter().filter(|reaction| reaction.present) {
            let body = reaction.anchor.body.value;
            self.world_body(request.world.value, body, "CSHARP_DYNAMICS_ANCHOR")?;
            let impulse = native_vec3_value(reaction.impulse);
            let lever = native_vec3_value(reaction.anchor.point)
                - native_vec3_value(reaction.anchor.center_of_mass);
            actions.push(DynamicsAction {
                body: DynamicsBodyId(body),
                force: [0.0; 3],
                torque: [0.0; 3],
                impulse: vec3_f64(impulse),
                torque_impulse: vec3_f64(lever.cross(impulse)),
                wake: true,
            });
        }
        self.execute_step(
            request.world.value,
            request.step_seconds,
            request.steps,
            &actions,
        )
    }
}

pub(super) unsafe extern "C" fn observe_anchor(
    context: *mut c_void,
    request: NativeDynamicsObserveAnchorRequest,
    output: *mut NativeDynamicsAnchorObservation,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    clear_receipt(operation_error);
    if context.is_null() || output.is_null() {
        return 0;
    }
    match unsafe { &*context.cast::<RuntimeDynamicsBridge>() }.observe_anchor(request) {
        Ok(value) => {
            unsafe { *output = value };
            ABI_OK
        }
        Err(error) => refuse(context, &error, operation_error),
    }
}
pub(super) unsafe extern "C" fn step_with_reactions(
    context: *mut c_void,
    request: *const NativeDynamicsStepWithReactionsRequest,
    output: *mut NativeDynamicsStepReceipt,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    clear_receipt(operation_error);
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
        Err(error) => refuse(context, &error, operation_error),
    }
}
