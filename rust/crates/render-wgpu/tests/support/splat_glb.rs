//! Splat GLBs written for the splat fixtures (`docs/splat-characters.md`):
//! one node drawing one `POINTS` primitive of Gaussians with `POSITION`,
//! `_ROTATION`, `_SCALE` and `_COLOR` (and `_ALBEDO` and `_GROUP` when any
//! Gaussian has one), as an asset writer exports them.

use serde_json::json;

#[derive(Clone, Copy, Debug)]
pub struct Gaussian {
    pub centre: [f32; 3],
    /// xyzw.
    pub rotation: [f32; 4],
    /// Standard deviation along each of its axes, in metres.
    pub scale: [f32; 3],
    /// sRGB-encoded display colour, then linear opacity.
    pub color: [u8; 4],
    /// `_ALBEDO`, sRGB-encoded (alpha unread), written when any Gaussian
    /// of a GLB has one (its colour standing in for those without).
    pub albedo: Option<[u8; 4]>,
    /// `_GROUP`, written when any Gaussian of a GLB is in a group.
    pub group: u8,
}

impl Gaussian {
    /// A round Gaussian of standard deviation `sigma`.
    pub fn round(centre: [f32; 3], sigma: f32, color: [u8; 4]) -> Self {
        Self {
            centre,
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [sigma; 3],
            color,
            albedo: None,
            group: 0,
        }
    }
}

/// A GLB whose one node, at `translation`, draws `gaussians`.
pub fn splat_glb(gaussians: &[Gaussian], translation: [f32; 3]) -> Vec<u8> {
    let count = gaussians.len();
    let mut bin = Vec::new();
    let mut views = Vec::new();
    let mut push_view = |bin: &mut Vec<u8>, bytes: Vec<u8>| {
        let offset = bin.len();
        bin.extend_from_slice(&bytes);
        while !bin.len().is_multiple_of(4) {
            bin.push(0);
        }
        views.push(json!({"buffer": 0, "byteOffset": offset, "byteLength": bytes.len()}));
        views.len() - 1
    };
    let floats = |values: &mut dyn Iterator<Item = f32>| -> Vec<u8> {
        values.flat_map(f32::to_le_bytes).collect()
    };
    let positions = push_view(
        &mut bin,
        floats(&mut gaussians.iter().flat_map(|g| g.centre)),
    );
    let rotations = push_view(
        &mut bin,
        floats(&mut gaussians.iter().flat_map(|g| g.rotation)),
    );
    let scales = push_view(
        &mut bin,
        floats(&mut gaussians.iter().flat_map(|g| g.scale)),
    );
    let colors = push_view(&mut bin, gaussians.iter().flat_map(|g| g.color).collect());
    let albedos = gaussians.iter().any(|g| g.albedo.is_some()).then(|| {
        push_view(
            &mut bin,
            gaussians
                .iter()
                .flat_map(|g| g.albedo.unwrap_or(g.color))
                .collect(),
        )
    });
    let groups = gaussians
        .iter()
        .any(|g| g.group != 0)
        .then(|| push_view(&mut bin, gaussians.iter().map(|g| g.group).collect()));
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];
    for gaussian in gaussians {
        for axis in 0..3 {
            min[axis] = min[axis].min(gaussian.centre[axis]);
            max[axis] = max[axis].max(gaussian.centre[axis]);
        }
    }
    let mut attributes = json!({"POSITION": 0, "_ROTATION": 1, "_SCALE": 2, "_COLOR": 3});
    let mut next = 4;
    if albedos.is_some() {
        attributes["_ALBEDO"] = json!(next);
        next += 1;
    }
    if groups.is_some() {
        attributes["_GROUP"] = json!(next);
    }
    let mut document = json!({
        "asset": {"version": "2.0", "generator": "render-wgpu splat fixtures"},
        "scene": 0,
        "scenes": [{"nodes": [0]}],
        "nodes": [{"name": "splats", "mesh": 0, "translation": translation}],
        "meshes": [{"name": "splats", "primitives": [{
            "mode": 0,
            "attributes": attributes
        }]}],
        "accessors": [
            {"bufferView": positions, "componentType": 5126, "count": count, "type": "VEC3",
             "min": min, "max": max},
            {"bufferView": rotations, "componentType": 5126, "count": count, "type": "VEC4"},
            {"bufferView": scales, "componentType": 5126, "count": count, "type": "VEC3"},
            {"bufferView": colors, "componentType": 5121, "normalized": true, "count": count,
             "type": "VEC4"},
        ],
        "bufferViews": views,
        "buffers": [{"byteLength": bin.len()}],
    });
    if let Some(view) = albedos {
        document["accessors"]
            .as_array_mut()
            .expect("accessors")
            .push(json!(
                {"bufferView": view, "componentType": 5121, "normalized": true, "count": count,
                 "type": "VEC4"}
            ));
    }
    if let Some(view) = groups {
        document["accessors"]
            .as_array_mut()
            .expect("accessors")
            .push(json!(
                {"bufferView": view, "componentType": 5121, "count": count, "type": "SCALAR"}
            ));
    }
    glb(&document, &bin)
}

