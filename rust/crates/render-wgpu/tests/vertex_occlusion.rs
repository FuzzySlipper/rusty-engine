//! Baked vertex occlusion (#9506) composes with an occlusion or ORM map: a
//! neutral map keeps the vertex darkening, a darker map darkens further.

mod common;

use common::*;
use render_model::*;
use render_wgpu::RendererOptions;

/// A card facing the camera whose vertex colours carry `occlusion` in alpha,
/// flagged as vertex occlusion.
fn card(occlusion: f32) -> MeshPayloadDescriptor {
    let positions = vec![
        -1.0, -1.0, 0.0, 1.0, -1.0, 0.0, 1.0, 1.0, 0.0, -1.0, 1.0, 0.0,
    ];
    let normals = [0.0, 0.0, 1.0].repeat(4);
    let mut payload = payload(positions, normals, vec![0, 1, 2, 0, 2, 3]);
    let MeshPayloadSource::Inline { colors, uvs, .. } = &mut payload.source else {
        unreachable!()
    };
    *colors = Some([1.0, 1.0, 1.0, occlusion].repeat(4));
    *uvs = Some(vec![0.0, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0]);
    payload.layout.attributes.push(MeshAttribute {
        name: MeshAttributeName::Uv,
        components: 2,
        kind: MeshAttributeKind::F32,
    });
    payload.layout.attributes.push(MeshAttribute {
        name: MeshAttributeName::Color,
        components: 4,
        kind: MeshAttributeKind::F32,
    });
    payload.vertex_occlusion = true;
    payload
}

/// The card's mean brightness with `occlusion` baked and an optional map:
/// (red, green, blue) texel and whether it is packed ORM.
fn brightness(occlusion: f32, map: Option<([u8; 3], bool)>) -> f64 {
    let mut harness = Harness::new(RendererOptions {
        default_world_lights: false,
        ..RendererOptions::default()
    });
    let mut ops = vec![
        RenderDiff::SetBackgroundColor {
            color: [0.0, 0.0, 0.0, 1.0],
        },
        // Ambient light alone: what occlusion scales.
        RenderDiff::CreateLight {
            handle: RenderHandle::new(77),
            parent: None,
            light: LightDescriptor::Ambient {
                color: [1.0; 3],
                intensity: 1.0,
                enabled: true,
                shadow_intent: LightShadowIntent::Disabled,
                shadow: Default::default(),
                range: None,
            },
        },
    ];
    if let Some((texel, _)) = map {
        let (define, _) = harness.resources.texture(
            "texture/occlusion",
            2,
            2,
            &[texel[0], texel[1], texel[2], 255].repeat(4),
            TextureFilter::Linear,
        );
        ops.push(define);
    }
    // The card is a voxel chunk's payload: vertex occlusion rides chunk meshes.
    let mut descriptor =
        match coloured_mesh("card", card(occlusion), [0.8, 0.8, 0.8, 1.0]).remove(0) {
            RenderDiff::DefineMaterial { material } => material,
            _ => unreachable!(),
        };
    descriptor.id = "voxel-material/0".into();
    if let Some((_, packed)) = map {
        descriptor.occlusion_map = Some(MaterialOcclusionMapDescriptor {
            texture: "texture/occlusion".into(),
            strength: 1.0,
            roughness_metalness: packed,
        });
    }
    ops.push(RenderDiff::DefineMaterial {
        material: descriptor,
    });
    let mut chunk = card(occlusion);
    chunk.provenance = MeshProvenance::VoxelChunk;
    ops.push(RenderDiff::Create {
        handle: RenderHandle::new(1),
        parent: None,
        node: RenderNode {
            transform: transform([0.0, 0.0, -3.0], 0.0, 1.0),
            ..RenderNode::new(Geometry::Cube)
        },
    });
    ops.push(RenderDiff::ReplaceMeshPayload {
        handle: RenderHandle::new(1),
        payload: chunk,
    });
    harness.apply(ops);
    let frame = harness.single(&camera("eye", [0.0, 0.0, 0.0], 0.0, 0.0));
    let (x0, x1, y0, y1) = (WIDTH * 2 / 5, WIDTH * 3 / 5, HEIGHT * 2 / 5, HEIGHT * 3 / 5);
    let mut sum = 0.0;
    for y in y0..y1 {
        for x in x0..x1 {
            sum += f64::from(pixel(&frame, WIDTH, x, y)[1]);
        }
    }
    sum / f64::from((x1 - x0) * (y1 - y0))
}

#[test]
fn a_neutral_occlusion_or_orm_map_keeps_the_baked_vertex_occlusion() {
    let open = brightness(1.0, None);
    let baked = brightness(0.35, None);
    assert!(
        baked + 20.0 < open,
        "the baked occlusion darkens: {baked} against {open}"
    );
    for (name, map) in [
        ("white occlusion map", ([255, 255, 255], false)),
        ("ORM map with red 1", ([255, 255, 0], true)),
    ] {
        let kept = brightness(0.35, Some(map));
        assert!((kept - baked).abs() < 3.0, "{name}: {kept} against {baked}");
    }
    for (name, map) in [
        ("grey occlusion map", ([128, 128, 128], false)),
        ("ORM map with red 0.5", ([128, 255, 0], true)),
    ] {
        let darker = brightness(0.35, Some(map));
        assert!(darker + 5.0 < baked, "{name}: {darker} against {baked}");
        // And the map alone, without baked occlusion, is lighter.
        let map_only = brightness(1.0, Some(map));
        assert!(
            darker + 5.0 < map_only,
            "{name}: {darker} against {map_only}"
        );
    }
}
