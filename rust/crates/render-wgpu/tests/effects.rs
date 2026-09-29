//! Sprite and particle fixtures:
//! - billboard modes, pixel size, viewport placement, atlas frames, tints and
//!   alpha modes;
//! - lit sprites;
//! - particle bursts and emitters, aged on Engine time only.
//!
//! `RENDER_WGPU_BLESS=1 cargo test -p render-wgpu --test effects` rewrites the
//! references.

mod common;

use common::*;
use render_model::*;
use render_presentation::{
    ParticleAnchor, ParticleColorKey, ParticleEmitterDescriptor, ParticleEmitterHandle,
    ParticleProjectionOp, ParticleScalarKey, ParticleSpriteRef, ParticleVisual,
    PresentationFrameDiff, PresentationOp, PresentationOpMeta,
};
use render_wgpu::{RendererOptions, TargetStatus};

const NO_ENTITIES: &dyn Fn(u64) -> Option<[f32; 3]> = &|_| None;

/// Two 8×8 frames side by side: a red and a green tile, each with a white top
/// band (to show orientation) and transparent corners.
fn atlas_image() -> Vec<u8> {
    let mut rgba = Vec::with_capacity(16 * 8 * 4);
    for y in 0..8 {
        for x in 0..16 {
            let local = x % 8;
            let corner = (local == 0 || local == 7) && (y == 0 || y == 7);
            let pixel = if corner {
                [0, 0, 0, 0]
            } else if y < 2 {
                [255, 255, 255, 255]
            } else if x < 8 {
                [220, 40, 30, 255]
            } else {
                [40, 200, 60, 255]
            };
            rgba.extend_from_slice(&pixel);
        }
    }
    rgba
}

fn atlas_ops(resources: &mut Resources) -> Vec<RenderDiff> {
    let (texture, _) = resources.texture(
        "texture/atlas",
        16,
        8,
        &atlas_image(),
        TextureFilter::Nearest,
    );
    vec![
        texture,
        RenderDiff::DefineSpriteAtlas {
            atlas: SpriteAtlasDescriptor {
                id: "sprite/atlas".to_owned(),
                texture: "texture/atlas".to_owned(),
                frames: vec![
                    SpriteFrameRect {
                        frame: 0,
                        uv_min: [0.0, 0.0],
                        uv_max: [0.5, 1.0],
                        size: None,
                    },
                    SpriteFrameRect {
                        frame: 1,
                        uv_min: [0.5, 0.0],
                        uv_max: [1.0, 1.0],
                        size: None,
                    },
                ],
            },
        },
    ]
}

fn sprite(frame: u32, translation: [f32; 3], billboard: BillboardMode) -> SpriteInstanceDescriptor {
    SpriteInstanceDescriptor {
        asset: "sprite/atlas".to_owned(),
        frame,
        pivot: [0.5, 0.5],
        size: [1.0, 1.0],
        size_mode: SpriteSizeMode::World,
        billboard,
        tint: [1.0; 4],
        render_order: 0,
        depth: SpriteDepthPolicy::Default,
        layer: RenderLayer::Scene,
        viewport_placement: None,
        shading: SpriteShading::Unlit,
        material: SpriteMaterialDescriptor::default(),
        visible: true,
        transform: transform(translation, 0.0, 1.0),
        attachment: SpriteAttachment::default(),
        metadata: RenderMetadata::default(),
    }
}

fn create(handle: u64, sprite: SpriteInstanceDescriptor) -> RenderDiff {
    RenderDiff::CreateSprite {
        handle: RenderHandle::new(handle),
        parent: None,
        sprite,
    }
}

fn scene() -> Vec<RenderDiff> {
    let mut ops = vec![RenderDiff::SetBackgroundColor {
        color: [0.03, 0.04, 0.08, 1.0],
    }];
    ops.extend(coloured_mesh("floor", floor(12.0), [0.45, 0.45, 0.5, 1.0]));
    ops.push(instance(
        1,
        None,
        "floor",
        transform([0.0, -0.5, -5.0], 0.0, 1.0),
    ));
    ops
}

