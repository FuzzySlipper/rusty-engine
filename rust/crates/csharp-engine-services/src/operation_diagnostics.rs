//! Borrowed call results shared by named service bridges. A bridge keeps the
//! backing of its latest result or refusal here; the pointers it returned stay
//! valid until its next result or refusal replaces them. Generated callers copy
//! them before returning to product code, so nothing is destroyed explicitly.
use crate::CsharpEngineServicesError;
use csharp_engine_abi::*;
use std::any::Any;

/// Owner of the latest borrowed `Native*Result` a bridge returned.
#[derive(Default)]
pub(crate) struct BorrowedResult(Option<Box<dyn Any>>);

impl BorrowedResult {
    /// Keeps `backing` alive until the next `hold` on this bridge. Moving a
    /// backing does not move the heap buffers its result points into.
    pub(crate) fn hold<T: 'static>(&mut self, backing: T) {
        self.0 = Some(Box::new(backing));
    }
}

/// Owner of the latest refusal a bridge reported through its receipt.
#[derive(Default)]
pub(crate) struct OperationDiagnostics {
    _text: Vec<Box<str>>,
    values: Vec<NativeEngineDiagnostic>,
}

impl OperationDiagnostics {
    /// Reports one refusal through the operation's receipt.
    pub(crate) fn retain(
        &mut self,
        error: &CsharpEngineServicesError,
        receipt: *mut NativeOperationErrorReceipt,
    ) {
        self.retain_all([(error.code(), error.detail(), "")], receipt);
    }

    /// Reports every `(code, message, source)` diagnostic of one refusal.
    pub(crate) fn retain_all<'a>(
        &mut self,
        diagnostics: impl IntoIterator<Item = (&'a str, &'a str, &'a str)>,
        receipt: *mut NativeOperationErrorReceipt,
    ) {
        if receipt.is_null() {
            return;
        }
        let mut text = Vec::new();
        let mut keep = |value: &str| {
            let value: Box<str> = value.into();
            let slice = NativeUtf8Slice {
                bytes: value.as_ptr(),
                len: value.len(),
            };
            text.push(value);
            slice
        };
        let values = diagnostics
            .into_iter()
            .map(|(code, message, source)| NativeEngineDiagnostic {
                code: keep(code),
                message: keep(message),
                source: keep(source),
            })
            .collect();
        self._text = text;
        self.values = values;
        // The generated caller supplies this out pointer and copies the
        // diagnostics before its next call on this bridge.
        unsafe {
            *receipt = NativeOperationErrorReceipt {
                diagnostics: self.values.as_ptr(),
                diagnostics_len: self.values.len(),
            };
        }
    }
}

thread_local! {
    /// Refusals reported by operations that keep no diagnostics of their own.
    static REFUSALS: std::cell::RefCell<OperationDiagnostics> = std::cell::RefCell::default();
}

/// Reports `error` through `receipt` and returns the refused ABI status. The
/// diagnostic stays valid until the next refusal on this thread; the generated
/// caller copies it before making another Engine call.
pub(crate) fn refuse(
    error: &CsharpEngineServicesError,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    REFUSALS.with(|refusals| refusals.borrow_mut().retain(error, receipt));
    0
}

/// Reports a refusal from a domain error that carries no Engine code.
pub(crate) fn refuse_as(
    code: &'static str,
    detail: impl std::fmt::Debug,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    refuse(
        &CsharpEngineServicesError::new(code, format!("{detail:?}")),
        receipt,
    )
}

/// Zeroes an operation receipt before a call writes it.
pub(crate) fn clear_receipt(receipt: *mut NativeOperationErrorReceipt) {
    if !receipt.is_null() {
        unsafe { *receipt = empty_receipt() };
    }
}

pub(crate) const fn empty_receipt() -> NativeOperationErrorReceipt {
    NativeOperationErrorReceipt {
        diagnostics: std::ptr::null(),
        diagnostics_len: 0,
    }
}

/// Copies the diagnostic codes a receipt borrows, as the generated caller does.
#[cfg(test)]
pub(crate) fn receipt_codes(receipt: &NativeOperationErrorReceipt) -> Vec<String> {
    if receipt.diagnostics_len == 0 {
        return Vec::new();
    }
    unsafe { std::slice::from_raw_parts(receipt.diagnostics, receipt.diagnostics_len) }
        .iter()
        .map(|value| {
            let bytes = unsafe { std::slice::from_raw_parts(value.code.bytes, value.code.len) };
            String::from_utf8(bytes.to_vec()).expect("diagnostic code is UTF-8")
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_refusal_stays_readable_until_the_next_one_replaces_it() {
        let mut diagnostics = OperationDiagnostics::default();
        let mut first = empty_receipt();
        diagnostics.retain(&CsharpEngineServicesError::new("first", "one"), &mut first);
        assert_eq!(receipt_codes(&first), ["first"]);
        let mut second = empty_receipt();
        diagnostics.retain_all([("a", "alpha", "source"), ("b", "beta", "")], &mut second);
        assert_eq!(receipt_codes(&second), ["a", "b"]);
    }
}
