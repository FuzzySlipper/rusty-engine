use std::{collections::BTreeMap, fmt};

use product_model::{ControllerAxis, ControllerButton, KeyboardControl, PointerButton};
use runtime_lifecycle::{
    RuntimeControlRevision, RuntimeGeneration, RuntimeInstanceId, SimulationStep,
};

use crate::CompiledInputIntent;

pub const MAX_PENDING_INGRESS: usize = 1_024;
pub const MAX_AXIS_MAGNITUDE: f32 = 8_192.0;
pub const MAX_DIRECT_INTENT_AXIS_MAGNITUDE: f32 = 1.0;
pub const MAX_CONTROLLER_AXIS_MAGNITUDE: f32 = 1.0;

/// Instance/generation/control correlation supplied by the explicit product
/// runtime. It has no global registry or generated identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RuntimeInputBinding {
    instance_id: RuntimeInstanceId,
    generation: RuntimeGeneration,
    control_revision: RuntimeControlRevision,
}

impl RuntimeInputBinding {
    pub const fn new(
        instance_id: RuntimeInstanceId,
        generation: RuntimeGeneration,
        control_revision: RuntimeControlRevision,
    ) -> Self {
        Self {
            instance_id,
            generation,
            control_revision,
        }
    }

    pub const fn instance_id(self) -> RuntimeInstanceId {
        self.instance_id
    }
    pub const fn generation(self) -> RuntimeGeneration {
        self.generation
    }
    pub const fn control_revision(self) -> RuntimeControlRevision {
        self.control_revision
    }
}

/// Parses the exact u64 wire vocabulary used by TypeScript/browser hosts.
/// Decimal strings avoid JavaScript's unsafe integer range and reject aliases.
pub fn parse_canonical_u64(value: &str) -> Result<u64, RuntimeInputError> {
    if value.is_empty()
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(RuntimeInputError::NonCanonicalWireInteger);
    }
    value
        .parse::<u64>()
        .map_err(|_| RuntimeInputError::NonCanonicalWireInteger)
}

/// Bounded product-defined input context. It is not a browser focus object.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InputContext(String);

