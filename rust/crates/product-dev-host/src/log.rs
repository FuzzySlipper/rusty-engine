use std::{
    collections::VecDeque,
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Instant,
};

use serde::{Deserialize, Serialize, Serializer};

use crate::{ProductDevHostError, ProductDevRuntimeBinding};

const DEFAULT_RING_CAPACITY: usize = 256;
const MAX_RING_CAPACITY: usize = 1_024;
const MAX_SOURCE_BYTES: usize = 64;
const MAX_CODE_BYTES: usize = 128;
const MAX_MESSAGE_BYTES: usize = 1_024;
const MAX_CORRELATION_BYTES: usize = 128;
const MAX_FIELDS: usize = 8;
const MAX_FIELD_KEY_BYTES: usize = 64;
const MAX_FIELD_VALUE_BYTES: usize = 256;
const MAX_LINE_BYTES: usize = 4 * 1024;
const DEFAULT_ROTATE_BYTES: u64 = 6 * 1024 * 1024;
const MAX_RETENTION_FILES: u8 = 4;
const MAX_RECOVERABLE_CODES: usize = 64;

/// Severity deliberately remains smaller than a general logging framework.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProductDevLogSeverity {
    Debug,
    Info,
    Warning,
    Error,
}

/// Recovery posture supplied by the Engine owner, never inferred from text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProductDevLogDisposition {
    Accepted,
    RejectedRecoverable,
    Degraded,
    ResyncRequired,
    Terminal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductDevLogField {
    key: String,
    value: String,
}

