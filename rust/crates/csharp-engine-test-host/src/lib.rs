//! The Engine's real C# service set, in process and headless, for product
//! unit tests. `rusty_engine_test_host_create` builds the same
//! `EngineServiceSet` a running product receives, without a renderer, browser,
//! window or audio device. A test runs each group of Engine calls inside a
//! product call, so ownership, admission and handle rules refuse exactly as
//! they do at run time. The renderer work a call produces is dropped.

use std::{collections::BTreeMap, ffi::c_void, path::PathBuf, ptr, sync::Arc};

use csharp_engine_abi::*;
use csharp_engine_services::{
    parse_runtime_appearance_catalog, CsharpEngineServicesError, EngineServiceSet,
};
use runtime_diagnostics::{RuntimeDiagnosticsConfig, RuntimeDiagnosticsSink};
use runtime_lifecycle::{RuntimeControlRevision, RuntimeGeneration, RuntimeInstanceId};
use runtime_ui::RuntimeUiRuntimeBinding;

const ABI_OK: i32 = 1;
/// The standard realtime runtime's fixed-step rate, which gameplay time
/// requests convert their durations at.
const STANDARD_REALTIME_HZ: u32 = 60;

/// The one product runtime a test host stands for.
const BINDING: RuntimeUiRuntimeBinding = RuntimeUiRuntimeBinding::new(
    RuntimeInstanceId::new(1),
    RuntimeGeneration::new(1),
    RuntimeControlRevision::new(1),
);

struct TestHost {
    services: Box<EngineServiceSet>,
    sealed: bool,
    refusal: Refusal,
}

/// The latest refusal a host reported; its text stays valid until the next.
#[derive(Default)]
struct Refusal {
    _text: [Box<str>; 2],
    diagnostic: Option<NativeEngineDiagnostic>,
}

impl Refusal {
    fn report(
        &mut self,
        error: &CsharpEngineServicesError,
        receipt: *mut NativeOperationErrorReceipt,
    ) {
        if receipt.is_null() {
            return;
        }
        let text: [Box<str>; 2] = [error.code().into(), error.detail().into()];
        let slice = |value: &str| NativeUtf8Slice {
            bytes: value.as_ptr(),
            len: value.len(),
        };
        self.diagnostic = Some(NativeEngineDiagnostic {
            code: slice(&text[0]),
            message: slice(&text[1]),
            source: NativeUtf8Slice::default(),
        });
        self._text = text;
        let diagnostic = self.diagnostic.as_ref().expect("just stored");
        // SAFETY: the caller supplies this out pointer and copies the
        // diagnostic before its next call on this host.
        unsafe {
            *receipt = NativeOperationErrorReceipt {
                diagnostics: diagnostic,
                diagnostics_len: 1,
            };
        }
    }
}

thread_local! {
    /// A creation refusal has no host to keep its text; the creating thread
    /// holds it until its next creation.
    static CREATE_REFUSAL: std::cell::RefCell<Refusal> = std::cell::RefCell::default();
}

fn clear(receipt: *mut NativeOperationErrorReceipt) {
    if !receipt.is_null() {
        // SAFETY: a non-null receipt is the caller's writable out value.
        unsafe {
            *receipt = NativeOperationErrorReceipt {
                diagnostics: ptr::null(),
                diagnostics_len: 0,
            };
        }
    }
}

/// # Safety
/// `request` must be valid for the call, with its borrowed slices; `result`
/// and `receipt` must be writable. The returned table is valid until its
/// `destroy`, which must be called exactly once.
#[no_mangle]
pub unsafe extern "C" fn rusty_engine_test_host_create(
    request: *const NativeEngineTestHostRequest,
    result: *mut NativeEngineTestHostApi,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    clear(receipt);
    if request.is_null() || result.is_null() {
        return 0;
    }
    match unsafe { create(&*request) } {
        Ok(api) => {
            unsafe { *result = api };
            ABI_OK
        }
        Err(error) => {
            CREATE_REFUSAL.with(|refusal| refusal.borrow_mut().report(&error, receipt));
            0
        }
    }
}

