//! A mesh moved into a packed mesh resource (as a scene snapshot moves every
//! inline mesh) draws as it did inline: its texture space travels with it, so
//! triplanar textures run on across voxel chunks offline as they do live
//! (#9847).

mod support;

use render_model::*;
use render_wgpu::RendererOptions;
use support::*;

const CHUNK: f32 = 4.0;

/// A voxel chunk's flat top, `CHUNK` square from its own origin, standing at
/// `at`, its texture space continuing the world's.
fn chunk(at: [f32; 3]) -> MeshPayloadDescriptor {
    let mut payload = payload(
        vec![
            0.0, 0.0, 0.0, 0.0, 0.0, CHUNK, CHUNK, 0.0, CHUNK, CHUNK, 0.0, 0.0,
        ],
        [0.0, 1.0, 0.0].repeat(4),
        vec![0.0; 8],
        vec![0, 1, 2, 0, 2, 3],
        &[(0, 6)],
    );
    payload.texture_space = Some(MeshTextureSpace {
        cell_size: 1.0,
        origin: at,
    });
    payload
}

const PLACES: [[f32; 3]; 2] = [[-CHUNK, -1.0, -8.0], [0.0, -1.0, -8.0]];

/// Two chunks side by side under a triplanar checker, each mesh as `prepare`
/// leaves it (with any resources it packs).
fn draw(
    prepare: impl Fn(MeshPayloadDescriptor, &mut Resources) -> MeshPayloadDescriptor,
) -> Vec<u8> {
    let mut harness = Harness::new(RendererOptions::default());
    let checker = checker(8, [220, 220, 220, 255], [40, 40, 50, 255]);
    let texture = harness
        .resources
        .texture("texture/checker", 8, 8, &checker, TextureWrap::Repeat);
    let mut ground = material("material/ground", [1.0; 4], Some("texture/checker"));
    ground.triplanar = Some(MaterialTriplanarDescriptor { sharpness: 4.0 });
    ground.texture_transform = Some(MaterialTextureTransformDescriptor {
        scale: [0.3, 0.3],
        offset: [0.0, 0.0],
    });
    let mut ops = vec![
        RenderDiff::DefineTexture { texture },
        RenderDiff::DefineMaterial { material: ground },
    ];
    for (index, at) in PLACES.into_iter().enumerate() {
        let asset = format!("mesh/chunk-{index}");
        let payload = prepare(chunk(at), &mut harness.resources);
        ops.push(static_mesh(&asset, payload, "material/ground"));
        ops.push(instance(
            10 + index as u64,
            None,
            &asset,
            transform(at, 0.0, [1.0; 3]),
        ));
    }
    harness.apply(ops);
    harness.render(&camera([0.0, 2.0, -2.0], 0.0, -40.0)).1
}

#[test]
fn packed_chunks_draw_as_inline_ones_with_their_texture_space() {
    let inline = draw(|payload, _| payload);
    let packed = draw(|payload, resources| {
        let mut set = pack_mesh_resources(&[payload], MAX_MESH_RESOURCE_BYTES).unwrap();
        for resource in set.resources {
            resources.insert(&resource.resource, resource.bytes);
        }
        let packed = set.payloads.remove(0);
        assert!(matches!(packed.source, MeshPayloadSource::Resource { .. }));
        packed
    });
    assert!(inline == packed, "the packed chunks draw differently");
    // Without their texture space each chunk restarts the checker at its
    // own origin: a seam where they meet.
    let chunk_local = draw(|mut payload, _| {
        payload.texture_space = None;
        payload
    });
    let differing = inline
        .as_chunks::<4>()
        .0
        .iter()
        .zip(chunk_local.as_chunks::<4>().0)
        .filter(|(a, b)| a != b)
        .count();
    assert!(
        differing > 1000,
        "texture space changes too little to tell: {differing} pixels"
    );
}
