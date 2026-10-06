//! Scene family fixtures (#8784): voxel scene chunks with voxel surface
//! materials, voxel objects and their frames, batched and culled static
//! instances, and requested shadows. Same harness, tolerance and blessing as
//! `tests/screenshots.rs` (`RENDER_WGPU_BLESS=1` rewrites the references).

mod support;

use std::collections::BTreeMap;

use engine_spatial::{
    MaterialVoxel, SurfaceMeshOptions, VoxelCollisionScene, VoxelEdit, VoxelEditService,
};
use render_model::*;
use render_projection::{VoxelProjectionInstance, VoxelRenderProjector};
use render_wgpu::RendererOptions;
use support::*;

const CHUNK_CELLS: u32 = 8;

/// An RGBA image of `width`×`height` filled by `pixel(x, y)`.
fn image(width: u32, height: u32, pixel: impl Fn(u32, u32) -> [u8; 4]) -> Vec<u8> {
    (0..height)
        .flat_map(|y| (0..width).map(move |x| (x, y)))
        .flat_map(|(x, y)| pixel(x, y))
        .collect()
}

fn voxel_material(
    slot: u16,
    color: [f32; 4],
    texture: Option<&TextureDescriptor>,
    mapping: Option<VoxelSurfaceMappingDescriptor>,
) -> RenderMaterialDescriptor {
    let mut descriptor = material(
        &format!("voxel-material/{slot}"),
        color,
        texture.map(|texture| texture.id.as_str()),
    );
    descriptor.roughness = 1.0;
    descriptor.voxel_surface = mapping.map(|mapping| VoxelSurfaceDescriptor {
        schema_version: 1,
        filter: TextureFilter::Nearest,
        wrap: match mapping {
            VoxelSurfaceMappingDescriptor::Repeat { .. } => TextureWrap::Repeat,
            VoxelSurfaceMappingDescriptor::Atlas { .. } => TextureWrap::Clamp,
        },
        alpha_mode: VoxelSurfaceAlphaModeDescriptor::Opaque,
        mapping,
    });
    descriptor
}

/// A floor of repeat-mapped stone, a wall of atlas-mapped brick and a flat
/// pillar, across several 8³ chunks.
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
    VoxelCollisionScene::from_material_voxels_with_mesh_options(
        1.0,
        CHUNK_CELLS,
        voxels,
        SurfaceMeshOptions::default(),
    )
    .expect("voxel room")
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
        .expect("project voxel room")
        .frame
        .ops
}

#[test]
fn voxel_scene_chunks_sample_repeat_and_atlas_surfaces_and_replace_only_changed_chunks() {
    let mut harness = Harness::new(RendererOptions::default());
    // Stone: an 8×8 two-tone checker, repeated every 2 cells from the origin.
    let stone = harness.resources.texture(
        "texture/stone",
        8,
        8,
        &checker(8, [150, 150, 140, 255], [90, 90, 85, 255]),
        TextureWrap::Repeat,
    );
    // Atlas: a blue left region and a brick right region with a magenta
    // one-texel padding that half-texel inset sampling never reaches.
    let atlas = harness.resources.texture(
        "texture/atlas",
        32,
        16,
        &image(32, 16, |x, y| {
            if x < 16 {
                [40, 60, 200, 255]
            } else if x == 16 || x == 31 || y == 0 || y == 15 {
                [255, 0, 255, 255]
            } else if y % 5 == 0 || (x + if (y / 5) % 2 == 0 { 0 } else { 3 }) % 7 == 0 {
                [70, 40, 30, 255]
            } else {
                [180, 90, 50, 255]
            }
        }),
        TextureWrap::Clamp,
    );
    let materials = BTreeMap::from([
        (
            1,
            voxel_material(
                1,
                [1.0; 4],
                Some(&stone),
                Some(VoxelSurfaceMappingDescriptor::Repeat {
                    texture: stone.id.clone(),
                    texture_version: stone.version,
                    texture_content_hash: stone.content_hash.clone().unwrap(),
                    tile_scale_cells: [2.0, 2.0],
                    tile_origin_cells: [0.0, 0.0],
                }),
            ),
        ),
        (
            2,
            voxel_material(
                2,
                [1.0; 4],
                Some(&atlas),
                Some(VoxelSurfaceMappingDescriptor::Atlas {
                    atlas: "sprite-sheet/voxel-atlas".to_owned(),
                    atlas_version: 1,
                    atlas_content_hash: "atlas-fixture".to_owned(),
                    texture: atlas.id.clone(),
                    texture_version: atlas.version,
                    texture_content_hash: atlas.content_hash.clone().unwrap(),
                    region: VoxelAtlasRegionDescriptor {
                        id: "brick".to_owned(),
                        content_min: [17, 1],
                        content_extent: [14, 14],
                        padding: VoxelAtlasPaddingDescriptor {
                            left: 1,
                            right: 1,
                            bottom: 1,
                            top: 1,
                        },
                        inset: "halfTexel".to_owned(),
                    },
                    tile_scale_cells: [2.0, 2.0],
                    tile_origin_cells: [0.0, 0.0],
                }),
            ),
        ),
        (3, voxel_material(3, [0.2, 0.7, 0.35, 1.0], None, None)),
    ]);
    let mut scene = voxel_room();
    let mut projector = VoxelRenderProjector::new();
    let mut ops = vec![
        RenderDiff::DefineTexture {
            texture: stone.clone(),
        },
        RenderDiff::DefineTexture {
            texture: atlas.clone(),
        },
    ];
    ops.extend(project(&mut projector, &scene, &materials));
    let chunks = ops
        .iter()
        .filter(|op| matches!(op, RenderDiff::ReplaceMeshPayload { .. }))
        .count();
    assert!(chunks > 2, "the room spans several chunks ({chunks})");
    harness.apply(ops);
    let view = camera([3.5, 3.0, 4.5], 30.0, -25.0);
    let (first, _) = harness.render(&view);
    assert!(first.draws > 0);

    // Clear the pillar's top cell: only its chunk is republished.
    VoxelEditService::apply(
        &mut scene,
        &[VoxelEdit::Clear {
            address: [2, 2, -2],
        }],
    )
    .expect("edit one cell");
    let delta = harness.apply(project(&mut projector, &scene, &materials));
    let replaced: Vec<usize> = delta
        .ops
        .iter()
        .filter_map(|op| match op {
            RenderDiff::ReplaceMeshPayload { payload, .. } => Some(payload.groups.len()),
            _ => None,
        })
        .collect();
    assert_eq!(replaced.len(), 1, "one chunk changed of {chunks}");
    let (edited, pixels) = harness.render(&view);
    assert_eq!(
        edited.parts_uploaded as usize, replaced[0],
        "only the replaced chunk's groups upload"
    );
    assert_screenshot("voxel-surfaces", &pixels);
}

/// A voxel object asset: a one-cell body (slot 1), and a two-cell body with a
/// glowing top (slot 2); frames `idle` and `idle-again` share the first mesh.
fn lantern() -> VoxelObjectRenderAsset {
    let body = VoxelObjectRenderMesh {
        payload: box_mesh([-0.5, 0.0, -0.5], [0.5, 1.0, 0.5], |_| 1),
    };
    let lit = VoxelObjectRenderMesh {
        payload: box_mesh([-0.5, 0.0, -0.5], [0.5, 2.0, 0.5], |normal| {
            if normal[1] > 0.5 {
                2
            } else {
                1
            }
        }),
    };
    VoxelObjectRenderAsset {
        asset: "voxel-object/lantern".to_owned(),
        content_hash: "lantern-fixture".to_owned(),
        meshes: vec![body, lit],
        frames: ["idle", "lit", "idle-again"]
            .iter()
            .zip([0, 1, 0])
            .map(|(id, mesh)| VoxelObjectRenderFrame {
                id: (*id).to_owned(),
                mesh,
            })
            .collect(),
        material_slots: vec![
            MeshMaterialSlot {
                slot: 1,
                material: "material/lantern-body".to_owned(),
            },
            MeshMaterialSlot {
                slot: 2,
                material: "material/lantern-glow".to_owned(),
            },
        ],
    }
}

fn voxel_object(handle: u64, frame: u32, x: f32, overrides: Vec<MeshMaterialSlot>) -> RenderDiff {
    RenderDiff::CreateVoxelObjectInstance {
        handle: RenderHandle::new(handle),
        parent: None,
        instance: VoxelObjectInstanceDescriptor {
            asset: "voxel-object/lantern".to_owned(),
            frame,
            transform: transform([x, 0.0, -4.0], 25.0, [1.0; 3]),
            visible: true,
            material_overrides: overrides,
            metadata: RenderMetadata::default(),
        },
    }
}

#[test]
fn voxel_objects_draw_their_frame_mesh_and_switch_frames_without_uploading_geometry() {
    let mut harness = Harness::new(RendererOptions::default());
    let mut glow = material("material/lantern-glow", [1.0, 0.8, 0.3, 1.0], None);
    glow.emission_color = [1.0, 0.7, 0.2];
    glow.emission_intensity = 1.5;
    harness.apply(vec![
        RenderDiff::DefineMaterial {
            material: material("material/lantern-body", [0.35, 0.3, 0.28, 1.0], None),
        },
        RenderDiff::DefineMaterial { material: glow },
        RenderDiff::DefineMaterial {
            material: material("material/lantern-alt", [0.2, 0.35, 0.6, 1.0], None),
        },
        RenderDiff::DefineVoxelObject { asset: lantern() },
        voxel_object(1, 0, -2.4, Vec::new()),
        voxel_object(2, 1, -0.8, Vec::new()),
        voxel_object(
            3,
            0,
            0.8,
            vec![MeshMaterialSlot {
                slot: 1,
                material: "material/lantern-alt".to_owned(),
            }],
        ),
        voxel_object(4, 2, 2.4, Vec::new()),
    ]);
    let view = camera([0.0, 3.6, 0.5], 0.0, -35.0);
    let (first, _) = harness.render(&view);
    // Lanterns 1 and 4 (frames sharing mesh 0) batch; 2 draws its three
    // groups (sides, glowing top, sides); 3 overrides its body material.
    assert_eq!((first.draws, first.instances), (5, 6));

    harness.apply(vec![RenderDiff::SetVoxelObjectFrame {
        handle: RenderHandle::new(1),
        frame: 1,
    }]);
    let (switched, pixels) = harness.render(&view);
    assert_eq!(
        switched.parts_uploaded, 3,
        "lantern 1 rebinds to three parts"
    );
    // Lantern 1 now batches with each of lantern 2's groups.
    assert_eq!((switched.draws, switched.instances), (5, 8));
    assert_eq!(harness.renderer.table_counts().voxel_objects, 1);
    assert_screenshot("voxel-objects", &pixels);
}

