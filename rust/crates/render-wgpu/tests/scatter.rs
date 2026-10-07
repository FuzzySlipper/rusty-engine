//! Scattered copies (#9546): a scatter patch draws its copies where a static
//! instance each would draw, as one instanced draw per mesh group culled by
//! the patch's bounds; copies take their tint, shrink away over the fade
//! distance, follow their parent and cast shadows only when asked.

mod common;

use common::*;
use render_model::*;
use render_wgpu::RendererOptions;

const POST: &str = "post";
const SPOTS: [[f32; 3]; 3] = [[-1.5, 0.0, 0.0], [0.0, 0.0, 0.0], [1.5, 0.0, 0.0]];
/// The patch's parent: the copies stand around (0, -1, -4).
const GROUND_AT: [f32; 3] = [0.0, -1.0, -4.0];

fn directional(shadow: bool) -> RenderDiff {
    RenderDiff::CreateLight {
        handle: RenderHandle::new(77),
        parent: None,
        light: LightDescriptor::Directional {
            color: [1.0; 3],
            intensity: 3.0,
            enabled: true,
            direction: [0.4, -0.6, -0.7],
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

/// A post standing on its origin, 0.3 m across and 1 m tall.
fn post() -> MeshPayloadDescriptor {
    let mut payload = cube();
    let MeshPayloadSource::Inline { positions, .. } = &mut payload.source else {
        unreachable!()
    };
    for point in positions.as_chunks_mut::<3>().0 {
        point[0] *= 0.3;
        point[1] += 0.5;
        point[2] *= 0.3;
    }
    payload.bounds = MeshBoundsDescriptor {
        min: [-0.15, 0.0, -0.15],
        max: [0.15, 1.0, 0.15],
    };
    payload
}

fn copy(at: [f32; 3], tint: [f32; 3]) -> ScatterInstance {
    ScatterInstance {
        translation: at,
        rotation: [0.0, 0.0, 0.0, 1.0],
        scale: 1.0,
        tint,
    }
}

fn patch(
    handle: u64,
    parent: u64,
    copies: Vec<ScatterInstance>,
    fade: ScatterFade,
    casts: bool,
) -> RenderDiff {
    RenderDiff::CreateScatterPatch {
        handle: RenderHandle::new(handle),
        parent: Some(RenderHandle::new(parent)),
        patch: ScatterPatchDescriptor {
            asset: format!("mesh/{POST}"),
            material_overrides: Vec::new(),
            instances: copies,
            fade,
            shadow_casting: if casts {
                ShadowCasting::Cast
            } else {
                ShadowCasting::None
            },
        },
    }
}

fn ground_node(handle: u64) -> RenderDiff {
    RenderDiff::Create {
        handle: RenderHandle::new(handle),
        parent: None,
        node: RenderNode {
            transform: transform(GROUND_AT, 0.0, 1.0),
            ..RenderNode::new(Geometry::Group)
        },
    }
}

/// The floor and the post mesh; the light casts when `shadow`.
fn stage(shadow: bool) -> Vec<RenderDiff> {
    let mut ops = vec![RenderDiff::SetBackgroundColor {
        color: [0.0, 0.0, 0.0, 1.0],
    }];
    ops.extend(coloured_mesh(POST, post(), [0.9, 0.9, 0.9, 1.0]));
    ops.extend(coloured_mesh("ground", floor(12.0), [0.3, 0.3, 0.3, 1.0]));
    ops.push(instance(2, None, "ground", transform(GROUND_AT, 0.0, 1.0)));
    ops.push(ground_node(5));
    ops.push(directional(shadow));
    ops
}

fn harness(shadows: bool) -> Harness {
    Harness::new(RendererOptions {
        default_world_lights: false,
        shadows,
        ..RendererOptions::default()
    })
}

fn eye() -> render_host_contracts::RendererCompositionCamera {
    camera("eye", [0.0, 0.0, 0.0], 0.0, 0.0)
}

fn render(harness: &mut Harness) -> (render_wgpu::FrameStats, Vec<u8>) {
    let stats = harness.renderer.render_offscreen(&eye(), &harness.target);
    (stats, harness.target.read_rgba(&harness.gpu))
}

/// Pixels as bright as a lit post.
fn bright(frame: &[u8]) -> usize {
    frame.chunks_exact(4).filter(|pixel| pixel[1] > 150).count()
}

fn whole() -> Vec<ScatterInstance> {
    SPOTS.iter().map(|at| copy(*at, [1.0; 3])).collect()
}

#[test]
fn a_patch_draws_its_copies_as_one_draw_where_instances_would_stand() {
    let mut separate = harness(false);
    let mut ops = stage(false);
    for (index, at) in SPOTS.iter().enumerate() {
        ops.push(instance(
            10 + index as u64,
            Some(5),
            POST,
            transform(*at, 0.0, 1.0),
        ));
    }
    separate.apply(ops);
    let (_, expected) = render(&mut separate);

    let mut scattered = harness(false);
    let mut ops = stage(false);
    ops.push(patch(20, 5, whole(), ScatterFade::default(), true));
    scattered.apply(ops);
    let (stats, drawn) = render(&mut scattered);
    assert!(bright(&drawn) > 300, "the posts are in view");
    assert_eq!(drawn, expected, "copies draw where instances would");
    // The floor and one draw of every copy.
    assert_eq!((stats.draws, stats.instances), (2, 4));

    // A patch out of view costs no draw.
    scattered.apply(vec![
        RenderDiff::Destroy {
            handle: RenderHandle::new(20),
        },
        RenderDiff::Create {
            handle: RenderHandle::new(6),
            parent: None,
            node: RenderNode {
                transform: transform([0.0, -1.0, 6.0], 0.0, 1.0),
                ..RenderNode::new(Geometry::Group)
            },
        },
        patch(21, 6, whole(), ScatterFade::default(), true),
    ]);
    let (stats, _) = render(&mut scattered);
    assert_eq!(
        (stats.draws, stats.instances),
        (1, 1),
        "only the floor draws"
    );
}

#[test]
fn copies_take_their_tint_and_follow_their_parent_until_it_goes() {
    let mut harness = harness(false);
    let mut ops = stage(false);
    let mut copies = whole();
    copies[0].tint = [1.0, 0.1, 0.1];
    ops.push(patch(20, 5, copies, ScatterFade::default(), true));
    harness.apply(ops);
    let (_, frame) = render(&mut harness);
    // The left post (left third of the image) is red; the others are white.
    let reds = |frame: &[u8], from: u32, to: u32| {
        (0..HEIGHT)
            .flat_map(|y| (from..to).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                let p = pixel(frame, WIDTH, x, y);
                p[0] > 150 && p[1] < p[0] / 2
            })
            .count()
    };
    assert!(reds(&frame, 0, WIDTH / 3) > 50, "the tinted copy is red");
    assert_eq!(
        reds(&frame, WIDTH / 3, WIDTH),
        0,
        "the others keep the material's colour"
    );

    // Moving the parent moves every copy with it.
    let before = bright(&frame);
    harness.apply(vec![RenderDiff::Update {
        handle: RenderHandle::new(5),
        transform: Some(transform([0.0, -1.0, -8.0], 0.0, 1.0)),
        material: None,
        visible: None,
        metadata: None,
    }]);
    let (_, farther) = render(&mut harness);
    let after = bright(&farther);
    assert!(
        after > 0 && after * 2 < before,
        "farther copies are smaller: {before} then {after}"
    );

    // Destroying the parent destroys the patch.
    harness.apply(vec![RenderDiff::Destroy {
        handle: RenderHandle::new(5),
    }]);
    let (stats, frame) = render(&mut harness);
    assert_eq!(bright(&frame), 0);
    assert_eq!(stats.instances, 1);
}

#[test]
fn copies_shrink_away_over_the_fade_distance() {
    let count = |fade: ScatterFade| {
        let mut harness = harness(false);
        let mut ops = stage(false);
        ops.push(patch(20, 5, whole(), fade, true));
        harness.apply(ops);
        bright(&render(&mut harness).1)
    };
    // The copies stand 4 to 4.5 m from the eye.
    let unfaded = count(ScatterFade::default());
    let near = count(ScatterFade {
        start: 10.0,
        end: 20.0,
    });
    let fading = count(ScatterFade {
        start: 3.0,
        end: 5.5,
    });
    let gone = count(ScatterFade {
        start: 1.0,
        end: 2.0,
    });
    assert_eq!(near, unfaded, "within the fade's start a copy is whole");
    assert!(
        fading > 0 && fading < unfaded * 3 / 4,
        "{fading} of {unfaded} pixels while fading"
    );
    assert_eq!(gone, 0, "beyond the fade's end a copy is gone");
}

#[test]
fn copies_cast_shadows_only_when_asked() {
    let shadowed = |casts: bool| {
        let mut harness = harness(true);
        let mut ops = stage(true);
        ops.push(patch(20, 5, whole(), ScatterFade::default(), casts));
        harness.apply(ops);
        let (_, frame) = render(&mut harness);
        // Dark floor pixels in the lower half.
        (WIDTH * HEIGHT / 2..WIDTH * HEIGHT)
            .filter(|index| {
                let p = &frame[*index as usize * 4..*index as usize * 4 + 4];
                p[0] < 25 && p[1] < 25
            })
            .count()
    };
    let casting = shadowed(true);
    let clear = shadowed(false);
    assert!(
        casting > clear + 100,
        "{casting} shadowed pixels casting, {clear} not"
    );
}

#[test]
fn gpu_culling_draws_the_cpu_picture_of_a_patch() {
    let mut harness = harness(false);
    let mut ops = stage(false);
    let many: Vec<ScatterInstance> = (0..200)
        .map(|index| {
            let (x, z) = ((index % 20) as f32 * 0.6 - 6.0, (index / 20) as f32 * -0.6);
            copy([x, 0.0, z], [1.0, 0.8 + 0.001 * index as f32, 1.0])
        })
        .collect();
    ops.push(patch(
        20,
        5,
        many,
        ScatterFade {
            start: 6.0,
            end: 9.0,
        },
        true,
    ));
    harness.apply(ops);
    let (_, cpu) = render(&mut harness);
    harness.renderer.set_options(RendererOptions {
        default_world_lights: false,
        gpu_culling: true,
        ..RendererOptions::default()
    });
    let (_, gpu) = render(&mut harness);
    if let Some(reason) = harness.renderer.gpu_readout().gpu_culling.refused {
        eprintln!("GPU culling refused on this device: {reason}");
        return;
    }
    assert!(bright(&cpu) > 1000);
    let differing = cpu
        .chunks_exact(4)
        .zip(gpu.chunks_exact(4))
        .filter(|(a, b)| a != b)
        .count();
    assert!(
        differing * 500 < cpu.len() / 4,
        "{differing} pixels differ between CPU and GPU culling"
    );
}
