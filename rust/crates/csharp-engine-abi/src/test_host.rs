use crate::*;
use std::ffi::c_void;

/// What an in-process test host is created with: the SDK's ABI fingerprint,
/// which must match the library's, the persistence root (empty: none) and
/// the product content files, borrowed for the call.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeEngineTestHostRequest {
    pub fingerprint: NativeProductAbiFingerprint,
    pub persistence_root: NativeUtf8Slice,
    pub content: *const NativeContentFile,
    pub content_len: usize,
}

pub type NativeBeginEngineTestHostCall = unsafe extern "C" fn(*mut c_void) -> i32;
pub type NativeFinishEngineTestHostCall =
    unsafe extern "C" fn(*mut c_void, *mut NativeOperationErrorReceipt) -> i32;
pub type NativeDestroyEngineTestHost = unsafe extern "C" fn(*mut c_void);

/// The Engine's real service set, headless, for a product's unit tests. The
/// test opens a product call, uses `engine` as a product callback would,
/// and finishes the call; `destroy` releases everything once.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeEngineTestHostApi {
    pub context: *mut c_void,
    pub engine: NativeEngineApi,
    pub begin_call: NativeBeginEngineTestHostCall,
    pub finish_call: NativeFinishEngineTestHostCall,
    pub destroy: NativeDestroyEngineTestHost,
}

/// The test host library's one export, `rusty_engine_test_host_create`.
pub type NativeCreateEngineTestHost = unsafe extern "C" fn(
    *const NativeEngineTestHostRequest,
    *mut NativeEngineTestHostApi,
    *mut NativeOperationErrorReceipt,
) -> i32;
