use std::time::Instant;

use serde_json::json;
use voxel_convert::{convert_glb, decode_conversion_request};

const SOURCE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../asha-engine/harness/fixtures/voxel-conversion/kenney-wall-a.glb"
));
const REQUEST: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../content/conversion/kenney-wall-a.request.json"
));
const DEFAULT_CONVERSIONS: usize = 256;
const MAX_CONVERSIONS: usize = 4_096;

fn main() {
    let conversions = conversion_count();
    let request = decode_conversion_request(REQUEST).expect("decode checked conversion request");
    let expected = convert_glb(&request, SOURCE).expect("baseline conversion");
    let mut total_nanos = 0u128;
    let mut maximum_nanos = 0u128;

    for _ in 0..conversions {
        let started = Instant::now();
        let receipt = convert_glb(&request, SOURCE).expect("bounded conversion");
        let elapsed = started.elapsed().as_nanos();
        total_nanos += elapsed;
        maximum_nanos = maximum_nanos.max(elapsed);
        assert_eq!(receipt.canonical_json, expected.canonical_json);
        assert_eq!(receipt.content_hash, expected.content_hash);
    }

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "workload": "bounded-static-glb-to-canonical-voxel-asset",
            "conversions": conversions,
            "sourceBytes": SOURCE.len(),
            "requestBytes": REQUEST.len(),
            "outputBytes": expected.canonical_json.len(),
            "sourceVertices": expected.source_vertices,
            "sourceTriangles": expected.source_triangles,
            "outputVoxels": expected.output_voxels,
            "sparseRuns": expected.sparse_runs,
            "contentHash": expected.content_hash,
            "totalMicros": total_nanos / 1_000,
            "averageMicrosPerConversion": total_nanos as f64 / conversions as f64 / 1_000.0,
            "maximumMicrosPerConversion": maximum_nanos / 1_000,
            "conversionsPerSecond": conversions as f64 / (total_nanos as f64 / 1_000_000_000.0),
            "allOutputsByteIdentical": true,
        }))
        .expect("encode workload receipt")
    );
}

fn conversion_count() -> usize {
    let mut arguments = std::env::args().skip(1);
    let Some(value) = arguments.next() else {
        return DEFAULT_CONVERSIONS;
    };
    assert!(
        arguments.next().is_none(),
        "usage: voxel-conversion-workload [conversions]"
    );
    let conversions = value.parse().expect("conversions must be an integer");
    assert!(
        (1..=MAX_CONVERSIONS).contains(&conversions),
        "conversions must be in 1..={MAX_CONVERSIONS}"
    );
    conversions
}
