use render_model::RenderFrameDiff;
use render_presentation::PresentationFrameDiff;
use runtime_input::{
    CompiledInputMappings, InputContext, InputFrame, RuntimeInputEvent, RuntimeInputLane,
    RuntimeIntentEnvelope,
};
use runtime_lifecycle::{
    ExternalStep, HostMonotonicTime, LifecycleReceipt, RuntimeInstanceId, RuntimeLifecycle,
    RuntimeLifecycleConfig, RuntimeMode, RuntimePhaseToken, RuntimeState, SimulationStep,
    SimulationStepAdmission,
};
use runtime_mutation::{
    CompiledMutationCatalog, EmptyMutationStepReceipt, MutationReceipt, RuntimeMutation,
};
use runtime_schedule::{CompiledRuntimeSchedule, RuntimeSchedule, ScheduleDispatcher};
use runtime_timeline::{
    RuntimeTimeline, TimelineCatalog, TimelineCompletionAdmission, TimelineCompletionEnvelope,
    TimelineRelease, MAX_TIMELINE_RELEASE_PREFIX,
};
use runtime_ui::{RuntimeUiProjection, RuntimeUiProjectionEnvelope};
use serde::Serialize;

use crate::{
    adapter::{ProductRuntimeAdapter, ProductRuntimeUi, MAX_PRODUCT_RUNTIME_TIMELINE_REQUESTS},
    error::{RuntimeCompositionBindError, RuntimeCompositionError},
};

/// The result of one mutation phase. Empty completion is a first-class
/// receipt, not an omitted step.
#[derive(Debug, Clone, PartialEq)]
pub enum MutationStepReceipt<G, E> {
    Applied(MutationReceipt<G, E>),
    Empty(EmptyMutationStepReceipt),
}

/// Immutable compiled inputs retained by a Created composition until each
/// instance lane is bound. Keeping these in one value also makes generated
/// product constructors auditable and prevents accidental catalog omission.
#[derive(Debug, Clone)]
pub struct RuntimeCompositionInputs {
    pub input_mappings: CompiledInputMappings,
    pub schedule: CompiledRuntimeSchedule,
    pub timeline: TimelineCatalog,
    pub mutation: CompiledMutationCatalog,
    pub input_context: InputContext,
}

impl RuntimeCompositionInputs {
    pub fn new(
        input_mappings: CompiledInputMappings,
        schedule: CompiledRuntimeSchedule,
        timeline: TimelineCatalog,
        mutation: CompiledMutationCatalog,
        input_context: InputContext,
    ) -> Self {
        Self {
            input_mappings,
            schedule,
            timeline,
            mutation,
            input_context,
        }
    }
}

/// All validated outputs and lane evidence produced by one complete step.
#[derive(Debug, PartialEq)]
pub struct RuntimeCompositionStep<O, G, E> {
    pub step: SimulationStep,
    pub input: InputFrame,
    pub intents: Vec<RuntimeIntentEnvelope>,
    pub schedule_outputs: Vec<O>,
    pub timeline: TimelineRelease,
    pub mutation: MutationStepReceipt<G, E>,
    pub ui: Vec<RuntimeUiProjectionEnvelope>,
    pub render: Option<RenderFrameDiff>,
    pub presentation: Option<PresentationFrameDiff>,
}

pub type ProductRuntimeStep<A> = RuntimeCompositionStep<
    <A as ProductRuntimeAdapter>::ScheduleOutput,
    <A as ProductRuntimeAdapter>::Guard,
    <A as ProductRuntimeAdapter>::Evidence,
>;

