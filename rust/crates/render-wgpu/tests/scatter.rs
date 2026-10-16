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
        rank: 0.5,
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
            lods: Vec::new(),
            lod_band: 0.0,
            thinning: Default::default(),
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
    frame
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|pixel| pixel[1] > 150)
        .count()
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
    // The floor and one draw of every copy: from the CPU list, or from the
    // per-copy selection, whose counts read back a frame later.
    assert_eq!(stats.draws, 2);
    let selection = selection_counts(&mut scattered);
    if selection.enabled {
        let copies = SPOTS.len() as u32;
        assert_eq!((selection.tested, selection.drawn), (copies, copies));
    } else {
        assert_eq!(stats.instances, 4);
    }

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
    let selection = selection_counts(&mut scattered);
    if selection.enabled {
        // Its copies are tested and none drawn.
        assert_eq!(
            (selection.drawn, selection.outside),
            (0, SPOTS.len() as u32)
        );
    } else {
        assert_eq!(
            (stats.draws, stats.instances),
            (1, 1),
            "only the floor draws"
        );
    }
}

/// The selection's counts once they have read back.
fn selection_counts(harness: &mut Harness) -> render_wgpu::ScatterSelectionReadout {
    for _ in 0..3 {
        render(harness);
    }
    harness.renderer.gpu_readout().scatter_selection
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
        ..ScatterFade::default()
    });
    let fading = count(ScatterFade {
        start: 3.0,
        end: 5.5,
        ..ScatterFade::default()
    });
    let gone = count(ScatterFade {
        start: 1.0,
        end: 2.0,
        ..ScatterFade::default()
    });
    assert_eq!(near, unfaded, "within the fade's start a copy is whole");
    assert!(
        fading > 0 && fading < unfaded * 3 / 4,
        "{fading} of {unfaded} pixels while fading"
    );
    assert_eq!(gone, 0, "beyond the fade's end a copy is gone");
}

#[test]
fn copies_grow_in_past_the_near_band() {
    let count = |fade: ScatterFade| {
        let mut harness = harness(false);
        let mut ops = stage(false);
        ops.push(patch(20, 5, whole(), fade, true));
        harness.apply(ops);
        bright(&render(&mut harness).1)
    };
    // The copies stand 4 to 4.5 m from the eye.
    let whole_copies = count(ScatterFade::default());
    let past = count(ScatterFade {
        near_start: 1.0,
        near_end: 2.0,
        ..ScatterFade::default()
    });
    let growing = count(ScatterFade {
        near_start: 3.0,
        near_end: 5.5,
        ..ScatterFade::default()
    });
    let cut = count(ScatterFade {
        near_start: 6.0,
        near_end: 6.0,
        ..ScatterFade::default()
    });
    assert_eq!(past, whole_copies, "past the near band a copy is whole");
    assert!(
        growing > 0 && growing < whole_copies * 3 / 4,
        "{growing} of {whole_copies} pixels while growing in"
    );
    assert_eq!(cut, 0, "within a hard near cut a copy is gone");
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
            ..ScatterFade::default()
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
        .as_chunks::<4>()
        .0
        .iter()
        .zip(gpu.as_chunks::<4>().0)
        .filter(|(a, b)| a != b)
        .count();
    assert!(
        differing * 500 < cpu.len() / 4,
        "{differing} pixels differ between CPU and GPU culling"
    );
}

/// Pixels as red as a lit red post.
fn red(frame: &[u8]) -> usize {
    frame
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|pixel| pixel[0] > 150 && pixel[1] < 75)
        .count()
}

const RED_POST: &str = "red-post";

/// Pixels where a red and a white post share the samples.
fn mixed(frame: &[u8]) -> usize {
    frame
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|pixel| pixel[0] > 150 && (75..=150).contains(&pixel[1]))
        .count()
}

/// Pixels a post covers, white, red or both.
fn covered(frame: &[u8]) -> usize {
    frame
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|pixel| pixel[0] > 150)
        .count()
}

