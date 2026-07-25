use serde::{Deserialize, Serialize};

use crate::{
    validate_annotation_layer, VoxelAnnotationBounds, VoxelAnnotationError, VoxelAnnotationKind,
    VoxelAnnotationLayer, VoxelAnnotationLimits, VoxelAnnotationRegion,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum VoxelAnnotationQueryMode {
    Cell { coordinate: [i64; 3] },
    Bounds { bounds: VoxelAnnotationBounds },
    Region { region_id: String },
    LayerSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VoxelAnnotationQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_layer_hash: Option<String>,
    pub mode: VoxelAnnotationQueryMode,
    pub max_results: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VoxelAnnotationRegionReadout {
    pub region_id: String,
    pub label: String,
    pub kind: VoxelAnnotationKind,
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_region_id: Option<String>,
    pub bounds: VoxelAnnotationBounds,
    pub assigned_cell_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoxelAnnotationQueryReadout {
    pub matched_regions: Vec<VoxelAnnotationRegionReadout>,
    pub total_layer_regions: usize,
    pub truncated: bool,
    pub layer_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoxelAnnotationQueryError {
    InvalidLayer(VoxelAnnotationError),
    StaleLayerHash { expected: String, actual: String },
    InvalidLimit { maximum: usize, actual: usize },
    OutOfBounds,
}

impl std::fmt::Display for VoxelAnnotationQueryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for VoxelAnnotationQueryError {}

pub fn query_annotation_layer(
    layer: &VoxelAnnotationLayer,
    request: &VoxelAnnotationQuery,
) -> Result<VoxelAnnotationQueryReadout, VoxelAnnotationQueryError> {
    validate_annotation_layer(layer, None, VoxelAnnotationLimits::default())
        .map_err(VoxelAnnotationQueryError::InvalidLayer)?;
    if request.max_results == 0
        || request.max_results > VoxelAnnotationLimits::default().max_regions
    {
        return Err(VoxelAnnotationQueryError::InvalidLimit {
            maximum: VoxelAnnotationLimits::default().max_regions,
            actual: request.max_results,
        });
    }
    if let Some(expected) = &request.expected_layer_hash {
        if expected != &layer.content_hashes.canonical_layer {
            return Err(VoxelAnnotationQueryError::StaleLayerHash {
                expected: expected.clone(),
                actual: layer.content_hashes.canonical_layer.clone(),
            });
        }
    }

    let mut matched: Vec<_> = match &request.mode {
        VoxelAnnotationQueryMode::Cell { coordinate } => {
            if !layer.target_bounds.contains(*coordinate) {
                return Err(VoxelAnnotationQueryError::OutOfBounds);
            }
            layer
                .regions
                .iter()
                .filter(|region| contains_cell(region, *coordinate))
                .map(readout)
                .collect()
        }
        VoxelAnnotationQueryMode::Bounds { bounds } => {
            if !bounds.is_valid() || !layer.target_bounds.contains_bounds(*bounds) {
                return Err(VoxelAnnotationQueryError::OutOfBounds);
            }
            layer
                .regions
                .iter()
                .filter(|region| intersects_selection(region, *bounds))
                .map(readout)
                .collect()
        }
        VoxelAnnotationQueryMode::Region { region_id } => layer
            .regions
            .iter()
            .filter(|region| region.region_id == *region_id)
            .map(readout)
            .collect(),
        VoxelAnnotationQueryMode::LayerSummary => layer.regions.iter().map(readout).collect(),
    };
    matched.sort_by(|left, right| left.region_id.cmp(&right.region_id));
    let truncated = matched.len() > request.max_results;
    matched.truncate(request.max_results);
    Ok(VoxelAnnotationQueryReadout {
        matched_regions: matched,
        total_layer_regions: layer.regions.len(),
        truncated,
        layer_hash: layer.content_hashes.canonical_layer.clone(),
    })
}

fn readout(region: &VoxelAnnotationRegion) -> VoxelAnnotationRegionReadout {
    VoxelAnnotationRegionReadout {
        region_id: region.region_id.clone(),
        label: region.label.clone(),
        kind: region.kind,
        tags: region.tags.clone(),
        parent_region_id: region.parent_region_id.clone(),
        bounds: region.bounds,
        assigned_cell_count: region
            .selection
            .sparse_runs
            .iter()
            .map(|run| u64::from(run.length))
            .sum(),
    }
}

fn contains_cell(region: &VoxelAnnotationRegion, coordinate: [i64; 3]) -> bool {
    region.selection.sparse_runs.iter().any(|run| {
        run.start[1] == coordinate[1]
            && run.start[2] == coordinate[2]
            && run.start[0] <= coordinate[0]
            && run.end_x().is_some_and(|end| coordinate[0] <= end)
    })
}

fn intersects_selection(region: &VoxelAnnotationRegion, bounds: VoxelAnnotationBounds) -> bool {
    region
        .selection
        .sparse_runs
        .iter()
        .filter_map(|run| run.bounds())
        .any(|run| run.intersects(bounds))
}