/// One Engine-owned runtime composition root.
///
/// Before [`Self::start`], this value owns only the immutable compiled lane
/// inputs and a Created lifecycle. Start binds every lane at once. After
/// start, product state and the adapter remain inside this same owner; callers
/// cannot swap an authority under a live mutation cursor.
pub struct RuntimeComposition<A>
where
    A: ProductRuntimeAdapter,
    A::Authority: runtime_mutation::MutationAuthority<Guard = A::Guard>,
    A::Guard: Clone,
    A::Evidence: Clone,
{
    lifecycle: RuntimeLifecycle,
    input_mappings: Option<CompiledInputMappings>,
    schedule_definition: Option<CompiledRuntimeSchedule>,
    timeline_definition: Option<TimelineCatalog>,
    mutation_definition: Option<CompiledMutationCatalog>,
    input: Option<RuntimeInputLane>,
    schedule: Option<RuntimeSchedule>,
    timeline: Option<RuntimeTimeline>,
    mutation: Option<RuntimeMutation<A::Authority, A::Evidence>>,
    ui: Option<RuntimeUiProjection>,
    input_context: InputContext,
    adapter: A,
    failed: bool,
    disposed: bool,
}

impl<A> RuntimeComposition<A>
where
    A: ProductRuntimeAdapter,
    A::Authority: runtime_mutation::MutationAuthority<Guard = A::Guard>,
    A::Guard: Clone,
    A::Evidence: Clone,
{
    /// Creates a Created composition. Compiled lane definitions are retained
    /// as immutable inputs until `start` binds their instance cursors.
    pub fn new(
        instance_id: RuntimeInstanceId,
        config: RuntimeLifecycleConfig,
        inputs: RuntimeCompositionInputs,
        adapter: A,
    ) -> Self {
        Self {
            lifecycle: RuntimeLifecycle::new(instance_id, config),
            input_mappings: Some(inputs.input_mappings),
            schedule_definition: Some(inputs.schedule),
            timeline_definition: Some(inputs.timeline),
            mutation_definition: Some(inputs.mutation),
            input: None,
            schedule: None,
            timeline: None,
            mutation: None,
            ui: None,
            input_context: inputs.input_context,
            adapter,
            failed: false,
            disposed: false,
        }
    }

    /// Starts the Created lifecycle and binds every lane against its first
    /// generation. A failed bind leaves the composition terminally failed.
    pub fn start(&mut self) -> Result<LifecycleReceipt, RuntimeCompositionBindError> {
        if self.disposed {
            return Err(RuntimeCompositionBindError::Lifecycle(
                runtime_lifecycle::RuntimeLifecycleError::WrongState {
                    operation: runtime_lifecycle::LifecycleOperation::Start,
                    state: RuntimeState::Shutdown,
                },
            ));
        }
        let receipt = self
            .lifecycle
            .start()
            .map_err(RuntimeCompositionBindError::Lifecycle)?;
        if let Err(error) = self.bind_running_lanes() {
            self.mark_failed();
            return Err(error);
        }
        Ok(receipt)
    }

    /// Focused-test/helper path for a lifecycle already started by an owning
    /// host. Normal product hosts should use `new` then `start`.
    pub fn from_running(
        lifecycle: RuntimeLifecycle,
        inputs: RuntimeCompositionInputs,
        adapter: A,
    ) -> Result<Self, RuntimeCompositionBindError> {
        let mut root = Self {
            lifecycle,
            input_mappings: Some(inputs.input_mappings),
            schedule_definition: Some(inputs.schedule),
            timeline_definition: Some(inputs.timeline),
            mutation_definition: Some(inputs.mutation),
            input: None,
            schedule: None,
            timeline: None,
            mutation: None,
            ui: None,
            input_context: inputs.input_context,
            adapter,
            failed: false,
            disposed: false,
        };
        root.bind_running_lanes()?;
        Ok(root)
    }

    pub fn lifecycle(&self) -> &RuntimeLifecycle {
        &self.lifecycle
    }

    pub fn adapter(&self) -> &A {
        &self.adapter
    }

    pub fn mode(&self) -> RuntimeMode {
        self.lifecycle.mode()
    }

    pub fn input(&self) -> Option<&RuntimeInputLane> {
        self.input.as_ref()
    }

    pub fn schedule(&self) -> Option<&RuntimeSchedule> {
        self.schedule.as_ref()
    }

    pub fn timeline(&self) -> Option<&RuntimeTimeline> {
        self.timeline.as_ref()
    }

    pub fn mutation(&self) -> Option<&RuntimeMutation<A::Authority, A::Evidence>> {
        self.mutation.as_ref()
    }

    pub fn ui(&self) -> Option<&RuntimeUiProjection> {
        self.ui.as_ref()
    }

    pub fn ingest(
        &mut self,
        event: RuntimeInputEvent,
    ) -> Result<(), RuntimeCompositionError<A::Error>> {
        self.ensure_usable()?;
        if self.lifecycle.state() != RuntimeState::Running {
            return Err(RuntimeCompositionError::Lifecycle(
                runtime_lifecycle::RuntimeLifecycleError::WrongState {
                    operation: runtime_lifecycle::LifecycleOperation::ValidateSimulationToken,
                    state: self.lifecycle.state(),
                },
            ));
        }
        let expected_binding = runtime_input::RuntimeInputBinding::new(
            self.lifecycle.instance_id(),
            self.lifecycle.generation(),
            self.lifecycle.control_revision(),
        );
        if event.runtime() != expected_binding {
            return Err(RuntimeCompositionError::Input(
                runtime_input::RuntimeInputError::BindingMismatch,
            ));
        }
        self.input
            .as_mut()
            .ok_or(RuntimeCompositionError::NotStarted)?
            .ingest(event)
            .map_err(RuntimeCompositionError::Input)
    }

    /// Queues a typed completion without re-entering a simulation phase.
    pub fn admit_timeline_completion(
        &mut self,
        envelope: TimelineCompletionEnvelope,
    ) -> Result<TimelineCompletionAdmission, RuntimeCompositionError<A::Error>> {
        self.ensure_usable()?;
        self.timeline
            .as_mut()
            .ok_or(RuntimeCompositionError::NotStarted)?
            .admit_completion(&self.lifecycle, envelope)
            .map_err(RuntimeCompositionError::Timeline)
    }

    /// Runs exactly one Demand step through the complete five-phase lane
    /// order. Any failure after admission terminally fails this composition;
    /// callers cannot accidentally retry against a partially advanced lane.
    pub fn demand_step(
        &mut self,
    ) -> Result<ProductRuntimeStep<A>, RuntimeCompositionError<A::Error>>
    where
        A::UiOutput: Serialize,
    {
        self.ensure_usable()?;
        let admission = self
            .lifecycle
            .admit_demand_step()
            .map_err(RuntimeCompositionError::Lifecycle)?;
        let step = admission.step_at(0).expect("one demand step is present");
        self.finish_step(step)
    }

    /// Runs one deterministic externally numbered step.
    pub fn external_step(
        &mut self,
        external: ExternalStep,
    ) -> Result<ProductRuntimeStep<A>, RuntimeCompositionError<A::Error>>
    where
        A::UiOutput: Serialize,
    {
        self.ensure_usable()?;
        let admission = self
            .lifecycle
            .admit_external_step(external)
            .map_err(RuntimeCompositionError::Lifecycle)?;
        let step = admission.step_at(0).expect("one external step is present");
        self.finish_step(step)
    }

    /// Runs all fixed steps admitted from a host monotonic time reading.
    pub fn advance_realtime(
        &mut self,
        observed: HostMonotonicTime,
    ) -> Result<Vec<ProductRuntimeStep<A>>, RuntimeCompositionError<A::Error>>
    where
        A::UiOutput: Serialize,
    {
        self.ensure_usable()?;
        let advance = self
            .lifecycle
            .advance_realtime(observed)
            .map_err(RuntimeCompositionError::Lifecycle)?;
        let Some(admission) = advance.simulation() else {
            return Ok(Vec::new());
        };
        (0..admission.step_count())
            .map(|offset| {
                let step = admission.step_at(offset).expect("admitted offset is valid");
                self.finish_step(step)
            })
            .collect::<Result<Vec<_>, _>>()
    }

    /// Pauses admission. Resume explicitly rebinds all lanes before allowing
    /// the next step, so old tokens cannot cross the control revision.
    pub fn pause(&mut self) -> Result<LifecycleReceipt, RuntimeCompositionError<A::Error>> {
        self.ensure_usable()?;
        let receipt = self
            .lifecycle
            .pause()
            .map_err(RuntimeCompositionError::Lifecycle)?;
        let binding = runtime_input::RuntimeInputBinding::new(
            self.lifecycle.instance_id(),
            self.lifecycle.generation(),
            self.lifecycle.control_revision(),
        );
        if let Err(error) = self
            .input
            .as_mut()
            .ok_or(RuntimeCompositionError::NotStarted)?
            .rebind(
                binding,
                self.input_context.clone(),
                runtime_input::InputClearReason::ControlRevisionChange,
            )
        {
            self.mark_failed();
            return Err(RuntimeCompositionError::Input(error));
        }
        Ok(receipt)
    }

    pub fn resume(&mut self) -> Result<LifecycleReceipt, RuntimeCompositionError<A::Error>> {
        self.ensure_usable()?;
        let receipt = self
            .lifecycle
            .resume()
            .map_err(RuntimeCompositionError::Lifecycle)?;
        if let Err(error) = self.rebind_running_lanes() {
            self.mark_failed();
            return Err(error);
        }
        Ok(receipt)
    }

    pub fn restart(&mut self) -> Result<LifecycleReceipt, RuntimeCompositionError<A::Error>> {
        if self.disposed {
            return Err(RuntimeCompositionError::Disposed);
        }
        if !self.failed && self.lifecycle.state() == RuntimeState::Created {
            return Err(RuntimeCompositionError::NotStarted);
        }
        if self.failed && self.lifecycle.state() != RuntimeState::Faulted {
            return Err(RuntimeCompositionError::TerminalFailure);
        }
        let receipt = self
            .lifecycle
            .restart()
            .map_err(RuntimeCompositionError::Lifecycle)?;
        if let Err(error) = self.rebuild_running_lanes() {
            self.mark_failed();
            return Err(error);
        }
        if let Err(error) = self.adapter.rebind(&self.lifecycle) {
            self.mark_failed();
            return Err(RuntimeCompositionError::Adapter(error));
        }
        self.failed = false;
        Ok(receipt)
    }

    pub fn shutdown(&mut self) -> Result<LifecycleReceipt, RuntimeCompositionError<A::Error>> {
        if self.disposed {
            return Err(RuntimeCompositionError::Disposed);
        }
        let receipt = self
            .lifecycle
            .shutdown()
            .map_err(RuntimeCompositionError::Lifecycle)?;
        self.dispose_lanes();
        Ok(receipt)
    }

    /// Records a host- or product-reported fault and makes this composition
    /// terminal until `restart` rebuilds every lane in a new generation.
    pub fn report_fault(&mut self) -> Result<LifecycleReceipt, RuntimeCompositionError<A::Error>> {
        if self.disposed {
            return Err(RuntimeCompositionError::Disposed);
        }
        let receipt = self
            .lifecycle
            .report_fault(runtime_lifecycle::RuntimeFault::OwnerReported)
            .map_err(RuntimeCompositionError::Lifecycle)?;
        self.failed = true;
        Ok(receipt)
    }

    /// Terminally disposes every lane and the product adapter.
    pub fn dispose(&mut self) {
        if self.disposed {
            return;
        }
        self.dispose_lanes();
    }

    fn bind_running_lanes(&mut self) -> Result<(), RuntimeCompositionBindError> {
        if self.lifecycle.state() != RuntimeState::Running {
            return Err(RuntimeCompositionBindError::LifecycleNotRunning);
        }
        let binding = runtime_input::RuntimeInputBinding::new(
            self.lifecycle.instance_id(),
            self.lifecycle.generation(),
            self.lifecycle.control_revision(),
        );
        let input_mappings = self
            .input_mappings
            .as_ref()
            .expect("lane definitions are present before first bind")
            .clone();
        let schedule_definition = self
            .schedule_definition
            .as_ref()
            .expect("lane definitions are present before first bind")
            .clone();
        let timeline_definition = self
            .timeline_definition
            .as_ref()
            .expect("lane definitions are present before first bind")
            .clone();
        let mutation_definition = self
            .mutation_definition
            .as_ref()
            .expect("lane definitions are present before first bind")
            .clone();
        self.input = Some(RuntimeInputLane::new(
            input_mappings,
            binding,
            self.input_context.clone(),
        ));
        self.schedule = Some(
            schedule_definition
                .bind(&self.lifecycle)
                .map_err(RuntimeCompositionBindError::Schedule)?,
        );
        self.timeline = Some(
            timeline_definition
                .bind(&self.lifecycle)
                .map_err(RuntimeCompositionBindError::Timeline)?,
        );
        self.mutation = Some(
            RuntimeMutation::bind(mutation_definition, &self.lifecycle)
                .map_err(RuntimeCompositionBindError::Mutation)?,
        );
        self.ui = Some(
            RuntimeUiProjection::bind(&self.lifecycle).map_err(RuntimeCompositionBindError::Ui)?,
        );
        Ok(())
    }

    fn rebind_running_lanes(&mut self) -> Result<(), RuntimeCompositionError<A::Error>> {
        let old_generation = self
            .input
            .as_ref()
            .ok_or(RuntimeCompositionError::NotStarted)?
            .binding()
            .generation();
        let reason = if self.lifecycle.generation() != old_generation {
            runtime_input::InputClearReason::Restart
        } else {
            runtime_input::InputClearReason::ControlRevisionChange
        };
        let binding = runtime_input::RuntimeInputBinding::new(
            self.lifecycle.instance_id(),
            self.lifecycle.generation(),
            self.lifecycle.control_revision(),
        );
        self.input
            .as_mut()
            .ok_or(RuntimeCompositionError::NotStarted)?
            .rebind(binding, self.input_context.clone(), reason)
            .map_err(RuntimeCompositionError::Input)?;
        self.schedule
            .as_mut()
            .ok_or(RuntimeCompositionError::NotStarted)?
            .rebind(&self.lifecycle)
            .map_err(RuntimeCompositionError::ScheduleStatic)?;
        self.timeline
            .as_mut()
            .ok_or(RuntimeCompositionError::NotStarted)?
            .rebind(&self.lifecycle)
            .map_err(RuntimeCompositionError::Timeline)?;
        self.mutation
            .as_mut()
            .ok_or(RuntimeCompositionError::NotStarted)?
            .rebind(&self.lifecycle)
            .map_err(RuntimeCompositionError::MutationStatic)?;
        self.ui
            .as_mut()
            .ok_or(RuntimeCompositionError::NotStarted)?
            .rebind(&self.lifecycle)
            .map_err(RuntimeCompositionError::Ui)?;
        self.adapter
            .rebind(&self.lifecycle)
            .map_err(RuntimeCompositionError::Adapter)
    }

    fn rebuild_running_lanes(&mut self) -> Result<(), RuntimeCompositionError<A::Error>> {
        if self.lifecycle.state() != RuntimeState::Running {
            return Err(RuntimeCompositionError::Lifecycle(
                runtime_lifecycle::RuntimeLifecycleError::WrongState {
                    operation: runtime_lifecycle::LifecycleOperation::Restart,
                    state: self.lifecycle.state(),
                },
            ));
        }
        let binding = runtime_input::RuntimeInputBinding::new(
            self.lifecycle.instance_id(),
            self.lifecycle.generation(),
            self.lifecycle.control_revision(),
        );
        let input = RuntimeInputLane::new(
            self.input_mappings
                .as_ref()
                .expect("input definition retained")
                .clone(),
            binding,
            self.input_context.clone(),
        );
        let schedule = self
            .schedule_definition
            .as_ref()
            .expect("schedule definition retained")
            .bind(&self.lifecycle)
            .map_err(RuntimeCompositionError::ScheduleStatic)?;
        let timeline = self
            .timeline_definition
            .as_ref()
            .expect("timeline definition retained")
            .bind(&self.lifecycle)
            .map_err(RuntimeCompositionError::Timeline)?;
        let mutation = RuntimeMutation::bind(
            self.mutation_definition
                .as_ref()
                .expect("mutation definition retained")
                .clone(),
            &self.lifecycle,
        )
        .map_err(RuntimeCompositionError::MutationStatic)?;
        let ui = RuntimeUiProjection::bind(&self.lifecycle).map_err(RuntimeCompositionError::Ui)?;
        self.input = Some(input);
        self.schedule = Some(schedule);
        self.timeline = Some(timeline);
        self.mutation = Some(mutation);
        self.ui = Some(ui);
        Ok(())
    }

    fn dispose_lanes(&mut self) {
        if let Some(input) = &mut self.input {
            input.dispose();
        }
        if let Some(schedule) = &mut self.schedule {
            schedule.dispose();
        }
        if let Some(timeline) = &mut self.timeline {
            timeline.dispose();
        }
        if let Some(mutation) = &mut self.mutation {
            mutation.dispose();
        }
        if let Some(ui) = &mut self.ui {
            ui.dispose();
        }
        self.adapter.dispose();
        self.disposed = true;
    }

    fn ensure_usable(&self) -> Result<(), RuntimeCompositionError<A::Error>> {
        if self.disposed {
            return Err(RuntimeCompositionError::Disposed);
        }
        if self.failed {
            return Err(RuntimeCompositionError::TerminalFailure);
        }
        if self.lifecycle.state() == RuntimeState::Created {
            return Err(RuntimeCompositionError::NotStarted);
        }
        Ok(())
    }

    fn mark_failed(&mut self) {
        self.failed = true;
        if matches!(
            self.lifecycle.state(),
            RuntimeState::Running | RuntimeState::Paused
        ) {
            let _ = self
                .lifecycle
                .report_fault(runtime_lifecycle::RuntimeFault::OwnerReported);
        }
    }

    fn finish_step(
        &mut self,
        admission: SimulationStepAdmission,
    ) -> Result<ProductRuntimeStep<A>, RuntimeCompositionError<A::Error>>
    where
        A::UiOutput: Serialize,
    {
        let result = self.finish_step_inner(admission);
        if result.is_err() {
            self.mark_failed();
        }
        result
    }

    fn finish_step_inner(
        &mut self,
        admission: SimulationStepAdmission,
    ) -> Result<ProductRuntimeStep<A>, RuntimeCompositionError<A::Error>>
    where
        A::UiOutput: Serialize,
    {
        let phases = admission.phases();
        let (input, intents) = self
            .input
            .as_mut()
            .ok_or(RuntimeCompositionError::NotStarted)?
            .snapshot_for_step(&self.lifecycle, phases.input_snapshot())
            .map_err(RuntimeCompositionError::Input)?;
        self.adapter
            .on_input(&input, &intents)
            .map_err(RuntimeCompositionError::Adapter)?;

        let mut schedule_outputs = Vec::new();
        schedule_outputs.extend(self.execute_phase(phases.input_snapshot())?);
        schedule_outputs.extend(self.execute_phase(phases.schedule())?);

        let timeline_requests = self
            .adapter
            .prepare_timeline(admission.token().step())
            .map_err(RuntimeCompositionError::Adapter)?;
        if timeline_requests.len() > MAX_PRODUCT_RUNTIME_TIMELINE_REQUESTS {
            return Err(RuntimeCompositionError::Timeline(
                runtime_timeline::RuntimeTimelineError::BoundsExceeded("product timeline requests"),
            ));
        }
        self.timeline
            .as_mut()
            .ok_or(RuntimeCompositionError::NotStarted)?
            .schedule_batch(&self.lifecycle, phases.timeline(), timeline_requests)
            .map_err(RuntimeCompositionError::Timeline)?;

        let releases = self
            .timeline
            .as_mut()
            .ok_or(RuntimeCompositionError::NotStarted)?
            .release_due(
                &self.lifecycle,
                phases.timeline(),
                MAX_TIMELINE_RELEASE_PREFIX,
            )
            .map_err(RuntimeCompositionError::Timeline)?;
        self.adapter
            .on_timeline_releases(&releases)
            .map_err(RuntimeCompositionError::Adapter)?;
        schedule_outputs.extend(self.execute_phase(phases.timeline())?);
        schedule_outputs.extend(self.execute_phase(phases.mutation())?);

        let mutation = match self
            .adapter
            .prepare_mutation(admission.token().step())
            .map_err(RuntimeCompositionError::Adapter)?
        {
            Some(batch) => {
                let (authority, planner) = self.adapter.mutation_parts();
                self.mutation
                    .as_mut()
                    .ok_or(RuntimeCompositionError::NotStarted)?
                    .apply_batch(
                        &self.lifecycle,
                        phases.mutation(),
                        authority,
                        planner,
                        batch,
                    )
                    .map(MutationStepReceipt::Applied)
                    .map_err(RuntimeCompositionError::Mutation)?
            }
            None => self
                .mutation
                .as_mut()
                .ok_or(RuntimeCompositionError::NotStarted)?
                .complete_empty_step(&self.lifecycle, phases.mutation())
                .map(MutationStepReceipt::Empty)
                .map_err(RuntimeCompositionError::MutationStatic)?,
        };

        schedule_outputs.extend(self.execute_phase(phases.projection())?);
        let outputs = self
            .adapter
            .project(&self.lifecycle, phases.projection())
            .map_err(RuntimeCompositionError::Adapter)?;
        let (ui_output, render, presentation) = outputs.into_parts();
        if let Some(frame) = &render {
            frame.validate().map_err(RuntimeCompositionError::Render)?;
        }
        if let Some(frame) = &presentation {
            frame
                .validate()
                .map_err(RuntimeCompositionError::Presentation)?;
        }
        let mut ui = Vec::with_capacity(ui_output.len());
        for (stream, contract, value) in ui_output.into_iter().map(ProductRuntimeUi::into_parts) {
            let value = serde_json::to_value(value).map_err(RuntimeCompositionError::UiEncoding)?;
            ui.push(
                self.ui
                    .as_mut()
                    .ok_or(RuntimeCompositionError::NotStarted)?
                    .emit_value(
                        &self.lifecycle,
                        phases.projection(),
                        stream,
                        contract,
                        value,
                    )
                    .map_err(RuntimeCompositionError::Ui)?,
            );
        }
        Ok(RuntimeCompositionStep {
            step: admission.token().step(),
            input,
            intents,
            schedule_outputs,
            timeline: releases,
            mutation,
            ui,
            render,
            presentation,
        })
    }

    fn execute_phase(
        &mut self,
        token: RuntimePhaseToken,
    ) -> Result<Vec<A::ScheduleOutput>, RuntimeCompositionError<A::Error>> {
        let lifecycle = &self.lifecycle;
        let adapter = &mut self.adapter;
        let dispatcher = &mut AdapterDispatcher {
            adapter,
            lifecycle,
            token,
        };
        self.schedule
            .as_mut()
            .ok_or(RuntimeCompositionError::NotStarted)?
            .execute_phase(lifecycle, token, &(), dispatcher)
            .map(|receipt| receipt.into_outputs())
            .map_err(RuntimeCompositionError::Schedule)
    }
}

struct AdapterDispatcher<'a, A: ProductRuntimeAdapter> {
    adapter: &'a mut A,
    lifecycle: &'a RuntimeLifecycle,
    token: RuntimePhaseToken,
}

impl<A: ProductRuntimeAdapter> ScheduleDispatcher<()> for AdapterDispatcher<'_, A> {
    type Output = A::ScheduleOutput;
    type Error = A::Error;

    fn dispatch(
        &mut self,
        invocation: runtime_schedule::ScheduleSystemInvocation<'_>,
        _context: &(),
    ) -> Result<Self::Output, Self::Error> {
        self.adapter
            .dispatch_schedule(invocation, self.lifecycle, self.token)
    }
}
