//! Path constraints: bones placed along the curve a path attachment draws
//! (rig-c's `src/core/constraints_path.ts`).
//!
//! The curve is the path its slot shows, as the slot's deform leaves it, in
//! world space at the moment the constraint runs. At constant speed the
//! curve is measured every pose: each curve's length by four
//! forward-difference steps, then the curve holding a length by ten. Off
//! constant speed the attachment's stated lengths are the table and a
//! curve's fraction is its cubic's parameter. Every constant and every order
//! of operations here is rig-c's, which holds them to its oracle to the bit.

use crate::rig::MeshData;
use crate::world::{BoneWorld, RAD, RUNTIME_PI};

/// Where a path constraint's `position` is along the curve.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum PositionMode {
    Fixed,
    Percent,
}

/// How a path constraint spaces its bones along the curve.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SpacingMode {
    Length,
    Fixed,
    Percent,
    Proportional,
}

/// Which way a path constraint turns its bones.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum RotateMode {
    Tangent,
    Chain,
    ChainScale,
}

pub(crate) struct PathData {
    pub bones: Vec<usize>,
    pub slot: usize,
    /// The slot's bone, whose reflection turns the offset rotation round.
    pub slot_bone: usize,
    pub position_mode: PositionMode,
    pub spacing_mode: SpacingMode,
    pub rotate_mode: RotateMode,
    /// Degrees.
    pub offset_rotation: f64,
}

/// The values a path constraint's timelines key, indices in
/// `ConstraintValues::values`.
pub(crate) const POSITION: usize = 0;
pub(crate) const SPACING: usize = 1;
pub(crate) const MIX_ROTATE: usize = 2;
pub(crate) const MIX_X: usize = 3;
pub(crate) const MIX_Y: usize = 4;

/// A path attachment: its knots and handles, and how it is walked.
pub(crate) struct PathGeometry {
    pub vertices: MeshData,
    pub closed: bool,
    pub constant_speed: bool,
    /// The cumulative length at each curve's end, as stated.
    pub lengths: Vec<f64>,
}

const EPSILON: f64 = 0.00001;

/// The lists one path solve works in, kept between steps.
#[derive(Clone, Default)]
pub(crate) struct Scratch {
    spaces: Vec<f64>,
    lengths: Vec<f64>,
    positions: Vec<f64>,
    chain: Vec<f64>,
    curve_ends: Vec<f64>,
}

/// rig-c's `curveLengthTable`: each curve's cumulative length, four
/// forward-difference steps a curve.
fn curve_length_table(chain: &[f64], curve_count: usize, out: &mut Vec<f64>) {
    out.clear();
    let mut total = 0.0;
    let (mut x1, mut y1) = (chain[0], chain[1]);
    let mut w = 2;
    for _ in 0..curve_count {
        let (cx1, cy1, cx2, cy2) = (chain[w], chain[w + 1], chain[w + 2], chain[w + 3]);
        let (x2, y2) = (chain[w + 4], chain[w + 5]);
        let h2x = (x1 - cx1 * 2.0 + cx2) * 0.1875;
        let h2y = (y1 - cy1 * 2.0 + cy2) * 0.1875;
        let d3x = ((cx1 - cx2) * 3.0 - x1 + x2) * 0.09375;
        let d3y = ((cy1 - cy2) * 3.0 - y1 + y2) * 0.09375;
        let mut d2x = h2x * 2.0 + d3x;
        let mut d2y = h2y * 2.0 + d3y;
        let mut d1x = (cx1 - x1) * 0.75 + h2x + d3x * 0.16666667;
        let mut d1y = (cy1 - y1) * 0.75 + h2y + d3y * 0.16666667;
        total += (d1x * d1x + d1y * d1y).sqrt();
        d1x += d2x;
        d1y += d2y;
        d2x += d3x;
        d2y += d3y;
        total += (d1x * d1x + d1y * d1y).sqrt();
        d1x += d2x;
        d1y += d2y;
        total += (d1x * d1x + d1y * d1y).sqrt();
        d1x += d2x + d3x;
        d1y += d2y + d3y;
        total += (d1x * d1x + d1y * d1y).sqrt();
        out.push(total);
        x1 = x2;
        y1 = y2;
        w += 6;
    }
}

fn on_line(p: f64, (x1, y1): (f64, f64), (x2, y2): (f64, f64), from: (f64, f64), out: &mut [f64]) {
    let r = (y2 - y1).atan2(x2 - x1);
    out[0] = from.0 + p * r.cos();
    out[1] = from.1 + p * r.sin();
    out[2] = r;
}