#[test]
fn sprites_face_the_camera_by_mode_and_place_in_the_viewport() {
    let mut harness = Harness::new(RendererOptions::default());
    let mut ops = scene();
    ops.extend(atlas_ops(&mut harness.resources));
    // Spherical, cylindrical (bottom pivot) and authored orientation.
    ops.push(create(
        10,
        sprite(0, [-1.6, 0.4, -4.0], BillboardMode::Spherical),
    ));
    let mut standing = sprite(1, [0.0, -0.5, -4.5], BillboardMode::Cylindrical);
    standing.pivot = [0.5, 0.0];
    standing.size = [1.0, 1.6];
    ops.push(create(11, standing));
    let mut turned = sprite(0, [1.6, 0.4, -4.0], BillboardMode::None);
    turned.transform = transform([1.6, 0.4, -4.0], 60.0, 1.0);
    ops.push(create(12, turned));
    // Constant 24-pixel size, far away.
    let mut pixel_sized = sprite(1, [0.0, 1.8, -9.0], BillboardMode::Spherical);
    pixel_sized.size_mode = SpriteSizeMode::Pixel;
    pixel_sized.size = [24.0, 24.0];
    ops.push(create(13, pixel_sized));
    // A tinted, half-transparent sprite over the first one.
    let mut glass = sprite(1, [-1.3, 0.55, -3.4], BillboardMode::Spherical);
    glass.tint = [0.3, 0.6, 1.0, 0.5];
    ops.push(create(14, glass));
    // Masked: the transparent corners are cut, no blending.
    let mut masked = sprite(0, [0.9, -0.1, -3.0], BillboardMode::Spherical);
    masked.size = [0.5, 0.5];
    masked.material.alpha = SpriteAlphaMode::Mask { cutoff: 0.5 };
    ops.push(create(15, masked));
    // A weapon-style viewmodel sprite placed in the lower right of the view,
    // drawn over everything.
    let mut weapon = sprite(1, [0.0; 3], BillboardMode::None);
    weapon.layer = RenderLayer::Viewmodel;
    weapon.depth = SpriteDepthPolicy::DepthTestOff;
    weapon.size = [2.0, 1.0];
    weapon.viewport_placement = Some(SpriteViewportPlacement {
        minimum: [0.6, 0.0],
        size: [0.4, 0.4],
        alignment: [1.0, 0.0],
        fit: SpriteViewportFit::Contain,
    });
    ops.push(create(16, weapon));
    harness.apply(ops);
    let eye = camera("eye", [0.0, 1.0, 1.0], 0.0, -10.0);
    let pixels = harness.single(&eye);
    assert_screenshot("sprites", WIDTH, HEIGHT, &pixels);

    // Playback arrives as frame updates; nothing animates on its own.
    assert_eq!(harness.single(&eye), pixels);
    harness.apply(vec![RenderDiff::UpdateSprite {
        handle: RenderHandle::new(10),
        frame: Some(1),
        tint: None,
        render_order: None,
        visible: None,
    }]);
    assert_ne!(harness.single(&eye), pixels);
}