impl InputContext {
    pub fn new(value: impl Into<String>) -> Result<Self, RuntimeInputError> {
        let value = value.into();
        if !is_identity(&value) {
            return Err(RuntimeInputError::InvalidContext);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A bounded, finite value supplied by normalized physical facts or direct UI
/// intent claims. Axis policy (deadzone, sensitivity, aggregation) stays with
/// the host or downstream product rather than this lane.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AxisValue(f32);

impl AxisValue {
    pub fn new(value: f32) -> Result<Self, RuntimeInputError> {
        if !value.is_finite() || value.abs() > MAX_AXIS_MAGNITUDE {
            return Err(RuntimeInputError::InvalidAxisValue);
        }
        // Make the single representable neutral fact deterministic across
        // hosts: IEEE -0.0 carries a distinct bit pattern despite equality.
        Ok(Self(if value == 0.0 { 0.0 } else { value }))
    }

    pub const fn value(self) -> f32 {
        self.0
    }
}

pub(crate) fn validate_controller_axis(value: AxisValue) -> Result<AxisValue, RuntimeInputError> {
    if value.value().abs() > MAX_CONTROLLER_AXIS_MAGNITUDE {
        return Err(RuntimeInputError::InvalidControllerAxisValue);
    }
    Ok(value)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhysicalEdge {
    Pressed,
    Released,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputClearReason {
    FocusLoss,
    InteractionModeLoss,
    PointerLockLoss,
    Restart,
    ControlRevisionChange,
    Dispose,
    IngressOverflow,
}

/// Host-neutral physical observation. The browser adapter performs DOM
/// conversion before constructing one of these values.
#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeInputFact {
    Key {
        code: KeyboardControl,
        edge: PhysicalEdge,
    },
    PointerButton {
        button: PointerButton,
        edge: PhysicalEdge,
    },
    PointerDelta {
        x: AxisValue,
        y: AxisValue,
    },
    Wheel {
        x: AxisValue,
        y: AxisValue,
    },
    ControllerButton {
        button: ControllerButton,
        edge: PhysicalEdge,
    },
    ControllerAxis {
        axis: ControllerAxis,
        value: AxisValue,
    },
    Clear {
        reason: InputClearReason,
    },
}

/// One ordered raw physical ingress. `sequence` is transport-neutral u64; its
/// browser wire representation is required to use [`parse_canonical_u64`].
#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeInputIngress {
    runtime: RuntimeInputBinding,
    sequence: u64,
    context: InputContext,
    fact: RuntimeInputFact,
}

impl RuntimeInputIngress {
    pub fn new(
        runtime: RuntimeInputBinding,
        sequence: u64,
        context: InputContext,
        fact: RuntimeInputFact,
    ) -> Self {
        Self {
            runtime,
            sequence,
            context,
            fact,
        }
    }
    pub const fn runtime(&self) -> RuntimeInputBinding {
        self.runtime
    }
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }
    pub fn context(&self) -> &InputContext {
        &self.context
    }
    pub fn fact(&self) -> &RuntimeInputFact {
        &self.fact
    }
}

/// The only direct UI claim vocabulary. It is admitted against the same
/// product intent descriptors as physical mappings before it enters the lane.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RuntimeIntentValue {
    Digital { active: bool },
    Axis { value: AxisValue },
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeDirectIntentClaim {
    runtime: RuntimeInputBinding,
    sequence: u64,
    context: InputContext,
    intent: String,
    value: RuntimeIntentValue,
}

impl RuntimeDirectIntentClaim {
    pub fn new(
        runtime: RuntimeInputBinding,
        sequence: u64,
        context: InputContext,
        intent: impl Into<String>,
        value: RuntimeIntentValue,
    ) -> Result<Self, RuntimeInputError> {
        let intent = intent.into();
        if !is_identity(&intent) {
            return Err(RuntimeInputError::InvalidIntent);
        }
        if let RuntimeIntentValue::Axis { value } = value {
            if value.value().abs() > MAX_DIRECT_INTENT_AXIS_MAGNITUDE {
                return Err(RuntimeInputError::InvalidDirectIntentAxisValue);
            }
        }
        Ok(Self {
            runtime,
            sequence,
            context,
            intent,
            value,
        })
    }
    pub const fn runtime(&self) -> RuntimeInputBinding {
        self.runtime
    }
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }
    pub fn context(&self) -> &InputContext {
        &self.context
    }
    pub fn intent(&self) -> &str {
        &self.intent
    }
    pub const fn value(&self) -> RuntimeIntentValue {
        self.value
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeInputEvent {
    Physical(RuntimeInputIngress),
    DirectIntent(RuntimeDirectIntentClaim),
}

impl RuntimeInputEvent {
    pub const fn runtime(&self) -> RuntimeInputBinding {
        match self {
            Self::Physical(value) => value.runtime(),
            Self::DirectIntent(value) => value.runtime(),
        }
    }
    pub const fn sequence(&self) -> u64 {
        match self {
            Self::Physical(value) => value.sequence(),
            Self::DirectIntent(value) => value.sequence(),
        }
    }
    pub fn context(&self) -> &InputContext {
        match self {
            Self::Physical(value) => value.context(),
            Self::DirectIntent(value) => value.context(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ButtonSnapshot<T> {
    control: T,
    held: bool,
    pressed: bool,
    released: bool,
}

impl<T: Copy> ButtonSnapshot<T> {
    pub(crate) const fn new(control: T, held: bool, pressed: bool, released: bool) -> Self {
        Self {
            control,
            held,
            pressed,
            released,
        }
    }
    pub const fn control(self) -> T {
        self.control
    }
    pub const fn held(self) -> bool {
        self.held
    }
    pub const fn pressed(self) -> bool {
        self.pressed
    }
    pub const fn released(self) -> bool {
        self.released
    }
}

/// One immutable deterministic observation of all state accumulated before a
/// caller-admitted simulation step. Transient flags and deltas are cleared only
/// after this value is assembled.
#[derive(Debug, Clone, PartialEq)]
pub struct InputFrame {
    runtime: RuntimeInputBinding,
    simulation_step: SimulationStep,
    context: InputContext,
    keyboard: Vec<ButtonSnapshot<KeyboardControl>>,
    pointer_buttons: Vec<ButtonSnapshot<PointerButton>>,
    controller_buttons: Vec<ButtonSnapshot<ControllerButton>>,
    pointer: (AxisValue, AxisValue),
    wheel: (AxisValue, AxisValue),
    controller_axes: BTreeMap<ControllerAxis, AxisValue>,
}

pub(crate) struct InputFrameFacts {
    pub(crate) keyboard: Vec<ButtonSnapshot<KeyboardControl>>,
    pub(crate) pointer_buttons: Vec<ButtonSnapshot<PointerButton>>,
    pub(crate) controller_buttons: Vec<ButtonSnapshot<ControllerButton>>,
    pub(crate) pointer: (AxisValue, AxisValue),
    pub(crate) wheel: (AxisValue, AxisValue),
    pub(crate) controller_axes: BTreeMap<ControllerAxis, AxisValue>,
}

impl InputFrame {
    pub(crate) fn new(
        runtime: RuntimeInputBinding,
        simulation_step: SimulationStep,
        context: InputContext,
        facts: InputFrameFacts,
    ) -> Self {
        Self {
            runtime,
            simulation_step,
            context,
            keyboard: facts.keyboard,
            pointer_buttons: facts.pointer_buttons,
            controller_buttons: facts.controller_buttons,
            pointer: facts.pointer,
            wheel: facts.wheel,
            controller_axes: facts.controller_axes,
        }
    }
    pub const fn runtime(&self) -> RuntimeInputBinding {
        self.runtime
    }
    pub const fn simulation_step(&self) -> SimulationStep {
        self.simulation_step
    }
    pub fn context(&self) -> &InputContext {
        &self.context
    }
    pub fn keyboard(&self) -> &[ButtonSnapshot<KeyboardControl>] {
        &self.keyboard
    }
    pub fn pointer_buttons(&self) -> &[ButtonSnapshot<PointerButton>] {
        &self.pointer_buttons
    }
    pub fn controller_buttons(&self) -> &[ButtonSnapshot<ControllerButton>] {
        &self.controller_buttons
    }
    pub const fn pointer(&self) -> (AxisValue, AxisValue) {
        self.pointer
    }
    pub const fn wheel(&self) -> (AxisValue, AxisValue) {
        self.wheel
    }
    pub fn controller_axis(&self, axis: ControllerAxis) -> Option<AxisValue> {
        self.controller_axes.get(&axis).copied()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntentPhase {
    Held,
    Pressed,
    Released,
    Axis,
    DirectUi,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntentProvenance {
    Physical { mapping_id: String },
    DirectUi,
}

/// Immutable typed output that downstream product code deliberately consumes;
/// the lane never applies a capability, mutates gameplay, or schedules work.
#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeIntentEnvelope {
    runtime: RuntimeInputBinding,
    simulation_step: SimulationStep,
    sequence: u64,
    descriptor: CompiledInputIntent,
    value: RuntimeIntentValue,
    phase: IntentPhase,
    provenance: IntentProvenance,
}

impl RuntimeIntentEnvelope {
    pub(crate) fn new(
        runtime: RuntimeInputBinding,
        simulation_step: SimulationStep,
        sequence: u64,
        descriptor: CompiledInputIntent,
        value: RuntimeIntentValue,
        phase: IntentPhase,
        provenance: IntentProvenance,
    ) -> Self {
        Self {
            runtime,
            simulation_step,
            sequence,
            descriptor,
            value,
            phase,
            provenance,
        }
    }
    pub const fn runtime(&self) -> RuntimeInputBinding {
        self.runtime
    }
    pub const fn simulation_step(&self) -> SimulationStep {
        self.simulation_step
    }
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }
    pub fn intent(&self) -> &str {
        self.descriptor.id()
    }
    /// Linked descriptor readout for caller-owned static dispatch. The lane
    /// does not invoke this capability or turn it into a service route.
    pub fn descriptor(&self) -> &CompiledInputIntent {
        &self.descriptor
    }
    pub const fn value(&self) -> RuntimeIntentValue {
        self.value
    }
    pub const fn phase(&self) -> IntentPhase {
        self.phase
    }
    pub fn provenance(&self) -> &IntentProvenance {
        &self.provenance
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeInputError {
    InvalidContext,
    InvalidIntent,
    InvalidAxisValue,
    InvalidDirectIntentAxisValue,
    InvalidControllerAxisValue,
    NonCanonicalWireInteger,
    WireMalformed,
    WireTooLarge,
    WireEventLimit,
    SequenceOutOfOrder { expected: u64, received: u64 },
    SequenceExhausted,
    BindingMismatch,
    InvalidRebindClear,
    PendingIngressOverflow,
    UnknownIntent,
    IntentValueKindMismatch,
    Disposed,
    WrongSnapshotPhase,
    LifecycleValidation,
    SnapshotOutOfOrder,
}

impl fmt::Display for RuntimeInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "runtime input error: {self:?}")
    }
}
impl std::error::Error for RuntimeInputError {}

pub(crate) fn is_identity(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
        && value
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_alphanumeric())
        && value
            .bytes()
            .last()
            .is_some_and(|byte| byte.is_ascii_alphanumeric())
        && !value.as_bytes().windows(2).any(|pair| {
            matches!(pair[0], b'.' | b'_' | b'-') && matches!(pair[1], b'.' | b'_' | b'-')
        })
}
