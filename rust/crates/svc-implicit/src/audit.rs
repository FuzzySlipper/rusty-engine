//! Opt-in diagnostics for independently extracted implicit pieces.
//!
//! The audit compares the extracted triangles, then uses only the signs of the
//! original fields on either side of a face.  A field value is deliberately
//! never interpreted as a distance: CSG, non-uniform placement, and authored
//! displacement preserve a zero surface without preserving that metric.
//!
//! Results are approximate at extraction resolution.  In particular a curved
//! dual-contoured surface may leave a gap between facets that represents no
//! authored gap, a narrow contact can fall between samples, and partial areas
//! are triangle/projection aggregates rather than an exact CSG intersection.
//! The audit has no camera, depth-buffer, material, or topology-repair model.
//! Absence of a diagnostic is therefore not a proof against camera-dependent
//! artifacts, contacts below the supplied extraction spacing, or malformed
//! open/non-manifold extraction topology.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use nalgebra::{Matrix4, Quaternion, UnitQuaternion, Vector3};

use crate::{Bounds, Error, Field, Node};

const PARALLEL_DOT: f64 = 0.999;
const COINCIDENT_FRACTION: f64 = 0.125;
const AREA_EPSILON: f64 = 1.0e-10;

/// A stable authored piece identity and the field/mesh evidence required for
/// an opt-in audit.  Positions are retained in world space.
#[derive(Clone)]
pub struct Piece {
    id: u64,
    field: Field,
    node: Node,
    positions: Vec<[f32; 3]>,
    triangles: Vec<[u32; 3]>,
    bounds: Bounds,
    triangle_bounds: Vec<Bounds>,
    sample_spacing: f32,
}

impl Piece {
    /// Copies an extracted local mesh and places both it and its source field
    /// with the same column-major affine matrix.
    pub fn new(
        id: u64,
        field: &Field,
        node: Node,
        positions: Vec<[f32; 3]>,
        triangles: Vec<[u32; 3]>,
        local_to_world: [f32; 16],
        sample_spacing: f32,
    ) -> Result<Self, Error> {
        validate_spacing(sample_spacing)?;
        validate_mesh(&positions, &triangles)?;

        // Field::transform owns the affine/invertibility validation and makes
        // the copied field evaluate in the same world coordinates as the mesh.
        let mut field = field.clone();
        let node = field.transform(node, local_to_world)?;
        let matrix = Matrix4::from_column_slice(&local_to_world);
        let positions: Vec<_> = positions
            .into_iter()
            .map(|position| transform_point(&matrix, position))
            .collect::<Result<_, _>>()?;
        let triangle_bounds = triangles
            .iter()
            .map(|&triangle| triangle_bounds(&positions, triangle))
            .collect::<Result<Vec<_>, _>>()?;
        let bounds = bounds_for_positions(&positions)?;
        // The largest singular value carries local extraction spacing into
        // world space through rotation/shear/nonuniform placement, without
        // treating field values as distances.
        let largest_scale = matrix
            .fixed_view::<3, 3>(0, 0)
            .into_owned()
            .svd(false, false)
            .singular_values
            .iter()
            .copied()
            .fold(0.0_f32, f32::max);
        let world_spacing = sample_spacing * largest_scale;
        validate_spacing(world_spacing)?;
        Ok(Self {
            id,
            field,
            node,
            positions,
            triangles,
            bounds,
            triangle_bounds,
            sample_spacing: world_spacing,
        })
    }

    /// Convenience placement matching [`Field::transform_trs`].
    #[allow(clippy::too_many_arguments)] // public ABI-facing capture contract
    pub fn new_trs(
        id: u64,
        field: &Field,
        node: Node,
        positions: Vec<[f32; 3]>,
        triangles: Vec<[u32; 3]>,
        translation: [f32; 3],
        rotation: [f32; 4],
        scale: [f32; 3],
        sample_spacing: f32,
    ) -> Result<Self, Error> {
        if translation
            .iter()
            .chain(rotation.iter())
            .chain(scale.iter())
            .any(|v| !v.is_finite())
        {
            return Err(Error("piece transform must be finite".into()));
        }
        let quaternion = Quaternion::new(rotation[3], rotation[0], rotation[1], rotation[2]);
        if !quaternion.norm().is_finite() || quaternion.norm() == 0.0 {
            return Err(Error(
                "piece rotation must have a finite nonzero norm".into(),
            ));
        }
        let matrix = Matrix4::new_translation(&Vector3::from(translation))
            * UnitQuaternion::new_normalize(quaternion).to_homogeneous()
            * Matrix4::new_nonuniform_scaling(&Vector3::from(scale));
        Self::new(
            id,
            field,
            node,
            positions,
            triangles,
            matrix.as_slice().try_into().expect("4x4 matrix"),
            sample_spacing,
        )
    }

