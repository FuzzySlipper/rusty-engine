//! A product image effect over the finished picture (`image_effect.wgsl`,
//! `RenderDiff::SetImageEffect`): none draws as before, a pass-through
//! leaves the picture, its parameters reach the product's WGSL, it samples
//! the picture anywhere, it reads the depth, its pass is timed, and it draws
//! over what a temporal stage resolved. Same harness as
//! `tests/screenshots.rs`.

mod support;

use render_model::*;
use render_wgpu::RendererOptions;
use support::*;

/// A pass-through, a white flash by `effect_parameter(0).x`, and a shift of
/// the picture by `effect_parameter(0).y` of its width.
const FLASH: &str = r#"
#import rusty::image::{ImagePixel, effect_parameter, picture_at}

fn image_effect(pixel: ImagePixel) -> vec4<f32> {
    let flash = effect_parameter(0u).x;
    let shift = effect_parameter(0u).y;
    let color = picture_at(pixel.uv + vec2<f32>(shift, 0.0));
    return vec4<f32>(mix(color.rgb, vec3<f32>(1.0), flash), color.a);
}
"#;

/// Red where nothing of the world lies (the far plane), else the picture.
const SKY: &str = r#"
#import rusty::image::ImagePixel

fn image_effect(pixel: ImagePixel) -> vec4<f32> {
    if pixel.depth >= 1.0 {
        return vec4<f32>(1.0, 0.0, 0.0, 1.0);
    }
    return pixel.color;
}
"#;

fn define(id: &str, source: &str) -> RenderDiff {
    RenderDiff::DefineShader {
        shader: ShaderDescriptor {
            id: id.to_owned(),
            path: format!("shaders/{}.wgsl", id.trim_start_matches("shader/")),
            source: source.to_owned(),
            keywords: Vec::new(),
        },
    }
}

fn effect(shader: &str, flash: f32, shift: f32) -> RenderDiff {
    RenderDiff::SetImageEffect {
        effect: Some(ImageEffectDescriptor {
            shader: shader.to_owned(),
            parameters: [[flash, shift, 0.0, 0.0], [0.0; 4], [0.0; 4], [0.0; 4]],
            textures: [None, None],
        }),
    }
}

/// A lit grey ground with a red box on it, under a blue background.
fn scene() -> Harness {
    let mut harness = Harness::new(RendererOptions::default());
    harness.apply(vec![
        RenderDiff::SetBackgroundColor {
            color: [0.2, 0.3, 0.6, 1.0],
        },
        RenderDiff::DefineMaterial {
            material: material("material/ground", [0.5, 0.5, 0.5, 1.0], None),
        },
        RenderDiff::DefineMaterial {
            material: material("material/box", [0.8, 0.1, 0.1, 1.0], None),
        },
        static_mesh(
            "mesh/ground",
            box_mesh([-40.0, -1.0, -40.0], [40.0, 0.0, 40.0], |_| 0),
            "material/ground",
        ),
        static_mesh(
            "mesh/box",
            box_mesh([-1.0, 0.0, -6.0], [1.0, 2.0, -4.0], |_| 0),
            "material/box",
        ),
        instance(20, None, "mesh/ground", transform([0.0; 3], 0.0, [1.0; 3])),
        instance(21, None, "mesh/box", transform([0.0; 3], 0.0, [1.0; 3])),
        define("shader/flash", FLASH),
        define("shader/sky", SKY),
    ]);
    harness
}

fn look(harness: &mut Harness) -> Vec<u8> {
    harness.render(&camera([0.0, 1.5, 2.0], 0.0, -5.0)).1
}

fn pixel(rgba: &[u8], x: u32, y: u32) -> [i32; 3] {
    let at = ((y * WIDTH + x) * 4) as usize;
    [rgba[at], rgba[at + 1], rgba[at + 2]].map(i32::from)
}

fn close(a: &[u8], b: &[u8]) -> bool {
    a.iter()
        .zip(b)
        .all(|(x, y)| (i32::from(*x) - i32::from(*y)).abs() <= 2)
}

#[test]
fn no_effect_and_a_pass_through_leave_the_picture() {
    let mut harness = scene();
    let plain = look(&mut harness);
    harness.apply(vec![effect("shader/flash", 0.0, 0.0)]);
    let passed = look(&mut harness);
    assert!(
        close(&plain, &passed),
        "a pass-through effect leaves the picture"
    );
    harness.apply(vec![RenderDiff::SetImageEffect { effect: None }]);
    assert_eq!(look(&mut harness), plain, "no effect draws as before");
}

