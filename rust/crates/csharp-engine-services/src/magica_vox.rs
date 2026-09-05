//! Bounded in-memory admission of the ordinary MagicaVoxel v150 model form.
//!
//! This is deliberately not a general MagicaVoxel scene importer. It accepts
//! only one `SIZE`, one `XYZI`, and one `RGBA` model, optionally accompanied
//! by a bounded single-model scene graph and inert metadata. It is the direct
//! occupied-voxel route used by the current product corpus.

use std::collections::{BTreeMap, BTreeSet};

use sha2::{Digest, Sha256};
use voxel_asset::{
    with_computed_voxel_frame_hash, with_computed_voxel_object_hashes, VoxelAssetBounds,
    VoxelAssetMaterialBinding, VoxelAssetMaterialMapping, VoxelCoordinateSystem, VoxelFrame,
    VoxelObjectAsset, VoxelObjectGrid, VoxelObjectProvenance, VoxelObjectProvenanceKind,
    VoxelRepresentation, VoxelRepresentationKind, VoxelSparseRun, VOXEL_OBJECT_SCHEMA_VERSION,
};

use csharp_engine_abi::{NativeMagicaVoxelOrientation, NativeMagicaVoxelPivotPolicy};

pub(crate) const MAX_MAGICA_VOX_SOURCE_BYTES: u64 = 8 * 1024 * 1024;
pub(crate) const MAX_MAGICA_VOX_DIMENSION: u32 = 256;
pub(crate) const MAX_MAGICA_VOX_VOXELS: u64 = 1_000_000;
/// A regular MagicaVoxel export can carry one `MATL` chunk for each of its
/// 256 palette metadata entries, plus a compact scene graph and notes. This is a cap
/// on direct children, not a recursive scene import budget.
pub(crate) const MAX_MAGICA_VOX_CHUNKS: u32 = 384;
pub(crate) const MAX_MAGICA_VOX_MATERIAL_SLOTS: u32 = 255;
const MAX_MAGICA_VOX_METADATA_ENTRIES: usize = 1024;

