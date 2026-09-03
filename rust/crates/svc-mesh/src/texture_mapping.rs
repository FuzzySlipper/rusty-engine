//! Coordinate conventions for runtime voxel surface textures.
//!
//! The greedy mesher owns rectangle geometry. This module only projects its
//! absolute integer grid points into deterministic two-dimensional tile space;
//! it does not select materials, decode images, or realize renderer resources.

use core_space::{Axis, Direction6};

/// Largest signed integer tile coordinate that remains exactly representable
/// by the `f32` vertex stream selected for voxel surface mapping.
pub const MAX_EXACT_TILE_COORDINATE: i64 = 1_i64 << 24;

/// Smallest supported authored tile period, in voxel-cell units.
pub const MIN_TILE_SCALE_CELLS: f64 = 1.0 / 256.0;

/// Largest supported authored tile period, in voxel-cell units.
pub const MAX_TILE_SCALE_CELLS: f64 = 4096.0;

/// Maximum ratio between a repeated coordinate magnitude and the smaller of
/// one cell or its authored tile period. At this bound an f32 varying retains
/// at least two representable intervals across that unit, including the
/// shortest supported 1/256-cell period.
pub const MAX_TILE_PHASE_RATIO: f64 = (1_u64 << 22) as f64;

/// One signed world/object axis used by the outward-facing texture basis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SignedTextureAxis {
    axis: Axis,
    sign: i8,
}

impl SignedTextureAxis {
    const fn positive(axis: Axis) -> Self {
        Self { axis, sign: 1 }
    }

    const fn negative(axis: Axis) -> Self {
        Self { axis, sign: -1 }
    }

    pub const fn axis(self) -> Axis {
        self.axis
    }

    pub const fn sign(self) -> i8 {
        self.sign
    }

    fn project(self, point: [i64; 3]) -> Result<i64, VoxelTextureMappingError> {
        let value = point[self.axis.index()];
        if self.sign < 0 {
            value
                .checked_neg()
                .ok_or(VoxelTextureMappingError::CoordinateOverflow)
        } else {
            Ok(value)
        }
    }
}

/// Deterministic face-local U/V basis. In every case `U × V` equals the
/// outward face normal, so opposite faces do not acquire an implicit mirror.
/// Vertical faces additionally keep world height on decreasing texture V so
/// authored images remain upright around all four cardinal directions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoxelSurfaceTextureBasis {
    pub u: SignedTextureAxis,
    pub v: SignedTextureAxis,
}

/// Return the stable outward-facing tile-space basis for one voxel face.
pub const fn voxel_surface_texture_basis(direction: Direction6) -> VoxelSurfaceTextureBasis {
    use Axis::{X, Y, Z};
    use Direction6::{NegX, NegY, NegZ, PosX, PosY, PosZ};

    match direction {
        PosX => VoxelSurfaceTextureBasis {
            u: SignedTextureAxis::positive(Z),
            v: SignedTextureAxis::negative(Y),
        },
        NegX => VoxelSurfaceTextureBasis {
            u: SignedTextureAxis::negative(Z),
            v: SignedTextureAxis::negative(Y),
        },
        PosY => VoxelSurfaceTextureBasis {
            u: SignedTextureAxis::positive(Z),
            v: SignedTextureAxis::positive(X),
        },
        NegY => VoxelSurfaceTextureBasis {
            u: SignedTextureAxis::positive(Z),
            v: SignedTextureAxis::negative(X),
        },
        PosZ => VoxelSurfaceTextureBasis {
            u: SignedTextureAxis::negative(X),
            v: SignedTextureAxis::negative(Y),
        },
        NegZ => VoxelSurfaceTextureBasis {
            u: SignedTextureAxis::positive(X),
            v: SignedTextureAxis::negative(Y),
        },
    }
}

