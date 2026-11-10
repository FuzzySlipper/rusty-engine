//! Parts rigs on a screen-pixel grid (`PartsRigInstanceDescriptor::pixel_grid`):
//! a rig of an upright piece and one turned 10° beside it, under an
//! orthographic camera of 20 pixels a metre, drawn in 2-pixel blocks:
//! - every block square, whole pixels, on one grid anchored at the rig
//!   origin's projection, across both pieces;
//! - a camera move of a fraction of a block keeps the blocks where they
//!   are, and a move of a whole pixel moves them with the figure;
//! - a box in front hides the blocks behind it, pixel by pixel;
//! - each block the coverage-weighted average of the smooth draw over it,
//!   taken offline from the smooth draw over two backgrounds, lit;
//! - with a palette, every pixel drawn a palette colour;
//! - a rig the scratch image has no room for left draws smooth, counted.

mod common;

use common::*;
use render_host_contracts::{RendererCameraProjection, RendererCompositionCamera};
use render_model::*;
use render_wgpu::RendererOptions;

const RIG: &str = "parts-rig/grid";
const PAGE: &str = "texture/grid";
/// The rig's origin in the world, metres: (80.6, 140) pixels under
/// `flat_camera`, so its grid's corner is pixel (81, 140).
const ORIGIN: [f32; 2] = [4.03, 2.0];
const ANCHOR: [u32; 2] = [81, 140];
const BLOCK: u32 = 2;
/// The pieces' side, metres (rig units at scale 1).
const SIDE: f32 = 2.5;
const BACKGROUND: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

/// A 16 × 16 page: red rising to the right, green rising down, blue half,
/// opaque but for a transparent diagonal band, so the pieces have cut edges
/// inside them as well as at their sides.
fn page() -> Vec<u8> {
    let mut rgba = Vec::new();
    for y in 0..16u32 {
        for x in 0..16u32 {
            let band = (x + y).abs_diff(18) <= 1;
            rgba.extend_from_slice(&[
                (x * 16 + 8) as u8,
                (y * 16 + 8) as u8,
                128,
                if band { 0 } else { 255 },
            ]);
        }
    }
    rgba
}

