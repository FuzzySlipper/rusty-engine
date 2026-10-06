//! Billboard label fixtures (#8827):
//! - text, value and icon labels, and structured indicators with meters and
//!   status cues;
//! - the depth layers against the scene's depth;
//! - structured layout policy: safe area, clamp and cull, stack and suppress;
//! - create, update and destroy, and fonts or icons that fail to load.
//!
//! `RENDER_WGPU_BLESS=1 cargo test -p render-wgpu --test labels` rewrites the
//! references.

mod common;

use common::*;
use render_host_contracts::RendererCompositionCamera;
use render_model::*;
use render_presentation::{
    BillboardAlignment, BillboardAnchor, BillboardContent, BillboardDescriptor,
    BillboardEdgeBehavior, BillboardFontRef, BillboardHandle, BillboardIndicator, BillboardLayer,
    BillboardLayoutPolicy, BillboardLayoutSizing, BillboardLocalizedText, BillboardMeter,
    BillboardMeterFillDirection, BillboardOverlapBehavior, BillboardPatch, BillboardProjectionOp,
    BillboardSafeArea, BillboardStatusCue, BillboardStyle, BillboardTemplateArgument,
    BillboardTextureRef, PresentationFrameDiff, PresentationOp, PresentationOpMeta,
};
use render_wgpu::{OffscreenTarget, RendererOptions};

const LABEL_WIDTH: u32 = 480;
const LABEL_HEIGHT: u32 = 270;
const NO_ENTITIES: &dyn Fn(u64) -> Option<[f32; 3]> = &|_| None;

fn harness() -> Harness {
    let mut harness = Harness::new(RendererOptions::default());
    harness.target = OffscreenTarget::new(&harness.gpu, LABEL_WIDTH, LABEL_HEIGHT, 4);
    harness.apply(room());
    harness
}

fn eye() -> RendererCompositionCamera {
    camera("eye", [0.0, 0.6, 1.0], 0.0, -6.0)
}

fn frame(ops: Vec<BillboardProjectionOp>) -> PresentationFrameDiff {
    PresentationFrameDiff::try_from_ops(
        ops.into_iter()
            .enumerate()
            .map(|(sequence, op)| PresentationOp::Billboard {
                meta: PresentationOpMeta::new(sequence as u32),
                op,
            })
            .collect(),
    )
    .expect("a valid presentation frame")
}

/// Apply billboard ops and return the issues.
fn apply(harness: &mut Harness, ops: Vec<BillboardProjectionOp>) -> Vec<String> {
    harness
        .renderer
        .apply_presentation(&frame(ops), &harness.resources, NO_ENTITIES)
        .into_iter()
        .map(|issue| issue.detail)
        .collect()
}

fn create(handle: u64, descriptor: BillboardDescriptor) -> BillboardProjectionOp {
    BillboardProjectionOp::Create {
        handle: BillboardHandle::new(handle),
        descriptor,
    }
}

fn text(fallback: &str) -> BillboardContent {
    BillboardContent::Text {
        localization_key: "fixture.text".to_owned(),
        fallback_text: fallback.to_owned(),
        arguments: Vec::new(),
    }
}

fn localized(text: &str) -> BillboardLocalizedText {
    BillboardLocalizedText {
        localization_key: format!("fixture.{text}"),
        fallback_text: text.to_owned(),
    }
}

fn label(
    position: [f32; 3],
    content: BillboardContent,
    layer: BillboardLayer,
) -> BillboardDescriptor {
    BillboardDescriptor {
        anchor: BillboardAnchor::World { position },
        content,
        font: BillboardFontRef::System {
            family: "sans-serif".to_owned(),
        },
        height_pixels: 16.0,
        color: [1.0, 1.0, 1.0, 1.0],
        background: [0.0, 0.0, 0.0, 0.6],
        max_distance: 50.0,
        layer,
        visible: true,
        layout: None,
    }
}