    pub fn id(&self) -> u64 {
        self.id
    }
}

/// The source of a geometry conflict.  Intentional solid intersections do
/// not produce coincident diagnostics; their occluded portions can still be
/// reported as `BuriedSurface`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Classification {
    CoincidentExposed,
    NearCoincidentExposed,
    BuriedSurface,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Diagnostic {
    pub piece_a: u64,
    pub piece_b: u64,
    pub classification: Classification,
    /// World-space enclosure of the contributing triangle regions.
    pub bounds: Bounds,
    /// Pairwise triangle/projection area accumulated at extraction resolution.
    /// A buried pair can include area from faces of either piece.
    pub approximate_area: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Report {
    pub diagnostics: Vec<Diagnostic>,
    /// Piece pairs surviving the world-space AABB sweep.
    pub candidate_pairs: u64,
    /// Triangle pairs surviving the per-piece triangle AABB sweep.
    pub triangle_pairs: u64,
}

/// Audits independently extracted pieces. `tolerance_cells` is multiplied by
/// the coarser of each pair's extraction spacings, so the reported tolerance
/// remains tied to finite extraction resolution rather than field magnitude.
pub fn audit(pieces: &[Arc<Piece>], tolerance_cells: f32) -> Result<Report, Error> {
    if !tolerance_cells.is_finite() || tolerance_cells <= 0.0 {
        return Err(Error(
            "audit tolerance_cells must be finite and positive".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for piece in pieces {
        if !ids.insert(piece.id) {
            return Err(Error(format!("duplicate audit piece id {}", piece.id)));
        }
    }
    if pieces.len() < 2 {
        return Ok(Report::default());
    }

    let max_tolerance = pieces
        .iter()
        .map(|piece| piece.sample_spacing as f64 * tolerance_cells as f64)
        .fold(0.0_f64, f64::max);
    if !max_tolerance.is_finite() {
        return Err(Error("audit tolerance is not representable".into()));
    }

    let mut order: Vec<_> = (0..pieces.len()).collect();
    order.sort_by(|&a, &b| {
        pieces[a].bounds.min[0]
            .total_cmp(&pieces[b].bounds.min[0])
            .then_with(|| pieces[a].id.cmp(&pieces[b].id))
    });

    let mut report = Report::default();
    let mut accumulated = BTreeMap::<(u64, u64, Classification), (Bounds, f64)>::new();
    for (offset, &left_index) in order.iter().enumerate() {
        let left = &pieces[left_index];
        for &right_index in &order[offset + 1..] {
            let right = &pieces[right_index];
            if right.bounds.min[0] as f64 > left.bounds.max[0] as f64 + max_tolerance {
                break;
            }
            let tolerance = pair_tolerance(left, right, tolerance_cells)?;
            if !bounds_overlap(left.bounds, right.bounds, tolerance) {
                continue;
            }
            report.candidate_pairs += 1;
            audit_pair(
                left,
                right,
                pieces,
                tolerance,
                &mut report,
                &mut accumulated,
            )?;
        }
    }
    report.diagnostics = accumulated
        .into_iter()
        .map(
            |((piece_a, piece_b, classification), (bounds, approximate_area))| Diagnostic {
                piece_a,
                piece_b,
                classification,
                bounds,
                approximate_area,
            },
        )
        .collect();
    Ok(report)
}

fn audit_pair(
    first: &Piece,
    second: &Piece,
    all_pieces: &[Arc<Piece>],
    tolerance: f64,
    report: &mut Report,
    accumulated: &mut BTreeMap<(u64, u64, Classification), (Bounds, f64)>,
) -> Result<(), Error> {
    let (left, right) = if first.id < second.id {
        (first, second)
    } else {
        (second, first)
    };
    let triangle_pairs = candidate_triangles(left, right, tolerance);
    for (a, b) in triangle_pairs {
        report.triangle_pairs += 1;
        if let Some(overlap) = coplanar_overlap(left, a, right, b, tolerance)? {
            for index in 1..overlap.polygon.len() - 1 {
                sample_triangle(
                    [
                        overlap.polygon[0],
                        overlap.polygon[index],
                        overlap.polygon[index + 1],
                    ],
                    left.sample_spacing.min(right.sample_spacing) as f64,
                    |point, bounds, area| {
                        if mutually_exposed(left, a, right, b, all_pieces, point, tolerance)? {
                            accumulate(
                                accumulated,
                                left.id,
                                right.id,
                                overlap.classification,
                                bounds,
                                area,
                            );
                        }
                        Ok(())
                    },
                )?;
            }
        }
    }
    // Sample partial internal patches at extraction spacing, independently of
    // coplanar matching: a fully enclosed piece has no nearby outside facets.
    for (surface, other) in [(left, right), (right, left)] {
        for triangle in 0..surface.triangles.len() {
            if !bounds_overlap(surface.triangle_bounds[triangle], other.bounds, tolerance) {
                continue;
            }
            sample_triangle(
                triangle_points(surface, triangle)?,
                surface.sample_spacing.min(other.sample_spacing) as f64,
                |point, bounds, area| {
                    if buried_against(surface, triangle, other, point, tolerance)? {
                        accumulate(
                            accumulated,
                            left.id,
                            right.id,
                            Classification::BuriedSurface,
                            bounds,
                            area,
                        );
                    }
                    Ok(())
                },
            )?;
        }
    }
    Ok(())
}

fn candidate_triangles(left: &Piece, right: &Piece, tolerance: f64) -> Vec<(usize, usize)> {
    let mut left_order: Vec<_> = (0..left.triangles.len()).collect();
    let mut right_order: Vec<_> = (0..right.triangles.len()).collect();
    left_order.sort_by(|&a, &b| {
        left.triangle_bounds[a].min[0].total_cmp(&left.triangle_bounds[b].min[0])
    });
    right_order.sort_by(|&a, &b| {
        right.triangle_bounds[a].min[0].total_cmp(&right.triangle_bounds[b].min[0])
    });
    let mut start = 0;
    let mut result = Vec::new();
    for a in left_order {
        while start < right_order.len()
            && (right.triangle_bounds[right_order[start]].max[0] as f64)
                < (left.triangle_bounds[a].min[0] as f64) - tolerance
        {
            start += 1;
        }
        for &b in &right_order[start..] {
            if right.triangle_bounds[b].min[0] as f64
                > left.triangle_bounds[a].max[0] as f64 + tolerance
            {
                break;
            }
            if bounds_overlap(left.triangle_bounds[a], right.triangle_bounds[b], tolerance) {
                result.push((a, b));
            }
        }
    }
    result
}

struct CoplanarOverlap {
    classification: Classification,
    polygon: Vec<[f64; 3]>,
}

fn coplanar_overlap(
    left: &Piece,
    left_triangle: usize,
    right: &Piece,
    right_triangle: usize,
    tolerance: f64,
) -> Result<Option<CoplanarOverlap>, Error> {
    let a = triangle_points(left, left_triangle)?;
    let b = triangle_points(right, right_triangle)?;
    let normal_a = triangle_normal(a)?;
    let normal_b = triangle_normal(b)?;
    if dot(normal_a, normal_b).abs() < PARALLEL_DOT {
        return Ok(None);
    }
    let plane_separation = b
        .iter()
        .map(|point| dot(sub(*point, a[0]), normal_a).abs())
        .fold(0.0_f64, f64::max);
    if plane_separation > tolerance {
        return Ok(None);
    }
    let axis = major_axis(normal_a);
    let mut subject = b.map(|point| project(point, axis)).to_vec();
    let mut clip = a.map(|point| project(point, axis)).to_vec();
    ensure_ccw(&mut subject);
    ensure_ccw(&mut clip);
    let overlap = clip_polygon(&subject, &clip);
    let projected_area = polygon_area(&overlap).abs();
    let area = projected_area / normal_a[axis].abs();
    if !area.is_finite() || area <= AREA_EPSILON {
        return Ok(None);
    }
    let polygon = overlap
        .iter()
        .map(|p| unproject(*p, axis, a[0], normal_a))
        .collect::<Result<Vec<_>, _>>()?;
    let classification = if plane_separation
        <= probe_roundoff(a[0])
            .max(probe_roundoff(b[0]))
            .min(tolerance)
    {
        Classification::CoincidentExposed
    } else {
        Classification::NearCoincidentExposed
    };
    Ok(Some(CoplanarOverlap {
        classification,
        polygon,
    }))
}

fn mutually_exposed(
    left: &Piece,
    left_triangle: usize,
    right: &Piece,
    right_triangle: usize,
    all_pieces: &[Arc<Piece>],
    point: [f64; 3],
    tolerance: f64,
) -> Result<bool, Error> {
    let Some(left_outside) = outside_against(left, left_triangle, right, point, tolerance)? else {
        return Ok(false);
    };
    let Some(right_outside) = outside_against(right, right_triangle, left, point, tolerance)?
    else {
        return Ok(false);
    };
    Ok(
        outside_other_pieces(left_outside, left.id, right.id, all_pieces)?
            && outside_other_pieces(right_outside, left.id, right.id, all_pieces)?,
    )
}

fn outside_against(
    surface: &Piece,
    triangle: usize,
    other: &Piece,
    point: [f64; 3],
    tolerance: f64,
) -> Result<Option<[f64; 3]>, Error> {
    let points = triangle_points(surface, triangle)?;
    let normal = triangle_normal(points)?;
    let point = add(point, scale(normal, dot(sub(points[0], point), normal)));
    let Some(outside) = outside_probe(surface, point, normal, tolerance)? else {
        return Ok(None);
    };
    Ok((other.field.sample(other.node, &[to_f32(outside)?])?[0] > 0.0).then_some(outside))
}

fn outside_other_pieces(
    point: [f64; 3],
    first_id: u64,
    second_id: u64,
    pieces: &[Arc<Piece>],
) -> Result<bool, Error> {
    for piece in pieces {
        if piece.id == first_id || piece.id == second_id || !point_in_bounds(point, piece.bounds) {
            continue;
        }
        if piece.field.sample(piece.node, &[to_f32(point)?])?[0] <= 0.0 {
            return Ok(false);
        }
    }
    Ok(true)
}

fn buried_against(
    surface: &Piece,
    triangle: usize,
    other: &Piece,
    point: [f64; 3],
    tolerance: f64,
) -> Result<bool, Error> {
    let normal = triangle_normal(triangle_points(surface, triangle)?)?;
    let Some(outside) = outside_probe(surface, point, normal, tolerance)? else {
        return Ok(false);
    };
    Ok(other.field.sample(other.node, &[to_f32(outside)?])?[0] <= 0.0)
}

fn probe_roundoff(point: [f64; 3]) -> f64 {
    point.into_iter().map(f64::abs).fold(1.0, f64::max) * f64::from(f32::EPSILON) * 4.0
}

// Start at representable coordinate resolution so a known planar gap is not
// crossed by the probe. Grow only if the extracted facet is offset from its
// field (as with curved DC facets). Failure to straddle remains uncertain.
fn outside_probe(
    surface: &Piece,
    point: [f64; 3],
    normal: [f64; 3],
    tolerance: f64,
) -> Result<Option<[f64; 3]>, Error> {
    let maximum = side_probe_step(surface, tolerance).max(probe_roundoff(point));
    let mut step = probe_roundoff(point);
    loop {
        let plus = add(point, scale(normal, step));
        let minus = sub(point, scale(normal, step));
        let values = surface
            .field
            .sample(surface.node, &[to_f32(plus)?, to_f32(minus)?])?;
        match (values[0] > 0.0, values[1] > 0.0) {
            (true, false) => return Ok(Some(plus)),
            (false, true) => return Ok(Some(minus)),
            _ if step >= maximum => return Ok(None),
            _ => step = (step * 4.0).min(maximum),
        }
    }
}

// Long-edge bisection keeps partial exposure/burial estimates tied to the
// supplied extraction resolution rather than to DC's large simplified facets.
fn sample_triangle(
    triangle: [[f64; 3]; 3],
    spacing: f64,
    mut visit: impl FnMut([f64; 3], Bounds, f64) -> Result<(), Error>,
) -> Result<(), Error> {
    let mut pending = vec![triangle];
    while let Some(points) = pending.pop() {
        let (edge, length_squared) = (0..3)
            .map(|i| {
                let d = sub(points[i], points[(i + 1) % 3]);
                (i, dot(d, d))
            })
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .expect("triangle edges");
        if length_squared > spacing * spacing {
            let (a, b, c) = (points[edge], points[(edge + 1) % 3], points[(edge + 2) % 3]);
            let middle = scale(add(a, b), 0.5);
            if middle == a || middle == b {
                return Err(Error(
                    "audit sampling spacing is below coordinate precision".into(),
                ));
            }
            pending.push([a, middle, c]);
            pending.push([middle, b, c]);
        } else {
            let cross = cross(sub(points[1], points[0]), sub(points[2], points[0]));
            let area = dot(cross, cross).sqrt() * 0.5;
            let mut bounds = Bounds {
                min: to_f32(points[0])?,
                max: to_f32(points[0])?,
            };
            for p in &points[1..] {
                let p = to_f32(*p)?;
                for (axis, value) in p.iter().enumerate() {
                    bounds.min[axis] = bounds.min[axis].min(*value);
                    bounds.max[axis] = bounds.max[axis].max(*value);
                }
            }
            visit(
                scale(add(add(points[0], points[1]), points[2]), 1.0 / 3.0),
                bounds,
                area,
            )?;
        }
    }
    Ok(())
}

fn accumulate(
    map: &mut BTreeMap<(u64, u64, Classification), (Bounds, f64)>,
    piece_a: u64,
    piece_b: u64,
    classification: Classification,
    bounds: Bounds,
    area: f64,
) {
    let entry = map
        .entry((piece_a, piece_b, classification))
        .or_insert((bounds, 0.0));
    entry.0 = union_bounds(entry.0, bounds);
    entry.1 += area;
}

fn pair_tolerance(a: &Piece, b: &Piece, cells: f32) -> Result<f64, Error> {
    let tolerance = a.sample_spacing.max(b.sample_spacing) as f64 * cells as f64;
    if tolerance.is_finite() && tolerance > 0.0 {
        Ok(tolerance)
    } else {
        Err(Error("audit tolerance is not representable".into()))
    }
}

// Probe just across the extracted face.  The supplied tolerance is a
// candidate-window width, so stepping all the way across it would turn a
// legitimate narrow exposed gap into a false buried result.
fn side_probe_step(piece: &Piece, tolerance: f64) -> f64 {
    (tolerance * COINCIDENT_FRACTION).min(piece.sample_spacing as f64 * 0.25)
}

fn validate_spacing(spacing: f32) -> Result<(), Error> {
    if spacing.is_finite() && spacing > 0.0 {
        Ok(())
    } else {
        Err(Error(
            "piece sample_spacing must be finite and positive".into(),
        ))
    }
}

fn validate_mesh(positions: &[[f32; 3]], triangles: &[[u32; 3]]) -> Result<(), Error> {
    if positions.is_empty() || triangles.is_empty() {
        return Err(Error("audit piece requires positions and triangles".into()));
    }
    if positions.iter().flatten().any(|value| !value.is_finite()) {
        return Err(Error("audit piece positions must be finite".into()));
    }
    for &triangle in triangles {
        if triangle
            .iter()
            .any(|&index| index as usize >= positions.len())
        {
            return Err(Error("audit piece triangle index is out of bounds".into()));
        }
        let points = triangle.map(|index| to_f64(positions[index as usize]));
        if triangle_normal(points).is_err() {
            return Err(Error("audit piece contains a degenerate triangle".into()));
        }
    }
    Ok(())
}

fn transform_point(matrix: &Matrix4<f32>, position: [f32; 3]) -> Result<[f32; 3], Error> {
    let point = matrix * nalgebra::Vector4::new(position[0], position[1], position[2], 1.0);
    if point.w != 1.0 || point.xyz().iter().any(|value| !value.is_finite()) {
        return Err(Error(
            "piece transform produced an invalid world position".into(),
        ));
    }
    Ok([point.x, point.y, point.z])
}

fn bounds_for_positions(positions: &[[f32; 3]]) -> Result<Bounds, Error> {
    let mut min = positions[0];
    let mut max = positions[0];
    for position in &positions[1..] {
        for axis in 0..3 {
            min[axis] = min[axis].min(position[axis]);
            max[axis] = max[axis].max(position[axis]);
        }
    }
    // A planar/open piece has zero extent along an axis, while Bounds is also
    // the public volume-generation type whose validator requires all extents.
    Ok(Bounds { min, max })
}

fn triangle_bounds(positions: &[[f32; 3]], triangle: [u32; 3]) -> Result<Bounds, Error> {
    let points = triangle.map(|index| positions[index as usize]);
    let mut min = points[0];
    let mut max = points[0];
    for point in &points[1..] {
        for axis in 0..3 {
            min[axis] = min[axis].min(point[axis]);
            max[axis] = max[axis].max(point[axis]);
        }
    }
    Ok(Bounds { min, max })
}

fn triangle_points(piece: &Piece, triangle: usize) -> Result<[[f64; 3]; 3], Error> {
    piece
        .triangles
        .get(triangle)
        .map(|indices| indices.map(|index| to_f64(piece.positions[index as usize])))
        .ok_or_else(|| Error("audit triangle is out of bounds".into()))
}

fn triangle_normal(points: [[f64; 3]; 3]) -> Result<[f64; 3], Error> {
    normalize(cross(sub(points[1], points[0]), sub(points[2], points[0])))
        .ok_or_else(|| Error("audit triangle has no usable normal".into()))
}

fn bounds_overlap(a: Bounds, b: Bounds, tolerance: f64) -> bool {
    (0..3).all(|axis| {
        a.min[axis] as f64 <= b.max[axis] as f64 + tolerance
            && b.min[axis] as f64 <= a.max[axis] as f64 + tolerance
    })
}

fn point_in_bounds(point: [f64; 3], bounds: Bounds) -> bool {
    point
        .iter()
        .enumerate()
        .all(|(axis, value)| *value >= bounds.min[axis] as f64 && *value <= bounds.max[axis] as f64)
}

fn union_bounds(a: Bounds, b: Bounds) -> Bounds {
    Bounds {
        min: std::array::from_fn(|axis| a.min[axis].min(b.min[axis])),
        max: std::array::from_fn(|axis| a.max[axis].max(b.max[axis])),
    }
}

fn major_axis(normal: [f64; 3]) -> usize {
    if normal[0].abs() >= normal[1].abs() && normal[0].abs() >= normal[2].abs() {
        0
    } else if normal[1].abs() >= normal[2].abs() {
        1
    } else {
        2
    }
}

fn project(point: [f64; 3], drop: usize) -> [f64; 2] {
    match drop {
        0 => [point[1], point[2]],
        1 => [point[0], point[2]],
        _ => [point[0], point[1]],
    }
}

fn unproject(
    point: [f64; 2],
    drop: usize,
    plane_point: [f64; 3],
    normal: [f64; 3],
) -> Result<[f64; 3], Error> {
    let d = dot(plane_point, normal);
    let coordinate = match drop {
        0 => (d - normal[1] * point[0] - normal[2] * point[1]) / normal[0],
        1 => (d - normal[0] * point[0] - normal[2] * point[1]) / normal[1],
        _ => (d - normal[0] * point[0] - normal[1] * point[1]) / normal[2],
    };
    let world = match drop {
        0 => [coordinate, point[0], point[1]],
        1 => [point[0], coordinate, point[1]],
        _ => [point[0], point[1], coordinate],
    };
    if world.iter().all(|value| value.is_finite()) {
        Ok(world)
    } else {
        Err(Error("audit overlap is not representable".into()))
    }
}

fn ensure_ccw(polygon: &mut [[f64; 2]]) {
    if polygon_area(polygon) < 0.0 {
        polygon.reverse();
    }
}
fn polygon_area(polygon: &[[f64; 2]]) -> f64 {
    polygon
        .iter()
        .enumerate()
        .map(|(index, point)| {
            let next = polygon[(index + 1) % polygon.len()];
            point[0] * next[1] - point[1] * next[0]
        })
        .sum::<f64>()
        * 0.5
}
fn clip_polygon(subject: &[[f64; 2]], clip: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let mut output = subject.to_vec();
    for (index, &start) in clip.iter().enumerate() {
        let end = clip[(index + 1) % clip.len()];
        let input = std::mem::take(&mut output);
        if input.is_empty() {
            break;
        }
        let mut previous = *input.last().expect("nonempty polygon");
        let mut previous_inside = cross2(sub2(end, start), sub2(previous, start)) >= -AREA_EPSILON;
        for current in input {
            let current_inside = cross2(sub2(end, start), sub2(current, start)) >= -AREA_EPSILON;
            if current_inside != previous_inside {
                output.push(line_intersection(previous, current, start, end));
            }
            if current_inside {
                output.push(current);
            }
            previous = current;
            previous_inside = current_inside;
        }
    }
    output
}
fn line_intersection(a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]) -> [f64; 2] {
    let ab = sub2(b, a);
    let cd = sub2(d, c);
    let denominator = cross2(ab, cd);
    if denominator.abs() <= f64::EPSILON {
        return a;
    }
    let t = cross2(sub2(c, a), cd) / denominator;
    [a[0] + ab[0] * t, a[1] + ab[1] * t]
}

fn to_f64(point: [f32; 3]) -> [f64; 3] {
    point.map(f64::from)
}
fn to_f32(point: [f64; 3]) -> Result<[f32; 3], Error> {
    if point
        .iter()
        .all(|value| value.is_finite() && *value >= f32::MIN as f64 && *value <= f32::MAX as f64)
    {
        Ok(point.map(|value| value as f32))
    } else {
        Err(Error("audit sample point is not representable".into()))
    }
}
fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|axis| a[axis] + b[axis])
}
fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|axis| a[axis] - b[axis])
}
fn scale(a: [f64; 3], amount: f64) -> [f64; 3] {
    a.map(|value| value * amount)
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|axis| a[axis] * b[axis]).sum()
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn length(value: [f64; 3]) -> f64 {
    dot(value, value).sqrt()
}
fn normalize(value: [f64; 3]) -> Option<[f64; 3]> {
    let length = length(value);
    (length.is_finite() && length > 0.0).then(|| scale(value, 1.0 / length))
}
fn sub2(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] - b[0], a[1] - b[1]]
}
fn cross2(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity() -> [f32; 16] {
        [
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ]
    }
    fn cube_mesh(min: [f32; 3], max: [f32; 3]) -> (Vec<[f32; 3]>, Vec<[u32; 3]>) {
        let [x0, y0, z0] = min;
        let [x1, y1, z1] = max;
        (
            vec![
                [x0, y0, z0],
                [x1, y0, z0],
                [x1, y1, z0],
                [x0, y1, z0],
                [x0, y0, z1],
                [x1, y0, z1],
                [x1, y1, z1],
                [x0, y1, z1],
            ],
            vec![
                [0, 2, 1],
                [0, 3, 2],
                [4, 5, 6],
                [4, 6, 7],
                [0, 1, 5],
                [0, 5, 4],
                [1, 2, 6],
                [1, 6, 5],
                [2, 3, 7],
                [2, 7, 6],
                [3, 0, 4],
                [3, 4, 7],
            ],
        )
    }
    fn box_piece(id: u64, min: [f32; 3], max: [f32; 3]) -> Arc<Piece> {
        let mut field = Field::new();
        let node = field.box_shape(Bounds { min, max }).unwrap();
        let (positions, triangles) = cube_mesh(min, max);
        Arc::new(Piece::new(id, &field, node, positions, triangles, identity(), 0.1).unwrap())
    }
    fn classes(report: &Report) -> Vec<Classification> {
        report
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.classification)
            .collect()
    }

    #[test]
    fn reports_coincident_and_partial_planar_overlap_without_shared_edges() {
        let a = box_piece(2, [0.0, 0.0, 0.0], [2.0, 2.0, 1.0]);
        let b = box_piece(1, [1.0, 0.0, 0.0], [3.0, 2.0, 1.0]);
        let report = audit(&[a, b], 0.5).unwrap();
        assert!(
            classes(&report).contains(&Classification::CoincidentExposed),
            "{report:?}"
        );
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.approximate_area > 1.9));
        assert_eq!(report.diagnostics[0].piece_a, 1);
    }

    #[test]
    fn loading_bay_column_cap_conflict_has_exact_area_and_trim_removes_exposed_conflict() {
        let column = box_piece(1, [-0.55, 0.0, -0.55], [0.55, 5.0, 0.55]);
        let capital = box_piece(2, [-0.7, 4.5, -0.7], [0.7, 5.0, 0.7]);
        let original = audit(&[column, capital.clone()], 0.5).unwrap();
        let top = original
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.classification == Classification::CoincidentExposed)
            .expect("untrimmed column top conflicts with capital top");
        assert!((top.approximate_area - 1.21).abs() < 0.0001, "{original:?}");

        let trimmed = box_piece(1, [-0.55, 0.0, -0.55], [0.55, 4.5, 0.55]);
        let corrected = audit(&[trimmed, capital], 0.5).unwrap();
        assert!(
            !classes(&corrected).iter().any(|classification| matches!(
                classification,
                Classification::CoincidentExposed | Classification::NearCoincidentExposed
            )),
            "{corrected:?}"
        );
    }

    #[test]
    fn distinguishes_near_separated_and_shared_edges() {
        let a = box_piece(1, [0.0, 0.0, 0.0], [1.0, 1.0, 1.0]);
        let near = box_piece(2, [0.0, 0.0, 1.03], [1.0, 1.0, 2.03]);
        let separated = box_piece(3, [0.0, 0.0, 1.2], [1.0, 1.0, 2.2]);
        let edge_only = box_piece(4, [1.0, 1.0, 1.0], [2.0, 2.0, 2.0]);
        let near_report = audit(&[a.clone(), near], 0.5).unwrap();
        assert!(
            classes(&near_report).contains(&Classification::NearCoincidentExposed),
            "{near_report:?}"
        );
        assert!(audit(&[a.clone(), separated], 0.5)
            .unwrap()
            .diagnostics
            .is_empty());
        assert!(audit(&[a, edge_only], 0.5).unwrap().diagnostics.is_empty());
    }

    #[test]
    fn rotated_piece_and_intentional_intersection_do_not_make_coincident_conflicts() {
        let a = box_piece(1, [-1.0, -1.0, -1.0], [1.0, 1.0, 1.0]);
        let mut field = Field::new();
        let node = field
            .box_shape(Bounds {
                min: [-1.0, -1.0, -1.0],
                max: [1.0, 1.0, 1.0],
            })
            .unwrap();
        let (positions, triangles) = cube_mesh([-1.0, -1.0, -1.0], [1.0, 1.0, 1.0]);
        let rotation = UnitQuaternion::from_euler_angles(0.37, 0.0, 0.79);
        let rotated = Arc::new(
            Piece::new_trs(
                2,
                &field,
                node,
                positions,
                triangles,
                [0.4, 0.0, 0.0],
                [rotation.i, rotation.j, rotation.k, rotation.w],
                [1.0, 1.0, 1.0],
                0.1,
            )
            .unwrap(),
        );
        let report = audit(&[a, rotated], 0.5).unwrap();
        assert!(!classes(&report).iter().any(|classification| matches!(
            classification,
            Classification::CoincidentExposed | Classification::NearCoincidentExposed
        )));
    }

    #[test]
    fn rotated_partial_overlap_uses_a_polygon_interior_probe() {
        let mut field = Field::new();
        let node = field
            .box_shape(Bounds {
                min: [-1.0, -1.0, -1.0],
                max: [1.0, 1.0, 1.0],
            })
            .unwrap();
        let rotation = UnitQuaternion::from_euler_angles(0.0, 0.0, 0.61);
        let make_piece = |id, translation| {
            let (positions, triangles) = cube_mesh([-1.0, -1.0, -1.0], [1.0, 1.0, 1.0]);
            Arc::new(
                Piece::new_trs(
                    id,
                    &field,
                    node,
                    positions,
                    triangles,
                    translation,
                    [rotation.i, rotation.j, rotation.k, rotation.w],
                    [1.0, 1.0, 1.0],
                    0.1,
                )
                .unwrap(),
            )
        };
        let report = audit(
            &[
                make_piece(1, [0.0, 0.0, 0.0]),
                make_piece(2, [0.4, 0.0, 0.0]),
            ],
            0.5,
        )
        .unwrap();
        assert!(
            classes(&report).contains(&Classification::CoincidentExposed),
            "{report:?}"
        );
    }

    #[test]
    fn reports_buried_inner_shape_and_rejects_duplicate_ids() {
        let outer = box_piece(1, [-2.0, -2.0, -2.0], [2.0, 2.0, 2.0]);
        let inner = box_piece(2, [-0.5, -0.5, -0.5], [0.5, 0.5, 0.5]);
        let report = audit(&[outer.clone(), inner], 0.5).unwrap();
        assert!(classes(&report).contains(&Classification::BuriedSurface));
        let duplicate = box_piece(1, [4.0, 4.0, 4.0], [5.0, 5.0, 5.0]);
        assert!(audit(&[outer, duplicate], 0.5).is_err());
    }

    #[test]
    fn enclosing_piece_suppresses_coincident_exposed_pair() {
        let outer = box_piece(1, [-2.0, -2.0, -2.0], [2.0, 2.0, 2.0]);
        let inner_a = box_piece(2, [-0.5, -0.5, -0.5], [0.5, 0.5, 0.5]);
        let inner_b = box_piece(3, [-0.5, -0.5, -0.5], [0.5, 0.5, 0.5]);
        let report = audit(&[outer, inner_a, inner_b], 0.5).unwrap();
        assert!(
            !classes(&report).iter().any(|classification| matches!(
                classification,
                Classification::CoincidentExposed | Classification::NearCoincidentExposed
            )),
            "{report:?}"
        );
    }
    #[test]
    fn narrow_planar_air_gap_remains_exposed_not_buried() {
        let a = box_piece(1, [0.0; 3], [1.0; 3]);
        let b = box_piece(2, [0.0, 0.0, 1.001], [1.0, 1.0, 2.001]);
        let report = audit(&[a, b], 0.5).unwrap();
        let near = report
            .diagnostics
            .iter()
            .find(|d| d.classification == Classification::NearCoincidentExposed)
            .expect("exposed narrow gap");
        assert!((near.approximate_area - 1.0).abs() < 1e-6, "{report:?}");
        assert!(
            !classes(&report).contains(&Classification::BuriedSurface),
            "{report:?}"
        );
    }

    #[test]
    fn partial_occluder_does_not_hide_whole_exposed_triangles() {
        let mut field = Field::new();
        let node = field
            .box_shape(Bounds {
                min: [0.0; 3],
                max: [2.0, 2.0, 1.0],
            })
            .unwrap();
        let (positions, _) = cube_mesh([0.0; 3], [2.0, 2.0, 1.0]);
        let pieces = [1, 2].map(|id| {
            Arc::new(
                Piece::new(
                    id,
                    &field,
                    node,
                    positions.clone(),
                    vec![[2, 3, 7], [2, 7, 6]],
                    identity(),
                    0.1,
                )
                .unwrap(),
            )
        });
        let occluder = box_piece(3, [0.55, 1.5, 0.2], [1.45, 3.0, 0.8]);
        let report = audit(&[pieces[0].clone(), pieces[1].clone(), occluder], 0.5).unwrap();
        let exposed = report
            .diagnostics
            .iter()
            .find(|d| {
                d.piece_a == 1
                    && d.piece_b == 2
                    && d.classification == Classification::CoincidentExposed
            })
            .expect("partly exposed top");
        assert!((exposed.approximate_area - 1.46).abs() < 0.1, "{report:?}");
    }
}
