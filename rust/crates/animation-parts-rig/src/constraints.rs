//! IK, transform and physics constraints, applied in the document's order
//! after the bones' world transforms (rig-c's `src/core/constraints.ts`;
//! physics in [`crate::physics`]).
//!
//! After each constraint, a bone it moved in world space has its local
//! values read back from that world transform, and every bone below a bone
//! it moved is posed again, so a later constraint reads the bones as the
//! earlier ones left them. Every constant and every order of operations
//! here is rig-c's, which holds them to its oracle to the bit.
//!
//! An ik over a bone the skin leaves inactive reads the frame above it as
//! this pass has it, where rig-c reads zeros when the runtime's update order
//! would not have reached it yet. That changes only the inactive bone's
//! local values, which nothing draws; rig-c refuses the rigs in which a
//! drawn bone reads them, and this core does not look for them.

use crate::physics::{self, Physics, PhysicsData};
use crate::rig::Rig;
use crate::world::{
    no_scale_direction, pose_bone, BoneWorld, Inherit, Local, DEG, M2, RAD, RUNTIME_PI,
};

/// What scales an ik's bone across its length as it compresses or stretches.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum IkScaleY {
    None,
    Uniform,
    Volume,
}

pub(crate) struct IkData {
    /// One bone, or a parent and its child.
    pub bones: Vec<usize>,
    pub target: usize,
    pub scale_y: IkScaleY,
}

/// One target property of a transform mapping. Properties are indices in
/// rig-c's order: rotate, x, y, scaleX, scaleY, shearY.
pub(crate) struct TransformTo {
    pub property: usize,
    pub offset: f64,
    pub scale: f64,
    pub max: f64,
}

pub(crate) struct TransformFrom {
    pub property: usize,
    pub offset: f64,
    pub to: Vec<TransformTo>,
}

pub(crate) struct TransformData {
    pub bones: Vec<usize>,
    pub source: usize,
    pub from: Vec<TransformFrom>,
    pub local_source: bool,
    pub local_target: bool,
    pub additive: bool,
    pub clamp: bool,
    /// The constraint's offsets, by property.
    pub offsets: [f64; 6],
}

pub(crate) enum ConstraintKind {
    Ik(IkData),
    Transform(TransformData),
    Physics(PhysicsData),
}

pub(crate) struct ConstraintData {
    pub name: String,
    /// Applied only under a skin whose list names it.
    pub skin_required: bool,
    pub kind: ConstraintKind,
    /// The values its timelines key, as the document sets them.
    pub setup: ConstraintValues,
}

/// The values a constraint's timelines key: an ik's mix and softness, then
/// its `bendPositive`, `compress` and `stretch`; a transform's six mixes; a
/// physics constraint's seven values (`crate::physics::INERTIA` on).
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct ConstraintValues {
    pub values: [f64; 7],
    pub flags: [bool; 3],
}

pub(crate) const BEND_POSITIVE: usize = 0;
pub(crate) const COMPRESS: usize = 1;
pub(crate) const STRETCH: usize = 2;

const ROTATE: usize = 0;
const X: usize = 1;
const Y: usize = 2;
const SCALE_X: usize = 3;
const SCALE_Y: usize = 4;
const SHEAR_Y: usize = 5;

/// rig-c's `RUNTIME_SOLVER_RULES`.
const IK_COLLAPSED_DET: f64 = 0.00001;
const IK_X_AXIS_FLOOR: f64 = 0.00001;
const IK_STRETCH_MIN_LENGTH: f64 = 0.00001;
const IK_TWO_BONE_NEAR_CHILD: f64 = 0.00001;
const UNIFORM_SCALE: f64 = 0.00001;
const READ_BACK_COLLAPSED_X: f64 = 0.0001;
const READ_BACK_COLLAPSED_Y: f64 = 0.00001;

/// The bones a rig's constraints work on, read once per rig.
pub(crate) struct Bones {
    pub parents: Vec<Option<usize>>,
    pub lengths: Vec<f64>,
    /// Each bone and every bone below it, in bone order.
    pub subtrees: Vec<Vec<usize>>,
}

