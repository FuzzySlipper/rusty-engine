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
//! - a sparse, wide region attachment beside an opaque atlas neighbour,
//!   snapped in the world and placed in the viewport, cell for cell against
//!   its page;
//! - pixel art placed in a rectangle of the viewport: whole pixels per
//!   texel, a pose moving inside its fixed frame and clipped by it, over a
//!   fogged scene, and its pages' colours over an exposed, graded and tone
//!   mapped one; and following a UI anchor's rect as the page reports it;
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
        figure: [0.0; 4],
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
        figure_dome: false,
        viewport_placement: None,
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
            figure: [0.0; 4],
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
            figure: [0.0; 4],
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

/// The checker page's rig drawing: one piece, the whole page.
fn checker_ops(harness: &mut Harness) -> Vec<RenderDiff> {
    let (page, _) = harness.resources.texture(
        "texture/checker",
        12,
        12,
        &checker_page(),
        TextureFilter::Nearest,
    );
    vec![
        page,
        RenderDiff::DefinePartsRig {
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
                figure: [0.0; 4],
            },
        },
    ]
}

/// The checker's 12 texels at 1 rig unit each, from rig unit (`x`, `y`).
fn checker_pose(x: f32, y: f32) -> PartsRigPose {
    pose(vec![(piece(0, PartsRigBlend::Normal), square(x, y, 12.0))])
}

