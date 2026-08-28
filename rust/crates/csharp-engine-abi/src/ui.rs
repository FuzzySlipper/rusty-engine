#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeUiStreamHandle {
    pub value: u64,
}

/// One borrowed UTF-8 identity. It is valid only for the immediate direct
/// service call that accepts it.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeUtf8Slice {
    pub bytes: *const u8,
    pub len: usize,
}

/// One file borrowed by trusted product code for the duration of creation.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeContentFile {
    pub path: *const u8,
    pub path_len: usize,
    pub bytes: *const u8,
    pub bytes_len: usize,
}

/// Tags for one borrowed node in a fixed-layout UI value arena.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeStructuredValueKind {
    Null = 0,
    Bool = 1,
    Number = 2,
    String = 3,
    Array = 4,
    Object = 5,
}

/// A node's object key/text ranges refer to `NativeStructuredValue::utf8`.
/// Array/object child ranges refer to its separate edge array, so nested
/// values never depend on incidental node layout. This is presentation data,
/// never an invocation or semantic-program representation.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeStructuredValueNode {
    pub kind: NativeStructuredValueKind,
    pub bool_value: u32,
    pub number_value: f64,
    pub key_offset: u32,
    pub key_len: u32,
    pub text_offset: u32,
    pub text_len: u32,
    pub first_edge: u32,
    pub child_count: u32,
}

/// Borrowed structured UI value storage. Rust copies it to `serde_json::Value`
/// before an envelope is staged; neither side retains these pointers.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeStructuredValue {
    pub nodes: *const NativeStructuredValueNode,
    pub node_count: usize,
    pub edges: *const u32,
    pub edge_count: usize,
    pub root: u32,
    pub utf8: *const u8,
    pub utf8_len: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeUiProjection {
    pub stream: NativeUiStreamHandle,
    pub sequence: u64,
    pub value: NativeStructuredValue,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeUiStreamRequest {
    pub stream: NativeUtf8Slice,
    pub contract: NativeUtf8Slice,
}
