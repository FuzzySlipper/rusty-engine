//! Call-scoped error storage shared by named service bridges. Generated callers
//! copy the receipt and release its lease before returning to product code.
use crate::CsharpEngineServicesError;
use csharp_engine_abi::*;
use std::collections::BTreeMap;

struct Diagnostic {
    _code: Box<str>,
    _message: Box<str>,
    value: Box<NativeEngineDiagnostic>,
}

#[derive(Default)]
pub(crate) struct OperationDiagnostics {
    next: u64,
    leases: BTreeMap<u64, Diagnostic>,
}

impl OperationDiagnostics {
    pub(crate) fn retain(
        &mut self,
        error: &CsharpEngineServicesError,
        receipt: *mut NativeOperationErrorReceipt,
    ) {
        if receipt.is_null() {
            return;
        }
        let Some(handle) = self.next.checked_add(1) else {
            return;
        };
        self.next = handle;
        let code: Box<str> = error.code().into();
        let message: Box<str> = error.detail().into();
        let utf8 = |value: &str| NativeUtf8Slice {
            bytes: value.as_ptr(),
            len: value.len(),
        };
        let value = Box::new(NativeEngineDiagnostic {
            code: utf8(&code),
            message: utf8(&message),
            source: utf8(""),
        });
        let lease = Diagnostic {
            _code: code,
            _message: message,
            value,
        };
        // The generated caller supplies this out pointer and owns the exact lease.
        unsafe {
            *receipt = NativeOperationErrorReceipt {
                service: utf8(""),
                operation: utf8(""),
                status: 0,
                diagnostics: NativeEngineDiagnosticLease {
                    handle: NativeEngineDiagnosticLeaseHandle { value: handle },
                    diagnostics: std::ptr::from_ref(lease.value.as_ref()),
                    diagnostics_len: 1,
                },
            };
        }
        self.leases.insert(handle, lease);
    }

    pub(crate) fn destroy(&mut self, handle: NativeEngineDiagnosticLeaseHandle) -> i32 {
        i32::from(self.leases.remove(&handle.value).is_some())
    }
}