/// A 12×12 icon: an amber disc on transparency.
fn icon_texture(harness: &mut Harness) -> BillboardTextureRef {
    let mut rgba = Vec::with_capacity(12 * 12 * 4);
    for y in 0..12 {
        for x in 0..12 {
            let (dx, dy) = (x as f32 - 5.5, y as f32 - 5.5);
            let inside = dx * dx + dy * dy <= 30.0;
            rgba.extend_from_slice(if inside {
                &[250, 180, 30, 255]
            } else {
                &[0, 0, 0, 0]
            });
        }
    }
    let (define, hash) =
        harness
            .resources
            .texture("texture/label-icon", 12, 12, &rgba, TextureFilter::Linear);
    harness.apply(vec![define]);
    BillboardTextureRef {
        asset: "texture/label-icon".to_owned(),
        content_hash: hash,
    }
}

fn meter(id: &str, current: f32, direction: BillboardMeterFillDirection) -> BillboardMeter {
    BillboardMeter {
        id: id.to_owned(),
        accessible_label: localized(id),
        current,
        min: 0.0,
        max: 1.0,
        preview: None,
        fill_direction: direction,
        segments: 1,
        fill: [0.2, 0.85, 0.35, 1.0],
        preview_fill: [0.95, 0.85, 0.3, 1.0],
        back: [0.05, 0.05, 0.08, 0.9],
        border: [0.8, 0.8, 0.85, 1.0],
    }
}

fn policy(edge: BillboardEdgeBehavior, overlap: BillboardOverlapBehavior) -> BillboardLayoutPolicy {
    BillboardLayoutPolicy {
        priority: 0,
        sizing: BillboardLayoutSizing::ConstantPixels,
        safe_area: BillboardSafeArea {
            top_pixels: 8.0,
            right_pixels: 8.0,
            bottom_pixels: 8.0,
            left_pixels: 8.0,
        },
        edge_behavior: edge,
        overlap_behavior: overlap,
    }
}

fn indicator(meters: Vec<BillboardMeter>) -> BillboardIndicator {
    BillboardIndicator {
        label: Some(localized("Exit")),
        icon: None,
        accessible_label: localized("Exit indicator"),
        meters,
        status_cues: Vec::new(),
        width_pixels: 140.0,
        spacing_pixels: 5.0,
        alignment: BillboardAlignment::Center,
        style: BillboardStyle {
            opacity: 0.9,
            backing: [0.04, 0.05, 0.1, 0.85],
            border: [0.6, 0.7, 0.9, 1.0],
            radius_pixels: 4.0,
        },
    }
}

fn structured(
    position: [f32; 3],
    indicator: BillboardIndicator,
    policy: BillboardLayoutPolicy,
) -> BillboardDescriptor {
    BillboardDescriptor {
        layout: Some(policy),
        ..label(
            position,
            BillboardContent::Structured { indicator },
            BillboardLayer::AlwaysOnTop,
        )
    }
}

/// Count pixels in a region that differ from the same region of `base`.
fn changed(base: &[u8], frame: &[u8], region: [u32; 4]) -> usize {
    let [x0, y0, x1, y1] = region;
    let mut count = 0;
    for y in y0..y1 {
        for x in x0..x1 {
            let (a, b) = (
                pixel(base, LABEL_WIDTH, x, y),
                pixel(frame, LABEL_WIDTH, x, y),
            );
            if a.iter().zip(b.iter()).any(|(a, b)| a.abs_diff(*b) > 24) {
                count += 1;
            }
        }
    }
    count
}

fn whole() -> [u32; 4] {
    [0, 0, LABEL_WIDTH, LABEL_HEIGHT]
}

