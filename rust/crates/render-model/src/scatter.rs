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
    /// Where the copy falls among its patch's copies, 0 to 1, drawn from its
    /// spot: thinning keeps copies whose rank is below the share it keeps.
    #[serde(default)]
    pub rank: f32,
}

/// The most levels of detail a patch has beyond its own mesh.
pub const MAX_SCATTER_LODS: usize = 3;

/// The most points a thinning profile has.
pub const MAX_SCATTER_THINNING_POINTS: usize = 4;

/// A farther level of detail: from `start` metres from the camera, copies
/// draw `asset` (its slots bound as `material_overrides` say) in place of
/// the level before, the two crossfading over the patch's `lod_band` metres
/// before `start`. With `impostor`, the level is a camera-facing card
/// showing `asset` from the nearest of the views the renderer bakes of it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScatterLod {
    pub asset: String,
    pub material_overrides: Vec<MeshMaterialSlot>,
    pub start: f32,
    #[serde(default)]
    pub impostor: bool,
}

/// Coverage-preserving thinning: the share of copies kept by distance from
/// the camera, through `points` (distance, share) in increasing distance,
/// linear between them and level beyond them. A copy is kept while its rank
/// is below the share, and grows by the inverse square root of the share (at
/// most `max_scale`), so the ground stays as covered while the count falls.
/// No points keeps every copy.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScatterThinning {
    pub points: Vec<[f32; 2]>,
    pub max_scale: f32,
}

impl ScatterThinning {
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }
}

/// Copies shrink into their origin between `start` and `end` metres from the
/// camera and are gone beyond `end`; `end` 0 keeps them whole at any
/// distance. Nearer than `near_start` they are gone too, and they grow back
/// to whole by `near_end` (equal: whole from `near_start`); `near_end` 0
/// keeps them whole however near. A copy another patch hands over to at
/// some distance (`render_projection::vegetation`) fades in where the
/// other fades out.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScatterFade {
    pub start: f32,
    pub end: f32,
    #[serde(default)]
    pub near_start: f32,
    #[serde(default)]
    pub near_end: f32,
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
    /// Farther levels of detail, in increasing `start`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lods: Vec<ScatterLod>,
    /// How many metres before a level's start it crossfades with the one
    /// before.
    #[serde(default)]
    pub lod_band: f32,
    #[serde(default, skip_serializing_if = "ScatterThinning::is_empty")]
    pub thinning: ScatterThinning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScatterPatchError {
    Asset(RenderAssetError),
    MaterialSlot(MeshMaterialSlotError),
    TooManyInstances {
        instances: usize,
        limit: usize,
    },
    InvalidInstance {
        index: usize,
    },
    InvalidFade,
    /// Too many levels, starts not increasing past the band, or a band not
    /// finite and at least 0.
    InvalidLods,
    /// Too many points, distances not increasing, shares outside (0, 1], or
    /// a maximum scale below 1.
    InvalidThinning,
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
        let ScatterFade {
            start,
            end,
            near_start,
            near_end,
        } = self.fade;
        let distances = [start, end, near_start, near_end];
        if !distances
            .iter()
            .all(|value| value.is_finite() && *value >= 0.0)
            || (end > 0.0 && end <= start)
            || (near_end > 0.0 && near_end < near_start)
            || (end > 0.0 && near_end > 0.0 && near_end > start)
        {
            return Err(ScatterPatchError::InvalidFade);
        }
        validate_detail(&self.lods, self.lod_band, &self.thinning)
    }
}

