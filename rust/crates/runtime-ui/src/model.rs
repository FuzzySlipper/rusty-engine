use std::fmt;

use runtime_lifecycle::{
    validate_runtime_identity, RuntimeControlRevision, RuntimeGeneration, RuntimeIdentityError,
    RuntimeInstanceId, RuntimeLifecycle, RuntimeLifecycleError, RuntimePhase, RuntimeState,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The current immutable UI projection artifact identity.
pub const RUNTIME_UI_PROJECTION_ARTIFACT: &str = "rusty.product.ui-projection";

/// Maximum number of distinct UI streams retained in one bound lane.
pub const MAX_RUNTIME_UI_PROJECTION_STREAMS: usize = 256;

/// Maximum compact JSON size of one copied product projection value.
pub const MAX_RUNTIME_UI_PROJECTION_VALUE_JSON_BYTES: usize = 65_536;

/// Maximum JSON node count accepted in one copied value. These shape bounds
/// are shared with the rich-DOM application host so Rust cannot emit a value
/// the host would later reject.
pub const MAX_RUNTIME_UI_PROJECTION_VALUE_NODES: usize = 2_048;

/// Maximum object/array nesting depth accepted in one copied value.
pub const MAX_RUNTIME_UI_PROJECTION_VALUE_DEPTH: usize = 16;

/// Maximum UTF-8 byte length of one string value in a copied DTO.
pub const MAX_RUNTIME_UI_PROJECTION_VALUE_STRING_BYTES: usize = 8_192;

/// Maximum entries in one array value.
pub const MAX_RUNTIME_UI_PROJECTION_VALUE_ARRAY_LENGTH: usize = 512;

/// Maximum keys in one object value.
pub const MAX_RUNTIME_UI_PROJECTION_VALUE_OBJECT_KEYS: usize = 256;

/// Largest integer-valued JSON number that crosses the JavaScript host
/// boundary without losing precision.
pub const MAX_RUNTIME_UI_PROJECTION_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

/// Maximum encoded envelope size accepted from or emitted to a host.
pub const MAX_RUNTIME_UI_PROJECTION_WIRE_BYTES: usize = 262_144;

/// Typed lifecycle identity retained by one UI projection envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RuntimeUiRuntimeBinding {
    instance_id: RuntimeInstanceId,
    generation: RuntimeGeneration,
    control_revision: RuntimeControlRevision,
}

impl RuntimeUiRuntimeBinding {
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

impl From<&RuntimeLifecycle> for RuntimeUiRuntimeBinding {
    fn from(lifecycle: &RuntimeLifecycle) -> Self {
        Self::new(
            lifecycle.instance_id(),
            lifecycle.generation(),
            lifecycle.control_revision(),
        )
    }
}

/// An owned, validated UI projection transport envelope.
///
/// The fields are private and there are no mutating accessors. Constructing
/// an envelope validates Product Model identities, canonical runtime values,
/// and the copied value's compact JSON bound. Use [`Self::encode_json`] to
/// obtain the strict host wire shape.
#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeUiProjectionEnvelope {
    runtime: RuntimeUiRuntimeBinding,
    sequence: u64,
    stream: String,
    contract: String,
    value: Value,
}

impl RuntimeUiProjectionEnvelope {
    /// Creates one validated envelope from typed runtime facts and an owned
    /// JSON value. The sequence is normally the admitted simulation step.
    pub fn new(
        runtime: RuntimeUiRuntimeBinding,
        sequence: u64,
        stream: impl Into<String>,
        contract: impl Into<String>,
        value: Value,
    ) -> Result<Self, RuntimeUiProjectionError> {
        let stream = validate_identity("stream", stream.into())?;
        let contract = validate_identity("contract", contract.into())?;
        validate_value(&value)?;
        let envelope = Self {
            runtime,
            sequence,
            stream,
            contract,
            value,
        };
        envelope.encoded_bytes()?;
        Ok(envelope)
    }

    pub const fn runtime(&self) -> RuntimeUiRuntimeBinding {
        self.runtime
    }

