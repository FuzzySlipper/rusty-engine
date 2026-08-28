use std::{collections::BTreeMap, ffi::c_void};

use csharp_engine_abi::*;
use runtime_lifecycle::{RuntimeControlRevision, RuntimeGeneration, RuntimeInstanceId};
use runtime_ui::{RuntimeUiProjectionEnvelope, RuntimeUiRuntimeBinding};
use serde_json::{Map, Number, Value};

use crate::{
    composition::ABI_OK,
    composition::{borrowed_utf8, CsharpEngineServicesError},
};

const INSTANCE_ID: u64 = 1;
const GENERATION: u64 = 1;
const CONTROL_REVISION: u64 = 1;

/// Callback state remains Engine-owned for the complete NativeAOT runtime lifetime.
pub(crate) struct RuntimeUiBridge {
    staged: Vec<RuntimeUiProjectionEnvelope>,
    streams: BTreeMap<u64, RuntimeUiStream>,
    staged_streams: Option<BTreeMap<u64, RuntimeUiStream>>,
    next_stream: u64,
    staged_next_stream: Option<u64>,
    callback_error: Option<CsharpEngineServicesError>,
    diagnostic_leases: BTreeMap<u64, RuntimeUiDiagnosticLease>,
    next_diagnostic_lease: u64,
}

#[derive(Debug, Clone)]
struct RuntimeUiStream {
    stream: String,
    contract: String,
    last_sequence: Option<u64>,
}

struct RuntimeUiDiagnosticLease {
    _diagnostics: Box<[RuntimeUiDiagnostic]>,
    readout: Box<[NativeEngineDiagnostic]>,
}

struct RuntimeUiDiagnostic {
    code: Box<[u8]>,
    message: Box<[u8]>,
}

impl RuntimeUiDiagnosticLease {
    fn from_error(error: &CsharpEngineServicesError) -> Self {
        let diagnostics = vec![RuntimeUiDiagnostic {
            code: error.code().as_bytes().into(),
            message: error.detail().as_bytes().into(),
        }]
        .into_boxed_slice();
        let readout = diagnostics
            .iter()
            .map(|diagnostic| NativeEngineDiagnostic {
                code: native_utf8(&diagnostic.code),
                message: native_utf8(&diagnostic.message),
                source: NativeUtf8Slice {
                    bytes: std::ptr::null(),
                    len: 0,
                },
            })
            .collect();
        Self {
            _diagnostics: diagnostics,
            readout,
        }
    }
}

impl RuntimeUiBridge {
    pub(crate) fn new() -> Self {
        Self {
            staged: Vec::new(),
            streams: BTreeMap::new(),
            staged_streams: None,
            next_stream: 1,
            staged_next_stream: None,
            callback_error: None,
            diagnostic_leases: BTreeMap::new(),
            next_diagnostic_lease: 1,
        }
    }

    pub(crate) fn begin_call(&mut self) {
        self.staged.clear();
        self.staged_streams = Some(self.streams.clone());
        self.staged_next_stream = Some(self.next_stream);
        self.callback_error = None;
    }

    pub(crate) fn discard_call(&mut self) {
        self.staged.clear();
        self.staged_streams = None;
        self.staged_next_stream = None;
        self.callback_error = None;
    }

    pub(crate) fn take_staged_call(&mut self) -> Result<RuntimeUiCall, CsharpEngineServicesError> {
        if let Some(error) = self.callback_error.take() {
            self.discard_call();
            return Err(error);
        }
        Ok(RuntimeUiCall {
            projections: std::mem::take(&mut self.staged),
            streams: self
                .staged_streams
                .take()
                .expect("every native call starts a UI stage"),
            next_stream: self
                .staged_next_stream
                .take()
                .expect("every native call starts a UI stage"),
        })
    }

    pub(crate) fn commit(&mut self, staged: RuntimeUiCall) {
        self.streams = staged.streams;
        self.next_stream = staged.next_stream;
    }