impl ProductDevLogField {
    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn value(&self) -> &str {
        &self.value
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductDevLogEvent {
    #[serde(serialize_with = "serialize_u64_as_string")]
    sequence: u64,
    #[serde(serialize_with = "serialize_u64_as_string")]
    monotonic_nanoseconds: u64,
    severity: ProductDevLogSeverity,
    disposition: ProductDevLogDisposition,
    source: String,
    code: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    runtime: Option<ProductDevRuntimeBinding>,
    #[serde(skip_serializing_if = "Option::is_none")]
    correlation: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    fields: Vec<ProductDevLogField>,
}

impl ProductDevLogEvent {
    pub fn code(&self) -> &str {
        &self.code
    }

    pub const fn severity(&self) -> ProductDevLogSeverity {
        self.severity
    }

    pub const fn disposition(&self) -> ProductDevLogDisposition {
        self.disposition
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn runtime(&self) -> Option<ProductDevRuntimeBinding> {
        self.runtime.clone()
    }

    pub fn correlation(&self) -> Option<&str> {
        self.correlation.as_deref()
    }

    pub fn fields(&self) -> &[ProductDevLogField] {
        &self.fields
    }

    pub fn new(
        severity: ProductDevLogSeverity,
        disposition: ProductDevLogDisposition,
        source: impl Into<String>,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Result<Self, ProductDevHostError> {
        let source = bounded_identity(source.into(), MAX_SOURCE_BYTES, "source")?;
        let code = bounded_identity(code.into(), MAX_CODE_BYTES, "code")?;
        let message = bounded_text(message.into(), MAX_MESSAGE_BYTES, "message")?;
        Ok(Self {
            sequence: 0,
            monotonic_nanoseconds: 0,
            severity,
            disposition,
            source,
            code,
            message,
            runtime: None,
            correlation: None,
            fields: Vec::new(),
        })
    }

    pub fn with_runtime(mut self, runtime: ProductDevRuntimeBinding) -> Self {
        self.runtime = Some(runtime);
        self
    }

    pub fn with_correlation(
        mut self,
        correlation: impl Into<String>,
    ) -> Result<Self, ProductDevHostError> {
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
    ) -> Result<Self, ProductDevHostError> {
        if self.fields.len() == MAX_FIELDS {
            return Err(ProductDevHostError::new(
                "DEV_HOST_LOG_FIELDS",
                "diagnostic event has too many fields",
            ));
        }
        let key = bounded_identity(key.into(), MAX_FIELD_KEY_BYTES, "field key")?;
        if self.fields.iter().any(|field| field.key == key) {
            return Err(ProductDevHostError::new(
                "DEV_HOST_LOG_FIELDS",
                "diagnostic event fields must have unique keys",
            ));
        }
        self.fields.push(ProductDevLogField {
            key,
            value: bounded_text(value.into(), MAX_FIELD_VALUE_BYTES, "field value")?,
        });
        Ok(self)
    }
}

#[derive(Debug, Clone)]
pub struct ProductDevLogConfig {
    path: Option<PathBuf>,
    ring_capacity: usize,
    rotate_bytes: u64,
    retention_files: u8,
}

impl Default for ProductDevLogConfig {
    fn default() -> Self {
        Self {
            path: diagnostic_path_from_environment(),
            ring_capacity: DEFAULT_RING_CAPACITY,
            rotate_bytes: DEFAULT_ROTATE_BYTES,
            retention_files: 3,
        }
    }
}

impl ProductDevLogConfig {
    pub fn with_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.path = Some(path.into());
        self
    }
    #[cfg(test)]
    fn with_rotation(mut self, rotate_bytes: u64, retention_files: u8) -> Self {
        self.rotate_bytes = rotate_bytes;
        self.retention_files = retention_files;
        self
    }
    #[cfg(test)]
    fn with_ring_capacity(mut self, capacity: usize) -> Self {
        self.ring_capacity = capacity;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductDevLogWriterState {
    Disabled,
    Ready,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductDevLogSnapshot {
    pub events: Vec<ProductDevLogEvent>,
    pub warning_count: u64,
    pub error_count: u64,
    pub dropped_count: u64,
    /// Number of actual stderr fallbacks emitted; this is bounded by rate limiting.
    pub stderr_fallback_count: u64,
    pub writer_state: ProductDevLogWriterState,
}

/// Independent bounded-reader result. A reader's cursor never advances the
/// process-owned ring, so multiple browsers can reconnect without affecting
/// one another.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductDevLogBatch {
    pub events: Vec<ProductDevLogEvent>,
    #[serde(serialize_with = "serialize_u64_as_string")]
    pub floor_sequence: u64,
    #[serde(serialize_with = "serialize_u64_as_string")]
    pub through_sequence: u64,
    #[serde(serialize_with = "serialize_u64_as_string")]
    pub next_cursor: u64,
    /// Sink monotonic time at this read, for durable age calculations without
    /// retaining a parallel browser-health clock.
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
pub struct ProductDevLog(Arc<Mutex<ProductDevLogInner>>);

struct ProductDevLogInner {
    started: Instant,
    next_sequence: u64,
    events: VecDeque<ProductDevLogEvent>,
    ring_capacity: usize,
    warning_count: u64,
    error_count: u64,
    dropped_count: u64,
    stderr_fallback_count: u64,
    /// Process-owned duplicate suppression across runtime, host, and browser
    /// layers. It is deliberately a small code-only window, not a logger.
    recoverable_codes: VecDeque<String>,
    writer: Writer,
    last_stderr_fallback: Option<Instant>,
}

enum Writer {
    Disabled,
    Ready {
        path: PathBuf,
        file: File,
        bytes: u64,
        rotate_bytes: u64,
        retention_files: u8,
    },
    Failed,
}

impl ProductDevLog {
    pub fn new(config: ProductDevLogConfig) -> Result<Self, ProductDevHostError> {
        if config.ring_capacity == 0
            || config.ring_capacity > MAX_RING_CAPACITY
            || config.rotate_bytes < MAX_LINE_BYTES as u64
            || !(1..=MAX_RETENTION_FILES).contains(&config.retention_files)
        {
            return Err(ProductDevHostError::new(
                "DEV_HOST_LOG_CONFIG",
                "diagnostic log configuration is outside fixed bounds",
            ));
        }
        let writer = match config.path {
            None => Writer::Disabled,
            Some(path) => open_writer(path, config.rotate_bytes, config.retention_files)
                .unwrap_or(Writer::Failed),
        };
        Ok(Self(Arc::new(Mutex::new(ProductDevLogInner {
            started: Instant::now(),
            next_sequence: 1,
            events: VecDeque::new(),
            ring_capacity: config.ring_capacity,
            warning_count: 0,
            error_count: 0,
            dropped_count: 0,
            stderr_fallback_count: 0,
            recoverable_codes: VecDeque::new(),
            writer,
            last_stderr_fallback: None,
        }))))
    }

    pub fn publish(&self, mut event: ProductDevLogEvent) -> Result<(), ProductDevHostError> {
        let mut inner = self.0.lock().map_err(|_| {
            ProductDevHostError::new("DEV_HOST_LOG_POISONED", "diagnostic sink lock is poisoned")
        })?;
        if event.disposition == ProductDevLogDisposition::RejectedRecoverable {
            if inner.recoverable_codes.contains(&event.code) {
                return Ok(());
            }
            if inner.recoverable_codes.len() == MAX_RECOVERABLE_CODES {
                inner.recoverable_codes.pop_front();
            }
            inner.recoverable_codes.push_back(event.code.clone());
            // Recoverable rejections are operational warnings even when an
            // upstream layer originally constructed them as runtime errors.
            event.severity = ProductDevLogSeverity::Warning;
        }
        event.sequence = inner.next_sequence;
        inner.next_sequence = inner.next_sequence.checked_add(1).ok_or_else(|| {
            ProductDevHostError::new("DEV_HOST_LOG_SEQUENCE", "diagnostic sequence exhausted")
        })?;
        event.monotonic_nanoseconds =
            inner.started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
        match event.severity {
            ProductDevLogSeverity::Warning => {
                inner.warning_count = inner.warning_count.saturating_add(1)
            }
            ProductDevLogSeverity::Error => inner.error_count = inner.error_count.saturating_add(1),
            _ => {}
        }
        if inner.events.len() == inner.ring_capacity {
            inner.events.pop_front();
            inner.dropped_count = inner.dropped_count.saturating_add(1);
        }
        let flush = matches!(
            event.severity,
            ProductDevLogSeverity::Warning | ProductDevLogSeverity::Error
        );
        if let Err(detail) = write_event(&mut inner.writer, &event, flush) {
            mark_writer_failed(&mut inner.writer);
            inner.dropped_count = inner.dropped_count.saturating_add(1);
            stderr_fallback(&mut inner, &event.code, &detail);
        }
        inner.events.push_back(event);
        Ok(())
    }

    pub fn flush(&self) {
        if let Ok(mut inner) = self.0.lock() {
            if let Err(detail) = flush_writer(&mut inner.writer) {
                mark_writer_failed(&mut inner.writer);
                stderr_fallback(&mut inner, "DEV_HOST_LOG_FLUSH", &detail);
            }
        }
    }

    /// Returns the monotonic timestamp used by diagnostic events and reads.
    /// Keeping this clock behind the diagnostic sink gives host telemetry one
    /// origin without exposing its `Instant` or creating a second clock.
    pub fn now_monotonic_nanoseconds(&self) -> Option<u64> {
        self.0
            .lock()
            .ok()
            .map(|inner| inner.started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64)
    }

    pub fn snapshot(&self) -> ProductDevLogSnapshot {
        let inner = self.0.lock().expect("diagnostic sink lock");
        ProductDevLogSnapshot {
            events: inner.events.iter().cloned().collect(),
            warning_count: inner.warning_count,
            error_count: inner.error_count,
            dropped_count: inner.dropped_count,
            stderr_fallback_count: inner.stderr_fallback_count,
            writer_state: match inner.writer {
                Writer::Disabled => ProductDevLogWriterState::Disabled,
                Writer::Ready { .. } => ProductDevLogWriterState::Ready,
                Writer::Failed => ProductDevLogWriterState::Failed,
            },
        }
    }

    /// Reads events strictly after `after`. Cursors are opaque sequence facts:
    /// a cursor below `floor_sequence - 1` is explicitly lagged rather than
    /// silently pretending the missing prefix was delivered.
    pub fn read_after(&self, after: Option<u64>) -> ProductDevLogBatch {
        let inner = self.0.lock().expect("diagnostic sink lock");
        let floor_sequence = inner
            .events
            .front()
            .map_or(inner.next_sequence, |event| event.sequence);
        let through_sequence = inner.next_sequence.saturating_sub(1);
        let read_monotonic_nanoseconds =
            inner.started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
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
        ProductDevLogBatch {
            events,
            floor_sequence,
            through_sequence,
            next_cursor,
            read_monotonic_nanoseconds,
            lagged,
            warning_count: inner.warning_count,
            error_count: inner.error_count,
            dropped_count: inner.dropped_count,
        }
    }
}

const MAX_BATCH_EVENTS: usize = 64;
const MAX_BATCH_BYTES: usize = 128 * 1024;

fn serialize_u64_as_string<S: Serializer>(value: &u64, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&value.to_string())
}

impl Drop for ProductDevLogInner {
    fn drop(&mut self) {
        if let Err(detail) = flush_writer(&mut self.writer) {
            mark_writer_failed(&mut self.writer);
            stderr_fallback(self, "DEV_HOST_LOG_DROP_FLUSH", &detail);
        }
    }
}

fn diagnostic_path_from_environment() -> Option<PathBuf> {
    std::env::var_os("RUSTY_ENGINE_DIAGNOSTICS_PATH")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("DEN_SERVE_SESSION_DIR")
                .map(|directory| PathBuf::from(directory).join("rusty-engine-diagnostics.ndjson"))
        })
}

fn open_writer(path: PathBuf, rotate_bytes: u64, retention_files: u8) -> Result<Writer, PathBuf> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|_| path.clone())?;
    }
    let bytes = fs::metadata(&path)
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|_| path.clone())?;
    Ok(Writer::Ready {
        path,
        file,
        bytes,
        rotate_bytes,
        retention_files,
    })
}

