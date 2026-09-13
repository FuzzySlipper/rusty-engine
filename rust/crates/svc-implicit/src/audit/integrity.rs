//! Exact mesh-connectivity checks for the optional implicit geometry audit.
//!
//! This is intentionally topology-first.  Positions that compare exactly equal
//! (including duplicated export-seam vertices) share a connectivity key, while
//! nearby positions never weld a gap away.  The reported widths and areas are
//! therefore evidence at the extraction resolution, not a mesh-repair policy.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::Arc,
};

use crate::{Bounds, Error};

use super::{AnalysisClassification, AnalysisDiagnostic, AnalysisReport, OpenRegion, Piece};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct VertexKey([u32; 3]);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct EdgeKey(VertexKey, VertexKey);

impl EdgeKey {
    fn new(a: VertexKey, b: VertexKey) -> Self {
        if a <= b {
            Self(a, b)
        } else {
            Self(b, a)
        }
    }
}

#[derive(Clone, Copy)]
struct EdgeEvidence {
    a: [f32; 3],
    b: [f32; 3],
}

impl EdgeEvidence {
    fn bounds(self) -> Bounds {
        Bounds {
            min: std::array::from_fn(|axis| self.a[axis].min(self.b[axis])),
            max: std::array::from_fn(|axis| self.a[axis].max(self.b[axis])),
        }
    }

    fn length(self) -> f64 {
        self.a
            .iter()
            .zip(self.b)
            .map(|(a, b)| (*a as f64 - b as f64).powi(2))
            .sum::<f64>()
            .sqrt()
    }
}

#[derive(Clone, Copy)]
struct Aggregate {
    bounds: Bounds,
    length: f64,
    area: f64,
    width: f64,
    resolution: f32,
}

impl Aggregate {
    fn edge(edge: EdgeEvidence, resolution: f32) -> Self {
        Self {
            bounds: edge.bounds(),
            length: edge.length(),
            area: 0.0,
            width: 0.0,
            resolution,
        }
    }

    fn triangle(points: [[f32; 3]; 3], resolution: f32) -> Self {
        let bounds = bounds_for_points(&points);
        let [a, b, c] = points;
        let area = triangle_area(a, b, c);
        Self {
            bounds,
            length: 0.0,
            area,
            width: 0.0,
            resolution,
        }
    }

    fn merge(&mut self, other: Self) {
        self.bounds = union_bounds(self.bounds, other.bounds);
        self.length += other.length;
        self.area += other.area;
        self.width = self.width.max(other.width);
        self.resolution = self.resolution.max(other.resolution);
    }
}

/// Inspects mesh topology without welding or modifying it.
///
/// `OpenRegion` is an explicit world-space exception for a single stable piece
/// id.  It suppresses a boundary diagnostic only when the whole boundary edge
/// lies inside the declared AABB.  A midpoint inside an opening is insufficient.
/// Exact duplicate positions share a vertex key so exporters may split seams
/// without creating artificial boundary edges; near-coincident positions remain
/// independent evidence.
pub fn inspect(pieces: &[Arc<Piece>], openings: &[OpenRegion]) -> Result<AnalysisReport, Error> {
    let mut ordered: Vec<_> = pieces.iter().map(Arc::as_ref).collect();
    ordered.sort_by_key(|piece| piece.id);
    for pair in ordered.windows(2) {
        if pair[0].id == pair[1].id {
            return Err(Error(format!("duplicate audit piece id {}", pair[0].id)));
        }
    }

    let known = ordered
        .iter()
        .map(|piece| piece.id)
        .collect::<BTreeSet<_>>();
    let mut opening_bounds = BTreeMap::<u64, Vec<Bounds>>::new();
    for opening in openings {
        if !known.contains(&opening.piece_id) {
            return Err(Error(format!(
                "open region names unknown audit piece {}",
                opening.piece_id
            )));
        }
        opening.bounds.validate()?;
        opening_bounds
            .entry(opening.piece_id)
            .or_default()
            .push(opening.bounds);
    }
    for bounds in opening_bounds.values_mut() {
        bounds.sort_by(bounds_order);
        bounds.dedup();
    }

    let mut report = AnalysisReport {
        resolution: ordered
            .iter()
            .map(|piece| piece.sample_spacing)
            .fold(0.0_f32, f32::max),
        ..Default::default()
    };
    let mut diagnostics = BTreeMap::<(u64, u64, AnalysisClassification), Aggregate>::new();
    for piece in ordered {
        report.sampled =
            report
                .sampled
                .checked_add(u64::try_from(piece.triangles.len()).map_err(|_| {
                    Error("audit triangle sample count is not representable".into())
                })?)
                .ok_or_else(|| Error("audit triangle sample count overflowed".into()))?;
        inspect_piece(
            piece,
            opening_bounds
                .get(&piece.id)
                .map(Vec::as_slice)
                .unwrap_or(&[]),
            &mut diagnostics,
        );
    }

    report.diagnostics = diagnostics
        .into_iter()
        .map(
            |((piece_a, piece_b, classification), evidence)| AnalysisDiagnostic {
                piece_a,
                piece_b,
                classification,
                bounds: evidence.bounds,
                approximate_width: evidence.width,
                approximate_length: evidence.length,
                approximate_area: evidence.area,
                resolution: evidence.resolution,
            },
        )
        .collect();
    Ok(report)
}

