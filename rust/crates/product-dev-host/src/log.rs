use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Instant,
};

pub use runtime_diagnostics::{
    RuntimeDiagnosticDisposition as ProductDevLogDisposition,
    RuntimeDiagnosticEvent as ProductDevLogEvent,
    RuntimeDiagnosticSeverity as ProductDevLogSeverity,
    RuntimeDiagnosticsBatch as ProductDevLogBatch, RuntimeDiagnosticsSink,
};
use runtime_diagnostics::{
    RuntimeDiagnosticEvent, RuntimeDiagnosticsConfig, RuntimeDiagnosticsError,
    RuntimeDiagnosticsSnapshot, RuntimeDiagnosticsWriter,
};

use crate::ProductDevHostError;

const MAX_LINE_BYTES: usize = 4 * 1024;
const DEFAULT_ROTATE_BYTES: u64 = 6 * 1024 * 1024;
const MAX_RETENTION_FILES: u8 = 4;

/// Host filesystem policy around the neutral retained diagnostic sink.
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
            ring_capacity: 256,
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
    pub stderr_fallback_count: u64,
    pub writer_state: ProductDevLogWriterState,
}

/// Host compatibility wrapper around the neutral sink. The handle given to
/// Engine services is cloned from this object, so C#-originated diagnostics
/// use the same ring and durable writer as host and worker diagnostics.
#[derive(Clone)]
pub struct ProductDevLog {
    sink: RuntimeDiagnosticsSink,
    writer: Arc<HostFileDiagnosticsWriter>,
}

impl ProductDevLog {
    pub fn new(config: ProductDevLogConfig) -> Result<Self, ProductDevHostError> {
        if config.rotate_bytes < MAX_LINE_BYTES as u64
            || !(1..=MAX_RETENTION_FILES).contains(&config.retention_files)
        {
            return Err(ProductDevHostError::new(
                "DEV_HOST_LOG_CONFIG",
                "diagnostic log configuration is outside fixed bounds",
            ));
        }
        let writer = Arc::new(HostFileDiagnosticsWriter::new(
            config.path,
            config.rotate_bytes,
            config.retention_files,
        ));
        let sink = RuntimeDiagnosticsSink::with_writer(
            RuntimeDiagnosticsConfig::default().with_ring_capacity(config.ring_capacity),
            Some(writer.clone()),
        )
        .map_err(ProductDevHostError::from)?;
        Ok(Self { sink, writer })
    }

    pub fn handle(&self) -> RuntimeDiagnosticsSink {
        self.sink.clone()
    }

    pub fn publish(&self, event: ProductDevLogEvent) -> Result<(), RuntimeDiagnosticsError> {
        self.sink.publish(event)
    }

    pub fn flush(&self) {
        self.sink.flush();
    }

    pub fn now_monotonic_nanoseconds(&self) -> Option<u64> {
        self.sink.now_monotonic_nanoseconds()
    }

    pub fn snapshot(&self) -> ProductDevLogSnapshot {
        let RuntimeDiagnosticsSnapshot {
            events,
            warning_count,
            error_count,
            dropped_count,
        } = self.sink.snapshot();
        ProductDevLogSnapshot {
            events,
            warning_count,
            error_count,
            dropped_count,
            stderr_fallback_count: self.writer.stderr_fallback_count(),
            writer_state: self.writer.state(),
        }
    }

    pub fn read_after(&self, after: Option<u64>) -> ProductDevLogBatch {
        self.sink.read_after(after)
    }
}

struct HostFileDiagnosticsWriter {
    state: Mutex<HostFileWriterState>,
    stderr_fallback_count: Mutex<u64>,
    last_stderr_fallback: Mutex<Option<Instant>>,
}

enum HostFileWriterState {
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

impl HostFileDiagnosticsWriter {
    fn new(path: Option<PathBuf>, rotate_bytes: u64, retention_files: u8) -> Self {
        let state = match path {
            None => HostFileWriterState::Disabled,
            Some(path) => open_writer(path, rotate_bytes, retention_files)
                .unwrap_or(HostFileWriterState::Failed),
        };
        Self {
            state: Mutex::new(state),
            stderr_fallback_count: Mutex::new(0),
            last_stderr_fallback: Mutex::new(None),
        }
    }

