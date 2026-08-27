#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeVec2 {
    pub x: f32,
    pub y: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeVec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeQuat {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeTransform {
    pub translation: NativeVec3,
    pub rotation: NativeQuat,
    pub scale: NativeVec3,
}

/// Borrowed bytes valid only for the direct service call accepting them.
/// Rust copies these bytes before retaining or committing them.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeByteSlice {
    pub bytes: *const u8,
    pub len: usize,
}

/// A typed owner for immutable bytes retained by one Engine service. The
/// accompanying [`NativeByteLease`] is valid until its exact destroy callback
/// consumes this handle.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeByteLeaseHandle {
    pub value: u64,
}

/// Immutable Engine-owned byte storage. Consumers copy it immediately and
/// release `handle`; neither the pointer nor its bytes are retained in public
/// managed values.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeByteLease {
    pub handle: NativeByteLeaseHandle,
    pub bytes: *const u8,
    pub len: usize,
}

/// Product-owned writable storage borrowed only for the direct service call.
/// Existing persistence consumers use this legacy immediate-copy request; new
/// Engine-owned byte output uses [`NativeByteLease`] instead.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeWritableByteSlice {
    pub bytes: *mut u8,
    pub len: usize,
}