/// The posts with a red level of detail from `start`, crossfading over
/// `band`; the frame and the selection's counts.
fn levels(start: f32, band: f32) -> (Vec<u8>, render_wgpu::ScatterSelectionReadout) {
    let mut harness = harness(false);
    let mut ops = stage(false);
    ops.extend(coloured_mesh(RED_POST, post(), [0.9, 0.05, 0.05, 1.0]));
    let mut created = patch(20, 5, whole(), ScatterFade::default(), true);
    if let RenderDiff::CreateScatterPatch { patch, .. } = &mut created {
        patch.lods = vec![ScatterLod {
            asset: format!("mesh/{RED_POST}"),
            material_overrides: Vec::new(),
            start,
            impostor: false,
        }];
        patch.lod_band = band;
    }
    ops.push(created);
    harness.apply(ops);
    let (_, frame) = render(&mut harness);
    let selection = selection_counts(&mut harness);
    (frame, selection)
}

#[test]
fn copies_draw_the_level_their_distance_reaches_and_crossfade_between_levels() {
    // The copies stand 4 to 4.5 m from the eye.
    let (near_frame, near) = levels(10.0, 0.0);
    let posts = covered(&near_frame);
    assert!(bright(&near_frame) > 300 && red(&near_frame) == 0);
    if !near.enabled {
        // Without the per-copy selection, copies draw their own mesh only.
        return;
    }
    assert_eq!(near.levels, [0, 0, 0]);
    let (far_frame, far) = levels(2.0, 0.0);
    assert!(red(&far_frame) > 300 && bright(&far_frame) == 0);
    assert_eq!(far.levels[0], SPOTS.len() as u32);
    // Within the band both levels draw, sharing each pixel's samples: the
    // posts cover what they did, mixed white and red.
    let (mid_frame, mid) = levels(5.0, 2.0);
    let shared = mixed(&mid_frame);
    assert!(shared > posts / 2, "{shared} of {posts}");
    assert!(
        covered(&mid_frame).abs_diff(posts) < posts / 10,
        "{} against {posts}",
        covered(&mid_frame)
    );
    assert_eq!(mid.drawn, 2 * SPOTS.len() as u32);
}

/// The posts ranked 0.1, 0.4 and 0.8, thinned to a share of a half past a
/// metre, growing at most `max_scale`; their bright pixels and the counts.
fn thinned(max_scale: f32) -> (usize, render_wgpu::ScatterSelectionReadout) {
    let mut harness = harness(false);
    let mut ops = stage(false);
    let mut copies = whole();
    for (copy, rank) in copies.iter_mut().zip([0.1, 0.4, 0.8]) {
        copy.rank = rank;
    }
    let mut created = patch(20, 5, copies, ScatterFade::default(), true);
    if let RenderDiff::CreateScatterPatch { patch, .. } = &mut created {
        patch.thinning = ScatterThinning {
            points: vec![[1.0, 0.5]],
            max_scale,
        };
    }
    ops.push(created);
    harness.apply(ops);
    let (_, frame) = render(&mut harness);
    (bright(&frame), selection_counts(&mut harness))
}

#[test]
fn thinning_keeps_the_copies_ranked_below_its_share_and_grows_them() {
    let whole_posts = bright(&levels(10.0, 0.0).0);
    let (kept, selection) = thinned(1.0);
    if !selection.enabled {
        return;
    }
    assert_eq!((selection.drawn, selection.thinned), (2, 1));
    assert!(
        kept < whole_posts && kept > whole_posts / 2,
        "{kept} of {whole_posts}"
    );
    // Grown by the inverse square root of the share, the two cover more.
    let (grown, _) = thinned(2.0);
    assert!(grown > kept * 3 / 2, "{grown} against {kept}");
}

#[test]
fn a_thinner_level_shows_inside_the_one_it_replaces_within_the_band() {
    let mut harness = harness(false);
    let mut ops = stage(false);
    let mut thin = post();
    let MeshPayloadSource::Inline { positions, .. } = &mut thin.source else {
        unreachable!()
    };
    for point in positions.as_chunks_mut::<3>().0 {
        point[0] *= 0.5;
        point[2] *= 0.5;
    }
    ops.extend(coloured_mesh(RED_POST, thin, [0.9, 0.05, 0.05, 1.0]));
    let mut created = patch(20, 5, whole(), ScatterFade::default(), true);
    if let RenderDiff::CreateScatterPatch { patch, .. } = &mut created {
        patch.lods = vec![ScatterLod {
            asset: format!("mesh/{RED_POST}"),
            material_overrides: Vec::new(),
            start: 5.0,
            impostor: false,
        }];
        patch.lod_band = 2.0;
    }
    ops.push(created);
    harness.apply(ops);
    let (_, frame) = render(&mut harness);
    if !selection_counts(&mut harness).enabled {
        return;
    }
    let reddish = frame
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|pixel| pixel[0] > 120 && pixel[1] + 40 < pixel[0])
        .count();
    assert!(reddish > 100, "{reddish} pixels show the incoming level");
}

