use std::collections::BTreeSet;

use render_model::{RenderHandle, Transform};
use render_presentation::*;

fn descriptor(source: u64) -> GhostPlateDescriptor {
    GhostPlateDescriptor {
        captured_scene: None,
        source: RenderHandle::new(source),
        placement: GhostPlatePlacement {
            transform: Transform::IDENTITY,
            width: 2.0,
            height: 3.0,
        },
        capture: GhostPlateCaptureSettings {
            resolution: 128,
            azimuth_degrees: 0.0,
            elevation_degrees: 12.0,
            near: 0.1,
            far: 20.0,
            field_of_view_degrees: 35.0,
            lighting: GhostPlateCaptureLighting {
                mode: GhostPlateCaptureLightingMode::Isolated,
                ambient_color: [1.0, 1.0, 1.0],
                ambient_intensity: 1.0,
                key_direction: [1.0, 1.0, 1.0],
                key_color: [1.0, 0.9, 0.8],
                key_intensity: 2.0,
                fill_direction: [-1.0, 1.0, 1.0],
                fill_color: [0.5, 0.7, 1.0],
                fill_intensity: 1.0,
            },
        },
        config: GhostPlateConfig {
            depth_retention: 0.15,
            anchor_policy: GhostPlateAnchorPolicy::BoundsCenter,
            anchor_value: 0.5,
            plate_mapping: GhostPlateMapping::PlateLocked,
            shell_mode: GhostPlateShellMode::WholeMesh,
            shell_depth_epsilon: 0.12,
            sector_count: 8,
            sector_hysteresis_degrees: 3.0,
        },
    }
}

#[test]
fn ghost_plate_projection_is_typed_source_bound_and_batch_atomic() {
    let targets = BTreeSet::from([RenderHandle::new(7)]);
    let handle = GhostPlateHandle::new(3);
    let mut projector = GhostPlateProjector::default();
    let created = projector
        .project(
            &targets,
            PresentationOpMeta::new(0),
            GhostPlateProjectionOp::Create {
                handle,
                descriptor: descriptor(7),
            },
        )
        .unwrap();
    assert!(matches!(created, PresentationOp::GhostPlate { .. }));
    assert_eq!(projector.readout().active_plates, 1);

    let before = projector.descriptor(handle).cloned().unwrap();
    let failure = projector.project_batch(
        &targets,
        vec![(
            PresentationOpMeta::new(0),
            GhostPlateProjectionOp::Update {
                handle,
                patch: GhostPlatePatch {
                    placement: None,
                    config: Some(GhostPlateConfig {
                        sector_count: 2,
                        ..before.config.clone()
                    }),
                },
            },
        )],
    );
    assert!(matches!(
        failure,
        Err(GhostPlateProjectionDiagnostic {
            code: GhostPlateProjectionDiagnosticCode::InvalidDescriptor,
            ..
        })
    ));
    assert_eq!(projector.descriptor(handle), Some(&before));

    let unknown_source = projector.project(
        &targets,
        PresentationOpMeta::new(0),
        GhostPlateProjectionOp::Create {
            handle: GhostPlateHandle::new(4),
            descriptor: descriptor(8),
        },
    );
    assert!(matches!(
        unknown_source,
        Err(GhostPlateProjectionDiagnostic {
            code: GhostPlateProjectionDiagnosticCode::UnknownSource,
            ..
        })
    ));
    assert_eq!(projector.readout().active_plates, 1);
}

#[test]
fn canonical_capture_survives_source_changes_and_explicit_recapture_replaces_it() {
    use render_model::{Geometry, RenderDiff, RenderFrameDiff, RenderNode};
    let mut world = PresentationWorld::default();
    let source = RenderHandle::new(7);
    world
        .apply(&RenderFrameDiff {
            ops: vec![RenderDiff::Create {
                handle: source,
                parent: None,
                node: RenderNode::new(Geometry::Cube),
            }],
            ..RenderFrameDiff::new()
        })
        .unwrap();
    let create = PresentationFrameDiff::try_from_ops(vec![PresentationOp::GhostPlate {
        meta: PresentationOpMeta::new(0),
        op: GhostPlateProjectionOp::Create {
            handle: GhostPlateHandle::new(3),
            descriptor: descriptor(7),
        },
    }])
    .unwrap();
    let first = world.apply_presentation(&create).unwrap();
    world.retain_effects(vec![create.clone()]);
    let capture = match &first.ops[0] {
        PresentationOp::GhostPlate {
            op: GhostPlateProjectionOp::Create { descriptor, .. },
            ..
        } => descriptor.captured_scene.clone().unwrap(),
        _ => unreachable!(),
    };
    world
        .apply(&RenderFrameDiff {
            ops: vec![RenderDiff::Update {
                handle: source,
                transform: Some(Transform {
                    translation: [10.0, 0.0, 0.0],
                    ..Transform::IDENTITY
                }),
                material: None,
                visible: None,
                metadata: None,
            }],
            ..RenderFrameDiff::new()
        })
        .unwrap();
    world.retain_effects(vec![create.clone()]);
    let baseline = world.effects_snapshot();
    assert!(
        matches!(&baseline[0].ops[0], PresentationOp::GhostPlate { op: GhostPlateProjectionOp::Create { descriptor, .. }, .. } if descriptor.captured_scene.as_ref() == Some(&capture))
    );
    let recapture = world
        .apply_presentation(
            &PresentationFrameDiff::try_from_ops(vec![PresentationOp::GhostPlate {
                meta: PresentationOpMeta::new(0),
                op: GhostPlateProjectionOp::Recapture {
                    handle: GhostPlateHandle::new(3),
                    capture: None,
                    captured_scene: None,
                },
            }])
            .unwrap(),
        )
        .unwrap();
    assert!(
        matches!(&recapture.ops[0], PresentationOp::GhostPlate { op: GhostPlateProjectionOp::Recapture { captured_scene: Some(next), .. }, .. } if next != &capture)
    );
}
