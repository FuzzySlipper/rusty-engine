use runtime_lifecycle::{RuntimeLifecycle, RuntimePhase, RuntimePhaseToken, SimulationStep};

use crate::{
    model::{validate_controller_axis, InputFrameFacts},
    AxisValue, ButtonSnapshot, CompiledInputMappings, ControllerAxis, ControllerButton, InputAxis,
    InputClearReason, InputContext, InputEdge, InputFrame, IntentPhase, IntentProvenance,
    IntentValueKind, KeyboardControl, PhysicalEdge, PointerButton, RuntimeDirectIntentClaim,
    RuntimeInputBatchReceipt, RuntimeInputBinding, RuntimeInputError, RuntimeInputEvent,
    RuntimeInputFact, RuntimeInputIngress, RuntimeInputTrigger, RuntimeIntentEnvelope,
    RuntimeIntentValue, MAX_PENDING_INGRESS,
};

#[derive(Debug, Clone, Copy, Default)]
struct ButtonState {
    held: bool,
    pressed: bool,
    released: bool,
}

impl ButtonState {
    fn apply(&mut self, edge: PhysicalEdge) {
        match edge {
            PhysicalEdge::Pressed if !self.held => {
                self.held = true;
                self.pressed = true;
            }
            PhysicalEdge::Pressed => {}
            PhysicalEdge::Released if self.held => {
                self.held = false;
                self.released = true;
            }
            PhysicalEdge::Released => {}
        }
    }
    fn clear_transient(&mut self) {
        self.pressed = false;
        self.released = false;
    }
}

#[derive(Debug, Clone)]
struct PendingIntent {
    sequence: u64,
    order: usize,
    descriptor: crate::CompiledInputIntent,
    value: RuntimeIntentValue,
    phase: IntentPhase,
    provenance: IntentProvenance,
}

#[derive(Debug, Clone)]
struct InputLaneCheckpoint {
    binding: RuntimeInputBinding,
    context: InputContext,
    last_sequence: Option<u64>,
    last_snapshot_step: Option<u64>,
    keyboard: Vec<(KeyboardControl, ButtonState)>,
    pointer_buttons: Vec<(PointerButton, ButtonState)>,
    controller_buttons: Vec<(ControllerButton, ButtonState)>,
    pointer: (AxisValue, AxisValue),
    wheel: (AxisValue, AxisValue),
    controller_axes: Vec<(ControllerAxis, AxisValue)>,
    mapping_active: Vec<bool>,
    pending_intents: Vec<PendingIntent>,
    disposed: bool,
}

type OrderedEnvelope = (RuntimeIntentEnvelope, (u8, usize));

/// One explicit input owner per runtime instance. It captures normalized facts,
/// creates one deterministic snapshot for an admitted simulation step, and
/// emits immutable semantic intents. It never owns a clock or consequences.
/// It is deliberately not `Clone`: duplicating sequence/held state would
/// create competing authorities for one runtime binding.
#[derive(Debug)]
pub struct RuntimeInputLane {
    mappings: CompiledInputMappings,
    binding: RuntimeInputBinding,
    context: InputContext,
    last_sequence: Option<u64>,
    last_snapshot_step: Option<u64>,
    keyboard: Vec<(KeyboardControl, ButtonState)>,
    pointer_buttons: Vec<(PointerButton, ButtonState)>,
    controller_buttons: Vec<(ControllerButton, ButtonState)>,
    pointer: (AxisValue, AxisValue),
    wheel: (AxisValue, AxisValue),
    controller_axes: Vec<(ControllerAxis, AxisValue)>,
    mapping_active: Vec<bool>,
    pending_intents: Vec<PendingIntent>,
    disposed: bool,
}

