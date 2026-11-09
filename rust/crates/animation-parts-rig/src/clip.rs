//! Clipping attachments: a world polygon that cuts what every slot from its
//! own through its `end` slot draws, as rig-c's core (`src/core/clipping.ts`)
//! cuts it.
//!
//! A strictly convex polygon cuts each triangle edge by edge
//! (Sutherland–Hodgman) after it is turned clockwise. A simple polygon that
//! is not convex is cut into convex pieces (ear clipping, then merged while
//! convex), and each triangle is cut against each piece in turn; with
//! `convex` set it is its convex hull instead. An `inverse` clip keeps the
//! parts of each triangle outside its convex hull. A polygon that is not
//! simple, or has a repeated vertex, is not cut through: a clip of it hides
//! what it covers, and an inverse one shows it whole.

type Pt = [f64; 2];

/// How one clip cuts, for the pose it was posed in.
pub(crate) enum Plan {
    /// One clockwise convex polygon.
    Convex(Vec<Pt>),
    /// Clockwise convex pieces whose union is the polygon.
    Pieces(Vec<Vec<Pt>>),
    /// The clockwise convex hull whose outside is kept.
    Inverse(Vec<Pt>),
    /// Nothing under the clip is drawn.
    Hide,
    /// What is under the clip is drawn as it is.
    Show,
}

/// A piece's geometry cut by a clip: positions, page UVs and triangles over
/// them, appended to the pose's.
pub(crate) struct Cut<'a> {
    pub vertices: &'a mut Vec<[f32; 2]>,
    pub uvs: &'a mut Vec<[f32; 2]>,
    pub triangles: &'a mut Vec<u32>,
}

fn pts(polygon: &[f64]) -> Vec<Pt> {
    polygon.as_chunks::<2>().0.to_vec()
}

/// The polygon's signed area, doubled: positive when counter-clockwise.
fn signed_area2(pts: &[Pt]) -> f64 {
    let n = pts.len();
    (0..n)
        .map(|i| {
            let j = (i + 1) % n;
            pts[i][0] * pts[j][1] - pts[j][0] * pts[i][1]
        })
        .sum()
}

