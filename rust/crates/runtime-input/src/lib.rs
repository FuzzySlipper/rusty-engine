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

pub use compile::{CompiledInputIntent, CompiledInputMapping, CompiledInputMappings};
pub use lane::RuntimeInputLane;
pub use model::{
    parse_canonical_u64, AxisValue, ButtonSnapshot, InputClearReason, InputContext, InputFrame,
    IntentPhase, IntentProvenance, PhysicalEdge, RuntimeDirectIntentClaim, RuntimeInputBinding,
    RuntimeInputError, RuntimeInputEvent, RuntimeInputFact, RuntimeInputIngress,
    RuntimeIntentEnvelope, RuntimeIntentValue, MAX_AXIS_MAGNITUDE, MAX_CONTROLLER_AXIS_MAGNITUDE,
    MAX_DIRECT_INTENT_AXIS_MAGNITUDE, MAX_PENDING_INGRESS,
};
pub use wire::{
    decode_runtime_input_wire_event_json, decode_runtime_input_wire_events_json,
    MAX_RUNTIME_INPUT_WIRE_BYTES, MAX_RUNTIME_INPUT_WIRE_EVENTS,
};
