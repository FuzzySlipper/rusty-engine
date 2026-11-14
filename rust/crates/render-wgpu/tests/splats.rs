//! Gaussian splats (#10805): splat GLBs admitted by `asset-import` as the
//! runtime admits them and drawn as Gaussians, blended in display space,
//! sorted back to front on the GPU and tested against the opaque world.
//! The GLBs are written here (`support::splat_glb`); no character bytes.

mod support;

use asset_import::{import_animated_glb_asset, ImportContext, SourceUri};
use render_model::*;
use render_wgpu::RendererOptions;
use support::splat_glb::{splat_glb, Gaussian};
use support::*;

/// Admit a splat GLB and hold its runtime resource.
fn admit(harness: &mut Harness, name: &str, bytes: Vec<u8>) -> AnimatedMeshAsset {
    let imported = import_animated_glb_asset(
        &SourceUri::RelativePath(format!("{name}.glb")),
        &bytes,
        &ImportContext::default(),
    );
    let imported = imported
        .assets
        .unwrap_or_else(|| panic!("{name} is admitted: {:?}", imported.diagnostics));
    let hash = imported
        .animated_mesh
        .content_hash
        .clone()
        .expect("content hash");
    harness.resources.insert(
        &format!(
            "animated-mesh-resource/{}",
            hash.trim_start_matches("sha256:")
        ),
        bytes,
    );
    imported.animated_mesh
}

fn splat_instance(handle: u64, asset: &AnimatedMeshAsset, translation: [f32; 3]) -> RenderDiff {
    RenderDiff::CreateAnimatedMeshInstance {
        handle: RenderHandle::new(handle),
        parent: None,
        instance: AnimatedMeshInstanceDescriptor {
            splat: Default::default(),
            playback_fade: None,
            inspection: AnimatedMeshInspection::default(),
            asset: asset.asset.clone(),
            transform: Transform {
                translation,
                ..Transform::IDENTITY
            },
            visible: true,
            material_overrides: Vec::new(),
            playback: None,
            metadata: RenderMetadata {
                source_entity: Some(handle),
                ..RenderMetadata::default()
            },
            layer: RenderLayer::Scene,
            shadow_casting: Default::default(),
        },
    }
}

/// Splats of `gaussians` as instance `handle` at `translation`.
fn splats(
    harness: &mut Harness,
    handle: u64,
    gaussians: &[Gaussian],
    translation: [f32; 3],
) -> Vec<RenderDiff> {
    let asset = admit(
        harness,
        &format!("splats-{handle}"),
        splat_glb(gaussians, [0.0; 3]),
    );
    vec![
        RenderDiff::DefineAnimatedMesh {
            asset: asset.clone(),
        },
        splat_instance(handle, &asset, translation),
    ]
}

/// The camera at the origin looking down −z.
fn front() -> render_host_contracts::RendererCompositionCamera {
    camera([0.0, 0.0, 0.0], 0.0, 0.0)
}

const CENTRE: (u32, u32) = (WIDTH / 2, HEIGHT / 2);

fn at(rgba: &[u8], (x, y): (u32, u32)) -> [i32; 3] {
    let index = ((y * WIDTH + x) * 4) as usize;
    [rgba[index], rgba[index + 1], rgba[index + 2]].map(i32::from)
}

#[track_caller]
fn assert_near(actual: [i32; 3], expected: [i32; 3], tolerance: i32) {
    assert!(
        actual
            .iter()
            .zip(expected)
            .all(|(a, e)| (a - e).abs() <= tolerance),
        "{actual:?} is not within {tolerance} of {expected:?}"
    );
}

/// An opaque Gaussian's centre draws its display colour exactly: the
/// layer holds the sRGB values it blended and the composite decodes them,
/// so the finish encodes the same values again.
#[test]
fn a_gaussian_draws_its_display_colour_at_its_centre() {
    let mut harness = Harness::new(RendererOptions::default());
    let (_, empty) = harness.render(&front());
    let ops = splats(
        &mut harness,
        1,
        &[Gaussian::round([0.0; 3], 0.3, [200, 100, 50, 255])],
        [0.0, 0.0, -3.0],
    );
    harness.apply(ops);
    let (_, rgba) = harness.render(&front());
    assert_near(at(&rgba, CENTRE), [200, 100, 50], 2);
    // Far outside the Gaussian the frame is as it was.
    assert_eq!(at(&rgba, (4, 4)), at(&empty, (4, 4)));
    let readout = harness.renderer.gpu_readout().splats;
    assert_eq!(readout.refused, None);
    assert_eq!(
        (readout.runs, readout.instances, readout.gaussians),
        (0, 0, 0)
    );
    harness.render(&front());
    let readout = harness.renderer.gpu_readout().splats;
    assert_eq!(
        (readout.runs, readout.instances, readout.gaussians),
        (1, 1, 1)
    );
}

/// A half-transparent Gaussian over an opaque one mixes their display
/// colours, as the splats were trained to: blended in linear light the mix
/// of red and blue would encode to about 188 a channel, not 128.
#[test]
fn a_translucent_gaussian_blends_over_another_in_display_space() {
    let mut harness = Harness::new(RendererOptions::default());
    let mut ops = splats(
        &mut harness,
        1,
        &[Gaussian::round([0.0; 3], 0.3, [0, 0, 255, 255])],
        [0.0, 0.0, -4.0],
    );
    ops.extend(splats(
        &mut harness,
        2,
        &[Gaussian::round([0.0; 3], 0.3, [255, 0, 0, 128])],
        [0.0, 0.0, -3.0],
    ));
    harness.apply(ops);
    let (_, rgba) = harness.render(&front());
    // Alpha 128/255 at the centre: red over blue.
    let alpha: f32 = 128.0 / 255.0;
    let expected = [
        (255.0 * alpha).round() as i32,
        0,
        (255.0 * (1.0 - alpha)).round() as i32,
    ];
    assert_near(at(&rgba, CENTRE), expected, 3);
}

/// Of two opaque Gaussians in one set the nearer covers the farther,
/// whichever the GLB wrote first: the sort orders them by depth.
#[test]
fn the_nearer_gaussian_covers_whichever_was_written_first() {
    for (handle, first_near) in [(1, true), (2, false)] {
        let mut harness = Harness::new(RendererOptions::default());
        let near = Gaussian::round([0.0, 0.0, 0.5], 0.2, [0, 255, 0, 255]);
        let far = Gaussian::round([0.0, 0.0, -0.5], 0.4, [255, 0, 255, 255]);
        let gaussians = if first_near { [near, far] } else { [far, near] };
        let ops = splats(&mut harness, handle, &gaussians, [0.0, 0.0, -4.0]);
        harness.apply(ops);
        let (_, rgba) = harness.render(&front());
        assert_near(at(&rgba, CENTRE), [0, 255, 0], 2);
    }
}

