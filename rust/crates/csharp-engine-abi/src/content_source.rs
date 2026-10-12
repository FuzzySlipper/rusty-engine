//! A `rusty dev` session's source content: the files a developer edits, as
//! opposed to the immutable admitted content. Only `rusty dev` staging names
//! a source content root; everywhere else every call refuses.

use crate::{NativeContentSha256, NativeUtf8Slice};
use std::ffi::c_void;

/// A content-root-relative path in the product's source content (`rusty dev`).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeContentSourceReadRequest {
    pub path: NativeUtf8Slice,
}

/// JSON pointer edits to a source JSON file that must still have
/// `expected_sha256` (the SHA-256 of the bytes a read returned): `edits` is a JSON array of RFC 6902 `replace`, `add`
/// and `remove` operations.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeContentSourceEditRequest {
    pub path: NativeUtf8Slice,
    pub expected_sha256: NativeContentSha256,
    pub edits: NativeUtf8Slice,
}

pub type NativeReadContentSource = unsafe extern "C" fn(
    *mut c_void,
    *const NativeContentSourceReadRequest,
    *mut crate::NativeByteResult,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;

pub type NativeEditContentSource = unsafe extern "C" fn(
    *mut c_void,
    *const NativeContentSourceEditRequest,
    *mut NativeContentSha256,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
