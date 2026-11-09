//! Parts rig fixtures:
//! - rig-c's gallery `squash`, posed by `animation-parts-rig`, standing in
//!   the world by each billboard mode and seen from three sides;
//! - a front-on orthographic view of it against `rigc render --poser core`;
//! - slot blend modes, the two-colour tint, premultiplied pages and draw
//!   order, against their arithmetic;
//! - pixel art snapped to the rig's texel grid;
//! - a snapshot of the world drawing the same rigs.
//!
//! `RENDER_WGPU_BLESS=1 cargo test -p render-wgpu --test parts_rigs` rewrites
//! the references. `parts_rigs/` holds rig-c's packed `squash` page and its
//! `rigc render --poser core` frame (README there).

mod common;

use animation_parts_rig::{BlendMode, Play, Player, Pose, Rig};
use common::*;
use render_host_contracts::RendererCameraProjection;
use render_model::*;
use render_wgpu::{decode_png_rgba, OffscreenTarget, RendererOptions};

const SQUASH: &str = include_str!("../../animation-parts-rig/tests/fixtures/squash.model.json");
const SQUASH_PAGE: &[u8] = include_bytes!("parts_rigs/squash.png");
const SQUASH_RIGC: &[u8] = include_bytes!("parts_rigs/squash-bounce-f0005.rigc.png");
const RIG: &str = "parts-rig/squash";
const PAGE: &str = "texture/squash";
/// Metres per rig unit: squash and its plate stand about 1.3 m.
const RIG_SCALE: f32 = 0.0016;

/// A rig's drawing as the services define it.
fn rig_asset(rig: &Rig, id: &str, pages: &[&str]) -> PartsRigAsset {
    PartsRigAsset {
        id: id.to_owned(),
        pages: rig
            .pages()
            .iter()
            .zip(pages)
            .map(|(page, texture)| PartsRigPage {
                texture: (*texture).to_owned(),
                premultiplied: page.pma,
            })
            .collect(),
        drawables: rig
            .drawables()
            .iter()
            .map(|drawable| PartsRigDrawable {
                page: drawable.page as u32,
                uvs: drawable.uvs.clone(),
                triangles: drawable.triangles.clone(),
            })
            .collect(),
    }
}

/// A pose as the services publish it.
fn rig_pose(pose: &Pose) -> PartsRigPose {
    PartsRigPose {
        seconds: pose.animation_time,
        pieces: pose
            .pieces
            .iter()
            .map(|piece| PartsRigPiece {
                drawable: piece.drawable as u32,
                blend: match piece.blend {
                    BlendMode::Normal => PartsRigBlend::Normal,
                    BlendMode::Additive => PartsRigBlend::Additive,
                    BlendMode::Multiply => PartsRigBlend::Multiply,
                    BlendMode::Screen => PartsRigBlend::Screen,
                },
                color: piece.color,
                dark: piece.dark.unwrap_or_default(),
            })
            .collect(),
        vertices: pose.vertices.as_slice().into(),
    }
}

/// `squash` playing `bounce`, posed after `steps` steps of 1/12 s, as
/// `rigc render` at 12 fps walks it.
fn squash_pose(rig: &Rig, steps: usize) -> PartsRigPose {
    let mut player = Player::new(rig);
    let bounce = rig.find_animation("bounce").expect("squash plays bounce");
    player.play(
        bounce,
        Play {
            looping: true,
            crossfade: 0.0,
        },
    );
    let mut pose = Pose::default();
    player.advance(rig, 0.0, &mut pose);
    for _ in 0..steps {
        player.advance(rig, 1.0 / 12.0, &mut pose);
    }
    rig_pose(&pose)
}

fn squash_ops(resources: &mut Resources) -> (Rig, Vec<RenderDiff>) {
    let rig = Rig::from_json(SQUASH).expect("squash reads");
    let (width, height, rgba) = decode_png_rgba(SQUASH_PAGE).expect("squash page");
    let (page, _) = resources.texture(PAGE, width, height, &rgba, TextureFilter::Linear);
    let asset = rig_asset(&rig, RIG, &[PAGE]);
    (rig, vec![page, RenderDiff::DefinePartsRig { asset }])
}

