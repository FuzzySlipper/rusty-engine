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
        (positions.into_vec(), normals.into_vec(), indices.into_vec())
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

/// An opaque wall 5 m wide and 2 m tall across the view, 3 m off.
fn wall() -> MeshPayloadDescriptor {
    let mut payload = cube();
    let MeshPayloadSource::Inline { positions, .. } = &mut payload.source else {
        unreachable!()
    };
    for point in positions.as_chunks_mut::<3>().0 {
        point[0] *= 5.0;
        point[1] *= 2.0;
        point[2] *= 0.2;
    }
    payload.bounds = MeshBoundsDescriptor {
        min: [-2.5, -1.0, -0.1],
        max: [2.5, 1.0, 0.1],
    };
    payload
}

/// The frame and the selection's counts of a scene with occlusion culling
/// as asked, through the view's own depth or the ambient occlusion
/// pre-pass's.
fn occluded_scene(
    occlusion_culling: bool,
    ambient_occlusion: bool,
    extra: Vec<RenderDiff>,
) -> (
    Vec<u8>,
    render_wgpu::ScatterSelectionReadout,
    render_wgpu::OcclusionReadout,
) {
    let mut ambient = render_wgpu::AmbientOcclusion::default();
    if ambient_occlusion {
        ambient.path = render_wgpu::AmbientOcclusionPath::Raster;
        ambient.strength = 1.0;
    }
    let mut harness = Harness::new(RendererOptions {
        default_world_lights: false,
        occlusion_culling,
        ambient_occlusion: ambient,
        ..RendererOptions::default()
    });
    let mut ops = stage(false);
    ops.extend(extra);
    harness.apply(ops);
    let selection = selection_counts(&mut harness);
    let frame = render(&mut harness).1;
    (frame, selection, harness.renderer.gpu_readout().occlusion)
}

#[test]
fn copies_wholly_behind_an_opaque_wall_are_dropped_and_the_picture_stays() {
    // Three posts 2 m behind a wall 3 m off: hidden, bounds and all. One
    // beside the wall and one standing over its top edge: seen.
    let mut ops = coloured_mesh("wall", wall(), [0.4, 0.4, 0.45, 1.0]);
    ops.push(instance(
        30,
        None,
        "wall",
        transform([0.0, 0.0, -3.0], 0.0, 1.0),
    ));
    let mut copies: Vec<_> = [-0.6, 0.0, 0.6]
        .into_iter()
        .map(|x| copy([x, 0.0, -2.0], [1.0; 3]))
        .collect();
    copies.push(copy([4.0, 0.0, -2.0], [1.0; 3]));
    let mut tall = copy([0.5, 1.75, -2.0], [1.0; 3]);
    tall.tint = [1.0, 0.2, 0.2];
    copies.push(tall);
    ops.push(patch(20, 5, copies, ScatterFade::default(), false));
    for ambient_occlusion in [false, true] {
        let (expected, _, _) = occluded_scene(false, ambient_occlusion, ops.clone());
        let (frame, selection, occlusion) = occluded_scene(true, ambient_occlusion, ops.clone());
        assert!(occlusion.enabled);
        assert_eq!(
            (selection.drawn, selection.occluded),
            (2, 3),
            "AO {ambient_occlusion}: {selection:?}"
        );
        assert_eq!(
            frame, expected,
            "AO {ambient_occlusion}: the picture changed"
        );
    }
}

#[test]
fn copies_never_hide_copies() {
    // A post ten times as large stands in front of three small ones: the
    // copies are not occluders, so all four draw.
    let mut big = copy([0.0, -0.5, 0.5], [1.0; 3]);
    big.scale = 10.0;
    let mut copies: Vec<_> = [-0.3, 0.0, 0.3]
        .into_iter()
        .map(|x| copy([x, 0.0, -3.0], [1.0; 3]))
        .collect();
    copies.push(big);
    let (_, selection, occlusion) = occluded_scene(
        true,
        false,
        vec![patch(20, 5, copies, ScatterFade::default(), false)],
    );
    assert!(occlusion.enabled);
    assert_eq!(
        (selection.drawn, selection.occluded),
        (4, 0),
        "{selection:?}"
    );
}

