//! The sole Rust-owned C ABI definition for trusted NativeAOT products.
//!
//! C# declarations are generated mechanically from these layouts. This module
//! intentionally contains no compatibility, permission, registry, or wire-format
//! layer: the product calls named Engine functions directly through the table.

use std::ffi::c_void;

/// One file borrowed by trusted product code for the duration of creation.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeContentFile {
    pub path: *const u8,
    pub path_len: usize,
    pub bytes: *const u8,
    pub bytes_len: usize,
}

/// One input event borrowed for the duration of a product turn.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeInputEvent {
    pub kind: u32,
    pub edge: u32,
    pub sequence: u64,
    pub x: f32,
    pub y: f32,
    pub label: *const u8,
    pub label_len: usize,
}

/// Explicit turn timing and its borrowed input slice.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeTurnArgs {
    /// 1 realtime (nanoseconds), 2 demand (step), 3 external (step).
    pub kind: u32,
    pub reserved: u32,
    pub observed_time_or_step: u64,
    pub events: *const NativeInputEvent,
    pub event_count: usize,
}

/// One product-selected renderer-neutral visual fact.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeVisualFact {
    pub object_id: u64,
    pub appearance: *const u8,
    pub appearance_len: usize,
    pub translation: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
    pub visible: u32,
}

pub type NativePublishVisualSnapshot =
    unsafe extern "C" fn(*mut c_void, *const NativeVisualFact, usize) -> i32;

/// Direct Engine functions available to trusted NativeAOT product code.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeEngineApi {
    pub context: *mut c_void,
    pub publish_visual_snapshot: NativePublishVisualSnapshot,
}

/// Borrowed creation inputs plus the direct Engine API.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeProductCreateArgs {
    pub content: *const NativeContentFile,
    pub content_len: usize,
    pub engine: NativeEngineApi,
}

pub type NativeProductCreate =
    unsafe extern "C" fn(*const NativeProductCreateArgs, *mut *mut c_void) -> i32;
pub type NativeProductAction = unsafe extern "C" fn(*mut c_void) -> i32;
pub type NativeProductTurn = unsafe extern "C" fn(*mut c_void, *const NativeTurnArgs) -> i32;
pub type NativeProductDestroy = unsafe extern "C" fn(*mut c_void);

/// Product functions supplied to Rust by the one NativeAOT bootstrap export.
/// Nullable fields let Rust receive and inspect an initially empty table safely.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeProductApi {
    pub create:
        Option<unsafe extern "C" fn(*const NativeProductCreateArgs, *mut *mut c_void) -> i32>,
    pub start: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
    pub turn: Option<unsafe extern "C" fn(*mut c_void, *const NativeTurnArgs) -> i32>,
    pub pause: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
    pub resume: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
    pub shutdown: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
    pub destroy: Option<unsafe extern "C" fn(*mut c_void)>,
}

pub type NativeProductBind = unsafe extern "C" fn(*mut NativeProductApi) -> i32;