/// Whether levels of detail and thinning are well formed (`ScatterPatchDescriptor::validate`).
pub fn validate_detail(
    lods: &[ScatterLod],
    band: f32,
    thinning: &ScatterThinning,
) -> Result<(), ScatterPatchError> {
    for lod in lods {
        validate_asset_id(&lod.asset, RenderAssetKind::StaticMesh)
            .map_err(ScatterPatchError::Asset)?;
        crate::mesh::validate_slots(&lod.material_overrides)
            .map_err(ScatterPatchError::MaterialSlot)?;
    }
    let mut floor = band;
    let starts_rise = lods.iter().all(|lod| {
        let rises = lod.start.is_finite() && lod.start > floor;
        floor = lod.start + band;
        rises
    });
    if lods.len() > MAX_SCATTER_LODS || !(band.is_finite() && band >= 0.0) || !starts_rise {
        return Err(ScatterPatchError::InvalidLods);
    }
    let mut last = -1.0;
    let points_rise = thinning.points.iter().all(|&[distance, share]| {
        let rises = distance.is_finite() && distance > last;
        last = distance;
        rises && share > 0.0 && share <= 1.0
    });
    if !thinning.is_empty()
        && (thinning.points.len() > MAX_SCATTER_THINNING_POINTS
            || !points_rise
            || !(thinning.max_scale.is_finite() && thinning.max_scale >= 1.0))
    {
        return Err(ScatterPatchError::InvalidThinning);
    }
    Ok(())
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
                .all(|value| value.is_finite() && *value >= 0.0)
            && (0.0..=1.0).contains(&self.rank)
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
                rank: 0.5,
            }],
            fade: ScatterFade {
                start: 20.0,
                end: 30.0,
                ..ScatterFade::default()
            },
            shadow_casting: ShadowCasting::None,
            lods: Vec::new(),
            lod_band: 0.0,
            thinning: ScatterThinning::default(),
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
            ..ScatterFade::default()
        };
        assert_eq!(bad.validate(), Err(ScatterPatchError::InvalidFade));
        let mut near = patch();
        near.fade.near_start = 4.0;
        near.fade.near_end = 6.0;
        assert_eq!(near.validate(), Ok(()));
        near.fade.near_end = 25.0;
        assert_eq!(near.validate(), Err(ScatterPatchError::InvalidFade));
        near.fade.near_end = 3.0;
        assert_eq!(near.validate(), Err(ScatterPatchError::InvalidFade));
        let mut whole = patch();
        whole.fade = ScatterFade::default();
        assert_eq!(whole.validate(), Ok(()));
        let json = serde_json::to_string(&patch()).unwrap();
        assert_eq!(
            serde_json::from_str::<ScatterPatchDescriptor>(&json).unwrap(),
            patch()
        );
    }

    #[test]
    fn levels_rise_past_their_band_and_thinning_keeps_a_share() {
        let lod = |start: f32| ScatterLod {
            asset: "mesh/grass-far".into(),
            material_overrides: Vec::new(),
            start,
            impostor: false,
        };
        let mut levels = patch();
        levels.lods = vec![lod(10.0), lod(20.0)];
        levels.lod_band = 4.0;
        assert_eq!(levels.validate(), Ok(()));
        levels.lods = vec![lod(10.0), lod(13.0)];
        assert_eq!(levels.validate(), Err(ScatterPatchError::InvalidLods));
        levels.lods = vec![lod(3.0)];
        assert_eq!(levels.validate(), Err(ScatterPatchError::InvalidLods));
        levels.lods = vec![lod(10.0), lod(20.0), lod(30.0), lod(40.0)];
        assert_eq!(levels.validate(), Err(ScatterPatchError::InvalidLods));

        let mut thinned = patch();
        thinned.thinning = ScatterThinning {
            points: vec![[20.0, 1.0], [60.0, 0.25]],
            max_scale: 2.0,
        };
        assert_eq!(thinned.validate(), Ok(()));
        thinned.thinning.points = vec![[60.0, 0.25], [20.0, 1.0]];
        assert_eq!(thinned.validate(), Err(ScatterPatchError::InvalidThinning));
        thinned.thinning.points = vec![[20.0, 0.0]];
        assert_eq!(thinned.validate(), Err(ScatterPatchError::InvalidThinning));
        thinned.thinning.points = vec![[20.0, 0.5]];
        thinned.thinning.max_scale = 0.5;
        assert_eq!(thinned.validate(), Err(ScatterPatchError::InvalidThinning));

        let mut ranked = patch();
        ranked.instances[0].rank = 1.5;
        assert!(ranked.validate().is_err());
        // A patch from before ranks and levels reads as before.
        let old = r#"{"asset":"mesh/grass","materialOverrides":[],"instances":[{"translation":[0,0,0],"rotation":[0,0,0,1],"scale":1,"tint":[1,1,1]}]}"#;
        let read: ScatterPatchDescriptor = serde_json::from_str(old).unwrap();
        assert_eq!(read.instances[0].rank, 0.0);
        assert!(read.lods.is_empty() && read.thinning.is_empty());
    }
}