fn define(harness: &mut Harness) -> Vec<RenderDiff> {
    let (texture, _) = harness
        .resources
        .texture(PAGE, 16, 16, &page(), TextureFilter::Linear);
    vec![
        texture,
        RenderDiff::DefinePartsRig {
            asset: PartsRigAsset {
                id: RIG.to_owned(),
                pages: vec![PartsRigPage {
                    texture: PAGE.to_owned(),
                    premultiplied: false,
                    detail: None,
                }],
                drawables: vec![PartsRigDrawable {
                    page: 0,
                    uvs: vec![[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
                    triangles: vec![0, 1, 2, 2, 3, 0],
                }]
                .into(),
                figure: [0.0; 4],
            },
        },
    ]
}

/// An upright piece from the origin, and beside it the same piece turned
/// 10° about its centre.
fn pose() -> PartsRigPose {
    let turn = 10f32.to_radians();
    let square = |x: f32, y: f32, turn: f32| {
        [[0.0, 0.0], [SIDE, 0.0], [SIDE, SIDE], [0.0, SIDE]].map(|[u, v]| {
            let (u, v) = (u - SIDE / 2.0, v - SIDE / 2.0);
            [
                u * turn.cos() - v * turn.sin() + x + SIDE / 2.0,
                u * turn.sin() + v * turn.cos() + y + SIDE / 2.0,
            ]
        })
    };
    let pieces = [square(0.0, 0.0, 0.0), square(SIDE, 0.2, turn)];
    PartsRigPose {
        seconds: 0.0,
        pieces: (0..2)
            .map(|_| PartsRigPiece {
                drawable: 0,
                clipped: None,
                blend: PartsRigBlend::Normal,
                color: [1.0; 4],
                dark: [0.0; 3],
            })
            .collect(),
        vertices: pieces.concat().into(),
        uvs: Default::default(),
        triangles: Default::default(),
    }
}

fn rig(
    pixel_grid: u32,
    material: SpriteMaterialDescriptor,
    at: [f32; 3],
) -> PartsRigInstanceDescriptor {
    PartsRigInstanceDescriptor {
        views: vec![PartsRigView {
            asset: RIG.to_owned(),
            heading: 0.0,
            mirror: false,
        }],
        scale: 1.0,
        size_mode: SpriteSizeMode::World,
        billboard: BillboardMode::None,
        tint: [1.0; 4],
        render_order: 0,
        depth: SpriteDepthPolicy::Default,
        layer: RenderLayer::Scene,
        material,
        texel_snap: 0.0,
        figure_dome: false,
        pixel_grid,
        pixel_coverage: 0.5,
        palette: Vec::new(),
        pose_rate: 0.0,
        viewport_placement: None,
        visible: true,
        transform: Transform {
            translation: at,
            ..Transform::IDENTITY
        },
        metadata: RenderMetadata::default(),
    }
}

fn unlit() -> SpriteMaterialDescriptor {
    SpriteMaterialDescriptor {
        alpha: SpriteAlphaMode::Mask { cutoff: 0.5 },
        ..Default::default()
    }
}

fn create(handle: u64, rig: PartsRigInstanceDescriptor) -> [RenderDiff; 2] {
    let handle = RenderHandle::new(handle);
    [
        RenderDiff::CreatePartsRig {
            handle,
            parent: None,
            rig,
        },
        RenderDiff::SetPartsRigPose {
            handle,
            view: 0,
            pose: pose(),
        },
    ]
}

/// A harness drawing `rig` over `background`, and whatever `more` adds.
fn scene(rig: PartsRigInstanceDescriptor, background: [f32; 4], more: Vec<RenderDiff>) -> Harness {
    let mut harness = Harness::new(RendererOptions {
        default_world_lights: false,
        ..RendererOptions::default()
    });
    let mut ops = vec![RenderDiff::SetBackgroundColor { color: background }];
    ops.extend(define(&mut harness));
    ops.extend(create(10, rig));
    ops.extend(more);
    harness.apply(ops);
    harness
}

/// An orthographic camera over x 0 to 16 m and y 0 to 9 m, moved `shift`
/// metres along x: 20 pixels a metre on the harness's 320 × 180 target.
fn flat_camera(shift: f64) -> RendererCompositionCamera {
    let mut eye = camera("flat", [8.0 + shift, 4.5, 50.0], 0.0, 0.0);
    eye.projection = RendererCameraProjection::Orthographic {
        vertical_size: 9.0,
        near: 1.0,
        far: 100.0,
    };
    eye
}

fn at(rgba: &[u8], x: u32, y: u32) -> [u8; 4] {
    pixel(rgba, WIDTH, x, y)
}

fn background() -> [u8; 4] {
    [0, 0, 0, 255]
}

/// The blocks of the grid with a corner at `anchor` that hold any drawn
/// pixel: (block corner, whether all its pixels are one colour).
fn blocks(rgba: &[u8], anchor: [u32; 2]) -> Vec<([u32; 2], bool)> {
    let mut found = Vec::new();
    let mut y = anchor[1] % BLOCK;
    while y + BLOCK <= HEIGHT {
        let mut x = anchor[0] % BLOCK;
        while x + BLOCK <= WIDTH {
            let first = at(rgba, x, y);
            let pixels: Vec<[u8; 4]> = (0..BLOCK * BLOCK)
                .map(|i| at(rgba, x + i % BLOCK, y + i / BLOCK))
                .collect();
            if pixels.iter().any(|p| *p != background()) {
                found.push(([x, y], pixels.iter().all(|p| *p == first)));
            }
            x += BLOCK;
        }
        y += BLOCK;
    }
    found
}

#[test]
fn blocks_are_square_whole_pixels_on_one_grid_anchored_at_the_origin() {
    let mut harness = scene(
        rig(BLOCK, unlit(), [ORIGIN[0], ORIGIN[1], 0.0]),
        BACKGROUND,
        Vec::new(),
    );
    let frame = harness.single(&flat_camera(0.0));
    let on_grid = blocks(&frame, ANCHOR);
    assert!(
        on_grid.len() > 1_000,
        "the figure draws: {} blocks",
        on_grid.len()
    );
    let split: Vec<_> = on_grid.iter().filter(|(_, whole)| !whole).collect();
    assert!(
        split.is_empty(),
        "blocks of more than one colour: {split:?}"
    );
    // Both pieces: the turned one stands from x = 4.03 + 2.5 m, pixel 131.
    assert!(on_grid.iter().any(|([x, _], _)| *x < 120));
    assert!(on_grid.iter().any(|([x, _], _)| *x > 140));
    // Off the anchor's grid by a pixel, blocks straddle colours.
    let shifted = blocks(&frame, [ANCHOR[0] + 1, ANCHOR[1] + 1]);
    assert!(
        shifted.iter().filter(|(_, whole)| !whole).count() > 100,
        "the grid is the anchor's"
    );
    let readout = {
        harness.single(&flat_camera(0.0));
        harness.renderer.gpu_readout().parts_rig_grid
    };
    assert_eq!((readout.drawn, readout.smooth), (1, 0));
    assert_screenshot("parts_rig_grid", WIDTH, HEIGHT, &frame);
}

#[test]
fn a_camera_move_of_part_of_a_block_keeps_the_blocks_on_the_figure() {
    let mut harness = scene(
        rig(BLOCK, unlit(), [ORIGIN[0], ORIGIN[1], 0.0]),
        BACKGROUND,
        Vec::new(),
    );
    let still = harness.single(&flat_camera(0.0));
    // The figure 0.3 pixels right: its origin at 80.9, still pixel 81's
    // grid, so the blocks stay where they are, their colours near.
    let nudged = harness.single(&flat_camera(-0.015));
    let (grid, moved) = (blocks(&still, ANCHOR), blocks(&nudged, ANCHOR));
    assert!(moved.iter().all(|(_, whole)| *whole));
    let near = grid
        .iter()
        .filter(|([x, y], _)| {
            let (a, b) = (at(&still, *x, *y), at(&nudged, *x, *y));
            a.iter().zip(b).all(|(a, b)| a.abs_diff(b) <= 12)
        })
        .count();
    assert!(
        near * 100 >= grid.len() * 97,
        "{near} of {} blocks keep their colour",
        grid.len()
    );
    // The figure a whole pixel right: its origin at 81.6, so the grid's
    // corner moves to 82 with it, and every block with it.
    let stepped = harness.single(&flat_camera(-0.05));
    let mut same = 0;
    let mut drawn = 0;
    for y in 0..HEIGHT {
        for x in 0..WIDTH - 1 {
            let before = at(&still, x, y);
            if before == background() {
                continue;
            }
            drawn += 1;
            let after = at(&stepped, x + 1, y);
            same += usize::from(before.iter().zip(after).all(|(a, b)| a.abs_diff(b) <= 1));
        }
    }
    assert!(
        same * 1000 >= drawn * 998,
        "{same} of {drawn} pixels move a whole pixel with the figure"
    );
}

#[test]
fn a_box_in_front_hides_the_blocks_behind_it() {
    let box_ops = |harness: &mut Harness| {
        let mut ops = coloured_mesh("box", cube(), [0.2, 0.9, 0.3, 1.0]);
        ops.push(RenderDiff::CreateLight {
            handle: RenderHandle::new(30),
            parent: None,
            light: LightDescriptor::Ambient {
                enabled: true,
                color: [1.0; 3],
                intensity: 1.0,
                shadow_intent: LightShadowIntent::Disabled,
                shadow: Default::default(),
                range: None,
            },
        });
        // A metre box nearer the eye, its left edge at x = 5.02 m (pixel
        // 100.4), inside a block, over the lower part of both pieces from
        // y = 2 to 3.4 m.
        ops.push(instance(
            20,
            None,
            "box",
            Transform {
                translation: [5.52, 2.7, 2.0],
                rotation: [0.0, 0.0, 0.0, 1.0],
                scale: [1.0, 1.4, 1.0],
            },
        ));
        let _ = harness;
        ops
    };
    let figure = rig(BLOCK, unlit(), [ORIGIN[0], ORIGIN[1], 0.0]);
    let mut alone = scene(figure.clone(), BACKGROUND, Vec::new());
    let rig_alone = alone.single(&flat_camera(0.0));
    let mut harness = scene(figure.clone(), BACKGROUND, Vec::new());
    let more = box_ops(&mut harness);
    harness.apply(more);
    let both = harness.single(&flat_camera(0.0));
    harness.apply(vec![RenderDiff::Destroy {
        handle: RenderHandle::new(10),
    }]);
    let box_alone = harness.single(&flat_camera(0.0));
    let (mut hidden, mut cut) = (0, 0);
    // The box's own edge pixels blend with what lies behind them.
    let in_box = |x: u32, y: u32| at(&box_alone, x, y) != background();
    let edge = |x: u32, y: u32| {
        [(-1, 0), (1, 0), (0, -1), (0, 1)].iter().any(|(dx, dy)| {
            let (nx, ny) = (x as i32 + dx, y as i32 + dy);
            nx >= 0
                && ny >= 0
                && (nx as u32) < WIDTH
                && (ny as u32) < HEIGHT
                && in_box(nx as u32, ny as u32) != in_box(x, y)
        })
    };
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            if edge(x, y) {
                continue;
            }
            let expected = if at(&box_alone, x, y) != background() {
                at(&box_alone, x, y)
            } else {
                at(&rig_alone, x, y)
            };
            let drawn = at(&both, x, y);
            assert!(
                drawn.iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 2),
                "pixel ({x}, {y}): {drawn:?}, expected {expected:?}"
            );
            if at(&box_alone, x, y) != background() && at(&rig_alone, x, y) != background() {
                hidden += 1;
            }
        }
    }
    // Blocks the box's edge splits: one pixel the box's, its neighbour in
    // the block the rig's.
    for ([x, y], _) in blocks(&rig_alone, ANCHOR) {
        let pixels: Vec<bool> = (0..BLOCK * BLOCK)
            .map(|i| at(&box_alone, x + i % BLOCK, y + i / BLOCK) != background())
            .collect();
        cut += usize::from(pixels.iter().any(|p| *p) && pixels.iter().any(|p| !*p));
    }
    assert!(
        hidden > 300,
        "the box hides {hidden} of the figure's pixels"
    );
    assert!(cut >= 8, "the box's edge splits {cut} blocks");
}

