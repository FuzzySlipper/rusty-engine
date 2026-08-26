use crate::*;
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeKeyedRngRequest {
    pub seed: u64,
    pub scope: NativeUtf8Slice,
    pub key: NativeUtf8Slice,
    pub minimum: i64,
    pub maximum: i64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeKeyedRngReceipt {
    pub value: i64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeScopedRngCreateRequest {
    pub seed: u64,
    pub scope: NativeUtf8Slice,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeScopedRngForkRequest {
    pub parent: NativeRngHandle,
    pub scope: NativeUtf8Slice,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeScopedRngBoundedRequest {
    pub stream: NativeRngHandle,
    pub upper: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeRngValue {
    pub value: u64,
}
