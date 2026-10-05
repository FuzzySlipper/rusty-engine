//! Outbound HTTPS for the product: `svc-http` transfers behind the generated
//! table. Workers never reach the product. Each product call begins with a
//! fresh snapshot of every transfer, so a call reads one consistent state.
use std::{collections::BTreeMap, ffi::c_void, path::PathBuf, sync::Arc};

use csharp_engine_abi::*;
use svc_http::{
    Destination, Failure, Headers, HttpClient, Transfer, TransferSnapshot, TransferState,
};

use crate::{
    composition::{borrowed_utf8, ABI_OK},
    operation_diagnostics::{clear_receipt, BorrowedResult, OperationDiagnostics},
    CsharpEngineServicesError,
};

struct Entry {
    transfer: Transfer,
    snapshot: TransferSnapshot,
}

pub(crate) struct RuntimeHttpBridge {
    /// Created on first use, so a product that never fetches starts nothing.
    client: Option<HttpClient>,
    transfers: BTreeMap<u64, Entry>,
    libraries: BTreeMap<u64, PathBuf>,
    next: u64,
    diagnostics: OperationDiagnostics,
    borrowed: BorrowedResult,
}

fn error(message: impl Into<String>) -> CsharpEngineServicesError {
    CsharpEngineServicesError::new("CSHARP_HTTP", message)
}

impl RuntimeHttpBridge {
    pub(crate) fn new() -> Self {
        Self {
            client: None,
            transfers: BTreeMap::new(),
            libraries: BTreeMap::new(),
            next: 1,
            diagnostics: OperationDiagnostics::default(),
            borrowed: BorrowedResult::default(),
        }
    }

    pub(crate) fn begin_call(&mut self) {
        for entry in self.transfers.values_mut() {
            if entry.snapshot.state == TransferState::Running {
                entry.snapshot = entry.transfer.snapshot();
            }
        }
    }

    fn handle(&mut self) -> u64 {
        let value = self.next;
        self.next += 1;
        value
    }

    fn start(
        &mut self,
        url: NativeUtf8Slice,
        headers: Headers,
        destination: Destination,
    ) -> Result<NativeHttpTransferHandle, CsharpEngineServicesError> {
        let url = utf8(url, "url")?;
        let transfer = self
            .client
            .get_or_insert_with(HttpClient::default)
            .start(url, headers, destination)
            .map_err(|cause| error(cause.0))?;
        let snapshot = transfer.snapshot();
        let value = self.handle();
        self.transfers.insert(value, Entry { transfer, snapshot });
        Ok(NativeHttpTransferHandle { value })
    }

    fn get(
        &mut self,
        request: &NativeHttpGetRequest,
    ) -> Result<NativeHttpTransferHandle, CsharpEngineServicesError> {
        let headers = Headers {
            user_agent: utf8(request.user_agent, "user agent")?.to_owned(),
            accept: utf8(request.accept, "accept")?.to_owned(),
            bearer_token: utf8(request.bearer_token, "bearer token")?.to_owned(),
            if_none_match: utf8(request.if_none_match, "if-none-match")?.to_owned(),
        };
        self.start(request.url, headers, Destination::Memory)
    }

    fn download(
        &mut self,
        request: &NativeHttpDownloadRequest,
    ) -> Result<NativeHttpTransferHandle, CsharpEngineServicesError> {
        let directory = self.library(request.library)?.clone();
        let headers = Headers {
            user_agent: utf8(request.user_agent, "user agent")?.to_owned(),
            accept: utf8(request.accept, "accept")?.to_owned(),
            bearer_token: utf8(request.bearer_token, "bearer token")?.to_owned(),
            if_none_match: String::new(),
        };
        let file_name = utf8(request.file_name, "file name")?.to_owned();
        self.start(
            request.url,
            headers,
            Destination::File {
                directory,
                file_name,
            },
        )
    }

    fn entry(&self, handle: NativeHttpTransferHandle) -> Result<&Entry, CsharpEngineServicesError> {
        self.transfers
            .get(&handle.value)
            .ok_or_else(|| error("unknown transfer"))
    }

