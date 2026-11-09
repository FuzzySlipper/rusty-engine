//! Parts rigs: 2D figures of posed pieces, drawn as one upright plane in the
//! world the way a sprite is.
//!
//! A rig's drawing (`DefinePartsRig`) is what never changes: its pages, and
//! each drawable's page UVs and triangles. An instance (`CreatePartsRig`) is
//! a node that faces the camera by its billboard mode, with a sprite's
//! lighting, alpha, depth and size options. Its pose (`SetPartsRigPose`) is
//! what the Engine evaluated for it on a step: the pieces in draw order and
//! their posed vertices, which realizers draw until the next pose replaces
//! it. A piece a clipping attachment cut carries its own UVs and triangles.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::{
    valid_color, validate_asset_id, BillboardMode, RenderAssetError, RenderAssetKind, RenderLayer,
    RenderMetadata, SpriteAlphaMode, SpriteDepthPolicy, SpriteLightingMode,
    SpriteMaterialDescriptor, SpriteMaterialError, SpriteSizeMode, Transform,
};

/// One page a rig draws from: a texture, and whether its colour is
/// premultiplied by its alpha.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PartsRigPage {
    pub texture: String,
    pub premultiplied: bool,
    /// The page's normal map or height map (red), in the page's layout:
    /// what a rig lit by `AuthoredNormal` or `AuthoredDepth` reads. A page
    /// without one lights flat under an authored normal, and by its colour's
    /// gradient under authored depth.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// What one attachment frame draws with: its page, a page UV per vertex, and
/// triangles over those vertices.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PartsRigDrawable {
    pub page: u32,
    /// Page UVs, 0 to 1 with v down, one per posed vertex.
    pub uvs: Vec<[f32; 2]>,
    pub triangles: Vec<u32>,
}

/// A rig's drawing: what its poses draw with.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PartsRigAsset {
    pub id: String,
    pub pages: Vec<PartsRigPage>,
    /// Shared, so copies of the asset do not copy its geometry.
    pub drawables: Arc<[PartsRigDrawable]>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PartsRigAssetError {
    Asset(RenderAssetError),
    Texture {
        page: usize,
        error: RenderAssetError,
    },
}

impl PartsRigAsset {
    /// The ids it is named by and names. Its drawables are the posing
    /// core's, from a rig it read.
    pub fn validate(&self) -> Result<(), PartsRigAssetError> {
        validate_asset_id(&self.id, RenderAssetKind::PartsRig)
            .map_err(PartsRigAssetError::Asset)?;
        for (page, value) in self.pages.iter().enumerate() {
            for texture in std::iter::once(&value.texture).chain(&value.detail) {
                validate_asset_id(texture, RenderAssetKind::Texture)
                    .map_err(|error| PartsRigAssetError::Texture { page, error })?;
            }
        }
        Ok(())
    }
}

/// How a slot's piece reaches what is drawn behind it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PartsRigBlend {
    #[default]
    Normal,
    Additive,
    Multiply,
    Screen,
}

/// A piece a clipping attachment cut: it draws its own UVs and triangles
/// rather than its drawable's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PartsRigClipped {
    /// Its vertices: the next this many of the pose's vertices and of its
    /// `uvs`, after the clipped pieces before it.
    pub vertices: u32,
    /// Its triangle indices, over its own vertices: the next this many of
    /// the pose's `triangles`. 0 when the clip cut it all away.
    pub triangles: u32,
}

/// One drawn piece of a pose, in draw order.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PartsRigPiece {
    /// The asset's drawable it draws with, and the page it draws from: its
    /// posed vertices are the next `uvs.len()` of the pose's, after the
    /// pieces before it, unless it is clipped.
    pub drawable: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clipped: Option<PartsRigClipped>,
    pub blend: PartsRigBlend,
    /// The slot's colour times its attachment's, RGBA from 0 to 1, as
    /// authored (sRGB).
    pub color: [f32; 4],
    /// The slot's two-colour tint dark colour, as authored; black when it has
    /// none, which leaves the colour alone.
    pub dark: [f32; 3],
}

