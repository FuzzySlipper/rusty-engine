//! Derived renderer state follows scene changes: after each kind of change,
//! a renderer that applied the changes one by one draws what a new renderer
//! given the retained world whole draws, and steady frames make no
//! pipelines. Same harness as `tests/scene.rs`.

mod support;

use std::collections::BTreeMap;

use engine_spatial::{MaterialVoxel, VoxelCollisionScene};
use render_model::*;
use render_projection::{voxel_material_id, VoxelProjectionInstance, VoxelRenderProjector};
use render_wgpu::{AmbientOcclusion, AmbientOcclusionPath, OffscreenTarget, RendererOptions};
use support::*;

const CHUNK_CELLS: u32 = 8;
/// The pillar's slot: its material arrives after the chunks.
const PILLAR: u16 = 3;
const WALL: u16 = 2;

/// A floor, a back wall and a pillar across several chunks, one material
/// slot each.
fn room() -> VoxelCollisionScene {
    let mut voxels = Vec::new();
    for x in -6..6 {
        for z in -6..6 {
            voxels.push(MaterialVoxel {
                state: 0,
                address: [x, -1, z],
                material_slot: 1,
            });
        }
        for y in 0..4 {
            voxels.push(MaterialVoxel {
                state: 0,
                address: [x, y, -6],
                material_slot: WALL,
            });
        }
    }
    for y in 0..3 {
        voxels.push(MaterialVoxel {
            state: 0,
            address: [2, y, -2],
            material_slot: PILLAR,
        });
    }
    VoxelCollisionScene::from_material_voxels(1.0, CHUNK_CELLS, voxels).expect("voxel room")
}

fn slot_material(slot: u16, color: [f32; 4]) -> RenderMaterialDescriptor {
    material(&voxel_material_id(slot), color, None)
}

/// The room's chunks with every slot's material but the pillar's.
fn room_without_pillar_material() -> Vec<RenderDiff> {
    let materials: BTreeMap<u16, RenderMaterialDescriptor> = [
        (1, [0.5, 0.48, 0.45, 1.0]),
        (WALL, [0.55, 0.5, 0.45, 1.0]),
        (PILLAR, [0.2, 0.7, 0.35, 1.0]),
    ]
    .into_iter()
    .map(|(slot, color)| (slot, slot_material(slot, color)))
    .collect();
    let scene = room();
    VoxelRenderProjector::new()
        .project(
            &[VoxelProjectionInstance {
                instance_id: "room".to_owned(),
                asset_id: "room".to_owned(),
                transform: Transform::IDENTITY,
                scene: &scene,
            }],
            &materials,
        )
        .expect("project voxel room")
        .frame
        .ops
        .into_iter()
        .filter(|op| {
            !matches!(op, RenderDiff::DefineMaterial { material }
                if material.id == voxel_material_id(PILLAR))
        })
        .collect()
}

fn crate_mesh(height: f32) -> MeshPayloadDescriptor {
    box_mesh([-0.5, 0.0, -0.5], [0.5, height, 0.5], |_| 0)
}

/// Crates: one whose mesh is redefined, and one whose slot material is
/// defined after it.
fn crates() -> Vec<RenderDiff> {
    vec![
        RenderDiff::DefineMaterial {
            material: material("material/crate", [0.7, 0.45, 0.3, 1.0], None),
        },
        static_mesh("mesh/crate", crate_mesh(1.0), "material/crate"),
        static_mesh("mesh/late", crate_mesh(0.6), "material/late"),
        instance(
            100,
            None,
            "mesh/crate",
            transform([-0.4, 0.0, 0.4], 20.0, [1.0; 3]),
        ),
        instance(
            101,
            None,
            "mesh/late",
            transform([1.0, 0.0, -0.5], -15.0, [1.0; 3]),
        ),
    ]
}

fn view() -> render_host_contracts::RendererCompositionCamera {
    camera([0.0, 5.0, 7.0], 0.0, -35.0)
}

/// The frame drawn now matches a new renderer's given the scene whole, and
/// a second frame makes no pipeline.
fn assert_fresh(harness: &mut Harness, step: &str) -> Vec<u8> {
    let (_, drawn) = harness.render(&view());
    let (steady, again) = harness.render(&view());
    assert_eq!(steady.pipelines_created, 0, "{step}: a steady frame");
    assert_eq!(
        differing_pixels(&again, &drawn),
        0,
        "{step}: a steady frame draws the same"
    );
    let (_, fresh) = harness.render_fresh(&view());
    let differing = differing_pixels(&drawn, &fresh);
    assert_eq!(differing, 0, "{step}: as a new renderer draws it");
    drawn
}

fn differing_pixels(a: &[u8], b: &[u8]) -> usize {
    a.as_chunks::<4>()
        .0
        .iter()
        .zip(b.as_chunks::<4>().0)
        .filter(|(a, b)| (0..3).any(|c| a[c].abs_diff(b[c]) > 2))
        .count()
}

