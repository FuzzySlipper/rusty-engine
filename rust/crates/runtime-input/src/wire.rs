use runtime_lifecycle::{RuntimeControlRevision, RuntimeGeneration, RuntimeInstanceId};
use serde::Deserialize;

use crate::{
    model::validate_controller_axis, parse_canonical_u64, AxisValue, ControllerAxis,
    ControllerButton, InputClearReason, InputContext, KeyboardControl, PhysicalEdge, PointerButton,
    RuntimeDirectIntentClaim, RuntimeInputBinding, RuntimeInputError, RuntimeInputEvent,
    RuntimeInputFact, RuntimeInputIngress, RuntimeIntentValue, RuntimeProductPayload,
};

/// Maximum bytes accepted from one host wire decode operation.
pub const MAX_RUNTIME_INPUT_WIRE_BYTES: usize = 524_288;
/// Maximum normalized physical/direct envelopes accepted in one wire batch.
pub const MAX_RUNTIME_INPUT_WIRE_EVENTS: usize = 1_024;

/// Strictly decodes one structural host envelope. The wire uses only canonical
/// decimal strings for u64 values so JavaScript never loses correlation bits.
pub fn decode_runtime_input_wire_event_json(
    bytes: &[u8],
) -> Result<RuntimeInputEvent, RuntimeInputError> {
    if bytes.len() > MAX_RUNTIME_INPUT_WIRE_BYTES {
        return Err(RuntimeInputError::WireTooLarge);
    }
    decode_exact::<WireInputEvent>(bytes)?.into_event()
}

/// Strictly decodes the ordered `drain()` array emitted by a host adapter.
/// Array order is preserved and is separately checked by [`crate::RuntimeInputLane`].
pub fn decode_runtime_input_wire_events_json(
    bytes: &[u8],
) -> Result<Vec<RuntimeInputEvent>, RuntimeInputError> {
    if bytes.len() > MAX_RUNTIME_INPUT_WIRE_BYTES {
        return Err(RuntimeInputError::WireTooLarge);
    }
    let events = decode_exact::<Vec<WireInputEvent>>(bytes)?;
    if events.len() > MAX_RUNTIME_INPUT_WIRE_EVENTS {
        return Err(RuntimeInputError::WireEventLimit);
    }
    events.into_iter().map(WireInputEvent::into_event).collect()
}

fn decode_exact<T: for<'de> Deserialize<'de>>(bytes: &[u8]) -> Result<T, RuntimeInputError> {
    let mut decoder = serde_json::Deserializer::from_slice(bytes);
    let value = T::deserialize(&mut decoder).map_err(|_| RuntimeInputError::WireMalformed)?;
    decoder
        .end()
        .map_err(|_| RuntimeInputError::WireMalformed)?;
    Ok(value)
}

#[derive(Deserialize)]
#[serde(untagged)]
enum WireInputEvent {
    Physical(WirePhysicalInputEvent),
    Direct(WireDirectIntentClaim),
}

