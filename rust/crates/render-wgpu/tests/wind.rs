//! The wind (#9541): a material with a wind bend leans with height above its
//! part's origin, a flutter moves vertices by their colour's alpha, both with
//! presentation time, and the shadow maps follow.

mod common;

use common::*;
use render_model::*;
use render_wgpu::RendererOptions;

fn directional(direction: [f32; 3], shadow: bool) -> RenderDiff {
    RenderDiff::CreateLight {
        handle: RenderHandle::new(77),
        parent: None,
        light: LightDescriptor::Directional {
            color: [1.0; 3],
            intensity: 3.0,
            enabled: true,
            direction,
            range: None,
            shadow_intent: if shadow {
                LightShadowIntent::Requested
            } else {
                LightShadowIntent::Disabled
            },
            shadow: Default::default(),
        },
    }
}

/// A post standing on its origin: `width` across, `height` tall.
fn post(width: f32, height: f32) -> MeshPayloadDescriptor {
    let mut payload = cube();
    let MeshPayloadSource::Inline { positions, .. } = &mut payload.source else {
        unreachable!()
    };
    for point in positions.as_chunks_mut::<3>().0 {
        point[0] *= width;
        point[1] = (point[1] + 0.5) * height;
        point[2] *= width;
    }
    payload.bounds = MeshBoundsDescriptor {
        min: [-width / 2.0, 0.0, -width / 2.0],
        max: [width / 2.0, height, width / 2.0],
    };
    payload
}

/// A card facing the camera, 1 m wide and `height` tall, its vertex colours'
/// alpha 0 along the ground and 1 at the top.
fn card(height: f32) -> MeshPayloadDescriptor {
    let positions = vec![
        -0.5, 0.0, 0.0, 0.5, 0.0, 0.0, 0.5, height, 0.0, -0.5, height, 0.0,
    ];
    let normals = [0.0, 0.0, 1.0].repeat(4);
    let mut payload = payload(positions, normals, vec![0, 1, 2, 0, 2, 3]);
    let MeshPayloadSource::Inline { colors, .. } = &mut payload.source else {
        unreachable!()
    };
    *colors = Some(vec![
        1.0, 1.0, 1.0, 0.0, 1.0, 1.0, 1.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0,
    ]);
    payload.layout.attributes.push(MeshAttribute {
        name: MeshAttributeName::Color,
        components: 4,
        kind: MeshAttributeKind::F32,
    });
    payload
}

fn scene(
    wind: Option<MaterialWindDescriptor>,
    mesh: MeshPayloadDescriptor,
    shadow: bool,
) -> Vec<RenderDiff> {
    let mut ops = vec![RenderDiff::SetBackgroundColor {
        color: [0.0, 0.0, 0.0, 1.0],
    }];
    let mut swaying = coloured_mesh("swaying", mesh, [0.9, 0.9, 0.9, 1.0]);
    if let RenderDiff::DefineMaterial { material } = &mut swaying[0] {
        material.wind = wind;
        material.double_sided = true;
    }
    ops.extend(swaying);
    ops.extend(coloured_mesh("ground", floor(12.0), [0.6, 0.6, 0.6, 1.0]));
    ops.push(instance(
        1,
        None,
        "swaying",
        transform([0.0, -1.0, -4.0], 0.0, 1.0),
    ));
    ops.push(instance(
        2,
        None,
        "ground",
        transform([0.0, -1.0, -4.0], 0.0, 1.0),
    ));
    ops.push(directional([0.0, -0.6, -0.8], shadow));
    ops
}

/// The swaying mesh is the brightest thing in view: lighter than the floor.
const SWAYING: u8 = 185;

/// The mean x of the swaying mesh's pixels on row `y`, if any.
fn centre_of(frame: &[u8], y: u32) -> Option<f32> {
    let bright: Vec<u32> = (0..WIDTH)
        .filter(|&x| pixel(frame, WIDTH, x, y)[0] > SWAYING)
        .collect();
    (!bright.is_empty()).then(|| bright.iter().sum::<u32>() as f32 / bright.len() as f32)
}