fn figure(billboard: BillboardMode, translation: [f32; 3], yaw: f32) -> PartsRigInstanceDescriptor {
    PartsRigInstanceDescriptor {
        asset: RIG.to_owned(),
        scale: RIG_SCALE,
        size_mode: SpriteSizeMode::World,
        billboard,
        tint: [1.0; 4],
        render_order: 0,
        depth: SpriteDepthPolicy::Default,
        layer: RenderLayer::Scene,
        material: SpriteMaterialDescriptor {
            alpha: SpriteAlphaMode::Mask { cutoff: 0.5 },
            ..Default::default()
        },
        texel_snap: 0.0,
        visible: true,
        transform: transform(translation, yaw, 1.0),
        metadata: RenderMetadata::default(),
    }
}

fn create(handle: u64, rig: PartsRigInstanceDescriptor, pose: &PartsRigPose) -> [RenderDiff; 2] {
    let handle = RenderHandle::new(handle);
    [
        RenderDiff::CreatePartsRig {
            handle,
            parent: None,
            rig,
        },
        RenderDiff::SetPartsRigPose {
            handle,
            pose: pose.clone(),
        },
    ]
}

/// Pixels that differ from the frame's top-left corner, the background.
fn drawn(rgba: &[u8]) -> usize {
    let background = &rgba[..4];
    rgba.as_chunks::<4>()
        .0
        .iter()
        .filter(|pixel| {
            pixel
                .iter()
                .zip(background)
                .any(|(a, b)| a.abs_diff(*b) > 8)
        })
        .count()
}

/// Four squashes on a floor, one per facing, at x = -2.1, -0.7, 0.7, 2.1:
/// authored (turned 40°), spherical, cylindrical toward the eye, and
/// cylindrical against the view's heading keeping half its own facing.
fn standing_squashes(harness: &mut Harness) -> Vec<RenderDiff> {
    let mut ops = vec![RenderDiff::SetBackgroundColor {
        color: [0.03, 0.04, 0.08, 1.0],
    }];
    ops.extend(coloured_mesh("floor", floor(12.0), [0.45, 0.45, 0.5, 1.0]));
    ops.push(instance(
        1,
        None,
        "floor",
        transform([0.0, 0.0, -5.0], 0.0, 1.0),
    ));
    let (rig, squash) = squash_ops(&mut harness.resources);
    ops.extend(squash);
    let pose = squash_pose(&rig, 5);
    // The rig's origin is at its plate's lower left; its feet stand about
    // 400 units across.
    let base = |x: f32| [x - 400.0 * RIG_SCALE, 0.0, -5.0];
    ops.extend(create(
        10,
        figure(BillboardMode::None, base(-2.1), 40.0),
        &pose,
    ));
    ops.extend(create(
        11,
        figure(BillboardMode::Spherical, base(-0.7), 0.0),
        &pose,
    ));
    ops.extend(create(
        12,
        figure(BillboardMode::Cylindrical, base(0.7), 0.0),
        &pose,
    ));
    let mut view = figure(BillboardMode::CylindricalView, base(2.1), 90.0);
    view.material.authored_facing = 0.5;
    ops.extend(create(13, view, &pose));
    ops
}

