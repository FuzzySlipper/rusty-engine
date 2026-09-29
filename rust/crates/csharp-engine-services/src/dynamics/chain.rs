use super::*;

/// A chain's bead bodies and links belong to the world, never to the product.
pub(super) struct DynamicsChain {
    pub(super) links: Vec<u64>,
    pub(super) bodies: Vec<u64>,
    pub(super) anchor: DynamicsTetherEndpoint,
}

pub(super) fn error(code: &'static str, message: &str) -> CsharpEngineServicesError {
    CsharpEngineServicesError::new(code, message)
}

fn endpoint_point(world: &DynamicsWorld, endpoint: DynamicsTetherEndpoint) -> Vec3 {
    match endpoint {
        DynamicsTetherEndpoint::Fixed(point) => vec3_f32(point),
        DynamicsTetherEndpoint::Body { body, local_anchor } => {
            output_transform(&world.body_output(body.0)).transform_point(vec3_f32(local_anchor))
        }
    }
}

impl DynamicsWorld {
    /// Remove a chain's beads, and with them its links. Returns the bead handles.
    pub(super) fn remove_chain_bodies(&mut self, chain: &DynamicsChain) -> Vec<u64> {
        for bead in &chain.bodies {
            self.solver.remove_body(DynamicsBodyId(*bead));
            self.bodies.remove(bead);
            self.contacts.remove(bead);
        }
        chain.bodies.clone()
    }
}

impl RuntimeDynamicsBridge {
    pub(super) fn set_chain_length(
        &mut self,
        request: NativeDynamicsChainLengthRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        if !request.target_length.is_finite()
            || request.target_length <= 0.0
            || !request.reel_speed.is_finite()
            || request.reel_speed < 0.0
        {
            return Err(error(
                "invalid-dynamics-chain-length",
                "invalid chain length control",
            ));
        }
        let world = self.active_world_mut(request.world.value)?;
        let chain = world
            .chains
            .get(&request.id)
            .ok_or_else(|| error("dynamics-chain-not-found", "missing chain"))?;
        let links = chain.links.len() as f64;
        for link in &chain.links {
            let mut definition = world.solver.tether(*link).expect("chain link");
            definition.target_length = f64::from(request.target_length) / links;
            definition.reel_speed = f64::from(request.reel_speed) / links;
            world
                .solver
                .set_tether(definition)
                .map_err(|failure| error(failure.code(), failure.code()))?;
        }
        Ok(())
    }

    pub(super) fn create_fixed_chain(
        &mut self,
        request: NativeDynamicsFixedChainRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        self.create_chain(
            request.world.value,
            DynamicsTetherEndpoint::Fixed(vec3_f64(native_vec3_value(request.anchor))),
            request.end,
            request.config,
        )
    }

    pub(super) fn create_body_chain(
        &mut self,
        request: NativeDynamicsBodyChainRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let anchor =
            self.tether_endpoint(request.world.value, request.body, request.local_anchor)?;
        self.create_chain(request.world.value, anchor, request.end, request.config)
    }

    fn create_chain(
        &mut self,
        world_handle: u64,
        anchor: DynamicsTetherEndpoint,
        end: NativeVec3,
        config: NativeDynamicsChainConfig,
    ) -> Result<(), CsharpEngineServicesError> {
        let world = self.active_world(world_handle)?;
        if world.chains.contains_key(&config.id) {
            return Err(error(
                "dynamics-chain-duplicate-id",
                "chain identity already exists; remove before recreation",
            ));
        }
        if config.bead_count == 0
            || !config.link_length.is_finite()
            || config.link_length <= 0.0
            || !config.radius.is_finite()
            || config.radius <= 0.0
        {
            return Err(error(
                "invalid-dynamics-chain-configuration",
                "invalid chain configuration",
            ));
        }
        let start = endpoint_point(world, anchor);
        let end = native_vec3_value(end);
        let separation = (end - start).length();
        if !finite_vec3(end)
            || !separation.is_finite()
            || separation > config.link_length * config.bead_count as f32 + 0.001
        {
            return Err(error(
                "dynamics-chain-invalid-anchor",
                "chain endpoints are nonfinite or out of reach",
            ));
        }
        let mut chain = DynamicsChain {
            links: Vec::new(),
            bodies: Vec::new(),
            anchor,
        };
        let built = self.build_chain(world_handle, &mut chain, start, end, config);
        let world = self.active_world_mut(world_handle)?;
        if let Err(failure) = built {
            let released = world.remove_chain_bodies(&chain);
            for handle in released {
                self.bodies.insert(handle, BodySlot::Tombstoned);
            }
            return Err(failure);
        }
        world.invalidated_chains.remove(&config.id);
        world.chains.insert(config.id, chain);
        Ok(())
    }

