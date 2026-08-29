use product_model::{
    admit_checked_product_composition, decode_compiled_composition, decode_product_manifest,
    link_admitted_product_composition, CapabilityAccess, CapabilityAvailability, CapabilityKind,
    CapabilityMetadata, CapabilityProvenance, CapabilityUses, ProductKernelCapabilityDescriptor,
};
use runtime_composition::{
    ProductRuntimeAdapter, ProductRuntimeOutputs, ProductRuntimeUi, RuntimeComposition,
    RuntimeCompositionInputs,
};
use runtime_input::InputContext;
use runtime_lifecycle::{RuntimeInstanceId, RuntimeLifecycleConfig, RuntimePhaseToken};
use runtime_mutation::{
    CompiledMutationCatalog, MutationAuthority, MutationBatch, MutationBatchId,
    MutationCapabilityDescriptor, MutationCausation, MutationOperation, MutationOperationId,
    MutationOwnerEvidence, MutationPlanner, MutationProvenance, MutationStage,
};
use runtime_schedule::{CompiledRuntimeSchedule, ScheduleSystemInvocation};
use runtime_timeline::TimelineCatalog;
use serde_json::{json, Value};

const MANIFEST: &str = include_str!("../../../../fixtures/product-model/minimum.rusty.toml");

#[derive(Debug, Clone, PartialEq, Eq)]
struct CounterAuthority {
    value: u64,
}

impl MutationAuthority for CounterAuthority {
    type Guard = u64;

    fn guard(&self) -> Self::Guard {
        self.value
    }

    fn publication_domain(&self) -> &str {
        "counter"
    }
}

#[derive(Debug, Default)]
struct CounterPlanner;

impl MutationPlanner<CounterAuthority, u32> for CounterPlanner {
    type Error = &'static str;

    fn stage(
        &mut self,
        authority: &CounterAuthority,
        batch: &runtime_mutation::MutationResolvedBatch,
    ) -> Result<MutationStage<CounterAuthority, u32>, Self::Error> {
        let mut candidate = authority.clone();
        candidate.value = candidate
            .value
            .checked_add(batch.operations().len() as u64)
            .ok_or("counter overflow")?;
        let evidence = batch
            .operations()
            .iter()
            .enumerate()
            .map(|(index, operation)| MutationOwnerEvidence::for_operation(operation, index as u32))
            .collect();
        Ok(MutationStage::new(candidate, evidence))
    }
}

struct CounterAdapter {
    authority: CounterAuthority,
    planner: CounterPlanner,
    claim_pending: bool,
    fail_projection: bool,
}

impl CounterAdapter {
    fn new() -> Self {
        Self {
            authority: CounterAuthority { value: 0 },
            planner: CounterPlanner,
            claim_pending: false,
            fail_projection: false,
        }
    }

    fn failing_projection() -> Self {
        Self {
            fail_projection: true,
            ..Self::new()
        }
    }
}

impl ProductRuntimeAdapter for CounterAdapter {
    type Authority = CounterAuthority;
    type Guard = u64;
    type Planner = CounterPlanner;
    type Evidence = u32;
    type Error = &'static str;
    type ScheduleOutput = (String, u64);
    type UiOutput = Value;

    fn on_input(
        &mut self,
        _frame: &runtime_input::InputFrame,
        intents: &[runtime_input::RuntimeIntentEnvelope],
    ) -> Result<(), Self::Error> {
        self.claim_pending = intents
            .iter()
            .any(|intent| intent.intent() == "counter.increment");
        Ok(())
    }

    fn dispatch_schedule(
        &mut self,
        invocation: ScheduleSystemInvocation<'_>,
        _lifecycle: &runtime_lifecycle::RuntimeLifecycle,
        _token: RuntimePhaseToken,
    ) -> Result<Self::ScheduleOutput, Self::Error> {
        Ok((invocation.system_id().to_owned(), self.authority.value))
    }