#[test]
fn a_patch_destroyed_before_its_first_draw_takes_its_pending_rows_with_it() {
    // More copies than the copy buffer first holds, made and destroyed
    // between draws: nothing of them is written, and the next patch draws.
    let mut harness = harness(false);
    let mut ops = stage(false);
    let many: Vec<_> = (0..5_000)
        .map(|index| {
            copy(
                [
                    (index % 100) as f32 * 0.05,
                    0.0,
                    -(index / 100) as f32 * 0.05,
                ],
                [1.0; 3],
            )
        })
        .collect();
    ops.push(patch(20, 5, many, ScatterFade::default(), false));
    ops.push(RenderDiff::Destroy {
        handle: RenderHandle::new(20),
    });
    harness.apply(ops);
    render(&mut harness);
    harness.apply(vec![patch(21, 5, whole(), ScatterFade::default(), false)]);
    let (_, frame) = render(&mut harness);
    assert!(bright(&frame) > 100, "the next patch draws");
}

#[test]
fn a_turned_copy_maps_its_triplanar_normals_as_an_instance_does() {
    // A tilted normal map through triplanar planes, under a raking sun: the
    // copy's own turn takes the mapped normal into the world.
    const YAW: f32 = 60.0;
    let scene = |scattered: bool| {
        let mut harness = harness(false);
        let mut ops = stage(false);
        let texel = [
            ((0.6_f32 * 0.5 + 0.5) * 255.0).round() as u8,
            128,
            ((0.8_f32 * 0.5 + 0.5) * 255.0).round() as u8,
            255,
        ];
        let (mut define_map, _) = harness.resources.texture(
            "texture/tilted",
            2,
            2,
            &texel.repeat(4),
            TextureFilter::Linear,
        );
        if let RenderDiff::DefineTexture { texture } = &mut define_map {
            texture.wrap = TextureWrap::Repeat;
            if let Some(payload) = texture.payload.as_mut() {
                payload.color_space = TextureColorSpace::Linear;
            }
        }
        let mut mapped = coloured_mesh("mapped", post(), [0.9, 0.9, 0.9, 1.0]);
        if let RenderDiff::DefineMaterial { material } = &mut mapped[0] {
            material.triplanar = Some(MaterialTriplanarDescriptor { sharpness: 4.0 });
            material.normal_map = Some(MaterialNormalMapDescriptor {
                texture: "texture/tilted".to_owned(),
                scale: 1.0,
            });
        }
        ops.push(define_map);
        ops.extend(mapped);
        let at = [0.0, 0.0, 0.0];
        if scattered {
            let half = YAW.to_radians() * 0.5;
            let mut turned = copy(at, [1.0; 3]);
            turned.rotation = [0.0, half.sin(), 0.0, half.cos()];
            turned.scale = 1.5;
            let mut created = patch(20, 5, vec![turned], ScatterFade::default(), false);
            if let RenderDiff::CreateScatterPatch { patch, .. } = &mut created {
                patch.asset = "mesh/mapped".to_owned();
            }
            ops.push(created);
        } else {
            ops.push(instance(20, Some(5), "mapped", transform(at, YAW, 1.5)));
        }
        harness.apply(ops);
        render(&mut harness).1
    };
    let expected = scene(false);
    let scattered = scene(true);
    let differing = expected
        .chunks(4)
        .zip(scattered.chunks(4))
        .filter(|(a, b)| a.iter().zip(*b).any(|(x, y)| x.abs_diff(*y) > 2))
        .count();
    assert_eq!(differing, 0, "{differing} pixels differ from the instance");
}