    fn build_chain(
        &mut self,
        world_handle: u64,
        chain: &mut DynamicsChain,
        start: Vec3,
        end: Vec3,
        config: NativeDynamicsChainConfig,
    ) -> Result<(), CsharpEngineServicesError> {
        // Link identities count down from the top of the namespace, clear of
        // product tether identities in ordinary use.
        let mut next_link = u64::MAX;
        let mut previous = chain.anchor;
        for index in 0..config.bead_count {
            let position = start + (end - start) * ((index + 1) as f32 / config.bead_count as f32);
            let bead = body_config_with_properties(
                EntityTransform {
                    translation: position,
                    rotation: Quat::IDENTITY,
                    scale: Vec3::ONE,
                },
                RigidBodyShape::Sphere {
                    radius: config.radius,
                },
                config.properties,
            )?;
            let handle = self.create_body_with_config(world_handle, bead)?.value;
            chain.bodies.push(handle);
            let world = self.active_world_mut(world_handle)?;
            while world.solver.tether(next_link).is_some()
                || world.invalidated_tethers.contains(&next_link)
            {
                next_link -= 1;
            }
            let endpoint = DynamicsTetherEndpoint::Body {
                body: DynamicsBodyId(handle),
                local_anchor: [0.0; 3],
            };
            world
                .solver
                .set_tether(DynamicsTether {
                    id: next_link,
                    first: previous,
                    second: endpoint,
                    maximum_length: f64::from(config.link_length),
                    target_length: f64::from(config.link_length),
                    reel_speed: 0.0,
                    contacts_enabled: false,
                })
                .map_err(|failure| error(failure.code(), failure.code()))?;
            chain.links.push(next_link);
            previous = endpoint;
        }
        Ok(())
    }

    pub(super) fn read_chain(
        &self,
        request: NativeDynamicsChainRequest,
    ) -> Result<NativeDynamicsChainReadout, CsharpEngineServicesError> {
        let world = self.active_world(request.world.value)?;
        let Some(chain) = world.chains.get(&request.id) else {
            return Ok(NativeDynamicsChainReadout {
                invalidated: world.invalidated_chains.contains(&request.id),
                ..Default::default()
            });
        };
        let mut result = NativeDynamicsChainReadout {
            present: true,
            point_count: chain.bodies.len() as u32 + 1,
            simulated: true,
            ..Default::default()
        };
        for link in &chain.links {
            let definition = world.solver.tether(*link).expect("chain link");
            result.effective_length += definition.maximum_length as f32;
            result.target_length += definition.target_length as f32;
            if let Some(readout) = world
                .solver
                .tether_readouts()
                .iter()
                .find(|readout| readout.id == *link)
            {
                result.force_proxy = result.force_proxy.max(readout.force_proxy as f32);
                result.taut_links += u32::from(readout.taut);
                result.caught_links += u32::from(readout.caught);
            } else {
                result.simulated = false;
            }
        }
        Ok(result)
    }

    pub(super) fn read_chain_point(
        &self,
        request: NativeDynamicsChainPointRequest,
    ) -> Result<NativeDynamicsChainPointReadout, CsharpEngineServicesError> {
        let world = self.active_world(request.world.value)?;
        let Some(chain) = world.chains.get(&request.id) else {
            return Ok(NativeDynamicsChainPointReadout::default());
        };
        let endpoint = if request.index == 0 {
            // The solver keeps fixed anchors rebased.
            world
                .solver
                .tether(chain.links[0])
                .expect("chain link")
                .first
        } else if let Some(bead) = chain.bodies.get(request.index as usize - 1) {
            DynamicsTetherEndpoint::Body {
                body: DynamicsBodyId(*bead),
                local_anchor: [0.0; 3],
            }
        } else {
            return Ok(NativeDynamicsChainPointReadout::default());
        };
        Ok(NativeDynamicsChainPointReadout {
            present: true,
            position: native_vec3(endpoint_point(world, endpoint)),
        })
    }

