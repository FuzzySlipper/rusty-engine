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
}

#[derive(Debug, Clone)]
struct RuntimeUiStream {
    stream: String,
    contract: String,
    last_sequence: Option<u64>,
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
) -> i32 {
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
            bridge.callback_error = Some(error);
            0
        }
    }
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
        0 => Value::Null,
        1 => Value::Bool(node.bool_value != 0),
        2 => Value::Number(Number::from_f64(node.number_value).ok_or_else(|| {
            CsharpEngineServicesError::new("CSHARP_UI_NUMBER", "C# UI number was not finite")
        })?),
        3 => Value::String(arena_text(bytes, node.text_offset, node.text_len, "text")?.to_owned()),
        4 => {
            let children = arena_children(node, edges)?;
            let mut values = Vec::with_capacity(children.len());
            for child in children {
                values.push(decode_structured_node(
                    child, nodes, edges, bytes, visiting,
                )?);
            }
            Value::Array(values)
        }
        5 => {
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
        _ => {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_UI_KIND",
                "C# UI node had an unknown kind",
            ))
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
        publish_projection: publish_ui_projection,
    }
}