#[test]
fn changing_parameters_reuses_the_pipeline_and_a_redefined_shader_rebuilds_it() {
    let mut harness = scene();
    harness.apply(vec![effect("shader/flash", 0.0, 0.0)]);
    let plain = look(&mut harness);
    let warm = harness.renderer.image_effect_builds();
    assert!(warm.0 >= 1 && warm.1 >= 1, "the effect was built: {warm:?}");
    // A decaying flash: new parameters every update.
    let mut frames = Vec::new();
    for flash in [0.8, 0.5, 0.2] {
        harness.apply(vec![effect("shader/flash", flash, 0.0)]);
        frames.push(look(&mut harness));
    }
    assert_eq!(
        harness.renderer.image_effect_builds(),
        warm,
        "parameters alone compile nothing"
    );
    assert!(
        frames[0] != plain && frames[0] != frames[2],
        "and still draw"
    );
    // The shader's source replaced: the effect follows it.
    harness.apply(vec![define(
        "shader/flash",
        r#"
#import rusty::image::ImagePixel

fn image_effect(pixel: ImagePixel) -> vec4<f32> {
    return vec4<f32>(0.0, 1.0, 0.0, 1.0);
}
"#,
    )]);
    let green = look(&mut harness);
    assert!(
        harness.renderer.image_effect_builds().0 > warm.0,
        "it recomposed"
    );
    assert_eq!(pixel(&green, WIDTH / 2, HEIGHT / 2), [0, 255, 0]);
}

#[test]
fn parameters_flash_the_picture_and_it_samples_anywhere() {
    let mut harness = scene();
    let plain = look(&mut harness);
    harness.apply(vec![effect("shader/flash", 1.0, 0.0)]);
    let flashed = look(&mut harness);
    assert!(
        flashed
            .as_chunks::<4>()
            .0
            .iter()
            .all(|p| p[0] >= 250 && p[1] >= 250 && p[2] >= 250),
        "a full flash whitens every pixel"
    );
    harness.apply(vec![effect("shader/flash", 0.0, 0.25)]);
    let shifted = look(&mut harness);
    // The box's centre column moves left by a quarter of the picture.
    let (x, y) = (WIDTH / 2, HEIGHT / 2 + 10);
    assert_eq!(
        pixel(&shifted, x - WIDTH / 4, y),
        pixel(&plain, x, y),
        "the picture is sampled a quarter over"
    );
    let passes = harness.renderer.gpu_readout().passes;
    assert!(passes.iter().any(|timing| timing.pass == "image-effect"));
}

#[test]
fn the_effect_reads_the_worlds_depth() {
    let mut harness = scene();
    harness.apply(vec![effect("shader/sky", 0.0, 0.0)]);
    let marked = look(&mut harness);
    assert_eq!(
        pixel(&marked, WIDTH / 2, 5),
        [255, 0, 0],
        "over the sky the depth is the far plane"
    );
    assert_ne!(
        pixel(&marked, WIDTH / 2, HEIGHT - 5),
        [255, 0, 0],
        "the ground has a depth"
    );
}

/// Draw with `antialiasing` at render scale `scale`, `frames` frames, on a
/// target sized as a host sizes it: each frame's stats, and the last frame.
fn frames(
    harness: &mut Harness,
    antialiasing: Antialiasing,
    scale: f32,
    frames: usize,
) -> (Vec<render_wgpu::FrameStats>, Vec<u8>) {
    harness.apply(vec![RenderDiff::SetRendererSettings {
        settings: RendererSettingsDescriptor {
            antialiasing,
            render_scale: scale,
            ..RendererSettingsDescriptor::DEFAULT
        },
    }]);
    harness.target =
        render_wgpu::OffscreenTarget::new(&harness.gpu, WIDTH, HEIGHT, harness.renderer.samples());
    let view = camera([0.0, 1.5, 2.0], 0.0, -5.0);
    let (mut stats, mut last) = (Vec::new(), Vec::new());
    for _ in 0..frames {
        let (frame, picture) = harness.render(&view);
        stats.push(frame);
        last = picture;
    }
    (stats, last)
}

/// Mean absolute difference per channel from `reference` over the pixels
/// where it has an edge (a neighbour differs by more than 24 levels).
fn edge_difference(reference: &[u8], picture: &[u8]) -> f64 {
    let at =
        |rgba: &[u8], x: u32, y: u32, c: usize| f64::from(rgba[((y * WIDTH + x) * 4) as usize + c]);
    let (mut sum, mut count) = (0.0, 0usize);
    for y in 1..HEIGHT - 1 {
        for x in 1..WIDTH - 1 {
            let edge = (0..3).any(|c| {
                (at(reference, x + 1, y, c) - at(reference, x, y, c)).abs() > 24.0
                    || (at(reference, x, y + 1, c) - at(reference, x, y, c)).abs() > 24.0
            });
            if edge {
                for c in 0..3 {
                    sum += (at(reference, x, y, c) - at(picture, x, y, c)).abs();
                }
                count += 3;
            }
        }
    }
    assert!(count > 100, "the scene has edges: {count}");
    sum / count as f64
}

