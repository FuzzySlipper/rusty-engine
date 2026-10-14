use std::ffi::c_void;

use crate::NativeUtf8Slice;

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeDiagnosticsSeverity {
    Debug = 0,
    Info = 1,
    Warning = 2,
    Error = 3,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeDiagnosticsDisposition {
    Accepted = 0,
    RejectedRecoverable = 1,
    Degraded = 2,
    ResyncRequired = 3,
    Terminal = 4,
}

/// One borrowed, typed product diagnostic. Strings are copied and validated by
/// the Engine sink during this direct call; no C# file handle or log protocol
/// is exposed.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDiagnosticsPublishRequest {
    pub severity: NativeDiagnosticsSeverity,
    pub disposition: NativeDiagnosticsDisposition,
    pub source: NativeUtf8Slice,
    pub code: NativeUtf8Slice,
    pub message: NativeUtf8Slice,
    pub correlation: NativeUtf8Slice,
}

pub type NativePublishDiagnostics = unsafe extern "C" fn(
    context: *mut c_void,
    request: *const NativeDiagnosticsPublishRequest,
) -> i32;

pub type NativeReadRendererDiagnostics =
    unsafe extern "C" fn(context: *mut c_void, readout: *mut crate::NativeByteResult) -> i32;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDiagnosticsApi {
    pub context: *mut c_void,
    pub read_renderer: NativeReadRendererDiagnostics,
    pub publish: NativePublishDiagnostics,
}