#[test]
fn text_value_and_icon_labels_draw_above_their_anchors() {
    let mut harness = harness();
    let icon = icon_texture(&mut harness);
    let base = harness.single(&eye());
    let issues = apply(
        &mut harness,
        vec![
            create(
                1,
                label(
                    [-1.3, 0.6, -4.0],
                    BillboardContent::Text {
                        localization_key: "fixture.crate".to_owned(),
                        fallback_text: "Crate {name}".to_owned(),
                        arguments: vec![BillboardTemplateArgument {
                            name: "name".to_owned(),
                            value: "A-7".to_owned(),
                        }],
                    },
                    BillboardLayer::AlwaysOnTop,
                ),
            ),
            create(
                2,
                BillboardDescriptor {
                    color: [0.4, 0.9, 1.0, 1.0],
                    background: [0.1, 0.0, 0.2, 0.8],
                    height_pixels: 20.0,
                    ..label(
                        [0.0, 1.1, -5.2],
                        BillboardContent::Value {
                            label_key: "fixture.mass".to_owned(),
                            fallback_label: "Mass".to_owned(),
                            value: "42".to_owned(),
                            unit_key: Some("fixture.kg".to_owned()),
                            fallback_unit: Some("kg".to_owned()),
                        },
                        BillboardLayer::AlwaysOnTop,
                    )
                },
            ),
            create(
                3,
                label(
                    [1.3, 0.6, -3.6],
                    BillboardContent::Icon {
                        texture: icon,
                        alt_key: "fixture.alert".to_owned(),
                        fallback_alt: "Alert".to_owned(),
                    },
                    BillboardLayer::AlwaysOnTop,
                ),
            ),
        ],
    );
    assert!(issues.is_empty(), "{issues:?}");
    let pixels = harness.single(&eye());
    assert!(changed(&base, &pixels, whole()) > 500);
    assert_screenshot("labels-content", LABEL_WIDTH, LABEL_HEIGHT, &pixels);
}

#[test]
fn structured_indicators_draw_meters_icons_and_cues() {
    let mut harness = harness();
    let icon = icon_texture(&mut harness);
    let mut health = meter("health", 0.65, BillboardMeterFillDirection::LeftToRight);
    health.preview = Some(0.8);
    let mut charge = meter("charge", 0.5, BillboardMeterFillDirection::RightToLeft);
    charge.segments = 4;
    charge.fill = [0.3, 0.55, 1.0, 1.0];
    let mut left = indicator(vec![health, charge]);
    left.icon = Some(icon.clone());
    left.status_cues = vec![BillboardStatusCue {
        id: "alert".to_owned(),
        label: localized("   Alerted"),
        icon: Some(icon),
    }];
    let mut right = indicator(vec![meter(
        "level",
        0.3,
        BillboardMeterFillDirection::BottomToTop,
    )]);
    right.label = Some(localized("Door"));
    right.alignment = BillboardAlignment::Start;
    right.width_pixels = 110.0;
    let issues = apply(
        &mut harness,
        vec![
            create(
                1,
                structured(
                    [-1.3, 0.6, -4.0],
                    left,
                    policy(
                        BillboardEdgeBehavior::Clamp,
                        BillboardOverlapBehavior::Stack,
                    ),
                ),
            ),
            create(
                2,
                structured(
                    [1.4, 0.6, -3.6],
                    right,
                    policy(
                        BillboardEdgeBehavior::Clamp,
                        BillboardOverlapBehavior::Stack,
                    ),
                ),
            ),
        ],
    );
    assert!(issues.is_empty(), "{issues:?}");
    let pixels = harness.single(&eye());
    assert_screenshot("labels-structured", LABEL_WIDTH, LABEL_HEIGHT, &pixels);
}

