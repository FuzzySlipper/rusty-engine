//! Skinned Gaussian splat spike (docs/gaussian-splats.md): a posed
//! `.vsplat` character among ordinary meshes, and its cost at 1, 8 and 32
//! instances.
//!
//! The characters live in the asset-pipeline checkout
//! (`SPLAT_ASSETS`, default `~/dev/asset-pipeline/tests/splats`); without it
//! the tests say so and pass.

mod support;

use std::path::PathBuf;
use std::time::Instant;

use glam::{Mat4, Quat, Vec3};
use render_model::*;
use render_wgpu::{
    encode_png, posed_palette, OffscreenTarget, RendererOptions, SplatAsset, SplatAssetId,
    SplatInstance, SplatPoint,
};
use support::*;

const CHARACTER_48K: &str = "public/pinoc/rat.vsplat";
const CHARACTER_16K: &str = "public/pinoc/rat.performance.vsplat";
/// The characters face +x; this turns them toward +z.
const FACING_Z: f32 = -std::f32::consts::FRAC_PI_2;
const SKELETON_441: &str = "extracted/sdk-source/src/loader/skeleton/skeleton-data-441.ts";

fn assets_root() -> Option<PathBuf> {
    let root = std::env::var_os("SPLAT_ASSETS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var("HOME").unwrap_or_default())
                .join("dev/asset-pipeline/tests/splats")
        });
    root.join(CHARACTER_48K).exists().then_some(root)
}

/// The Viggle 441-bone rig's names, parents and bind rotations, from the
/// SDK's tables.
struct Rig {
    names: Vec<String>,
    parents: Vec<i32>,
    /// Each bone's bind rotation in its parent's frame.
    rotations: Vec<Quat>,
}

impl Rig {
    /// World rest joints of a character on this rig. Its `joints` block
    /// holds each bone's translation in its parent's frame, so the world
    /// bind is the hierarchy of those translations and the bind rotations.
    fn world_joints(&self, local: &[Vec3]) -> Vec<Vec3> {
        let mut world: Vec<Mat4> = Vec::with_capacity(local.len());
        for (j, translation) in local.iter().enumerate() {
            let parent = usize::try_from(self.parents[j]).map_or(Mat4::IDENTITY, |p| world[p]);
            world.push(parent * Mat4::from_rotation_translation(self.rotations[j], *translation));
        }
        world.iter().map(|m| m.w_axis.truncate()).collect()
    }
}

/// A 441-rig character from its `.vsplat`, its joints made world.
fn character(root: &std::path::Path, path: &str, rig: &Rig) -> SplatAsset {
    let decoded = asset_import::decode_vsplat(&std::fs::read(root.join(path)).unwrap())
        .expect("the character decodes");
    let points: Vec<SplatPoint> = decoded
        .splats
        .iter()
        .map(|splat| SplatPoint {
            position: splat.position,
            rotation: splat.rotation,
            sigma: splat.sigma,
            color: splat.color,
            opacity: splat.opacity,
            bones: splat.bones,
            weights: splat.weights,
        })
        .collect();
    let local: Vec<Vec3> = decoded
        .joints
        .iter()
        .map(|&j| Vec3::from_array(j))
        .collect();
    SplatAsset::new(&points, rig.world_joints(&local))
}

fn rig(root: &std::path::Path) -> Rig {
    let source = std::fs::read_to_string(root.join(SKELETON_441)).expect("skeleton tables");
    let array = |name: &str| -> String {
        let line = source
            .lines()
            .find(|line| line.starts_with(&format!("export const {name}")))
            .unwrap_or_else(|| panic!("{name} in the skeleton tables"));
        let open = line.find("= [").unwrap() + 2;
        let close = line.rfind(']').unwrap();
        line[open + 1..close].to_owned()
    };
    Rig {
        names: array("BONE_NAMES_441")
            .split(',')
            .map(|name| name.trim().trim_matches('"').to_owned())
            .collect(),
        parents: array("PARENTS_441")
            .split(',')
            .map(|parent| parent.trim().parse().unwrap())
            .collect(),
        rotations: array("REST_ROTATIONS_441")
            .split(',')
            .map(|value| value.trim().parse::<f32>().unwrap())
            .collect::<Vec<_>>()
            .chunks(4)
            .map(|q| Quat::from_xyzw(q[0], q[1], q[2], q[3]).normalize())
            .collect(),
    }
}

