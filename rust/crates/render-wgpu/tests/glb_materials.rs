//! GLB material maps (#9090): the emissive texture masks emission, the
//! normal map bends shading toward its normals under a moving light, the
//! occlusion map darkens indirect light, and `KHR_texture_transform` tiles
//! and shifts each slot. Each fixture is a camera-facing plane whose maps are
//! two texels wide (left half, right half), admitted as the runtime admits it.
//! Instance parameters (#9367) replace a slot's base colour and emission per
//! instance while its textures still apply.

mod support;

use asset_import::{import_animated_glb_asset, ImportContext, SourceUri};
use render_model::*;
use render_wgpu::{encode_png, RendererOptions};
use serde_json::{json, Value};
use support::*;

const PLANE: u64 = 1;
const LIGHT: u64 = 2;
/// Screen columns over the plane's left and right halves, and its quarters,
/// on the fixture row.
const LEFT: usize = 128;
const RIGHT: usize = 192;
const QUARTERS: [usize; 4] = [113, 144, 176, 207];
const ROW: usize = 90;

/// A 2×1 PNG: the left texel, then the right one.
fn pair(left: [u8; 4], right: [u8; 4]) -> Vec<u8> {
    encode_png(2, 1, &[left, right].concat()).expect("encode fixture map")
}

/// The plane's standard `TEXCOORD_0`: u along +X, v down the image.
const UPRIGHT_UVS: [f32; 8] = [0.0, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0];
/// u mirrored: u along -X, v still down the image.
const MIRRORED_UVS: [f32; 8] = [1.0, 1.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0];

/// Optional plane streams beyond the defaults.
#[derive(Default)]
struct Streams {
    uvs: Option<[f32; 8]>,
    uvs1: Option<[f32; 8]>,
    /// One TANGENT for all four vertices.
    tangent: Option<[f32; 4]>,
}

/// A GLB with one plane facing +Z (x and y in -1..1, uv's v down the image
/// as glTF has it), `material`, and `images` embedded as textures 0, 1, …
/// sampled nearest with repeat.
fn plane_glb(material: Value, images: &[Vec<u8>]) -> Vec<u8> {
    plane_glb_with(material, images, Streams::default())
}