fn write_event(writer: &mut Writer, event: &ProductDevLogEvent, flush: bool) -> Result<(), String> {
    if matches!(writer, Writer::Disabled) {
        return Ok(());
    }
    if matches!(writer, Writer::Failed) {
        return Err("diagnostic writer is unavailable".to_owned());
    }
    let Writer::Ready {
        path,
        file,
        bytes,
        rotate_bytes,
        retention_files,
    } = writer
    else {
        return Ok(());
    };
    let mut encoded = serde_json::to_vec(event).map_err(|_| "event encoding failed".to_owned())?;
    encoded.push(b'\n');
    if encoded.len() > MAX_LINE_BYTES {
        return Err("encoded event exceeded fixed line bound".to_owned());
    }
    if bytes.saturating_add(encoded.len() as u64) > *rotate_bytes {
        rotate(path, *retention_files)?;
        *file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|_| "rotated diagnostic file could not open".to_owned())?;
        *bytes = 0;
    }
    file.write_all(&encoded)
        .map_err(|_| "diagnostic file write failed".to_owned())?;
    *bytes = bytes.saturating_add(encoded.len() as u64);
    if flush {
        file.flush()
            .map_err(|_| "diagnostic file flush failed".to_owned())?;
    }
    Ok(())
}

fn rotate(path: &Path, retention_files: u8) -> Result<(), String> {
    for index in (1..retention_files).rev() {
        let source = rotated_path(path, index);
        let target = rotated_path(path, index + 1);
        if source.exists() {
            fs::rename(source, target)
                .map_err(|_| "diagnostic retention rotation failed".to_owned())?;
        }
    }
    if path.exists() {
        fs::rename(path, rotated_path(path, 1))
            .map_err(|_| "diagnostic rotation failed".to_owned())?;
    }
    Ok(())
}

