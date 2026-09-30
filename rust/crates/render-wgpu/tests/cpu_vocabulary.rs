//! `render_wgpu::cpu` is public for render-export (#8948). Its decoded GLB
//! values cross as plain Engine arrays: glam is private to this crate, so
//! `Trs` and `GlbSkin` keep their glam fields crate-private and answer in
//! arrays. This file compiles only while that holds.

mod support;

use asset_import::{import_animated_glb_asset, ImportContext, SourceUri};
use render_wgpu::cpu::{decode_animated_asset, GlbModel, GlbSkin, Trs};
use support::Resources;

type Rest = ([f32; 3], [f32; 4], [f32; 3]);

fn rest(trs: &Trs) -> Rest {
    (trs.translation(), trs.rotation(), trs.scale())
}

fn inverse_binds(skin: &GlbSkin) -> Vec<[[f32; 4]; 4]> {
    skin.inverse_binds()
}

fn character() -> GlbModel {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../fixtures/render/assets/kenney-retro-character/character-medium.glb"
    );
    let bytes = std::fs::read(path).expect("read the character fixture");
    let imported = import_animated_glb_asset(
        &SourceUri::RelativePath("character.glb".to_owned()),
        &bytes,
        &ImportContext::default(),
    );
    let imported = imported.assets.expect("the character is admitted");
    let asset = imported.animated_mesh;
    let mut resources = Resources::default();
    let hash = asset.content_hash.as_deref().expect("content hash");
    resources.insert(
        &format!(
            "animated-mesh-resource/{}",
            hash.trim_start_matches("sha256:")
        ),
        imported.runtime_resource_bytes,
    );
    decode_animated_asset(&asset, &resources)
        .expect("the admitted character decodes")
        .0
}

#[test]
fn decoded_rest_poses_and_inverse_binds_are_engine_arrays() {
    let model = character();
    for node in &model.nodes {
        let (_, rotation, _) = rest(&node.rest);
        let length = rotation.iter().map(|value| value * value).sum::<f32>();
        assert!((length - 1.0).abs() < 1e-4, "{:?}: {rotation:?}", node.name);
    }
    let skin = model.skins.first().expect("the character is skinned");
    let matrices = inverse_binds(skin);
    assert_eq!(matrices.len(), skin.joints.len());
    assert!(matrices.iter().all(|matrix| matrix[3][3] == 1.0));
}