#[test]
fn static_instances_batch_cull_and_keep_the_instance_list_while_nothing_changes() {
    let mut harness = Harness::new(RendererOptions::default());
    let crate_texture = harness.resources.texture(
        "texture/crate",
        4,
        4,
        &image(4, 4, |x, _| {
            [60 + 60 * x as u8, 120, 200 - 40 * x as u8, 255]
        }),
        TextureWrap::Clamp,
    );
    let mut ops = vec![
        RenderDiff::DefineTexture {
            texture: crate_texture,
        },
        RenderDiff::DefineMaterial {
            material: material("material/crate", [1.0; 4], Some("texture/crate")),
        },
        RenderDiff::DefineMaterial {
            material: material("material/floor", [0.35, 0.38, 0.35, 1.0], None),
        },
        static_mesh(
            "mesh/crate",
            box_mesh([-0.4, 0.0, -0.4], [0.4, 0.8, 0.4], |_| 0),
            "material/crate",
        ),
        static_mesh(
            "mesh/floor",
            box_mesh([-12.0, -0.1, -12.0], [12.0, 0.0, 12.0], |_| 0),
            "material/floor",
        ),
        instance(1, None, "mesh/floor", Transform::IDENTITY),
    ];
    let mut handle = 100;
    for x in -5..5 {
        for z in -5..5 {
            ops.push(instance(
                handle,
                None,
                "mesh/crate",
                transform(
                    [x as f32 * 2.0 + 1.0, 0.0, z as f32 * 2.0 + 1.0],
                    0.0,
                    [1.0; 3],
                ),
            ));
            handle += 1;
        }
    }
    // A mirrored crate: faces wind the other way and must still face out.
    ops.push(instance(
        handle,
        None,
        "mesh/crate",
        transform([0.0, 1.2, -3.0], 30.0, [-1.5, 1.5, 1.5]),
    ));
    harness.apply(ops);

    let view = camera([0.0, 3.0, 9.0], 0.0, -20.0);
    let (first, pixels) = harness.render(&view);
    // Floor, the crate batch and the mirrored crate; crates outside the
    // frustum are culled.
    assert_eq!(first.draws, 3);
    assert!(first.instances < 102, "culled to {}", first.instances);
    assert_screenshot("static-batching", &pixels);

    let (again, _) = harness.render(&view);
    assert_eq!(
        (again.parts_uploaded, again.instances_uploaded),
        (0, 0),
        "nothing changed: no row or instance upload"
    );

    // Moving one crate rewrites its row only; the drawn set is the same.
    harness.apply(vec![RenderDiff::Update {
        handle: RenderHandle::new(155),
        transform: Some(transform([1.0, 0.3, 1.0], 20.0, [1.0; 3])),
        material: None,
        visible: None,
        metadata: None,
    }]);
    let (moved, _) = harness.render(&view);
    // The drawn set is unchanged, so no instance ids upload (review of
    // fcdbeb388).
    assert_eq!(
        (moved.parts_uploaded, moved.draws, moved.instances_uploaded),
        (1, 3, 0)
    );

    // Turning away culls a different set.
    let (turned, _) = harness.render(&camera([0.0, 3.0, 9.0], 60.0, -20.0));
    assert!(turned.instances_uploaded > 0 && turned.instances != first.instances);
}

fn shadow_scene(harness: &mut Harness) {
    harness.apply(vec![
        RenderDiff::DefineMaterial {
            material: material("material/floor", [0.75, 0.75, 0.72, 1.0], None),
        },
        RenderDiff::DefineMaterial {
            material: material("material/crate", [0.7, 0.45, 0.3, 1.0], None),
        },
        static_mesh(
            "mesh/floor",
            box_mesh([-8.0, -0.1, -8.0], [8.0, 0.0, 8.0], |_| 0),
            "material/floor",
        ),
        static_mesh(
            "mesh/crate",
            box_mesh([-0.5, 0.0, -0.5], [0.5, 1.0, 0.5], |_| 0),
            "material/crate",
        ),
        instance(1, None, "mesh/floor", Transform::IDENTITY),
        instance(
            2,
            None,
            "mesh/crate",
            transform([-1.5, 0.0, -1.0], 20.0, [1.0; 3]),
        ),
        instance(
            3,
            None,
            "mesh/crate",
            transform([1.2, 0.0, 0.5], -15.0, [1.0, 1.6, 1.0]),
        ),
        RenderDiff::CreateLight {
            handle: RenderHandle::new(10),
            parent: None,
            light: LightDescriptor::Directional {
                color: [1.0, 0.95, 0.85],
                intensity: 2.0,
                enabled: true,
                direction: [-0.9, -0.8, 0.6],
                range: None,
                shadow_intent: LightShadowIntent::Requested,
            },
        },
        RenderDiff::CreateLight {
            handle: RenderHandle::new(11),
            parent: None,
            light: LightDescriptor::Spot {
                color: [0.4, 0.6, 1.0],
                intensity: 30.0,
                enabled: true,
                position: [3.0, 4.0, 3.0],
                direction: [-0.6, -1.0, -0.6],
                range: Some(14.0),
                decay: 2.0,
                outer_angle_radians: 0.6,
                penumbra: 0.2,
                shadow_intent: LightShadowIntent::Requested,
            },
        },
        RenderDiff::CreateLight {
            handle: RenderHandle::new(12),
            parent: None,
            light: LightDescriptor::Point {
                color: [1.0, 0.55, 0.2],
                intensity: 8.0,
                enabled: true,
                position: [-0.2, 1.4, -2.6],
                range: Some(8.0),
                decay: 2.0,
                shadow_intent: LightShadowIntent::Requested,
            },
        },
        RenderDiff::CreateLight {
            handle: RenderHandle::new(13),
            parent: None,
            light: LightDescriptor::Ambient {
                color: [1.0; 3],
                intensity: 0.4,
                enabled: true,
                shadow_intent: LightShadowIntent::Disabled,
            },
        },
    ]);
}

#[test]
fn requested_shadows_render_per_light_layers_only_when_the_scene_changes() {
    let mut harness = Harness::new(RendererOptions {
        default_world_lights: false,
        shadows: true,
        ..RendererOptions::default()
    });
    shadow_scene(&mut harness);
    let view = camera([0.0, 4.5, 6.5], 0.0, -32.0);
    let (first, pixels) = harness.render(&view);
    // The directional light takes four cascades, the spot light one layer
    // and the point light six.
    assert_eq!(harness.renderer.table_counts().shadow_layers, 11);
    assert_eq!(first.shadow_layers, 11, "every layer rendered once");
    assert_screenshot("shadows", &pixels);

    let (again, _) = harness.render(&view);
    assert_eq!(again.shadow_layers, 0, "an unchanged scene reuses the maps");
    let (moved, _) = harness.render(&camera([0.5, 4.5, 6.5], 4.0, -32.0));
    assert!(
        (1..=4).contains(&moved.shadow_layers),
        "camera motion re-renders cascades only: {moved:?}"
    );

    // A new object casts at once, with no per-object setup. Only the layers
    // that see it render again, and the result is what a fresh renderer
    // draws.
    let crate_at = |at| {
        vec![instance(
            4,
            None,
            "mesh/crate",
            transform(at, 0.0, [1.0; 3]),
        )]
    };
    harness.apply(crate_at([0.0, 0.0, 2.0]));
    let (added, pixels) = harness.render(&view);
    assert!(
        0 < added.shadow_layers && added.shadow_layers < 8,
        "{added:?}"
    );
    let mut fresh = Harness::new(RendererOptions {
        default_world_lights: false,
        shadows: true,
        ..RendererOptions::default()
    });
    shadow_scene(&mut fresh);
    fresh.apply(crate_at([0.0, 0.0, 2.0]));
    assert_eq!(pixels, fresh.render(&view).1);

    // Moving it out of every light's view re-renders the layers it left.
    harness.apply(vec![RenderDiff::Update {
        handle: RenderHandle::new(4),
        transform: Some(transform([0.0, 0.0, 40.0], 0.0, [1.0; 3])),
        material: None,
        visible: None,
        metadata: None,
    }]);
    let (left, pixels) = harness.render(&view);
    assert_eq!(left.shadow_layers, added.shadow_layers, "{left:?}");
    let mut fresh = Harness::new(RendererOptions {
        default_world_lights: false,
        shadows: true,
        ..RendererOptions::default()
    });
    shadow_scene(&mut fresh);
    fresh.apply(crate_at([0.0, 0.0, 40.0]));
    assert_eq!(pixels, fresh.render(&view).1);
}

#[test]
fn a_light_changing_intensity_renders_no_shadow_layer() {
    let mut harness = Harness::new(RendererOptions {
        default_world_lights: false,
        shadows: true,
        ..RendererOptions::default()
    });
    shadow_scene(&mut harness);
    let view = camera([0.0, 4.5, 6.5], 0.0, -32.0);
    harness.render(&view);
    let point = |intensity| LightDescriptor::Point {
        color: [1.0, 0.55, 0.2],
        intensity,
        enabled: true,
        position: [-0.2, 1.4, -2.6],
        range: Some(8.0),
        decay: 2.0,
        shadow_intent: LightShadowIntent::Requested,
    };
    // A flickering lamp.
    harness.apply(vec![RenderDiff::UpdateLight {
        handle: RenderHandle::new(12),
        light: point(5.0),
    }]);
    let (dimmed, _) = harness.render(&view);
    assert_eq!((dimmed.shadow_layers, dimmed.shadow_draws), (0, 0));
    // Moving it re-renders its six faces only.
    let mut moved = point(5.0);
    if let LightDescriptor::Point { position, .. } = &mut moved {
        *position = [0.2, 1.4, -2.6];
    }
    harness.apply(vec![RenderDiff::UpdateLight {
        handle: RenderHandle::new(12),
        light: moved,
    }]);
    assert_eq!(harness.render(&view).0.shadow_layers, 6);
    // A turning sun re-renders its four cascades only.
    harness.apply(vec![RenderDiff::UpdateLight {
        handle: RenderHandle::new(10),
        light: LightDescriptor::Directional {
            color: [1.0, 0.95, 0.85],
            intensity: 2.0,
            enabled: true,
            direction: [-0.85, -0.85, 0.6],
            range: None,
            shadow_intent: LightShadowIntent::Requested,
        },
    }]);
    assert_eq!(harness.render(&view).0.shadow_layers, 4);
}

#[test]
fn a_light_with_a_range_draws_only_the_casters_it_reaches() {
    let mut harness = Harness::new(RendererOptions {
        default_world_lights: false,
        shadows: true,
        ..RendererOptions::default()
    });
    let mut ops = vec![
        RenderDiff::DefineMaterial {
            material: material("material/crate", [0.7, 0.45, 0.3, 1.0], None),
        },
        static_mesh(
            "mesh/crate",
            box_mesh([-0.5, 0.0, -0.5], [0.5, 1.0, 0.5], |_| 0),
            "material/crate",
        ),
        RenderDiff::CreateLight {
            handle: RenderHandle::new(100),
            parent: None,
            light: LightDescriptor::Point {
                color: [1.0; 3],
                intensity: 8.0,
                enabled: true,
                position: [0.0, 0.5, 0.0],
                range: Some(3.0),
                decay: 2.0,
                shadow_intent: LightShadowIntent::Requested,
            },
        },
    ];
    // One crate beside the light, on its +X side; a row of twenty beyond
    // its range.
    ops.push(instance(
        1,
        None,
        "mesh/crate",
        transform([1.5, 0.0, 0.0], 0.0, [1.0; 3]),
    ));
    for index in 0..20 {
        let x = 5.0 + index as f32;
        ops.push(instance(
            2 + index,
            None,
            "mesh/crate",
            transform([x, 0.0, 0.0], 0.0, [1.0; 3]),
        ));
    }
    harness.apply(ops);
    let (stats, _) = harness.render(&camera([0.0, 3.0, 6.0], 0.0, -25.0));
    assert_eq!(stats.shadow_layers, 6);
    // The near crate reaches the +X face (and no face sees the far row);
    // its bounds may touch the faces beside it, never the -X face.
    assert!(
        (1..=5).contains(&stats.shadow_casters),
        "{} casters",
        stats.shadow_casters
    );
}