/// Project one mesher grid point into exact tile space.
///
/// `coordinate_origin` is the absolute voxel coordinate of a chunk-local
/// mesher origin for `engine-spatial`, and `[0, 0, 0]` for voxel objects. The
/// result therefore remains continuous across independently rebuilt chunks
/// while moving with an object's local transform.
pub fn project_voxel_surface_tile_point(
    direction: Direction6,
    grid_point: [i64; 3],
    coordinate_origin: [i64; 3],
) -> Result<[f32; 2], VoxelTextureMappingError> {
    let mut absolute = [0_i64; 3];
    for axis in 0..3 {
        absolute[axis] = grid_point[axis]
            .checked_add(coordinate_origin[axis])
            .ok_or(VoxelTextureMappingError::CoordinateOverflow)?;
    }
    let basis = voxel_surface_texture_basis(direction);
    let projected = [basis.u.project(absolute)?, basis.v.project(absolute)?];
    for coordinate in projected {
        if !(-MAX_EXACT_TILE_COORDINATE..=MAX_EXACT_TILE_COORDINATE).contains(&coordinate) {
            return Err(VoxelTextureMappingError::CoordinateOutOfExactRange {
                coordinate,
                limit: MAX_EXACT_TILE_COORDINATE,
            });
        }
    }
    Ok([projected[0] as f32, projected[1] as f32])
}

/// Project the four already-wound grid corners of one greedy rectangle.
pub fn project_voxel_surface_tile_corners(
    direction: Direction6,
    grid_corners: [[i64; 3]; 4],
    coordinate_origin: [i64; 3],
) -> Result<[[f32; 2]; 4], VoxelTextureMappingError> {
    let mut output = [[0.0_f32; 2]; 4];
    for (target, point) in output.iter_mut().zip(grid_corners) {
        *target = project_voxel_surface_tile_point(direction, point, coordinate_origin)?;
    }
    Ok(output)
}

/// Compute the selected shader's repeat coordinate before atlas remapping.
/// `rem_euclid` gives negative coordinates the same stable phase as positive
/// coordinates and maps an exact tile boundary back to zero.
pub fn repeat_voxel_tile_coordinate(
    tile_coordinate: [f64; 2],
    tile_scale_cells: [f64; 2],
    tile_origin_cells: [f64; 2],
) -> Result<[f64; 2], VoxelTextureMappingError> {
    let mut output = [0.0_f64; 2];
    for axis in 0..2 {
        let coordinate = tile_coordinate[axis];
        let scale = tile_scale_cells[axis];
        let origin = tile_origin_cells[axis];
        if !coordinate.is_finite() || !origin.is_finite() {
            return Err(VoxelTextureMappingError::NonFiniteMapping);
        }
        if coordinate.abs() > MAX_EXACT_TILE_COORDINATE as f64
            || origin.abs() > MAX_EXACT_TILE_COORDINATE as f64
        {
            return Err(VoxelTextureMappingError::MappingCoordinateOutOfRange);
        }
        if !scale.is_finite() || !(MIN_TILE_SCALE_CELLS..=MAX_TILE_SCALE_CELLS).contains(&scale) {
            return Err(VoxelTextureMappingError::InvalidTileScale);
        }
        let precision_unit = scale.min(1.0);
        let precision_bound = precision_unit * MAX_TILE_PHASE_RATIO;
        if coordinate.abs() > precision_bound || origin.abs() > precision_bound {
            return Err(VoxelTextureMappingError::InsufficientTileCoordinatePrecision);
        }
        let delta = coordinate - origin;
        output[axis] = delta.rem_euclid(scale) / scale;
    }
    Ok(output)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoxelTextureMappingError {
    CoordinateOverflow,
    CoordinateOutOfExactRange { coordinate: i64, limit: i64 },
    NonFiniteMapping,
    MappingCoordinateOutOfRange,
    InvalidTileScale,
    InsufficientTileCoordinatePrecision,
}

impl core::fmt::Display for VoxelTextureMappingError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::CoordinateOverflow => write!(formatter, "voxel texture coordinate overflow"),
            Self::CoordinateOutOfExactRange { coordinate, limit } => write!(
                formatter,
                "voxel texture coordinate {coordinate} exceeds exact f32 limit {limit}",
            ),
            Self::NonFiniteMapping => write!(formatter, "voxel texture mapping must be finite"),
            Self::MappingCoordinateOutOfRange => write!(
                formatter,
                "voxel texture mapping coordinate exceeds exact f32 limit {MAX_EXACT_TILE_COORDINATE}",
            ),
            Self::InvalidTileScale => write!(
                formatter,
                "voxel texture tile scale must be from {MIN_TILE_SCALE_CELLS} through {MAX_TILE_SCALE_CELLS} cells",
            ),
            Self::InsufficientTileCoordinatePrecision => write!(
                formatter,
                "voxel texture coordinate and scale exceed the f32 repeat-phase precision bound",
            ),
        }
    }
}