fn cross3(o: Pt, a: Pt, b: Pt) -> f64 {
    (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
}

/// rig-c's `convexWhy` is null: every turn has the same sign, and none is 0.
fn strictly_convex(pts: &[Pt]) -> bool {
    let n = pts.len();
    if n < 3 {
        return false;
    }
    let mut sign = 0.0;
    for i in 0..n {
        let (a, b, c) = (pts[i], pts[(i + 1) % n], pts[(i + 2) % n]);
        let cross = (b[0] - a[0]) * (c[1] - b[1]) - (b[1] - a[1]) * (c[0] - b[0]);
        if cross == 0.0 {
            return false;
        }
        let s = cross.signum();
        if sign == 0.0 {
            sign = s;
        } else if s != sign {
            return false;
        }
    }
    true
}

/// Walked clockwise, as the clipper walks it.
fn clockwise(mut pts: Vec<Pt>) -> Vec<Pt> {
    if signed_area2(&pts) > 0.0 {
        pts.reverse();
    }
    pts
}

fn counter_clockwise(mut pts: Vec<Pt>) -> Vec<Pt> {
    if signed_area2(&pts) < 0.0 {
        pts.reverse();
    }
    pts
}

fn segments_meet(a: Pt, b: Pt, c: Pt, d: Pt) -> bool {
    let (d1, d2, d3, d4) = (
        cross3(c, d, a),
        cross3(c, d, b),
        cross3(a, b, c),
        cross3(a, b, d),
    );
    let opposite = |p: f64, q: f64| (p > 0.0 && q < 0.0) || (p < 0.0 && q > 0.0);
    if opposite(d1, d2) && opposite(d3, d4) {
        return true;
    }
    let on = |p: Pt, q: Pt, r: Pt| {
        p[0].min(q[0]) <= r[0]
            && r[0] <= p[0].max(q[0])
            && p[1].min(q[1]) <= r[1]
            && r[1] <= p[1].max(q[1])
    };
    (d1 == 0.0 && on(c, d, a))
        || (d2 == 0.0 && on(c, d, b))
        || (d3 == 0.0 && on(a, b, c))
        || (d4 == 0.0 && on(a, b, d))
}

/// Two edges that are not neighbours meet, or a neighbour folds back.
fn self_crossing(pts: &[Pt]) -> bool {
    let n = pts.len();
    for i in 0..n {
        let (a, b, c) = (pts[i], pts[(i + 1) % n], pts[(i + 2) % n]);
        if cross3(a, b, c) == 0.0
            && (c[0] - b[0]) * (a[0] - b[0]) + (c[1] - b[1]) * (a[1] - b[1]) > 0.0
        {
            return true;
        }
        for j in i + 2..n {
            if i == 0 && j == n - 1 {
                continue;
            }
            if segments_meet(a, b, pts[j], pts[(j + 1) % n]) {
                return true;
            }
        }
    }
    false
}

/// Every vertex on the line through its two neighbours dropped.
fn without_collinear(pts: &[Pt]) -> Vec<Pt> {
    let mut out = pts.to_vec();
    let mut changed = true;
    while changed && out.len() > 3 {
        changed = false;
        let n = out.len();
        if let Some(i) =
            (0..n).find(|&i| cross3(out[(i + n - 1) % n], out[i], out[(i + 1) % n]) == 0.0)
        {
            out.remove(i);
            changed = true;
        }
    }
    out
}

/// Andrew's monotone chain: counter-clockwise, no collinear vertex.
fn convex_hull(pts: &[Pt]) -> Vec<Pt> {
    let mut sorted = pts.to_vec();
    sorted.sort_by(|p, q| p[0].total_cmp(&q[0]).then(p[1].total_cmp(&q[1])));
    let half = |list: &mut dyn Iterator<Item = Pt>| {
        let mut h: Vec<Pt> = Vec::new();
        for p in list {
            while h.len() >= 2 && cross3(h[h.len() - 2], h[h.len() - 1], p) <= 0.0 {
                h.pop();
            }
            h.push(p);
        }
        h.pop();
        h
    };
    let mut hull = half(&mut sorted.iter().copied());
    hull.extend(half(&mut sorted.iter().rev().copied()));
    hull
}

/// A simple counter-clockwise polygon without collinear vertices cut into
/// convex pieces: ear clipping from vertex 0, then Hertel–Mehlhorn merging
/// across each shared edge while the union stays strictly convex.
fn convex_pieces(pts: &[Pt]) -> Vec<Vec<Pt>> {
    let in_tri = |p: Pt, a: Pt, b: Pt, c: Pt| {
        cross3(a, b, p) >= 0.0 && cross3(b, c, p) >= 0.0 && cross3(c, a, p) >= 0.0
    };
    let mut idx: Vec<usize> = (0..pts.len()).collect();
    let mut pieces: Vec<Vec<usize>> = Vec::new();
    while idx.len() > 3 {
        let n = idx.len();
        let ear = (0..n).find(|&k| {
            let (i0, i1, i2) = (idx[(k + n - 1) % n], idx[k], idx[(k + 1) % n]);
            cross3(pts[i0], pts[i1], pts[i2]) > 0.0
                && !idx.iter().any(|&j| {
                    j != i0 && j != i1 && j != i2 && in_tri(pts[j], pts[i0], pts[i1], pts[i2])
                })
        });
        // A simple polygon always has an ear; float ties can leave none, and
        // what is left is kept whole.
        let Some(k) = ear else { break };
        pieces.push(vec![idx[(k + n - 1) % n], idx[k], idx[(k + 1) % n]]);
        idx.remove(k);
    }
    pieces.push(idx);
    let convex = |ring: &[usize]| {
        let n = ring.len();
        (0..n).all(|i| {
            cross3(
                pts[ring[(i + n - 1) % n]],
                pts[ring[i]],
                pts[ring[(i + 1) % n]],
            ) > 0.0
        })
    };
    'merge: loop {
        for a in 0..pieces.len() {
            for b in a + 1..pieces.len() {
                let (pa, pb) = (&pieces[a], &pieces[b]);
                for i in 0..pa.len() {
                    let (u, v) = (pa[i], pa[(i + 1) % pa.len()]);
                    let Some(j) = pb.iter().position(|&x| x == v) else {
                        continue;
                    };
                    if pb[(j + 1) % pb.len()] != u {
                        continue;
                    }
                    let mut ring = pa[..=i].to_vec();
                    let mut k = (j + 2) % pb.len();
                    while k != j {
                        ring.push(pb[k]);
                        k = (k + 1) % pb.len();
                    }
                    ring.extend_from_slice(&pa[i + 1..]);
                    if !convex(&ring) {
                        continue;
                    }
                    pieces[a] = ring;
                    pieces.remove(b);
                    continue 'merge;
                }
            }
        }
        break;
    }
    pieces
        .into_iter()
        .map(|ring| ring.into_iter().map(|v| pts[v]).collect())
        .collect()
}