/// Two instances' Gaussians in one run interleave by depth: each instance
/// is a sheet of Gaussians, and the sheets cross, so each covers the other
/// on one side.
#[test]
fn instances_in_one_run_interleave_gaussian_by_gaussian() {
    let mut harness = Harness::new(RendererOptions::default());
    // A sheet tilted about y: its left nearer than its right, and one tilted
    // the other way.
    let sheet = |tilt: f32, color: [u8; 4]| -> Vec<Gaussian> {
        (-20..=20)
            .flat_map(|i| {
                (-6..=6).map(move |j| {
                    let x = i as f32 * 0.05;
                    Gaussian::round([x, j as f32 * 0.05, tilt * x], 0.04, color)
                })
            })
            .collect()
    };
    let mut ops = splats(
        &mut harness,
        1,
        &sheet(0.8, [255, 0, 0, 255]),
        [0.0, 0.0, -3.0],
    );
    ops.extend(splats(
        &mut harness,
        2,
        &sheet(-0.8, [0, 0, 255, 255]),
        [0.0, 0.0, -3.0],
    ));
    harness.apply(ops);
    let (_, rgba) = harness.render(&front());
    // The readout is the last frame's.
    harness.render(&front());
    let readout = harness.renderer.gpu_readout().splats;
    assert_eq!((readout.runs, readout.instances), (1, 2), "one run of two");
    // The red sheet stands farther left of centre and nearer right of it;
    // the blue sheet the other way round.
    let (left, right) = ((CENTRE.0 - 40, CENTRE.1), (CENTRE.0 + 40, CENTRE.1));
    assert_near(at(&rgba, left), [0, 0, 255], 3);
    assert_near(at(&rgba, right), [255, 0, 0], 3);
}

/// The layer is tested against the world's opaque depth: a box between the
/// camera and the Gaussians hides them, and a wall behind them does not.
#[test]
fn opaque_geometry_hides_gaussians_behind_it_and_not_those_in_front() {
    let mut harness = Harness::new(RendererOptions::default());
    let unlit = |id: &str, color: [f32; 4]| RenderDiff::DefineMaterial {
        material: RenderMaterialDescriptor {
            unlit: true,
            ..material(id, color, None)
        },
    };
    let mut ops = vec![
        unlit("material/occluder", [0.0, 1.0, 0.0, 1.0]),
        unlit("material/wall", [1.0, 1.0, 1.0, 1.0]),
        static_mesh(
            "mesh/occluder",
            box_mesh([-1.0, -0.6, -0.1], [0.0, 0.6, 0.1], |_| 0),
            "material/occluder",
        ),
        static_mesh(
            "mesh/wall",
            box_mesh([-4.0, -3.0, -0.1], [4.0, 3.0, 0.1], |_| 0),
            "material/wall",
        ),
        instance(
            10,
            None,
            "mesh/occluder",
            transform([0.0, 0.0, -2.0], 0.0, [1.0; 3]),
        ),
        instance(
            11,
            None,
            "mesh/wall",
            transform([0.0, 0.0, -6.0], 0.0, [1.0; 3]),
        ),
    ];
    // A dense row of opaque red Gaussians across the view at z = −4: dense
    // enough that nothing of the wall shows through it, which the composite
    // would blend in linear light (a soft edge's light shows brighter than
    // in display space).
    let row: Vec<Gaussian> = (-40..=40)
        .map(|i| Gaussian::round([i as f32 * 0.025, 0.0, 0.0], 0.1, [255, 0, 0, 255]))
        .collect();
    ops.extend(splats(&mut harness, 1, &row, [0.0, 0.0, -4.0]));
    harness.apply(ops);
    let (_, rgba) = harness.render(&front());
    // Left of centre the box covers the row; right of it the row covers the
    // wall.
    assert_near(at(&rgba, (CENTRE.0 - 20, CENTRE.1)), [0, 255, 0], 2);
    assert_near(at(&rgba, (CENTRE.0 + 20, CENTRE.1)), [255, 0, 0], 2);
    assert_near(
        at(&rgba, (CENTRE.0 + 20, CENTRE.1 - 40)),
        [255, 255, 255],
        2,
    );
}

/// Admit a clip pack and hold it under its runtime identity.
fn admit_pack(harness: &mut Harness, bytes: Vec<u8>) -> AnimatedMeshAsset {
    let imported = import_animated_glb_asset(
        &SourceUri::RelativePath("pack.glb".to_owned()),
        &bytes,
        &ImportContext::default(),
    );
    let pack = imported
        .assets
        .unwrap_or_else(|| panic!("the pack is admitted: {:?}", imported.diagnostics))
        .animated_mesh;
    let hash = pack.content_hash.clone().expect("content hash");
    harness.resources.insert(
        &format!("clip-pack-resource/{}", hash.trim_start_matches("sha256:")),
        bytes,
    );
    pack
}

/// Where the front camera at (0, 1, 6) draws a point on the z = 0 plane.
fn on_plane(x: f32, y: f32) -> (u32, u32) {
    let focal = (HEIGHT as f32 / 2.0) / 30f32.to_radians().tan();
    (
        (WIDTH as f32 / 2.0 + focal * x / 6.0) as u32,
        (HEIGHT as f32 / 2.0 - focal * (y - 1.0) / 6.0) as u32,
    )
}

/// A skinned splat figure takes its pose from a clip pack on a skeleton of
/// other bone lengths, bound by joint names: the clip's rotation turns its
/// arm, the arm keeps the figure's own length (the pack holds its
/// translation still), and the root's translation is fitted.
#[test]
fn a_skinned_splat_figure_takes_its_pose_from_a_clip_pack_bound_by_joint_names() {
    use support::splat_glb::{arm_clip_pack_glb, skinned_splat_glb};
    let mut harness = Harness::new(RendererOptions::default());
    let mut body = admit(
        &mut harness,
        "skinned",
        skinned_splat_glb(
            &[
                Gaussian::round([0.0, 0.5, 0.0], 0.1, [0, 255, 0, 255]),
                Gaussian::round([0.0, 1.5, 0.0], 0.1, [255, 0, 0, 255]),
            ],
            &[0, 1],
        ),
    );
    assert!(body.rig.is_some(), "a skinned splat GLB names its rig");
    let pack = admit_pack(&mut harness, arm_clip_pack_glb());
    let hash = pack.content_hash.clone().expect("content hash");
    body.clip_packs.push(AnimationClipPack {
        asset: format!("animation-clip-pack/{}", hash.trim_start_matches("sha256:")),
        runtime_format: pack.runtime_format,
        content_hash: hash.clone(),
        rig: pack.rig.clone().expect("the pack's rig"),
        clips: pack.clips.clone(),
        provenance: AnimationClipPackProvenance {
            producer: "fixture".to_owned(),
            source_hash: hash,
            target_hash: body.content_hash.clone().expect("content hash"),
            license: "CC0".to_owned(),
        },
        binding: AnimationClipPackBinding::JointNames,
        translation_fits: vec![AnimationClipTranslationFit {
            clip: "raise".to_owned(),
            joint: "root".to_owned(),
            scale: [1.0, 0.5, 1.0],
            offset: [0.0, 0.25, 0.0],
        }],
    });
    let instance = match splat_instance(1, &body, [0.0; 3]) {
        RenderDiff::CreateAnimatedMeshInstance {
            handle,
            parent,
            mut instance,
        } => {
            instance.playback = Some(AnimatedMeshPlaybackCommand::Sample {
                clip: "raise".to_owned(),
                normalized_time: 0.0,
            });
            RenderDiff::CreateAnimatedMeshInstance {
                handle,
                parent,
                instance,
            }
        }
        _ => unreachable!(),
    };
    harness.apply(vec![
        RenderDiff::DefineAnimatedMesh { asset: body },
        instance,
    ]);
    let camera = camera([0.0, 1.0, 6.0], 0.0, 0.0);
    let (_, rgba) = harness.render(&camera);
    // The root fitted up to y = 0.25: its Gaussian at (0, 0.75).
    assert_near(at(&rgba, on_plane(0.0, 0.75)), [0, 255, 0], 3);
    // The arm one metre above the root, turned: its Gaussian at (−0.5, 1.25).
    assert_near(at(&rgba, on_plane(-0.5, 1.25)), [255, 0, 0], 3);
    // Not at the pack's arm length, nor where it stood at rest.
    let background = at(&rgba, (4, 4));
    assert_eq!(at(&rgba, on_plane(-0.5, 2.25)), background);
    assert_eq!(at(&rgba, on_plane(0.0, 1.5)), background);
}