#[test]
fn a_rig_placed_in_the_viewport_draws_whole_pixel_texels_in_a_steady_frame_over_the_scene() {
    let mut harness = Harness::new(RendererOptions::default());
    let mut ops = vec![RenderDiff::SetBackgroundColor {
        color: [0.0, 0.0, 0.0, 1.0],
    }];
    ops.extend(checker_ops(&mut harness));
    // A 37-pixel square of the 320 × 180 view at (100, 50) from its lower
    // left, over a 12-unit frame snapped at a texel a unit: 3 whole pixels
    // a texel (36 pixels, centred), wherever the node stands.
    let mut rig = figure(BillboardMode::None, [40.0, -3.0, -500.0], 70.0);
    rig.views[0].asset = "parts-rig/checker".to_owned();
    rig.layer = RenderLayer::Viewmodel;
    rig.texel_snap = 1.0;
    rig.viewport_placement = Some(PartsRigViewportPlacement {
        rectangle: SpriteViewportPlacement {
            minimum: [100.0 / WIDTH as f32, 50.0 / HEIGHT as f32],
            size: [37.0 / WIDTH as f32, 37.0 / HEIGHT as f32],
            alignment: [0.5, 0.5],
            fit: SpriteViewportFit::Contain,
        },
        frame: [0.0, 0.0, 12.0, 12.0],
        anchor: None,
    });
    ops.extend(create(10, rig.clone(), &checker_pose(0.0, 0.0)));
    harness.apply(ops);
    let eye = camera("eye", [0.0, 1.0, 1.0], 0.0, -10.0);
    let frame = harness.single(&eye);

    // The frame's left edge is at 100.5 pixels, rounded to 101, its bottom
    // at 51 (row 129 from the top). The page's coloured texels are 2 to 9
    // of 12, so the checker covers columns 107 to 130 and rows 99 to 122.
    let cell = |frame: &[u8], i: u32, j: u32| {
        let (x, y) = (101 + 3 * i, 93 + 3 * j);
        let first = pixel(frame, WIDTH, x, y);
        let whole = (0..3).all(|dy| (0..3).all(|dx| pixel(frame, WIDTH, x + dx, y + dy) == first));
        whole.then_some(first)
    };
    for j in 0..12 {
        for i in 0..12 {
            let colour = cell(&frame, i, j)
                .unwrap_or_else(|| panic!("texel ({i}, {j}) is a whole 3 × 3 block"));
            let inside = (2..10).contains(&i) && (2..10).contains(&j);
            assert_eq!(colour[..3] != [0, 0, 0], inside, "texel ({i}, {j})");
            if inside && i > 2 {
                assert_ne!(
                    cell(&frame, i - 1, j),
                    Some(colour),
                    "the checker alternates"
                );
            }
        }
    }
    for y in 0..HEIGHT {
        for x in [106, 131] {
            assert_eq!(
                pixel(&frame, WIDTH, x, y)[..3],
                [0, 0, 0],
                "outside at ({x}, {y})"
            );
        }
    }

    // A pose reaching past the frame moves inside it: the same texels, 2
    // units right and 1 up, 6 pixels right and 3 up.
    harness.apply(vec![RenderDiff::SetPartsRigPose {
        handle: RenderHandle::new(10),
        view: 0,
        pose: checker_pose(2.0, 1.0),
    }]);
    let moved = harness.single(&eye);
    for y in 99..123 {
        for x in 107..131 {
            assert_eq!(
                pixel(&moved, WIDTH, x + 6, y - 3),
                pixel(&frame, WIDTH, x, y),
                "({x}, {y}) moved with the pose, not the frame"
            );
        }
    }

    // It draws only inside its frame: moved 6 units right, the checker's
    // last 4 columns would lie past it (columns 137 to 148).
    harness.apply(vec![RenderDiff::SetPartsRigPose {
        handle: RenderHandle::new(10),
        view: 0,
        pose: checker_pose(6.0, 0.0),
    }]);
    let past = harness.single(&eye);
    assert_ne!(
        pixel(&past, WIDTH, 136, 110)[..3],
        [0, 0, 0],
        "inside the frame"
    );
    for y in 90..135 {
        for x in 137..152 {
            assert_eq!(
                pixel(&past, WIDTH, x, y)[..3],
                [0, 0, 0],
                "({x}, {y}) is past the frame"
            );
        }
    }

    // Over a floor in thick fog, every texel draws as it did.
    let mut ops = coloured_mesh("floor", floor(40.0), [0.7, 0.6, 0.5, 1.0]);
    ops.push(instance(
        1,
        None,
        "floor",
        transform([0.0, 0.0, -10.0], 0.0, 1.0),
    ));
    ops.push(RenderDiff::SetFog {
        fog: Some(FogDescriptor::Exponential {
            color: [0.6, 0.7, 0.85],
            density: 0.5,
        }),
    });
    ops.push(RenderDiff::SetPartsRigPose {
        handle: RenderHandle::new(10),
        view: 0,
        pose: checker_pose(0.0, 0.0),
    });
    harness.apply(ops);
    let fogged = harness.single(&eye);
    assert_ne!(
        pixel(&fogged, WIDTH, 20, 170),
        pixel(&frame, WIDTH, 20, 170),
        "the floor drew"
    );
    for y in 99..123 {
        for x in 107..131 {
            assert_eq!(
                pixel(&fogged, WIDTH, x, y),
                pixel(&frame, WIDTH, x, y),
                "({x}, {y}) is unfogged and over the floor"
            );
        }
    }

    // Exposed four times, bloomed, graded and tone mapped by ACES, the
    // scene changes and every texel is still its page's colour: the rig
    // draws after the finish.
    harness.apply(vec![
        RenderDiff::SetToneMapping {
            tone_mapping: ToneMappingDescriptor {
                operator: ToneMappingOperator::AcesFilmic,
                exposure: 4.0,
            },
        },
        RenderDiff::SetColorGrading {
            color_grading: Some(ColorGradingDescriptor {
                temperature: 0.4,
                tint: -0.2,
                contrast: 1.3,
                saturation: 0.6,
            }),
        },
        RenderDiff::SetBloom {
            bloom: Some(BloomDescriptor {
                threshold: 0.5,
                intensity: 1.0,
            }),
        },
    ]);
    let finished = harness.single(&eye);
    assert!(
        differing_pixels(
            &finished[..WIDTH as usize * 4 * 90],
            &fogged[..WIDTH as usize * 4 * 90]
        ) > 1000,
        "the scene is finished"
    );
    let page = checker_page();
    for j in 0..12 {
        for i in 0..12 {
            let texel = &page[((j * 12 + i) * 4) as usize..][..4];
            if texel[3] == 255 {
                let colour = cell(&finished, i, j)
                    .unwrap_or_else(|| panic!("texel ({i}, {j}) is a whole 3 × 3 block"));
                assert_eq!(colour[..3], texel[..3], "texel ({i}, {j}) is its page's");
            }
        }
    }
    assert_screenshot("parts_rigs_viewport", WIDTH, HEIGHT, &fogged);
}

