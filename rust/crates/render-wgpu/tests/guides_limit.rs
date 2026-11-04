//! Guide targets on a device at WebGPU's colour attachment guarantee
//! (32 bytes a sample), short of the 40 the world pass with guides draws
//! (`motion.rs`): the readout says why, and the frame draws its picture
//! without them.

mod common;

use common::*;
use render_wgpu::{Gpu, Guide, OverlayShown, RendererOptions};

#[test]
fn a_device_short_of_the_bytes_refuses_the_guides_and_says_why() {
    let gpu = Gpu::headless_with_color_attachment_limit(32).expect("a device");
    let mut harness = Harness::on(gpu, RendererOptions::default());
    harness.apply(coloured_mesh("box", cube(), [0.8, 0.1, 0.1, 1.0]));
    harness.apply(vec![instance(
        1,
        None,
        "box",
        transform([0.0, 0.0, -4.0], 0.0, 1.0),
    )]);
    let eye = camera("eye", [0.0, 0.0, 0.0], 0.0, 0.0);
    let picture = harness.single(&eye);
    assert_eq!(
        harness.renderer.gpu_readout().guides_refused,
        None,
        "none asked for"
    );
    harness.renderer.set_overlay_shown(OverlayShown {
        guide: Some(Guide::DiffuseAlbedo),
        ..OverlayShown::default()
    });
    let refused = harness
        .renderer
        .gpu_readout()
        .guides_refused
        .expect("refused");
    assert!(refused.contains("32 bytes"), "{refused}");
    assert_eq!(harness.single(&eye), picture, "the picture, not a guide");
}