fn srgb_linear(value: u8) -> f32 {
    let value = f32::from(value) / 255.0;
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_srgb(value: f32) -> u8 {
    let value = value.clamp(0.0, 1.0);
    let encoded = if value <= 0.003_130_8 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    };
    (encoded * 255.0).round() as u8
}

/// Light from the upper left and in front, and a dim ambient.
fn lights() -> Vec<RenderDiff> {
    [
        (
            30,
            LightDescriptor::Directional {
                color: [1.0, 0.95, 0.85],
                intensity: 2.0,
                enabled: true,
                direction: [1.0, -0.5, -0.6],
                range: None,
                shadow_intent: LightShadowIntent::Disabled,
                shadow: Default::default(),
            },
        ),
        (
            31,
            LightDescriptor::Ambient {
                enabled: true,
                color: [0.6, 0.7, 1.0],
                intensity: 0.2,
                shadow_intent: LightShadowIntent::Disabled,
                shadow: Default::default(),
                range: None,
            },
        ),
    ]
    .into_iter()
    .map(|(handle, light)| RenderDiff::CreateLight {
        handle: RenderHandle::new(handle),
        parent: None,
        light,
    })
    .collect()
}

/// The blocks match the smooth draw averaged offline over the same grid:
/// its coverage from the smooth draw over black and over white, and its
/// colour premultiplied by it over black, both in linear light. A block
/// draws where the coverage reaches a half, in that colour.
#[test]
fn each_block_is_the_coverage_weighted_average_of_the_smooth_draw() {
    let lit = SpriteMaterialDescriptor {
        lighting: SpriteLightingMode::Synthetic,
        normal_strength: 0.5,
        alpha: SpriteAlphaMode::Mask { cutoff: 0.5 },
        ..Default::default()
    };
    let at_origin = [ORIGIN[0], ORIGIN[1], 0.0];
    let render = |grid: u32, background: [f32; 4]| {
        scene(rig(grid, lit.clone(), at_origin), background, lights()).single(&flat_camera(0.0))
    };
    let over_black = render(0, [0.0, 0.0, 0.0, 1.0]);
    let over_white = render(0, [1.0, 1.0, 1.0, 1.0]);
    let blocked = render(BLOCK, [0.0, 0.0, 0.0, 1.0]);
    let (mut compared, mut drawn) = (0, 0);
    let mut y = ANCHOR[1] % BLOCK;
    while y + BLOCK <= HEIGHT {
        let mut x = ANCHOR[0] % BLOCK;
        while x + BLOCK <= WIDTH {
            let mut premultiplied = [0.0f32; 3];
            let mut coverage = 0.0;
            for i in 0..BLOCK * BLOCK {
                let (px, py) = (x + i % BLOCK, y + i / BLOCK);
                let (black, white) = (at(&over_black, px, py), at(&over_white, px, py));
                // Over black c·a; over white c·a + 1 − a.
                let alpha = (1.0 - (srgb_linear(white[0]) - srgb_linear(black[0])))
                    .min(1.0 - (srgb_linear(white[1]) - srgb_linear(black[1])))
                    .clamp(0.0, 1.0);
                coverage += alpha;
                for c in 0..3 {
                    premultiplied[c] += srgb_linear(black[c]);
                }
            }
            let share = coverage / (BLOCK * BLOCK) as f32;
            let block = at(&blocked, x, y);
            // Near the cutoff, 8-bit captures cannot tell which side.
            if (share - 0.5).abs() > 0.08 {
                compared += 1;
                assert_eq!(
                    block != background(),
                    share > 0.5,
                    "block ({x}, {y}) covered {share:.2}: drew {block:?}"
                );
                if share > 0.5 {
                    drawn += 1;
                    let expected = premultiplied.map(|c| linear_srgb(c / coverage));
                    assert!(
                        block[..3]
                            .iter()
                            .zip(expected)
                            .all(|(a, b)| a.abs_diff(b) <= 4),
                        "block ({x}, {y}): drew {block:?}, the smooth draw averages {expected:?}"
                    );
                }
            }
            x += BLOCK;
        }
        y += BLOCK;
    }
    assert!(
        drawn > 1_000,
        "{drawn} of {compared} blocks drawn and compared"
    );
}

