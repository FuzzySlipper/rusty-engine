use super::*;

pub(super) struct OperationDiagnosticLease {
    _code: Box<str>,
    _message: Box<str>,
    diagnostic: Box<NativeEngineDiagnostic>,
}

fn utf8(bytes: &[u8]) -> NativeUtf8Slice {
    NativeUtf8Slice {
        bytes: bytes.as_ptr(),
        len: bytes.len(),
    }
}

impl RuntimeDynamicsBridge {
    pub(super) fn retain_operation_error(
        &mut self,
        error: &CsharpEngineServicesError,
        receipt: *mut NativeOperationErrorReceipt,
        operation: &'static [u8],
    ) {
        let value = self.next_diagnostic_lease;
        let Some(next) = value.checked_add(1) else {
            return;
        };
        let code: Box<str> = error.code().into();
        let message: Box<str> = error.detail().into();
        let diagnostic = NativeEngineDiagnostic {
            code: utf8(code.as_bytes()),
            message: utf8(message.as_bytes()),
            source: utf8(b""),
        };
        self.diagnostic_leases.insert(
            value,
            OperationDiagnosticLease {
                _code: code,
                _message: message,
                diagnostic: Box::new(diagnostic),
            },
        );
        self.next_diagnostic_lease = next;
        let lease = &self.diagnostic_leases[&value];
        // Callback checked the output pointer; generated C# copies and releases this exact lease.
        unsafe {
            *receipt = NativeOperationErrorReceipt {
                service: utf8(b"Dynamics"),
                operation: utf8(operation),
                status: 0,
                diagnostics: NativeEngineDiagnosticLease {
                    handle: NativeEngineDiagnosticLeaseHandle { value },
                    diagnostics: std::ptr::from_ref(lease.diagnostic.as_ref()),
                    diagnostics_len: 1,
                },
            };
        }
    }
}

pub(super) unsafe extern "C" fn destroy_operation_diagnostic_lease(
    context: *mut c_void,
    handle: NativeEngineDiagnosticLeaseHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeDynamicsBridge>() };
    i32::from(handle.value != 0 && bridge.diagnostic_leases.remove(&handle.value).is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operation_diagnostics_remain_owned_until_exact_release() {
        let spatial = crate::spatial::RuntimeSpatialBridge::new();
        let mut bridge = RuntimeDynamicsBridge::new(spatial.collision_source());
        let context = std::ptr::from_mut(&mut bridge).cast();
        let mut receipts = Vec::new();
        for _ in 0..32 {
            let mut receipt = unsafe { std::mem::zeroed::<NativeOperationErrorReceipt>() };
            bridge.retain_operation_error(
                &CsharpEngineServicesError::new("dynamics-tether-budget-exceeded", "budget"),
                &mut receipt,
                b"SetFixedTether",
            );
            receipts.push(receipt);
        }
        for receipt in receipts {
            let diagnostic = unsafe { &*receipt.diagnostics.diagnostics };
            let code =
                unsafe { std::slice::from_raw_parts(diagnostic.code.bytes, diagnostic.code.len) };
            assert_eq!(code, b"dynamics-tether-budget-exceeded");
            assert_eq!(
                unsafe { destroy_operation_diagnostic_lease(context, receipt.diagnostics.handle) },
                ABI_OK
            );
            assert_eq!(
                unsafe { destroy_operation_diagnostic_lease(context, receipt.diagnostics.handle) },
                0
            );
        }
        assert!(bridge.diagnostic_leases.is_empty());
    }
}
