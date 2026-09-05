//! Bounded, host-neutral runtime diagnostics.
//!
//! This crate owns the diagnostic vocabulary, retained reader ring, cursors,
//! and call-local update attribution. A concrete host may attach one durable
//! writer, but filesystem policy and browser transport remain outside here.

use std::{
    collections::VecDeque,
    fmt,
    sync::{Arc, Mutex},
    time::Instant,
};

use serde::{Deserialize, Serialize, Serializer};

const DEFAULT_RING_CAPACITY: usize = 256;
const MAX_RING_CAPACITY: usize = 1_024;
const MAX_SOURCE_BYTES: usize = 64;
const MAX_CODE_BYTES: usize = 128;
const MAX_MESSAGE_BYTES: usize = 1_024;
const MAX_CORRELATION_BYTES: usize = 128;
const MAX_FIELDS: usize = 8;
const MAX_FIELD_KEY_BYTES: usize = 64;
const MAX_FIELD_VALUE_BYTES: usize = 256;
const MAX_BATCH_EVENTS: usize = 64;
const MAX_BATCH_BYTES: usize = 128 * 1024;
const MAX_RECOVERABLE_CODES: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeDiagnosticsError {
    code: &'static str,
    detail: String,
}

impl RuntimeDiagnosticsError {
    fn new(code: &'static str, detail: impl Into<String>) -> Self {
        let mut detail = detail.into();
        if detail.len() > 512 {
            let mut end = 512;
            while !detail.is_char_boundary(end) {
                end -= 1;
            }
            detail.truncate(end);
        }
        Self { code, detail }
    }

    pub const fn code(&self) -> &'static str {
        self.code
    }

    pub fn detail(&self) -> &str {
        &self.detail
    }
}

impl fmt::Display for RuntimeDiagnosticsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.detail)
    }
}

impl std::error::Error for RuntimeDiagnosticsError {}

/// A JSON u64 represented as canonical decimal text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CanonicalU64(u64);

impl CanonicalU64 {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }

    pub fn decode_json(bytes: &[u8]) -> Result<Self, RuntimeDiagnosticsError> {
        serde_json::from_slice(bytes).map_err(|_| {
            RuntimeDiagnosticsError::new(
                "RUNTIME_DIAGNOSTICS_CANONICAL_U64",
                "canonical u64 JSON is invalid",
            )
        })
    }
}

impl fmt::Display for CanonicalU64 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl Serialize for CanonicalU64 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for CanonicalU64 {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        if raw.is_empty()
            || (raw.len() > 1 && raw.starts_with('0'))
            || !raw.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(serde::de::Error::custom(
                "u64 must be canonical decimal text",
            ));
        }
        raw.parse().map(Self).map_err(serde::de::Error::custom)
    }
}

