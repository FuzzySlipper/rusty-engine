//! Parts rig fixtures:
//! - rig-c's gallery `squash`, posed by `animation-parts-rig`, standing in
//!   the world by each billboard mode and seen from three sides;
//! - a front-on orthographic view of it against `rigc render --poser core`;
//! - the same for a rig of clipping attachments: convex and weighted,
//!   concave, inverse, a concave one as its hull, and an alpha-derived
//!   aperture;
//! - slot blend modes, the two-colour tint, premultiplied pages and draw
//!   order, against their arithmetic;
//! - pixel art snapped to the rig's texel grid;
//! - a snapshot of the world drawing the same rigs;
//! - a rig of several views, each camera drawing the view nearest its
//!   bearing, and a mirrored view drawn flipped.
//!
//! `RENDER_WGPU_BLESS=1 cargo test -p render-wgpu --test parts_rigs` rewrites
//! the references. `parts_rigs/` holds rig-c's packed `squash` and
//! `clipping` pages and their `rigc render --poser core` frames (README
//! there).

mod common;

use animation_parts_rig::{BlendMode, Play, Player, Pose, Rig};
use common::*;
use render_host_contracts::RendererCameraProjection;
use render_model::*;
use render_wgpu::{decode_png_rgba, OffscreenTarget, RendererOptions};

const SQUASH: &str = include_str!("../../animation-parts-rig/tests/fixtures/squash.model.json");
const SQUASH_PAGE: &[u8] = include_bytes!("parts_rigs/squash.png");
const SQUASH_RIGC: &[u8] = include_bytes!("parts_rigs/squash-bounce-f0005.rigc.png");
const CLIPPING: &str = include_str!("../../animation-parts-rig/tests/fixtures/clipping.model.json");
const CLIPPING_PAGE: &[u8] = include_bytes!("parts_rigs/clipping.png");
const CLIPPING_RIGC: &[u8] = include_bytes!("parts_rigs/clipping-a-f0004.rigc.png");
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
                detail: None,
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
                clipped: piece.clipped.as_ref().map(|c| PartsRigClipped {
                    vertices: c.uvs.len() as u32,
                    triangles: c.triangles.len() as u32,
                }),
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
        uvs: pose.uvs.as_slice().into(),
        triangles: pose.triangles.as_slice().into(),
    }
}

/// `squash` playing `bounce`, posed after `steps` steps of 1/12 s, as
/// `rigc render` at 12 fps walks it.
fn squash_pose(rig: &Rig, steps: usize) -> PartsRigPose {
    rigc_pose(rig, "bounce", steps)
}

/// `rig` playing `clip`, posed after `steps` steps of 1/12 s.
fn rigc_pose(rig: &Rig, clip: &str, steps: usize) -> PartsRigPose {
    let mut player = Player::new(rig);
    let bounce = rig.find_animation(clip).expect("the rig plays the clip");
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
    rig_ops(resources, SQUASH, SQUASH_PAGE, RIG, PAGE)
}

/// A rig read from its model document, and the ops defining its one page
/// and its drawing.
fn rig_ops(
    resources: &mut Resources,
    model: &str,
    png: &[u8],
    id: &str,
    page: &str,
) -> (Rig, Vec<RenderDiff>) {
    let rig = Rig::from_json(model).expect("the rig reads");
    let (width, height, rgba) = decode_png_rgba(png).expect("the rig's page");
    let (texture, _) = resources.texture(page, width, height, &rgba, TextureFilter::Linear);
    let asset = rig_asset(&rig, id, &[page]);
    (rig, vec![texture, RenderDiff::DefinePartsRig { asset }])
}