#[test]
fn patches_of_one_mesh_share_one_draw_and_draw_what_one_patch_would() {
    let scene = |split: bool| {
        let mut harness = harness(false);
        let mut ops = stage(false);
        if split {
            for (index, at) in SPOTS.iter().enumerate() {
                ops.push(patch(
                    20 + index as u64,
                    5,
                    vec![copy(*at, [1.0; 3])],
                    ScatterFade::default(),
                    false,
                ));
            }
        } else {
            ops.push(patch(20, 5, whole(), ScatterFade::default(), false));
        }
        harness.apply(ops);
        let selection = selection_counts(&mut harness);
        (render(&mut harness).1, selection)
    };
    let (whole_frame, _) = scene(false);
    let (split_frame, selection) = scene(true);
    assert_eq!(
        (
            selection.runs,
            selection.parts,
            selection.draws,
            selection.drawn
        ),
        (3, 3, 1, 3)
    );
    assert_eq!(split_frame, whole_frame);
}

#[test]
fn shadows_take_copies_only_within_the_cascades_reach() {
    let shadowed = |range: Option<f32>| {
        let mut harness = harness(true);
        let mut ops = stage(true);
        for op in &mut ops {
            if let RenderDiff::CreateLight {
                light: LightDescriptor::Directional { range: reach, .. },
                ..
            } = op
            {
                *reach = range;
            }
        }
        ops.push(patch(20, 5, whole(), ScatterFade::default(), true));
        harness.apply(ops);
        selection_counts(&mut harness).shadow
    };
    // The copies stand about 4 m off.
    assert_eq!(shadowed(None), SPOTS.len() as u32);
    assert_eq!(shadowed(Some(2.0)), 0);
}

/// A tree: a 0.2 m trunk to 1 m under a crown 1.6 m across.
fn tree() -> MeshPayloadDescriptor {
    let block = |half: [f32; 3], low: f32| {
        let mut payload = cube();
        let MeshPayloadSource::Inline { positions, .. } = &mut payload.source else {
            unreachable!()
        };
        // The unit cube, to `half` either side and from `low` up.
        for point in positions.as_chunks_mut::<3>().0 {
            point[0] *= 2.0 * half[0];
            point[1] = (point[1] + 0.5) * 2.0 * half[1] + low;
            point[2] *= 2.0 * half[2];
        }
        payload
    };
    let (trunk, crown) = (block([0.1, 0.5, 0.1], 0.0), block([0.8, 0.2, 0.8], 1.0));
    let unpack = |payload: MeshPayloadDescriptor| {
        let MeshPayloadSource::Inline {
            positions,
            normals,
            indices,
            ..
        } = payload.source
        else {
            unreachable!()
        };
        (positions, normals, indices)
    };
    let (mut positions, mut normals, mut indices) = unpack(trunk);
    let (crown_positions, crown_normals, crown_indices) = unpack(crown);
    let base = (positions.len() / 3) as u32;
    positions.extend(crown_positions);
    normals.extend(crown_normals);
    indices.extend(crown_indices.iter().map(|index| index + base));
    payload(positions, normals, indices)
}

/// The light a tree stands under.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Under {
    /// The sun, high: its cascades.
    Sun,
    /// An ambient light's sky, which casts through one fixed layer looking
    /// down.
    Sky,
    /// A spot light above, with a caster moving beside the tree, so its
    /// layer redraws the moving casters over its cached static ones.
    Spot,
}

