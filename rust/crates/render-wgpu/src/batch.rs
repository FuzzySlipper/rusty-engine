//! Draw lists: the parts a pass draws, culled and batched into instance runs.
//!
//! Every part's GPU row already lives in the dense `parts` storage buffer, so
//! a batch is a run of part ids in the `instances` buffer drawn with one
//! `draw_indexed`. Parts that share a mesh, index range and material (a batch
//! key, interned in `Parts`) draw together; colour, tint and emission are per
//! row, so instance parameters do not split a batch. Blended parts stay one
//! draw each, back to front.
//!
//! A moved part only rewrites its own row. The instance runs change when the
//! drawn set does (a part is created, destroyed, shown, hidden or culled),
//! and they are re-uploaded only then. Nothing copies matrices into batches,
//! so a part under a moving parent batches like any other.

use glam::{Mat4, Vec3, Vec4};
use render_model::RenderLayer;

use crate::tables::{Aabb, PartClass, PartId, Parts};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Pass {
    Opaque,
    /// Negative determinant: front faces wind clockwise (Three flips
    /// `frontFace` for such meshes).
    OpaqueMirrored,
    OpaqueDoubleSided,
    Lines,
    Blend,
    BlendMirrored,
    BlendDoubleSided,
}

impl Pass {
    pub fn of(class: PartClass, mirrored: bool) -> Self {
        match (class.lines, class.blend, class.double_sided, mirrored) {
            (true, ..) => Self::Lines,
            (false, false, true, _) => Self::OpaqueDoubleSided,
            (false, false, false, false) => Self::Opaque,
            (false, false, false, true) => Self::OpaqueMirrored,
            (false, true, true, _) => Self::BlendDoubleSided,
            (false, true, false, false) => Self::Blend,
            (false, true, false, true) => Self::BlendMirrored,
        }
    }

    fn blends(self) -> bool {
        self >= Self::Blend
    }

    /// Sort bucket: opaque passes group by pass; every blend pass shares one
    /// bucket so blended parts interleave back to front whatever their face
    /// culling, each still drawn with its own pipeline.
    fn bucket(self) -> Self {
        if self.blends() {
            Self::Blend
        } else {
            self
        }
    }
}

/// One instanced draw: `instances` parts sharing `part`'s mesh range and
/// material, whose ids start at `first_instance` in the instance buffer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Batch {
    pub pass: Pass,
    pub part: PartId,
    pub first_instance: u32,
    pub instances: u32,
}

#[derive(Default, PartialEq)]
pub(crate) struct DrawList {
    pub batches: Vec<Batch>,
    /// Part ids, one run per batch; offset by the list's base in the buffer.
    pub ids: Vec<u32>,
}

impl DrawList {
    pub fn instances(&self) -> u32 {
        self.ids.len() as u32
    }
}

/// View frustum planes for wgpu clip space (depth 0..1), pointing inward.
pub(crate) struct Frustum([Vec4; 6]);

impl Frustum {
    pub fn new(view_proj: &Mat4) -> Self {
        let (r0, r1, r2, r3) = (
            view_proj.row(0),
            view_proj.row(1),
            view_proj.row(2),
            view_proj.row(3),
        );
        Self([r3 + r0, r3 - r0, r3 + r1, r3 - r1, r2, r3 - r2])
    }

    /// Whether any part of the box may be inside (conservative).
    pub fn intersects(&self, bounds: &Aabb) -> bool {
        if bounds.is_empty() {
            return false;
        }
        self.0.iter().all(|plane| {
            let normal = plane.truncate();
            let farthest = Vec3::select(normal.cmpge(Vec3::ZERO), bounds.max, bounds.min);
            normal.dot(farthest) + plane.w >= 0.0
        })
    }
}

