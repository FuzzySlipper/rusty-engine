use core_space::Face;
use entity_state::EntityTransform;

use crate::{VoxelCollisionScene, VoxelEdit};

/// Renderer or UI proposal. Every field is untrusted until the scene re-casts
/// the ray against its current authoritative collision projection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VoxelPickHint {
    pub origin: [f64; 3],
    pub direction: [f64; 3],
    pub max_distance: f64,
    pub claimed_voxel: [i64; 3],
    pub claimed_face: Face,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VoxelPickAnchor {
    pub hit_voxel: [i64; 3],
    pub hit_face: Face,
    pub place_voxel: [i64; 3],
    pub point: [f64; 3],
    pub distance: f64,
}

impl VoxelPickAnchor {
    pub const fn place_edit(self, material_slot: u16) -> VoxelEdit {
        VoxelEdit::Set {
            address: self.place_voxel,
            material_slot,
        }
    }

    pub const fn remove_edit(self) -> VoxelEdit {
        VoxelEdit::Clear {
            address: self.hit_voxel,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InstanceVoxelPickAnchor {
    pub local: VoxelPickAnchor,
    pub world_point: [f64; 3],
    pub world_distance: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VoxelPickError {
    InvalidRay,
    InvalidTransform,
    NoHit,
    HintMismatch {
        authoritative_voxel: [i64; 3],
        authoritative_face: Face,
        claimed_voxel: [i64; 3],
        claimed_face: Face,
    },
    PlacementOverflow,
}

impl std::fmt::Display for VoxelPickError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for VoxelPickError {}

#[derive(Debug, Default, Clone, Copy)]
pub struct VoxelPickService;

impl VoxelPickService {
    pub fn validate(
        scene: &VoxelCollisionScene,
        hint: VoxelPickHint,
    ) -> Result<VoxelPickAnchor, VoxelPickError> {
        validate_ray(hint.direction, hint.max_distance)?;
        let hit = scene
            .raycast(hint.origin, hint.direction, hint.max_distance)
            .ok_or(VoxelPickError::NoHit)?;
        if hit.voxel != hint.claimed_voxel || hit.face != hint.claimed_face {
            return Err(VoxelPickError::HintMismatch {
                authoritative_voxel: hit.voxel,
                authoritative_face: hit.face,
                claimed_voxel: hint.claimed_voxel,
                claimed_face: hint.claimed_face,
            });
        }
        let offset = hit.face.offset().map(i64::from);
        let place_voxel = [0, 1, 2].map(|axis| {
            hit.voxel[axis]
                .checked_add(offset[axis])
                .ok_or(VoxelPickError::PlacementOverflow)
        });
        Ok(VoxelPickAnchor {
            hit_voxel: hit.voxel,
            hit_face: hit.face,
            place_voxel: [place_voxel[0]?, place_voxel[1]?, place_voxel[2]?],
            point: hit.point,
            distance: hit.distance,
        })
    }

    /// Revalidate a world-space hint against an asset-local scene using an
    /// ordinary entity transform. Non-uniform scale is accounted for when
    /// converting the world-distance bound.
    pub fn validate_instance(
        scene: &VoxelCollisionScene,
        transform: EntityTransform,
        world_hint: VoxelPickHint,
    ) -> Result<InstanceVoxelPickAnchor, VoxelPickError> {
        validate_transform(transform)?;
        validate_ray(world_hint.direction, world_hint.max_distance)?;
        let local = inverse_transform_ray(transform, world_hint)?;
        let anchor = Self::validate(scene, local.hint)?;
        let world_point = transform_point(transform, anchor.point);
        Ok(InstanceVoxelPickAnchor {
            local: anchor,
            world_point,
            world_distance: anchor.distance / local.distance_scale,
        })
    }
}

#[derive(Debug, Clone, Copy)]
struct LocalPick {
    hint: VoxelPickHint,
    distance_scale: f64,
}

fn inverse_transform_ray(
    transform: EntityTransform,
    hint: VoxelPickHint,
) -> Result<LocalPick, VoxelPickError> {
    let world_length = vector_length(hint.direction);
    let world_direction = hint.direction.map(|component| component / world_length);
    let translated_origin = [
        hint.origin[0] - f64::from(transform.translation.x),
        hint.origin[1] - f64::from(transform.translation.y),
        hint.origin[2] - f64::from(transform.translation.z),
    ];
    let local_origin_rotated = inverse_rotate(transform, translated_origin);
    let local_direction_rotated = inverse_rotate(transform, world_direction);
    let scale = [
        f64::from(transform.scale.x),
        f64::from(transform.scale.y),
        f64::from(transform.scale.z),
    ];
    let local_origin = [0, 1, 2].map(|axis| local_origin_rotated[axis] / scale[axis]);
    let local_direction = [0, 1, 2].map(|axis| local_direction_rotated[axis] / scale[axis]);
    let distance_scale = vector_length(local_direction);
    if !distance_scale.is_finite() || distance_scale <= 0.0 {
        return Err(VoxelPickError::InvalidRay);
    }
    Ok(LocalPick {
        hint: VoxelPickHint {
            origin: local_origin,
            direction: local_direction,
            max_distance: hint.max_distance * distance_scale,
            claimed_voxel: hint.claimed_voxel,
            claimed_face: hint.claimed_face,
        },
        distance_scale,
    })
}

fn validate_ray(direction: [f64; 3], max_distance: f64) -> Result<(), VoxelPickError> {
    let length = vector_length(direction);
    if direction.iter().any(|value| !value.is_finite())
        || !length.is_finite()
        || length <= 0.0
        || !max_distance.is_finite()
        || max_distance <= 0.0
    {
        Err(VoxelPickError::InvalidRay)
    } else {
        Ok(())
    }
}

fn validate_transform(transform: EntityTransform) -> Result<(), VoxelPickError> {
    let values = [
        transform.translation.x,
        transform.translation.y,
        transform.translation.z,
        transform.scale.x,
        transform.scale.y,
        transform.scale.z,
        transform.rotation.x,
        transform.rotation.y,
        transform.rotation.z,
        transform.rotation.w,
    ];
    let norm = f64::from(transform.rotation.x).powi(2)
        + f64::from(transform.rotation.y).powi(2)
        + f64::from(transform.rotation.z).powi(2)
        + f64::from(transform.rotation.w).powi(2);
    if values.iter().any(|value| !value.is_finite())
        || transform.scale.x <= 0.0
        || transform.scale.y <= 0.0
        || transform.scale.z <= 0.0
        || !norm.is_finite()
        || (norm - 1.0).abs() > 0.002
    {
        Err(VoxelPickError::InvalidTransform)
    } else {
        Ok(())
    }
}

fn inverse_rotate(transform: EntityTransform, vector: [f64; 3]) -> [f64; 3] {
    let q = normalized_quaternion(transform);
    rotate_vector([-q[0], -q[1], -q[2], q[3]], vector)
}

fn transform_point(transform: EntityTransform, point: [f64; 3]) -> [f64; 3] {
    let scaled = [
        point[0] * f64::from(transform.scale.x),
        point[1] * f64::from(transform.scale.y),
        point[2] * f64::from(transform.scale.z),
    ];
    let rotated = rotate_vector(normalized_quaternion(transform), scaled);
    [
        rotated[0] + f64::from(transform.translation.x),
        rotated[1] + f64::from(transform.translation.y),
        rotated[2] + f64::from(transform.translation.z),
    ]
}

fn normalized_quaternion(transform: EntityTransform) -> [f64; 4] {
    let q = [
        f64::from(transform.rotation.x),
        f64::from(transform.rotation.y),
        f64::from(transform.rotation.z),
        f64::from(transform.rotation.w),
    ];
    let inverse_norm = 1.0 / (q.iter().map(|value| value * value).sum::<f64>()).sqrt();
    q.map(|value| value * inverse_norm)
}

fn rotate_vector(q: [f64; 4], vector: [f64; 3]) -> [f64; 3] {
    let axis = [q[0], q[1], q[2]];
    let uv = cross(axis, vector);
    let uuv = cross(axis, uv);
    [0, 1, 2].map(|index| vector[index] + 2.0 * (q[3] * uv[index] + uuv[index]))
}

fn cross(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    ]
}

fn vector_length(vector: [f64; 3]) -> f64 {
    vector.iter().map(|value| value * value).sum::<f64>().sqrt()
}