/// A walk-like test pose at `seconds`: arms lowered from the bind pose and
/// swinging, legs striding, the head turning. The Viggle characters face +x
/// in their own frame, so the stride turns about z.
fn pose(joints: &[Vec3], rig: &Rig, seconds: f32) -> Vec<Mat4> {
    let bone = |name: &str| rig.names.iter().position(|n| n == name).unwrap();
    let mut rotations = vec![Quat::IDENTITY; joints.len()];
    let swing = (seconds * 4.0).sin();
    for (side, sign) in [("l", 1.0f32), ("r", -1.0)] {
        let upper = bone(&format!("upperarm_{side}"));
        let hand = bone(&format!("hand_{side}"));
        let reach = (joints[hand] - joints[upper]).normalize();
        let lowered = (Vec3::new(reach.x * 0.25, -1.0, reach.z)).normalize();
        rotations[upper] =
            Quat::from_rotation_z(0.5 * sign * swing) * Quat::from_rotation_arc(reach, lowered);
        rotations[bone(&format!("thigh_{side}"))] = Quat::from_rotation_z(-0.45 * sign * swing);
        rotations[bone(&format!("calf_{side}"))] =
            Quat::from_rotation_z(-0.6 * (sign * swing).max(0.0));
    }
    rotations[bone("head")] = Quat::from_rotation_y(0.6 * (seconds * 1.3).sin());
    posed_palette(joints, &rig.parents, &rotations)
}

fn scene(harness: &mut Harness) {
    harness.apply(vec![
        RenderDiff::DefineMaterial {
            material: material("material/floor", [0.45, 0.45, 0.42, 1.0], None),
        },
        RenderDiff::DefineMaterial {
            material: material("material/red", [0.8, 0.2, 0.15, 1.0], None),
        },
        RenderDiff::DefineMaterial {
            material: material("material/blue", [0.15, 0.3, 0.85, 1.0], None),
        },
        static_mesh(
            "mesh/floor",
            box_mesh([-12.0, -0.1, -12.0], [12.0, 0.0, 12.0], |_| 0),
            "material/floor",
        ),
        static_mesh(
            "mesh/pillar",
            box_mesh([-0.15, 0.0, -0.15], [0.15, 1.2, 0.15], |_| 0),
            "material/red",
        ),
        static_mesh(
            "mesh/crate",
            box_mesh([-0.5, 0.0, -0.5], [0.5, 1.0, 0.5], |_| 0),
            "material/blue",
        ),
        // In front of the character's left leg: it must hide it.
        instance(4, None, "mesh/floor", Transform::IDENTITY),
        instance(
            1,
            None,
            "mesh/pillar",
            transform([0.25, 0.0, 0.9], 0.0, [1.0; 3]),
        ),
        instance(
            2,
            None,
            "mesh/crate",
            transform([-1.4, 0.0, -0.6], 25.0, [1.0; 3]),
        ),
        instance(
            3,
            None,
            "mesh/crate",
            transform([1.5, 0.0, -1.2], -10.0, [1.0, 1.8, 1.0]),
        ),
    ]);
}

fn save(name: &str, width: u32, height: u32, rgba: &[u8]) -> PathBuf {
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../target/render-wgpu-splats");
    std::fs::create_dir_all(&out).unwrap();
    let path = out.join(format!("{name}.png"));
    std::fs::write(&path, encode_png(width, height, rgba).unwrap()).unwrap();
    path
}

