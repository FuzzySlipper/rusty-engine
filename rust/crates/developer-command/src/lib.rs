//! Host-neutral, typed developer-command contracts and in-process dispatch.
//!
//! A downstream product constructs one [`CommandBindings`] instance at its own
//! composition root, binds explicit handlers, and calls [`CommandBindings::dispatch`]
//! from the product's existing queue at a product-selected safe point. This crate
//! owns neither that queue nor any world, scheduler, transport, filesystem, or
//! service locator. Lanes describe commands; they never arrive from a request.

#![forbid(unsafe_code)]

mod descriptor;
mod dispatch;
mod identity;
mod wire;

pub use descriptor::{
    CommandDescriptor, CommandDescriptorError, CommandLane, CommandProfile, DiscoveryEntry,
    DiscoverySnapshot, ParameterDescriptor, TypeDescriptor, TypeDescriptorError,
    MAX_COMMAND_ALIASES, MAX_DESCRIPTOR_COLLECTION_ITEMS, MAX_DESCRIPTOR_DEPTH,
    MAX_DESCRIPTOR_NODES, MAX_DESCRIPTOR_STRING_BYTES, MAX_DISCOVERED_COMMANDS,
    MAX_PARAMETERS_PER_COMMAND,
};
pub use dispatch::{
    CommandBindings, CommandBindingsError, CommandContext, CommandHandler, CommandHistoryEntry,
    CommandHistoryOutcome, CommandProvenance, CommandRequest, CommandResponse, DeveloperCommand,
    DispatchError, DispatchFacts, EnvelopeError, ExpectedFacts, HandlerResult, MAX_COMMAND_HISTORY,
    MAX_TRACKED_CORRELATIONS,
};
pub use identity::{
    CommandAlias, CommandId, CommandIdentityError, CorrelationId, ProfileId, ProtocolVersion,
    RuntimeInstanceId, CURRENT_PROTOCOL_VERSION, MAX_COMMAND_ID_BYTES, MAX_CORRELATION_ID_BYTES,
    MAX_PROFILE_ID_BYTES, MAX_RUNTIME_INSTANCE_ID_BYTES,
};
pub use wire::{
    developer_command_wire_contract, developer_command_wire_contract_json,
    DeveloperCommandWireContract, WireIdentityBounds, WireLimits, WireSequenceContract,
};