/// A rig's pose after one step.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PartsRigPose {
    /// The Engine presentation time the pose was evaluated at.
    pub seconds: f64,
    pub pieces: Arc<[PartsRigPiece]>,
    /// Posed vertices in rig units, x right and y up from the rig's origin,
    /// piece after piece.
    pub vertices: Arc<[[f32; 2]]>,
    /// The page UVs of clipped pieces' vertices, piece after piece.
    #[serde(default, skip_serializing_if = "<[_]>::is_empty")]
    pub uvs: Arc<[[f32; 2]]>,
    /// The triangles of clipped pieces, piece after piece.
    #[serde(default, skip_serializing_if = "<[_]>::is_empty")]
    pub triangles: Arc<[u32]>,
}

/// One drawn parts rig: a node whose posed pieces lie on one plane through
/// its origin, turned to the camera by `billboard` as a sprite is.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PartsRigInstanceDescriptor {
    /// The rig's drawing (`DefinePartsRig`).
    pub asset: String,
    /// Metres per rig unit, or CSS pixels per rig unit for a pixel-sized
    /// rig, before the node's scale.
    pub scale: f32,
    pub size_mode: SpriteSizeMode,
    pub billboard: BillboardMode,
    /// Multiplies every piece's colour, linear.
    pub tint: [f32; 4],
    pub render_order: i32,
    pub depth: SpriteDepthPolicy,
    #[serde(default)]
    pub layer: RenderLayer,
    /// A sprite's material: its lighting, alpha, shadow policy, emission and
    /// authored facing. The pages are its colour, and their detail maps
    /// (`PartsRigPage::detail`) what authored normal and depth lighting
    /// read; the material names no texture, and softness and wind are a
    /// sprite's alone.
    #[serde(default)]
    pub material: SpriteMaterialDescriptor,
    /// Rig units per texel of the grid the figure draws on, for pixel art:
    /// each cell of the grid in the rig's plane shows the rig's colour at its
    /// centre. 0 draws it smooth.
    #[serde(default)]
    pub texel_snap: f32,
    pub visible: bool,
    pub transform: Transform,
    pub metadata: RenderMetadata,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PartsRigError {
    Asset(RenderAssetError),
    Material(SpriteMaterialError),
    NonPositiveScale,
    InvalidTint,
    InvalidTexelSnap,
    /// A sprite material option a rig does not draw: a texture (normal,
    /// depth, tint mask or emission map), softness or wind.
    UnsupportedMaterial,
}

impl PartsRigInstanceDescriptor {
    pub fn validate(&self) -> Result<(), PartsRigError> {
        validate_asset_id(&self.asset, RenderAssetKind::PartsRig).map_err(PartsRigError::Asset)?;
        if !(self.scale.is_finite() && self.scale > 0.0) {
            return Err(PartsRigError::NonPositiveScale);
        }
        if !valid_color(self.tint) {
            return Err(PartsRigError::InvalidTint);
        }
        if !(self.texel_snap.is_finite() && self.texel_snap >= 0.0) {
            return Err(PartsRigError::InvalidTexelSnap);
        }
        let material = &self.material;
        if material.normal_texture.is_some()
            || material.depth_texture.is_some()
            || material.tint_mask.is_some()
            || material.emission_map.is_some()
            || material.softness_metres > 0.0
            || material.wind_bend > 0.0
        {
            return Err(PartsRigError::UnsupportedMaterial);
        }
        // Its authored lighting reads its pages' detail maps, not a texture
        // the material names; the rest of the material is a sprite's.
        SpriteMaterialDescriptor {
            lighting: match material.lighting {
                SpriteLightingMode::AuthoredNormal | SpriteLightingMode::AuthoredDepth => {
                    SpriteLightingMode::Unlit
                }
                lighting => lighting,
            },
            ..material.clone()
        }
        .validate()
        .map_err(PartsRigError::Material)?;
        Ok(())
    }

    /// Whether the rig draws over what is behind it rather than solid.
    pub fn blended(&self) -> bool {
        self.material.alpha == SpriteAlphaMode::Blend || self.tint[3] < 1.0
    }
}
