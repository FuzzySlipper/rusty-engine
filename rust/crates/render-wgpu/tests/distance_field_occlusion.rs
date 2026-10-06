//! Distance-field ambient occlusion (#5911) on a real headless device: voxel
//! chunks publish their fields with their meshes, the renderer atlases them,
//! and the `DistanceField` occlusion path darkens the room's corners from
//! the fields rather than the view's depth alone; a chunk edit swaps its
//! brick, and the atlas grows past its first capacity. Same harness,
//! tolerance and blessing as `tests/screenshots.rs` (`RENDER_WGPU_BLESS=1`
//! rewrites the references). Needs a wgpu adapter: CI uses
//! `WGPU_BACKEND=vulkan` with llvmpipe.

mod support;

use std::collections::BTreeMap;

use engine_spatial::{
    MaterialVoxel, SurfaceMeshOptions, VoxelCollisionScene, VoxelEdit, VoxelEditService,
};
use render_model::*;
use render_projection::{VoxelProjectionInstance, VoxelRenderProjector};
use render_wgpu::{AmbientOcclusion, AmbientOcclusionPath, RendererOptions};
use support::*;

const CHUNK_CELLS: u32 = 8;

fn options(path: AmbientOcclusionPath) -> RendererOptions {
    RendererOptions {
        ambient_occlusion: AmbientOcclusion {
            path,
            strength: 1.0,
        },
        ..RendererOptions::default()
    }
}

fn materials() -> BTreeMap<u16, RenderMaterialDescriptor> {
    [
        (1, [0.5, 0.48, 0.45, 1.0]),
        (2, [0.55, 0.5, 0.45, 1.0]),
        (3, [0.2, 0.7, 0.35, 1.0]),
    ]
    .into_iter()
    .map(|(slot, color)| {
        let mut descriptor = material(&format!("voxel-material/{slot}"), color, None);
        descriptor.roughness = 1.0;
        (slot, descriptor)
    })
    .collect()
}

fn scene_from(voxels: Vec<MaterialVoxel>) -> VoxelCollisionScene {
    VoxelCollisionScene::from_material_voxels_with_mesh_options(
        1.0,
        CHUNK_CELLS,
        voxels,
        SurfaceMeshOptions::default(),
    )
    .expect("voxel scene")
}

/// A floor, a back wall and a pillar across several 8³ chunks: the foot of
/// the wall and the pillar's base are where the fields darken.
fn voxel_room() -> VoxelCollisionScene {
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
                material_slot: 2,
            });
        }
    }
    for y in 0..3 {
        voxels.push(MaterialVoxel {
            state: 0,
            address: [2, y, -2],
            material_slot: 3,
        });
    }
    scene_from(voxels)
}

fn project(
    projector: &mut VoxelRenderProjector,
    scene: &VoxelCollisionScene,
    materials: &BTreeMap<u16, RenderMaterialDescriptor>,
) -> Vec<RenderDiff> {
    projector
        .project(
            &[VoxelProjectionInstance {
                instance_id: "room".to_owned(),
                asset_id: "room".to_owned(),
                transform: Transform::IDENTITY,
                scene,
            }],
            materials,
        )
        .expect("project voxel scene")
        .frame
        .ops
}

fn view() -> render_host_contracts::RendererCompositionCamera {
    camera([3.5, 3.0, 4.5], 30.0, -25.0)
}

/// Per-pixel luminance differences `with - without`, in 8-bit units.
fn differences(without: &[u8], with: &[u8]) -> Vec<i32> {
    without
        .as_chunks::<4>()
        .0
        .iter()
        .zip(with.as_chunks::<4>().0)
        .map(|(a, b)| {
            let luminance = |p: &[u8; 4]| i32::from(p[0]) + i32::from(p[1]) + i32::from(p[2]);
            (luminance(b) - luminance(a)) / 3
        })
        .collect()
}

fn render_with(harness: &mut Harness, path: AmbientOcclusionPath) -> Vec<u8> {
    harness.renderer.set_options(options(path));
    harness.render(&view()).1
}