/// The lower-left corner (from the top) of the snapped checker's 3-pixel
/// texel grid in `frame`: the first row and column it draws.
fn checker_corner(frame: &[u8]) -> Option<(u32, u32)> {
    let drawn = |x, y| pixel(frame, WIDTH, x, y)[..3] != [0, 0, 0];
    let left = (0..WIDTH).find(|&x| (0..HEIGHT).any(|y| drawn(x, y)))?;
    let top = (0..HEIGHT).find(|&y| (0..WIDTH).any(|x| drawn(x, y)))?;
    Some((left, top))
}

#[test]
fn a_placed_rig_follows_its_ui_anchor_while_the_page_reports_it() {
    use render_host_contracts::{RendererViewport, RendererViewportAnchors};
    let mut harness = Harness::new(RendererOptions::default());
    let mut ops = vec![RenderDiff::SetBackgroundColor {
        color: [0.0, 0.0, 0.0, 1.0],
    }];
    ops.extend(checker_ops(&mut harness));
    let mut rig = figure(BillboardMode::None, [0.0; 3], 0.0);
    rig.views[0].asset = "parts-rig/checker".to_owned();
    rig.layer = RenderLayer::Viewmodel;
    rig.texel_snap = 1.0;
    // The rectangle stands while no page reports `bust`: 36 pixels at
    // (100, 50) from the lower left.
    rig.viewport_placement = Some(PartsRigViewportPlacement {
        rectangle: SpriteViewportPlacement {
            minimum: [100.0 / WIDTH as f32, 50.0 / HEIGHT as f32],
            size: [36.0 / WIDTH as f32, 36.0 / HEIGHT as f32],
            alignment: [0.5, 0.5],
            fit: SpriteViewportFit::Contain,
        },
        frame: [0.0, 0.0, 12.0, 12.0],
        anchor: Some("bust".to_owned()),
    });
    ops.extend(create(10, rig, &checker_pose(0.0, 0.0)));
    harness.apply(ops);
    let eye = camera("eye", [0.0, 1.0, 1.0], 0.0, -10.0);
    // The checker's coloured texels start 2 in: 6 pixels at 3 a texel.
    let at_rectangle = harness.single(&eye);
    assert_eq!(checker_corner(&at_rectangle), Some((106, 100)));

    let report = |harness: &mut Harness, x: f64, y: f64, side: f64| {
        let mut anchors = RendererViewportAnchors::new();
        anchors.insert(
            "bust".to_owned(),
            RendererViewport {
                x: x / f64::from(WIDTH),
                y: y / f64::from(HEIGHT),
                width: side / f64::from(WIDTH),
                height: side / f64::from(HEIGHT),
            },
        );
        harness.renderer.set_viewport_anchors(anchors);
        harness.single(&eye)
    };
    // The page reports the element at (200, 20), 24 pixels: 2 a texel. With
    // no product op, the rig draws there; then the element moves and grows
    // to 48 pixels at (10, 90), 4 a texel.
    for (x, y, side, per) in [(200.0, 20.0, 24.0, 2), (10.0, 90.0, 48.0, 4)] {
        let frame = report(&mut harness, x, y, side);
        let top = HEIGHT - y as u32 - side as u32;
        assert_eq!(
            checker_corner(&frame),
            Some((x as u32 + 2 * per, top + 2 * per)),
            "at the anchor's {side}-pixel rect"
        );
        for j in 2..10 {
            for i in 2..10 {
                let (px, py) = (x as u32 + i * per, top + j * per);
                let first = pixel(&frame, WIDTH, px, py);
                assert!(
                    (0..per).all(
                        |dy| (0..per).all(|dx| pixel(&frame, WIDTH, px + dx, py + dy) == first)
                    ),
                    "texel ({i}, {j}) is a whole {per} × {per} block"
                );
            }
        }
    }
    // Reported no more, it is at its rectangle again.
    harness
        .renderer
        .set_viewport_anchors(RendererViewportAnchors::new());
    assert_eq!(harness.single(&eye), at_rectangle);
}

/// Page width and height of the sparse region's atlas.
const SPARSE_PAGE: (u32, u32) = (48, 13);
/// The sparse region: x, y, width, height in page texels; an opaque
/// neighbour lies 2 texels right of it, a clear gutter between.
const SPARSE_REGION: [u32; 4] = [1, 1, 35, 11];
const SPARSE_WIRE: [u8; 4] = [70, 30, 20, 255];
/// What its clear texels hold: red, as a painting's matte might.
const SPARSE_CLEAR: [u8; 4] = [220, 20, 20, 0];

