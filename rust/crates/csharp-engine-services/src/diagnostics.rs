use std::{collections::BTreeMap, ffi::c_void, sync::Arc};

use csharp_engine_abi::{
    NativeByteLease, NativeByteLeaseHandle, NativeDiagnosticsApi, NativeDiagnosticsDisposition,
    NativeDiagnosticsPublishRequest, NativeDiagnosticsSeverity,
};
use product_dev_host::{
    ProductDevLog, ProductDevLogDisposition, ProductDevLogEvent, ProductDevLogSeverity,
};

use crate::composition::ABI_OK;

pub(crate) struct RuntimeDiagnosticsBridge {
    sink: ProductDevLog,
    renderer_json: Option<Arc<[u8]>>,
    leases: BTreeMap<u64, Arc<[u8]>>,
    next_lease: u64,
}

impl RuntimeDiagnosticsBridge {
    pub(crate) fn new(sink: ProductDevLog) -> Self {
        Self {
            renderer_json: None,
            leases: BTreeMap::new(),
            next_lease: 1,
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
            NativeDiagnosticsSeverity::Debug => ProductDevLogSeverity::Debug,
            NativeDiagnosticsSeverity::Info => ProductDevLogSeverity::Info,
            NativeDiagnosticsSeverity::Warning => ProductDevLogSeverity::Warning,
            NativeDiagnosticsSeverity::Error => ProductDevLogSeverity::Error,
        };
        let disposition = match request.disposition {
            NativeDiagnosticsDisposition::Accepted => ProductDevLogDisposition::Accepted,
            NativeDiagnosticsDisposition::RejectedRecoverable => {
                ProductDevLogDisposition::RejectedRecoverable
            }
            NativeDiagnosticsDisposition::Degraded => ProductDevLogDisposition::Degraded,
            NativeDiagnosticsDisposition::ResyncRequired => {
                ProductDevLogDisposition::ResyncRequired
            }
            NativeDiagnosticsDisposition::Terminal => ProductDevLogDisposition::Terminal,
        };
        let event = ProductDevLogEvent::new(severity, disposition, source, code, message)
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
        destroy_byte_lease,
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

unsafe extern "C" fn read_renderer(context: *mut c_void, readout: *mut NativeByteLease) -> i32 {
    if context.is_null() || readout.is_null() {
        return 0;
    }
    // SAFETY: the function-table context is the live bridge and the caller
    // borrows the output only for this direct generated service call.
    let bridge = unsafe { &mut *(context.cast::<RuntimeDiagnosticsBridge>()) };
    let Some(snapshot) = bridge.renderer_json.clone() else {
        return 0;
    };
    let handle = bridge.next_lease;
    bridge.next_lease = bridge.next_lease.checked_add(1).unwrap_or(1);
    bridge.leases.insert(handle, Arc::clone(&snapshot));
    unsafe {
        *readout = NativeByteLease {
            handle: NativeByteLeaseHandle { value: handle },
            bytes: snapshot.as_ptr(),
            len: snapshot.len(),
        };
    }
    ABI_OK
}

unsafe extern "C" fn destroy_byte_lease(context: *mut c_void, lease: NativeByteLeaseHandle) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *(context.cast::<RuntimeDiagnosticsBridge>()) };
    if bridge.leases.remove(&lease.value).is_some() {
        ABI_OK
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renderer_snapshot_is_borrowed_through_one_exact_release() {
        let mut bridge =
            RuntimeDiagnosticsBridge::new(ProductDevLog::new(Default::default()).unwrap());
        bridge
            .ingest_renderer(&serde_json::json!({"schemaVersion": 1, "renderer": "accelerated"}))
            .unwrap();
        let api = api(&mut bridge);
        let mut lease = NativeByteLease {
            handle: NativeByteLeaseHandle::default(),
            bytes: std::ptr::null(),
            len: 0,
        };
        assert_eq!(
            unsafe { (api.read_renderer)(api.context, &mut lease) },
            ABI_OK
        );
        let bytes = unsafe { std::slice::from_raw_parts(lease.bytes, lease.len) };
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(bytes).unwrap()["renderer"],
            "accelerated"
        );
        assert_eq!(
            unsafe { (api.destroy_byte_lease)(api.context, lease.handle) },
            ABI_OK
        );
        assert_eq!(
            unsafe { (api.destroy_byte_lease)(api.context, lease.handle) },
            0
        );
    }

    #[test]
    fn publish_copies_bounded_csharp_diagnostic_into_the_engine_sink() {
        let sink = ProductDevLog::new(Default::default()).unwrap();
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
