//! Projected decals (`decals.rs`, `rusty::decals`, `CameraView.SetDecal`):
//! a decal lays its atlas region over the opaque world inside its box,
//! wrapping flat ground, a slope, a step's edge and a curved mesh; its angle
//! falloff keeps the projection off faces that turn away; vegetation copies
//! and animated meshes are passed by unless the decal asks; a decal fades
//! out between its fade times; a nearest-mipmapped atlas stays crisp; and a
//! normal atlas bends the light. Same harness as `tests/screenshots.rs`.

mod support;

use std::path::PathBuf;

use asset_import::{import_animated_glb_asset, ImportContext, SourceUri};
use render_host_contracts::RendererCompositionCamera;
use render_model::*;
use render_wgpu::{encode_png, RendererOptions};
use support::*;

const ATLAS: &str = "texture/decal-atlas";
const NORMALS: &str = "texture/decal-normals";

/// Grey ground with a 30-degree ramp rising toward +x, a raised platform
/// whose edge faces +x, a sphere, and short posts scattered at the back,
/// lit by a sun and a hemisphere fill.
fn scene() -> Harness {
    let mut harness = Harness::new(RendererOptions {
        default_world_lights: false,
        ..RendererOptions::default()
    });
    let rise = 30.0f32.to_radians();
    let (run, height) = (2.0, 2.0 * rise.tan());
    let normal = [-rise.sin(), rise.cos(), 0.0];
    let ramp = payload(
        vec![
            4.0,
            0.0,
            -1.0,
            4.0 + run,
            height,
            -1.0,
            4.0 + run,
            height,
            1.0,
            4.0,
            0.0,
            1.0,
        ],
        [normal; 4].concat(),
        vec![0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0],
        vec![0, 2, 1, 0, 3, 2],
        &[(0, 6)],
    );
    harness.apply(vec![
        RenderDiff::DefineMaterial {
            material: material("material/stone", [0.6, 0.6, 0.6, 1.0], None),
        },
        static_mesh(
            "mesh/ground",
            box_mesh([-20.0, -1.0, -20.0], [20.0, 0.0, 20.0], |_| 0),
            "material/stone",
        ),
        static_mesh("mesh/ramp", ramp, "material/stone"),
        static_mesh(
            "mesh/platform",
            box_mesh([-6.0, 0.0, -1.0], [-3.0, 0.5, 1.0], |_| 0),
            "material/stone",
        ),
        static_mesh("mesh/sphere", sphere(1.0, 24), "material/stone"),
        static_mesh(
            "mesh/post",
            box_mesh([-0.2, 0.0, -0.2], [0.2, 0.3, 0.2], |_| 0),
            "material/stone",
        ),
        instance(20, None, "mesh/ground", transform([0.0; 3], 0.0, [1.0; 3])),
        instance(21, None, "mesh/ramp", transform([0.0; 3], 0.0, [1.0; 3])),
        instance(
            22,
            None,
            "mesh/platform",
            transform([0.0; 3], 0.0, [1.0; 3]),
        ),
        instance(
            23,
            None,
            "mesh/sphere",
            transform([0.0, 1.0, -6.0], 0.0, [1.0; 3]),
        ),
        group(24, None, transform([0.0; 3], 0.0, [1.0; 3])),
        RenderDiff::CreateScatterPatch {
            handle: RenderHandle::new(25),
            parent: Some(RenderHandle::new(24)),
            patch: ScatterPatchDescriptor {
                asset: "mesh/post".into(),
                material_overrides: Vec::new(),
                instances: (0..9)
                    .map(|index| ScatterInstance {
                        translation: [-1.0 + (index % 3) as f32, 0.0, -12.0 + (index / 3) as f32],
                        rotation: [0.0, 0.0, 0.0, 1.0],
                        scale: 1.0,
                        tint: [1.0; 3],
                        rank: 0.5,
                    })
                    .collect(),
                fade: ScatterFade::default(),
                shadow_casting: ShadowCasting::None,
                lods: Vec::new(),
                lod_band: 0.0,
                thinning: Default::default(),
            },
        },
        RenderDiff::CreateLight {
            handle: RenderHandle::new(10),
            parent: None,
            light: LightDescriptor::Directional {
                color: [1.0; 3],
                intensity: 1.5,
                enabled: true,
                direction: [-0.3, -0.8, -0.4],
                range: None,
                shadow_intent: LightShadowIntent::Disabled,
                shadow: Default::default(),
            },
        },
        RenderDiff::CreateLight {
            handle: RenderHandle::new(11),
            parent: None,
            light: LightDescriptor::Hemisphere {
                color: [0.6, 0.65, 0.7],
                ground_color: [0.3, 0.3, 0.3],
                intensity: 0.5,
                enabled: true,
            },
        },
    ]);
    harness
}