    /// Rebinds an already admitted projection to a newer Engine lifecycle
    /// identity without changing its copied product payload or stream state.
    ///
    /// This is for Engine-owned staging when a callback runs before a
    /// lifecycle action commits. `RuntimeUiRuntimeBinding` is already typed,
    /// while the stream, contract, and value were validated by [`Self::new`].
    pub fn with_runtime(mut self, runtime: RuntimeUiRuntimeBinding) -> Self {
        self.runtime = runtime;
        self
    }

    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub fn stream(&self) -> &str {
        &self.stream
    }

    pub fn contract(&self) -> &str {
        &self.contract
    }

    /// Returns an immutable view of the copied product DTO JSON.
    pub fn value(&self) -> &Value {
        &self.value
    }

    pub fn into_value(self) -> Value {
        self.value
    }

    /// Encodes the strict compact wire envelope. Runtime identifiers and the
    /// sequence are decimal strings so JavaScript cannot lose u64 precision.
    pub fn encode_json(&self) -> Result<Vec<u8>, RuntimeUiProjectionError> {
        self.encoded_bytes()
    }

    pub fn encode_json_string(&self) -> Result<String, RuntimeUiProjectionError> {
        String::from_utf8(self.encode_json()?).map_err(|_| RuntimeUiProjectionError::WireEncoding)
    }

    /// Strictly decodes the current wire shape, rejecting unknown fields and
    /// any non-whitespace trailing bytes.
    pub fn decode_json(bytes: &[u8]) -> Result<Self, RuntimeUiProjectionError> {
        if bytes.len() > MAX_RUNTIME_UI_PROJECTION_WIRE_BYTES {
            return Err(RuntimeUiProjectionError::WireTooLarge {
                bytes: bytes.len(),
                maximum: MAX_RUNTIME_UI_PROJECTION_WIRE_BYTES,
            });
        }
        let mut decoder = serde_json::Deserializer::from_slice(bytes);
        let wire = WireEnvelope::deserialize(&mut decoder)
            .map_err(|_| RuntimeUiProjectionError::WireMalformed)?;
        decoder
            .end()
            .map_err(|_| RuntimeUiProjectionError::WireMalformed)?;
        wire.into_envelope()
    }

    fn encoded_bytes(&self) -> Result<Vec<u8>, RuntimeUiProjectionError> {
        let wire = WireEnvelope::from(self);
        let bytes =
            serde_json::to_vec(&wire).map_err(|_| RuntimeUiProjectionError::WireEncoding)?;
        if bytes.len() > MAX_RUNTIME_UI_PROJECTION_WIRE_BYTES {
            return Err(RuntimeUiProjectionError::WireTooLarge {
                bytes: bytes.len(),
                maximum: MAX_RUNTIME_UI_PROJECTION_WIRE_BYTES,
            });
        }
        Ok(bytes)
    }
}

/// Encodes one validated UI projection envelope to its strict wire bytes.
pub fn encode_runtime_ui_projection_json(
    envelope: &RuntimeUiProjectionEnvelope,
) -> Result<Vec<u8>, RuntimeUiProjectionError> {
    envelope.encode_json()
}

/// Decodes one strict UI projection envelope from host wire bytes.
pub fn decode_runtime_ui_projection_json(
    bytes: &[u8],
) -> Result<RuntimeUiProjectionEnvelope, RuntimeUiProjectionError> {
    RuntimeUiProjectionEnvelope::decode_json(bytes)
}

/// Read-only facts about a bound UI projection lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeUiProjectionReadout {
    pub(crate) runtime: RuntimeUiRuntimeBinding,
    pub(crate) stream_count: usize,
    pub(crate) disposed: bool,
}

impl RuntimeUiProjectionReadout {
    pub const fn runtime(self) -> RuntimeUiRuntimeBinding {
        self.runtime
    }

    pub const fn stream_count(self) -> usize {
        self.stream_count
    }

    pub const fn is_disposed(self) -> bool {
        self.disposed
    }
}

