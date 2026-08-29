use std::{collections::BTreeMap, fmt};

use runtime_lifecycle::{
    validate_runtime_identity, RuntimeControlRevision, RuntimeGeneration, RuntimeInstanceId,
    SimulationStep,
};
use serde::{Deserialize, Serialize};

use crate::CompiledInputIntent;

pub const MAX_PENDING_INGRESS: usize = 1_024;
pub const MAX_AXIS_MAGNITUDE: f32 = 8_192.0;
pub const MAX_DIRECT_INTENT_AXIS_MAGNITUDE: f32 = 1.0;
pub const MAX_CONTROLLER_AXIS_MAGNITUDE: f32 = 1.0;
/// Maximum canonical JSON bytes one direct product-payload intent may carry.
pub const MAX_DIRECT_INTENT_PRODUCT_PAYLOAD_BYTES: usize = 65_536;
pub const MAX_DIRECT_INTENT_PRODUCT_PAYLOAD_JSON_BYTES: usize =
    MAX_DIRECT_INTENT_PRODUCT_PAYLOAD_BYTES;
pub const MAX_DIRECT_INTENT_PRODUCT_PAYLOAD_DEPTH: usize = 32;
pub const MAX_DIRECT_INTENT_PRODUCT_PAYLOAD_NODES: usize = 4_096;
pub const MAX_DIRECT_INTENT_PRODUCT_PAYLOAD_STRING_BYTES: usize = 16_384;
pub const MAX_DIRECT_INTENT_PRODUCT_PAYLOAD_ARRAY_ENTRIES: usize = 1_024;
pub const MAX_DIRECT_INTENT_PRODUCT_PAYLOAD_OBJECT_ENTRIES: usize = 1_024;
pub const MAX_DIRECT_INTENT_PRODUCT_PAYLOAD_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