    fn on_timeline_releases(
        &mut self,
        _releases: &runtime_timeline::TimelineRelease,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn prepare_mutation(
        &mut self,
        step: runtime_lifecycle::SimulationStep,
    ) -> Result<Option<MutationBatch>, Self::Error> {
        if !self.claim_pending {
            return Ok(None);
        }
        self.claim_pending = false;
        let operation = MutationOperation::new(
            MutationOperationId::new(step.value()),
            "counter.increment",
            "kernel.counter-increment",
            json!({"counter": 1}),
        )
        .map_err(|_| "operation")?;
        Ok(Some(
            MutationBatch::new(
                MutationBatchId::new(format!("counter-{}", step.value())).map_err(|_| "batch")?,
                MutationCausation::new("counter-input").map_err(|_| "causation")?,
                MutationProvenance::new("counter.adapter").map_err(|_| "provenance")?,
                vec![operation],
            )
            .map_err(|_| "batch data")?,
        ))
    }

    fn mutation_parts(&mut self) -> (&mut Self::Authority, &mut Self::Planner) {
        (&mut self.authority, &mut self.planner)
    }

    fn project(
        &mut self,
        _lifecycle: &runtime_lifecycle::RuntimeLifecycle,
        _token: RuntimePhaseToken,
    ) -> Result<ProductRuntimeOutputs<Self::UiOutput>, Self::Error> {
        if self.fail_projection {
            return Err("projection failed");
        }
        ProductRuntimeOutputs::new(
            vec![ProductRuntimeUi::new(
                "counter",
                "counter.v1",
                json!({"counter": self.authority.value}),
            )],
            None,
            None,
        )
        .map_err(|_| "ui")
    }

    fn rebind(
        &mut self,
        _lifecycle: &runtime_lifecycle::RuntimeLifecycle,
    ) -> Result<(), Self::Error> {
        self.fail_projection = false;
        Ok(())
    }
}

fn linked() -> product_model::LinkedProductComposition {
    let composition = decode_compiled_composition(
        br#"{
          "product":"example.product",
          "intentDescriptors":[{"id":"counter.increment","valueKind":"digital","capability":"counter.increment","payload":{}}],
          "inputMap":[{"id":"counter.increment","intent":"counter.increment","trigger":{"kind":"key","code":"space","edge":"pressed"}}],
          "schedule":[
            {"phase":"input","mode":"append","systems":[]},
            {"phase":"simulation","mode":"append","systems":[]},
            {"phase":"consequences","mode":"append","systems":[]},
            {"phase":"commit","mode":"append","systems":[]},
            {"phase":"projection","mode":"append","systems":[]}
          ],
          "gameplayDefinitions":[],
          "timelines":[],
          "capabilityBindings":[{"id":"counter.increment","target":"kernel.counter-increment"}]
        }"#,
    )
    .expect("counter composition");
    let manifest = decode_product_manifest(MANIFEST).expect("manifest");
    let admitted = admit_checked_product_composition(&manifest, composition).expect("admission");
    link_admitted_product_composition(
        admitted,
        &[ProductKernelCapabilityDescriptor::new(
            "counter-increment",
            CapabilityMetadata::new(
                CapabilityKind::Operation,
                CapabilityUses::INPUT_MAP,
                CapabilityAvailability::Linkable,
                CapabilityAccess::new(&[], &[]),
                product_model::CapabilityBudget::new(1_024),
                CapabilityProvenance::new("counter", "kernel/counter.rs", "increment"),
            ),
        )],
    )
    .expect("linkage")
}

fn root_with_adapter(
    config: RuntimeLifecycleConfig,
    adapter: CounterAdapter,
) -> RuntimeComposition<CounterAdapter> {
    let linked = linked();
    let input = runtime_input::CompiledInputMappings::standard(
        [runtime_input::DirectInputIntentDescriptor::new(
            "counter.increment",
            runtime_input::IntentValueKind::Digital,
        )
        .expect("input descriptor")],
        [runtime_input::RuntimeInputMapping::new(
            "counter.increment",
            "counter.increment",
            runtime_input::RuntimeInputTrigger::Key {
                code: runtime_input::KeyboardControl::Space,
                edge: runtime_input::InputEdge::Pressed,
                chord: Vec::new(),
                context: Some(InputContext::new("gameplay").expect("context")),
            },
        )
        .expect("input mapping")],
    )
    .expect("input");
    let schedule = CompiledRuntimeSchedule::compile(&linked).expect("schedule");
    let timeline = TimelineCatalog::empty();
    let mutation = CompiledMutationCatalog::compile(
        &linked,
        &[MutationCapabilityDescriptor::new(
            "counter.increment",
            "kernel.counter-increment",
            "counter",
            "counter.adapter",
            "counter.increment.v1",
        )],
    )
    .expect("mutation");
    let context = InputContext::new("gameplay").expect("context");
    let mut root = RuntimeComposition::new(
        RuntimeInstanceId::new(7),
        config,
        RuntimeCompositionInputs::new(input, schedule, timeline, mutation, context),
        adapter,
    );
    root.start().expect("start");
    root
}

fn root_with_config(config: RuntimeLifecycleConfig) -> RuntimeComposition<CounterAdapter> {
    root_with_adapter(config, CounterAdapter::new())
}

fn root() -> RuntimeComposition<CounterAdapter> {
    root_with_config(RuntimeLifecycleConfig::Demand)
}

#[test]
fn demand_counter_publishes_then_explicitly_completes_empty_step() {
    let mut root = root();
    let binding = root.input().expect("input").binding();
    root.ingest(runtime_input::RuntimeInputEvent::DirectIntent(
        runtime_input::RuntimeDirectIntentClaim::new(
            binding,
            0,
            InputContext::new("gameplay").expect("context"),
            "counter.increment",
            runtime_input::RuntimeIntentValue::Digital { active: true },
        )
        .expect("claim"),
    ))
    .expect("input claim");
    let first = root.demand_step().expect("first step");
    assert_eq!(first.step.value(), 0);
    assert!(matches!(
        first.mutation,
        runtime_composition::MutationStepReceipt::Applied(_)
    ));
    assert_eq!(root.adapter().authority.value, 1);
    assert_eq!(first.ui[0].value()["counter"], 1);

    let second = root.demand_step().expect("second step");
    assert_eq!(second.step.value(), 1);
    assert!(matches!(
        second.mutation,
        runtime_composition::MutationStepReceipt::Empty(_)
    ));
    assert_eq!(root.adapter().authority.value, 1);
    assert_eq!(second.ui[0].value()["counter"], 1);
}

