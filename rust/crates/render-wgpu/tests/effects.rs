//! Sprite and particle fixtures:
//! - billboard modes, pixel size, viewport placement, atlas frames, tints and
//!   alpha modes;
//! - lit sprites, tint masks and sprite emission;
//! - sprites in the wind;
//! - billboard copies lit and swayed as the same sprite;
//! - particle bursts and emitters, aged on Engine time only.
//!
//! `RENDER_WGPU_BLESS=1 cargo test -p render-wgpu --test effects` rewrites the
//! references.

mod common;

use common::*;
use render_model::*;
use render_presentation::{
    ParticleAnchor, ParticleBlendMode, ParticleColorKey, ParticleEmitterDescriptor,
    ParticleEmitterHandle, ParticleEmitterPatch, ParticleProjectionOp, ParticleScalarKey,
    ParticleSizeMode, ParticleSpriteRef, ParticleVisual, PresentationFrameDiff, PresentationOp,
    PresentationOpMeta,
};
use render_wgpu::{OffscreenTarget, RendererOptions, TargetStatus};

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
        emission_intensity: None,
    }]);
    assert_ne!(harness.single(&eye), pixels);
}

#[test]
fn sprite_discovery_follows_the_sprites_not_the_scene() {
    let mut harness = Harness::new(RendererOptions::default());
    let eye = camera("eye", [0.0, 0.6, 1.0], 0.0, -4.0);
    let render = |harness: &mut Harness| {
        let stats = harness.renderer.render_offscreen(&eye, &harness.target);
        (stats, harness.target.read_rgba(&harness.gpu))
    };
    harness.apply(scene());
    let (empty, _) = render(&mut harness);
    assert_eq!(empty.sprite_candidates, 0, "no sprites, nothing examined");

    let mut ops = atlas_ops(&mut harness.resources);
    ops.push(create(
        10,
        sprite(0, [-1.2, 0.4, -4.0], BillboardMode::Spherical),
    ));
    // A sprite under a group-like parent, removed with its subtree below.
    ops.push(instance(
        20,
        None,
        "floor",
        transform([0.0, -5.0, -40.0], 0.0, 0.1),
    ));
    ops.push(RenderDiff::CreateSprite {
        handle: RenderHandle::new(21),
        parent: Some(RenderHandle::new(20)),
        sprite: sprite(1, [12.0, 5.4, 36.0], BillboardMode::Spherical),
    });
    harness.apply(ops);
    let (few, few_pixels) = render(&mut harness);
    assert!(few.sprite_candidates > 0);

    // 500 unrelated static nodes behind the camera: the image and the sprite
    // work stay the same.
    let mut crowd = coloured_mesh("crowd", cube(), [0.8, 0.2, 0.2, 1.0]);
    crowd.extend((0..500).map(|index| {
        instance(
            1_000 + index,
            None,
            "crowd",
            transform(
                [index as f32 % 25.0, 0.0, 20.0 + (index / 25) as f32],
                0.0,
                0.2,
            ),
        )
    }));
    harness.apply(crowd);
    let (many, many_pixels) = render(&mut harness);
    assert_eq!(many.sprite_candidates, few.sprite_candidates);
    assert_eq!(many_pixels, few_pixels);

    // Destroying the parent removes its sprite from discovery.
    harness.apply(vec![RenderDiff::Destroy {
        handle: RenderHandle::new(20),
    }]);
    let (fewer, _) = render(&mut harness);
    assert_eq!(fewer.sprite_candidates * 2, few.sprite_candidates);
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
                shadow: Default::default(),
            },
        ),
        (
            31,
            LightDescriptor::Ambient {
                enabled: true,
                color: [0.4, 0.45, 0.6],
                intensity: 0.4,
                shadow_intent: LightShadowIntent::Disabled,
                shadow: Default::default(),
                range: None,
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

/// A synthetic dome spans the sprite's atlas frame: a sprite drawn from one
/// cell of a sheet shades as the same image drawn as a whole texture.
#[test]
fn synthetic_sprite_domes_span_the_atlas_frame() {
    let render = |whole: bool| {
        let mut harness = Harness::new(RendererOptions {
            default_world_lights: false,
            ..RendererOptions::default()
        });
        let mut ops = scene();
        if whole {
            // Frame 0 of the sheet, alone.
            let red: Vec<u8> = atlas_image()
                .as_chunks::<4>()
                .0
                .iter()
                .enumerate()
                .filter(|(index, _)| index % 16 < 8)
                .flat_map(|(_, pixel)| pixel.to_vec())
                .collect();
            let (texture, _) =
                harness
                    .resources
                    .texture("texture/atlas", 8, 8, &red, TextureFilter::Nearest);
            ops.push(texture);
            ops.push(RenderDiff::DefineSpriteAtlas {
                atlas: SpriteAtlasDescriptor {
                    id: "sprite/atlas".to_owned(),
                    texture: "texture/atlas".to_owned(),
                    frames: vec![SpriteFrameRect {
                        frame: 0,
                        uv_min: [0.0, 0.0],
                        uv_max: [1.0, 1.0],
                        size: None,
                    }],
                },
            });
        } else {
            ops.extend(atlas_ops(&mut harness.resources));
        }
        let mut lit = sprite(0, [0.0, 0.4, -4.0], BillboardMode::Spherical);
        lit.size = [1.3, 1.3];
        lit.material.lighting = SpriteLightingMode::Synthetic;
        lit.material.normal_strength = 1.5;
        ops.push(create(20, lit));
        ops.push(RenderDiff::CreateLight {
            handle: RenderHandle::new(30),
            parent: None,
            light: LightDescriptor::Point {
                enabled: true,
                color: [1.0, 0.85, 0.6],
                intensity: 18.0,
                position: [-1.5, 2.0, -2.5],
                range: None,
                decay: 2.0,
                shadow_intent: LightShadowIntent::Disabled,
                shadow: Default::default(),
            },
        });
        harness.apply(ops);
        harness.single(&camera("eye", [0.0, 0.6, 1.0], 0.0, -4.0))
    };
    let (cell, whole) = (render(false), render(true));
    let differing = cell
        .as_chunks::<4>()
        .0
        .iter()
        .zip(whole.as_chunks::<4>().0)
        .filter(|(a, b)| a.iter().zip(b.iter()).any(|(a, b)| a.abs_diff(*b) > 2))
        .count();
    assert_eq!(
        differing, 0,
        "an atlas cell and a whole texture shade differently"
    );
}

fn particle_frame(ops: Vec<ParticleProjectionOp>) -> PresentationFrameDiff {
    PresentationFrameDiff::from_ops(
        ops.into_iter()
            .enumerate()
            .map(|(sequence, op)| PresentationOp::Particle {
                meta: PresentationOpMeta::new(sequence as u32),
                op,
            })
            .collect(),
    )
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
        size_mode: Default::default(),
        blend: Default::default(),
        softness_metres: 0.0,
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
        backdrop: false,
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

/// Rows of `rgba` holding a lit (non-background) pixel.
fn covered_rows(rgba: &[u8]) -> usize {
    rgba.as_chunks::<{ WIDTH as usize * 4 }>()
        .0
        .iter()
        .filter(|row| row.as_chunks::<4>().0.iter().any(|pixel| pixel[0] > 128))
        .count()
}

/// A world-size billboard particle is an edge length in metres: 0.5 covers half
/// the height of a 1-unit sprite at the same distance, near or far. A screen
/// size keeps its pixels at every distance.
#[test]
fn world_size_particles_shrink_with_distance_like_sprites() {
    let white = vec![255; 8 * 8 * 4];
    let height = |distance: f32, what: &str| {
        let mut harness = Harness::new(RendererOptions::default());
        let mut ops = vec![RenderDiff::SetBackgroundColor {
            color: [0.0, 0.0, 0.0, 1.0],
        }];
        let (texture, hash) =
            harness
                .resources
                .texture("texture/white", 8, 8, &white, TextureFilter::Nearest);
        ops.push(texture);
        if what == "sprite" {
            ops.push(RenderDiff::DefineSpriteAtlas {
                atlas: SpriteAtlasDescriptor {
                    id: "sprite/atlas".to_owned(),
                    texture: "texture/white".to_owned(),
                    frames: vec![SpriteFrameRect {
                        frame: 0,
                        uv_min: [0.0, 0.0],
                        uv_max: [1.0, 1.0],
                        size: None,
                    }],
                },
            });
            ops.push(create(
                20,
                sprite(0, [0.0, 0.0, -distance], BillboardMode::Spherical),
            ));
        }
        harness.apply(ops);
        if what != "sprite" {
            let mut still = emitter(
                ParticleVisual::Billboard {
                    sprite: ParticleSpriteRef {
                        asset: "texture/white".to_owned(),
                        content_hash: hash,
                        frame_count: 1,
                    },
                },
                [0.0, 0.0, -distance],
                1,
                5,
            );
            still.size_mode = if what == "world" {
                ParticleSizeMode::World
            } else {
                ParticleSizeMode::Screen
            };
            still.velocity_min = [0.0; 3];
            still.velocity_max = [0.0; 3];
            still.acceleration = [0.0; 3];
            still.lifetime_seconds = [10.0, 10.0];
            for key in &mut still.size_curve {
                key.value = 0.5;
            }
            for key in &mut still.color_curve {
                key.color = [1.0; 4];
            }
            let issues = harness.renderer.apply_presentation(
                &particle_frame(vec![ParticleProjectionOp::Emit {
                    signal_id: "still".to_owned(),
                    descriptor: still,
                }]),
                &harness.resources,
                NO_ENTITIES,
            );
            assert!(issues.is_empty(), "{issues:?}");
            harness.renderer.advance_effects(0.01, NO_ENTITIES);
        }
        covered_rows(&harness.single(&camera("eye", [0.0, 0.0, 0.0], 0.0, 0.0)))
    };
    for distance in [3.0, 8.0] {
        let (sprite, particle) = (height(distance, "sprite"), height(distance, "world"));
        assert!(
            sprite >= 19,
            "a 1 m sprite at {distance} m covers {sprite} rows"
        );
        assert!(
            (sprite as f32 / 2.0 - particle as f32).abs() <= 1.0,
            "at {distance} m a 1 m sprite covers {sprite} rows and a 0.5 m particle {particle}"
        );
    }
    assert_eq!(height(3.0, "screen"), height(8.0, "screen"));
}

/// One still 2 m world-size billboard of a flat colour at (0, 0, -4), over
/// an 8 m floor at y = 0 (so its lower half is under the floor) and, with
/// `wall`, in front of a lit grey wall. Returns the frame's rgba.
fn still_particle(
    samples: u32,
    wall: bool,
    color: [f32; 4],
    blend: ParticleBlendMode,
    softness_metres: f32,
) -> Vec<u8> {
    still_particle_by(samples, wall, false, color, blend, softness_metres)
}

/// `still_particle` with, when `post` is set, a slanted post standing in
/// front of the billboard, so pixels along its edge cover the world's
/// samples unevenly.
fn still_particle_by(
    samples: u32,
    wall: bool,
    post: bool,
    color: [f32; 4],
    blend: ParticleBlendMode,
    softness_metres: f32,
) -> Vec<u8> {
    let mut harness = Harness::new(RendererOptions::default());
    harness.target = OffscreenTarget::new(&harness.gpu, WIDTH, HEIGHT, samples);
    let mut ops = vec![RenderDiff::SetBackgroundColor {
        color: [0.0, 0.0, 0.0, 1.0],
    }];
    ops.extend(coloured_mesh("floor", floor(8.0), [0.5, 0.5, 0.5, 1.0]));
    ops.push(instance(
        1,
        None,
        "floor",
        transform([0.0, 0.0, -4.0], 0.0, 1.0),
    ));
    if wall {
        ops.extend(coloured_mesh("wall", cube(), [0.5, 0.5, 0.5, 1.0]));
        ops.push(instance(
            2,
            None,
            "wall",
            transform([0.0, 0.0, -9.0], 0.0, 6.0),
        ));
    }
    if post {
        ops.extend(coloured_mesh("post", cube(), [0.3, 0.3, 0.3, 1.0]));
        ops.push(instance(
            3,
            None,
            "post",
            transform([0.7, 0.9, -2.6], 25.0, 1.1),
        ));
    }
    let white = vec![255; 8 * 8 * 4];
    let (texture, hash) =
        harness
            .resources
            .texture("texture/white", 8, 8, &white, TextureFilter::Nearest);
    ops.push(texture);
    harness.apply(ops);
    let mut still = emitter(
        ParticleVisual::Billboard {
            sprite: ParticleSpriteRef {
                asset: "texture/white".to_owned(),
                content_hash: hash,
                frame_count: 1,
            },
        },
        [0.0, 0.0, -4.0],
        1,
        5,
    );
    still.size_mode = ParticleSizeMode::World;
    still.blend = blend;
    still.softness_metres = softness_metres;
    still.velocity_min = [0.0; 3];
    still.velocity_max = [0.0; 3];
    still.acceleration = [0.0; 3];
    still.lifetime_seconds = [10.0, 10.0];
    for key in &mut still.size_curve {
        key.value = 2.0;
    }
    for key in &mut still.color_curve {
        key.color = color;
    }
    let issues = harness.renderer.apply_presentation(
        &particle_frame(vec![ParticleProjectionOp::Emit {
            signal_id: "still".to_owned(),
            descriptor: still,
        }]),
        &harness.resources,
        NO_ENTITIES,
    );
    assert!(issues.is_empty(), "{issues:?}");
    harness.renderer.advance_effects(0.01, NO_ENTITIES);
    harness.single(&camera("eye", [0.0, 1.0, 0.0], 0.0, -14.0))
}

/// A soft billboard fades out as it nears the floor behind it, over its
/// softness in metres: where a hard one meets the floor in one edge row, a
/// soft one grades over many, and both stay hidden under the floor. Both
/// the single-sample and the multisampled depth paths.
#[test]
fn soft_particles_fade_into_the_scene_they_meet() {
    for samples in [1, 4] {
        let centre = |rgba: &[u8]| -> Vec<u8> {
            (0..HEIGHT)
                .map(|y| pixel(rgba, WIDTH, WIDTH / 2, y)[0])
                .collect()
        };
        let white = [1.0, 1.0, 1.0, 1.0];
        let hard = centre(&still_particle(
            samples,
            false,
            white,
            ParticleBlendMode::Alpha,
            0.0,
        ));
        let soft = centre(&still_particle(
            samples,
            false,
            white,
            ParticleBlendMode::Alpha,
            2.0,
        ));
        // The floor's own brightness, at the frame's bottom row.
        let level = *hard.last().expect("a frame has rows");
        // Rows of neither the particle's white nor the floor's grey.
        let graded = |column: &[u8]| {
            column
                .iter()
                .filter(|&&red| red > level + 6 && red < 250)
                .count()
        };
        assert!(
            graded(&hard) <= 2,
            "{samples} samples: a hard particle meets the floor in {} graded rows",
            graded(&hard)
        );
        assert!(
            graded(&soft) >= 6,
            "{samples} samples: a soft particle fades over {} rows",
            graded(&soft)
        );
        let top = |column: &[u8]| column.iter().position(|&red| red >= 250);
        assert_eq!(
            top(&hard),
            top(&soft),
            "{samples} samples: far from the floor the particle is as bright as a hard one"
        );
        let floor_rows = |column: &[u8]| {
            column
                .iter()
                .rev()
                .take_while(|&&red| red <= level + 3)
                .count()
        };
        assert!(
            floor_rows(&hard) > 20 && floor_rows(&soft) >= floor_rows(&hard),
            "{samples} samples: the floor hides the particle under it ({} and {} rows)",
            floor_rows(&hard),
            floor_rows(&soft)
        );
    }
}

/// An additive billboard adds its colour to the lit wall behind it, where an
/// alpha one of the same opaque colour replaces it, and over nothing lit the
/// two are the same; a faded additive particle adds nothing.
/// Particle colour keys are linear, as every other world colour is: a key
/// of 0.5 draws as linear 0.5 (188 once the frame is sRGB-encoded), not as
/// an sRGB 0.5 (128).
#[test]
fn particle_colour_keys_are_linear() {
    let at = |rgba: &[u8]| pixel(rgba, WIDTH, WIDTH / 2, HEIGHT / 2 - 20);
    let drawn = at(&still_particle(
        4,
        false,
        [0.5, 0.5, 0.5, 1.0],
        ParticleBlendMode::Alpha,
        0.0,
    ));
    for channel in &drawn[..3] {
        assert!(channel.abs_diff(188) <= 3, "linear 0.5 draws as {drawn:?}");
    }
}

#[test]
fn additive_particles_add_to_the_scene_behind_them() {
    let grey = [0.4, 0.4, 0.4, 1.0];
    let at = |rgba: &[u8]| pixel(rgba, WIDTH, WIDTH / 2, HEIGHT / 2 - 20)[0];
    let alpha = at(&still_particle(
        4,
        true,
        grey,
        ParticleBlendMode::Alpha,
        0.0,
    ));
    let additive = at(&still_particle(
        4,
        true,
        grey,
        ParticleBlendMode::Additive,
        0.0,
    ));
    let unlit = at(&still_particle(
        4,
        false,
        grey,
        ParticleBlendMode::Alpha,
        0.0,
    ));
    let wall = at(&still_particle(
        4,
        true,
        [0.0, 0.0, 0.0, 0.0],
        ParticleBlendMode::Alpha,
        0.0,
    ));
    assert!(wall > 40, "the wall is lit: {wall}");
    assert!(
        alpha.abs_diff(unlit) <= 2,
        "an alpha particle covers the wall: {alpha} over it, {unlit} over nothing"
    );
    assert!(
        additive > alpha + 20 && additive > wall + 20,
        "an additive particle brightens the wall: {additive} over its {wall}, alpha {alpha}"
    );
    let faded = at(&still_particle(
        4,
        true,
        [0.4, 0.4, 0.4, 0.0],
        ParticleBlendMode::Additive,
        0.0,
    ));
    assert_eq!(faded, wall, "a faded additive particle adds nothing");
}

#[test]
fn particle_textures_are_released_when_their_last_emitter_and_particle_leave_or_are_replaced() {
    let mut harness = Harness::new(RendererOptions::default());
    let mut ops = scene();
    // Three distinct images, so three content hashes.
    let mut visual = |id: &str, tint: u8| {
        let mut image = atlas_image();
        image[0] = tint;
        let (op, hash) = harness
            .resources
            .texture(id, 16, 8, &image, TextureFilter::Nearest);
        ops.push(op);
        ParticleVisual::Billboard {
            sprite: ParticleSpriteRef {
                asset: id.to_owned(),
                content_hash: hash,
                frame_count: 2,
            },
        }
    };
    let (a, b, c) = (
        visual("texture/a", 10),
        visual("texture/b", 20),
        visual("texture/c", 30),
    );
    harness.apply(ops);
    let handle = ParticleEmitterHandle::new(1);
    let mut fountain = emitter(b, [1.2, -0.5, -4.5], 0, 3);
    fountain.rate_per_second = 40.0;
    let frame = |ops| particle_frame(ops);
    let present = |harness: &mut Harness, ops: Vec<ParticleProjectionOp>| {
        let issues =
            harness
                .renderer
                .apply_presentation(&frame(ops), &harness.resources, NO_ENTITIES);
        assert!(issues.is_empty(), "{issues:?}");
    };
    present(
        &mut harness,
        vec![
            ParticleProjectionOp::Emit {
                signal_id: "impact".to_owned(),
                descriptor: emitter(a.clone(), [-1.2, -0.3, -4.0], 48, 7),
            },
            ParticleProjectionOp::Create {
                handle,
                descriptor: fountain,
            },
        ],
    );
    for _ in 0..4 {
        harness.renderer.advance_effects(1.0 / 20.0, NO_ENTITIES);
    }
    assert_eq!(harness.renderer.particle_texture_count(), 2);

    // A new visual: particles already alive keep drawing texture B.
    present(
        &mut harness,
        vec![ParticleProjectionOp::Update {
            handle,
            patch: ParticleEmitterPatch {
                visual: Some(c),
                ..ParticleEmitterPatch::default()
            },
        }],
    );
    assert_eq!(harness.renderer.particle_texture_count(), 3);
    harness.renderer.advance_effects(1.0 / 20.0, NO_ENTITIES);
    assert_eq!(harness.renderer.particle_texture_count(), 3);

    // The burst and B's particles age out; the fountain keeps C and renders.
    for _ in 0..60 {
        harness.renderer.advance_effects(1.0 / 20.0, NO_ENTITIES);
    }
    assert_eq!(harness.renderer.particle_texture_count(), 1);
    let eye = camera("eye", [0.0, 1.0, 1.5], 0.0, -10.0);
    let fountain_only = harness.single(&eye);
    let fountain_particles = harness.renderer.particle_counts().0;
    assert!(fountain_particles > 0);

    // Creating the live handle again replaces the fountain: its particles
    // leave with it and C is released; the replacement bursts with A.
    present(
        &mut harness,
        vec![ParticleProjectionOp::Create {
            handle,
            descriptor: emitter(a.clone(), [1.2, -0.5, -4.5], 5, 3),
        }],
    );
    assert_eq!(harness.renderer.particle_counts(), (5, 1));
    assert_eq!(harness.renderer.particle_texture_count(), 1);

    // Destroying the emitter removes its particles, and with them A.
    present(&mut harness, vec![ParticleProjectionOp::Destroy { handle }]);
    assert_eq!(harness.renderer.particle_texture_count(), 0);
    assert_eq!(harness.renderer.particle_counts(), (0, 0));
    assert_ne!(harness.single(&eye), fountain_only);

    // A later burst loads its texture again into a reused slot.
    present(
        &mut harness,
        vec![ParticleProjectionOp::Emit {
            signal_id: "impact".to_owned(),
            descriptor: emitter(a, [-1.2, -0.3, -4.0], 8, 7),
        }],
    );
    assert_eq!(harness.renderer.particle_texture_count(), 1);
    for _ in 0..60 {
        harness.renderer.advance_effects(1.0 / 20.0, NO_ENTITIES);
    }
    assert_eq!(harness.renderer.particle_texture_count(), 0);
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
            viewport_anchor: None,
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

/// Blended sprites and blended meshes share one order: render order, then
/// back to front (#8787 review). A red half-transparent sprite behind blue
/// glass shows blue-dominant, in front of it red-dominant; a positive render
/// order draws it after the glass even from behind.
#[test]
fn blended_sprites_and_blended_meshes_share_one_back_to_front_order() {
    let mut harness = Harness::new(RendererOptions {
        default_world_lights: false,
        ..RendererOptions::default()
    });
    let (texture, _) =
        harness
            .resources
            .texture("texture/white", 1, 1, &[255; 4], TextureFilter::Nearest);
    let mut ops = vec![
        RenderDiff::SetBackgroundColor {
            color: [0.0, 0.0, 0.0, 1.0],
        },
        texture,
        RenderDiff::DefineSpriteAtlas {
            atlas: SpriteAtlasDescriptor {
                id: "sprite/white".into(),
                texture: "texture/white".into(),
                frames: vec![SpriteFrameRect {
                    frame: 0,
                    uv_min: [0.0; 2],
                    uv_max: [1.0; 2],
                    size: None,
                }],
            },
        },
    ];
    let plane = payload(
        vec![
            -1.0, -1.0, 0.0, 1.0, -1.0, 0.0, 1.0, 1.0, 0.0, -1.0, 1.0, 0.0,
        ],
        [0.0, 0.0, 1.0].repeat(4),
        vec![0, 1, 2, 0, 2, 3],
    );
    let mut glass = coloured_mesh("glass", plane, [0.0, 0.0, 1.0, 0.5]);
    if let RenderDiff::DefineMaterial { material } = &mut glass[0] {
        material.alpha_mode = MaterialAlphaModeDescriptor::Blend;
        material.emission_color = [0.0, 0.0, 1.0];
        material.emission_intensity = 1.0;
    }
    ops.extend(glass);
    ops.push(instance(
        1,
        None,
        "glass",
        transform([0.0, 0.0, -3.0], 0.0, 1.0),
    ));
    let mut red = sprite(0, [0.0, 0.0, -4.0], BillboardMode::None);
    red.asset = "sprite/white".into();
    red.size = [2.0; 2];
    red.tint = [1.0, 0.0, 0.0, 0.5];
    red.material.alpha = SpriteAlphaMode::Blend;
    ops.push(create(2, red));
    harness.apply(ops);
    let eye = camera("eye", [0.0; 3], 0.0, 0.0);
    let centre = |harness: &mut Harness| pixel(&harness.single(&eye), WIDTH, WIDTH / 2, HEIGHT / 2);

    let behind = centre(&mut harness);
    assert!(behind[2] > behind[0], "sprite behind the glass: {behind:?}");

    harness.apply(vec![RenderDiff::UpdateSprite {
        handle: RenderHandle::new(2),
        frame: None,
        tint: None,
        render_order: Some(1),
        visible: None,
        emission_intensity: None,
    }]);
    let ordered = centre(&mut harness);
    assert!(
        ordered[0] > ordered[2],
        "render order 1 draws after the glass: {ordered:?}"
    );

    harness.apply(vec![
        RenderDiff::UpdateSprite {
            handle: RenderHandle::new(2),
            frame: None,
            tint: None,
            render_order: Some(0),
            visible: None,
            emission_intensity: None,
        },
        RenderDiff::Update {
            handle: RenderHandle::new(2),
            transform: Some(transform([0.0, 0.0, -2.0], 0.0, 1.0)),
            material: None,
            visible: None,
            metadata: None,
        },
    ]);
    let front = centre(&mut harness);
    assert!(
        front[0] > front[2],
        "sprite in front of the glass: {front:?}"
    );
}

/// A device pixel ratio of 2 (#8853): a pixel-sized sprite covers twice the
/// target pixels in a target twice the size, keeping its CSS size.
#[test]
fn pixel_sized_sprites_keep_their_css_size_at_device_pixel_ratio_two() {
    let render = |ratio: u32| {
        let mut harness = Harness::new(RendererOptions::default());
        harness.target = OffscreenTarget::new(&harness.gpu, WIDTH * ratio, HEIGHT * ratio, 4);
        harness.renderer.set_pixel_ratio(ratio as f32);
        let mut ops = vec![RenderDiff::SetBackgroundColor {
            color: [0.0, 0.0, 0.0, 1.0],
        }];
        ops.extend(atlas_ops(&mut harness.resources));
        let mut pixel_sized = sprite(1, [0.0, 1.0, -9.0], BillboardMode::Spherical);
        pixel_sized.size_mode = SpriteSizeMode::Pixel;
        pixel_sized.size = [24.0, 24.0];
        ops.push(create(13, pixel_sized));
        harness.apply(ops);
        let pixels = harness.single(&camera("eye", [0.0, 1.0, 1.0], 0.0, 0.0));
        // Columns and rows the sprite covers.
        let (width, height) = (WIDTH * ratio, HEIGHT * ratio);
        let covered = |horizontal: bool| {
            let (outer, inner) = if horizontal {
                (width, height)
            } else {
                (height, width)
            };
            (0..outer)
                .filter(|&a| {
                    (0..inner).any(|b| {
                        let (x, y) = if horizontal { (a, b) } else { (b, a) };
                        pixel(&pixels, width, x, y)[..3]
                            .iter()
                            .any(|channel| *channel > 16)
                    })
                })
                .count() as i64
        };
        (covered(true), covered(false))
    };
    let (one_w, one_h) = render(1);
    let (two_w, two_h) = render(2);
    assert!(one_w > 10 && one_h > 10, "{one_w}x{one_h}");
    assert!((two_w - 2 * one_w).abs() <= 2, "{one_w} -> {two_w}");
    assert!((two_h - 2 * one_h).abs() <= 2, "{one_h} -> {two_h}");
}

/// Under multisampling a soft billboard decides per scene sample which samples
/// it covers, as the world pass's depth test would: with a negligible
/// softness it matches the hard billboard pixel for pixel along a slanted
/// post's edge, where the post covers some of a pixel's samples and not
/// others. Testing one sample for the whole pixel would leak the billboard
/// over the post, or lose it, along that edge.
#[test]
fn multisampled_soft_particles_cover_the_samples_the_world_leaves() {
    let white = [1.0, 1.0, 1.0, 1.0];
    let hard = still_particle_by(4, false, true, white, ParticleBlendMode::Alpha, 0.0);
    let soft = still_particle_by(4, false, true, white, ParticleBlendMode::Alpha, 0.001);
    // The post's edge crosses the billboard: pixels of neither the billboard's
    // white nor the post's grey are the mixed-coverage ones this test is about.
    let mixed = (0..WIDTH * HEIGHT)
        .filter(|index| {
            let red = hard[(index * 4) as usize];
            red > 90 && red < 240
        })
        .count();
    assert!(
        mixed >= 20,
        "the post's edge should cross the billboard: {mixed} mixed pixels"
    );
    let worst = hard
        .chunks(4)
        .zip(soft.chunks(4))
        .map(|(a, b)| (i32::from(a[0]) - i32::from(b[0])).abs())
        .max()
        .unwrap_or(0);
    assert!(
        worst <= 12,
        "a soft billboard with negligible softness differs from the hard one by {worst}/255"
    );
}

/// A still 2 m white sprite 4 m out over the floor, like `still_particle`,
/// with the material's blend and softness.
fn still_sprite(
    samples: u32,
    wall: bool,
    tint: [f32; 4],
    blend: SpriteBlendMode,
    softness_metres: f32,
) -> Vec<u8> {
    let mut harness = Harness::new(RendererOptions::default());
    harness.target = OffscreenTarget::new(&harness.gpu, WIDTH, HEIGHT, samples);
    let (texture, _) =
        harness
            .resources
            .texture("texture/white", 1, 1, &[255; 4], TextureFilter::Nearest);
    let mut ops = vec![
        RenderDiff::SetBackgroundColor {
            color: [0.0, 0.0, 0.0, 1.0],
        },
        texture,
        RenderDiff::DefineSpriteAtlas {
            atlas: SpriteAtlasDescriptor {
                id: "sprite/white".into(),
                texture: "texture/white".into(),
                frames: vec![SpriteFrameRect {
                    frame: 0,
                    uv_min: [0.0; 2],
                    uv_max: [1.0; 2],
                    size: None,
                }],
            },
        },
    ];
    ops.extend(coloured_mesh("floor", floor(8.0), [0.5, 0.5, 0.5, 1.0]));
    ops.push(instance(
        1,
        None,
        "floor",
        transform([0.0, 0.0, -4.0], 0.0, 1.0),
    ));
    if wall {
        ops.extend(coloured_mesh("wall", cube(), [0.5, 0.5, 0.5, 1.0]));
        ops.push(instance(
            2,
            None,
            "wall",
            transform([0.0, 0.0, -9.0], 0.0, 6.0),
        ));
    }
    let mut still = sprite(0, [0.0, 0.0, -4.0], BillboardMode::Spherical);
    still.asset = "sprite/white".into();
    still.size = [2.0; 2];
    still.tint = tint;
    still.material.alpha = SpriteAlphaMode::Blend;
    still.material.blend = blend;
    still.material.softness_metres = softness_metres;
    ops.push(create(3, still));
    harness.apply(ops);
    harness.single(&camera("eye", [0.0, 1.0, 0.0], 0.0, -14.0))
}

/// A blended sprite with a softness fades into the floor behind it as a
/// soft billboard does, where a hard one meets it in one row, under both
/// depth sample counts; without a softness it draws exactly as before.
#[test]
fn soft_sprites_fade_into_the_scene_they_meet() {
    for samples in [1, 4] {
        let centre = |rgba: &[u8]| -> Vec<u8> {
            (0..HEIGHT)
                .map(|y| pixel(rgba, WIDTH, WIDTH / 2, y)[0])
                .collect()
        };
        let white = [1.0, 1.0, 1.0, 1.0];
        let hard = centre(&still_sprite(
            samples,
            false,
            white,
            SpriteBlendMode::Alpha,
            0.0,
        ));
        let soft = centre(&still_sprite(
            samples,
            false,
            white,
            SpriteBlendMode::Alpha,
            2.0,
        ));
        let level = *hard.last().expect("a frame has rows");
        let graded = |column: &[u8]| {
            column
                .iter()
                .filter(|&&red| red > level + 6 && red < 250)
                .count()
        };
        assert!(
            graded(&hard) <= 2,
            "{samples}x: hard sprite grades over {} rows",
            graded(&hard)
        );
        assert!(
            graded(&soft) >= 6,
            "{samples}x: soft sprite grades over {} rows",
            graded(&soft)
        );
        let top = |column: &[u8]| column.iter().copied().max().unwrap_or(0);
        assert!(
            top(&soft) >= 250 && top(&hard) >= 250,
            "{samples}x: both sprites reach white away from the floor"
        );
        assert!(
            hard[HEIGHT as usize - 1] == soft[HEIGHT as usize - 1],
            "{samples}x: the floor still hides both at the frame's bottom"
        );
    }
}

/// An additive sprite brightens the wall behind it where an alpha one of the
/// same colour replaces it, and over nothing the two agree.
#[test]
fn additive_sprites_add_to_the_scene_behind_them() {
    let grey = [0.4, 0.4, 0.4, 1.0];
    let at = |rgba: &[u8]| pixel(rgba, WIDTH, WIDTH / 2, HEIGHT / 2 - 20)[0];
    let alpha = at(&still_sprite(4, true, grey, SpriteBlendMode::Alpha, 0.0));
    let additive = at(&still_sprite(4, true, grey, SpriteBlendMode::Additive, 0.0));
    let wall = at(&still_sprite(
        4,
        true,
        [0.0, 0.0, 0.0, 0.0],
        SpriteBlendMode::Alpha,
        0.0,
    ));
    assert!(
        additive > wall + 20,
        "additive sprite brightens the wall: {additive} over {wall}"
    );
    assert!(
        additive > alpha + 10,
        "additive sprite is brighter than the alpha one it replaces: {additive} vs {alpha}"
    );
}

#[test]
fn a_sprite_batch_draws_what_its_sprites_would_in_one_draw() {
    // A grid of masked sprites of both frames and three scales, drawn as
    // single sprites and as one batch whose node is moved and scaled.
    let node = [0.4, -0.2, -1.0];
    let node_scale = 1.5;
    let instances: Vec<SpriteBatchInstance> = (0..48)
        .map(|index| SpriteBatchInstance {
            position: [
                (index % 8) as f32 * 0.6 - 2.1,
                (index / 8) as f32 * 0.35 - 0.6,
                -3.0 - (index % 5) as f32 * 0.7,
            ],
            scale: [0.3, 0.45, 0.6][index % 3],
            frame: (index % 2) as u32,
        })
        .collect();
    let eye = camera("eye", [0.3, 1.2, 1.5], 12.0, -12.0);
    for billboard in [
        BillboardMode::Spherical,
        BillboardMode::Cylindrical,
        BillboardMode::None,
    ] {
        let mut shared = sprite(0, node, billboard);
        shared.pivot = [0.5, 0.0];
        shared.material.alpha = SpriteAlphaMode::Mask { cutoff: 0.5 };

        let mut singles = Harness::new(RendererOptions::default());
        let mut ops = scene();
        ops.extend(atlas_ops(&mut singles.resources));
        ops.extend(instances.iter().enumerate().map(|(index, instance)| {
            let mut single = shared.clone();
            single.frame = instance.frame;
            single.size = [instance.scale * node_scale; 2];
            single.transform = transform(
                std::array::from_fn(|axis| node[axis] + instance.position[axis] * node_scale),
                0.0,
                1.0,
            );
            create(100 + index as u64, single)
        }));
        singles.apply(ops);
        let single_stats = singles.renderer.render_offscreen(&eye, &singles.target);
        let expected = singles.target.read_rgba(&singles.gpu);

        let mut batched = Harness::new(RendererOptions::default());
        let mut ops = scene();
        ops.extend(atlas_ops(&mut batched.resources));
        let mut template = shared.clone();
        template.transform = transform(node, 0.0, node_scale);
        ops.push(RenderDiff::CreateSpriteBatch {
            handle: RenderHandle::new(100),
            parent: None,
            batch: SpriteBatchDescriptor {
                sprite: template,
                instances: instances.clone().into(),
            },
        });
        batched.apply(ops);
        let stats = batched.renderer.render_offscreen(&eye, &batched.target);
        let pixels = batched.target.read_rgba(&batched.gpu);
        assert_eq!(
            stats.sprite_candidates * 48,
            single_stats.sprite_candidates,
            "{billboard:?}: one node"
        );
        assert_eq!(
            stats.draws + 47,
            single_stats.draws,
            "{billboard:?}: one draw"
        );
        assert_eq!(stats.sprite_batch_instances, 48, "{billboard:?}");
        // The same quads to within rounding at their edges.
        let differing = expected
            .as_chunks::<4>()
            .0
            .iter()
            .zip(pixels.as_chunks::<4>().0)
            .filter(|(a, b)| a.iter().zip(b.iter()).any(|(a, b)| a.abs_diff(*b) > 8))
            .count();
        let drawn = expected
            .as_chunks::<4>()
            .0
            .iter()
            .zip(scene_only(&eye).as_chunks::<4>().0)
            .filter(|(a, b)| a != b)
            .count();
        assert!(
            drawn > 2_000,
            "{billboard:?}: the sprites show ({drawn} pixels)"
        );
        assert!(
            differing * 100 < drawn,
            "{billboard:?}: {differing} of {drawn} sprite pixels differ"
        );

        // A tint change rewrites the shared values, not the rows.
        batched.apply(vec![RenderDiff::UpdateSprite {
            handle: RenderHandle::new(100),
            frame: None,
            tint: Some([0.2, 0.2, 1.0, 1.0]),
            render_order: None,
            visible: None,
            emission_intensity: None,
        }]);
        assert_ne!(batched.single(&eye), pixels, "{billboard:?}: tinted");
        // Hidden, and gone with its node.
        batched.apply(vec![RenderDiff::Destroy {
            handle: RenderHandle::new(100),
        }]);
        assert_eq!(batched.single(&eye), scene_only(&eye), "{billboard:?}");
    }
}

#[test]
fn sprite_batch_buffers_go_with_their_node_when_destroyed_or_replaced() {
    let mut harness = Harness::new(RendererOptions::default());
    let mut ops = scene();
    ops.extend(atlas_ops(&mut harness.resources));
    let batch = |x: f32, count: u32| SpriteBatchDescriptor {
        sprite: sprite(0, [x, 0.0, -4.0], BillboardMode::Spherical),
        instances: (0..count)
            .map(|index| SpriteBatchInstance {
                position: [index as f32 * 0.4, 0.0, 0.0],
                scale: 0.3,
                frame: index % 2,
            })
            .collect(),
    };
    let (group, first, second) = (
        RenderHandle::new(90),
        RenderHandle::new(100),
        RenderHandle::new(101),
    );
    ops.push(RenderDiff::Create {
        handle: group,
        parent: None,
        node: RenderNode::new(Geometry::Group),
    });
    for (handle, x) in [(first, -1.5), (second, 0.5)] {
        ops.push(RenderDiff::CreateSpriteBatch {
            handle,
            parent: Some(group),
            batch: batch(x, 3),
        });
    }
    harness.apply(ops);
    let eye = camera("eye", [0.0, 0.6, 1.0], 0.0, -4.0);
    let both = harness.single(&eye);
    assert_eq!(harness.renderer.sprite_batch_buffer_count(), 2);

    // A batch replaced by a batch: its buffers go at once and the next draw
    // builds the replacement's (four instances, not three).
    harness.apply(vec![RenderDiff::CreateSpriteBatch {
        handle: first,
        parent: Some(group),
        batch: batch(-1.5, 4),
    }]);
    assert_eq!(harness.renderer.sprite_batch_buffer_count(), 1);
    let stats = harness.renderer.render_offscreen(&eye, &harness.target);
    assert_eq!(stats.sprite_batch_instances, 7);
    assert_eq!(harness.renderer.sprite_batch_buffer_count(), 2);
    assert_ne!(harness.target.read_rgba(&harness.gpu), both);

    // Replaced by another kind of node: no buffers, no batch drawn.
    harness.apply(vec![RenderDiff::Create {
        handle: first,
        parent: Some(group),
        node: RenderNode::new(Geometry::Group),
    }]);
    assert_eq!(harness.renderer.sprite_batch_buffer_count(), 1);
    let stats = harness.renderer.render_offscreen(&eye, &harness.target);
    assert_eq!(stats.sprite_batch_instances, 3);

    // Destroying the parent takes the surviving batch's buffers with it.
    harness.apply(vec![RenderDiff::Destroy { handle: group }]);
    assert_eq!(harness.renderer.sprite_batch_buffer_count(), 0);
    assert_eq!(harness.single(&eye), scene_only(&eye));
}

#[test]
fn a_soft_sprite_batch_draws_in_the_particle_pass() {
    let mut harness = Harness::new(RendererOptions::default());
    let mut ops = scene();
    ops.extend(atlas_ops(&mut harness.resources));
    let mut shared = sprite(0, [0.0, 0.0, -4.0], BillboardMode::Spherical);
    shared.tint = [1.0, 1.0, 1.0, 0.8];
    shared.material.softness_metres = 0.5;
    ops.push(RenderDiff::CreateSpriteBatch {
        handle: RenderHandle::new(100),
        parent: None,
        batch: SpriteBatchDescriptor {
            sprite: shared,
            instances: (0..6)
                .map(|index| SpriteBatchInstance {
                    position: [index as f32 * 0.5 - 1.25, 0.0, 0.0],
                    scale: 0.6,
                    frame: index % 2,
                })
                .collect(),
        },
    });
    harness.apply(ops);
    let eye = camera("eye", [0.0, 0.6, 1.0], 0.0, -4.0);
    let stats = harness.renderer.render_offscreen(&eye, &harness.target);
    assert_eq!(stats.sprite_batch_instances, 6);
    assert_ne!(harness.target.read_rgba(&harness.gpu), scene_only(&eye));
}

fn scene_only(eye: &render_host_contracts::RendererCompositionCamera) -> Vec<u8> {
    let mut harness = Harness::new(RendererOptions::default());
    let mut ops = scene();
    ops.extend(atlas_ops(&mut harness.resources));
    harness.apply(ops);
    harness.single(eye)
}

/// A 2 m grey sprite straight ahead, tinted a bright red, drawn alone or as
/// a one-sprite batch, with or without a tint mask opaque over its left half.
fn masked_tint_sprite(masked: bool, tint: [f32; 4], batch: bool) -> Vec<u8> {
    let mut harness = Harness::new(RendererOptions::default());
    let grey = [128, 128, 128, 255];
    let (texture, _) = harness.resources.texture(
        "texture/grey",
        2,
        1,
        &[grey, grey].concat(),
        TextureFilter::Nearest,
    );
    let (mask, _) = harness.resources.texture(
        "texture/tint-mask",
        2,
        1,
        &[[255, 255, 255, 255], [255, 255, 255, 0]].concat(),
        TextureFilter::Nearest,
    );
    let mut ops = vec![
        RenderDiff::SetBackgroundColor {
            color: [0.0, 0.0, 0.0, 1.0],
        },
        texture,
        mask,
        RenderDiff::DefineSpriteAtlas {
            atlas: SpriteAtlasDescriptor {
                id: "sprite/grey".into(),
                texture: "texture/grey".into(),
                frames: vec![SpriteFrameRect {
                    frame: 0,
                    uv_min: [0.0; 2],
                    uv_max: [1.0; 2],
                    size: None,
                }],
            },
        },
    ];
    let mut shared = sprite(0, [0.0, 1.0, -4.0], BillboardMode::Spherical);
    shared.asset = "sprite/grey".into();
    shared.tint = tint;
    shared.material.alpha = SpriteAlphaMode::Opaque;
    shared.material.tint_mask = masked.then(|| "texture/tint-mask".into());
    if batch {
        ops.push(RenderDiff::CreateSpriteBatch {
            handle: RenderHandle::new(3),
            parent: None,
            batch: SpriteBatchDescriptor {
                sprite: shared,
                instances: vec![SpriteBatchInstance {
                    position: [0.0; 3],
                    scale: 2.0,
                    frame: 0,
                }]
                .into(),
            },
        });
    } else {
        shared.size = [2.0; 2];
        ops.push(create(3, shared));
    }
    harness.apply(ops);
    harness.single(&camera("eye", [0.0, 1.0, 0.0], 0.0, 0.0))
}

/// A tint mask recolours only where it is opaque: the masked half draws as
/// the whole sprite tinted without a mask does, the clear half as the
/// untinted sprite, for single sprites and batches alike. The tint's colour
/// may exceed 1 (a brightness folded in) and is not clamped before it
/// multiplies.
#[test]
fn a_tint_mask_recolours_only_where_it_is_opaque() {
    let red = [1.3, 0.26, 0.26, 1.0];
    let left = |rgba: &[u8]| pixel(rgba, WIDTH, WIDTH / 2 - 20, HEIGHT / 2);
    let right = |rgba: &[u8]| pixel(rgba, WIDTH, WIDTH / 2 + 20, HEIGHT / 2);
    for batch in [false, true] {
        let untinted = masked_tint_sprite(false, [1.0; 4], batch);
        let tinted = masked_tint_sprite(false, red, batch);
        let masked = masked_tint_sprite(true, red, batch);
        let [r, g, b, _] = left(&untinted);
        assert!(r > 20 && r == g && g == b, "batch {batch}: untinted grey");
        let [r, g, _, _] = left(&tinted);
        assert!(r > g + 60, "batch {batch}: the tint reddens the sprite");
        assert_eq!(left(&masked), left(&tinted), "batch {batch}: masked half");
        assert_eq!(
            right(&masked),
            right(&untinted),
            "batch {batch}: clear half"
        );
        let brighter = masked_tint_sprite(true, [1.0, 0.2, 0.2, 1.0], batch);
        assert!(
            left(&masked)[0] > left(&brighter)[0],
            "batch {batch}: a tint above 1 brightens"
        );
    }
}

/// The centre pixel of a lit sprite facing the camera, under a point light,
/// drawn in `layer` at the viewport's centre or, unplaced, at a screen size.
fn lit_hand(
    lighting: SpriteLightingMode,
    layer: RenderLayer,
    placed: bool,
    light_intensity: f32,
) -> [u8; 4] {
    let mut harness = Harness::new(RendererOptions {
        default_world_lights: false,
        ..RendererOptions::default()
    });
    let mut ops = vec![RenderDiff::SetBackgroundColor {
        color: [0.0, 0.0, 0.0, 1.0],
    }];
    ops.extend(atlas_ops(&mut harness.resources));
    let mut hand = sprite(0, [0.0, 0.0, -4.0], BillboardMode::None);
    hand.size = [64.0, 64.0];
    hand.size_mode = SpriteSizeMode::Pixel;
    hand.depth = SpriteDepthPolicy::DepthTestOff;
    hand.layer = layer;
    hand.material.lighting = lighting;
    hand.material.alpha = SpriteAlphaMode::Opaque;
    hand.viewport_placement = placed.then_some(SpriteViewportPlacement {
        minimum: [0.25, 0.25],
        size: [0.5, 0.5],
        alignment: [0.0, 0.0],
        fit: SpriteViewportFit::Stretch,
    });
    ops.push(create(3, hand));
    ops.push(RenderDiff::CreateLight {
        handle: RenderHandle::new(31),
        parent: None,
        light: LightDescriptor::Point {
            enabled: true,
            color: [1.0, 1.0, 1.0],
            intensity: light_intensity,
            position: [0.0, 1.0, 1.0],
            range: None,
            decay: 2.0,
            shadow_intent: LightShadowIntent::Disabled,
            shadow: Default::default(),
        },
    });
    harness.apply(ops);
    let rgba = harness.single(&camera("eye", [0.0, 0.0, 0.0], 0.0, 0.0));
    pixel(&rgba, WIDTH, WIDTH / 2, HEIGHT / 2)
}

/// A viewport-placed sprite is lit as any sprite of its layer: facing the
/// camera, at its place in the view. Its normal used to be undefined, which
/// drew it black in the world and white in the viewmodel.
#[test]
fn a_viewport_placed_sprite_lights_as_one_facing_the_camera() {
    for lighting in [
        SpriteLightingMode::Synthetic,
        SpriteLightingMode::DerivedGradient,
    ] {
        let [dim, ..] = lit_hand(lighting, RenderLayer::Scene, true, 2.0);
        let [r, g, b, _] = lit_hand(lighting, RenderLayer::Scene, true, 8.0);
        assert!(r > dim && dim > 0, "{lighting:?}: follows its light");
        assert!(
            r > g + 40 && r > b + 40,
            "{lighting:?}: keeps the art's red"
        );

        // The viewmodel rig lights it as it lights an unplaced sprite.
        let placed = lit_hand(lighting, RenderLayer::Viewmodel, true, 2.0);
        let unplaced = lit_hand(lighting, RenderLayer::Viewmodel, false, 2.0);
        for channel in 0..3 {
            assert!(
                placed[channel].abs_diff(unplaced[channel]) <= 2,
                "{lighting:?}: {placed:?} against {unplaced:?}"
            );
        }
    }
}

/// What the sprite emission tests draw: a dark grey sprite of two frames, its
/// map lit on the left of frame 0 and the right of frame 1, in a world with no
/// light at all.
struct Emissive {
    lighting: SpriteLightingMode,
    color: [f32; 3],
    intensity: f32,
    mapped: bool,
    batch: bool,
    frame: u32,
    alpha: SpriteAlphaMode,
    tint: [f32; 4],
}

impl Default for Emissive {
    fn default() -> Self {
        Self {
            lighting: SpriteLightingMode::Synthetic,
            color: [1.0, 0.5, 0.25],
            intensity: 1.0,
            mapped: true,
            batch: false,
            frame: 0,
            alpha: SpriteAlphaMode::Opaque,
            tint: [1.0; 4],
        }
    }
}

const EMISSIVE_HANDLE: u64 = 3;

fn emissive_harness(request: &Emissive) -> Harness {
    let mut harness = Harness::new(RendererOptions {
        default_world_lights: false,
        ..RendererOptions::default()
    });
    let (dark, lit, clear) = ([40, 40, 40, 255], [255; 4], [255, 255, 255, 0]);
    // Each frame is two texels wide; the last texel of frame 1 is clear.
    let (texture, _) = harness.resources.texture(
        "texture/dark",
        4,
        1,
        &[dark, dark, dark, clear].concat(),
        TextureFilter::Nearest,
    );
    // Black and clear both say no: the second frame's left texel is clear.
    let (map, _) = harness.resources.texture(
        "texture/emission",
        4,
        1,
        &[lit, [0, 0, 0, 255], clear, lit].concat(),
        TextureFilter::Nearest,
    );
    let frame = |frame, u0, u1| SpriteFrameRect {
        frame,
        uv_min: [u0, 0.0],
        uv_max: [u1, 1.0],
        size: None,
    };
    let mut ops = vec![
        RenderDiff::SetBackgroundColor {
            color: [0.0, 0.0, 0.0, 1.0],
        },
        texture,
        map,
        RenderDiff::DefineSpriteAtlas {
            atlas: SpriteAtlasDescriptor {
                id: "sprite/dark".into(),
                texture: "texture/dark".into(),
                frames: vec![frame(0, 0.0, 0.5), frame(1, 0.5, 1.0)],
            },
        },
    ];
    let mut shared = sprite(request.frame, [0.0, 1.0, -4.0], BillboardMode::Spherical);
    shared.asset = "sprite/dark".into();
    shared.tint = request.tint;
    shared.material.lighting = request.lighting;
    shared.material.alpha = request.alpha;
    shared.material.emission_color = request.color;
    shared.material.emission_intensity = request.intensity;
    shared.material.emission_map = request.mapped.then(|| "texture/emission".into());
    if request.batch {
        ops.push(RenderDiff::CreateSpriteBatch {
            handle: RenderHandle::new(EMISSIVE_HANDLE),
            parent: None,
            batch: SpriteBatchDescriptor {
                sprite: shared,
                instances: vec![SpriteBatchInstance {
                    position: [0.0; 3],
                    scale: 2.0,
                    frame: request.frame,
                }]
                .into(),
            },
        });
    } else {
        shared.size = [2.0; 2];
        ops.push(create(EMISSIVE_HANDLE, shared));
    }
    harness.apply(ops);
    harness
}

fn emissive_eye() -> render_host_contracts::RendererCompositionCamera {
    camera("eye", [0.0, 1.0, 0.0], 0.0, 0.0)
}

fn emissive_sprite(request: Emissive) -> Vec<u8> {
    emissive_harness(&request).single(&emissive_eye())
}

fn left(rgba: &[u8]) -> [u8; 4] {
    pixel(rgba, WIDTH, WIDTH / 2 - 20, HEIGHT / 2)
}

fn right(rgba: &[u8]) -> [u8; 4] {
    pixel(rgba, WIDTH, WIDTH / 2 + 20, HEIGHT / 2)
}

/// In the dark a lit sprite is black but for what its map emits, whatever its
/// lighting mode, from single sprites and batches alike; with no map it emits
/// over all of it. Unlit, its art stays and the emission adds to it.
#[test]
fn a_sprite_emits_where_its_map_does_and_needs_no_light() {
    for batch in [false, true] {
        for lighting in [
            SpriteLightingMode::Synthetic,
            SpriteLightingMode::DerivedGradient,
        ] {
            let name = format!("{lighting:?}, batch {batch}");
            let mapped = emissive_sprite(Emissive {
                lighting,
                batch,
                ..Default::default()
            });
            let [r, g, b, _] = left(&mapped);
            assert!(r > g && g > b && b > 0, "{name}: the emission's colour");
            assert_eq!(right(&mapped)[..3], [0; 3], "{name}: unmapped stays dark");

            let whole = emissive_sprite(Emissive {
                lighting,
                batch,
                mapped: false,
                ..Default::default()
            });
            assert_eq!(left(&whole), left(&mapped), "{name}: no map emits on");
            assert_eq!(right(&whole), left(&mapped), "{name}: all of the sprite");

            let off = emissive_sprite(Emissive {
                lighting,
                batch,
                intensity: 0.0,
                ..Default::default()
            });
            assert_eq!(left(&off)[..3], [0; 3], "{name}: no intensity, no light");
        }
        let art = emissive_sprite(Emissive {
            lighting: SpriteLightingMode::Unlit,
            batch,
            intensity: 0.0,
            ..Default::default()
        });
        let glowing = emissive_sprite(Emissive {
            lighting: SpriteLightingMode::Unlit,
            batch,
            ..Default::default()
        });
        assert!(right(&art)[0] > 0, "batch {batch}: unlit art shows");
        assert_eq!(right(&glowing), right(&art), "batch {batch}: unmapped");
        assert!(
            left(&glowing)[0] > left(&art)[0],
            "batch {batch}: adds to it"
        );
    }
}

/// The map shares the colour texture's uv, so it follows the frame: each
/// frame shows its own part of the map, as a playback's frame updates do.
#[test]
fn a_sprite_emission_map_follows_the_atlas_frame() {
    let mut harness = emissive_harness(&Emissive::default());
    let first = harness.single(&emissive_eye());
    assert!(left(&first)[0] > 0 && right(&first)[0] == 0, "frame 0");
    harness.apply(vec![RenderDiff::UpdateSprite {
        handle: RenderHandle::new(EMISSIVE_HANDLE),
        frame: Some(1),
        tint: None,
        render_order: None,
        visible: None,
        emission_intensity: None,
    }]);
    let second = harness.single(&emissive_eye());
    assert_eq!(left(&second)[..3], [0; 3], "frame 1: its clear texel");
    // The frame's right texel is clear in the art, opaque under this alpha.
    assert!(right(&second)[0] > 0, "frame 1: its lit texel");

    // A batch's instances keep the frames they were placed with.
    let batched = emissive_sprite(Emissive {
        batch: true,
        frame: 1,
        ..Default::default()
    });
    assert_eq!(left(&batched)[..3], [0; 3], "batch frame 1: clear texel");
    assert!(right(&batched)[0] > 0, "batch frame 1: lit texel");
}

/// The sprite's alpha masks what it emits: a cut texel emits nothing and a
/// fading sprite's emission fades with it, while the tint's colour leaves it
/// alone.
#[test]
fn a_sprite_alpha_masks_its_emission_and_its_tint_does_not_colour_it() {
    let cut = emissive_sprite(Emissive {
        alpha: SpriteAlphaMode::Mask { cutoff: 0.5 },
        mapped: false,
        frame: 1,
        ..Default::default()
    });
    assert!(left(&cut)[0] > 0, "the opaque texel emits");
    assert_eq!(right(&cut)[..3], [0; 3], "the cut texel does not");

    let solid = left(&emissive_sprite(Emissive::default()));
    let faded = left(&emissive_sprite(Emissive {
        alpha: SpriteAlphaMode::Blend,
        tint: [1.0, 1.0, 1.0, 0.5],
        ..Default::default()
    }));
    assert!(
        faded[0] < solid[0] && faded[0] > 0,
        "{faded:?} under {solid:?}"
    );

    let tinted = left(&emissive_sprite(Emissive {
        tint: [0.0, 0.0, 1.0, 1.0],
        ..Default::default()
    }));
    assert_eq!(tinted, solid, "the tint's colour leaves emission alone");
}

/// Emission above 1 stays above 1 in the linear target, so it blooms past
/// the sprite, more as it rises; well under the threshold nothing spreads.
#[test]
fn a_sprite_emission_above_one_blooms() {
    let render = |intensity: f32| {
        let request = Emissive {
            color: [1.0, 1.0, 1.0],
            intensity,
            ..Default::default()
        };
        let mut harness = emissive_harness(&request);
        harness.apply(vec![RenderDiff::SetBloom {
            bloom: Some(BloomDescriptor {
                intensity: 1.0,
                threshold: 1.0,
            }),
        }]);
        harness.single(&emissive_eye())
    };
    // Black background above the sprite's top edge.
    let above = |rgba: &[u8]| pixel(rgba, WIDTH, WIDTH / 2 - 20, HEIGHT / 2 - 54);
    assert_eq!(
        above(&render(0.5))[..3],
        [0; 3],
        "under the knee, no spread"
    );
    let (at_one, above_one) = (above(&render(1.0))[0], above(&render(4.0))[0]);
    assert!(above_one > at_one, "{above_one} spreads past {at_one}");
}

/// Changing a retained sprite's intensity keeps the node, its frame and its
/// batch rows: a lamp goes out at dawn while its playback runs.
#[test]
fn a_retained_sprite_changes_its_emission_intensity_in_place() {
    for batch in [false, true] {
        let mut harness = emissive_harness(&Emissive {
            batch,
            ..Default::default()
        });
        let lit = harness.single(&emissive_eye());
        let change = |intensity| {
            vec![RenderDiff::UpdateSprite {
                handle: RenderHandle::new(EMISSIVE_HANDLE),
                frame: None,
                tint: None,
                render_order: None,
                visible: None,
                emission_intensity: Some(intensity),
            }]
        };
        harness.apply(change(0.0));
        assert_eq!(left(&harness.single(&emissive_eye()))[..3], [0; 3], "out");
        harness.apply(change(1.0));
        assert_eq!(harness.single(&emissive_eye()), lit, "lit again");
    }
}

/// A solid white standing sprite 2 m tall, its base at the view's centre
/// line, over a dark clear colour; a whole-texture 8 texels wide and tall.
fn windy_sprite(bend: f32, whole_texels: bool) -> SpriteInstanceDescriptor {
    let mut standing = sprite(0, [0.0, -1.0, -4.0], BillboardMode::Cylindrical);
    standing.pivot = [0.5, 0.0];
    standing.size = [1.0, 2.0];
    standing.material.alpha = SpriteAlphaMode::Mask { cutoff: 0.5 };
    standing.material.wind_bend = bend;
    standing.material.wind_whole_texels = whole_texels;
    standing
}

/// The view of `ops` on a dark sky in a wind toward +x at `strength`, at
/// presentation time `time`.
fn windy(ops: Vec<RenderDiff>, strength: f32, time: f64) -> Vec<u8> {
    let mut harness = Harness::new(RendererOptions {
        default_world_lights: false,
        samples: 1,
        ..RendererOptions::default()
    });
    let mut all = vec![
        RenderDiff::SetBackgroundColor {
            color: [0.0, 0.0, 0.0, 1.0],
        },
        RenderDiff::SetWind {
            wind: Some(WindDescriptor {
                direction: [1.0, 0.0],
                strength,
                gust: 0.0,
            }),
        },
    ];
    all.extend(atlas_ops(&mut harness.resources));
    all.extend(ops);
    harness.apply(all);
    harness.renderer.set_animation_time(time);
    let eye = camera("eye", [0.0, 0.0, 0.0], 0.0, 0.0);
    harness.renderer.render_offscreen(&eye, &harness.target);
    harness.target.read_rgba(&harness.gpu)
}

/// The mean x of the lit pixels in a pixel row, if any.
fn row_centre(rgba: &[u8], y: u32) -> Option<f32> {
    let lit: Vec<u32> = (0..WIDTH)
        .filter(|&x| {
            pixel(rgba, WIDTH, x, y)[..3]
                .iter()
                .any(|&value| value > 24)
        })
        .collect();
    (!lit.is_empty()).then(|| lit.iter().sum::<u32>() as f32 / lit.len() as f32)
}

/// The lowest and highest rows the sprite draws in.
fn lit_rows(rgba: &[u8]) -> (u32, u32) {
    let rows: Vec<u32> = (0..HEIGHT)
        .filter(|&y| row_centre(rgba, y).is_some())
        .collect();
    (*rows.last().unwrap(), rows[0])
}

#[test]
fn a_sprite_with_a_wind_bend_leans_from_its_base() {
    let still = windy(vec![create(10, windy_sprite(0.0, false))], 2.0, 3.0);
    let bent = windy(vec![create(10, windy_sprite(0.2, false))], 2.0, 3.0);
    let (bottom, top) = lit_rows(&bent);
    let base = row_centre(&bent, bottom).unwrap() - row_centre(&still, bottom).unwrap();
    let lean = row_centre(&bent, top).unwrap() - row_centre(&still, top).unwrap();
    assert!(base.abs() < 2.0, "the base stays planted: {base}");
    assert!(lean > 8.0, "the top leans along the wind: {lean}");
}

#[test]
fn sprite_batches_and_synthetic_sprites_lean_as_a_sprite_does() {
    let batch = |sprite: SpriteInstanceDescriptor| {
        vec![RenderDiff::CreateSpriteBatch {
            handle: RenderHandle::new(10),
            parent: None,
            batch: SpriteBatchDescriptor {
                sprite,
                instances: vec![SpriteBatchInstance {
                    position: [0.0; 3],
                    scale: 1.0,
                    frame: 0,
                }]
                .into(),
            },
        }]
    };
    // Lit by nothing here, so its emission shows where it draws.
    let synthetic = |bend, whole_texels| {
        let mut sprite = windy_sprite(bend, whole_texels);
        sprite.material.lighting = SpriteLightingMode::Synthetic;
        sprite.material.emission_color = [1.0; 3];
        sprite.material.emission_intensity = 1.0;
        sprite
    };
    for whole_texels in [false, true] {
        for (what, still, bent) in [
            (
                "batch",
                batch(windy_sprite(0.0, false)),
                batch(windy_sprite(0.2, whole_texels)),
            ),
            (
                "synthetic",
                vec![create(10, synthetic(0.0, false))],
                vec![create(10, synthetic(0.2, whole_texels))],
            ),
            (
                "synthetic batch",
                batch(synthetic(0.0, false)),
                batch(synthetic(0.2, whole_texels)),
            ),
        ] {
            let (still, bent) = (windy(still, 2.0, 3.0), windy(bent, 2.0, 3.0));
            let (bottom, top) = lit_rows(&bent);
            let base = row_centre(&bent, bottom).unwrap() - row_centre(&still, bottom).unwrap();
            let lean = row_centre(&bent, top).unwrap() - row_centre(&still, top).unwrap();
            assert!(
                base.abs() < 2.0,
                "{what}, whole texels {whole_texels}: the base stays: {base}"
            );
            assert!(
                lean > 8.0,
                "{what}, whole texels {whole_texels}: the top leans: {lean}"
            );
        }
    }
}

#[test]
fn no_wind_or_no_bend_draws_a_sprite_as_it_was() {
    let unset = windy(vec![create(10, windy_sprite(0.0, false))], 2.0, 3.0);
    assert_eq!(
        windy(vec![create(10, windy_sprite(0.0, true))], 2.0, 3.0),
        unset,
        "an unset sprite ignores the wind"
    );
    assert_eq!(
        windy(vec![create(10, windy_sprite(0.2, false))], 0.0, 3.0),
        unset,
        "zero wind draws an opted-in sprite as an unset one"
    );
    assert_eq!(
        windy(vec![create(10, windy_sprite(0.2, true))], 0.0, 3.0),
        unset,
        "whole texels too"
    );
}

#[test]
fn neighbouring_sprites_lean_apart() {
    let pair = |x: f32| {
        let mut one = windy_sprite(0.2, false);
        one.transform = transform([x, -1.0, -4.0], 0.0, 1.0);
        one
    };
    let gusty = |one: SpriteInstanceDescriptor| {
        let mut harness = Harness::new(RendererOptions {
            default_world_lights: false,
            samples: 1,
            ..RendererOptions::default()
        });
        let mut ops = vec![
            RenderDiff::SetBackgroundColor {
                color: [0.0, 0.0, 0.0, 1.0],
            },
            RenderDiff::SetWind {
                wind: Some(WindDescriptor {
                    direction: [1.0, 0.0],
                    strength: 2.0,
                    gust: 1.0,
                }),
            },
        ];
        ops.extend(atlas_ops(&mut harness.resources));
        ops.push(create(10, one));
        harness.apply(ops);
        harness.renderer.set_animation_time(3.0);
        let eye = camera("eye", [0.0, 0.0, 0.0], 0.0, 0.0);
        harness.renderer.render_offscreen(&eye, &harness.target);
        let rgba = harness.target.read_rgba(&harness.gpu);
        let (bottom, top) = lit_rows(&rgba);
        row_centre(&rgba, top).unwrap() - row_centre(&rgba, bottom).unwrap()
    };
    let (left, right) = (gusty(pair(-1.3)), gusty(pair(1.3)));
    assert!((left - right).abs() > 2.0, "{left} against {right}");
}

/// The x of a sprite's left edge in each pixel row it draws in.
fn left_edges(rgba: &[u8]) -> Vec<u32> {
    (0..HEIGHT)
        .filter_map(|y| {
            (0..WIDTH).find(|&x| {
                pixel(rgba, WIDTH, x, y)[..3]
                    .iter()
                    .any(|&value| value > 24)
            })
        })
        .collect()
}

#[test]
fn whole_texels_lean_row_by_row_in_whole_steps() {
    let smooth = left_edges(&windy(vec![create(10, windy_sprite(0.2, false))], 2.0, 3.0));
    let edges = left_edges(&windy(vec![create(10, windy_sprite(0.2, true))], 2.0, 3.0));
    let steps = |edges: &[u32]| {
        let mut runs = edges.to_vec();
        runs.dedup();
        runs.len()
    };
    let moved = edges.first().unwrap().abs_diff(*edges.last().unwrap());
    assert!(moved > 8, "it leans: {moved} pixels");
    assert!(
        steps(&edges) * 2 < steps(&smooth),
        "fewer, larger steps: {} against {}",
        steps(&edges),
        steps(&smooth)
    );
}

/// A lean of 16 texels at the top, far enough that a share of it given to the
/// bottom row would be a whole texel.
#[test]
fn a_strong_whole_texel_lean_keeps_the_bottom_row_planted() {
    let batch = |sprite: SpriteInstanceDescriptor| {
        vec![RenderDiff::CreateSpriteBatch {
            handle: RenderHandle::new(10),
            parent: None,
            batch: SpriteBatchDescriptor {
                sprite,
                instances: vec![SpriteBatchInstance {
                    position: [0.0; 3],
                    scale: 1.0,
                    frame: 0,
                }]
                .into(),
            },
        }]
    };
    for (what, still, bent) in [
        (
            "sprite",
            vec![create(10, windy_sprite(0.0, false))],
            vec![create(10, windy_sprite(0.5, true))],
        ),
        (
            "batch",
            batch(windy_sprite(0.0, false)),
            batch(windy_sprite(0.5, true)),
        ),
    ] {
        let (still, bent) = (windy(still, 2.0, 3.0), windy(bent, 2.0, 3.0));
        let (bottom, top) = lit_rows(&still);
        // Every pixel row of the frame's bottom texel row stands where it
        // stood.
        let texel_row = (bottom - top) / 8;
        for y in bottom - texel_row + 1..=bottom {
            assert_eq!(
                row_centre(&bent, y),
                row_centre(&still, y),
                "{what}: the bottom row at pixel row {y}"
            );
        }
        let lean = row_centre(&bent, top).unwrap() - row_centre(&still, top).unwrap();
        assert!(lean > 8.0, "{what}: the top leans: {lean}");
    }
}

#[test]
fn a_small_change_of_wind_holds_whole_texels_where_a_smooth_lean_slides() {
    let at = |whole_texels, strength| {
        windy(
            vec![create(10, windy_sprite(0.2, whole_texels))],
            strength,
            3.0,
        )
    };
    assert_ne!(at(false, 2.0), at(false, 2.01), "a smooth lean slides");
    assert_eq!(at(true, 2.0), at(true, 2.01), "whole texels hold");
}

/// The atlas's frame 0, as a billboard material's and quad's uv.
const FRAME: [f32; 4] = [0.0, 0.0, 0.5, 1.0];

/// A surface on the atlas, its other fields a plain material's.
fn flat_material() -> RenderMaterialDescriptor {
    let mut ops = coloured_mesh("unused", cube(), [1.0; 4]);
    let Some(RenderDiff::DefineMaterial { mut material }) = ops
        .iter()
        .position(|op| matches!(op, RenderDiff::DefineMaterial { .. }))
        .map(|at| ops.remove(at))
    else {
        unreachable!()
    };
    material.id = "material/flat".to_owned();
    material.texture = Some("texture/atlas".to_owned());
    material
}

/// A quad 2 m tall on frame 0, as `VegetationSprite` makes one.
fn billboard_quad() -> MeshPayloadDescriptor {
    let mut quad = payload(
        vec![-1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 2.0, 0.0, -1.0, 2.0, 0.0],
        [0.0, 0.0, 1.0].repeat(4),
        vec![0, 1, 2, 0, 2, 3],
    );
    quad.billboard = true;
    let MeshPayloadSource::Inline { uvs, .. } = &mut quad.source else {
        unreachable!()
    };
    *uvs = Some(vec![0.0, 1.0, 0.5, 1.0, 0.5, 0.0, 0.0, 0.0].into());
    quad
}

/// One spot of `sprite`'s look: `sprite` standing there, or the billboard copy
/// of it, in a light from the right, on black.
fn lit_look(
    material: &SpriteMaterialDescriptor,
    billboard: Option<(MaterialBillboardLighting, Option<&str>)>,
    wind: f32,
) -> Vec<u8> {
    look_in(material, billboard, wind, [1.0, 0.0], false)
}

/// `lit_look` with the wind toward `direction`, and with `occlusion` on a floor
/// behind the copy, whose depth-only pass reads the copy's shape.
fn look_in(
    material: &SpriteMaterialDescriptor,
    billboard: Option<(MaterialBillboardLighting, Option<&str>)>,
    wind: f32,
    direction: [f32; 2],
    occlusion: bool,
) -> Vec<u8> {
    let mut harness = Harness::new(RendererOptions {
        default_world_lights: false,
        samples: 1,
        ..RendererOptions::default()
    });
    let mut ops = vec![
        RenderDiff::SetBackgroundColor {
            color: [0.0, 0.0, 0.0, 1.0],
        },
        RenderDiff::CreateLight {
            handle: RenderHandle::new(40),
            parent: None,
            light: LightDescriptor::Directional {
                color: [1.0; 3],
                intensity: 3.0,
                enabled: true,
                direction: [-0.8, -0.1, -0.6],
                range: None,
                shadow_intent: LightShadowIntent::Disabled,
                shadow: Default::default(),
            },
        },
        RenderDiff::SetWind {
            wind: Some(WindDescriptor {
                direction,
                strength: wind,
                gust: 0.0,
            }),
        },
    ];
    if occlusion {
        ops.push(RenderDiff::SetRendererSettings {
            settings: RendererSettingsDescriptor {
                ambient_occlusion: AmbientOcclusionSettings {
                    mode: AmbientOcclusionMode::ScreenSpace,
                    ..AmbientOcclusionSettings::DEFAULT
                },
                ..RendererSettingsDescriptor::DEFAULT
            },
        });
        ops.extend(scene());
        ops.push(RenderDiff::CreateLight {
            handle: RenderHandle::new(41),
            parent: None,
            light: LightDescriptor::Ambient {
                color: [1.0; 3],
                intensity: 1.0,
                enabled: true,
                shadow_intent: LightShadowIntent::Disabled,
                shadow: Default::default(),
                range: None,
            },
        });
    }
    ops.extend(atlas_ops(&mut harness.resources));
    let (detail, _) = harness.resources.texture(
        "texture/detail",
        16,
        8,
        &(0..16 * 8)
            .flat_map(|at: u32| {
                // Texels of very different heights and normals.
                let (x, y) = (at % 16, at / 16);
                let (a, b) = ((x * 97 + y * 41) % 200, (x * 53 + y * 131) % 200);
                [(a + 28) as u8, (b + 28) as u8, 200, 255]
            })
            .collect::<Vec<u8>>(),
        TextureFilter::Nearest,
    );
    ops.push(detail);
    match billboard {
        None => {
            let mut standing = sprite(0, [0.0, -1.0, -4.0], BillboardMode::Cylindrical);
            standing.pivot = [0.5, 0.0];
            standing.size = [2.0, 2.0];
            standing.material = material.clone();
            ops.push(create(10, standing));
        }
        Some((lighting, map)) => {
            let mut paint = flat_material();
            paint.roughness = 0.82;
            paint.double_sided = true;
            paint.unlit = material.lighting == SpriteLightingMode::Unlit;
            paint.emission_color = material.emission_color;
            paint.emission_intensity = material.emission_intensity;
            paint.alpha_mode = match material.alpha {
                SpriteAlphaMode::Mask { cutoff } => MaterialAlphaModeDescriptor::Mask { cutoff },
                SpriteAlphaMode::Blend => MaterialAlphaModeDescriptor::Blend,
                SpriteAlphaMode::Opaque => MaterialAlphaModeDescriptor::Opaque,
            };
            paint.normal_map = map.map(|texture| MaterialNormalMapDescriptor {
                texture: texture.to_owned(),
                scale: 1.0,
            });
            paint.wind = (material.wind_bend > 0.0).then_some(MaterialWindDescriptor {
                bend: material.wind_bend,
                flutter: 0.0,
            });
            paint.billboard = Some(MaterialBillboardDescriptor {
                lighting,
                normal_strength: material.normal_strength,
                normal_bias: material.normal_bias,
                size: [2.0, 2.0],
                frame: FRAME,
                whole_texels: material.wind_whole_texels,
                heading: false,
                authored_facing: material.authored_facing,
            });
            ops.extend(coloured_mesh("flat", billboard_quad(), [1.0; 4]));
            ops.push(RenderDiff::DefineMaterial { material: paint });
            ops.push(RenderDiff::Create {
                handle: RenderHandle::new(5),
                parent: None,
                node: RenderNode::new(Geometry::Group),
            });
            ops.push(RenderDiff::CreateScatterPatch {
                handle: RenderHandle::new(11),
                parent: Some(RenderHandle::new(5)),
                patch: ScatterPatchDescriptor {
                    asset: "mesh/flat".to_owned(),
                    material_overrides: Vec::new(),
                    instances: vec![ScatterInstance {
                        translation: [0.0, -1.0, -4.0],
                        rotation: [0.0, 0.0, 0.0, 1.0],
                        scale: 1.0,
                        tint: [1.0; 3],
                        rank: 0.5,
                    }],
                    fade: ScatterFade::default(),
                    shadow_casting: ShadowCasting::None,
                    lods: Vec::new(),
                    lod_band: 0.0,
                    thinning: Default::default(),
                },
            });
        }
    }
    harness.apply(ops);
    harness.renderer.set_animation_time(3.0);
    let eye = camera("eye", [0.0, 0.0, 0.0], 0.0, 0.0);
    for _ in 0..3 {
        harness.renderer.render_offscreen(&eye, &harness.target);
    }
    harness.target.read_rgba(&harness.gpu)
}

/// The mean difference per channel over pixels either frame lights.
fn lit_difference(a: &[u8], b: &[u8]) -> (f64, usize) {
    let (mut sum, mut count) = (0.0, 0);
    for (a, b) in a.as_chunks::<4>().0.iter().zip(b.as_chunks::<4>().0) {
        if a[..3].iter().chain(&b[..3]).any(|&value| value > 8) {
            sum += (0..3)
                .map(|channel| f64::from(a[channel].abs_diff(b[channel])))
                .sum::<f64>()
                / 3.0;
            count += 1;
        }
    }
    (sum / count.max(1) as f64, count)
}

fn masked(lighting: SpriteLightingMode) -> SpriteMaterialDescriptor {
    SpriteMaterialDescriptor {
        lighting,
        normal_strength: 0.9,
        normal_texture: (lighting == SpriteLightingMode::AuthoredNormal)
            .then(|| "texture/detail".to_owned()),
        depth_texture: (lighting == SpriteLightingMode::AuthoredDepth)
            .then(|| "texture/detail".to_owned()),
        alpha: SpriteAlphaMode::Mask { cutoff: 0.5 },
        ..SpriteMaterialDescriptor::default()
    }
}

#[test]
fn a_billboard_copy_lights_as_the_same_sprite_in_every_mode() {
    use MaterialBillboardLighting as Billboard;
    use SpriteLightingMode as Sprite;
    for (mode, lighting, map) in [
        (Sprite::Unlit, Billboard::Flat, None),
        (Sprite::Synthetic, Billboard::Dome, None),
        (
            Sprite::AuthoredNormal,
            Billboard::NormalMap,
            Some("texture/detail"),
        ),
        (
            Sprite::AuthoredDepth,
            Billboard::Height,
            Some("texture/detail"),
        ),
        (Sprite::DerivedGradient, Billboard::ColorHeight, None),
    ] {
        let material = masked(mode);
        let sprite = lit_look(&material, None, 0.0);
        let copy = lit_look(&material, Some((lighting, map)), 0.0);
        let (difference, pixels) = lit_difference(&sprite, &copy);
        assert!(pixels > 1_000, "{mode:?} draws: {pixels}");
        assert!(difference < 3.0, "{mode:?}: mean difference {difference}");
        if mode != Sprite::Unlit {
            let flat = lit_look(&material, Some((Billboard::Flat, None)), 0.0);
            let (shaped, _) = lit_difference(&flat, &copy);
            // The atlas's flat colours leave a colour height few edges.
            assert!(shaped > 1.0, "{mode:?} shapes the light: {shaped}");
        }
    }
}

#[test]
fn an_unlit_billboard_copy_gives_off_its_light() {
    let mut material = masked(SpriteLightingMode::Unlit);
    material.emission_color = [0.0, 0.0, 1.0];
    material.emission_intensity = 0.5;
    let sprite = lit_look(&material, None, 0.0);
    let copy = lit_look(
        &material,
        Some((MaterialBillboardLighting::Flat, None)),
        0.0,
    );
    assert_ne!(
        copy,
        lit_look(
            &masked(SpriteLightingMode::Unlit),
            Some((MaterialBillboardLighting::Flat, None)),
            0.0
        )
    );
    let (difference, _) = lit_difference(&sprite, &copy);
    assert!(difference < 3.0, "mean difference {difference}");
}

#[test]
fn a_blended_billboard_copy_draws() {
    let mut material = masked(SpriteLightingMode::Unlit);
    material.alpha = SpriteAlphaMode::Blend;
    let copy = lit_look(
        &material,
        Some((MaterialBillboardLighting::Flat, None)),
        0.0,
    );
    let sprite = lit_look(&material, None, 0.0);
    let (difference, pixels) = lit_difference(&sprite, &copy);
    assert!(pixels > 1_000, "it draws: {pixels}");
    assert!(difference < 3.0, "mean difference {difference}");
}

#[test]
fn a_billboard_copy_leans_in_whole_texels_as_a_sprite_does() {
    let leaning = |whole_texels: bool, strength: f32| {
        let mut material = masked(SpriteLightingMode::Unlit);
        material.wind_bend = 0.3;
        material.wind_whole_texels = whole_texels;
        lit_look(
            &material,
            Some((MaterialBillboardLighting::Flat, None)),
            strength,
        )
    };
    let still = leaning(true, 0.0);
    let whole = leaning(true, 2.0);
    let smooth = leaning(false, 2.0);
    assert_ne!(whole, still, "it leans");
    assert_eq!(leaning(true, 2.01), whole, "whole texels hold");
    assert_ne!(leaning(false, 2.01), smooth, "a smooth lean slides");
    let (bottom, top) = lit_rows(&still);
    assert_eq!(
        row_centre(&whole, bottom),
        row_centre(&still, bottom),
        "the bottom row stays planted"
    );
    let lean = row_centre(&whole, top).unwrap() - row_centre(&still, top).unwrap();
    assert!(lean > 8.0, "the top leans: {lean}");
    let mut sprite = masked(SpriteLightingMode::Unlit);
    sprite.wind_bend = 0.3;
    sprite.wind_whole_texels = true;
    let (difference, _) = lit_difference(&lit_look(&sprite, None, 2.0), &whole);
    assert!(difference < 3.0, "as the sprite leans: {difference}");
}

#[test]
fn the_depth_pass_sees_a_whole_texel_lean_as_the_picture_does() {
    let leaning = |strength: f32, direction: [f32; 2], alpha: SpriteAlphaMode| {
        let mut material = masked(SpriteLightingMode::Unlit);
        material.alpha = alpha;
        material.wind_bend = 0.3;
        material.wind_whole_texels = true;
        look_in(
            &material,
            Some((MaterialBillboardLighting::Flat, None)),
            strength,
            direction,
            true,
        )
    };
    let mask = SpriteAlphaMode::Mask { cutoff: 0.5 };
    // Toward the camera the picture does not move, so neither may its depth:
    // a depth pass leaning smoothly would shade the floor behind differently.
    assert_eq!(
        leaning(2.0, [0.0, 1.0], mask),
        leaning(0.0, [0.0, 1.0], mask),
        "wind toward the eye"
    );
    // Across the view the rows hold between steps, and so does their depth,
    // with a mask and without one.
    for alpha in [mask, SpriteAlphaMode::Opaque] {
        assert_eq!(
            leaning(2.0, [1.0, 0.0], alpha),
            leaning(2.01, [1.0, 0.0], alpha),
            "wind across the view, {alpha:?}"
        );
    }
}

/// Where an upright sprite turns: a single sprite, a batch of one, or a
/// vegetation billboard copy.
#[derive(Debug, Clone, Copy, PartialEq)]
enum UprightPath {
    Single,
    Batch,
    Copy,
}

const UPRIGHT_AT: [f32; 3] = [1.2, -1.0, -4.0];

/// One unlit, masked 2 m frame standing at `UPRIGHT_AT` on black, its node
/// turned `yaw` degrees about world up, drawn along `path` in `billboard` mode
/// keeping `keep` of its own facing, seen from `eye`.
fn upright(
    path: UprightPath,
    billboard: BillboardMode,
    keep: f32,
    yaw: f32,
    eye: &render_host_contracts::RendererCompositionCamera,
) -> Vec<u8> {
    let mut harness = Harness::new(RendererOptions {
        default_world_lights: false,
        samples: 1,
        ..RendererOptions::default()
    });
    let mut material = masked(SpriteLightingMode::Unlit);
    material.authored_facing = keep;
    let mut ops = vec![RenderDiff::SetBackgroundColor {
        color: [0.0, 0.0, 0.0, 1.0],
    }];
    ops.extend(atlas_ops(&mut harness.resources));
    let mut standing = sprite(0, UPRIGHT_AT, billboard);
    standing.pivot = [0.5, 0.0];
    standing.size = [2.0, 2.0];
    standing.material = material.clone();
    standing.transform = transform(UPRIGHT_AT, yaw, 1.0);
    match path {
        UprightPath::Single => ops.push(create(10, standing)),
        UprightPath::Batch => ops.push(RenderDiff::CreateSpriteBatch {
            handle: RenderHandle::new(10),
            parent: None,
            batch: SpriteBatchDescriptor {
                sprite: standing,
                instances: vec![SpriteBatchInstance {
                    position: [0.0; 3],
                    scale: 1.0,
                    frame: 0,
                }]
                .into(),
            },
        }),
        UprightPath::Copy => {
            let mut paint = flat_material();
            paint.double_sided = true;
            paint.unlit = true;
            paint.alpha_mode = MaterialAlphaModeDescriptor::Mask { cutoff: 0.5 };
            paint.billboard = Some(MaterialBillboardDescriptor {
                lighting: MaterialBillboardLighting::Flat,
                normal_strength: material.normal_strength,
                normal_bias: material.normal_bias,
                size: [2.0, 2.0],
                frame: [0.0, 0.0, 0.5, 1.0],
                whole_texels: false,
                heading: billboard == BillboardMode::CylindricalView,
                authored_facing: keep,
            });
            ops.extend(coloured_mesh("flat", billboard_quad(), [1.0; 4]));
            ops.push(RenderDiff::DefineMaterial { material: paint });
            ops.push(RenderDiff::Create {
                handle: RenderHandle::new(5),
                parent: None,
                node: RenderNode::new(Geometry::Group),
            });
            ops.push(RenderDiff::CreateScatterPatch {
                handle: RenderHandle::new(11),
                parent: Some(RenderHandle::new(5)),
                patch: ScatterPatchDescriptor {
                    asset: "mesh/flat".to_owned(),
                    material_overrides: Vec::new(),
                    instances: vec![ScatterInstance {
                        translation: UPRIGHT_AT,
                        rotation: transform(UPRIGHT_AT, yaw, 1.0).rotation,
                        scale: 1.0,
                        tint: [1.0; 3],
                        rank: 0.5,
                    }],
                    fade: ScatterFade::default(),
                    shadow_casting: ShadowCasting::None,
                    lods: Vec::new(),
                    lod_band: 0.0,
                    thinning: Default::default(),
                },
            });
        }
    }
    harness.apply(ops);
    for _ in 0..3 {
        harness.renderer.render_offscreen(eye, &harness.target);
    }
    harness.target.read_rgba(&harness.gpu)
}

/// The same frame with no billboard, its node turned `yaw` degrees.
fn authored(yaw: f32, eye: &render_host_contracts::RendererCompositionCamera) -> Vec<u8> {
    upright(UprightPath::Single, BillboardMode::None, 0.0, yaw, eye)
}

fn assert_looks_like(path: UprightPath, case: &str, drawn: &[u8], expected: &[u8]) {
    let (difference, pixels) = lit_difference(drawn, expected);
    assert!(pixels > 1_000, "{path:?} {case}: draws {pixels}");
    assert!(
        difference < 3.0,
        "{path:?} {case}: mean difference {difference}"
    );
}

fn assert_looks_unlike(path: UprightPath, case: &str, drawn: &[u8], other: &[u8]) {
    let (difference, _) = lit_difference(drawn, other);
    assert!(
        difference > 10.0,
        "{path:?} {case}: mean difference only {difference}"
    );
}

#[test]
fn a_heading_aligned_sprite_turns_with_the_view_not_with_the_eye() {
    use BillboardMode::{Cylindrical, CylindricalView};
    let ahead = camera("eye", [0.0, 0.0, 0.0], 0.0, 0.0);
    let aside = camera("eye", [-1.5, 0.0, 0.0], 0.0, 0.0);
    let yawed = camera("eye", [0.0, 0.0, 0.0], 25.0, 0.0);
    let mut close_near = yawed.clone();
    if let render_host_contracts::RendererCameraProjection::Perspective { near, .. } =
        &mut close_near.projection
    {
        *near = 0.01;
    }
    // A camera turned +x by `yaw` looks along (sin, 0, -cos): a frame
    // facing back along that is a node turned by minus the yaw.
    let (still, side, turned) = (
        authored(0.0, &ahead),
        authored(0.0, &aside),
        authored(-25.0, &yawed),
    );
    let turned_close = authored(-25.0, &close_near);
    // Toward the eye from `UPRIGHT_AT` seen from `ahead`, in node yaw.
    let toward_eye = (-UPRIGHT_AT[0]).atan2(-UPRIGHT_AT[2]).to_degrees();
    for path in [UprightPath::Single, UprightPath::Batch, UprightPath::Copy] {
        // Parallel to the screen wherever the eye stands; turning with it.
        assert_looks_like(
            path,
            "ahead",
            &upright(path, CylindricalView, 0.0, 0.0, &ahead),
            &still,
        );
        let walked = upright(path, CylindricalView, 0.0, 0.0, &aside);
        assert_looks_like(path, "aside", &walked, &side);
        assert_looks_like(
            path,
            "yawed",
            &upright(path, CylindricalView, 0.0, 70.0, &yawed),
            &turned,
        );
        // A near plane of 1 cm shrinks the projection's spans; the turn holds.
        assert_looks_like(
            path,
            "yawed, near 1 cm",
            &upright(path, CylindricalView, 0.0, 70.0, &close_near),
            &turned_close,
        );
        // A cylindrical one turns toward the eye as it walks past.
        let facing_eye = upright(path, Cylindrical, 0.0, 0.0, &aside);
        assert_looks_unlike(path, "cylindrical aside", &facing_eye, &walked);
        assert_looks_like(
            path,
            "cylindrical ahead",
            &upright(path, Cylindrical, 0.0, 0.0, &ahead),
            &authored(toward_eye, &ahead),
        );
        // Part way: half of a 40 degree facing kept; the toward-eye turn
        // halved; and a facing turned away shows its front.
        assert_looks_like(
            path,
            "keep half",
            &upright(path, CylindricalView, 0.5, 40.0, &ahead),
            &authored(20.0, &ahead),
        );
        assert_looks_like(
            path,
            "cylindrical keep half",
            &upright(path, Cylindrical, 0.5, 0.0, &ahead),
            &authored(toward_eye * 0.5, &ahead),
        );
        assert_looks_like(
            path,
            "keep all, turned away",
            &upright(path, CylindricalView, 1.0, 160.0, &ahead),
            &authored(-20.0, &ahead),
        );
    }
}

/// The floor under a masked sprite standing on it, lit by a sun from its
/// side that casts, the sprite's material casting as `shadow` asks.
fn sprite_shadow(shadow: SpriteShadowPolicy) -> Vec<u8> {
    let mut harness = Harness::new(RendererOptions {
        default_world_lights: false,
        shadows: true,
        ..RendererOptions::default()
    });
    let mut ops = scene();
    ops.extend(atlas_ops(&mut harness.resources));
    let mut standing = sprite(0, [0.0, 0.0, -5.0], BillboardMode::Cylindrical);
    standing.material.alpha = SpriteAlphaMode::Mask { cutoff: 0.5 };
    standing.material.shadow = shadow;
    ops.push(create(10, standing));
    ops.push(RenderDiff::CreateLight {
        handle: RenderHandle::new(20),
        parent: None,
        light: LightDescriptor::Directional {
            color: [1.0, 1.0, 1.0],
            intensity: 3.0,
            enabled: true,
            direction: [-1.0, -0.6, 0.0],
            range: None,
            shadow_intent: LightShadowIntent::Requested,
            shadow: Default::default(),
        },
    });
    harness.apply(ops);
    harness.single(&camera("above", [0.0, 4.0, -1.5], 0.0, -60.0))
}

#[test]
fn a_sprite_whose_material_casts_shadows_the_floor_with_its_quad_turned_to_the_sun() {
    // The camera sees the sprite face on and the sun lights it from its
    // side: only a plane turned to the sun casts more than a line.
    let none = sprite_shadow(SpriteShadowPolicy::None);
    let receive = sprite_shadow(SpriteShadowPolicy::Receive);
    let cast = sprite_shadow(SpriteShadowPolicy::Cast);
    assert_eq!(
        differing_pixels(&receive, &none),
        0,
        "a sprite that only receives casts nothing"
    );
    let shadow = differing_pixels(&cast, &none);
    assert!(
        shadow > 400,
        "the sprite's shadow darkens {shadow} floor pixels"
    );
    assert_screenshot("sprite_shadow", WIDTH, HEIGHT, &cast);
}
