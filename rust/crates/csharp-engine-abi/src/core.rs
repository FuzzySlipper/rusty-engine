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
