//! Strict Rust adapters for the generated `developer-command-client` v1 host
//! envelope.
//!
//! These DTOs own only the common envelope. A product supplies the concrete
//! payload, reply, and owner-error types and maps them at its composition
//! boundary. The DTOs deliberately do not turn `TypeDescriptor` into a codec
//! or add transport, queue, authorization, or replay behavior.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{
    CommandBindings, CommandDescriptor, CommandId, CommandLane, CommandProvenance, CommandRequest,
    CommandResponse, DiscoverySnapshot, DispatchError, DispatchFacts, EnvelopeError, ExpectedFacts,
    HandlerResult, ProfileId, ProtocolVersion, RuntimeInstanceId, CURRENT_PROTOCOL_VERSION,
};

pub const MAX_HOST_RECEIPT_REFS: usize = 32;
pub const MAX_HOST_ERROR_MESSAGE_BYTES: usize = 1024;

/// A strict decimal-string representation of an unsigned 64-bit fact.
///
/// JSON numbers are intentionally not accepted: JavaScript cannot represent
/// every `u64` exactly, while the generated client contract uses decimal
/// strings for revisions and catalog epochs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HostDecimalU64(u64);

impl HostDecimalU64 {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

impl Serialize for HostDecimalU64 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for HostDecimalU64 {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        parse_decimal_u64(&value)
            .map(Self)
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HostExpectedFacts {
    pub profile: ProfileId,
    pub revision: HostDecimalU64,
    pub catalog_epoch: HostDecimalU64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HostCommandRequest<P> {
    pub protocol_version: ProtocolVersion,
    pub command: CommandId,
    pub correlation: crate::CorrelationId,
    pub runtime: RuntimeInstanceId,
    pub expected: HostExpectedFacts,
    pub payload: P,
}

impl<P> HostCommandRequest<P> {
    /// Maps the strict host envelope into the typed in-process request. The
    /// host contract has no cancellation or timeout fields; a product may set
    /// those flags on the returned request from its own queue state.
    pub fn into_command_request(self) -> Result<CommandRequest<P>, HostWireError> {
        self.into_command_parts().map(|(request, _context)| request)
    }

    /// Maps the strict host envelope and retains the response-only context
    /// needed when dispatch rejects before an owner is entered.
    pub fn into_command_parts(
        self,
    ) -> Result<(CommandRequest<P>, HostResponseContext), HostWireError> {
        if self.protocol_version != CURRENT_PROTOCOL_VERSION {
            return Err(HostWireError::UnsupportedProtocol {
                provided: self.protocol_version,
                supported: CURRENT_PROTOCOL_VERSION,
            });
        }
        if self.expected.profile.as_str().is_empty() {
            return Err(HostWireError::InvalidIdentity {
                field: "expected.profile",
            });
        }
        let response_context = HostResponseContext {
            correlation: self.correlation.clone(),
            profile: self.expected.profile.clone(),
        };
        Ok((
            CommandRequest {
                protocol_version: self.protocol_version,
                command: self.command,
                correlation: self.correlation,
                runtime: self.runtime,
                expected: ExpectedFacts {
                    profile: Some(self.expected.profile),
                    revision: Some(self.expected.revision.get()),
                    catalog_epoch: Some(self.expected.catalog_epoch.get()),
                },
                cancelled: false,
                timed_out: false,
                payload: self.payload,
            },
            response_context,
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostCommandResponse<R, E> {
    pub correlation: crate::CorrelationId,
    pub runtime: RuntimeInstanceId,
    pub profile: ProfileId,
    pub revision: HostDecimalU64,
    pub catalog_epoch: HostDecimalU64,
    pub outcome: HostCommandOutcome<R, E>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum HostCommandOutcome<R, E> {
    Success {
        value: R,
        #[serde(rename = "receiptRefs")]
        receipt_refs: HostReceiptRefs,
    },
    Error {
        code: HostErrorCode,
        message: HostErrorMessage,
        #[serde(skip_serializing_if = "Option::is_none")]
        details: Option<E>,
    },
}

/// Product-owned receipt references remain bounded identities at the common
/// envelope. Their meaning and persistence stay with the product owner.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HostReceiptRef(String);

impl HostReceiptRef {
    pub fn parse(value: impl Into<String>) -> Result<Self, HostWireError> {
        let value = value.into();
        crate::CommandId::parse(value.clone())
            .map(|_| Self(value))
            .map_err(|_| HostWireError::InvalidIdentity {
                field: "outcome.receiptRefs",
            })
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Serialize for HostReceiptRef {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct HostReceiptRefs(Vec<HostReceiptRef>);

impl HostReceiptRefs {
    pub fn new(values: Vec<HostReceiptRef>) -> Result<Self, HostWireError> {
        if values.len() > MAX_HOST_RECEIPT_REFS {
            return Err(HostWireError::TooManyReceiptRefs {
                maximum: MAX_HOST_RECEIPT_REFS,
                actual: values.len(),
            });
        }
        Ok(Self(values))
    }

    pub const fn empty() -> Self {
        Self(Vec::new())
    }

    pub fn as_slice(&self) -> &[HostReceiptRef] {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct HostErrorCode(String);

impl HostErrorCode {
    pub fn parse(value: impl Into<String>) -> Result<Self, HostWireError> {
        let value = value.into();
        CommandId::parse(value.clone())
            .map(|_| Self(value))
            .map_err(|_| HostWireError::InvalidIdentity {
                field: "outcome.code",
            })
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct HostErrorMessage(String);

impl HostErrorMessage {
    pub fn new(message: impl Into<String>) -> Result<Self, HostWireError> {
        let message = message.into();
        let actual = message.len();
        if actual > MAX_HOST_ERROR_MESSAGE_BYTES {
            return Err(HostWireError::MessageTooLong {
                maximum: MAX_HOST_ERROR_MESSAGE_BYTES,
                actual,
            });
        }
        Ok(Self(message))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostResponseContext {
    correlation: crate::CorrelationId,
    profile: ProfileId,
}

impl HostResponseContext {
    pub fn new(correlation: crate::CorrelationId, profile: ProfileId) -> Self {
        Self {
            correlation,
            profile,
        }
    }

    pub fn correlation(&self) -> &crate::CorrelationId {
        &self.correlation
    }

    pub fn profile(&self) -> &ProfileId {
        &self.profile
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostErrorPhase {
    PreDispatch,
    EnteredOwner,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostResponseMetadata {
    /// Internal provenance is retained for the product's local evidence even
    /// though the generated v1 response DTO intentionally carries only its
    /// six stable wire fields.
    pub provenance: Option<CommandProvenance>,
    pub error_phase: Option<HostErrorPhase>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MappedHostCommandResponse<R, E> {
    pub wire: HostCommandResponse<R, E>,
    pub metadata: HostResponseMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostErrorBody<E> {
    pub code: HostErrorCode,
    pub message: HostErrorMessage,
    pub details: Option<E>,
}

/// Maps an in-process response into the generated v1 host response while
/// retaining provenance and the error phase as a Rust-side sidecar.
///
/// The request context is used only when pre-dispatch rejection intentionally
/// has no provenance. Once an owner is entered, the reserved provenance is the
/// response authority. All fallible host values are admitted before this
/// function so mapping cannot lose an already-completed owner result.
pub fn map_command_response<R, E, M, D>(
    response: CommandResponse<R, E>,
    request_context: HostResponseContext,
    receipt_refs: HostReceiptRefs,
    map_error: M,
) -> MappedHostCommandResponse<R, D>
where
    M: FnOnce(E) -> HostErrorBody<D>,
{
    let provenance = response.provenance.clone();
    let (correlation, profile) = provenance
        .as_ref()
        .map(|entered| (entered.correlation.clone(), entered.profile.clone()))
        .unwrap_or((request_context.correlation, request_context.profile));
    let (outcome, error_phase) = match response.result {
        HandlerResult::Success(value) => (
            HostCommandOutcome::Success {
                value,
                receipt_refs,
            },
            None,
        ),
        HandlerResult::Rejected(DispatchError::Command(error)) => {
            let mapped = map_error(error);
            (
                HostCommandOutcome::Error {
                    code: mapped.code,
                    message: mapped.message,
                    details: mapped.details,
                },
                Some(HostErrorPhase::EnteredOwner),
            )
        }
        HandlerResult::Rejected(DispatchError::Envelope(error)) => {
            // A normal envelope rejection has no provenance because it failed
            // before reservation. Binding-invariant failures are the one
            // internal envelope case that can be produced after reservation,
            // so use the already-captured provenance as the phase authority.
            let phase = if provenance.is_some() {
                HostErrorPhase::EnteredOwner
            } else {
                HostErrorPhase::PreDispatch
            };
            (
                HostCommandOutcome::Error {
                    code: HostErrorCode::parse(envelope_code(&error))
                        .expect("static envelope code must be a valid command identity"),
                    message: HostErrorMessage::new(envelope_message(&error))
                        .expect("static envelope message must fit the host bound"),
                    details: None,
                },
                Some(phase),
            )
        }
    };
    MappedHostCommandResponse {
        wire: HostCommandResponse {
            correlation,
            runtime: response.facts.runtime,
            profile,
            revision: HostDecimalU64::new(response.facts.revision),
            catalog_epoch: HostDecimalU64::new(response.facts.catalog_epoch),
            outcome,
        },
        metadata: HostResponseMetadata {
            provenance,
            error_phase,
        },
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostCommandDescriptor {
    pub id: CommandId,
    pub aliases: Vec<crate::CommandAlias>,
    pub lane: String,
    pub summary: String,
}

impl Serialize for HostCommandDescriptor {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Wire<'a> {
            id: &'a CommandId,
            aliases: &'a [crate::CommandAlias],
            lane: &'a str,
            summary: &'a str,
        }
        Wire {
            id: &self.id,
            aliases: &self.aliases,
            lane: &self.lane,
            summary: &self.summary,
        }
        .serialize(serializer)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostCommandDiscovery {
    pub protocol_version: ProtocolVersion,
    pub runtime: RuntimeInstanceId,
    pub profile: ProfileId,
    pub permitted_lanes: Vec<String>,
    pub revision: HostDecimalU64,
    pub catalog_epoch: HostDecimalU64,
    pub contract_fingerprint: CommandId,
    pub commands: Vec<HostCommandDescriptor>,
}

impl HostCommandDiscovery {
    /// Builds the exact generated-client discovery shape from a bindings port.
    /// Unavailable declared descriptors stay out of the executable host list;
    /// the in-process [`DiscoverySnapshot`] remains available when a product
    /// needs declared/help-only inventory for diagnostics.
    pub fn from_bindings(bindings: &CommandBindings, contract_fingerprint: CommandId) -> Self {
        let snapshot = bindings.discover();
        Self::from_snapshot(
            snapshot,
            bindings.facts(),
            bindings.profile().id().clone(),
            CommandLane::ALL
                .into_iter()
                .filter(|lane| bindings.profile().permits(*lane))
                .map(lane_name)
                .map(str::to_owned)
                .collect(),
            contract_fingerprint,
        )
    }

    pub fn from_snapshot(
        snapshot: DiscoverySnapshot,
        facts: &DispatchFacts,
        profile: ProfileId,
        permitted_lanes: Vec<String>,
        contract_fingerprint: CommandId,
    ) -> Self {
        let commands = snapshot
            .commands
            .into_iter()
            .filter(|entry| entry.bound)
            .map(|entry| descriptor_wire(entry.descriptor))
            .collect();
        Self {
            protocol_version: CURRENT_PROTOCOL_VERSION,
            runtime: facts.runtime.clone(),
            profile,
            permitted_lanes,
            revision: HostDecimalU64::new(facts.revision),
            catalog_epoch: HostDecimalU64::new(facts.catalog_epoch),
            contract_fingerprint,
            commands,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostWireError {
    UnsupportedProtocol {
        provided: ProtocolVersion,
        supported: ProtocolVersion,
    },
    InvalidDecimal {
        field: &'static str,
    },
    InvalidIdentity {
        field: &'static str,
    },
    TooManyReceiptRefs {
        maximum: usize,
        actual: usize,
    },
    MessageTooLong {
        maximum: usize,
        actual: usize,
    },
}

impl std::fmt::Display for HostWireError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedProtocol {
                provided,
                supported,
            } => write!(
                formatter,
                "unsupported developer-command protocol {} (supported {})",
                provided.value(),
                supported.value()
            ),
            Self::InvalidDecimal { field } => {
                write!(formatter, "invalid decimal u64 field {field}")
            }
            Self::InvalidIdentity { field } => write!(formatter, "invalid identity field {field}"),
            Self::TooManyReceiptRefs { maximum, actual } => {
                write!(
                    formatter,
                    "too many receipt references: {actual} exceeds {maximum}"
                )
            }
            Self::MessageTooLong { maximum, actual } => {
                write!(
                    formatter,
                    "host error message is {actual} bytes; maximum is {maximum}"
                )
            }
        }
    }
}

impl std::error::Error for HostWireError {}

fn parse_decimal_u64(value: &str) -> Result<u64, HostWireError> {
    if value.is_empty()
        || value.len() > 20
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(HostWireError::InvalidDecimal {
            field: "revision/catalogEpoch",
        });
    }
    value.parse().map_err(|_| HostWireError::InvalidDecimal {
        field: "revision/catalogEpoch",
    })
}

fn descriptor_wire(descriptor: CommandDescriptor) -> HostCommandDescriptor {
    HostCommandDescriptor {
        id: descriptor.id().clone(),
        aliases: descriptor.aliases().to_vec(),
        lane: lane_name(descriptor.lane()).to_owned(),
        summary: descriptor.summary().to_owned(),
    }
}

fn lane_name(lane: CommandLane) -> &'static str {
    match lane {
        CommandLane::Inspect => "inspect",
        CommandLane::Preview => "preview",
        CommandLane::Play => "play",
        CommandLane::Admin => "admin",
        CommandLane::Session => "session",
        CommandLane::Author => "author",
        CommandLane::Fault => "fault",
    }
}

fn envelope_code(error: &EnvelopeError) -> &'static str {
    match error {
        EnvelopeError::UnsupportedProtocol { .. } => "unsupported_protocol",
        EnvelopeError::UnknownCommand { .. } => "unknown_command",
        EnvelopeError::CommandUnavailable { .. } => "command_unavailable",
        EnvelopeError::CommandMismatch { .. } => "command_mismatch",
        EnvelopeError::RuntimeMismatch { .. } => "runtime_mismatch",
        EnvelopeError::StaleRevision { .. } => "stale_revision",
        EnvelopeError::StaleCatalogEpoch { .. } => "stale_catalog_epoch",
        EnvelopeError::StaleProfile { .. } => "stale_profile",
        EnvelopeError::DuplicateCorrelation { .. } => "duplicate_correlation",
        EnvelopeError::CorrelationCapacityExceeded { .. } => "correlation_capacity_exceeded",
        EnvelopeError::CorrelationMismatch { .. } => "correlation_mismatch",
        EnvelopeError::Cancelled => "cancelled",
        EnvelopeError::TimedOut => "timed_out",
        EnvelopeError::SequenceExhausted => "sequence_exhausted",
        EnvelopeError::BindingInvariant => "binding_invariant",
    }
}

fn envelope_message(error: &EnvelopeError) -> String {
    format!(
        "developer command envelope rejected: {}",
        envelope_code(error)
    )
}