#[test]
fn shadow_requests_are_ignored_unless_the_host_enables_shadows() {
    let mut harness = Harness::new(RendererOptions {
        default_world_lights: false,
        ..RendererOptions::default()
    });
    shadow_scene(&mut harness);
    let (stats, _) = harness.render(&camera([0.0, 4.5, 6.5], 0.0, -32.0));
    assert_eq!(
        (
            stats.shadow_draws,
            harness.renderer.table_counts().shadow_layers
        ),
        (0, 0)
    );
}

/// Give an inline payload one RGBA colour per vertex.
fn with_colors(
    mut payload: MeshPayloadDescriptor,
    color: impl Fn([f32; 3]) -> [f32; 4],
) -> MeshPayloadDescriptor {
    let MeshPayloadSource::Inline {
        positions, colors, ..
    } = &mut payload.source
    else {
        unreachable!("fixture payloads are inline")
    };
    *colors = Some(
        positions
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|p| color([p[0], p[1], p[2]]))
            .collect(),
    );
    payload.layout.attributes.push(MeshAttribute {
        name: MeshAttributeName::Color,
        components: 4,
        kind: MeshAttributeKind::F32,
    });
    payload
}

#[test]
fn static_mesh_vertex_colours_multiply_the_material_and_payloads_ignore_them() {
    let mut harness = Harness::new(RendererOptions {
        default_world_lights: false,
        ..RendererOptions::default()
    });
    let flat = material("material/flat", [1.0; 4], None);
    let panel = |x0: f32| box_mesh([x0, -0.8, -3.0], [x0 + 1.0, 0.8, -2.9], |_| 0);
    // Red to the left of the panel's centre, blue to the right.
    let colored = with_colors(panel(-1.2), |p| {
        if p[0] < -0.7 {
            [1.0, 0.0, 0.0, 1.0]
        } else {
            [0.0, 0.0, 1.0, 1.0]
        }
    });
    harness.apply(vec![
        RenderDiff::DefineMaterial { material: flat },
        static_mesh("mesh/colored", colored, "material/flat"),
        instance(1, None, "mesh/colored", Transform::IDENTITY),
        RenderDiff::DefineMaterial {
            material: voxel_material(0, [1.0; 4], None, None),
        },
        group(2, None, Transform::IDENTITY),
        // Ambient π: a lit surface shows exactly its base colour.
        RenderDiff::CreateLight {
            handle: RenderHandle::new(4),
            parent: None,
            light: LightDescriptor::Ambient {
                color: [1.0; 3],
                intensity: std::f32::consts::PI,
                enabled: true,
                shadow_intent: LightShadowIntent::Disabled,
            },
        },
    ]);
    // The same colours on an uploaded payload are ignored.
    let payload = with_colors(panel(0.2), |_| [1.0, 0.0, 0.0, 1.0]);
    let mut node = RenderNode::new(Geometry::Cube);
    node.material = Material {
        color: [1.0; 4],
        wireframe: false,
    };
    harness.apply(vec![
        RenderDiff::Create {
            handle: RenderHandle::new(3),
            parent: None,
            node,
        },
        RenderDiff::ReplaceMeshPayload {
            handle: RenderHandle::new(3),
            payload,
        },
    ]);
    let (_, pixels) = harness.render(&camera([0.0, 0.0, 0.0], 0.0, 0.0));
    let pixel = |x: u32, y: u32| {
        let at = ((y * WIDTH + x) * 4) as usize;
        [pixels[at], pixels[at + 1], pixels[at + 2]]
    };
    let project = |x: f32| {
        (WIDTH as f32 * 0.5 + x / 2.9 * (HEIGHT as f32 * 0.5) / (30f32.to_radians().tan())) as u32
    };
    // Colours interpolate across the face: sample near each edge.
    let (red, blue, payload) = (
        pixel(project(-1.15), HEIGHT / 2),
        pixel(project(-0.25), HEIGHT / 2),
        pixel(project(0.7), HEIGHT / 2),
    );
    assert!(
        red[0] > 200 && red[2] < 100 && red[1] < 20,
        "left edge is red: {red:?}"
    );
    assert!(
        blue[2] > 200 && blue[0] < 100 && blue[1] < 20,
        "right edge is blue: {blue:?}"
    );
    assert!(
        payload[1] > 100,
        "the payload ignores its vertex colours: {payload:?}"
    );
}

#[test]
fn materials_share_a_pipeline_per_feature_set_made_when_the_material_is_defined() {
    let mut harness = Harness::new(RendererOptions::default());
    let masked = |id: &str, color| RenderMaterialDescriptor {
        alpha_mode: MaterialAlphaModeDescriptor::Mask { cutoff: 0.5 },
        ..material(id, color, None)
    };
    let mut ops = vec![
        RenderDiff::DefineMaterial {
            material: material("material/red", [0.8, 0.2, 0.2, 1.0], None),
        },
        RenderDiff::DefineMaterial {
            material: material("material/green", [0.2, 0.8, 0.2, 1.0], None),
        },
        RenderDiff::DefineMaterial {
            material: masked("material/blue", [0.2, 0.2, 0.8, 1.0]),
        },
    ];
    for (handle, (asset, material)) in [
        ("mesh/red", "material/red"),
        ("mesh/blue", "material/blue"),
        ("mesh/green", "material/green"),
    ]
    .into_iter()
    .enumerate()
    {
        ops.push(static_mesh(
            asset,
            box_mesh([-0.4, 0.0, -0.4], [0.4, 0.8, 0.4], |_| 0),
            material,
        ));
        let x = handle as f32 * 1.2 - 1.2;
        ops.push(instance(
            handle as u64 + 1,
            None,
            asset,
            transform([x, 0.0, 0.0], 0.0, [1.0; 3]),
        ));
    }
    harness.apply(ops);
    let view = camera([0.0, 1.5, 4.0], 0.0, -15.0);

    // Red and green compile no features and share one pipeline; blue's alpha
    // mask is a second. The target is new, so both are made for this frame.
    let (first, _) = harness.render(&view);
    assert_eq!(
        (first.draws, first.pipeline_binds, first.pipelines_created),
        (3, 2, 2)
    );

    // Another masked material reuses blue's pipeline; a voxel surface is a
    // new feature set, compiled when it is defined rather than when drawn.
    harness.apply(vec![
        RenderDiff::DefineMaterial {
            material: masked("material/yellow", [0.8, 0.8, 0.2, 1.0]),
        },
        RenderDiff::DefineMaterial {
            material: voxel_material(
                9,
                [0.6, 0.6, 0.6, 1.0],
                None,
                Some(VoxelSurfaceMappingDescriptor::Repeat {
                    texture: String::new(),
                    texture_version: 0,
                    texture_content_hash: String::new(),
                    tile_scale_cells: [1.0, 1.0],
                    tile_origin_cells: [0.0, 0.0],
                }),
            ),
        },
        static_mesh(
            "mesh/yellow",
            box_mesh([-0.4, 0.0, -0.4], [0.4, 0.8, 0.4], |_| 0),
            "material/yellow",
        ),
        static_mesh(
            "mesh/stone",
            box_mesh([-0.4, 0.0, -0.4], [0.4, 0.8, 0.4], |_| 0),
            "voxel-material/9",
        ),
        instance(
            10,
            None,
            "mesh/yellow",
            transform([0.0, 1.0, 0.0], 0.0, [1.0; 3]),
        ),
        instance(
            11,
            None,
            "mesh/stone",
            transform([0.0, -1.0, 0.0], 0.0, [1.0; 3]),
        ),
    ]);
    let (second, _) = harness.render(&view);
    assert_eq!(
        (
            second.draws,
            second.pipeline_binds,
            second.pipelines_created
        ),
        (5, 3, 0)
    );
}

#[test]
fn metal_materials_lose_their_diffuse_and_tint_their_specular() {
    let mut harness = Harness::new(RendererOptions {
        default_world_lights: false,
        ..RendererOptions::default()
    });
    let gold = [1.0, 0.75, 0.3, 1.0];
    let mut ops = vec![
        RenderDiff::SetBackgroundColor {
            color: [0.1, 0.1, 0.12, 1.0],
        },
        RenderDiff::CreateLight {
            handle: RenderHandle::new(90),
            parent: None,
            light: LightDescriptor::Directional {
                color: [1.0, 1.0, 1.0],
                intensity: 3.0,
                enabled: true,
                direction: [0.4, -0.6, -1.0],
                range: None,
                shadow_intent: LightShadowIntent::Disabled,
            },
        },
        RenderDiff::CreateLight {
            handle: RenderHandle::new(91),
            parent: None,
            light: LightDescriptor::Ambient {
                color: [0.6, 0.7, 0.9],
                intensity: 0.8,
                enabled: true,
                shadow_intent: LightShadowIntent::Disabled,
            },
        },
    ];
    for (index, metalness) in [0.0, 1.0].into_iter().enumerate() {
        let id = format!("material/gold-{index}");
        let mesh = format!("mesh/gold-{index}");
        ops.push(RenderDiff::DefineMaterial {
            material: RenderMaterialDescriptor {
                metalness,
                roughness: 0.35,
                ..material(&id, gold, None)
            },
        });
        ops.push(static_mesh(
            &mesh,
            box_mesh([-0.6, -0.6, -0.6], [0.6, 0.6, 0.6], |_| 0),
            &id,
        ));
        let x = if index == 0 { -0.9 } else { 0.9 };
        ops.push(instance(
            index as u64 + 1,
            None,
            &mesh,
            transform([x, 0.0, -4.0], 35.0, [1.0; 3]),
        ));
    }
    harness.apply(ops);
    let (_, rgba) = harness.render(&camera([0.0, 0.0, 0.0], 0.0, 0.0));
    assert_screenshot("metalness", &rgba);

    // The two boxes' front faces: the metal one has no diffuse. Away from
    // the sun's highlight it shows the ambient light reflected and tinted
    // gold, darker here than the dielectric's diffuse.
    let at = |x: usize| {
        let offset = (HEIGHT as usize / 2 * WIDTH as usize + x) * 4;
        [rgba[offset], rgba[offset + 1], rgba[offset + 2]].map(f32::from)
    };
    let quarter = WIDTH as usize / 4;
    let (dielectric, metal) = (at(quarter + 20), at(3 * quarter - 10));
    let luminance = |c: [f32; 3]| 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
    assert!(
        luminance(metal) < luminance(dielectric),
        "metal {metal:?} dielectric {dielectric:?}"
    );
    assert!(
        metal[0] > metal[2],
        "metal reflection keeps the gold tint: {metal:?}"
    );
}

