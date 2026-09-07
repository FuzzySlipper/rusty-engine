use super::*;

use render_model::{
    MeshAttribute, MeshAttributeKind, MeshAttributeName, MeshBoundsDescriptor, MeshBufferLayout,
    MeshIndexWidth, MeshPayloadDescriptor, MeshPayloadSource, MeshProvenance, RenderDiff,
    RenderFrameDiff, RenderHandle,
};
use runtime_lifecycle::{RuntimeControlRevision, RuntimeGeneration, RuntimeInstanceId};

const INCREMENTAL_VERTEX_COUNT: usize = 750_000;
const LARGE_BASELINE_VERTEX_COUNT: usize = 1_400_000;

fn fixture_binding(instance: u64) -> RuntimeInputBinding {
    RuntimeInputBinding::new(
        RuntimeInstanceId::new(instance),
        RuntimeGeneration::new(1),
        RuntimeControlRevision::new(1),
    )
}

fn inline_mesh_frame(vertex_count: usize) -> RuntimePublication {
    let vertex_count = u32::try_from(vertex_count).expect("fixture vertex count fits u32");
    let payload = MeshPayloadDescriptor {
        layout: MeshBufferLayout {
            vertex_count,
            index_count: 0,
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
        groups: Vec::new(),
        bounds: MeshBoundsDescriptor {
            min: [0.0; 3],
            max: [1.0; 3],
        },
        source: MeshPayloadSource::Inline {
            positions: vec![0.0; vertex_count as usize * 3],
            normals: vec![0.0; vertex_count as usize * 3],
            uvs: None,
            colors: None,
            indices: Vec::new(),
        },
        provenance: MeshProvenance::Generated,
    };
    let frame = RenderFrameDiff::try_from_ops(vec![RenderDiff::ReplaceMeshPayload {
        handle: RenderHandle::new(1),
        payload,
    }])
    .expect("valid typed inline mesh frame");
    RuntimePublication::frame(&frame).expect("valid runtime publication")
}

#[test]
fn large_mesh_publication_remains_an_incremental_through_host_adaptation() {
    let publication = inline_mesh_frame(INCREMENTAL_VERTEX_COUNT);
    let receipt = ProductDevRuntimeReceipt::new((), vec![publication.clone()]).unwrap();
    let (_, wire) = receipt.into_wire_parts().unwrap();
    assert_eq!(wire.len(), 1);
    assert!(serde_json::to_vec(&wire).unwrap().len() > 16 * 1024 * 1024);
    let actual = wire.into_iter().next().unwrap().into_publication().unwrap();
    assert_eq!(actual, publication);
    assert!(actual.binding_marker().is_none());
}

#[test]
fn retained_baseline_and_following_delta_keep_their_publication_order() {
    let binding = fixture_binding(19);
    let outputs = vec![
        RuntimePublication::binding(binding, 1),
        inline_mesh_frame(LARGE_BASELINE_VERTEX_COUNT),
        RuntimePublication::complete_baseline(binding),
        RuntimePublication::frame(&RenderFrameDiff::new()).unwrap(),
    ];
    let receipt = ProductDevRuntimeReceipt::new((), outputs.clone()).unwrap();
    let (_, wire) = receipt.into_wire_parts().unwrap();
    let decoded = wire
        .into_iter()
        .map(|output| output.into_publication().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(decoded, outputs);
}
