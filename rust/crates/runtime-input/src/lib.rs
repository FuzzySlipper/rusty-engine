//! Host-neutral normalized input facts and one instance-owned typed intent lane.
//!
//! Browser/application hosts translate DOM events into the bounded ingress
//! values here. This crate owns no DOM, controller polling, scheduler, global
//! bus, capability invocation, movement, collision, or gameplay consequence.

#![forbid(unsafe_code)]

mod compile;
mod lane;
mod model;
mod wire;

pub use compile::{
    CompiledInputIntent, CompiledInputMapping, CompiledInputMappings, DirectInputIntentDescriptor,
    RuntimeInputMapping, RuntimeInputTrigger,
};
pub use lane::RuntimeInputLane;
pub use model::{
    parse_canonical_u64, AxisValue, ButtonSnapshot, ControllerAxis, ControllerButton, InputAxis,
    InputClearReason, InputContext, InputEdge, InputFrame, IntentPhase, IntentProvenance,
    IntentValueKind, KeyboardControl, PhysicalEdge, PointerButton, RuntimeDirectIntentClaim,
    RuntimeInputBinding, RuntimeInputError, RuntimeInputEvent, RuntimeInputFact,
    RuntimeInputIngress, RuntimeIntentEnvelope, RuntimeIntentValue, RuntimeProductPayload,
    MAX_AXIS_MAGNITUDE, MAX_CONTROLLER_AXIS_MAGNITUDE, MAX_DIRECT_INTENT_AXIS_MAGNITUDE,
    MAX_DIRECT_INTENT_PRODUCT_PAYLOAD_ARRAY_ENTRIES, MAX_DIRECT_INTENT_PRODUCT_PAYLOAD_BYTES,
    MAX_DIRECT_INTENT_PRODUCT_PAYLOAD_DEPTH, MAX_DIRECT_INTENT_PRODUCT_PAYLOAD_JSON_BYTES,
    MAX_DIRECT_INTENT_PRODUCT_PAYLOAD_NODES, MAX_DIRECT_INTENT_PRODUCT_PAYLOAD_OBJECT_ENTRIES,
    MAX_DIRECT_INTENT_PRODUCT_PAYLOAD_SAFE_INTEGER, MAX_DIRECT_INTENT_PRODUCT_PAYLOAD_STRING_BYTES,
    MAX_PENDING_INGRESS,
};
pub use wire::{
    decode_runtime_input_wire_event_json, decode_runtime_input_wire_events_json,
    MAX_RUNTIME_INPUT_WIRE_BYTES, MAX_RUNTIME_INPUT_WIRE_EVENTS,
};