#[test]
fn linear_textures_mipmap_so_a_receding_tiled_floor_settles_to_its_average() {
    // A one-texel black and white checker tiled 200 times over a 100 m
    // floor, seen at a grazing angle.
    let checker: Vec<u8> = (0..64)
        .flat_map(|texel| {
            let white = (texel % 8 + texel / 8) % 2 == 0;
            if white {
                [255, 255, 255, 255]
            } else {
                [0, 0, 0, 255]
            }
        })
        .collect();
    let floor = payload(
        vec![
            -50.0, 0.0, -50.0, 50.0, 0.0, -50.0, 50.0, 0.0, 50.0, -50.0, 0.0, 50.0,
        ],
        vec![0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0],
        vec![0.0, 0.0, 200.0, 0.0, 200.0, 200.0, 0.0, 200.0],
        vec![0, 2, 1, 0, 3, 2],
        &[(0, 6)],
    );
    let render = |filter: TextureFilter| {
        let mut harness = Harness::new(RendererOptions::default());
        let mut texture =
            harness
                .resources
                .texture("texture/checker", 8, 8, &checker, TextureWrap::Repeat);
        texture.filter = filter;
        harness.apply(vec![
            RenderDiff::DefineTexture { texture },
            RenderDiff::DefineMaterial {
                material: material("material/checker", [1.0; 4], Some("texture/checker")),
            },
            static_mesh("mesh/floor", floor.clone(), "material/checker"),
            instance(1, None, "mesh/floor", Transform::IDENTITY),
        ]);
        harness.render(&camera([0.0, 1.5, 0.0], 0.0, -8.0)).1
    };
    // Luminance spread along rows just below the horizon, where each pixel
    // covers many texels.
    let far_spread = |rgba: &[u8]| {
        let rows = HEIGHT as usize / 2 + 8..HEIGHT as usize / 2 + 24;
        let mut spread = 0.0;
        for y in rows.clone() {
            let row: Vec<f32> = (40..WIDTH as usize - 40)
                .map(|x| f32::from(rgba[(y * WIDTH as usize + x) * 4 + 1]))
                .collect();
            let mean = row.iter().sum::<f32>() / row.len() as f32;
            spread +=
                (row.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / row.len() as f32).sqrt();
        }
        spread / rows.len() as f32
    };
    // No reference image: anisotropic filtering is implementation-defined,
    // and llvmpipe and RADV differ on this checker near the Nyquist limit.
    let mipmapped = render(TextureFilter::Linear);
    let single = render(TextureFilter::Nearest);
    let (smooth, aliased) = (far_spread(&mipmapped), far_spread(&single));
    assert!(
        smooth * 4.0 < aliased,
        "far texels average to grey with mips: spread {smooth} against {aliased} unmipmapped"
    );
}

/// A normal map texture tilted toward tangent-space +X, opened as linear
/// data as a product opens one.
fn tilted_normal_map(harness: &mut Harness, id: &str, wrap: TextureWrap) -> TextureDescriptor {
    let texel = [
        ((0.6_f32 * 0.5 + 0.5) * 255.0).round() as u8,
        128,
        ((0.8_f32 * 0.5 + 0.5) * 255.0).round() as u8,
        255,
    ];
    let mut texture = harness.resources.texture(id, 2, 2, &texel.repeat(4), wrap);
    if let Some(payload) = texture.payload.as_mut() {
        payload.color_space = TextureColorSpace::Linear;
    }
    texture
}

fn sun(direction: [f32; 3]) -> RenderDiff {
    RenderDiff::CreateLight {
        handle: RenderHandle::new(90),
        parent: None,
        light: LightDescriptor::Directional {
            color: [1.0; 3],
            intensity: 2.5,
            enabled: true,
            direction,
            range: None,
            shadow_intent: LightShadowIntent::Disabled,
        },
    }
}

#[test]
fn an_engine_material_normal_map_turns_shading_as_the_light_moves() {
    let render = |normal_map: bool, direction: [f32; 3]| {
        let mut harness = Harness::new(RendererOptions {
            default_world_lights: false,
            ..RendererOptions::default()
        });
        let map = tilted_normal_map(&mut harness, "texture/normal", TextureWrap::Clamp);
        let mut descriptor = material("material/wall", [0.8, 0.8, 0.8, 1.0], None);
        descriptor.normal_map = normal_map.then(|| MaterialNormalMapDescriptor {
            texture: map.id.clone(),
            scale: 1.0,
        });
        harness.apply(vec![
            RenderDiff::DefineTexture { texture: map },
            RenderDiff::DefineMaterial {
                material: descriptor,
            },
            static_mesh(
                "mesh/wall",
                box_mesh([-1.0, -1.0, -0.1], [1.0, 1.0, 0.1], |_| 0),
                "material/wall",
            ),
            instance(1, None, "mesh/wall", Transform::IDENTITY),
            sun(direction),
        ]);
        let (_, rgba) = harness.render(&camera([0.0, 0.0, 3.0], 0.0, 0.0));
        let at = (HEIGHT as usize / 2 * WIDTH as usize + WIDTH as usize / 2) * 4;
        u32::from(rgba[at]) + u32::from(rgba[at + 1]) + u32::from(rgba[at + 2])
    };
    // The face's +u runs along +X, so the map turns it toward +X.
    let from_right = render(true, [-1.0, 0.0, -1.0]);
    let from_left = render(true, [1.0, 0.0, -1.0]);
    assert!(
        from_right > from_left + 60,
        "lit from +X {from_right}, from -X {from_left}"
    );
    // Without the map the face is lit alike from either side.
    let (plain_right, plain_left) = (
        render(false, [-1.0, 0.0, -1.0]),
        render(false, [1.0, 0.0, -1.0]),
    );
    assert!(
        plain_right.abs_diff(plain_left) < 6,
        "{plain_right} {plain_left}"
    );
}

#[test]
fn a_voxel_surface_normal_map_follows_its_tiles_without_seams() {
    let render = |direction: [f32; 3]| {
        let mut harness = Harness::new(RendererOptions {
            default_world_lights: false,
            ..RendererOptions::default()
        });
        let stone = harness.resources.texture(
            "texture/stone",
            2,
            2,
            &[200, 200, 200, 255].repeat(4),
            TextureWrap::Repeat,
        );
        let map = tilted_normal_map(&mut harness, "texture/stone-normal", TextureWrap::Repeat);
        let mut floor = voxel_material(
            1,
            [1.0; 4],
            Some(&stone),
            Some(VoxelSurfaceMappingDescriptor::Repeat {
                texture: stone.id.clone(),
                texture_version: stone.version,
                texture_content_hash: stone.content_hash.clone().unwrap(),
                tile_scale_cells: [1.0, 1.0],
                tile_origin_cells: [0.0, 0.0],
            }),
        );
        floor.normal_map = Some(MaterialNormalMapDescriptor {
            texture: map.id.clone(),
            scale: 1.0,
        });
        let materials = BTreeMap::from([(1, floor)]);
        let floor_only = VoxelCollisionScene::from_material_voxels_with_mesh_options(
            1.0,
            CHUNK_CELLS,
            (-6..6).flat_map(|x| {
                (-12..2).map(move |z| MaterialVoxel {
                    state: 0,
                    address: [x, -1, z],
                    material_slot: 1,
                })
            }),
            SurfaceMeshOptions::default(),
        )
        .unwrap();
        let mut projector = VoxelRenderProjector::new();
        let mut ops = vec![
            RenderDiff::DefineTexture { texture: stone },
            RenderDiff::DefineTexture { texture: map },
            sun(direction),
        ];
        ops.extend(project(&mut projector, &floor_only, &materials));
        harness.apply(ops);
        harness.render(&camera([0.0, 1.5, 1.0], 0.0, -35.0)).1
    };
    // A floor tile's +u runs along +Z, so the map tilts the floor toward +Z.
    let lit = render([0.0, -0.6, -1.0]);
    let dim = render([0.0, -0.6, 1.0]);
    // A block of floor spanning several tile seams in each direction.
    let block = |rgba: &[u8]| -> Vec<u32> {
        (HEIGHT as usize / 3..HEIGHT as usize)
            .flat_map(|y| (WIDTH as usize / 4..WIDTH as usize * 3 / 4).map(move |x| (x, y)))
            .map(|(x, y)| {
                let at = (y * WIDTH as usize + x) * 4;
                u32::from(rgba[at]) + u32::from(rgba[at + 1]) + u32::from(rgba[at + 2])
            })
            .collect()
    };
    let (lit, dim) = (block(&lit), block(&dim));
    let mean = |values: &[u32]| values.iter().sum::<u32>() / values.len() as u32;
    assert!(
        mean(&lit) > mean(&dim) + 30,
        "tilted relief: {} against {}",
        mean(&lit),
        mean(&dim)
    );
    // Uniform tiles: no darker line where one tile meets the next, as a
    // frame from the wrapped uv would draw.
    let (low, high) = (lit.iter().min().unwrap(), lit.iter().max().unwrap());
    assert!(
        high - low < 12,
        "seam lines across the floor: {low}..{high}"
    );
}

/// Blocks across several chunks, on both sides of the origin: a floor, a
/// wall facing +Z, and a pillar showing its ±X faces.
fn block_faces() -> VoxelCollisionScene {
    let mut voxels = Vec::new();
    for x in -10..6 {
        for z in -12..2 {
            voxels.push(MaterialVoxel {
                state: 0,
                address: [x, -1, z],
                material_slot: 1,
            });
        }
        for y in 0..5 {
            voxels.push(MaterialVoxel {
                state: 0,
                address: [x, y, -12],
                material_slot: 1,
            });
        }
    }
    for y in 0..4 {
        voxels.push(MaterialVoxel {
            state: 0,
            address: [-3, y, -6],
            material_slot: 1,
        });
    }
    VoxelCollisionScene::from_material_voxels_with_mesh_options(
        1.0,
        CHUNK_CELLS,
        voxels,
        SurfaceMeshOptions::default(),
    )
    .expect("block faces")
}

