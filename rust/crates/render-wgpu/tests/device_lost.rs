//! A lost device leaves the renderer applying and drawing without a panic,
//! and says why it was lost (#10272). A product mesh uploaded after the
//! loss panicked the realtime scheduler in `create_buffer_init`, poisoning
//! the runtime, and the loss itself went unreported.

mod support;

use render_wgpu::RendererOptions;
use support::*;

#[test]
fn a_lost_device_applies_and_draws_without_a_panic_and_reports_why() {
    let mut harness = Harness::new(RendererOptions::default());
    assert_eq!(harness.gpu.lost(), None);
    harness.gpu.destroy_device();
    let triangle = payload(
        vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0].repeat(3),
        vec![0.0; 6],
        vec![0, 1, 2],
        &[(0, 3)],
    );
    harness.apply(vec![
        render_model::RenderDiff::DefineMaterial {
            material: material("material/plain", [1.0; 4], None),
        },
        static_mesh("mesh/runtime-1", triangle, "material/plain"),
        instance(
            1,
            None,
            "mesh/runtime-1",
            transform([0.0, 0.0, -3.0], 0.0, [1.0; 3]),
        ),
    ]);
    harness.render(&camera([0.0, 0.0, 0.0], 0.0, 0.0));
    let lost = harness.gpu.lost().expect("the loss is reported");
    assert!(lost.starts_with("Destroyed"), "{lost}");
    assert_eq!(harness.renderer.gpu_readout().device_lost, Some(lost));
}
