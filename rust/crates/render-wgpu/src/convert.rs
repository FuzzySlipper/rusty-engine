//! The only seam between Engine value types and glam.
//!
//! Engine descriptors and readouts carry plain arrays: f32 for presentation
//! values and f64 for world-space camera values. glam is this crate's
//! private math (`EXTERNAL_DEPENDENCY_OWNERS`) and never crosses the crate
//! boundary. Every conversion between the two goes through these functions.
//! Packing glam values into GPU buffers is the owning pass's layout, not a
//! crossing of this seam, and glTF decoding reads its file straight into glam.

use glam::{DVec3, Mat4, Quat, Vec3};
use render_model::Transform;

/// A presentation position, direction or scale.
pub(crate) fn vec3(value: [f32; 3]) -> Vec3 {
    Vec3::from_array(value)
}

/// A world-space camera value, narrowed to the renderer's f32.
pub(crate) fn world_vec3(value: [f64; 3]) -> Vec3 {
    DVec3::from_array(value).as_vec3()
}

/// A presentation rotation as `[x, y, z, w]`, normalized.
pub(crate) fn rotation(value: [f32; 4]) -> Quat {
    Quat::from_array(value).normalize()
}

/// A retained node transform as a local matrix.
pub(crate) fn transform_matrix(transform: &Transform) -> Mat4 {
    Mat4::from_scale_rotation_translation(
        vec3(transform.scale),
        rotation(transform.rotation),
        vec3(transform.translation),
    )
}

/// A presentation value written back into an Engine descriptor or fact.
pub(crate) fn array(value: Vec3) -> [f32; 3] {
    value.to_array()
}

/// A world-space value written back into an Engine readout.
pub(crate) fn world_array(value: Vec3) -> [f64; 3] {
    value.as_dvec3().to_array()
}