impl std::error::Error for VoxelTextureMappingError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_face_bases_are_outward_and_not_mirrored() {
        for direction in Direction6::ALL {
            let basis = voxel_surface_texture_basis(direction);
            let u = signed_vector(basis.u);
            let v = signed_vector(basis.v);
            assert_eq!(cross(u, v), direction.offset(), "{direction:?}");
        }
    }

    #[test]
    fn vertical_faces_keep_world_height_on_texture_v() {
        let lower = [3, 7, 11];
        let upper = [3, 12, 11];
        for direction in [
            Direction6::PosX,
            Direction6::NegX,
            Direction6::PosZ,
            Direction6::NegZ,
        ] {
            let lower_uv = project_voxel_surface_tile_point(direction, lower, [0, 0, 0]).unwrap();
            let upper_uv = project_voxel_surface_tile_point(direction, upper, [0, 0, 0]).unwrap();
            assert_eq!(lower_uv[1], -7.0, "{direction:?}");
            assert_eq!(upper_uv[1], -12.0, "{direction:?}");
            assert_eq!(upper_uv[0], lower_uv[0], "{direction:?}");
        }
    }

    #[test]
    fn large_vertical_greedy_faces_keep_upright_height_span() {
        let faces = [
            (
                Direction6::PosX,
                [[4, 2, 3], [4, 2, 10], [4, 8, 10], [4, 8, 3]],
            ),
            (
                Direction6::NegX,
                [[4, 2, 10], [4, 2, 3], [4, 8, 3], [4, 8, 10]],
            ),
            (
                Direction6::PosZ,
                [[9, 2, 3], [2, 2, 3], [2, 8, 3], [9, 8, 3]],
            ),
            (
                Direction6::NegZ,
                [[2, 2, 3], [9, 2, 3], [9, 8, 3], [2, 8, 3]],
            ),
        ];

        for (direction, corners) in faces {
            let uv = project_voxel_surface_tile_corners(direction, corners, [13, 17, 19]).unwrap();
            assert_eq!(uv[0][1], -19.0, "{direction:?}");
            assert_eq!(uv[1][1], -19.0, "{direction:?}");
            assert_eq!(uv[2][1], -25.0, "{direction:?}");
            assert_eq!(uv[3][1], -25.0, "{direction:?}");
        }
    }

    #[test]
    fn negative_coordinates_repeat_with_euclidean_phase() {
        assert_eq!(
            repeat_voxel_tile_coordinate([-0.25, -2.0], [1.0, 3.0], [0.0, 0.0]),
            Ok([0.75, 1.0 / 3.0]),
        );
        assert_eq!(
            repeat_voxel_tile_coordinate([4.0, 8.0], [2.0, 4.0], [0.0, 0.0]),
            Ok([0.0, 0.0]),
        );
    }

    #[test]
    fn coordinate_projection_is_exact_at_limit_and_rejects_one_over() {
        assert_eq!(
            project_voxel_surface_tile_point(
                Direction6::NegZ,
                [MAX_EXACT_TILE_COORDINATE, 0, 0],
                [0, 0, 0],
            ),
            Ok([MAX_EXACT_TILE_COORDINATE as f32, 0.0]),
        );
        assert!(matches!(
            project_voxel_surface_tile_point(
                Direction6::PosZ,
                [MAX_EXACT_TILE_COORDINATE + 1, 0, 0],
                [0, 0, 0],
            ),
            Err(VoxelTextureMappingError::CoordinateOutOfExactRange { .. })
        ));
        assert_eq!(
            repeat_voxel_tile_coordinate([0.0, 0.0], [MIN_TILE_SCALE_CELLS / 2.0, 1.0], [0.0, 0.0],),
            Err(VoxelTextureMappingError::InvalidTileScale),
        );
        assert_eq!(
            repeat_voxel_tile_coordinate(
                [0.0, 0.0],
                [1.0, 1.0],
                [MAX_EXACT_TILE_COORDINATE as f64 + 1.0, 0.0],
            ),
            Err(VoxelTextureMappingError::MappingCoordinateOutOfRange),
        );
    }

    #[test]
    fn shader_equivalent_repeat_rejects_quantized_phase_at_the_coordinate_limit() {
        let minimum = MIN_TILE_SCALE_CELLS;
        assert_eq!(
            repeat_voxel_tile_coordinate(
                [MAX_EXACT_TILE_COORDINATE as f64, 0.0],
                [minimum, minimum],
                [0.0, 0.0],
            ),
            Err(VoxelTextureMappingError::InsufficientTileCoordinatePrecision),
        );
        assert_eq!(
            repeat_voxel_tile_coordinate(
                [MAX_EXACT_TILE_COORDINATE as f64, 0.0],
                [1.0, 1.0],
                [MAX_EXACT_TILE_COORDINATE as f64, 0.0],
            ),
            Err(VoxelTextureMappingError::InsufficientTileCoordinatePrecision),
        );
        assert_eq!(
            repeat_voxel_tile_coordinate(
                [-(MAX_EXACT_TILE_COORDINATE as f64), 0.0],
                [1.0, 1.0],
                [-(MAX_EXACT_TILE_COORDINATE as f64), 0.0],
            ),
            Err(VoxelTextureMappingError::InsufficientTileCoordinatePrecision),
        );

        let limiting_coordinate = minimum * MAX_TILE_PHASE_RATIO;
        let base = limiting_coordinate as f32;
        let half_phase = (limiting_coordinate + minimum / 2.0) as f32;
        let full_phase = (limiting_coordinate + minimum) as f32;
        assert_ne!(base, half_phase);
        assert_ne!(half_phase, full_phase);
        assert_eq!(
            repeat_voxel_tile_coordinate(
                [
                    f64::from((-limiting_coordinate + minimum / 2.0) as f32),
                    0.0
                ],
                [minimum, minimum],
                [0.0, 0.0],
            ),
            Ok([0.5, 0.0]),
        );
        assert_eq!(
            repeat_voxel_tile_coordinate(
                [limiting_coordinate, limiting_coordinate - 16.0],
                [minimum, minimum],
                [0.0, 0.0],
            ),
            Ok([0.0, 0.0]),
        );
        assert_eq!(
            repeat_voxel_tile_coordinate(
                [limiting_coordinate + minimum, 0.0],
                [minimum, minimum],
                [0.0, 0.0],
            ),
            Err(VoxelTextureMappingError::InsufficientTileCoordinatePrecision),
        );

        let integer_limit = limiting_coordinate as i64;
        let left_chunk_seam = project_voxel_surface_tile_point(
            Direction6::PosZ,
            [16, 0, 0],
            [integer_limit - 16, 0, 0],
        )
        .unwrap();
        let right_chunk_seam =
            project_voxel_surface_tile_point(Direction6::PosZ, [0, 0, 0], [integer_limit, 0, 0])
                .unwrap();
        assert_eq!(left_chunk_seam, right_chunk_seam);
        assert_eq!(
            repeat_voxel_tile_coordinate(
                left_chunk_seam.map(f64::from),
                [minimum, minimum],
                [0.0, 0.0],
            ),
            repeat_voxel_tile_coordinate(
                right_chunk_seam.map(f64::from),
                [minimum, minimum],
                [0.0, 0.0],
            ),
        );
    }

    fn signed_vector(axis: SignedTextureAxis) -> [i32; 3] {
        let mut output = [0_i32; 3];
        output[axis.axis().index()] = i32::from(axis.sign());
        output
    }

    fn cross(left: [i32; 3], right: [i32; 3]) -> [i32; 3] {
        [
            left[1] * right[2] - left[2] * right[1],
            left[2] * right[0] - left[0] * right[2],
            left[0] * right[1] - left[1] * right[0],
        ]
    }
}