#[test]
fn with_a_palette_every_pixel_drawn_is_a_palette_colour() {
    let palette = vec![
        [0.05, 0.02, 0.2],
        [0.2, 0.1, 0.25],
        [0.6, 0.15, 0.2],
        [0.15, 0.5, 0.25],
        [0.9, 0.7, 0.3],
        [0.95, 0.95, 0.9],
    ];
    let mut figure = rig(BLOCK, unlit(), [ORIGIN[0], ORIGIN[1], 0.0]);
    figure.palette = palette.clone();
    let encoded: Vec<[u8; 3]> = palette.iter().map(|c| c.map(linear_srgb)).collect();
    let frame = scene(figure, BACKGROUND, Vec::new()).single(&flat_camera(0.0));
    let mut used = std::collections::BTreeSet::new();
    let mut drawn = 0;
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let p = at(&frame, x, y);
            if p == background() {
                continue;
            }
            drawn += 1;
            let index = encoded
                .iter()
                .position(|c| c.iter().zip(&p[..3]).all(|(a, b)| a.abs_diff(*b) <= 1));
            assert!(
                index.is_some(),
                "pixel ({x}, {y}) {p:?} is no palette colour"
            );
            used.extend(index);
        }
    }
    assert!(drawn > 4_000);
    assert!(used.len() >= 4, "the figure's colours take {used:?}");
}