/// A binary glTF of `document` and its BIN chunk.
pub fn glb(document: &serde_json::Value, bin: &[u8]) -> Vec<u8> {
    let mut json = serde_json::to_vec(document).expect("the document serializes");
    while !json.len().is_multiple_of(4) {
        json.push(b' ');
    }
    let total = 12 + 8 + json.len() + 8 + bin.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(json.len() as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(&json);
    out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    out.extend_from_slice(b"BIN\0");
    out.extend_from_slice(bin);
    out
}

/// Two joints for the skinned fixtures: `root` at the origin and `arm` at
/// `arm_length` above it.
fn joints_json(arm_length: f32) -> serde_json::Value {
    json!([
        {"name": "character", "children": [1]},
        {"name": "root", "children": [2]},
        {"name": "arm", "translation": [0.0, arm_length, 0.0]},
    ])
}

/// A skinned splat GLB on [`joints_json`] with an arm of length 1: each
/// Gaussian moves with the one joint `joints[i]` names (0 root, 1 arm), its
/// centre in bind space.
pub fn skinned_splat_glb(gaussians: &[Gaussian], joints: &[u8]) -> Vec<u8> {
    #[rustfmt::skip]
    let inverse_binds = [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, -1.0, 0.0, 1.0,
    ];
    skinned_splat_glb_on(joints_json(1.0), &inverse_binds, gaussians, joints)
}

/// A skinned splat GLB on `skeleton` (node 0 a non-joint holding the
/// joints, nodes 1 and 2 the skin's two joints) with the joints' inverse
/// binds, column-major: each Gaussian moves with the joint `joints[i]` names.
pub fn skinned_splat_glb_on(
    skeleton: serde_json::Value,
    inverse_binds: &[f32; 32],
    gaussians: &[Gaussian],
    joints: &[u8],
) -> Vec<u8> {
    assert_eq!(gaussians.len(), joints.len());
    let count = gaussians.len();
    let mut bin = Vec::new();
    let mut views = Vec::new();
    let mut push_view = |bin: &mut Vec<u8>, bytes: Vec<u8>| {
        let offset = bin.len();
        bin.extend_from_slice(&bytes);
        while !bin.len().is_multiple_of(4) {
            bin.push(0);
        }
        views.push(json!({"buffer": 0, "byteOffset": offset, "byteLength": bytes.len()}));
        views.len() - 1
    };
    let floats = |values: &mut dyn Iterator<Item = f32>| -> Vec<u8> {
        values.flat_map(f32::to_le_bytes).collect()
    };
    let positions = push_view(
        &mut bin,
        floats(&mut gaussians.iter().flat_map(|g| g.centre)),
    );
    let rotations = push_view(
        &mut bin,
        floats(&mut gaussians.iter().flat_map(|g| g.rotation)),
    );
    let scales = push_view(
        &mut bin,
        floats(&mut gaussians.iter().flat_map(|g| g.scale)),
    );
    let colors = push_view(&mut bin, gaussians.iter().flat_map(|g| g.color).collect());
    let joint_bytes = push_view(
        &mut bin,
        joints.iter().flat_map(|j| [*j, 0, 0, 0]).collect(),
    );
    let weights = push_view(
        &mut bin,
        joints.iter().flat_map(|_| [255u8, 0, 0, 0]).collect(),
    );
    let binds = push_view(&mut bin, floats(&mut inverse_binds.iter().copied()));
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];
    for gaussian in gaussians {
        for axis in 0..3 {
            min[axis] = min[axis].min(gaussian.centre[axis]);
            max[axis] = max[axis].max(gaussian.centre[axis]);
        }
    }
    let mut nodes = skeleton;
    nodes
        .as_array_mut()
        .expect("nodes")
        .push(json!({"name": "body", "mesh": 0, "skin": 0}));
    let document = json!({
        "asset": {"version": "2.0", "generator": "render-wgpu splat fixtures"},
        "scene": 0,
        "scenes": [{"nodes": [0, 3]}],
        "nodes": nodes,
        "skins": [{"joints": [1, 2], "inverseBindMatrices": 6}],
        "meshes": [{"name": "body", "primitives": [{
            "mode": 0,
            "attributes": {"POSITION": 0, "_ROTATION": 1, "_SCALE": 2, "_COLOR": 3,
                           "JOINTS_0": 4, "WEIGHTS_0": 5}
        }]}],
        "accessors": [
            {"bufferView": positions, "componentType": 5126, "count": count, "type": "VEC3",
             "min": min, "max": max},
            {"bufferView": rotations, "componentType": 5126, "count": count, "type": "VEC4"},
            {"bufferView": scales, "componentType": 5126, "count": count, "type": "VEC3"},
            {"bufferView": colors, "componentType": 5121, "normalized": true, "count": count,
             "type": "VEC4"},
            {"bufferView": joint_bytes, "componentType": 5121, "count": count, "type": "VEC4"},
            {"bufferView": weights, "componentType": 5121, "normalized": true, "count": count,
             "type": "VEC4"},
            {"bufferView": binds, "componentType": 5126, "count": 2, "type": "MAT4"},
        ],
        "bufferViews": views,
        "buffers": [{"byteLength": bin.len()}],
    });
    glb(&document, &bin)
}