fn inspect_piece(
    piece: &Piece,
    openings: &[Bounds],
    diagnostics: &mut BTreeMap<(u64, u64, AnalysisClassification), Aggregate>,
) {
    let vertices = piece
        .positions
        .iter()
        .copied()
        .map(|point| (vertex_key(point), point))
        .collect::<BTreeMap<_, _>>();
    let mut edges = BTreeMap::<EdgeKey, u32>::new();
    let mut fans = BTreeMap::<VertexKey, Vec<(VertexKey, VertexKey)>>::new();

    for triangle in piece.triangles.iter().copied() {
        let points = triangle.map(|index| piece.positions[index as usize]);
        if triangle_area(points[0], points[1], points[2]) == 0.0 {
            accumulate(
                diagnostics,
                piece,
                AnalysisClassification::DegenerateTriangle,
                Aggregate::triangle(points, piece.sample_spacing),
            );
            continue;
        }
        let vertices = points.map(vertex_key);
        for (left, right) in [(0, 1), (1, 2), (2, 0)] {
            let incident = edges
                .entry(EdgeKey::new(vertices[left], vertices[right]))
                .or_default();
            *incident = incident.saturating_add(1);
        }
        for vertex in 0..3 {
            let other = match vertex {
                0 => (1, 2),
                1 => (2, 0),
                2 => (0, 1),
                _ => unreachable!(),
            };
            fans.entry(vertices[vertex])
                .or_default()
                .push((vertices[other.0], vertices[other.1]));
        }
    }

    for (edge, incidents) in &edges {
        let evidence = edge_evidence(&vertices, *edge);
        match *incidents {
            1 if whole_edge_in_opening(evidence, openings) => accumulate(
                diagnostics,
                piece,
                AnalysisClassification::IntentionalBoundary,
                Aggregate::edge(evidence, piece.sample_spacing),
            ),
            1 => accumulate(
                diagnostics,
                piece,
                AnalysisClassification::OpenBoundary,
                Aggregate::edge(evidence, piece.sample_spacing),
            ),
            0..=2 => {}
            _ => accumulate(
                diagnostics,
                piece,
                AnalysisClassification::NonManifold,
                Aggregate::edge(evidence, piece.sample_spacing),
            ),
        }
    }

    for (vertex, incidents) in fans {
        if disconnected_fans(&incidents) {
            let point = vertices[&vertex];
            accumulate(
                diagnostics,
                piece,
                AnalysisClassification::NonManifold,
                Aggregate {
                    bounds: Bounds {
                        min: point,
                        max: point,
                    },
                    length: 0.0,
                    area: 0.0,
                    width: 0.0,
                    resolution: piece.sample_spacing,
                },
            );
        }
    }
}