#[test]
fn lit_sprites_shade_with_synthetic_normal_and_height_maps() {
    let mut harness = Harness::new(RendererOptions {
        default_world_lights: false,
        ..RendererOptions::default()
    });
    let mut ops = scene();
    ops.extend(atlas_ops(&mut harness.resources));
    // A flat normal map (0.5, 0.5, 1) with a bump in the middle, linear.
    let normals: Vec<u8> = (0..64)
        .flat_map(|index| {
            let (x, y) = (index % 8, index / 8);
            let tilt = if (2..6).contains(&x) && (2..6).contains(&y) {
                60
            } else {
                0
            };
            [128 + tilt, 128, 255 - tilt, 255]
        })
        .collect();
    let (mut normal_texture, _) =
        harness
            .resources
            .texture("texture/normal", 8, 8, &normals, TextureFilter::Linear);
    if let RenderDiff::DefineTexture { texture } = &mut normal_texture {
        if let Some(payload) = texture.payload.as_mut() {
            payload.color_space = TextureColorSpace::Linear;
        }
    }
    ops.push(normal_texture);
    for (handle, x, lighting, normal_texture) in [
        (20, -2.4, SpriteLightingMode::Unlit, None),
        (21, -0.8, SpriteLightingMode::Synthetic, None),
        (
            22,
            0.8,
            SpriteLightingMode::AuthoredNormal,
            Some("texture/normal"),
        ),
        (23, 2.4, SpriteLightingMode::DerivedGradient, None),
    ] {
        let mut lit = sprite(0, [x, 0.4, -4.0], BillboardMode::Spherical);
        lit.size = [1.3, 1.3];
        lit.material.lighting = lighting;
        lit.material.normal_texture = normal_texture.map(str::to_owned);
        lit.material.normal_strength = 1.5;
        ops.push(create(handle, lit));
    }
    // A warm point light up and to the left, and a dim ambient.
    for (handle, light) in [
        (
            30,
            LightDescriptor::Point {
                enabled: true,
                color: [1.0, 0.85, 0.6],
                intensity: 18.0,
                position: [-1.5, 2.0, -2.5],
                range: None,
                decay: 2.0,
                shadow_intent: LightShadowIntent::Disabled,
            },
        ),
        (
            31,
            LightDescriptor::Ambient {
                enabled: true,
                color: [0.4, 0.45, 0.6],
                intensity: 0.4,
                shadow_intent: LightShadowIntent::Disabled,
            },
        ),
    ] {
        ops.push(RenderDiff::CreateLight {
            handle: RenderHandle::new(handle),
            parent: None,
            light,
        });
    }
    harness.apply(ops);
    let pixels = harness.single(&camera("eye", [0.0, 0.6, 1.0], 0.0, -4.0));
    assert_screenshot("sprites-lit", WIDTH, HEIGHT, &pixels);
}

fn particle_frame(ops: Vec<ParticleProjectionOp>) -> PresentationFrameDiff {
    PresentationFrameDiff::try_from_ops(
        ops.into_iter()
            .enumerate()
            .map(|(sequence, op)| PresentationOp::Particle {
                meta: PresentationOpMeta::new(sequence as u32),
                op,
            })
            .collect(),
    )
    .expect("a valid presentation frame")
}

fn emitter(
    visual: ParticleVisual,
    position: [f32; 3],
    count: u32,
    seed: u64,
) -> ParticleEmitterDescriptor {
    ParticleEmitterDescriptor {
        anchor: ParticleAnchor::World { position },
        visual,
        rate_per_second: 0.0,
        burst_count: count,
        lifetime_seconds: [1.5, 2.5],
        velocity_min: [-1.2, 1.5, -1.2],
        velocity_max: [1.2, 3.5, 1.2],
        acceleration: [0.0, -4.0, 0.0],
        size_curve: vec![
            ParticleScalarKey {
                age: 0.0,
                value: 0.6,
            },
            ParticleScalarKey {
                age: 1.0,
                value: 0.2,
            },
        ],
        color_curve: vec![
            ParticleColorKey {
                age: 0.0,
                color: [1.0, 0.9, 0.3, 1.0],
            },
            ParticleColorKey {
                age: 1.0,
                color: [1.0, 0.2, 0.1, 0.2],
            },
        ],
        flipbook_frames_per_second: 4.0,
        seed,
        max_particles: 256,
        visible: true,
        collision: None,
    }
}