#[test]
fn a_triplanar_material_keeps_block_faces_as_box_projection_draws_them() {
    let render = |triplanar: bool| {
        let mut harness = Harness::new(RendererOptions {
            default_world_lights: false,
            ..RendererOptions::default()
        });
        let stone = harness.resources.texture(
            "texture/stone",
            8,
            8,
            &image(8, 8, |x, y| {
                [40 + 25 * x as u8, 40 + 25 * y as u8, 120, 255]
            }),
            TextureWrap::Repeat,
        );
        let map = tilted_normal_map(&mut harness, "texture/stone-normal", TextureWrap::Repeat);
        let mut stone_material = voxel_material(
            1,
            [1.0; 4],
            Some(&stone),
            Some(VoxelSurfaceMappingDescriptor::Repeat {
                texture: stone.id.clone(),
                texture_version: stone.version,
                texture_content_hash: stone.content_hash.clone().unwrap(),
                tile_scale_cells: [2.0, 2.0],
                tile_origin_cells: [0.5, 0.0],
            }),
        );
        stone_material.normal_map = Some(MaterialNormalMapDescriptor {
            texture: map.id.clone(),
            scale: 1.0,
        });
        stone_material.triplanar =
            triplanar.then_some(MaterialTriplanarDescriptor { sharpness: 4.0 });
        let materials = BTreeMap::from([(1, stone_material)]);
        let mut projector = VoxelRenderProjector::new();
        let mut ops = vec![
            RenderDiff::DefineTexture { texture: stone },
            RenderDiff::DefineTexture { texture: map },
            sun([0.4, -0.6, -0.7]),
        ];
        ops.extend(project(&mut projector, &block_faces(), &materials));
        harness.apply(ops);
        harness.render(&camera([3.0, 2.5, 3.0], 20.0, -15.0)).1
    };
    let (boxed, triplanar) = (render(false), render(true));
    // An axis-aligned face takes its one plane whole, in its face's texture
    // basis, so texels and the normal map's frame land where the tile
    // coordinates put them.
    let differing = boxed
        .as_chunks::<4>()
        .0
        .iter()
        .zip(triplanar.as_chunks::<4>().0)
        .filter(|(a, b)| a.iter().zip(b.iter()).any(|(a, b)| a.abs_diff(*b) > 2))
        .count();
    assert!(
        differing * 1000 < boxed.len() / 4,
        "{differing} pixels differ from box projection"
    );
}

#[test]
fn a_triplanar_material_draws_a_dual_contoured_mound_without_chart_seams() {
    let mound = VoxelCollisionScene::from_material_voxels_with_mesh_options(
        1.0,
        CHUNK_CELLS,
        (-7..=7).flat_map(|x| {
            (0..=7).flat_map(move |y| {
                (-7..=7).filter_map(move |z| {
                    (x * x + y * y + z * z <= 42).then_some(MaterialVoxel {
                        state: 0,
                        address: [x, y - 1, z - 10],
                        material_slot: 1,
                    })
                })
            })
        }),
        // Smooth placement, shaded smooth: a rounded rock, whose dominant
        // axis turns continuously.
        SurfaceMeshOptions {
            materials: engine_spatial::SurfaceMaterials::new([(
                1,
                engine_spatial::MaterialSurface {
                    mode: engine_spatial::SurfaceMode::DualContouring,
                    character: engine_spatial::SurfaceCharacter {
                        placement: engine_spatial::VertexPlacement::Smooth,
                        crease_angle_degrees: 180.0,
                        roughness: 0.0,
                    },
                },
            )])
            .unwrap(),
            ..SurfaceMeshOptions::default()
        },
    )
    .expect("dual-contoured mound");
    let render = |triplanar: Option<bool>| {
        let mut harness = Harness::new(RendererOptions {
            default_world_lights: false,
            ..RendererOptions::default()
        });
        // Smooth and periodic in both directions, so only a chart switch can
        // make a sharp step in it.
        let wave =
            |at: u32| (128.0 + 100.0 * (at as f32 * std::f32::consts::TAU / 16.0).cos()) as u8;
        let mut rock = harness.resources.texture(
            "texture/rock",
            16,
            16,
            &image(16, 16, |x, y| [wave(x), wave(y), 128, 255]),
            TextureWrap::Repeat,
        );
        let mut rock_material = voxel_material(
            1,
            [1.0; 4],
            Some(&rock),
            Some(VoxelSurfaceMappingDescriptor::Repeat {
                texture: rock.id.clone(),
                texture_version: rock.version,
                texture_content_hash: rock.content_hash.clone().unwrap(),
                tile_scale_cells: [4.0, 4.0],
                tile_origin_cells: [0.0, 0.0],
            }),
        );
        rock.filter = TextureFilter::Linear;
        if let Some(surface) = rock_material.voxel_surface.as_mut() {
            surface.filter = TextureFilter::Linear;
        }
        rock_material.triplanar =
            (triplanar == Some(true)).then_some(MaterialTriplanarDescriptor { sharpness: 4.0 });
        // Ambient π: the surface shows exactly its texture.
        let mut ops = vec![
            RenderDiff::DefineTexture { texture: rock },
            RenderDiff::CreateLight {
                handle: RenderHandle::new(90),
                parent: None,
                light: LightDescriptor::Ambient {
                    color: [1.0; 3],
                    intensity: std::f32::consts::PI,
                    enabled: true,
                    shadow_intent: LightShadowIntent::Disabled,
                },
            },
        ];
        if triplanar.is_some() {
            let mut projector = VoxelRenderProjector::new();
            ops.extend(project(
                &mut projector,
                &mound,
                &BTreeMap::from([(1, rock_material)]),
            ));
        }
        harness.apply(ops);
        harness.render(&camera([0.0, 6.0, 2.0], 0.0, -30.0)).1
    };
    let background = render(None);
    let (boxed, triplanar) = (render(Some(false)), render(Some(true)));
    // Steps between neighbouring pixels sharper than the texture's own
    // gradient draws, across the mound's middle (its sides fold behind
    // themselves, which steps too).
    let seam_steps = |rgba: &[u8]| {
        let at = |x: usize, y: usize| &rgba[(y * WIDTH as usize + x) * 4..][..3];
        let inside = |x: usize, y: usize| {
            (x - 2..=x + 2).all(|x| {
                (y - 2..=y + 2)
                    .all(|y| at(x, y) != &background[(y * WIDTH as usize + x) * 4..][..3])
            })
        };
        let (mut steps, mut pixels) = (0, 0);
        for y in 2..HEIGHT as usize - 3 {
            for x in WIDTH as usize * 3 / 10..WIDTH as usize * 7 / 10 {
                if !inside(x, y) || !inside(x + 1, y) || !inside(x, y + 1) {
                    continue;
                }
                pixels += 1;
                for (nx, ny) in [(x + 1, y), (x, y + 1)] {
                    if (0..3).any(|channel| at(x, y)[channel].abs_diff(at(nx, ny)[channel]) > 60) {
                        steps += 1;
                    }
                }
            }
        }
        assert!(pixels > 10_000, "the mound covers {pixels} pixels");
        steps
    };
    let (box_steps, triplanar_steps) = (seam_steps(&boxed), seam_steps(&triplanar));
    assert!(
        box_steps > 200,
        "box projection's chart seams step {box_steps} times"
    );
    assert_eq!(triplanar_steps, 0, "triplanar seams");
    assert_screenshot("scene_triplanar_dual_contoured_mound", &triplanar);
}

/// Open ground in front of a hill with a tunnel driven into it.
fn cave_mouth() -> VoxelCollisionScene {
    let mut voxels = Vec::new();
    for x in -14..14 {
        for z in -24..6 {
            voxels.push(MaterialVoxel {
                state: 0,
                address: [x, -1, z],
                material_slot: 1,
            });
            let hill = (-6..6).contains(&x) && (-22..-8).contains(&z);
            let tunnel = (-2..2).contains(&x) && z >= -18;
            for y in 0..6 {
                if hill && !(tunnel && y < 3) {
                    voxels.push(MaterialVoxel {
                        state: 0,
                        address: [x, y, z],
                        material_slot: 1,
                    });
                }
            }
        }
    }
    VoxelCollisionScene::from_material_voxels_with_mesh_options(
        1.0,
        CHUNK_CELLS,
        voxels,
        SurfaceMeshOptions::default(),
    )
    .expect("cave mouth")
}

#[test]
fn an_ambient_light_requesting_shadows_leaves_a_cave_darker_than_open_ground() {
    let scene = cave_mouth();
    let render = |shadow_intent| {
        let mut harness = Harness::new(RendererOptions {
            default_world_lights: false,
            shadows: true,
            ..RendererOptions::default()
        });
        let materials = BTreeMap::from([(1, voxel_material(1, [0.6, 0.55, 0.5, 1.0], None, None))]);
        let mut projector = VoxelRenderProjector::new();
        let mut ops = vec![RenderDiff::CreateLight {
            handle: RenderHandle::new(90),
            parent: None,
            light: LightDescriptor::Ambient {
                color: [1.0; 3],
                intensity: std::f32::consts::PI,
                enabled: true,
                shadow_intent,
            },
        }];
        ops.extend(project(&mut projector, &scene, &materials));
        harness.apply(ops);
        harness.render(&camera([0.5, 1.6, 2.0], 0.0, -5.0)).1
    };
    let open = render(LightShadowIntent::Disabled);
    let sky = render(LightShadowIntent::Requested);
    let luminance = |rgba: &[u8], x: u32, y: u32| {
        let at = ((y * WIDTH + x) * 4) as usize;
        rgba[at..at + 3]
            .iter()
            .map(|value| u32::from(*value))
            .sum::<u32>()
    };
    // Unshadowed, the ambient light draws every surface alike: the open
    // ground below the horizon and the tunnel in the middle.
    let (centre, ground) = ((WIDTH / 2, HEIGHT / 2), (WIDTH / 2, HEIGHT - 4));
    assert_eq!(
        luminance(&open, centre.0, centre.1),
        luminance(&open, ground.0, ground.1)
    );
    // Under its sky layer, the open ground keeps its light and the tunnel
    // loses it.
    assert_eq!(
        luminance(&sky, ground.0, ground.1),
        luminance(&open, ground.0, ground.1)
    );
    assert!(
        luminance(&sky, centre.0, centre.1) * 4 < luminance(&open, centre.0, centre.1),
        "tunnel {} against open {}",
        luminance(&sky, centre.0, centre.1),
        luminance(&open, centre.0, centre.1)
    );
    assert_screenshot("scene_ambient_sky_cave_mouth", &sky);
}

/// Sand (slot 1) west of x = 0 and rock (slot 2) east of it on a gentle
/// dual-contoured slope, with a strip of stone (slot 3) outside the layer set
/// along its near edge.
fn sand_and_rock(transition_cells: Option<u8>) -> VoxelCollisionScene {
    sand_and_rock_layers(transition_cells.map(|cells| (vec![1, 2], cells)))
}

/// [`sand_and_rock`] with its terrain layers' slots and transition.
fn sand_and_rock_layers(layers: Option<(Vec<u16>, u8)>) -> VoxelCollisionScene {
    sand_and_rock_scene(
        layers.map(|(slots, cells)| engine_spatial::TerrainLayers::new(slots, cells).unwrap()),
        false,
    )
}

/// [`sand_and_rock`] with every odd column of sand made dirt (slot 4) and of
/// rock gravel (slot 5), mapped onto the sand and rock layers.
fn sand_and_rock_aliased(transition_cells: Option<u8>) -> VoxelCollisionScene {
    sand_and_rock_scene(
        transition_cells.map(|cells| {
            engine_spatial::TerrainLayers::mapped(vec![1, 4, 2, 5], vec![0, 0, 1, 1], cells)
                .unwrap()
        }),
        true,
    )
}