#[test]
fn an_impostor_bakes_again_when_its_material_is_defined_anew() {
    // The posts' material turns blue after the cards were baked white: the
    // cards follow it.
    let mut harness = harness(false);
    let mut ops = stage(false);
    let mut created = patch(20, 5, whole(), ScatterFade::default(), false);
    if let RenderDiff::CreateScatterPatch { patch, .. } = &mut created {
        patch.lods = vec![ScatterLod {
            asset: format!("mesh/{POST}"),
            material_overrides: Vec::new(),
            start: 2.0,
            impostor: true,
        }];
    }
    ops.push(created);
    harness.apply(ops);
    let blue = |frame: &[u8]| {
        frame
            .chunks(4)
            .filter(|p| p[2] > 120 && u16::from(p[2]) > u16::from(p[0]) + 60)
            .count()
    };
    let (_, white) = render(&mut harness);
    if !harness.renderer.gpu_readout().scatter_selection.enabled {
        return;
    }
    assert_eq!(blue(&white), 0);
    let recoloured = coloured_mesh(POST, post(), [0.1, 0.2, 0.9, 1.0]);
    harness.apply(vec![recoloured[0].clone()]);
    let (_, frame) = render(&mut harness);
    assert!(blue(&frame) > 100, "{} blue card pixels", blue(&frame));
    assert_eq!(harness.renderer.gpu_readout().impostors.impostors, 1);
}

#[test]
fn copies_shadows_move_with_the_wind_as_an_instances_do() {
    // A tree bending in a gusting wind under the sun, the camera and light
    // still: as time moves, its shadow moves as the same tree placed as an
    // instance moves its own.
    let scene = |scattered: bool| {
        let mut harness = harness(true);
        let mut ops = stage(true);
        for op in &mut ops {
            if let RenderDiff::CreateLight {
                light: LightDescriptor::Directional { direction, .. },
                ..
            } = op
            {
                *direction = [0.5, -0.8, 0.1];
            }
        }
        let mut tree_ops = coloured_mesh("windy", tree(), [0.9, 0.9, 0.9, 1.0]);
        if let RenderDiff::DefineMaterial { material } = &mut tree_ops[0] {
            material.wind = Some(MaterialWindDescriptor {
                bend: 0.3,
                flutter: 0.0,
            });
        }
        ops.extend(tree_ops);
        ops.push(RenderDiff::SetWind {
            wind: Some(WindDescriptor {
                direction: [1.0, 0.0],
                strength: 3.0,
                gust: 1.0,
            }),
        });
        let at = [0.0, 0.0, 0.0];
        if scattered {
            let mut created = patch(
                20,
                5,
                vec![copy(at, [1.0; 3])],
                ScatterFade::default(),
                true,
            );
            if let RenderDiff::CreateScatterPatch { patch, .. } = &mut created {
                patch.asset = "mesh/windy".to_owned();
            }
            ops.push(created);
        } else {
            ops.push(instance(20, Some(5), "windy", transform(at, 0.0, 1.0)));
        }
        harness.apply(ops);
        harness.renderer.set_animation_time(0.0);
        render(&mut harness);
        render(&mut harness);
        harness.renderer.set_animation_time(5.3);
        render(&mut harness).1
    };
    let expected = scene(false);
    let scattered = scene(true);
    let differing = expected
        .chunks(4)
        .zip(scattered.chunks(4))
        .filter(|(a, b)| a.iter().zip(*b).any(|(x, y)| x.abs_diff(*y) > 2))
        .count();
    assert_eq!(differing, 0, "{differing} pixels differ from the instance");
}