    fn retain_operation_error(
        &mut self,
        error: &CsharpEngineServicesError,
        receipt: *mut NativeOperationErrorReceipt,
    ) {
        if receipt.is_null() {
            return;
        }
        let handle = self.next_diagnostic_lease;
        let Some(next_handle) = handle.checked_add(1) else {
            return;
        };
        let lease = RuntimeUiDiagnosticLease::from_error(error);
        let diagnostic_lease = NativeEngineDiagnosticLease {
            handle: NativeEngineDiagnosticLeaseHandle { value: handle },
            diagnostics: lease.readout.as_ptr(),
            diagnostics_len: lease.readout.len(),
        };
        self.diagnostic_leases.insert(handle, lease);
        self.next_diagnostic_lease = next_handle;
        // SAFETY: null was rejected above; this out receipt is valid only for
        // the direct callback and names the independently retained lease.
        unsafe {
            *receipt = NativeOperationErrorReceipt {
                service: native_utf8(b"Ui"),
                operation: native_utf8(b"PublishProjection"),
                status: 0,
                diagnostics: diagnostic_lease,
            };
        }
    }

    fn release_operation_diagnostic_lease(
        &mut self,
        handle: NativeEngineDiagnosticLeaseHandle,
    ) -> bool {
        handle.value != 0 && self.diagnostic_leases.remove(&handle.value).is_some()
    }

    fn stage_open_stream(
        &mut self,
        request: *const NativeUiStreamRequest,
        handle: *mut NativeUiStreamHandle,
    ) -> Result<(), CsharpEngineServicesError> {
        if request.is_null() || handle.is_null() {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_UI_STREAM_POINTER",
                "C# UI stream open had a null request or result pointer",
            ));
        }
        // SAFETY: pointers are valid for this synchronous callback and each UTF-8 slice is copied.
        let request = unsafe { *request };
        let stream = unsafe { borrowed_utf8(request.stream.bytes, request.stream.len, "stream") }?
            .to_owned();
        let contract =
            unsafe { borrowed_utf8(request.contract.bytes, request.contract.len, "contract") }?
                .to_owned();
        let streams = self
            .staged_streams
            .as_mut()
            .expect("open stream only during a native call");
        let next_stream = self
            .staged_next_stream
            .as_mut()
            .expect("open stream only during a native call");
        let value = *next_stream;
        *next_stream = next_stream.checked_add(1).ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_UI_STREAM_HANDLE",
                "C# UI stream handles exhausted",
            )
        })?;
        streams.insert(
            value,
            RuntimeUiStream {
                stream,
                contract,
                last_sequence: None,
            },
        );
        // SAFETY: result pointer was checked above and belongs to the immediate direct call.
        unsafe { *handle = NativeUiStreamHandle { value } };
        Ok(())
    }

    fn destroy_stream(
        &mut self,
        handle: NativeUiStreamHandle,
    ) -> Result<(), CsharpEngineServicesError> {
        if handle.value == 0 {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_UI_STREAM",
                "C# UI stream handle was zero",
            ));
        }
        if let Some(streams) = self.staged_streams.as_mut() {
            return streams.remove(&handle.value).map(|_| ()).ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_UI_STREAM",
                    "C# UI stream handle was unknown or already closed",
                )
            });
        }
        self.streams
            .remove(&handle.value)
            .map(|_| ())
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_UI_STREAM",
                    "C# UI stream handle was unknown or already closed",
                )
            })
    }

    unsafe fn stage_projection(
        &mut self,
        projection: *const NativeUiProjection,
    ) -> Result<(), CsharpEngineServicesError> {
        if projection.is_null() {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_UI_PROJECTION_POINTER",
                "C# UI publication had a null projection pointer",
            ));
        }
        // SAFETY: the callback is synchronous and its projection points to product memory
        // retained for the direct call. `decode_structured_value` copies it before return.
        let projection = unsafe { *projection };
        let stream = self
            .staged_streams
            .as_mut()
            .expect("publish only during a native call")
            .get_mut(&projection.stream.value)
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_UI_STREAM",
                    "C# UI projection used an unopened stream handle",
                )
            })?;
        if stream
            .last_sequence
            .is_some_and(|sequence| projection.sequence <= sequence)
        {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_UI_SEQUENCE",
                "C# UI sequence did not advance",
            ));
        }
        // SAFETY: pointer/null and range checks occur in the decoder before every slice.
        let value = unsafe { decode_structured_value(projection.value) }?;
        let envelope = RuntimeUiProjectionEnvelope::new(
            RuntimeUiRuntimeBinding::new(
                RuntimeInstanceId::new(INSTANCE_ID),
                RuntimeGeneration::new(GENERATION),
                RuntimeControlRevision::new(CONTROL_REVISION),
            ),
            projection.sequence,
            &stream.stream,
            &stream.contract,
            value,
        )
        .map_err(|error| {
            CsharpEngineServicesError::new("CSHARP_UI_PROJECTION", error.to_string())
        })?;
        stream.last_sequence = Some(projection.sequence);
        self.staged.push(envelope);
        Ok(())
    }
}

