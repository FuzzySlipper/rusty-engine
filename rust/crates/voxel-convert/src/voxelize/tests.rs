use std::collections::{BTreeSet, VecDeque};

use serde::Deserialize;
use sha2::{Digest, Sha256};
use voxel_asset::{
    VoxelAssetMaterialBinding, VoxelAssetMaterialMapping, VoxelConversionFitPolicy,
    VoxelConversionOriginPolicy,
};

use super::*;
use crate::{ImportedMaterial, ImportedPrimitiveGroup, ImportedTriangle};

const GEOMETRY_FIXTURE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../fixtures/voxel-conversion/geometric-voxelization.fixture.json"
));
const GEOMETRY_GOLDEN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../fixtures/voxel-conversion/geometric-voxelization.golden.json"
));
const GEOMETRY_LICENSE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../fixtures/voxel-conversion/geometric-voxelization.LICENSE.txt"
));

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeometryFixture {
    schema_version: u32,
    license: String,
    purpose: String,
    meshes: Vec<FixtureMesh>,
}

#[derive(Debug, Deserialize)]
struct FixtureMesh {
    id: String,
    positions: Vec<[f64; 3]>,
    triangles: Vec<FixtureTriangle>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
struct FixtureTriangle {
    indices: [u32; 3],
    material: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeometryGolden {
    schema_version: u32,
    surface: GoldenCase,
    solid_cavity: GoldenCase,
    solid_slanted: GoldenCase,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoldenCase {
    mesh: String,
    resolution: [u32; 3],
    voxel_count: usize,
    work: u64,
    coordinate_evidence_sha256: String,
}

#[test]
fn triangle_cell_sat_accepts_intersections_and_rejects_aabb_only_overlap() {
    let triangle = [[-0.4, -0.4, 0.0], [0.4, -0.4, 0.0], [0.0, 0.4, 0.0]];
    assert!(triangle_intersects_cell(triangle, [0.0, 0.0, 0.0]));
    assert!(!triangle_intersects_cell(triangle, [1.0, 1.0, 0.0]));
}

#[test]
fn closest_evidence_is_barycentric_and_repeatable() {
    let triangle = [[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 2.0, 0.0]];
    let (barycentric, distance) = closest_triangle_barycentric(triangle, [0.5, 0.5, 1.0]);
    assert_eq!(barycentric, [0.5, 0.25, 0.25]);
    assert_eq!(distance, 1.0);
}

#[test]
fn checked_geometric_corpus_matches_surface_and_solid_goldens() {
    let fixture = fixture();
    let golden: GeometryGolden = serde_json::from_str(GEOMETRY_GOLDEN).unwrap();
    assert_eq!(golden.schema_version, 1);
    for (case, mode) in [
        (&golden.surface, VoxelConversionMode::Surface),
        (&golden.solid_cavity, VoxelConversionMode::Solid),
        (&golden.solid_slanted, VoxelConversionMode::Solid),
    ] {
        let mesh = imported_mesh(&fixture, &case.mesh);
        let result = voxelize(&request(&mesh, case.resolution, mode), &mesh).unwrap();
        assert_eq!(
            (
                result.cells.len(),
                result.work,
                evidence_hash(&result.cells)
            ),
            (
                case.voxel_count,
                case.work,
                case.coordinate_evidence_sha256.clone()
            ),
            "geometric golden drift for {}",
            case.mesh
        );
        assert!(is_connected(&result.cells));
    }
}

#[test]
fn hollow_solid_preserves_the_exterior_cavity_and_surface_materials() {
    let fixture = fixture();
    let mesh = imported_mesh(&fixture, "hollow-cube");
    let result = voxelize(
        &request(&mesh, [13, 13, 13], VoxelConversionMode::Solid),
        &mesh,
    )
    .unwrap();
    for z in 5..=7 {
        for y in 5..=7 {
            for x in 5..=7 {
                assert!(!result.cells.contains_key(&[x, y, z]));
            }
        }
    }
    assert!(result.cells.contains_key(&[1, 6, 6]));
    assert_eq!(
        result
            .cells
            .values()
            .map(|evidence| evidence.source_material_slot)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([1, 2, 3])
    );
}

#[test]
fn conservative_surface_retains_thin_features_across_scale_variation() {
    let fixture = fixture();
    let mesh = imported_mesh(&fixture, "slanted-thin-sheet");
    let coarse = voxelize(
        &request(&mesh, [5, 5, 5], VoxelConversionMode::Surface),
        &mesh,
    )
    .unwrap();
    let fine = voxelize(
        &request(&mesh, [17, 17, 17], VoxelConversionMode::Surface),
        &mesh,
    )
    .unwrap();
    assert!(!coarse.cells.is_empty());
    assert!(fine.cells.len() > coarse.cells.len());
    assert!(is_connected(&coarse.cells));
    assert!(is_connected(&fine.cells));
}

#[test]
fn solid_rejects_non_manifold_input_before_raster_work() {
    let fixture = fixture();
    let mut mesh = imported_mesh(&fixture, "slanted-tetrahedron");
    mesh.triangles.push(mesh.triangles[0]);
    let error = voxelize(
        &request(&mesh, [11, 11, 11], VoxelConversionMode::Solid),
        &mesh,
    )
    .unwrap_err();
    assert_eq!(
        error.diagnostics()[0].code,
        "conversion.unsupportedTopology"
    );
}

#[test]
fn geometric_candidate_work_is_bounded_before_cell_iteration() {
    let mesh = ImportedStaticMesh {
        positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 1.0], [0.0, 1.0, 1.0]],
        texture_coordinates: Vec::new(),
        triangles: vec![ImportedTriangle {
            indices: [0, 1, 2],
            source_material_slot: 1,
        }],
        primitive_groups: vec![ImportedPrimitiveGroup {
            source_node_index: 0,
            source_mesh_index: 0,
            source_primitive_index: 0,
            source_material_slot: 1,
            triangle_start: 0,
            triangle_count: 1,
        }],
        materials: vec![ImportedMaterial {
            source_material_slot: 1,
            source_material_name: None,
        }],
    };
    let error = voxelize(
        &request(&mesh, [256, 256, 256], VoxelConversionMode::Surface),
        &mesh,
    )
    .unwrap_err();
    assert_eq!(error.diagnostics()[0].code, "conversion.resourceLimit");
    assert!(error.diagnostics()[0]
        .message
        .contains("surface triangle/cell coverage"));
}

#[test]
fn high_span_surface_output_is_rejected_before_over_limit_retention() {
    let mesh = ImportedStaticMesh {
        positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0 / 2_048.0, 0.0]],
        texture_coordinates: Vec::new(),
        triangles: vec![ImportedTriangle {
            indices: [0, 1, 2],
            source_material_slot: 1,
        }],
        primitive_groups: vec![ImportedPrimitiveGroup {
            source_node_index: 0,
            source_mesh_index: 0,
            source_primitive_index: 0,
            source_material_slot: 1,
            triangle_start: 0,
            triangle_count: 1,
        }],
        materials: vec![ImportedMaterial {
            source_material_slot: 1,
            source_material_name: None,
        }],
    };
    let mut request = request(&mesh, [4_096, 4, 2], VoxelConversionMode::Surface);
    request.settings.max_output_voxels = 8;

    let error = voxelize(&request, &mesh).unwrap_err();
    assert_eq!(error.diagnostics()[0].code, "conversion.outputLimit");
    assert!(error.diagnostics()[0]
        .message
        .contains("conversion would produce 9 voxels; requested limit is 8"));
}