#[test]
fn copies_whose_caster_stage_reads_time_cast_as_an_instance_does_as_time_moves() {
    // A tree whose product caster stage casts only in the first half of
    // each second, under the sun, the camera and light still and nothing
    // else timed: at 10.6 its shadow is gone, for the copy as for the same
    // tree placed as an instance.
    const BLINKING: &str = "#import rusty::types::{Surface, Caster}
#import rusty::view::frame
#import rusty::shade::standard_shade

fn shade(surface: Surface) -> vec4<f32> {
    return standard_shade(surface);
}

fn cast_shadow(caster: Caster) {
    if fract(frame.time.x) > 0.5 {
        discard;
    }
}
";
    let scene = |scattered: bool| {
        let mut harness = harness(true);
        let mut ops = stage(true);
        ops.push(RenderDiff::DefineShader {
            shader: ShaderDescriptor {
                id: "shader/blinking".into(),
                path: "shaders/blinking.wgsl".into(),
                keywords: Vec::new(),
                source: BLINKING.into(),
            },
        });
        let mut tree_ops = coloured_mesh("blinking", tree(), [0.9, 0.9, 0.9, 1.0]);
        if let RenderDiff::DefineMaterial { material } = &mut tree_ops[0] {
            material.shader = Some(MaterialShaderDescriptor {
                shader: "shader/blinking".into(),
                parameters: [[0.0; 4]; 4],
                textures: [None, None],
                displacement_bound: 0.0,
            });
        }
        ops.extend(tree_ops);
        let at = [0.0, 0.0, 0.0];
        if scattered {
            let mut created = patch(
                20,
                5,
                vec![copy(at, [1.0; 3])],
                ScatterFade::default(),
                true,
            );
            if let RenderDiff::CreateScatterPatch { patch, .. } = &mut created {
                patch.asset = "mesh/blinking".to_owned();
            }
            ops.push(created);
        } else {
            ops.push(instance(20, Some(5), "blinking", transform(at, 0.0, 1.0)));
        }
        harness.apply(ops);
        let mut at = |time: f64| {
            harness.renderer.set_animation_time(time);
            render(&mut harness).1
        };
        at(10.0);
        let cast = at(10.0);
        let gone = at(10.6);
        let held = at(10.6);
        (cast, gone, held)
    };
    let (cast, gone, held) = scene(false);
    assert!(cast != gone, "the instance's shadow follows time");
    assert!(held == gone);
    let (copy_cast, copy_gone, copy_held) = scene(true);
    for (name, expected, scattered) in [
        ("cast", &cast, &copy_cast),
        ("gone", &gone, &copy_gone),
        ("held", &held, &copy_held),
    ] {
        let differing = expected
            .chunks(4)
            .zip(scattered.chunks(4))
            .filter(|(a, b)| a.iter().zip(*b).any(|(x, y)| x.abs_diff(*y) > 2))
            .count();
        assert_eq!(
            differing, 0,
            "{name}: {differing} pixels differ from the instance"
        );
    }
}

#[test]
fn a_fixed_lights_shadow_follows_the_copies_level_as_the_eye_moves() {
    // Under an ambient light's sky, a copy draws as a wide slab from 7 m and
    // as a post nearer. The eye walks in across 7 m with nothing else
    // moving: the sky's shadow follows the copy's level, as a renderer that
    // starts at the near eye draws it.
    const SLAB: &str = "slab";
    let near = camera("eye", [0.0, 0.0, 0.0], 0.0, 0.0);
    let far = camera("eye", [0.0, 0.0, 5.0], 0.0, 0.0);
    let scene = |eyes: &[&render_host_contracts::RendererCompositionCamera]| {
        let mut harness = harness(true);
        let mut ops = stage(true);
        for op in &mut ops {
            if let RenderDiff::CreateLight {
                light: LightDescriptor::Directional { shadow_intent, .. },
                ..
            } = op
            {
                *shadow_intent = LightShadowIntent::Disabled;
            }
        }
        ops.push(RenderDiff::CreateLight {
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
        });
        let mut slab = cube();
        if let MeshPayloadSource::Inline { positions, .. } = &mut slab.source {
            for point in positions.as_chunks_mut::<3>().0 {
                point[0] *= 2.5;
                point[1] = point[1] * 0.2 + 1.2;
                point[2] *= 2.5;
            }
        }
        slab.bounds = MeshBoundsDescriptor {
            min: [-1.25, 1.1, -1.25],
            max: [1.25, 1.3, 1.25],
        };
        ops.extend(coloured_mesh(SLAB, slab, [0.9, 0.9, 0.9, 1.0]));
        let mut created = patch(
            20,
            5,
            vec![copy([0.0; 3], [1.0; 3])],
            ScatterFade::default(),
            true,
        );
        if let RenderDiff::CreateScatterPatch { patch, .. } = &mut created {
            patch.lods = vec![ScatterLod {
                asset: format!("mesh/{SLAB}"),
                material_overrides: Vec::new(),
                start: 7.0,
                impostor: false,
            }];
        }
        ops.push(created);
        harness.apply(ops);
        let mut frame = Vec::new();
        for eye in eyes {
            for _ in 0..2 {
                harness.renderer.render_offscreen(eye, &harness.target);
            }
            frame = harness.target.read_rgba(&harness.gpu);
        }
        frame
    };
    let expected = scene(&[&near]);
    let walked = scene(&[&far, &near]);
    let differing = expected
        .chunks(4)
        .zip(walked.chunks(4))
        .filter(|(a, b)| a.iter().zip(*b).any(|(x, y)| x.abs_diff(*y) > 2))
        .count();
    assert_eq!(
        differing, 0,
        "{differing} pixels differ from a renderer that started near"
    );
}

