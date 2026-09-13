use std::sync::Arc;

use nalgebra::{Matrix4, UnitQuaternion, Vector3};

use crate::GenerateOptions;

use super::{
    continuity::{enclosure, expected_join, Enclosure, Join},
    AnalysisClassification, Bounds, Field, Piece,
};

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

fn piece(
    id: u64,
    positions: Vec<[f32; 3]>,
    triangles: Vec<[u32; 3]>,
    transform: [f32; 16],
) -> Arc<Piece> {
    let mut field = Field::new();
    let node = field
        .box_shape(super::bounds_for_positions(&positions).unwrap())
        .unwrap();
    Arc::new(Piece::new(id, &field, node, positions, triangles, transform, 0.05).unwrap())
}

fn enclosure_query(openings: Vec<Bounds>, max_samples: u64) -> Enclosure {
    Enclosure {
        bounds: Bounds {
            min: [-1.2, -1.2, -1.2],
            max: [1.2, 1.2, 1.2],
        },
        interior: [0.5, -0.5, 0.0],
        openings,
        sample_spacing: 0.2,
        max_samples,
    }
}

fn has(report: &super::AnalysisReport, classification: AnalysisClassification) -> bool {
    report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.classification == classification)
}

fn rotated_matrix(rotation: UnitQuaternion<f32>, translation: Vector3<f32>) -> [f32; 16] {
    let matrix = Matrix4::new_translation(&translation) * rotation.to_homogeneous();
    matrix.as_slice().try_into().expect("4x4 transform")
}

fn rotated_join(max_samples: u64) -> Join {
    let rotation = UnitQuaternion::from_euler_angles(0.31, -0.42, 0.17);
    let u = rotation * Vector3::x() * 0.8;
    let v = rotation * Vector3::y() * 0.7;
    Join {
        piece_a: 10,
        piece_b: 11,
        center: [0.0, 0.0, 0.0],
        half_u: [u.x, u.y, u.z],
        half_v: [v.x, v.y, v.z],
        search_distance: 0.5,
        tolerance_cells: 0.5,
        sample_spacing: 0.2,
        max_samples,
    }
}

#[test]
fn rotated_closed_pieces_touch_and_a_crack_reports_its_width() {
    let rotation = UnitQuaternion::from_euler_angles(0.31, -0.42, 0.17);
    let transform = rotated_matrix(rotation, Vector3::zeros());
    let (a_positions, a_triangles) = cube_mesh([-1.0, -1.0, -0.2], [1.0, 1.0, 0.0]);
    let a = piece(10, a_positions, a_triangles, transform);
    let (touch_positions, touch_triangles) = cube_mesh([-1.0, -1.0, 0.0], [1.0, 1.0, 0.2]);
    let touching = piece(11, touch_positions, touch_triangles, transform);
    let touching_report = expected_join(&[a.clone(), touching], rotated_join(128)).unwrap();
    assert!(touching_report.complete, "{touching_report:?}");
    assert!(
        touching_report.diagnostics.is_empty(),
        "{touching_report:?}"
    );

    let (overlap_positions, overlap_triangles) = cube_mesh([-1.0, -1.0, -0.1], [1.0, 1.0, 0.2]);
    let overlap = piece(11, overlap_positions, overlap_triangles, transform);
    assert!(
        expected_join(&[a.clone(), overlap], rotated_join(128))
            .unwrap()
            .diagnostics
            .is_empty(),
        "solid overlap is not an air gap"
    );

    let (crack_positions, crack_triangles) = cube_mesh([-1.0, -1.0, 0.15], [1.0, 1.0, 0.35]);
    let crack = piece(11, crack_positions, crack_triangles, transform);
    let crack_report = expected_join(&[a, crack], rotated_join(128)).unwrap();
    let diagnostic = crack_report
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.classification == AnalysisClassification::JoinGap)
        .expect("crack wider than extraction tolerance");
    assert!(
        (diagnostic.approximate_width - 0.15).abs() < 1e-5,
        "{crack_report:?}"
    );
}

#[test]
fn expected_join_marks_a_sample_budget_as_incomplete() {
    let rotation = UnitQuaternion::from_euler_angles(0.31, -0.42, 0.17);
    let transform = rotated_matrix(rotation, Vector3::zeros());
    let (a_positions, a_triangles) = cube_mesh([-1.0, -1.0, -0.2], [1.0, 1.0, 0.0]);
    let a = piece(10, a_positions, a_triangles, transform);
    let (b_positions, b_triangles) = cube_mesh([-1.0, -1.0, 0.0], [1.0, 1.0, 0.2]);
    let b = piece(11, b_positions, b_triangles, transform);
    let report = expected_join(&[a, b], rotated_join(1)).unwrap();
    assert!(!report.complete, "{report:?}");
    assert!(has(&report, AnalysisClassification::IncompleteCoverage));
}