impl RuntimeInputLane {
    pub fn new(
        mappings: CompiledInputMappings,
        binding: RuntimeInputBinding,
        context: InputContext,
    ) -> Self {
        let zero = AxisValue::new(0.0).expect("zero is always a valid axis value");
        let mapping_count = mappings.mappings().len();
        Self {
            mappings,
            binding,
            context,
            last_sequence: None,
            last_snapshot_step: None,
            keyboard: Vec::new(),
            pointer_buttons: Vec::new(),
            controller_buttons: Vec::new(),
            pointer: (zero, zero),
            wheel: (zero, zero),
            controller_axes: Vec::new(),
            mapping_active: vec![false; mapping_count],
            pending_intents: Vec::new(),
            disposed: false,
        }
    }

    pub const fn binding(&self) -> RuntimeInputBinding {
        self.binding
    }
    pub fn context(&self) -> &InputContext {
        &self.context
    }
    pub fn last_sequence(&self) -> Option<u64> {
        self.last_sequence
    }

    /// Ingests an ordered batch as one lane transaction. A same-binding,
    /// same-context event whose sequence is already behind the lane cursor is
    /// a safe stale/duplicate observation and is dropped without changing
    /// held or pending state. Any other error restores the complete
    /// pre-batch checkpoint, so callers never forward a successfully ingested
    /// prefix while reporting a failed batch.
    pub fn ingest_batch(
        &mut self,
        events: &[RuntimeInputEvent],
    ) -> Result<RuntimeInputBatchReceipt, RuntimeInputError> {
        let checkpoint = self.checkpoint();
        let mut accepted_count = 0;
        let mut dropped_count = 0;
        let mut accepted_through: Option<u64> = None;
        let mut consumed_through: Option<u64> = None;
        let mut accepted_indices = Vec::with_capacity(events.len());

        for (index, event) in events.iter().enumerate() {
            if self.is_safe_stale_duplicate(event) {
                dropped_count += 1;
                consumed_through = Some(
                    consumed_through.map_or(event.sequence(), |value| value.max(event.sequence())),
                );
                continue;
            }
            if let Err(error) = self.ingest(event.clone()) {
                self.restore(checkpoint);
                return Err(error);
            }
            accepted_count += 1;
            accepted_indices.push(index);
            accepted_through = Some(event.sequence());
            consumed_through = Some(
                consumed_through.map_or(event.sequence(), |value| value.max(event.sequence())),
            );
        }

        Ok(RuntimeInputBatchReceipt::new(
            events.len(),
            accepted_count,
            dropped_count,
            accepted_through,
            consumed_through,
            expected_sequence(self.last_sequence).ok(),
            accepted_indices,
        ))
    }

    /// Accepts one host-normalized fact or direct product UI claim. The host
    /// must provide a gap-free sequence for the active binding. Any mismatch
    /// clears state before returning an error, so stale held input cannot leak.
    pub fn ingest(&mut self, event: RuntimeInputEvent) -> Result<(), RuntimeInputError> {
        if self.disposed {
            return Err(RuntimeInputError::Disposed);
        }
        if event.runtime() != self.binding {
            return self.rebind_event(event);
        }
        let expected = match expected_sequence(self.last_sequence) {
            Ok(value) => value,
            Err(error) => {
                self.clear_state();
                return Err(error);
            }
        };
        if event.sequence() != expected {
            self.clear_state();
            return Err(RuntimeInputError::SequenceOutOfOrder {
                expected,
                received: event.sequence(),
            });
        }
        // A product context transition is itself an ordered physical loss fact.
        // It is the sole event allowed to carry the new context while retaining
        // the current runtime binding; every other fact must name the current
        // context exactly.
        let is_context_transition = matches!(
            &event,
            RuntimeInputEvent::Physical(ingress)
                if matches!(ingress.fact(), RuntimeInputFact::Clear { reason: InputClearReason::InteractionModeLoss })
        );
        if event.context() != &self.context && !is_context_transition {
            self.clear_state();
            return Err(RuntimeInputError::BindingMismatch);
        }
        self.last_sequence = Some(event.sequence());
        let result = match event {
            RuntimeInputEvent::Physical(ingress) => self.ingest_physical(ingress),
            RuntimeInputEvent::DirectIntent(claim) => self.ingest_direct(claim),
        };
        if result.is_err() {
            self.clear_state();
        }
        result
    }