#[test]
fn chunk_fields_darken_the_corners_and_follow_edits() {
    let mut harness = Harness::new(options(AmbientOcclusionPath::Off));
    let materials = materials();
    let mut scene = voxel_room();
    let mut projector = VoxelRenderProjector::new();
    let ops = project(&mut projector, &scene, &materials);
    let chunks = ops
        .iter()
        .filter(|op| match op {
            RenderDiff::ReplaceMeshPayload { payload, .. } => {
                let field = payload
                    .distance_field
                    .as_ref()
                    .expect("every chunk publishes its field");
                assert_eq!(field.cells, 8);
                assert_eq!(field.data.len(), 512);
                assert_eq!(field.extent, [8.0; 3], "one chunk box");
                true
            }
            _ => false,
        })
        .count();
    assert!(chunks > 2, "the room spans several chunks ({chunks})");
    harness.apply(ops);

    let off = render_with(&mut harness, AmbientOcclusionPath::Off);
    let readout = harness.renderer.gpu_readout();
    assert_eq!(
        readout.distance_fields.resident_fields as usize, chunks,
        "each chunk holds a brick while its mesh is retained"
    );
    assert_eq!(readout.distance_fields.atlas_bricks, 512);

    let with = render_with(&mut harness, AmbientOcclusionPath::DistanceField);
    let readout = harness.renderer.gpu_readout();
    let occlusion = &readout.ambient_occlusion;
    match &readout.distance_fields.refused {
        Some(reason) => {
            eprintln!("distance fields refused on this adapter: {reason}");
            assert_eq!(
                occlusion.path,
                AmbientOcclusionPath::Raster,
                "a refused field path falls back to the raster path"
            );
            return;
        }
        None => {
            assert_eq!(occlusion.path, AmbientOcclusionPath::DistanceField);
            assert_eq!(
                occlusion.workgroups,
                (WIDTH / 2).div_ceil(16) * (HEIGHT / 2).div_ceil(16),
                "one 16×16 workgroup per half-resolution tile"
            );
            assert_eq!(
                readout.distance_fields.lookup_entries as usize, chunks,
                "every resident field sits in the lookup grid around the camera"
            );
        }
    }
    let deltas = differences(&off, &with);
    let pixels = deltas.len() as f64;
    let darkened = deltas.iter().filter(|delta| **delta < -8).count() as f64 / pixels;
    let brightened = deltas.iter().filter(|delta| **delta > 4).count() as f64 / pixels;
    let mean = deltas.iter().map(|delta| f64::from(*delta)).sum::<f64>() / pixels;
    assert!(
        darkened > 0.01,
        "{:.2}% of pixels darkened; the wall's foot and the pillar's base should show",
        darkened * 100.0
    );
    assert!(
        brightened < 0.001,
        "{:.3}% of pixels brightened; occlusion only darkens",
        brightened * 100.0
    );
    assert!(
        mean > -40.0,
        "mean change {mean:.1}; the open floor should keep its light"
    );
    eprintln!(
        "distance field: {:.1}% of pixels darkened by more than 8, mean change {mean:.2}",
        darkened * 100.0
    );
    assert_screenshot("distance-field-occlusion", &with);

    // Clearing the pillar's top republishes its chunk: the brick is swapped,
    // not leaked.
    VoxelEditService::apply(
        &mut scene,
        &[VoxelEdit::Clear {
            address: [2, 2, -2],
        }],
    )
    .expect("edit one cell");
    let delta = harness.apply(project(&mut projector, &scene, &materials));
    assert_eq!(
        delta
            .ops
            .iter()
            .filter(|op| matches!(op, RenderDiff::ReplaceMeshPayload { .. }))
            .count(),
        1,
        "one chunk's mesh changed"
    );
    let fields = delta
        .ops
        .iter()
        .filter(|op| matches!(op, RenderDiff::ReplaceMeshDistanceField { .. }))
        .count();
    assert!(
        fields >= 1,
        "the neighbours within the field's reach republish only their fields ({fields})"
    );
    let edited = render_with(&mut harness, AmbientOcclusionPath::DistanceField);
    assert_eq!(
        harness
            .renderer
            .gpu_readout()
            .distance_fields
            .resident_fields as usize,
        chunks
    );
    assert_ne!(edited, with, "the pillar's top is gone");
}

#[test]
fn the_atlas_grows_past_its_first_bricks() {
    let mut harness = Harness::new(options(AmbientOcclusionPath::DistanceField));
    let materials = materials();
    // One voxel per chunk over a 25×25 chunk field: 625 fields, more than
    // the first 8³ bricks.
    let side = 25_i64;
    let voxels = (0..side)
        .flat_map(|i| (0..side).map(move |j| (i, j)))
        .map(|(i, j)| MaterialVoxel {
            state: 0,
            address: [i * i64::from(CHUNK_CELLS), 0, j * i64::from(CHUNK_CELLS)],
            material_slot: 1,
        })
        .collect();
    let scene = scene_from(voxels);
    let mut projector = VoxelRenderProjector::new();
    harness.apply(project(&mut projector, &scene, &materials));
    let before = harness.renderer.gpu_readout().distance_fields;
    assert_eq!(before.resident_fields, 625);
    assert_eq!(before.atlas_bricks, 4096, "doubled per axis once");
    assert_eq!(before.atlas_bytes, 4096 * 512);
    // Looking across the field from one corner: the lookup grid around the
    // camera holds the fields within 16 chunks of it.
    let pixels = harness.render(&camera([4.0, 6.0, 4.0], 45.0, -30.0)).1;
    let readout = harness.renderer.gpu_readout();
    if readout.distance_fields.refused.is_none() {
        assert_eq!(
            readout.ambient_occlusion.path,
            AmbientOcclusionPath::DistanceField
        );
        assert!(
            readout.distance_fields.lookup_entries > 200,
            "{} fields placed",
            readout.distance_fields.lookup_entries
        );
    }
    assert!(pixels.iter().any(|value| *value != 0));
}
