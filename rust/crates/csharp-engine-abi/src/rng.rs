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

/// One explicit-state compatibility draw from the common 32-bit LCG15
/// sequence. This is intentionally a reproducibility primitive rather than a
/// general random-number API: the state is supplied and retained by the
/// caller, and the 15-bit sample is reduced with modulo arithmetic.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeLcg15Request {
    pub state: u32,
    pub upper_exclusive: u32,
}

/// The advanced explicit state and its bounded compatibility sample.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeLcg15Receipt {
    pub state: u32,
    pub value: u32,
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