    /// Explicitly moves this lane to a newer lifecycle binding and clears
    /// every held, pending, and transient fact. Composition roots call this
    /// after pause/resume or restart; it is the same strict sequence-zero
    /// clear path used by a host-provided rebind ingress.
    pub fn rebind(
        &mut self,
        binding: RuntimeInputBinding,
        context: InputContext,
        reason: InputClearReason,
    ) -> Result<(), RuntimeInputError> {
        self.rebind_event(RuntimeInputEvent::Physical(RuntimeInputIngress::new(
            binding,
            0,
            context,
            RuntimeInputFact::Clear { reason },
        )))
    }

    /// Terminally disposes this instance-owned lane. No queued, held, edge,
    /// pointer, wheel, or controller fact survives, and later ingest/snapshot
    /// attempts fail instead of accidentally reviving a discarded instance.
    pub fn dispose(&mut self) {
        self.clear_state();
        self.disposed = true;
    }

    /// Forms exactly one snapshot and its mapped/direct semantic envelopes for
    /// one caller-admitted simulation step. Edge and axis mappings retain the
    /// raw ingress sequence; held mappings are emitted once per step at the
    /// latest admitted sequence. Equal-sequence physical mappings use authored
    /// map order, so one aggregate frame never erases input chronology.
    pub fn snapshot_for_step(
        &mut self,
        lifecycle: &RuntimeLifecycle,
        token: RuntimePhaseToken,
    ) -> Result<(InputFrame, Vec<RuntimeIntentEnvelope>), RuntimeInputError> {
        if self.disposed {
            return Err(RuntimeInputError::Disposed);
        }
        if token.phase() != RuntimePhase::InputSnapshot {
            return Err(RuntimeInputError::WrongSnapshotPhase);
        }
        if lifecycle
            .validate_phase_token(token, RuntimePhase::InputSnapshot)
            .is_err()
        {
            self.clear_state();
            return Err(RuntimeInputError::LifecycleValidation);
        }
        let simulation = token.simulation();
        if simulation.instance_id() != self.binding.instance_id()
            || simulation.generation() != self.binding.generation()
            || simulation.control_revision() != self.binding.control_revision()
        {
            self.clear_state();
            return Err(RuntimeInputError::BindingMismatch);
        }
        let simulation_step = simulation.step();
        if self
            .last_snapshot_step
            .is_some_and(|previous| simulation_step.value() <= previous)
        {
            return Err(RuntimeInputError::SnapshotOutOfOrder);
        }
        let frame = self.frame(simulation_step);
        let mut paired =
            Vec::with_capacity(self.pending_intents.len() + self.mappings.mappings().len());
        for pending in &self.pending_intents {
            paired.push((
                RuntimeIntentEnvelope::new(
                    self.binding,
                    simulation_step,
                    pending.sequence,
                    pending.descriptor.clone(),
                    pending.value.clone(),
                    pending.phase,
                    pending.provenance.clone(),
                ),
                (0_u8, pending.order),
            ));
        }
        paired.extend(self.held_envelopes(&frame, simulation_step)?);
        paired.sort_by(|(left, left_order), (right, right_order)| {
            left.sequence()
                .cmp(&right.sequence())
                .then(left_order.cmp(right_order))
        });
        let envelopes = paired
            .into_iter()
            .map(|(entry, _)| entry)
            .collect::<Vec<_>>();
        self.pending_intents.clear();
        self.clear_transient();
        self.last_snapshot_step = Some(simulation_step.value());
        Ok((frame, envelopes))
    }

    fn is_safe_stale_duplicate(&self, event: &RuntimeInputEvent) -> bool {
        event.runtime() == self.binding
            && event.context() == &self.context
            && expected_sequence(self.last_sequence)
                .is_ok_and(|expected| event.sequence() < expected)
    }