impl Bones {
    pub fn new(parents: Vec<Option<usize>>, lengths: Vec<f64>) -> Bones {
        let mut subtrees = vec![Vec::new(); parents.len()];
        for k in 0..parents.len() {
            let mut at = Some(k);
            while let Some(b) = at {
                subtrees[b].push(k);
                at = parents[b];
            }
        }
        Bones {
            parents,
            lengths,
            subtrees,
        }
    }
}

/// The lists one pass of the constraints works in, kept between steps.
#[derive(Clone, Default)]
pub(crate) struct Scratch {
    changed: Vec<usize>,
    below: Vec<usize>,
    visit: Vec<usize>,
}

/// What the solver reads and writes: every bone's local values and world
/// transform, and which bones are active.
struct State<'a> {
    bones: &'a Bones,
    locals: &'a mut [Local],
    world: &'a mut [BoneWorld],
    active: &'a [bool],
    below: &'a mut Vec<usize>,
    visit: &'a mut Vec<usize>,
}

const IDENTITY: BoneWorld = BoneWorld {
    a: 1.0,
    b: 0.0,
    c: 0.0,
    d: 1.0,
    x: 0.0,
    y: 0.0,
};

impl State<'_> {
    fn parent_world(&self, bone: usize) -> BoneWorld {
        self.bones.parents[bone].map_or(IDENTITY, |p| self.world[p])
    }
}

/// Applies every constraint of the rig that `applied` keeps, in order, with
/// the values `values` holds for each, to the bones' locals and world
/// transforms, stepping the physics constraints on `physics`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn apply(
    rig: &Rig,
    values: &[ConstraintValues],
    applied: &[bool],
    locals: &mut [Local],
    world: &mut [BoneWorld],
    active: &[bool],
    scratch: &mut Scratch,
    physics: &mut Physics,
) {
    let Scratch {
        changed,
        below,
        visit,
    } = scratch;
    let mut state = State {
        bones: &rig.constraint_bones,
        locals,
        world,
        active,
        below,
        visit,
    };
    for (i, c) in rig.constraints.iter().enumerate() {
        if !applied[i] {
            continue;
        }
        let v = &values[i];
        match &c.kind {
            // An ik whose target is inactive is not applied.
            ConstraintKind::Ik(ik) => {
                if v.values[0] == 0.0 || !active[ik.target] {
                    continue;
                }
                let moved = match ik.bones.as_slice() {
                    [] => continue,
                    [bone] => solve_one(&mut state, ik, *bone, v, ik.scale_y, v.flags[COMPRESS]),
                    [parent, child, ..] => solve_two(&mut state, ik, *parent, *child, v),
                };
                if let Some(bone) = moved {
                    repose(&mut state, &[bone], &[]);
                }
            }
            // A transform is applied when its source is active, to every bone
            // it names.
            ConstraintKind::Transform(t) => {
                if !active[t.source] {
                    continue;
                }
                changed.clear();
                for &bone in &t.bones {
                    if solve_transform(&mut state, t, bone, &v.values) {
                        changed.push(bone);
                    }
                }
                if !changed.is_empty() {
                    let in_world: &[usize] = if t.local_target { &[] } else { changed };
                    repose(&mut state, changed, in_world);
                }
            }
            // A physics constraint steps when its bone is active, and moves
            // its bone in world space.
            ConstraintKind::Physics(p) => {
                if !active[p.bone] {
                    continue;
                }
                let mut w = state.world[p.bone];
                let length = state.bones.lengths[p.bone];
                let mut s = std::mem::take(&mut physics.states[i]);
                let moved = physics::step(p, &v.values, &mut s, &mut w, length, physics);
                physics.states[i] = s;
                if moved {
                    state.world[p.bone] = w;
                    repose(&mut state, &[p.bone], &[p.bone]);
                }
            }
        }
    }
}