/// The parts a view pass sees: shown, in the viewmodel layer or (for a world
/// pass) any other layer, inside the frustum.
/// Opaque parts are grouped by pass and batch key; blended parts are sorted
/// back to front from `eye`. Instance ids are offset by `base`.
pub(crate) fn view_list(
    parts: &Parts,
    viewmodel: bool,
    frustum: &Frustum,
    eye: Vec3,
    base: u32,
) -> DrawList {
    let mut entries: Vec<Entry> = Vec::new();
    for (id, state) in parts.state.iter().enumerate() {
        if parts.meta[id].is_none()
            || !state.shown
            || (state.layer == RenderLayer::Viewmodel) != viewmodel
            || !frustum.intersects(&state.world_bounds)
        {
            continue;
        }
        let pass = Pass::of(state.class, state.mirrored);
        let order = if pass.blends() {
            // Farthest first: larger distances sort earlier.
            let center = (state.world_bounds.min + state.world_bounds.max) * 0.5;
            u32::MAX - center.distance_squared(eye).to_bits()
        } else {
            state.key
        };
        entries.push((pass.bucket(), order, id as PartId, pass));
    }
    group(parts, entries, base)
}

/// (sort bucket, order within the bucket, part, pass the part draws with)
type Entry = (Pass, u32, PartId, Pass);

/// Shadow casters: every shown triangle part of the scene layer, not culled
/// by any camera. Blended parts cast as opaque, as Three's depth material
/// did. Passes select the face culling: single-sided parts render their back
/// faces, double-sided parts both.
pub(crate) fn caster_list(parts: &Parts, base: u32) -> DrawList {
    let mut entries: Vec<Entry> = Vec::new();
    for (id, state) in parts.state.iter().enumerate() {
        if parts.meta[id].is_none()
            || !state.shown
            || state.layer != RenderLayer::Scene
            || state.class.lines
        {
            continue;
        }
        let opaque = PartClass {
            blend: false,
            ..state.class
        };
        let pass = Pass::of(opaque, state.mirrored);
        entries.push((pass, state.key, id as PartId, pass));
    }
    group(parts, entries, base)
}

fn group(parts: &Parts, mut entries: Vec<Entry>, base: u32) -> DrawList {
    entries.sort_unstable();
    let mut list = DrawList {
        batches: Vec::new(),
        ids: Vec::with_capacity(entries.len()),
    };
    let mut previous: Option<(Pass, u32)> = None;
    for (_, _, id, pass) in entries {
        let key = parts.state[id as usize].key;
        match list.batches.last_mut() {
            Some(batch) if !pass.blends() && previous == Some((pass, key)) => {
                batch.instances += 1;
            }
            _ => list.batches.push(Batch {
                pass,
                part: id,
                first_instance: base + list.ids.len() as u32,
                instances: 1,
            }),
        }
        previous = Some((pass, key));
        list.ids.push(id);
    }
    list
}

#[cfg(test)]
mod tests {
    use super::*;

    fn aabb(min: [f32; 3], max: [f32; 3]) -> Aabb {
        Aabb {
            min: Vec3::from(min),
            max: Vec3::from(max),
        }
    }

    #[test]
    fn frustum_keeps_boxes_in_front_and_drops_boxes_behind_or_aside() {
        let view = Mat4::look_to_rh(Vec3::ZERO, Vec3::NEG_Z, Vec3::Y);
        let projection = Mat4::perspective_rh(60f32.to_radians(), 16.0 / 9.0, 0.1, 100.0);
        let frustum = Frustum::new(&(projection * view));
        assert!(frustum.intersects(&aabb([-0.5, -0.5, -5.5], [0.5, 0.5, -4.5])));
        assert!(!frustum.intersects(&aabb([-0.5, -0.5, 4.5], [0.5, 0.5, 5.5])));
        assert!(!frustum.intersects(&aabb([40.0, -0.5, -5.5], [41.0, 0.5, -4.5])));
        assert!(!frustum.intersects(&aabb([-0.5, -0.5, -200.0], [0.5, 0.5, -150.0])));
        // A box straddling the near plane and the left edge is kept.
        assert!(frustum.intersects(&aabb([-50.0, -1.0, -3.0], [-1.0, 1.0, 1.0])));
        assert!(!frustum.intersects(&Aabb::EMPTY));
    }
}