fn figure(billboard: BillboardMode, translation: [f32; 3], yaw: f32) -> PartsRigInstanceDescriptor {
    PartsRigInstanceDescriptor {
        views: vec![PartsRigView {
            asset: RIG.to_owned(),
            heading: 0.0,
            mirror: false,
        }],
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
            view: 0,
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

/// A `rigc render` frame's viewport (its `frames.json`): the lower left
/// corner and size in rig units, and pixels per rig unit. The image's top
/// row is the viewport's top.
struct Viewport {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    scale: f64,
}

/// The Engine's front-on orthographic view of rig `id` posed in `pose`, over
/// a `rigc render --poser core` frame's viewport and background (232 grey),
/// compared with that frame: rigc composites every piece over what is
/// behind it by its alpha. Writes the frame and the count to
/// `PARTS_RIG_EVIDENCE` when it is set; returns the fraction of pixels that
/// differ by more than `CHANNEL_TOLERANCE` in a channel.
fn compare_with_rigc(
    rig_ops: impl FnOnce(&mut Resources) -> (Rig, Vec<RenderDiff>),
    id: &str,
    pose: impl FnOnce(&Rig) -> PartsRigPose,
    frame: &[u8],
    viewport: Viewport,
    name: &str,
) -> f64 {
    let (width, height, expected) = decode_png_rgba(frame).expect("rigc frame");
    let mut harness = Harness::new(RendererOptions::default());
    harness.target = OffscreenTarget::new(&harness.gpu, width, height, 1);
    let background = ((232.0f32 / 255.0 + 0.055) / 1.055).powf(2.4);
    let mut ops = vec![RenderDiff::SetBackgroundColor {
        color: [background, background, background, 1.0],
    }];
    let (rig, defined) = rig_ops(&mut harness.resources);
    ops.extend(defined);
    let mut rig_figure = figure(BillboardMode::None, [0.0; 3], 0.0);
    rig_figure.views[0].asset = id.to_owned();
    rig_figure.scale = 1.0;
    rig_figure.material.alpha = SpriteAlphaMode::Blend;
    ops.extend(create(10, rig_figure, &pose(&rig)));
    harness.apply(ops);
    let tall = f64::from(height) / viewport.scale;
    let mut eye = camera(
        "rigc",
        [
            viewport.x + viewport.width * 0.5,
            viewport.y + viewport.height - tall * 0.5,
            100.0,
        ],
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
            out.join(format!("{name}.engine.png")),
            render_wgpu::encode_png(width, height, &actual).unwrap(),
        )
        .unwrap();
        std::fs::write(
            out.join(format!("{name}.compare.txt")),
            format!(
                "{differing} of {} pixels differ by more than {CHANNEL_TOLERANCE} in a channel ({:.3}%)\n",
                width * height,
                fraction * 100.0
            ),
        )
        .unwrap();
    }
    fraction
}

/// `rigc render --candidate squash --poser core --animation bounce --max 256`
/// frame 5: its viewport, x -32, y -32, 864 × 784 rig units at 256/864 pixels
/// a unit.
#[test]
fn a_front_on_orthographic_rig_matches_rig_c_core_render() {
    let width = 864.000_018_044_288_3;
    let fraction = compare_with_rigc(
        squash_ops,
        RIG,
        |rig| squash_pose(rig, 5),
        SQUASH_RIGC,
        Viewport {
            x: -32.000_017_375_981_37,
            y: -32.000_000_668_306_974,
            width,
            height: 784.000_001_336_613_8,
            scale: 256.0 / width,
        },
        "squash-bounce-f0005",
    );
    assert!(
        fraction < 0.01,
        "{:.2}% of pixels differ from rigc's frame by more than {CHANNEL_TOLERANCE}",
        fraction * 100.0
    );
}

/// `rigc render --candidate clipping --poser core --animation a --max 512`
/// frame 4: a hexagon weighted to two bones cutting a bar, a ball and an ear
/// the draw order moved under it, with a second clip inside it ignored; a
/// concave L cutting a torso; an inverse diamond cutting a hole in a plate;
/// a concave arrow clipping by its hull; an iris inside an aperture traced
/// from its socket's alpha; and a clip with no end slot.
#[test]
fn clipped_pieces_match_rig_c_core_render() {
    let fraction = compare_with_rigc(
        |resources| {
            rig_ops(
                resources,
                CLIPPING,
                CLIPPING_PAGE,
                "parts-rig/clipping",
                "texture/clipping",
            )
        },
        "parts-rig/clipping",
        |rig| rigc_pose(rig, "a", 4),
        CLIPPING_RIGC,
        Viewport {
            x: -20.000_019_457_014_85,
            y: -12.324_840_324_946_727,
            width: 864.000_021_603_022_9,
            height: 739.819_127_231_513_6,
            scale: 0.592_592_577_775_704_8,
        },
        "clipping-a-f0004",
    );
    assert!(
        fraction < 0.01,
        "{:.2}% of pixels differ from rigc's frame by more than {CHANNEL_TOLERANCE}",
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
        clipped: None,
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
        uvs: Default::default(),
        triangles: Default::default(),
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
                    detail: None,
                },
                PartsRigPage {
                    texture: "texture/half".to_owned(),
                    premultiplied: false,
                    detail: None,
                },
                PartsRigPage {
                    texture: "texture/premultiplied".to_owned(),
                    premultiplied: true,
                    detail: None,
                },
            ],
            drawables: drawables.into(),
        },
    });
    let mut rig = figure(BillboardMode::None, [0.0; 3], 0.0);
    rig.views[0].asset = "parts-rig/blends".to_owned();
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
        view: 0,
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
                detail: None,
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
        uvs: Default::default(),
        triangles: Default::default(),
    };
    let mut rig = figure(BillboardMode::None, [0.0; 3], 0.0);
    rig.views[0].asset = "parts-rig/checker".to_owned();
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
        view: 0,
        pose: pose.clone(),
    }]);
    assert_eq!(drawn(&harness.single(&eye)), 0, "no drawing");
    harness.apply(squash);
    assert!(drawn(&harness.single(&eye)) > 1_000, "posed and drawn");
}

