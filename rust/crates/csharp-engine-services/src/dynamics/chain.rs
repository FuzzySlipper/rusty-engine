use super::*;

pub(super) const MAX_ROPES: usize = 64;
const MAX_BEADS: u32 = 8;
const MAX_WORLD_BODIES: usize = engine_spatial::MAX_DYNAMICS_BODIES;

/// Bodies and links are owned by the world, never exposed as disposable product bodies.
pub(super) struct DynamicsChain {
    pub links: Vec<u64>,
    pub(super) bodies: Vec<(u64, EntityId)>,
    pub(super) anchor: DynamicsTetherEndpoint,
}

pub(super) fn error(code: &'static str, message: &str) -> CsharpEngineServicesError {
    CsharpEngineServicesError::new(code, message)
}

pub(super) fn authored_count(world: &DynamicsWorld) -> usize {
    world.service.tether_count()
        - world
            .chains
            .values()
            .map(|chain| chain.links.len())
            .sum::<usize>()
        + world.chains.len()
        + world.invalidated_tethers.len()
        + world.invalidated_chains.len()
}

fn endpoint_point(
    world: &DynamicsWorld,
    endpoint: DynamicsTetherEndpoint,
) -> Result<Vec3, CsharpEngineServicesError> {
    match endpoint {
        DynamicsTetherEndpoint::Fixed(point) => {
            Ok(Vec3::new(point[0] as f32, point[1] as f32, point[2] as f32))
        }
        DynamicsTetherEndpoint::Body { body, local_anchor } => {
            let transform = world
                .entities
                .view(EntityId::new(body.0))
                .map_err(|_| error("dynamics-chain-invalid-anchor", "missing chain anchor"))?
                .transform
                .ok_or_else(|| {
                    error(
                        "dynamics-chain-invalid-anchor",
                        "missing chain anchor transform",
                    )
                })?;
            Ok(transform.transform().transform_point(Vec3::new(
                local_anchor[0] as f32,
                local_anchor[1] as f32,
                local_anchor[2] as f32,
            )))
        }
    }
}

impl RuntimeDynamicsBridge {
    pub(super) fn set_chain_length(
        &mut self,
        request: NativeDynamicsChainLengthRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        const MAX_TOTAL_REEL_SPEED: f32 = engine_spatial::MAX_TETHER_REEL_SPEED as f32;
        if !request.target_length.is_finite()
            || request.target_length <= 0.0
            || !request.reel_speed.is_finite()
            || !(0.0..=MAX_TOTAL_REEL_SPEED).contains(&request.reel_speed)
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
        let mut definitions = world.service.capture_tethers();
        for definition in &mut definitions {
            if chain.links.contains(&definition.id) {
                definition.target_length =
                    f64::from(request.target_length) / chain.links.len() as f64;
                definition.reel_speed = f64::from(request.reel_speed) / chain.links.len() as f64;
            }
        }
        world
            .service
            .replace_tethers(&world.entities, definitions)
            .map_err(|failure| error(failure.code(), failure.code()))?;
        world
            .last_tethers
            .retain(|readout| !chain.links.contains(&readout.id));
        Ok(())
    }

