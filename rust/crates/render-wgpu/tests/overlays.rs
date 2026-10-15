//! Overlay sets and the Debug layer (#10022): nothing draws, and nothing is
//! resolved, while inspection shows nothing; shown sets draw their lines in
//! the item's space, follow the node they are placed on, and the Debug
//! layer's parts draw only while it is shown.

mod common;

use common::*;
use render_model::*;
use render_wgpu::{OverlayShown, RendererOptions};

const MARKED: u64 = 77;
const MAGENTA: [f32; 4] = [1.0, 0.0, 1.0, 1.0];
const CYAN: [f32; 4] = [0.0, 1.0, 1.0, 1.0];

fn harness() -> Harness {
    let mut harness = Harness::new(RendererOptions::default());
    harness.apply(room());
    // The blue box is published for an entity the overlay names.
    harness.apply(vec![RenderDiff::Update {
        handle: RenderHandle::new(12),
        transform: None,
        material: None,
        visible: None,
        metadata: Some(RenderMetadata {
            source_entity: Some(MARKED),
            ..RenderMetadata::default()
        }),
    }]);
    let eye = camera("eye", [0.0, 0.6, 1.0], 0.0, -6.0);
    harness.renderer.set_view_composition(
        &composition(
            vec![eye],
            vec![primary_view("main", "eye", viewport(0.0, 0.0, 1.0, 1.0), 0)],
        ),
        0.0,
    );
    harness
}

fn guide() -> RenderDiff {
    RenderDiff::SetOverlaySet {
        name: "guide".to_owned(),
        set: OverlaySetDescriptor {
            items: vec![
                OverlayItem {
                    shape: OverlayShape::Segment {
                        from: [-1.5, 0.0, -4.0],
                        to: [1.5, 0.0, -4.0],
                    },
                    space: OverlaySpace::World,
                    color: MAGENTA,
                    on_top: true,
                },
                OverlayItem {
                    shape: OverlayShape::WireSphere {
                        center: [0.0, 0.0, 0.0],
                        radius: 0.6,
                    },
                    space: OverlaySpace::Model { entity: MARKED },
                    color: CYAN,
                    on_top: true,
                },
                OverlayItem {
                    shape: OverlayShape::Label {
                        at: [0.0, 0.8, 0.0],
                        text: "blue".to_owned(),
                    },
                    space: OverlaySpace::Model { entity: MARKED },
                    color: CYAN,
                    on_top: true,
                },
            ],
        },
    }
}

fn debug_box() -> RenderDiff {
    let mut node = instance(20, None, "green", transform([0.0, 1.1, -4.0], 0.0, 0.5));
    if let RenderDiff::CreateStaticMeshInstance { instance, .. } = &mut node {
        instance.layer = RenderLayer::Debug;
    }
    node
}

/// The mean x of pixels near `color` after tone mapping (its full channels
/// bright, its empty ones dark), and how many there are.
fn find(rgba: &[u8], color: [f32; 4]) -> (f32, usize) {
    let (mut sum, mut count) = (0.0, 0);
    for (index, pixel) in rgba.as_chunks::<4>().0.iter().enumerate() {
        if (0..3).all(|channel| {
            (pixel[channel] > 160) == (color[channel] > 0.5)
                && (pixel[channel] < 120) == (color[channel] < 0.5)
        }) {
            sum += (index as u32 % WIDTH) as f32;
            count += 1;
        }
    }
    (if count == 0 { 0.0 } else { sum / count as f32 }, count)
}

fn shown(edit: impl FnOnce(&mut OverlayShown)) -> OverlayShown {
    let mut shown = OverlayShown::default();
    edit(&mut shown);
    shown
}