/// Two wall halves 3 m off with a vertical slit `gap` metres wide between
/// them, centred on x = `at`.
fn slit_wall(gap: f32, at: f32) -> Vec<RenderDiff> {
    let half = |x: f32| {
        let mut payload = cube();
        if let MeshPayloadSource::Inline { positions, .. } = &mut payload.source {
            for point in positions.as_chunks_mut::<3>().0 {
                point[0] *= 3.0;
                point[1] *= 2.0;
                point[2] *= 0.2;
            }
        }
        payload.bounds = MeshBoundsDescriptor {
            min: [-1.5, -1.0, -0.1],
            max: [1.5, 1.0, 0.1],
        };
        (payload, x)
    };
    let mut ops = Vec::new();
    for (index, (payload, x)) in [half(at - gap / 2.0 - 1.5), half(at + gap / 2.0 + 1.5)]
        .into_iter()
        .enumerate()
    {
        let name = format!("wall-{index}");
        ops.extend(coloured_mesh(&name, payload, [0.4, 0.4, 0.45, 1.0]));
        ops.push(instance(
            30 + index as u64,
            None,
            &name,
            transform([x, 0.0, -3.0], 0.0, 1.0),
        ));
    }
    ops
}

#[test]
fn copies_seen_through_an_opening_narrower_than_a_pixel_stay() {
    // A post behind a slit a fifth of a pixel wide, the eye stepping
    // sideways so the slit crosses the pixels' samples, with and without
    // ambient occlusion: every frame is the one without occlusion culling.
    let eyes: Vec<_> = (0..6)
        .map(|step| camera("eye", [f64::from(step) * 0.004, 0.0, 0.0], 0.0, 0.0))
        .collect();
    let frames = |culling: bool, ambient_occlusion: bool| {
        let mut ambient = render_wgpu::AmbientOcclusion::default();
        if ambient_occlusion {
            ambient.path = render_wgpu::AmbientOcclusionPath::Raster;
            ambient.strength = 1.0;
        }
        let mut harness = Harness::new(RendererOptions {
            default_world_lights: false,
            occlusion_culling: culling,
            ambient_occlusion: ambient,
            ..RendererOptions::default()
        });
        let mut ops = stage(false);
        ops.extend(slit_wall(0.004, 0.0));
        let post = copy([0.0, 0.0, -2.0], [1.0, 0.2, 0.2]);
        ops.push(patch(20, 5, vec![post], ScatterFade::default(), false));
        harness.apply(ops);
        eyes.iter()
            .map(|eye| {
                for _ in 0..2 {
                    harness.renderer.render_offscreen(eye, &harness.target);
                }
                harness.target.read_rgba(&harness.gpu)
            })
            .collect::<Vec<_>>()
    };
    for ambient_occlusion in [false, true] {
        let expected = frames(false, ambient_occlusion);
        let culled = frames(true, ambient_occlusion);
        let seen = expected
            .iter()
            .filter(|frame| {
                frame
                    .chunks(4)
                    .any(|p| u16::from(p[0]) > u16::from(p[1]) + 15)
            })
            .count();
        assert!(seen > 0, "the post shows through the slit");
        for (step, (a, b)) in expected.iter().zip(&culled).enumerate() {
            let differing = a
                .chunks(4)
                .zip(b.chunks(4))
                .filter(|(a, b)| a.iter().zip(*b).any(|(x, y)| x.abs_diff(*y) > 2))
                .count();
            assert_eq!(
                differing, 0,
                "AO {ambient_occlusion}, step {step}: {differing} pixels differ"
            );
        }
    }
}

