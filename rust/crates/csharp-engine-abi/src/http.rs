//! Engine-owned outbound HTTPS: small fetches into memory and large downloads
//! into a product-chosen library directory. Transfers run on Engine threads;
//! their progress and outcome reach the product only as readouts refreshed at
//! the start of each product call. Nothing calls into the product.
use crate::*;
use std::ffi::c_void;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeHttpTransferHandle {
    pub value: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeHttpLibraryHandle {
    pub value: u64,
}

/// A GET whose body the Engine keeps in memory. Empty header fields send no
/// header (the User-Agent then names the Engine). Any HTTP status, including
/// 304 Not Modified, completes the transfer.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeHttpGetRequest {
    /// An absolute `https://` or `http://` URL. Redirects are followed.
    pub url: NativeUtf8Slice,
    pub user_agent: NativeUtf8Slice,
    pub accept: NativeUtf8Slice,
    /// Sent as `Authorization: Bearer …`, and only to the URL's own host.
    pub bearer_token: NativeUtf8Slice,
    pub if_none_match: NativeUtf8Slice,
}

/// A GET whose body is written to `file_name` in `library`. The body goes to
/// a hidden partial file first and is renamed into place only once complete,
/// replacing any file of that name. A status other than 2xx fails it.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeHttpDownloadRequest {
    pub library: NativeHttpLibraryHandle,
    /// A plain file name inside the library: no directory separators.
    pub file_name: NativeUtf8Slice,
    pub url: NativeUtf8Slice,
    pub user_agent: NativeUtf8Slice,
    pub accept: NativeUtf8Slice,
    pub bearer_token: NativeUtf8Slice,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeHttpTransferState {
    Running = 0,
    Completed = 1,
    Failed = 2,
    Cancelled = 3,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeHttpFailure {
    None = 0,
    /// Name resolution, connection or TLS failed before a response arrived.
    Connect = 1,
    /// The connection failed or stalled while the body was arriving.
    Interrupted = 2,
    /// A download answered with a status other than 2xx.
    Status = 3,
    /// A GET body exceeded the in-memory limit; use a download instead.
    BodyTooLarge = 4,
    /// Writing or moving the downloaded file failed.
    Storage = 5,
    /// The server broke HTTP, or redirected too many times.
    Protocol = 6,
}

/// The transfer as of the start of the current product call.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeHttpTransferReadout {
    pub state: NativeHttpTransferState,
    pub failure: NativeHttpFailure,
    /// The final response's status; zero until a response arrives.
    pub status: u16,
    pub received_bytes: u64,
    /// The body length the server announced; zero when it announced none.
    pub expected_bytes: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeHttpHeader {
    /// Lower-case header name.
    pub name: NativeUtf8Slice,
    pub value: NativeUtf8Slice,
}

/// The final response's headers, borrowed until the next Http call.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeHttpHeaderResult {
    pub headers: *const NativeHttpHeader,
    pub headers_len: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeHttpLibraryOpenRequest {
    /// A directory path, absolute or relative to the process directory. It is
    /// created if absent.
    pub path: NativeUtf8Slice,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeHttpLibraryFile {
    pub file_name: NativeUtf8Slice,
    pub byte_length: u64,
}

/// The library's complete files, borrowed until the next Http call. Partial
/// downloads are never listed.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeHttpLibraryFileResult {
    pub files: *const NativeHttpLibraryFile,
    pub files_len: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeHttpLibraryFileRequest {
    pub library: NativeHttpLibraryHandle,
    pub file_name: NativeUtf8Slice,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeHttpRemoveReceipt {
    /// False when no such file existed.
    pub removed: bool,
}

pub type NativeHttpGet = unsafe extern "C" fn(
    *mut c_void,
    *const NativeHttpGetRequest,
    *mut NativeHttpTransferHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeHttpDownload = unsafe extern "C" fn(
    *mut c_void,
    *const NativeHttpDownloadRequest,
    *mut NativeHttpTransferHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadHttpTransfer = unsafe extern "C" fn(
    *mut c_void,
    NativeHttpTransferHandle,
    *mut NativeHttpTransferReadout,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadHttpHeaders = unsafe extern "C" fn(
    *mut c_void,
    NativeHttpTransferHandle,
    *mut NativeHttpHeaderResult,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadHttpBody = unsafe extern "C" fn(
    *mut c_void,
    NativeHttpTransferHandle,
    *mut NativeByteResult,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadHttpDiagnostic = unsafe extern "C" fn(
    *mut c_void,
    NativeHttpTransferHandle,
    *mut NativeByteResult,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeCancelHttpTransfer =
    unsafe extern "C" fn(*mut c_void, NativeHttpTransferHandle) -> i32;
pub type NativeDestroyHttpTransfer =
    unsafe extern "C" fn(*mut c_void, NativeHttpTransferHandle) -> i32;
pub type NativeOpenHttpLibrary = unsafe extern "C" fn(
    *mut c_void,
    *const NativeHttpLibraryOpenRequest,
    *mut NativeHttpLibraryHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyHttpLibrary =
    unsafe extern "C" fn(*mut c_void, NativeHttpLibraryHandle) -> i32;
pub type NativeReadHttpLibraryFiles = unsafe extern "C" fn(
    *mut c_void,
    NativeHttpLibraryHandle,
    *mut NativeHttpLibraryFileResult,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeRemoveHttpLibraryFile = unsafe extern "C" fn(
    *mut c_void,
    *const NativeHttpLibraryFileRequest,
    *mut NativeHttpRemoveReceipt,
    *mut NativeOperationErrorReceipt,
) -> i32;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeHttpApi {
    pub context: *mut c_void,
    pub get: NativeHttpGet,
    pub download: NativeHttpDownload,
    pub read: NativeReadHttpTransfer,
    pub read_headers: NativeReadHttpHeaders,
    /// A completed GET's body.
    pub read_body: NativeReadHttpBody,
    /// UTF-8 text describing a failure; empty otherwise.
    pub read_diagnostic: NativeReadHttpDiagnostic,
    /// Stops a running transfer; its partial file is removed.
    pub cancel: NativeCancelHttpTransfer,
    pub destroy_transfer: NativeDestroyHttpTransfer,
    pub open_library: NativeOpenHttpLibrary,
    pub destroy_library: NativeDestroyHttpLibrary,
    pub read_library_files: NativeReadHttpLibraryFiles,
    pub remove_library_file: NativeRemoveHttpLibraryFile,
}
