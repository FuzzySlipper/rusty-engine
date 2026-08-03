use std::collections::{BTreeMap, BTreeSet};

use voxel_asset::{
    VoxelConversionFitPolicy, VoxelConversionMode, VoxelConversionOriginPolicy,
    VoxelConversionRequest, VoxelConversionSettings,
};

use crate::{ConversionError, ImportedStaticMesh};

pub const MAX_GEOMETRIC_VOXELIZATION_WORK: u64 = 10_000_000;

const CELL_HALF_EXTENT: f64 = 0.5;
const ROW_PERTURBATIONS: [[f64; 2]; 3] = [
    [1.192_092_895_507_812_5e-7, 5.960_464_477_539_063e-8],
    [-5.960_464_477_539_063e-8, 1.192_092_895_507_812_5e-7],
    [2.384_185_791_015_625e-7, -1.192_092_895_507_812_5e-7],
];

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct MaterialEvidence {
    pub source_material_slot: u32,
    pub triangle_index: usize,
    pub barycentric: [f64; 3],
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct VoxelizationResult {
    pub cells: BTreeMap<[i64; 3], MaterialEvidence>,
    pub work: u64,
}

/// One immutable source-space envelope used to map multiple sampled meshes to
/// the same voxel grid. Object conversion computes this once across every
/// selected frame; individual frame geometry must remain inside it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct VoxelizationSourceBounds {
    pub min: [f64; 3],
    pub max: [f64; 3],
}

impl VoxelizationSourceBounds {
    pub(crate) fn for_mesh(mesh: &ImportedStaticMesh) -> Result<Self, ConversionError> {
        let first = *mesh.positions.first().ok_or_else(|| {
            ConversionError::one(
                "conversion.invalidGeometry",
                "source.positions",
                "mesh has no positions",
            )
        })?;
        let mut bounds = Self {
            min: first,
            max: first,
        };
        for position in mesh.positions.iter().skip(1) {
            bounds.include_position(*position)?;
        }
        bounds.validate()?;
        Ok(bounds)
    }

    pub(crate) fn include_position(&mut self, position: [f64; 3]) -> Result<(), ConversionError> {
        if position.iter().any(|component| !component.is_finite()) {
            return Err(ConversionError::one(
                "conversion.invalidGeometry",
                "source.positions",
                "source positions must be finite",
            ));
        }
        for (axis, component) in position.into_iter().enumerate() {
            self.min[axis] = self.min[axis].min(component);
            self.max[axis] = self.max[axis].max(component);
        }
        Ok(())
    }

    fn validate(self) -> Result<(), ConversionError> {
        if self
            .min
            .iter()
            .chain(self.max.iter())
            .any(|component| !component.is_finite())
            || (0..3).any(|axis| self.min[axis] > self.max[axis])
        {
            return Err(ConversionError::one(
                "conversion.invalidGeometry",
                "source.bounds",
                "source bounds must be finite and ordered",
            ));
        }
        Ok(())
    }

    fn contains(self, position: [f64; 3]) -> bool {
        (0..3).all(|axis| {
            position[axis].is_finite()
                && position[axis] >= self.min[axis]
                && position[axis] <= self.max[axis]
        })
    }
}

#[derive(Debug, Clone, Copy)]
struct MappedTriangle {
    points: [[f64; 3]; 3],
    source_material_slot: u32,
    triangle_index: usize,
}

#[derive(Debug, Clone, Copy)]
struct SurfaceCandidate {
    evidence: MaterialEvidence,
    distance_squared: f64,
}

#[derive(Debug, Clone, Copy)]
struct RayIntersection {
    x: f64,
    evidence: MaterialEvidence,
}

#[derive(Debug, Default)]
struct WorkMeter {
    used: u64,
}

impl WorkMeter {
    fn charge(&mut self, amount: u64, stage: &'static str) -> Result<(), ConversionError> {
        self.used = self
            .used
            .checked_add(amount)
            .ok_or_else(|| work_limit_error(u64::MAX, stage))?;
        if self.used > MAX_GEOMETRIC_VOXELIZATION_WORK {
            return Err(work_limit_error(self.used, stage));
        }
        Ok(())
    }
}

pub(crate) fn voxelize(
    request: &VoxelConversionRequest,
    mesh: &ImportedStaticMesh,
) -> Result<VoxelizationResult, ConversionError> {
    let source_bounds = VoxelizationSourceBounds::for_mesh(mesh)?;
    voxelize_with_source_bounds(request, mesh, source_bounds)
}

