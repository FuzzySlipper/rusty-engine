//! The ray scene (`ray_scene.rs`): acceleration structures built from the
//! parts the world pass draws, traced by test rays. A static mesh, a
//! replaced payload (as a voxel chunk's) and a skinned character are in it;
//! a new payload and a new pose build new geometry. A device made without
//! ray queries, or a software one, refuses it with the reason.

mod support;

use std::path::PathBuf;

use asset_import::{import_animated_glb_asset, ImportContext, SourceUri};
use render_model::*;
use render_wgpu::{
    Gpu, ProbeHit, ProbeRay, RaySceneReadout, Renderer, RendererOptions, SettingRefusal,
};
use support::*;

const BOX: u64 = 10;
const CHUNK: u64 = 20;
const BODY: u64 = 30;

/// Every instance mask bit.
const ALL: u8 = 0xff;

fn fixture(path: &str) -> Vec<u8> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures");
    std::fs::read(root.join(path)).unwrap_or_else(|error| panic!("read {path}: {error}"))
}

/// Admit a GLB as the runtime does and hold its resource.
fn admit(harness: &mut Harness, path: &str) -> AnimatedMeshAsset {
    let bytes = fixture(path);
    let imported = import_animated_glb_asset(
        &SourceUri::RelativePath(path.to_owned()),
        &bytes,
        &ImportContext::default(),
    );
    let asset = imported
        .assets
        .unwrap_or_else(|| panic!("{path} is admitted: {:?}", imported.diagnostics))
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

fn character(asset: &AnimatedMeshAsset, normalized_time: f32) -> RenderDiff {
    RenderDiff::CreateAnimatedMeshInstance {
        handle: RenderHandle::new(BODY),
        parent: None,
        instance: AnimatedMeshInstanceDescriptor {
            playback_fade: None,
            inspection: AnimatedMeshInspection::default(),
            asset: asset.asset.clone(),
            transform: transform([-3.0, 0.0, -5.0], 0.0, [1.0; 3]),
            visible: true,
            material_overrides: Vec::new(),
            playback: Some(pose(normalized_time)),
            metadata: RenderMetadata {
                source_entity: Some(BODY),
                ..RenderMetadata::default()
            },
            layer: RenderLayer::Scene,
            shadow_casting: Default::default(),
        },
    }
}

fn pose(normalized_time: f32) -> AnimatedMeshPlaybackCommand {
    AnimatedMeshPlaybackCommand::Sample {
        clip: "run".to_owned(),
        normalized_time,
    }
}

/// A voxel chunk's payload: a box from the ground up to `height`, at x 3.
fn chunk(height: f32) -> RenderDiff {
    let mut payload = box_mesh([2.5, 0.0, -5.5], [3.5, height, -4.5], |_| 0);
    payload.provenance = MeshProvenance::VoxelChunk;
    RenderDiff::ReplaceMeshPayload {
        handle: RenderHandle::new(CHUNK),
        payload,
    }
}

/// A box, a payload chunk and a skinned character in a row 5 m ahead.
fn scene(harness: &mut Harness) {
    let body = admit(harness, "csharp-joint-attachments/content/body.glb");
    harness.apply(vec![
        RenderDiff::DefineMaterial {
            material: material("material/grey", [0.6, 0.6, 0.6, 1.0], None),
        },
        static_mesh(
            "mesh/box",
            box_mesh([-0.5, 0.0, -0.5], [0.5, 1.0, 0.5], |_| 0),
            "material/grey",
        ),
        instance(
            BOX,
            None,
            "mesh/box",
            transform([0.0, 0.0, -5.0], 0.0, [1.0; 3]),
        ),
        RenderDiff::Create {
            handle: RenderHandle::new(CHUNK),
            parent: None,
            node: RenderNode::new(Geometry::Cube),
        },
        chunk(1.0),
        RenderDiff::DefineAnimatedMesh {
            asset: body.clone(),
        },
        character(&body, 0.0),
    ]);
}

fn frame(harness: &mut Harness) {
    harness.render(&camera([0.0, 1.5, 2.0], 0.0, -10.0));
}

fn down(x: f32, z: f32) -> ProbeRay {
    ProbeRay {
        origin: [x, 5.0, z],
        direction: [0.0, -1.0, 0.0],
        reach: 10.0,
        mask: ALL,
    }
}

fn readout(renderer: &Renderer) -> RaySceneReadout {
    renderer
        .gpu_readout()
        .ray_scene
        .expect("the ray scene is asked for")
}

/// Horizontal rays across the character from its side, on a grid over its
/// height and depth: what they hit of it.
fn across_character(renderer: &mut Renderer) -> Vec<Option<u32>> {
    let rays: Vec<ProbeRay> = (0..40)
        .flat_map(|row| {
            (0..24).map(move |column| ProbeRay {
                origin: [-6.0, 0.05 + row as f32 * 0.05, -5.6 + column as f32 * 0.05],
                direction: [1.0, 0.0, 0.0],
                reach: 3.5,
                mask: ALL,
            })
        })
        .collect();
    renderer
        .trace_rays(&rays)
        .into_iter()
        .map(|hit| {
            hit.map(|hit| {
                assert_eq!(hit.node, Some(RenderHandle::new(BODY)));
                // Millimetres, so poses compare exactly.
                (hit.distance * 1000.0) as u32
            })
        })
        .collect()
}

fn assert_hit(hit: Option<ProbeHit>, node: u64, distance: f32, what: &str) {
    let hit = hit.unwrap_or_else(|| panic!("{what}: no hit"));
    assert_eq!(hit.node, Some(RenderHandle::new(node)), "{what}");
    assert!(
        (hit.distance - distance).abs() < 1e-3,
        "{what}: hit at {}, expected {distance}",
        hit.distance
    );
    assert!(hit.front_face, "{what}: the outside of the mesh");
}

#[test]
fn the_ray_scene_holds_the_drawn_parts_and_follows_new_payloads_and_poses() {
    let gpu = Gpu::headless_with_ray_queries().expect("a device");
    let mut harness = Harness::on(gpu, RendererOptions::default());
    scene(&mut harness);
    frame(&mut harness);
    assert_eq!(
        harness.renderer.gpu_readout().ray_scene,
        None,
        "nothing is built unless asked for"
    );
    harness.renderer.set_ray_scene(true);
    if let Some((refusal, detail)) = readout(&harness.renderer).refused {
        // llvmpipe advertises ray queries; the software adapter refuses.
        assert!(harness.gpu.is_software(), "refused: {detail}");
        assert_eq!(refusal, SettingRefusal::SoftwareAdapter);
        return;
    }
    frame(&mut harness);
    let built = readout(&harness.renderer);
    assert_eq!(built.waiting, 0);
    assert!(built.blases >= 3, "{built:?}");
    assert!(built.tlas_instances >= 3, "{built:?}");
    assert!(built.triangles > 24 && built.blas_bytes > 0, "{built:?}");

    let probe = |renderer: &mut Renderer| {
        renderer.trace_rays(&[
            // At the box's front face, 4.5 m ahead.
            ProbeRay {
                origin: [0.0, 0.5, 0.0],
                direction: [0.0, 0.0, -1.0],
                reach: 20.0,
                mask: ALL,
            },
            down(3.0, -5.0),
            // Between them: nothing.
            down(1.5, -5.0),
        ])
    };
    let hits = probe(&mut harness.renderer);
    assert_hit(hits[0], BOX, 4.5, "the box");
    assert_hit(hits[1], CHUNK, 4.0, "the chunk");
    assert_eq!(hits[2], None, "between them");
    let posed = across_character(&mut harness.renderer);
    assert!(
        posed.iter().filter(|hit| hit.is_some()).count() > 20,
        "the character is in the ray scene"
    );

    // Built BLAS compact once their builds complete; the TLAS takes the
    // compacted ones and the rays still find the same surfaces.
    for _ in 0..3 {
        frame(&mut harness);
    }
    let compacted = readout(&harness.renderer);
    assert!(compacted.compacted >= 2, "{compacted:?}");
    let hits = probe(&mut harness.renderer);
    assert_hit(hits[0], BOX, 4.5, "the compacted box");
    assert_hit(hits[1], CHUNK, 4.0, "the compacted chunk");

    // A new payload is new geometry: a new BLAS, and the chunk is taller.
    harness.apply(vec![chunk(2.0)]);
    frame(&mut harness);
    let rebuilt = readout(&harness.renderer);
    assert!(rebuilt.blas_builds > compacted.blas_builds, "{rebuilt:?}");
    assert_eq!(rebuilt.blases, compacted.blases, "the old BLAS is dropped");
    assert_hit(
        probe(&mut harness.renderer)[1],
        CHUNK,
        3.0,
        "the taller chunk",
    );

    // A new pose rebuilds the character's BLAS, and the rays see it.
    harness.apply(vec![RenderDiff::SetAnimatedMeshPlayback {
        handle: RenderHandle::new(BODY),
        playback: pose(0.5),
    }]);
    frame(&mut harness);
    let reposed = across_character(&mut harness.renderer);
    assert!(
        readout(&harness.renderer).blas_builds > rebuilt.blas_builds,
        "the pose builds again"
    );
    assert_ne!(posed, reposed, "the rays follow the new pose");

    // Hidden, a part leaves the ray scene.
    harness.apply(vec![RenderDiff::Update {
        handle: RenderHandle::new(BOX),
        transform: None,
        material: None,
        visible: Some(false),
        metadata: None,
    }]);
    frame(&mut harness);
    assert_eq!(probe(&mut harness.renderer)[0], None, "the hidden box");

    // Not asked for, it is dropped.
    harness.renderer.set_ray_scene(false);
    assert_eq!(harness.renderer.gpu_readout().ray_scene, None);
    assert!(harness.renderer.trace_rays(&[down(3.0, -5.0)]).is_empty());
    drop(harness);
    refused_without_ray_queries();
}

/// A device made without ray queries refuses the ray scene. In the test
/// above, so the binary's devices are made one at a time.
fn refused_without_ray_queries() {
    let gpu = Gpu::headless().expect("a device");
    let mut renderer = Renderer::new(&gpu, RendererOptions::default());
    renderer.set_ray_scene(true);
    let (refusal, detail) = renderer
        .gpu_readout()
        .ray_scene
        .and_then(|readout| readout.refused)
        .expect("refused");
    if gpu.is_software() {
        assert_eq!(refusal, SettingRefusal::SoftwareAdapter, "{detail}");
    } else {
        assert_eq!(refusal, SettingRefusal::NoRayQueries, "{detail}");
        assert!(detail.contains("ray queries"), "{detail}");
    }
}