#[allow(clippy::too_many_arguments)]
fn on_curve(
    p: f64,
    (x1, y1): (f64, f64),
    (cx1, cy1): (f64, f64),
    (cx2, cy2): (f64, f64),
    (x2, y2): (f64, f64),
    out: &mut [f64],
    wants_tangent: bool,
) {
    if p.is_nan() {
        out[0] = x1;
        out[1] = y1;
        out[2] = (cy1 - y1).atan2(cx1 - x1);
        return;
    }
    let t2 = p * p;
    let t3 = t2 * p;
    let u = 1.0 - p;
    let u2 = u * u;
    let u3 = u2 * u;
    let ut = u * p;
    let ut3x = ut * 3.0;
    let w1 = u * ut3x;
    let w2 = ut3x * p;
    let x = x1 * u3 + cx1 * w1 + cx2 * w2 + x2 * t3;
    let y = y1 * u3 + cy1 * w1 + cy2 * w2 + y2 * t3;
    out[0] = x;
    out[1] = y;
    if wants_tangent {
        out[2] = if p < 0.001 {
            (cy1 - y1).atan2(cx1 - x1)
        } else {
            (y - (y1 * u2 + cy1 * ut * 2.0 + cy2 * t2))
                .atan2(x - (x1 * u2 + cx1 * ut * 2.0 + cx2 * t2))
        };
    }
}

/// The curve holding length `p` in the cumulative table `ends`, searched
/// from `curve` forward, and `p` as a fraction of it. A length past the
/// table reads as rig-c's does: not a number.
fn find(ends: &[f64], curve: &mut usize, p: f64) -> f64 {
    loop {
        let length = ends.get(*curve).copied().unwrap_or(f64::NAN);
        if p > length {
            *curve += 1;
            continue;
        }
        return if *curve == 0 {
            p / length
        } else {
            let prev = ends[*curve - 1];
            (p - prev) / (length - prev)
        };
    }
}