#[test]
fn a_posed_vsplat_draws_among_meshes() {
    let Some(root) = assets_root() else {
        eprintln!("no splat assets; set SPLAT_ASSETS to the asset-pipeline tests/splats folder");
        return;
    };
    let rig = rig(&root);
    let asset = character(&root, CHARACTER_48K, &rig);
    assert_eq!((asset.splats(), asset.joints.len()), (48_000, 441));
    let joints = asset.joints.clone();
    // The bind skeleton stands in the splats: the head at the top of their
    // bounds, the hands out at their sides.
    let head = joints[rig.names.iter().position(|n| n == "head").unwrap()];
    assert!(
        (asset.bounds.1.y - head.y).abs() < 0.35,
        "head {head} in {:?}",
        asset.bounds
    );

    let (width, height) = (960, 540);
    let mut harness = Harness::new(RendererOptions::default());
    harness.target = OffscreenTarget::new(&harness.gpu, width, height, 4);
    scene(&mut harness);
    let view = camera([0.4, 1.2, 2.6], 4.0, -8.0);
    let (_, meshes_only) = harness.render(&view);

    let id = harness.renderer.add_splat_asset(&asset);
    let instances = |palette: Vec<Mat4>| {
        vec![SplatInstance {
            asset: id,
            transform: Mat4::from_rotation_y(FACING_Z),
            palette,
        }]
    };
    harness.renderer.set_splat_instances(instances(Vec::new()));
    let (_, rest) = harness.render(&view);
    harness
        .renderer
        .set_splat_instances(instances(pose(&joints, &rig, 0.4)));
    let (_, posed) = harness.render(&view);
    println!("rest: {}", save("rest", width, height, &rest).display());
    println!("posed: {}", save("posed", width, height, &posed).display());
    let (_, side) = harness.render(&camera([2.6, 1.1, 0.3], -90.0, -6.0));
    println!(
        "side: {}",
        save("posed-side", width, height, &side).display()
    );

    let differing = |a: &[u8], b: &[u8]| {
        a.chunks(4)
            .zip(b.chunks(4))
            .filter(|(a, b)| a.iter().zip(b.iter()).any(|(a, b)| a.abs_diff(*b) > 12))
            .count()
    };
    let drawn = differing(&meshes_only, &rest);
    assert!(drawn > 5_000, "the character covers pixels: {drawn}");
    assert!(
        differing(&rest, &posed) > 1_000,
        "the pose moves the character"
    );

    // The pillar's middle stays the pillar's: the world's depth hides the
    // splats behind it.
    let pillar = pixel_of(&harness, &view, Vec3::new(0.25, 0.6, 1.05), width, height);
    let at = |rgba: &[u8]| rgba[pillar..pillar + 4].to_vec();
    assert_eq!(
        at(&meshes_only),
        at(&posed),
        "the pillar hides the character"
    );
}

/// The byte offset of `point`'s pixel under `view`.
fn pixel_of(
    _harness: &Harness,
    view: &render_host_contracts::RendererCompositionCamera,
    point: Vec3,
    width: u32,
    height: u32,
) -> usize {
    let pose = &view.pose;
    let eye = Vec3::new(
        pose.position[0] as f32,
        pose.position[1] as f32,
        pose.position[2] as f32,
    );
    let rotation = Quat::from_rotation_y(-(pose.yaw_degrees as f32).to_radians())
        * Quat::from_rotation_x((pose.pitch_degrees as f32).to_radians());
    let local = rotation.inverse() * (point - eye);
    let aspect = width as f32 / height as f32;
    let f = 1.0 / (30.0f32.to_radians()).tan();
    let ndc = (local.x * f / aspect / -local.z, local.y * f / -local.z);
    let x = ((ndc.0 * 0.5 + 0.5) * width as f32) as usize;
    let y = ((0.5 - ndc.1 * 0.5) * height as f32) as usize;
    (y * width as usize + x) * 4
}