/// rig-c's `repose`: the bones moved in world space read back into local
/// values, then every bone below one moved posed again, in bone order. An
/// inactive bone is not posed again, and the bones below it only when the
/// constraint moved it itself.
fn repose(state: &mut State<'_>, changed: &[usize], in_world: &[usize]) {
    for &bone in in_world {
        let parent = state.parent_world(bone);
        let world = state.world[bone];
        local_from_world(
            &mut state.locals[bone],
            state.bones.parents[bone].is_none(),
            &parent,
            &world,
        );
    }
    let mut visit = std::mem::take(state.visit);
    let mut below = std::mem::take(state.below);
    let list: &[usize] = if let [one] = changed {
        &state.bones.subtrees[*one]
    } else {
        visit.clear();
        visit.extend(
            changed
                .iter()
                .flat_map(|b| state.bones.subtrees[*b].iter().copied()),
        );
        visit.sort_unstable();
        visit.dedup();
        &visit
    };
    below.clear();
    below.extend_from_slice(changed);
    for &k in list {
        if let Some(p) = state.bones.parents[k] {
            if below.contains(&p)
                && !below.contains(&k)
                && (state.active[p] || changed.contains(&p))
            {
                below.push(k);
            }
        }
        if below.contains(&k) && !in_world.contains(&k) && state.active[k] {
            let parent = state.bones.parents[k].map(|p| state.world[p]);
            state.world[k] = pose_bone(&state.locals[k], parent.as_ref());
        }
    }
    *state.visit = visit;
    *state.below = below;
}

/// rig-c's `localFromWorld`: a bone's local values read back from its world
/// transform under its parent's.
fn local_from_world(b: &mut Local, root: bool, p: &BoneWorld, w: &BoneWorld) {
    let mode = if root { Inherit::Normal } else { b.inherit };
    let pid = 1.0 / (p.a * p.d - p.b * p.c);
    let dx = w.x - p.x;
    let dy = w.y - p.y;
    b.x = dx * (p.d * pid) - dy * (p.b * pid);
    b.y = dy * (p.a * pid) - dx * (p.c * pid);
    b.shear_x = 0.0;
    let (la, lb, lc, ld) = match mode {
        Inherit::NoScale | Inherit::NoScaleOrReflection => {
            let det = p.a * p.d - p.b * p.c;
            let flip = if mode == Inherit::NoScale && det < 0.0 {
                -1.0
            } else {
                1.0
            };
            let sign = if det < 0.0 { -1.0 } else { 1.0 };
            let rotation =
                ((p.a * w.c - p.c * w.a) * sign).atan2((p.d * w.a - p.b * w.c) * sign) * DEG;
            let (ux, uy) = no_scale_direction([p.a, p.b, p.c, p.d], rotation);
            decompose(
                b,
                ux * w.a + uy * w.c,
                (ux * w.c - uy * w.a) * flip,
                ux * w.b + uy * w.d,
                (ux * w.d - uy * w.b) * flip,
                false,
            );
            b.rotation = rotation;
            return;
        }
        Inherit::OnlyTranslation => (w.a, w.b, w.c, w.d),
        Inherit::NoRotationOrReflection => {
            let x_length_inverse = 1.0 / (p.a * p.a + p.c * p.c);
            let det_inverse = 1.0 / (p.a * p.d - p.b * p.c).abs();
            let m: M2 = [
                (p.a * w.a + p.c * w.c) * x_length_inverse,
                (p.a * w.b + p.c * w.d) * x_length_inverse,
                (p.a * w.c - p.c * w.a) * det_inverse,
                (p.a * w.d - p.c * w.b) * det_inverse,
            ];
            decompose(b, m[0], m[2], m[1], m[3], true);
            b.rotation += p.c.atan2(p.a) * DEG;
            return;
        }
        Inherit::Normal => {
            let (ia, ib, ic, id) = (p.d * pid, p.b * pid, p.c * pid, p.a * pid);
            (
                ia * w.a - ib * w.c,
                ia * w.b - ib * w.d,
                id * w.c - ic * w.a,
                id * w.d - ic * w.b,
            )
        }
    };
    decompose(b, la, lc, lb, ld, true);
}