fn changes_redraw_as_the_whole_scene(gpu_culling: bool) {
    let mut harness = Harness::new(RendererOptions {
        gpu_culling,
        ..RendererOptions::default()
    });
    let mut ops = room_without_pillar_material();
    ops.extend(crates());
    harness.apply(ops);
    let start = assert_fresh(&mut harness, "start");

    // A slot material arriving after its chunks: the pillar leaves the
    // fallback.
    harness.apply(vec![RenderDiff::DefineMaterial {
        material: slot_material(PILLAR, [0.9, 0.1, 0.1, 1.0]),
    }]);
    let pillar = assert_fresh(&mut harness, "late slot material");
    assert!(
        differing_pixels(&pillar, &start) > 0,
        "the pillar draws its material"
    );

    // A static mesh's slot material arriving after it.
    harness.apply(vec![RenderDiff::DefineMaterial {
        material: material("material/late", [0.1, 0.3, 0.9, 1.0], None),
    }]);
    let late = assert_fresh(&mut harness, "late mesh material");
    assert!(
        differing_pixels(&late, &pillar) > 0,
        "the crate draws its material"
    );

    // A redefined material, and one redefined blended and double-sided:
    // new passes.
    let mut glass = material("material/crate", [0.3, 0.8, 0.8, 0.5], None);
    glass.alpha_mode = MaterialAlphaModeDescriptor::Blend;
    glass.double_sided = true;
    harness.apply(vec![RenderDiff::DefineMaterial { material: glass }]);
    let glass = assert_fresh(&mut harness, "redefined material");
    assert!(differing_pixels(&glass, &late) > 0, "the crate is glass");

    // A released slot material: the wall's chunks draw the fallback.
    harness.apply(vec![RenderDiff::ReleaseMaterial {
        id: voxel_material_id(WALL),
    }]);
    let released = assert_fresh(&mut harness, "released slot material");
    assert!(
        differing_pixels(&released, &glass) > 0,
        "the wall draws the fallback"
    );

    // A deformed instance (a soft body's surface): its replaced payload
    // draws in place of its asset's geometry, with the asset's material.
    harness.apply(vec![RenderDiff::ReplaceMeshPayload {
        handle: RenderHandle::new(101),
        payload: crate_mesh(1.6),
    }]);
    let deformed = assert_fresh(&mut harness, "replaced instance payload");
    assert!(
        differing_pixels(&deformed, &released) > 0,
        "the late crate is taller"
    );

    // A redefined mesh: its instance draws the new shape.
    harness.apply(vec![static_mesh(
        "mesh/crate",
        crate_mesh(2.0),
        "material/crate",
    )]);
    let taller = assert_fresh(&mut harness, "redefined mesh");
    assert!(
        differing_pixels(&taller, &deformed) > 0,
        "the crate is taller"
    );

    // A released mesh, once its instance is gone.
    harness.apply(vec![RenderDiff::Destroy {
        handle: RenderHandle::new(100),
    }]);
    harness.apply(vec![RenderDiff::ReleaseStaticMesh {
        asset: "mesh/crate".to_owned(),
    }]);
    assert_fresh(&mut harness, "released mesh");
}

#[test]
fn material_and_mesh_changes_redraw_as_the_whole_scene_does() {
    changes_redraw_as_the_whole_scene(false);
}

#[test]
fn material_and_mesh_changes_redraw_culled_on_the_gpu_as_the_whole_scene_does() {
    changes_redraw_as_the_whole_scene(true);
}

#[test]
fn a_new_target_shadows_and_occlusion_draw_the_variants_already_in_use() {
    let mut harness = Harness::new(RendererOptions::default());
    let mut ops = room_without_pillar_material();
    ops.extend(crates());
    ops.push(RenderDiff::CreateLight {
        handle: RenderHandle::new(10),
        parent: None,
        light: LightDescriptor::Directional {
            color: [1.0, 0.95, 0.85],
            intensity: 2.0,
            enabled: true,
            direction: [-0.9, -0.8, 0.6],
            range: None,
            shadow_intent: LightShadowIntent::Requested,
            shadow: Default::default(),
        },
    });
    harness.apply(ops);
    assert_fresh(&mut harness, "start");

    // A single-sample target is a new pipeline set: the world draws into
    // it with every variant already in use.
    let single = OffscreenTarget::new(&harness.gpu, WIDTH, HEIGHT, 1);
    harness.renderer.render_offscreen(&view(), &single);
    let drawn = single.read_rgba(&harness.gpu);
    let mut fresh = render_wgpu::Renderer::new(&harness.gpu, harness.renderer.options());
    fresh.apply(&harness.world.snapshot().frame, &harness.resources);
    fresh.render_offscreen(&view(), &single);
    assert_eq!(
        differing_pixels(&drawn, &single.read_rgba(&harness.gpu)),
        0,
        "a new target"
    );

    // Shadows turned on: the casters' pipelines are made as they start.
    let unshadowed = harness.render(&view()).1;
    harness.renderer.set_options(RendererOptions {
        shadows: true,
        ..harness.renderer.options()
    });
    let shadowed = assert_fresh(&mut harness, "shadows on");
    assert!(
        differing_pixels(&shadowed, &unshadowed) > 0,
        "the crates cast"
    );

    // Ambient occlusion turned on: the pre-pass pipelines likewise.
    harness.renderer.set_options(RendererOptions {
        ambient_occlusion: AmbientOcclusion {
            path: AmbientOcclusionPath::Raster,
            strength: 1.0,
            radius: AmbientOcclusion::default().radius,
        },
        ..harness.renderer.options()
    });
    assert_fresh(&mut harness, "occlusion on");

    // A part bringing a mesh feature set the scene had not drawn, with
    // shadows and occlusion already drawing.
    let mut colored = crate_mesh(1.0);
    if let MeshPayloadSource::Inline {
        positions, colors, ..
    } = &mut colored.source
    {
        *colors = Some(vec![1.0; positions.len() / 3 * 4].into());
    }
    colored.layout.attributes.push(MeshAttribute {
        name: MeshAttributeName::Color,
        components: 4,
        kind: MeshAttributeKind::F32,
    });
    colored.layer_weights = true;
    harness.apply(vec![
        static_mesh("mesh/weighted", colored, "material/crate"),
        instance(
            102,
            None,
            "mesh/weighted",
            transform([1.5, 0.0, 0.5], 0.0, [1.0; 3]),
        ),
    ]);
    assert_fresh(&mut harness, "a new mesh feature set");
}
