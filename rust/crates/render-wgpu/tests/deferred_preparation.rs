//! Deferred preparation (#10693): textures and static meshes defined
//! `deferred` are decoded and uploaded on workers and taken in at the start
//! of a later committed call; what draws them draws nothing until then, and
//! the readout says which batches are drawn.

mod support;

use render_host_contracts::RendererCompositionCamera;
use render_model::*;
use render_wgpu::{PreparationReadout, Renderer, RendererOptions};
use support::*;

const RED: [u8; 4] = [220, 30, 30, 255];

fn view() -> RendererCompositionCamera {
    camera([0.0, 1.0, 4.0], 0.0, -10.0)
}

/// A textured box: a red texture, a material sampling it and a mesh drawing
/// it; `deferred` picks which of the texture and the mesh are deferred.
fn textured_box(
    harness: &mut Harness,
    deferred_texture: bool,
    deferred_mesh: bool,
) -> Vec<RenderDiff> {
    let mut texture =
        harness
            .resources
            .texture("texture/red", 2, 2, &RED.repeat(4), TextureWrap::Clamp);
    texture.deferred = deferred_texture;
    let RenderDiff::DefineStaticMesh { mut asset } = static_mesh(
        "mesh/box",
        box_mesh([-0.5, 0.0, -0.5], [0.5, 1.0, 0.5], |_| 0),
        "material/red",
    ) else {
        unreachable!("a static mesh");
    };
    asset.deferred = deferred_mesh;
    vec![
        RenderDiff::DefineTexture { texture },
        RenderDiff::DefineMaterial {
            material: material("material/red", [1.0; 4], Some("texture/red")),
        },
        RenderDiff::DefineStaticMesh { asset },
        instance(1, None, "mesh/box", Transform::IDENTITY),
    ]
}

/// The centre pixel, where the box is.
fn centre(rgba: &[u8]) -> [u8; 4] {
    let at = ((HEIGHT / 2 * WIDTH + WIDTH / 2) * 4) as usize;
    rgba[at..at + 4].try_into().expect("a pixel")
}

fn reddish(pixel: [u8; 4]) -> bool {
    pixel[0] > 2 * u16::from(pixel[1]).max(20) as u8
}

#[test]
fn a_deferred_texture_and_mesh_draw_once_taken_in_and_every_batch_completes() {
    for (deferred_texture, deferred_mesh) in [(true, false), (false, true), (true, true)] {
        let mut harness = Harness::new(RendererOptions::default());
        harness.renderer.set_preparation_batch(1);
        let ops = textured_box(&mut harness, deferred_texture, deferred_mesh);
        harness.apply(ops);
        let pending = u32::from(deferred_texture) + u32::from(deferred_mesh);
        let readout = harness.renderer.preparation_readout();
        assert_eq!(
            readout.pending, pending,
            "{deferred_texture} {deferred_mesh}"
        );
        assert_eq!(readout.completed_batch, 0);
        let (stats, pixels) = harness.render(&view());
        assert_eq!(
            stats.instances, 0,
            "the box waits for what it draws with ({deferred_texture} {deferred_mesh})"
        );
        assert!(!reddish(centre(&pixels)));

        // Workers finish; the next committed call takes them in.
        assert!(harness.renderer.settle_prepared().is_empty());
        harness.renderer.set_preparation_batch(2);
        assert_eq!(
            harness.renderer.preparation_readout(),
            PreparationReadout {
                completed_batch: 2,
                pending: 0,
                installed: u64::from(pending),
                failed: 0,
            }
        );
        let (stats, pixels) = harness.render(&view());
        assert!(stats.instances > 0, "the box draws");
        assert!(
            reddish(centre(&pixels)),
            "with its texture: {:?}",
            centre(&pixels)
        );
        // As a renderer given the whole scene at once draws it once it has
        // waited for it, as an offline reading does.
        let mut fresh = Renderer::new(&harness.gpu, harness.renderer.options());
        let issues = fresh.apply(&harness.world.snapshot().frame, &harness.resources);
        assert!(issues.is_empty(), "{issues:?}");
        assert!(fresh.settle_prepared().is_empty());
        fresh.render_offscreen(&view(), &harness.target);
        let fresh = harness.target.read_rgba(&harness.gpu);
        assert_eq!(centre(&pixels), centre(&fresh));
    }
}

