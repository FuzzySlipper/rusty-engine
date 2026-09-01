use std::{collections::BTreeMap, ffi::c_void, sync::Arc};

use csharp_engine_abi::{NativeByteLease, NativeByteLeaseHandle, NativeDiagnosticsApi};

use crate::composition::ABI_OK;

pub(crate) struct RuntimeDiagnosticsBridge {
    renderer_json: Option<Arc<[u8]>>,
    leases: BTreeMap<u64, Arc<[u8]>>,
    next_lease: u64,
}

impl Default for RuntimeDiagnosticsBridge {
    fn default() -> Self {
        Self {
            renderer_json: None,
            leases: BTreeMap::new(),
            next_lease: 1,
        }
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
        destroy_byte_lease,
    }
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
        let mut bridge = RuntimeDiagnosticsBridge::default();
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
}
