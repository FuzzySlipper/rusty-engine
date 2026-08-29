use rusty_engine::render_model::{
    RenderDiff, RenderFrameDiff, SpriteAtlasDescriptor, SpriteFrameRect,
};

#[test]
fn public_facade_admits_and_serializes_optional_sprite_frame_world_size() {
    let frame = RenderFrameDiff::try_from_ops(vec![RenderDiff::DefineSpriteAtlas {
        atlas: SpriteAtlasDescriptor {
            id: "sprite/creature".to_owned(),
            texture: "texture/creature".to_owned(),
            frames: vec![
                SpriteFrameRect {
                    frame: 0,
                    uv_min: [0.0, 0.0],
                    uv_max: [0.5, 1.0],
                    size: Some([1.25, 2.5]),
                },
                SpriteFrameRect {
                    frame: 1,
                    uv_min: [0.5, 0.0],
                    uv_max: [1.0, 1.0],
                    size: None,
                },
            ],
        },
    }])
    .expect("facade frame should admit positive finite frame sizes");

    let serialized = serde_json::to_value(frame).expect("serialize public facade frame");
    let frames = serialized["ops"][0]["atlas"]["frames"]
        .as_array()
        .expect("serialized atlas frames");
    assert_eq!(frames[0]["size"], serde_json::json!([1.25, 2.5]));
    assert!(!frames[1]
        .as_object()
        .expect("serialized fallback frame")
        .contains_key("size"));
}