/// An atlas page holding a wide, sparse region, like a pair of
/// spectacles: wire texels with alpha (two lens rims and a bridge, some at
/// alpha 200, one at 100, under a 0.5 cutoff), clear red elsewhere, and an
/// opaque green neighbour region beside it.
fn sparse_page() -> Vec<u8> {
    let (width, height) = SPARSE_PAGE;
    let [rx, ry, rw, rh] = SPARSE_REGION;
    let mut rgba = Vec::new();
    for y in 0..height {
        for x in 0..width {
            let (u, v) = (x as i32 - rx as i32, y as i32 - ry as i32);
            let inside = (0..rw as i32).contains(&u) && (0..rh as i32).contains(&v);
            let rim = |left: i32| {
                (u == left || u == left + 11) && (2..9).contains(&v)
                    || (v == 2 || v == 8) && (left..=left + 11).contains(&u)
            };
            let texel = if inside && (rim(2) || rim(21) || (v == 4 && (13..21).contains(&u))) {
                if (u + v) % 5 == 0 {
                    [70, 30, 20, 200]
                } else if u == 13 {
                    [70, 30, 20, 100]
                } else {
                    SPARSE_WIRE
                }
            } else if x >= rx + rw + 2 {
                [40, 200, 60, 255]
            } else {
                SPARSE_CLEAR
            };
            rgba.extend_from_slice(&texel);
        }
    }
    rgba
}