fn mean(rgba: &[u8]) -> f64 {
    rgba.iter().map(|value| f64::from(*value)).sum::<f64>() / rgba.len() as f64
}

/// An image effect and the temporal stage draw together: the stage resolves
/// the jittered frames into an output-size picture and the effect runs over
/// it. A pass-through effect leaves TAAU's anti-aliased picture, a flash
/// brightens it, the frames jitter with no reset after the first, and the
/// readout refuses nothing. At native scale and at 0.67, and under DLSS
/// where it draws. A software adapter refuses the stage and the effect
/// upscales alone.
#[test]
fn an_image_effect_draws_over_the_temporal_stage() {
    let mut harness = scene();
    if harness.gpu.is_software() {
        harness.apply(vec![effect("shader/flash", 0.0, 0.0)]);
        let (stats, picture) = frames(&mut harness, Antialiasing::Temporal, 0.67, 2);
        let readout = harness.renderer.settings_readout();
        assert_eq!(
            readout.antialiasing,
            Some(render_wgpu::SettingRefusal::SoftwareAdapter)
        );
        assert_eq!(stats[1].upscale, None, "the effect upscales");
        assert!(picture.chunks(4).any(|p| p[0] > 100), "the box drew");
        return;
    }
    for scale in [1.0, 0.67] {
        let (_, msaa) = frames(&mut scene(), Antialiasing::Msaa4, 1.0, 2);
        let (_, plain) = frames(&mut scene(), Antialiasing::Temporal, scale, 32);
        let mut aliased = scene();
        aliased.apply(vec![effect("shader/flash", 0.0, 0.0)]);
        let (_, aliased) = frames(&mut aliased, Antialiasing::Off, scale, 2);
        let mut harness = scene();
        harness.apply(vec![effect("shader/flash", 0.0, 0.0)]);
        let (stats, passed) = frames(&mut harness, Antialiasing::Temporal, scale, 32);
        let mut phases = Vec::new();
        for (index, frame) in stats.iter().enumerate() {
            assert_eq!(
                frame.upscale,
                Some(render_wgpu::UpscaleMethod::Taau),
                "{scale}: frame {index}"
            );
            let motion = frame.motion.expect("the stage draws motion");
            assert_eq!(motion.history_reset, index == 0, "{scale}: frame {index}");
            phases.push(motion.jitter_phase.expect("the views jitter"));
        }
        assert!(
            phases.windows(2).all(|pair| pair[0] != pair[1]),
            "{scale}: each frame takes the next jitter phase: {phases:?}"
        );
        let readout = harness.renderer.settings_readout();
        assert_eq!(readout.antialiasing, None, "{scale}: nothing refused");
        assert_eq!(readout.effective.antialiasing, Antialiasing::Temporal);
        let passes = harness.renderer.gpu_readout().passes;
        for pass in ["upscale", "image-effect"] {
            assert!(
                passes.iter().any(|timing| timing.pass == pass),
                "{scale}: {pass} drew"
            );
        }
        assert!(
            close(&passed, &plain),
            "{scale}: a pass-through effect leaves TAAU's picture"
        );
        let (temporal, off) = (
            edge_difference(&msaa, &passed),
            edge_difference(&msaa, &aliased),
        );
        assert!(
            temporal < off * 0.85,
            "{scale}: edges under the effect ({temporal:.2}) nearer 4x MSAA's than without the stage ({off:.2})"
        );
        harness.apply(vec![effect("shader/flash", 0.5, 0.0)]);
        let (stats, flashed) = frames(&mut harness, Antialiasing::Temporal, scale, 2);
        assert!(
            stats
                .iter()
                .all(|frame| frame.motion.is_some_and(|motion| !motion.history_reset)),
            "{scale}: a parameter change keeps the history"
        );
        assert!(
            mean(&flashed) > mean(&passed) + 20.0,
            "{scale}: the flash brightens the picture: {} to {}",
            mean(&passed),
            mean(&flashed)
        );
    }
    let (stats, plain) = frames(&mut scene(), Antialiasing::Dlss, 0.67, 32);
    if stats.last().unwrap().upscale != Some(render_wgpu::UpscaleMethod::Dlss) {
        return;
    }
    let mut harness = scene();
    harness.apply(vec![effect("shader/flash", 0.0, 0.0)]);
    let (stats, passed) = frames(&mut harness, Antialiasing::Dlss, 0.67, 32);
    assert_eq!(
        stats.last().unwrap().upscale,
        Some(render_wgpu::UpscaleMethod::Dlss)
    );
    assert!(
        close(&passed, &plain),
        "a pass-through effect leaves DLSS's picture"
    );
}