pub(crate) fn voxelize_with_source_bounds(
    request: &VoxelConversionRequest,
    mesh: &ImportedStaticMesh,
    source_bounds: VoxelizationSourceBounds,
) -> Result<VoxelizationResult, ConversionError> {
    source_bounds.validate()?;
    if mesh
        .positions
        .iter()
        .any(|position| !source_bounds.contains(*position))
    {
        return Err(ConversionError::one(
            "conversion.invalidGeometry",
            "source.bounds",
            "sampled mesh lies outside the fixed object conversion bounds",
        ));
    }
    if request.settings.mode == VoxelConversionMode::Solid {
        validate_closed_topology(mesh)?;
    }
    let mapper = CoordinateMapper::new(&request.settings, source_bounds);
    let triangles = mesh
        .triangles
        .iter()
        .enumerate()
        .map(|(triangle_index, triangle)| MappedTriangle {
            points: triangle
                .indices
                .map(|index| mapper.map_clamped(mesh.positions[index as usize])),
            source_material_slot: triangle.source_material_slot,
            triangle_index,
        })
        .collect::<Vec<_>>();
    let mut meter = WorkMeter::default();
    let mut candidates = conservative_surface_cells(request, &triangles, &mut meter)?;
    if request.settings.mode == VoxelConversionMode::Solid {
        classify_solid_interior(request, &triangles, &mapper, &mut candidates, &mut meter)?;
    }
    if candidates.is_empty() {
        return Err(ConversionError::one(
            "conversion.invalidGeometry",
            "source",
            "conversion produced no voxels",
        ));
    }
    Ok(VoxelizationResult {
        cells: candidates
            .into_iter()
            .map(|(coordinate, candidate)| (coordinate, candidate.evidence))
            .collect(),
        work: meter.used,
    })
}

fn conservative_surface_cells(
    request: &VoxelConversionRequest,
    triangles: &[MappedTriangle],
    meter: &mut WorkMeter,
) -> Result<BTreeMap<[i64; 3], SurfaceCandidate>, ConversionError> {
    let mut cells = BTreeMap::<[i64; 3], SurfaceCandidate>::new();
    for triangle in triangles {
        let Some(bounds) = candidate_cell_bounds(triangle.points, request.settings.resolution)
        else {
            continue;
        };
        meter.charge(bounds.volume()?, "surface triangle/cell coverage")?;
        for z in bounds.min[2]..=bounds.max[2] {
            for y in bounds.min[1]..=bounds.max[1] {
                for x in bounds.min[0]..=bounds.max[0] {
                    let coordinate = [x, y, z];
                    let center = coordinate.map(|value| value as f64);
                    if !triangle_intersects_cell(triangle.points, center) {
                        continue;
                    }
                    let (barycentric, distance_squared) =
                        closest_triangle_barycentric(triangle.points, center);
                    let candidate = SurfaceCandidate {
                        evidence: MaterialEvidence {
                            source_material_slot: triangle.source_material_slot,
                            triangle_index: triangle.triangle_index,
                            barycentric,
                        },
                        distance_squared,
                    };
                    match cells.get_mut(&coordinate) {
                        Some(current) if candidate_precedes(candidate, *current) => {
                            *current = candidate;
                        }
                        Some(_) => {}
                        None => {
                            if cells.len() >= request.settings.max_output_voxels as usize {
                                return Err(output_limit_error(cells.len() + 1, request));
                            }
                            cells.insert(coordinate, candidate);
                        }
                    }
                }
            }
        }
    }
    Ok(cells)
}

fn classify_solid_interior(
    request: &VoxelConversionRequest,
    triangles: &[MappedTriangle],
    mapper: &CoordinateMapper,
    cells: &mut BTreeMap<[i64; 3], SurfaceCandidate>,
    meter: &mut WorkMeter,
) -> Result<(), ConversionError> {
    let Some(center_bounds) = mapper.center_bounds() else {
        return Ok(());
    };
    for z in center_bounds.min[2]..=center_bounds.max[2] {
        for y in center_bounds.min[1]..=center_bounds.max[1] {
            let intersections = stable_row_intersections(y, z, triangles, meter)?;
            if intersections.is_empty() {
                continue;
            }
            let mut intersection_index = 0usize;
            for x in center_bounds.min[0]..=center_bounds.max[0] {
                meter.charge(1, "solid interior classification")?;
                let center_x = x as f64;
                while intersection_index < intersections.len()
                    && intersections[intersection_index].x <= center_x
                {
                    intersection_index += 1;
                }
                if (intersections.len() - intersection_index).is_multiple_of(2) {
                    continue;
                }
                let coordinate = [x, y, z];
                if cells.contains_key(&coordinate) {
                    continue;
                }
                let exit = intersections[intersection_index];
                cells.insert(
                    coordinate,
                    SurfaceCandidate {
                        evidence: exit.evidence,
                        distance_squared: (exit.x - center_x).powi(2),
                    },
                );
                if cells.len() > request.settings.max_output_voxels as usize {
                    return Err(output_limit_error(cells.len(), request));
                }
            }
        }
    }
    Ok(())
}

