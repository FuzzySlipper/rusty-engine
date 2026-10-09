//! The host the product runs in: what it offers the product, and the
//! product's request to end it (a title menu's Quit).
use crate::NativeOperationErrorReceipt;
use std::ffi::c_void;

/// `exit_available` is true when the product has its own window (window
/// output): a requested exit closes it and stops the host, as closing the
/// window does. Streamed output has no window to close, so a product hides
/// its Quit there.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeHostReadout {
    pub exit_available: bool,
}

pub type NativeReadHost = unsafe extern "C" fn(
    *mut c_void,
    *mut NativeHostReadout,
    *mut NativeOperationErrorReceipt,
) -> i32;

/// Explicit empty request, as the generated direct API takes one input.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeHostExitRequest {
    pub reserved: u32,
}

/// Closes the window and stops the host once the current product call
/// returns. Refuses with `ENGINE_HOST_EXIT_UNAVAILABLE` where
/// `exit_available` is false.
pub type NativeRequestHostExit = unsafe extern "C" fn(
    *mut c_void,
    *const NativeHostExitRequest,
    *mut NativeOperationErrorReceipt,
) -> i32;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeHostApi {
    pub context: *mut c_void,
    pub read: NativeReadHost,
    pub request_exit: NativeRequestHostExit,
}