/// rig-c's `decompose`: the local matrix with columns `(la, lc)` and
/// `(lb, ld)` as rotation, scales and shears. With `x_angle_is_rotation`
/// false (the two `noScale` modes) the x column's angle is kept as `shearX`.
fn decompose(b: &mut Local, la: f64, lc: f64, lb: f64, ld: f64, x_angle_is_rotation: bool) {
    let sx = (la * la + lc * lc).sqrt();
    let y_length = (lb * lb + ld * ld).sqrt();
    // Not above the bound, NaN included.
    if sx.is_nan() || sx <= READ_BACK_COLLAPSED_X {
        b.scale_x = 0.0;
        b.scale_y = y_length;
        b.shear_y = 0.0;
        b.rotation = if y_length > READ_BACK_COLLAPSED_Y {
            ld.atan2(lb) * DEG - 90.0
        } else {
            0.0
        };
        return;
    }
    let x_angle = lc.atan2(la) * DEG;
    let det = la * ld - lb * lc;
    let y_angle = ld.atan2(lb) * DEG;
    if x_angle_is_rotation {
        b.rotation = x_angle;
    } else {
        b.shear_x = x_angle;
    }
    b.scale_x = sx;
    b.scale_y = if det < 0.0 { -y_length } else { y_length };
    let from = if x_angle_is_rotation { x_angle } else { 0.0 };
    let right = if det < 0.0 { -90.0 } else { 90.0 };
    let mut shear = y_angle - (from + right);
    shear = if shear > 180.0 {
        shear - 360.0
    } else if shear < -180.0 {
        shear + 360.0
    } else {
        shear
    };
    b.shear_y = if y_length > READ_BACK_COLLAPSED_Y {
        shear
    } else {
        0.0
    };
}

/// A change brought once into (−180, 180]: −180 goes to 180.
fn wrap180(r: f64) -> f64 {
    if r > 180.0 {
        r - 360.0
    } else if r <= -180.0 {
        r + 360.0
    } else {
        r
    }
}

/// The runtime's `scaleY` for a bone an ik scaled by `s` along its length.
fn volume_scale_y(scale_y: f64, s: f64) -> f64 {
    scale_y / if s < 0.7 { 0.25 + s * 0.642857 } else { s }
}

/// rig-c's `solveOne`: the bone turns so its x axis points at the target.
/// Returns the bone it changed.
fn solve_one(
    state: &mut State<'_>,
    ik: &IkData,
    bone: usize,
    v: &ConstraintValues,
    scale_y_mode: IkScaleY,
    compress: bool,
) -> Option<usize> {
    let mix = v.values[0];
    let stretch = v.flags[STRETCH];
    let p = state.parent_world(bone);
    let bw = state.world[bone];
    let t = state.world[ik.target];
    let b = &mut state.locals[bone];
    let mode = b.inherit;
    let (pa, mut pb, pc, mut pd) = (p.a, p.b, p.c, p.d);
    let mut turn = 0.0;
    let in_parent = |pa: f64, pb: f64, pc: f64, pd: f64| -> (f64, f64) {
        let det = pa * pd - pb * pc;
        let x = t.x - p.x;
        let y = t.y - p.y;
        if det.abs() <= IK_COLLAPSED_DET {
            (0.0, 0.0)
        } else {
            ((x * pd - y * pb) / det - b.x, (y * pa - x * pc) / det - b.y)
        }
    };
    let (lx, ly) = match mode {
        Inherit::NoRotationOrReflection => {
            let k = (pa * pd - pb * pc).abs() / IK_X_AXIS_FLOOR.max(pa * pa + pc * pc);
            pb = -pc * k;
            pd = pa * k;
            turn = pc.atan2(pa) * DEG;
            in_parent(pa, pb, pc, pd)
        }
        Inherit::OnlyTranslation => (t.x - bw.x, t.y - bw.y),
        _ => in_parent(pa, pb, pc, pd),
    };
    let rotation = b.rotation;
    let scale_x = b.scale_x;
    let mut r = -b.shear_x - rotation + turn + ly.atan2(lx) * DEG;
    if scale_x < 0.0 {
        r += 180.0;
    }
    b.rotation = rotation + wrap180(r) * mix;
    let len = state.bones.lengths[bone] * scale_x;
    if (compress || stretch) && len > IK_STRETCH_MIN_LENGTH {
        let d = if matches!(mode, Inherit::NoScale | Inherit::NoScaleOrReflection) {
            let (wx, wy) = (t.x - bw.x, t.y - bw.y);
            (wx * wx + wy * wy).sqrt()
        } else {
            (lx * lx + ly * ly).sqrt()
        };
        if (compress && d < len) || (stretch && d > len) {
            let s = (d / len - 1.0) * mix + 1.0;
            b.scale_x = scale_x * s;
            match scale_y_mode {
                IkScaleY::Uniform => b.scale_y *= s,
                IkScaleY::Volume => b.scale_y = volume_scale_y(b.scale_y, s),
                IkScaleY::None => {}
            }
        }
    }
    Some(bone)
}

