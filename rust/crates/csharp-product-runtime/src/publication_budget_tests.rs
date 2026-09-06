use super::*;

use std::cell::Cell;

use render_model::{
    MeshAttribute, MeshAttributeKind, MeshAttributeName, MeshBoundsDescriptor, MeshBufferLayout,
    MeshIndexWidth, MeshPayloadDescriptor, MeshPayloadSource, MeshProvenance, RenderDiff,
    RenderFrameDiff, RenderHandle,
};
use runtime_lifecycle::{RuntimeControlRevision, RuntimeGeneration, RuntimeInstanceId};

const INCREMENTAL_VERTEX_COUNT: usize = 750_000;
const OVERSIZED_BASELINE_VERTEX_COUNT: usize = 1_400_000;

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

fn output_group_error(outputs: &[RuntimePublication]) -> product_dev_host::ProductDevHostError {
    let wire = outputs
        .iter()
        .cloned()
        .map(ProductDevRuntimeOutput::from_publication)
        .collect::<Result<Vec<_>, _>>()
        .expect("typed publications adapt to host outputs");
    ProductDevRuntimeOutput::validate_output_group(&wire)
        .expect_err("fixture must exceed the selected host budget")
}

#[test]
fn publication_budget_leaves_small_ordinary_group_unchanged() {
    let outputs = vec![RuntimePublication::frame(&RenderFrameDiff::new())
        .expect("empty frame is a valid ordinary publication")];
    let reconstructed = Cell::new(false);

    let actual = fit_publication_budget(outputs.clone(), |unexpected| {
        reconstructed.set(true);
        Ok(unexpected)
    })
    .expect("small ordinary group remains admitted");

    assert_eq!(actual, outputs);
    assert!(
        !reconstructed.get(),
        "small group must not reconstruct a baseline"
    );
}

#[test]
fn publication_budget_reconstructs_large_inline_mesh_as_complete_baseline_once() {
    let outputs = vec![inline_mesh_frame(INCREMENTAL_VERTEX_COUNT)];
    assert_eq!(
        output_group_error(&outputs).code(),
        "DEV_HOST_OUTPUT_BOUNDS"
    );
    let binding = fixture_binding(7);
    let reconstructions = Cell::new(0);

    let actual = fit_publication_budget(outputs, |deltas| {
        reconstructions.set(reconstructions.get() + 1);
        let mut baseline = Vec::with_capacity(deltas.len() + 2);
        baseline.push(RuntimePublication::binding(binding, 3));
        baseline.extend(deltas);
        baseline.push(RuntimePublication::complete_baseline(binding));
        Ok(baseline)
    })
    .expect("large retained replacement reconstructs within the baseline budget");

    assert_eq!(reconstructions.get(), 1);
    assert!(matches!(
        actual.as_slice(),
        [
            RuntimePublication::Binding { runtime, .. },
            RuntimePublication::Frame(_),
            RuntimePublication::CompleteBaseline {
                runtime: completion,
                ..
            },
        ] if *runtime == binding && *completion == binding
    ));
    let wire = actual
        .into_iter()
        .map(ProductDevRuntimeOutput::from_publication)
        .collect::<Result<Vec<_>, _>>()
        .expect("reconstructed publications adapt to host outputs");
    assert!(
        ProductDevRuntimeOutput::validate_output_group(&wire).is_ok(),
        "binding-through-completion reconstruction receives the unchanged baseline limit"
    );
}

#[test]
fn publication_budget_does_not_retry_a_malformed_reconstructed_baseline() {
    let binding = fixture_binding(11);
    let reconstructions = Cell::new(0);

    let error = fit_publication_budget(
        vec![inline_mesh_frame(INCREMENTAL_VERTEX_COUNT)],
        |mut deltas| {
            reconstructions.set(reconstructions.get() + 1);
            Ok(vec![
                RuntimePublication::binding(binding, 1),
                deltas.pop().expect("large source publication"),
                RuntimePublication::complete_baseline(fixture_binding(12)),
            ])
        },
    )
    .expect_err("mismatched completion remains an error");

    assert_eq!(error.code(), "DEV_HOST_OUTPUT_BASELINE");
    assert_eq!(
        reconstructions.get(),
        1,
        "malformed replacement cannot loop"
    );
}

#[test]
fn publication_budget_does_not_retry_an_oversized_reconstructed_baseline() {
    let binding = fixture_binding(19);
    let reconstructions = Cell::new(0);

    let error = fit_publication_budget(
        vec![inline_mesh_frame(OVERSIZED_BASELINE_VERTEX_COUNT)],
        |mut deltas| {
            reconstructions.set(reconstructions.get() + 1);
            let frame = deltas.pop().expect("large source publication");
            Ok(vec![
                RuntimePublication::binding(binding, 1),
                frame.clone(),
                frame,
                RuntimePublication::complete_baseline(binding),
            ])
        },
    )
    .expect_err("replacement keeps the existing 64 MiB baseline limit");

    assert_eq!(error.code(), "DEV_HOST_OUTPUT_BOUNDS");
    assert_eq!(
        reconstructions.get(),
        1,
        "oversized replacement cannot loop"
    );
}
