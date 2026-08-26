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
    pub invert_horizontal: u32,
    pub invert_vertical: u32,
    pub wrap_yaw: u32,
    pub reserved: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeLookRequest {
    pub state: NativeLookState,
    pub delta: NativeVec2,
    pub config: NativeLookConfig,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeLookReceipt {
    pub state: NativeLookState,
    pub orientation: NativeQuat,
    pub forward: NativeVec3,
    pub right: NativeVec3,
    pub up: NativeVec3,
}