#[test]
fn extracted_hole_leaks_even_when_its_source_field_is_sealed() {
    let (positions, mut triangles) = cube_mesh([-1.0; 3], [1.0; 3]);
    triangles.remove(2); // one top triangle is absent from the extracted mesh only.
    let report = enclosure(
        &[piece(1, positions, triangles, identity())],
        &enclosure_query(vec![], 20_000),
    )
    .unwrap();
    assert!(report.complete, "{report:?}");
    assert!(
        has(&report, AnalysisClassification::EnclosureLeak),
        "{report:?}"
    );
    assert!(report.path.len() >= 3, "{report:?}");
    assert_eq!(report.path.first().copied(), Some([0.5, -0.5, 0.0]));
}

#[test]
fn extracted_loading_bay_captures_the_door_pose_and_missing_transition() {
    let doorway_cap = Bounds {
        // The virtual cap deliberately straddles the physical doorway. It is
        // an informational exception, so a closed door must still prevent a leak.
        min: [3.45, 1.45, 0.45],
        max: [3.85, 2.55, 1.85],
    };
    let closed = extracted_loading_bay(true, false);
    let closed_report = enclosure(&closed, &loading_bay_query(vec![doorway_cap])).unwrap();
    assert!(closed_report.complete, "{closed_report:?}");
    assert!(
        !has(&closed_report, AnalysisClassification::EnclosureLeak),
        "{closed_report:?}"
    );

    let open = extracted_loading_bay(true, true);
    let open_report = enclosure(&open, &loading_bay_query(vec![doorway_cap])).unwrap();
    assert!(
        !has(&open_report, AnalysisClassification::EnclosureLeak),
        "{open_report:?}"
    );
    assert!(
        has(&open_report, AnalysisClassification::IntentionalOpening),
        "{open_report:?}"
    );

    let missing_transition = extracted_loading_bay(false, false);
    let missing_report = enclosure(&missing_transition, &loading_bay_query(vec![])).unwrap();
    assert!(
        has(&missing_report, AnalysisClassification::EnclosureLeak),
        "{missing_report:?}"
    );
    assert!(missing_report.path.len() >= 3, "{missing_report:?}");
}

#[test]
fn loading_bay_ceiling_step_requires_its_vertical_closure() {
    let corrected = step_shell(true);
    let corrected_report = enclosure(
        &[piece(1, corrected.0, corrected.1, identity())],
        &Enclosure {
            bounds: Bounds {
                min: [0.0, 0.0, 0.0],
                max: [4.0, 4.0, 4.0],
            },
            interior: [3.0, 2.0, 1.0],
            openings: vec![],
            sample_spacing: 0.25,
            max_samples: 20_000,
        },
    )
    .unwrap();
    assert!(
        !has(&corrected_report, AnalysisClassification::EnclosureLeak),
        "{corrected_report:?}"
    );

    let missing = step_shell(false);
    let missing_report = enclosure(
        &[piece(1, missing.0, missing.1, identity())],
        &Enclosure {
            bounds: Bounds {
                min: [0.0, 0.0, 0.0],
                max: [4.0, 4.0, 4.0],
            },
            interior: [3.0, 2.0, 1.0],
            openings: vec![],
            sample_spacing: 0.25,
            max_samples: 20_000,
        },
    )
    .unwrap();
    assert!(
        has(&missing_report, AnalysisClassification::EnclosureLeak),
        "{missing_report:?}"
    );
    assert!(missing_report.path.len() >= 3, "{missing_report:?}");
}

fn loading_bay_query(openings: Vec<Bounds>) -> Enclosure {
    Enclosure {
        bounds: Bounds {
            min: [0.0, 0.0, 0.0],
            max: [4.0, 4.0, 4.0],
        },
        interior: [3.0, 2.0, 1.0],
        openings,
        sample_spacing: 0.25,
        max_samples: 20_000,
    }
}