fn rotated_path(path: &Path, index: u8) -> PathBuf {
    PathBuf::from(format!("{}.{}", path.display(), index))
}
fn flush_writer(writer: &mut Writer) -> Result<(), String> {
    match writer {
        Writer::Ready { file, .. } => file
            .flush()
            .map_err(|_| "diagnostic file flush failed".to_owned()),
        Writer::Disabled => Ok(()),
        Writer::Failed => Err("diagnostic writer is unavailable".to_owned()),
    }
}

fn mark_writer_failed(writer: &mut Writer) {
    if matches!(writer, Writer::Ready { .. }) {
        *writer = Writer::Failed;
    }
}

fn stderr_fallback(inner: &mut ProductDevLogInner, code: &str, detail: &str) {
    if inner
        .last_stderr_fallback
        .is_none_or(|last| last.elapsed().as_secs() >= 60)
    {
        eprintln!("rusty-engine diagnostics writer failed [{code}]: {detail}");
        inner.last_stderr_fallback = Some(Instant::now());
        inner.stderr_fallback_count = inner.stderr_fallback_count.saturating_add(1);
    }
}
fn bounded_identity(
    value: String,
    maximum: usize,
    field: &str,
) -> Result<String, ProductDevHostError> {
    if value.is_empty()
        || value.len() > maximum
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b':'))
    {
        return Err(ProductDevHostError::new(
            "DEV_HOST_LOG_EVENT",
            format!("diagnostic {field} is invalid"),
        ));
    }
    Ok(value)
}
fn bounded_text(value: String, maximum: usize, field: &str) -> Result<String, ProductDevHostError> {
    if value.is_empty()
        || value.len() > maximum
        || value
            .chars()
            .any(|character| character.is_control() && character != '\t')
    {
        return Err(ProductDevHostError::new(
            "DEV_HOST_LOG_EVENT",
            format!("diagnostic {field} is invalid"),
        ));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_ring_rotates_and_retains_counters() {
        let root = std::env::temp_dir().join(format!("rusty-engine-log-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let path = root.join("events.ndjson");
        let log = ProductDevLog::new(
            ProductDevLogConfig::default()
                .with_path(&path)
                .with_ring_capacity(2)
                .with_rotation(4096, 3),
        )
        .unwrap();
        for index in 0..6 {
            log.publish(
                ProductDevLogEvent::new(
                    ProductDevLogSeverity::Warning,
                    ProductDevLogDisposition::RejectedRecoverable,
                    "host",
                    format!("CODE_{index}"),
                    "x".repeat(1_000),
                )
                .unwrap(),
            )
            .unwrap();
        }
        let snapshot = log.snapshot();
        assert_eq!(snapshot.events.len(), 2);
        assert_eq!(snapshot.dropped_count, 4);
        assert_eq!(snapshot.warning_count, 6);
        assert!(path.exists());
        assert!(rotated_path(&path, 1).exists());
        drop(log);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn recoverable_codes_coalesce_across_cloned_runtime_host_and_browser_sinks() {
        let log = ProductDevLog::new(ProductDevLogConfig::default()).unwrap();
        let host_log = log.clone();
        let browser_log = log.clone();
        for (sink, source, message) in [
            (&log, "csharp-runtime", "clock regressed in runtime"),
            (&host_log, "runtime", "clock regressed in host"),
            (&browser_log, "browser-host", "clock observation dropped"),
        ] {
            sink.publish(
                ProductDevLogEvent::new(
                    ProductDevLogSeverity::Error,
                    ProductDevLogDisposition::RejectedRecoverable,
                    source,
                    "CSHARP_LIFECYCLE_CLOCK_REGRESSION",
                    message,
                )
                .unwrap(),
            )
            .unwrap();
        }
        let snapshot = log.snapshot();
        assert_eq!(snapshot.events.len(), 1);
        assert_eq!(snapshot.warning_count, 1);
        assert_eq!(snapshot.error_count, 0);
        assert_eq!(snapshot.events[0].severity, ProductDevLogSeverity::Warning);
        assert_eq!(
            snapshot.events[0].disposition,
            ProductDevLogDisposition::RejectedRecoverable
        );

        browser_log
            .publish(
                ProductDevLogEvent::new(
                    ProductDevLogSeverity::Info,
                    ProductDevLogDisposition::Accepted,
                    "browser-host",
                    "BROWSER_HOST_PROGRESS",
                    "later cadence accepted",
                )
                .unwrap(),
            )
            .unwrap();
        let snapshot = log.snapshot();
        assert_eq!(snapshot.events.len(), 2);
        assert_eq!(snapshot.warning_count, 1);
        assert_eq!(snapshot.error_count, 0);
        assert_eq!(snapshot.events[1].code, "BROWSER_HOST_PROGRESS");

        host_log
            .publish(
                ProductDevLogEvent::new(
                    ProductDevLogSeverity::Error,
                    ProductDevLogDisposition::Terminal,
                    "runtime",
                    "CSHARP_LIFECYCLE_COUNTER_EXHAUSTED",
                    "counter exhausted",
                )
                .unwrap(),
            )
            .unwrap();
        let snapshot = log.snapshot();
        assert_eq!(snapshot.events.len(), 3);
        assert_eq!(snapshot.warning_count, 1);
        assert_eq!(snapshot.error_count, 1);
        assert_eq!(
            snapshot.events[2].disposition,
            ProductDevLogDisposition::Terminal
        );
    }
    #[test]
    fn invalid_event_and_failed_writer_keep_memory() {
        let log =
            ProductDevLog::new(ProductDevLogConfig::default().with_path("/dev/null/not-a-file"))
                .unwrap();
        assert_eq!(
            log.snapshot().writer_state,
            ProductDevLogWriterState::Failed
        );
        assert!(ProductDevLogEvent::new(
            ProductDevLogSeverity::Info,
            ProductDevLogDisposition::Accepted,
            "bad space",
            "CODE",
            "message"
        )
        .is_err());
        log.publish(
            ProductDevLogEvent::new(
                ProductDevLogSeverity::Error,
                ProductDevLogDisposition::Terminal,
                "host",
                "CODE",
                "message",
            )
            .unwrap(),
        )
        .unwrap();
        let snapshot = log.snapshot();
        assert_eq!(snapshot.events.len(), 1);
        assert_eq!(snapshot.dropped_count, 1);
        assert_eq!(snapshot.stderr_fallback_count, 1);
        assert_eq!(snapshot.writer_state, ProductDevLogWriterState::Failed);

        log.publish(
            ProductDevLogEvent::new(
                ProductDevLogSeverity::Error,
                ProductDevLogDisposition::Terminal,
                "host",
                "SECOND_CODE",
                "message",
            )
            .unwrap(),
        )
        .unwrap();
        log.flush();
        let snapshot = log.snapshot();
        assert_eq!(snapshot.events.len(), 2);
        assert_eq!(snapshot.dropped_count, 2);
        assert_eq!(snapshot.stderr_fallback_count, 1);
    }

    #[test]
    fn independent_readers_observe_floor_and_lag_without_consuming_the_ring() {
        let log = ProductDevLog::new(ProductDevLogConfig::default().with_ring_capacity(2)).unwrap();
        for code in ["ONE", "TWO", "THREE"] {
            log.publish(
                ProductDevLogEvent::new(
                    ProductDevLogSeverity::Info,
                    ProductDevLogDisposition::Accepted,
                    "host",
                    code,
                    "message",
                )
                .unwrap(),
            )
            .unwrap();
        }
        let first_reader = log.read_after(None);
        assert!(!first_reader.lagged);
        assert_eq!(first_reader.floor_sequence, 2);
        assert_eq!(first_reader.events.len(), 2);
        let stale_reader = log.read_after(Some(0));
        assert!(stale_reader.lagged);
        assert_eq!(stale_reader.events.len(), 2);
        let caught_up_reader = log.read_after(Some(first_reader.next_cursor));
        assert!(!caught_up_reader.lagged);
        assert!(caught_up_reader.events.is_empty());
    }

    #[test]
    fn drop_flushes_the_last_non_warning_event() {
        let root =
            std::env::temp_dir().join(format!("rusty-engine-log-flush-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let path = root.join("events.ndjson");
        {
            let log = ProductDevLog::new(ProductDevLogConfig::default().with_path(&path)).unwrap();
            log.publish(
                ProductDevLogEvent::new(
                    ProductDevLogSeverity::Info,
                    ProductDevLogDisposition::Accepted,
                    "host",
                    "FLUSH_ON_DROP",
                    "flush",
                )
                .unwrap(),
            )
            .unwrap();
        }
        assert!(fs::read_to_string(&path).unwrap().contains("FLUSH_ON_DROP"));
        let _ = fs::remove_dir_all(root);
    }
}