    fn checkpoint(&self) -> InputLaneCheckpoint {
        InputLaneCheckpoint {
            binding: self.binding,
            context: self.context.clone(),
            last_sequence: self.last_sequence,
            last_snapshot_step: self.last_snapshot_step,
            keyboard: self.keyboard.clone(),
            pointer_buttons: self.pointer_buttons.clone(),
            controller_buttons: self.controller_buttons.clone(),
            pointer: self.pointer,
            wheel: self.wheel,
            controller_axes: self.controller_axes.clone(),
            mapping_active: self.mapping_active.clone(),
            pending_intents: self.pending_intents.clone(),
            disposed: self.disposed,
        }
    }

    fn restore(&mut self, checkpoint: InputLaneCheckpoint) {
        self.binding = checkpoint.binding;
        self.context = checkpoint.context;
        self.last_sequence = checkpoint.last_sequence;
        self.last_snapshot_step = checkpoint.last_snapshot_step;
        self.keyboard = checkpoint.keyboard;
        self.pointer_buttons = checkpoint.pointer_buttons;
        self.controller_buttons = checkpoint.controller_buttons;
        self.pointer = checkpoint.pointer;
        self.wheel = checkpoint.wheel;
        self.controller_axes = checkpoint.controller_axes;
        self.mapping_active = checkpoint.mapping_active;
        self.pending_intents = checkpoint.pending_intents;
        self.disposed = checkpoint.disposed;
    }

    fn rebind_event(&mut self, event: RuntimeInputEvent) -> Result<(), RuntimeInputError> {
        let RuntimeInputEvent::Physical(ingress) = event else {
            self.clear_state();
            return Err(RuntimeInputError::BindingMismatch);
        };
        let RuntimeInputFact::Clear { reason } = ingress.fact() else {
            self.clear_state();
            return Err(RuntimeInputError::InvalidRebindClear);
        };
        if ingress.sequence() != 0 || !valid_rebind(self.binding, ingress.runtime(), *reason) {
            self.clear_state();
            return Err(RuntimeInputError::InvalidRebindClear);
        }
        self.clear_state();
        self.binding = ingress.runtime();
        self.context = ingress.context().clone();
        self.last_sequence = Some(0);
        self.last_snapshot_step = None;
        Ok(())
    }

    fn ingest_physical(&mut self, ingress: RuntimeInputIngress) -> Result<(), RuntimeInputError> {
        let terminal_dispose = matches!(
            ingress.fact(),
            RuntimeInputFact::Clear {
                reason: InputClearReason::Dispose
            }
        );
        match ingress.fact() {
            RuntimeInputFact::Clear { .. } => {
                self.clear_state();
                self.context = ingress.context().clone();
                self.last_sequence = Some(ingress.sequence());
            }
            RuntimeInputFact::Key { code, edge } => set_button(&mut self.keyboard, *code, *edge),
            RuntimeInputFact::PointerButton { button, edge } => {
                set_button(&mut self.pointer_buttons, *button, *edge)
            }
            RuntimeInputFact::ControllerButton { button, edge } => {
                set_button(&mut self.controller_buttons, *button, *edge)
            }
            RuntimeInputFact::PointerDelta { x, y } => {
                self.pointer = add_pair(self.pointer, (*x, *y))?
            }
            RuntimeInputFact::Wheel { x, y } => self.wheel = add_pair(self.wheel, (*x, *y))?,
            RuntimeInputFact::ControllerAxis { axis, value } => set_axis(
                &mut self.controller_axes,
                *axis,
                validate_controller_axis(*value)?,
            ),
        }
        if !matches!(ingress.fact(), RuntimeInputFact::Clear { .. }) {
            self.queue_physical_mappings(ingress.sequence(), ingress.fact())?;
        }
        if terminal_dispose {
            self.disposed = true;
        }
        Ok(())
    }