/// A clip pack on [`joints_json`] with an arm of length 2: clip `raise`
/// holds the arm turned 90° about +z and at its translation (the pack's own
/// bone length) and lifts the root from y = 0 to y = 1 over one second.
pub fn arm_clip_pack_glb() -> Vec<u8> {
    let floats =
        |values: &[f32]| -> Vec<u8> { values.iter().flat_map(|v| v.to_le_bytes()).collect() };
    let half = std::f32::consts::FRAC_1_SQRT_2;
    let mut bin = Vec::new();
    let mut views = Vec::new();
    for bytes in [
        floats(&[0.0, 1.0]),
        floats(&[0.0, 0.0, half, half, 0.0, 0.0, half, half]),
        floats(&[0.0, 2.0, 0.0, 0.0, 2.0, 0.0]),
        floats(&[0.0, 0.0, 0.0, 0.0, 1.0, 0.0]),
        floats(&[
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, //
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, -2.0, 0.0, 1.0,
        ]),
    ] {
        views.push(json!({"buffer": 0, "byteOffset": bin.len(), "byteLength": bytes.len()}));
        bin.extend_from_slice(&bytes);
    }
    let document = json!({
        "asset": {"version": "2.0", "generator": "render-wgpu splat fixtures"},
        "scene": 0,
        "scenes": [{"nodes": [0]}],
        "nodes": joints_json(2.0),
        "skins": [{"joints": [1, 2], "inverseBindMatrices": 4}],
        "accessors": [
            {"bufferView": 0, "componentType": 5126, "count": 2, "type": "SCALAR",
             "min": [0.0], "max": [1.0]},
            {"bufferView": 1, "componentType": 5126, "count": 2, "type": "VEC4"},
            {"bufferView": 2, "componentType": 5126, "count": 2, "type": "VEC3"},
            {"bufferView": 3, "componentType": 5126, "count": 2, "type": "VEC3"},
            {"bufferView": 4, "componentType": 5126, "count": 2, "type": "MAT4"},
        ],
        "animations": [{"name": "raise", "samplers": [
            {"input": 0, "output": 1, "interpolation": "LINEAR"},
            {"input": 0, "output": 2, "interpolation": "LINEAR"},
            {"input": 0, "output": 3, "interpolation": "LINEAR"},
        ], "channels": [
            {"sampler": 0, "target": {"node": 2, "path": "rotation"}},
            {"sampler": 1, "target": {"node": 2, "path": "translation"}},
            {"sampler": 2, "target": {"node": 1, "path": "translation"}},
        ]}],
        "bufferViews": views,
        "buffers": [{"byteLength": bin.len()}],
    });
    glb(&document, &bin)
}