#[test]
fn particle_bursts_age_on_engine_time_and_freeze_when_held() {
    let mut harness = Harness::new(RendererOptions::default());
    let mut ops = scene();
    let (sparks, hash) = harness.resources.texture(
        "texture/sparks",
        16,
        8,
        &atlas_image(),
        TextureFilter::Nearest,
    );
    ops.push(sparks);
    harness.apply(ops);
    let billboards = ParticleVisual::Billboard {
        sprite: ParticleSpriteRef {
            asset: "texture/sparks".to_owned(),
            content_hash: hash,
            frame_count: 2,
        },
    };
    let mut fountain = emitter(billboards.clone(), [1.2, -0.5, -4.5], 0, 3);
    fountain.rate_per_second = 40.0;
    let issues = harness.renderer.apply_presentation(
        &particle_frame(vec![
            ParticleProjectionOp::Emit {
                signal_id: "impact".to_owned(),
                descriptor: emitter(billboards, [-1.2, -0.3, -4.0], 48, 7),
            },
            ParticleProjectionOp::Emit {
                signal_id: "debris".to_owned(),
                descriptor: emitter(ParticleVisual::Cube, [0.0, -0.3, -5.0], 32, 11),
            },
            ParticleProjectionOp::Create {
                handle: ParticleEmitterHandle::new(1),
                descriptor: fountain,
            },
        ]),
        &harness.resources,
        NO_ENTITIES,
    );
    assert!(issues.is_empty(), "{issues:?}");
    for _ in 0..6 {
        harness.renderer.advance_effects(1.0 / 20.0, NO_ENTITIES);
    }
    let eye = camera("eye", [0.0, 1.0, 1.5], 0.0, -10.0);
    let burst = harness.single(&eye);
    assert_screenshot("particles", WIDTH, HEIGHT, &burst);

    // Held: the presentation clock may run, but no Engine time passes.
    harness.renderer.advance_effects(0.0, NO_ENTITIES);
    assert_eq!(harness.single(&eye), burst, "a held burst is frozen");
    harness.renderer.advance_effects(1.0 / 20.0, NO_ENTITIES);
    assert_ne!(harness.single(&eye), burst, "Engine time moves it again");

    // Bursts end with their particles; the retained emitter keeps going.
    for _ in 0..60 {
        harness.renderer.advance_effects(1.0 / 20.0, NO_ENTITIES);
    }
    let (live, emitters) = harness.renderer.particle_counts();
    assert_eq!(emitters, 1);
    assert!(live > 0 && live < 150, "only the fountain remains: {live}");
}

#[test]
fn particles_mark_offscreen_targets_stale_only_when_they_move() {
    use render_host_contracts::{
        RendererCompositionTarget, RendererCompositionView, RendererTargetColor,
        RendererTargetDepth, RendererTargetSampling, RendererViewTarget,
    };
    let mut harness = Harness::new(RendererOptions::default());
    harness.apply(scene());
    let mut view = composition(
        vec![camera("eye", [0.0, 1.0, 1.5], 0.0, -10.0)],
        vec![RendererCompositionView {
            id: "monitor".to_owned(),
            camera_id: "eye".to_owned(),
            target: RendererViewTarget::Offscreen {
                target_id: "monitor".to_owned(),
                target_revision: 1,
            },
            viewport: viewport(0.0, 0.0, 1.0, 1.0),
            order: 0,
        }],
    );
    view.targets.push(RendererCompositionTarget {
        id: "monitor".to_owned(),
        revision: 1,
        width: 64,
        height: 64,
        color: RendererTargetColor::Rgba8Srgb,
        depth: RendererTargetDepth::Depth24,
        sampling: RendererTargetSampling::Linear,
    });
    harness.renderer.set_view_composition(&view, 0.0);
    harness.renderer.apply_presentation(
        &particle_frame(vec![ParticleProjectionOp::Emit {
            signal_id: "hit".to_owned(),
            descriptor: emitter(ParticleVisual::Cube, [0.0, 0.0, -4.0], 8, 1),
        }]),
        &harness.resources,
        NO_ENTITIES,
    );
    harness.composition(0.0);
    harness.renderer.advance_effects(0.0, NO_ENTITIES);
    let readout = harness.renderer.view_composition_readout();
    assert_eq!(readout.targets[0].status, TargetStatus::Current, "held");
    harness.renderer.advance_effects(0.05, NO_ENTITIES);
    let readout = harness.renderer.view_composition_readout();
    assert_eq!(readout.targets[0].status, TargetStatus::Stale, "advanced");
}