    fn ingest_direct(&mut self, claim: RuntimeDirectIntentClaim) -> Result<(), RuntimeInputError> {
        let descriptor = self
            .mappings
            .intent(claim.intent())
            .ok_or(RuntimeInputError::UnknownIntent)?;
        let actual = match claim.value() {
            RuntimeIntentValue::Digital { .. } => IntentValueKind::Digital,
            RuntimeIntentValue::Axis { .. } => IntentValueKind::Axis,
            RuntimeIntentValue::ProductPayload { .. } => IntentValueKind::ProductPayload,
        };
        if descriptor.value_kind() != actual {
            return Err(RuntimeInputError::IntentValueKindMismatch);
        }
        if let RuntimeIntentValue::ProductPayload { payload } = claim.value() {
            if descriptor.payload_contract() != Some(payload.contract()) {
                return Err(RuntimeInputError::ProductPayloadContractMismatch);
            }
        }
        if self.pending_intents.len() >= MAX_PENDING_INGRESS {
            self.clear_state();
            return Err(RuntimeInputError::PendingIngressOverflow);
        }
        self.pending_intents.push(PendingIntent {
            sequence: claim.sequence(),
            order: usize::MAX,
            descriptor: descriptor.clone(),
            value: claim.value().clone(),
            phase: IntentPhase::DirectUi,
            provenance: IntentProvenance::DirectUi,
        });
        Ok(())
    }

    fn frame(&self, simulation_step: SimulationStep) -> InputFrame {
        InputFrame::new(
            self.binding,
            simulation_step,
            self.context.clone(),
            InputFrameFacts {
                keyboard: snapshot_buttons(&self.keyboard),
                pointer_buttons: snapshot_buttons(&self.pointer_buttons),
                controller_buttons: snapshot_buttons(&self.controller_buttons),
                pointer: self.pointer,
                wheel: self.wheel,
                controller_axes: self.controller_axes.iter().copied().collect(),
            },
        )
    }

    fn held_envelopes(
        &self,
        frame: &InputFrame,
        simulation_step: SimulationStep,
    ) -> Result<Vec<OrderedEnvelope>, RuntimeInputError> {
        let sequence = self.last_sequence.unwrap_or(0);
        let mut output = Vec::new();
        for (index, mapping) in self.mappings.mappings().iter().enumerate() {
            if !matches!(
                mapping.trigger(),
                RuntimeInputTrigger::Key {
                    edge: InputEdge::Held,
                    ..
                } | RuntimeInputTrigger::PointerButton {
                    edge: InputEdge::Held,
                    ..
                } | RuntimeInputTrigger::ControllerButton {
                    edge: InputEdge::Held,
                    ..
                } | RuntimeInputTrigger::ControllerAxis { .. }
            ) {
                continue;
            }
            let Some((value, phase)) = trigger_value(mapping.trigger(), frame)? else {
                continue;
            };
            let descriptor = self
                .mappings
                .intent(mapping.intent())
                .ok_or(RuntimeInputError::UnknownIntent)?;
            output.push((
                RuntimeIntentEnvelope::new(
                    self.binding,
                    simulation_step,
                    sequence,
                    descriptor.clone(),
                    value,
                    phase,
                    IntentProvenance::Physical {
                        mapping_id: mapping.id().to_owned(),
                    },
                    // Held readouts are synthesized by the snapshot, never by a
                    // source ingress. They deliberately follow every source-derived
                    // envelope with the same sequence while map index keeps authored
                    // held order deterministic.
                ),
                (1, index),
            ));
        }
        Ok(output)
    }

    fn clear_state(&mut self) {
        self.keyboard.clear();
        self.pointer_buttons.clear();
        self.controller_buttons.clear();
        let zero = AxisValue::new(0.0).expect("zero is always a valid axis value");
        self.pointer = (zero, zero);
        self.wheel = (zero, zero);
        self.controller_axes.clear();
        self.mapping_active.fill(false);
        self.pending_intents.clear();
    }

    fn clear_transient(&mut self) {
        for (_, state) in &mut self.keyboard {
            state.clear_transient();
        }
        for (_, state) in &mut self.pointer_buttons {
            state.clear_transient();
        }
        for (_, state) in &mut self.controller_buttons {
            state.clear_transient();
        }
        let zero = AxisValue::new(0.0).expect("zero is always a valid axis value");
        self.pointer = (zero, zero);
        self.wheel = (zero, zero);
    }