#[test]
fn parts_rigs_face_the_camera_by_billboard_mode_from_every_side() {
    let mut harness = Harness::new(RendererOptions::default());
    let ops = standing_squashes(&mut harness);
    harness.apply(ops);
    let front = harness.single(&camera("front", [0.0, 0.7, -2.0], 0.0, -5.0));
    let side = harness.single(&camera("side", [3.5, 0.9, -2.0], -45.0, -8.0));
    let above = harness.single(&camera("above", [0.0, 6.0, -3.0], 0.0, -70.0));
    for (name, pixels) in [
        ("parts_rigs_front", &front),
        ("parts_rigs_side", &side),
        ("parts_rigs_above", &above),
    ] {
        assert!(drawn(pixels) > 2_000, "{name} draws the rigs");
        assert_screenshot(name, WIDTH, HEIGHT, pixels);
    }

    // From above, a spherical rig lies back to face the eye and an upright
    // one is foreshortened.
    let eye = camera("above", [0.0, 6.0, -3.0], 0.0, -70.0);
    let floor_only = shown_alone(&mut harness, None, &eye);
    let spherical = differing_pixels(&shown_alone(&mut harness, Some(11), &eye), &floor_only);
    let cylindrical = differing_pixels(&shown_alone(&mut harness, Some(12), &eye), &floor_only);
    assert!(
        spherical > cylindrical * 3 / 2,
        "spherical {spherical} px, cylindrical {cylindrical} px"
    );
}

/// The frame with only rig `keep` of the four shown.
fn shown_alone(
    harness: &mut Harness,
    keep: Option<u64>,
    eye: &render_host_contracts::RendererCompositionCamera,
) -> Vec<u8> {
    let visibility = |shown: &dyn Fn(u64) -> bool| -> Vec<RenderDiff> {
        (10..14)
            .map(|handle| RenderDiff::Update {
                handle: RenderHandle::new(handle),
                transform: None,
                material: None,
                visible: Some(shown(handle)),
                metadata: None,
            })
            .collect()
    };
    harness.apply(visibility(&|handle| Some(handle) == keep));
    let pixels = harness.single(eye);
    harness.apply(visibility(&|_| true));
    pixels
}

/// `rigc render --candidate squash --poser core --animation bounce --max 256`
/// frame 5: its viewport, x -32, y -32, 864 × 784 rig units at 256/864 pixels
/// a unit, over (232, 232, 232).
#[test]
fn a_front_on_orthographic_rig_matches_rig_c_core_render() {
    let (width, height, expected) = decode_png_rgba(SQUASH_RIGC).expect("rigc frame");
    assert_eq!((width, height), (256, 232));
    let mut harness = Harness::new(RendererOptions::default());
    harness.target = OffscreenTarget::new(&harness.gpu, width, height, 1);
    let background = ((232.0f32 / 255.0 + 0.055) / 1.055).powf(2.4);
    let mut ops = vec![RenderDiff::SetBackgroundColor {
        color: [background, background, background, 1.0],
    }];
    let (rig, squash) = squash_ops(&mut harness.resources);
    ops.extend(squash);
    // rigc composites every piece over what is behind it by its alpha.
    let mut rig_figure = figure(BillboardMode::None, [0.0; 3], 0.0);
    rig_figure.scale = 1.0;
    rig_figure.material.alpha = SpriteAlphaMode::Blend;
    ops.extend(create(10, rig_figure, &squash_pose(&rig, 5)));
    harness.apply(ops);
    let scale = 256.0 / 864.000_018_044_288_3;
    let (min_x, max_y) = (
        -32.000_017_375_981_37,
        -32.000_000_668_306_974 + 784.000_001_336_613_8,
    );
    let tall = f64::from(height) / scale;
    let mut eye = camera(
        "rigc",
        [min_x + 432.000_009, max_y - tall * 0.5, 100.0],
        0.0,
        0.0,
    );
    eye.projection = RendererCameraProjection::Orthographic {
        vertical_size: tall,
        near: 1.0,
        far: 200.0,
    };
    let actual = harness.single(&eye);
    let differing = differing_pixels(&expected, &actual);
    let fraction = differing as f64 / f64::from(width * height);
    if let Some(out) = std::env::var_os("PARTS_RIG_EVIDENCE") {
        let out = std::path::PathBuf::from(out);
        std::fs::create_dir_all(&out).unwrap();
        std::fs::write(
            out.join("squash-bounce-f0005.engine.png"),
            render_wgpu::encode_png(width, height, &actual).unwrap(),
        )
        .unwrap();
        std::fs::write(
            out.join("squash-bounce-f0005.compare.txt"),
            format!(
                "{differing} of {} pixels differ by more than {CHANNEL_TOLERANCE} in a channel ({:.3}%)\n",
                width * height,
                fraction * 100.0
            ),
        )
        .unwrap();
    }
    assert!(
        fraction < 0.01,
        "{differing} pixels ({:.2}%) differ from rigc's frame by more than {CHANNEL_TOLERANCE}",
        fraction * 100.0
    );
}