/// The Hips rig: a non-joint `character` holding `Hips` one metre up and
/// `Arm` half a metre above it, with no `root` joint (Mixamo's shape).
fn hips_json(hips: serde_json::Value, arm_length: f32) -> serde_json::Value {
    json!([
        {"name": "character", "children": [1]},
        hips,
        {"name": "Arm", "translation": [0.0, arm_length, 0.0]},
    ])
}

/// A skinned splat GLB on the Hips rig: a green Gaussian on `Hips` at
/// y = 1 and a red one on `Arm` at y = 1.5, in bind space.
pub fn hips_splat_glb() -> Vec<u8> {
    #[rustfmt::skip]
    let inverse_binds = [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, -1.0, 0.0, 1.0,
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, -1.5, 0.0, 1.0,
    ];
    skinned_splat_glb_on(
        hips_json(
            json!({"name": "Hips", "translation": [0.0, 1.0, 0.0], "children": [2]}),
            0.5,
        ),
        &inverse_binds,
        &[
            Gaussian::round([0.0, 1.0, 0.0], 0.1, [0, 255, 0, 255]),
            Gaussian::round([0.0, 1.5, 0.0], 0.1, [255, 0, 0, 255]),
        ],
        &[0, 1],
    )
}

/// A clip pack on the Hips rig with a two-metre arm, its skeleton wrapped in
/// a non-joint `Armature.001` turned +90° about x (a Z-up export): its
/// joints' values are in that frame. Clip `turn`, in the Y-up world: Hips
/// turned 90° about +z and lifted from y = 1 to y = 1.5 over one second.
pub fn wrapped_hips_pack_glb() -> Vec<u8> {
    let floats =
        |values: &[f32]| -> Vec<u8> { values.iter().flat_map(|v| v.to_le_bytes()).collect() };
    let h = std::f32::consts::FRAC_1_SQRT_2;
    // The wrapper's inverse, R_x(-90°), carries Y-up values into its frame.
    let unwrap = [-h, 0.0, 0.0, h];
    // R_x(-90°) * R_z(90°), xyzw.
    let turned = {
        let [ax, ay, az, aw] = unwrap;
        let [bx, by, bz, bw] = [0.0, 0.0, h, h];
        [
            aw * bx + ax * bw + ay * bz - az * by,
            aw * by - ax * bz + ay * bw + az * bx,
            aw * bz + ax * by - ay * bx + az * bw,
            aw * bw - ax * bx - ay * by - az * bz,
        ]
    };
    let mut bin = Vec::new();
    let mut views = Vec::new();
    for bytes in [
        floats(&[0.0, 1.0]),
        floats(&[
            turned[0], turned[1], turned[2], turned[3], turned[0], turned[1], turned[2], turned[3],
        ]),
        // R_x(-90°) * (0, y, 0) = (0, 0, -y).
        floats(&[0.0, 0.0, -1.0, 0.0, 0.0, -1.5]),
        floats(&[0.0, 2.0, 0.0, 0.0, 2.0, 0.0]),
        floats(&[
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, //
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ]),
    ] {
        views.push(json!({"buffer": 0, "byteOffset": bin.len(), "byteLength": bytes.len()}));
        bin.extend_from_slice(&bytes);
    }
    let mut nodes = hips_json(
        json!({"name": "Hips", "translation": [0.0, 0.0, -1.0], "rotation": unwrap, "children": [2]}),
        2.0,
    );
    nodes[0] = json!({"name": "Armature.001", "rotation": [h, 0.0, 0.0, h], "children": [1]});
    let document = json!({
        "asset": {"version": "2.0", "generator": "render-wgpu splat fixtures"},
        "scene": 0,
        "scenes": [{"nodes": [0]}],
        "nodes": nodes,
        "skins": [{"joints": [1, 2], "inverseBindMatrices": 4}],
        "accessors": [
            {"bufferView": 0, "componentType": 5126, "count": 2, "type": "SCALAR",
             "min": [0.0], "max": [1.0]},
            {"bufferView": 1, "componentType": 5126, "count": 2, "type": "VEC4"},
            {"bufferView": 2, "componentType": 5126, "count": 2, "type": "VEC3"},
            {"bufferView": 3, "componentType": 5126, "count": 2, "type": "VEC3"},
            {"bufferView": 4, "componentType": 5126, "count": 2, "type": "MAT4"},
        ],
        "animations": [{"name": "turn", "samplers": [
            {"input": 0, "output": 1, "interpolation": "LINEAR"},
            {"input": 0, "output": 2, "interpolation": "LINEAR"},
            {"input": 0, "output": 3, "interpolation": "LINEAR"},
        ], "channels": [
            {"sampler": 0, "target": {"node": 1, "path": "rotation"}},
            {"sampler": 1, "target": {"node": 1, "path": "translation"}},
            {"sampler": 2, "target": {"node": 2, "path": "translation"}},
        ]}],
        "bufferViews": views,
        "buffers": [{"byteLength": bin.len()}],
    });
    glb(&document, &bin)
}