#[test]
fn a_sparse_regions_quad_draws_only_its_own_texels_under_texel_snap() {
    let mut harness = Harness::new(RendererOptions::default());
    let (width, height) = SPARSE_PAGE;
    let page = sparse_page();
    let (texture, _) = harness.resources.texture(
        "texture/sparse",
        width,
        height,
        &page,
        TextureFilter::Nearest,
    );
    let [rx, ry, rw, rh] = SPARSE_REGION.map(|value| value as f32);
    let (u0, v0) = (rx / width as f32, ry / height as f32);
    let (u1, v1) = ((rx + rw) / width as f32, (ry + rh) / height as f32);
    let mut ops = vec![
        RenderDiff::SetBackgroundColor {
            color: [0.0, 0.0, 0.0, 1.0],
        },
        texture,
        RenderDiff::DefinePartsRig {
            asset: PartsRigAsset {
                id: "parts-rig/sparse".to_owned(),
                pages: vec![PartsRigPage {
                    texture: "texture/sparse".to_owned(),
                    premultiplied: false,
                    detail: None,
                }],
                drawables: vec![PartsRigDrawable {
                    page: 0,
                    // A region attachment's quad: bottom left, bottom right,
                    // top right, top left (v down), split on its diagonal.
                    uvs: vec![[u0, v1], [u1, v1], [u1, v0], [u0, v0]],
                    triangles: vec![0, 1, 2, 2, 3, 0],
                }]
                .into(),
                figure: [0.0; 4],
            },
        },
    ];
    // The region at a texel a rig unit, turned 2° about its centre at rig
    // unit (17.6, 5.7), so the quad's diagonal crosses the grid's cells at
    // every phase.
    let turn = 2f32.to_radians();
    let centre = [17.6f32, 5.7];
    let corners = [[0.0, 0.0], [rw, 0.0], [rw, rh], [0.0, rh]].map(|[x, y]| {
        let (x, y) = (x - rw / 2.0, y - rh / 2.0);
        [
            x * turn.cos() - y * turn.sin() + centre[0],
            x * turn.sin() + y * turn.cos() + centre[1],
        ]
    });
    let posed = pose(vec![(piece(0, PartsRigBlend::Normal), corners)]);
    // In the world: 3 pixels a rig unit under `flat_camera` (20 a metre),
    // its origin on a pixel corner, 160 pixels across and 20 up.
    let mut world = figure(BillboardMode::None, [8.0, 1.0, 0.0], 0.0);
    world.views[0].asset = "parts-rig/sparse".to_owned();
    world.scale = 0.15;
    world.texel_snap = 1.0;
    ops.extend(create(10, world.clone(), &posed));
    // Placed: its frame, 40 by 19 texels from rig unit (-2, -4), fits
    // 120 by 57 pixels from (20, 20), 3 pixels a texel, its origin 6 and 12
    // pixels in.
    let mut placed = world;
    placed.layer = RenderLayer::Viewmodel;
    placed.viewport_placement = Some(PartsRigViewportPlacement {
        rectangle: SpriteViewportPlacement {
            minimum: [20.0 / WIDTH as f32, 20.0 / HEIGHT as f32],
            size: [120.0 / WIDTH as f32, 57.0 / HEIGHT as f32],
            alignment: [0.5, 0.5],
            fit: SpriteViewportFit::Contain,
        },
        frame: [-2.0, -4.0, 38.0, 15.0],
        anchor: None,
    });
    ops.extend(create(11, placed, &posed));
    harness.apply(ops);
    let frame = harness.single(&flat_camera());

    // The page texel at each cell centre of the rig's grid, by the quad's
    // map from rig units to the page: the region's own texel, or nothing
    // past it or below the cutoff.
    let (sin, cos) = turn.sin_cos();
    let sampled = |cx: f32, cy: f32| -> Option<[u8; 3]> {
        let (x, y) = (cx - centre[0], cy - centre[1]);
        let (x, y) = (x * cos + y * sin + rw / 2.0, -x * sin + y * cos + rh / 2.0);
        if !(0.0..rw).contains(&x) || !(0.0..rh).contains(&y) {
            return None;
        }
        let (px, py) = ((rx + x).floor() as u32, (ry + rh - y).floor() as u32);
        let texel = &page[((py * width + px) * 4) as usize..][..4];
        (texel[3] >= 128).then(|| [texel[0], texel[1], texel[2]])
    };
    let mut wire = 0;
    for (name, (left, bottom)) in [("in the world", (160, 20)), ("placed", (26, 32))] {
        let mut wrong = Vec::new();
        for l in -2..15 {
            for k in -2..38 {
                let expected = sampled(k as f32 + 0.5, l as f32 + 0.5);
                let x = (left + 3 * k + 1) as u32;
                let y = HEIGHT - 1 - (bottom + 3 * l + 1) as u32;
                let drawn = pixel(&frame, WIDTH, x, y);
                wire += usize::from(expected.is_some());
                if drawn[..3] != expected.unwrap_or([0, 0, 0]) {
                    wrong.push((k, l, drawn, expected));
                }
            }
        }
        assert!(wrong.is_empty(), "{name}: cells unlike the page: {wrong:?}");
    }
    assert!(wire > 100, "the wire draws: {wire} cells");
    assert_screenshot("parts_rigs_sparse_region", WIDTH, HEIGHT, &frame);
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
            figure: [0.0; 4],
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
                figure: [0.0; 4],
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
            figure: [0.0; 4],
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

/// `drawable`'s triangles wound the other way, as some of a rig-c rig's
/// pieces are.
fn clockwise(mut drawable: PartsRigDrawable) -> PartsRigDrawable {
    for triangle in drawable.triangles.chunks_mut(3) {
        triangle.swap(1, 2);
    }
    drawable
}

/// Grey pieces lit `Synthetic` at strength 1, light from the left: one
/// piece, two abutting pieces (the right one wound clockwise) under each
/// piece's own dome, and the same two under one figure dome. The figure
/// dome shades the two as the one piece is shaded, with no seam where they
/// meet, whichever way they wind; each piece's own dome turns its edges
/// apart there.
#[test]
fn a_figure_dome_shades_abutting_pieces_as_one_surface() {
    let mut harness = Harness::new(RendererOptions {
        default_world_lights: false,
        ..RendererOptions::default()
    });
    let mut ops = vec![RenderDiff::SetBackgroundColor {
        color: [0.0, 0.0, 0.0, 1.0],
    }];
    ops.push(blends_rig(&mut harness, "parts-rig/one"));
    // A figure 4.5 wide from x = 5.75 and 2.5 tall from y = 3.
    let (width, height, bottom) = (4.5, 2.5, 3.0);
    ops.push(RenderDiff::DefinePartsRig {
        asset: PartsRigAsset {
            id: "parts-rig/two".to_owned(),
            pages: vec![PartsRigPage {
                texture: "texture/straight".to_owned(),
                premultiplied: false,
                detail: None,
            }],
            // The grey quadrant, and the same wound clockwise.
            drawables: vec![quadrant(0, 3), clockwise(quadrant(0, 3))].into(),
            figure: [5.75, bottom, 5.75 + width, bottom + height],
        },
    });
    // The same figure 3.5 higher and on the rig's left, for a mirrored view
    // that draws it where the first stands.
    let (above, left) = (bottom + 3.5, -5.75 - width);
    ops.push(RenderDiff::DefinePartsRig {
        asset: PartsRigAsset {
            id: "parts-rig/mirrored".to_owned(),
            pages: vec![PartsRigPage {
                texture: "texture/straight".to_owned(),
                premultiplied: false,
                detail: None,
            }],
            drawables: vec![quadrant(0, 3), clockwise(quadrant(0, 3))].into(),
            figure: [left, above, left + width, above + height],
        },
    });
    ops.extend(side_lights());
    let halves = |x: f32, bottom: f32| {
        let half = |x: f32| {
            [
                [x, bottom],
                [x + width / 2.0, bottom],
                [x + width / 2.0, bottom + height],
                [x, bottom + height],
            ]
        };
        pose(vec![
            (piece(0, PartsRigBlend::Normal), half(x)),
            (piece(1, PartsRigBlend::Normal), half(x + width / 2.0)),
        ])
    };
    let one = pose(vec![(
        piece(3, PartsRigBlend::Normal),
        [
            [0.5, bottom],
            [0.5 + width, bottom],
            [0.5 + width, bottom + height],
            [0.5, bottom + height],
        ],
    )]);
    let synthetic = lit(SpriteLightingMode::Synthetic);
    let mut figure_dome = flat_rig("parts-rig/two", 0.0, synthetic.clone());
    figure_dome.figure_dome = true;
    // Each piece's own dome, though its asset has a figure.
    ops.extend(create(
        10,
        flat_rig("parts-rig/one", 0.0, synthetic.clone()),
        &one,
    ));
    let mut mirrored = flat_rig("parts-rig/mirrored", 0.0, synthetic.clone());
    mirrored.figure_dome = true;
    mirrored.views[0].mirror = true;
    ops.extend(create(
        11,
        flat_rig("parts-rig/two", -1.0, synthetic),
        &halves(5.75, bottom),
    ));
    ops.extend(create(12, figure_dome, &halves(5.75, bottom)));
    ops.extend(create(13, mirrored, &halves(left, above)));
    harness.apply(ops);
    let frame = harness.single(&flat_camera());
    let brightness = |x: f32| {
        at(&frame, x, bottom + height / 2.0)[..3]
            .iter()
            .map(|c| i32::from(*c))
            .sum::<i32>()
    };
    // A pixel each side of the seam, 0.05 m (one pixel) away.
    let seam = 5.75 + width / 2.0;
    let jump = brightness(seam - 0.05) - brightness(seam + 0.05);
    assert!(
        jump.abs() <= 6,
        "the figure dome is continuous across the seam: {} then {}",
        brightness(seam - 0.05),
        brightness(seam + 0.05)
    );
    // Seen from the front: the side toward the light is the bright one.
    let (lit, unlit) = (brightness(5.75 + 0.3), brightness(5.75 + width - 0.3));
    assert!(
        lit > unlit + 60,
        "lit from the left: {lit} on the left, {unlit} on the right"
    );
    let mirrored = |x: f32| {
        at(&frame, x, above + height / 2.0)[..3]
            .iter()
            .map(|c| i32::from(*c))
            .sum::<i32>()
    };
    for step in 1..18 {
        let offset = step as f32 * width / 18.0;
        let (single, figure) = (brightness(0.5 + offset), brightness(5.75 + offset));
        assert!(
            (single - figure).abs() <= 6,
            "{offset} m in, the two pieces shade as the one piece does: {figure} against {single}"
        );
        let flipped = mirrored(5.75 + offset);
        assert!(
            (flipped - figure).abs() <= 6,
            "{offset} m in, the mirrored view shades as the figure does: {flipped} against {figure}"
        );
    }
    // Hide the figure dome: each piece's own dome shows the seam.
    assert_screenshot("parts_rigs_figure_dome", WIDTH, HEIGHT, &frame);
    harness.apply(vec![RenderDiff::Destroy {
        handle: RenderHandle::new(12),
    }]);
    let frame = harness.single(&flat_camera());
    let brightness = |x: f32| {
        at(&frame, x, bottom + height / 2.0)[..3]
            .iter()
            .map(|c| i32::from(*c))
            .sum::<i32>()
    };
    let jump = brightness(seam + 0.05) - brightness(seam - 0.05);
    assert!(
        jump > 60,
        "each piece's own dome turns its edges apart at the seam: {} then {}",
        brightness(seam - 0.05),
        brightness(seam + 0.05)
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