fn sand_and_rock_scene(
    terrain_layers: Option<engine_spatial::TerrainLayers>,
    aliased: bool,
) -> VoxelCollisionScene {
    let voxels = (-12..12).flat_map(|x: i64| {
        let alias = if aliased && x % 2 != 0 { 3 } else { 0 };
        (-16..-2).flat_map(move |z: i64| {
            (-3..(x + 12) / 8 - 1).map(move |y| MaterialVoxel {
                state: 0,
                address: [x, y, z],
                material_slot: if z == -3 {
                    3
                } else if x < 0 {
                    1 + alias
                } else {
                    2 + alias
                },
            })
        })
    });
    VoxelCollisionScene::from_material_voxels_with_mesh_options(
        1.0,
        CHUNK_CELLS,
        voxels,
        SurfaceMeshOptions {
            terrain_layers,
            ..SurfaceMeshOptions::with_mode(engine_spatial::SurfaceMode::DualContouring)
        },
    )
    .expect("sand and rock slope")
}

#[test]
fn terrain_layers_blend_sand_into_rock_over_the_chosen_width() {
    const SAND: [u8; 4] = [230, 200, 80, 255];
    const ROCK: [u8; 4] = [70, 80, 110, 255];
    const STONE: [u8; 4] = [255, 255, 255, 255];
    // `mapped`: triplanar planes and flat normal maps on every layer, which
    // must draw as the plain surface does; `lit`: a raking sun as well, which
    // shows any normal change.
    let render_scene =
        |transition_cells: Option<u8>, contrast: f32, mapped: bool, lit: bool, aliased: bool| {
            let mut harness = Harness::new(RendererOptions {
                default_world_lights: false,
                ..RendererOptions::default()
            });
            let mut plain = |id: &str, color: [u8; 4]| {
                harness
                    .resources
                    .texture(id, 4, 4, &image(4, 4, |_, _| color), TextureWrap::Repeat)
            };
            let (sand, rock, stone) = (
                plain("texture/sand", SAND),
                plain("texture/rock", ROCK),
                plain("texture/stone", STONE),
            );
            let mut flat = plain("texture/flat", [128, 128, 255, 255]);
            if let Some(payload) = flat.payload.as_mut() {
                payload.color_space = TextureColorSpace::Linear;
            }
            let flat_map = mapped.then(|| MaterialNormalMapDescriptor {
                texture: flat.id.clone(),
                scale: 1.0,
            });
            let repeat = |texture: &TextureDescriptor| VoxelSurfaceMappingDescriptor::Repeat {
                texture: texture.id.clone(),
                texture_version: texture.version,
                texture_content_hash: texture.content_hash.clone().unwrap(),
                tile_scale_cells: [2.0, 2.0],
                tile_origin_cells: [0.0, 0.0],
            };
            let surface = |slot: u16, texture: &TextureDescriptor| {
                voxel_material(slot, [1.0; 4], Some(texture), Some(repeat(texture)))
            };
            let mut materials = BTreeMap::from([
                (1, surface(1, &sand)),
                (2, surface(2, &rock)),
                (3, surface(3, &stone)),
                (4, surface(4, &sand)),
                (5, surface(5, &rock)),
            ]);
            if transition_cells.is_some() {
                // Every layer slot draws one blend: sand, then rock.
                let rock_layer = MaterialTerrainLayerDescriptor {
                    voxel_surface: materials[&2].voxel_surface.clone().unwrap(),
                    normal_map: flat_map.clone(),
                };
                for slot in [1, 2, 4, 5] {
                    let mut blend = surface(slot, &sand);
                    blend.terrain_layers = Some(MaterialTerrainLayersDescriptor {
                        layers: vec![rock_layer.clone()],
                        contrast,
                    });
                    blend.normal_map = flat_map.clone();
                    blend.triplanar =
                        mapped.then_some(MaterialTriplanarDescriptor { sharpness: 4.0 });
                    materials.insert(slot, blend);
                }
            }
            let mut ops = vec![
                RenderDiff::DefineTexture { texture: flat },
                RenderDiff::DefineTexture { texture: sand },
                RenderDiff::DefineTexture { texture: rock },
                RenderDiff::DefineTexture { texture: stone },
                // Ambient π: the surface shows exactly its texture.
                RenderDiff::CreateLight {
                    handle: RenderHandle::new(90),
                    parent: None,
                    light: LightDescriptor::Ambient {
                        color: [1.0; 3],
                        intensity: std::f32::consts::PI,
                        enabled: true,
                        shadow_intent: LightShadowIntent::Disabled,
                    },
                },
            ];
            if lit {
                ops.push(RenderDiff::CreateLight {
                    handle: RenderHandle::new(91),
                    parent: None,
                    light: LightDescriptor::Directional {
                        color: [1.0; 3],
                        intensity: 3.0,
                        enabled: true,
                        direction: [-0.8, -0.4, 0.3],
                        range: None,
                        shadow_intent: LightShadowIntent::Disabled,
                    },
                });
            }
            let mut projector = VoxelRenderProjector::new();
            let scene = if aliased {
                sand_and_rock_aliased(transition_cells)
            } else {
                sand_and_rock(transition_cells)
            };
            ops.extend(project(&mut projector, &scene, &materials));
            harness.apply(ops);
            harness.render(&camera([0.0, 9.0, 1.0], 0.0, -55.0)).1
        };
    let render_with = |transition_cells: Option<u8>, contrast: f32, mapped: bool, lit: bool| {
        render_scene(transition_cells, contrast, mapped, lit, false)
    };
    let render = |transition_cells: Option<u8>, contrast: f32| {
        render_with(transition_cells, contrast, false, false)
    };
    let pixel = |rgba: &[u8], x: u32, y: u32| -> [u8; 4] {
        rgba[((y * WIDTH + x) * 4) as usize..][..4]
            .try_into()
            .unwrap()
    };
    // How far each pixel of the middle row is from sand toward rock.
    let row = |rgba: &[u8]| -> Vec<f32> {
        (0..WIDTH)
            .map(|x| {
                let p = pixel(rgba, x, HEIGHT / 2);
                let along = (0..3)
                    .map(|c| {
                        (f32::from(p[c]) - f32::from(SAND[c]))
                            * (f32::from(ROCK[c]) - f32::from(SAND[c]))
                    })
                    .sum::<f32>();
                let length = (0..3)
                    .map(|c| (f32::from(ROCK[c]) - f32::from(SAND[c])).powi(2))
                    .sum::<f32>();
                along / length
            })
            .collect()
    };
    let mixed = |rgba: &[u8]| row(rgba).iter().filter(|t| **t > 0.1 && **t < 0.9).count();
    let hard = render(None, 1.0);
    let narrow = render(Some(1), 1.0);
    let broad = render(Some(3), 1.0);
    let sharpened = render(Some(3), 8.0);
    let (hard_mixed, narrow_mixed, broad_mixed, sharpened_mixed) = (
        mixed(&hard),
        mixed(&narrow),
        mixed(&broad),
        mixed(&sharpened),
    );
    // Both ends of the row stay pure sand and pure rock.
    for rgba in [&hard, &narrow, &broad, &sharpened] {
        let t = row(rgba);
        assert!(
            t[WIDTH as usize / 10].abs() < 0.05,
            "sand end {}",
            t[WIDTH as usize / 10]
        );
        assert!((t[WIDTH as usize * 9 / 10] - 1.0).abs() < 0.05, "rock end");
    }
    assert!(
        hard_mixed <= 2,
        "unlayered materials meet at an edge: {hard_mixed}"
    );
    assert!(narrow_mixed > hard_mixed, "{narrow_mixed}");
    assert!(
        broad_mixed > 2 * narrow_mixed,
        "{broad_mixed} vs {narrow_mixed}"
    );
    assert!(
        sharpened_mixed < broad_mixed,
        "contrast narrows a transition: {sharpened_mixed} vs {broad_mixed}"
    );
    assert_screenshot("scene_terrain_layers_broad", &broad);
    // Dirt and gravel columns mapped onto the sand and rock layers draw the
    // same blend as plain sand and rock: the weights count every slot of a
    // layer as that layer.
    let aliased = render_scene(Some(3), 1.0, false, false, true);
    let differing = broad
        .chunks(4)
        .zip(aliased.chunks(4))
        .filter(|(a, b)| (0..3).any(|c| a[c].abs_diff(b[c]) > 2))
        .count();
    assert_eq!(differing, 0, "aliased slots blend as their layers");
    // The stone strip outside the set is untinted by its layer weights.
    let near = (0..WIDTH)
        .map(|x| pixel(&broad, x, HEIGHT * 3 / 4))
        .filter(|p| p[..3] == STONE[..3])
        .count();
    assert!(near > WIDTH as usize / 2, "stone keeps its colour: {near}");
    // Flat normal maps leave the blend's shading as it was, and triplanar
    // planes of a uniform texture sample the same colours.
    let (plain_lit, mapped) = (
        render_with(Some(3), 1.0, false, true),
        render_with(Some(3), 1.0, true, true),
    );
    assert_ne!(plain_lit, broad, "the sun shades the slope");
    let differing = plain_lit
        .chunks(4)
        .zip(mapped.chunks(4))
        .filter(|(a, b)| (0..3).any(|c| a[c].abs_diff(b[c]) > 6))
        .count();
    assert!(
        differing < (WIDTH * HEIGHT / 200) as usize,
        "{differing} pixels differ with normal maps and triplanar planes"
    );
}

#[test]
fn a_terrain_layer_normal_map_shades_its_layer_as_its_own_material_does() {
    // White surfaces lit by a raking sun; only rock has a tilted normal map.
    let render = |layered: bool| {
        let mut harness = Harness::new(RendererOptions {
            default_world_lights: false,
            ..RendererOptions::default()
        });
        let white = harness.resources.texture(
            "texture/white",
            4,
            4,
            &image(4, 4, |_, _| [255; 4]),
            TextureWrap::Repeat,
        );
        let tilted = tilted_normal_map(&mut harness, "texture/tilted", TextureWrap::Repeat);
        let repeat = VoxelSurfaceMappingDescriptor::Repeat {
            texture: white.id.clone(),
            texture_version: white.version,
            texture_content_hash: white.content_hash.clone().unwrap(),
            tile_scale_cells: [2.0, 2.0],
            tile_origin_cells: [0.0, 0.0],
        };
        let surface =
            |slot: u16| voxel_material(slot, [1.0; 4], Some(&white), Some(repeat.clone()));
        let tilt = MaterialNormalMapDescriptor {
            texture: tilted.id.clone(),
            scale: 1.0,
        };
        let mut rock = surface(2);
        rock.normal_map = Some(tilt.clone());
        let mut materials = BTreeMap::from([(1, surface(1)), (2, rock), (3, surface(3))]);
        if layered {
            for slot in [1, 2] {
                let mut blend = surface(slot);
                blend.terrain_layers = Some(MaterialTerrainLayersDescriptor {
                    layers: vec![MaterialTerrainLayerDescriptor {
                        voxel_surface: blend.voxel_surface.clone().unwrap(),
                        normal_map: Some(tilt.clone()),
                    }],
                    contrast: 8.0,
                });
                materials.insert(slot, blend);
            }
        }
        let mut ops = vec![
            RenderDiff::DefineTexture { texture: white },
            RenderDiff::DefineTexture { texture: tilted },
            sun([-0.8, -0.4, 0.3]),
        ];
        let mut projector = VoxelRenderProjector::new();
        ops.extend(project(
            &mut projector,
            &sand_and_rock(layered.then_some(1)),
            &materials,
        ));
        harness.apply(ops);
        harness.render(&camera([0.0, 9.0, 1.0], 0.0, -55.0)).1
    };
    let (separate, layered) = (render(false), render(true));
    let luminance = |rgba: &[u8], x: u32| {
        let at = ((HEIGHT / 2 * WIDTH + x) * 4) as usize;
        u32::from(rgba[at]) + u32::from(rgba[at + 1]) + u32::from(rgba[at + 2])
    };
    let (sand_x, rock_x) = (WIDTH / 10, WIDTH * 9 / 10);
    assert!(
        luminance(&separate, sand_x).abs_diff(luminance(&separate, rock_x)) > 30,
        "the tilted map shades rock differently from flat sand"
    );
    for x in [sand_x, rock_x] {
        assert!(
            luminance(&separate, x).abs_diff(luminance(&layered, x)) <= 9,
            "layered shading at {x}: {} vs {}",
            luminance(&layered, x),
            luminance(&separate, x)
        );
    }
}