    fn queue_physical_mappings(
        &mut self,
        sequence: u64,
        fact: &RuntimeInputFact,
    ) -> Result<(), RuntimeInputError> {
        let mappings = self.mappings.mappings().to_vec();
        for (index, mapping) in mappings.iter().enumerate() {
            let mut was_active = self.mapping_active[index];
            let result = physical_trigger_value(mapping.trigger(), fact, self, &mut was_active)?;
            self.mapping_active[index] = was_active;
            let Some((value, phase)) = result else {
                continue;
            };
            let descriptor = self
                .mappings
                .intent(mapping.intent())
                .ok_or(RuntimeInputError::UnknownIntent)?;
            if self.pending_intents.len() >= MAX_PENDING_INGRESS {
                self.clear_state();
                return Err(RuntimeInputError::PendingIngressOverflow);
            }
            self.pending_intents.push(PendingIntent {
                sequence,
                order: index,
                descriptor: descriptor.clone(),
                value,
                phase,
                provenance: IntentProvenance::Physical {
                    mapping_id: mapping.id().to_owned(),
                },
            });
        }
        Ok(())
    }
}

fn valid_rebind(
    old: RuntimeInputBinding,
    new: RuntimeInputBinding,
    reason: InputClearReason,
) -> bool {
    if old.instance_id() != new.instance_id() {
        return reason == InputClearReason::Restart;
    }
    if new.generation() > old.generation() {
        return new.control_revision() > old.control_revision()
            && reason == InputClearReason::Restart;
    }
    new.generation() == old.generation()
        && new.control_revision() > old.control_revision()
        && reason == InputClearReason::ControlRevisionChange
}

fn expected_sequence(last: Option<u64>) -> Result<u64, RuntimeInputError> {
    match last {
        None => Ok(0),
        Some(u64::MAX) => Err(RuntimeInputError::SequenceExhausted),
        Some(sequence) => Ok(sequence + 1),
    }
}

fn set_button<T: Copy + PartialEq>(
    entries: &mut Vec<(T, ButtonState)>,
    control: T,
    edge: PhysicalEdge,
) {
    if let Some((_, state)) = entries.iter_mut().find(|(known, _)| *known == control) {
        state.apply(edge);
    } else {
        let mut state = ButtonState::default();
        state.apply(edge);
        entries.push((control, state));
    }
}

fn set_axis<T: Copy + PartialEq>(entries: &mut Vec<(T, AxisValue)>, axis: T, value: AxisValue) {
    if let Some((_, known)) = entries.iter_mut().find(|(existing, _)| *existing == axis) {
        *known = value;
    } else {
        entries.push((axis, value));
    }
}

fn add_pair(
    left: (AxisValue, AxisValue),
    right: (AxisValue, AxisValue),
) -> Result<(AxisValue, AxisValue), RuntimeInputError> {
    Ok((
        AxisValue::new(left.0.value() + right.0.value())?,
        AxisValue::new(left.1.value() + right.1.value())?,
    ))
}

fn snapshot_buttons<T: Copy + Ord>(entries: &[(T, ButtonState)]) -> Vec<ButtonSnapshot<T>> {
    let mut snapshot = entries
        .iter()
        .map(|(control, state)| {
            ButtonSnapshot::new(*control, state.held, state.pressed, state.released)
        })
        .collect::<Vec<_>>();
    snapshot.sort_by_key(|entry| entry.control());
    snapshot
}

