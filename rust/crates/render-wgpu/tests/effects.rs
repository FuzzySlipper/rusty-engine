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
fn particle_textures_are_released_when_their_last_emitter_and_particle_leave() {
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

    // Destroying the emitter removes its particles, and with them C.
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