/// How a clip with this world polygon and these flags cuts: rig-c's
/// `clipThrough`, which plans a polygon that is cut into pieces again from
/// its float32 spelling.
pub(crate) fn plan(polygon: &[f64], inverse: bool, convex: bool) -> Plan {
    match shape_of(polygon, inverse, convex) {
        Plan::Pieces(_) => {
            let rounded: Vec<f64> = polygon.iter().map(|v| f64::from(*v as f32)).collect();
            shape_of(&rounded, inverse, convex)
        }
        plan => plan,
    }
}

/// rig-c's `clipShapeOf`.
fn shape_of(polygon: &[f64], inverse: bool, convex: bool) -> Plan {
    let refused = if inverse { Plan::Show } else { Plan::Hide };
    let p = pts(polygon);
    let n = p.len();
    if n < 3 {
        return refused;
    }
    let pairs: usize = (0..n)
        .map(|i| (i + 1..n).filter(|&j| p[i] == p[j]).count())
        .sum();
    if pairs > 0 {
        // Consecutive repeats of an otherwise strictly convex polygon: the
        // convex rule, whose zero-length edge has no inside.
        let distinct: Vec<Pt> = (0..n)
            .filter(|&i| p[i] != p[(i + 1) % n])
            .map(|i| p[i])
            .collect();
        if !inverse
            && distinct.len() == n - pairs
            && distinct.len() >= 3
            && strictly_convex(&distinct)
        {
            return Plan::Convex(clockwise(p));
        }
        return refused;
    }
    if self_crossing(&p) {
        return refused;
    }
    let simple = without_collinear(&p);
    if simple.len() == 3 && cross3(simple[0], simple[1], simple[2]) == 0.0 {
        return refused;
    }
    if inverse {
        return Plan::Inverse(clockwise(convex_hull(&simple)));
    }
    if strictly_convex(&p) {
        return Plan::Convex(clockwise(p));
    }
    if convex {
        return Plan::Convex(clockwise(convex_hull(&simple)));
    }
    let mut pieces = convex_pieces(&counter_clockwise(simple));
    if pieces.len() == 1 {
        return Plan::Convex(clockwise(pieces.remove(0)));
    }
    Plan::Pieces(pieces.into_iter().map(clockwise).collect())
}

fn intersect(s: Pt, e: Pt, p: Pt, q: Pt) -> Pt {
    let (dx, dy) = (e[0] - s[0], e[1] - s[1]);
    let (ex, ey) = (q[0] - p[0], q[1] - p[1]);
    let t = (ex * (s[1] - p[1]) - ey * (s[0] - p[0])) / (ey * dx - ex * dy);
    [s[0] + t * dx, s[1] + t * dy]
}

/// One Sutherland–Hodgman step: `input` kept inside edge `(p, q)`. Returns
/// whether any side was outside it.
fn clip_half(input: &[Pt], p: Pt, q: Pt, out: &mut Vec<Pt>) -> bool {
    let inside = |v: Pt| (q[0] - p[0]) * (v[1] - p[1]) - (q[1] - p[1]) * (v[0] - p[0]) < 0.0;
    out.clear();
    let mut cut = false;
    for k in 0..input.len() {
        let (s, e) = (input[k], input[(k + 1) % input.len()]);
        match (inside(s), inside(e)) {
            (true, true) => out.push(e),
            (true, false) => {
                out.push(intersect(s, e, p, q));
                cut = true;
            }
            (false, true) => {
                out.push(intersect(s, e, p, q));
                out.push(e);
                cut = true;
            }
            (false, false) => cut = true,
        }
    }
    cut
}