/// A clip pack whose skeleton is wrapped in a non-joint node of its own (a
/// Z-up export's armature, turned +90° about x) binds by joint names to a
/// figure whose top joint is `Hips`, no `root` above it: the wrapper's turn
/// is folded into the top joint's channels, so the clip poses the figure as
/// authored in the Y-up world.
#[test]
fn a_wrapped_clip_pack_binds_to_a_hips_rooted_figure() {
    use support::splat_glb::{hips_splat_glb, wrapped_hips_pack_glb};
    let mut harness = Harness::new(RendererOptions::default());
    let mut body = admit(&mut harness, "hips", hips_splat_glb());
    let rig = body
        .rig
        .as_ref()
        .expect("a skinned splat GLB names its rig");
    assert_eq!(rig.structural_root_ids, ["Hips"]);
    let pack = admit_pack(&mut harness, wrapped_hips_pack_glb());
    let hash = pack.content_hash.clone().expect("content hash");
    body.clip_packs.push(AnimationClipPack {
        asset: format!("animation-clip-pack/{}", hash.trim_start_matches("sha256:")),
        runtime_format: pack.runtime_format,
        content_hash: hash.clone(),
        rig: pack.rig.clone().expect("the pack's rig"),
        clips: pack.clips.clone(),
        provenance: AnimationClipPackProvenance {
            producer: "fixture".to_owned(),
            source_hash: hash,
            target_hash: body.content_hash.clone().expect("content hash"),
            license: "CC0".to_owned(),
        },
        binding: AnimationClipPackBinding::JointNames,
        translation_fits: Vec::new(),
    });
    let instance = match splat_instance(1, &body, [0.0; 3]) {
        RenderDiff::CreateAnimatedMeshInstance {
            handle,
            parent,
            mut instance,
        } => {
            instance.playback = Some(AnimatedMeshPlaybackCommand::Sample {
                clip: "turn".to_owned(),
                normalized_time: 0.0,
            });
            RenderDiff::CreateAnimatedMeshInstance {
                handle,
                parent,
                instance,
            }
        }
        _ => unreachable!(),
    };
    harness.apply(vec![
        RenderDiff::DefineAnimatedMesh { asset: body },
        instance,
    ]);
    let (_, rgba) = harness.render(&camera([0.0, 1.0, 6.0], 0.0, 0.0));
    // Hips where the clip puts it in the Y-up world, at y = 1 (the arm's
    // Gaussian a few pixels off tints it slightly).
    assert_near(at(&rgba, on_plane(0.0, 1.0)), [0, 255, 0], 6);
    // The arm turned 90° about +z, at the figure's own half-metre length.
    assert_near(at(&rgba, on_plane(-0.5, 1.0)), [255, 0, 0], 6);
    let background = at(&rgba, (4, 4));
    assert_eq!(at(&rgba, on_plane(0.0, 1.5)), background);
}