fn accumulate(
    diagnostics: &mut BTreeMap<(u64, u64, AnalysisClassification), Aggregate>,
    piece: &Piece,
    classification: AnalysisClassification,
    evidence: Aggregate,
) {
    let key = (piece.id, piece.id, classification);
    match diagnostics.get_mut(&key) {
        Some(existing) => existing.merge(evidence),
        None => {
            diagnostics.insert(key, evidence);
        }
    }
}

fn disconnected_fans(incidents: &[(VertexKey, VertexKey)]) -> bool {
    if incidents.len() < 2 {
        return false;
    }
    let mut neighbors = vec![Vec::new(); incidents.len()];
    for left in 0..incidents.len() {
        for right in left + 1..incidents.len() {
            let (left_a, left_b) = incidents[left];
            let (right_a, right_b) = incidents[right];
            if left_a == right_a || left_a == right_b || left_b == right_a || left_b == right_b {
                neighbors[left].push(right);
                neighbors[right].push(left);
            }
        }
    }
    let mut visited = vec![false; incidents.len()];
    let mut components = 0;
    for start in 0..incidents.len() {
        if visited[start] {
            continue;
        }
        components += 1;
        if components > 1 {
            return true;
        }
        let mut queue = VecDeque::from([start]);
        visited[start] = true;
        while let Some(current) = queue.pop_front() {
            for &next in &neighbors[current] {
                if !visited[next] {
                    visited[next] = true;
                    queue.push_back(next);
                }
            }
        }
    }
    false
}

fn edge_evidence(vertices: &BTreeMap<VertexKey, [f32; 3]>, edge: EdgeKey) -> EdgeEvidence {
    EdgeEvidence {
        a: vertices[&edge.0],
        b: vertices[&edge.1],
    }
}

fn vertex_key(point: [f32; 3]) -> VertexKey {
    VertexKey(point.map(|value| if value == 0.0 { 0 } else { value.to_bits() }))
}

fn whole_edge_in_opening(edge: EdgeEvidence, openings: &[Bounds]) -> bool {
    openings
        .iter()
        .copied()
        .any(|opening| point_in_bounds(edge.a, opening) && point_in_bounds(edge.b, opening))
}

fn point_in_bounds(point: [f32; 3], bounds: Bounds) -> bool {
    (0..3).all(|axis| point[axis] >= bounds.min[axis] && point[axis] <= bounds.max[axis])
}