    fn read(
        &self,
        handle: NativeHttpTransferHandle,
    ) -> Result<NativeHttpTransferReadout, CsharpEngineServicesError> {
        let snapshot = &self.entry(handle)?.snapshot;
        let (state, failure) = match snapshot.state {
            TransferState::Running => (NativeHttpTransferState::Running, NativeHttpFailure::None),
            TransferState::Completed => {
                (NativeHttpTransferState::Completed, NativeHttpFailure::None)
            }
            TransferState::Cancelled => {
                (NativeHttpTransferState::Cancelled, NativeHttpFailure::None)
            }
            TransferState::Failed(failure) => (
                NativeHttpTransferState::Failed,
                match failure {
                    Failure::Connect => NativeHttpFailure::Connect,
                    Failure::Interrupted => NativeHttpFailure::Interrupted,
                    Failure::Status => NativeHttpFailure::Status,
                    Failure::BodyTooLarge => NativeHttpFailure::BodyTooLarge,
                    Failure::Storage => NativeHttpFailure::Storage,
                    Failure::Protocol => NativeHttpFailure::Protocol,
                },
            ),
        };
        Ok(NativeHttpTransferReadout {
            state,
            failure,
            status: snapshot.status,
            received_bytes: snapshot.received_bytes,
            expected_bytes: snapshot.expected_bytes,
        })
    }

    fn read_headers(
        &mut self,
        handle: NativeHttpTransferHandle,
    ) -> Result<NativeHttpHeaderResult, CsharpEngineServicesError> {
        let headers = Arc::clone(&self.entry(handle)?.snapshot.headers);
        let values: Vec<NativeHttpHeader> = headers
            .iter()
            .map(|(name, value)| NativeHttpHeader {
                name: slice(name.as_bytes()),
                value: slice(value.as_bytes()),
            })
            .collect();
        let result = NativeHttpHeaderResult {
            headers: values.as_ptr(),
            headers_len: values.len(),
        };
        self.borrowed.hold((headers, values));
        Ok(result)
    }

    fn read_bytes(
        &mut self,
        handle: NativeHttpTransferHandle,
        diagnostic: bool,
    ) -> Result<NativeByteResult, CsharpEngineServicesError> {
        let snapshot = &self.entry(handle)?.snapshot;
        let bytes: Arc<[u8]> = if diagnostic {
            Arc::from(snapshot.diagnostic.as_bytes())
        } else if snapshot.state == TransferState::Completed {
            Arc::clone(&snapshot.body)
        } else {
            return Err(error("the transfer has not completed"));
        };
        let result = NativeByteResult {
            bytes: bytes.as_ptr(),
            len: bytes.len(),
        };
        self.borrowed.hold(bytes);
        Ok(result)
    }

    fn cancel(
        &mut self,
        handle: NativeHttpTransferHandle,
    ) -> Result<(), CsharpEngineServicesError> {
        let entry = self
            .transfers
            .get_mut(&handle.value)
            .ok_or_else(|| error("unknown transfer"))?;
        entry.transfer.cancel();
        entry.snapshot = entry.transfer.snapshot();
        Ok(())
    }

    fn library(
        &self,
        handle: NativeHttpLibraryHandle,
    ) -> Result<&PathBuf, CsharpEngineServicesError> {
        self.libraries
            .get(&handle.value)
            .ok_or_else(|| error("unknown library"))
    }

    fn open_library(
        &mut self,
        request: &NativeHttpLibraryOpenRequest,
    ) -> Result<NativeHttpLibraryHandle, CsharpEngineServicesError> {
        let path = PathBuf::from(utf8(request.path, "library path")?);
        svc_http::open_library(&path).map_err(|cause| error(cause.0))?;
        let value = self.handle();
        self.libraries.insert(value, path);
        Ok(NativeHttpLibraryHandle { value })
    }