/// A post with a bend leans with the wind, further up its height; its foot
/// stands where it stood. The wind feature stands still in no wind and
/// draws as a material without it.
#[test]
fn a_bending_material_leans_with_the_wind_by_its_height() {
    let render = |wind: Option<MaterialWindDescriptor>, scene_wind: Option<WindDescriptor>| {
        let mut harness = Harness::new(RendererOptions {
            default_world_lights: false,
            ..RendererOptions::default()
        });
        let mut ops = scene(wind, post(0.3, 2.0), false);
        ops.push(RenderDiff::SetWind { wind: scene_wind });
        harness.apply(ops);
        harness.renderer.set_animation_time(3.0);
        harness.single(&camera("eye", [0.0, 0.0, 0.0], 0.0, 0.0))
    };
    let bend = Some(MaterialWindDescriptor {
        bend: 0.25,
        flutter: 0.0,
    });
    let blowing = Some(WindDescriptor {
        direction: [1.0, 0.0],
        strength: 2.0,
        gust: 0.0,
    });
    let still = render(None, blowing);
    let standing = render(bend, None);
    let leaning = render(bend, blowing);
    assert_eq!(still, standing, "no wind leaves the feature where it stood");
    // The post stands from a third of the way down the image to nearly
    // three quarters: its foot stays (within the lean of its lowest few
    // centimetres), its top moves right.
    let (foot, top) = (HEIGHT * 72 / 100, HEIGHT / 3);
    let foot_still = centre_of(&still, foot).expect("foot in view");
    let foot_leaning = centre_of(&leaning, foot).expect("leaning foot in view");
    assert!(
        (foot_still - foot_leaning).abs() < 3.0,
        "the foot stands at {foot_still} and {foot_leaning}"
    );
    let top_still = centre_of(&still, top).expect("top in view");
    let top_leaning = centre_of(&leaning, top).expect("leaning top in view");
    assert!(
        top_leaning > top_still + 8.0,
        "the top leans from {top_still} to {top_leaning}"
    );
}

/// A gusting lean moves with presentation time, and the shadow map redraws
/// for the time alone; a held time draws the same frame.
#[test]
fn a_gusting_lean_moves_with_time_and_its_shadow_follows() {
    let mut harness = Harness::new(RendererOptions {
        default_world_lights: false,
        shadows: true,
        ..RendererOptions::default()
    });
    let mut ops = scene(
        Some(MaterialWindDescriptor {
            bend: 0.4,
            flutter: 0.0,
        }),
        post(0.3, 2.0),
        true,
    );
    ops.push(RenderDiff::SetWind {
        wind: Some(WindDescriptor {
            direction: [1.0, 0.0],
            strength: 2.0,
            gust: 1.0,
        }),
    });
    harness.apply(ops);
    // Looking down from behind the post, so its shadow lies across the floor.
    let view = camera("eye", [0.0, 2.0, 0.0], 0.0, -30.0);
    let mut at = |time: f64| {
        harness.renderer.set_animation_time(time);
        harness.single(&view)
    };
    let first = at(1.0);
    let later = at(1.7);
    let held = at(1.7);
    assert_ne!(first, later, "the lean moves with time");
    assert_eq!(held, later, "a held time draws the same");
    // The floor's shadow (the dark pixels below the post's foot) moves too.
    let shadow = |frame: &[u8]| -> Vec<(u32, u32)> {
        (0..WIDTH)
            .flat_map(|x| (HEIGHT / 2..HEIGHT).map(move |y| (x, y)))
            .filter(|&(x, y)| pixel(frame, WIDTH, x, y)[0] < 40)
            .collect()
    };
    let (first_shadow, later_shadow) = (shadow(&first), shadow(&later));
    assert!(first_shadow.len() > 50, "a shadow lies on the floor");
    assert_ne!(first_shadow, later_shadow, "the shadow follows the lean");
}

/// A flutter moves the vertices whose colour alpha weights it: the card's
/// top edge moves between two times, its foot (alpha 0) does not.
#[test]
fn a_flutter_moves_vertices_by_their_colour_alpha() {
    let mut harness = Harness::new(RendererOptions {
        default_world_lights: false,
        ..RendererOptions::default()
    });
    let mut ops = scene(
        Some(MaterialWindDescriptor {
            bend: 0.0,
            flutter: 0.3,
        }),
        card(1.5),
        false,
    );
    ops.push(RenderDiff::SetWind {
        wind: Some(WindDescriptor {
            direction: [0.0, 1.0],
            strength: 1.0,
            gust: 0.0,
        }),
    });
    harness.apply(ops);
    let view = camera("eye", [0.0, 0.0, 0.0], 0.0, 0.0);
    let mut at = |time: f64| {
        harness.renderer.set_animation_time(time);
        harness.single(&view)
    };
    let first = at(0.0);
    let later = at(0.25);
    let edge = |frame: &[u8], from_top: bool| -> u32 {
        let rows: Vec<u32> = (0..HEIGHT)
            .filter(|&y| (0..WIDTH).any(|x| pixel(frame, WIDTH, x, y)[0] > SWAYING))
            .collect();
        if from_top {
            rows[0]
        } else {
            *rows.last().unwrap()
        }
    };
    assert_eq!(
        edge(&first, false),
        edge(&later, false),
        "the foot, at alpha 0, holds"
    );
    assert_ne!(first, later, "the top flutters");
    let left_edge = |frame: &[u8]| -> u32 {
        let y = HEIGHT * 45 / 100;
        (0..WIDTH)
            .find(|&x| pixel(frame, WIDTH, x, y)[0] > SWAYING)
            .expect("the card's upper part in view")
    };
    assert_ne!(left_edge(&first), left_edge(&later), "the top's edge moves");
}