#[test]
fn a_masked_wall_hides_no_copies() {
    // A masked material keeps the samples its alpha covers, which the world
    // pass may share out differently from a depth pass: it never occludes.
    let mut ops = coloured_mesh("wall", wall(), [0.4, 0.4, 0.45, 1.0]);
    if let RenderDiff::DefineMaterial { material } = &mut ops[0] {
        material.alpha_mode = MaterialAlphaModeDescriptor::Mask { cutoff: 0.5 };
    }
    ops.push(instance(
        30,
        None,
        "wall",
        transform([0.0, 0.0, -3.0], 0.0, 1.0),
    ));
    let copies: Vec<_> = [-0.6, 0.0, 0.6]
        .into_iter()
        .map(|x| copy([x, 0.0, -2.0], [1.0; 3]))
        .collect();
    ops.push(patch(20, 5, copies, ScatterFade::default(), false));
    let (expected, _, _) = occluded_scene(false, false, ops.clone());
    let (frame, selection, occlusion) = occluded_scene(true, false, ops);
    assert!(occlusion.enabled);
    assert_eq!(selection.occluded, 0, "{selection:?}");
    assert_eq!(frame, expected);
}

#[test]
fn a_wall_whose_product_shader_discards_hides_no_copies() {
    // A product shader that discards a strip down the wall's middle in its
    // shade stage, with no caster stage: the posts behind show through, so
    // the wall never occludes.
    let mut ops = vec![RenderDiff::DefineShader {
        shader: ShaderDescriptor {
            id: "shader/slit".into(),
            path: "shaders/slit.wgsl".into(),
            keywords: Vec::new(),
            source: "#import rusty::types::Surface
#import rusty::finish::finish

fn shade(surface: Surface) -> vec4<f32> {
    if abs(surface.world_position.x) < 0.9 {
        discard;
    }
    return finish(surface.base, surface.world_position);
}
"
            .into(),
        },
    }];
    let mut wall_ops = coloured_mesh("wall", wall(), [0.4, 0.4, 0.45, 1.0]);
    if let RenderDiff::DefineMaterial { material } = &mut wall_ops[0] {
        material.shader = Some(MaterialShaderDescriptor {
            shader: "shader/slit".into(),
            parameters: [[0.0; 4]; 4],
            textures: [None, None],
            displacement_bound: 0.0,
        });
    }
    ops.extend(wall_ops);
    ops.push(instance(
        30,
        None,
        "wall",
        transform([0.0, 0.0, -3.0], 0.0, 1.0),
    ));
    let copies: Vec<_> = [-0.6, 0.0, 0.6]
        .into_iter()
        .map(|x| copy([x, 0.0, -2.0], [1.0; 3]))
        .collect();
    ops.push(patch(20, 5, copies, ScatterFade::default(), false));
    let (expected, _, _) = occluded_scene(false, false, ops.clone());
    let (frame, selection, occlusion) = occluded_scene(true, false, ops);
    assert!(occlusion.enabled);
    assert_eq!(selection.occluded, 0, "{selection:?}");
    assert_eq!(frame, expected);
}

#[test]
fn wind_material_copies_in_still_air_leave_the_shadow_maps_alone() {
    // A tree whose material bends in the wind, under the sun, with no wind
    // blowing: as time moves with the camera still, no shadow layer draws
    // again.
    let mut harness = harness(true);
    let mut ops = stage(true);
    let mut tree_ops = coloured_mesh("windy", tree(), [0.9, 0.9, 0.9, 1.0]);
    if let RenderDiff::DefineMaterial { material } = &mut tree_ops[0] {
        material.wind = Some(MaterialWindDescriptor {
            bend: 0.3,
            flutter: 0.1,
        });
    }
    ops.extend(tree_ops);
    let mut created = patch(
        20,
        5,
        vec![copy([0.0; 3], [1.0; 3])],
        ScatterFade::default(),
        true,
    );
    if let RenderDiff::CreateScatterPatch { patch, .. } = &mut created {
        patch.asset = "mesh/windy".to_owned();
    }
    ops.push(created);
    harness.apply(ops);
    for step in 0..3 {
        harness.renderer.set_animation_time(f64::from(step));
        render(&mut harness);
    }
    if !harness.renderer.gpu_readout().scatter_selection.enabled {
        return;
    }
    for step in 3..6 {
        harness.renderer.set_animation_time(f64::from(step));
        let (stats, _) = render(&mut harness);
        assert_eq!(stats.shadow_layers, 0, "step {step}: {stats:?}");
    }
}