/// rig-c's `solveTwo`: the child's tip reaches the target. Returns the
/// parent, the child being below it.
fn solve_two(
    state: &mut State<'_>,
    ik: &IkData,
    parent: usize,
    child: usize,
    v: &ConstraintValues,
) -> Option<usize> {
    if state.locals[parent].inherit != Inherit::Normal
        || state.locals[child].inherit != Inherit::Normal
    {
        return None;
    }
    let mix = v.values[0];
    let softness = v.values[1];
    let stretch = v.flags[STRETCH];
    let g = state.parent_world(parent);
    let pw = state.world[parent];
    let t = state.world[ik.target];
    let gdet = g.a * g.d - g.b * g.c;
    let gid = if gdet.abs() <= IK_COLLAPSED_DET {
        0.0
    } else {
        1.0 / gdet
    };
    let in_grand = |wx: f64, wy: f64| -> (f64, f64) {
        let x = wx - g.x;
        let y = wy - g.y;
        ((x * g.d - y * g.b) * gid, (y * g.a - x * g.c) * gid)
    };
    let pl = state.locals[parent];
    let cl = state.locals[child];
    let (px, py, psx, psy) = (pl.x, pl.y, pl.scale_x, pl.scale_y);
    let csx = cl.scale_x;
    let (mut sx, mut sy) = (psx, psy);
    let cx = cl.x;
    let mut cy = cl.y;
    let uniform = (psx.abs() - psy.abs()).abs() <= UNIFORM_SCALE;
    if !uniform || stretch {
        cy = 0.0;
    }
    let (ox, oy) = in_grand(pw.a * cx + pw.b * cy + pw.x, pw.c * cx + pw.d * cy + pw.y);
    let dx = ox - px;
    let dy = oy - py;
    let l1 = (dx * dx + dy * dy).sqrt();
    let mut l2 = state.bones.lengths[child] * csx.abs();
    // The child's origin at the parent's: a one-bone ik on the parent,
    // stretched but neither compressed nor scaled across, the child at
    // rotation 0.
    if l1 < IK_TWO_BONE_NEAR_CHILD {
        solve_one(state, ik, parent, v, IkScaleY::None, false);
        let c = &mut state.locals[child];
        c.rotation = 0.0;
        c.y = cy;
        return Some(parent);
    }
    let (mut tx, mut ty) = in_grand(t.x, t.y);
    tx -= px;
    ty -= py;
    let mut d2 = tx * tx + ty * ty;
    let bend = if v.flags[BEND_POSITIVE] { 1.0 } else { -1.0 };
    if softness != 0.0 {
        let soft = softness * (psx.abs() * (csx.abs() + 1.0) * 0.5);
        let d = d2.sqrt();
        let sd = d - l1 - l2 * psx.abs() + soft;
        if sd > 0.0 {
            let q = (sd / (soft * 2.0)).min(1.0) - 1.0;
            let pull = (sd - soft * (1.0 - q * q)) / d;
            tx -= pull * tx;
            ty -= pull * ty;
            d2 = tx * tx + ty * ty;
        }
    }
    let a1: f64;
    let a2: f64;
    if uniform {
        l2 *= psx.abs();
        let mut cos = (d2 - l1 * l1 - l2 * l2) / (2.0 * l1 * l2);
        if cos < -1.0 {
            cos = -1.0;
            a2 = std::f64::consts::PI * bend;
        } else if cos > 1.0 {
            cos = 1.0;
            a2 = 0.0;
            if stretch {
                let f = (d2.sqrt() / (l1 + l2) - 1.0) * mix + 1.0;
                sx *= f;
                match ik.scale_y {
                    IkScaleY::Uniform => sy *= f,
                    IkScaleY::Volume => sy = volume_scale_y(sy, f),
                    IkScaleY::None => {}
                }
            }
        } else {
            a2 = cos.acos() * bend;
        }
        let along = l1 + l2 * cos;
        let across = l2 * a2.sin();
        a1 = (ty * along - tx * across).atan2(tx * along + ty * across);
    } else {
        let ea = psx.abs() * l2;
        let eb = psy.abs() * l2;
        let aa = ea * ea;
        let bb = eb * eb;
        let qa = bb - aa;
        let qb = -2.0 * bb * l1;
        let qc = bb * l1 * l1 + aa * d2 - aa * bb;
        let disc = qb * qb - 4.0 * qa * qc;
        let mut solved = None;
        if disc >= 0.0 {
            let mut q = disc.sqrt();
            if qb < 0.0 {
                q = -q;
            }
            q = -(qb + q) * 0.5;
            let r0 = q / qa;
            let r1 = qc / q;
            let x = if r0.abs() < r1.abs() { r0 } else { r1 };
            let y2 = d2 - x * x;
            if y2 >= 0.0 {
                let y = y2.sqrt() * bend;
                solved = Some((
                    ty.atan2(tx) - y.atan2(x),
                    (y / psy.abs()).atan2((x - l1) / psx.abs()),
                ));
            }
        }
        (a1, a2) = match solved {
            Some(angles) => angles,
            None => {
                // The nearest reachable pose: folded, straight or the
                // ellipse's vertex.
                let mut near = (RUNTIME_PI, l1 - ea, 0.0, (l1 - ea) * (l1 - ea));
                let mut far = (0.0, l1 + ea, 0.0, (l1 + ea) * (l1 + ea));
                let vertex = (-ea * l1) / (aa - bb);
                if (-1.0..=1.0).contains(&vertex) {
                    let angle = vertex.acos();
                    let x = ea * angle.cos() + l1;
                    let y = eb * angle.sin();
                    let dist = x * x + y * y;
                    if dist < near.3 {
                        near = (angle, x, y, dist);
                    }
                    if dist > far.3 {
                        far = (angle, x, y, dist);
                    }
                }
                let pick = if d2 <= (near.3 + far.3) * 0.5 {
                    near
                } else {
                    far
                };
                (ty.atan2(tx) - (pick.2 * bend).atan2(pick.1), pick.0 * bend)
            }
        };
    }
    // A scale of 0 counts as positive.
    let signs = (if psx < 0.0 { -1.0 } else { 1.0 }) * (if psy < 0.0 { -1.0 } else { 1.0 });
    let offset = cy.atan2(cx);
    let p = &mut state.locals[parent];
    let parent_rotation = p.rotation;
    let parent_change =
        (a1 - signs * offset) * DEG + if psx < 0.0 { 180.0 } else { 0.0 } - parent_rotation;
    p.rotation = parent_rotation + wrap180(parent_change) * mix;
    p.scale_x = sx;
    p.scale_y = sy;
    let c = &mut state.locals[child];
    let child_target =
        ((a2 + signs * offset) * DEG - c.shear_x) * signs + if csx < 0.0 { 180.0 } else { 0.0 };
    let child_rotation = c.rotation;
    c.y = cy;
    c.rotation = child_rotation + wrap180(child_target - child_rotation) * mix;
    Some(parent)
}