/// The blends asset: `quadrant_page` straight, at full alpha.
fn blends_rig(harness: &mut Harness, id: &str) -> RenderDiff {
    let (page, _) = harness.resources.texture(
        "texture/straight",
        8,
        8,
        &quadrant_page(255, false),
        TextureFilter::Nearest,
    );
    harness.apply(vec![page]);
    RenderDiff::DefinePartsRig {
        asset: PartsRigAsset {
            id: id.to_owned(),
            pages: vec![PartsRigPage {
                texture: "texture/straight".to_owned(),
                premultiplied: false,
                detail: None,
            }],
            drawables: (0..4).map(|q| quadrant(0, q)).collect::<Vec<_>>().into(),
        },
    }
}

fn flat_rig(id: &str, z: f32, material: SpriteMaterialDescriptor) -> PartsRigInstanceDescriptor {
    let mut rig = figure(BillboardMode::None, [0.0, 0.0, z], 0.0);
    rig.views[0].asset = id.to_owned();
    rig.scale = 1.0;
    rig.material = material;
    rig
}

#[test]
fn a_solid_rigs_additive_multiply_and_screen_pieces_blend_over_the_solids_behind_it() {
    let mut harness = Harness::new(RendererOptions::default());
    let mut ops = vec![RenderDiff::SetBackgroundColor {
        color: [0.0, 0.0, 0.0, 1.0],
    }];
    ops.push(blends_rig(&mut harness, "parts-rig/blends"));
    let mask = SpriteMaterialDescriptor {
        alpha: SpriteAlphaMode::Mask { cutoff: 0.5 },
        ..Default::default()
    };
    // Behind: a solid grey square. In front, a metre nearer: a solid rig
    // whose red normal piece overlaps the square's corner, and whose
    // additive, multiply and screen pieces lie over the square, outside
    // its normal piece. Solid draws go front to back, so the nearer rig
    // would draw first.
    let behind = pose(vec![(
        piece(3, PartsRigBlend::Normal),
        [[5.5, 2.0], [11.0, 2.0], [11.0, 7.0], [5.5, 7.0]],
    )]);
    let strip = |x: f32| [[x, 3.0], [x + 1.0, 3.0], [x + 1.0, 6.0], [x, 6.0]];
    let front = pose(vec![
        (piece(0, PartsRigBlend::Normal), square(3.0, 0.5, 3.0)),
        (piece(0, PartsRigBlend::Additive), strip(6.0)),
        (piece(0, PartsRigBlend::Multiply), strip(7.5)),
        (piece(0, PartsRigBlend::Screen), strip(9.0)),
    ]);
    ops.extend(create(
        11,
        flat_rig("parts-rig/blends", -1.0, mask.clone()),
        &behind,
    ));
    ops.extend(create(10, flat_rig("parts-rig/blends", 0.0, mask), &front));
    harness.apply(ops);
    let frame = harness.single(&flat_camera());

    let red = [200, 60, 40].map(srgb_linear);
    let grey = [srgb_linear(128); 3];
    let each = |f: &dyn Fn(f32, f32) -> f32| [0, 1, 2].map(|c| f(red[c], grey[c]));
    assert_near(
        at(&frame, 5.75, 2.7),
        red,
        "the normal piece covers the square",
    );
    assert_near(
        at(&frame, 6.5, 4.5),
        each(&|t, b| t + b),
        "additive over the square",
    );
    assert_near(
        at(&frame, 8.0, 4.5),
        each(&|t, b| t * b),
        "multiply over the square",
    );
    assert_near(
        at(&frame, 9.5, 4.5),
        each(&|t, b| t + b * (1.0 - t)),
        "screen over the square",
    );
    assert_near(at(&frame, 10.5, 4.5), grey, "the square beside them");
}

