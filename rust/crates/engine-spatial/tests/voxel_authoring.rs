use core_math::Vec3;
use core_space::Face;
use engine_spatial::{
    MaterialVoxel, VoxelBoxFill, VoxelCollisionScene, VoxelEdit, VoxelPickError, VoxelPickHint,
    VoxelPickService, VoxelPrimitive, VoxelPrimitiveEditService, VoxelPrimitiveError,
    VoxelPrimitiveMaterial, VoxelPrimitiveRequest, VoxelTemplate, VoxelTemplateEditService,
    VoxelTemplateError, VoxelTemplateRequest, MAX_VOXEL_EDITS_PER_TRANSACTION,
    VOXEL_HOUSE_TEMPLATE_BOUNDS,
};
use entity_state::{EntityTransform, Quat};

#[test]
fn house_template_is_deterministic_bounded_and_preserves_openings() {
    let edits = VoxelTemplateEditService
        .generate(VoxelTemplateRequest {
            template: VoxelTemplate::House,
            origin: [20, -2, 7],
            material_slot: 3,
        })
        .unwrap();
    assert_eq!(edits.len(), 329);
    assert_eq!(VOXEL_HOUSE_TEMPLATE_BOUNDS, [[0, 0, 0], [10, 12, 8]]);
    assert!(edits.contains(&VoxelEdit::Set {
        address: [20, -2, 7],
        material_slot: 3,
    }));
    assert!(!edits.contains(&VoxelEdit::Set {
        address: [25, 0, 7],
        material_slot: 3,
    }));
    assert!(edits.contains(&VoxelEdit::Set {
        address: [28, 10, 13],
        material_slot: 3,
    }));
    assert!(edits
        .windows(2)
        .all(|pair| pair[0].address() < pair[1].address()));
}

#[test]
fn house_template_rejects_invalid_material_and_overflow_without_output() {
    assert!(matches!(
        VoxelTemplateEditService.generate(VoxelTemplateRequest {
            template: VoxelTemplate::House,
            origin: [0, 0, 0],
            material_slot: 0,
        }),
        Err(VoxelTemplateError::InvalidMaterial(_))
    ));
    assert!(matches!(
        VoxelTemplateEditService.generate(VoxelTemplateRequest {
            template: VoxelTemplate::House,
            origin: [i64::MAX, 0, 0],
            material_slot: 1,
        }),
        Err(VoxelTemplateError::InvalidOrigin(_)) | Err(VoxelTemplateError::CoordinateOverflow)
    ));
}

#[test]
fn renderer_hints_are_revalidated_for_local_and_transformed_instances() {
    let scene = scene();
    let local_hint = VoxelPickHint {
        origin: [1.1, 0.5, 0.5],
        direction: [1.0, 0.0, 0.0],
        max_distance: 10.0,
        claimed_voxel: [2, 0, 0],
        claimed_face: Face::NegX,
    };
    let anchor = VoxelPickService::validate(&scene, local_hint).unwrap();
    assert_eq!(anchor.place_voxel, [1, 0, 0]);
    assert_eq!(
        anchor.place_edit(3),
        VoxelEdit::Set {
            address: [1, 0, 0],
            material_slot: 3
        }
    );
    let mut stale = local_hint;
    stale.claimed_voxel = [3, 0, 0];
    assert!(matches!(
        VoxelPickService::validate(&scene, stale),
        Err(VoxelPickError::HintMismatch { .. })
    ));

    let transform = EntityTransform {
        translation: Vec3::new(10.0, 0.0, 0.0),
        rotation: Quat::IDENTITY,
        scale: Vec3::new(2.0, 1.0, 1.0),
    };
    let world = VoxelPickHint {
        origin: [12.2, 0.5, 0.5],
        direction: [1.0, 0.0, 0.0],
        max_distance: 10.0,
        ..local_hint
    };
    let instance = VoxelPickService::validate_instance(&scene, transform, world).unwrap();
    assert_eq!(instance.local.hit_voxel, [2, 0, 0]);
    assert!((instance.world_point[0] - 14.0).abs() < 1.0e-9);
    assert!((instance.world_distance - 1.8).abs() < 1.0e-9);
}