/// rig-c's `positionsOf`: each space's world position and angle along the
/// curve whose world points are `pts`, three numbers a space, into `out`.
#[allow(clippy::too_many_arguments)]
fn positions_of(
    p: &PathData,
    position: f64,
    g: &PathGeometry,
    pts: &[[f64; 2]],
    spaces: &[f64],
    wants_tangent: bool,
    out: &mut Vec<f64>,
    chain: &mut Vec<f64>,
    curve_ends: &mut Vec<f64>,
) {
    let space_count = spaces.len();
    let n = pts.len();
    let at = |k: usize| (pts[k][0], pts[k][1]);
    out.clear();
    out.resize(space_count * 3 + 2, 0.0);
    let closed = g.closed;
    let mut position = position;
    let scale_of = |total: f64| match p.spacing_mode {
        SpacingMode::Percent => total,
        SpacingMode::Proportional => total / space_count as f64,
        _ => 1.0,
    };
    if !g.constant_speed {
        let lengths = &g.lengths;
        let curve_count = n / 3 - if closed { 1 } else { 2 };
        let total = lengths[curve_count];
        if p.position_mode == PositionMode::Percent {
            position *= total;
        }
        let space_scale = scale_of(total);
        let mut curve = 0;
        for (i, &space) in spaces.iter().enumerate() {
            let o = 3 * i;
            let space = space * space_scale;
            position += space;
            let mut q = position;
            if closed {
                q %= total;
                if q < 0.0 {
                    q += total;
                }
                curve = 0;
            } else if q < 0.0 {
                on_line(q, at(1), at(2), at(1), &mut out[o..]);
                continue;
            } else if q > total {
                on_line(q - total, at(n - 3), at(n - 2), at(n - 2), &mut out[o..]);
                continue;
            }
            let q = find(lengths, &mut curve, q);
            let k = 3 * curve + 1;
            let c = |m: usize| at((k + m) % n);
            on_curve(
                q,
                c(0),
                c(1),
                c(2),
                c(3),
                &mut out[o..],
                wants_tangent || (i > 0 && space == 0.0),
            );
        }
        return;
    }
    // Constant speed: the chain, knot handle handle knot …
    chain.clear();
    if closed {
        for p in &pts[1..] {
            chain.extend(p);
        }
        chain.extend(pts[0]);
        chain.extend(pts[1]);
    } else {
        for p in &pts[1..n - 1] {
            chain.extend(p);
        }
    }
    let curve_count = if closed { n / 3 } else { n / 3 - 1 };
    curve_length_table(chain, curve_count, curve_ends);
    let total = curve_ends.last().copied().unwrap_or(0.0);
    if p.position_mode == PositionMode::Percent {
        position *= total;
    }
    let space_scale = scale_of(total);
    let mut segment_ends = [0.0; 10];
    let mut curve_total = 0.0;
    let mut last_curve = usize::MAX;
    let (mut x1, mut y1, mut cx1, mut cy1, mut cx2, mut cy2, mut x2, mut y2) =
        (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    let (mut curve, mut segment) = (0, 0);
    for (i, &space) in spaces.iter().enumerate() {
        let o = 3 * i;
        let space = space * space_scale;
        position += space;
        let mut q = position;
        if closed {
            q %= total;
            if q < 0.0 {
                q += total;
            }
            curve = 0;
            segment = 0;
        } else if q < 0.0 {
            on_line(
                q,
                (chain[0], chain[1]),
                (chain[2], chain[3]),
                (chain[0], chain[1]),
                &mut out[o..],
            );
            continue;
        } else if q > total {
            let e = chain.len();
            let (a, b) = ((chain[e - 4], chain[e - 3]), (chain[e - 2], chain[e - 1]));
            on_line(q - total, a, b, b, &mut out[o..]);
            continue;
        }
        let mut q = find(curve_ends, &mut curve, q);
        if curve != last_curve {
            last_curve = curve;
            let ii = curve * 6;
            x1 = chain[ii];
            y1 = chain[ii + 1];
            cx1 = chain[ii + 2];
            cy1 = chain[ii + 3];
            cx2 = chain[ii + 4];
            cy2 = chain[ii + 5];
            x2 = chain[ii + 6];
            y2 = chain[ii + 7];
            let h2x = (x1 - cx1 * 2.0 + cx2) * 0.03;
            let h2y = (y1 - cy1 * 2.0 + cy2) * 0.03;
            let d3x = ((cx1 - cx2) * 3.0 - x1 + x2) * 0.006;
            let d3y = ((cy1 - cy2) * 3.0 - y1 + y2) * 0.006;
            let mut d2x = h2x * 2.0 + d3x;
            let mut d2y = h2y * 2.0 + d3y;
            let mut d1x = (cx1 - x1) * 0.3 + h2x + d3x * 0.16666667;
            let mut d1y = (cy1 - y1) * 0.3 + h2y + d3y * 0.16666667;
            curve_total = (d1x * d1x + d1y * d1y).sqrt();
            segment_ends[0] = curve_total;
            for end in segment_ends.iter_mut().take(8).skip(1) {
                d1x += d2x;
                d1y += d2y;
                d2x += d3x;
                d2y += d3y;
                curve_total += (d1x * d1x + d1y * d1y).sqrt();
                *end = curve_total;
            }
            d1x += d2x;
            d1y += d2y;
            curve_total += (d1x * d1x + d1y * d1y).sqrt();
            segment_ends[8] = curve_total;
            d1x += d2x + d3x;
            d1y += d2y + d3y;
            curve_total += (d1x * d1x + d1y * d1y).sqrt();
            segment_ends[9] = curve_total;
            segment = 0;
        }
        q *= curve_total;
        loop {
            let length = segment_ends.get(segment).copied().unwrap_or(f64::NAN);
            if q > length {
                segment += 1;
                continue;
            }
            q = if segment == 0 {
                q / length
            } else {
                let prev = segment_ends[segment - 1];
                segment as f64 + (q - prev) / (length - prev)
            };
            break;
        }
        on_curve(
            q * 0.1,
            (x1, y1),
            (cx1, cy1),
            (cx2, cy2),
            (x2, y2),
            &mut out[o..],
            wants_tangent || (i > 0 && space == 0.0),
        );
    }
}

/// rig-c's `solvePath`: places the constraint's bones along the curve whose
/// world points are `pts`, with `values` its keyed values. `slot_world` is
/// the slot bone's world as the runtime last brought it up to date, which
/// only the offset rotation's sign reads. Returns whether it moved them:
/// every bone, in world space.
#[allow(clippy::too_many_arguments)]
pub(crate) fn solve(
    world: &mut [BoneWorld],
    lengths: &[f64],
    p: &PathData,
    values: &[f64; 7],
    g: &PathGeometry,
    pts: &[[f64; 2]],
    slot_world: &BoneWorld,
    scratch: &mut Scratch,
) -> bool {
    let (mix_rotate, mix_x, mix_y) = (values[MIX_ROTATE], values[MIX_X], values[MIX_Y]);
    if mix_rotate == 0.0 && mix_x == 0.0 && mix_y == 0.0 {
        return false;
    }
    let wants_tangent = p.rotate_mode == RotateMode::Tangent;
    let scale = p.rotate_mode == RotateMode::ChainScale;
    let bone_count = p.bones.len();
    let space_count = if wants_tangent {
        bone_count
    } else {
        bone_count + 1
    };
    let Scratch {
        spaces,
        lengths: bone_lengths,
        positions,
        chain,
        curve_ends,
    } = scratch;
    spaces.clear();
    spaces.resize(space_count, 0.0);
    bone_lengths.clear();
    if scale {
        bone_lengths.resize(bone_count, 0.0);
    }
    let spacing = values[SPACING];
    let world_length = |b: usize, length: f64| {
        let w = &world[b];
        let x = length * w.a;
        let y = length * w.c;
        (x * x + y * y).sqrt()
    };
    match p.spacing_mode {
        SpacingMode::Percent => {
            if scale {
                for (length, &b) in bone_lengths.iter_mut().zip(&p.bones) {
                    *length = world_length(b, lengths[b]);
                }
            }
            for s in spaces.iter_mut().skip(1) {
                *s = spacing;
            }
        }
        SpacingMode::Proportional => {
            let mut sum = 0.0;
            for i in 0..space_count - 1 {
                let b = p.bones[i];
                let bone_length = lengths[b];
                if bone_length < EPSILON {
                    if scale {
                        bone_lengths[i] = 0.0;
                    }
                    spaces[i + 1] = spacing;
                } else {
                    let length = world_length(b, bone_length);
                    if scale {
                        bone_lengths[i] = length;
                    }
                    spaces[i + 1] = length;
                    sum += length;
                }
            }
            if sum > 0.0 {
                let k = (space_count as f64 / sum) * spacing;
                for s in spaces.iter_mut().skip(1) {
                    *s *= k;
                }
            }
        }
        SpacingMode::Length | SpacingMode::Fixed => {
            let adds_length = p.spacing_mode == SpacingMode::Length;
            for i in 0..space_count - 1 {
                let b = p.bones[i];
                let bone_length = lengths[b];
                if bone_length < EPSILON {
                    if scale {
                        bone_lengths[i] = 0.0;
                    }
                    spaces[i + 1] = spacing;
                } else {
                    let length = world_length(b, bone_length);
                    if scale {
                        bone_lengths[i] = length;
                    }
                    let along = if adds_length {
                        (bone_length + spacing).max(0.0)
                    } else {
                        spacing
                    };
                    spaces[i + 1] = (along * length) / bone_length;
                }
            }
        }
    }
    positions_of(
        p,
        values[POSITION],
        g,
        pts,
        spaces,
        wants_tangent,
        positions,
        chain,
        curve_ends,
    );
    let (mut at_x, mut at_y) = (positions[0], positions[1]);
    let mut offset_rotation = p.offset_rotation;
    let from_tip = if offset_rotation == 0.0 {
        p.rotate_mode == RotateMode::Chain
    } else {
        let s = slot_world;
        offset_rotation *= if s.a * s.d - s.b * s.c > 0.0 {
            RAD
        } else {
            -RAD
        };
        false
    };
    for i in 0..bone_count {
        let k = 3 * (i + 1);
        let b = p.bones[i];
        let mut w = world[b];
        w.x += (at_x - w.x) * mix_x;
        w.y += (at_y - w.y) * mix_y;
        let (x, y) = (positions[k], positions[k + 1]);
        let (dx, dy) = (x - at_x, y - at_y);
        if scale {
            let length = bone_lengths[i];
            if length >= EPSILON {
                let s = ((dx * dx + dy * dy).sqrt() / length - 1.0) * mix_rotate + 1.0;
                w.a *= s;
                w.c *= s;
            }
        }
        at_x = x;
        at_y = y;
        if mix_rotate > 0.0 {
            let (a, bb, c, d) = (w.a, w.b, w.c, w.d);
            let mut r = if wants_tangent {
                positions[k - 1]
            } else if spaces[i + 1] == 0.0 {
                positions[k + 2]
            } else {
                dy.atan2(dx)
            };
            r -= c.atan2(a);
            if from_tip {
                let (cos, sin) = (r.cos(), r.sin());
                let length = lengths[b];
                at_x += (length * (cos * a - sin * c) - dx) * mix_rotate;
                at_y += (length * (sin * a + cos * c) - dy) * mix_rotate;
            } else {
                r += offset_rotation;
            }
            if r > RUNTIME_PI {
                r -= RUNTIME_PI * 2.0;
            } else if r < -RUNTIME_PI {
                r += RUNTIME_PI * 2.0;
            }
            r *= mix_rotate;
            let (cos, sin) = (r.cos(), r.sin());
            w.a = cos * a - sin * c;
            w.b = cos * bb - sin * d;
            w.c = sin * a + cos * c;
            w.d = sin * bb + cos * d;
        }
        world[b] = w;
    }
    true
}