    fn read_library_files(
        &mut self,
        handle: NativeHttpLibraryHandle,
    ) -> Result<NativeHttpLibraryFileResult, CsharpEngineServicesError> {
        let files =
            svc_http::library_files(self.library(handle)?).map_err(|cause| error(cause.0))?;
        let values: Vec<NativeHttpLibraryFile> = files
            .iter()
            .map(|(name, byte_length)| NativeHttpLibraryFile {
                file_name: slice(name.as_bytes()),
                byte_length: *byte_length,
            })
            .collect();
        let result = NativeHttpLibraryFileResult {
            files: values.as_ptr(),
            files_len: values.len(),
        };
        self.borrowed.hold((files, values));
        Ok(result)
    }

    fn remove_library_file(
        &mut self,
        request: &NativeHttpLibraryFileRequest,
    ) -> Result<NativeHttpRemoveReceipt, CsharpEngineServicesError> {
        let file_name = utf8(request.file_name, "file name")?;
        let removed = svc_http::remove_library_file(self.library(request.library)?, file_name)
            .map_err(|cause| error(cause.0))?;
        Ok(NativeHttpRemoveReceipt { removed })
    }
}

/// The borrowed text, valid only during the ABI call that supplied it.
fn utf8<'call>(
    value: NativeUtf8Slice,
    field: &'static str,
) -> Result<&'call str, CsharpEngineServicesError> {
    unsafe { borrowed_utf8(value.bytes, value.len, field) }
}

fn slice(bytes: &[u8]) -> NativeUtf8Slice {
    NativeUtf8Slice {
        bytes: bytes.as_ptr(),
        len: bytes.len(),
    }
}

/// Runs one refusable operation: writes its result or reports its refusal
/// through the receipt.
unsafe fn refusable<Input, Output>(
    context: *mut c_void,
    input: Input,
    result: *mut Output,
    receipt: *mut NativeOperationErrorReceipt,
    operation: impl FnOnce(&mut RuntimeHttpBridge, Input) -> Result<Output, CsharpEngineServicesError>,
) -> i32 {
    clear_receipt(receipt);
    if context.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeHttpBridge>() };
    match operation(bridge, input) {
        Ok(value) => {
            unsafe { *result = value };
            ABI_OK
        }
        Err(error) => {
            bridge.diagnostics.retain(&error, receipt);
            0
        }
    }
}

/// Dereferences a borrowed request, or refuses a null one.
unsafe fn request<'a, T>(request: *const T) -> Result<&'a T, CsharpEngineServicesError> {
    unsafe { request.as_ref() }.ok_or_else(|| error("the request was null"))
}

unsafe extern "C" fn get(
    context: *mut c_void,
    input: *const NativeHttpGetRequest,
    result: *mut NativeHttpTransferHandle,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    unsafe {
        refusable(context, input, result, receipt, |bridge, input| {
            bridge.get(request(input)?)
        })
    }
}

unsafe extern "C" fn download(
    context: *mut c_void,
    input: *const NativeHttpDownloadRequest,
    result: *mut NativeHttpTransferHandle,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    unsafe {
        refusable(context, input, result, receipt, |bridge, input| {
            bridge.download(request(input)?)
        })
    }
}

unsafe extern "C" fn read(
    context: *mut c_void,
    handle: NativeHttpTransferHandle,
    result: *mut NativeHttpTransferReadout,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    unsafe {
        refusable(context, handle, result, receipt, |bridge, handle| {
            bridge.read(handle)
        })
    }
}

unsafe extern "C" fn read_headers(
    context: *mut c_void,
    handle: NativeHttpTransferHandle,
    result: *mut NativeHttpHeaderResult,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    unsafe {
        refusable(context, handle, result, receipt, |bridge, handle| {
            bridge.read_headers(handle)
        })
    }
}

unsafe extern "C" fn read_body(
    context: *mut c_void,
    handle: NativeHttpTransferHandle,
    result: *mut NativeByteResult,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    unsafe {
        refusable(context, handle, result, receipt, |bridge, handle| {
            bridge.read_bytes(handle, false)
        })
    }
}

