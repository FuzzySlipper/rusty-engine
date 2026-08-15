use render_model::*;
use render_projection::{
    build_model_material_preview, ModelMaterialPreviewClassification, ModelMaterialPreviewError,
    ModelMaterialPreviewRequest,
};

#[test]
fn resolved_model_material_preview_builds_a_complete_retained_frame() {
    let request = request();
    let snapshot = build_model_material_preview(request.clone()).unwrap();
    assert_eq!(snapshot.material, request.material);
    assert_eq!(snapshot.mesh_asset, request.mesh_asset);
    assert_eq!(
        snapshot.renderer_classification,
        ModelMaterialPreviewClassification::ReferencePreview
    );
    assert!(snapshot.diagnostics.is_empty());
    assert!(matches!(
        snapshot.preview_frame.ops.as_slice(),
        [
            RenderDiff::DefineMaterial { .. },
            RenderDiff::DefineStaticMesh { .. },
            RenderDiff::CreateStaticMeshInstance { .. }
        ]
    ));
    let json = serde_json::to_string(&snapshot).unwrap();
    assert_eq!(
        serde_json::from_str::<render_projection::ModelMaterialPreviewSnapshot>(&json).unwrap(),
        snapshot
    );
}

#[test]
fn preview_rejects_a_material_the_mesh_does_not_bind() {
    let mut request = request();
    request.material.id = "material/not-bound".to_owned();
    assert!(matches!(
        build_model_material_preview(request),
        Err(ModelMaterialPreviewError::MaterialNotBound { .. })
    ));
}

fn request() -> ModelMaterialPreviewRequest {
    ModelMaterialPreviewRequest {
        material: RenderMaterialDescriptor {
            schema_version: 2,
            id: "material/copper".to_owned(),
            color: [0.8, 0.4, 0.2, 1.0],
            texture: None,
            roughness: 0.6,
            texture_tint: [1.0; 4],
            emission_color: [0.0; 3],
            emission_intensity: 0.0,
            uv_strategy: MaterialUvStrategy::Flat,
            alpha_mode: Default::default(),
            double_sided: false,
            voxel_surface: None,
        },
        mesh_asset: StaticMeshAsset {
            asset: "mesh/preview-triangle".to_owned(),
            payload: MeshPayloadDescriptor {
                layout: MeshBufferLayout {
                    vertex_count: 3,
                    index_count: 3,
                    index_width: MeshIndexWidth::U32,
                    attributes: vec![
                        MeshAttribute {
                            name: MeshAttributeName::Position,
                            components: 3,
                            kind: MeshAttributeKind::F32,
                        },
                        MeshAttribute {
                            name: MeshAttributeName::Normal,
                            components: 3,
                            kind: MeshAttributeKind::F32,
                        },
                    ],
                },
                groups: vec![MeshGroupDescriptor {
                    material_slot: 0,
                    start: 0,
                    count: 3,
                }],
                bounds: MeshBoundsDescriptor {
                    min: [0.0; 3],
                    max: [1.0, 1.0, 0.0],
                },
                source: MeshPayloadSource::Inline {
                    positions: vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
                    normals: vec![0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0],
                    uvs: None,
                    colors: None,
                    indices: vec![0, 1, 2],
                },
                provenance: MeshProvenance::Generated,
            },
            material_slots: vec![MeshMaterialSlot {
                slot: 0,
                material: "material/copper".to_owned(),
            }],
            collision: MeshCollisionPolicy::VisualOnly,
        },
        instance_handle: RenderHandle::new(7_001),
    }
}