unsafe fn create(
    request: &NativeEngineTestHostRequest,
) -> Result<NativeEngineTestHostApi, CsharpEngineServicesError> {
    if request.fingerprint != PRODUCT_ABI_FINGERPRINT {
        return Err(CsharpEngineServicesError::new(
            "CSHARP_TEST_HOST_ABI",
            "the test host library and the Rusty.Engine SDK are from different pairs; use the runtime pack of the pinned pair",
        ));
    }
    let persistence_root = unsafe { utf8(request.persistence_root, "persistence root") }?;
    let persistence_root = (!persistence_root.is_empty()).then(|| PathBuf::from(persistence_root));
    let files = if request.content_len == 0 {
        &[][..]
    } else if request.content.is_null() {
        return Err(CsharpEngineServicesError::new(
            "CSHARP_TEST_HOST_CONTENT",
            "content had a length without files",
        ));
    } else {
        // SAFETY: the caller borrows `content_len` files for this call.
        unsafe { std::slice::from_raw_parts(request.content, request.content_len) }
    };
    let mut content = BTreeMap::new();
    for file in files {
        let path = unsafe {
            utf8(
                NativeUtf8Slice {
                    bytes: file.path,
                    len: file.path_len,
                },
                "content path",
            )
        }?;
        let bytes = unsafe { bytes(file.bytes, file.bytes_len) }?;
        content.insert(path.to_owned(), Arc::<[u8]>::from(bytes));
    }
    let catalog = parse_runtime_appearance_catalog(
        content
            .get("runtime-appearances.json")
            .map(|bytes: &Arc<[u8]>| bytes.as_ref()),
    )?;
    let diagnostics =
        RuntimeDiagnosticsSink::new(RuntimeDiagnosticsConfig::default()).map_err(|error| {
            CsharpEngineServicesError::new("CSHARP_TEST_HOST_DIAGNOSTICS", error.to_string())
        })?;
    // The service table holds raw pointers into the set, so it is boxed once
    // and never moves until destroy.
    let mut services = Box::new(EngineServiceSet::new(
        catalog,
        content,
        persistence_root,
        diagnostics,
    )?);
    // Gameplay time answers as in the standard realtime runtime; a request
    // stages and reads back within its call, with no lifecycle to settle it.
    services.set_gameplay_time(
        STANDARD_REALTIME_HZ,
        runtime_lifecycle::GameplayTime::default(),
    );
    let engine = services.api();
    let host = Box::new(TestHost {
        services,
        sealed: false,
        refusal: Refusal::default(),
    });
    Ok(NativeEngineTestHostApi {
        context: Box::into_raw(host).cast(),
        engine,
        begin_call,
        finish_call,
        destroy,
    })
}

unsafe fn utf8<'a>(
    value: NativeUtf8Slice,
    field: &str,
) -> Result<&'a str, CsharpEngineServicesError> {
    std::str::from_utf8(unsafe { bytes(value.bytes, value.len) }?).map_err(|_| {
        CsharpEngineServicesError::new("CSHARP_UTF8", format!("test host {field} was not UTF-8"))
    })
}

unsafe fn bytes<'a>(pointer: *const u8, len: usize) -> Result<&'a [u8], CsharpEngineServicesError> {
    match (pointer.is_null(), len) {
        (_, 0) => Ok(&[]),
        (true, _) => Err(CsharpEngineServicesError::new(
            "CSHARP_BYTES_POINTER",
            "test host bytes had a length without storage",
        )),
        // SAFETY: the caller borrows this range for the create call.
        (false, len) => Ok(unsafe { std::slice::from_raw_parts(pointer, len) }),
    }
}

unsafe extern "C" fn begin_call(context: *mut c_void) -> i32 {
    if context.is_null() {
        return 0;
    }
    let host = unsafe { &mut *context.cast::<TestHost>() };
    host.services.begin_call(BINDING);
    ABI_OK
}

unsafe extern "C" fn finish_call(
    context: *mut c_void,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    clear(receipt);
    if context.is_null() {
        return 0;
    }
    let host = unsafe { &mut *context.cast::<TestHost>() };
    let finished = host.services.finish_call();
    // The runtime drops the constructor's content snapshot once the product
    // exists; the first finished call stands for that point.
    if !host.sealed {
        host.services.seal_resource_selection();
        host.sealed = true;
    }
    match finished {
        Ok(_) => ABI_OK,
        Err(error) => {
            host.refusal.report(&error, receipt);
            0
        }
    }
}