unsafe extern "C" fn read_diagnostic(
    context: *mut c_void,
    handle: NativeHttpTransferHandle,
    result: *mut NativeByteResult,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    unsafe {
        refusable(context, handle, result, receipt, |bridge, handle| {
            bridge.read_bytes(handle, true)
        })
    }
}

unsafe extern "C" fn cancel(context: *mut c_void, handle: NativeHttpTransferHandle) -> i32 {
    match unsafe { context.cast::<RuntimeHttpBridge>().as_mut() } {
        Some(bridge) => {
            if bridge.cancel(handle).is_ok() {
                ABI_OK
            } else {
                0
            }
        }
        None => 0,
    }
}

/// Releases a transfer, cancelling it if it is still running.
unsafe extern "C" fn destroy_transfer(
    context: *mut c_void,
    handle: NativeHttpTransferHandle,
) -> i32 {
    match unsafe { context.cast::<RuntimeHttpBridge>().as_mut() } {
        Some(bridge) => {
            if bridge.transfers.remove(&handle.value).is_some() {
                ABI_OK
            } else {
                0
            }
        }
        None => 0,
    }
}

unsafe extern "C" fn open_library(
    context: *mut c_void,
    input: *const NativeHttpLibraryOpenRequest,
    result: *mut NativeHttpLibraryHandle,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    unsafe {
        refusable(context, input, result, receipt, |bridge, input| {
            bridge.open_library(request(input)?)
        })
    }
}

/// Forgets a library. Its files, and downloads into it, are unaffected.
unsafe extern "C" fn destroy_library(context: *mut c_void, handle: NativeHttpLibraryHandle) -> i32 {
    match unsafe { context.cast::<RuntimeHttpBridge>().as_mut() } {
        Some(bridge) => {
            if bridge.libraries.remove(&handle.value).is_some() {
                ABI_OK
            } else {
                0
            }
        }
        None => 0,
    }
}

unsafe extern "C" fn read_library_files(
    context: *mut c_void,
    handle: NativeHttpLibraryHandle,
    result: *mut NativeHttpLibraryFileResult,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    unsafe {
        refusable(context, handle, result, receipt, |bridge, handle| {
            bridge.read_library_files(handle)
        })
    }
}

unsafe extern "C" fn remove_library_file(
    context: *mut c_void,
    input: *const NativeHttpLibraryFileRequest,
    result: *mut NativeHttpRemoveReceipt,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    unsafe {
        refusable(context, input, result, receipt, |bridge, input| {
            bridge.remove_library_file(request(input)?)
        })
    }
}

