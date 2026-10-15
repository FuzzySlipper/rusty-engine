//! Inspection views (#10023): Engine cameras beside the product's views.
//! An anchored view draws at its rect only while the page reports it and
//! leaves the player view elsewhere as it was; the viewmodel layer draws
//! in it as world geometry at the primary camera's pose, so an outside
//! camera sees the held assembly where the player's camera holds it.

mod common;

use common::*;
use render_host_contracts::{RendererCameraPose, RendererCameraProjection};
use render_model::*;
use render_wgpu::{InspectionView, OffscreenTarget, RendererOptions};

const EYE: [f64; 3] = [0.0, 0.6, 1.0];
const HELD: u64 = 30;

fn harness() -> Harness {
    let mut harness = Harness::new(RendererOptions::default());
    harness.apply(room());
    // A held box in front of the camera, on the viewmodel layer.
    let mut held = instance(HELD, None, "red", transform([0.15, -0.15, -0.5], 0.0, 0.15));
    if let RenderDiff::CreateStaticMeshInstance { instance, .. } = &mut held {
        instance.layer = RenderLayer::Viewmodel;
    }
    harness.apply(vec![held]);
    let eye = camera("eye", EYE, 0.0, -6.0);
    harness.renderer.set_view_composition(
        &composition(
            vec![eye],
            vec![primary_view("main", "eye", viewport(0.0, 0.0, 1.0, 1.0), 0)],
        ),
        0.0,
    );
    harness
}

/// A camera at `position` looking at the player's eye.
fn looking_at_eye(id: &str, position: [f64; 3], anchor: Option<&str>) -> InspectionView {
    let to = [
        EYE[0] - position[0],
        EYE[1] - position[1],
        EYE[2] - position[2],
    ];
    let length = (to[0] * to[0] + to[1] * to[1] + to[2] * to[2]).sqrt();
    InspectionView {
        id: id.to_owned(),
        pose: RendererCameraPose {
            position,
            yaw_degrees: to[0].atan2(-to[2]).to_degrees(),
            pitch_degrees: (to[1] / length).asin().to_degrees(),
        },
        projection: RendererCameraProjection::Perspective {
            fov_y_degrees: 50.0,
            near: 0.02,
            far: 100.0,
        },
        anchor: anchor.map(str::to_owned),
        focus: EYE,
    }
}

/// How many pixels are the held box's red, and their mean x.
fn red(rgba: &[u8], width: u32) -> (usize, f32) {
    let (mut count, mut sum) = (0, 0.0);
    for (index, pixel) in rgba.as_chunks::<4>().0.iter().enumerate() {
        if pixel[0] > 150 && pixel[1] < 90 && pixel[2] < 90 {
            count += 1;
            sum += (index as u32 % width) as f32;
        }
    }
    (count, if count == 0 { 0.0 } else { sum / count as f32 })
}

#[test]
fn an_anchored_view_draws_at_its_rect_only_while_the_page_reports_it() {
    let mut harness = harness();
    let (_, player) = harness.composition(0.0);
    harness.renderer.set_inspection_views(vec![looking_at_eye(
        "side",
        [1.2, 0.8, 0.6],
        Some("pane"),
    )]);
    let (_, unreported) = harness.composition(0.0);
    assert_eq!(unreported, player, "an unconsumed view draws nothing");

    harness
        .renderer
        .set_viewport_anchors([("pane".to_owned(), viewport(0.5, 0.0, 0.5, 0.5))].into());
    let (_, shown) = harness.composition(0.0);
    // Viewports are bottom-left based: the pane is the lower right quarter.
    let inside = |x: u32, y: u32| x >= WIDTH / 2 && y >= HEIGHT / 2;
    let mut changed_inside = 0;
    for (index, (a, b)) in player
        .as_chunks::<4>()
        .0
        .iter()
        .zip(shown.as_chunks::<4>().0)
        .enumerate()
    {
        let (x, y) = (index as u32 % WIDTH, index as u32 / WIDTH);
        if inside(x, y) {
            changed_inside += usize::from(a != b);
        } else {
            assert_eq!(a, b, "the player view outside the pane at ({x}, {y})");
        }
    }
    assert!(changed_inside > 1000, "the pane draws its own camera");

    harness.renderer.set_inspection_views(Vec::new());
    let (_, closed) = harness.composition(0.0);
    assert_eq!(closed, player, "closing the view restores the player view");
}

#[test]
fn a_frame_records_the_inspection_views_it_drew_with_their_cameras() {
    let mut harness = harness();
    let side = looking_at_eye("side", [1.2, 0.8, 0.6], Some("pane"));
    let capture_only = looking_at_eye("off", [0.0, 2.0, 2.0], None);
    harness
        .renderer
        .set_inspection_views(vec![side.clone(), capture_only]);
    harness.composition(0.0);
    assert!(
        harness.renderer.drawn_inspection_views().is_empty(),
        "no rect reported, no capture-only view: nothing drawn"
    );
    harness
        .renderer
        .set_viewport_anchors([("pane".to_owned(), viewport(0.5, 0.0, 0.5, 0.5))].into());
    harness.composition(0.0);
    assert_eq!(
        harness.renderer.drawn_inspection_views(),
        std::slice::from_ref(&side)
    );
    // A camera change shows in the next frame's record, not the last one's.
    let mut moved = side.clone();
    moved.pose.position = [2.0, 1.0, 0.0];
    harness.renderer.set_inspection_views(vec![moved.clone()]);
    assert_eq!(harness.renderer.drawn_inspection_views(), [side]);
    harness.composition(0.0);
    assert_eq!(harness.renderer.drawn_inspection_views(), [moved]);
}

#[test]
fn the_viewmodel_stays_at_the_primary_camera_as_an_inspection_camera_moves() {
    let mut harness = harness();
    let target = OffscreenTarget::new(&harness.gpu, WIDTH, HEIGHT, 4);
    let capture = |harness: &mut Harness, position: [f64; 3]| {
        harness
            .renderer
            .set_inspection_views(vec![looking_at_eye("orbit", position, None)]);
        harness
            .renderer
            .render_inspection_view(&target, "orbit", 0.0)
            .expect("the view exists");
        red(&target.read_rgba(&harness.gpu), WIDTH)
    };
    // From either side of the player's eye, the held box is in view, on
    // the side of the eye it is held on: right of the eye, so on the left
    // from a camera looking back at the eye from in front.
    let (right_side, right_x) = capture(&mut harness, [0.9, 0.6, 0.2]);
    let (front, front_x) = capture(&mut harness, [0.0, 0.6, 0.1]);
    assert!(right_side > 50 && front > 50, "{right_side} {front}");
    assert!(
        front_x < WIDTH as f32 / 2.0,
        "seen from in front, the box is left of the eye: {front_x}"
    );
    assert!(right_x > 0.0);
    assert!(
        harness
            .renderer
            .render_inspection_view(&target, "missing", 0.0)
            .is_none(),
        "no view, no capture"
    );
}