#[test]
fn a_released_or_redefined_definition_drops_what_was_being_prepared() {
    let mut harness = Harness::new(RendererOptions::default());
    harness.renderer.set_preparation_batch(1);
    let ops = textured_box(&mut harness, true, true);
    harness.apply(ops);
    assert_eq!(harness.renderer.preparation_readout().pending, 2);
    // The mesh is released and the texture defined again at once.
    let texture =
        harness
            .resources
            .texture("texture/red", 2, 2, &RED.repeat(4), TextureWrap::Clamp);
    harness.apply(vec![
        RenderDiff::Destroy {
            handle: RenderHandle::new(1),
        },
        RenderDiff::ReleaseStaticMesh {
            asset: "mesh/box".to_owned(),
        },
        RenderDiff::DefineTexture { texture },
    ]);
    let readout = harness.renderer.preparation_readout();
    assert_eq!((readout.pending, readout.completed_batch), (0, 1));
    // The workers' results arrive and are dropped.
    assert!(harness.renderer.settle_prepared().is_empty());
    let readout = harness.renderer.preparation_readout();
    assert_eq!((readout.installed, readout.failed), (0, 0));
}

#[test]
fn a_batch_completes_only_when_its_last_resource_is_taken_in() {
    let mut harness = Harness::new(RendererOptions::default());
    harness.renderer.set_preparation_batch(1);
    let ops = textured_box(&mut harness, true, false);
    harness.apply(ops);
    harness.renderer.set_preparation_batch(2);
    let RenderDiff::DefineStaticMesh { mut asset } = static_mesh(
        "mesh/other",
        box_mesh([1.0, 0.0, -0.5], [2.0, 1.0, 0.5], |_| 0),
        "material/red",
    ) else {
        unreachable!("a static mesh");
    };
    asset.deferred = true;
    harness.apply(vec![RenderDiff::DefineStaticMesh { asset }]);
    let readout = harness.renderer.preparation_readout();
    assert_eq!((readout.pending, readout.completed_batch), (2, 0));
    harness.renderer.settle_prepared();
    assert_eq!(harness.renderer.preparation_readout().completed_batch, 2);
}

#[test]
fn a_deferred_texture_that_does_not_decode_fails_and_its_batch_completes() {
    let mut harness = Harness::new(RendererOptions::default());
    harness.renderer.set_preparation_batch(1);
    let mut texture =
        harness
            .resources
            .texture("texture/broken", 2, 2, &RED.repeat(4), TextureWrap::Clamp);
    texture.deferred = true;
    if let Some(TexturePayloadSource::Resource { resource }) =
        texture.payload.as_ref().map(|payload| &payload.source)
    {
        harness.resources.insert(resource, b"not a png".to_vec());
    }
    harness.apply(vec![RenderDiff::DefineTexture { texture }]);
    let issues = harness.renderer.settle_prepared();
    assert_eq!(issues.len(), 1, "{issues:?}");
    assert_eq!(issues[0].op, "defineTexture");
    let readout = harness.renderer.preparation_readout();
    assert_eq!((readout.completed_batch, readout.failed), (1, 1));
}

#[test]
fn a_texture_prepared_before_anything_samples_it_draws_when_a_material_does() {
    let mut harness = Harness::new(RendererOptions::default());
    harness.renderer.set_preparation_batch(1);
    let mut texture =
        harness
            .resources
            .texture("texture/red", 2, 2, &RED.repeat(4), TextureWrap::Clamp);
    texture.deferred = true;
    harness.apply(vec![RenderDiff::DefineTexture { texture }]);
    assert_eq!(harness.renderer.preparation_readout().pending, 1);
    assert!(harness.renderer.settle_prepared().is_empty());
    harness.renderer.set_preparation_batch(2);
    let prepared = harness.renderer.preparation_readout();
    assert_eq!((prepared.completed_batch, prepared.installed), (2, 1));

    // A later call makes the material and the box: nothing is prepared again.
    let mut ops = textured_box(&mut harness, false, false);
    ops.remove(0);
    harness.apply(ops);
    assert_eq!(harness.renderer.preparation_readout(), prepared);
    let (stats, pixels) = harness.render(&view());
    assert!(stats.instances > 0, "the box draws");
    assert!(
        reddish(centre(&pixels)),
        "with its texture: {:?}",
        centre(&pixels)
    );
}
