//! Parts rigs: 2D figures of posed pieces, drawn as one upright plane in the
//! world the way a sprite is.
//!
//! A rig's drawing (`DefinePartsRig`) is what never changes: its pages, and
//! each drawable's page UVs and triangles. An instance (`CreatePartsRig`) is
//! a node that faces the camera by its billboard mode, with a sprite's
//! lighting, alpha, depth and size options. It holds one or more views, each
//! a drawing seen from one heading, and each reading draws the view nearest
//! the bearing its camera sees the node from. A view's pose
//! (`SetPartsRigPose`) is what the Engine evaluated for it on a step: the
//! pieces in draw order and their posed vertices, which realizers draw until
//! the next pose of that view replaces it. A piece a clipping attachment cut
//! carries its own UVs and triangles.

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

/// One view of a parts rig node: the drawing it shows from one heading.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PartsRigView {
    /// The view's drawing (`DefinePartsRig`).
    pub asset: String,
    /// The bearing it is seen from, in radians about the node's up axis from
    /// its +z toward its +x: 0 is a camera in front of the node. Each
    /// reading draws the view whose heading is nearest the bearing of its
    /// camera, the way the node's billboard mode faces it (the eye, or
    /// against the view's heading for an orthographic or `CylindricalView`
    /// camera).
    pub heading: f32,
    /// Drawn mirrored across the rig's vertical axis, as a negative x scale
    /// on the plane: one side's rig shows the other side.
    pub mirror: bool,
}

/// One drawn parts rig: a node whose posed pieces lie on one plane through
/// its origin, turned to the camera by `billboard` as a sprite is.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PartsRigInstanceDescriptor {
    /// What it shows from each heading; one view is shown from every side.
    pub views: Vec<PartsRigView>,
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
    NoViews,
    Asset(RenderAssetError),
    InvalidHeading,
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
        if self.views.is_empty() {
            return Err(PartsRigError::NoViews);
        }
        for view in &self.views {
            validate_asset_id(&view.asset, RenderAssetKind::PartsRig)
                .map_err(PartsRigError::Asset)?;
            if !view.heading.is_finite() {
                return Err(PartsRigError::InvalidHeading);
            }
        }
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

/// The view of `views` a camera seeing the node from `bearing` (radians
/// about its up axis from its +z toward its +x) draws: the one whose heading
/// is nearest, the first of equally near ones.
pub fn parts_rig_view(views: &[PartsRigView], bearing: f32) -> usize {
    let distance = |heading: f32| {
        let turn = (bearing - heading).rem_euclid(std::f32::consts::TAU);
        turn.min(std::f32::consts::TAU - turn)
    };
    let mut nearest = 0;
    for (index, view) in views.iter().enumerate().skip(1) {
        if distance(view.heading) < distance(views[nearest].heading) {
            nearest = index;
        }
    }
    nearest
}

/// A camera as a parts rig's placement reads it: where its eye is and its
/// axes in the world (right, up, and the way it looks), and whether it is
/// orthographic.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PartsRigCamera {
    pub eye: [f32; 3],
    pub right: [f32; 3],
    pub up: [f32; 3],
    pub forward: [f32; 3],
    pub orthographic: bool,
}

/// A point of a world-sized parts rig as one camera's reading draws it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PartsRigPlacedPoint {
    /// The view that reading draws.
    pub view: usize,
    pub world: [f32; 3],
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn scaled(a: [f32; 3], by: f32) -> [f32; 3] {
    a.map(|v| v * by)
}

fn length(a: [f32; 3]) -> f32 {
    dot(a, a).sqrt()
}

/// The way an upright plane faces, level and of unit length: `toward` (the
/// eye, or against the view) with its height dropped, turned back toward
/// `authored`, the node's own facing, by `keep` of the angle between them.
/// The angle is taken the short way round the plane, so the result stays
/// within `keep` times 90 degrees of `toward` and the plane shows its front.
/// Looking straight up or down leaves no level `toward`, and the authored
/// facing stands. Sprites and parts rigs turn by it (render-wgpu, and
/// `effects.wgsl` `upright_facing`, which is the same).
pub fn upright_facing(toward: [f32; 3], authored: [f32; 3], keep: f32) -> [f32; 3] {
    let level = |facing: [f32; 3]| {
        let facing = [facing[0], 0.0, facing[2]];
        let length = length(facing);
        (length * length > f32::EPSILON).then(|| scaled(facing, 1.0 / length))
    };
    let authored = level(authored).unwrap_or([0.0, 0.0, 1.0]);
    let Some(toward) = level(toward) else {
        return authored;
    };
    if keep <= 0.0 {
        return toward;
    }
    let mut angle = cross(toward, authored)[1].atan2(dot(toward, authored));
    if angle > std::f32::consts::FRAC_PI_2 {
        angle -= std::f32::consts::PI;
    } else if angle < -std::f32::consts::FRAC_PI_2 {
        angle += std::f32::consts::PI;
    }
    // Turned about +y by `angle * keep`.
    let (sin, cos) = (angle * keep.min(1.0)).sin_cos();
    [
        cos * toward[0] + sin * toward[2],
        toward[1],
        -sin * toward[0] + cos * toward[2],
    ]
}