#[test]
fn primitive_boxes_preserve_filled_shell_and_edge_semantics() {
    let generate = |fill| {
        VoxelPrimitiveEditService
            .generate(VoxelPrimitiveRequest {
                primitive: VoxelPrimitive::Box {
                    start: [2, 2, 2],
                    end: [0, 0, 0],
                    fill,
                },
                material: VoxelPrimitiveMaterial::Set { material_slot: 7 },
            })
            .unwrap()
    };
    let filled = generate(VoxelBoxFill::Filled);
    let shell = generate(VoxelBoxFill::Shell);
    let edges = generate(VoxelBoxFill::Edges);
    assert_eq!(filled.len(), 27);
    assert_eq!(shell.len(), 26);
    assert_eq!(edges.len(), 20);
    assert!(filled.contains(&VoxelEdit::Set {
        address: [1, 1, 1],
        material_slot: 7,
    }));
    assert!(!shell.contains(&VoxelEdit::Set {
        address: [1, 1, 1],
        material_slot: 7,
    }));
    assert!(shell.contains(&VoxelEdit::Set {
        address: [1, 1, 0],
        material_slot: 7,
    }));
    assert!(!edges.contains(&VoxelEdit::Set {
        address: [1, 1, 0],
        material_slot: 7,
    }));
}

#[test]
fn primitive_lines_round_half_away_from_zero_and_deduplicate_radius() {
    let positive = VoxelPrimitiveEditService
        .generate(VoxelPrimitiveRequest {
            primitive: VoxelPrimitive::Line {
                start: [0, 0, 0],
                end: [2, 1, 0],
                radius: 0,
            },
            material: VoxelPrimitiveMaterial::Clear,
        })
        .unwrap();
    assert_eq!(
        positive,
        vec![
            VoxelEdit::Clear { address: [0, 0, 0] },
            VoxelEdit::Clear { address: [1, 1, 0] },
            VoxelEdit::Clear { address: [2, 1, 0] },
        ]
    );
    let negative = VoxelPrimitiveEditService
        .generate(VoxelPrimitiveRequest {
            primitive: VoxelPrimitive::Line {
                start: [0, 0, 0],
                end: [-2, -1, 0],
                radius: 0,
            },
            material: VoxelPrimitiveMaterial::Clear,
        })
        .unwrap();
    assert!(negative.contains(&VoxelEdit::Clear {
        address: [-1, -1, 0],
    }));

    let thick = VoxelPrimitiveEditService
        .generate(VoxelPrimitiveRequest {
            primitive: VoxelPrimitive::Line {
                start: [0, 0, 0],
                end: [1, 0, 0],
                radius: 1,
            },
            material: VoxelPrimitiveMaterial::Set { material_slot: 2 },
        })
        .unwrap();
    assert_eq!(thick.len(), 36);
}

#[test]
fn primitive_generation_rejects_invalid_or_unbounded_requests_before_authority() {
    assert!(matches!(
        VoxelPrimitiveEditService.generate(VoxelPrimitiveRequest {
            primitive: VoxelPrimitive::Line {
                start: [0, 0, 0],
                end: [1, 1, 1],
                radius: 5,
            },
            material: VoxelPrimitiveMaterial::Set { material_slot: 1 },
        }),
        Err(VoxelPrimitiveError::RadiusTooLarge { .. })
    ));
    assert!(matches!(
        VoxelPrimitiveEditService.generate(VoxelPrimitiveRequest {
            primitive: VoxelPrimitive::Box {
                start: [0, 0, 0],
                end: [MAX_VOXEL_EDITS_PER_TRANSACTION as i64, 0, 0],
                fill: VoxelBoxFill::Filled,
            },
            material: VoxelPrimitiveMaterial::Clear,
        }),
        Err(VoxelPrimitiveError::TooManyEdits { .. })
    ));
    assert!(matches!(
        VoxelPrimitiveEditService.generate(VoxelPrimitiveRequest {
            primitive: VoxelPrimitive::Block { address: [0, 0, 0] },
            material: VoxelPrimitiveMaterial::Set { material_slot: 0 },
        }),
        Err(VoxelPrimitiveError::InvalidMaterial(_))
    ));
}

fn scene() -> VoxelCollisionScene {
    VoxelCollisionScene::from_material_voxels(
        1.0,
        8,
        [
            MaterialVoxel {
                state: 0,
                address: [0, 0, 0],
                material_slot: 1,
            },
            MaterialVoxel {
                state: 0,
                address: [2, 0, 0],
                material_slot: 1,
            },
        ],
    )
    .unwrap()
}
