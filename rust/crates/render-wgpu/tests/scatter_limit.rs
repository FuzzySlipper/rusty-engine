//! Scatter copies past a device's storage binding limit (#10040): on a
//! device whose bindings hold 1 MiB (16,384 copy rows, 131,072 selection
//! entries), what does not fit is refused, drawn as nothing and counted
//! (`ScatterSelectionReadout::over_limit`), and the frame still draws.

mod common;

use common::*;
use render_model::*;
use render_wgpu::{Gpu, RendererOptions};

const POST: &str = "post";
const BINDING_LIMIT: u64 = 1 << 20;
/// Copy rows a binding of `BINDING_LIMIT` holds.
const ROWS: usize = (BINDING_LIMIT / 64) as usize;

/// One device for the binary, its bindings limited.
fn limited() -> Gpu {
    static GPU: std::sync::OnceLock<Gpu> = std::sync::OnceLock::new();
    GPU.get_or_init(|| {
        Gpu::headless_with_binding_limit(BINDING_LIMIT).expect("scatter tests need a wgpu adapter")
    })
    .clone()
}

/// A post 0.3 m across and 1 m tall, standing on its origin.
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

/// `count` posts on a grid before the camera.
fn copies(count: usize) -> Vec<ScatterInstance> {
    (0..count)
        .map(|index| ScatterInstance {
            translation: [
                (index % 100) as f32 * 0.06 - 3.0,
                -1.0,
                -3.0 - (index / 100) as f32 * 0.06,
            ],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: 0.2,
            tint: [1.0; 3],
            rank: 0.5,
        })
        .collect()
}

fn patch(handle: u64, copies: Vec<ScatterInstance>, lods: usize) -> RenderDiff {
    RenderDiff::CreateScatterPatch {
        handle: RenderHandle::new(handle),
        parent: None,
        patch: ScatterPatchDescriptor {
            asset: format!("mesh/{POST}"),
            material_overrides: Vec::new(),
            instances: copies,
            fade: ScatterFade::default(),
            shadow_casting: ShadowCasting::None,
            lods: (1..=lods)
                .map(|level| ScatterLod {
                    asset: format!("mesh/{POST}"),
                    material_overrides: Vec::new(),
                    start: 10.0 * level as f32,
                    impostor: false,
                })
                .collect(),
            lod_band: 0.0,
            thinning: Default::default(),
        },
    }
}

fn harness() -> Harness {
    let mut harness = Harness::on(
        limited(),
        RendererOptions {
            default_world_lights: true,
            ..RendererOptions::default()
        },
    );
    let mut ops = vec![RenderDiff::SetBackgroundColor {
        color: [0.0, 0.0, 0.0, 1.0],
    }];
    ops.extend(coloured_mesh(POST, post(), [0.9, 0.9, 0.9, 1.0]));
    harness.apply(ops);
    harness
}

fn readout(harness: &mut Harness) -> (render_wgpu::ScatterSelectionReadout, Vec<u8>) {
    let eye = camera("eye", [0.0, 0.0, 0.0], 0.0, -20.0);
    let mut frame = Vec::new();
    for _ in 0..3 {
        harness.renderer.render_offscreen(&eye, &harness.target);
        frame = harness.target.read_rgba(&harness.gpu);
    }
    (harness.renderer.gpu_readout().scatter_selection, frame)
}

fn lit(frame: &[u8]) -> usize {
    frame
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|pixel| pixel[1] > 40)
        .count()
}

#[test]
fn copies_past_the_binding_limit_are_refused_counted_and_the_frame_draws() {
    // Two patches of 10,000 posts: the first fits the copy rows, the
    // second does not, so it draws nothing until it is made again.
    let mut harness = harness();
    harness.apply(vec![patch(20, copies(10_000), 0)]);
    let (_, alone) = readout(&mut harness);
    harness.apply(vec![patch(21, copies(10_000), 0)]);
    let (selection, frame) = readout(&mut harness);
    assert_eq!(selection.over_limit, 10_000, "{selection:?}");
    if selection.enabled {
        assert_eq!(selection.copies, 10_000, "{selection:?}");
    }
    assert!(lit(&frame) > 0);
    assert_eq!(frame, alone, "the refused patch drew");
    // A refused patch stays refused when another gives its rows back, and
    // its count goes with it.
    harness.apply(vec![RenderDiff::Destroy {
        handle: RenderHandle::new(20),
    }]);
    assert_eq!(readout(&mut harness).0.over_limit, 10_000);
    harness.apply(vec![RenderDiff::Destroy {
        handle: RenderHandle::new(21),
    }]);
    assert_eq!(readout(&mut harness).0.over_limit, 0);
    // Made again with the rows free, it draws.
    harness.apply(vec![patch(21, copies(10_000), 0)]);
    let (selection, again) = readout(&mut harness);
    assert_eq!(selection.over_limit, 0, "{selection:?}");
    assert_eq!(again, alone);
}