pub(crate) struct RuntimeUiCall {
    pub(crate) projections: Vec<RuntimeUiProjectionEnvelope>,
    streams: BTreeMap<u64, RuntimeUiStream>,
    next_stream: u64,
}

unsafe extern "C" fn publish_ui_projection(
    context: *mut c_void,
    projection: *const NativeUiProjection,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    if receipt.is_null() {
        return 0;
    }
    // SAFETY: null was rejected above; initialize the explicit readout on
    // every observable path before an owner-backed failure can be reported.
    unsafe { *receipt = std::mem::zeroed() };
    if context.is_null() {
        return 0;
    }
    // SAFETY: `context` is a stable pointer to the Box retained by
    // `CsharpProductRuntime`, and calls are serialized by the development host.
    let bridge = unsafe { &mut *context.cast::<RuntimeUiBridge>() };
    // SAFETY: all raw callback pointers are validated and copied by this helper.
    match unsafe { bridge.stage_projection(projection) } {
        Ok(()) => 1,
        Err(error) => {
            bridge.retain_operation_error(&error, receipt);
            bridge.callback_error = Some(error);
            0
        }
    }
}

unsafe extern "C" fn destroy_ui_operation_diagnostic_lease(
    context: *mut c_void,
    handle: NativeEngineDiagnosticLeaseHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    // SAFETY: context remains valid for the runtime lifetime and the exact
    // service owning this handle also owns the lease registry.
    let bridge = unsafe { &mut *context.cast::<RuntimeUiBridge>() };
    i32::from(bridge.release_operation_diagnostic_lease(handle))
}

unsafe extern "C" fn open_ui_stream(
    context: *mut c_void,
    request: *const NativeUiStreamRequest,
    handle: *mut NativeUiStreamHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    // SAFETY: `context` is stable for the complete product lifetime.
    let bridge = unsafe { &mut *context.cast::<RuntimeUiBridge>() };
    match bridge.stage_open_stream(request, handle) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.callback_error = Some(error);
            0
        }
    }
}

unsafe extern "C" fn destroy_ui_stream(context: *mut c_void, handle: NativeUiStreamHandle) -> i32 {
    if context.is_null() {
        return 0;
    }
    // SAFETY: `context` is stable for the complete product lifetime.
    let bridge = unsafe { &mut *context.cast::<RuntimeUiBridge>() };
    match bridge.destroy_stream(handle) {
        Ok(()) => ABI_OK,
        Err(error) => {
            if bridge.staged_streams.is_some() {
                bridge.callback_error = Some(error);
            }
            0
        }
    }
}

