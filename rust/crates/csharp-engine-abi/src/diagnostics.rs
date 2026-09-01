use std::ffi::c_void;

pub type NativeReadRendererDiagnostics =
    unsafe extern "C" fn(context: *mut c_void, readout: *mut crate::NativeByteLease) -> i32;

pub type NativeDestroyDiagnosticsByteLease =
    unsafe extern "C" fn(context: *mut c_void, lease: crate::NativeByteLeaseHandle) -> i32;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDiagnosticsApi {
    pub context: *mut c_void,
    pub read_renderer: NativeReadRendererDiagnostics,
    pub destroy_byte_lease: NativeDestroyDiagnosticsByteLease,
}
