use std::{
    collections::BTreeMap,
    ffi::c_void,
    sync::{Arc, Mutex, PoisonError},
};

use csharp_engine_abi::*;
use runtime_ui::{RuntimeUiProjectionEnvelope, RuntimeUiRuntimeBinding};
use serde_json::{Map, Number, Value};

use crate::{
    composition::ABI_OK,
    composition::{borrowed_utf8, CsharpEngineServicesError},
    content::RuntimeContentBridge,
};

const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";

/// The PNGs the product granted its UI, by image ID. The product host serves
/// them to the page until the product releases them.
#[derive(Default)]
pub struct UiImages(Mutex<BTreeMap<u64, Arc<[u8]>>>);

impl UiImages {
    pub fn png(&self, id: u64) -> Option<Arc<[u8]>> {
        self.lock().get(&id).cloned()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, BTreeMap<u64, Arc<[u8]>>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Callback state remains Engine-owned for the complete NativeAOT runtime lifetime.
pub(crate) struct RuntimeUiBridge {
    /// Projections published by the current call, in order.
    published: Vec<RuntimeUiProjectionEnvelope>,
    binding: Option<RuntimeUiRuntimeBinding>,
    streams: BTreeMap<u64, RuntimeUiStream>,
    next_stream: u64,
    images: Arc<UiImages>,
    next_image: u64,
    content: Option<*const RuntimeContentBridge>,
    operation_diagnostics: crate::operation_diagnostics::OperationDiagnostics,
}

#[derive(Debug, Clone)]
struct RuntimeUiStream {
    stream: String,
    contract: String,
    last_sequence: Option<u64>,
    /// The last committed product projection for this live stream. Delivery is
    /// deliberately separate: a new renderer attachment gets this copied
    /// baseline without asking the product to publish again.
    latest: Option<RuntimeUiProjectionEnvelope>,
}

impl RuntimeUiBridge {
    pub(crate) fn new() -> Self {
        Self {
            published: Vec::new(),
            binding: None,
            streams: BTreeMap::new(),
            next_stream: 1,
            images: Arc::default(),
            next_image: 1,
            content: None,
            operation_diagnostics: Default::default(),
        }
    }

    pub(crate) fn bind_content(&mut self, content: &RuntimeContentBridge) {
        self.content = Some(content as *const RuntimeContentBridge);
    }

    pub(crate) fn images(&self) -> Arc<UiImages> {
        Arc::clone(&self.images)
    }

    fn open_image(
        &mut self,
        request: *const NativeUiImageRequest,
        handle: *mut NativeUiImageHandle,
    ) -> Result<(), CsharpEngineServicesError> {
        if request.is_null() || handle.is_null() {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_UI_IMAGE_POINTER",
                "C# UI image open had a null request or result pointer",
            ));
        }
        // SAFETY: pointers are valid for this synchronous callback.
        let request = unsafe { *request };
        // SAFETY: the content bridge is boxed by the service set, which outlives this bridge.
        let content = self
            .content
            .and_then(|content| unsafe { &*content }.retained_content(request.content))
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_UI_IMAGE_CONTENT",
                    "C# UI image named a content reference that is not open",
                )
            })?;
        // The host serves these bytes as image/png.
        if !content.bytes.starts_with(PNG_SIGNATURE) {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_UI_IMAGE_NOT_PNG",
                format!("C# UI image `{}` is not a PNG", content.path),
            ));
        }
        let value = self.next_image;
        self.next_image += 1;
        self.images.lock().insert(value, content.bytes);
        // SAFETY: result pointer was checked above and belongs to the immediate direct call.
        unsafe { *handle = NativeUiImageHandle { value } };
        Ok(())
    }

    fn destroy_image(
        &mut self,
        handle: NativeUiImageHandle,
    ) -> Result<(), CsharpEngineServicesError> {
        self.images
            .lock()
            .remove(&handle.value)
            .map(|_| ())
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_UI_IMAGE",
                    "C# UI image handle was unknown or already released",
                )
            })
    }

    pub(crate) fn begin_call(&mut self, binding: RuntimeUiRuntimeBinding) {
        self.published.clear();
        self.binding = Some(binding);
    }

    /// Ends the call and returns the projections it published.
    pub(crate) fn finish_call(&mut self) -> Vec<RuntimeUiProjectionEnvelope> {
        self.binding = None;
        std::mem::take(&mut self.published)
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
        let value = self.next_stream;
        self.next_stream += 1;
        self.streams.insert(
            value,
            RuntimeUiStream {
                stream,
                contract,
                last_sequence: None,
                latest: None,
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
        let binding = self.binding.ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_UI_CALL",
                "C# UI projection was published outside a product call",
            )
        })?;
        let stream = self
            .streams
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
        // The browser already shows an unchanged value under the same binding;
        // only the product's sequence advances. A rebind or baseline still
        // sends `latest`.
        if stream
            .latest
            .as_ref()
            .is_some_and(|latest| latest.runtime() == binding && *latest.value() == value)
        {
            stream.last_sequence = Some(projection.sequence);
            return Ok(());
        }
        let envelope = RuntimeUiProjectionEnvelope::new(
            binding,
            projection.sequence,
            &stream.stream,
            &stream.contract,
            value,
        )
        .map_err(|error| {
            CsharpEngineServicesError::new("CSHARP_UI_PROJECTION", error.to_string())
        })?;
        stream.last_sequence = Some(projection.sequence);
        stream.latest = Some(envelope.clone());
        self.published.push(envelope);
        Ok(())
    }

    /// Copies the current projection of every live UI stream for a fresh host
    /// attachment. Rebinding changes only the Engine-owned lifecycle fence;
    /// it never advances product stream sequences or mutates retained state.
    pub(crate) fn snapshot_projections(
        &self,
        binding: RuntimeUiRuntimeBinding,
    ) -> Vec<RuntimeUiProjectionEnvelope> {
        self.streams
            .values()
            .filter_map(|stream| stream.latest.clone())
            .map(|projection| projection.with_runtime(binding))
            .collect()
    }
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
    // `CsharpProductRuntime`, and calls are serialized by the product host.
    let bridge = unsafe { &mut *context.cast::<RuntimeUiBridge>() };
    // SAFETY: all raw callback pointers are validated and copied by this helper.
    match unsafe { bridge.stage_projection(projection) } {
        Ok(()) => 1,
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, receipt);
            0
        }
    }
}

