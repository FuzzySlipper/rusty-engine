//! Bounded semantic voxel-edit generation for authoring tools.
//!
//! Primitive requests remain proposals. This service validates coordinates,
//! materials, radii, and expansion size before returning the same [`VoxelEdit`]
//! values consumed by [`crate::VoxelEditService`] and durable edit history.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{
    validate_voxel_address, validate_voxel_material_slot, VoxelAuthorityValidationError, VoxelEdit,
    MAX_VOXEL_EDITS_PER_TRANSACTION,
};

/// Donor-compatible line thickness remains deliberately small and reviewable.
pub const MAX_VOXEL_LINE_RADIUS: u32 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VoxelBoxFill {
    Filled,
    Shell,
    Edges,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum VoxelPrimitive {
    Block {
        address: [i64; 3],
    },
    Box {
        start: [i64; 3],
        end: [i64; 3],
        fill: VoxelBoxFill,
    },
    Line {
        start: [i64; 3],
        end: [i64; 3],
        radius: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum VoxelPrimitiveMaterial {
    Set { material_slot: u16 },
    Clear,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VoxelPrimitiveRequest {
    pub primitive: VoxelPrimitive,
    pub material: VoxelPrimitiveMaterial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoxelPrimitiveError {
    InvalidAddress(VoxelAuthorityValidationError),
    InvalidMaterial(VoxelAuthorityValidationError),
    RadiusTooLarge { maximum: u32, actual: u32 },
    TooManyEdits { limit: usize, actual: u128 },
}

impl std::fmt::Display for VoxelPrimitiveError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "voxel primitive rejected: {self:?}")
    }
}

impl std::error::Error for VoxelPrimitiveError {}

#[derive(Debug, Default, Clone, Copy)]
pub struct VoxelPrimitiveEditService;

impl VoxelPrimitiveEditService {
    /// Produce canonical coordinate-ordered edits without touching voxel authority.
    pub fn generate(
        self,
        request: VoxelPrimitiveRequest,
    ) -> Result<Vec<VoxelEdit>, VoxelPrimitiveError> {
        validate_material(request.material)?;
        let addresses = match request.primitive {
            VoxelPrimitive::Block { address } => {
                validate_address(address)?;
                BTreeSet::from([address])
            }
            VoxelPrimitive::Box { start, end, fill } => box_addresses(start, end, fill)?,
            VoxelPrimitive::Line { start, end, radius } => line_addresses(start, end, radius)?,
        };
        Ok(addresses
            .into_iter()
            .map(|address| match request.material {
                VoxelPrimitiveMaterial::Set { material_slot } => VoxelEdit::Set {
                    address,
                    material_slot,
                },
                VoxelPrimitiveMaterial::Clear => VoxelEdit::Clear { address },
            })
            .collect())
    }
}

fn box_addresses(
    start: [i64; 3],
    end: [i64; 3],
    fill: VoxelBoxFill,
) -> Result<BTreeSet<[i64; 3]>, VoxelPrimitiveError> {
    validate_address(start)?;
    validate_address(end)?;
    let minimum = [0, 1, 2].map(|axis| start[axis].min(end[axis]));
    let maximum = [0, 1, 2].map(|axis| start[axis].max(end[axis]));
    let dimensions = [0, 1, 2].map(|axis| {
        u128::try_from(maximum[axis] - minimum[axis] + 1)
            .expect("validated voxel coordinate span is positive")
    });
    let boundary = dimensions.map(|size| size.min(2));
    let interior = [0, 1, 2].map(|axis| dimensions[axis] - boundary[axis]);
    let actual = match fill {
        VoxelBoxFill::Filled => dimensions.into_iter().product(),
        VoxelBoxFill::Shell => {
            dimensions.into_iter().product::<u128>() - interior.into_iter().product::<u128>()
        }
        VoxelBoxFill::Edges => {
            boundary[0] * boundary[1] * interior[2]
                + boundary[0] * interior[1] * boundary[2]
                + interior[0] * boundary[1] * boundary[2]
                + boundary.into_iter().product::<u128>()
        }
    };
    enforce_count(actual)?;

    let mut output = BTreeSet::new();
    for z in minimum[2]..=maximum[2] {
        for y in minimum[1]..=maximum[1] {
            for x in minimum[0]..=maximum[0] {
                let address = [x, y, z];
                let boundary_axes = [0, 1, 2]
                    .into_iter()
                    .filter(|axis| {
                        address[*axis] == minimum[*axis] || address[*axis] == maximum[*axis]
                    })
                    .count();
                let include = match fill {
                    VoxelBoxFill::Filled => true,
                    VoxelBoxFill::Shell => boundary_axes >= 1,
                    VoxelBoxFill::Edges => boundary_axes >= 2,
                };
                if include {
                    output.insert(address);
                }
            }
        }
    }
    debug_assert_eq!(output.len() as u128, actual);
    Ok(output)
}

fn line_addresses(
    start: [i64; 3],
    end: [i64; 3],
    radius: u32,
) -> Result<BTreeSet<[i64; 3]>, VoxelPrimitiveError> {
    validate_address(start)?;
    validate_address(end)?;
    if radius > MAX_VOXEL_LINE_RADIUS {
        return Err(VoxelPrimitiveError::RadiusTooLarge {
            maximum: MAX_VOXEL_LINE_RADIUS,
            actual: radius,
        });
    }
    let delta = [0, 1, 2].map(|axis| end[axis] - start[axis]);
    let steps = delta
        .into_iter()
        .map(i64::unsigned_abs)
        .max()
        .expect("three axes are present");
    enforce_count(u128::from(steps) + 1)?;
    let radius = i64::from(radius);
    let mut output = BTreeSet::new();
    for step in 0..=steps {
        let center = if steps == 0 {
            start
        } else {
            [0, 1, 2].map(|axis| {
                let interpolated =
                    start[axis] as f64 + delta[axis] as f64 * step as f64 / steps as f64;
                interpolated.round() as i64
            })
        };
        for z in -radius..=radius {
            for y in -radius..=radius {
                for x in -radius..=radius {
                    let address = [center[0] + x, center[1] + y, center[2] + z];
                    validate_address(address)?;
                    output.insert(address);
                    if output.len() > MAX_VOXEL_EDITS_PER_TRANSACTION {
                        return Err(VoxelPrimitiveError::TooManyEdits {
                            limit: MAX_VOXEL_EDITS_PER_TRANSACTION,
                            actual: output.len() as u128,
                        });
                    }
                }
            }
        }
    }
    Ok(output)
}

fn validate_material(material: VoxelPrimitiveMaterial) -> Result<(), VoxelPrimitiveError> {
    if let VoxelPrimitiveMaterial::Set { material_slot } = material {
        validate_voxel_material_slot(material_slot)
            .map_err(VoxelPrimitiveError::InvalidMaterial)?;
    }
    Ok(())
}

fn validate_address(address: [i64; 3]) -> Result<(), VoxelPrimitiveError> {
    validate_voxel_address(address).map_err(VoxelPrimitiveError::InvalidAddress)
}

fn enforce_count(actual: u128) -> Result<(), VoxelPrimitiveError> {
    if actual > MAX_VOXEL_EDITS_PER_TRANSACTION as u128 {
        return Err(VoxelPrimitiveError::TooManyEdits {
            limit: MAX_VOXEL_EDITS_PER_TRANSACTION,
            actual,
        });
    }
    Ok(())
}