unsafe fn decode_structured_value(
    arena: NativeStructuredValue,
) -> Result<Value, CsharpEngineServicesError> {
    if arena.node_count == 0 || arena.nodes.is_null() {
        return Err(CsharpEngineServicesError::new(
            "CSHARP_UI_NODES",
            "C# UI arena had no root node",
        ));
    }
    if arena.utf8_len > 0 && arena.utf8.is_null() {
        return Err(CsharpEngineServicesError::new(
            "CSHARP_UI_UTF8_POINTER",
            "C# UI arena had UTF-8 length without bytes",
        ));
    }
    if arena.edge_count > 0 && arena.edges.is_null() {
        return Err(CsharpEngineServicesError::new(
            "CSHARP_UI_EDGES_POINTER",
            "C# UI arena had edge length without edges",
        ));
    }
    if usize::try_from(arena.root)
        .map_err(|_| CsharpEngineServicesError::new("CSHARP_UI_ROOT", "C# UI root overflowed"))?
        >= arena.node_count
    {
        return Err(CsharpEngineServicesError::new(
            "CSHARP_UI_ROOT",
            "C# UI root was outside its node arena",
        ));
    }
    // SAFETY: pointers were checked and the fixed callback contract keeps source storage alive.
    let nodes = unsafe { std::slice::from_raw_parts(arena.nodes, arena.node_count) };
    let bytes = if arena.utf8_len == 0 {
        &[]
    } else {
        // SAFETY: non-empty byte ranges were checked for a non-null pointer above.
        unsafe { std::slice::from_raw_parts(arena.utf8, arena.utf8_len) }
    };
    let edges = if arena.edge_count == 0 {
        &[]
    } else {
        // SAFETY: non-empty edge ranges were checked for a non-null pointer above.
        unsafe { std::slice::from_raw_parts(arena.edges, arena.edge_count) }
    };
    let mut visiting = vec![false; nodes.len()];
    decode_structured_node(arena.root as usize, nodes, edges, bytes, &mut visiting)
}

fn decode_structured_node(
    index: usize,
    nodes: &[NativeStructuredValueNode],
    edges: &[u32],
    bytes: &[u8],
    visiting: &mut [bool],
) -> Result<Value, CsharpEngineServicesError> {
    let node = nodes.get(index).ok_or_else(|| {
        CsharpEngineServicesError::new("CSHARP_UI_NODE", "C# UI child was outside its node arena")
    })?;
    if visiting[index] {
        return Err(CsharpEngineServicesError::new(
            "CSHARP_UI_CYCLE",
            "C# UI arena contained a child cycle",
        ));
    }
    visiting[index] = true;
    let value = match node.kind {
        NativeStructuredValueKind::Null => Value::Null,
        NativeStructuredValueKind::Bool => Value::Bool(node.bool_value != 0),
        NativeStructuredValueKind::Number => {
            Value::Number(Number::from_f64(node.number_value).ok_or_else(|| {
                CsharpEngineServicesError::new("CSHARP_UI_NUMBER", "C# UI number was not finite")
            })?)
        }
        NativeStructuredValueKind::String => {
            Value::String(arena_text(bytes, node.text_offset, node.text_len, "text")?.to_owned())
        }
        NativeStructuredValueKind::Array => {
            let children = arena_children(node, edges)?;
            let mut values = Vec::with_capacity(children.len());
            for child in children {
                values.push(decode_structured_node(
                    child, nodes, edges, bytes, visiting,
                )?);
            }
            Value::Array(values)
        }
        NativeStructuredValueKind::Object => {
            let children = arena_children(node, edges)?;
            let mut values = Map::new();
            for child in children {
                let child_node = nodes.get(child).ok_or_else(|| {
                    CsharpEngineServicesError::new(
                        "CSHARP_UI_CHILDREN",
                        "C# UI child index exceeded nodes",
                    )
                })?;
                let key =
                    arena_text(bytes, child_node.key_offset, child_node.key_len, "key")?.to_owned();
                values.insert(
                    key,
                    decode_structured_node(child, nodes, edges, bytes, visiting)?,
                );
            }
            Value::Object(values)
        }
    };
    visiting[index] = false;
    Ok(value)
}

