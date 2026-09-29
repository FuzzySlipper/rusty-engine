//! Borrowed refusal diagnostics shared by named service bridges. A bridge keeps
//! its latest refusal here; the receipt's pointers stay valid until its next
//! refusal replaces them. Generated callers copy them before throwing, so
//! nothing is destroyed explicitly.
use crate::CsharpEngineServicesError;
use csharp_engine_abi::*;

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
