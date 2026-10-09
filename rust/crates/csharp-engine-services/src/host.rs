//! The `Host` service: whether the host can end at the product's request,
//! and the request itself. A host that can (one with a window) enables it and
//! watches the returned flag, stopping as closing the window does.

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use csharp_engine_abi::{
    NativeHostApi, NativeHostExitRequest, NativeHostReadout, NativeOperationErrorReceipt,
};

use crate::composition::ABI_OK;
use crate::operation_diagnostics::{clear_receipt, refuse};
use crate::CsharpEngineServicesError;

#[derive(Default)]
pub(crate) struct RuntimeHostBridge {
    /// Present once the host has enabled exit.
    exit: Option<Arc<AtomicBool>>,
}

impl RuntimeHostBridge {
    /// Makes exit available and returns the flag a request sets.
    pub(crate) fn enable_exit(&mut self) -> Arc<AtomicBool> {
        Arc::clone(self.exit.get_or_insert_with(Arc::default))
    }
}

pub(crate) fn api(bridge: &mut RuntimeHostBridge) -> NativeHostApi {
    NativeHostApi {
        context: (bridge as *mut RuntimeHostBridge).cast(),
        read,
        request_exit,
    }
}

unsafe extern "C" fn read(
    context: *mut c_void,
    output: *mut NativeHostReadout,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    clear_receipt(error);
    if context.is_null() || output.is_null() {
        return 0;
    }
    let bridge = unsafe { &*context.cast::<RuntimeHostBridge>() };
    unsafe {
        *output = NativeHostReadout {
            exit_available: bridge.exit.is_some(),
        }
    };
    ABI_OK
}

unsafe extern "C" fn request_exit(
    context: *mut c_void,
    _request: *const NativeHostExitRequest,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    clear_receipt(error);
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &*context.cast::<RuntimeHostBridge>() };
    match &bridge.exit {
        Some(exit) => {
            exit.store(true, Ordering::Release);
            ABI_OK
        }
        None => refuse(
            &CsharpEngineServicesError::new(
                "ENGINE_HOST_EXIT_UNAVAILABLE",
                "this output has no window to close; streamed output stops when `rusty dev` does",
            ),
            error,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operation_diagnostics::receipt_codes;

    /// Streamed output reports exit unavailable and refuses the request; once
    /// the host enables it (window output), a request sets the flag it watches.
    #[test]
    fn exit_is_refused_until_the_host_enables_it_and_then_sets_its_flag() {
        let mut bridge = RuntimeHostBridge::default();
        let api = api(&mut bridge);
        let read = |api: &NativeHostApi| {
            let mut readout = NativeHostReadout::default();
            assert_eq!(
                unsafe { (api.read)(api.context, &mut readout, std::ptr::null_mut()) },
                ABI_OK
            );
            readout.exit_available
        };
        let request = |api: &NativeHostApi| {
            let mut receipt = crate::operation_diagnostics::empty_receipt();
            let status = unsafe {
                (api.request_exit)(api.context, &NativeHostExitRequest::default(), &mut receipt)
            };
            (status, receipt_codes(&receipt))
        };
        assert!(!read(&api));
        assert_eq!(
            request(&api),
            (0, vec!["ENGINE_HOST_EXIT_UNAVAILABLE".to_owned()])
        );

        let exit = bridge.enable_exit();
        let api = super::api(&mut bridge);
        assert!(read(&api));
        assert!(!exit.load(Ordering::Acquire));
        assert_eq!(request(&api), (ABI_OK, Vec::new()));
        assert!(exit.load(Ordering::Acquire));
    }
}