/// A UV sphere of `radius` about its origin.
fn sphere(radius: f32, segments: u32) -> MeshPayloadDescriptor {
    let (mut positions, mut normals, mut uvs, mut indices) = (vec![], vec![], vec![], vec![]);
    let rings = segments / 2;
    for ring in 0..=rings {
        let theta = std::f32::consts::PI * ring as f32 / rings as f32;
        for step in 0..=segments {
            let phi = std::f32::consts::TAU * step as f32 / segments as f32;
            let normal = [
                theta.sin() * phi.cos(),
                theta.cos(),
                theta.sin() * phi.sin(),
            ];
            positions.extend(normal.map(|value| value * radius));
            normals.extend(normal);
            uvs.extend([step as f32 / segments as f32, ring as f32 / rings as f32]);
        }
    }
    let across = segments + 1;
    for ring in 0..rings {
        for step in 0..segments {
            let (a, b) = (ring * across + step, (ring + 1) * across + step);
            indices.extend([a, a + 1, b, a + 1, b + 1, b]);
        }
    }
    let count = indices.len() as u32;
    payload(positions, normals, uvs, indices, &[(0, count)])
}

/// A level decal of `size` (x, height, z) at `center`, red without an
/// atlas.
fn decal(center: [f32; 3], size: [f32; 3]) -> DecalDescriptor {
    DecalDescriptor {
        center,
        rotation: [0.0, 0.0, 0.0, 1.0],
        size,
        uv_rect: [0.0, 0.0, 1.0, 1.0],
        color: [0.9, 0.05, 0.05, 1.0],
        roughness: 0.0,
        normal_strength: 0.0,
        angle_falloff: false,
        on_vegetation: false,
        on_animated: false,
        fade_start: 0.0,
        fade_end: 0.0,
    }
}

fn place(id: u64, decal: DecalDescriptor) -> RenderDiff {
    RenderDiff::SetDecal { id, decal }
}

/// Straight down from `height` over (x, z).
fn above(x: f64, height: f64, z: f64) -> RendererCompositionCamera {
    camera([x, height, z], 0.0, -89.0)
}

/// Toward -x at the platform's edge, from just above the ground.
fn at_the_edge() -> RendererCompositionCamera {
    camera([-0.5, 0.35, 0.0], -90.0, -5.0)
}

fn render(harness: &mut Harness, view: &RendererCompositionCamera) -> Vec<u8> {
    harness.render(view).1
}

fn rgb(rgba: &[u8], x: u32, y: u32) -> [f64; 3] {
    let at = ((y * WIDTH + x) * 4) as usize;
    [rgba[at], rgba[at + 1], rgba[at + 2]].map(f64::from)
}

/// How red a rectangle is: its mean red less its mean green.
fn redness(rgba: &[u8], x: std::ops::Range<u32>, y: std::ops::Range<u32>) -> f64 {
    let (mut sum, mut count) = (0.0, 0.0);
    for row in y {
        for column in x.clone() {
            let [r, g, _] = rgb(rgba, column, row);
            sum += r - g;
            count += 1.0;
        }
    }
    sum / count
}

fn centre(rgba: &[u8]) -> f64 {
    redness(
        rgba,
        WIDTH / 2 - 6..WIDTH / 2 + 6,
        HEIGHT / 2 - 6..HEIGHT / 2 + 6,
    )
}

/// Mean absolute difference of two renders, over the whole frame.
fn change(a: &[u8], b: &[u8]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(a, b)| f64::from(a.abs_diff(*b)))
        .sum::<f64>()
        / a.len() as f64
}

