//! Skinning on the GPU (`skin.rs`) draws what skinning on the CPU draws:
//! the same skinned character with a weapon on its hand, drawn plain, with
//! its motion, in cascaded and traced shadows, and in an output capture,
//! by a renderer that skins on the GPU and one that skins on the CPU
//! (`RendererOptions::cpu_skinning`). The two differ only by float
//! rounding: at most `TOLERANCE` levels in a channel, in a few pixels.

mod support;

use std::path::PathBuf;

use asset_import::{import_animated_glb_asset, ImportContext, SourceUri};
use render_model::*;
use render_wgpu::{Gpu, OverlayShown, RendererOptions};
use support::*;

const BODY: u64 = 1;
const WEAPON: u64 = 2;
const RUNNER: u64 = 3;

/// The largest channel difference rounding may make between the two.
const TOLERANCE: u8 = 2;
/// The share of pixels that may differ at all.
const DIFFERING: f64 = 0.002;

fn fixture(path: &str) -> Vec<u8> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures");
    std::fs::read(root.join(path)).unwrap_or_else(|error| panic!("read {path}: {error}"))
}

fn admit(harness: &mut Harness, path: &str) -> AnimatedMeshAsset {
    let bytes = fixture(path);
    let imported = import_animated_glb_asset(
        &SourceUri::RelativePath(path.to_owned()),
        &bytes,
        &ImportContext::default(),
    );
    let imported = imported
        .assets
        .unwrap_or_else(|| panic!("{path} is admitted: {:?}", imported.diagnostics));
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

fn animated(
    handle: u64,
    parent: Option<u64>,
    asset: &AnimatedMeshAsset,
    transform: Transform,
    playback: Option<AnimatedMeshPlaybackCommand>,
) -> RenderDiff {
    RenderDiff::CreateAnimatedMeshInstance {
        handle: RenderHandle::new(handle),
        parent: parent.map(RenderHandle::new),
        instance: AnimatedMeshInstanceDescriptor {
            splat: Default::default(),
            splat_groups: Vec::new(),
            playback_fade: None,
            inspection: AnimatedMeshInspection::default(),
            asset: asset.asset.clone(),
            transform,
            visible: true,
            material_overrides: Vec::new(),
            playback,
            metadata: RenderMetadata {
                source_entity: Some(handle),
                ..RenderMetadata::default()
            },
            layer: RenderLayer::Scene,
            shadow_casting: Default::default(),
        },
    }
}

fn sample(normalized_time: f32) -> AnimatedMeshPlaybackCommand {
    AnimatedMeshPlaybackCommand::Sample {
        clip: "run".to_owned(),
        normalized_time,
    }
}

/// The joint attachment character, running, with a weapon on its right
/// hand, over a floor in a sun's light.
fn scene(harness: &mut Harness) {
    let body = admit(harness, "csharp-joint-attachments/content/body.glb");
    let weapon = admit(harness, "csharp-joint-attachments/content/weapon.glb");
    harness.apply(vec![
        RenderDiff::DefineAnimatedMesh {
            asset: body.clone(),
        },
        RenderDiff::DefineAnimatedMesh {
            asset: weapon.clone(),
        },
        animated(BODY, None, &body, Transform::IDENTITY, Some(sample(0.5))),
        // A second runner, whose vertices lie further into the arena than
        // the first's: drawn, traced and copied from where they lie.
        animated(
            RUNNER,
            None,
            &body,
            transform([-1.6, 0.0, -0.8], 30.0, [1.0; 3]),
            Some(sample(0.2)),
        ),
        animated(
            WEAPON,
            Some(BODY),
            &weapon,
            Transform {
                translation: [0.0; 3],
                rotation: [0.0, 0.0, 0.0, 1.0],
                scale: [0.01; 3],
            },
            None,
        ),
        RenderDiff::SetParentJoint {
            handle: RenderHandle::new(WEAPON),
            joint: Some("RightHand".to_owned()),
        },
        RenderDiff::DefineMaterial {
            material: material("material/floor", [0.75, 0.75, 0.72, 1.0], None),
        },
        static_mesh(
            "mesh/floor",
            box_mesh([-4.0, -0.1, -4.0], [4.0, 0.0, 4.0], |_| 0),
            "material/floor",
        ),
        instance(20, None, "mesh/floor", Transform::IDENTITY),
        group(23, None, transform([2.5, 4.0, 1.5], 0.0, [1.0; 3])),
        RenderDiff::CreateLight {
            handle: RenderHandle::new(21),
            parent: Some(RenderHandle::new(23)),
            light: LightDescriptor::Directional {
                color: [1.0, 0.97, 0.9],
                intensity: 2.5,
                enabled: true,
                direction: [-0.5, -1.0, -0.3],
                range: None,
                shadow_intent: LightShadowIntent::Requested,
                shadow: Default::default(),
            },
        },
        RenderDiff::CreateLight {
            handle: RenderHandle::new(22),
            parent: None,
            light: LightDescriptor::Ambient {
                color: [1.0; 3],
                intensity: 0.35,
                enabled: true,
                shadow_intent: LightShadowIntent::Disabled,
                shadow: Default::default(),
                range: None,
            },
        },
    ]);
}

/// From the front, and from above where the shadows fall.
fn views() -> [render_host_contracts::RendererCompositionCamera; 2] {
    [
        camera([4.0, 3.0, 6.0], -33.690_067, -11.750_674),
        camera([0.5, 5.5, 3.5], 0.0, -58.0),
    ]
}

/// A harness on `gpu` that skins on the GPU, and one that skins on the CPU.
fn pair(gpu: &Gpu, options: RendererOptions) -> [Harness; 2] {
    [false, true].map(|cpu_skinning| {
        let mut harness = Harness::on(
            gpu.clone(),
            RendererOptions {
                cpu_skinning,
                ..options
            },
        );
        scene(&mut harness);
        harness
    })
}

fn pose(harnesses: &mut [Harness; 2], normalized_time: f32) {
    for harness in harnesses {
        harness.apply(vec![RenderDiff::SetAnimatedMeshPlayback {
            handle: RenderHandle::new(BODY),
            playback: sample(normalized_time),
        }]);
    }
}

/// The two pictures differ by rounding at most.
#[track_caller]
fn same(what: &str, gpu: &[u8], cpu: &[u8]) {
    assert_eq!(gpu.len(), cpu.len());
    let mut differing = 0;
    let mut largest = 0;
    for (a, b) in gpu.as_chunks::<4>().0.iter().zip(cpu.as_chunks::<4>().0) {
        let difference = a
            .iter()
            .zip(b)
            .map(|(a, b)| a.abs_diff(*b))
            .max()
            .unwrap_or(0);
        largest = largest.max(difference);
        differing += usize::from(difference > 0);
    }
    let share = differing as f64 / (gpu.len() / 4) as f64;
    eprintln!("{what}: {differing} pixels differ, by at most {largest}");
    assert!(
        largest <= TOLERANCE && share <= DIFFERING,
        "{what}: {differing} pixels differ ({share:.4}), by at most {largest}"
    );
}

/// The GPU's pass posed the skinned meshes, and the CPU's renderer had none.
fn skinned_on_the_gpu(harnesses: &[Harness; 2]) {
    let timed = |harness: &Harness| {
        let readout = harness.renderer.gpu_readout();
        readout.timestamps.then(|| {
            readout
                .passes
                .iter()
                .find(|pass| pass.pass == "skinning")
                .map_or(0, |pass| pass.timed_frames)
        })
    };
    if let (Some(gpu), Some(cpu)) = (timed(&harnesses[0]), timed(&harnesses[1])) {
        assert!(gpu > 0, "the GPU renderer's skinning pass ran");
        assert_eq!(cpu, 0, "the CPU renderer ran no skinning pass");
    }
}

#[test]
fn a_character_skinned_on_the_gpu_draws_as_one_skinned_on_the_cpu() {
    let mut harnesses = pair(
        &gpu(),
        RendererOptions {
            default_world_lights: false,
            ..RendererOptions::default()
        },
    );
    for (step, normalized_time) in [0.5, 0.1, 0.35, 0.8].into_iter().enumerate() {
        pose(&mut harnesses, normalized_time);
        for (index, view) in views().iter().enumerate() {
            let [gpu, cpu] = harnesses.each_mut().map(|harness| harness.render(view).1);
            same(&format!("pose {step}, view {index}"), &gpu, &cpu);
        }
    }
    skinned_on_the_gpu(&harnesses);
}

#[test]
fn shadows_cast_by_a_character_skinned_on_the_gpu_fall_as_the_cpus_do() {
    let mut harnesses = pair(
        &gpu(),
        RendererOptions {
            default_world_lights: false,
            shadows: true,
            ..RendererOptions::default()
        },
    );
    let view = &views()[1];
    for (step, normalized_time) in [0.5, 0.0, 0.6].into_iter().enumerate() {
        pose(&mut harnesses, normalized_time);
        let [gpu, cpu] = harnesses.each_mut().map(|harness| harness.render(view).1);
        same(&format!("shadowed pose {step}"), &gpu, &cpu);
    }
}

#[test]
fn traced_shadows_of_a_character_skinned_on_the_gpu_fall_as_the_cpus_do() {
    let gpu = Gpu::headless_with_ray_queries().expect("a device");
    let mut harnesses = pair(
        &gpu,
        RendererOptions {
            default_world_lights: false,
            shadows: true,
            ray_traced_shadows: true,
            ..RendererOptions::default()
        },
    );
    if harnesses[0]
        .renderer
        .settings_readout()
        .ray_traced_shadows
        .is_some()
    {
        eprintln!("this device traces no shadows: nothing to compare");
        return;
    }
    let view = &views()[1];
    for (step, normalized_time) in [0.5, 0.0, 0.6].into_iter().enumerate() {
        pose(&mut harnesses, normalized_time);
        // The first frame of a pose builds its BLAS, the next traces it.
        let [gpu, cpu] = harnesses.each_mut().map(|harness| {
            harness.render(view);
            harness.render(view).1
        });
        same(&format!("traced pose {step}"), &gpu, &cpu);
    }
}

#[test]
fn the_motion_of_a_character_skinned_on_the_gpu_is_the_cpus() {
    let mut harnesses = pair(&gpu(), RendererOptions::default());
    for harness in &mut harnesses {
        harness.renderer.set_overlay_shown(OverlayShown {
            motion: true,
            ..Default::default()
        });
    }
    let view = &views()[0];
    for harness in &mut harnesses {
        harness.render(view);
        harness.render(view);
    }
    // A new pose draws its motion; held, the next frame draws none; and a
    // new pose after a held one draws its motion again.
    for (step, normalized_time) in [0.56, 0.56, 0.62, 0.1].into_iter().enumerate() {
        pose(&mut harnesses, normalized_time);
        let [gpu, cpu] = harnesses.each_mut().map(|harness| harness.render(view).1);
        same(&format!("motion step {step}"), &gpu, &cpu);
    }
}

#[test]
fn an_output_capture_of_a_character_skinned_on_the_gpu_is_the_cpus() {
    use render_host_contracts::{RenderOutputJob, RenderOutputOperation};
    let harnesses = pair(
        &gpu(),
        RendererOptions {
            default_world_lights: false,
            ..RendererOptions::default()
        },
    );
    let capture = |harness: &Harness, normalized_time: f32| {
        let mut frame = harness
            .world
            .capture_output_scene(RenderHandle::new(BODY), true)
            .expect("capture the body subtree");
        frame.push(RenderDiff::SetEvaluatedPose {
            handle: RenderHandle::new(BODY),
            pose: harness
                .poses
                .sample_clip(
                    RenderHandle::new(BODY),
                    "run",
                    normalized_time,
                    &harness.world,
                )
                .expect("the body is posed"),
        });
        let job = RenderOutputJob {
            id: 1,
            source: RenderHandle::new(BODY),
            frame,
            operation: RenderOutputOperation::Image {
                camera: Box::new(views()[0].clone()),
                width: 256,
                height: 256,
                background: [0.0; 4],
                use_camera_background: false,
                exposure: 1.0,
                aces_filmic: false,
                samples: 1,
            },
        };
        harness
            .renderer
            .capture_image(&job, &harness.resources)
            .expect("pose capture")
    };
    for normalized_time in [0.1, 0.6] {
        let [gpu, cpu] = harnesses
            .each_ref()
            .map(|harness| capture(harness, normalized_time));
        same(&format!("capture at {normalized_time}"), &gpu, &cpu);
    }
}
