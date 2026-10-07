//! Scattered instances: one retained node draws many copies of a static mesh.
//!
//! A scatter patch is grass, stones or flowers over one piece of ground: the
//! Engine places them (`render-projection` `voxel_scatter`) and the renderer
//! draws the patch's copies of each mesh group as one instanced draw, culled
//! by the patch's bounds. The copies are rows of the patch, not nodes: they
//! are never picked, moved, parented or collided with one by one.

use serde::{Deserialize, Serialize};

use crate::{
    validate_asset_id, MeshMaterialSlot, MeshMaterialSlotError, RenderAssetError, RenderAssetKind,
    ShadowCasting,
};

/// The most copies one patch may hold.
pub const MAX_SCATTER_PATCH_INSTANCES: usize = 65_536;

/// One copy of the patch's mesh, in the patch node's space.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScatterInstance {
    pub translation: [f32; 3],
    /// Unit quaternion (x, y, z, w).
    pub rotation: [f32; 4],
    /// Uniform scale; positive.
    pub scale: f32,
    /// Linear colour multiplied into the material's colour.
    pub tint: [f32; 3],
}

/// Copies shrink into their origin between `start` and `end` metres from the
/// camera and are gone beyond `end`; `end` 0 keeps them whole at any
/// distance.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScatterFade {
    pub start: f32,
    pub end: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScatterPatchDescriptor {
    /// The static mesh every copy draws.
    pub asset: String,
    pub material_overrides: Vec<MeshMaterialSlot>,
    pub instances: Vec<ScatterInstance>,
    #[serde(default)]
    pub fade: ScatterFade,
    /// Whether the copies cast shadows (`Cast` by default).
    #[serde(default, skip_serializing_if = "ShadowCasting::is_cast")]
    pub shadow_casting: ShadowCasting,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScatterPatchError {
    Asset(RenderAssetError),
    MaterialSlot(MeshMaterialSlotError),
    TooManyInstances { instances: usize, limit: usize },
    InvalidInstance { index: usize },
    InvalidFade,
}

impl ScatterPatchDescriptor {
    pub fn validate(&self) -> Result<(), ScatterPatchError> {
        validate_asset_id(&self.asset, RenderAssetKind::StaticMesh)
            .map_err(ScatterPatchError::Asset)?;
        crate::mesh::validate_slots(&self.material_overrides)
            .map_err(ScatterPatchError::MaterialSlot)?;
        if self.instances.len() > MAX_SCATTER_PATCH_INSTANCES {
            return Err(ScatterPatchError::TooManyInstances {
                instances: self.instances.len(),
                limit: MAX_SCATTER_PATCH_INSTANCES,
            });
        }
        if let Some(index) = self.instances.iter().position(|instance| !instance.valid()) {
            return Err(ScatterPatchError::InvalidInstance { index });
        }
        let ScatterFade { start, end } = self.fade;
        if !(start.is_finite() && end.is_finite() && start >= 0.0 && end >= 0.0)
            || (end > 0.0 && end <= start)
        {
            return Err(ScatterPatchError::InvalidFade);
        }
        Ok(())
    }
}

impl ScatterInstance {
    fn valid(&self) -> bool {
        let length = self
            .rotation
            .iter()
            .map(|value| value * value)
            .sum::<f32>()
            .sqrt();
        self.translation.iter().all(|value| value.is_finite())
            && self.rotation.iter().all(|value| value.is_finite())
            && (length - 1.0).abs() <= 1.0e-3
            && self.scale.is_finite()
            && self.scale > 0.0
            && self
                .tint
                .iter()
                .all(|value| value.is_finite() && (0.0..=16.0).contains(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn patch() -> ScatterPatchDescriptor {
        ScatterPatchDescriptor {
            asset: "mesh/grass".into(),
            material_overrides: Vec::new(),
            instances: vec![ScatterInstance {
                translation: [1.0, 2.0, 3.0],
                rotation: [0.0, 0.0, 0.0, 1.0],
                scale: 1.0,
                tint: [1.0; 3],
            }],
            fade: ScatterFade {
                start: 20.0,
                end: 30.0,
            },
            shadow_casting: ShadowCasting::None,
        }
    }

    #[test]
    fn a_patch_admits_unit_rotations_positive_scales_and_an_ordered_fade() {
        assert_eq!(patch().validate(), Ok(()));
        let mut bad = patch();
        bad.instances[0].rotation = [0.0, 0.0, 0.0, 2.0];
        assert_eq!(
            bad.validate(),
            Err(ScatterPatchError::InvalidInstance { index: 0 })
        );
        let mut bad = patch();
        bad.instances[0].scale = 0.0;
        assert!(bad.validate().is_err());
        let mut bad = patch();
        bad.fade = ScatterFade {
            start: 30.0,
            end: 20.0,
        };
        assert_eq!(bad.validate(), Err(ScatterPatchError::InvalidFade));
        let mut whole = patch();
        whole.fade = ScatterFade::default();
        assert_eq!(whole.validate(), Ok(()));
        let json = serde_json::to_string(&patch()).unwrap();
        assert_eq!(
            serde_json::from_str::<ScatterPatchDescriptor>(&json).unwrap(),
            patch()
        );
    }
}