/// The splat multiplier scales a figure's colour in linear light.
#[test]
fn the_splat_multiplier_scales_colour_in_linear_light() {
    let mut harness = Harness::new(RendererOptions::default());
    let ops = splats(
        &mut harness,
        1,
        &[Gaussian::round([0.0; 3], 0.3, [200, 100, 50, 255])],
        [0.0, 0.0, -3.0],
    );
    harness.apply(ops);
    harness.apply(vec![RenderDiff::SetSplatAppearance {
        handle: RenderHandle::new(1),
        appearance: SplatAppearance {
            multiplier: [0.5, 0.5, 0.25],
            ..SplatAppearance::default()
        },
    }]);
    let (_, rgba) = harness.render(&front());
    let scaled = |byte: u8, by: f32| {
        let linear = |c: f32| {
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        let encode = |c: f32| {
            if c <= 0.003_130_8 {
                c * 12.92
            } else {
                1.055 * c.powf(1.0 / 2.4) - 0.055
            }
        };
        (encode(linear(f32::from(byte) / 255.0) * by) * 255.0).round() as i32
    };
    assert_near(
        at(&rgba, CENTRE),
        [scaled(200, 0.5), scaled(100, 0.5), scaled(50, 0.25)],
        2,
    );
}

/// On a pixel grid a figure draws in whole blocks anchored at its origin's
/// projection, each block one palette colour, and what stands in front of
/// it still hides it pixel by pixel.
#[test]
fn a_figure_on_a_pixel_grid_draws_palette_blocks_behind_what_stands_in_front() {
    const SIDE: u32 = 4;
    let mut harness = Harness::new(RendererOptions::default());
    // A slab 1.6 m by 1 m at z = −4, red left of its origin and blue right
    // of it: about 62 by 39 pixels, its origin at the view's centre.
    let slab: Vec<Gaussian> = (-32..=32)
        .flat_map(|i| (-20..=20).map(move |j| (i, j)))
        .map(|(i, j)| {
            let color = if i < 0 {
                [255, 0, 0, 255]
            } else {
                [0, 0, 255, 255]
            };
            Gaussian::round([i as f32 * 0.025, j as f32 * 0.025, 0.0], 0.03, color)
        })
        .collect();
    // A thin white box in front, its right edge at x = −0.22 m two metres away:
    // 17 pixels left of centre, inside a block.
    let mut ops = vec![
        RenderDiff::DefineMaterial {
            material: RenderMaterialDescriptor {
                unlit: true,
                ..material("material/box", [1.0; 4], None)
            },
        },
        static_mesh(
            "mesh/box",
            box_mesh([-2.0, -1.0, -0.01], [-0.22, 1.0, 0.01], |_| 0),
            "material/box",
        ),
        instance(
            10,
            None,
            "mesh/box",
            transform([0.0, 0.0, -2.0], 0.0, [1.0; 3]),
        ),
    ];
    ops.extend(splats(&mut harness, 1, &slab, [0.0, 0.0, -4.0]));
    harness.apply(ops);
    harness.apply(vec![RenderDiff::SetSplatAppearance {
        handle: RenderHandle::new(1),
        appearance: SplatAppearance {
            pixel_grid: SIDE,
            palette: vec![[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            ..SplatAppearance::default()
        },
    }]);
    let (_, rgba) = harness.render(&front());
    let (red, blue, white) = ([255, 0, 0], [0, 0, 255], [255, 255, 255]);
    // Blocks wholly on the figure and clear of the box, two rows of them
    // about the centre: each one palette colour, red left and blue right
    // (the two blocks about the centre mix them).
    let (x0, y0) = (CENTRE.0 - 4 * SIDE, CENTRE.1 - SIDE);
    for block_x in (x0..CENTRE.0 + 6 * SIDE)
        .step_by(SIDE as usize)
        .filter(|x| x.abs_diff(CENTRE.0) > SIDE)
    {
        for block_y in (y0..CENTRE.1 + SIDE).step_by(SIDE as usize) {
            let expected = if block_x < CENTRE.0 { red } else { blue };
            for x in block_x..block_x + SIDE {
                for y in block_y..block_y + SIDE {
                    assert_eq!(
                        at(&rgba, (x, y)),
                        expected,
                        "block ({block_x}, {block_y}) at ({x}, {y})"
                    );
                }
            }
        }
    }
    // The block the box's edge crosses: the box where it stands, the block's
    // colour beside it.
    let edge = x0 - SIDE;
    for y in y0..y0 + SIDE {
        for x in edge..edge + 3 {
            assert_eq!(at(&rgba, (x, y)), white, "box at ({x}, {y})");
        }
        assert_eq!(
            at(&rgba, (edge + 3, y)),
            red,
            "block beside the box at y {y}"
        );
    }
}

/// A flat Gaussian facing the camera at z = −3, lit when `lit` with a
/// light floor of `floor`, under a sun shining along `sun` (and casting
/// through its cascades when the harness draws shadows), with a box at
/// `blocker` when given.
fn lit_disc(
    harness: &mut Harness,
    lit: bool,
    floor: f32,
    sun: [f32; 3],
    blocker: Option<[f32; 3]>,
) {
    let disc = Gaussian {
        scale: [0.3, 0.3, 0.005],
        ..Gaussian::round([0.0; 3], 0.3, [200, 100, 50, 255])
    };
    let mut ops = splats(harness, 1, &[disc], [0.0, 0.0, -3.0]);
    ops.push(RenderDiff::CreateLight {
        handle: RenderHandle::new(20),
        parent: None,
        light: LightDescriptor::Directional {
            color: [1.0; 3],
            intensity: 3.0,
            enabled: true,
            direction: sun,
            range: None,
            shadow_intent: LightShadowIntent::Requested,
            shadow: Default::default(),
        },
    });
    if let Some(at) = blocker {
        ops.push(RenderDiff::DefineMaterial {
            material: material("material/blocker", [0.5, 0.5, 0.5, 1.0], None),
        });
        ops.push(static_mesh(
            "mesh/blocker",
            box_mesh([-0.6, -0.1, -0.6], [0.6, 0.1, 0.6], |_| 0),
            "material/blocker",
        ));
        ops.push(instance(
            30,
            None,
            "mesh/blocker",
            transform(at, 0.0, [1.0; 3]),
        ));
    }
    ops.push(RenderDiff::SetSplatAppearance {
        handle: RenderHandle::new(1),
        appearance: SplatAppearance {
            lighting: if lit {
                SplatLighting::Lit
            } else {
                SplatLighting::Unlit
            },
            light_floor: floor,
            ..SplatAppearance::default()
        },
    });
    harness.apply(ops);
}

fn luminance([r, g, b]: [i32; 3]) -> i32 {
    r + g + b
}

/// Lit, a figure takes the world's light at its own normals: a flat
/// Gaussian is bright with the sun before it and falls to its light floor
/// with the sun behind it, its hue kept; unlit it draws as baked.
#[test]
fn a_lit_splat_takes_the_suns_light_at_its_normal_down_to_its_floor() {
    let options = RendererOptions {
        default_world_lights: false,
        ..RendererOptions::default()
    };
    let render = |lit, floor, sun| {
        let mut harness = Harness::new(options);
        lit_disc(&mut harness, lit, floor, sun, None);
        at(&harness.render(&front()).1, CENTRE)
    };
    let unlit = render(false, 0.0, [0.0, 0.0, -1.0]);
    assert_near(unlit, [200, 100, 50], 2);
    let before = render(true, 0.25, [0.0, 0.0, -1.0]);
    let behind = render(true, 0.25, [0.0, 0.0, 1.0]);
    assert!(
        luminance(before) > luminance(behind) + 150,
        "sun before {before:?}, behind {behind:?}"
    );
    // Behind, a quarter of the baked colour in linear light.
    let quarter = |byte: u8| encode(decode(byte) * 0.25);
    assert_near(behind, [quarter(200), quarter(100), quarter(50)], 3);
    // Lit, red still leads green and green blue.
    assert!(before[0] > before[1] && before[1] > before[2], "{before:?}");

    // Under a temporal stage the view draws motion and the normal takes a
    // target of its own: the light is the same.
    let temporal = |sun| {
        let mut harness = Harness::new(options);
        lit_disc(&mut harness, true, 0.25, sun, None);
        harness.apply(vec![RenderDiff::SetRendererSettings {
            settings: RendererSettingsDescriptor {
                antialiasing: Antialiasing::Temporal,
                render_scale: 1.0,
                ..RendererSettingsDescriptor::DEFAULT
            },
        }]);
        let samples = harness.renderer.samples();
        harness.target = render_wgpu::OffscreenTarget::new(&harness.gpu, WIDTH, HEIGHT, samples);
        let mut rgba = Vec::new();
        for _ in 0..16 {
            rgba = harness.render(&front()).1;
        }
        at(&rgba, CENTRE)
    };
    assert_near(temporal([0.0, 0.0, -1.0]), before, 3);
    assert_near(temporal([0.0, 0.0, 1.0]), behind, 3);

    // On a pixel grid the block is lit once, then snapped: a bright
    // palette colour with the sun before it, a dark one behind.
    let palette = vec![[0.9, 0.4, 0.1], [0.05, 0.02, 0.01]];
    let gridded = |sun| {
        let mut harness = Harness::new(options);
        lit_disc(&mut harness, true, 0.25, sun, None);
        harness.apply(vec![RenderDiff::SetSplatAppearance {
            handle: RenderHandle::new(1),
            appearance: SplatAppearance {
                lighting: SplatLighting::Lit,
                light_floor: 0.25,
                pixel_grid: 4,
                palette: palette.clone(),
                ..SplatAppearance::default()
            },
        }]);
        at(&harness.render(&front()).1, CENTRE)
    };
    let byte = |linear: f32| encode(linear);
    assert_near(
        gridded([0.0, 0.0, -1.0]),
        [byte(0.9), byte(0.4), byte(0.1)],
        1,
    );
    assert_near(
        gridded([0.0, 0.0, 1.0]),
        [byte(0.05), byte(0.02), byte(0.01)],
        1,
    );
}

/// Under a temporal stage a lit run's normals blend in a target of their
/// own as they do beside the depth without one: a translucent Gaussian
/// over an opaque one, their normals tilted apart, lights the same.
#[test]
fn overlapping_lit_gaussians_light_alike_with_and_without_motion() {
    let options = RendererOptions {
        default_world_lights: false,
        ..RendererOptions::default()
    };
    // Flat Gaussians turned alike about y. The fallback normal points away
    // from their centre, so the front one's tilts toward +x and the back
    // one's toward −x: blended by alpha they face the camera, and a blend
    // that lost the front's alpha would tilt them. The sun from the right.
    let turned = |degrees: f32, z: f32, opacity: u8| {
        let (sin, cos) = (degrees.to_radians() * 0.5).sin_cos();
        Gaussian {
            rotation: [0.0, sin, 0.0, cos],
            scale: [0.3, 0.3, 0.005],
            ..Gaussian::round([0.0, 0.0, z], 0.3, [200, 160, 120, opacity])
        }
    };
    let render = |temporal: bool| {
        let mut harness = Harness::new(options);
        let mut ops = splats(
            &mut harness,
            1,
            &[turned(50.0, -0.2, 255), turned(50.0, 0.2, 128)],
            [0.0, 0.0, -3.0],
        );
        ops.push(RenderDiff::CreateLight {
            handle: RenderHandle::new(20),
            parent: None,
            light: LightDescriptor::Directional {
                color: [1.0; 3],
                intensity: 3.0,
                enabled: true,
                direction: [-1.0, 0.0, -0.4],
                range: None,
                shadow_intent: LightShadowIntent::Disabled,
                shadow: Default::default(),
            },
        });
        ops.push(RenderDiff::SetSplatAppearance {
            handle: RenderHandle::new(1),
            appearance: SplatAppearance {
                lighting: SplatLighting::Lit,
                light_floor: 0.1,
                ..SplatAppearance::default()
            },
        });
        if temporal {
            ops.push(RenderDiff::SetRendererSettings {
                settings: RendererSettingsDescriptor {
                    antialiasing: Antialiasing::Temporal,
                    render_scale: 1.0,
                    ..RendererSettingsDescriptor::DEFAULT
                },
            });
        }
        harness.apply(ops);
        let samples = harness.renderer.samples();
        harness.target = render_wgpu::OffscreenTarget::new(&harness.gpu, WIDTH, HEIGHT, samples);
        let mut rgba = Vec::new();
        for _ in 0..16 {
            rgba = harness.render(&front()).1;
        }
        at(&rgba, CENTRE)
    };
    let still = render(false);
    let temporal = render(true);
    assert_near(temporal, still, 3);
}

/// A lit figure receives the sun's cascaded shadow: a box between the sun
/// and it darkens it to its floor.
#[test]
fn a_lit_splat_receives_a_cascaded_shadow() {
    let options = RendererOptions {
        default_world_lights: false,
        shadows: true,
        ..RendererOptions::default()
    };
    // The sun from above and in front; the box half way along its light.
    let sun = [0.0, -1.0, -1.0];
    let render = |blocker| {
        let mut harness = Harness::new(options);
        lit_disc(&mut harness, true, 0.1, sun, blocker);
        at(&harness.render(&front()).1, CENTRE)
    };
    let open = render(None);
    let shadowed = render(Some([0.0, 1.5, -1.5]));
    assert!(
        luminance(open) > luminance(shadowed) + 150,
        "open {open:?}, shadowed {shadowed:?}"
    );
    let tenth = |byte: u8| encode(decode(byte) * 0.1);
    assert_near(shadowed, [tenth(200), tenth(100), tenth(50)], 3);
}

/// A splat instance casts into the sun's cascades as its Gaussians or as
/// ellipsoids, or not at all: a sheet of Gaussians a metre over the floor,
/// the sun slanting through it, darkens the floor where its shadow falls, and
/// moving it away brings the light back.
#[test]
fn a_splat_figure_casts_as_its_gaussians_or_as_ellipsoids() {
    let options = RendererOptions {
        default_world_lights: false,
        shadows: true,
        ..RendererOptions::default()
    };
    // A sheet a metre square, flat in x and z.
    let sheet: Vec<Gaussian> = (-10..=10)
        .flat_map(|i| (-10..=10).map(move |j| (i, j)))
        .map(|(i, j)| Gaussian {
            scale: [0.06, 0.01, 0.06],
            ..Gaussian::round(
                [i as f32 * 0.05, 0.0, j as f32 * 0.05],
                0.06,
                [200, 200, 200, 255],
            )
        })
        .collect();
    // The sun along +x and down: the sheet at y = 1 shadows the floor
    // about x = 1, where the camera looks straight down.
    let view = camera([1.0, 4.0, 0.0], 0.0, -89.9);
    let floor_at = |caster: SplatShadowCaster, moved: bool| {
        let mut harness = Harness::new(options);
        let mut ops = splats(&mut harness, 1, &sheet, [0.0, 1.0, 0.0]);
        ops.extend([
            RenderDiff::CreateLight {
                handle: RenderHandle::new(20),
                parent: None,
                light: LightDescriptor::Directional {
                    color: [1.0; 3],
                    intensity: 3.0,
                    enabled: true,
                    direction: [1.0, -1.0, 0.05],
                    range: None,
                    shadow_intent: LightShadowIntent::Requested,
                    shadow: Default::default(),
                },
            },
            RenderDiff::DefineMaterial {
                material: material("material/floor", [0.6, 0.6, 0.6, 1.0], None),
            },
            static_mesh(
                "mesh/floor",
                box_mesh([-5.0, -0.1, -5.0], [5.0, 0.0, 5.0], |_| 0),
                "material/floor",
            ),
            instance(30, None, "mesh/floor", transform([0.0; 3], 0.0, [1.0; 3])),
            RenderDiff::SetSplatAppearance {
                handle: RenderHandle::new(1),
                appearance: SplatAppearance {
                    shadow_caster: caster,
                    ..SplatAppearance::default()
                },
            },
        ]);
        harness.apply(ops);
        let shadowed = luminance(at(&harness.render(&view).1, CENTRE));
        if !moved {
            return (shadowed, shadowed);
        }
        harness.apply(vec![RenderDiff::Update {
            handle: RenderHandle::new(1),
            transform: Some(Transform {
                translation: [0.0, 1.0, 3.0],
                ..Transform::IDENTITY
            }),
            material: None,
            visible: None,
            metadata: None,
        }]);
        (shadowed, luminance(at(&harness.render(&view).1, CENTRE)))
    };
    let (open, _) = floor_at(SplatShadowCaster::None, false);
    let (gaussians, moved_away) = floor_at(SplatShadowCaster::Splats, true);
    let (ellipsoids, _) = floor_at(SplatShadowCaster::Ellipsoids, false);
    assert!(
        open > gaussians + 100,
        "open {open}, under its Gaussians {gaussians}"
    );
    assert!(
        open > ellipsoids + 100,
        "open {open}, under its ellipsoid {ellipsoids}"
    );
    assert!(
        (moved_away - open).abs() < 6,
        "open {open}, after it moved away {moved_away}"
    );
}

fn decode(byte: u8) -> f32 {
    let c = f32::from(byte) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn encode(linear: f32) -> i32 {
    let c = linear.clamp(0.0, 1.0);
    let display = if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (display * 255.0).round() as i32
}

/// The camera's focal length in pixels: 60° over `HEIGHT` rows.
fn focal() -> f32 {
    (HEIGHT as f32 / 2.0) / 30f32.to_radians().tan()
}

/// The pixel whose centre is nearest where a camera at the origin looking
/// down −z draws a point.
fn ahead(x: f32, y: f32, z: f32) -> (u32, u32) {
    (
        (WIDTH as f32 / 2.0 + focal() * x / -z).floor() as u32,
        (HEIGHT as f32 / 2.0 - focal() * y / -z).floor() as u32,
    )
}

/// The finish fogs a splat at its own distance: the composite writes the
/// depth of the nearest Gaussian at least half opaque, so linear fog over
/// ten metres takes half of one five metres away and four fifths of one
/// eight metres away, where without that depth it would take the
/// background's (none).
#[test]
fn splats_are_fogged_at_their_own_distance() {
    let mut harness = Harness::new(RendererOptions::default());
    let fog = [0.5, 0.25, 0.75];
    let color = [200, 100, 50, 255];
    let mut ops = splats(
        &mut harness,
        1,
        &[Gaussian::round([0.0; 3], 0.3, color)],
        [-1.0, 0.0, -5.0],
    );
    ops.extend(splats(
        &mut harness,
        2,
        &[Gaussian::round([0.0; 3], 0.4, color)],
        [1.5, 0.0, -8.0],
    ));
    ops.push(RenderDiff::SetFog {
        fog: Some(FogDescriptor::Linear {
            color: fog,
            start: 0.0,
            end: 10.0,
        }),
    });
    harness.apply(ops);
    let (_, rgba) = harness.render(&front());
    let fogged = |visibility: f32| -> [i32; 3] {
        [0, 1, 2].map(|c| encode(decode(color[c]) * visibility + fog[c] * (1.0 - visibility)))
    };
    // The distance along the ray to each Gaussian's centre.
    let near = ahead(-1.0, 0.0, -5.0);
    let distance = (1.0f32 + 25.0).sqrt();
    assert_near(at(&rgba, near), fogged(1.0 - distance / 10.0), 3);
    let far = ahead(1.5, 0.0, -8.0);
    let distance = (1.5f32 * 1.5 + 64.0).sqrt();
    assert_near(at(&rgba, far), fogged(1.0 - distance / 10.0), 3);
}

/// A blended sprite and two splat instances, seen from either side: from
/// the front `near` (red) covers its pixel and the half-transparent blue
/// sprite blends over `far` (green); from behind the order turns, the
/// sprite between them splitting them into two runs both ways.
#[test]
fn splat_instances_and_a_blended_sprite_turn_their_order_with_the_camera() {
    let mut harness = Harness::new(RendererOptions {
        default_world_lights: false,
        ..RendererOptions::default()
    });
    let texture = harness
        .resources
        .texture("texture/white", 1, 1, &[255; 4], TextureWrap::Clamp);
    let mut ops = vec![
        RenderDiff::SetBackgroundColor {
            color: [0.0, 0.0, 0.0, 1.0],
        },
        RenderDiff::DefineTexture { texture },
        RenderDiff::DefineSpriteAtlas {
            atlas: SpriteAtlasDescriptor {
                id: "sprite/white".into(),
                texture: "texture/white".into(),
                frames: vec![SpriteFrameRect {
                    frame: 0,
                    uv_min: [0.0; 2],
                    uv_max: [1.0; 2],
                    size: None,
                }],
            },
        },
        RenderDiff::CreateSprite {
            handle: RenderHandle::new(10),
            parent: None,
            sprite: SpriteInstanceDescriptor {
                asset: "sprite/white".to_owned(),
                frame: 0,
                pivot: [0.5, 0.5],
                size: [3.0, 3.0],
                size_mode: SpriteSizeMode::World,
                billboard: BillboardMode::Spherical,
                tint: [0.0, 0.0, 1.0, 0.5],
                render_order: 0,
                depth: SpriteDepthPolicy::Default,
                layer: RenderLayer::Scene,
                viewport_placement: None,
                shading: SpriteShading::Unlit,
                material: SpriteMaterialDescriptor {
                    alpha: SpriteAlphaMode::Blend,
                    ..SpriteMaterialDescriptor::default()
                },
                visible: true,
                transform: Transform {
                    translation: [0.0, 0.0, -5.0],
                    ..Transform::IDENTITY
                },
                attachment: SpriteAttachment::default(),
                metadata: RenderMetadata::default(),
            },
        },
    ];
    ops.extend(splats(
        &mut harness,
        1,
        &[Gaussian::round([0.0; 3], 0.15, [255, 0, 0, 255])],
        [-0.5, 0.0, -4.0],
    ));
    ops.extend(splats(
        &mut harness,
        2,
        &[Gaussian::round([0.0; 3], 0.15, [0, 255, 0, 255])],
        [0.5, 0.0, -6.0],
    ));
    harness.apply(ops);
    // Within half a pixel of a Gaussian's centre its falloff is 0.996, and
    // the sRGB curve is steep near black: the sprite under it shows by a
    // few levels. Drawn in the wrong order it would show by 186.
    const NEAR: i32 = 8;
    // Half blue over a colour, blended in linear light.
    let under_sprite = |under: [i32; 3]| -> [i32; 3] {
        let blue = [0.0, 0.0, 1.0];
        [0, 1, 2].map(|c| encode(0.5 * blue[c] + 0.5 * decode(under[c] as u8)))
    };
    let front_view = front();
    harness.render(&front_view);
    let (_, rgba) = harness.render(&front_view);
    assert_near(at(&rgba, ahead(-0.5, 0.0, -4.0)), [255, 0, 0], NEAR);
    assert_near(
        at(&rgba, ahead(0.5, 0.0, -6.0)),
        under_sprite([0, 255, 0]),
        NEAR,
    );
    assert_eq!(harness.renderer.gpu_readout().splats.runs, 2);

    // From ten metres down the axis, looking back: +x is on the left.
    let behind = camera([0.0, 0.0, -10.0], 180.0, 0.0);
    harness.render(&behind);
    let (_, rgba) = harness.render(&behind);
    let from_behind = |x: f32, z: f32| ahead(-x, 0.0, -(z + 10.0));
    assert_near(at(&rgba, from_behind(0.5, -6.0)), [0, 255, 0], NEAR);
    assert_near(
        at(&rgba, from_behind(-0.5, -4.0)),
        under_sprite([255, 0, 0]),
        NEAR,
    );
    assert_eq!(harness.renderer.gpu_readout().splats.runs, 2);
}

/// The motion view's range: `compose.wgsl` `MOTION_VIEW_RANGE`.
const MOTION_RANGE: f32 = 16.0;

/// The motion the motion view shows at a pixel, render pixels right and
/// down.
fn motion_at(rgba: &[u8], (x, y): (u32, u32)) -> [f32; 2] {
    let index = ((y * WIDTH + x) * 4) as usize;
    [rgba[index], rgba[index + 1]]
        .map(|value| (f32::from(value) / 255.0 - 0.5) * 2.0 * MOTION_RANGE)
}

#[track_caller]
fn assert_motion(found: [f32; 2], expected: [f32; 2], tolerance: f32) {
    assert!(
        (found[0] - expected[0]).abs() <= tolerance && (found[1] - expected[1]).abs() <= tolerance,
        "motion {found:?}, expected {expected:?}"
    );
}

fn motion_shown(harness: &mut Harness) {
    harness
        .renderer
        .set_overlay_shown(render_wgpu::OverlayShown {
            motion: true,
            ..Default::default()
        });
}

/// A moved splat instance draws its Gaussians' motion where it covers the
/// pixel, once, and holds still after.
#[test]
fn a_moved_splat_instance_moves_once_and_then_holds() {
    let mut harness = Harness::new(RendererOptions::default());
    motion_shown(&mut harness);
    let ops = splats(
        &mut harness,
        1,
        &[Gaussian::round([0.0; 3], 0.3, [200, 100, 50, 255])],
        [0.0, 0.0, -3.0],
    );
    harness.apply(ops);
    harness.render(&front());
    harness.render(&front());
    harness.apply(vec![RenderDiff::Update {
        handle: RenderHandle::new(1),
        transform: Some(Transform {
            translation: [0.2, 0.0, -3.0],
            ..Transform::IDENTITY
        }),
        material: None,
        visible: None,
        metadata: None,
    }]);
    let (_, moved) = harness.render(&front());
    let centre = ahead(0.2, 0.0, -3.0);
    assert_motion(motion_at(&moved, centre), [focal() * 0.2 / 3.0, 0.0], 0.3);
    let (_, held) = harness.render(&front());
    assert_motion(motion_at(&held, centre), [0.0, 0.0], 0.15);
}

/// A skinned splat figure whose pose moves draws its Gaussians' motion
/// from the previous palette: the fitted root climbs half a metre between
/// two samples of the clip, the instance itself standing still.
#[test]
fn a_posed_splat_figure_moves_by_its_previous_palette() {
    let mut harness = Harness::new(RendererOptions::default());
    motion_shown(&mut harness);
    let (body, instance) = skinned_arm_figure(&mut harness, 0.0);
    harness.apply(vec![
        RenderDiff::DefineAnimatedMesh { asset: body },
        instance,
    ]);
    let view = camera([0.0, 1.0, 6.0], 0.0, 0.0);
    harness.render(&view);
    harness.render(&view);
    harness.apply(vec![RenderDiff::SetAnimatedMeshPlayback {
        handle: RenderHandle::new(1),
        playback: AnimatedMeshPlaybackCommand::Sample {
            clip: "raise".to_owned(),
            normalized_time: 1.0,
        },
    }]);
    let (_, moved) = harness.render(&view);
    // Half a metre up at six metres; y down.
    let climb = [0.0, -focal() * 0.5 / 6.0];
    assert_motion(motion_at(&moved, on_plane(0.0, 1.25)), climb, 0.4);
    assert_motion(motion_at(&moved, on_plane(-0.5, 1.75)), climb, 0.4);
    let (_, held) = harness.render(&view);
    assert_motion(motion_at(&held, on_plane(0.0, 1.25)), [0.0, 0.0], 0.15);
}

/// A skinned splat on a joint posed at a rest scale of 2, bound under an
/// inverse bind of a half, is drawn at its own size (the shader takes the
/// palette's scale off its shape), so it is culled by its whole reach: one
/// whose centre stands 0.2 m past the edge of an orthographic view still
/// shows its tail there, two standard deviations out.
#[test]
fn a_skinned_splat_past_the_edge_of_the_view_shows_its_reach() {
    use support::splat_glb::{arm_clip_pack_glb, skinned_splat_glb_on};
    let mut harness = Harness::new(RendererOptions::default());
    let skeleton = serde_json::json!([
        {"name": "character", "children": [1]},
        {"name": "root", "scale": [2.0, 2.0, 2.0], "children": [2]},
        {"name": "arm", "translation": [0.0, 1.0, 0.0]},
    ]);
    #[rustfmt::skip]
    let inverse_binds = [
        0.5, 0.0, 0.0, 0.0, 0.0, 0.5, 0.0, 0.0, 0.0, 0.0, 0.5, 0.0, 0.0, 0.0, 0.0, 1.0,
        0.5, 0.0, 0.0, 0.0, 0.0, 0.5, 0.0, 0.0, 0.0, 0.0, 0.5, 0.0, 0.0, -1.0, 0.0, 1.0,
    ];
    let mut body = admit(
        &mut harness,
        "scaled",
        skinned_splat_glb_on(
            skeleton,
            &inverse_binds,
            &[Gaussian::round([0.0; 3], 0.1, [255, 0, 0, 255])],
            &[0],
        ),
    );
    let pack = admit_pack(&mut harness, arm_clip_pack_glb());
    let hash = pack.content_hash.clone().expect("content hash");
    body.clip_packs.push(AnimationClipPack {
        asset: format!("animation-clip-pack/{}", hash.trim_start_matches("sha256:")),
        runtime_format: pack.runtime_format,
        content_hash: hash.clone(),
        rig: pack.rig.clone().expect("the pack's rig"),
        clips: pack.clips.clone(),
        provenance: AnimationClipPackProvenance {
            producer: "fixture".to_owned(),
            source_hash: hash,
            target_hash: body.content_hash.clone().expect("content hash"),
            license: "CC0".to_owned(),
        },
        binding: AnimationClipPackBinding::JointNames,
        translation_fits: Vec::new(),
    });
    // The view reaches 16/9 m either side of its axis; the Gaussian's
    // centre stands 0.2 m past its right edge.
    let half_width = WIDTH as f32 / HEIGHT as f32;
    let instance = match splat_instance(1, &body, [half_width + 0.2, 0.0, -3.0]) {
        RenderDiff::CreateAnimatedMeshInstance {
            handle,
            parent,
            mut instance,
        } => {
            instance.playback = Some(AnimatedMeshPlaybackCommand::Sample {
                clip: "raise".to_owned(),
                normalized_time: 0.0,
            });
            RenderDiff::CreateAnimatedMeshInstance {
                handle,
                parent,
                instance,
            }
        }
        _ => unreachable!(),
    };
    harness.apply(vec![
        RenderDiff::DefineAnimatedMesh { asset: body },
        instance,
    ]);
    let mut view = front();
    view.projection = render_host_contracts::RendererCameraProjection::Orthographic {
        vertical_size: 2.0,
        near: 0.1,
        far: 100.0,
    };
    harness.render(&view);
    let (_, rgba) = harness.render(&view);
    assert_eq!(harness.renderer.gpu_readout().splats.instances, 1);
    let edge = at(&rgba, (WIDTH - 1, HEIGHT / 2));
    let background = at(&rgba, (4, 4));
    assert!(
        edge[0] > background[0] + 20,
        "the tail at the edge {edge:?} over {background:?}"
    );
}

/// A snapshot of the world drawn by a fresh renderer, as a scene reading
/// draws it, draws its splat instances as the running renderer does.
#[test]
fn a_fresh_renderer_draws_a_snapshots_splats_as_the_running_one() {
    let mut harness = Harness::new(RendererOptions::default());
    let mut ops = splats(
        &mut harness,
        1,
        &[
            Gaussian::round([0.0; 3], 0.3, [200, 100, 50, 200]),
            Gaussian::round([0.2, 0.1, 0.1], 0.2, [50, 100, 200, 160]),
        ],
        [0.0, 0.0, -3.0],
    );
    let (body, instance) = skinned_arm_figure(&mut harness, 0.5);
    ops.push(RenderDiff::DefineAnimatedMesh { asset: body });
    ops.push(match instance {
        RenderDiff::CreateAnimatedMeshInstance {
            parent,
            mut instance,
            ..
        } => {
            instance.transform.translation = [1.0, -1.0, -5.0];
            RenderDiff::CreateAnimatedMeshInstance {
                handle: RenderHandle::new(2),
                parent,
                instance,
            }
        }
        _ => unreachable!(),
    });
    harness.apply(ops);
    harness.render(&front());
    let (_, running) = harness.render(&front());
    let (_, fresh) = harness.render_fresh(&front());
    assert_ne!(
        at(&running, CENTRE),
        at(&running, (4, 4)),
        "the splats draw"
    );
    assert!(
        running == fresh,
        "the fresh renderer draws the snapshot's splats alike"
    );
}

/// The skinned arm figure (`skinned_splat_glb`) with the arm pack bound by
/// joint names, its root fitted, sampling `raise` at `normalized_time`, as
/// instance 1 at the origin.
fn skinned_arm_figure(
    harness: &mut Harness,
    normalized_time: f32,
) -> (AnimatedMeshAsset, RenderDiff) {
    use support::splat_glb::{arm_clip_pack_glb, skinned_splat_glb};
    let mut body = admit(
        harness,
        "skinned",
        skinned_splat_glb(
            &[
                Gaussian::round([0.0, 0.5, 0.0], 0.1, [0, 255, 0, 255]),
                Gaussian::round([0.0, 1.5, 0.0], 0.1, [255, 0, 0, 255]),
            ],
            &[0, 1],
        ),
    );
    let pack = admit_pack(harness, arm_clip_pack_glb());
    let hash = pack.content_hash.clone().expect("content hash");
    body.clip_packs.push(AnimationClipPack {
        asset: format!("animation-clip-pack/{}", hash.trim_start_matches("sha256:")),
        runtime_format: pack.runtime_format,
        content_hash: hash.clone(),
        rig: pack.rig.clone().expect("the pack's rig"),
        clips: pack.clips.clone(),
        provenance: AnimationClipPackProvenance {
            producer: "fixture".to_owned(),
            source_hash: hash,
            target_hash: body.content_hash.clone().expect("content hash"),
            license: "CC0".to_owned(),
        },
        binding: AnimationClipPackBinding::JointNames,
        translation_fits: vec![AnimationClipTranslationFit {
            clip: "raise".to_owned(),
            joint: "root".to_owned(),
            scale: [1.0, 0.5, 1.0],
            offset: [0.0, 0.25, 0.0],
        }],
    });
    let instance = match splat_instance(1, &body, [0.0; 3]) {
        RenderDiff::CreateAnimatedMeshInstance {
            handle,
            parent,
            mut instance,
        } => {
            instance.playback = Some(AnimatedMeshPlaybackCommand::Sample {
                clip: "raise".to_owned(),
                normalized_time,
            });
            RenderDiff::CreateAnimatedMeshInstance {
                handle,
                parent,
                instance,
            }
        }
        _ => unreachable!(),
    };
    (body, instance)
}

/// The committed splat fixtures (`fixtures/render/assets/splats`) and the
/// bytes the writer here makes for them.
fn committed_fixtures() -> [(std::path::PathBuf, Vec<u8>); 2] {
    use support::splat_glb::{arm_clip_pack_glb, skinned_splat_glb};
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../fixtures/render/assets/splats");
    [
        (
            dir.join("skinned-arm.glb"),
            skinned_splat_glb(
                &[
                    Gaussian::round([0.0, 0.5, 0.0], 0.1, [0, 255, 0, 255]),
                    Gaussian::round([0.0, 1.5, 0.0], 0.1, [255, 0, 0, 255]),
                ],
                &[0, 1],
            ),
        ),
        (dir.join("arm-pack.glb"), arm_clip_pack_glb()),
    ]
}

/// Other crates' tests read the committed fixtures; they are what the
/// writer here makes (`write_the_splat_fixtures` writes them again).
#[test]
fn the_committed_splat_fixtures_are_the_writers() {
    for (path, bytes) in committed_fixtures() {
        let committed =
            std::fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        assert!(
            committed == bytes,
            "{} differs from its writer: run `cargo test -p render-wgpu --test splats write_the_splat_fixtures -- --ignored`",
            path.display()
        );
    }
}

#[test]
#[ignore = "writes fixtures/render/assets/splats"]
fn write_the_splat_fixtures() {
    for (path, bytes) in committed_fixtures() {
        std::fs::write(&path, bytes).expect("write the fixture");
    }
}
