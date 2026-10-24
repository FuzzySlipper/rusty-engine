//! A mesh the device has no room for is refused, on a device that is still
//! alive (#10272, R10272-1): its upload's out-of-memory or validation error
//! is the apply issue for that mesh, and nothing panics. Before, the error
//! reached the uncaptured-error handler, which panics, and unwound the
//! realtime scheduler. A device whose buffers hold 1 MiB stands in for one
//! out of memory: wgpu raises a validation error there, not out of memory,
//! and both are caught the same way.

mod common;

use common::*;
use render_model::*;
use render_wgpu::{Gpu, RendererOptions};

const BUFFER_LIMIT: u64 = 1 << 20;

/// One device for the binary, its buffers limited.
fn limited() -> Gpu {
    static GPU: std::sync::OnceLock<Gpu> = std::sync::OnceLock::new();
    GPU.get_or_init(|| {
        Gpu::headless_with_binding_limit(BUFFER_LIMIT).expect("upload tests need a wgpu adapter")
    })
    .clone()
}

/// A metre-wide strip of `quads` quads along x: over 1 MiB of vertices at
/// 30,000.
fn strip(quads: usize) -> MeshPayloadDescriptor {
    let width = 1.0 / quads as f32;
    let mut positions = Vec::with_capacity(quads * 12);
    let mut indices = Vec::with_capacity(quads * 6);
    for quad in 0..quads {
        let x = quad as f32 * width;
        positions.extend_from_slice(&[x, 0.0, 0.0, x + width, 0.0, 0.0]);
        positions.extend_from_slice(&[x + width, 1.0, 0.0, x, 1.0, 0.0]);
        let base = quad as u32 * 4;
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    let normals = [0.0, 0.0, 1.0].repeat(positions.len() / 3);
    payload(positions, normals, indices)
}

#[test]
fn a_mesh_too_large_for_the_device_is_refused_and_the_frame_draws() {
    let mut harness = Harness::on(
        limited(),
        RendererOptions {
            default_world_lights: true,
            ..RendererOptions::default()
        },
    );
    let mut ops = coloured_mesh("cube", cube(), [0.9, 0.9, 0.9, 1.0]);
    ops.push(instance(
        1,
        None,
        "cube",
        transform([0.0, 0.0, -3.0], 0.0, 1.0),
    ));
    harness.apply(ops);
    let eye = camera("eye", [0.0, 0.0, 0.0], 0.0, 0.0);
    let alone = harness.single(&eye);

    let mut ops = coloured_mesh("strip", strip(30_000), [0.9, 0.9, 0.9, 1.0]);
    ops.push(instance(
        2,
        None,
        "strip",
        transform([-1.5, -0.5, -4.0], 0.0, 1.0),
    ));
    let delta = harness
        .world
        .apply(RenderFrameDiff {
            publication: None,
            ops,
        })
        .expect("retained model admits the strip");
    let issues = harness.renderer.apply(&delta, &harness.resources);
    let [issue] = issues.as_slice() else {
        panic!("the strip alone is refused: {issues:?}");
    };
    assert!(
        issue.detail.contains("mesh/strip could not be uploaded"),
        "{issue:?}"
    );

    assert_eq!(harness.single(&eye), alone, "the refused strip drew");
    assert_eq!(harness.gpu.lost(), None);
    // The device goes on taking meshes that fit.
    let mut ops = coloured_mesh("small", strip(100), [0.9, 0.9, 0.9, 1.0]);
    ops.push(instance(
        3,
        None,
        "small",
        transform([-1.5, -0.5, -4.0], 0.0, 1.0),
    ));
    harness.apply(ops);
    assert_ne!(harness.single(&eye), alone, "the strip that fits draws");
}