#[test]
fn a_patch_replaced_in_one_update_takes_the_rows_it_replaces() {
    // 10,000 posts, then in one update destroyed and made again, then
    // rebuilt as their mesh is defined anew: the rows given back are free
    // for their replacement, so nothing passes the ceiling.
    let mut harness = harness();
    harness.apply(vec![patch(20, copies(10_000), 0)]);
    let (_, alone) = readout(&mut harness);
    harness.apply(vec![
        RenderDiff::Destroy {
            handle: RenderHandle::new(20),
        },
        patch(21, copies(10_000), 0),
    ]);
    let (selection, replaced) = readout(&mut harness);
    assert_eq!(selection.over_limit, 0, "{selection:?}");
    assert_eq!(replaced, alone);
    let defined = coloured_mesh(POST, post(), [0.9, 0.9, 0.9, 1.0]).remove(1);
    assert!(matches!(defined, RenderDiff::DefineStaticMesh { .. }));
    harness.apply(vec![defined]);
    let (selection, rebuilt) = readout(&mut harness);
    assert_eq!(selection.over_limit, 0, "{selection:?}");
    assert_eq!(rebuilt, alone);
}

#[test]
fn a_patch_whose_groups_do_not_all_fit_is_refused_whole() {
    // Posts of two mesh groups take a row a group: the first group's rows
    // fit beside the posts already there, the second's do not, so the
    // patch draws nothing and counts its copies once.
    let mut harness = harness();
    let mut split = post();
    split.groups = vec![
        MeshGroupDescriptor {
            material_slot: 0,
            start: 0,
            count: 18,
        },
        MeshGroupDescriptor {
            material_slot: 1,
            start: 18,
            count: 18,
        },
    ];
    let material = format!("material/{POST}");
    harness.apply(vec![
        RenderDiff::DefineStaticMesh {
            asset: StaticMeshAsset {
                asset: "mesh/split".into(),
                payload: split,
                material_slots: vec![
                    MeshMaterialSlot {
                        slot: 0,
                        material: material.clone(),
                    },
                    MeshMaterialSlot { slot: 1, material },
                ],
                collision: MeshCollisionPolicy::VisualOnly,
            },
        },
        patch(20, copies(6_000), 0),
    ]);
    let (_, alone) = readout(&mut harness);
    let mut split = patch(21, copies(6_000), 0);
    if let RenderDiff::CreateScatterPatch { patch, .. } = &mut split {
        patch.asset = "mesh/split".into();
    }
    harness.apply(vec![split]);
    let (selection, frame) = readout(&mut harness);
    assert_eq!(selection.over_limit, 6_000, "{selection:?}");
    assert_eq!(frame, alone, "a part of the refused patch drew");
    harness.apply(vec![RenderDiff::Destroy {
        handle: RenderHandle::new(21),
    }]);
    assert_eq!(readout(&mut harness).0.over_limit, 0);
}

#[test]
fn a_run_past_the_selections_entries_is_left_out_and_counted() {
    // Every copy row a binding holds, at four levels: two entries a level
    // take every entry a binding holds, and the view regions before them
    // leave less, so the selection leaves the run out.
    let mut harness = harness();
    harness.apply(vec![patch(20, copies(ROWS), 3)]);
    let selection = readout(&mut harness).0;
    if !selection.enabled {
        return;
    }
    assert_eq!(selection.over_limit, ROWS as u64, "{selection:?}");
    assert_eq!(selection.copies, 0, "{selection:?}");
    // At one level it is selected.
    harness.apply(vec![
        RenderDiff::Destroy {
            handle: RenderHandle::new(20),
        },
        patch(21, copies(ROWS), 0),
    ]);
    let (selection, frame) = readout(&mut harness);
    assert_eq!(selection.over_limit, 0, "{selection:?}");
    assert_eq!(selection.copies, ROWS as u32, "{selection:?}");
    assert!(lit(&frame) > 0);
}