/// A card: a textured rectangle carrying one tile of its GLB's atlas.
#[derive(Clone, Copy, Debug)]
pub struct Card {
    pub centre: [f32; 3],
    /// xyzw: local x along the card's first half-axis, y its second.
    pub rotation: [f32; 4],
    /// Half-extents along its local x and y, in metres.
    pub half: [f32; 2],
    /// Its tile, in tiles from the atlas's top left.
    pub tile: [u16; 2],
    /// `_GROUP`, written when any card of a GLB is in a group.
    pub group: u8,
}

impl Card {
    /// A card facing +z of half-extent `half` carrying `tile`.
    pub fn facing(centre: [f32; 3], half: f32, tile: [u16; 2]) -> Self {
        Self {
            centre,
            rotation: [0.0, 0.0, 0.0, 1.0],
            half: [half; 2],
            tile,
            group: 0,
        }
    }
}

/// A card GLB whose one node, at `translation`, draws `cards` with tiles
/// of `tile_pixels` from an atlas of `width` × `height` RGBA texels,
/// embedded as a PNG (`extras.splatKind = "billboard"`).
pub fn cards_glb(
    cards: &[Card],
    tile_pixels: u32,
    (width, height, atlas): (u32, u32, &[u8]),
    translation: [f32; 3],
) -> Vec<u8> {
    let count = cards.len();
    let png = render_wgpu::encode_png(width, height, atlas).expect("the atlas encodes");
    let mut bin = Vec::new();
    let mut views = Vec::new();
    let mut push_view = |bin: &mut Vec<u8>, bytes: Vec<u8>| {
        let offset = bin.len();
        bin.extend_from_slice(&bytes);
        while !bin.len().is_multiple_of(4) {
            bin.push(0);
        }
        views.push(json!({"buffer": 0, "byteOffset": offset, "byteLength": bytes.len()}));
        views.len() - 1
    };
    let floats = |values: &mut dyn Iterator<Item = f32>| -> Vec<u8> {
        values.flat_map(f32::to_le_bytes).collect()
    };
    let positions = push_view(&mut bin, floats(&mut cards.iter().flat_map(|c| c.centre)));
    let rotations = push_view(&mut bin, floats(&mut cards.iter().flat_map(|c| c.rotation)));
    let halves = push_view(&mut bin, floats(&mut cards.iter().flat_map(|c| c.half)));
    let tiles = push_view(
        &mut bin,
        cards
            .iter()
            .flat_map(|c| c.tile)
            .flat_map(u16::to_le_bytes)
            .collect(),
    );
    let image = push_view(&mut bin, png);
    let groups = cards
        .iter()
        .any(|c| c.group != 0)
        .then(|| push_view(&mut bin, cards.iter().map(|c| c.group).collect()));
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];
    for card in cards {
        for axis in 0..3 {
            min[axis] = min[axis].min(card.centre[axis]);
            max[axis] = max[axis].max(card.centre[axis]);
        }
    }
    let mut attributes = json!({"POSITION": 0, "_ROTATION": 1, "_SCALE": 2, "_TILE_ORIGIN": 3});
    if groups.is_some() {
        attributes["_GROUP"] = json!(4);
    }
    let mut document = json!({
        "asset": {"version": "2.0", "generator": "render-wgpu card fixtures"},
        "scene": 0,
        "scenes": [{"nodes": [0]}],
        "nodes": [{"name": "cards", "mesh": 0, "translation": translation}],
        "meshes": [{"name": "cards", "primitives": [{
            "mode": 0,
            "attributes": attributes,
            "extras": {"splatKind": "billboard", "bbsplat": {
                "tilePx": tile_pixels, "atlasW": width, "atlasH": height, "atlasTexture": 0}},
        }]}],
        "accessors": [
            {"bufferView": positions, "componentType": 5126, "count": count, "type": "VEC3",
             "min": min, "max": max},
            {"bufferView": rotations, "componentType": 5126, "count": count, "type": "VEC4"},
            {"bufferView": halves, "componentType": 5126, "count": count, "type": "VEC2"},
            {"bufferView": tiles, "componentType": 5123, "count": count, "type": "VEC2"},
        ],
        "bufferViews": views,
        "buffers": [{"byteLength": bin.len()}],
        "images": [{"bufferView": image, "mimeType": "image/png"}],
        "textures": [{"source": 0}],
    });
    if let Some(view) = groups {
        document["accessors"]
            .as_array_mut()
            .expect("accessors")
            .push(json!(
                {"bufferView": view, "componentType": 5121, "count": count, "type": "SCALAR"}
            ));
    }
    glb(&document, &bin)
}