#[derive(Debug, Clone, Copy)]
pub(crate) struct MagicaVoxelAdmissionOptions {
    pub cell_size: f64,
    pub pivot_policy: NativeMagicaVoxelPivotPolicy,
    pub explicit_pivot: [f64; 3],
    pub orientation: NativeMagicaVoxelOrientation,
    pub max_source_bytes: u64,
    pub max_dimension: u32,
    pub max_voxel_count: u64,
    pub max_chunk_count: u32,
    pub max_material_slots: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MagicaVoxelPaletteRow {
    pub material_slot: u16,
    pub source_color_index: u8,
    pub rgba: [u8; 4],
}

#[derive(Debug)]
pub(crate) struct MagicaVoxelAdmission {
    pub object: VoxelObjectAsset,
    pub palette: Vec<MagicaVoxelPaletteRow>,
    pub source_hash: String,
    pub source_byte_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MagicaVoxelError {
    InvalidOptions(&'static str),
    SourceLimit,
    InvalidHeader,
    UnsupportedVersion(u32),
    Truncated,
    InvalidChunk,
    UnsupportedChunk([u8; 4]),
    DuplicateChunk([u8; 4]),
    MissingChunk([u8; 4]),
    InvalidSize,
    DimensionLimit,
    VoxelLimit,
    InvalidVoxelPayload,
    InvalidPalettePayload,
    DuplicateCell,
    InvalidColorIndex,
    InvalidScene,
    UnsupportedScene,
    Canonical(String),
}

#[derive(Debug)]
enum SceneNode {
    Transform { child: i32 },
    Group { children: Vec<i32> },
    Shape { model_id: i32 },
}

impl std::fmt::Display for MagicaVoxelError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for MagicaVoxelError {}

pub(crate) fn admit_magica_vox(
    bytes: &[u8],
    asset_id: String,
    source_path: String,
    options: MagicaVoxelAdmissionOptions,
) -> Result<MagicaVoxelAdmission, MagicaVoxelError> {
    validate_options(options)?;
    if u64::try_from(bytes.len()).map_err(|_| MagicaVoxelError::SourceLimit)?
        > options.max_source_bytes
    {
        return Err(MagicaVoxelError::SourceLimit);
    }
    if bytes.len() < 20 || &bytes[..4] != b"VOX " {
        return Err(MagicaVoxelError::InvalidHeader);
    }
    if read_u32(&bytes[4..8])? != 150 {
        return Err(MagicaVoxelError::UnsupportedVersion(read_u32(
            &bytes[4..8],
        )?));
    }
    if &bytes[8..12] != b"MAIN" || read_u32(&bytes[12..16])? != 0 {
        return Err(MagicaVoxelError::InvalidHeader);
    }
    let children_len =
        usize::try_from(read_u32(&bytes[16..20])?).map_err(|_| MagicaVoxelError::Truncated)?;
    if bytes.len().checked_sub(20) != Some(children_len) {
        return Err(MagicaVoxelError::Truncated);
    }

    let mut cursor = 20usize;
    let mut chunk_count = 0u32;
    let mut size = None;
    let mut voxels = None;
    let mut palette = None;
    let mut scene_nodes = BTreeMap::new();
    let mut material_ids = BTreeSet::new();
    let mut layer_ids = BTreeSet::new();
    while cursor < bytes.len() {
        let (kind, content, children) = parse_chunk(bytes, &mut cursor)?;
        chunk_count = chunk_count
            .checked_add(1)
            .ok_or(MagicaVoxelError::InvalidChunk)?;
        if chunk_count > options.max_chunk_count {
            return Err(MagicaVoxelError::InvalidChunk);
        }
        if !children.is_empty() {
            return Err(MagicaVoxelError::UnsupportedChunk(kind));
        }
        match &kind {
            b"SIZE" => {
                if size
                    .replace(parse_size(content, options.max_dimension)?)
                    .is_some()
                {
                    return Err(MagicaVoxelError::DuplicateChunk(kind));
                }
            }
            b"XYZI" => {
                if voxels
                    .replace(parse_voxels(content, options.max_voxel_count)?)
                    .is_some()
                {
                    return Err(MagicaVoxelError::DuplicateChunk(kind));
                }
            }
            b"RGBA" => {
                if palette.replace(parse_palette(content)?).is_some() {
                    return Err(MagicaVoxelError::DuplicateChunk(kind));
                }
            }
            // These metadata chunks are parsed and bounded for source
            // integrity, but deliberately do not affect ordinary matte
            // palette materials selected by the product.
            b"NOTE" => parse_note(content)?,
            b"MATL" => {
                let material_id = parse_material(content)?;
                if !material_ids.insert(material_id) {
                    return Err(MagicaVoxelError::InvalidScene);
                }
            }
            b"LAYR" => {
                let layer_id = parse_layer(content)?;
                if !layer_ids.insert(layer_id) {
                    return Err(MagicaVoxelError::InvalidScene);
                }
            }
            b"nTRN" | b"nGRP" | b"nSHP" => {
                let (node_id, node) = parse_scene_node(kind, content)?;
                if scene_nodes.insert(node_id, node).is_some() {
                    return Err(MagicaVoxelError::InvalidScene);
                }
            }
            // `PACK` announces multiple model records. Single-model source
            // admission intentionally requires it to be absent.
            b"PACK" => return Err(MagicaVoxelError::UnsupportedScene),
            _ => return Err(MagicaVoxelError::UnsupportedChunk(kind)),
        }
    }
    let dimensions = size.ok_or(MagicaVoxelError::MissingChunk(*b"SIZE"))?;
    let voxels = voxels.ok_or(MagicaVoxelError::MissingChunk(*b"XYZI"))?;
    let colors = palette.ok_or(MagicaVoxelError::MissingChunk(*b"RGBA"))?;
    if !scene_nodes.is_empty() {
        validate_single_model_scene(&scene_nodes)?;
    }

    let mut cells = BTreeMap::<[i64; 3], u16>::new();
    let mut used_indices = BTreeSet::new();
    for voxel in voxels {
        if u32::from(voxel[0]) >= dimensions[0]
            || u32::from(voxel[1]) >= dimensions[1]
            || u32::from(voxel[2]) >= dimensions[2]
        {
            return Err(MagicaVoxelError::InvalidVoxelPayload);
        }
        let color_index = voxel[3];
        if color_index == 0 {
            return Err(MagicaVoxelError::InvalidColorIndex);
        }
        let coordinate = convert_coordinate(voxel, options.orientation)?;
        if cells.insert(coordinate, u16::from(color_index)).is_some() {
            return Err(MagicaVoxelError::DuplicateCell);
        }
        used_indices.insert(color_index);
    }
    if used_indices.is_empty() || used_indices.len() > options.max_material_slots as usize {
        return Err(MagicaVoxelError::VoxelLimit);
    }

    let sparse_runs = sparse_runs(cells)?;
    let bounds = bounds(&sparse_runs)?;
    let pivot = pivot(bounds, options)?;
    let palette_rows = used_indices
        .iter()
        .map(|&index| MagicaVoxelPaletteRow {
            material_slot: u16::from(index),
            source_color_index: index,
            rgba: colors[usize::from(index - 1)],
        })
        .collect::<Vec<_>>();
    let material_palette = palette_rows
        .iter()
        .map(|row| VoxelAssetMaterialBinding {
            material_slot: row.material_slot,
            material_asset_id: format!("material/magica-vox-color-{:03}", row.source_color_index),
            display_name: Some(format!("MagicaVoxel color {}", row.source_color_index)),
        })
        .collect::<Vec<_>>();
    let material_map = palette_rows
        .iter()
        .map(|row| VoxelAssetMaterialMapping {
            source_material_slot: u32::from(row.source_color_index),
            source_material_name: None,
            voxel_material_slot: row.material_slot,
        })
        .collect::<Vec<_>>();
    let frame = with_computed_voxel_frame_hash(
        VoxelFrame {
            bounds,
            representation: VoxelRepresentation {
                kind: VoxelRepresentationKind::SparseRuns,
                sparse_runs,
            },
            voxel_data_hash: String::new(),
        },
        material_palette.iter().map(|binding| binding.material_slot),
    )
    .map_err(|error| MagicaVoxelError::Canonical(error.to_string()))?;
    let source_hash = format!("sha256:{:x}", Sha256::digest(bytes));
    let settings_hash = settings_hash(options);
    // Scene `_r`/`_t` facts are structurally validated above but intentionally
    // not applied. This operation admits source-model occupancy; the product
    // explicitly selects Engine-local orientation, pivot, and placement in
    // the typed request rather than inheriting hidden scene placement.
    let object = with_computed_voxel_object_hashes(VoxelObjectAsset {
        schema_version: VOXEL_OBJECT_SCHEMA_VERSION,
        asset_id,
        grid: VoxelObjectGrid {
            coordinate_system: VoxelCoordinateSystem::RightHandedYUp,
            cell_size: options.cell_size,
            chunk_size: 32,
            pivot,
        },
        bounds,
        default_frame: frame,
        clips: Vec::new(),
        default_clip: None,
        material_palette,
        material_map,
        provenance: VoxelObjectProvenance {
            kind: VoxelObjectProvenanceKind::Authored,
            source_path,
            source_sha256: source_hash.clone(),
            source_byte_count: bytes.len() as u64,
            converter: "magica-vox-v150".to_string(),
            settings_sha256: settings_hash,
            license_path: None,
            source_clips: Vec::new(),
        },
        content_hash: String::new(),
    })
    .map_err(|error| MagicaVoxelError::Canonical(error.to_string()))?;
    Ok(MagicaVoxelAdmission {
        object,
        palette: palette_rows,
        source_hash,
        source_byte_count: bytes.len() as u64,
    })
}

fn validate_options(options: MagicaVoxelAdmissionOptions) -> Result<(), MagicaVoxelError> {
    if !options.cell_size.is_finite() || options.cell_size <= 0.0 {
        return Err(MagicaVoxelError::InvalidOptions("cell size"));
    }
    if !options.explicit_pivot.iter().all(|value| value.is_finite()) {
        return Err(MagicaVoxelError::InvalidOptions("pivot"));
    }
    if options.orientation != NativeMagicaVoxelOrientation::XRightYUpNegativeZForward {
        return Err(MagicaVoxelError::InvalidOptions("orientation"));
    }
    if !matches!(
        options.pivot_policy,
        NativeMagicaVoxelPivotPolicy::Explicit
            | NativeMagicaVoxelPivotPolicy::BoundsCenter
            | NativeMagicaVoxelPivotPolicy::BaseCenter
    ) {
        return Err(MagicaVoxelError::InvalidOptions("pivot policy"));
    }
    if options.max_source_bytes == 0 || options.max_source_bytes > MAX_MAGICA_VOX_SOURCE_BYTES {
        return Err(MagicaVoxelError::InvalidOptions("source bytes"));
    }
    if options.max_dimension == 0 || options.max_dimension > MAX_MAGICA_VOX_DIMENSION {
        return Err(MagicaVoxelError::InvalidOptions("dimension"));
    }
    if options.max_voxel_count == 0 || options.max_voxel_count > MAX_MAGICA_VOX_VOXELS {
        return Err(MagicaVoxelError::InvalidOptions("voxel count"));
    }
    if options.max_chunk_count < 3 || options.max_chunk_count > MAX_MAGICA_VOX_CHUNKS {
        return Err(MagicaVoxelError::InvalidOptions("chunk count"));
    }
    if options.max_material_slots == 0 || options.max_material_slots > MAX_MAGICA_VOX_MATERIAL_SLOTS
    {
        return Err(MagicaVoxelError::InvalidOptions("material slots"));
    }
    Ok(())
}

type ParsedChunk<'a> = ([u8; 4], &'a [u8], &'a [u8]);

fn parse_chunk<'a>(
    bytes: &'a [u8],
    cursor: &mut usize,
) -> Result<ParsedChunk<'a>, MagicaVoxelError> {
    let header_end = cursor.checked_add(12).ok_or(MagicaVoxelError::Truncated)?;
    let header = bytes
        .get(*cursor..header_end)
        .ok_or(MagicaVoxelError::Truncated)?;
    let kind = [header[0], header[1], header[2], header[3]];
    let content_len =
        usize::try_from(read_u32(&header[4..8])?).map_err(|_| MagicaVoxelError::Truncated)?;
    let children_len =
        usize::try_from(read_u32(&header[8..12])?).map_err(|_| MagicaVoxelError::Truncated)?;
    let content_start = header_end;
    let content_end = content_start
        .checked_add(content_len)
        .ok_or(MagicaVoxelError::Truncated)?;
    let children_end = content_end
        .checked_add(children_len)
        .ok_or(MagicaVoxelError::Truncated)?;
    let content = bytes
        .get(content_start..content_end)
        .ok_or(MagicaVoxelError::Truncated)?;
    let children = bytes
        .get(content_end..children_end)
        .ok_or(MagicaVoxelError::Truncated)?;
    *cursor = children_end;
    Ok((kind, content, children))
}

fn parse_size(content: &[u8], max_dimension: u32) -> Result<[u32; 3], MagicaVoxelError> {
    if content.len() != 12 {
        return Err(MagicaVoxelError::InvalidSize);
    }
    let dimensions = [
        read_u32(&content[0..4])?,
        read_u32(&content[4..8])?,
        read_u32(&content[8..12])?,
    ];
    if dimensions
        .iter()
        .any(|&dimension| dimension == 0 || dimension > max_dimension)
    {
        return Err(MagicaVoxelError::DimensionLimit);
    }
    Ok(dimensions)
}

fn parse_voxels(content: &[u8], max_voxels: u64) -> Result<Vec<[u8; 4]>, MagicaVoxelError> {
    if content.len() < 4 {
        return Err(MagicaVoxelError::InvalidVoxelPayload);
    }
    let count =
        usize::try_from(read_u32(&content[..4])?).map_err(|_| MagicaVoxelError::VoxelLimit)?;
    if count == 0 || count as u64 > max_voxels {
        return Err(MagicaVoxelError::VoxelLimit);
    }
    let expected = count.checked_mul(4).and_then(|value| value.checked_add(4));
    if expected != Some(content.len()) {
        return Err(MagicaVoxelError::InvalidVoxelPayload);
    }
    Ok(content[4..].as_chunks::<4>().0.to_vec())
}

fn parse_palette(content: &[u8]) -> Result<[[u8; 4]; 256], MagicaVoxelError> {
    if content.len() != 256 * 4 {
        return Err(MagicaVoxelError::InvalidPalettePayload);
    }
    let mut colors = [[0; 4]; 256];
    for (index, chunk) in content.as_chunks::<4>().0.iter().enumerate() {
        colors[index] = *chunk;
    }
    Ok(colors)
}

fn parse_note(content: &[u8]) -> Result<(), MagicaVoxelError> {
    let mut cursor = 0;
    let count = read_i32_at(content, &mut cursor)?;
    if count < 0 || count as usize > MAX_MAGICA_VOX_METADATA_ENTRIES {
        return Err(MagicaVoxelError::InvalidScene);
    }
    for _ in 0..count {
        skip_string(content, &mut cursor)?;
    }
    expect_end(content, cursor)
}

fn parse_material(content: &[u8]) -> Result<i32, MagicaVoxelError> {
    let mut cursor = 0;
    let material_id = read_i32_at(content, &mut cursor)?;
    // MATL ids are metadata ids, not the 1-based voxel color indices. Common
    // exporters emit the full 0..=255 table even though index 0 is never an
    // occupied voxel color.
    if !(0..=255).contains(&material_id) {
        return Err(MagicaVoxelError::InvalidScene);
    }
    skip_dictionary(content, &mut cursor)?;
    expect_end(content, cursor)?;
    Ok(material_id)
}

fn parse_layer(content: &[u8]) -> Result<i32, MagicaVoxelError> {
    let mut cursor = 0;
    let layer_id = read_i32_at(content, &mut cursor)?;
    if layer_id < 0 {
        return Err(MagicaVoxelError::InvalidScene);
    }
    skip_dictionary(content, &mut cursor)?;
    let _reserved = read_i32_at(content, &mut cursor)?;
    expect_end(content, cursor)?;
    Ok(layer_id)
}

fn parse_scene_node(kind: [u8; 4], content: &[u8]) -> Result<(i32, SceneNode), MagicaVoxelError> {
    let mut cursor = 0;
    let node_id = read_i32_at(content, &mut cursor)?;
    if node_id < 0 {
        return Err(MagicaVoxelError::InvalidScene);
    }
    skip_dictionary(content, &mut cursor)?;
    let node = match &kind {
        b"nTRN" => {
            let child = read_i32_at(content, &mut cursor)?;
            let _reserved = read_i32_at(content, &mut cursor)?;
            let _layer = read_i32_at(content, &mut cursor)?;
            let frames = read_i32_at(content, &mut cursor)?;
            if child < 0 || frames != 1 {
                return Err(MagicaVoxelError::UnsupportedScene);
            }
            skip_dictionary(content, &mut cursor)?;
            SceneNode::Transform { child }
        }
        b"nGRP" => {
            let child_count = read_i32_at(content, &mut cursor)?;
            if child_count <= 0 || child_count as usize > MAX_MAGICA_VOX_METADATA_ENTRIES {
                return Err(MagicaVoxelError::InvalidScene);
            }
            let mut children = Vec::with_capacity(child_count as usize);
            for _ in 0..child_count {
                let child = read_i32_at(content, &mut cursor)?;
                if child < 0 {
                    return Err(MagicaVoxelError::InvalidScene);
                }
                children.push(child);
            }
            SceneNode::Group { children }
        }
        b"nSHP" => {
            let model_count = read_i32_at(content, &mut cursor)?;
            if model_count != 1 {
                return Err(MagicaVoxelError::UnsupportedScene);
            }
            let model_id = read_i32_at(content, &mut cursor)?;
            if model_id != 0 {
                return Err(MagicaVoxelError::UnsupportedScene);
            }
            skip_dictionary(content, &mut cursor)?;
            SceneNode::Shape { model_id }
        }
        _ => return Err(MagicaVoxelError::InvalidScene),
    };
    expect_end(content, cursor)?;
    Ok((node_id, node))
}

fn validate_single_model_scene(nodes: &BTreeMap<i32, SceneNode>) -> Result<(), MagicaVoxelError> {
    if !matches!(nodes.get(&0), Some(SceneNode::Transform { .. })) {
        return Err(MagicaVoxelError::InvalidScene);
    }
    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    let mut shape_count = 0usize;
    resolve_scene_node(0, nodes, &mut visiting, &mut visited, &mut shape_count)?;
    if visited.len() != nodes.len() || shape_count != 1 {
        return Err(MagicaVoxelError::UnsupportedScene);
    }
    Ok(())
}

fn resolve_scene_node(
    node_id: i32,
    nodes: &BTreeMap<i32, SceneNode>,
    visiting: &mut BTreeSet<i32>,
    visited: &mut BTreeSet<i32>,
    shape_count: &mut usize,
) -> Result<(), MagicaVoxelError> {
    if !visiting.insert(node_id) || visited.contains(&node_id) {
        return Err(MagicaVoxelError::UnsupportedScene);
    }
    let node = nodes.get(&node_id).ok_or(MagicaVoxelError::InvalidScene)?;
    match node {
        SceneNode::Transform { child } => {
            resolve_scene_node(*child, nodes, visiting, visited, shape_count)?;
        }
        SceneNode::Group { children } => {
            for child in children {
                resolve_scene_node(*child, nodes, visiting, visited, shape_count)?;
            }
        }
        SceneNode::Shape { model_id } => {
            if *model_id != 0 {
                return Err(MagicaVoxelError::UnsupportedScene);
            }
            *shape_count = shape_count
                .checked_add(1)
                .ok_or(MagicaVoxelError::InvalidScene)?;
        }
    }
    visiting.remove(&node_id);
    visited.insert(node_id);
    Ok(())
}

fn read_i32_at(bytes: &[u8], cursor: &mut usize) -> Result<i32, MagicaVoxelError> {
    let end = cursor.checked_add(4).ok_or(MagicaVoxelError::Truncated)?;
    let value = bytes.get(*cursor..end).ok_or(MagicaVoxelError::Truncated)?;
    *cursor = end;
    Ok(i32::from_le_bytes(
        value.try_into().map_err(|_| MagicaVoxelError::Truncated)?,
    ))
}

fn skip_dictionary(bytes: &[u8], cursor: &mut usize) -> Result<(), MagicaVoxelError> {
    let count = read_i32_at(bytes, cursor)?;
    if count < 0 || count as usize > MAX_MAGICA_VOX_METADATA_ENTRIES {
        return Err(MagicaVoxelError::InvalidScene);
    }
    for _ in 0..count {
        skip_string(bytes, cursor)?;
        skip_string(bytes, cursor)?;
    }
    Ok(())
}

fn skip_string(bytes: &[u8], cursor: &mut usize) -> Result<(), MagicaVoxelError> {
    let length = read_i32_at(bytes, cursor)?;
    if length < 0 {
        return Err(MagicaVoxelError::InvalidScene);
    }
    let end = cursor
        .checked_add(length as usize)
        .ok_or(MagicaVoxelError::Truncated)?;
    if bytes.get(*cursor..end).is_none() {
        return Err(MagicaVoxelError::Truncated);
    }
    *cursor = end;
    Ok(())
}

fn expect_end(bytes: &[u8], cursor: usize) -> Result<(), MagicaVoxelError> {
    if cursor == bytes.len() {
        Ok(())
    } else {
        Err(MagicaVoxelError::InvalidScene)
    }
}

fn read_u32(bytes: &[u8]) -> Result<u32, MagicaVoxelError> {
    let array: [u8; 4] = bytes.try_into().map_err(|_| MagicaVoxelError::Truncated)?;
    Ok(u32::from_le_bytes(array))
}

fn convert_coordinate(
    voxel: [u8; 4],
    orientation: NativeMagicaVoxelOrientation,
) -> Result<[i64; 3], MagicaVoxelError> {
    match orientation {
        NativeMagicaVoxelOrientation::XRightYUpNegativeZForward => Ok([
            i64::from(voxel[0]),
            i64::from(voxel[2]),
            -i64::from(voxel[1]),
        ]),
    }
}

fn sparse_runs(cells: BTreeMap<[i64; 3], u16>) -> Result<Vec<VoxelSparseRun>, MagicaVoxelError> {
    let mut runs = Vec::new();
    let mut next = cells.iter().peekable();
    while let Some((&start, &material_slot)) = next.next() {
        let mut length = 1u32;
        while let Some((&coordinate, &next_slot)) = next.peek() {
            let expected_x = start[0]
                .checked_add(i64::from(length))
                .ok_or(MagicaVoxelError::VoxelLimit)?;
            if coordinate[1] != start[1]
                || coordinate[2] != start[2]
                || coordinate[0] != expected_x
                || next_slot != material_slot
            {
                break;
            }
            length = length.checked_add(1).ok_or(MagicaVoxelError::VoxelLimit)?;
            next.next();
        }
        runs.push(VoxelSparseRun {
            start,
            length,
            material_slot,
        });
    }
    Ok(runs)
}

fn bounds(runs: &[VoxelSparseRun]) -> Result<VoxelAssetBounds, MagicaVoxelError> {
    let mut min = [i64::MAX; 3];
    let mut max = [i64::MIN; 3];
    for run in runs {
        min[0] = min[0].min(run.start[0]);
        min[1] = min[1].min(run.start[1]);
        min[2] = min[2].min(run.start[2]);
        max[0] = max[0].max(
            run.start[0]
                .checked_add(i64::from(run.length) - 1)
                .ok_or(MagicaVoxelError::VoxelLimit)?,
        );
        max[1] = max[1].max(run.start[1]);
        max[2] = max[2].max(run.start[2]);
    }
    if runs.is_empty() {
        return Err(MagicaVoxelError::VoxelLimit);
    }
    Ok(VoxelAssetBounds { min, max })
}

fn pivot(
    bounds: VoxelAssetBounds,
    options: MagicaVoxelAdmissionOptions,
) -> Result<[f64; 3], MagicaVoxelError> {
    let center = [
        (bounds.min[0] as f64 + bounds.max[0] as f64 + 1.0) / 2.0,
        (bounds.min[1] as f64 + bounds.max[1] as f64 + 1.0) / 2.0,
        (bounds.min[2] as f64 + bounds.max[2] as f64 + 1.0) / 2.0,
    ];
    let pivot = match options.pivot_policy {
        NativeMagicaVoxelPivotPolicy::Explicit => options.explicit_pivot,
        NativeMagicaVoxelPivotPolicy::BoundsCenter => center,
        NativeMagicaVoxelPivotPolicy::BaseCenter => [center[0], bounds.min[1] as f64, center[2]],
    };
    if pivot.iter().any(|value| !value.is_finite()) {
        return Err(MagicaVoxelError::InvalidOptions("pivot"));
    }
    Ok(pivot)
}

fn settings_hash(options: MagicaVoxelAdmissionOptions) -> String {
    let mut bytes = Vec::with_capacity(64);
    bytes.extend_from_slice(b"magica-vox-v150");
    bytes.extend_from_slice(&options.cell_size.to_le_bytes());
    bytes.extend_from_slice(&(options.pivot_policy as u32).to_le_bytes());
    for value in options.explicit_pivot {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.extend_from_slice(&(options.orientation as u32).to_le_bytes());
    format!("sha256:{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options() -> MagicaVoxelAdmissionOptions {
        MagicaVoxelAdmissionOptions {
            cell_size: 0.25,
            pivot_policy: NativeMagicaVoxelPivotPolicy::BaseCenter,
            explicit_pivot: [0.0; 3],
            orientation: NativeMagicaVoxelOrientation::XRightYUpNegativeZForward,
            max_source_bytes: MAX_MAGICA_VOX_SOURCE_BYTES,
            max_dimension: MAX_MAGICA_VOX_DIMENSION,
            max_voxel_count: MAX_MAGICA_VOX_VOXELS,
            max_chunk_count: MAX_MAGICA_VOX_CHUNKS,
            max_material_slots: MAX_MAGICA_VOX_MATERIAL_SLOTS,
        }
    }

    fn fixture(voxels: &[[u8; 4]]) -> Vec<u8> {
        let mut children = Vec::new();
        children.extend_from_slice(b"SIZE");
        children.extend_from_slice(&12u32.to_le_bytes());
        children.extend_from_slice(&0u32.to_le_bytes());
        children.extend_from_slice(&[2, 0, 0, 0, 2, 0, 0, 0, 2, 0, 0, 0]);
        let mut xyzi = Vec::new();
        xyzi.extend_from_slice(&(voxels.len() as u32).to_le_bytes());
        for voxel in voxels {
            xyzi.extend_from_slice(voxel);
        }
        children.extend_from_slice(b"XYZI");
        children.extend_from_slice(&(xyzi.len() as u32).to_le_bytes());
        children.extend_from_slice(&0u32.to_le_bytes());
        children.extend_from_slice(&xyzi);
        let mut rgba = vec![0u8; 256 * 4];
        rgba[..4].copy_from_slice(&[12, 34, 56, 255]);
        rgba[4..8].copy_from_slice(&[90, 80, 70, 128]);
        children.extend_from_slice(b"RGBA");
        children.extend_from_slice(&(rgba.len() as u32).to_le_bytes());
        children.extend_from_slice(&0u32.to_le_bytes());
        children.extend_from_slice(&rgba);
        let mut result = b"VOX ".to_vec();
        result.extend_from_slice(&150u32.to_le_bytes());
        result.extend_from_slice(b"MAIN");
        result.extend_from_slice(&0u32.to_le_bytes());
        result.extend_from_slice(&(children.len() as u32).to_le_bytes());
        result.extend_from_slice(&children);
        result
    }

    fn append_chunk(bytes: &mut Vec<u8>, kind: &[u8; 4], content: &[u8]) {
        bytes.extend_from_slice(kind);
        bytes.extend_from_slice(&(content.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(content);
        let child_len = u32::from_le_bytes(bytes[16..20].try_into().expect("MAIN child length"));
        bytes[16..20].copy_from_slice(&(child_len + 12 + content.len() as u32).to_le_bytes());
    }

    fn empty_dictionary(bytes: &mut Vec<u8>) {
        bytes.extend_from_slice(&0i32.to_le_bytes());
    }

    fn transform_node(node_id: i32, child_id: i32) -> Vec<u8> {
        let mut content = Vec::new();
        content.extend_from_slice(&node_id.to_le_bytes());
        empty_dictionary(&mut content);
        content.extend_from_slice(&child_id.to_le_bytes());
        content.extend_from_slice(&(-1i32).to_le_bytes());
        content.extend_from_slice(&0i32.to_le_bytes());
        content.extend_from_slice(&1i32.to_le_bytes());
        empty_dictionary(&mut content);
        content
    }

    fn group_node(node_id: i32, children: &[i32]) -> Vec<u8> {
        let mut content = Vec::new();
        content.extend_from_slice(&node_id.to_le_bytes());
        empty_dictionary(&mut content);
        content.extend_from_slice(&(children.len() as i32).to_le_bytes());
        for child in children {
            content.extend_from_slice(&child.to_le_bytes());
        }
        content
    }

    fn shape_node(node_id: i32, model_id: i32) -> Vec<u8> {
        let mut content = Vec::new();
        content.extend_from_slice(&node_id.to_le_bytes());
        empty_dictionary(&mut content);
        content.extend_from_slice(&1i32.to_le_bytes());
        content.extend_from_slice(&model_id.to_le_bytes());
        empty_dictionary(&mut content);
        content
    }

    #[test]
    fn admits_a_single_model_and_preserves_exact_used_palette_rows() {
        let bytes = fixture(&[[0, 0, 0, 1], [1, 0, 0, 2]]);
        let admission = admit_magica_vox(
            &bytes,
            "voxel-object/test-vox".into(),
            "test.vox".into(),
            options(),
        )
        .expect("fixture admits");
        assert_eq!(admission.palette.len(), 2);
        assert_eq!(admission.palette[0].rgba, [12, 34, 56, 255]);
        assert_eq!(admission.palette[1].rgba, [90, 80, 70, 128]);
        assert_eq!(
            admission
                .object
                .default_frame
                .representation
                .sparse_runs
                .len(),
            2
        );
        assert!(admission.object.content_hash.starts_with("sha256:"));
    }

    #[test]
    fn admits_bounded_single_model_scene_metadata_without_inheriting_scene_placement() {
        let mut bytes = fixture(&[[0, 0, 0, 1]]);
        append_chunk(&mut bytes, b"nTRN", &transform_node(0, 1));
        append_chunk(&mut bytes, b"nGRP", &group_node(1, &[2]));
        append_chunk(&mut bytes, b"nTRN", &transform_node(2, 3));
        append_chunk(&mut bytes, b"nSHP", &shape_node(3, 0));
        append_chunk(&mut bytes, b"NOTE", &0i32.to_le_bytes());
        let mut material = 1i32.to_le_bytes().to_vec();
        empty_dictionary(&mut material);
        append_chunk(&mut bytes, b"MATL", &material);
        let mut layer = 0i32.to_le_bytes().to_vec();
        empty_dictionary(&mut layer);
        layer.extend_from_slice(&(-1i32).to_le_bytes());
        append_chunk(&mut bytes, b"LAYR", &layer);
        let admission = admit_magica_vox(
            &bytes,
            "voxel-object/scene-vox".into(),
            "scene.vox".into(),
            options(),
        )
        .expect("single model scene admits");
        assert_eq!(admission.object.grid.pivot, [0.5, 0.0, 0.5]);
        assert_eq!(admission.object.default_frame.bounds.min, [0, 0, 0]);
    }

    #[test]
    fn rejects_truncated_unsupported_duplicate_and_over_budget_sources() {
        let bytes = fixture(&[[0, 0, 0, 1]]);
        assert!(matches!(
            admit_magica_vox(
                &bytes[..bytes.len() - 1],
                "voxel-object/test-vox".into(),
                "test.vox".into(),
                options()
            ),
            Err(MagicaVoxelError::Truncated)
        ));
        assert!(matches!(
            admit_magica_vox(
                &fixture(&[[0, 0, 0, 1], [0, 0, 0, 1]]),
                "voxel-object/test-vox".into(),
                "test.vox".into(),
                options()
            ),
            Err(MagicaVoxelError::DuplicateCell)
        ));
        let mut limited = options();
        limited.max_voxel_count = 1;
        assert!(matches!(
            admit_magica_vox(
                &fixture(&[[0, 0, 0, 1], [1, 0, 0, 1]]),
                "voxel-object/test-vox".into(),
                "test.vox".into(),
                limited
            ),
            Err(MagicaVoxelError::VoxelLimit)
        ));
        let mut scene = fixture(&[[0, 0, 0, 1]]);
        scene.extend_from_slice(b"nTRN");
        scene.extend_from_slice(&0u32.to_le_bytes());
        scene.extend_from_slice(&0u32.to_le_bytes());
        let child_len = u32::from_le_bytes(scene[16..20].try_into().expect("MAIN child length"));
        scene[16..20].copy_from_slice(&(child_len + 12).to_le_bytes());
        assert!(matches!(
            admit_magica_vox(
                &scene,
                "voxel-object/test-vox".into(),
                "test.vox".into(),
                options()
            ),
            Err(MagicaVoxelError::Truncated)
        ));
    }

    #[test]
    #[ignore = "read-only integration check for the selected CraftSurvive source"]
    fn admits_selected_craftsurvive_source() {
        let source_path =
            "/home/dev/rusty-craftsurvive/content/voxels/woodland-shrine-nano-model-solid64.vox";
        let bytes = std::fs::read(source_path).expect("selected source is available");
        let admission = admit_magica_vox(
            &bytes,
            "voxel-object/woodland-shrine-nano-model-solid64".into(),
            source_path.into(),
            options(),
        )
        .expect("selected v150 source admits");
        assert_eq!(admission.source_byte_count, 99_682);
        assert_eq!(
            admission.source_hash,
            "sha256:a0a38ff44c6f753df55c772bbf957841fe82d074128dbd9b0e07deaebadd20c6"
        );
        assert!(!admission.palette.is_empty());
    }
}