fn stable_row_intersections(
    y: i64,
    z: i64,
    triangles: &[MappedTriangle],
    meter: &mut WorkMeter,
) -> Result<Vec<RayIntersection>, ConversionError> {
    for [y_offset, z_offset] in ROW_PERTURBATIONS {
        meter.charge(triangles.len() as u64, "solid ray/triangle intersection")?;
        let mut intersections = triangles
            .iter()
            .filter_map(|triangle| {
                intersect_x_ray(triangle, y as f64 + y_offset, z as f64 + z_offset)
            })
            .collect::<Vec<_>>();
        intersections.sort_by(|left, right| {
            left.x.total_cmp(&right.x).then_with(|| {
                left.evidence
                    .triangle_index
                    .cmp(&right.evidence.triangle_index)
            })
        });
        let intersections = deduplicate_intersections(intersections);
        if intersections.len().is_multiple_of(2) {
            return Ok(intersections);
        }
    }
    Err(ConversionError::one(
        "conversion.ambiguousInterior",
        "source.triangles",
        format!("closed mesh produced an odd ray crossing count near voxel row y={y}, z={z}"),
    ))
}

fn deduplicate_intersections(intersections: Vec<RayIntersection>) -> Vec<RayIntersection> {
    let mut unique = Vec::<RayIntersection>::new();
    for candidate in intersections {
        match unique.last_mut() {
            Some(current) if nearly_equal(current.x, candidate.x) => {
                if candidate.evidence.triangle_index < current.evidence.triangle_index {
                    *current = candidate;
                }
            }
            _ => unique.push(candidate),
        }
    }
    unique
}

fn intersect_x_ray(
    triangle: &MappedTriangle,
    sample_y: f64,
    sample_z: f64,
) -> Option<RayIntersection> {
    let [a, b, c] = triangle.points;
    let denominator = (b[1] - c[1]) * (a[2] - c[2]) + (c[2] - b[2]) * (a[1] - c[1]);
    let tolerance = numeric_tolerance(&[a[1], a[2], b[1], b[2], c[1], c[2]]);
    if denominator.abs() <= tolerance {
        return None;
    }
    let first =
        ((b[1] - c[1]) * (sample_z - c[2]) + (c[2] - b[2]) * (sample_y - c[1])) / denominator;
    let second =
        ((c[1] - a[1]) * (sample_z - c[2]) + (a[2] - c[2]) * (sample_y - c[1])) / denominator;
    let third = 1.0 - first - second;
    let barycentric = [first, second, third];
    if barycentric
        .iter()
        .any(|weight| *weight < -tolerance || *weight > 1.0 + tolerance)
    {
        return None;
    }
    Some(RayIntersection {
        x: a[0] * first + b[0] * second + c[0] * third,
        evidence: MaterialEvidence {
            source_material_slot: triangle.source_material_slot,
            triangle_index: triangle.triangle_index,
            barycentric,
        },
    })
}

#[derive(Debug, Clone, Copy)]
struct CellBounds {
    min: [i64; 3],
    max: [i64; 3],
}

impl CellBounds {
    fn volume(self) -> Result<u64, ConversionError> {
        (0..3).try_fold(1u64, |volume, axis| {
            let length = u64::try_from(self.max[axis] - self.min[axis] + 1)
                .map_err(|_| work_limit_error(u64::MAX, "surface candidate bounds"))?;
            volume
                .checked_mul(length)
                .ok_or_else(|| work_limit_error(u64::MAX, "surface candidate bounds"))
        })
    }
}

