//! Explicit host wire contract for the generic developer-command envelope.
//!
//! This is intentionally separate from discovery `TypeDescriptor`: descriptors
//! help a person choose a command, while this contract fixes the envelope,
//! correlation, context, error, history, and sequence wire vocabulary.

use serde::Serialize;

use crate::{
    CommandLane, CURRENT_PROTOCOL_VERSION, MAX_COMMAND_ALIASES, MAX_COMMAND_HISTORY,
    MAX_COMMAND_ID_BYTES, MAX_CORRELATION_ID_BYTES, MAX_DESCRIPTOR_STRING_BYTES,
    MAX_PROFILE_ID_BYTES, MAX_RUNTIME_INSTANCE_ID_BYTES,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeveloperCommandWireContract {
    pub kind: &'static str,
    pub protocol_version: u16,
    pub identity: WireIdentityBounds,
    pub limits: WireLimits,
    pub lanes: Vec<&'static str>,
    pub discovery_fields: [&'static str; 8],
    pub request_fields: [&'static str; 6],
    pub response_fields: [&'static str; 6],
    pub outcomes: [&'static str; 2],
    pub sequence: WireSequenceContract,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WireIdentityBounds {
    pub command_bytes: usize,
    pub correlation_bytes: usize,
    pub runtime_bytes: usize,
    pub profile_bytes: usize,
    pub charset: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WireLimits {
    pub command_aliases: usize,
    pub summary_bytes: usize,
    pub history_entries: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WireSequenceContract {
    pub kind: &'static str,
    pub deterministic_replay: bool,
    pub required_entry_facts: [&'static str; 6],
}

pub fn developer_command_wire_contract() -> DeveloperCommandWireContract {
    DeveloperCommandWireContract {
        kind: "rusty-developer-command.v1",
        protocol_version: CURRENT_PROTOCOL_VERSION.value(),
        identity: WireIdentityBounds {
            command_bytes: MAX_COMMAND_ID_BYTES,
            correlation_bytes: MAX_CORRELATION_ID_BYTES,
            runtime_bytes: MAX_RUNTIME_INSTANCE_ID_BYTES,
            profile_bytes: MAX_PROFILE_ID_BYTES,
            charset: "lowercase-ascii-alnum-dot-dash-underscore-colon",
        },
        limits: WireLimits {
            command_aliases: MAX_COMMAND_ALIASES,
            summary_bytes: MAX_DESCRIPTOR_STRING_BYTES,
            history_entries: MAX_COMMAND_HISTORY,
        },
        lanes: CommandLane::ALL
            .into_iter()
            .map(|lane| match lane {
                CommandLane::Inspect => "inspect",
                CommandLane::Preview => "preview",
                CommandLane::Play => "play",
                CommandLane::Admin => "admin",
                CommandLane::Session => "session",
                CommandLane::Author => "author",
                CommandLane::Fault => "fault",
            })
            .collect(),
        discovery_fields: [
            "protocolVersion",
            "runtime",
            "profile",
            "permittedLanes",
            "revision",
            "catalogEpoch",
            "contractFingerprint",
            "commands",
        ],
        request_fields: [
            "protocolVersion",
            "command",
            "correlation",
            "runtime",
            "expected",
            "payload",
        ],
        response_fields: [
            "correlation",
            "runtime",
            "profile",
            "revision",
            "catalogEpoch",
            "outcome",
        ],
        outcomes: ["success", "error"],
        sequence: WireSequenceContract {
            kind: "rusty_developer_command.sequence.v1",
            deterministic_replay: false,
            required_entry_facts: [
                "runtime",
                "profile",
                "revision",
                "catalogEpoch",
                "outcome",
                "receiptRefs",
            ],
        },
    }
}

pub fn developer_command_wire_contract_json() -> String {
    serde_json::to_string_pretty(&developer_command_wire_contract())
        .expect("developer command wire contract is serializable")
        + "\n"
}

#[cfg(test)]
mod tests {
    use super::developer_command_wire_contract_json;

    #[test]
    fn committed_typescript_generation_input_matches_rust_contract() {
        assert_eq!(
            developer_command_wire_contract_json(),
            include_str!("../../../../render/contracts/developer-command-contract.json"),
        );
    }
}