/// A node matrix taken apart as realizers do: the plane's axes scaled by
/// the rig's scale, the position, the signed scale (a reflection is a
/// negative x) and the rotation's x and z axes.
struct NodeFrame {
    position: [f32; 3],
    c0: [f32; 3],
    c1: [f32; 3],
    scale: [f32; 3],
    r0: [f32; 3],
    r2: [f32; 3],
}

impl NodeFrame {
    fn new(world: &[[f32; 4]; 4], rig_scale: f32) -> NodeFrame {
        let column = |k: usize| scaled([world[k][0], world[k][1], world[k][2]], rig_scale);
        let (c0, c1, c2) = (column(0), column(1), column(2));
        let sign = if dot(cross(c0, c1), c2) < 0.0 {
            -1.0
        } else {
            1.0
        };
        let scale = [sign * length(c0), length(c1), length(c2)];
        let axis = |c: [f32; 3], s: f32| if s == 0.0 { c } else { scaled(c, 1.0 / s) };
        NodeFrame {
            position: [world[3][0], world[3][1], world[3][2]],
            c0,
            c1,
            scale,
            r0: axis(c0, scale[0]),
            r2: axis(c2, scale[2]),
        }
    }
}

impl PartsRigInstanceDescriptor {
    /// The direction `camera` sees a node at `position` from, the way the
    /// billboard mode turns to it: toward the eye, or against the view for
    /// an orthographic camera or `CylindricalView`.
    fn toward(&self, position: [f32; 3], camera: &PartsRigCamera) -> [f32; 3] {
        if self.billboard == BillboardMode::CylindricalView || camera.orthographic {
            scaled(camera.forward, -1.0)
        } else {
            sub(camera.eye, position)
        }
    }

    fn view_from(&self, frame: &NodeFrame, camera: &PartsRigCamera) -> usize {
        if self.views.len() < 2 {
            return 0;
        }
        let toward = self.toward(frame.position, camera);
        parts_rig_view(
            &self.views,
            dot(toward, frame.r0).atan2(dot(toward, frame.r2)),
        )
    }

    /// The view `camera`'s reading draws, with the node at `world` (a
    /// column-major matrix): the one whose heading is nearest the bearing
    /// it sees the node from.
    pub fn view_seen(&self, world: &[[f32; 4]; 4], camera: &PartsRigCamera) -> usize {
        self.view_from(&NodeFrame::new(world, 1.0), camera)
    }

    /// Where `point` of the rig's plane (rig units, x right and y up, as
    /// authored) lies in the world as `camera`'s reading draws the rig, with
    /// its node at `world` (a column-major matrix): the view that reading
    /// picks, that view's mirror, and the plane turned by the billboard
    /// mode, as realizers place it. None for a pixel-sized rig, whose size
    /// follows the reading's viewport.
    pub fn place_point(
        &self,
        world: &[[f32; 4]; 4],
        camera: &PartsRigCamera,
        point: [f32; 2],
    ) -> Option<PartsRigPlacedPoint> {
        if self.size_mode == SpriteSizeMode::Pixel || self.views.is_empty() {
            return None;
        }
        let frame = NodeFrame::new(world, self.scale);
        let NodeFrame {
            position,
            c0,
            c1,
            scale,
            r2,
            ..
        } = frame;
        let view = self.view_from(&frame, camera);
        let mirror = if self.views[view].mirror { -1.0 } else { 1.0 };
        let [x, y] = [point[0] * mirror, point[1]];
        let (right, up) = match self.billboard {
            BillboardMode::None => {
                let world = [0, 1, 2].map(|k| position[k] + c0[k] * x + c1[k] * y);
                return Some(PartsRigPlacedPoint { view, world });
            }
            BillboardMode::Spherical => (camera.right, camera.up),
            BillboardMode::Cylindrical | BillboardMode::CylindricalView => {
                let toward = self.toward(position, camera);
                let forward = upright_facing(toward, r2, self.material.authored_facing);
                let right = cross([0.0, 1.0, 0.0], forward);
                (scaled(right, 1.0 / length(right)), [0.0, 1.0, 0.0])
            }
        };
        let world = [0, 1, 2].map(|k| position[k] + right[k] * scale[0] * x + up[k] * scale[1] * y);
        Some(PartsRigPlacedPoint { view, world })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(heading: f32) -> PartsRigView {
        PartsRigView {
            asset: "parts-rig/a".to_owned(),
            heading,
            mirror: false,
        }
    }

    #[test]
    fn a_camera_draws_the_view_nearest_its_bearing_the_short_way_round() {
        use std::f32::consts::{FRAC_PI_2, PI};
        let views = [view(0.0), view(FRAC_PI_2), view(PI), view(-FRAC_PI_2)];
        assert_eq!(parts_rig_view(&views, 0.2), 0);
        assert_eq!(parts_rig_view(&views, -0.2), 0);
        assert_eq!(parts_rig_view(&views, 1.4), 1);
        assert_eq!(parts_rig_view(&views, 3.0), 2);
        assert_eq!(parts_rig_view(&views, -3.0), 2);
        assert_eq!(parts_rig_view(&views, 4.8), 3, "past a full half turn");
        assert_eq!(parts_rig_view(&views, -1.7), 3);
        assert_eq!(
            parts_rig_view(&views[..1], 2.0),
            0,
            "one view shows from every side"
        );
    }
}