#[test]
fn patches_of_two_meshes_with_one_material_draw_in_one_command() {
    // Posts and wide posts share a material: two draws, one multi-draw from
    // the mesh pool, and the picture of the same posts as instances.
    let wide = || {
        let mut payload = post();
        if let MeshPayloadSource::Inline { positions, .. } = &mut payload.source {
            for point in positions.as_chunks_mut::<3>().0 {
                point[0] *= 2.0;
            }
        }
        payload.bounds = MeshBoundsDescriptor {
            min: [-0.3, 0.0, -0.15],
            max: [0.3, 1.0, 0.15],
        };
        payload
    };
    let scene = |scattered: bool| {
        let mut harness = harness(false);
        let mut ops = stage(false);
        // The wide post binds the post's material.
        let mut defined = coloured_mesh("wide", wide(), [0.9, 0.9, 0.9, 1.0]);
        if let RenderDiff::DefineStaticMesh { asset } = &mut defined[1] {
            asset.material_slots[0].material = format!("material/{POST}");
        }
        ops.push(defined.remove(1));
        if scattered {
            ops.push(patch(
                20,
                5,
                vec![copy(SPOTS[0], [1.0; 3])],
                ScatterFade::default(),
                false,
            ));
            let mut wide_patch = patch(
                21,
                5,
                vec![copy(SPOTS[2], [1.0; 3])],
                ScatterFade::default(),
                false,
            );
            if let RenderDiff::CreateScatterPatch { patch, .. } = &mut wide_patch {
                patch.asset = "mesh/wide".to_owned();
            }
            ops.push(wide_patch);
        } else {
            ops.push(instance(20, Some(5), POST, transform(SPOTS[0], 0.0, 1.0)));
            ops.push(instance(21, Some(5), "wide", transform(SPOTS[2], 0.0, 1.0)));
        }
        harness.apply(ops);
        let selection = selection_counts(&mut harness);
        (render(&mut harness).1, selection)
    };
    let (expected, _) = scene(false);
    let (frame, selection) = scene(true);
    if !selection.enabled {
        return;
    }
    assert_eq!(
        (selection.draws, selection.commands),
        (2, 1),
        "{selection:?}"
    );
    assert!(selection.pool_bytes > 0);
    assert_eq!(frame, expected);
}

#[test]
fn a_mesh_defined_anew_draws_anew_from_the_pool() {
    // The posts' mesh is defined again twice as wide after the copies drew:
    // they draw the new mesh, as a scene that had it from the start does.
    let wide = || {
        let mut payload = post();
        if let MeshPayloadSource::Inline { positions, .. } = &mut payload.source {
            for point in positions.as_chunks_mut::<3>().0 {
                point[0] *= 2.0;
            }
        }
        payload.bounds = MeshBoundsDescriptor {
            min: [-0.3, 0.0, -0.15],
            max: [0.3, 1.0, 0.15],
        };
        payload
    };
    let redefine = || coloured_mesh(POST, wide(), [0.9, 0.9, 0.9, 1.0]).remove(1);
    let mut changed = harness(false);
    let mut ops = stage(false);
    ops.push(patch(20, 5, whole(), ScatterFade::default(), false));
    changed.apply(ops);
    render(&mut changed);
    changed.apply(vec![redefine()]);
    render(&mut changed);
    let (_, frame) = render(&mut changed);
    let mut fresh = harness(false);
    let mut ops = stage(false);
    ops.push(redefine());
    ops.push(patch(20, 5, whole(), ScatterFade::default(), false));
    fresh.apply(ops);
    render(&mut fresh);
    let (_, expected) = render(&mut fresh);
    assert_eq!(frame, expected);
}
