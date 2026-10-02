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
                shadow_intent: LightShadowIntent::Requested,
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
    // Directional and spot take one layer each, the point light six.
    assert_eq!(harness.renderer.table_counts().shadow_layers, 8);
    assert!(first.shadow_draws >= 8, "casters drawn into every layer");
    assert_screenshot("shadows", &pixels);

    let (again, _) = harness.render(&camera([0.5, 4.5, 6.5], 4.0, -32.0));
    assert_eq!(again.shadow_draws, 0, "camera motion alone reuses the maps");

    // A new object casts at once, with no per-object setup.
    harness.apply(vec![instance(
        4,
        None,
        "mesh/crate",
        transform([0.0, 0.0, 2.0], 0.0, [1.0; 3]),
    )]);
    let (added, _) = harness.render(&view);
    assert!(added.shadow_draws >= 8);
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
