use std::{collections::BTreeSet, ffi::c_void, sync::Arc};

use csharp_engine_abi::{
    NativeByteResult, NativeDiagnosticsApi, NativeDiagnosticsDisposition,
    NativeDiagnosticsPublishRequest, NativeDiagnosticsSeverity,
};
use runtime_diagnostics::{
    RuntimeDiagnosticDisposition, RuntimeDiagnosticEvent, RuntimeDiagnosticSeverity,
    RuntimeDiagnosticsSink,
};

use crate::composition::ABI_OK;

pub(crate) struct RuntimeDiagnosticsBridge {
    sink: RuntimeDiagnosticsSink,
    renderer_json: Option<Arc<[u8]>>,
    borrowed: crate::operation_diagnostics::BorrowedResult,
}

/// Product-owned cosmetic admission can legitimately meet an Engine bound.
/// Keep the corresponding real diagnostic useful without allowing a retrying
/// product loop to flood the same bounded sink.
pub(crate) fn publish_recoverable_once(
    sink: &RuntimeDiagnosticsSink,
    reported_codes: &mut BTreeSet<&'static str>,
    source: &'static str,
    code: &'static str,
    message: &'static str,
) {
    if !reported_codes.insert(code) {
        return;
    }
    let Ok(event) = RuntimeDiagnosticEvent::new(
        RuntimeDiagnosticSeverity::Warning,
        RuntimeDiagnosticDisposition::RejectedRecoverable,
        source,
        code,
        message,
    ) else {
        return;
    };
    // Recoverable product feedback must not become a callback fault merely
    // because diagnostics persistence has become unavailable.
    let _ = sink.publish(event);
}

impl RuntimeDiagnosticsBridge {
    pub(crate) fn new(sink: RuntimeDiagnosticsSink) -> Self {
        Self {
            renderer_json: None,
            borrowed: Default::default(),
            sink,
        }
    }

    fn publish(&self, request: &NativeDiagnosticsPublishRequest) -> Result<(), ()> {
        let source = unsafe {
            crate::composition::borrowed_utf8(
                request.source.bytes,
                request.source.len,
                "diagnostics source",
            )
        }
        .map_err(|_| ())?;
        let code = unsafe {
            crate::composition::borrowed_utf8(
                request.code.bytes,
                request.code.len,
                "diagnostics code",
            )
        }
        .map_err(|_| ())?;
        let message = unsafe {
            crate::composition::borrowed_utf8(
                request.message.bytes,
                request.message.len,
                "diagnostics message",
            )
        }
        .map_err(|_| ())?;
        let severity = match request.severity {
            NativeDiagnosticsSeverity::Debug => RuntimeDiagnosticSeverity::Debug,
            NativeDiagnosticsSeverity::Info => RuntimeDiagnosticSeverity::Info,
            NativeDiagnosticsSeverity::Warning => RuntimeDiagnosticSeverity::Warning,
            NativeDiagnosticsSeverity::Error => RuntimeDiagnosticSeverity::Error,
        };
        let disposition = match request.disposition {
            NativeDiagnosticsDisposition::Accepted => RuntimeDiagnosticDisposition::Accepted,
            NativeDiagnosticsDisposition::RejectedRecoverable => {
                RuntimeDiagnosticDisposition::RejectedRecoverable
            }
            NativeDiagnosticsDisposition::Degraded => RuntimeDiagnosticDisposition::Degraded,
            NativeDiagnosticsDisposition::ResyncRequired => {
                RuntimeDiagnosticDisposition::ResyncRequired
            }
            NativeDiagnosticsDisposition::Terminal => RuntimeDiagnosticDisposition::Terminal,
        };
        let event = RuntimeDiagnosticEvent::new(severity, disposition, source, code, message)
            .map_err(|_| ())?;
        let correlation = unsafe {
            crate::composition::borrowed_utf8(
                request.correlation.bytes,
                request.correlation.len,
                "diagnostics correlation",
            )
        }
        .map_err(|_| ())?;
        let event = if correlation.is_empty() {
            event
        } else {
            event.with_correlation(correlation).map_err(|_| ())?
        };
        self.sink.publish(event).map_err(|_| ())
    }
}