impl WireInputEvent {
    fn into_event(self) -> Result<RuntimeInputEvent, RuntimeInputError> {
        match self {
            Self::Physical(value) => value.into_event(),
            Self::Direct(value) => value.into_event(),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WirePhysicalInputEvent {
    runtime: WireRuntimeBinding,
    sequence: String,
    context: String,
    fact: WireFact,
}

impl WirePhysicalInputEvent {
    fn into_event(self) -> Result<RuntimeInputEvent, RuntimeInputError> {
        let runtime = self.runtime.into_binding()?;
        let sequence = parse_canonical_u64(&self.sequence)?;
        let context = InputContext::new(self.context)?;
        Ok(RuntimeInputEvent::Physical(RuntimeInputIngress::new(
            runtime,
            sequence,
            context,
            self.fact.into_fact()?,
        )))
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireDirectIntentClaim {
    runtime: WireRuntimeBinding,
    sequence: String,
    context: String,
    intent: String,
    value: WireIntentValue,
}

impl WireDirectIntentClaim {
    fn into_event(self) -> Result<RuntimeInputEvent, RuntimeInputError> {
        Ok(RuntimeInputEvent::DirectIntent(
            RuntimeDirectIntentClaim::new(
                self.runtime.into_binding()?,
                parse_canonical_u64(&self.sequence)?,
                InputContext::new(self.context)?,
                self.intent,
                self.value.into_value()?,
            )?,
        ))
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireRuntimeBinding {
    instance_id: String,
    generation: String,
    control_revision: String,
}

impl WireRuntimeBinding {
    fn into_binding(self) -> Result<RuntimeInputBinding, RuntimeInputError> {
        Ok(RuntimeInputBinding::new(
            RuntimeInstanceId::new(parse_canonical_u64(&self.instance_id)?),
            RuntimeGeneration::new(parse_canonical_u64(&self.generation)?),
            RuntimeControlRevision::new(parse_canonical_u64(&self.control_revision)?),
        ))
    }
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
enum WireFact {
    Key {
        code: KeyboardControl,
        edge: WirePhysicalEdge,
    },
    PointerButton {
        button: PointerButton,
        edge: WirePhysicalEdge,
    },
    PointerDelta {
        x: f32,
        y: f32,
    },
    Wheel {
        x: f32,
        y: f32,
    },
    ControllerButton {
        button: ControllerButton,
        edge: WirePhysicalEdge,
    },
    ControllerAxis {
        axis: ControllerAxis,
        value: f32,
    },
    Clear {
        reason: WireClearReason,
    },
}

impl WireFact {
    fn into_fact(self) -> Result<RuntimeInputFact, RuntimeInputError> {
        Ok(match self {
            Self::Key { code, edge } => RuntimeInputFact::Key {
                code,
                edge: edge.into_edge(),
            },
            Self::PointerButton { button, edge } => RuntimeInputFact::PointerButton {
                button,
                edge: edge.into_edge(),
            },
            Self::PointerDelta { x, y } => RuntimeInputFact::PointerDelta {
                x: AxisValue::new(x)?,
                y: AxisValue::new(y)?,
            },
            Self::Wheel { x, y } => RuntimeInputFact::Wheel {
                x: AxisValue::new(x)?,
                y: AxisValue::new(y)?,
            },
            Self::ControllerButton { button, edge } => RuntimeInputFact::ControllerButton {
                button,
                edge: edge.into_edge(),
            },
            Self::ControllerAxis { axis, value } => RuntimeInputFact::ControllerAxis {
                axis,
                value: validate_controller_axis(AxisValue::new(value)?)?,
            },
            Self::Clear { reason } => RuntimeInputFact::Clear {
                reason: reason.into_reason(),
            },
        })
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WirePhysicalEdge {
    Pressed,
    Released,
}

impl WirePhysicalEdge {
    const fn into_edge(self) -> PhysicalEdge {
        match self {
            Self::Pressed => PhysicalEdge::Pressed,
            Self::Released => PhysicalEdge::Released,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireClearReason {
    FocusLoss,
    InteractionModeLoss,
    PointerLockLoss,
    Restart,
    ControlRevisionChange,
    Dispose,
    IngressOverflow,
}

impl WireClearReason {
    const fn into_reason(self) -> InputClearReason {
        match self {
            Self::FocusLoss => InputClearReason::FocusLoss,
            Self::InteractionModeLoss => InputClearReason::InteractionModeLoss,
            Self::PointerLockLoss => InputClearReason::PointerLockLoss,
            Self::Restart => InputClearReason::Restart,
            Self::ControlRevisionChange => InputClearReason::ControlRevisionChange,
            Self::Dispose => InputClearReason::Dispose,
            Self::IngressOverflow => InputClearReason::IngressOverflow,
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
enum WireIntentValue {
    Digital {
        active: bool,
    },
    Axis {
        value: f32,
    },
    ProductPayload {
        contract: String,
        data: serde_json::Value,
    },
}

impl WireIntentValue {
    fn into_value(self) -> Result<RuntimeIntentValue, RuntimeInputError> {
        Ok(match self {
            Self::Digital { active } => RuntimeIntentValue::Digital { active },
            Self::Axis { value } => RuntimeIntentValue::Axis {
                value: AxisValue::new(value)?,
            },
            Self::ProductPayload { contract, data } => RuntimeIntentValue::ProductPayload {
                payload: RuntimeProductPayload::new(contract, data)?,
            },
        })
    }
}