/// An 8×8 detail page: normals tilted `tilt` toward +x (negative toward
/// −x) everywhere, linear, or heights rising toward +x in red.
fn detail_page(tilt: i32, heights: bool) -> Vec<u8> {
    (0..64)
        .flat_map(|index: i32| {
            if heights {
                let height = (index % 8 * 32) as u8;
                [height, height, height, 255]
            } else {
                let z = 255 - tilt.abs();
                [(128 + tilt) as u8, 128, z as u8, 255]
            }
        })
        .collect()
}

/// A detail page (`detail_page`) as a linear texture.
fn detail_texture(harness: &mut Harness, id: &str, tilt: i32, heights: bool) -> RenderDiff {
    let (mut detail, _) =
        harness
            .resources
            .texture(id, 8, 8, &detail_page(tilt, heights), TextureFilter::Linear);
    if let RenderDiff::DefineTexture { texture } = &mut detail {
        if let Some(payload) = texture.payload.as_mut() {
            payload.color_space = TextureColorSpace::Linear;
        }
    }
    detail
}

/// Light from the left and in front, and a dim ambient.
fn side_lights() -> [RenderDiff; 2] {
    [
        (
            30,
            LightDescriptor::Directional {
                color: [1.0, 1.0, 1.0],
                intensity: 2.0,
                enabled: true,
                direction: [1.0, 0.0, -0.4],
                range: None,
                shadow_intent: LightShadowIntent::Disabled,
                shadow: Default::default(),
            },
        ),
        (
            31,
            LightDescriptor::Ambient {
                enabled: true,
                color: [1.0, 1.0, 1.0],
                intensity: 0.1,
                shadow_intent: LightShadowIntent::Disabled,
                shadow: Default::default(),
                range: None,
            },
        ),
    ]
    .map(|(handle, light)| RenderDiff::CreateLight {
        handle: RenderHandle::new(handle),
        parent: None,
        light,
    })
}

/// Grey squares lit by `lighting`, masked.
fn lit(lighting: SpriteLightingMode) -> SpriteMaterialDescriptor {
    SpriteMaterialDescriptor {
        lighting,
        normal_strength: 1.0,
        alpha: SpriteAlphaMode::Mask { cutoff: 0.5 },
        ..Default::default()
    }
}