/// An 8×8 page: four 4×4 quadrants, red, green, blue and grey, straight
/// alpha `alpha`, or premultiplied in their encoded values.
fn quadrant_page(alpha: u8, premultiplied: bool) -> Vec<u8> {
    let colors = [
        [200u8, 60, 40],
        [60, 180, 80],
        [50, 90, 210],
        [128, 128, 128],
    ];
    let mut rgba = Vec::new();
    for y in 0..8 {
        for x in 0..8 {
            let [r, g, b] = colors[(y / 4) * 2 + x / 4];
            let scale = |c: u8| {
                if premultiplied {
                    ((u32::from(c) * u32::from(alpha) + 127) / 255) as u8
                } else {
                    c
                }
            };
            rgba.extend_from_slice(&[scale(r), scale(g), scale(b), alpha]);
        }
    }
    rgba
}

/// A quad drawable over one quadrant (`quadrant` 0 to 3) of a page.
fn quadrant(page: u32, quadrant: u32) -> PartsRigDrawable {
    let (u, v) = (
        (quadrant % 2) as f32 * 0.5 + 0.125,
        (quadrant / 2) as f32 * 0.5 + 0.125,
    );
    PartsRigDrawable {
        page,
        // Bottom left, bottom right, top right, top left (v down).
        uvs: vec![[u, v + 0.25], [u + 0.25, v + 0.25], [u + 0.25, v], [u, v]],
        triangles: vec![0, 1, 2, 2, 3, 0],
    }
}

fn square(x: f32, y: f32, size: f32) -> [[f32; 2]; 4] {
    [[x, y], [x + size, y], [x + size, y + size], [x, y + size]]
}

fn piece(drawable: u32, blend: PartsRigBlend) -> PartsRigPiece {
    PartsRigPiece {
        drawable,
        blend,
        color: [1.0; 4],
        dark: [0.0; 3],
    }
}

fn pose(pieces: Vec<(PartsRigPiece, [[f32; 2]; 4])>) -> PartsRigPose {
    PartsRigPose {
        seconds: 0.0,
        vertices: pieces
            .iter()
            .flat_map(|(_, corners)| *corners)
            .collect::<Vec<_>>()
            .into(),
        pieces: pieces.into_iter().map(|(piece, _)| piece).collect(),
    }
}