#[test]
fn a_copy_shades_and_shadows_as_an_instance_standing_there_would() {
    // Turned and scaled, under each kind of shadow layer, near and in a far
    // cascade, opaque, masked and in the wind: its crown shades its trunk
    // and the ground as the same tree placed as an instance does.
    const TREE: &str = "tree";
    const YAW: f32 = 35.0;
    const FAR: u64 = 6;
    const MOVER: u64 = 8;
    let scene =
        |scattered: bool, away: f32, scale: f32, masked: bool, windy: bool, under: Under| {
            let mut harness = harness(true);
            let mut ops = stage(true);
            for op in &mut ops {
                if let RenderDiff::CreateLight {
                    light:
                        LightDescriptor::Directional {
                            direction,
                            shadow_intent,
                            ..
                        },
                    ..
                } = op
                {
                    *direction = [0.15, -1.0, 0.1];
                    if under != Under::Sun {
                        *shadow_intent = LightShadowIntent::Disabled;
                    }
                }
            }
            let at = [0.3 * scale, -1.0, -away];
            match under {
                Under::Sun => {}
                Under::Sky => ops.push(RenderDiff::CreateLight {
                    handle: RenderHandle::new(78),
                    parent: None,
                    light: LightDescriptor::Ambient {
                        color: [1.0; 3],
                        intensity: 1.0,
                        enabled: true,
                        range: Some(8.0),
                        shadow_intent: LightShadowIntent::Requested,
                        shadow: Default::default(),
                    },
                }),
                Under::Spot => {
                    ops.push(RenderDiff::CreateLight {
                        handle: RenderHandle::new(78),
                        parent: None,
                        light: LightDescriptor::Spot {
                            color: [1.0; 3],
                            intensity: 40.0,
                            enabled: true,
                            position: [at[0] + 0.5, 4.0, at[2] + 0.5],
                            direction: [-0.1, -1.0, -0.1],
                            range: Some(12.0),
                            decay: 2.0,
                            outer_angle_radians: 0.8,
                            penumbra: 0.2,
                            shadow_intent: LightShadowIntent::Requested,
                            shadow: Default::default(),
                        },
                    });
                    ops.push(instance(
                        MOVER,
                        None,
                        POST,
                        transform([at[0] + 1.5, -1.0, at[2] - 1.0], 0.0, 1.0),
                    ));
                }
            }
            let mut tree_ops = coloured_mesh(TREE, tree(), [0.9, 0.9, 0.9, 1.0]);
            if let RenderDiff::DefineMaterial { material } = &mut tree_ops[0] {
                if masked {
                    material.alpha_mode = MaterialAlphaModeDescriptor::Mask { cutoff: 0.5 };
                }
                if windy {
                    material.wind = Some(MaterialWindDescriptor {
                        bend: 0.0,
                        flutter: 0.0,
                    });
                }
            }
            ops.extend(tree_ops);
            ops.push(RenderDiff::Create {
                handle: RenderHandle::new(FAR),
                parent: None,
                node: RenderNode {
                    transform: transform(at, 0.0, 1.0),
                    ..RenderNode::new(Geometry::Group)
                },
            });
            ops.push(instance(
                7,
                Some(FAR),
                "ground",
                transform([0.0; 3], 0.0, scale * 2.0),
            ));
            if scattered {
                let half = YAW.to_radians() * 0.5;
                let mut copy = copy([0.0; 3], [1.0; 3]);
                copy.rotation = [0.0, half.sin(), 0.0, half.cos()];
                copy.scale = scale;
                let mut tree = patch(20, FAR, vec![copy], ScatterFade::default(), true);
                if let RenderDiff::CreateScatterPatch { patch, .. } = &mut tree {
                    patch.asset = format!("mesh/{TREE}");
                }
                ops.push(tree);
            } else {
                ops.push(instance(
                    20,
                    Some(FAR),
                    TREE,
                    transform([0.0; 3], YAW, scale),
                ));
            }
            harness.apply(ops);
            render(&mut harness);
            if under == Under::Spot {
                for step in 1..=2 {
                    harness.apply(vec![RenderDiff::Update {
                        handle: RenderHandle::new(MOVER),
                        transform: Some(transform(
                            [at[0] + 1.5, -1.0 + 0.01 * step as f32, at[2] - 1.0],
                            0.0,
                            1.0,
                        )),
                        material: None,
                        visible: None,
                        metadata: None,
                    }]);
                    render(&mut harness);
                }
            }
            render(&mut harness).1
        };
    for (away, scale, masked, windy, under) in [
        (4.5, 1.2, false, false, Under::Sun),
        (80.0, 10.0, false, false, Under::Sun),
        (4.5, 1.2, true, true, Under::Sun),
        (4.5, 1.2, true, true, Under::Sky),
        (4.5, 1.2, false, false, Under::Spot),
    ] {
        let expected = scene(false, away, scale, masked, windy, under);
        let scattered = scene(true, away, scale, masked, windy, under);
        let differing = expected
            .chunks(4)
            .zip(scattered.chunks(4))
            .filter(|(a, b)| a.iter().zip(*b).any(|(x, y)| x.abs_diff(*y) > 2))
            .count();
        assert_eq!(
            differing, 0,
            "{under:?}, {away} m away, masked {masked}, windy {windy}: {differing} pixels differ from the instance"
        );
    }
}