/// A splat figure's matte inset core for the fixtures: a square of side
/// `2·half` facing +z at depth `z` in the figure's space.
#[derive(Clone, Copy, Debug)]
pub struct Core {
    pub half: f32,
    pub z: f32,
    /// `COLOR_0`, linear.
    pub color: [f32; 4],
    /// `_ALBEDO`, sRGB-encoded, when the core has one.
    pub albedo: Option<[u8; 4]>,
}

/// [`splat_glb`]'s figure with `core` as a second primitive of its mesh
/// (`extras.splatCore`).
pub fn splat_glb_with_core(gaussians: &[Gaussian], core: Core) -> Vec<u8> {
    let bytes = splat_glb(gaussians, [0.0; 3]);
    let json_length = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    let mut document: serde_json::Value =
        serde_json::from_slice(&bytes[20..20 + json_length]).expect("the fixture's JSON");
    let mut bin = bytes[28 + json_length..].to_vec();
    let mut push_view = |bin: &mut Vec<u8>, bytes: Vec<u8>| {
        let offset = bin.len();
        bin.extend_from_slice(&bytes);
        while !bin.len().is_multiple_of(4) {
            bin.push(0);
        }
        let views = document["bufferViews"].as_array_mut().expect("views");
        views.push(json!({"buffer": 0, "byteOffset": offset, "byteLength": bytes.len()}));
        views.len() - 1
    };
    let (h, z) = (core.half, core.z);
    let corners = [[-h, -h, z], [h, -h, z], [h, h, z], [-h, h, z]];
    let floats =
        |values: Vec<f32>| -> Vec<u8> { values.into_iter().flat_map(f32::to_le_bytes).collect() };
    let positions = push_view(
        &mut bin,
        floats(corners.iter().flatten().copied().collect()),
    );
    let normals = push_view(&mut bin, floats([0.0, 0.0, 1.0].repeat(4)));
    let colors = push_view(&mut bin, floats(core.color.repeat(4)));
    let albedo = core
        .albedo
        .map(|albedo| push_view(&mut bin, albedo.repeat(4)));
    let indices = push_view(
        &mut bin,
        [0u32, 1, 2, 0, 2, 3]
            .iter()
            .flat_map(|i| i.to_le_bytes())
            .collect(),
    );
    let accessors = document["accessors"].as_array_mut().expect("accessors");
    let mut accessor = |value: serde_json::Value| {
        accessors.push(value);
        accessors.len() - 1
    };
    let position = accessor(
        json!({"bufferView": positions, "componentType": 5126, "count": 4,
        "type": "VEC3", "min": [-h, -h, z], "max": [h, h, z]}),
    );
    let normal =
        accessor(json!({"bufferView": normals, "componentType": 5126, "count": 4, "type": "VEC3"}));
    let color =
        accessor(json!({"bufferView": colors, "componentType": 5126, "count": 4, "type": "VEC4"}));
    let albedo = albedo.map(|view| {
        accessor(
            json!({"bufferView": view, "componentType": 5121, "normalized": true, "count": 4,
            "type": "VEC4"}),
        )
    });
    let index = accessor(
        json!({"bufferView": indices, "componentType": 5125, "count": 6, "type": "SCALAR"}),
    );
    let mut attributes = json!({"POSITION": position, "NORMAL": normal, "COLOR_0": color});
    if let Some(albedo) = albedo {
        attributes["_ALBEDO"] = json!(albedo);
    }
    document["meshes"][0]["primitives"]
        .as_array_mut()
        .expect("primitives")
        .push(
            json!({"mode": 4, "attributes": attributes, "indices": index,
            "extras": {"splatCore": true}}),
        );
    document["buffers"][0]["byteLength"] = json!(bin.len());
    glb(&document, &bin)
}