fn fixture() -> GeometryFixture {
    let fixture: GeometryFixture = serde_json::from_str(GEOMETRY_FIXTURE).unwrap();
    assert_eq!(fixture.schema_version, 1);
    assert_eq!(fixture.license, "CC0-1.0");
    assert!(fixture.purpose.contains("Rusty Engine"));
    assert!(GEOMETRY_LICENSE.contains("CC0 1.0 Universal"));
    fixture
}

fn imported_mesh(fixture: &GeometryFixture, id: &str) -> ImportedStaticMesh {
    let source = fixture.meshes.iter().find(|mesh| mesh.id == id).unwrap();
    let material_slots = source
        .triangles
        .iter()
        .map(|triangle| triangle.material)
        .collect::<BTreeSet<_>>();
    ImportedStaticMesh {
        positions: source.positions.clone(),
        texture_coordinates: Vec::new(),
        triangles: source
            .triangles
            .iter()
            .map(|triangle| ImportedTriangle {
                indices: triangle.indices,
                source_material_slot: triangle.material,
            })
            .collect(),
        primitive_groups: source
            .triangles
            .iter()
            .enumerate()
            .map(|(index, triangle)| ImportedPrimitiveGroup {
                source_node_index: 0,
                source_mesh_index: 0,
                source_primitive_index: index as u32,
                source_material_slot: triangle.material,
                triangle_start: index as u32,
                triangle_count: 1,
            })
            .collect(),
        materials: material_slots
            .into_iter()
            .map(|source_material_slot| ImportedMaterial {
                source_material_slot,
                source_material_name: Some(format!("fixture/{source_material_slot}")),
            })
            .collect(),
    }
}

