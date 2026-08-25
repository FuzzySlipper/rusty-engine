use rusty_engine::{
    core_ids::EntityId,
    core_math::Vec3,
    engine_spatial::VoxelCollisionScene,
    entity_state::{
        ComponentRegistration, ComponentTypeId, EntityAuthoringService, EntityComponent,
        EntityDefinition, EntityState,
    },
    product_kernel::{
        ProductKernelRuntimeDefinition, ProductKernelRuntimeMutationDescriptor,
        ProductKernelRuntimeSelection, ProductKernelStandardCapabilityBindError,
        ProductRuntimeResources,
    },
    product_model::{
        CapabilityAccess, CapabilityAvailability, CapabilityBudget, CapabilityKind,
        CapabilityMetadata, CapabilityProvenance, CapabilityUses,
        ProductKernelCapabilityDescriptor,
    },
    runtime_composition::{ProductRuntimeAdapter, ProductRuntimeOutputs, ProductRuntimeUi},
    runtime_input, runtime_lifecycle, runtime_mutation, runtime_schedule,
    runtime_standard_capabilities::{
        BoundStandardCapabilities, ObservePairsBatchIdentity, ObservePairsObserver,
        ObservePairsObserverFacts, ObservePairsPlan, ObservePairsTarget,
    },
    runtime_timeline,
};

#[derive(Clone)]
pub struct CounterAuthority {
    value: u64,
    observed_targets: u64,
    recurring_steps: u64,
    revision: u64,
}

impl runtime_mutation::MutationAuthority for CounterAuthority {
    type Guard = (u64, u64, u64, u64);

    fn guard(&self) -> Self::Guard {
        (
            self.value,
            self.observed_targets,
            self.recurring_steps,
            self.revision,
        )
    }

    fn publication_domain(&self) -> &str {
        "counter"
    }
}

#[derive(Default)]
pub struct CounterPlanner;

impl runtime_mutation::MutationPlanner<CounterAuthority, u32> for CounterPlanner {
    type Error = String;