fn candidate_cell_bounds(points: [[f64; 3]; 3], resolution: [u32; 3]) -> Option<CellBounds> {
    let mut min = [0; 3];
    let mut max = [0; 3];
    for axis in 0..3 {
        let triangle_min = points
            .iter()
            .map(|point| point[axis])
            .fold(f64::INFINITY, f64::min);
        let triangle_max = points
            .iter()
            .map(|point| point[axis])
            .fold(f64::NEG_INFINITY, f64::max);
        let grid_max = f64::from(resolution[axis].saturating_sub(1));
        if triangle_max < -CELL_HALF_EXTENT || triangle_min > grid_max + CELL_HALF_EXTENT {
            return None;
        }
        min[axis] = (triangle_min - CELL_HALF_EXTENT)
            .ceil()
            .clamp(0.0, grid_max) as i64;
        max[axis] = (triangle_max + CELL_HALF_EXTENT)
            .floor()
            .clamp(0.0, grid_max) as i64;
        if min[axis] > max[axis] {
            return None;
        }
    }
    Some(CellBounds { min, max })
}

fn triangle_intersects_cell(points: [[f64; 3]; 3], center: [f64; 3]) -> bool {
    let relative = points.map(|point| subtract(point, center));
    let edges = [
        subtract(relative[1], relative[0]),
        subtract(relative[2], relative[1]),
        subtract(relative[0], relative[2]),
    ];
    let mut axes = Vec::with_capacity(13);
    axes.extend([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);
    axes.push(cross(edges[0], edges[1]));
    for edge in edges {
        axes.push(cross(edge, [1.0, 0.0, 0.0]));
        axes.push(cross(edge, [0.0, 1.0, 0.0]));
        axes.push(cross(edge, [0.0, 0.0, 1.0]));
    }
    axes.into_iter().all(|axis| {
        let axis_length_squared = dot(axis, axis);
        if axis_length_squared <= f64::EPSILON {
            return true;
        }
        let projections = relative.map(|point| dot(point, axis));
        let triangle_min = projections.into_iter().fold(f64::INFINITY, f64::min);
        let triangle_max = projections.into_iter().fold(f64::NEG_INFINITY, f64::max);
        let radius = CELL_HALF_EXTENT * (axis[0].abs() + axis[1].abs() + axis[2].abs());
        let tolerance =
            numeric_tolerance(&[triangle_min, triangle_max, radius, axis_length_squared]);
        triangle_min <= radius + tolerance && triangle_max >= -radius - tolerance
    })
}

fn closest_triangle_barycentric(triangle: [[f64; 3]; 3], point: [f64; 3]) -> ([f64; 3], f64) {
    let [a, b, c] = triangle;
    let ab = subtract(b, a);
    let ac = subtract(c, a);
    if dot(cross(ab, ac), cross(ab, ac)) <= f64::EPSILON {
        return closest_degenerate_barycentric(triangle, point);
    }
    let ap = subtract(point, a);
    let d1 = dot(ab, ap);
    let d2 = dot(ac, ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return ([1.0, 0.0, 0.0], distance_squared(point, a));
    }
    let bp = subtract(point, b);
    let d3 = dot(ab, bp);
    let d4 = dot(ac, bp);
    if d3 >= 0.0 && d4 <= d3 {
        return ([0.0, 1.0, 0.0], distance_squared(point, b));
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let weight = d1 / (d1 - d3);
        let barycentric = [1.0 - weight, weight, 0.0];
        return (barycentric, evidence_distance(triangle, point, barycentric));
    }
    let cp = subtract(point, c);
    let d5 = dot(ab, cp);
    let d6 = dot(ac, cp);
    if d6 >= 0.0 && d5 <= d6 {
        return ([0.0, 0.0, 1.0], distance_squared(point, c));
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let weight = d2 / (d2 - d6);
        let barycentric = [1.0 - weight, 0.0, weight];
        return (barycentric, evidence_distance(triangle, point, barycentric));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let weight = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        let barycentric = [0.0, 1.0 - weight, weight];
        return (barycentric, evidence_distance(triangle, point, barycentric));
    }
    let denominator = 1.0 / (va + vb + vc);
    let second = vb * denominator;
    let third = vc * denominator;
    let barycentric = [1.0 - second - third, second, third];
    (barycentric, evidence_distance(triangle, point, barycentric))
}

fn closest_degenerate_barycentric(triangle: [[f64; 3]; 3], point: [f64; 3]) -> ([f64; 3], f64) {
    let candidates = [
        closest_segment_barycentric(triangle[0], triangle[1], point, 0, 1),
        closest_segment_barycentric(triangle[1], triangle[2], point, 1, 2),
        closest_segment_barycentric(triangle[2], triangle[0], point, 2, 0),
    ];
    candidates
        .into_iter()
        .min_by(|left, right| left.1.total_cmp(&right.1))
        .expect("three triangle edges")
}

fn closest_segment_barycentric(
    start: [f64; 3],
    end: [f64; 3],
    point: [f64; 3],
    start_index: usize,
    end_index: usize,
) -> ([f64; 3], f64) {
    let segment = subtract(end, start);
    let length_squared = dot(segment, segment);
    let weight = if length_squared <= f64::EPSILON {
        0.0
    } else {
        (dot(subtract(point, start), segment) / length_squared).clamp(0.0, 1.0)
    };
    let mut barycentric = [0.0; 3];
    barycentric[start_index] = 1.0 - weight;
    barycentric[end_index] = weight;
    let closest = add(start, scale(segment, weight));
    (barycentric, distance_squared(point, closest))
}

fn evidence_distance(triangle: [[f64; 3]; 3], point: [f64; 3], barycentric: [f64; 3]) -> f64 {
    let closest = (0..3).fold([0.0; 3], |sum, index| {
        add(sum, scale(triangle[index], barycentric[index]))
    });
    distance_squared(point, closest)
}

fn candidate_precedes(candidate: SurfaceCandidate, current: SurfaceCandidate) -> bool {
    candidate
        .distance_squared
        .total_cmp(&current.distance_squared)
        .then_with(|| {
            candidate
                .evidence
                .source_material_slot
                .cmp(&current.evidence.source_material_slot)
        })
        .then_with(|| {
            candidate
                .evidence
                .triangle_index
                .cmp(&current.evidence.triangle_index)
        })
        .is_lt()
}

fn validate_closed_topology(mesh: &ImportedStaticMesh) -> Result<(), ConversionError> {
    let geometric_vertex_ids = geometric_vertex_ids(&mesh.positions);
    let mut faces = BTreeSet::<[usize; 3]>::new();
    let mut edges = BTreeMap::<(usize, usize), Vec<(usize, usize)>>::new();
    for triangle in &mesh.triangles {
        let [a, b, c] = triangle
            .indices
            .map(|index| geometric_vertex_ids[index as usize]);
        let mut face = [a, b, c];
        face.sort_unstable();
        if !faces.insert(face) {
            return Err(topology_error(
                "solid conversion requires unique triangle faces",
            ));
        }
        for (from, to) in [(a, b), (b, c), (c, a)] {
            let edge = if from <= to { (from, to) } else { (to, from) };
            edges.entry(edge).or_default().push((from, to));
        }
    }
    if edges.is_empty()
        || edges.values().any(|uses| uses.len() != 2)
        || edges.values().any(|uses| uses[0] == uses[1])
    {
        return Err(topology_error(
            "solid conversion requires a closed, consistently wound indexed manifold",
        ));
    }
    Ok(())
}

fn geometric_vertex_ids(positions: &[[f64; 3]]) -> Vec<usize> {
    let mut ids_by_position = BTreeMap::<[u64; 3], usize>::new();
    positions
        .iter()
        .map(|position| {
            let key = position.map(|component| {
                if component == 0.0 {
                    0.0f64.to_bits()
                } else {
                    component.to_bits()
                }
            });
            if let Some(id) = ids_by_position.get(&key) {
                *id
            } else {
                let id = ids_by_position.len();
                ids_by_position.insert(key, id);
                id
            }
        })
        .collect()
}

struct CoordinateMapper {
    source_min: [f64; 3],
    source_max: [f64; 3],
    resolution: [u32; 3],
    cell_size: f64,
    scale: [f64; 3],
    offset_cells: [f64; 3],
    origin_policy: VoxelConversionOriginPolicy,
}

impl CoordinateMapper {
    fn new(settings: &VoxelConversionSettings, bounds: VoxelizationSourceBounds) -> Self {
        let source_min = bounds.min;
        let source_max = bounds.max;
        let source_span: [f64; 3] = std::array::from_fn(|axis| source_max[axis] - source_min[axis]);
        let target_span: [f64; 3] = std::array::from_fn(|axis| {
            f64::from(settings.resolution[axis].saturating_sub(1)) * settings.cell_size
        });
        let ratios: [Option<f64>; 3] = std::array::from_fn(|axis| {
            (source_span[axis] > f64::EPSILON).then(|| target_span[axis] / source_span[axis])
        });
        let scale = match settings.fit_policy {
            VoxelConversionFitPolicy::Stretch => {
                std::array::from_fn(|axis| ratios[axis].unwrap_or(1.0))
            }
            VoxelConversionFitPolicy::Contain | VoxelConversionFitPolicy::Cover => {
                let uniform = match settings.fit_policy {
                    VoxelConversionFitPolicy::Contain => {
                        ratios.into_iter().flatten().reduce(f64::min).unwrap_or(1.0)
                    }
                    VoxelConversionFitPolicy::Cover => {
                        ratios.into_iter().flatten().reduce(f64::max).unwrap_or(1.0)
                    }
                    VoxelConversionFitPolicy::Stretch => unreachable!(),
                };
                [uniform; 3]
            }
        };
        let offset_cells = match settings.origin_policy {
            VoxelConversionOriginPolicy::SourceOrigin | VoxelConversionOriginPolicy::TargetMin => {
                [0.0; 3]
            }
            VoxelConversionOriginPolicy::Centered => std::array::from_fn(|axis| {
                ((target_span[axis] - source_span[axis] * scale[axis]) / 2.0).max(0.0)
                    / settings.cell_size
            }),
        };
        Self {
            source_min,
            source_max,
            resolution: settings.resolution,
            cell_size: settings.cell_size,
            scale,
            offset_cells,
            origin_policy: settings.origin_policy,
        }
    }

    fn map_continuous(&self, position: [f64; 3]) -> [f64; 3] {
        std::array::from_fn(|axis| {
            let anchored = if self.origin_policy == VoxelConversionOriginPolicy::SourceOrigin {
                position[axis]
            } else {
                position[axis] - self.source_min[axis]
            };
            (anchored * self.scale[axis] / self.cell_size) + self.offset_cells[axis]
        })
    }

    fn map_clamped(&self, position: [f64; 3]) -> [f64; 3] {
        let continuous = self.map_continuous(position);
        std::array::from_fn(|axis| {
            continuous[axis].clamp(0.0, f64::from(self.resolution[axis].saturating_sub(1)))
        })
    }

    fn center_bounds(&self) -> Option<CellBounds> {
        let mapped_min = self.map_clamped(self.source_min);
        let mapped_max = self.map_clamped(self.source_max);
        let mut min = [0; 3];
        let mut max = [0; 3];
        for axis in 0..3 {
            let low = mapped_min[axis].min(mapped_max[axis]);
            let high = mapped_min[axis].max(mapped_max[axis]);
            let grid_max = f64::from(self.resolution[axis].saturating_sub(1));
            if high < 0.0 || low > grid_max {
                return None;
            }
            min[axis] = low.ceil().clamp(0.0, grid_max) as i64;
            max[axis] = high.floor().clamp(0.0, grid_max) as i64;
            if min[axis] > max[axis] {
                return None;
            }
        }
        Some(CellBounds { min, max })
    }
}

fn add(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [left[0] + right[0], left[1] + right[1], left[2] + right[2]]
}

fn subtract(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}

fn scale(value: [f64; 3], scalar: f64) -> [f64; 3] {
    [value[0] * scalar, value[1] * scalar, value[2] * scalar]
}

fn dot(left: [f64; 3], right: [f64; 3]) -> f64 {
    left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

fn cross(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    ]
}

fn distance_squared(left: [f64; 3], right: [f64; 3]) -> f64 {
    let difference = subtract(left, right);
    dot(difference, difference)
}

fn nearly_equal(left: f64, right: f64) -> bool {
    (left - right).abs() <= numeric_tolerance(&[left, right])
}

fn numeric_tolerance(values: &[f64]) -> f64 {
    let scale = values.iter().copied().map(f64::abs).fold(1.0, f64::max);
    f64::EPSILON * 128.0 * scale
}

fn output_limit_error(count: usize, request: &VoxelConversionRequest) -> ConversionError {
    ConversionError::one(
        "conversion.outputLimit",
        "settings.maxOutputVoxels",
        format!(
            "conversion would produce {count} voxels; requested limit is {}",
            request.settings.max_output_voxels
        ),
    )
}

fn work_limit_error(work: u64, stage: &'static str) -> ConversionError {
    ConversionError::one(
        "conversion.resourceLimit",
        "source.triangles",
        format!(
            "geometric voxelization work {work} exceeds limit {MAX_GEOMETRIC_VOXELIZATION_WORK} during {stage}"
        ),
    )
}

fn topology_error(message: &'static str) -> ConversionError {
    ConversionError::one(
        "conversion.unsupportedTopology",
        "source.triangles",
        message,
    )
}

#[cfg(test)]
#[path = "voxelize/tests.rs"]
mod tests;