/// Splat cost at 1920x1080 for 1, 8 and 32 instances of a 48k and a 16k
/// character, each posed every frame. Run on a real GPU:
/// `cargo test -p render-wgpu --release --test gaussian_splats -- --ignored --nocapture`
#[test]
#[ignore = "measurement; needs a hardware adapter and the splat assets"]
fn splat_costs() {
    let Some(root) = assets_root() else {
        eprintln!("no splat assets");
        return;
    };
    let rig = rig(&root);
    let (width, height) = (1920, 1080);
    let mut harness = Harness::new(RendererOptions::default());
    require_hardware_adapter(&harness.gpu, "splat costs");
    println!("adapter: {:?}", harness.gpu.adapter_summary());
    harness.target = OffscreenTarget::new(&harness.gpu, width, height, 4);
    scene(&mut harness);
    let assets: Vec<(&str, SplatAsset, Vec<Vec3>)> =
        [("48k", CHARACTER_48K), ("16k", CHARACTER_16K)]
            .into_iter()
            .map(|(name, path)| {
                let asset = character(&root, path, &rig);
                let joints = asset.joints.clone();
                (name, asset, joints)
            })
            .collect();
    let ids: Vec<SplatAssetId> = assets
        .iter()
        .map(|(_, asset, _)| harness.renderer.add_splat_asset(asset))
        .collect();
    let view = camera([0.0, 1.6, 7.0], 0.0, -8.0);
    // Close enough that the front row fills the frame: the raster's worst
    // case.
    let near = camera([0.0, 1.1, 1.6], 0.0, -4.0);

    let mut measure = |label: &str,
                       view: &render_host_contracts::RendererCompositionCamera,
                       bits: u32,
                       instances: &dyn Fn(f32) -> Vec<SplatInstance>| {
        harness.renderer.set_splat_sort_bits(bits);
        const WARM: u32 = 30;
        const FRAMES: u32 = 150;
        let mut wall = Vec::new();
        let mut pose_ms = 0.0;
        for frame in 0..WARM + FRAMES {
            let t = frame as f32 / 60.0;
            let start = Instant::now();
            let set = instances(t);
            let posed = start.elapsed().as_secs_f64() * 1000.0;
            harness.renderer.set_splat_instances(set);
            let start = Instant::now();
            harness.renderer.render_offscreen(view, &harness.target);
            harness.renderer.wait_for_gpu();
            if frame >= WARM {
                wall.push(start.elapsed().as_secs_f64() * 1000.0);
                pose_ms += posed;
            }
        }
        wall.sort_by(f64::total_cmp);
        let readout = harness.renderer.gpu_readout();
        let gpu = |pass: &str| {
            readout
                .passes
                .iter()
                .find(|timing| timing.pass == pass)
                .map_or(0.0, |timing| timing.median_gpu_ms)
        };
        let visible = harness.renderer.splat_visible_count();
        if label.ends_with("(24-bit sort)")
            && !label.starts_with("1 x")
            && !label.starts_with("8 x")
        {
            let pixels = harness.target.read_rgba(&harness.gpu);
            let name = label.replace(' ', "").replace(['(', ')'], "-");
            println!("{}", save(&name, width, height, &pixels).display());
        }
        println!(
            "{label:<28} {visible:>8} drawn | wall {:>6.2} ms | world {:>5.2} | skin {:>5.2} sort {:>5.2} draw {:>5.2} (splats {:>5.2}) | pose cpu {:>5.2} ms",
            wall[wall.len() / 2],
            gpu("world"),
            gpu("splat-skin"),
            gpu("splat-sort"),
            gpu("splat-draw"),
            gpu("splat-skin") + gpu("splat-sort") + gpu("splat-draw"),
            pose_ms / f64::from(FRAMES),
        );
    };

    measure("no splats", &view, 24, &|_| Vec::new());
    for ((name, _, joints), id) in assets.iter().zip(&ids) {
        for count in [1usize, 8, 32] {
            let instances = |t: f32| {
                (0..count)
                    .map(|i| {
                        // A crowd in rows of eight across the view.
                        let across = count.min(8) as f32;
                        let x = ((i % 8) as f32 - (across - 1.0) * 0.5) * 0.9;
                        let z = -((i / 8) as f32) * 1.2;
                        SplatInstance {
                            asset: *id,
                            transform: Mat4::from_translation(Vec3::new(x, 0.0, z))
                                * Mat4::from_rotation_y(FACING_Z),
                            palette: pose(joints, &rig, t + i as f32 * 0.37),
                        }
                    })
                    .collect()
            };
            let bits: &[u32] = if count == 32 { &[24, 16, 32] } else { &[24] };
            for &bits in bits {
                measure(
                    &format!("{count} x {name} ({bits}-bit sort)"),
                    &view,
                    bits,
                    &instances,
                );
            }
            if count != 8 {
                measure(
                    &format!("{count} x {name} near (24-bit sort)"),
                    &near,
                    24,
                    &instances,
                );
            }
        }
    }
}