fn arena_children(
    node: &NativeStructuredValueNode,
    edges: &[u32],
) -> Result<Vec<usize>, CsharpEngineServicesError> {
    let first = node.first_edge as usize;
    let end = first
        .checked_add(node.child_count as usize)
        .ok_or_else(|| {
            CsharpEngineServicesError::new("CSHARP_UI_CHILDREN", "C# UI child range overflowed")
        })?;
    let child_edges = edges.get(first..end).ok_or_else(|| {
        CsharpEngineServicesError::new("CSHARP_UI_CHILDREN", "C# UI child range exceeded edges")
    })?;
    child_edges
        .iter()
        .map(|child| {
            usize::try_from(*child).map_err(|_| {
                CsharpEngineServicesError::new("CSHARP_UI_CHILDREN", "C# UI child index overflowed")
            })
        })
        .collect()
}

fn arena_text<'a>(
    bytes: &'a [u8],
    offset: u32,
    len: u32,
    field: &'static str,
) -> Result<&'a str, CsharpEngineServicesError> {
    let start = offset as usize;
    let end = start.checked_add(len as usize).ok_or_else(|| {
        CsharpEngineServicesError::new(
            "CSHARP_UI_UTF8_RANGE",
            format!("C# UI {field} range overflowed"),
        )
    })?;
    let slice = bytes.get(start..end).ok_or_else(|| {
        CsharpEngineServicesError::new(
            "CSHARP_UI_UTF8_RANGE",
            format!("C# UI {field} range exceeded bytes"),
        )
    })?;
    std::str::from_utf8(slice).map_err(|_| {
        CsharpEngineServicesError::new("CSHARP_UI_UTF8", format!("C# UI {field} was not UTF-8"))
    })
}

pub(crate) fn api(bridge: &mut RuntimeUiBridge) -> NativeUiApi {
    NativeUiApi {
        context: (bridge as *mut RuntimeUiBridge).cast(),
        open_stream: open_ui_stream,
        destroy_stream: destroy_ui_stream,
        publish_projection: publish_ui_projection,
        destroy_operation_diagnostic_lease: destroy_ui_operation_diagnostic_lease,
    }
}

