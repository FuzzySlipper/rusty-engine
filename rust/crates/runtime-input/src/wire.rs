use runtime_lifecycle::{RuntimeControlRevision, RuntimeGeneration, RuntimeInstanceId};
use serde::Deserialize;
use ts_rs::TS;

use crate::{
    model::{validate_controller_axis, validate_controller_button_value},
    parse_canonical_u64, AxisValue, ControllerAxis, ControllerButton, InputClearReason,
    InputContext, KeyboardControl, PhysicalEdge, PointerButton, RuntimeDirectIntentClaim,
    RuntimeInputBinding, RuntimeInputError, RuntimeInputEvent, RuntimeInputFact,
    RuntimeInputIngress, RuntimeIntentValue, RuntimeProductPayload,
};

/// Maximum normalized physical/direct envelopes accepted in one wire batch.
pub const MAX_RUNTIME_INPUT_WIRE_EVENTS: usize = 1_024;

/// Strictly decodes one structural host envelope. The wire uses only canonical
/// decimal strings for u64 values so JavaScript never loses correlation bits.
pub fn decode_runtime_input_wire_event_json(
    bytes: &[u8],
) -> Result<RuntimeInputEvent, RuntimeInputError> {
    decode_exact::<RuntimeInputWireEvent>(bytes)?.into_event()
}

/// Strictly decodes the ordered `drain()` array emitted by a host adapter.
/// Array order is preserved and is separately checked by [`crate::RuntimeInputLane`].
pub fn decode_runtime_input_wire_events_json(
    bytes: &[u8],
) -> Result<Vec<RuntimeInputEvent>, RuntimeInputError> {
    let events = decode_exact::<Vec<RuntimeInputWireEvent>>(bytes)?;
    if events.len() > MAX_RUNTIME_INPUT_WIRE_EVENTS {
        return Err(RuntimeInputError::WireEventLimit);
    }
    events
        .into_iter()
        .map(RuntimeInputWireEvent::into_event)
        .collect()
}

fn decode_exact<T: for<'de> Deserialize<'de>>(bytes: &[u8]) -> Result<T, RuntimeInputError> {
    let mut decoder = serde_json::Deserializer::from_slice(bytes);
    let value = T::deserialize(&mut decoder).map_err(|_| RuntimeInputError::WireMalformed)?;
    decoder
        .end()
        .map_err(|_| RuntimeInputError::WireMalformed)?;
    Ok(value)
}

/// One input envelope a host submits, in observation order: a physical fact
/// or an intent claimed by product UI.
#[derive(Deserialize, TS)]
#[serde(untagged)]
pub enum RuntimeInputWireEvent {
    Physical(RuntimeInputWirePhysical),
    Direct(RuntimeInputWireIntentClaim),
}

impl RuntimeInputWireEvent {
    fn into_event(self) -> Result<RuntimeInputEvent, RuntimeInputError> {
        match self {
            Self::Physical(value) => value.into_event(),
            Self::Direct(value) => value.into_event(),
        }
    }
}

/// A physical input fact observed for one runtime binding.
#[derive(Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeInputWirePhysical {
    runtime: RuntimeInputWireBinding,
    /// Canonical decimal u64, increasing within the binding.
    sequence: String,
    /// Product-declared input context.
    context: String,
    fact: RuntimeInputWireFact,
}

impl RuntimeInputWirePhysical {
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

/// A product-declared intent claimed by product UI, in the same ordered lane.
#[derive(Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeInputWireIntentClaim {
    runtime: RuntimeInputWireBinding,
    /// Canonical decimal u64, increasing within the binding.
    sequence: String,
    /// Product-declared input context.
    context: String,
    /// Product-declared intent name.
    intent: String,
    value: RuntimeInputWireIntentValue,
}

impl RuntimeInputWireIntentClaim {
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

/// The runtime binding an envelope belongs to, as canonical decimal text.
#[derive(Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeInputWireBinding {
    instance_id: String,
    generation: String,
    control_revision: String,
}

impl RuntimeInputWireBinding {
    fn into_binding(self) -> Result<RuntimeInputBinding, RuntimeInputError> {
        Ok(RuntimeInputBinding::new(
            RuntimeInstanceId::new(parse_canonical_u64(&self.instance_id)?),
            RuntimeGeneration::new(parse_canonical_u64(&self.generation)?),
            RuntimeControlRevision::new(parse_canonical_u64(&self.control_revision)?),
        ))
    }
}

/// A physical fact from the closed Engine input catalog.
#[derive(Deserialize, TS)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum RuntimeInputWireFact {
    Key {
        code: KeyboardControl,
        edge: RuntimeInputWireEdge,
    },
    PointerButton {
        button: PointerButton,
        edge: RuntimeInputWireEdge,
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
        edge: RuntimeInputWireEdge,
    },
    ControllerAxis {
        axis: ControllerAxis,
        value: f32,
    },
    ControllerButtonValue {
        button: ControllerButton,
        value: f32,
    },
    Clear {
        reason: RuntimeInputWireClearReason,
    },
}

impl RuntimeInputWireFact {
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
            Self::ControllerButtonValue { button, value } => {
                RuntimeInputFact::ControllerButtonValue {
                    button,
                    value: validate_controller_button_value(AxisValue::new(value)?)?,
                }
            }
            Self::Clear { reason } => RuntimeInputFact::Clear {
                reason: reason.into_reason(),
            },
        })
    }
}

#[derive(Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeInputWireEdge {
    Pressed,
    Released,
}

impl RuntimeInputWireEdge {
    const fn into_edge(self) -> PhysicalEdge {
        match self {
            Self::Pressed => PhysicalEdge::Pressed,
            Self::Released => PhysicalEdge::Released,
        }
    }
}

/// Why a host cleared held input.
#[derive(Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeInputWireClearReason {
    FocusLoss,
    InteractionModeLoss,
    PointerLockLoss,
    Restart,
    ControlRevisionChange,
    Dispose,
    IngressOverflow,
}

impl RuntimeInputWireClearReason {
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

#[derive(Deserialize, TS)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum RuntimeInputWireIntentValue {
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

impl RuntimeInputWireIntentValue {
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