fn trigger_value(
    trigger: &RuntimeInputTrigger,
    frame: &InputFrame,
) -> Result<Option<(RuntimeIntentValue, IntentPhase)>, RuntimeInputError> {
    let context_matches =
        |context: Option<&InputContext>| context.is_none_or(|value| value == frame.context());
    match trigger {
        RuntimeInputTrigger::Key {
            code,
            edge,
            chord,
            context,
        } => {
            if !context_matches(context.as_ref())
                || !chord
                    .iter()
                    .all(|control| keyboard_state(frame, *control).held())
            {
                return Ok(None);
            }
            Ok(digital_trigger(keyboard_state(frame, *code), *edge))
        }
        RuntimeInputTrigger::PointerButton {
            button,
            edge,
            context,
        } => {
            if !context_matches(context.as_ref()) {
                return Ok(None);
            }
            Ok(digital_trigger(pointer_state(frame, *button), *edge))
        }
        RuntimeInputTrigger::ControllerButton {
            button,
            edge,
            context,
        } => {
            if !context_matches(context.as_ref()) {
                return Ok(None);
            }
            Ok(digital_trigger(controller_state(frame, *button), *edge))
        }
        RuntimeInputTrigger::PointerAxis { axis, context } => {
            if !context_matches(context.as_ref()) {
                return Ok(None);
            }
            axis_trigger(match axis {
                InputAxis::X => frame.pointer().0,
                InputAxis::Y => frame.pointer().1,
            })
        }
        RuntimeInputTrigger::Wheel { axis, context } => {
            if !context_matches(context.as_ref()) {
                return Ok(None);
            }
            axis_trigger(match axis {
                InputAxis::X => frame.wheel().0,
                InputAxis::Y => frame.wheel().1,
            })
        }
        RuntimeInputTrigger::ControllerAxis { axis, context } => {
            if !context_matches(context.as_ref()) {
                return Ok(None);
            }
            Ok(frame
                .controller_axis(*axis)
                .map(|value| (RuntimeIntentValue::Axis { value }, IntentPhase::Axis)))
        }
    }
}

fn physical_trigger_value(
    trigger: &RuntimeInputTrigger,
    fact: &RuntimeInputFact,
    lane: &RuntimeInputLane,
    was_active: &mut bool,
) -> Result<Option<(RuntimeIntentValue, IntentPhase)>, RuntimeInputError> {
    let context_matches =
        |context: Option<&InputContext>| context.is_none_or(|value| value == &lane.context);
    match trigger {
        RuntimeInputTrigger::Key {
            code,
            edge,
            chord,
            context,
        } if matches!(fact, RuntimeInputFact::Key { .. }) && context_matches(context.as_ref()) => {
            let active =
                held_key(lane, *code) && chord.iter().all(|control| held_key(lane, *control));
            transition_value(*edge, was_active, active)
        }
        RuntimeInputTrigger::PointerButton {
            button,
            edge,
            context,
        } if matches!(fact, RuntimeInputFact::PointerButton { .. })
            && context_matches(context.as_ref()) =>
        {
            let active = held_pointer(lane, *button);
            transition_value(*edge, was_active, active)
        }
        RuntimeInputTrigger::ControllerButton {
            button,
            edge,
            context,
        } if matches!(fact, RuntimeInputFact::ControllerButton { .. })
            && context_matches(context.as_ref()) =>
        {
            let active = held_controller(lane, *button);
            transition_value(*edge, was_active, active)
        }
        RuntimeInputTrigger::PointerAxis { axis, context }
            if let RuntimeInputFact::PointerDelta { x, y } = fact
                && context_matches(context.as_ref()) =>
        {
            axis_trigger(match axis {
                InputAxis::X => *x,
                InputAxis::Y => *y,
            })
        }
        RuntimeInputTrigger::Wheel { axis, context }
            if let RuntimeInputFact::Wheel { x, y } = fact
                && context_matches(context.as_ref()) =>
        {
            axis_trigger(match axis {
                InputAxis::X => *x,
                InputAxis::Y => *y,
            })
        }
        // Controller axes are persistent state. A snapshot emits every
        // observed value (including zero), avoiding a duplicate on ingress.
        RuntimeInputTrigger::ControllerAxis { .. }
            if matches!(fact, RuntimeInputFact::ControllerAxis { .. }) =>
        {
            Ok(None)
        }
        _ => Ok(None),
    }
}

