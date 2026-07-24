use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use sha2::{Digest, Sha256};
use voxel_asset::{
    conversion_settings_sha256, encode_voxel_asset, validate_conversion_request,
    with_computed_content_hash, VoxelAsset, VoxelAssetBounds, VoxelAssetGrid, VoxelAssetProvenance,
    VoxelAssetProvenanceKind, VoxelConversionFitPolicy, VoxelConversionMode,
    VoxelConversionOriginPolicy, VoxelConversionRequest, VoxelConversionSettings,
    VoxelCoordinateSystem, VoxelRepresentation, VoxelRepresentationKind, VoxelSparseRun,
    VOXEL_ASSET_SCHEMA_VERSION,
};

use crate::{ConversionError, ImportedStaticMesh};

pub const CONVERTER_ID: &str = "rusty-engine.mesh-to-voxel.v1";
pub const MAX_SURFACE_SAMPLE_WORK: u64 = 10_000_000;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversionReceipt {
    pub asset: VoxelAsset,
    pub canonical_json: String,
    pub source_sha256: String,
    pub settings_sha256: String,
    pub content_hash: String,
    pub source_vertices: usize,
    pub source_triangles: usize,
    pub output_voxels: usize,
    pub sparse_runs: usize,
    pub bounds: VoxelAssetBounds,
}

pub fn convert_glb(
    request: &VoxelConversionRequest,
    source: &[u8],
) -> Result<ConversionReceipt, ConversionError> {
    validate_conversion_request(request, source.len() as u64)?;
    let source_sha256 = sha256(source);
    if source_sha256 != request.expected_source_sha256 {
        return Err(ConversionError::one(
            "conversion.sourceHashMismatch",
            "expectedSourceSha256",
            format!(
                "expected {}, computed {source_sha256}",
                request.expected_source_sha256
            ),
        ));
    }

    let mesh = crate::import_static_glb(source)?;
    validate_material_map(request, &mesh)?;
    let cells = convert_cells(request, &mesh)?;
    let bounds = bounds_for_cells(&cells).expect("conversion rejects empty output");
    let sparse_runs = sparse_runs(&cells);
    let settings_sha256 = conversion_settings_sha256(&request.settings);
    let asset = with_computed_content_hash(VoxelAsset {
        schema_version: VOXEL_ASSET_SCHEMA_VERSION,
        asset_id: request.asset_id.clone(),
        grid: VoxelAssetGrid {
            coordinate_system: VoxelCoordinateSystem::RightHandedYUp,
            cell_size: request.settings.cell_size,
            chunk_size: request.settings.chunk_size,
            origin: request.settings.origin,
        },
        bounds,
        representation: VoxelRepresentation {
            kind: VoxelRepresentationKind::SparseRuns,
            sparse_runs,
        },
        material_map: request.settings.material_map.clone(),
        provenance: VoxelAssetProvenance {
            kind: VoxelAssetProvenanceKind::ConvertedStaticMesh,
            source_path: request.source_path.clone(),
            source_sha256: source_sha256.clone(),
            source_byte_count: source.len() as u64,
            converter: CONVERTER_ID.to_string(),
            settings_sha256: settings_sha256.clone(),
            license_path: request.license_path.clone(),
        },
        content_hash: String::new(),
    })
    .map_err(asset_error)?;
    let canonical_json = encode_voxel_asset(&asset).map_err(asset_error)?;
    let content_hash = asset.content_hash.clone();
    let output_voxels = asset
        .representation
        .sparse_runs
        .iter()
        .map(|run| run.length as usize)
        .sum();

    Ok(ConversionReceipt {
        source_vertices: mesh.positions.len(),
        source_triangles: mesh.triangles.len(),
        output_voxels,
        sparse_runs: asset.representation.sparse_runs.len(),
        bounds,
        asset,
        canonical_json,
        source_sha256,
        settings_sha256,
        content_hash,
    })
}

