//! Capture a splat GLB (`docs/splat-characters.md`) from the cameras the
//! splat acceptance compares with the web reference: front, both sides,
//! back and three-quarter, each looking at the figure from `--distance`
//! at `--eye-height`, with a 60° vertical field of view. With `--occluded`
//! a box stands between the camera and the figure's legs and a wall behind
//! it. The GLB is read from a local path: character bytes stay out of the
//! repository.
//!
//! ```text
//! cargo run -p render-wgpu --example splat_capture -- \
//!     --glb PATH --out DIR [--size 420x720] [--eye-height 0.88] [--distance 3.6] [--occluded]
//! ```
//!
//! Writes `<view>.png` per view and `readout.json` (the splat readout and
//! the `splats` pass timing over the captures).

#[path = "../tests/support/mod.rs"]
mod support;

use asset_import::{import_animated_glb_asset, ImportContext, SourceUri};
use render_model::*;
use render_wgpu::{OffscreenTarget, RendererOptions};
use support::*;

struct Arguments {
    glb: std::path::PathBuf,
    out: std::path::PathBuf,
    size: (u32, u32),
    eye_height: f64,
    distance: f64,
    occluded: bool,
}

fn arguments() -> Result<Arguments, String> {
    let mut glb = None;
    let mut out = None;
    let mut size = (420, 720);
    let mut eye_height = 0.88;
    let mut distance = 3.6;
    let mut occluded = false;
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let mut value = || args.next().ok_or(format!("{flag} takes a value"));
        match flag.as_str() {
            "--glb" => glb = Some(value()?.into()),
            "--out" => out = Some(value()?.into()),
            "--size" => {
                let text = value()?;
                let (w, h) = text.split_once('x').ok_or("--size is WIDTHxHEIGHT")?;
                size = (
                    w.parse().map_err(|_| "--size width")?,
                    h.parse().map_err(|_| "--size height")?,
                );
            }
            "--eye-height" => eye_height = value()?.parse().map_err(|_| "--eye-height")?,
            "--distance" => distance = value()?.parse().map_err(|_| "--distance")?,
            "--occluded" => occluded = true,
            other => return Err(format!("unknown flag {other}")),
        }
    }
    Ok(Arguments {
        glb: glb.ok_or("--glb PATH is required")?,
        out: out.ok_or("--out DIR is required")?,
        size,
        eye_height,
        distance,
        occluded,
    })
}