/// Rejections from context, lifecycle, wire, identity, and bounded transport
/// validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeUiProjectionError {
    Disposed,
    LifecycleNotRunning {
        state: RuntimeState,
    },
    RebindNotRunning {
        state: RuntimeState,
    },
    RebindForeignInstance {
        expected: RuntimeInstanceId,
        received: RuntimeInstanceId,
    },
    RebindRegression {
        expected: RuntimeUiRuntimeBinding,
        received: RuntimeUiRuntimeBinding,
    },
    RebindRequired {
        expected: RuntimeUiRuntimeBinding,
        received: RuntimeUiRuntimeBinding,
    },
    LifecycleBindingChanged,
    WrongPhase {
        expected: RuntimePhase,
        received: RuntimePhase,
    },
    Lifecycle(RuntimeLifecycleError),
    InvalidIdentity {
        field: &'static str,
        value: String,
        diagnostic: Box<RuntimeIdentityError>,
    },
    StreamLimit {
        maximum: usize,
    },
    ContractChanged {
        stream: String,
        previous: String,
        received: String,
    },
    DuplicateSequence {
        stream: String,
        sequence: u64,
    },
    SequenceRegression {
        stream: String,
        previous: u64,
        received: u64,
    },
    ValueTooLarge {
        bytes: usize,
        maximum: usize,
    },
    ValueNodeLimit {
        nodes: usize,
        maximum: usize,
    },
    ValueDepthLimit {
        depth: usize,
        maximum: usize,
    },
    ValueStringLimit {
        bytes: usize,
        maximum: usize,
    },
    ValueArrayLimit {
        entries: usize,
        maximum: usize,
    },
    ValueObjectLimit {
        entries: usize,
        maximum: usize,
    },
    ValueUnsafeInteger {
        value: String,
    },
    ValueEncoding(String),
    WireTooLarge {
        bytes: usize,
        maximum: usize,
    },
    WireMalformed,
    WireUnknownArtifact {
        received: String,
    },
    WireNonCanonicalInteger {
        field: &'static str,
    },
    WireIntegerOutOfRange {
        field: &'static str,
    },
    WireEncoding,
}

impl fmt::Display for RuntimeUiProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "runtime UI projection rejected: {self:?}")
    }
}

impl std::error::Error for RuntimeUiProjectionError {}

pub(crate) fn validate_identity(
    field: &'static str,
    value: String,
) -> Result<String, RuntimeUiProjectionError> {
    if let Err(diagnostic) = validate_runtime_identity(&value) {
        return Err(RuntimeUiProjectionError::InvalidIdentity {
            field,
            value: value.clone(),
            diagnostic: Box::new(diagnostic),
        });
    }
    Ok(value)
}

pub(crate) fn validate_value(value: &Value) -> Result<(), RuntimeUiProjectionError> {
    let bytes = serde_json::to_vec(value)
        .map_err(|error| RuntimeUiProjectionError::ValueEncoding(error.to_string()))?;
    if bytes.len() > MAX_RUNTIME_UI_PROJECTION_VALUE_JSON_BYTES {
        return Err(RuntimeUiProjectionError::ValueTooLarge {
            bytes: bytes.len(),
            maximum: MAX_RUNTIME_UI_PROJECTION_VALUE_JSON_BYTES,
        });
    }
    let mut nodes = 0;
    validate_value_shape(value, 0, &mut nodes)
}

fn validate_value_shape(
    value: &Value,
    depth: usize,
    nodes: &mut usize,
) -> Result<(), RuntimeUiProjectionError> {
    *nodes = nodes
        .checked_add(1)
        .ok_or(RuntimeUiProjectionError::ValueNodeLimit {
            nodes: usize::MAX,
            maximum: MAX_RUNTIME_UI_PROJECTION_VALUE_NODES,
        })?;
    if *nodes > MAX_RUNTIME_UI_PROJECTION_VALUE_NODES {
        return Err(RuntimeUiProjectionError::ValueNodeLimit {
            nodes: *nodes,
            maximum: MAX_RUNTIME_UI_PROJECTION_VALUE_NODES,
        });
    }
    if depth > MAX_RUNTIME_UI_PROJECTION_VALUE_DEPTH {
        return Err(RuntimeUiProjectionError::ValueDepthLimit {
            depth,
            maximum: MAX_RUNTIME_UI_PROJECTION_VALUE_DEPTH,
        });
    }
    match value {
        Value::Null | Value::Bool(_) => {}
        Value::Number(number) => validate_number(number)?,
        Value::String(text) => {
            if text.len() > MAX_RUNTIME_UI_PROJECTION_VALUE_STRING_BYTES {
                return Err(RuntimeUiProjectionError::ValueStringLimit {
                    bytes: text.len(),
                    maximum: MAX_RUNTIME_UI_PROJECTION_VALUE_STRING_BYTES,
                });
            }
        }
        Value::Array(values) => {
            if values.len() > MAX_RUNTIME_UI_PROJECTION_VALUE_ARRAY_LENGTH {
                return Err(RuntimeUiProjectionError::ValueArrayLimit {
                    entries: values.len(),
                    maximum: MAX_RUNTIME_UI_PROJECTION_VALUE_ARRAY_LENGTH,
                });
            }
            for value in values {
                validate_value_shape(value, depth + 1, nodes)?;
            }
        }
        Value::Object(values) => {
            if values.len() > MAX_RUNTIME_UI_PROJECTION_VALUE_OBJECT_KEYS {
                return Err(RuntimeUiProjectionError::ValueObjectLimit {
                    entries: values.len(),
                    maximum: MAX_RUNTIME_UI_PROJECTION_VALUE_OBJECT_KEYS,
                });
            }
            for value in values.values() {
                validate_value_shape(value, depth + 1, nodes)?;
            }
        }
    }
    Ok(())
}