#[test]
fn pause_resume_rebinds_every_lane_and_stale_ingress_fails_closed() {
    let mut root = root();
    let binding = root.input().expect("input").binding();
    root.ingest(runtime_input::RuntimeInputEvent::DirectIntent(
        runtime_input::RuntimeDirectIntentClaim::new(
            binding,
            0,
            InputContext::new("gameplay").expect("context"),
            "counter.increment",
            runtime_input::RuntimeIntentValue::Digital { active: true },
        )
        .expect("claim"),
    ))
    .expect("input claim");
    root.demand_step().expect("first step");
    let binding = root.input().expect("input").binding();
    root.pause().expect("pause");
    let stale = runtime_input::RuntimeDirectIntentClaim::new(
        binding,
        0,
        InputContext::new("gameplay").expect("context"),
        "counter.increment",
        runtime_input::RuntimeIntentValue::Digital { active: true },
    )
    .expect("claim");
    assert!(root
        .ingest(runtime_input::RuntimeInputEvent::DirectIntent(stale))
        .is_err());
    root.resume().expect("resume");
    let second = root.demand_step().expect("step after resume");
    assert_eq!(second.step.value(), 1);
}

#[test]
fn projection_failure_faults_and_restart_rebuilds_fresh_cursors() {
    let mut root = root_with_adapter(
        RuntimeLifecycleConfig::Demand,
        CounterAdapter::failing_projection(),
    );
    let error = root.demand_step().expect_err("projection failure");
    assert!(matches!(
        error,
        runtime_composition::RuntimeCompositionError::Adapter("projection failed")
    ));
    assert_eq!(
        root.lifecycle().state(),
        runtime_lifecycle::RuntimeState::Faulted
    );
    assert!(matches!(
        root.demand_step(),
        Err(runtime_composition::RuntimeCompositionError::TerminalFailure)
    ));
    root.restart().expect("fresh generation");
    let binding = root.input().expect("fresh input").binding();
    root.ingest(runtime_input::RuntimeInputEvent::DirectIntent(
        runtime_input::RuntimeDirectIntentClaim::new(
            binding,
            0,
            InputContext::new("gameplay").expect("context"),
            "counter.increment",
            runtime_input::RuntimeIntentValue::Digital { active: true },
        )
        .expect("claim"),
    ))
    .expect("fresh claim");
    let step = root.demand_step().expect("fresh step");
    assert_eq!(step.step.value(), 0);
    assert_eq!(root.adapter().authority.value, 1);
}

#[test]
fn explicit_fault_is_terminal_but_shutdown_remains_available() {
    let mut root = root();
    root.report_fault().expect("report fault");
    assert_eq!(
        root.lifecycle().state(),
        runtime_lifecycle::RuntimeState::Faulted
    );
    assert!(matches!(
        root.demand_step(),
        Err(runtime_composition::RuntimeCompositionError::TerminalFailure)
    ));
    root.shutdown().expect("shutdown faulted composition");
    assert_eq!(
        root.lifecycle().state(),
        runtime_lifecycle::RuntimeState::Shutdown
    );
    assert!(matches!(
        root.report_fault(),
        Err(runtime_composition::RuntimeCompositionError::Disposed)
    ));
}

#[test]
fn external_ordering_and_realtime_zero_or_multiple_admissions_are_typed() {
    let mut external = root_with_config(RuntimeLifecycleConfig::External);
    let first = external.external_step(runtime_lifecycle::ExternalStep::new(0));
    assert!(first.is_ok());
    assert!(matches!(
        external.external_step(runtime_lifecycle::ExternalStep::new(2)),
        Err(runtime_composition::RuntimeCompositionError::Lifecycle(
            runtime_lifecycle::RuntimeLifecycleError::ExternalStepOutOfOrder { .. }
        ))
    ));

    let mut realtime = root_with_config(RuntimeLifecycleConfig::Realtime(
        runtime_lifecycle::RealtimeLifecycleConfig::new(60, 4).expect("realtime"),
    ));
    assert!(realtime
        .advance_realtime(runtime_lifecycle::HostMonotonicTime::from_nanoseconds(0))
        .expect("baseline")
        .is_empty());
    assert_eq!(
        realtime
            .advance_realtime(runtime_lifecycle::HostMonotonicTime::from_nanoseconds(
                16_666_667
            ))
            .expect("one step")
            .len(),
        1
    );
    assert_eq!(
        realtime
            .advance_realtime(runtime_lifecycle::HostMonotonicTime::from_nanoseconds(
                1_000_000_000
            ))
            .expect("bounded catchup")
            .len(),
        4
    );
}