pub(crate) fn api(bridge: &mut RuntimeHttpBridge) -> NativeHttpApi {
    NativeHttpApi {
        context: (bridge as *mut RuntimeHttpBridge).cast(),
        get,
        download,
        read,
        read_headers,
        read_body,
        read_diagnostic,
        cancel,
        destroy_transfer,
        open_library,
        destroy_library,
        read_library_files,
        remove_library_file,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operation_diagnostics::{empty_receipt, receipt_codes};
    use std::{
        io::{BufRead, BufReader, Write},
        net::TcpListener,
        time::{Duration, Instant},
    };

    fn text(value: &str) -> NativeUtf8Slice {
        slice(value.as_bytes())
    }

    fn serve_once(body: &'static [u8]) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            while reader.read_line(&mut line).unwrap() > 2 {
                line.clear();
            }
            write!(
                stream,
                "HTTP/1.1 200 OK\r\ncontent-length: {}\r\netag: \"v7\"\r\nconnection: close\r\n\r\n",
                body.len()
            )
            .unwrap();
            stream.write_all(body).unwrap();
        });
        format!("http://{address}/module.rpak")
    }

    fn settle(
        bridge: &mut RuntimeHttpBridge,
        handle: NativeHttpTransferHandle,
    ) -> NativeHttpTransferReadout {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            bridge.begin_call();
            let readout = bridge.read(handle).unwrap();
            if readout.state != NativeHttpTransferState::Running || Instant::now() > deadline {
                return readout;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn observations_change_only_when_a_call_begins() {
        let mut bridge = RuntimeHttpBridge::new();
        let api = api(&mut bridge);
        let url = serve_once(b"hello");
        let request = NativeHttpGetRequest {
            url: text(&url),
            user_agent: text(""),
            accept: text(""),
            bearer_token: text(""),
            if_none_match: text(""),
        };
        let mut handle = NativeHttpTransferHandle::default();
        let mut receipt = empty_receipt();
        assert_eq!(
            unsafe { (api.get)(api.context, &request, &mut handle, &mut receipt) },
            ABI_OK
        );
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(
            bridge.read(handle).unwrap().state,
            NativeHttpTransferState::Running,
            "a call sees the snapshot taken when it began"
        );
        let done = settle(&mut bridge, handle);
        assert_eq!(done.state, NativeHttpTransferState::Completed);
        assert_eq!(
            (done.status, done.received_bytes, done.expected_bytes),
            (200, 5, 5)
        );

        let body = bridge.read_bytes(handle, false).unwrap();
        assert_eq!(
            unsafe { std::slice::from_raw_parts(body.bytes, body.len) },
            b"hello"
        );
        let headers = bridge.read_headers(handle).unwrap();
        let headers = unsafe { std::slice::from_raw_parts(headers.headers, headers.headers_len) };
        let etag = headers
            .iter()
            .find(|header| unsafe { std::slice::from_raw_parts(header.name.bytes, header.name.len) } == b"etag")
            .map(|header| unsafe { std::slice::from_raw_parts(header.value.bytes, header.value.len) }.to_vec());
        assert_eq!(etag.as_deref(), Some(&b"\"v7\""[..]));

        assert_eq!(unsafe { destroy_transfer(api.context, handle) }, ABI_OK);
        assert_eq!(
            unsafe {
                (api.read)(
                    api.context,
                    handle,
                    &mut NativeHttpTransferReadout {
                        state: NativeHttpTransferState::Running,
                        failure: NativeHttpFailure::None,
                        status: 0,
                        received_bytes: 0,
                        expected_bytes: 0,
                    },
                    &mut receipt,
                )
            },
            0
        );
        assert_eq!(receipt_codes(&receipt), ["CSHARP_HTTP"]);
    }

    #[test]
    fn a_library_receives_downloads_and_lists_and_removes_files() {
        let mut bridge = RuntimeHttpBridge::new();
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("modules");
        let library = bridge
            .open_library(&NativeHttpLibraryOpenRequest {
                path: text(path.to_str().unwrap()),
            })
            .unwrap();
        let url = serve_once(b"container bytes");
        let handle = bridge
            .download(&NativeHttpDownloadRequest {
                library,
                file_name: text("castle-1.0.rpak"),
                url: text(&url),
                user_agent: text(""),
                accept: text(""),
                bearer_token: text(""),
            })
            .unwrap();
        assert_eq!(
            settle(&mut bridge, handle).state,
            NativeHttpTransferState::Completed
        );
        let files = bridge.read_library_files(library).unwrap();
        let files = unsafe { std::slice::from_raw_parts(files.files, files.files_len) };
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].byte_length, 15);
        assert!(
            bridge.read_bytes(handle, false).unwrap().len == 0,
            "a download keeps no body"
        );
        let file = |name| NativeHttpLibraryFileRequest {
            library,
            file_name: text(name),
        };
        assert!(
            bridge
                .remove_library_file(&file("castle-1.0.rpak"))
                .unwrap()
                .removed
        );
        assert!(
            !bridge
                .remove_library_file(&file("castle-1.0.rpak"))
                .unwrap()
                .removed
        );
        assert!(bridge.remove_library_file(&file("../escape")).is_err());
    }

    #[test]
    fn a_bad_url_is_refused_at_the_call() {
        let mut bridge = RuntimeHttpBridge::new();
        let refused = bridge.get(&NativeHttpGetRequest {
            url: text("github.com/owner/repo"),
            user_agent: text(""),
            accept: text(""),
            bearer_token: text(""),
            if_none_match: text(""),
        });
        assert!(refused.is_err());
        assert!(bridge.transfers.is_empty());
    }
}
