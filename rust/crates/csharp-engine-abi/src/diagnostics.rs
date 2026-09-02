use std::ffi::c_void;

use crate::NativeUtf8Slice;

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeDiagnosticsSeverity {
    Debug = 1,
    Info = 2,
    Warning = 3,
    Error = 4,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeDiagnosticsDisposition {
    Accepted = 1,
    RejectedRecoverable = 2,
    Degraded = 3,
    ResyncRequired = 4,
    Terminal = 5,
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
    unsafe extern "C" fn(context: *mut c_void, readout: *mut crate::NativeByteLease) -> i32;

pub type NativeDestroyDiagnosticsByteLease =
    unsafe extern "C" fn(context: *mut c_void, lease: crate::NativeByteLeaseHandle) -> i32;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDiagnosticsApi {
    pub context: *mut c_void,
    pub read_renderer: NativeReadRendererDiagnostics,
    pub publish: NativePublishDiagnostics,
    pub destroy_byte_lease: NativeDestroyDiagnosticsByteLease,
}