#[test]
fn overlays_and_the_debug_layer_draw_only_while_inspection_shows_them() {
    let mut harness = harness();
    let (_, plain) = harness.composition(0.0);

    harness.apply(vec![guide(), debug_box()]);
    let (stats, hidden) = harness.composition(0.0);
    assert_eq!(stats.overlay_lines, 0, "nothing is shown");
    assert_eq!(
        hidden, plain,
        "hidden overlays and Debug parts draw nothing"
    );
    assert_eq!(harness.renderer.overlay_sets()[0].items, 3);

    harness.renderer.set_overlay_shown(shown(|shown| {
        shown.all_sets = true;
        shown.debug_layer = true;
    }));
    let (stats, all) = harness.composition(0.0);
    // One segment and a sphere's three circles.
    assert_eq!(stats.overlay_lines, 1 + 3 * 24);
    assert!(find(&all, MAGENTA).1 > 60, "the world segment draws");
    let (sphere_x, sphere) = find(&all, CYAN);
    assert!(sphere > 50, "the sphere on the blue box draws");

    harness
        .renderer
        .set_overlay_shown(shown(|shown| shown.debug_layer = true));
    let (stats, debug) = harness.composition(0.0);
    assert_eq!(stats.overlay_lines, 0);
    assert_ne!(debug, plain, "the Debug layer's box draws");
    assert_eq!(find(&debug, MAGENTA).1, 0);

    // A set placed on a node follows it.
    harness.renderer.set_overlay_shown(shown(|shown| {
        shown.sets.insert("guide".to_owned());
    }));
    harness.apply(vec![RenderDiff::Update {
        handle: RenderHandle::new(12),
        transform: Some(transform([0.6, 0.0, -3.6], -20.0, 1.0)),
        material: None,
        visible: None,
        metadata: None,
    }]);
    let (_, moved) = harness.composition(0.0);
    assert!(
        find(&moved, CYAN).0 < sphere_x - 20.0,
        "the sphere moved left with its box"
    );

    harness.renderer.set_overlay_shown(OverlayShown::default());
    harness.apply(vec![RenderDiff::Update {
        handle: RenderHandle::new(12),
        transform: Some(transform([1.3, 0.0, -3.6], -20.0, 1.0)),
        material: None,
        visible: None,
        metadata: None,
    }]);
    let (stats, again) = harness.composition(0.0);
    assert_eq!(stats.overlay_lines, 0);
    assert_eq!(again, plain, "hiding everything leaves the scene as it was");

    // An empty set removes it.
    harness.apply(vec![RenderDiff::SetOverlaySet {
        name: "guide".to_owned(),
        set: OverlaySetDescriptor::default(),
    }]);
    assert!(harness.renderer.overlay_sets().is_empty());
}

#[test]
fn a_camera_space_label_stays_on_its_marker_in_every_view() {
    let mut harness = harness();
    // Over the sky, where the marker's colour is its own.
    let at = [0.0, 0.25, -1.0];
    harness.apply(vec![RenderDiff::SetOverlaySet {
        name: "held".to_owned(),
        set: OverlaySetDescriptor {
            items: vec![
                OverlayItem {
                    shape: OverlayShape::Marker { at, size: 0.4 },
                    space: OverlaySpace::Camera,
                    color: MAGENTA,
                    on_top: true,
                },
                OverlayItem {
                    shape: OverlayShape::Label {
                        at,
                        text: "HELD".to_owned(),
                    },
                    space: OverlaySpace::Camera,
                    color: CYAN,
                    on_top: true,
                },
            ],
        },
    }]);
    harness
        .renderer
        .set_overlay_shown(shown(|shown| shown.all_sets = true));
    let aligned = |rgba: &[u8], what: &str| {
        let (marker_x, marker) = find(rgba, MAGENTA);
        let (label_x, label) = find(rgba, CYAN);
        assert!(
            marker > 10 && label > 10,
            "{what}: both draw ({marker}, {label})"
        );
        assert!(
            (marker_x - label_x).abs() < 12.0,
            "{what}: the label at x {label_x} is on its marker at x {marker_x}"
        );
    };
    aligned(&harness.composition(0.0).1, "the player view");

    // The observer moves the world's projection, not the viewmodel's.
    let observer = camera("observer", [0.5, 0.6, 1.0], 0.0, -6.0);
    harness.renderer.set_observer(Some(observer.pose));
    aligned(&harness.composition(0.0).1, "the observer");
}
