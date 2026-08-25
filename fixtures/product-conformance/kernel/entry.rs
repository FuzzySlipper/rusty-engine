use rusty_engine::{
    product_kernel::{
        ProductKernelRuntimeDefinition, ProductKernelRuntimeMutationDescriptor,
        ProductKernelRuntimeSelection, ProductRuntimeResources,
    },
    product_model::{
        CapabilityAccess, CapabilityAvailability, CapabilityBudget, CapabilityKind,
        CapabilityMetadata, CapabilityProvenance, CapabilityUses,
        ProductKernelCapabilityDescriptor,
    },
    runtime_composition::{ProductRuntimeAdapter, ProductRuntimeOutputs, ProductRuntimeUi},
    runtime_input, runtime_lifecycle, runtime_mutation, runtime_schedule, runtime_timeline,
};

pub struct CounterAuthority {
    value: u64,
    revision: u64,
}

impl runtime_mutation::MutationAuthority for CounterAuthority {
    type Guard = (u64, u64);

    fn guard(&self) -> Self::Guard {
        (self.value, self.revision)
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
        let mut candidate = CounterAuthority {
            value: authority.value,
            revision: authority.revision,
        };
        for operation in batch.operations() {
            let amount = operation
                .payload()
                .get("amount")
                .and_then(serde_json::Value::as_u64)
                .ok_or_else(|| "counter mutation payload has no amount".to_owned())?;
            candidate.value = candidate
                .value
                .checked_add(amount)
                .ok_or_else(|| "counter value overflowed".to_owned())?;
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

pub struct CounterAdapter {
    authority: CounterAuthority,
    planner: CounterPlanner,
    pending: Option<runtime_mutation::MutationBatch>,
}

impl ProductRuntimeAdapter for CounterAdapter {
    type Authority = CounterAuthority;
    type Guard = (u64, u64);
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
            let operation = runtime_mutation::MutationOperation::new(
                runtime_mutation::MutationOperationId::new(intent.sequence()),
                "counter.increment",
                "kernel.counter-increment",
                serde_json::json!({"amount": 1}),
            )
            .map_err(|error| format!("counter operation: {error}"))?;
            self.pending = Some(
                runtime_mutation::MutationBatch::new(
                    runtime_mutation::MutationBatchId::new(format!(
                        "counter-step-{}",
                        intent.sequence()
                    ))
                    .map_err(|error| format!("counter batch: {error}"))?,
                    runtime_mutation::MutationCausation::new("input.increment")
                        .map_err(|error| format!("counter causation: {error}"))?,
                    runtime_mutation::MutationProvenance::new("counter.runtime")
                        .map_err(|error| format!("counter provenance: {error}"))?,
                    vec![operation],
                )
                .map_err(|error| format!("counter batch: {error}"))?,
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
        Err(format!(
            "unexpected counter system {}",
            invocation.system_id()
        ))
    }

    fn on_timeline_releases(
        &mut self,
        _releases: &runtime_timeline::TimelineRelease,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn prepare_mutation(
        &mut self,
        _step: runtime_lifecycle::SimulationStep,
    ) -> Result<Option<runtime_mutation::MutationBatch>, Self::Error> {
        Ok(self.pending.take())
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
                self.authority.value.to_string(),
            )],
            None,
            None,
        )
        .map_err(|error| error.to_string())
    }
}

pub struct RustyProductRuntime;

const CAPABILITIES: &[ProductKernelCapabilityDescriptor] =
    &[ProductKernelCapabilityDescriptor::new(
        "counter-increment",
        CapabilityMetadata::new(
            CapabilityKind::Operation,
            CapabilityUses::INPUT_MAP,
            CapabilityAvailability::Linkable,
            CapabilityAccess::new(&[], &["counter.value"]),
            CapabilityBudget::new(4096),
            CapabilityProvenance::new(
                "rusty.product.conformance",
                "kernel/entry.rs",
                "counter_increment",
            ),
        ),
    )];

const SELECTIONS: &[ProductKernelRuntimeSelection] = &[ProductKernelRuntimeSelection::new(
    "counter-increment",
    "kernel.counter-increment",
    "counter.increment.v1",
    CapabilityKind::Operation,
)];

const MUTATIONS: &[ProductKernelRuntimeMutationDescriptor] =
    &[ProductKernelRuntimeMutationDescriptor::new(
        "counter.increment",
        "kernel.counter-increment",
        "counter",
        "rusty.product.conformance",
        "counter.increment.v1",
    )];

impl ProductKernelRuntimeDefinition for RustyProductRuntime {
    type Adapter = CounterAdapter;
    type Error = String;
    type ProductState = CounterAuthority;
    type ObserverComponent = ();
    type TargetComponent = ();

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
        if resources.resource("content/manifest.json").is_none() {
            return Err("generated content resources are not available".to_owned());
        }
        Ok(CounterAdapter {
            authority: CounterAuthority {
                value: 0,
                revision: 0,
            },
            planner: CounterPlanner,
            pending: None,
        })
    }
}