fn validate_number(number: &serde_json::Number) -> Result<(), RuntimeUiProjectionError> {
    let unsafe_integer = if let Some(value) = number.as_i64() {
        value.unsigned_abs() > MAX_RUNTIME_UI_PROJECTION_SAFE_INTEGER
    } else if let Some(value) = number.as_u64() {
        value > MAX_RUNTIME_UI_PROJECTION_SAFE_INTEGER
    } else if let Some(value) = number.as_f64() {
        value.fract() == 0.0 && value.abs() > MAX_RUNTIME_UI_PROJECTION_SAFE_INTEGER as f64
    } else {
        true
    };
    if unsafe_integer {
        return Err(RuntimeUiProjectionError::ValueUnsafeInteger {
            value: number.to_string(),
        });
    }
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireEnvelope {
    artifact: String,
    runtime: WireRuntime,
    sequence: String,
    stream: String,
    contract: String,
    value: Value,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireRuntime {
    instance_id: String,
    generation: String,
    control_revision: String,
}

impl From<&RuntimeUiProjectionEnvelope> for WireEnvelope {
    fn from(envelope: &RuntimeUiProjectionEnvelope) -> Self {
        Self {
            artifact: RUNTIME_UI_PROJECTION_ARTIFACT.to_owned(),
            runtime: WireRuntime {
                instance_id: envelope.runtime.instance_id().value().to_string(),
                generation: envelope.runtime.generation().value().to_string(),
                control_revision: envelope.runtime.control_revision().value().to_string(),
            },
            sequence: envelope.sequence.to_string(),
            stream: envelope.stream.clone(),
            contract: envelope.contract.clone(),
            value: envelope.value.clone(),
        }
    }
}

impl WireEnvelope {
    fn into_envelope(self) -> Result<RuntimeUiProjectionEnvelope, RuntimeUiProjectionError> {
        if self.artifact != RUNTIME_UI_PROJECTION_ARTIFACT {
            return Err(RuntimeUiProjectionError::WireUnknownArtifact {
                received: self.artifact,
            });
        }
        let runtime = RuntimeUiRuntimeBinding::new(
            RuntimeInstanceId::new(parse_canonical_u64(
                "runtime.instanceId",
                &self.runtime.instance_id,
            )?),
            RuntimeGeneration::new(parse_canonical_u64(
                "runtime.generation",
                &self.runtime.generation,
            )?),
            RuntimeControlRevision::new(parse_canonical_u64(
                "runtime.controlRevision",
                &self.runtime.control_revision,
            )?),
        );
        let sequence = parse_canonical_u64("sequence", &self.sequence)?;
        RuntimeUiProjectionEnvelope::new(runtime, sequence, self.stream, self.contract, self.value)
    }
}

fn parse_canonical_u64(field: &'static str, value: &str) -> Result<u64, RuntimeUiProjectionError> {
    if value.is_empty()
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(RuntimeUiProjectionError::WireNonCanonicalInteger { field });
    }
    value
        .parse::<u64>()
        .map_err(|_| RuntimeUiProjectionError::WireIntegerOutOfRange { field })
}