/// Runtime provenance attached to a diagnostic without depending on a host
/// transport binding type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeDiagnosticRuntimeBinding {
    pub instance_id: CanonicalU64,
    pub generation: CanonicalU64,
    pub control_revision: CanonicalU64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeDiagnosticSeverity {
    Debug,
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeDiagnosticDisposition {
    Accepted,
    RejectedRecoverable,
    Degraded,
    ResyncRequired,
    Terminal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeDiagnosticField {
    key: String,
    value: String,
}

impl RuntimeDiagnosticField {
    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn value(&self) -> &str {
        &self.value
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeDiagnosticEvent {
    #[serde(serialize_with = "serialize_u64_as_string")]
    sequence: u64,
    #[serde(serialize_with = "serialize_u64_as_string")]
    monotonic_nanoseconds: u64,
    severity: RuntimeDiagnosticSeverity,
    disposition: RuntimeDiagnosticDisposition,
    source: String,
    code: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    runtime: Option<RuntimeDiagnosticRuntimeBinding>,
    #[serde(skip_serializing_if = "Option::is_none")]
    correlation: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    fields: Vec<RuntimeDiagnosticField>,
}

impl RuntimeDiagnosticEvent {
    pub fn new(
        severity: RuntimeDiagnosticSeverity,
        disposition: RuntimeDiagnosticDisposition,
        source: impl Into<String>,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Result<Self, RuntimeDiagnosticsError> {
        Ok(Self {
            sequence: 0,
            monotonic_nanoseconds: 0,
            severity,
            disposition,
            source: bounded_identity(source.into(), MAX_SOURCE_BYTES, "source")?,
            code: bounded_identity(code.into(), MAX_CODE_BYTES, "code")?,
            message: bounded_text(message.into(), MAX_MESSAGE_BYTES, "message")?,
            runtime: None,
            correlation: None,
            fields: Vec::new(),
        })
    }

    pub fn with_runtime(mut self, runtime: impl Into<RuntimeDiagnosticRuntimeBinding>) -> Self {
        self.runtime = Some(runtime.into());
        self
    }

    pub fn with_correlation(
        mut self,
        correlation: impl Into<String>,
    ) -> Result<Self, RuntimeDiagnosticsError> {
        self.correlation = Some(bounded_identity(
            correlation.into(),
            MAX_CORRELATION_BYTES,
            "correlation",
        )?);
        Ok(self)
    }

    pub fn with_field(
        mut self,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<Self, RuntimeDiagnosticsError> {
        if self.fields.len() == MAX_FIELDS {
            return Err(RuntimeDiagnosticsError::new(
                "RUNTIME_DIAGNOSTICS_FIELDS",
                "diagnostic event has too many fields",
            ));
        }
        let key = bounded_identity(key.into(), MAX_FIELD_KEY_BYTES, "field key")?;
        if self.fields.iter().any(|field| field.key == key) {
            return Err(RuntimeDiagnosticsError::new(
                "RUNTIME_DIAGNOSTICS_FIELDS",
                "diagnostic event fields must have unique keys",
            ));
        }
        self.fields.push(RuntimeDiagnosticField {
            key,
            value: bounded_text(value.into(), MAX_FIELD_VALUE_BYTES, "field value")?,
        });
        Ok(self)
    }

    pub fn code(&self) -> &str {
        &self.code
    }
    pub const fn severity(&self) -> RuntimeDiagnosticSeverity {
        self.severity
    }
    pub const fn disposition(&self) -> RuntimeDiagnosticDisposition {
        self.disposition
    }
    pub fn source(&self) -> &str {
        &self.source
    }
    pub fn message(&self) -> &str {
        &self.message
    }
    pub fn runtime(&self) -> Option<RuntimeDiagnosticRuntimeBinding> {
        self.runtime
    }
    pub fn correlation(&self) -> Option<&str> {
        self.correlation.as_deref()
    }
    pub fn fields(&self) -> &[RuntimeDiagnosticField] {
        &self.fields
    }
}

/// One durable writer owned by a concrete host. The sink remains usable when
/// this writer fails; persistence policy stays with that host.
pub trait RuntimeDiagnosticsWriter: Send + Sync {
    fn write(&self, event: &RuntimeDiagnosticEvent, flush: bool) -> Result<(), String>;
    fn flush(&self) -> Result<(), String>;
}

#[derive(Debug, Clone)]
pub struct RuntimeDiagnosticsConfig {
    ring_capacity: usize,
}

impl Default for RuntimeDiagnosticsConfig {
    fn default() -> Self {
        Self {
            ring_capacity: DEFAULT_RING_CAPACITY,
        }
    }
}

impl RuntimeDiagnosticsConfig {
    pub fn with_ring_capacity(mut self, capacity: usize) -> Self {
        self.ring_capacity = capacity;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeDiagnosticsSnapshot {
    pub events: Vec<RuntimeDiagnosticEvent>,
    pub warning_count: u64,
    pub error_count: u64,
    pub dropped_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeDiagnosticsBatch {
    pub events: Vec<RuntimeDiagnosticEvent>,
    #[serde(serialize_with = "serialize_u64_as_string")]
    pub floor_sequence: u64,
    #[serde(serialize_with = "serialize_u64_as_string")]
    pub through_sequence: u64,
    #[serde(serialize_with = "serialize_u64_as_string")]
    pub next_cursor: u64,
    #[serde(serialize_with = "serialize_u64_as_string")]
    pub read_monotonic_nanoseconds: u64,
    pub lagged: bool,
    #[serde(serialize_with = "serialize_u64_as_string")]
    pub warning_count: u64,
    #[serde(serialize_with = "serialize_u64_as_string")]
    pub error_count: u64,
    #[serde(serialize_with = "serialize_u64_as_string")]
    pub dropped_count: u64,
}

#[derive(Clone)]
pub struct RuntimeDiagnosticsSink(Arc<Mutex<RuntimeDiagnosticsInner>>);

struct RuntimeDiagnosticsInner {
    started: Instant,
    next_sequence: u64,
    events: VecDeque<RuntimeDiagnosticEvent>,
    ring_capacity: usize,
    warning_count: u64,
    error_count: u64,
    dropped_count: u64,
    recoverable_codes: VecDeque<String>,
    writer: Option<Arc<dyn RuntimeDiagnosticsWriter>>,
}

impl RuntimeDiagnosticsSink {
    pub fn new(config: RuntimeDiagnosticsConfig) -> Result<Self, RuntimeDiagnosticsError> {
        Self::with_writer(config, None)
    }

    pub fn with_writer(
        config: RuntimeDiagnosticsConfig,
        writer: Option<Arc<dyn RuntimeDiagnosticsWriter>>,
    ) -> Result<Self, RuntimeDiagnosticsError> {
        if config.ring_capacity == 0 || config.ring_capacity > MAX_RING_CAPACITY {
            return Err(RuntimeDiagnosticsError::new(
                "RUNTIME_DIAGNOSTICS_CONFIG",
                "diagnostic ring capacity is outside fixed bounds",
            ));
        }
        Ok(Self(Arc::new(Mutex::new(RuntimeDiagnosticsInner {
            started: Instant::now(),
            next_sequence: 1,
            events: VecDeque::new(),
            ring_capacity: config.ring_capacity,
            warning_count: 0,
            error_count: 0,
            dropped_count: 0,
            recoverable_codes: VecDeque::new(),
            writer,
        }))))
    }

    pub fn publish(
        &self,
        mut event: RuntimeDiagnosticEvent,
    ) -> Result<(), RuntimeDiagnosticsError> {
        let mut inner = self.0.lock().map_err(|_| {
            RuntimeDiagnosticsError::new(
                "RUNTIME_DIAGNOSTICS_POISONED",
                "diagnostic sink lock is poisoned",
            )
        })?;
        if event.disposition == RuntimeDiagnosticDisposition::RejectedRecoverable {
            if inner.recoverable_codes.contains(&event.code) {
                return Ok(());
            }
            if inner.recoverable_codes.len() == MAX_RECOVERABLE_CODES {
                inner.recoverable_codes.pop_front();
            }
            inner.recoverable_codes.push_back(event.code.clone());
            event.severity = RuntimeDiagnosticSeverity::Warning;
        }
        event.sequence = inner.next_sequence;
        inner.next_sequence = inner.next_sequence.checked_add(1).ok_or_else(|| {
            RuntimeDiagnosticsError::new(
                "RUNTIME_DIAGNOSTICS_SEQUENCE",
                "diagnostic sequence exhausted",
            )
        })?;
        event.monotonic_nanoseconds = monotonic_nanoseconds(inner.started);
        match event.severity {
            RuntimeDiagnosticSeverity::Warning => {
                inner.warning_count = inner.warning_count.saturating_add(1)
            }
            RuntimeDiagnosticSeverity::Error => {
                inner.error_count = inner.error_count.saturating_add(1)
            }
            _ => {}
        }
        if inner.events.len() == inner.ring_capacity {
            inner.events.pop_front();
            inner.dropped_count = inner.dropped_count.saturating_add(1);
        }
        let flush = matches!(
            event.severity,
            RuntimeDiagnosticSeverity::Warning | RuntimeDiagnosticSeverity::Error
        );
        if inner
            .writer
            .as_ref()
            .is_some_and(|writer| writer.write(&event, flush).is_err())
        {
            inner.dropped_count = inner.dropped_count.saturating_add(1);
        }
        inner.events.push_back(event);
        Ok(())
    }

    pub fn flush(&self) {
        if let Ok(inner) = self.0.lock() {
            let _ = inner.writer.as_ref().map(|writer| writer.flush());
        }
    }

    pub fn now_monotonic_nanoseconds(&self) -> Option<u64> {
        self.0
            .lock()
            .ok()
            .map(|inner| monotonic_nanoseconds(inner.started))
    }

    pub fn snapshot(&self) -> RuntimeDiagnosticsSnapshot {
        let inner = self.0.lock().expect("diagnostic sink lock");
        RuntimeDiagnosticsSnapshot {
            events: inner.events.iter().cloned().collect(),
            warning_count: inner.warning_count,
            error_count: inner.error_count,
            dropped_count: inner.dropped_count,
        }
    }

    pub fn read_after(&self, after: Option<u64>) -> RuntimeDiagnosticsBatch {
        let inner = self.0.lock().expect("diagnostic sink lock");
        let floor_sequence = inner
            .events
            .front()
            .map_or(inner.next_sequence, |event| event.sequence);
        let through_sequence = inner.next_sequence.saturating_sub(1);
        let cursor = after.unwrap_or(0);
        let lagged = after.is_some_and(|cursor| cursor.saturating_add(1) < floor_sequence);
        let start = cursor.max(floor_sequence.saturating_sub(1));
        let mut encoded_bytes = 0_usize;
        let events: Vec<_> = inner
            .events
            .iter()
            .filter(|event| event.sequence > start)
            .take_while(|event| {
                let bytes = serde_json::to_vec(event).map_or(MAX_BATCH_BYTES, |value| value.len());
                if encoded_bytes.saturating_add(bytes) > MAX_BATCH_BYTES {
                    return false;
                }
                encoded_bytes = encoded_bytes.saturating_add(bytes);
                true
            })
            .take(MAX_BATCH_EVENTS)
            .cloned()
            .collect();
        let next_cursor = events.last().map_or(start, |event| event.sequence);
        RuntimeDiagnosticsBatch {
            events,
            floor_sequence,
            through_sequence,
            next_cursor,
            read_monotonic_nanoseconds: monotonic_nanoseconds(inner.started),
            lagged,
            warning_count: inner.warning_count,
            error_count: inner.error_count,
            dropped_count: inner.dropped_count,
        }
    }
}

/// Call-local Engine service totals. The host converts these raw values to its
/// wire spelling when it records completed callback telemetry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RuntimeUpdateAttribution {
    pub callback_duration_us: u64,
    pub character_step_calls: u64,
    pub character_step_duration_us: u64,
    pub character_step_cast_count: u64,
    pub character_step_candidate_count: u64,
    pub character_step_narrow_phase_count: u64,
    pub voxel_residency_calls: u64,
    pub voxel_residency_duration_us: u64,
    pub voxel_scene_presentation_calls: u64,
    pub voxel_scene_presentation_duration_us: u64,
}

fn monotonic_nanoseconds(started: Instant) -> u64 {
    started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64
}

fn serialize_u64_as_string<S: Serializer>(value: &u64, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&value.to_string())
}

fn bounded_identity(
    value: String,
    maximum: usize,
    field: &str,
) -> Result<String, RuntimeDiagnosticsError> {
    if value.is_empty()
        || value.len() > maximum
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b':'))
    {
        return Err(RuntimeDiagnosticsError::new(
            "RUNTIME_DIAGNOSTICS_EVENT",
            format!("diagnostic {field} is invalid"),
        ));
    }
    Ok(value)
}

fn bounded_text(
    value: String,
    maximum: usize,
    field: &str,
) -> Result<String, RuntimeDiagnosticsError> {
    if value.is_empty()
        || value.len() > maximum
        || value
            .chars()
            .any(|character| character.is_control() && character != '\t')
    {
        return Err(RuntimeDiagnosticsError::new(
            "RUNTIME_DIAGNOSTICS_EVENT",
            format!("diagnostic {field} is invalid"),
        ));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_ring_coalesces_and_reports_lagged_reader() {
        let sink =
            RuntimeDiagnosticsSink::new(RuntimeDiagnosticsConfig::default().with_ring_capacity(2))
                .unwrap();
        let event = |code| {
            RuntimeDiagnosticEvent::new(
                RuntimeDiagnosticSeverity::Error,
                RuntimeDiagnosticDisposition::RejectedRecoverable,
                "runtime",
                code,
                "retryable",
            )
            .unwrap()
        };
        sink.publish(event("SAME")).unwrap();
        sink.publish(event("SAME")).unwrap();
        sink.publish(event("OTHER")).unwrap();
        sink.publish(event("THIRD")).unwrap();
        let snapshot = sink.snapshot();
        assert_eq!(snapshot.events.len(), 2);
        assert_eq!(snapshot.warning_count, 3);
        assert_eq!(snapshot.dropped_count, 1);
        let batch = sink.read_after(Some(0));
        assert!(batch.lagged);
        assert_eq!(batch.events.len(), 2);
    }
}