#[test]
fn layers_follow_the_scene_depth() {
    let mut harness = harness();
    // All three anchors sit behind the green box, as the camera sees it.
    let behind = [0.0, 0.25, -6.5];
    let base = harness.single(&eye());
    let issues = apply(
        &mut harness,
        vec![create(
            1,
            label(behind, text("Hid"), BillboardLayer::Occluded),
        )],
    );
    assert!(issues.is_empty(), "{issues:?}");
    let occluded = harness.single(&eye());
    assert_eq!(
        changed(&base, &occluded, whole()),
        0,
        "an occluded anchor hides its whole label"
    );

    apply(
        &mut harness,
        vec![BillboardProjectionOp::Update {
            handle: BillboardHandle::new(1),
            patch: BillboardPatch {
                layer: Some(BillboardLayer::DepthTested),
                ..BillboardPatch::default()
            },
        }],
    );
    let depth_tested = harness.single(&eye());
    assert_eq!(
        changed(&base, &depth_tested, whole()),
        0,
        "a depth-tested label behind the box is hidden where the box covers it"
    );

    apply(
        &mut harness,
        vec![BillboardProjectionOp::Update {
            handle: BillboardHandle::new(1),
            patch: BillboardPatch {
                layer: Some(BillboardLayer::AlwaysOnTop),
                ..BillboardPatch::default()
            },
        }],
    );
    let on_top = harness.single(&eye());
    assert!(changed(&base, &on_top, whole()) > 100);

    // A long depth-tested label whose anchor is visible beside the red box
    // but which runs behind it: only the uncovered part draws. An occluded
    // label with a visible anchor draws whole, over the box.
    apply(
        &mut harness,
        vec![
            BillboardProjectionOp::Destroy {
                handle: BillboardHandle::new(1),
            },
            create(
                2,
                label(
                    [-0.55, -0.1, -5.0],
                    text("Depth tested behind the red box"),
                    BillboardLayer::DepthTested,
                ),
            ),
            create(
                3,
                label(
                    [1.3, 0.75, -3.6],
                    text("Occluded, anchor visible"),
                    BillboardLayer::Occluded,
                ),
            ),
        ],
    );
    let pixels = harness.single(&eye());
    assert_screenshot("labels-layers", LABEL_WIDTH, LABEL_HEIGHT, &pixels);
}

#[test]
fn structured_layout_clamps_culls_stacks_and_suppresses() {
    let mut harness = harness();
    let base = harness.single(&eye());
    // Anchored far to the right of the view: clamped inside the safe area.
    let clamped = structured(
        [6.0, 0.6, -4.0],
        indicator(vec![meter(
            "a",
            0.7,
            BillboardMeterFillDirection::LeftToRight,
        )]),
        policy(
            BillboardEdgeBehavior::Clamp,
            BillboardOverlapBehavior::Stack,
        ),
    );
    let culled = structured(
        [-6.0, 0.6, -4.0],
        indicator(vec![meter(
            "b",
            0.7,
            BillboardMeterFillDirection::LeftToRight,
        )]),
        policy(BillboardEdgeBehavior::Cull, BillboardOverlapBehavior::Stack),
    );
    // Two at one anchor: the second (lower priority) stacks above the first.
    let mut first = structured(
        [-0.2, 0.3, -4.0],
        indicator(vec![meter(
            "c",
            0.9,
            BillboardMeterFillDirection::LeftToRight,
        )]),
        policy(
            BillboardEdgeBehavior::Clamp,
            BillboardOverlapBehavior::Stack,
        ),
    );
    first.layout.as_mut().unwrap().priority = 2;
    let stacked = structured(
        [-0.2, 0.3, -4.0],
        indicator(vec![meter(
            "d",
            0.4,
            BillboardMeterFillDirection::LeftToRight,
        )]),
        policy(
            BillboardEdgeBehavior::Clamp,
            BillboardOverlapBehavior::Stack,
        ),
    );
    // A third at the same anchor that suppresses instead.
    let suppressed = structured(
        [-0.2, 0.3, -4.0],
        indicator(vec![meter(
            "e",
            0.1,
            BillboardMeterFillDirection::LeftToRight,
        )]),
        policy(
            BillboardEdgeBehavior::Clamp,
            BillboardOverlapBehavior::Suppress,
        ),
    );
    let issues = apply(
        &mut harness,
        vec![
            create(1, clamped),
            create(2, culled),
            create(3, first),
            create(4, stacked),
            create(5, suppressed),
        ],
    );
    assert!(issues.is_empty(), "{issues:?}");
    let pixels = harness.single(&eye());
    // Nothing of the culled label on the left half's edge band.
    assert_eq!(changed(&base, &pixels, [0, 0, 8, LABEL_HEIGHT]), 0);
    // The clamped one reaches the right safe area and no farther.
    let right = LABEL_WIDTH - 8;
    assert!(changed(&base, &pixels, [right - 20, 0, right, LABEL_HEIGHT]) > 50);
    assert_eq!(
        changed(&base, &pixels, [right, 0, LABEL_WIDTH, LABEL_HEIGHT]),
        0
    );
    assert_screenshot("labels-layout", LABEL_WIDTH, LABEL_HEIGHT, &pixels);
}