#[test]
fn a_decal_lays_over_the_ground_inside_its_box_and_removing_it_restores_the_picture() {
    let mut harness = scene();
    let view = above(0.0, 3.0, 2.0);
    let bare = render(&mut harness, &view);
    harness.apply(vec![place(1, decal([0.0, 0.0, 2.0], [1.0, 0.5, 1.0]))]);
    let marked = render(&mut harness, &view);
    assert!(
        centre(&marked) > 100.0,
        "the decal shows: {:.1}",
        centre(&marked)
    );
    // Its box is a metre across: 3 m up, the frame's corners are outside it.
    let corner = redness(&marked, 0..20, 0..20);
    assert!(corner.abs() < 2.0, "outside the box is bare: {corner:.1}");
    // A decal whose box stands above the ground lays nothing.
    harness.apply(vec![place(1, decal([0.0, 1.0, 2.0], [1.0, 0.5, 1.0]))]);
    assert_eq!(render(&mut harness, &view), bare);
    harness.apply(vec![RenderDiff::RemoveDecal { id: 1 }]);
    assert_eq!(render(&mut harness, &view), bare);
}

#[test]
fn decals_wrap_a_slope_and_a_curved_mesh() {
    let mut harness = scene();
    // The ramp's middle rises 0.58 m; the box reaches above and below it.
    harness.apply(vec![
        place(1, decal([5.0, 0.58, 0.0], [1.0, 1.5, 1.0])),
        // Over the sphere's crown, 0.6 m deep: its top cap.
        place(2, decal([0.0, 1.9, -6.0], [1.2, 0.6, 1.2])),
    ]);
    let ramp = render(&mut harness, &above(5.0, 3.5, 0.0));
    assert!(
        centre(&ramp) > 80.0,
        "the ramp under the box is marked: {:.1}",
        centre(&ramp)
    );
    let crown = render(&mut harness, &above(0.0, 4.5, -6.0));
    assert!(
        centre(&crown) > 80.0,
        "the sphere's crown is marked: {:.1}",
        centre(&crown)
    );
    // The sphere's side, below the box, is not: looked at level.
    let side = render(&mut harness, &camera([0.0, 0.6, -3.0], 0.0, 0.0));
    let below = redness(
        &side,
        WIDTH / 2 - 10..WIDTH / 2 + 10,
        HEIGHT / 2 - 4..HEIGHT / 2 + 4,
    );
    assert!(
        below.abs() < 2.0,
        "below the box the sphere is bare: {below:.1}"
    );
}

#[test]
fn the_angle_falloff_keeps_the_projection_off_a_steps_riser() {
    let mut harness = scene();
    // A box over the platform's edge, reaching down to the ground.
    let mut over_edge = decal([-3.0, 0.25, 0.0], [1.2, 1.0, 1.2]);
    let view = at_the_edge();
    let bare = render(&mut harness, &view);
    let riser = |rgba: &[u8]| {
        redness(
            rgba,
            WIDTH / 2 - 8..WIDTH / 2 + 8,
            HEIGHT / 2 - 4..HEIGHT / 2 + 4,
        )
    };
    harness.apply(vec![place(1, over_edge)]);
    let smeared = render(&mut harness, &view);
    over_edge.angle_falloff = true;
    harness.apply(vec![place(1, over_edge)]);
    let kept = render(&mut harness, &view);
    assert!(
        riser(&smeared) > riser(&bare) + 60.0,
        "without the falloff the riser takes the projection: {:.1} to {:.1}",
        riser(&bare),
        riser(&smeared)
    );
    assert!(
        (riser(&kept) - riser(&bare)).abs() < 2.0,
        "with it the riser stays bare: {:.1} to {:.1}",
        riser(&bare),
        riser(&kept)
    );
    // The platform's top and the ground before it are marked either way.
    let top = render(&mut harness, &above(-3.0, 3.0, 0.0));
    let (left, right) = (
        redness(
            &top,
            WIDTH / 2 - 30..WIDTH / 2 - 10,
            HEIGHT / 2 - 4..HEIGHT / 2 + 4,
        ),
        redness(
            &top,
            WIDTH / 2 + 10..WIDTH / 2 + 30,
            HEIGHT / 2 - 4..HEIGHT / 2 + 4,
        ),
    );
    assert!(
        left > 80.0 && right > 80.0,
        "the top and the ground: {left:.1}, {right:.1}"
    );
}