fn native_utf8(value: &[u8]) -> NativeUtf8Slice {
    NativeUtf8Slice {
        bytes: if value.is_empty() {
            std::ptr::null()
        } else {
            value.as_ptr()
        },
        len: value.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stream_request() -> NativeUiStreamRequest {
        NativeUiStreamRequest {
            stream: NativeUtf8Slice {
                bytes: b"fixture".as_ptr(),
                len: b"fixture".len(),
            },
            contract: NativeUtf8Slice {
                bytes: b"fixture.v1".as_ptr(),
                len: b"fixture.v1".len(),
            },
        }
    }

    #[test]
    fn publish_projection_returns_owned_error_diagnostic_and_releases_it_once() {
        let mut bridge = RuntimeUiBridge::new();
        bridge.begin_call();
        let api = api(&mut bridge);
        let mut receipt: NativeOperationErrorReceipt = unsafe { std::mem::zeroed() };

        let status =
            unsafe { (api.publish_projection)(api.context, std::ptr::null(), &mut receipt) };

        assert_eq!(status, 0);
        assert_eq!(receipt.status, 0);
        assert_eq!(
            unsafe {
                std::str::from_utf8_unchecked(std::slice::from_raw_parts(
                    receipt.service.bytes,
                    receipt.service.len,
                ))
            },
            "Ui"
        );
        assert_eq!(
            unsafe {
                std::str::from_utf8_unchecked(std::slice::from_raw_parts(
                    receipt.operation.bytes,
                    receipt.operation.len,
                ))
            },
            "PublishProjection"
        );
        assert_eq!(receipt.diagnostics.diagnostics_len, 1);
        let diagnostic = unsafe { *receipt.diagnostics.diagnostics };
        assert_eq!(
            unsafe {
                std::str::from_utf8_unchecked(std::slice::from_raw_parts(
                    diagnostic.code.bytes,
                    diagnostic.code.len,
                ))
            },
            "CSHARP_UI_PROJECTION_POINTER"
        );
        assert_eq!(
            unsafe {
                std::str::from_utf8_unchecked(std::slice::from_raw_parts(
                    diagnostic.message.bytes,
                    diagnostic.message.len,
                ))
            },
            "C# UI publication had a null projection pointer"
        );

        assert_eq!(
            unsafe {
                (api.destroy_operation_diagnostic_lease)(api.context, receipt.diagnostics.handle)
            },
            1
        );
        assert_eq!(
            unsafe {
                (api.destroy_operation_diagnostic_lease)(api.context, receipt.diagnostics.handle)
            },
            0
        );
    }

    #[test]
    fn stream_close_stages_rollback_and_committed_teardown() {
        let mut bridge = RuntimeUiBridge::new();
        let api = api(&mut bridge);
        let mut stream = NativeUiStreamHandle::default();

        bridge.begin_call();
        assert_eq!(
            unsafe { (api.open_stream)(api.context, &stream_request(), &mut stream) },
            ABI_OK
        );
        let initial_call = bridge.take_staged_call().expect("initial staged stream");
        bridge.commit(initial_call);

        let nodes = [NativeStructuredValueNode {
            kind: NativeStructuredValueKind::Null,
            bool_value: 0,
            number_value: 0.0,
            key_offset: 0,
            key_len: 0,
            text_offset: 0,
            text_len: 0,
            first_edge: 0,
            child_count: 0,
        }];
        let projection = NativeUiProjection {
            stream,
            sequence: 1,
            value: NativeStructuredValue {
                nodes: nodes.as_ptr(),
                node_count: nodes.len(),
                edges: std::ptr::null(),
                edge_count: 0,
                root: 0,
                utf8: std::ptr::null(),
                utf8_len: 0,
            },
        };
        bridge.begin_call();
        let mut receipt: NativeOperationErrorReceipt = unsafe { std::mem::zeroed() };
        assert_eq!(
            unsafe { (api.publish_projection)(api.context, &projection, &mut receipt) },
            ABI_OK,
            "typed structured projection is accepted before close"
        );
        let published_call = bridge.take_staged_call().expect("published projection");
        assert_eq!(published_call.projections.len(), 1);
        bridge.commit(published_call);

        bridge.begin_call();
        assert_eq!(unsafe { (api.destroy_stream)(api.context, stream) }, ABI_OK);
        assert_eq!(
            unsafe { (api.destroy_stream)(api.context, stream) },
            0,
            "duplicate staged close is rejected"
        );
        assert!(
            bridge.take_staged_call().is_err(),
            "failed call cannot commit"
        );

        assert_eq!(
            unsafe { (api.destroy_stream)(api.context, stream) },
            ABI_OK,
            "discard rolled the staged close back into committed state"
        );
        assert_eq!(
            unsafe { (api.destroy_stream)(api.context, stream) },
            0,
            "duplicate committed teardown is rejected"
        );

        bridge.begin_call();
        assert!(
            bridge.take_staged_call().is_ok(),
            "out-of-call close failure does not poison the next call"
        );

        bridge.begin_call();
        let mut stale_receipt: NativeOperationErrorReceipt = unsafe { std::mem::zeroed() };
        assert_eq!(
            unsafe { (api.publish_projection)(api.context, &projection, &mut stale_receipt) },
            0,
            "publish after committed close is rejected"
        );
        assert!(
            bridge.take_staged_call().is_err(),
            "stale publish prevents the call from committing"
        );
    }
}