/// An orthographic camera over x 0 to 16 m and y 0 to 9 m: 20 pixels a
/// metre on the harness's 320 × 180 target.
fn flat_camera() -> render_host_contracts::RendererCompositionCamera {
    let mut eye = camera("flat", [8.0, 4.5, 50.0], 0.0, 0.0);
    eye.projection = RendererCameraProjection::Orthographic {
        vertical_size: 9.0,
        near: 1.0,
        far: 100.0,
    };
    eye
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

/// The pixel at (x, y) metres under `flat_camera`.
fn at(rgba: &[u8], x: f32, y: f32) -> [u8; 4] {
    let px = (x / 16.0 * WIDTH as f32) as u32;
    let py = ((9.0 - y) / 9.0 * HEIGHT as f32) as u32;
    pixel(rgba, WIDTH, px, py)
}

#[track_caller]
fn assert_near(actual: [u8; 4], expected: [f32; 3], what: &str) {
    let expected = expected.map(linear_srgb);
    assert!(
        actual[..3]
            .iter()
            .zip(expected)
            .all(|(a, b)| a.abs_diff(b) <= 3),
        "{what}: drew {actual:?}, expected {expected:?}"
    );
}

#[test]
fn slot_blend_modes_dark_tint_premultiplied_pages_and_draw_order_follow_their_arithmetic() {
    let mut harness = Harness::new(RendererOptions::default());
    let mut ops = vec![RenderDiff::SetBackgroundColor {
        color: [0.0, 0.0, 0.0, 1.0],
    }];
    let (straight, _) = harness.resources.texture(
        "texture/straight",
        8,
        8,
        &quadrant_page(255, false),
        TextureFilter::Nearest,
    );
    let (half, _) = harness.resources.texture(
        "texture/half",
        8,
        8,
        &quadrant_page(128, false),
        TextureFilter::Nearest,
    );
    let (premultiplied, _) = harness.resources.texture(
        "texture/premultiplied",
        8,
        8,
        &quadrant_page(128, true),
        TextureFilter::Nearest,
    );
    ops.extend([straight, half, premultiplied]);
    let mut drawables = Vec::new();
    for page in 0..3 {
        drawables.extend((0..4).map(|q| quadrant(page, q)));
    }
    ops.push(RenderDiff::DefinePartsRig {
        asset: PartsRigAsset {
            id: "parts-rig/blends".to_owned(),
            pages: vec![
                PartsRigPage {
                    texture: "texture/straight".to_owned(),
                    premultiplied: false,
                },
                PartsRigPage {
                    texture: "texture/half".to_owned(),
                    premultiplied: false,
                },
                PartsRigPage {
                    texture: "texture/premultiplied".to_owned(),
                    premultiplied: true,
                },
            ],
            drawables: drawables.into(),
        },
    });
    let mut rig = figure(BillboardMode::None, [0.0; 3], 0.0);
    rig.asset = "parts-rig/blends".to_owned();
    rig.scale = 1.0;
    rig.material.alpha = SpriteAlphaMode::Blend;
    // Over a grey square: red (drawable 0) under each blend mode, a grey
    // square tinted with a dark colour, the half-alpha red from a straight
    // and a premultiplied page, and green under blue, then over it.
    let mut tinted = piece(3, PartsRigBlend::Normal);
    tinted.color = [1.0, 0.5, 0.5, 1.0];
    tinted.dark = [0.0, 0.0, 1.0];
    let first = pose(vec![
        (piece(3, PartsRigBlend::Normal), square(0.0, 0.0, 16.0)),
        (piece(0, PartsRigBlend::Normal), square(0.5, 5.0, 3.0)),
        (piece(0, PartsRigBlend::Additive), square(4.5, 5.0, 3.0)),
        (piece(0, PartsRigBlend::Multiply), square(8.5, 5.0, 3.0)),
        (piece(0, PartsRigBlend::Screen), square(12.5, 5.0, 3.0)),
        (tinted, square(0.5, 0.5, 3.0)),
        (piece(4, PartsRigBlend::Normal), square(4.5, 0.5, 3.0)),
        (piece(8, PartsRigBlend::Normal), square(8.5, 0.5, 3.0)),
        (piece(1, PartsRigBlend::Normal), square(12.0, 0.5, 2.5)),
        (piece(2, PartsRigBlend::Normal), square(13.0, 1.5, 2.5)),
    ]);
    ops.extend(create(10, rig, &first));
    harness.apply(ops);
    let eye = flat_camera();
    let frame = harness.single(&eye);

    let red = [200, 60, 40].map(srgb_linear);
    let grey = srgb_linear(128);
    let bg = [grey; 3];
    let each = |f: &dyn Fn(f32, f32) -> f32, a: [f32; 3], b: [f32; 3]| {
        [f(a[0], b[0]), f(a[1], b[1]), f(a[2], b[2])]
    };
    assert_near(at(&frame, 2.0, 6.5), red, "normal");
    assert_near(
        at(&frame, 6.0, 6.5),
        each(&|t, b| t + b, red, bg),
        "additive",
    );
    assert_near(
        at(&frame, 10.0, 6.5),
        each(&|t, b| t * b, red, bg),
        "multiply",
    );
    assert_near(
        at(&frame, 14.0, 6.5),
        each(&|t, b| t + b * (1.0 - t), red, bg),
        "screen",
    );
    // Spine's two-colour tint in linear light: (1 - t)·dark + t·light.
    let light = [1.0, srgb_linear(128), srgb_linear(128)];
    let dark = [0.0, 0.0, 1.0];
    let tinted = [0, 1, 2].map(|c| (1.0 - grey) * dark[c] + grey * light[c]);
    assert_near(at(&frame, 2.0, 2.0), tinted, "dark tint");
    let alpha = 128.0 / 255.0;
    let half_red = each(&|t, b| t * alpha + b * (1.0 - alpha), red, bg);
    assert_near(at(&frame, 6.0, 2.0), half_red, "half-alpha straight page");
    assert_near(
        at(&frame, 10.0, 2.0),
        half_red,
        "half-alpha premultiplied page",
    );
    let green = [60, 180, 80].map(srgb_linear);
    let blue = [50, 90, 210].map(srgb_linear);
    assert_near(at(&frame, 14.0, 2.5), blue, "blue drawn after green");

    // The next pose draws green last: draw order is the pose's.
    let mut second: Vec<_> = first
        .pieces
        .iter()
        .copied()
        .zip(first.vertices.as_chunks::<4>().0.iter().copied())
        .collect();
    second.swap(8, 9);
    harness.apply(vec![RenderDiff::SetPartsRigPose {
        handle: RenderHandle::new(10),
        pose: pose(second),
    }]);
    assert_near(
        at(&harness.single(&eye), 14.0, 2.5),
        green,
        "green drawn after blue",
    );
}

/// A 12×12 page of an 8×8 checker of 1-texel cells inside a 2-texel clear
/// border.
fn checker_page() -> Vec<u8> {
    let mut rgba = Vec::new();
    for y in 0..12 {
        for x in 0..12 {
            let inside = (2..10).contains(&x) && (2..10).contains(&y);
            rgba.extend_from_slice(match (inside, (x + y) % 2 == 0) {
                (false, _) => &[0, 0, 0, 0],
                (true, true) => &[240, 200, 40, 255],
                (true, false) => &[40, 60, 200, 255],
            });
        }
    }
    rgba
}

#[test]
fn texel_snapped_rigs_draw_whole_cells_of_their_grid() {
    let mut harness = Harness::new(RendererOptions::default());
    let mut ops = vec![RenderDiff::SetBackgroundColor {
        color: [0.0, 0.0, 0.0, 1.0],
    }];
    let (page, _) = harness.resources.texture(
        "texture/checker",
        12,
        12,
        &checker_page(),
        TextureFilter::Nearest,
    );
    ops.push(page);
    ops.push(RenderDiff::DefinePartsRig {
        asset: PartsRigAsset {
            id: "parts-rig/checker".to_owned(),
            pages: vec![PartsRigPage {
                texture: "texture/checker".to_owned(),
                premultiplied: false,
            }],
            drawables: vec![PartsRigDrawable {
                page: 0,
                uvs: vec![[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
                triangles: vec![0, 1, 2, 2, 3, 0],
            }]
            .into(),
        },
    });
    // The page's 12 texels at 1 rig unit each, turned 27° about their
    // centre, which stands at rig unit (32, 18). (At 30° cell centres fall
    // exactly on texel edges, where either texel is right.)
    let turn = 27f32.to_radians();
    let corners: Vec<[f32; 2]> = [[0.0, 0.0], [12.0, 0.0], [12.0, 12.0], [0.0, 12.0]]
        .iter()
        .map(|[x, y]| {
            let (x, y) = (x - 6.0, y - 6.0);
            [
                x * turn.cos() - y * turn.sin() + 32.0,
                x * turn.sin() + y * turn.cos() + 18.0,
            ]
        })
        .collect();
    let posed = PartsRigPose {
        seconds: 0.0,
        pieces: vec![piece(0, PartsRigBlend::Normal)].into(),
        vertices: corners.into(),
    };
    let mut rig = figure(BillboardMode::None, [0.0; 3], 0.0);
    rig.asset = "parts-rig/checker".to_owned();
    rig.scale = 0.25;
    rig.texel_snap = 1.0;
    ops.extend(create(10, rig.clone(), &posed));
    rig.texel_snap = 0.0;
    rig.transform = transform([5.0, 0.0, 0.0], 0.0, 1.0);
    ops.extend(create(11, rig, &posed));
    harness.apply(ops);
    let frame = harness.single(&flat_camera());

    // At 20 pixels a metre, a rig unit (1/4 m) is 5 pixels, its grid on the
    // pixels': every 5×5 cell of the snapped rig (pixels 110 to 210) is one
    // colour, and the smooth one (210 to 310) has cells that are not.
    let cell_colours = |x0: u32| {
        let (mut whole, mut split) = (0, 0);
        for cy in 0..36 {
            for cx in 0..20 {
                let (px, py) = (x0 + cx * 5, cy * 5);
                let first = pixel(&frame, WIDTH, px, py);
                let uniform = (0..5)
                    .all(|dy| (0..5).all(|dx| pixel(&frame, WIDTH, px + dx, py + dy) == first));
                if first[..3] != [0, 0, 0] || !uniform {
                    if uniform {
                        whole += 1;
                    } else {
                        split += 1;
                    }
                }
            }
        }
        (whole, split)
    };
    let (snapped_whole, snapped_split) = cell_colours(110);
    let (smooth_whole, smooth_split) = cell_colours(210);
    assert!(
        snapped_whole >= 50,
        "the snapped rig draws whole cells: {snapped_whole}"
    );
    assert_eq!(snapped_split, 0, "no snapped cell is split");
    assert!(
        smooth_split > 8,
        "the smooth rig splits cells: {smooth_split} ({smooth_whole} whole)"
    );
    assert_screenshot("parts_rigs_texel_snap", WIDTH, HEIGHT, &frame);
}

#[test]
fn a_snapshot_of_the_world_draws_the_same_rigs() {
    let mut harness = Harness::new(RendererOptions::default());
    let ops = standing_squashes(&mut harness);
    harness.apply(ops);
    let eye = camera("front", [0.0, 0.7, -2.0], 0.0, -5.0);
    let live = harness.single(&eye);
    let snapshot = harness.world.snapshot().frame;
    assert!(snapshot
        .ops
        .iter()
        .any(|op| matches!(op, RenderDiff::SetPartsRigPose { .. })));
    let mut copy = Harness::new(RendererOptions::default());
    copy.resources = std::mem::take(&mut harness.resources);
    copy.apply(snapshot.ops);
    assert_eq!(differing_pixels(&live, &copy.single(&eye)), 0);
}

#[test]
fn a_rig_without_a_pose_or_drawing_draws_nothing() {
    let mut harness = Harness::new(RendererOptions::default());
    let (rig, squash) = squash_ops(&mut harness.resources);
    let pose = squash_pose(&rig, 0);
    let mut ops = vec![RenderDiff::SetBackgroundColor {
        color: [0.1, 0.1, 0.1, 1.0],
    }];
    ops.push(RenderDiff::CreatePartsRig {
        handle: RenderHandle::new(10),
        parent: None,
        rig: figure(BillboardMode::Spherical, [-0.6, -0.6, -3.0], 0.0),
    });
    harness.apply(ops);
    let eye = camera("front", [0.0, 0.0, 0.0], 0.0, 0.0);
    let empty = harness.single(&eye);
    assert_eq!(drawn(&empty), 0, "no pose");
    harness.apply(vec![RenderDiff::SetPartsRigPose {
        handle: RenderHandle::new(10),
        pose: pose.clone(),
    }]);
    assert_eq!(drawn(&harness.single(&eye)), 0, "no drawing");
    harness.apply(squash);
    assert!(drawn(&harness.single(&eye)) > 1_000, "posed and drawn");
}