#[test]
fn vegetation_copies_are_passed_by_unless_the_decal_asks() {
    let mut harness = scene();
    let view = above(0.0, 2.5, -11.0);
    let bare = render(&mut harness, &view);
    let mut over_posts = decal([0.0, 0.2, -11.0], [3.4, 1.0, 3.4]);
    harness.apply(vec![place(1, over_posts)]);
    let passed = render(&mut harness, &view);
    over_posts.on_vegetation = true;
    harness.apply(vec![place(1, over_posts)]);
    let laid = render(&mut harness, &view);
    // The middle post's top, under the camera.
    let post = |rgba: &[u8]| centre(rgba);
    assert!(
        (post(&passed) - post(&bare)).abs() < 2.0 && post(&laid) > 80.0,
        "the post's top: bare {:.1}, passed by {:.1}, laid over {:.1}",
        post(&bare),
        post(&passed),
        post(&laid)
    );
    // The ground between the posts is marked either way.
    let between = |rgba: &[u8]| {
        redness(
            rgba,
            WIDTH / 2 + 20..WIDTH / 2 + 30,
            HEIGHT / 2 - 3..HEIGHT / 2 + 3,
        )
    };
    assert!(
        between(&passed) > 80.0,
        "between the posts: {:.1}",
        between(&passed)
    );
}

#[test]
fn animated_meshes_are_passed_by_unless_the_decal_asks() {
    let mut harness = Harness::new(RendererOptions::default());
    let path = "csharp-joint-attachments/content/body.glb";
    let bytes = std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../fixtures")
            .join(path),
    )
    .expect("the body fixture");
    let imported = import_animated_glb_asset(
        &SourceUri::RelativePath(path.to_owned()),
        &bytes,
        &ImportContext::default(),
    )
    .assets
    .expect("the body is admitted")
    .animated_mesh;
    let hash = imported.content_hash.clone().expect("content hash");
    harness.resources.insert(
        &format!(
            "animated-mesh-resource/{}",
            hash.trim_start_matches("sha256:")
        ),
        bytes,
    );
    // The body's own material is unlit, which decals pass by: draw it lit.
    harness.apply(vec![
        RenderDiff::DefineMaterial {
            material: material("material/skin", [0.8, 0.7, 0.6, 1.0], None),
        },
        RenderDiff::DefineAnimatedMesh {
            asset: imported.clone(),
        },
        RenderDiff::CreateAnimatedMeshInstance {
            handle: RenderHandle::new(1),
            parent: None,
            instance: AnimatedMeshInstanceDescriptor {
                inspection: AnimatedMeshInspection::default(),
                asset: imported.asset.clone(),
                transform: Transform::IDENTITY,
                visible: true,
                material_overrides: vec![MeshMaterialSlot {
                    slot: 0,
                    material: "material/skin".into(),
                }],
                playback: Some(AnimatedMeshPlaybackCommand::Sample {
                    clip: "run".into(),
                    normalized_time: 0.5,
                }),
                metadata: RenderMetadata::default(),
                layer: RenderLayer::Scene,
                shadow_casting: Default::default(),
            },
        },
    ]);
    let view = camera([4.0, 3.0, 6.0], -33.690_067, -11.750_674);
    let bare = render(&mut harness, &view);
    let mut over_body = decal([0.0, 1.0, 0.0], [3.0, 3.0, 3.0]);
    harness.apply(vec![place(1, over_body)]);
    let passed = render(&mut harness, &view);
    over_body.on_animated = true;
    harness.apply(vec![place(1, over_body)]);
    let laid = render(&mut harness, &view);
    assert_eq!(passed, bare, "the body is passed by");
    assert!(
        change(&laid, &bare) > 0.3,
        "asked, the decal lays over it: {:.2}",
        change(&laid, &bare)
    );
}

#[test]
fn a_decal_fades_out_between_its_fade_times() {
    let mut harness = scene();
    let view = above(0.0, 3.0, 2.0);
    let bare = render(&mut harness, &view);
    let mut fading = decal([0.0, 0.0, 2.0], [1.0, 0.5, 1.0]);
    fading.fade_start = 10.0;
    fading.fade_end = 12.0;
    harness.apply(vec![place(1, fading)]);
    let mut seen = Vec::new();
    for seconds in [9.0, 11.0, 12.5] {
        harness.renderer.set_animation_time(seconds);
        seen.push(render(&mut harness, &view));
    }
    let (whole, half) = (centre(&seen[0]), centre(&seen[1]));
    assert!(
        whole > 100.0 && half > 20.0 && half < whole * 0.75,
        "whole before its fade, part way during it: {whole:.1}, {half:.1}"
    );
    assert_eq!(seen[2], bare, "gone after it");
}