fn plane_glb_with(material: Value, images: &[Vec<u8>], streams: Streams) -> Vec<u8> {
    let floats: Vec<f32> = [
        // positions
        [
            -1.0, -1.0, 0.0, 1.0, -1.0, 0.0, 1.0, 1.0, 0.0, -1.0, 1.0, 0.0,
        ]
        .as_slice(),
        // normals
        &[0.0, 0.0, 1.0].repeat(4),
        // uvs
        &streams.uvs.unwrap_or(UPRIGHT_UVS),
    ]
    .concat();
    let mut bin: Vec<u8> = bytemuck_floats(&floats);
    for index in [0_u16, 1, 2, 0, 2, 3] {
        bin.extend_from_slice(&index.to_le_bytes());
    }
    let mut views = vec![
        json!({"buffer": 0, "byteOffset": 0, "byteLength": 48}),
        json!({"buffer": 0, "byteOffset": 48, "byteLength": 48}),
        json!({"buffer": 0, "byteOffset": 96, "byteLength": 32}),
        json!({"buffer": 0, "byteOffset": 128, "byteLength": 12}),
    ];
    let mut accessors = vec![
        json!({"bufferView": 0, "componentType": 5126, "count": 4, "type": "VEC3",
               "min": [-1.0, -1.0, 0.0], "max": [1.0, 1.0, 0.0]}),
        json!({"bufferView": 1, "componentType": 5126, "count": 4, "type": "VEC3"}),
        json!({"bufferView": 2, "componentType": 5126, "count": 4, "type": "VEC2"}),
        json!({"bufferView": 3, "componentType": 5123, "count": 6, "type": "SCALAR"}),
    ];
    let mut attributes = json!({"POSITION": 0, "NORMAL": 1, "TEXCOORD_0": 2});
    let mut stream = |floats: &[f32], kind: &str, name: &str| {
        bin.resize(bin.len().next_multiple_of(4), 0);
        views.push(json!({"buffer": 0, "byteOffset": bin.len(), "byteLength": floats.len() * 4}));
        bin.extend(bytemuck_floats(floats));
        accessors.push(
            json!({"bufferView": views.len() - 1, "componentType": 5126, "count": 4, "type": kind}),
        );
        attributes[name] = json!(accessors.len() - 1);
    };
    if let Some(uvs1) = streams.uvs1 {
        stream(&uvs1, "VEC2", "TEXCOORD_1");
    }
    if let Some(tangent) = streams.tangent {
        stream(&tangent.repeat(4), "VEC4", "TANGENT");
    }
    let mut image_values = Vec::new();
    for image in images {
        bin.resize(bin.len().next_multiple_of(4), 0);
        views.push(json!({"buffer": 0, "byteOffset": bin.len(), "byteLength": image.len()}));
        image_values.push(json!({"bufferView": views.len() - 1, "mimeType": "image/png"}));
        bin.extend_from_slice(image);
    }
    bin.resize(bin.len().next_multiple_of(4), 0);
    let mut extensions = vec!["KHR_texture_transform"];
    if material
        .pointer("/extensions/KHR_materials_unlit")
        .is_some()
    {
        extensions.push("KHR_materials_unlit");
    }
    let document = json!({
        "asset": {"version": "2.0"},
        "extensionsUsed": extensions,
        "scene": 0,
        "scenes": [{"nodes": [0]}],
        "nodes": [{"mesh": 0}],
        "meshes": [{"primitives": [{
            "attributes": attributes,
            "indices": 3,
            "material": 0,
        }]}],
        "accessors": accessors,
        "bufferViews": views,
        "buffers": [{"byteLength": bin.len()}],
        "images": image_values,
        "samplers": [{"magFilter": 9728, "minFilter": 9728, "wrapS": 10497, "wrapT": 10497}],
        "textures": (0..images.len()).map(|source| json!({"source": source, "sampler": 0})).collect::<Vec<_>>(),
        "materials": [material],
    });
    let mut json = serde_json::to_vec(&document).unwrap();
    json.resize(json.len().next_multiple_of(4), b' ');
    let mut glb = Vec::new();
    for word in [0x4654_6c67_u32, 2, (28 + json.len() + bin.len()) as u32] {
        glb.extend_from_slice(&word.to_le_bytes());
    }
    glb.extend_from_slice(&(json.len() as u32).to_le_bytes());
    glb.extend_from_slice(b"JSON");
    glb.extend(json);
    glb.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    glb.extend_from_slice(b"BIN\0");
    glb.extend(bin);
    glb
}