fn request(
    mesh: &ImportedStaticMesh,
    resolution: [u32; 3],
    mode: VoxelConversionMode,
) -> VoxelConversionRequest {
    let material_slots = mesh
        .materials
        .iter()
        .map(|material| material.source_material_slot)
        .collect::<Vec<_>>();
    VoxelConversionRequest {
        asset_id: "voxel-volume/geometric-fixture".to_owned(),
        source_path: "fixtures/voxel-conversion/geometric-voxelization.fixture.json".to_owned(),
        expected_source_sha256: format!("sha256:{}", "0".repeat(64)),
        license_path: None,
        settings: VoxelConversionSettings {
            resolution,
            cell_size: 1.0,
            chunk_size: 16,
            origin: [0, 0, 0],
            fit_policy: VoxelConversionFitPolicy::Contain,
            origin_policy: VoxelConversionOriginPolicy::Centered,
            mode,
            material_palette: material_slots
                .iter()
                .map(|slot| VoxelAssetMaterialBinding {
                    material_slot: *slot as u16,
                    material_asset_id: format!("material/fixture-{slot}"),
                    display_name: None,
                })
                .collect(),
            material_map: material_slots
                .iter()
                .map(|slot| VoxelAssetMaterialMapping {
                    source_material_slot: *slot,
                    source_material_name: None,
                    voxel_material_slot: *slot as u16,
                })
                .collect(),
            max_output_voxels: 1_000_000,
        },
    }
}

fn evidence_hash(cells: &BTreeMap<[i64; 3], MaterialEvidence>) -> String {
    let mut digest = Sha256::new();
    digest.update(b"rusty-engine.geometric-voxelization.golden.v1\0");
    for (coordinate, evidence) in cells {
        for component in coordinate {
            digest.update(component.to_le_bytes());
        }
        digest.update(evidence.source_material_slot.to_le_bytes());
        digest.update((evidence.triangle_index as u64).to_le_bytes());
    }
    format!("sha256:{:x}", digest.finalize())
}

fn is_connected(cells: &BTreeMap<[i64; 3], MaterialEvidence>) -> bool {
    let Some(start) = cells.keys().next().copied() else {
        return false;
    };
    let coordinates = cells.keys().copied().collect::<BTreeSet<_>>();
    let mut visited = BTreeSet::from([start]);
    let mut pending = VecDeque::from([start]);
    while let Some(current) = pending.pop_front() {
        for z in -1..=1 {
            for y in -1..=1 {
                for x in -1..=1 {
                    if x == 0 && y == 0 && z == 0 {
                        continue;
                    }
                    let neighbor = [current[0] + x, current[1] + y, current[2] + z];
                    if coordinates.contains(&neighbor) && visited.insert(neighbor) {
                        pending.push_back(neighbor);
                    }
                }
            }
        }
    }
    visited.len() == coordinates.len()
}