#[test]
fn authored_normal_and_depth_lighting_read_the_pages_detail_maps() {
    let mut harness = Harness::new(RendererOptions {
        default_world_lights: false,
        ..RendererOptions::default()
    });
    let mut ops = vec![RenderDiff::SetBackgroundColor {
        color: [0.0, 0.0, 0.0, 1.0],
    }];
    ops.push(blends_rig(&mut harness, "parts-rig/flat"));
    for (id, tilt, heights) in [
        ("texture/left", -90, false),
        ("texture/right", 90, false),
        ("texture/ramp", 0, true),
    ] {
        ops.push(detail_texture(&mut harness, id, tilt, heights));
        ops.push(RenderDiff::DefinePartsRig {
            asset: PartsRigAsset {
                id: format!("parts-rig/{}", &id["texture/".len()..]),
                pages: vec![PartsRigPage {
                    texture: "texture/straight".to_owned(),
                    premultiplied: false,
                    detail: Some(id.to_owned()),
                }],
                drawables: (0..4).map(|q| quadrant(0, q)).collect::<Vec<_>>().into(),
            },
        });
    }
    ops.extend(side_lights());
    // Grey squares (drawable 3), lit by authored normals tilted left, flat
    // (a page with no detail map) and tilted right, by authored depth over
    // a ramp, and by authored depth over no map (its colour's gradient).
    for (handle, asset, lighting, x) in [
        (
            10,
            "parts-rig/left",
            SpriteLightingMode::AuthoredNormal,
            0.5,
        ),
        (
            11,
            "parts-rig/flat",
            SpriteLightingMode::AuthoredNormal,
            3.5,
        ),
        (
            12,
            "parts-rig/right",
            SpriteLightingMode::AuthoredNormal,
            6.5,
        ),
        (13, "parts-rig/ramp", SpriteLightingMode::AuthoredDepth, 9.5),
        (
            14,
            "parts-rig/flat",
            SpriteLightingMode::AuthoredDepth,
            12.5,
        ),
    ] {
        let posed = pose(vec![(piece(3, PartsRigBlend::Normal), square(x, 3.0, 2.5))]);
        ops.extend(create(handle, flat_rig(asset, 0.0, lit(lighting)), &posed));
    }
    harness.apply(ops);
    let frame = harness.single(&flat_camera());
    let brightness = |x: f32| {
        at(&frame, x + 1.25, 4.25)[..3]
            .iter()
            .map(|c| u32::from(*c))
            .sum::<u32>()
    };
    let (left, flat, right, ramp, plain) = (
        brightness(0.5),
        brightness(3.5),
        brightness(6.5),
        brightness(9.5),
        brightness(12.5),
    );
    assert!(
        left > flat + 30 && flat > right + 30,
        "normals toward the light shade brighter: left {left}, flat {flat}, right {right}"
    );
    assert!(
        ramp > plain + 12,
        "heights rising away from the light turn the square toward it: ramp {ramp}, no map {plain}"
    );
    assert_screenshot("parts_rigs_authored_lighting", WIDTH, HEIGHT, &frame);
}

/// Two pages share one colour texture and differ in their normal maps:
/// consecutive pieces from them each light by their own page's map.
#[test]
fn pages_sharing_a_colour_texture_light_by_their_own_detail_maps() {
    let mut harness = Harness::new(RendererOptions {
        default_world_lights: false,
        ..RendererOptions::default()
    });
    let mut ops = vec![RenderDiff::SetBackgroundColor {
        color: [0.0, 0.0, 0.0, 1.0],
    }];
    ops.push(blends_rig(&mut harness, "parts-rig/flat"));
    ops.push(detail_texture(&mut harness, "texture/left", -90, false));
    ops.push(detail_texture(&mut harness, "texture/right", 90, false));
    ops.push(RenderDiff::DefinePartsRig {
        asset: PartsRigAsset {
            id: "parts-rig/two-pages".to_owned(),
            pages: ["texture/left", "texture/right"]
                .map(|detail| PartsRigPage {
                    texture: "texture/straight".to_owned(),
                    premultiplied: false,
                    detail: Some(detail.to_owned()),
                })
                .to_vec(),
            drawables: vec![quadrant(0, 3), quadrant(1, 3)].into(),
        },
    });
    ops.extend(side_lights());
    let posed = pose(vec![
        (piece(0, PartsRigBlend::Normal), square(3.0, 3.0, 2.5)),
        (piece(1, PartsRigBlend::Normal), square(9.0, 3.0, 2.5)),
    ]);
    ops.extend(create(
        10,
        flat_rig(
            "parts-rig/two-pages",
            0.0,
            lit(SpriteLightingMode::AuthoredNormal),
        ),
        &posed,
    ));
    harness.apply(ops);
    let frame = harness.single(&flat_camera());
    let brightness = |x: f32| {
        at(&frame, x, 4.25)[..3]
            .iter()
            .map(|c| u32::from(*c))
            .sum::<u32>()
    };
    let (left, right) = (brightness(4.25), brightness(10.25));
    assert!(
        left > right + 60,
        "the left-tilted page shades brighter than the right-tilted one: {left}, {right}"
    );
}