fn transition_value(
    expected: InputEdge,
    was_active: &mut bool,
    active: bool,
) -> Result<Option<(RuntimeIntentValue, IntentPhase)>, RuntimeInputError> {
    let output = match expected {
        InputEdge::Pressed if !*was_active && active => Some((
            RuntimeIntentValue::Digital { active: true },
            IntentPhase::Pressed,
        )),
        InputEdge::Released if *was_active && !active => Some((
            RuntimeIntentValue::Digital { active: false },
            IntentPhase::Released,
        )),
        _ => None,
    };
    *was_active = active;
    Ok(output)
}

fn held_pointer(lane: &RuntimeInputLane, control: PointerButton) -> bool {
    lane.pointer_buttons
        .iter()
        .find(|(known, _)| *known == control)
        .is_some_and(|(_, state)| state.held)
}

fn held_controller(lane: &RuntimeInputLane, control: ControllerButton) -> bool {
    lane.controller_buttons
        .iter()
        .find(|(known, _)| *known == control)
        .is_some_and(|(_, state)| state.held)
}

// Axis changes are emitted on their own ingress event; digital transition
// mappings are evaluated after every matching-source fact so chord activation
// and deactivation edges remain exact.

fn held_key(lane: &RuntimeInputLane, control: KeyboardControl) -> bool {
    lane.keyboard
        .iter()
        .find(|(known, _)| *known == control)
        .is_some_and(|(_, state)| state.held)
}

fn keyboard_state(frame: &InputFrame, control: KeyboardControl) -> ButtonSnapshot<KeyboardControl> {
    frame
        .keyboard()
        .iter()
        .copied()
        .find(|entry| entry.control() == control)
        .unwrap_or(ButtonSnapshot::new(control, false, false, false))
}
fn pointer_state(frame: &InputFrame, control: PointerButton) -> ButtonSnapshot<PointerButton> {
    frame
        .pointer_buttons()
        .iter()
        .copied()
        .find(|entry| entry.control() == control)
        .unwrap_or(ButtonSnapshot::new(control, false, false, false))
}
fn controller_state(
    frame: &InputFrame,
    control: ControllerButton,
) -> ButtonSnapshot<ControllerButton> {
    frame
        .controller_buttons()
        .iter()
        .copied()
        .find(|entry| entry.control() == control)
        .unwrap_or(ButtonSnapshot::new(control, false, false, false))
}
fn digital_trigger(
    state: impl ButtonLike + Copy,
    edge: InputEdge,
) -> Option<(RuntimeIntentValue, IntentPhase)> {
    match edge {
        InputEdge::Held if state.held() => Some((
            RuntimeIntentValue::Digital { active: true },
            IntentPhase::Held,
        )),
        InputEdge::Pressed if state.pressed() => Some((
            RuntimeIntentValue::Digital { active: true },
            IntentPhase::Pressed,
        )),
        InputEdge::Released if state.released() => Some((
            RuntimeIntentValue::Digital { active: false },
            IntentPhase::Released,
        )),
        _ => None,
    }
}
trait ButtonLike {
    fn held(self) -> bool;
    fn pressed(self) -> bool;
    fn released(self) -> bool;
}
impl<T: Copy> ButtonLike for ButtonSnapshot<T> {
    fn held(self) -> bool {
        self.held()
    }
    fn pressed(self) -> bool {
        self.pressed()
    }
    fn released(self) -> bool {
        self.released()
    }
}
fn axis_trigger(
    value: AxisValue,
) -> Result<Option<(RuntimeIntentValue, IntentPhase)>, RuntimeInputError> {
    Ok((value.value() != 0.0).then_some((RuntimeIntentValue::Axis { value }, IntentPhase::Axis)))
}

#[cfg(test)]
mod tests {
    use super::expected_sequence;
    use crate::RuntimeInputError;

    #[test]
    fn sequence_exhaustion_never_wraps_to_a_reused_zero() {
        assert_eq!(expected_sequence(None), Ok(0));
        assert_eq!(expected_sequence(Some(u64::MAX - 1)), Ok(u64::MAX));
        assert_eq!(
            expected_sequence(Some(u64::MAX)),
            Err(RuntimeInputError::SequenceExhausted)
        );
    }
}