#[test]
fn each_run_draws_its_own_mesh_whatever_order_its_parts_draw_in() {
    // The square's run is made first (its batch key sorts first) but has
    // the higher handle (it is gathered second): each run still draws its
    // own mesh's indices, as instances standing there would.
    const SQUARE: &str = "square";
    let scene = |scattered: bool| {
        let mut harness = harness(false);
        let mut ops = stage(false);
        ops.extend(coloured_mesh(SQUARE, floor(0.8), [0.9, 0.05, 0.05, 1.0]));
        if scattered {
            let mut square = patch(
                30,
                5,
                vec![copy([-1.0, 0.0, 0.5], [1.0; 3])],
                ScatterFade::default(),
                true,
            );
            if let RenderDiff::CreateScatterPatch { patch, .. } = &mut square {
                patch.asset = format!("mesh/{SQUARE}");
            }
            ops.push(square);
            ops.push(patch(
                20,
                5,
                vec![copy([1.0, 0.0, 0.0], [1.0; 3])],
                ScatterFade::default(),
                true,
            ));
        } else {
            ops.push(instance(
                30,
                Some(5),
                SQUARE,
                transform([-1.0, 0.0, 0.5], 0.0, 1.0),
            ));
            ops.push(instance(
                20,
                Some(5),
                POST,
                transform([1.0, 0.0, 0.0], 0.0, 1.0),
            ));
        }
        harness.apply(ops);
        render(&mut harness).1
    };
    let expected = scene(false);
    assert_eq!(scene(true), expected);
}

#[test]
fn an_impostor_level_draws_the_posts_baked_views_as_cards() {
    // The posts as cards from 2 m (every copy): about where and as large as
    // the posts themselves, lit by their baked normals.
    let draw = |impostor_from: Option<f32>| {
        let mut harness = harness(false);
        let mut ops = stage(false);
        let mut created = patch(20, 5, whole(), ScatterFade::default(), true);
        if let (RenderDiff::CreateScatterPatch { patch, .. }, Some(start)) =
            (&mut created, impostor_from)
        {
            patch.lods = vec![ScatterLod {
                asset: format!("mesh/{POST}"),
                material_overrides: Vec::new(),
                start,
                impostor: true,
            }];
        }
        ops.push(created);
        harness.apply(ops);
        let (_, frame) = render(&mut harness);
        let readout = harness.renderer.gpu_readout();
        (frame, readout)
    };
    let (posts, _) = draw(None);
    let (cards, readout) = draw(Some(2.0));
    if !readout.scatter_selection.enabled {
        return;
    }
    assert_eq!(readout.impostors.impostors, 1);
    let (real, carded) = (bright(&posts), bright(&cards));
    assert!(carded > 0, "the cards draw");
    assert!(
        carded.abs_diff(real) < real / 3,
        "{carded} card pixels against {real} post pixels"
    );
    // Each card stands where its post does: the bright pixels' columns agree.
    let columns = |frame: &[u8]| -> Vec<bool> {
        (0..WIDTH)
            .map(|x| (0..HEIGHT).any(|y| pixel(frame, WIDTH, x, y)[1] > 150))
            .collect()
    };
    let (post_columns, card_columns) = (columns(&posts), columns(&cards));
    let agree = post_columns
        .iter()
        .zip(&card_columns)
        .filter(|(a, b)| a == b)
        .count();
    assert!(
        agree as u32 > WIDTH * 9 / 10,
        "{agree} of {WIDTH} columns agree"
    );
}
