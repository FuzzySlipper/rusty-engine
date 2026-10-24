//! A skinned mesh the device has no room for is refused, on a device that
//! is still alive (#10272, R10272-2): creating its instance reports the
//! upload's error as the operation's issue, nothing panics, the frame and
//! later meshes still draw, and the refused mesh is not tried again each
//! frame. A device whose buffers hold 1 MiB stands in for one out of
//! memory: the strip's 30,000 skinned vertices take 1.44 MB.

mod support;

use asset_import::{import_animated_glb_asset, ImportContext, SourceUri};
use render_model::*;
use render_wgpu::{Gpu, RendererOptions};
use serde_json::json;
use support::*;

const BUFFER_LIMIT: u64 = 1 << 20;
const QUADS: usize = 7_500;

/// One device for the binary, its buffers limited.
fn limited() -> Gpu {
    static GPU: std::sync::OnceLock<Gpu> = std::sync::OnceLock::new();
    GPU.get_or_init(|| {
        Gpu::headless_with_binding_limit(BUFFER_LIMIT).expect("upload tests need a wgpu adapter")
    })
    .clone()
}

fn floats(values: &[f32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

/// A metre-wide strip of `QUADS` quads facing +z, every vertex skinned
/// wholly to one root joint, with a still clip.
fn skinned_strip() -> Vec<u8> {
    let width = 1.0 / QUADS as f32;
    let vertices = QUADS * 4;
    let mut positions = Vec::with_capacity(vertices * 3);
    let mut indices = Vec::with_capacity(QUADS * 6);
    for quad in 0..QUADS {
        let x = quad as f32 * width - 0.5;
        positions.extend_from_slice(&[x, 0.0, 0.0, x + width, 0.0, 0.0]);
        positions.extend_from_slice(&[x + width, 1.0, 0.0, x, 1.0, 0.0]);
        let base = quad as u32 * 4;
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    let normals = [0.0, 0.0, 1.0].repeat(vertices);
    let weights = [1.0, 0.0, 0.0, 0.0].repeat(vertices);
    let identity = [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ];
    let mut bin = Vec::new();
    let mut views = Vec::new();
    let mut view = |bytes: Vec<u8>| {
        views.push(json!({"buffer": 0, "byteOffset": bin.len(), "byteLength": bytes.len()}));
        bin.extend(bytes);
        bin.resize(bin.len().next_multiple_of(4), 0);
        views.len() - 1
    };
    let position = view(floats(&positions));
    let normal = view(floats(&normals));
    let joint = view(vec![0_u8; vertices * 4]);
    let weight = view(floats(&weights));
    let index = view(
        indices
            .iter()
            .flat_map(|index| index.to_le_bytes())
            .collect(),
    );
    let inverse = view(floats(&identity));
    // A still one-second clip: the importer takes a skinned GLB with its
    // clips.
    let time = view(floats(&[0.0, 1.0]));
    let rotation = view(floats(&[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]));
    let document = json!({
        "asset": {"version": "2.0"},
        "scene": 0,
        "scenes": [{"nodes": [0, 1]}],
        "nodes": [{"mesh": 0, "skin": 0}, {"name": "root"}],
        "skins": [{"joints": [1], "inverseBindMatrices": 5}],
        "animations": [{
            "name": "rest",
            "samplers": [{"input": 6, "output": 7, "interpolation": "LINEAR"}],
            "channels": [{"sampler": 0, "target": {"node": 1, "path": "rotation"}}],
        }],
        "meshes": [{"primitives": [{
            "attributes": {"POSITION": 0, "NORMAL": 1, "JOINTS_0": 2, "WEIGHTS_0": 3},
            "indices": 4,
        }]}],
        "accessors": [
            {"bufferView": position, "componentType": 5126, "count": vertices, "type": "VEC3",
             "min": [-0.5, 0.0, 0.0], "max": [0.5, 1.0, 0.0]},
            {"bufferView": normal, "componentType": 5126, "count": vertices, "type": "VEC3"},
            {"bufferView": joint, "componentType": 5121, "count": vertices, "type": "VEC4"},
            {"bufferView": weight, "componentType": 5126, "count": vertices, "type": "VEC4"},
            {"bufferView": index, "componentType": 5125, "count": indices.len(), "type": "SCALAR"},
            {"bufferView": inverse, "componentType": 5126, "count": 1, "type": "MAT4"},
            {"bufferView": time, "componentType": 5126, "count": 2, "type": "SCALAR",
             "min": [0.0], "max": [1.0]},
            {"bufferView": rotation, "componentType": 5126, "count": 2, "type": "VEC4"},
        ],
        "bufferViews": views,
        "buffers": [{"byteLength": bin.len()}],
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

/// Admit the strip and hold its runtime resource.
fn strip(harness: &mut Harness) -> AnimatedMeshAsset {
    let bytes = skinned_strip();
    let imported = import_animated_glb_asset(
        &SourceUri::RelativePath("strip.glb".to_owned()),
        &bytes,
        &ImportContext::default(),
    );
    let asset = imported
        .assets
        .unwrap_or_else(|| panic!("the strip is admitted: {:?}", imported.diagnostics))
        .animated_mesh;
    let hash = asset.content_hash.clone().expect("content hash");
    harness.resources.insert(
        &format!(
            "animated-mesh-resource/{}",
            hash.trim_start_matches("sha256:")
        ),
        bytes,
    );
    asset
}

#[test]
fn a_skinned_mesh_too_large_for_the_device_is_refused_and_the_frame_draws() {
    let mut harness = Harness::on(
        limited(),
        RendererOptions {
            default_world_lights: true,
            ..RendererOptions::default()
        },
    );
    let strip = strip(&mut harness);
    harness.apply(vec![RenderDiff::DefineAnimatedMesh {
        asset: strip.clone(),
    }]);
    let eye = camera([0.0, 0.5, 3.0], 0.0, 0.0);
    let (_, empty) = harness.render(&eye);

    let delta = harness
        .world
        .apply(RenderFrameDiff {
            publication: None,
            ops: vec![RenderDiff::CreateAnimatedMeshInstance {
                handle: RenderHandle::new(1),
                parent: None,
                instance: AnimatedMeshInstanceDescriptor {
                    playback_fade: None,
                    inspection: AnimatedMeshInspection::default(),
                    asset: strip.asset.clone(),
                    transform: Transform::IDENTITY,
                    visible: true,
                    material_overrides: Vec::new(),
                    playback: None,
                    metadata: RenderMetadata::default(),
                    layer: RenderLayer::Scene,
                    shadow_casting: Default::default(),
                },
            }],
        })
        .expect("retained model admits the instance");
    let issues = harness.renderer.apply(&delta, &harness.resources);
    let [issue] = issues.as_slice() else {
        panic!("the instance's skinned mesh alone is refused: {issues:?}");
    };
    assert!(issue.detail.contains("could not be uploaded"), "{issue:?}");

    // The refused strip draws nothing, frame after frame.
    for _ in 0..3 {
        assert_eq!(harness.render(&eye).1, empty, "the refused strip drew");
    }
    assert_eq!(harness.gpu.lost(), None);
    // The device goes on taking meshes that fit.
    let triangle = payload(
        vec![-0.5, 0.0, 0.0, 0.5, 0.0, 0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0].repeat(3),
        vec![0.0; 6],
        vec![0, 1, 2],
        &[(0, 3)],
    );
    harness.apply(vec![
        RenderDiff::DefineMaterial {
            material: material("material/plain", [1.0; 4], None),
        },
        static_mesh("mesh/runtime-1", triangle, "material/plain"),
        instance(
            2,
            None,
            "mesh/runtime-1",
            transform([0.0, 0.0, 0.0], 0.0, [1.0; 3]),
        ),
    ]);
    assert_ne!(harness.render(&eye).1, empty, "the mesh that fits draws");
}