/// rig-c's `sourceValue`: the source's value of a property, offset by the
/// constraint's.
fn source_value(state: &State<'_>, t: &TransformData, property: usize) -> f64 {
    let o = &t.offsets;
    if t.local_source {
        let s = &state.locals[t.source];
        return match property {
            ROTATE => s.rotation + o[ROTATE],
            X => s.x + o[X],
            Y => s.y + o[Y],
            SCALE_X => s.scale_x + o[SCALE_X],
            SCALE_Y => s.scale_y + o[SCALE_Y],
            _ => s.shear_y + o[SHEAR_Y],
        };
    }
    let w = &state.world[t.source];
    match property {
        ROTATE => {
            // The offset is negated unless the determinant is above 0.
            let det = w.a * w.d - w.b * w.c;
            let mut r = w.c.atan2(w.a) * DEG + if det > 0.0 { o[ROTATE] } else { -o[ROTATE] };
            if r < 0.0 {
                r += 360.0;
            }
            r
        }
        X => w.a * o[X] + w.b * o[Y] + w.x,
        Y => w.c * o[X] + w.d * o[Y] + w.y,
        SCALE_X => (w.a * w.a + w.c * w.c).sqrt() + o[SCALE_X],
        SCALE_Y => (w.b * w.b + w.d * w.d).sqrt() + o[SCALE_Y],
        _ => (w.d.atan2(w.b) - w.c.atan2(w.a)) * DEG - 90.0 + o[SHEAR_Y],
    }
}