impl RuntimeDiagnosticsBridge {
    pub(crate) fn ingest_renderer(
        &mut self,
        snapshot: &serde_json::Value,
    ) -> Result<(), serde_json::Error> {
        self.renderer_json = Some(Arc::from(serde_json::to_vec(snapshot)?));
        Ok(())
    }

    pub(crate) fn renderer_json(&self) -> Option<&str> {
        self.renderer_json
            .as_deref()
            .and_then(|bytes| std::str::from_utf8(bytes).ok())
    }
}

pub(crate) fn api(bridge: &mut RuntimeDiagnosticsBridge) -> NativeDiagnosticsApi {
    NativeDiagnosticsApi {
        context: (bridge as *mut RuntimeDiagnosticsBridge).cast(),
        read_renderer,
        publish,
    }
}

unsafe extern "C" fn publish(
    context: *mut c_void,
    request: *const NativeDiagnosticsPublishRequest,
) -> i32 {
    if context.is_null() || request.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *(context.cast::<RuntimeDiagnosticsBridge>()) };
    let request = unsafe { &*request };
    bridge.publish(request).map(|()| ABI_OK).unwrap_or(0)
}

unsafe extern "C" fn read_renderer(context: *mut c_void, readout: *mut NativeByteResult) -> i32 {
    if context.is_null() || readout.is_null() {
        return 0;
    }
    // SAFETY: the function-table context is the live bridge and the caller
    // borrows the output only for this direct generated service call.
    let bridge = unsafe { &mut *(context.cast::<RuntimeDiagnosticsBridge>()) };
    let Some(snapshot) = bridge.renderer_json.clone() else {
        return 0;
    };
    unsafe {
        *readout = NativeByteResult {
            bytes: snapshot.as_ptr(),
            len: snapshot.len(),
        };
    }
    bridge.borrowed.hold(snapshot);
    ABI_OK
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renderer_snapshot_is_borrowed_until_the_next_call() {
        let mut bridge =
            RuntimeDiagnosticsBridge::new(RuntimeDiagnosticsSink::new(Default::default()).unwrap());
        bridge
            .ingest_renderer(&serde_json::json!({"schemaVersion": 1, "renderer": "accelerated"}))
            .unwrap();
        let api = api(&mut bridge);
        let mut result = NativeByteResult {
            bytes: std::ptr::null(),
            len: 0,
        };
        assert_eq!(
            unsafe { (api.read_renderer)(api.context, &mut result) },
            ABI_OK
        );
        let bytes = unsafe { std::slice::from_raw_parts(result.bytes, result.len) };
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(bytes).unwrap()["renderer"],
            "accelerated"
        );
    }

    #[test]
    fn publish_copies_bounded_csharp_diagnostic_into_the_engine_sink() {
        let sink = RuntimeDiagnosticsSink::new(Default::default()).unwrap();
        let mut bridge = RuntimeDiagnosticsBridge::new(sink.clone());
        let api = api(&mut bridge);
        let source = b"product";
        let code = b"PRODUCT_NOTICE";
        let message = b"bounded diagnostic";
        let correlation = b"update-7";
        let request = NativeDiagnosticsPublishRequest {
            severity: NativeDiagnosticsSeverity::Warning,
            disposition: NativeDiagnosticsDisposition::Degraded,
            source: csharp_engine_abi::NativeUtf8Slice {
                bytes: source.as_ptr(),
                len: source.len(),
            },
            code: csharp_engine_abi::NativeUtf8Slice {
                bytes: code.as_ptr(),
                len: code.len(),
            },
            message: csharp_engine_abi::NativeUtf8Slice {
                bytes: message.as_ptr(),
                len: message.len(),
            },
            correlation: csharp_engine_abi::NativeUtf8Slice {
                bytes: correlation.as_ptr(),
                len: correlation.len(),
            },
        };
        assert_eq!(unsafe { (api.publish)(api.context, &request) }, ABI_OK);
        let snapshot = sink.snapshot();
        assert_eq!(snapshot.warning_count, 1);
        assert_eq!(snapshot.events[0].code(), "PRODUCT_NOTICE");

        let invalid = NativeDiagnosticsPublishRequest {
            message: csharp_engine_abi::NativeUtf8Slice {
                bytes: b"\xff".as_ptr(),
                len: 1,
            },
            ..request
        };
        assert_eq!(unsafe { (api.publish)(api.context, &invalid) }, 0);
        assert_eq!(sink.snapshot().events.len(), 1);
    }
}