#[test]
fn a_rig_the_scratch_image_has_no_room_for_draws_smooth_and_is_counted() {
    // Two figures each over most of the view: the second finds no room.
    let big = |x: f32, z: f32| {
        let mut figure = rig(BLOCK, unlit(), [x, 0.5, z]);
        figure.scale = 1.6;
        figure
    };
    let mut ops = Vec::new();
    ops.extend(create(11, big(1.0, -1.0)));
    let mut harness = scene(big(0.5, 0.0), BACKGROUND, ops);
    harness.single(&flat_camera(0.0));
    let frame = harness.single(&flat_camera(0.0));
    let readout = harness.renderer.gpu_readout().parts_rig_grid;
    assert_eq!((readout.drawn, readout.smooth), (1, 1));
    // The smooth one shows where the blocked one does not cover it.
    assert!(blocks(&frame, [0, 0]).iter().any(|(_, whole)| !whole));
}

/// Under a temporal stage the world pass writes motion, and grid rigs'
/// block quads mark its reactive mask (a pipeline of their own); the rig
/// still draws as blocks.
#[test]
fn grid_rigs_draw_under_a_temporal_stage() {
    let mut harness = scene(
        rig(BLOCK, unlit(), [ORIGIN[0], ORIGIN[1], 0.0]),
        BACKGROUND,
        vec![RenderDiff::SetRendererSettings {
            settings: RendererSettingsDescriptor {
                antialiasing: Antialiasing::Temporal,
                ..RendererSettingsDescriptor::DEFAULT
            },
        }],
    );
    let samples = harness.renderer.samples();
    harness.target = render_wgpu::OffscreenTarget::new(&harness.gpu, WIDTH, HEIGHT, samples);
    let whole = viewport(0.0, 0.0, 1.0, 1.0);
    harness.renderer.set_view_composition(
        &composition(
            vec![flat_camera(0.0)],
            vec![primary_view("main", "flat", whole, 0)],
        ),
        0.0,
    );
    let mut frame = Vec::new();
    for _ in 0..4 {
        frame = harness.composition(0.0).1;
    }
    let drawn = frame
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|p| p[..3] != background()[..3])
        .count();
    assert!(drawn > 4_000, "{drawn} pixels drawn");
    assert_eq!(harness.renderer.gpu_readout().parts_rig_grid.drawn, 1);
}