#[test]
fn distance_scaled_indicators_shrink_with_distance() {
    let mut harness = harness();
    let base = harness.single(&eye());
    let mut scaled = policy(
        BillboardEdgeBehavior::Clamp,
        BillboardOverlapBehavior::Stack,
    );
    scaled.sizing = BillboardLayoutSizing::DistanceScaled {
        reference_distance: 2.0,
        min_scale: 0.5,
        max_scale: 1.5,
    };
    let near = structured(
        [0.0, 0.6, -4.0],
        indicator(vec![meter(
            "a",
            0.7,
            BillboardMeterFillDirection::LeftToRight,
        )]),
        scaled.clone(),
    );
    apply(&mut harness, vec![create(1, near)]);
    let near_pixels = changed(&base, &harness.single(&eye()), whole());
    apply(
        &mut harness,
        vec![BillboardProjectionOp::Update {
            handle: BillboardHandle::new(1),
            patch: BillboardPatch {
                anchor: Some(BillboardAnchor::World {
                    position: [0.0, 0.6, -1.0],
                }),
                ..BillboardPatch::default()
            },
        }],
    );
    let nearer_pixels = changed(&base, &harness.single(&eye()), whole());
    assert!(
        nearer_pixels > near_pixels * 2,
        "{nearer_pixels} pixels at 2 m, {near_pixels} at 5 m"
    );
}

#[test]
fn visibility_distance_and_destroy_hide_labels() {
    let mut harness = harness();
    let base = harness.single(&eye());
    apply(
        &mut harness,
        vec![create(
            1,
            label(
                [0.0, 0.6, -4.0],
                text("Hidden"),
                BillboardLayer::AlwaysOnTop,
            ),
        )],
    );
    let shown = harness.single(&eye());
    assert!(changed(&base, &shown, whole()) > 100);
    for patch in [
        BillboardPatch {
            visible: Some(false),
            ..BillboardPatch::default()
        },
        BillboardPatch {
            visible: Some(true),
            max_distance: Some(3.0),
            ..BillboardPatch::default()
        },
    ] {
        apply(
            &mut harness,
            vec![BillboardProjectionOp::Update {
                handle: BillboardHandle::new(1),
                patch,
            }],
        );
        assert_eq!(harness.single(&eye()), base);
    }
    apply(
        &mut harness,
        vec![BillboardProjectionOp::Update {
            handle: BillboardHandle::new(1),
            patch: BillboardPatch {
                max_distance: Some(50.0),
                ..BillboardPatch::default()
            },
        }],
    );
    assert_eq!(harness.single(&eye()), shown);
    apply(
        &mut harness,
        vec![BillboardProjectionOp::Destroy {
            handle: BillboardHandle::new(1),
        }],
    );
    assert_eq!(harness.single(&eye()), base);
}

