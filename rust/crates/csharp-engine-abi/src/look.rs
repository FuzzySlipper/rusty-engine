use crate::*;
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeLookState {
    pub yaw_radians: f32,
    pub pitch_radians: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeLookConfig {
    pub horizontal_radians_per_unit: f32,
    pub vertical_radians_per_unit: f32,
    pub minimum_pitch_radians: f32,
    pub maximum_pitch_radians: f32,
    pub maximum_delta_radians: f32,
    pub invert_horizontal: bool,
    pub invert_vertical: bool,
    pub wrap_yaw: bool,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeLookRequest {
    pub state: NativeLookState,
    pub delta: NativeVec2,
    pub config: NativeLookConfig,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeLookResetRequest {
    pub state: NativeLookState,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeLookRebaseRequest {
    pub state: NativeLookState,
    pub target: NativeLookState,
    pub config: NativeLookConfig,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeLookDiagnostic {
    Accepted = 0,
    InvalidConfig = 1,
    InvalidState = 2,
    InvalidCommand = 3,
    DeltaLimitExceeded = 4,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeLookReceipt {
    pub before: NativeLookState,
    pub after: NativeLookState,
    pub orientation: NativeQuat,
    pub forward: NativeVec3,
    pub right: NativeVec3,
    pub up: NativeVec3,
}
