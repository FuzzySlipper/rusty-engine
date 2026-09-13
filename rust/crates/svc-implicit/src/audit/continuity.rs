//! Explicit, bounded authoring queries over captured extracted geometry.
use super::*;
use std::collections::VecDeque;

#[derive(Clone, Copy, Debug)]
pub struct OpenRegion {
    pub piece_id: u64,
    pub bounds: Bounds,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum AnalysisClassification {
    OpenBoundary,
    IntentionalBoundary,
    NonManifold,
    DegenerateTriangle,
    JoinGap,
    MissingJoinSurface,
    EnclosureLeak,
    IntentionalOpening,
    IncompleteCoverage,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AnalysisDiagnostic {
    pub piece_a: u64,
    pub piece_b: u64,
    pub classification: AnalysisClassification,
    pub bounds: Bounds,
    pub approximate_width: f64,
    pub approximate_length: f64,
    pub approximate_area: f64,
    pub resolution: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AnalysisReport {
    pub diagnostics: Vec<AnalysisDiagnostic>,
    pub sampled: u64,
    /// The declared discrete query completed. Never a sub-resolution guarantee.
    pub complete: bool,
    pub resolution: f32,
    /// One unintended interior-to-outside route, including its exit endpoint.
    pub path: Vec<[f32; 3]>,
}
impl Default for AnalysisReport {
    fn default() -> Self {
        Self {
            diagnostics: vec![],
            sampled: 0,
            complete: true,
            resolution: 0.0,
            path: vec![],
        }
    }
}

/// A rectangular expected contact patch. Half axes may be rotated, but must be
/// perpendicular. Rays normal to this patch search for each named mesh surface.
#[derive(Clone, Copy, Debug)]
pub struct Join {
    pub piece_a: u64,
    pub piece_b: u64,
    pub center: [f32; 3],
    pub half_u: [f32; 3],
    pub half_v: [f32; 3],
    pub search_distance: f32,
    pub tolerance_cells: f32,
    pub sample_spacing: f32,
    pub max_samples: u64,
}

/// A bounded world-space enclosure domain and one known interior seed. Moving
/// doors must be captured in the intended pose. Intentional opening volumes
/// act as virtual caps; other routes to the domain boundary remain reportable.
#[derive(Clone, Debug)]
pub struct Enclosure {
    pub bounds: Bounds,
    pub interior: [f32; 3],
    pub openings: Vec<Bounds>,
    pub sample_spacing: f32,
    pub max_samples: u64,
}

fn valid_bounds(bounds: Bounds) -> bool {
    (0..3).all(|a| {
        bounds.min[a].is_finite() && bounds.max[a].is_finite() && bounds.min[a] < bounds.max[a]
    })
}
fn contains(bounds: Bounds, p: [f64; 3]) -> bool {
    (0..3).all(|a| p[a] >= f64::from(bounds.min[a]) && p[a] <= f64::from(bounds.max[a]))
}
fn v3(p: [f32; 3]) -> Vector3<f64> {
    Vector3::from(p.map(f64::from))
}
fn arr(p: Vector3<f64>) -> [f32; 3] {
    [p.x as f32, p.y as f32, p.z as f32]
}
fn diagnostic(
    classification: AnalysisClassification,
    bounds: Bounds,
    resolution: f32,
) -> AnalysisDiagnostic {
    AnalysisDiagnostic {
        piece_a: 0,
        piece_b: 0,
        classification,
        bounds,
        approximate_width: 0.0,
        approximate_length: 0.0,
        approximate_area: 0.0,
        resolution,
    }
}
fn incomplete(bounds: Bounds, resolution: f32) -> AnalysisReport {
    AnalysisReport {
        complete: false,
        resolution,
        diagnostics: vec![diagnostic(
            AnalysisClassification::IncompleteCoverage,
            bounds,
            resolution,
        )],
        ..Default::default()
    }
}

/// Ray/triangle intersection parameter; double-sided, including shared edges.
fn intersection(
    origin: Vector3<f64>,
    direction: Vector3<f64>,
    points: [[f32; 3]; 3],
) -> Option<f64> {
    let [a, b, c] = points.map(v3);
    let e1 = b - a;
    let e2 = c - a;
    let h = direction.cross(&e2);
    let determinant = e1.dot(&h);
    if determinant.abs() <= 1e-12 * e1.norm() * e2.norm() * direction.norm() {
        return None;
    }
    let inv = determinant.recip();
    let s = origin - a;
    let u = inv * s.dot(&h);
    let q = s.cross(&e1);
    let v = inv * direction.dot(&q);
    if u < -1e-8 || v < -1e-8 || u + v > 1.0 + 1e-8 {
        return None;
    }
    Some(inv * e2.dot(&q))
}

fn nearest(
    piece: &Piece,
    origin: Vector3<f64>,
    direction: Vector3<f64>,
    radius: f64,
) -> Option<f64> {
    let ends = [
        arr(origin - direction * radius),
        arr(origin + direction * radius),
    ];
    let ray_bounds = bounds_for_positions(&ends).ok()?;
    piece
        .triangles
        .iter()
        .zip(&piece.triangle_bounds)
        .filter(|(_, bounds)| bounds_overlap(ray_bounds, **bounds, 1e-7))
        .filter_map(|(triangle, _)| {
            intersection(
                origin,
                direction,
                triangle.map(|i| piece.positions[i as usize]),
            )
        })
        .filter(|t| t.abs() <= radius)
        .min_by(|a, b| a.abs().total_cmp(&b.abs()))
}

// Parity uses extracted closed geometry, independent of field approximation or
// triangle winding. Three non-axis-aligned rays expose ambiguous edge/tangent
// cases instead of silently accepting inconsistent containment evidence.
fn inside_mesh(piece: &Piece, point: Vector3<f64>) -> Option<bool> {
    let epsilon = f64::EPSILON * point.norm().max(1.0) * 128.0;
    let mut votes = Vec::new();
    for direction in [
        [1.0, 0.371, 0.129],
        [-0.193, 1.0, 0.417],
        [0.233, -0.337, 1.0],
    ] {
        let mut hits = piece
            .triangles
            .iter()
            .filter_map(|triangle| {
                intersection(
                    point,
                    Vector3::from(direction),
                    triangle.map(|i| piece.positions[i as usize]),
                )
            })
            .filter(|t| *t > epsilon)
            .collect::<Vec<_>>();
        hits.sort_by(f64::total_cmp);
        hits.dedup_by(|a, b| (*a - *b).abs() <= epsilon);
        votes.push(hits.len() % 2 == 1);
    }
    votes.iter().all(|v| *v == votes[0]).then_some(votes[0])
}

pub fn expected_join(pieces: &[Arc<Piece>], join: Join) -> Result<AnalysisReport, Error> {
    validate_spacing(join.sample_spacing)?;
    validate_spacing(join.search_distance)?;
    validate_spacing(join.tolerance_cells)?;
    if join
        .center
        .iter()
        .chain(join.half_u.iter())
        .chain(join.half_v.iter())
        .any(|v| !v.is_finite())
        || join.piece_a == join.piece_b
    {
        return Err(Error(
            "join needs finite axes and two distinct pieces".into(),
        ));
    }
    let a = pieces
        .iter()
        .find(|p| p.id == join.piece_a)
        .ok_or_else(|| Error("unknown join piece A".into()))?;
    let b = pieces
        .iter()
        .find(|p| p.id == join.piece_b)
        .ok_or_else(|| Error("unknown join piece B".into()))?;
    let u = v3(join.half_u);
    let v = v3(join.half_v);
    let center = v3(join.center);
    let lengths = [u.norm() * 2.0, v.norm() * 2.0];
    if lengths.iter().any(|v| *v <= 0.0) || u.dot(&v).abs() > 1e-5 * u.norm() * v.norm() {
        return Err(Error(
            "join patch needs nonzero perpendicular half axes".into(),
        ));
    }
    let normal = u.cross(&v).normalize();
    let corners = [
        arr(center - u - v),
        arr(center - u + v),
        arr(center + u - v),
        arr(center + u + v),
    ];
    let bounds = bounds_for_positions(&corners)?;
    let counts =
        lengths.map(|length| (length / f64::from(join.sample_spacing)).ceil().max(1.0) as u64);
    let Some(count) = counts[0]
        .checked_mul(counts[1])
        .filter(|n| *n <= join.max_samples)
    else {
        return Ok(incomplete(
            bounds,
            join.sample_spacing
                .max(a.sample_spacing)
                .max(b.sample_spacing),
        ));
    };
    let tolerance =
        f64::from(a.sample_spacing.max(b.sample_spacing)) * f64::from(join.tolerance_cells);
    let resolution = join
        .sample_spacing
        .max(a.sample_spacing)
        .max(b.sample_spacing);
    let mut report = AnalysisReport {
        resolution,
        ..Default::default()
    };
    let topology = super::integrity::inspect(&[Arc::clone(a), Arc::clone(b)], &[])?;
    let closed_a = !topology.diagnostics.iter().any(|d| d.piece_a == a.id);
    let closed_b = !topology.diagnostics.iter().any(|d| d.piece_a == b.id);
    let mut groups: BTreeMap<AnalysisClassification, AnalysisDiagnostic> = BTreeMap::new();
    for i in 0..counts[0] {
        for j in 0..counts[1] {
            let p = center
                + u * (2.0 * (i as f64 + 0.5) / counts[0] as f64 - 1.0)
                + v * (2.0 * (j as f64 + 0.5) / counts[1] as f64 - 1.0);
            let left = nearest(a, p, normal, f64::from(join.search_distance));
            let right = nearest(b, p, normal, f64::from(join.search_distance));
            report.sampled += 1;
            let (class, width) = match (left, right) {
                (Some(l), Some(r)) if (l - r).abs() > tolerance => {
                    let midpoint = p + normal * ((l + r) * 0.5);
                    let inside_a = if closed_a {
                        inside_mesh(a, midpoint)
                    } else {
                        Some(false)
                    };
                    let inside_b = if closed_b {
                        inside_mesh(b, midpoint)
                    } else {
                        Some(false)
                    };
                    match (inside_a, inside_b) {
                        (Some(true), _) | (_, Some(true)) => continue,
                        (Some(false), Some(false)) => {
                            (AnalysisClassification::JoinGap, (l - r).abs())
                        }
                        _ => {
                            report.complete = false;
                            (AnalysisClassification::IncompleteCoverage, (l - r).abs())
                        }
                    }
                }
                (Some(_), Some(_)) => continue,
                _ => (AnalysisClassification::MissingJoinSurface, 0.0),
            };
            let patch = bounds_for_positions(&[
                arr(p - u / counts[0] as f64 - v / counts[1] as f64),
                arr(p + u / counts[0] as f64 + v / counts[1] as f64),
                arr(p - u / counts[0] as f64 + v / counts[1] as f64),
                arr(p + u / counts[0] as f64 - v / counts[1] as f64),
            ])?;
            let d = groups.entry(class).or_insert_with(|| {
                let mut d = diagnostic(class, patch, resolution);
                d.piece_a = a.id;
                d.piece_b = b.id;
                d
            });
            for axis in 0..3 {
                d.bounds.min[axis] = d.bounds.min[axis].min(patch.min[axis]);
                d.bounds.max[axis] = d.bounds.max[axis].max(patch.max[axis]);
            }
            d.approximate_width = d.approximate_width.max(width);
            d.approximate_area += lengths[0] * lengths[1] / count as f64;
        }
    }
    report.diagnostics = groups.into_values().collect();
    Ok(report)
}

// Slab segment/AABB intersection, used for intentional-opening caps and BVH.
fn segment_bounds(a: Vector3<f64>, b: Vector3<f64>, bounds: Bounds) -> bool {
    let mut lo: f64 = 0.0;
    let mut hi: f64 = 1.0;
    for axis in 0..3 {
        let delta = b[axis] - a[axis];
        if delta.abs() < 1e-15 {
            if a[axis] < f64::from(bounds.min[axis]) - 1e-7
                || a[axis] > f64::from(bounds.max[axis]) + 1e-7
            {
                return false;
            }
        } else {
            let mut t0 = (f64::from(bounds.min[axis]) - a[axis]) / delta;
            let mut t1 = (f64::from(bounds.max[axis]) - a[axis]) / delta;
            if t0 > t1 {
                std::mem::swap(&mut t0, &mut t1);
            }
            lo = lo.max(t0);
            hi = hi.min(t1);
            if lo > hi + 1e-8 {
                return false;
            }
        }
    }
    true
}

#[derive(Clone)]
struct Triangle {
    points: [[f32; 3]; 3],
    bounds: Bounds,
    piece: u64,
}
struct Tree {
    bounds: Bounds,
    kind: TreeKind,
}
enum TreeKind {
    Leaf(Vec<Triangle>),
    Split(Box<Tree>, Box<Tree>),
}
impl Tree {
    fn build(mut triangles: Vec<Triangle>) -> Option<Self> {
        let first = triangles.first()?;
        let mut bounds = first.bounds;
        for triangle in &triangles {
            for axis in 0..3 {
                bounds.min[axis] = bounds.min[axis].min(triangle.bounds.min[axis]);
                bounds.max[axis] = bounds.max[axis].max(triangle.bounds.max[axis]);
            }
        }
        let kind = if triangles.len() <= 8 {
            TreeKind::Leaf(triangles)
        } else {
            let axis = (0..3)
                .max_by(|&a, &b| {
                    (bounds.max[a] - bounds.min[a]).total_cmp(&(bounds.max[b] - bounds.min[b]))
                })
                .expect("three axes");
            triangles.sort_by(|a, b| {
                (f64::from(a.bounds.min[axis]) + f64::from(a.bounds.max[axis]))
                    .total_cmp(&(f64::from(b.bounds.min[axis]) + f64::from(b.bounds.max[axis])))
            });
            let right = triangles.split_off(triangles.len() / 2);
            TreeKind::Split(
                Box::new(Self::build(triangles).expect("left")),
                Box::new(Self::build(right).expect("right")),
            )
        };
        Some(Self { bounds, kind })
    }
    fn hit(&self, a: Vector3<f64>, b: Vector3<f64>) -> Option<u64> {
        if !segment_bounds(a, b, self.bounds) {
            return None;
        }
        match &self.kind {
            TreeKind::Leaf(triangles) => triangles
                .iter()
                .find(|t| {
                    intersection(a, b - a, t.points)
                        .is_some_and(|t| (-1e-8..=1.0 + 1e-8).contains(&t))
                })
                .map(|t| t.piece),
            TreeKind::Split(l, r) => l.hit(a, b).or_else(|| r.hit(a, b)),
        }
    }
}

pub fn enclosure(pieces: &[Arc<Piece>], query: &Enclosure) -> Result<AnalysisReport, Error> {
    validate_spacing(query.sample_spacing)?;
    if !valid_bounds(query.bounds)
        || !contains(query.bounds, query.interior.map(f64::from))
        || query.interior.iter().any(|x| !x.is_finite())
        || query.openings.iter().any(|b| !valid_bounds(*b))
    {
        return Err(Error(
            "enclosure needs finite positive bounds, interior seed and opening volumes".into(),
        ));
    }
    let counts = std::array::from_fn::<_, 3, _>(|a| {
        ((f64::from(query.bounds.max[a]) - f64::from(query.bounds.min[a]))
            / f64::from(query.sample_spacing))
        .ceil()
        .max(1.0) as usize
    });
    let count = counts.iter().try_fold(1usize, |v, n| v.checked_mul(*n));
    let Some(count) = count.filter(|n| *n as u64 <= query.max_samples && *n <= isize::MAX as usize)
    else {
        return Ok(incomplete(
            query.bounds,
            pieces
                .iter()
                .map(|p| p.sample_spacing)
                .fold(query.sample_spacing, f32::max),
        ));
    };
    let spacing = std::array::from_fn::<_, 3, _>(|a| {
        (f64::from(query.bounds.max[a]) - f64::from(query.bounds.min[a])) / counts[a] as f64
    });
    let location = |cell: [usize; 3]| {
        Vector3::from(std::array::from_fn::<_, 3, _>(|a| {
            f64::from(query.bounds.min[a]) + (cell[a] as f64 + 0.5) * spacing[a]
        }))
    };
    let index = |cell: [usize; 3]| cell[0] + counts[0] * (cell[1] + counts[1] * cell[2]);
    let cell = |id: usize| {
        [
            id % counts[0],
            (id / counts[0]) % counts[1],
            id / (counts[0] * counts[1]),
        ]
    };
    let seed = std::array::from_fn::<_, 3, _>(|a| {
        (((f64::from(query.interior[a]) - f64::from(query.bounds.min[a])) / spacing[a]).floor()
            as usize)
            .min(counts[a] - 1)
    });
    let triangles = pieces
        .iter()
        .flat_map(|p| {
            p.triangles
                .iter()
                .zip(&p.triangle_bounds)
                .map(|(t, b)| Triangle {
                    points: t.map(|i| p.positions[i as usize]),
                    bounds: *b,
                    piece: p.id,
                })
        })
        .collect();
    let tree = Tree::build(triangles);
    let blocked = |a, b| tree.as_ref().and_then(|t| t.hit(a, b));
    let resolution = pieces
        .iter()
        .map(|p| p.sample_spacing)
        .fold(query.sample_spacing, f32::max);
    // Do not silently move an author's seed through a wall to its grid cell.
    if blocked(v3(query.interior), location(seed)).is_some()
        || query
            .openings
            .iter()
            .any(|b| contains(*b, query.interior.map(f64::from)))
    {
        return Ok(incomplete(query.bounds, resolution));
    }
    let mut report = AnalysisReport {
        resolution,
        ..Default::default()
    };
    let mut parent = vec![usize::MAX; count];
    let start = index(seed);
    parent[start] = start;
    let mut queue = VecDeque::from([start]);
    let mut touched = vec![false; query.openings.len()];
    'search: while let Some(current) = queue.pop_front() {
        report.sampled += 1;
        let here = cell(current);
        let a = location(here);
        for axis in 0..3 {
            for sign in [-1isize, 1] {
                let coordinate = here[axis] as isize + sign;
                let outside = coordinate < 0 || coordinate >= counts[axis] as isize;
                let mut next = here;
                let b = if outside {
                    let mut b = a;
                    b[axis] = f64::from(if sign < 0 {
                        query.bounds.min[axis]
                    } else {
                        query.bounds.max[axis]
                    });
                    b
                } else {
                    next[axis] = coordinate as usize;
                    location(next)
                };
                if !outside && parent[index(next)] != usize::MAX {
                    continue;
                }
                if blocked(a, b).is_some() {
                    continue;
                }
                if let Some(opening) = query
                    .openings
                    .iter()
                    .position(|bounds| segment_bounds(a, b, *bounds))
                {
                    touched[opening] = true;
                    continue;
                }
                if outside {
                    let mut cursor = current;
                    loop {
                        report.path.push(arr(location(cell(cursor))));
                        if cursor == start {
                            break;
                        }
                        cursor = parent[cursor];
                    }
                    report.path.reverse();
                    report.path.insert(0, query.interior);
                    report.path.push(arr(b));
                    let mut d = diagnostic(
                        AnalysisClassification::EnclosureLeak,
                        bounds_for_positions(&[arr(a), arr(b)])?,
                        resolution,
                    );
                    d.approximate_length = report
                        .path
                        .windows(2)
                        .map(|pair| (v3(pair[1]) - v3(pair[0])).norm())
                        .sum();
                    d.approximate_width = f64::from(query.sample_spacing);
                    report.diagnostics.push(d);
                    break 'search;
                }
                let n = index(next);
                parent[n] = current;
                queue.push_back(n);
            }
        }
    }
    for (i, seen) in touched.into_iter().enumerate() {
        if seen {
            report.diagnostics.push(diagnostic(
                AnalysisClassification::IntentionalOpening,
                query.openings[i],
                resolution,
            ));
        }
    }
    Ok(report)
}