/// A 4×4 atlas of black and white texels, filtered `filter`.
fn checker_atlas(harness: &mut Harness, filter: TextureFilter) -> RenderDiff {
    let rgba = checker(4, [255, 255, 255, 255], [0, 0, 0, 255]);
    let png = encode_png(4, 4, &rgba).expect("encode the atlas");
    let texture = TextureDescriptor::admit_png_rgba8_resource(
        ATLAS.to_owned(),
        &png,
        filter,
        TextureWrap::Clamp,
        1,
    )
    .expect("admit the atlas");
    if let Some(TexturePayloadSource::Resource { resource }) =
        texture.payload.as_ref().map(|payload| &payload.source)
    {
        harness.resources.insert(resource, png);
    }
    RenderDiff::DefineTexture { texture }
}

/// Distinct brightnesses in the frame's middle.
fn distinct(rgba: &[u8]) -> usize {
    let mut seen = std::collections::BTreeSet::new();
    for y in HEIGHT / 2 - 40..HEIGHT / 2 + 40 {
        for x in WIDTH / 2 - 40..WIDTH / 2 + 40 {
            seen.insert(rgba[((y * WIDTH + x) * 4) as usize]);
        }
    }
    seen.len()
}

#[test]
fn a_nearest_mipmapped_atlas_stays_crisp() {
    let mut white = decal([0.0, 0.0, 2.0], [1.0, 0.5, 1.0]);
    white.color = [1.0; 4];
    let view = above(0.0, 1.2, 2.0);
    let mut counts = Vec::new();
    for filter in [TextureFilter::NearestMipmapped, TextureFilter::Linear] {
        let mut harness = scene();
        let atlas = checker_atlas(&mut harness, filter);
        harness.apply(vec![
            atlas,
            RenderDiff::SetDecalAtlas {
                atlas: Some(DecalAtlasDescriptor {
                    color: ATLAS.into(),
                    normals: None,
                }),
            },
            place(1, white),
        ]);
        counts.push(distinct(&render(&mut harness, &view)));
    }
    assert!(
        counts[0] * 4 < counts[1],
        "texels stay whole, nearest: {} brightnesses, linear: {}",
        counts[0],
        counts[1]
    );
}

#[test]
fn a_normal_atlas_bends_the_light() {
    let mut harness = scene();
    // Normals leaning toward the box's +x, the sun's side.
    let lean = [230u8, 128, 180, 255].repeat(16);
    let png = encode_png(4, 4, &lean).expect("encode the normals");
    let texture = TextureDescriptor::admit_png_rgba8_resource(
        NORMALS.to_owned(),
        &png,
        TextureFilter::Linear,
        TextureWrap::Clamp,
        1,
    )
    .expect("admit the normals");
    if let Some(TexturePayloadSource::Resource { resource }) =
        texture.payload.as_ref().map(|payload| &payload.source)
    {
        harness.resources.insert(resource, png);
    }
    let atlas = checker_atlas(&mut harness, TextureFilter::Linear);
    harness.apply(vec![
        atlas,
        RenderDiff::DefineTexture { texture },
        RenderDiff::SetDecalAtlas {
            atlas: Some(DecalAtlasDescriptor {
                color: ATLAS.into(),
                normals: Some(NORMALS.into()),
            }),
        },
    ]);
    let mut flat = decal([0.0, 0.0, 2.0], [1.0, 0.5, 1.0]);
    flat.uv_rect = [0.0, 0.0, 0.25, 0.25];
    flat.color = [1.0; 4];
    let view = above(0.0, 3.0, 2.0);
    harness.apply(vec![place(1, flat)]);
    let level = render(&mut harness, &view);
    flat.normal_strength = 1.0;
    harness.apply(vec![place(1, flat)]);
    let bent = render(&mut harness, &view);
    let [before, after] = [&level, &bent].map(|rgba| rgb(rgba, WIDTH / 2, HEIGHT / 2)[0]);
    assert!(
        (before - after).abs() > 8.0,
        "the light on it changes: {before} to {after}"
    );
}