fn triangle_area(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> f64 {
    let ab = sub(b, a);
    let ac = sub(c, a);
    let cross = [
        ab[1] * ac[2] - ab[2] * ac[1],
        ab[2] * ac[0] - ab[0] * ac[2],
        ab[0] * ac[1] - ab[1] * ac[0],
    ];
    0.5 * cross.iter().map(|value| value * value).sum::<f64>().sqrt()
}

fn sub(left: [f32; 3], right: [f32; 3]) -> [f64; 3] {
    std::array::from_fn(|axis| left[axis] as f64 - right[axis] as f64)
}

fn bounds_for_points(points: &[[f32; 3]; 3]) -> Bounds {
    Bounds {
        min: std::array::from_fn(|axis| {
            points
                .iter()
                .map(|point| point[axis])
                .fold(f32::INFINITY, f32::min)
        }),
        max: std::array::from_fn(|axis| {
            points
                .iter()
                .map(|point| point[axis])
                .fold(f32::NEG_INFINITY, f32::max)
        }),
    }
}

fn union_bounds(left: Bounds, right: Bounds) -> Bounds {
    Bounds {
        min: std::array::from_fn(|axis| left.min[axis].min(right.min[axis])),
        max: std::array::from_fn(|axis| left.max[axis].max(right.max[axis])),
    }
}

fn bounds_order(left: &Bounds, right: &Bounds) -> std::cmp::Ordering {
    left.min
        .iter()
        .chain(left.max.iter())
        .zip(right.min.iter().chain(right.max.iter()))
        .map(|(a, b)| a.total_cmp(b))
        .find(|order| !order.is_eq())
        .unwrap_or(std::cmp::Ordering::Equal)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Field, Node};

    fn piece(id: u64, positions: Vec<[f32; 3]>, triangles: Vec<[u32; 3]>) -> Arc<Piece> {
        let mut field = Field::new();
        let node: Node = field
            .box_shape(Bounds {
                min: [-2.0, -2.0, -2.0],
                max: [2.0, 2.0, 2.0],
            })
            .unwrap();
        Arc::new(
            Piece::new(
                id,
                &field,
                node,
                positions,
                triangles,
                [
                    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
                ],
                0.25,
            )
            .unwrap(),
        )
    }

    fn tetrahedron_without_face(id: u64) -> Arc<Piece> {
        piece(
            id,
            vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 0.0, 1.0],
            ],
            vec![[0, 1, 3], [1, 2, 3], [2, 0, 3]],
        )
    }

    fn has(report: &AnalysisReport, classification: AnalysisClassification) -> bool {
        report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.classification == classification)
    }

    #[test]
    fn reports_missing_tetrahedron_face_as_open_boundary() {
        let report = inspect(&[tetrahedron_without_face(7)], &[]).unwrap();
        assert_eq!(report.sampled, 3);
        assert!(report.complete);
        assert!(has(&report, AnalysisClassification::OpenBoundary));
        assert!(!has(&report, AnalysisClassification::IntentionalBoundary));
    }

    #[test]
    fn declared_whole_edge_opening_reclassifies_boundary() {
        let report = inspect(
            &[tetrahedron_without_face(7)],
            &[OpenRegion {
                piece_id: 7,
                bounds: Bounds {
                    min: [-0.1, -0.1, -0.1],
                    max: [1.1, 1.1, 0.1],
                },
            }],
        )
        .unwrap();
        assert!(has(&report, AnalysisClassification::IntentionalBoundary));
        assert!(!has(&report, AnalysisClassification::OpenBoundary));
    }

    #[test]
    fn opening_requires_the_exact_piece_and_whole_edge_coverage() {
        let mesh = tetrahedron_without_face(7);
        let partial = OpenRegion {
            piece_id: 7,
            bounds: Bounds {
                min: [0.4, -0.1, -0.1],
                max: [0.6, 1.1, 0.1],
            },
        };
        let report = inspect(std::slice::from_ref(&mesh), &[partial]).unwrap();
        assert!(has(&report, AnalysisClassification::OpenBoundary));
        assert!(!has(&report, AnalysisClassification::IntentionalBoundary));
        assert!(inspect(
            std::slice::from_ref(&mesh),
            &[OpenRegion {
                piece_id: 8,
                bounds: partial.bounds
            }],
        )
        .unwrap_err()
        .to_string()
        .contains("unknown audit piece 8"));
        assert!(inspect(
            &[mesh],
            &[OpenRegion {
                piece_id: 7,
                bounds: Bounds {
                    min: [0.0, 0.0, 0.0],
                    max: [1.0, 1.0, 0.0]
                },
            }],
        )
        .unwrap_err()
        .to_string()
        .contains("bounds"));
    }

    #[test]
    fn reports_three_faces_on_one_edge_as_non_manifold() {
        let report = inspect(
            &[piece(
                9,
                vec![
                    [0.0, 0.0, 0.0],
                    [1.0, 0.0, 0.0],
                    [0.0, 1.0, 0.0],
                    [0.0, 0.0, 1.0],
                    [0.0, -1.0, 0.0],
                ],
                vec![[0, 1, 2], [1, 0, 3], [0, 1, 4]],
            )],
            &[],
        )
        .unwrap();
        assert!(has(&report, AnalysisClassification::NonManifold));
    }

    #[test]
    fn reports_degenerate_triangles_without_welding_neighbors() {
        let report = inspect(
            &[piece(
                11,
                vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]],
                vec![[0, 1, 2]],
            )],
            &[],
        )
        .unwrap();
        assert!(has(&report, AnalysisClassification::DegenerateTriangle));
        assert!(!has(&report, AnalysisClassification::OpenBoundary));
    }
}