#[test]
fn entity_anchors_follow_their_entity() {
    let mut harness = harness();
    let base = harness.single(&eye());
    let position = std::cell::Cell::new([-1.3_f32, 0.6, -4.0]);
    let entities = |entity: u64| (entity == 7).then(|| position.get());
    let descriptor = BillboardDescriptor {
        anchor: BillboardAnchor::EntityAttached {
            entity: 7,
            offset: [0.0, 0.2, 0.0],
        },
        ..label([0.0; 3], text("Follow"), BillboardLayer::AlwaysOnTop)
    };
    let issues = harness.renderer.apply_presentation(
        &frame(vec![create(1, descriptor)]),
        &harness.resources,
        &entities,
    );
    assert!(issues.is_empty(), "{issues:?}");
    let left = harness.single(&eye());
    let half = LABEL_WIDTH / 2;
    assert!(changed(&base, &left, [0, 0, half, LABEL_HEIGHT]) > 100);
    assert_eq!(
        changed(&base, &left, [half, 0, LABEL_WIDTH, LABEL_HEIGHT]),
        0
    );
    position.set([1.3, 0.6, -4.0]);
    harness.renderer.advance_effects(0.0, &entities);
    let right = harness.single(&eye());
    assert_eq!(changed(&base, &right, [0, 0, half, LABEL_HEIGHT]), 0);
    assert!(changed(&base, &right, [half, 0, LABEL_WIDTH, LABEL_HEIGHT]) > 100);
    // An entity the Engine no longer knows hides its label.
    harness.renderer.advance_effects(0.0, NO_ENTITIES);
    assert_eq!(harness.single(&eye()), base);
}

#[test]
fn asset_fonts_load_from_resources() {
    let mut harness = harness();
    let base = harness.single(&eye());
    harness.resources.0.insert(
        "font/fixture".to_owned(),
        include_bytes!("../fonts/DejaVuSans.ttf").to_vec(),
    );
    let descriptor = BillboardDescriptor {
        font: BillboardFontRef::Asset {
            asset: "font/fixture".to_owned(),
            content_hash: "sha256:fixture".to_owned(),
            family: "Fixture".to_owned(),
        },
        ..label(
            [0.0, 0.6, -4.0],
            text("Asset font"),
            BillboardLayer::AlwaysOnTop,
        )
    };
    let issues = apply(&mut harness, vec![create(1, descriptor)]);
    assert!(issues.is_empty(), "{issues:?}");
    assert!(changed(&base, &harness.single(&eye()), whole()) > 100);
}

#[test]
fn a_label_whose_font_fails_to_load_is_realized_by_a_later_update() {
    let mut harness = harness();
    let base = harness.single(&eye());
    let missing_font = BillboardDescriptor {
        font: BillboardFontRef::Asset {
            asset: "font/missing".to_owned(),
            content_hash: "sha256:missing".to_owned(),
            family: "Missing".to_owned(),
        },
        ..label([0.0, 0.6, -4.0], text("Later"), BillboardLayer::AlwaysOnTop)
    };
    let issues = apply(&mut harness, vec![create(1, missing_font)]);
    assert_eq!(issues.len(), 1);
    assert!(issues[0].starts_with("fontLoadFailed"), "{issues:?}");
    assert_eq!(harness.single(&eye()), base);
    let issues = apply(
        &mut harness,
        vec![BillboardProjectionOp::Update {
            handle: BillboardHandle::new(1),
            patch: BillboardPatch {
                font: Some(BillboardFontRef::System {
                    family: "sans-serif".to_owned(),
                }),
                ..BillboardPatch::default()
            },
        }],
    );
    assert!(issues.is_empty(), "{issues:?}");
    assert!(changed(&base, &harness.single(&eye()), whole()) > 100);
}