/// A triangle against a clockwise convex polygon: its points, and whether
/// any side was outside any edge.
fn clip_triangle(tri: &[Pt; 3], poly: &[Pt], points: &mut Vec<Pt>, scratch: &mut Vec<Pt>) -> bool {
    points.clear();
    points.extend_from_slice(tri);
    let mut cut = false;
    for i in 0..poly.len() {
        if points.is_empty() {
            break;
        }
        cut |= clip_half(points, poly[i], poly[(i + 1) % poly.len()], scratch);
        std::mem::swap(points, scratch);
    }
    cut
}

/// Buffers a cut reuses from piece to piece.
#[derive(Clone, Default)]
pub(crate) struct Scratch {
    points: Vec<Pt>,
    half: Vec<Pt>,
    remaining: Vec<Pt>,
    /// One triangle's parts: their points, one span per part.
    parts: Vec<Pt>,
    spans: Vec<(usize, usize, bool)>,
    /// Each convex region's bounds: min x, min y, max x, max y.
    bounds: Vec<[f64; 4]>,
    /// Per vertex of the piece and region, which of the region's edges it
    /// is inside, as `clip_half` tests it; for an inverse clip, also which
    /// hull edges it is outside, as the reversed edge tests it.
    inside: Vec<u64>,
    outside: Vec<u64>,
}

/// The bits of the edges of clockwise `poly` that `v` is inside, as
/// `clip_half(.., p, q, ..)` tests it.
fn inside_bits(poly: &[Pt], v: Pt) -> u64 {
    let n = poly.len();
    (0..n).fold(0, |bits, i| {
        let (p, q) = (poly[i], poly[(i + 1) % n]);
        let inside = (q[0] - p[0]) * (v[1] - p[1]) - (q[1] - p[1]) * (v[0] - p[0]) < 0.0;
        bits | (u64::from(inside) << i)
    })
}

/// The bits of the hull edges `v` is outside, as `clip_half(.., q, p, ..)`
/// tests it.
fn outside_bits(hull: &[Pt], v: Pt) -> u64 {
    let n = hull.len();
    (0..n).fold(0, |bits, i| {
        let (q, p) = (hull[i], hull[(i + 1) % n]);
        let outside = (q[0] - p[0]) * (v[1] - p[1]) - (q[1] - p[1]) * (v[0] - p[0]) < 0.0;
        bits | (u64::from(outside) << i)
    })
}

fn all_edges(n: usize) -> u64 {
    if n >= 64 {
        u64::MAX
    } else {
        (1 << n) - 1
    }
}

fn bounds(pts: &[Pt]) -> [f64; 4] {
    pts.iter().fold(
        [f64::MAX, f64::MAX, f64::MIN, f64::MIN],
        |[x0, y0, x1, y1], [x, y]| [x0.min(*x), y0.min(*y), x1.max(*x), y1.max(*y)],
    )
}

/// Whether a triangle's bounds and a region's are apart, so no point of the
/// triangle is inside every edge of the region: it is wholly cut away.
fn apart(a: &[f64; 4], b: &[f64; 4]) -> bool {
    a[2] < b[0] || b[2] < a[0] || a[3] < b[1] || b[3] < a[1]
}

/// An inverse clip's walk over a triangle, decided from its corners' bits
/// where the walk would not cut: the edges before the first one some corner
/// is not inside give nothing and leave it whole; if every corner is then
/// outside that edge, the whole triangle is that edge's part. Inside every
/// edge, it gives nothing at all. False where the walk must run.
fn inverse_shortcut(
    n: usize,
    idx: [usize; 3],
    inside: &[u64],
    outside: &[u64],
    part: &mut impl FnMut(&[Pt], bool),
    tri: &[Pt; 3],
) -> bool {
    let all = all_edges(n);
    let fwd = idx.map(|i| inside[i]);
    let rev = idx.map(|i| outside[i]);
    let every_in = fwd[0] & fwd[1] & fwd[2];
    let any_out = rev[0] | rev[1] | rev[2];
    // The edges before `first` are cleanly passed: every corner inside and
    // none outside.
    let clean = every_in & !any_out & all;
    let first = clean.trailing_ones() as usize;
    if first >= n {
        for _ in 0..n {
            part(&[], true);
        }
        return true;
    }
    let bit = 1u64 << first;
    if rev.iter().all(|r| r & bit != 0) && fwd.iter().all(|f| f & bit == 0) {
        for _ in 0..first {
            part(&[], true);
        }
        part(tri, false);
        return true;
    }
    false
}