#[test]
fn a_high_contrast_keeps_a_blend_between_non_base_layers() {
    // Layer 0 is a red slot no voxel uses; sand and rock are layers 1 and 2.
    const SAND: [u8; 4] = [230, 200, 80, 255];
    const ROCK: [u8; 4] = [70, 80, 110, 255];
    const RED: [u8; 4] = [255, 0, 0, 255];
    let render = |contrast: f32| {
        let mut harness = Harness::new(RendererOptions {
            default_world_lights: false,
            ..RendererOptions::default()
        });
        let mut plain = |id: &str, color: [u8; 4]| {
            harness
                .resources
                .texture(id, 4, 4, &image(4, 4, |_, _| color), TextureWrap::Repeat)
        };
        let (red, sand, rock) = (
            plain("texture/red", RED),
            plain("texture/sand", SAND),
            plain("texture/rock", ROCK),
        );
        let repeat = |texture: &TextureDescriptor| VoxelSurfaceMappingDescriptor::Repeat {
            texture: texture.id.clone(),
            texture_version: texture.version,
            texture_content_hash: texture.content_hash.clone().unwrap(),
            tile_scale_cells: [2.0, 2.0],
            tile_origin_cells: [0.0, 0.0],
        };
        let layer = |texture: &TextureDescriptor| MaterialTerrainLayerDescriptor {
            voxel_surface: voxel_material(0, [1.0; 4], Some(texture), Some(repeat(texture)))
                .voxel_surface
                .unwrap(),
            normal_map: None,
        };
        let mut materials = BTreeMap::new();
        for slot in [1, 2, 3] {
            let mut blend = voxel_material(slot, [1.0; 4], Some(&red), Some(repeat(&red)));
            blend.terrain_layers = Some(MaterialTerrainLayersDescriptor {
                layers: vec![layer(&sand), layer(&rock)],
                contrast,
            });
            materials.insert(slot, blend);
        }
        let mut ops = vec![
            RenderDiff::DefineTexture { texture: red },
            RenderDiff::DefineTexture { texture: sand },
            RenderDiff::DefineTexture { texture: rock },
            RenderDiff::CreateLight {
                handle: RenderHandle::new(90),
                parent: None,
                light: LightDescriptor::Ambient {
                    color: [1.0; 3],
                    intensity: std::f32::consts::PI,
                    enabled: true,
                    shadow_intent: LightShadowIntent::Disabled,
                },
            },
        ];
        let mut projector = VoxelRenderProjector::new();
        ops.extend(project(
            &mut projector,
            &sand_and_rock_layers(Some((vec![9, 1, 2], 3))),
            &materials,
        ));
        harness.apply(ops);
        harness.render(&camera([0.0, 9.0, 1.0], 0.0, -55.0)).1
    };
    let row = |rgba: &[u8]| -> Vec<[u8; 4]> {
        (0..WIDTH)
            .map(|x| {
                rgba[((HEIGHT / 2 * WIDTH + x) * 4) as usize..][..4]
                    .try_into()
                    .unwrap()
            })
            .collect()
    };
    let reddish = |pixels: &[[u8; 4]]| {
        pixels
            .iter()
            .filter(|p| p[0] > 200 && p[1] < 100 && p[2] < 100)
            .count()
    };
    let mixed = |pixels: &[[u8; 4]]| {
        pixels
            .iter()
            .filter(|p| {
                let along = (0..3)
                    .map(|c| {
                        (f32::from(p[c]) - f32::from(SAND[c]))
                            * (f32::from(ROCK[c]) - f32::from(SAND[c]))
                    })
                    .sum::<f32>()
                    / (0..3)
                        .map(|c| (f32::from(ROCK[c]) - f32::from(SAND[c])).powi(2))
                        .sum::<f32>();
                along > 0.1 && along < 0.9
            })
            .count()
    };
    let (gentle, sharp) = (row(&render(1.0)), row(&render(32.0)));
    assert_eq!(reddish(&gentle), 0, "an absent layer shows at contrast 1");
    assert_eq!(reddish(&sharp), 0, "an absent layer shows at contrast 32");
    assert!(mixed(&gentle) > 0);
    assert!(
        mixed(&sharp) < mixed(&gentle),
        "contrast narrows the sand-rock transition: {} vs {}",
        mixed(&sharp),
        mixed(&gentle)
    );
}