unsafe extern "C" fn open_ui_stream(
    context: *mut c_void,
    request: *const NativeUiStreamRequest,
    handle: *mut NativeUiStreamHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() {
        return 0;
    }
    // SAFETY: `context` is stable for the complete product lifetime.
    let bridge = unsafe { &mut *context.cast::<RuntimeUiBridge>() };
    match bridge.stage_open_stream(request, handle) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            0
        }
    }
}

unsafe extern "C" fn destroy_ui_stream(
    context: *mut c_void,
    handle: NativeUiStreamHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() {
        return 0;
    }
    // SAFETY: `context` is stable for the complete product lifetime.
    let bridge = unsafe { &mut *context.cast::<RuntimeUiBridge>() };
    match bridge.destroy_stream(handle) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
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

unsafe extern "C" fn open_ui_image(
    context: *mut c_void,
    request: *const NativeUiImageRequest,
    handle: *mut NativeUiImageHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() {
        return 0;
    }
    // SAFETY: `context` is stable for the complete product lifetime.
    let bridge = unsafe { &mut *context.cast::<RuntimeUiBridge>() };
    match bridge.open_image(request, handle) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            0
        }
    }
}

unsafe extern "C" fn destroy_ui_image(
    context: *mut c_void,
    handle: NativeUiImageHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() {
        return 0;
    }
    // SAFETY: `context` is stable for the complete product lifetime.
    let bridge = unsafe { &mut *context.cast::<RuntimeUiBridge>() };
    match bridge.destroy_image(handle) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            0
        }
    }
}