    fn stage(
        &mut self,
        authority: &CounterAuthority,
        batch: &runtime_mutation::MutationResolvedBatch,
    ) -> Result<runtime_mutation::MutationStage<CounterAuthority, u32>, Self::Error> {
        let mut candidate = authority.clone();
        for operation in batch.operations() {
            match operation.binding_id() {
                "counter.increment" | "counter.timeline" => {
                    let amount = operation
                        .payload()
                        .get("amount")
                        .and_then(serde_json::Value::as_u64)
                        .ok_or_else(|| "counter increment payload has no amount".to_owned())?;
                    candidate.value = candidate
                        .value
                        .checked_add(amount)
                        .ok_or_else(|| "counter value overflowed".to_owned())?;
                }
                "counter.observe-result" => {
                    let results = operation
                        .payload()
                        .get("results")
                        .and_then(serde_json::Value::as_array)
                        .ok_or_else(|| "counter observation payload has no results".to_owned())?;
                    candidate.observed_targets = candidate
                        .observed_targets
                        .checked_add(results.len() as u64)
                        .ok_or_else(|| "counter observation count overflowed".to_owned())?;
                }
                "counter.recurring-result" => {
                    candidate.recurring_steps = candidate
                        .recurring_steps
                        .checked_add(1)
                        .ok_or_else(|| "counter recurring step overflowed".to_owned())?;
                }
                binding => return Err(format!("counter planner rejected binding {binding}")),
            }
        }
        candidate.revision = candidate
            .revision
            .checked_add(1)
            .ok_or_else(|| "counter revision overflowed".to_owned())?;
        let evidence = batch
            .operations()
            .iter()
            .enumerate()
            .map(|(index, operation)| {
                runtime_mutation::MutationOwnerEvidence::for_operation(operation, index as u32)
            })
            .collect();
        Ok(runtime_mutation::MutationStage::new(candidate, evidence))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ConformanceObserver;

impl EntityComponent for ConformanceObserver {}

impl ObservePairsObserver for ConformanceObserver {
    fn facts(&self) -> ObservePairsObserverFacts {
        ObservePairsObserverFacts {
            local_origin: Vec3::ZERO,
            local_forward: Vec3::new(1.0, 0.0, 0.0),
            maximum_distance: 4.0,
            minimum_facing_cosine: 0.5,
            evidence: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ConformanceTarget;

impl EntityComponent for ConformanceTarget {}

impl ObservePairsTarget for ConformanceTarget {
    fn local_center(&self) -> Vec3 {
        Vec3::ZERO
    }
}

pub struct CounterAdapter {
    authority: CounterAuthority,
    planner: CounterPlanner,
    pending_operations: Vec<runtime_mutation::MutationOperation>,
    observe_pairs: Vec<ObservePairsPlan>,
    entities: EntityState,
    scene: VoxelCollisionScene,
    timeline_generation: runtime_lifecycle::RuntimeGeneration,
    timeline_scheduled: bool,
}

impl CounterAdapter {
    fn queue(&mut self, operation: runtime_mutation::MutationOperation) {
        self.pending_operations.push(operation);
    }

    fn observe(
        &mut self,
        invocation: runtime_schedule::ScheduleSystemInvocation<'_>,
    ) -> Result<String, String> {
        let plan = self
            .observe_pairs
            .iter()
            .find(|plan| plan.matches_system(invocation.system()))
            .ok_or_else(|| {
                format!(
                    "counter has no retained observe-pairs plan for compiled system {}",
                    invocation.system_id()
                )
            })?;
        let step = invocation.step().value();
        let emission = plan
            .evaluate_and_batch::<ConformanceObserver, ConformanceTarget>(
                &self.entities,
                &self.scene,
                ObservePairsBatchIdentity {
                    batch_id: runtime_mutation::MutationBatchId::new(format!(
                        "counter-observe-{step}"
                    ))
                    .map_err(|error| format!("counter observe batch: {error}"))?,
                    causation: runtime_mutation::MutationCausation::new("counter.observe-pairs")
                        .map_err(|error| format!("counter observe causation: {error}"))?,
                    provenance: runtime_mutation::MutationProvenance::new("counter.runtime")
                        .map_err(|error| format!("counter observe provenance: {error}"))?,
                    operation_id: runtime_mutation::MutationOperationId::new(
                        step.checked_mul(4)
                            .and_then(|value| value.checked_add(1))
                            .ok_or_else(|| "counter observe operation id overflowed".to_owned())?,
                    ),
                },
            )
            .map_err(|error| format!("counter observe execution: {error}"))?;
        self.pending_operations
            .extend(emission.batch.operations().iter().cloned());
        Ok(format!(
            "counter.observe-pairs visible={}",
            emission.readout.visible_pairs
        ))
    }
}

impl ProductRuntimeAdapter for CounterAdapter {
    type Authority = CounterAuthority;
    type Guard = (u64, u64, u64, u64);
    type Planner = CounterPlanner;
    type Evidence = u32;
    type Error = String;
    type ScheduleOutput = String;
    type UiOutput = String;

    fn on_input(
        &mut self,
        _frame: &runtime_input::InputFrame,
        intents: &[runtime_input::RuntimeIntentEnvelope],
    ) -> Result<(), Self::Error> {
        for intent in intents {
            if intent.intent() != "increment" {
                continue;
            }
            let runtime_input::RuntimeIntentValue::Digital { active } = intent.value() else {
                return Err("counter increment requires a digital intent".to_owned());
            };
            if !active {
                continue;
            }
            self.queue(
                runtime_mutation::MutationOperation::new(
                    runtime_mutation::MutationOperationId::new(
                        intent
                            .sequence()
                            .checked_mul(4)
                            .ok_or_else(|| "counter input operation id overflowed".to_owned())?,
                    ),
                    "counter.increment",
                    "kernel.counter-increment",
                    serde_json::json!({"amount": 1}),
                )
                .map_err(|error| format!("counter input operation: {error}"))?,
            );
        }
        Ok(())
    }

    fn dispatch_schedule(
        &mut self,
        invocation: runtime_schedule::ScheduleSystemInvocation<'_>,
        _lifecycle: &runtime_lifecycle::RuntimeLifecycle,
        _token: runtime_lifecycle::RuntimePhaseToken,
    ) -> Result<Self::ScheduleOutput, Self::Error> {
        match invocation.system_id() {
            "counter.observe-pairs" => self.observe(invocation),
            "counter.recurring" => {
                let step = invocation.step().value();
                self.queue(
                    runtime_mutation::MutationOperation::new(
                        runtime_mutation::MutationOperationId::new(
                            step.checked_mul(4)
                                .and_then(|value| value.checked_add(3))
                                .ok_or_else(|| "counter recurring operation id overflowed".to_owned())?,
                        ),
                        "counter.recurring-result",
                        "kernel.counter-recurring-result",
                        serde_json::json!({"step": step}),
                    )
                    .map_err(|error| format!("counter recurring operation: {error}"))?,
                );
                Ok(format!("counter.recurring queued={step}"))
            }
            system => Err(format!("unexpected counter system {system}")),
        }
    }

    fn prepare_timeline(
        &mut self,
        step: runtime_lifecycle::SimulationStep,
    ) -> Result<Vec<runtime_timeline::TimelineOperationSpec>, Self::Error> {
        if self.timeline_scheduled {
            return Ok(Vec::new());
        }
        let request = runtime_timeline::TimelineOperationSpec::new(
            "counter.pulse",
            "counter.timeline.increment",
            runtime_timeline::TimelineOperationIdentity::new(1),
            step,
            runtime_timeline::TimelineRecurrence::Every {
                interval_steps: 2,
                remaining: 3,
            },
            runtime_timeline::RuntimeProvenance::new("counter.timeline", None)
                .map_err(|error| format!("counter timeline provenance: {error}"))?,
        )
        .map_err(|error| format!("counter timeline request: {error}"))?;
        self.timeline_scheduled = true;
        Ok(vec![request])
    }

    fn on_timeline_releases(
        &mut self,
        releases: &runtime_timeline::TimelineRelease,
    ) -> Result<(), Self::Error> {
        for event in releases.events() {
            let runtime_timeline::ReleasedTimelineEvent::Operation(operation) = event else {
                continue;
            };
            if operation.step().capability().id() != "counter.timeline" {
                return Err("counter timeline released an unexpected capability".to_owned());
            }
            let amount = operation
                .step()
                .payload()
                .get("amount")
                .and_then(serde_json::Value::as_u64)
                .ok_or_else(|| "counter timeline payload has no amount".to_owned())?;
            self.queue(
                runtime_mutation::MutationOperation::new(
                    runtime_mutation::MutationOperationId::new(
                        operation
                            .operation_id()
                            .value()
                            .checked_mul(4)
                            .and_then(|value| value.checked_add(2))
                            .ok_or_else(|| "counter timeline operation id overflowed".to_owned())?,
                    ),
                    "counter.timeline",
                    "kernel.counter-timeline",
                    serde_json::json!({"amount": amount}),
                )
                .map_err(|error| format!("counter timeline operation: {error}"))?,
            );
        }
        Ok(())
    }

    fn prepare_mutation(
        &mut self,
        step: runtime_lifecycle::SimulationStep,
    ) -> Result<Option<runtime_mutation::MutationBatch>, Self::Error> {
        if self.pending_operations.is_empty() {
            return Ok(None);
        }
        let operations = std::mem::take(&mut self.pending_operations);
        Ok(Some(
            runtime_mutation::MutationBatch::new(
                runtime_mutation::MutationBatchId::new(format!("counter-step-{}", step.value()))
                    .map_err(|error| format!("counter batch: {error}"))?,
                runtime_mutation::MutationCausation::new("counter.runtime-step")
                    .map_err(|error| format!("counter causation: {error}"))?,
                runtime_mutation::MutationProvenance::new("counter.runtime")
                    .map_err(|error| format!("counter provenance: {error}"))?,
                operations,
            )
            .map_err(|error| format!("counter batch: {error}"))?,
        ))
    }

    fn mutation_parts(&mut self) -> (&mut Self::Authority, &mut Self::Planner) {
        (&mut self.authority, &mut self.planner)
    }

    fn project(
        &mut self,
        _lifecycle: &runtime_lifecycle::RuntimeLifecycle,
        _token: runtime_lifecycle::RuntimePhaseToken,
    ) -> Result<ProductRuntimeOutputs<Self::UiOutput>, Self::Error> {
        ProductRuntimeOutputs::new(
            vec![ProductRuntimeUi::new(
                "counter",
                "counter.v1",
                format!(
                    "value={};observed={};recurring={};revision={}",
                    self.authority.value,
                    self.authority.observed_targets,
                    self.authority.recurring_steps,
                    self.authority.revision,
                ),
            )],
            None,
            None,
        )
        .map_err(|error| error.to_string())
    }

    fn rebind(
        &mut self,
        lifecycle: &runtime_lifecycle::RuntimeLifecycle,
    ) -> Result<(), Self::Error> {
        self.pending_operations.clear();
        let generation = lifecycle.readout().generation();
        if generation != self.timeline_generation {
            self.timeline_generation = generation;
            self.timeline_scheduled = false;
        }
        Ok(())
    }
}

pub struct RustyProductRuntime;

const CAPABILITIES: &[ProductKernelCapabilityDescriptor] = &[
    ProductKernelCapabilityDescriptor::new(
        "counter-increment",
        CapabilityMetadata::new(
            CapabilityKind::Operation,
            CapabilityUses::INPUT_MAP,
            CapabilityAvailability::Linkable,
            CapabilityAccess::new(&[], &["counter.value"]),
            CapabilityBudget::new(4096),
            CapabilityProvenance::new("rusty.product.conformance", "kernel/entry.rs", "counter_increment"),
        ),
    ),
    ProductKernelCapabilityDescriptor::new(
        "counter-observe",
        CapabilityMetadata::new(
            CapabilityKind::Operation,
            CapabilityUses::SCHEDULE,
            CapabilityAvailability::Linkable,
            CapabilityAccess::new(&["runtime-mutation.operations"], &["counter.observed-targets"]),
            CapabilityBudget::new(16_384),
            CapabilityProvenance::new("rusty.product.conformance", "kernel/entry.rs", "counter_observe"),
        ),
    ),
    ProductKernelCapabilityDescriptor::new(
        "counter-recurring",
        CapabilityMetadata::new(
            CapabilityKind::System,
            CapabilityUses::SCHEDULE,
            CapabilityAvailability::Linkable,
            CapabilityAccess::new(&["counter.value"], &["counter.recurring-readout"]),
            CapabilityBudget::new(4096),
            CapabilityProvenance::new("rusty.product.conformance", "kernel/entry.rs", "counter_recurring"),
        ),
    ),
    ProductKernelCapabilityDescriptor::new(
        "counter-recurring-result",
        CapabilityMetadata::new(
            CapabilityKind::Operation,
            CapabilityUses::SCHEDULE,
            CapabilityAvailability::Linkable,
            CapabilityAccess::new(&["runtime-schedule.system"], &["counter.recurring-readout"]),
            CapabilityBudget::new(4096),
            CapabilityProvenance::new("rusty.product.conformance", "kernel/entry.rs", "counter_recurring_result"),
        ),
    ),
    ProductKernelCapabilityDescriptor::new(
        "counter-timeline",
        CapabilityMetadata::new(
            CapabilityKind::Operation,
            CapabilityUses::TIMELINE,
            CapabilityAvailability::Linkable,
            CapabilityAccess::new(&[], &["counter.value"]),
            CapabilityBudget::new(4096),
            CapabilityProvenance::new("rusty.product.conformance", "kernel/entry.rs", "counter_timeline"),
        ),
    ),
];

const SELECTIONS: &[ProductKernelRuntimeSelection] = &[
    ProductKernelRuntimeSelection::new("counter-increment", "kernel.counter-increment", "counter.increment.v1", CapabilityKind::Operation),
    ProductKernelRuntimeSelection::new("counter-observe", "kernel.counter-observe", "counter.observe.v1", CapabilityKind::Operation),
    ProductKernelRuntimeSelection::new("counter-recurring", "kernel.counter-recurring", "counter.recurring.v1", CapabilityKind::System),
    ProductKernelRuntimeSelection::new("counter-recurring-result", "kernel.counter-recurring-result", "counter.recurring-result.v1", CapabilityKind::Operation),
    ProductKernelRuntimeSelection::new("counter-timeline", "kernel.counter-timeline", "counter.timeline.v1", CapabilityKind::Operation),
];

const MUTATIONS: &[ProductKernelRuntimeMutationDescriptor] = &[
    ProductKernelRuntimeMutationDescriptor::new("counter.increment", "kernel.counter-increment", "counter", "rusty.product.conformance", "counter.increment.v1"),
    ProductKernelRuntimeMutationDescriptor::new("counter.observe-result", "kernel.counter-observe", "counter", "rusty.product.conformance", "engine.runtime.observe-pairs.result.v1"),
    ProductKernelRuntimeMutationDescriptor::new("counter.recurring-result", "kernel.counter-recurring-result", "counter", "rusty.product.conformance", "counter.recurring-result.v1"),
    ProductKernelRuntimeMutationDescriptor::new("counter.timeline", "kernel.counter-timeline", "counter", "rusty.product.conformance", "counter.timeline.v1"),
];

impl ProductKernelRuntimeDefinition for RustyProductRuntime {
    type Adapter = CounterAdapter;
    type Error = String;
    type ProductState = CounterAuthority;
    type ObserverComponent = ConformanceObserver;
    type TargetComponent = ConformanceTarget;

    fn capabilities() -> &'static [ProductKernelCapabilityDescriptor] {
        CAPABILITIES
    }

    fn selections() -> &'static [ProductKernelRuntimeSelection] {
        SELECTIONS
    }

    fn mutation_descriptors() -> &'static [ProductKernelRuntimeMutationDescriptor] {
        MUTATIONS
    }

    fn build(resources: ProductRuntimeResources<'_>) -> Result<Self::Adapter, Self::Error> {
        if resources.resource("content/counter.json").is_none() {
            return Err("generated content resources are not available".to_owned());
        }
        let observer = EntityId::new(1);
        let target = EntityId::new(2);
        let mut entities = EntityState::from_definitions([
            EntityDefinition::new(observer, "counter observer").with_transform(Vec3::ZERO),
            EntityDefinition::new(target, "counter target").with_transform(Vec3::new(1.0, 0.0, 0.0)),
        ])
        .map_err(|error| format!("counter entities: {error}"))?;
        entities
            .register_component(ComponentRegistration::<ConformanceObserver>::runtime_only(
                ComponentTypeId::parse("counter.observer")
                    .map_err(|error| format!("counter observer role: {error}"))?,
            ))
            .map_err(|error| format!("counter observer registration: {error}"))?;
        entities
            .register_component(ComponentRegistration::<ConformanceTarget>::runtime_only(
                ComponentTypeId::parse("counter.target")
                    .map_err(|error| format!("counter target role: {error}"))?,
            ))
            .map_err(|error| format!("counter target registration: {error}"))?;
        let observer_revision = entities
            .component_revision::<ConformanceObserver>(observer)
            .map_err(|error| format!("counter observer revision: {error}"))?;
        EntityAuthoringService
            .attach_component(&mut entities, observer_revision, observer, ConformanceObserver)
            .map_err(|error| format!("counter observer attachment: {error}"))?;
        let target_revision = entities
            .component_revision::<ConformanceTarget>(target)
            .map_err(|error| format!("counter target revision: {error}"))?;
        EntityAuthoringService
            .attach_component(&mut entities, target_revision, target, ConformanceTarget)
            .map_err(|error| format!("counter target attachment: {error}"))?;
        Ok(CounterAdapter {
            authority: CounterAuthority {
                value: 0,
                observed_targets: 0,
                recurring_steps: 0,
                revision: 0,
            },
            planner: CounterPlanner,
            pending_operations: Vec::new(),
            observe_pairs: Vec::new(),
            entities,
            scene: VoxelCollisionScene::from_solid_voxels(1.0, 8, [])
                .map_err(|error| format!("counter collision scene: {error}"))?,
            timeline_generation: runtime_lifecycle::RuntimeGeneration::ZERO,
            timeline_scheduled: false,
        })
    }

    fn bind_standard_capabilities(
        adapter: &mut Self::Adapter,
        plans: BoundStandardCapabilities,
    ) -> Result<(), ProductKernelStandardCapabilityBindError> {
        let received = plans.observe_pairs().len();
        if received != 1 {
            return Err(ProductKernelStandardCapabilityBindError::UnexpectedObservePairs {
                expected: 1,
                received,
            });
        }
        adapter.observe_pairs = plans.into_observe_pairs();
        Ok(())
    }
}
