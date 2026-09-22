//! Typed Engine-owned full-viewport video presentation.

use crate::{NativeContentReferenceHandle, NativeUtf8Slice};

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVideoPlaybackHandle {
    pub value: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePlayVideoRequest {
    /// Normalized product-content path of an admitted `.webm` resource.
    pub path: NativeUtf8Slice,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePlayVideoFromContentRequest {
    pub content: NativeContentReferenceHandle,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeVideoReadout {
    pub active: bool,
    pub active_handle: NativeVideoPlaybackHandle,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeVideoRealizationFactKind {
    None = 0,
    Completed = 1,
    Skipped = 2,
    Failed = 3,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeVideoFailureCode {
    None = 0,
    DecodeFailed = 1,
    PlaybackBlocked = 2,
    HostFailure = 3,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeVideoRealizationReadout {
    pub retained_fact_count: u32,
    pub evicted_fact_count: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeVideoRealizationFactAtRequest {
    pub index: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeVideoRealizationFactAtReceipt {
    pub present: bool,
    pub kind: NativeVideoRealizationFactKind,
    pub fact_id: u64,
    pub handle: NativeVideoPlaybackHandle,
    pub failure: NativeVideoFailureCode,
}

impl Default for NativeVideoRealizationFactAtReceipt {
    fn default() -> Self {
        Self {
            present: false,
            kind: NativeVideoRealizationFactKind::None,
            fact_id: 0,
            handle: NativeVideoPlaybackHandle::default(),
            failure: NativeVideoFailureCode::None,
        }
    }
}