/// Quads in the z = 0 plane facing +Z, one per material slot, each `width`
/// wide and side by side from x = `-width × count / 2`; uv 0..1 across each.
fn facing_quads(count: u16, width: f32, height: f32) -> MeshPayloadDescriptor {
    let (mut positions, mut normals, mut uvs, mut indices) = (vec![], vec![], vec![], vec![]);
    let left = -width * f32::from(count) / 2.0;
    for quad in 0..count {
        let x = left + width * f32::from(quad);
        let base = (positions.len() / 3) as u32;
        positions.extend_from_slice(&[
            x,
            -height / 2.0,
            0.0,
            x + width,
            -height / 2.0,
            0.0,
            x + width,
            height / 2.0,
            0.0,
            x,
            height / 2.0,
            0.0,
        ]);
        normals.extend_from_slice(&[0.0, 0.0, 1.0].repeat(4));
        uvs.extend_from_slice(&[0.0, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0]);
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    let groups: Vec<(u16, u32)> = (0..count).map(|slot| (slot, 6)).collect();
    payload(positions, normals, uvs, indices, &groups)
}

/// A static mesh asset binding slot `n` to `materials[n]`.
fn multi_material_mesh(
    asset: &str,
    payload: MeshPayloadDescriptor,
    materials: &[&str],
) -> RenderDiff {
    RenderDiff::DefineStaticMesh {
        asset: StaticMeshAsset {
            asset: asset.to_owned(),
            payload,
            material_slots: materials
                .iter()
                .enumerate()
                .map(|(slot, material)| MeshMaterialSlot {
                    slot: slot as u16,
                    material: (*material).to_owned(),
                })
                .collect(),
            collision: MeshCollisionPolicy::VisualOnly,
        },
    }
}

/// Bright/dark changes along the middle row between pixel columns `from`
/// and `to`.
fn row_transitions(rgba: &[u8], from: usize, to: usize) -> usize {
    let y = HEIGHT as usize / 2;
    let bright: Vec<bool> = (from..to)
        .map(|x| rgba[(y * WIDTH as usize + x) * 4 + 1] > 100)
        .collect();
    bright.windows(2).filter(|pair| pair[0] != pair[1]).count()
}

#[test]
fn texture_transforms_set_each_materials_repeat_on_one_mesh_at_unit_scale() {
    let mut harness = Harness::new(RendererOptions::default());
    let checker = harness.resources.texture(
        "texture/checker",
        2,
        2,
        &checker(2, [240, 240, 240, 255], [10, 10, 10, 255]),
        TextureWrap::Repeat,
    );
    let textured = |id: &str, scale: f32, triplanar: bool| {
        let mut descriptor = material(id, [1.0; 4], Some("texture/checker"));
        descriptor.texture_transform = Some(MaterialTextureTransformDescriptor {
            scale: [scale, scale],
            offset: [0.25, 0.0],
        });
        descriptor.triplanar = triplanar.then_some(MaterialTriplanarDescriptor { sharpness: 4.0 });
        RenderDiff::DefineMaterial {
            material: descriptor,
        }
    };
    // Two triplanar materials, 2 and 0.5 repeats per metre, and a uv-mapped
    // one repeating 3 times across its quad, on 2 m quads of one mesh.
    harness.apply(vec![
        RenderDiff::DefineTexture { texture: checker },
        textured("material/fine", 2.0, true),
        textured("material/coarse", 0.5, true),
        textured("material/uv", 3.0, false),
        multi_material_mesh(
            "mesh/wall",
            facing_quads(3, 2.0, 2.0),
            &["material/fine", "material/coarse", "material/uv"],
        ),
        instance(1, None, "mesh/wall", Transform::IDENTITY),
    ]);
    let (_, rgba) = harness.render(&camera([0.0, 0.0, 4.0], 0.0, 0.0));
    // 39 pixels per metre at 4 m: each quad spans about 78 columns.
    let quad = |index: usize| {
        let left = WIDTH as usize / 2 - 117 + 78 * index;
        row_transitions(&rgba, left + 4, left + 74)
    };
    // A checker repeat has two cells: 2 m at 2 repeats per metre is 8
    // cells, at 0.5 is 2, and 3 uv repeats are 6. The quarter-repeat offset
    // moves every cell edge half a cell off the quad's edges, so each quad
    // shows as many edges as cells.
    assert_eq!(
        [quad(0), quad(1), quad(2)],
        [8, 2, 6],
        "cell edges inside each quad"
    );
    assert_screenshot("scene_texture_transforms", &rgba);
}

/// A smooth periodic height field over one texture repeat (u, v in 0..1),
/// and its slopes along u and v.
fn bumps(u: f32, v: f32) -> (f32, f32, f32) {
    use std::f32::consts::TAU;
    let waves = [(2.0, 1.0, 0.3), (1.0, -3.0, 1.7), (3.0, 2.0, 4.1)];
    let mut height = 0.5;
    let (mut along_u, mut along_v) = (0.0, 0.0);
    for (fu, fv, phase) in waves {
        let angle = TAU * (fu * u + fv * v) + phase;
        height += 0.15 * angle.sin();
        along_u += 0.15 * TAU * fu * angle.cos();
        along_v += 0.15 * TAU * fv * angle.cos();
    }
    (height, along_u, along_v)
}

/// Pearson correlation of two equal-length series.
fn correlation(a: &[f32], b: &[f32]) -> f32 {
    let mean = |s: &[f32]| s.iter().sum::<f32>() / s.len() as f32;
    let (ma, mb) = (mean(a), mean(b));
    let (mut ab, mut aa, mut bb) = (0.0, 0.0, 0.0);
    for (x, y) in a.iter().zip(b) {
        ab += (x - ma) * (y - mb);
        aa += (x - ma) * (x - ma);
        bb += (y - mb) * (y - mb);
    }
    ab / (aa * bb).sqrt()
}

#[test]
fn stochastic_tiling_hides_a_repeat_without_seams_and_keeps_its_normal_map_with_its_colour() {
    const SIZE: u32 = 64;
    // 2 m per repeat at about 39 pixels per metre.
    const PERIOD_PIXELS: usize = 78;
    let render = |triplanar: bool, stochastic: bool, normal_map: bool| {
        let mut harness = Harness::new(RendererOptions {
            default_world_lights: false,
            ..RendererOptions::default()
        });
        let texel = |x: u32, y: u32| {
            bumps(
                (x as f32 + 0.5) / SIZE as f32,
                (y as f32 + 0.5) / SIZE as f32,
            )
        };
        let mut base = harness.resources.texture(
            "texture/bumps",
            SIZE,
            SIZE,
            &image(SIZE, SIZE, |x, y| {
                let grey = (texel(x, y).0 * 255.0) as u8;
                [grey, grey, grey, 255]
            }),
            TextureWrap::Repeat,
        );
        base.filter = TextureFilter::Linear;
        // Tangent space: x along +u, y up the image (-v); the slopes are per
        // repeat, so a tenth of them keeps the tilt moderate.
        let mut normals = harness.resources.texture(
            "texture/bumps-normal",
            SIZE,
            SIZE,
            &image(SIZE, SIZE, |x, y| {
                let (_, along_u, along_v) = texel(x, y);
                let tilt = [-along_u * 0.1, along_v * 0.1, 1.0];
                let length = tilt.iter().map(|c| c * c).sum::<f32>().sqrt();
                let encode = |c: f32| ((c / length * 0.5 + 0.5) * 255.0).round() as u8;
                [encode(tilt[0]), encode(tilt[1]), encode(tilt[2]), 255]
            }),
            TextureWrap::Repeat,
        );
        normals.filter = TextureFilter::Linear;
        if let Some(payload) = normals.payload.as_mut() {
            payload.color_space = TextureColorSpace::Linear;
        }
        let mut descriptor = material("material/bumps", [1.0; 4], Some("texture/bumps"));
        descriptor.roughness = 1.0;
        descriptor.normal_map = normal_map.then(|| MaterialNormalMapDescriptor {
            texture: normals.id.clone(),
            scale: 1.0,
        });
        // A repeat every 2 m of the 12 × 8 m wall: through its uv, or the
        // triplanar planes' metres.
        descriptor.texture_transform = Some(MaterialTextureTransformDescriptor {
            scale: if triplanar { [0.5, 0.5] } else { [6.0, 4.0] },
            offset: [0.0, 0.0],
        });
        descriptor.triplanar = triplanar.then_some(MaterialTriplanarDescriptor { sharpness: 4.0 });
        descriptor.stochastic_tiling =
            stochastic.then_some(MaterialStochasticTilingDescriptor { contrast: 4.0 });
        harness.apply(vec![
            RenderDiff::DefineTexture { texture: base },
            RenderDiff::DefineTexture { texture: normals },
            RenderDiff::DefineMaterial {
                material: descriptor,
            },
            // A wall wider and taller than the view.
            static_mesh("mesh/wall", facing_quads(1, 12.0, 8.0), "material/bumps"),
            instance(1, None, "mesh/wall", Transform::IDENTITY),
            // From +X, so tilts toward +X brighten.
            sun([-1.0, 0.0, -1.0]),
        ]);
        harness.render(&camera([0.0, 0.0, 4.0], 0.0, 0.0)).1
    };
    let green = |rgba: &[u8], x: usize, y: usize| f32::from(rgba[(y * WIDTH as usize + x) * 4 + 1]);
    let rows = 20..HEIGHT as usize - 20;
    // Repetition: each pixel against the one a repeat to its right.
    let repetition = |rgba: &[u8]| {
        let (mut here, mut there) = (vec![], vec![]);
        for y in rows.clone() {
            for x in 4..WIDTH as usize - PERIOD_PIXELS - 4 {
                here.push(green(rgba, x, y));
                there.push(green(rgba, x + PERIOD_PIXELS, y));
            }
        }
        correlation(&here, &there)
    };
    // Seams: the largest step between neighbouring pixels.
    let largest_step = |rgba: &[u8]| {
        let mut largest = 0.0f32;
        for y in rows.clone() {
            for x in 4..WIDTH as usize - 5 {
                largest = largest
                    .max((green(rgba, x + 1, y) - green(rgba, x, y)).abs())
                    .max((green(rgba, x, y + 1) - green(rgba, x, y)).abs());
            }
        }
        largest
    };
    // Normal map against colour: the shading the map adds should follow the
    // height's slope toward the light, which the colour's own x step shows.
    let consistency = |flat: &[u8], mapped: &[u8]| {
        let (mut slope, mut shading) = (vec![], vec![]);
        for y in rows.clone() {
            for x in 4..WIDTH as usize - 5 {
                slope.push(green(flat, x + 1, y) - green(flat, x, y));
                shading.push(green(mapped, x, y) / green(flat, x, y).max(1.0));
            }
        }
        correlation(&slope, &shading)
    };
    for triplanar in [false, true] {
        let (plain, plain_mapped) = (
            render(triplanar, false, false),
            render(triplanar, false, true),
        );
        let (tiled, tiled_mapped) = (
            render(triplanar, true, false),
            render(triplanar, true, true),
        );
        let path = if triplanar { "triplanar" } else { "uv" };
        let (plain_repeat, tiled_repeat) = (repetition(&plain), repetition(&tiled));
        assert!(
            plain_repeat > 0.99 && tiled_repeat.abs() < 0.3,
            "{path}: correlation a repeat apart {plain_repeat} plain, {tiled_repeat} tiled"
        );
        // Blending patches steepens the texture a little; a seam would jump
        // by the difference between patches, several times more.
        let (plain_step, tiled_step) = (largest_step(&plain), largest_step(&tiled));
        assert!(
            tiled_step < 3.0 * plain_step,
            "{path}: largest step {tiled_step} tiled, {plain_step} plain"
        );
        // Each patch's normals turn with its colour: without that turn the
        // shading would not follow the colour at all (about 0).
        let (plain_follows, tiled_follows) = (
            consistency(&plain, &plain_mapped),
            consistency(&tiled, &tiled_mapped),
        );
        assert!(
            plain_follows < -0.9 && tiled_follows < -0.6,
            "{path}: shading against slope {plain_follows} plain, {tiled_follows} tiled"
        );
    }
    let tiled_mapped = render(false, true, true);
    assert_screenshot("scene_stochastic_tiling", &tiled_mapped);
}

/// A sun far from its light node over a long field of posts, each wider,
/// taller and deeper with distance so its shadow covers a few pixels.
fn sunlit_field(harness: &mut Harness, origin: [f32; 3], posts: &[f32]) {
    let at = |x: f32, y: f32, z: f32| [origin[0] + x, origin[1] + y, origin[2] + z];
    let mut ops = vec![
        RenderDiff::DefineMaterial {
            material: material("material/ground", [0.8, 0.8, 0.78, 1.0], None),
        },
        static_mesh(
            "mesh/ground",
            box_mesh(at(-80.0, -0.1, -260.0), at(80.0, 0.0, 10.0), |_| 0),
            "material/ground",
        ),
        RenderDiff::CreateLight {
            handle: RenderHandle::new(100),
            parent: None,
            light: LightDescriptor::Directional {
                color: [1.0; 3],
                intensity: 3.0,
                enabled: true,
                direction: [-1.0, -1.0, 0.0],
                range: None,
                shadow_intent: LightShadowIntent::Requested,
            },
        },
        RenderDiff::CreateLight {
            handle: RenderHandle::new(101),
            parent: None,
            light: LightDescriptor::Ambient {
                color: [1.0; 3],
                intensity: 0.3,
                enabled: true,
                shadow_intent: LightShadowIntent::Disabled,
            },
        },
        instance(1, None, "mesh/ground", Transform::IDENTITY),
    ];
    for (index, &distance) in posts.iter().enumerate() {
        let (width, height, depth) = post_size(distance);
        let asset = format!("mesh/post-{index}");
        ops.push(static_mesh(
            &asset,
            box_mesh(
                at(-width / 2.0, 0.0, -distance - depth / 2.0),
                at(width / 2.0, height, -distance + depth / 2.0),
                |_| 0,
            ),
            "material/ground",
        ));
        ops.push(instance(
            10 + index as u64,
            None,
            &asset,
            Transform::IDENTITY,
        ));
    }
    harness.apply(ops);
}

fn post_size(distance: f32) -> (f32, f32, f32) {
    (1.0 + distance / 30.0, 3.0 + distance / 10.0, distance / 3.0)
}

#[test]
fn a_sun_shadows_the_view_from_near_to_its_range_wherever_its_node_is() {
    let origin = [300.0, 0.0, -200.0];
    let posts = [15.0, 35.0, 70.0, 170.0];
    let options = |shadows| RendererOptions {
        default_world_lights: false,
        shadows,
        ..RendererOptions::default()
    };
    let (mut shadowed, mut plain) = (Harness::new(options(true)), Harness::new(options(false)));
    sunlit_field(&mut shadowed, origin, &posts);
    sunlit_field(&mut plain, origin, &posts);
    let eye = glam::Vec3::new(origin[0], 12.0, origin[2]);
    let pitch = -15f32;
    let mut view = camera(
        [eye.x as f64, eye.y as f64, eye.z as f64],
        0.0,
        f64::from(pitch),
    );
    view.projection = render_host_contracts::RendererCameraProjection::Perspective {
        fov_y_degrees: 60.0,
        near: 0.1,
        far: 500.0,
    };
    let (stats, lit) = shadowed.render(&view);
    assert_eq!(stats.shadow_layers, 4, "{stats:?}");
    assert_screenshot("sun-cascades", &lit);
    let unshadowed = plain.render(&view).1;

    // The ground beside each post, where the sun (travelling -X and down)
    // throws its shadow.
    let forward = glam::Vec3::new(0.0, pitch.to_radians().sin(), -pitch.to_radians().cos());
    let view_proj =
        glam::Mat4::perspective_rh(60f32.to_radians(), WIDTH as f32 / HEIGHT as f32, 0.1, 500.0)
            * glam::Mat4::look_to_rh(eye, forward, glam::Vec3::Y);
    let brightness = |pixels: &[u8], point: glam::Vec3| {
        let ndc = view_proj.project_point3(point);
        let x = ((ndc.x * 0.5 + 0.5) * WIDTH as f32) as usize;
        let y = ((0.5 - ndc.y * 0.5) * HEIGHT as f32) as usize;
        let index = (y * WIDTH as usize + x) * 4;
        pixels[index..index + 3]
            .iter()
            .map(|&c| u32::from(c))
            .sum::<u32>()
    };
    for &distance in &posts {
        let (width, height, _) = post_size(distance);
        let shade = glam::Vec3::new(
            origin[0] - width / 2.0 - height / 2.0,
            0.0,
            origin[2] - distance,
        );
        let (with, without) = (brightness(&lit, shade), brightness(&unshadowed, shade));
        if distance < 100.0 {
            assert!(
                with + 60 < without,
                "{distance} m: {with} shadowed, {without} plain"
            );
        } else {
            assert_eq!(with, without, "{distance} m is past the sun's 100 m range");
        }
    }
}