/// Builds a small bay from independently extracted implicit boxes. The low
/// roof at x<2 has exterior above it; the vertical transition closes that
/// exterior from the taller right-hand bay. The door itself is another
/// extracted box, placed through `Piece::new_trs` in its captured pose.
fn extracted_loading_bay(include_transition: bool, door_open: bool) -> Vec<Arc<Piece>> {
    let mut pieces = Vec::new();
    let mut id = 100;
    let mut add = |min, max| {
        pieces.push(extracted_box_piece(id, Bounds { min, max }));
        id += 1;
    };
    // Floor; low/high roof; the two end walls; and split side walls.
    add([0.5, 0.5, 0.25], [3.5, 3.5, 0.5]);
    add([0.5, 0.5, 2.0], [2.0, 3.5, 2.25]);
    add([2.0, 0.5, 3.5], [3.5, 3.5, 3.75]);
    add([0.25, 0.5, 0.5], [0.5, 3.5, 2.25]);
    for y in [0.25, 3.5] {
        add([0.5, y, 0.5], [2.0, y + 0.25, 2.25]);
        add([2.0, y, 0.5], [3.5, y + 0.25, 3.75]);
    }
    // Right wall panels leave a 1.0 by 1.3 doorway for the separately posed door.
    add([3.5, 0.5, 0.5], [3.75, 1.5, 3.75]);
    add([3.5, 2.5, 0.5], [3.75, 3.5, 3.75]);
    add([3.5, 1.5, 1.8], [3.75, 2.5, 3.75]);
    if include_transition {
        add([1.875, 0.5, 2.0], [2.125, 3.5, 3.5]);
    }
    pieces.push(extracted_door_piece(id, door_open));
    pieces
}

fn extracted_box_piece(id: u64, bounds: Bounds) -> Arc<Piece> {
    let mut field = Field::new();
    let node = field.box_shape(bounds).unwrap();
    let geometry = field.generate(node, extraction_options(bounds)).unwrap();
    Arc::new(
        Piece::new(
            id,
            &field,
            node,
            geometry.positions,
            geometry.triangles,
            identity(),
            geometry.cell_size[0],
        )
        .unwrap(),
    )
}

fn extracted_door_piece(id: u64, open: bool) -> Arc<Piece> {
    let local = Bounds {
        min: [-0.125, -0.5, -0.65],
        max: [0.125, 0.5, 0.65],
    };
    let mut field = Field::new();
    let node = field.box_shape(local).unwrap();
    let geometry = field.generate(node, extraction_options(local)).unwrap();
    Arc::new(
        Piece::new_trs(
            id,
            &field,
            node,
            geometry.positions,
            geometry.triangles,
            if open {
                [4.625, 2.0, 1.15]
            } else {
                [3.625, 2.0, 1.15]
            },
            [0.0, 0.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
            geometry.cell_size[0],
        )
        .unwrap(),
    )
}

fn extraction_options(shape: Bounds) -> GenerateOptions {
    const PADDING: f32 = 0.3;
    GenerateOptions {
        bounds: Bounds {
            min: shape.min.map(|value| value - PADDING),
            max: shape.max.map(|value| value + PADDING),
        },
        cell_size: 0.2,
        max_vertices: 50_000,
        max_triangles: 100_000,
    }
}

fn step_shell(include_step: bool) -> (Vec<[f32; 3]>, Vec<[u32; 3]>) {
    let mut positions = Vec::new();
    let mut triangles = Vec::new();
    let mut quad = |a, b, c, d| add_quad(&mut positions, &mut triangles, a, b, c, d);
    // Floor, stepped roof, and their exterior walls form a small loading bay.
    quad(
        [0.5, 0.5, 0.5],
        [3.5, 0.5, 0.5],
        [3.5, 3.5, 0.5],
        [0.5, 3.5, 0.5],
    );
    quad(
        [0.5, 0.5, 2.0],
        [2.0, 0.5, 2.0],
        [2.0, 3.5, 2.0],
        [0.5, 3.5, 2.0],
    );
    quad(
        [2.0, 0.5, 3.5],
        [3.5, 0.5, 3.5],
        [3.5, 3.5, 3.5],
        [2.0, 3.5, 3.5],
    );
    quad(
        [0.5, 0.5, 0.5],
        [0.5, 3.5, 0.5],
        [0.5, 3.5, 2.0],
        [0.5, 0.5, 2.0],
    );
    quad(
        [3.5, 0.5, 0.5],
        [3.5, 3.5, 0.5],
        [3.5, 3.5, 3.5],
        [3.5, 0.5, 3.5],
    );
    for y in [0.5, 3.5] {
        quad([0.5, y, 0.5], [2.0, y, 0.5], [2.0, y, 2.0], [0.5, y, 2.0]);
        quad([2.0, y, 0.5], [3.5, y, 0.5], [3.5, y, 3.5], [2.0, y, 3.5]);
    }
    if include_step {
        quad(
            [2.0, 0.5, 2.0],
            [2.0, 3.5, 2.0],
            [2.0, 3.5, 3.5],
            [2.0, 0.5, 3.5],
        );
    }
    (positions, triangles)
}

fn add_quad(
    positions: &mut Vec<[f32; 3]>,
    triangles: &mut Vec<[u32; 3]>,
    a: [f32; 3],
    b: [f32; 3],
    c: [f32; 3],
    d: [f32; 3],
) {
    let index = u32::try_from(positions.len()).expect("small fixture");
    positions.extend([a, b, c, d]);
    triangles.extend([[index, index + 1, index + 2], [index, index + 2, index + 3]]);
}