#[test]
fn a_content_patch_to_text_drops_the_structured_layout() {
    let mut harness = harness();
    let base = harness.single(&eye());
    apply(
        &mut harness,
        vec![create(
            1,
            structured(
                [0.0, 0.6, -4.0],
                indicator(Vec::new()),
                policy(
                    BillboardEdgeBehavior::Clamp,
                    BillboardOverlapBehavior::Stack,
                ),
            ),
        )],
    );
    let issues = apply(
        &mut harness,
        vec![BillboardProjectionOp::Update {
            handle: BillboardHandle::new(1),
            patch: BillboardPatch {
                content: Some(text("Now text")),
                ..BillboardPatch::default()
            },
        }],
    );
    assert!(issues.is_empty(), "{issues:?}");
    assert!(changed(&base, &harness.single(&eye()), whole()) > 100);
}

/// Changed pixels' bounds (x0, y0, x1, y1) against `base`, in a `width` wide image.
fn changed_bounds(base: &[u8], frame: &[u8], width: u32, height: u32) -> [u32; 4] {
    let mut bounds = [u32::MAX, u32::MAX, 0, 0];
    for y in 0..height {
        for x in 0..width {
            let (a, b) = (pixel(base, width, x, y), pixel(frame, width, x, y));
            if a.iter().zip(b.iter()).any(|(a, b)| a.abs_diff(*b) > 24) {
                bounds = [
                    bounds[0].min(x),
                    bounds[1].min(y),
                    bounds[2].max(x + 1),
                    bounds[3].max(y + 1),
                ];
            }
        }
    }
    bounds
}

fn ratio_scene() -> Vec<BillboardProjectionOp> {
    let mut left = indicator(vec![meter(
        "health",
        0.65,
        BillboardMeterFillDirection::LeftToRight,
    )]);
    left.label = Some(localized("Guard"));
    vec![
        create(
            1,
            structured(
                [-1.3, 0.6, -4.0],
                left,
                policy(
                    BillboardEdgeBehavior::Clamp,
                    BillboardOverlapBehavior::Stack,
                ),
            ),
        ),
        create(
            2,
            label(
                [1.3, 0.6, -3.6],
                text("Crate A-7"),
                BillboardLayer::AlwaysOnTop,
            ),
        ),
    ]
}

/// A device pixel ratio of 2 (#8853): the same scene in a target twice the
/// size draws every label at twice the target pixels, rasterized at the ratio
/// rather than magnified, so it keeps its CSS size on the denser output.
#[test]
fn labels_keep_their_css_size_at_device_pixel_ratio_two() {
    let render = |ratio: u32, before_labels: bool| {
        let mut harness = harness();
        harness.target =
            OffscreenTarget::new(&harness.gpu, LABEL_WIDTH * ratio, LABEL_HEIGHT * ratio, 4);
        if before_labels {
            harness.renderer.set_pixel_ratio(ratio as f32);
        }
        let base = harness.single(&eye());
        let issues = apply(&mut harness, ratio_scene());
        assert!(issues.is_empty(), "{issues:?}");
        if !before_labels {
            // Labels created at ratio 1 rasterize again at the new ratio.
            harness.renderer.set_pixel_ratio(ratio as f32);
        }
        (base, harness.single(&eye()))
    };
    let (base, one) = render(1, true);
    let (base_two, two) = render(2, true);
    let one_bounds = changed_bounds(&base, &one, LABEL_WIDTH, LABEL_HEIGHT);
    let two_bounds = changed_bounds(&base_two, &two, LABEL_WIDTH * 2, LABEL_HEIGHT * 2);
    for (single, double) in one_bounds.iter().zip(two_bounds) {
        assert!(
            (double as i64 - 2 * *single as i64).abs() <= 3,
            "ratio 2 bounds {two_bounds:?} are not twice ratio 1 bounds {one_bounds:?}"
        );
    }
    assert_screenshot("labels-ratio-2", LABEL_WIDTH * 2, LABEL_HEIGHT * 2, &two);
    let (_, raised) = render(2, false);
    assert_eq!(
        raised, two,
        "raising the ratio after creation must rasterize again"
    );
}
