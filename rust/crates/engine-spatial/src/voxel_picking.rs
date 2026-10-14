use crate::{CollisionRayHit, VoxelCollisionScene};

/// A ray to pick a voxel with, cast against the scene's current collision
/// projection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VoxelPickRay {
    pub origin: [f64; 3],
    pub direction: [f64; 3],
    pub max_distance: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoxelPickError {
    InvalidRay,
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
    /// The first voxel the ray meets within its distance, or `None`.
    pub fn pick(
        scene: &VoxelCollisionScene,
        ray: VoxelPickRay,
    ) -> Result<Option<CollisionRayHit>, VoxelPickError> {
        let length = ray.direction.iter().map(|value| value * value).sum::<f64>();
        if ray.direction.iter().any(|value| !value.is_finite())
            || !length.is_finite()
            || length <= 0.0
            || !ray.max_distance.is_finite()
            || ray.max_distance <= 0.0
        {
            return Err(VoxelPickError::InvalidRay);
        }
        Ok(scene.raycast(ray.origin, ray.direction, ray.max_distance))
    }
}
