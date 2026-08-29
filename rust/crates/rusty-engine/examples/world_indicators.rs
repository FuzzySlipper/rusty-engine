use std::collections::BTreeMap;

use rusty_engine::render_model::ResolvedRenderAsset;
use rusty_engine::render_presentation::{
    BillboardAlignment, BillboardAnchor, BillboardContent, BillboardDescriptor,
    BillboardEdgeBehavior, BillboardFontRef, BillboardHandle, BillboardIndicator, BillboardLayer,
    BillboardLayoutPolicy, BillboardLayoutSizing, BillboardLocalizedText, BillboardMeter,
    BillboardMeterFillDirection, BillboardOverlapBehavior, BillboardProjectionOp,
    BillboardProjector, BillboardSafeArea, BillboardStyle, PresentationFrameDiff,
    PresentationOpMeta,
};

fn text(key: &str, fallback: &str) -> BillboardLocalizedText {
    BillboardLocalizedText {
        localization_key: key.into(),
        fallback_text: fallback.into(),
    }
}

fn main() {
    let descriptor = BillboardDescriptor {
        anchor: BillboardAnchor::EntityAttached {
            entity: 42,
            offset: [0.0, 1.9, 0.0],
        },
        content: BillboardContent::Structured {
            indicator: BillboardIndicator {
                label: Some(text("actor.ranger.name", "Ranger")),
                icon: None,
                accessible_label: text("actor.ranger.indicator", "Ranger status"),
                meters: vec![BillboardMeter {
                    id: "health".into(),
                    accessible_label: text("resource.health", "Health"),
                    current: 72.0,
                    min: 0.0,
                    max: 100.0,
                    preview: Some(64.0),
                    fill_direction: BillboardMeterFillDirection::LeftToRight,
                    segments: 10,
                    fill: [0.16, 0.72, 0.28, 1.0],
                    preview_fill: [0.95, 0.72, 0.12, 1.0],
                    back: [0.04, 0.04, 0.04, 0.9],
                    border: [0.0, 0.0, 0.0, 1.0],
                }],
                status_cues: Vec::new(),
                width_pixels: 192.0,
                spacing_pixels: 6.0,
                alignment: BillboardAlignment::Center,
                style: BillboardStyle {
                    opacity: 0.96,
                    backing: [0.0, 0.0, 0.0, 0.58],
                    border: [0.2, 0.2, 0.2, 1.0],
                    radius_pixels: 6.0,
                },
            },
        },
        font: BillboardFontRef::System {
            family: "sans-serif".into(),
        },
        height_pixels: 20.0,
        color: [1.0; 4],
        background: [0.0; 4],
        max_distance: 80.0,
        layer: BillboardLayer::Occluded,
        visible: true,
        layout: Some(BillboardLayoutPolicy {
            priority: 100,
            sizing: BillboardLayoutSizing::DistanceScaled {
                reference_distance: 12.0,
                min_scale: 0.75,
                max_scale: 1.25,
            },
            safe_area: BillboardSafeArea {
                top_pixels: 12.0,
                right_pixels: 12.0,
                bottom_pixels: 12.0,
                left_pixels: 12.0,
            },
            edge_behavior: BillboardEdgeBehavior::Clamp,
            overlap_behavior: BillboardOverlapBehavior::Suppress,
        }),
    };

    // Games own the entity facts and the mapping from entity 42 to a world
    // position. Engine owns bounded projection, strict validation, and host
    // realization of this presentation-only readout.
    let mut projector = BillboardProjector::default();
    let assets = BTreeMap::<String, ResolvedRenderAsset>::new();
    let op = projector
        .project(
            &assets,
            PresentationOpMeta::new(0),
            BillboardProjectionOp::Create {
                handle: BillboardHandle::new(1),
                descriptor,
            },
        )
        .expect("structured world indicator must satisfy the public contract");
    let frame = PresentationFrameDiff::try_from_ops(vec![op])
        .expect("projected operation must form a valid presentation frame");

    println!(
        "{}",
        serde_json::to_string_pretty(&frame).expect("presentation frame must serialize")
    );
}