/// Cuts a piece's triangles through `plan`, appending what is drawn to
/// `out`. Returns false, appending nothing, when the clip cut nothing: the
/// piece is drawn as it is.
pub(crate) fn cut(
    plan: &Plan,
    world: &[Pt],
    uvs: &[[f32; 2]],
    triangles: &[u32],
    scratch: &mut Scratch,
    out: Cut<'_>,
) -> bool {
    cut_walked(plan, world, uvs, triangles, scratch, out, true)
}

/// `cut`, deciding whole triangles from their corners' edge tests when
/// `decide` is set, else walking every triangle through every edge.
fn cut_walked(
    plan: &Plan,
    world: &[Pt],
    uvs: &[[f32; 2]],
    triangles: &[u32],
    scratch: &mut Scratch,
    out: Cut<'_>,
    decide: bool,
) -> bool {
    let (start_v, start_t) = (out.vertices.len(), out.triangles.len());
    let Scratch {
        points,
        half,
        remaining,
        parts,
        spans,
        bounds: regions,
        inside,
        outside,
    } = scratch;
    // Each vertex tested once against each edge: a triangle whose corners
    // are all inside every edge is not cut, and one whose corners are all
    // outside one edge is cut away, as the edge walk would find.
    let polys: &[Vec<Pt>] = match plan {
        Plan::Convex(poly) | Plan::Inverse(poly) => std::slice::from_ref(poly),
        Plan::Pieces(pieces) => pieces,
        Plan::Hide | Plan::Show => &[],
    };
    let tested = decide && polys.iter().all(|poly| poly.len() <= 64);
    regions.clear();
    regions.extend(polys.iter().map(|poly| bounds(poly)));
    inside.clear();
    outside.clear();
    if tested {
        for &v in world {
            inside.extend(polys.iter().map(|poly| inside_bits(poly, v)));
            if let Plan::Inverse(hull) = plan {
                outside.push(outside_bits(hull, v));
            }
        }
    }
    let regions_n = polys.len();
    let mut clipped = false;
    for t in triangles.as_chunks::<3>().0 {
        let idx = [t[0] as usize, t[1] as usize, t[2] as usize];
        let (Some(&a), Some(&b), Some(&c)) =
            (world.get(idx[0]), world.get(idx[1]), world.get(idx[2]))
        else {
            continue;
        };
        let tri = [a, b, c];
        let tri_bounds = bounds(&tri);
        // A corner's bits for region `r`: (inside every edge, outside some
        // one edge the other corners are also outside).
        let corners = |r: usize, n: usize| {
            let bits = idx.map(|i| inside[i * regions_n + r]);
            let all = all_edges(n);
            (
                bits.iter().all(|b| *b & all == all),
                (!bits[0] & !bits[1] & !bits[2]) & all != 0,
            )
        };
        parts.clear();
        spans.clear();
        let mut part = |points: &[Pt], cut: bool| {
            spans.push((parts.len(), parts.len() + points.len(), cut));
            parts.extend_from_slice(points);
        };
        match plan {
            Plan::Convex(poly) => {
                let (within, beyond) = if tested {
                    corners(0, poly.len())
                } else {
                    (false, false)
                };
                if within {
                    part(&tri, false);
                } else if beyond || apart(&tri_bounds, &regions[0]) {
                    part(&[], true);
                } else {
                    let cut = clip_triangle(&tri, poly, points, half);
                    part(points, cut);
                }
            }
            Plan::Pieces(pieces) => {
                for (r, (poly, region)) in pieces.iter().zip(regions.iter()).enumerate() {
                    let (within, beyond) = if tested {
                        corners(r, poly.len())
                    } else {
                        (false, false)
                    };
                    if within {
                        part(&tri, false);
                    } else if beyond || apart(&tri_bounds, region) {
                        part(&[], true);
                    } else {
                        let cut = clip_triangle(&tri, poly, points, half);
                        part(points, cut);
                    }
                }
            }
            Plan::Inverse(hull)
                if tested
                    && inverse_shortcut(hull.len(), idx, inside, outside, &mut part, &tri) =>
            {
                clipped = true;
            }
            Plan::Inverse(hull) => {
                // Outside each hull edge and inside every edge before it.
                remaining.clear();
                remaining.extend_from_slice(&tri);
                let mut cut_so_far = false;
                for i in 0..hull.len() {
                    if remaining.is_empty() {
                        break;
                    }
                    let (p, q) = (hull[i], hull[(i + 1) % hull.len()]);
                    let cut = clip_half(remaining, q, p, points) || cut_so_far;
                    part(if cut { points } else { &tri }, cut);
                    cut_so_far |= clip_half(remaining, p, q, half);
                    std::mem::swap(remaining, half);
                }
                clipped = true;
            }
            Plan::Hide => {
                clipped = true;
                continue;
            }
            Plan::Show => continue,
        }
        let tuv = idx.map(|i| uvs.get(i).copied().unwrap_or_default().map(f64::from));
        let d0 = b[1] - c[1];
        let d1 = c[0] - b[0];
        let d2 = a[0] - c[0];
        let d4 = c[1] - a[1];
        let d = 1.0 / (d0 * d2 + d1 * (a[1] - c[1]));
        for &(from, to, cut) in spans.iter() {
            clipped |= cut;
            let points = &parts[from..to];
            if points.len() < 3 {
                continue;
            }
            let base = (out.vertices.len() - start_v) as u32;
            if !cut {
                // A triangle wholly inside keeps its corners and their UVs.
                for (p, uv) in tri.iter().zip(&tuv) {
                    out.vertices.push([p[0] as f32, p[1] as f32]);
                    out.uvs.push([uv[0] as f32, uv[1] as f32]);
                }
            } else {
                for &[x, y] in points {
                    let wa = (d0 * (x - c[0]) + d1 * (y - c[1])) * d;
                    let wb = (d4 * (x - c[0]) + d2 * (y - c[1])) * d;
                    let wc = 1.0 - wa - wb;
                    out.vertices.push([x as f32, y as f32]);
                    out.uvs.push([
                        (tuv[0][0] * wa + tuv[1][0] * wb + tuv[2][0] * wc) as f32,
                        (tuv[0][1] * wa + tuv[1][1] * wb + tuv[2][1] * wc) as f32,
                    ]);
                }
            }
            for j in 1..points.len() as u32 - 1 {
                out.triangles.extend([base, base + j, base + j + 1]);
            }
        }
    }
    if !clipped {
        out.vertices.truncate(start_v);
        out.uvs.truncate(start_v);
        out.triangles.truncate(start_t);
    }
    clipped
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(vertices: &[[f32; 2]], triangles: &[u32]) -> f64 {
        triangles
            .as_chunks::<3>()
            .0
            .iter()
            .map(|t| {
                let [a, b, c] = [0, 1, 2].map(|k| vertices[t[k] as usize].map(f64::from));
                (cross3(a, b, c) / 2.0).abs()
            })
            .sum()
    }

    /// A 4×4 square of two triangles, cut by `polygon`: the drawn area.
    fn covered(polygon: &[f64], inverse: bool, convex: bool) -> Option<f64> {
        let world = [[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0]];
        let uvs = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];
        let (mut v, mut u, mut t) = (Vec::new(), Vec::new(), Vec::new());
        let plan = plan(polygon, inverse, convex);
        let cut = cut(
            &plan,
            &world,
            &uvs,
            &[0, 1, 2, 2, 3, 0],
            &mut Scratch::default(),
            Cut {
                vertices: &mut v,
                uvs: &mut u,
                triangles: &mut t,
            },
        );
        assert_eq!(v.len(), u.len());
        cut.then(|| area(&v, &t))
    }

    #[test]
    fn convex_concave_hull_and_inverse_cover_their_areas() {
        // Wholly inside a big square: not cut.
        assert_eq!(
            covered(&[-1.0, -1.0, 5.0, -1.0, 5.0, 5.0, -1.0, 5.0], false, false),
            None
        );
        // A 2×2 square, either winding.
        let small = [1.0, 1.0, 3.0, 1.0, 3.0, 3.0, 1.0, 3.0];
        assert!((covered(&small, false, false).unwrap() - 4.0).abs() < 1e-9);
        let reversed = [1.0, 3.0, 3.0, 3.0, 3.0, 1.0, 1.0, 1.0];
        assert!((covered(&reversed, false, false).unwrap() - 4.0).abs() < 1e-9);
        // An L of area 3 inside the square.
        let l = [1.0, 1.0, 3.0, 1.0, 3.0, 2.0, 2.0, 2.0, 2.0, 3.0, 1.0, 3.0];
        assert!((covered(&l, false, false).unwrap() - 3.0).abs() < 1e-9);
        // The same L as its hull, and inverse: everything outside the hull.
        assert!((covered(&l, false, true).unwrap() - 3.5).abs() < 1e-9);
        assert!((covered(&l, true, false).unwrap() - (16.0 - 3.5)).abs() < 1e-9);
        // A polygon crossing itself hides what it covers; inverse shows it.
        let bow = [1.0, 1.0, 3.0, 3.0, 3.0, 1.0, 1.0, 3.0];
        assert_eq!(covered(&bow, false, false), Some(0.0));
        assert_eq!(covered(&bow, true, false), None);
        // Collinear: a closed aperture hides everything.
        assert_eq!(
            covered(&[0.0, 2.0, 2.0, 2.0, 4.0, 2.0], false, false),
            Some(0.0)
        );
    }

    #[test]
    fn cut_uvs_are_barycentric_over_the_source_triangle() {
        let world = [[0.0, 0.0], [4.0, 0.0], [0.0, 4.0]];
        let uvs = [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]];
        let (mut v, mut u, mut t) = (Vec::new(), Vec::new(), Vec::new());
        let plan = plan(&[1.0, 1.0, 2.0, 1.0, 2.0, 2.0, 1.0, 2.0], false, false);
        assert!(cut(
            &plan,
            &world,
            &uvs,
            &[0, 1, 2],
            &mut Scratch::default(),
            Cut {
                vertices: &mut v,
                uvs: &mut u,
                triangles: &mut t,
            },
        ));
        for (p, uv) in v.iter().zip(&u) {
            assert!((p[0] / 4.0 - uv[0]).abs() < 1e-6 && (p[1] / 4.0 - uv[1]).abs() < 1e-6);
        }
    }

    /// Deciding whole triangles from their corners gives what walking every
    /// edge gives, on a jittered grid mesh under random convex, concave and
    /// inverse polygons.
    #[test]
    fn deciding_whole_triangles_changes_nothing() {
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        let mut rand = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed >> 11) as f64 / (1u64 << 53) as f64
        };
        let n = 12;
        let mut world = Vec::new();
        let mut uvs = Vec::new();
        for y in 0..=n {
            for x in 0..=n {
                world.push([
                    x as f64 * 10.0 + rand() * 4.0,
                    y as f64 * 10.0 + rand() * 4.0,
                ]);
                uvs.push([x as f32 / n as f32, y as f32 / n as f32]);
            }
        }
        let mut triangles = Vec::new();
        for y in 0..n {
            for x in 0..n {
                let i = (y * (n + 1) + x) as u32;
                let row = n as u32 + 1;
                triangles.extend([i, i + 1, i + row + 1, i + row + 1, i + row, i]);
            }
        }
        let mut decided_cases = 0;
        for case in 0..300 {
            let sides = 3 + case % 20;
            let (cx, cy, r) = (
                20.0 + rand() * 80.0,
                20.0 + rand() * 80.0,
                15.0 + rand() * 50.0,
            );
            let polygon: Vec<f64> = (0..sides)
                .flat_map(|i| {
                    let a = (i as f64 + rand() * 0.5) / sides as f64 * std::f64::consts::TAU;
                    // Every third polygon has a dent, so it is concave.
                    let k = if case % 3 == 1 && i % 2 == 1 {
                        0.5
                    } else {
                        1.0
                    };
                    [cx + r * k * a.cos(), cy + r * k * a.sin()]
                })
                .collect();
            let plan = plan(&polygon, case % 3 == 2, false);
            let run = |decide: bool| {
                let (mut v, mut u, mut t) = (Vec::new(), Vec::new(), Vec::new());
                let cut = cut_walked(
                    &plan,
                    &world,
                    &uvs,
                    &triangles,
                    &mut Scratch::default(),
                    Cut {
                        vertices: &mut v,
                        uvs: &mut u,
                        triangles: &mut t,
                    },
                    decide,
                );
                (cut, v, u, t)
            };
            let walked = run(false);
            assert_eq!(run(true), walked, "case {case}");
            decided_cases += usize::from(walked.0);
        }
        assert!(decided_cases > 250, "{decided_cases} cases cut");
    }
}