fn validate_material_map(
    request: &VoxelConversionRequest,
    mesh: &ImportedStaticMesh,
) -> Result<(), ConversionError> {
    let imported = mesh
        .materials
        .iter()
        .map(|material| {
            (
                material.source_material_slot,
                &material.source_material_name,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let requested = request
        .settings
        .material_map
        .iter()
        .enumerate()
        .map(|(index, mapping)| (mapping.source_material_slot, (index, mapping)))
        .collect::<BTreeMap<_, _>>();

    for (slot, name) in &imported {
        let Some((index, mapping)) = requested.get(slot) else {
            return Err(ConversionError::one(
                "conversion.materialMapMismatch",
                "settings.materialMap",
                format!("source material slot {slot} ({name:?}) has no mapping"),
            ));
        };
        if let Some(expected_name) = &mapping.source_material_name {
            if name.as_deref() != Some(expected_name.as_str()) {
                return Err(ConversionError::one(
                    "conversion.materialMapMismatch",
                    format!("settings.materialMap[{index}].sourceMaterialName"),
                    format!("expected source material name {expected_name:?}, imported {name:?}"),
                ));
            }
        }
    }
    let extras = requested
        .keys()
        .filter(|slot| !imported.contains_key(slot))
        .copied()
        .collect::<Vec<_>>();
    if !extras.is_empty() {
        return Err(ConversionError::one(
            "conversion.materialMapMismatch",
            "settings.materialMap",
            format!("material mappings reference absent source slots {extras:?}"),
        ));
    }
    Ok(())
}

fn convert_cells(
    request: &VoxelConversionRequest,
    mesh: &ImportedStaticMesh,
) -> Result<BTreeMap<[i64; 3], u16>, ConversionError> {
    let mapper = CoordinateMapper::new(&request.settings, &mesh.positions);
    let material_map = request
        .settings
        .material_map
        .iter()
        .map(|mapping| (mapping.source_material_slot, mapping.voxel_material_slot))
        .collect::<BTreeMap<_, _>>();
    let mut source_cells = sampled_surface_cells(request, mesh, &mapper, &material_map)?;
    if request.settings.mode == VoxelConversionMode::Solid {
        validate_closed_topology(mesh)?;
        fill_solid_bounds(request, mesh, &mapper, &material_map, &mut source_cells)?;
    }
    if source_cells.is_empty() {
        return Err(ConversionError::one(
            "conversion.invalidGeometry",
            "source",
            "conversion produced no voxels",
        ));
    }
    Ok(source_cells
        .into_iter()
        .map(|(coordinate, (_, material))| (coordinate, material))
        .collect())
}

fn sampled_surface_cells(
    request: &VoxelConversionRequest,
    mesh: &ImportedStaticMesh,
    mapper: &CoordinateMapper,
    material_map: &BTreeMap<u32, u16>,
) -> Result<BTreeMap<[i64; 3], (u32, u16)>, ConversionError> {
    let mut cells = BTreeMap::<[i64; 3], (u32, u16)>::new();
    let mut work = 0u64;
    for triangle in &mesh.triangles {
        let points = triangle
            .indices
            .map(|index| mapper.map_continuous(mesh.positions[index as usize]));
        let max_edge = distance(points[0], points[1])
            .max(distance(points[1], points[2]))
            .max(distance(points[2], points[0]));
        let steps = (max_edge * 2.0).ceil().max(1.0) as u32;
        let triangle_work = u64::from(steps + 1)
            .checked_mul(u64::from(steps + 2))
            .and_then(|value| value.checked_div(2))
            .ok_or_else(|| work_limit_error(work))?;
        work = work
            .checked_add(triangle_work)
            .ok_or_else(|| work_limit_error(work))?;
        if work > MAX_SURFACE_SAMPLE_WORK {
            return Err(work_limit_error(work));
        }
        let material = material_map[&triangle.source_material_slot];
        for a in 0..=steps {
            for b in 0..=(steps - a) {
                let u = f64::from(a) / f64::from(steps);
                let v = f64::from(b) / f64::from(steps);
                let w = 1.0 - u - v;
                let point = [
                    points[0][0] * u + points[1][0] * v + points[2][0] * w,
                    points[0][1] * u + points[1][1] * v + points[2][1] * w,
                    points[0][2] * u + points[1][2] * v + points[2][2] * w,
                ];
                let coordinate = mapper.round_clamped(point);
                let candidate = (triangle.source_material_slot, material);
                match cells.get_mut(&coordinate) {
                    Some(current) if candidate.0 < current.0 => *current = candidate,
                    Some(_) => {}
                    None => {
                        cells.insert(coordinate, candidate);
                    }
                }
            }
        }
        if cells.len() > request.settings.max_output_voxels as usize {
            return Err(output_limit_error(cells.len(), request));
        }
    }
    Ok(cells)
}

fn fill_solid_bounds(
    request: &VoxelConversionRequest,
    mesh: &ImportedStaticMesh,
    mapper: &CoordinateMapper,
    material_map: &BTreeMap<u32, u16>,
    cells: &mut BTreeMap<[i64; 3], (u32, u16)>,
) -> Result<(), ConversionError> {
    let mapped = mesh
        .positions
        .iter()
        .map(|position| mapper.map(*position))
        .collect::<Vec<_>>();
    let bounds = bounds_for_coordinates(&mapped).expect("mesh has positions");
    let dimensions = (0..3)
        .map(|axis| (bounds.max[axis] - bounds.min[axis] + 1) as u64)
        .collect::<Vec<_>>();
    let volume = dimensions
        .into_iter()
        .try_fold(1u64, |total, dimension| total.checked_mul(dimension))
        .ok_or_else(|| output_limit_error(usize::MAX, request))?;
    if volume > u64::from(request.settings.max_output_voxels) {
        return Err(output_limit_error(volume as usize, request));
    }
    let default_source_slot = mesh
        .materials
        .first()
        .expect("mesh has materials")
        .source_material_slot;
    let default_material = material_map[&default_source_slot];
    for z in bounds.min[2]..=bounds.max[2] {
        for y in bounds.min[1]..=bounds.max[1] {
            for x in bounds.min[0]..=bounds.max[0] {
                cells
                    .entry([x, y, z])
                    .or_insert((default_source_slot, default_material));
            }
        }
    }
    Ok(())
}

fn validate_closed_topology(mesh: &ImportedStaticMesh) -> Result<(), ConversionError> {
    let mut faces = BTreeSet::<[u32; 3]>::new();
    let mut edges = BTreeMap::<(u32, u32), Vec<(u32, u32)>>::new();
    for triangle in &mesh.triangles {
        let [a, b, c] = triangle.indices;
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

fn sparse_runs(cells: &BTreeMap<[i64; 3], u16>) -> Vec<VoxelSparseRun> {
    let mut rows = BTreeMap::<(i64, i64), Vec<(i64, u16)>>::new();
    for (coordinate, material) in cells {
        rows.entry((coordinate[1], coordinate[2]))
            .or_default()
            .push((coordinate[0], *material));
    }
    let mut runs = Vec::new();
    for ((y, z), row) in &mut rows {
        row.sort_unstable();
        for (x, material) in row.iter().copied() {
            let extend = runs.last_mut().is_some_and(|prior: &mut VoxelSparseRun| {
                prior.start[1] == *y
                    && prior.start[2] == *z
                    && prior.material_slot == material
                    && prior.start[0] + i64::from(prior.length) == x
            });
            if extend {
                runs.last_mut().expect("checked last run").length += 1;
            } else {
                runs.push(VoxelSparseRun {
                    start: [x, *y, *z],
                    length: 1,
                    material_slot: material,
                });
            }
        }
    }
    runs
}

struct CoordinateMapper {
    source_min: [f64; 3],
    resolution: [u32; 3],
    cell_size: f64,
    scale: [f64; 3],
    offset_cells: [f64; 3],
}

impl CoordinateMapper {
    fn new(settings: &VoxelConversionSettings, positions: &[[f64; 3]]) -> Self {
        let mut source_min = [f64::INFINITY; 3];
        let mut source_max = [f64::NEG_INFINITY; 3];
        for position in positions {
            for axis in 0..3 {
                source_min[axis] = source_min[axis].min(position[axis]);
                source_max[axis] = source_max[axis].max(position[axis]);
            }
        }
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
            VoxelConversionFitPolicy::Contain => {
                let uniform = ratios.into_iter().flatten().reduce(f64::min).unwrap_or(1.0);
                [uniform; 3]
            }
        };
        let offset_cells = match settings.origin_policy {
            VoxelConversionOriginPolicy::TargetMin => [0.0; 3],
            VoxelConversionOriginPolicy::Centered => std::array::from_fn(|axis| {
                ((target_span[axis] - source_span[axis] * scale[axis]) / 2.0).max(0.0)
                    / settings.cell_size
            }),
        };
        Self {
            source_min,
            resolution: settings.resolution,
            cell_size: settings.cell_size,
            scale,
            offset_cells,
        }
    }

    fn map_continuous(&self, position: [f64; 3]) -> [f64; 3] {
        std::array::from_fn(|axis| {
            ((position[axis] - self.source_min[axis]) * self.scale[axis] / self.cell_size)
                + self.offset_cells[axis]
        })
    }

    fn round_clamped(&self, continuous: [f64; 3]) -> [i64; 3] {
        std::array::from_fn(|axis| {
            continuous[axis]
                .round()
                .clamp(0.0, f64::from(self.resolution[axis].saturating_sub(1))) as i64
        })
    }

    fn map(&self, position: [f64; 3]) -> [i64; 3] {
        self.round_clamped(self.map_continuous(position))
    }
}

fn bounds_for_cells(cells: &BTreeMap<[i64; 3], u16>) -> Option<VoxelAssetBounds> {
    bounds_for_coordinates(&cells.keys().copied().collect::<Vec<_>>())
}

fn bounds_for_coordinates(coordinates: &[[i64; 3]]) -> Option<VoxelAssetBounds> {
    let first = *coordinates.first()?;
    let mut min = first;
    let mut max = first;
    for coordinate in coordinates.iter().skip(1) {
        for axis in 0..3 {
            min[axis] = min[axis].min(coordinate[axis]);
            max[axis] = max[axis].max(coordinate[axis]);
        }
    }
    Some(VoxelAssetBounds { min, max })
}

fn distance(left: [f64; 3], right: [f64; 3]) -> f64 {
    ((left[0] - right[0]).powi(2) + (left[1] - right[1]).powi(2) + (left[2] - right[2]).powi(2))
        .sqrt()
}

fn sha256(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
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

fn work_limit_error(work: u64) -> ConversionError {
    ConversionError::one(
        "conversion.resourceLimit",
        "source.triangles",
        format!("surface sampling work {work} exceeds limit {MAX_SURFACE_SAMPLE_WORK}"),
    )
}

fn topology_error(message: &'static str) -> ConversionError {
    ConversionError::one(
        "conversion.unsupportedTopology",
        "source.triangles",
        message,
    )
}

fn asset_error(error: voxel_asset::VoxelAssetError) -> ConversionError {
    let first = error
        .diagnostics()
        .first()
        .expect("asset error has diagnostic");
    ConversionError::one(first.code, first.path.clone(), first.message.clone())
}
