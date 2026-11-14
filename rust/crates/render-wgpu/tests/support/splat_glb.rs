//! Splat GLBs written for the splat fixtures (`docs/splat-characters.md`):
//! one node drawing one `POINTS` primitive of Gaussians with `POSITION`,
//! `_ROTATION`, `_SCALE` and `_COLOR`, as an asset writer exports them.

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
}

impl Gaussian {
    /// A round Gaussian of standard deviation `sigma`.
    pub fn round(centre: [f32; 3], sigma: f32, color: [u8; 4]) -> Self {
        Self {
            centre,
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [sigma; 3],
            color,
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
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];
    for gaussian in gaussians {
        for axis in 0..3 {
            min[axis] = min[axis].min(gaussian.centre[axis]);
            max[axis] = max[axis].max(gaussian.centre[axis]);
        }
    }
    let document = json!({
        "asset": {"version": "2.0", "generator": "render-wgpu splat fixtures"},
        "scene": 0,
        "scenes": [{"nodes": [0]}],
        "nodes": [{"name": "splats", "mesh": 0, "translation": translation}],
        "meshes": [{"name": "splats", "primitives": [{
            "mode": 0,
            "attributes": {"POSITION": 0, "_ROTATION": 1, "_SCALE": 2, "_COLOR": 3}
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