fn bytemuck_floats(values: &[f32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

/// Renders the plane, admitted from `glb`, with `lights`, from straight in
/// front. Returns the frame's RGBA pixels.
fn render(glb: Vec<u8>, lights: Vec<RenderDiff>) -> Vec<u8> {
    let mut harness = Harness::new(RendererOptions {
        default_world_lights: false,
        ..RendererOptions::default()
    });
    let imported = import_animated_glb_asset(
        &SourceUri::RelativePath("plane.glb".to_owned()),
        &glb,
        &ImportContext::default(),
    );
    let imported = imported
        .assets
        .unwrap_or_else(|| panic!("the fixture plane is admitted: {:?}", imported.diagnostics));
    let asset = imported.animated_mesh;
    let hash = asset.content_hash.clone().expect("content hash");
    harness.resources.insert(
        &format!(
            "animated-mesh-resource/{}",
            hash.trim_start_matches("sha256:")
        ),
        imported.runtime_resource_bytes,
    );
    let mut ops = vec![
        RenderDiff::DefineAnimatedMesh {
            asset: asset.clone(),
        },
        RenderDiff::CreateAnimatedMeshInstance {
            handle: RenderHandle::new(PLANE),
            parent: None,
            instance: AnimatedMeshInstanceDescriptor {
                inspection: AnimatedMeshInspection::default(),
                asset: asset.asset.clone(),
                transform: Transform::IDENTITY,
                visible: true,
                material_overrides: Vec::new(),
                playback: None,
                metadata: RenderMetadata::default(),
                layer: RenderLayer::Scene,
            },
        },
    ];
    ops.extend(lights);
    harness.apply(ops);
    harness.render(&camera([0.0, 0.0, 2.5], 0.0, 0.0)).1
}

fn pixel(frame: &[u8], x: usize) -> [u8; 3] {
    let at = (ROW * WIDTH as usize + x) * 4;
    [frame[at], frame[at + 1], frame[at + 2]]
}

fn luminance(frame: &[u8], x: usize) -> u32 {
    pixel(frame, x)
        .iter()
        .map(|channel| u32::from(*channel))
        .sum()
}

fn directional(direction: [f32; 3]) -> RenderDiff {
    RenderDiff::CreateLight {
        handle: RenderHandle::new(LIGHT),
        parent: None,
        light: LightDescriptor::Directional {
            color: [1.0; 3],
            intensity: 3.0,
            enabled: true,
            direction,
            range: None,
            shadow_intent: LightShadowIntent::Disabled,
        },
    }
}

const WHITE: [u8; 4] = [255; 4];
const BLACK: [u8; 4] = [0, 0, 0, 255];

#[test]
fn the_emissive_texture_masks_emission_through_its_transform() {
    let emissive = |transform: Value| {
        let mut texture = json!({"index": 0});
        if !transform.is_null() {
            texture["extensions"] = json!({"KHR_texture_transform": transform});
        }
        plane_glb(
            json!({
                "pbrMetallicRoughness": {"baseColorFactor": [0.0, 0.0, 0.0, 1.0]},
                "emissiveFactor": [1.0, 0.0, 0.0],
                "emissiveTexture": texture,
            }),
            &[pair(WHITE, BLACK)],
        )
    };
    // The white texel lights its half; the black one leaves it dark.
    let frame = render(emissive(Value::Null), Vec::new());
    assert!(pixel(&frame, LEFT)[0] > 240, "{:?}", pixel(&frame, LEFT));
    assert!(pixel(&frame, RIGHT)[0] < 10, "{:?}", pixel(&frame, RIGHT));
    // Shifted half a texture, the halves swap.
    let frame = render(emissive(json!({"offset": [0.5, 0.0]})), Vec::new());
    assert!(pixel(&frame, LEFT)[0] < 10, "{:?}", pixel(&frame, LEFT));
    assert!(pixel(&frame, RIGHT)[0] > 240, "{:?}", pixel(&frame, RIGHT));
    // Without a mask the factor lights the whole surface.
    let unmasked = plane_glb(
        json!({
            "pbrMetallicRoughness": {"baseColorFactor": [0.0, 0.0, 0.0, 1.0]},
            "emissiveFactor": [1.0, 0.0, 0.0],
        }),
        &[],
    );
    let frame = render(unmasked, Vec::new());
    assert!(pixel(&frame, LEFT)[0] > 240 && pixel(&frame, RIGHT)[0] > 240);
}

/// Tangent-space normals as a normal map stores them.
fn encoded(normal: [f32; 3]) -> [u8; 4] {
    let [x, y, z] = normal.map(|value| ((value * 0.5 + 0.5) * 255.0).round() as u8);
    [x, y, z, 255]
}

#[test]
fn the_normal_map_turns_each_half_toward_its_normal_as_the_light_moves() {
    let mapped = |left: [f32; 3], right: [f32; 3], scale: f32| {
        plane_glb(
            json!({
                "pbrMetallicRoughness": {"baseColorFactor": [1.0, 1.0, 1.0, 1.0],
                                         "metallicFactor": 0.0, "roughnessFactor": 1.0},
                "normalTexture": {"index": 0, "scale": scale},
            }),
            &[pair(encoded(left), encoded(right))],
        )
    };
    // Left tilted toward +X (the texture's +u), right toward -X.
    let sideways = || mapped([0.6, 0.0, 0.8], [-0.6, 0.0, 0.8], 1.0);
    let from_right = render(sideways(), vec![directional([-1.0, 0.0, -1.0])]);
    let from_left = render(sideways(), vec![directional([1.0, 0.0, -1.0])]);
    assert!(
        luminance(&from_right, LEFT) > luminance(&from_right, RIGHT) + 60,
        "light from +X: {:?} {:?}",
        pixel(&from_right, LEFT),
        pixel(&from_right, RIGHT)
    );
    assert!(
        luminance(&from_left, RIGHT) > luminance(&from_left, LEFT) + 60,
        "light from -X: {:?} {:?}",
        pixel(&from_left, LEFT),
        pixel(&from_left, RIGHT)
    );
    // Tangent-space +Y is up the image, here world +Y.
    let up = mapped([0.0, 0.6, 0.8], [0.0, 0.6, 0.8], 1.0);
    let from_above = render(up.clone(), vec![directional([0.0, -1.0, -1.0])]);
    let from_below = render(up, vec![directional([0.0, 1.0, -1.0])]);
    assert!(luminance(&from_above, LEFT) > luminance(&from_below, LEFT) + 60);
    // Scale 0 flattens the map: both halves shade as the plane.
    let flat = render(
        mapped([0.6, 0.0, 0.8], [-0.6, 0.0, 0.8], 0.0),
        vec![directional([-1.0, 0.0, -1.0])],
    );
    assert!(luminance(&flat, LEFT).abs_diff(luminance(&flat, RIGHT)) < 6);
}

#[test]
fn the_occlusion_map_darkens_ambient_light_by_its_strength() {
    let occluded = |strength: f32| {
        plane_glb(
            json!({
                "pbrMetallicRoughness": {"baseColorFactor": [1.0, 1.0, 1.0, 1.0],
                                         "metallicFactor": 0.0},
                "occlusionTexture": {"index": 0, "strength": strength},
            }),
            &[pair(BLACK, WHITE)],
        )
    };
    let ambient = || {
        vec![RenderDiff::CreateLight {
            handle: RenderHandle::new(LIGHT),
            parent: None,
            light: LightDescriptor::Ambient {
                color: [1.0; 3],
                intensity: 2.0,
                enabled: true,
                shadow_intent: LightShadowIntent::Disabled,
            },
        }]
    };
    let full = render(occluded(1.0), ambient());
    assert!(luminance(&full, LEFT) < 10, "{:?}", pixel(&full, LEFT));
    assert!(luminance(&full, RIGHT) > 300, "{:?}", pixel(&full, RIGHT));
    let half = render(occluded(0.5), ambient());
    assert!(luminance(&half, LEFT) > luminance(&full, LEFT) + 100);
    assert!(luminance(&half, LEFT) + 100 < luminance(&half, RIGHT));
    // Direct light ignores the occlusion map.
    let direct = render(occluded(1.0), vec![directional([0.0, 0.0, -1.0])]);
    assert!(luminance(&direct, LEFT).abs_diff(luminance(&direct, RIGHT)) < 6);
}

#[test]
fn the_base_colour_tiles_and_shifts_through_its_texture_transform() {
    const RED: [u8; 4] = [255, 0, 0, 255];
    const GREEN: [u8; 4] = [0, 255, 0, 255];
    let tiled = |transform: Value| {
        plane_glb(
            json!({
                "pbrMetallicRoughness": {"baseColorTexture": {
                    "index": 0,
                    "extensions": {"KHR_texture_transform": transform},
                }},
                "extensions": {"KHR_materials_unlit": {}},
            }),
            &[pair(RED, GREEN)],
        )
    };
    let colour = |frame: &[u8], x: usize| {
        let [r, g, _] = pixel(frame, x);
        if r > 200 && g < 50 {
            'r'
        } else if g > 200 && r < 50 {
            'g'
        } else {
            '?'
        }
    };
    let quarters = |frame: &[u8]| QUARTERS.map(|x| colour(frame, x));
    assert_eq!(
        quarters(&render(tiled(json!({})), Vec::new())),
        ['r', 'r', 'g', 'g']
    );
    assert_eq!(
        quarters(&render(tiled(json!({"scale": [2.0, 1.0]})), Vec::new())),
        ['r', 'g', 'r', 'g']
    );
    assert_eq!(
        quarters(&render(
            tiled(json!({"scale": [2.0, 1.0], "offset": [0.5, 0.0]})),
            Vec::new()
        )),
        ['g', 'r', 'g', 'r']
    );
}

/// A normal map tilted up the image (+Y), lit from above or below.
fn up_tilted(streams: Streams) -> (u32, u32) {
    let glb = || {
        plane_glb_with(
            json!({
                "pbrMetallicRoughness": {"baseColorFactor": [1.0, 1.0, 1.0, 1.0],
                                         "metallicFactor": 0.0, "roughnessFactor": 1.0},
                "normalTexture": {"index": 0},
            }),
            &[pair(encoded([0.0, 0.6, 0.8]), encoded([0.0, 0.6, 0.8]))],
            Streams { ..streams },
        )
    };
    let from_above = render(glb(), vec![directional([0.0, -1.0, -1.0])]);
    let from_below = render(glb(), vec![directional([0.0, 1.0, -1.0])]);
    (luminance(&from_above, LEFT), luminance(&from_below, LEFT))
}

#[test]
fn authored_tangents_and_their_handedness_orient_the_normal_map() {
    // u mirrored: the authored tangent follows -X with handedness -1, so the
    // bitangent cross(N, T)·w is still +Y, up the image.
    let (above, below) = up_tilted(Streams {
        uvs: Some(MIRRORED_UVS),
        tangent: Some([-1.0, 0.0, 0.0, -1.0]),
        ..Streams::default()
    });
    assert!(
        above > below + 60,
        "mirrored handedness: above {above}, below {below}"
    );
    // The same mirrored layout without TANGENT: generated tangents carry the
    // flipped handedness too.
    let (above, below) = up_tilted(Streams {
        uvs: Some(MIRRORED_UVS),
        ..Streams::default()
    });
    assert!(
        above > below + 60,
        "generated handedness: above {above}, below {below}"
    );
    // An authored tangent is used as given, not regenerated: claiming +u
    // runs along +Y turns the texture's +X tilt toward world +Y.
    let sideways = plane_glb_with(
        json!({
            "pbrMetallicRoughness": {"baseColorFactor": [1.0, 1.0, 1.0, 1.0],
                                     "metallicFactor": 0.0, "roughnessFactor": 1.0},
            "normalTexture": {"index": 0},
        }),
        &[pair(encoded([0.6, 0.0, 0.8]), encoded([0.6, 0.0, 0.8]))],
        Streams {
            tangent: Some([0.0, 1.0, 0.0, 1.0]),
            ..Streams::default()
        },
    );
    let glb = sideways.clone();
    let from_above = render(glb, vec![directional([0.0, -1.0, -1.0])]);
    let from_below = render(sideways, vec![directional([0.0, 1.0, -1.0])]);
    assert!(luminance(&from_above, LEFT) > luminance(&from_below, LEFT) + 60);
}

#[test]
fn each_slot_reads_the_uv_set_its_tex_coord_names() {
    const RED: [u8; 4] = [255, 0, 0, 255];
    const GREEN: [u8; 4] = [0, 255, 0, 255];
    let unlit = |texture: Value| {
        plane_glb_with(
            json!({
                "pbrMetallicRoughness": {"baseColorTexture": texture},
                "extensions": {"KHR_materials_unlit": {}},
            }),
            &[pair(RED, GREEN)],
            // TEXCOORD_1 mirrors u, so red and green swap sides.
            Streams {
                uvs1: Some(MIRRORED_UVS),
                ..Streams::default()
            },
        )
    };
    let sides = |glb: Vec<u8>| {
        let frame = render(glb, Vec::new());
        (pixel(&frame, LEFT), pixel(&frame, RIGHT))
    };
    let red_green = ([255, 0, 0], [0, 255, 0]);
    let green_red = ([0, 255, 0], [255, 0, 0]);
    assert_eq!(sides(unlit(json!({"index": 0}))), red_green);
    assert_eq!(sides(unlit(json!({"index": 0, "texCoord": 1}))), green_red);
    // KHR_texture_transform's texCoord overrides the reference's.
    assert_eq!(
        sides(unlit(json!({
            "index": 0,
            "texCoord": 0,
            "extensions": {"KHR_texture_transform": {"texCoord": 1}},
        }))),
        green_red
    );
}

/// Two instances of one admitted plane GLB, side by side, each with its own
/// instance parameters for the plane's material slot 0. Returns the frame.
fn render_pair(
    glb: Vec<u8>,
    left: Option<MaterialInstanceParameters>,
    right: Option<MaterialInstanceParameters>,
) -> Vec<u8> {
    let mut harness = Harness::new(RendererOptions {
        default_world_lights: false,
        ..RendererOptions::default()
    });
    let imported = import_animated_glb_asset(
        &SourceUri::RelativePath("plane.glb".to_owned()),
        &glb,
        &ImportContext::default(),
    );
    let imported = imported
        .assets
        .unwrap_or_else(|| panic!("the fixture plane is admitted: {:?}", imported.diagnostics));
    let asset = imported.animated_mesh;
    assert_eq!(asset.embedded_material_slots[0].slot, 0);
    let hash = asset.content_hash.clone().expect("content hash");
    harness.resources.insert(
        &format!(
            "animated-mesh-resource/{}",
            hash.trim_start_matches("sha256:")
        ),
        imported.runtime_resource_bytes,
    );
    let mut ops = vec![RenderDiff::DefineAnimatedMesh {
        asset: asset.clone(),
    }];
    for (handle, x, parameters) in [(PLANE, -1.1, left), (PLANE + 10, 1.1, right)] {
        ops.push(RenderDiff::CreateAnimatedMeshInstance {
            handle: RenderHandle::new(handle),
            parent: None,
            instance: AnimatedMeshInstanceDescriptor {
                inspection: AnimatedMeshInspection::default(),
                asset: asset.asset.clone(),
                transform: Transform {
                    translation: [x, 0.0, 0.0],
                    ..Transform::IDENTITY
                },
                visible: true,
                material_overrides: Vec::new(),
                playback: None,
                metadata: RenderMetadata::default(),
                layer: RenderLayer::Scene,
            },
        });
        ops.push(RenderDiff::SetMaterialInstanceParameters {
            handle: RenderHandle::new(handle),
            slot: 0,
            parameters,
        });
    }
    harness.apply(ops);
    harness.render(&camera([0.0, 0.0, 5.0], 0.0, 0.0)).1
}

/// `render_pair` columns over each plane's white (left) and black (right)
/// texel.
const PAIR_LEFT_WHITE: usize = 110;
const PAIR_LEFT_BLACK: usize = 141;
const PAIR_RIGHT_WHITE: usize = 178;
const PAIR_RIGHT_BLACK: usize = 209;

fn factors(
    base_color: Option<[f32; 4]>,
    emission: Option<([f32; 3], f32)>,
) -> MaterialInstanceParameters {
    MaterialInstanceParameters {
        base_color,
        texture_tint: [1.0; 4],
        emission: emission.map(|(color, intensity)| MaterialInstanceEmission { color, intensity }),
    }
}

#[test]
fn instances_of_one_glb_replace_its_base_colour_factor_and_keep_its_texture() {
    // Unlit, so the frame shows base colour times texture exactly.
    let glb = plane_glb(
        json!({
            "pbrMetallicRoughness": {"baseColorFactor": [0.5, 0.5, 0.5, 1.0], "baseColorTexture": {"index": 0}},
            "extensions": {"KHR_materials_unlit": {}},
        }),
        &[pair(WHITE, BLACK)],
    );
    let own = render_pair(glb.clone(), None, None);
    let grey = pixel(&own, PAIR_LEFT_WHITE);
    assert!(
        grey.iter().all(|channel| (100..=200).contains(channel)),
        "{grey:?}"
    );

    let frame = render_pair(
        glb,
        Some(factors(Some([1.0, 0.0, 0.0, 1.0]), None)),
        Some(factors(Some([0.0, 1.0, 0.0, 1.0]), None)),
    );
    // Each instance draws its own colour where the texture is white...
    assert_eq!(pixel(&frame, PAIR_LEFT_WHITE), [255, 0, 0]);
    assert_eq!(pixel(&frame, PAIR_RIGHT_WHITE), [0, 255, 0]);
    // ...and the texture's black texel still darkens it.
    assert_eq!(pixel(&frame, PAIR_LEFT_BLACK), [0, 0, 0]);
    assert_eq!(pixel(&frame, PAIR_RIGHT_BLACK), [0, 0, 0]);
}

#[test]
fn instances_of_one_glb_replace_its_emission_and_keep_its_emissive_texture() {
    let glb = plane_glb(
        json!({
            "pbrMetallicRoughness": {"baseColorFactor": [0.0, 0.0, 0.0, 1.0]},
            "emissiveFactor": [1.0, 0.0, 0.0],
            "emissiveTexture": {"index": 0},
        }),
        &[pair(WHITE, BLACK)],
    );
    // The left instance turns blue at full strength; the right keeps the
    // GLB's red.
    let frame = render_pair(glb, Some(factors(None, Some(([0.0, 0.0, 1.0], 1.0)))), None);
    let blue = pixel(&frame, PAIR_LEFT_WHITE);
    assert!(blue[2] > 240 && blue[0] < 10, "{blue:?}");
    let red = pixel(&frame, PAIR_RIGHT_WHITE);
    assert!(red[0] > 240 && red[2] < 10, "{red:?}");
    // The emissive texture's black texel still masks both.
    for x in [PAIR_LEFT_BLACK, PAIR_RIGHT_BLACK] {
        assert!(luminance(&frame, x) < 30, "{:?}", pixel(&frame, x));
    }
}