unsafe extern "C" fn destroy(context: *mut c_void) {
    if !context.is_null() {
        // SAFETY: `context` came from `Box::into_raw` in `create`, and the
        // caller destroys it exactly once.
        drop(unsafe { Box::from_raw(context.cast::<TestHost>()) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(
        fingerprint: NativeProductAbiFingerprint,
        root: &str,
    ) -> NativeEngineTestHostRequest {
        NativeEngineTestHostRequest {
            fingerprint,
            persistence_root: NativeUtf8Slice {
                bytes: root.as_ptr(),
                len: root.len(),
            },
            content: ptr::null(),
            content_len: 0,
        }
    }

    fn codes(receipt: &NativeOperationErrorReceipt) -> Vec<String> {
        (0..receipt.diagnostics_len)
            .map(|index| {
                let code = unsafe { (*receipt.diagnostics.add(index)).code };
                let bytes = unsafe { std::slice::from_raw_parts(code.bytes, code.len) };
                String::from_utf8(bytes.to_vec()).unwrap()
            })
            .collect()
    }

    #[test]
    fn another_pairs_sdk_is_refused() {
        let mut api = std::mem::MaybeUninit::<NativeEngineTestHostApi>::uninit();
        let mut receipt = NativeOperationErrorReceipt {
            diagnostics: ptr::null(),
            diagnostics_len: 0,
        };
        let other = NativeProductAbiFingerprint {
            word0: PRODUCT_ABI_FINGERPRINT.word0 ^ 1,
            ..PRODUCT_ABI_FINGERPRINT
        };
        let status = unsafe {
            rusty_engine_test_host_create(&request(other, ""), api.as_mut_ptr(), &mut receipt)
        };
        assert_eq!(status, 0);
        assert_eq!(codes(&receipt), ["CSHARP_TEST_HOST_ABI"]);
    }

    #[test]
    fn services_run_inside_calls_with_isolated_persistence_scopes() {
        let root = tempfile::tempdir().unwrap();
        let root_text = root.path().to_str().unwrap();
        let mut api = std::mem::MaybeUninit::<NativeEngineTestHostApi>::uninit();
        let mut receipt = NativeOperationErrorReceipt {
            diagnostics: ptr::null(),
            diagnostics_len: 0,
        };
        let status = unsafe {
            rusty_engine_test_host_create(
                &request(PRODUCT_ABI_FINGERPRINT, root_text),
                api.as_mut_ptr(),
                &mut receipt,
            )
        };
        assert_eq!(status, ABI_OK);
        let api = unsafe { api.assume_init() };
        let persistence = api.engine.persistence;
        assert_eq!(unsafe { (api.begin_call)(api.context) }, ABI_OK);
        let mut stores = [NativePersistenceStoreHandle::default(); 2];
        for (store, scope) in stores.iter_mut().zip(["first", "second"]) {
            let open = NativePersistenceOpenRequest {
                scope: NativeUtf8Slice {
                    bytes: scope.as_ptr(),
                    len: scope.len(),
                },
            };
            let status = unsafe {
                (persistence.open_store)(persistence.context, &open, store, &mut receipt)
            };
            assert_eq!(status, ABI_OK);
        }
        assert_ne!(stores[0], stores[1]);
        assert_eq!(
            unsafe { (api.finish_call)(api.context, &mut receipt) },
            ABI_OK
        );
        assert!(root.path().join("first").is_dir() && root.path().join("second").is_dir());
        unsafe { (api.destroy)(api.context) };
    }

    #[test]
    fn refusals_carry_their_engine_codes() {
        let mut api = std::mem::MaybeUninit::<NativeEngineTestHostApi>::uninit();
        let mut receipt = NativeOperationErrorReceipt {
            diagnostics: ptr::null(),
            diagnostics_len: 0,
        };
        let status = unsafe {
            rusty_engine_test_host_create(
                &request(PRODUCT_ABI_FINGERPRINT, ""),
                api.as_mut_ptr(),
                &mut receipt,
            )
        };
        assert_eq!(status, ABI_OK);
        let api = unsafe { api.assume_init() };
        assert_eq!(unsafe { (api.begin_call)(api.context) }, ABI_OK);
        let spatial = api.engine.spatial;
        let mut replaced = NativeNavigationReplaceReceipt::default();
        let unknown_session = NativeNavigationReplaceRequest {
            session: NativeSpatialSessionHandle { value: 99 },
            config: NativePlanarNavConfig {
                grid_id: 1,
                cell_size: 1.0,
                chunk_size: 16,
                max_step_cells: 1,
            },
            cells: ptr::null(),
            cells_len: 0,
        };
        assert_eq!(
            unsafe {
                (spatial.replace_navigation)(
                    spatial.context,
                    &unknown_session,
                    &mut replaced,
                    &mut receipt,
                )
            },
            0
        );
        assert_eq!(codes(&receipt), ["CSHARP_SPATIAL_SESSION"]);
        assert_eq!(
            unsafe { (api.finish_call)(api.context, &mut receipt) },
            ABI_OK
        );
        unsafe { (api.destroy)(api.context) };
    }
}
