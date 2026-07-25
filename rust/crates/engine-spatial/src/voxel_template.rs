//! Deterministic, bounded voxel templates expressed as ordinary edit proposals.
//!
//! Templates do not own assets, persistence, or live voxel authority. They
//! produce the same canonical [`crate::VoxelEdit`] values as direct authoring
//! primitives so a project owner can commit them through normal edit history.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{
    validate_voxel_address, validate_voxel_material_slot, VoxelAuthorityValidationError, VoxelEdit,
};

pub const VOXEL_HOUSE_TEMPLATE_BOUNDS: [[i64; 3]; 2] = [[0, 0, 0], [10, 12, 8]];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VoxelTemplate {
    House,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VoxelTemplateRequest {
    pub template: VoxelTemplate,
    pub origin: [i64; 3],
    pub material_slot: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoxelTemplateError {
    InvalidOrigin(VoxelAuthorityValidationError),
    InvalidMaterial(VoxelAuthorityValidationError),
    CoordinateOverflow,
}

impl std::fmt::Display for VoxelTemplateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "voxel template rejected: {self:?}")
    }
}

impl std::error::Error for VoxelTemplateError {}

#[derive(Debug, Default, Clone, Copy)]
pub struct VoxelTemplateEditService;

impl VoxelTemplateEditService {
    /// Generate one coordinate-ordered, deduplicated template edit transaction.
    pub fn generate(
        self,
        request: VoxelTemplateRequest,
    ) -> Result<Vec<VoxelEdit>, VoxelTemplateError> {
        validate_voxel_address(request.origin).map_err(VoxelTemplateError::InvalidOrigin)?;
        validate_voxel_material_slot(request.material_slot)
            .map_err(VoxelTemplateError::InvalidMaterial)?;
        let local = match request.template {
            VoxelTemplate::House => house_voxels(),
        };
        local
            .into_iter()
            .map(|address| {
                let address = [0, 1, 2].map(|axis| {
                    request.origin[axis]
                        .checked_add(address[axis])
                        .ok_or(VoxelTemplateError::CoordinateOverflow)
                });
                let address = [address[0]?, address[1]?, address[2]?];
                validate_voxel_address(address).map_err(VoxelTemplateError::InvalidOrigin)?;
                Ok(VoxelEdit::Set {
                    address,
                    material_slot: request.material_slot,
                })
            })
            .collect()
    }
}

fn house_voxels() -> BTreeSet<[i64; 3]> {
    let mut voxels = BTreeSet::new();
    let mut add = |x, y, z| {
        voxels.insert([x, y, z]);
    };

    // Floor: XZ footprint at the minimum Y boundary.
    for x in 0..=10 {
        for z in 0..=8 {
            add(x, 0, z);
        }
    }

    // Front and back walls with a front door and paired windows.
    for z in [0, 8] {
        for x in 0..=10 {
            for y in 1..=5 {
                let doorway = z == 0 && (4..=6).contains(&x) && y <= 3;
                let window = ((1..=3).contains(&x) || (7..=9).contains(&x)) && (2..=3).contains(&y);
                if !doorway && !window {
                    add(x, y, z);
                }
            }
        }
    }

    // Side walls with paired windows.
    for x in [0, 10] {
        for z in 1..=7 {
            for y in 1..=5 {
                let window = ((1..=3).contains(&z) || (5..=7).contains(&z)) && (2..=3).contains(&y);
                if !window {
                    add(x, y, z);
                }
            }
        }
    }

    // Stepped gable roof with the ridge running across the house width.
    for inset in 0..=4 {
        let roof_y = 6 + inset;
        for x in 0..=10 {
            add(x, roof_y, inset);
            add(x, roof_y, 8 - inset);
        }
    }

    // Chimney above the rear roof slope.
    for x in 8..=9 {
        for y in 9..=12 {
            add(x, y, 6);
        }
    }
    voxels
}
