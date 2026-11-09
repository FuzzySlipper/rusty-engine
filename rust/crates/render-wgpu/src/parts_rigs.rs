//! Parts rigs: posed 2D figures drawn as their pieces on one upright plane.
//!
//! A rig's drawing (`DefinePartsRig`) is kept as each drawable's page, UVs,
//! triangles and page region. A rig node (`CreatePartsRig`) holds the pose
//! the Engine last evaluated for it (`SetPartsRigPose`); from that pose and
//! the drawing, the effects family (`effects.rs`) builds the node's vertex
//! and index buffers once per pose, not per view, and each view pass places
//! them on the plane its camera turns the rig to, as it places a sprite.
//!
//! Pieces draw in the pose's order on one plane, so they meet the depth
//! test at one depth and later pieces draw over earlier ones. Consecutive
//! pieces of one page and slot blend mode draw as one indexed run.

use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use render_model::{
    PartsRigAsset, PartsRigBlend, PartsRigDrawable, PartsRigInstanceDescriptor, PartsRigPose,
};

use crate::Renderer;

/// A rig's drawing, with its names resolved at apply.
pub(crate) struct PartsRigAssetRow {
    pub drawables: Arc<[PartsRigDrawable]>,
    pub pages: Vec<RigPage>,
    /// Each drawable's page region: the bounds of its UVs.
    pub regions: Vec<[f32; 4]>,
    /// Changes with each definition, so buffers built from the last one are
    /// built again.
    pub generation: u64,
}

/// A rig page with its names resolved (`Tables::names`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RigPage {
    pub texture: u32,
    pub premultiplied: bool,
    /// Its normal or height map.
    pub detail: Option<u32>,
}

/// A rig node's descriptor and the pose it draws.
pub(crate) struct PartsRigRow {
    pub descriptor: PartsRigInstanceDescriptor,
    /// Name id of the rig's drawing.
    pub asset: u32,
    pub pose: Option<PartsRigPose>,
    /// Counts its poses, so buffers built from an earlier one are built
    /// again.
    pub pose_revision: u64,
}

impl PartsRigRow {
    /// How far from the rig's origin its pose reaches, in rig units.
    pub fn reach(&self) -> f32 {
        self.pose.as_ref().map_or(0.0, |pose| {
            pose.vertices
                .iter()
                .fold(0.0f32, |reach, [x, y]| reach.max(x.hypot(*y)))
        })
    }
}

/// One vertex of a posed rig (`effects.wgsl` `PartsRigIn`, locations 0 and
/// 11 to 14).
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct RigVertex {
    pub position: [f32; 2],
    pub uv: [f32; 2],
    /// The piece's colour, sRGB with linear alpha, as authored.
    pub light: [u8; 4],
    /// The piece's dark colour, sRGB; alpha holds its page's flags
    /// (`PREMULTIPLIED`, `DETAIL`).
    pub dark: [u8; 4],
    /// The piece's page region, u0, v0, u1, v1.
    pub region: [u16; 4],
}

pub(crate) const RIG_VERTEX_BYTES: u64 = std::mem::size_of::<RigVertex>() as u64;

/// A vertex's page is premultiplied (`effects.wgsl` `shaded_parts_rig`).
const PREMULTIPLIED: u8 = 1;
/// A vertex's page has a detail map.
const DETAIL: u8 = 2;

/// Pieces drawn by one indexed call: they share a page and a blend mode.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RigRun {
    /// The page's texture name id, and its detail map's.
    pub texture: u32,
    pub detail: Option<u32>,
    pub blend: PartsRigBlend,
    pub first: u32,
    pub count: u32,
}

/// A pose's geometry: vertices, indices, runs, and the bounds of its
/// vertices in rig units (min x, min y, max x, max y).
#[derive(Default)]
pub(crate) struct RigGeometry {
    pub vertices: Vec<RigVertex>,
    pub indices: Vec<u32>,
    pub runs: Vec<RigRun>,
    pub bounds: [f32; 4],
}

fn unorm8(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn unorm16(value: f32) -> u16 {
    (value.clamp(0.0, 1.0) * 65535.0).round() as u16
}

/// Build `pose`'s geometry over `asset` into `out`. A piece naming a
/// drawable the drawing does not have, or vertices past the pose's, draws
/// nothing: a pose evaluated for a drawing since replaced.
pub(crate) fn build(asset: &PartsRigAssetRow, pose: &PartsRigPose, out: &mut RigGeometry) {
    out.vertices.clear();
    out.indices.clear();
    out.runs.clear();
    let mut bounds = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
    let mut next = 0usize;
    for piece in pose.pieces.iter() {
        let Some(drawable) = asset.drawables.get(piece.drawable as usize) else {
            break;
        };
        let vertices = next..next + drawable.uvs.len();
        next = vertices.end;
        let (Some(positions), Some(page)) = (
            pose.vertices.get(vertices),
            asset.pages.get(drawable.page as usize),
        ) else {
            break;
        };
        let base = out.vertices.len() as u32;
        let region = asset.regions[piece.drawable as usize].map(unorm16);
        let light = piece.color.map(unorm8);
        let [r, g, b] = piece.dark.map(unorm8);
        let flags = if page.premultiplied { PREMULTIPLIED } else { 0 }
            | if page.detail.is_some() { DETAIL } else { 0 };
        let dark = [r, g, b, flags];
        for (position, uv) in positions.iter().zip(&drawable.uvs) {
            bounds = [
                bounds[0].min(position[0]),
                bounds[1].min(position[1]),
                bounds[2].max(position[0]),
                bounds[3].max(position[1]),
            ];
            out.vertices.push(RigVertex {
                position: *position,
                uv: *uv,
                light,
                dark,
                region,
            });
        }
        let first = out.indices.len() as u32;
        out.indices
            .extend(drawable.triangles.iter().map(|index| base + index));
        let count = drawable.triangles.len() as u32;
        match out.runs.last_mut() {
            Some(run) if run.texture == page.texture && run.blend == piece.blend => {
                run.count += count
            }
            _ => out.runs.push(RigRun {
                texture: page.texture,
                detail: page.detail,
                blend: piece.blend,
                first,
                count,
            }),
        }
    }
    out.bounds = if out.vertices.is_empty() {
        [0.0; 4]
    } else {
        bounds
    };
}

impl Renderer {
    pub(crate) fn define_parts_rig(&mut self, asset: &PartsRigAsset) {
        let id = self.tables.names.id(&asset.id);
        let pages = asset
            .pages
            .iter()
            .map(|page| RigPage {
                texture: self.tables.names.id(&page.texture),
                premultiplied: page.premultiplied,
                detail: page.detail.as_deref().map(|id| self.tables.names.id(id)),
            })
            .collect();
        let regions = asset
            .drawables
            .iter()
            .map(|drawable| {
                drawable.uvs.iter().fold(
                    [f32::MAX, f32::MAX, f32::MIN, f32::MIN],
                    |[u0, v0, u1, v1], [u, v]| [u0.min(*u), v0.min(*v), u1.max(*u), v1.max(*v)],
                )
            })
            .collect();
        self.parts_rig_generation += 1;
        self.tables.parts_rigs.insert(
            id,
            PartsRigAssetRow {
                drawables: asset.drawables.clone(),
                pages,
                regions,
                generation: self.parts_rig_generation,
            },
        );
    }
}