    pub(super) fn create_fixed_chain(
        &mut self,
        request: NativeDynamicsFixedChainRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        self.create_chain(
            request.world.value,
            DynamicsTetherEndpoint::Fixed([
                f64::from(request.anchor.x),
                f64::from(request.anchor.y),
                f64::from(request.anchor.z),
            ]),
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
        if !(1..=MAX_BEADS).contains(&config.bead_count)
            || !config.link_length.is_finite()
            || config.link_length <= 0.0
            || !(config.link_length * config.bead_count as f32).is_finite()
            || !config.radius.is_finite()
            || config.radius <= 0.0
            || !config.properties.enabled
        {
            return Err(error(
                "invalid-dynamics-chain-configuration",
                "invalid chain configuration",
            ));
        }
        if (!world.invalidated_chains.contains(&config.id) && authored_count(world) >= MAX_ROPES)
            || world.bodies.len() + config.bead_count as usize > MAX_WORLD_BODIES
        {
            return Err(error(
                "dynamics-chain-budget-exceeded",
                "chain or body budget exceeded",
            ));
        }
        let start = endpoint_point(world, anchor)?;
        let end = native_vec3_value(end);
        let separation = (end - start).length();
        if !finite_vec3(start)
            || !finite_vec3(end)
            || !separation.is_finite()
            || separation > config.link_length * config.bead_count as f32 + 0.001
        {
            return Err(error(
                "dynamics-chain-invalid-anchor",
                "chain endpoints are nonfinite or out of reach",
            ));
        }
        let mut entities = world.entities.clone();
        let mut service = world.service.clone();
        let mut definitions = service.capture_tethers();
        let mut next_entity = self.next_entity;
        let mut next_body = self.next_body;
        // Internal link identities are allocated from the available namespace,
        // not derived from a product ID. Public tether edits cannot address them.
        let mut next_link = u64::MAX;
        let mut chain = DynamicsChain {
            links: Vec::new(),
            bodies: Vec::new(),
            anchor,
        };
        let mut previous = anchor;
        for index in 0..config.bead_count {
            let entity = EntityId::new(Self::allocate(&mut next_entity, "chain entity")?);
            let handle = Self::allocate(&mut next_body, "chain body")?;
            let position = start + (end - start) * ((index + 1) as f32 / config.bead_count as f32);
            let body = body_config_with_properties(
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
            insert_body(&mut entities, entity, body)?;
            while definitions
                .iter()
                .any(|definition| definition.id == next_link)
                || world.invalidated_tethers.contains(&next_link)
            {
                next_link = next_link.checked_sub(1).ok_or_else(|| {
                    error("dynamics-chain-budget-exceeded", "link identity exhausted")
                })?;
            }
            let endpoint = DynamicsTetherEndpoint::Body {
                body: DynamicsBodyId(entity.raw()),
                local_anchor: [0.0; 3],
            };
            definitions.push(DynamicsTether {
                id: next_link,
                first: previous,
                second: endpoint,
                maximum_length: f64::from(config.link_length),
                target_length: f64::from(config.link_length),
                reel_speed: 0.0,
                was_taut: false,
                wake: true,
                contacts_enabled: false,
            });
            chain.links.push(next_link);
            chain.bodies.push((handle, entity));
            previous = endpoint;
        }
        service
            .replace_tethers(&entities, definitions)
            .map_err(|failure| error(failure.code(), failure.code()))?;
        // No canonical mutation precedes full body/link validation.
        let world = self.active_world_mut(world_handle)?;
        world.entities = entities;
        world.service = service;
        for (handle, entity) in &chain.bodies {
            world.bodies.insert(*handle, *entity);
        }
        world.invalidated_chains.remove(&config.id);
        let slots = chain.bodies.clone();
        world.chains.insert(config.id, chain);
        for (handle, entity) in slots {
            self.bodies.insert(
                handle,
                BodySlot::Active {
                    world: world_handle,
                    entity,
                },
            );
        }
        self.next_entity = next_entity;
        self.next_body = next_body;
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
            let definition = world
                .service
                .tether(*link)
                .ok_or_else(|| error("dynamics-chain-not-found", "missing chain link"))?;
            result.effective_length += definition.maximum_length as f32;
            result.target_length += definition.target_length as f32;
            if let Some(readout) = world
                .last_tethers
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
            // Fixed endpoints are canonically rebased in the service.
            world
                .service
                .tether(chain.links[0])
                .ok_or_else(|| error("dynamics-chain-not-found", "missing chain link"))?
                .first
        } else if let Some((_, entity)) = chain.bodies.get(request.index as usize - 1) {
            DynamicsTetherEndpoint::Body {
                body: DynamicsBodyId(entity.raw()),
                local_anchor: [0.0; 3],
            }
        } else {
            return Ok(NativeDynamicsChainPointReadout::default());
        };
        Ok(NativeDynamicsChainPointReadout {
            present: true,
            position: native_vec3(endpoint_point(world, endpoint)?),
        })
    }

    pub(super) fn remove_chain(
        &mut self,
        request: NativeDynamicsChainRequest,
    ) -> Result<NativeDynamicsChainReleaseReceipt, CsharpEngineServicesError> {
        let world = self.active_world_mut(request.world.value)?;
        let Some(chain) = world.chains.get(&request.id) else {
            world.invalidated_chains.remove(&request.id);
            return Ok(NativeDynamicsChainReleaseReceipt::default());
        };
        let mut entities = world.entities.clone();
        for (_, entity) in &chain.bodies {
            let revision = entities.revision();
            EntityAuthoringService
                .destroy(&mut entities, revision, *entity)
                .map_err(|failure| error("dynamics-chain-release-failed", &failure.to_string()))?;
        }
        let mut service = world.service.clone();
        let definitions = service
            .capture_tethers()
            .into_iter()
            .filter(|definition| !chain.links.contains(&definition.id))
            .collect();
        service
            .replace_tethers(&entities, definitions)
            .map_err(|failure| error(failure.code(), failure.code()))?;
        let chain = world.chains.remove(&request.id).expect("validated chain");
        world.entities = entities;
        world.service = service;
        world
            .last_tethers
            .retain(|readout| !chain.links.contains(&readout.id));
        for (handle, entity) in &chain.bodies {
            world.bodies.remove(handle);
            world.last_contacts.remove(entity);
            world
                .last_contact_receipts
                .retain(|contact| contact.first != *entity && contact.second != Some(*entity));
        }
        for (handle, _) in &chain.bodies {
            self.bodies.insert(*handle, BodySlot::Tombstoned);
        }
        Ok(NativeDynamicsChainReleaseReceipt {
            released: true,
            removed_bodies: chain.bodies.len() as u32,
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
            unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }.retain_operation_error(
                &error,
                receipt,
                b"CreateFixedChain",
            );
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
            unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }.retain_operation_error(
                &error,
                receipt,
                b"CreateBodyChain",
            );
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
            unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }.retain_operation_error(
                &error,
                receipt,
                b"ReadChain",
            );
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
            unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }.retain_operation_error(
                &error,
                receipt,
                b"ReadChainPoint",
            );
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
            unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }.retain_operation_error(
                &error,
                receipt,
                b"RemoveChain",
            );
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
            unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }.retain_operation_error(
                &error,
                receipt,
                b"SetChainLength",
            );
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
                .service
                .configure_rope_solver(engine_spatial::DynamicsRopeSolverConfig {
                    substeps: request.substeps as usize,
                    iterations: request.iterations as usize,
                })
                .map_err(|error| CsharpEngineServicesError::new(error.code(), error.code()))
        });
    match result {
        Ok(()) => ABI_OK,
        Err(error) => {
            unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() }.retain_operation_error(
                &error,
                receipt,
                b"ConfigureRopes",
            );
            0
        }
    }
}