pub(crate) fn api(bridge: &mut RuntimeUiBridge) -> NativeUiApi {
    NativeUiApi {
        context: (bridge as *mut RuntimeUiBridge).cast(),
        open_stream: open_ui_stream,
        destroy_stream: destroy_ui_stream,
        publish_projection: publish_ui_projection,
        open_image: open_ui_image,
        destroy_image: destroy_ui_image,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_images_serve_granted_pngs_until_released() {
        let png: Arc<[u8]> = Arc::from(&b"\x89PNG\r\n\x1a\nportrait"[..]);
        let mut content = RuntimeContentBridge::new(BTreeMap::from([
            ("portrait.png".to_owned(), Arc::clone(&png)),
            ("notes.txt".to_owned(), Arc::from(&b"text"[..])),
        ]));
        let content_api = crate::content::api(&mut content);
        let open = |path: &str| {
            let mut reference = NativeContentReferenceHandle::default();
            let request = NativeContentOpenRequest {
                path: NativeUtf8Slice {
                    bytes: path.as_ptr(),
                    len: path.len(),
                },
            };
            let status = unsafe {
                (content_api.open_reference)(
                    content_api.context,
                    &request,
                    &mut reference,
                    std::ptr::null_mut(),
                )
            };
            assert_eq!(status, ABI_OK);
            reference
        };
        let mut bridge = RuntimeUiBridge::new();
        bridge.bind_content(&content);
        let images = bridge.images();
        let mut grant = |content| {
            let mut handle = NativeUiImageHandle::default();
            bridge
                .open_image(&NativeUiImageRequest { content }, &mut handle)
                .map(|()| handle)
                .map_err(|error| error.code().to_owned())
        };
        let portrait = grant(open("portrait.png")).unwrap();
        assert_eq!(images.png(portrait.value).as_deref(), Some(&*png));
        assert_eq!(
            grant(open("notes.txt")).unwrap_err(),
            "CSHARP_UI_IMAGE_NOT_PNG"
        );
        assert_eq!(
            grant(NativeContentReferenceHandle { value: 999 }).unwrap_err(),
            "CSHARP_UI_IMAGE_CONTENT"
        );
        bridge.destroy_image(portrait).unwrap();
        assert!(images.png(portrait.value).is_none());
        assert!(bridge.destroy_image(portrait).is_err());
    }
    use runtime_lifecycle::{RuntimeControlRevision, RuntimeGeneration, RuntimeInstanceId};

    fn binding(control_revision: u64) -> RuntimeUiRuntimeBinding {
        RuntimeUiRuntimeBinding::new(
            RuntimeInstanceId::new(7),
            RuntimeGeneration::new(11),
            RuntimeControlRevision::new(control_revision),
        )
    }

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
        bridge.begin_call(binding(13));
        let api = api(&mut bridge);
        let mut receipt: NativeOperationErrorReceipt = unsafe { std::mem::zeroed() };

        let status =
            unsafe { (api.publish_projection)(api.context, std::ptr::null(), &mut receipt) };

        assert_eq!(status, 0);
        assert_eq!(receipt.diagnostics_len, 1);
        let diagnostic = unsafe { *receipt.diagnostics };
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
    }

    #[test]
    fn publication_carries_the_call_binding_and_close_is_final() {
        let mut bridge = RuntimeUiBridge::new();
        let api = api(&mut bridge);
        let mut stream = NativeUiStreamHandle::default();

        bridge.begin_call(binding(13));
        assert_eq!(
            unsafe {
                (api.open_stream)(
                    api.context,
                    &stream_request(),
                    &mut stream,
                    std::ptr::null_mut(),
                )
            },
            ABI_OK
        );
        bridge.finish_call();

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
        let mut projection = NativeUiProjection {
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
        bridge.begin_call(binding(13));
        let mut receipt: NativeOperationErrorReceipt = unsafe { std::mem::zeroed() };
        assert_eq!(
            unsafe { (api.publish_projection)(api.context, &projection, &mut receipt) },
            ABI_OK,
            "typed structured projection is accepted before close"
        );
        let published = bridge.finish_call();
        assert_eq!(published.len(), 1);
        assert_eq!(published[0].runtime(), binding(13));

        projection.sequence = 2;
        bridge.begin_call(binding(14));
        assert_eq!(
            unsafe { (api.publish_projection)(api.context, &projection, &mut receipt) },
            ABI_OK,
            "the next call accepts an advancing stream sequence"
        );
        let rebound = bridge.finish_call();
        assert_eq!(rebound.len(), 1);
        assert_eq!(rebound[0].runtime(), binding(14));

        projection.sequence = 3;
        bridge.begin_call(binding(14));
        assert_eq!(
            unsafe { (api.publish_projection)(api.context, &projection, &mut receipt) },
            ABI_OK
        );
        assert!(
            bridge.finish_call().is_empty(),
            "an unchanged value under the same binding is not republished"
        );
        bridge.begin_call(binding(14));
        assert_eq!(
            unsafe { (api.publish_projection)(api.context, &projection, &mut receipt) },
            0,
            "the skipped publish still advanced the stream sequence"
        );
        bridge.finish_call();
        let changed = [NativeStructuredValueNode {
            kind: NativeStructuredValueKind::Bool,
            bool_value: 1,
            ..nodes[0]
        }];
        projection.sequence = 4;
        projection.value.nodes = changed.as_ptr();
        bridge.begin_call(binding(14));
        assert_eq!(
            unsafe { (api.publish_projection)(api.context, &projection, &mut receipt) },
            ABI_OK
        );
        let published = bridge.finish_call();
        assert_eq!(published.len(), 1, "a changed value is published");
        assert_eq!(published[0].sequence(), 4);
        projection.value.nodes = nodes.as_ptr();

        bridge.begin_call(binding(14));
        assert_eq!(
            unsafe { (api.destroy_stream)(api.context, stream, std::ptr::null_mut()) },
            ABI_OK
        );
        assert_eq!(
            unsafe { (api.destroy_stream)(api.context, stream, std::ptr::null_mut()) },
            0,
            "duplicate close is rejected"
        );
        bridge.finish_call();

        bridge.begin_call(binding(14));
        let mut stale_receipt: NativeOperationErrorReceipt = unsafe { std::mem::zeroed() };
        assert_eq!(
            unsafe { (api.publish_projection)(api.context, &projection, &mut stale_receipt) },
            0,
            "publish after close is rejected"
        );
        assert!(bridge.finish_call().is_empty());
    }

    #[test]
    fn snapshot_retags_the_latest_projection() {
        let mut bridge = RuntimeUiBridge::new();
        let api = api(&mut bridge);
        let mut stream = NativeUiStreamHandle::default();
        bridge.begin_call(binding(13));
        assert_eq!(
            unsafe {
                (api.open_stream)(
                    api.context,
                    &stream_request(),
                    &mut stream,
                    std::ptr::null_mut(),
                )
            },
            ABI_OK
        );
        bridge.finish_call();

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
        let mut projection = NativeUiProjection {
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
        let mut receipt: NativeOperationErrorReceipt = unsafe { std::mem::zeroed() };
        bridge.begin_call(binding(13));
        assert_eq!(
            unsafe { (api.publish_projection)(api.context, &projection, &mut receipt) },
            ABI_OK
        );
        bridge.finish_call();

        projection.sequence = 2;
        bridge.begin_call(binding(14));
        assert_eq!(
            unsafe { (api.publish_projection)(api.context, &projection, &mut receipt) },
            ABI_OK
        );

        bridge.finish_call();

        let snapshot = bridge.snapshot_projections(binding(99));
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0].runtime(), binding(99));
        assert_eq!(snapshot[0].sequence(), 2);
    }
}
