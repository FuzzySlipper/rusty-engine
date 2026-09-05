//! Closed, local shell-to-worker messages for the disposable product runtime.
//!
//! This is deliberately a tiny process-control vocabulary.  It carries one
//! already-admitted product bundle at worker readiness plus concrete runtime
//! operations and their receipts; it is not a product extension protocol.

use std::{
    io::{self, Read, Write},
    sync::mpsc::SyncSender,
    time::Instant,
};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    ProductDevHostError, ProductDevLogDisposition, ProductDevLogEvent, ProductDevLogSeverity,
    ProductDevRuntimeBinding, ProductDevRuntimeError, ProductDevRuntimeRecovery, MAX_BUNDLE_BYTES,
};

/// The worker channel uses a four-byte little-endian length followed by one
/// JSON envelope.  Readiness may carry one full already-admitted browser
/// bundle. JSON represents those binary bytes as decimal array entries, so
/// the outer frame permits that local encoding overhead above the existing
/// raw aggregate bundle bound.
/// Individual operation and output payloads keep their narrower normal-host
/// bounds before becoming a worker message.
pub const MAX_WORKER_FRAME_BYTES: usize = MAX_BUNDLE_BYTES * 5;

/// The immutable browser files needed when a freshly loaded worker becomes
/// the current product incarnation.  The shell replaces its admitted bundle
/// only after this one finite ready fact arrives.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProductDevWorkerBundle {
    pub entries: Vec<ProductDevWorkerBundleEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProductDevWorkerBundleEntry {
    pub path: String,
    pub content_type: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProductDevWorkerLifecycleOperation {
    Connect,
    Start,
    Pause,
    Resume,
    Restart,
    Shutdown,
    ReportFault,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProductDevWorkerControlOperation {
    Replace,
    Release,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProductDevWorkerUpdateOperation {
    AdvanceRealtime,
    AdmitDemandStep,
    AdmitExternalStep,
    CompleteTimeline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProductDevWorkerFeedbackOperation {
    Audio,
    Animation,
    GhostPlate,
    RendererDiagnostics,
}

/// Closed worker operations.  `payload` is an admitted route-shaped JSON
/// value, never a method name/argument dispatcher.  Each variant has exactly
/// one Engine-owned meaning and remains bounded by the normal host limits.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ProductDevWorkerRequest {
    Lifecycle {
        request_id: u64,
        operation: ProductDevWorkerLifecycleOperation,
        binding: Option<ProductDevRuntimeBinding>,
    },
    Control {
        request_id: u64,
        operation: ProductDevWorkerControlOperation,
        binding: ProductDevRuntimeBinding,
    },
    Input {
        request_id: u64,
        payload: Value,
    },
    Update {
        request_id: u64,
        operation: ProductDevWorkerUpdateOperation,
        payload: Value,
    },
    Debug {
        request_id: u64,
        command: Option<String>,
    },
    Feedback {
        request_id: u64,
        operation: ProductDevWorkerFeedbackOperation,
        payload: Value,
    },
    Health {
        request_id: u64,
    },
    Activate {
        request_id: u64,
    },
    Shutdown {
        request_id: u64,
    },
}

/// One terminal response for a concrete request.  The shell matches only the
/// monotonically allocated id it wrote; it never retries an ambiguous call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProductDevWorkerResponse {
    pub request_id: u64,
    pub attribution: Option<crate::ProductDevUpdateAttribution>,
    pub result: Option<Value>,
    pub outputs: Vec<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ProductDevWorkerFault>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProductDevWorkerFault {
    pub code: String,
    pub diagnostic: String,
    pub recovery: crate::ProductDevRuntimeRecovery,
}

/// A bounded worker-log fact. The shell recreates the event in its own
/// `ProductDevLog`, assigning its local sequence and monotonic time while
/// preserving the source-owned meaning and fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProductDevWorkerDiagnostic {
    pub severity: ProductDevLogSeverity,
    pub disposition: ProductDevLogDisposition,
    pub source: String,
    pub code: String,
    pub message: String,
    pub runtime: Option<ProductDevRuntimeBinding>,
    pub correlation: Option<String>,
    pub fields: Vec<ProductDevWorkerDiagnosticField>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProductDevWorkerDiagnosticField {
    pub key: String,
    pub value: String,
}

/// One asynchronous worker output batch, tagged by the shell-owned worker
/// generation so a retired reader can never publish into a newer baseline.
#[derive(Debug, Clone, PartialEq)]
pub struct ProductDevWorkerOutputBatch {
    pub generation: u64,
    pub outputs: Vec<crate::ProductDevRuntimeOutput>,
    pub received_at: Instant,
    pub decode_duration_us: u64,
}

/// One ordered shell-side publication item from a disposable worker.
///
/// A connection boundary follows every output frame that the worker emitted
/// before a connection snapshot response. The browser host acknowledges the
/// exact retained output cursor after it has committed that prefix; it does
/// not cross the worker wire or become a product callback.
pub enum ProductDevWorkerPublication {
    Outputs(ProductDevWorkerOutputBatch),
    UpdateTelemetry {
        generation: u64,
        telemetry: Box<ProductDevWorkerUpdateTelemetry>,
        delivery_interval_us: Option<u64>,
    },
    ConnectionBoundary {
        generation: u64,
        acknowledged: SyncSender<Option<u64>>,
    },
}

impl ProductDevWorkerDiagnostic {
    pub fn from_log_event(event: ProductDevLogEvent) -> Self {
        Self {
            severity: event.severity(),
            disposition: event.disposition(),
            source: event.source().to_owned(),
            code: event.code().to_owned(),
            message: event.message().to_owned(),
            runtime: event.runtime().map(Into::into),
            correlation: event.correlation().map(str::to_owned),
            fields: event
                .fields()
                .iter()
                .map(|field| ProductDevWorkerDiagnosticField {
                    key: field.key().to_owned(),
                    value: field.value().to_owned(),
                })
                .collect(),
        }
    }

    pub fn from_runtime_error(error: ProductDevRuntimeError) -> Self {
        Self {
            severity: ProductDevLogSeverity::Error,
            disposition: match crate::runtime_fault_disposition(&error) {
                crate::ProductDevFaultDisposition::Accepted => ProductDevLogDisposition::Accepted,
                crate::ProductDevFaultDisposition::RejectedRecoverable => {
                    ProductDevLogDisposition::RejectedRecoverable
                }
                crate::ProductDevFaultDisposition::Degraded => ProductDevLogDisposition::Degraded,
                crate::ProductDevFaultDisposition::ResyncRequired => {
                    ProductDevLogDisposition::ResyncRequired
                }
                crate::ProductDevFaultDisposition::Terminal => ProductDevLogDisposition::Terminal,
            },
            source: "worker-runtime".to_owned(),
            code: error.code().to_owned(),
            message: error.diagnostic().to_owned(),
            runtime: None,
            correlation: None,
            fields: Vec::new(),
        }
    }

    pub fn into_log_event(self) -> Result<ProductDevLogEvent, ProductDevHostError> {
        let mut event = ProductDevLogEvent::new(
            self.severity,
            self.disposition,
            self.source,
            self.code,
            self.message,
        )?;
        if let Some(runtime) = self.runtime {
            event = event.with_runtime(runtime);
        }
        if let Some(correlation) = self.correlation {
            event = event.with_correlation(correlation)?;
        }
        for field in self.fields {
            event = event.with_field(field.key, field.value)?;
        }
        Ok(event)
    }
}

/// One scheduler observation, emitted after its output frame on the same
/// ordered channel. No worker absolute timestamp crosses this boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductDevWorkerUpdateTelemetry {
    pub worker_pid: crate::CanonicalU64,
    pub readout: Option<crate::ProductDevRuntimeReadout>,
    pub attribution: Option<crate::ProductDevUpdateAttribution>,
    pub phases: runtime_diagnostics::RuntimeWorkerPhases,
}

/// Worker-originated facts.  Runtime output is separate so the worker-side
/// realtime scheduler can publish without waking or recreating an HTTP host.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ProductDevWorkerEvent {
    Ready {
        bundle: ProductDevWorkerBundle,
        outputs: Vec<Value>,
        diagnostics: Vec<ProductDevWorkerDiagnostic>,
    },
    Response(ProductDevWorkerResponse),
    /// A fresh-connection response. The shell reader inserts its local
    /// publication boundary immediately before forwarding this response to
    /// the invoking runtime owner.
    ConnectionResponse(ProductDevWorkerResponse),
    Outputs {
        outputs: Vec<Value>,
    },
    UpdateTelemetry {
        telemetry: Box<ProductDevWorkerUpdateTelemetry>,
    },
    Diagnostics {
        diagnostics: Vec<ProductDevWorkerDiagnostic>,
    },
    Health {
        code: String,
        detail: String,
        recovery: ProductDevRuntimeRecovery,
    },
    SchedulerActivity {
        active: bool,
    },
}

/// Writes one bounded, closed worker envelope.  The bound is enforced before
/// writing its prefix so a malformed candidate cannot desynchronize a live
/// shell channel.
pub fn write_worker_frame<T: Serialize>(
    writer: &mut impl Write,
    value: &T,
) -> Result<(), ProductDevHostError> {
    let bytes = serde_json::to_vec(value).map_err(|_| {
        ProductDevHostError::new(
            "DEV_HOST_WORKER_ENCODE",
            "worker message could not be encoded",
        )
    })?;
    if bytes.len() > MAX_WORKER_FRAME_BYTES {
        return Err(ProductDevHostError::new(
            "DEV_HOST_WORKER_BOUNDS",
            "worker message exceeds the maximum frame length",
        ));
    }
    let length = u32::try_from(bytes.len()).map_err(|_| {
        ProductDevHostError::new(
            "DEV_HOST_WORKER_BOUNDS",
            "worker message length cannot be represented",
        )
    })?;
    writer
        .write_all(&length.to_le_bytes())
        .and_then(|_| writer.write_all(&bytes))
        .map_err(|error| ProductDevHostError::io("DEV_HOST_WORKER_WRITE", error))
}

/// Reads one full worker envelope.  EOF before a prefix or body is a worker
/// exit, not a recoverable request result; the shell must fence the active
/// incarnation and never replay the in-flight operation.
pub fn read_worker_frame<T: for<'de> Deserialize<'de>>(
    reader: &mut impl Read,
) -> Result<T, ProductDevHostError> {
    let mut prefix = [0_u8; 4];
    read_exact_worker(reader, &mut prefix, "DEV_HOST_WORKER_EOF")?;
    let length = u32::from_le_bytes(prefix) as usize;
    if length > MAX_WORKER_FRAME_BYTES {
        return Err(ProductDevHostError::new(
            "DEV_HOST_WORKER_BOUNDS",
            "worker message exceeds the maximum frame length",
        ));
    }
    let mut bytes = vec![0_u8; length];
    read_exact_worker(reader, &mut bytes, "DEV_HOST_WORKER_EOF")?;
    serde_json::from_slice(&bytes).map_err(|_| {
        ProductDevHostError::new(
            "DEV_HOST_WORKER_DECODE",
            "worker message is not a closed envelope",
        )
    })
}

fn read_exact_worker(
    reader: &mut impl Read,
    bytes: &mut [u8],
    eof_code: &'static str,
) -> Result<(), ProductDevHostError> {
    reader.read_exact(bytes).map_err(|error| {
        let code = if error.kind() == io::ErrorKind::UnexpectedEof {
            eof_code
        } else {
            "DEV_HOST_WORKER_READ"
        };
        ProductDevHostError::io(code, error)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_frames_round_trip_one_closed_health_event() {
        let event = ProductDevWorkerEvent::Health {
            code: "WORKER_READY".to_owned(),
            detail: "worker owns the runtime".to_owned(),
            recovery: crate::ProductDevRuntimeRecovery::not_applied(),
        };
        let mut bytes = Vec::new();
        write_worker_frame(&mut bytes, &event).unwrap();
        assert_eq!(
            read_worker_frame::<ProductDevWorkerEvent>(&mut bytes.as_slice()).unwrap(),
            event
        );
    }

    #[test]
    fn worker_frames_keep_activation_and_scheduler_liveness_closed() {
        let request = ProductDevWorkerRequest::Activate { request_id: 7 };
        let event = ProductDevWorkerEvent::SchedulerActivity { active: true };
        let mut bytes = Vec::new();
        write_worker_frame(&mut bytes, &request).unwrap();
        assert_eq!(
            read_worker_frame::<ProductDevWorkerRequest>(&mut bytes.as_slice()).unwrap(),
            request
        );
        bytes.clear();
        write_worker_frame(&mut bytes, &event).unwrap();
        assert_eq!(
            read_worker_frame::<ProductDevWorkerEvent>(&mut bytes.as_slice()).unwrap(),
            event
        );
    }

    #[test]
    fn worker_frame_rejects_declared_length_over_the_bundle_bound() {
        let mut bytes = ((MAX_WORKER_FRAME_BYTES as u32).saturating_add(1))
            .to_le_bytes()
            .to_vec();
        bytes.extend_from_slice(b"{} ");
        let error = read_worker_frame::<ProductDevWorkerEvent>(&mut bytes.as_slice()).unwrap_err();
        assert_eq!(error.code(), "DEV_HOST_WORKER_BOUNDS");
    }

    #[test]
    fn worker_frame_reports_eof_without_treating_it_as_a_response() {
        let mut bytes = 12_u32.to_le_bytes().to_vec();
        bytes.extend_from_slice(b"{} ");
        let error = read_worker_frame::<ProductDevWorkerEvent>(&mut bytes.as_slice()).unwrap_err();
        assert_eq!(error.code(), "DEV_HOST_WORKER_EOF");
    }
}