    fn state(&self) -> ProductDevLogWriterState {
        match *self.state.lock().expect("diagnostic writer lock") {
            HostFileWriterState::Disabled => ProductDevLogWriterState::Disabled,
            HostFileWriterState::Ready { .. } => ProductDevLogWriterState::Ready,
            HostFileWriterState::Failed => ProductDevLogWriterState::Failed,
        }
    }

    fn stderr_fallback_count(&self) -> u64 {
        *self
            .stderr_fallback_count
            .lock()
            .expect("diagnostic stderr count lock")
    }

    fn fallback(&self, code: &str, detail: &str) {
        let Ok(mut last) = self.last_stderr_fallback.lock() else {
            return;
        };
        if last.is_none_or(|last| last.elapsed().as_secs() >= 60) {
            eprintln!("rusty-engine diagnostics writer failed [{code}]: {detail}");
            *last = Some(Instant::now());
            if let Ok(mut count) = self.stderr_fallback_count.lock() {
                *count = count.saturating_add(1);
            }
        }
    }
}

impl RuntimeDiagnosticsWriter for HostFileDiagnosticsWriter {
    fn write(&self, event: &RuntimeDiagnosticEvent, flush: bool) -> Result<(), String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "diagnostic writer lock is poisoned".to_owned())?;
        let result = write_event(&mut state, event, flush);
        if let Err(detail) = &result {
            if matches!(*state, HostFileWriterState::Ready { .. }) {
                *state = HostFileWriterState::Failed;
            }
            self.fallback(event.code(), detail);
        }
        result
    }

    fn flush(&self) -> Result<(), String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "diagnostic writer lock is poisoned".to_owned())?;
        let result = flush_writer(&mut state);
        if let Err(detail) = &result {
            if matches!(*state, HostFileWriterState::Ready { .. }) {
                *state = HostFileWriterState::Failed;
            }
            self.fallback("DEV_HOST_LOG_FLUSH", detail);
        }
        result
    }
}

impl Drop for HostFileDiagnosticsWriter {
    fn drop(&mut self) {
        let _ = self.flush();
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

fn open_writer(
    path: PathBuf,
    rotate_bytes: u64,
    retention_files: u8,
) -> Result<HostFileWriterState, PathBuf> {
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
    Ok(HostFileWriterState::Ready {
        path,
        file,
        bytes,
        rotate_bytes,
        retention_files,
    })
}

fn write_event(
    state: &mut HostFileWriterState,
    event: &RuntimeDiagnosticEvent,
    flush: bool,
) -> Result<(), String> {
    let HostFileWriterState::Ready {
        path,
        file,
        bytes,
        rotate_bytes,
        retention_files,
    } = state
    else {
        return match state {
            HostFileWriterState::Disabled => Ok(()),
            HostFileWriterState::Failed => Err("diagnostic writer is unavailable".to_owned()),
            HostFileWriterState::Ready { .. } => unreachable!(),
        };
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

fn flush_writer(state: &mut HostFileWriterState) -> Result<(), String> {
    match state {
        HostFileWriterState::Ready { file, .. } => file
            .flush()
            .map_err(|_| "diagnostic file flush failed".to_owned()),
        HostFileWriterState::Disabled => Ok(()),
        HostFileWriterState::Failed => Err("diagnostic writer is unavailable".to_owned()),
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_writer_uses_the_same_neutral_ring_and_rotates() {
        let root = std::env::temp_dir().join(format!("rusty-engine-log-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let path = root.join("events.ndjson");
        let log = ProductDevLog::new(
            ProductDevLogConfig::default()
                .with_path(&path)
                .with_rotation(4096, 3)
                .with_ring_capacity(2),
        )
        .unwrap();
        for index in 0..6 {
            log.handle()
                .publish(
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
        assert_eq!(log.snapshot().events.len(), 2);
        assert!(path.exists());
        assert!(rotated_path(&path, 1).exists());
        let _ = fs::remove_dir_all(root);
    }
}