fn main() {
    let arguments = arguments().unwrap_or_else(|error| {
        eprintln!("splat_capture: {error}");
        std::process::exit(2);
    });
    let bytes = std::fs::read(&arguments.glb).unwrap_or_else(|error| {
        eprintln!(
            "splat_capture: cannot read {}: {error} (character GLBs are local, outside the repository)",
            arguments.glb.display()
        );
        std::process::exit(2);
    });
    std::fs::create_dir_all(&arguments.out).expect("create the output directory");

    let mut harness = Harness::new(RendererOptions::default());
    let (width, height) = arguments.size;
    harness.target = OffscreenTarget::new(&harness.gpu, width, height, 1);
    let imported = import_animated_glb_asset(
        &SourceUri::RelativePath("character.glb".to_owned()),
        &bytes,
        &ImportContext::default(),
    );
    let asset = imported.assets.unwrap_or_else(|| {
        eprintln!("splat_capture: not admitted: {:?}", imported.diagnostics);
        std::process::exit(1);
    });
    let asset = asset.animated_mesh;
    let hash = asset.content_hash.clone().expect("content hash");
    harness.resources.insert(
        &format!(
            "animated-mesh-resource/{}",
            hash.trim_start_matches("sha256:")
        ),
        bytes,
    );
    let mut ops = vec![
        RenderDiff::DefineAnimatedMesh {
            asset: asset.clone(),
        },
        RenderDiff::CreateAnimatedMeshInstance {
            handle: RenderHandle::new(1),
            parent: None,
            instance: AnimatedMeshInstanceDescriptor {
                playback_fade: None,
                inspection: AnimatedMeshInspection::default(),
                asset: asset.asset.clone(),
                transform: Transform::IDENTITY,
                visible: true,
                material_overrides: Vec::new(),
                playback: None,
                metadata: RenderMetadata {
                    source_entity: Some(1),
                    ..RenderMetadata::default()
                },
                layer: RenderLayer::Scene,
                shadow_casting: Default::default(),
            },
        },
    ];
    if arguments.occluded {
        // Unlit, so their colours read back as written.
        for (id, color) in [
            ("material/occluder", [0.1, 0.5, 0.1, 1.0]),
            ("material/wall", [0.6, 0.6, 0.7, 1.0]),
        ] {
            ops.push(RenderDiff::DefineMaterial {
                material: RenderMaterialDescriptor {
                    unlit: true,
                    ..material(id, color, None)
                },
            });
        }
        ops.push(static_mesh(
            "mesh/occluder",
            box_mesh([-0.6, 0.0, -0.05], [0.6, 0.5, 0.05], |_| 0),
            "material/occluder",
        ));
        ops.push(static_mesh(
            "mesh/wall",
            box_mesh([-3.0, 0.0, -0.05], [3.0, 3.0, 0.05], |_| 0),
            "material/wall",
        ));
    }
    harness.apply(ops);

    let d = arguments.distance;
    let h = arguments.eye_height;
    let diagonal = d * std::f64::consts::FRAC_1_SQRT_2;
    // Engine yaw 0 looks down −z and positive yaw turns toward +x; the
    // figure faces +z.
    let views = [
        ("front", [0.0, h, d], 0.0),
        ("left", [d, h, 0.0], -90.0),
        ("right", [-d, h, 0.0], 90.0),
        ("back", [0.0, h, -d], 180.0),
        ("three-quarter", [diagonal, h, diagonal], -45.0),
    ];
    let mut handle = 100;
    for (name, position, yaw) in views {
        if arguments.occluded {
            // The box between this camera and the figure, the wall behind it.
            let toward = [position[0] / d, position[2] / d];
            let place = |distance: f64, id: &str, handle: u64| {
                instance(
                    handle,
                    None,
                    id,
                    transform(
                        [
                            (toward[0] * distance) as f32,
                            0.0,
                            (toward[1] * distance) as f32,
                        ],
                        // Square to the camera: a transform's yaw turns the
                        // other way from a camera's.
                        -yaw as f32,
                        [1.0; 3],
                    ),
                )
            };
            harness.apply(vec![
                place(0.9, "mesh/occluder", handle),
                place(-1.2, "mesh/wall", handle + 1),
            ]);
        }
        let camera = camera(position, yaw, 0.0);
        // Twice: the second frame has the first's readout.
        harness.render(&camera);
        let (_, rgba) = harness.render(&camera);
        let png = render_wgpu::encode_png(width, height, &rgba).expect("encode the capture");
        std::fs::write(arguments.out.join(format!("{name}.png")), png).expect("write the capture");
        if arguments.occluded {
            harness.apply(vec![
                RenderDiff::Destroy {
                    handle: RenderHandle::new(handle),
                },
                RenderDiff::Destroy {
                    handle: RenderHandle::new(handle + 1),
                },
            ]);
            handle += 2;
        }
    }
    // Time the splats over a few more frames of the last view.
    let camera = camera(views[0].1, views[0].2, 0.0);
    for _ in 0..30 {
        harness.render(&camera);
    }
    let readout = harness.renderer.gpu_readout();
    let timing = readout
        .passes
        .iter()
        .find(|timing| timing.pass == "splats")
        .map(|timing| (timing.timed_frames, timing.median_gpu_ms));
    let report = format!(
        "{{\"runs\": {}, \"instances\": {}, \"gaussians\": {}, \"refused\": {:?}, \"splats_pass\": {:?}, \"adapter\": {:?}}}\n",
        readout.splats.runs,
        readout.splats.instances,
        readout.splats.gaussians,
        readout.splats.refused,
        timing,
        harness.gpu.adapter_summary(),
    );
    std::fs::write(arguments.out.join("readout.json"), &report).expect("write the readout");
    print!("{report}");
}
