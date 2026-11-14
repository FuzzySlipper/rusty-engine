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
