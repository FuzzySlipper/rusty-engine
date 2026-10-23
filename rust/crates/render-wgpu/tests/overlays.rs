//! Overlay sets and the Debug layer (#10022): nothing draws, and nothing is
//! resolved, while inspection shows nothing; shown sets draw their lines in
//! the item's space, follow the node they are placed on, and the Debug
//! layer's parts draw only while it is shown. Landmark pairs (#10036)
//! measure across spaces as drawn and draw green within tolerance, red
//! outside.

mod common;

use common::*;
use render_host_contracts::{RendererCameraInterpolation, RendererCameraMotion};
use render_model::*;
use render_wgpu::{CollisionShown, LandmarkDrawn, OverlayLine, OverlayShown, RendererOptions};

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
            pairs: Vec::new(),
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

/// Collision wires (#10288) are lines the Engine gives the renderer: they
/// draw, depth-tested in the world pass, only while inspection shows them.
#[test]
fn collision_lines_draw_only_while_inspection_shows_collision() {
    let mut harness = harness();
    let (_, plain) = harness.composition(0.0);
    harness.renderer.set_collision_lines(&[OverlayLine {
        // Above the floor: the wire is depth-tested.
        from: [-1.5, 0.3, -3.0],
        to: [1.5, 0.3, -3.0],
        color: MAGENTA,
    }]);
    let (stats, hidden) = harness.composition(0.0);
    assert_eq!(stats.overlay_lines, 0);
    assert_eq!(hidden, plain);

    let collision = shown(|shown| {
        shown.collision = Some(CollisionShown {
            at: None,
            radius: 8.0,
        })
    });
    assert!(collision.any());
    harness.renderer.set_overlay_shown(collision);
    let (stats, drawn) = harness.composition(0.0);
    assert_eq!(stats.overlay_lines, 1);
    assert!(find(&drawn, MAGENTA).1 > 60, "the wire draws");

    harness.renderer.set_overlay_shown(OverlayShown::default());
    let (stats, again) = harness.composition(0.0);
    assert_eq!(stats.overlay_lines, 0);
    assert_eq!(again, plain);
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
            pairs: Vec::new(),
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

fn at(space: OverlaySpace, point: [f32; 3], axis: [f32; 3]) -> Landmark {
    Landmark::At {
        space,
        point,
        rotation: [0.0, 0.0, 0.0, 1.0],
        axis,
    }
}

fn pair(name: &str, from: Landmark, to: Landmark, tolerance: f32) -> LandmarkPair {
    LandmarkPair {
        name: name.to_owned(),
        from,
        to,
        tolerance,
        aim: LandmarkAim::None,
        angle_tolerance: 0.0,
    }
}

fn aimed(mut pair: LandmarkPair, aim: LandmarkAim, angle_tolerance: f32) -> LandmarkPair {
    pair.aim = aim;
    pair.angle_tolerance = angle_tolerance;
    pair
}

fn close(a: [f32; 3], b: [f32; 3]) -> bool {
    a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-3)
}