    pub(super) fn remove_chain(
        &mut self,
        request: NativeDynamicsChainRequest,
    ) -> Result<NativeDynamicsChainReleaseReceipt, CsharpEngineServicesError> {
        let world = self.active_world_mut(request.world.value)?;
        world.invalidated_chains.remove(&request.id);
        let Some(chain) = world.chains.remove(&request.id) else {
            return Ok(NativeDynamicsChainReleaseReceipt::default());
        };
        let released = world.remove_chain_bodies(&chain);
        let removed_bodies = released.len() as u32;
        for handle in released {
            self.bodies.insert(handle, BodySlot::Tombstoned);
        }
        Ok(NativeDynamicsChainReleaseReceipt {
            released: true,
            removed_bodies,
        })
    }
}

pub(super) unsafe extern "C" fn create_fixed_chain(
    context: *mut c_void,
    request: NativeDynamicsFixedChainRequest,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    if receipt.is_null() {
        return 0;
    }
    unsafe { *receipt = std::mem::zeroed() };
    if context.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }.create_fixed_chain(request) {
        Ok(()) => ABI_OK,
        Err(error) => {
            unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }
                .operation_diagnostics
                .retain(&error, receipt);
            0
        }
    }
}
pub(super) unsafe extern "C" fn create_body_chain(
    context: *mut c_void,
    request: NativeDynamicsBodyChainRequest,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    if receipt.is_null() {
        return 0;
    }
    unsafe { *receipt = std::mem::zeroed() };
    if context.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }.create_body_chain(request) {
        Ok(()) => ABI_OK,
        Err(error) => {
            unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }
                .operation_diagnostics
                .retain(&error, receipt);
            0
        }
    }
}
pub(super) unsafe extern "C" fn read_chain(
    context: *mut c_void,
    request: NativeDynamicsChainRequest,
    result: *mut NativeDynamicsChainReadout,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    if receipt.is_null() {
        return 0;
    }
    unsafe { *receipt = std::mem::zeroed() };
    if context.is_null() || result.is_null() {
        return 0;
    }
    match unsafe { &*context.cast::<RuntimeDynamicsBridge>() }.read_chain(request) {
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
pub(super) unsafe extern "C" fn read_chain_point(
    context: *mut c_void,
    request: NativeDynamicsChainPointRequest,
    result: *mut NativeDynamicsChainPointReadout,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    if receipt.is_null() {
        return 0;
    }
    unsafe { *receipt = std::mem::zeroed() };
    if context.is_null() || result.is_null() {
        return 0;
    }
    match unsafe { &*context.cast::<RuntimeDynamicsBridge>() }.read_chain_point(request) {
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
pub(super) unsafe extern "C" fn remove_chain(
    context: *mut c_void,
    request: NativeDynamicsChainRequest,
    result: *mut NativeDynamicsChainReleaseReceipt,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    if receipt.is_null() {
        return 0;
    }
    unsafe { *receipt = std::mem::zeroed() };
    if context.is_null() || result.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }.remove_chain(request) {
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

pub(super) unsafe extern "C" fn set_chain_length(
    context: *mut c_void,
    request: NativeDynamicsChainLengthRequest,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    if receipt.is_null() {
        return 0;
    }
    unsafe { *receipt = std::mem::zeroed() };
    if context.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }.set_chain_length(request) {
        Ok(()) => ABI_OK,
        Err(error) => {
            unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }
                .operation_diagnostics
                .retain(&error, receipt);
            0
        }
    }
}

pub(super) unsafe extern "C" fn configure_ropes(
    context: *mut c_void,
    request: NativeDynamicsRopeSolverRequest,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    if receipt.is_null() {
        return 0;
    }
    unsafe { *receipt = std::mem::zeroed() };
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() };
    let result = bridge
        .active_world_mut(request.world.value)
        .and_then(|world| {
            world
                .solver
                .configure_rope_solver(engine_spatial::DynamicsRopeSolverConfig {
                    substeps: request.substeps as usize,
                    iterations: request.iterations as usize,
                })
                .map_err(|error| CsharpEngineServicesError::new(error.code(), error.code()))
        });
    match result {
        Ok(()) => ABI_OK,
        Err(error) => {
            unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }
                .operation_diagnostics
                .retain(&error, receipt);
            0
        }
    }
}
