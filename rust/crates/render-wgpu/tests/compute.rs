//! The compute pass foundation (#9509) on a real headless device: its proof
//! workload writes each part's world position from the uploaded rows, it
//! follows the scene across frames, it is timed through the device's
//! timestamp queries where it has them, and an adapter that refuses it still
//! draws the frame. Needs a wgpu adapter: CI uses `WGPU_BACKEND=vulkan` with
//! llvmpipe.

mod support;

use render_model::*;
use render_wgpu::RendererOptions;
use support::*;

const CRATE: &str = "static-mesh/compute-crate";
const MATERIAL: &str = "material/compute-crate";

fn crate_scene(harness: &mut Harness) {
    harness.apply(vec![
        RenderDiff::DefineMaterial {
            material: material(MATERIAL, [0.6, 0.5, 0.4, 1.0], None),
        },
        static_mesh(
            CRATE,
            box_mesh([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5], |_| 0),
            MATERIAL,
        ),
    ]);
}

/// The output rows of the parts the scene holds, as (position, processed).
fn positions(rows: &[[f32; 4]]) -> Vec<([i32; 3], bool)> {
    let mut positions: Vec<([i32; 3], bool)> = rows
        .iter()
        .map(|row| {
            (
                [
                    row[0].round() as i32,
                    row[1].round() as i32,
                    row[2].round() as i32,
                ],
                row[3] == 1.0,
            )
        })
        .collect();
    positions.sort();
    positions
}

#[test]
fn the_compute_pass_writes_each_parts_world_position_from_the_uploaded_rows() {
    let mut harness = Harness::new(RendererOptions::default());
    crate_scene(&mut harness);
    // A root crate, and one under a group so its row is a composed world.
    harness.apply(vec![
        instance(10, None, CRATE, transform([-4.0, 5.0, 6.0], 0.0, [1.0; 3])),
        group(20, None, transform([10.0, 0.0, 0.0], 0.0, [1.0; 3])),
        instance(
            21,
            Some(20),
            CRATE,
            transform([1.0, 2.0, 3.0], 0.0, [1.0; 3]),
        ),
    ]);
    let view = camera([0.0, 3.0, 14.0], 0.0, -10.0);
    let (stats, _) = harness.render(&view);
    let readout = harness.renderer.compute_readout();
    if let Some(reason) = &readout.refused {
        eprintln!("compute pass refused on this adapter: {reason}");
        assert!(
            harness.renderer.read_compute_output().is_empty(),
            "a refused pass has no output"
        );
        assert_eq!(stats.draws, 2, "the frame still draws without the pass");
        return;
    }
    assert_eq!(readout.workgroups, 1, "two rows fit one workgroup");
    assert_eq!(
        positions(&harness.renderer.read_compute_output()),
        vec![([-4, 5, 6], true), ([11, 2, 3], true)]
    );

    // Moving the group moves its child's row; the next frame's dispatch
    // reads the row the upload wrote.
    harness.apply(vec![RenderDiff::Update {
        handle: RenderHandle::new(20),
        transform: Some(transform([-10.0, 0.0, 0.0], 0.0, [1.0; 3])),
        material: None,
        visible: None,
        metadata: None,
    }]);
    harness.render(&view);
    assert_eq!(
        positions(&harness.renderer.read_compute_output()),
        vec![([-9, 2, 3], true), ([-4, 5, 6], true)]
    );

    // Past one workgroup of parts, the dispatch grows with the rows and the
    // output buffer with it.
    let many: Vec<RenderDiff> = (0..100)
        .map(|index| {
            instance(
                100 + index,
                None,
                CRATE,
                transform([index as f32, 0.0, -20.0], 0.0, [1.0; 3]),
            )
        })
        .collect();
    harness.apply(many);
    harness.render(&view);
    let readout = harness.renderer.compute_readout();
    assert_eq!(readout.workgroups, 2, "102 rows take two workgroups of 64");
    let rows = harness.renderer.read_compute_output();
    assert_eq!(rows.len(), 102);
    assert!(
        (0..100).all(|index| positions(&rows).contains(&([index, 0, -20], true))),
        "every added crate's position is in the output"
    );
}

#[test]
fn the_pass_is_timed_through_timestamp_queries_where_the_device_has_them() {
    let mut harness = Harness::new(RendererOptions::default());
    crate_scene(&mut harness);
    harness.apply(vec![instance(
        10,
        None,
        CRATE,
        transform([0.0, 0.0, 0.0], 0.0, [1.0; 3]),
    )]);
    let view = camera([0.0, 1.0, 4.0], 0.0, 0.0);
    // A timed frame's stamps are read back on a later frame without waiting
    // for the GPU, so several frames pass before the first time lands.
    for _ in 0..30 {
        harness.render(&view);
    }
    let readout = harness.renderer.compute_readout();
    let adapter = harness.gpu.adapter_summary();
    // Evidence for the foundation's Den note: the near-empty dispatch's cost
    // and the compute limits this adapter reports.
    eprintln!(
        "{} ({}): timestamps {}, {} timed frames, median {:.4} ms, limits {:?}",
        adapter.name,
        adapter.backend,
        readout.timestamps,
        readout.timed_frames,
        readout.median_gpu_ms,
        readout.limits
    );
    if readout.refused.is_some() {
        assert_eq!(readout.timed_frames, 0);
        return;
    }
    if readout.timestamps {
        assert!(readout.timed_frames > 0, "a timed frame was read back");
        assert!(
            readout.median_gpu_ms.is_finite() && readout.median_gpu_ms >= 0.0,
            "{}",
            readout.median_gpu_ms
        );
    } else {
        assert_eq!(readout.timed_frames, 0, "untimed without timestamp queries");
    }
    assert!(
        readout.limits.workgroup_size[0] >= 64 && readout.limits.invocations_per_workgroup >= 64,
        "the pass ran, so the device met its workgroup size: {:?}",
        readout.limits
    );
}