#[test]
fn landmark_pairs_measure_across_spaces_and_draw_by_tolerance() {
    let mut harness = harness();
    // The blue box a metre right of the world landmarks, turned a quarter
    // about y: its +x frame axis is the world's -z.
    harness.apply(vec![RenderDiff::Update {
        handle: RenderHandle::new(12),
        transform: Some(transform([1.0, 0.0, -4.0], 90.0, 1.0)),
        material: None,
        visible: None,
        metadata: None,
    }]);
    let boxed = OverlaySpace::Model { entity: MARKED };
    // Where the player's camera (0, 0.6, 1), pitched 6 degrees down, sees a
    // metre ahead.
    let pitch = (-6.0f32).to_radians();
    let seen = [0.0, 0.6 + pitch.sin(), 1.0 - pitch.cos()];
    harness.apply(vec![RenderDiff::SetOverlaySet {
        name: "grip".to_owned(),
        set: OverlaySetDescriptor {
            items: Vec::new(),
            pairs: vec![
                pair(
                    "near",
                    at(OverlaySpace::World, [0.98, 0.0, -4.0], [0.0; 3]),
                    at(boxed, [0.0; 3], [0.0; 3]),
                    0.05,
                ),
                pair(
                    "far",
                    at(OverlaySpace::World, [0.0, 0.0, -4.0], [0.0; 3]),
                    at(boxed, [0.0; 3], [0.0; 3]),
                    0.05,
                ),
                aimed(
                    pair(
                        "facing",
                        at(OverlaySpace::World, [0.0, 1.0, -4.0], [1.0, 0.0, 0.0]),
                        at(boxed, [0.0; 3], [0.0, 1.0, 0.0]),
                        0.0,
                    ),
                    LandmarkAim::Line,
                    10.0,
                ),
                aimed(
                    pair(
                        "across",
                        at(OverlaySpace::World, [0.0, 1.0, -4.0], [0.0, 0.0, -1.0]),
                        at(boxed, [0.0; 3], [1.0, 0.0, 0.0]),
                        0.0,
                    ),
                    LandmarkAim::Axis,
                    10.0,
                ),
                pair(
                    "seen",
                    at(OverlaySpace::Camera, [0.0, 0.0, -1.0], [0.0; 3]),
                    at(OverlaySpace::World, seen, [0.0; 3]),
                    0.001,
                ),
                pair(
                    "nothing",
                    at(OverlaySpace::Model { entity: 404 }, [0.0; 3], [0.0; 3]),
                    at(boxed, [0.0; 3], [0.0; 3]),
                    0.05,
                ),
            ],
        },
    }]);
    // Landmarks are measured where the last frame drew them.
    harness.composition(0.0);
    let [set] = harness.renderer.landmark_sets().try_into().unwrap();
    assert_eq!((set.name.as_str(), set.shown), ("grip", false));
    let drawn = |name: &str| -> Option<LandmarkDrawn> {
        set.pairs
            .iter()
            .find(|pair| pair.name == name)
            .unwrap()
            .drawn
    };
    let near = drawn("near").unwrap();
    assert!(
        (near.distance - 0.02).abs() < 1e-4 && near.within,
        "{near:?}"
    );
    // The turned box's +z is world +x, so 2 cm toward world -x is 2 cm
    // ahead along its -z.
    assert!(close(near.offset, [0.0, 0.0, -0.02]), "{near:?}");
    let far = drawn("far").unwrap();
    assert!((far.distance - 1.0).abs() < 1e-4 && !far.within, "{far:?}");
    assert_eq!(far.angle, None);
    // Facing the box's vertical line: straight at it, though 1 m above it.
    let facing = drawn("facing").unwrap();
    assert!(facing.angle.unwrap() < 0.1 && facing.within, "{facing:?}");
    // World -z against the box's turned +x, which is world -z.
    let across = drawn("across").unwrap();
    assert!(across.angle.unwrap() < 0.1 && across.within, "{across:?}");
    let seen_pair = drawn("seen").unwrap();
    assert!(
        seen_pair.within,
        "camera space reaches the world: {seen_pair:?}"
    );
    assert!(close(seen_pair.from, seen), "{seen_pair:?}");
    assert_eq!(drawn("nothing"), None, "a landmark on nothing drawn");

    let (stats, _) = harness.composition(0.0);
    assert_eq!(stats.overlay_lines, 0, "a hidden set draws no pairs");
    harness
        .renderer
        .set_overlay_shown(shown(|shown| shown.all_sets = true));
    let (stats, rgba) = harness.composition(0.0);
    // Five drawn pairs: a segment and a cross each, and the two aims.
    assert_eq!(stats.overlay_lines, 5 * 4 + 2);
    assert!(
        find(&rgba, [0.0, 1.0, 0.0, 1.0]).1 > 20,
        "pairs within draw green"
    );
    assert!(
        find(&rgba, [1.0, 0.0, 0.0, 1.0]).1 > 20,
        "a pair outside draws red"
    );
    assert!(harness.renderer.landmark_sets()[0].shown);
}

#[test]
fn a_landmark_readout_places_camera_points_by_the_camera_the_last_frame_drew_with() {
    let mut harness = harness();
    // The camera moves from x 0 to x 1 between two samples a tenth of a
    // second apart, presented 0.1 s late.
    let sample = |id: &str, x: f64, time: f64| {
        let mut moving = camera("moving", [x, 0.6, 1.0], 0.0, -6.0);
        moving.motion = Some(RendererCameraMotion {
            sample_id: id.to_owned(),
            sample_time_seconds: time,
            delay_seconds: 0.1,
            interpolation: RendererCameraInterpolation::Pose,
            cut: false,
        });
        composition(
            vec![moving],
            vec![primary_view(
                "main",
                "moving",
                viewport(0.0, 0.0, 1.0, 1.0),
                0,
            )],
        )
    };
    harness
        .renderer
        .set_view_composition(&sample("1", 0.0, 0.0), 0.0);
    harness
        .renderer
        .set_view_composition(&sample("2", 1.0, 0.1), 0.1);
    // A metre ahead of the camera halfway along, where it is at 0.15.
    let pitch = (-6.0f32).to_radians();
    let seen = |x: f32| [x, 0.6 + pitch.sin(), 1.0 - pitch.cos()];
    harness.apply(vec![RenderDiff::SetOverlaySet {
        name: "held".to_owned(),
        set: OverlaySetDescriptor {
            items: Vec::new(),
            pairs: vec![pair(
                "ahead",
                at(OverlaySpace::Camera, [0.0, 0.0, -1.0], [0.0; 3]),
                at(OverlaySpace::World, seen(0.5), [0.0; 3]),
                0.01,
            )],
        },
    }]);
    let ahead = |harness: &Harness| harness.renderer.landmark_sets()[0].pairs[0].drawn.unwrap();
    harness.composition(0.15);
    let drawn = ahead(&harness);
    assert!(
        drawn.within && drawn.distance < 1e-3,
        "drawn halfway: {drawn:?}"
    );
    assert!(close(drawn.from, seen(0.5)), "{drawn:?}");
    // Read again later without drawing: the camera motion has moved on, but
    // the readout is still the frame that was drawn.
    assert_eq!(ahead(&harness), drawn);
    // The next frame draws the camera at its last sample, and the readout
    // follows it.
    harness.composition(0.4);
    let moved = ahead(&harness);
    assert!(close(moved.from, seen(1.0)), "{moved:?}");
    assert!(
        !moved.within && (moved.distance - 0.5).abs() < 1e-3,
        "{moved:?}"
    );
}