/// rig-c's `applyWorld`.
fn apply_world(w: &mut BoneWorld, property: usize, v: f64, mix: f64, additive: bool) {
    match property {
        ROTATE => {
            let mut r = if additive {
                v * RAD
            } else {
                v * RAD - w.c.atan2(w.a)
            };
            if r > RUNTIME_PI {
                r -= 2.0 * RUNTIME_PI;
            } else if r < -RUNTIME_PI {
                r += 2.0 * RUNTIME_PI;
            }
            r *= mix;
            let (cos, sin) = (r.cos(), r.sin());
            let (a, b) = (w.a, w.b);
            w.a = cos * a - sin * w.c;
            w.b = cos * b - sin * w.d;
            w.c = sin * a + cos * w.c;
            w.d = sin * b + cos * w.d;
        }
        X => {
            w.x = if additive {
                w.x + v * mix
            } else {
                w.x + (v - w.x) * mix
            }
        }
        Y => {
            w.y = if additive {
                w.y + v * mix
            } else {
                w.y + (v - w.y) * mix
            }
        }
        SCALE_X | SCALE_Y => {
            let (p, q) = if property == SCALE_X {
                (&mut w.a, &mut w.c)
            } else {
                (&mut w.b, &mut w.d)
            };
            let s = (*p * *p + *q * *q).sqrt();
            if s == 0.0 {
                return;
            }
            let k = if additive {
                1.0 + (v - 1.0) * mix
            } else {
                1.0 + ((v - s) * mix) / s
            };
            *p *= k;
            *q *= k;
        }
        _ => {
            let y_angle = w.d.atan2(w.b);
            let x_angle = w.c.atan2(w.a);
            let mut r = (v + 90.0) * RAD
                - if additive {
                    90.0 * RAD
                } else {
                    y_angle - x_angle
                };
            if !additive {
                r = if r > RUNTIME_PI {
                    r - 2.0 * RUNTIME_PI
                } else if r < -RUNTIME_PI {
                    r + 2.0 * RUNTIME_PI
                } else {
                    r
                };
            }
            let angle = y_angle + r * mix;
            let s = (w.b * w.b + w.d * w.d).sqrt();
            w.b = angle.cos() * s;
            w.d = angle.sin() * s;
        }
    }
}

/// rig-c's `applyLocal`.
fn apply_local(b: &mut Local, property: usize, v: f64, mix: f64, additive: bool) {
    let field = match property {
        ROTATE => &mut b.rotation,
        X => &mut b.x,
        Y => &mut b.y,
        SCALE_X => &mut b.scale_x,
        SCALE_Y => &mut b.scale_y,
        _ => &mut b.shear_y,
    };
    let current = *field;
    *field = if !additive {
        current + (v - current) * mix
    } else if property == SCALE_X || property == SCALE_Y {
        current * (1.0 + (v - 1.0) * mix)
    } else {
        current + v * mix
    };
}

/// rig-c's `solveTransform` for one bone: whether it changed it.
fn solve_transform(
    state: &mut State<'_>,
    t: &TransformData,
    bone: usize,
    mixes: &[f64; 7],
) -> bool {
    let mut w = state.world[bone];
    let mut applied = false;
    for from in &t.from {
        let value = source_value(state, t, from.property) - from.offset;
        for to in &from.to {
            let mix = mixes[to.property];
            if mix == 0.0 {
                continue;
            }
            let mut v = value * to.scale + to.offset;
            if t.clamp {
                let lo = to.offset.min(to.max);
                let hi = to.offset.max(to.max);
                v = if v < lo {
                    lo
                } else if v > hi {
                    hi
                } else {
                    v
                };
            }
            applied = true;
            if t.local_target {
                apply_local(&mut state.locals[bone], to.property, v, mix, t.additive);
            } else {
                apply_world(&mut w, to.property, v, mix, t.additive);
            }
        }
    }
    if applied && !t.local_target {
        state.world[bone] = w;
    }
    applied
}