/// An orthographic camera `distance` from the origin at `bearing` degrees
/// about up from +z toward +x, looking at the point `right` units to its
/// right and `up` units up from the origin.
fn orbit(bearing: f64, right: f64, up: f64) -> render_host_contracts::RendererCompositionCamera {
    let (sin, cos) = bearing.to_radians().sin_cos();
    let distance = 600.0;
    let mut eye = camera(
        "orbit",
        [
            sin * distance + cos * right,
            up,
            cos * distance - sin * right,
        ],
        -bearing,
        0.0,
    );
    eye.projection = RendererCameraProjection::Orthographic {
        vertical_size: 1000.0,
        near: 1.0,
        far: 2000.0,
    };
    eye
}

#[test]
fn each_camera_draws_the_rig_view_nearest_its_bearing_and_mirrored_views_flip() {
    use std::f32::consts::{FRAC_PI_2, PI};
    let mut harness = Harness::new(RendererOptions::default());
    let (rig, mut ops) = squash_ops(&mut harness.resources);
    // Four views of one drawing, told apart by their poses.
    let poses = [0, 3, 6, 9].map(|steps| squash_pose(&rig, steps));
    let views = [
        (0.0, false),
        (FRAC_PI_2, false),
        (PI, false),
        (-FRAC_PI_2, true),
    ];
    let shown = |views: Vec<PartsRigView>| {
        let mut rig = figure(BillboardMode::Cylindrical, [0.0; 3], 0.0);
        rig.scale = 1.0;
        rig.views = views;
        rig
    };
    let view = |(heading, mirror): (f32, bool)| PartsRigView {
        asset: RIG.to_owned(),
        heading,
        mirror,
    };
    ops.push(RenderDiff::CreatePartsRig {
        handle: RenderHandle::new(10),
        parent: None,
        rig: shown(views.iter().copied().map(view).collect()),
    });
    for (index, pose) in poses.iter().enumerate() {
        ops.push(RenderDiff::SetPartsRigPose {
            handle: RenderHandle::new(10),
            view: index as u32,
            pose: pose.clone(),
        });
    }
    harness.apply(ops);
    let alone = |harness: &mut Harness,
                 view: Option<(PartsRigView, &PartsRigPose)>,
                 eye: &render_host_contracts::RendererCompositionCamera| {
        let mut ops = vec![RenderDiff::Update {
            handle: RenderHandle::new(10),
            transform: None,
            material: None,
            visible: Some(view.is_none()),
            metadata: None,
        }];
        if let Some((view, pose)) = &view {
            ops.extend(create(20, shown(vec![view.clone()]), pose));
        }
        harness.apply(ops);
        let pixels = harness.single(eye);
        if view.is_some() {
            harness.apply(vec![RenderDiff::Destroy {
                handle: RenderHandle::new(20),
            }]);
        }
        pixels
    };
    // Seen from near each heading, the set draws that view's pose.
    let mut references = Vec::new();
    for (index, bearing) in [10.0, 80.0, 190.0, -100.0].into_iter().enumerate() {
        let eye = orbit(bearing, 0.0, 360.0);
        let set = alone(&mut harness, None, &eye);
        let expected = alone(
            &mut harness,
            Some((view(views[index]), &poses[index])),
            &eye,
        );
        assert!(drawn(&set) > 2_000, "bearing {bearing} draws the rig");
        assert_eq!(
            differing_pixels(&set, &expected),
            0,
            "bearing {bearing} draws view {index}"
        );
        references.push(expected);
    }
    assert!(
        differing_pixels(&references[0], &references[1]) > 500,
        "the views' poses differ"
    );
    // A mirrored view is its unmirrored drawing flipped about the rig's
    // origin, which the camera centres.
    let eye = orbit(-100.0, 0.0, 360.0);
    let plain = alone(
        &mut harness,
        Some((view((-FRAC_PI_2, false)), &poses[3])),
        &eye,
    );
    let flipped: Vec<u8> = plain
        .as_chunks::<4>()
        .0
        .chunks(WIDTH as usize)
        .flat_map(|row| row.iter().rev().flatten().copied())
        .collect();
    let differing = differing_pixels(&references[3], &flipped);
    let covered = drawn(&plain);
    assert!(
        differing_pixels(&references[3], &plain) * 10 > covered,
        "the mirror changes the drawing"
    );
    assert!(
        differing * 50 < covered,
        "{differing} of {covered} drawn pixels differ from the flipped drawing"
    );
}