/// The retained host-neutral direct input value vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IntentValueKind {
    Digital,
    Axis,
    ProductPayload,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InputEdge {
    Held,
    Pressed,
    Released,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum KeyboardControl {
    KeyA,
    KeyB,
    KeyC,
    KeyD,
    KeyE,
    KeyF,
    KeyG,
    KeyH,
    KeyI,
    KeyJ,
    KeyK,
    KeyL,
    KeyM,
    KeyN,
    KeyO,
    KeyP,
    KeyQ,
    KeyR,
    KeyS,
    KeyT,
    KeyU,
    KeyV,
    KeyW,
    KeyX,
    KeyY,
    KeyZ,
    Digit0,
    Digit1,
    Digit2,
    Digit3,
    Digit4,
    Digit5,
    Digit6,
    Digit7,
    Digit8,
    Digit9,
    Space,
    Enter,
    Escape,
    ShiftLeft,
    ShiftRight,
    ControlLeft,
    ControlRight,
    AltLeft,
    AltRight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PointerButton {
    Primary,
    Secondary,
    Middle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InputAxis {
    X,
    Y,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ControllerButton {
    #[serde(rename = "button-0")]
    Button0,
    #[serde(rename = "button-1")]
    Button1,
    #[serde(rename = "button-2")]
    Button2,
    #[serde(rename = "button-3")]
    Button3,
    #[serde(rename = "button-4")]
    Button4,
    #[serde(rename = "button-5")]
    Button5,
    #[serde(rename = "button-6")]
    Button6,
    #[serde(rename = "button-7")]
    Button7,
    #[serde(rename = "button-8")]
    Button8,
    #[serde(rename = "button-9")]
    Button9,
    #[serde(rename = "button-10")]
    Button10,
    #[serde(rename = "button-11")]
    Button11,
    #[serde(rename = "button-12")]
    Button12,
    #[serde(rename = "button-13")]
    Button13,
    #[serde(rename = "button-14")]
    Button14,
    #[serde(rename = "button-15")]
    Button15,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ControllerAxis {
    #[serde(rename = "axis-0")]
    Axis0,
    #[serde(rename = "axis-1")]
    Axis1,
    #[serde(rename = "axis-2")]
    Axis2,
    #[serde(rename = "axis-3")]
    Axis3,
}

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
/// Bounded plain JSON provided by a direct product UI claim. Its contract is
/// later matched with the descriptor-owned contract before it can reach a
/// Product Runtime Adapter.
#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeProductPayload {
    contract: String,
    data: serde_json::Value,
    bytes: Vec<u8>,
}

impl RuntimeProductPayload {
    pub fn new(
        contract: impl Into<String>,
        data: serde_json::Value,
    ) -> Result<Self, RuntimeInputError> {
        let contract = contract.into();
        if !is_identity(&contract) {
            return Err(RuntimeInputError::InvalidProductPayloadContract);
        }
        let bytes = validate_product_payload_json(&data)?;
        Ok(Self {
            contract,
            data,
            bytes,
        })
    }

    pub fn contract(&self) -> &str {
        &self.contract
    }
    pub fn data(&self) -> &serde_json::Value {
        &self.data
    }

    /// Canonical bounded payload bytes retained at direct-claim admission.
    /// Product adapters copy this opaque data; they do not reinterpret it.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// The closed direct UI claim value vocabulary. Product payloads are data only:
/// no callback, command route, host handle, timer, or mutable reference enters
/// this host-neutral lane.
#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeIntentValue {
    Digital { active: bool },
    Axis { value: AxisValue },
    ProductPayload { payload: RuntimeProductPayload },
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
        if let RuntimeIntentValue::Axis { value } = &value {
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
    pub fn value(&self) -> RuntimeIntentValue {
        self.value.clone()
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
    pub fn value(&self) -> RuntimeIntentValue {
        self.value.clone()
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
    InvalidProductPayloadContract,
    ProductPayloadTooLarge { actual: usize, maximum: usize },
    ProductPayloadStructureOutOfBounds(&'static str),
    ProductPayloadContractMismatch,
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
    DuplicateIntent,
    InvalidMapping,
    DuplicateMapping,
    DirectIntentPayloadUnsupported,
    UnknownIntent,
    IntentValueKindMismatch,
    Disposed,
    WrongSnapshotPhase,
    LifecycleValidation,
    SnapshotOutOfOrder,
}

fn validate_product_payload_json(value: &serde_json::Value) -> Result<Vec<u8>, RuntimeInputError> {
    let bytes = serde_json::to_vec(value).map_err(|_| RuntimeInputError::WireMalformed)?;
    if bytes.len() > MAX_DIRECT_INTENT_PRODUCT_PAYLOAD_JSON_BYTES {
        return Err(RuntimeInputError::ProductPayloadTooLarge {
            actual: bytes.len(),
            maximum: MAX_DIRECT_INTENT_PRODUCT_PAYLOAD_JSON_BYTES,
        });
    }
    let mut nodes = 0usize;
    validate_product_payload_value(value, 1, &mut nodes)?;
    Ok(bytes)
}

fn validate_product_payload_value(
    value: &serde_json::Value,
    depth: usize,
    nodes: &mut usize,
) -> Result<(), RuntimeInputError> {
    if depth > MAX_DIRECT_INTENT_PRODUCT_PAYLOAD_DEPTH {
        return Err(RuntimeInputError::ProductPayloadStructureOutOfBounds(
            "depth",
        ));
    }
    *nodes = nodes
        .checked_add(1)
        .ok_or(RuntimeInputError::ProductPayloadStructureOutOfBounds(
            "nodes",
        ))?;
    if *nodes > MAX_DIRECT_INTENT_PRODUCT_PAYLOAD_NODES {
        return Err(RuntimeInputError::ProductPayloadStructureOutOfBounds(
            "nodes",
        ));
    }
    match value {
        serde_json::Value::Null | serde_json::Value::Bool(_) => Ok(()),
        serde_json::Value::String(value) => {
            if value.len() > MAX_DIRECT_INTENT_PRODUCT_PAYLOAD_STRING_BYTES {
                Err(RuntimeInputError::ProductPayloadStructureOutOfBounds(
                    "string",
                ))
            } else {
                Ok(())
            }
        }
        serde_json::Value::Number(value) => {
            let Some(number) = value.as_f64() else {
                return Err(RuntimeInputError::ProductPayloadStructureOutOfBounds(
                    "number",
                ));
            };
            if !number.is_finite()
                || (number.fract() == 0.0
                    && number.abs() > MAX_DIRECT_INTENT_PRODUCT_PAYLOAD_SAFE_INTEGER as f64)
            {
                Err(RuntimeInputError::ProductPayloadStructureOutOfBounds(
                    "integer",
                ))
            } else {
                Ok(())
            }
        }
        serde_json::Value::Array(values) => {
            if values.len() > MAX_DIRECT_INTENT_PRODUCT_PAYLOAD_ARRAY_ENTRIES {
                return Err(RuntimeInputError::ProductPayloadStructureOutOfBounds(
                    "array",
                ));
            }
            values
                .iter()
                .try_for_each(|value| validate_product_payload_value(value, depth + 1, nodes))
        }
        serde_json::Value::Object(values) => {
            if values.len() > MAX_DIRECT_INTENT_PRODUCT_PAYLOAD_OBJECT_ENTRIES {
                return Err(RuntimeInputError::ProductPayloadStructureOutOfBounds(
                    "object",
                ));
            }
            for (key, value) in values {
                if key.len() > MAX_DIRECT_INTENT_PRODUCT_PAYLOAD_STRING_BYTES {
                    return Err(RuntimeInputError::ProductPayloadStructureOutOfBounds(
                        "object-key",
                    ));
                }
                validate_product_payload_value(value, depth + 1, nodes)?;
            }
            Ok(())
        }
    }
}

impl fmt::Display for RuntimeInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "runtime input error: {self:?}")
    }
}
impl std::error::Error for RuntimeInputError {}

pub(crate) fn is_identity(value: &str) -> bool {
    validate_runtime_identity(value).is_ok()
}
