use super::*;
use render_presentation::PresentationWorld;

const FEATURES: &str =
    include_str!("../../../../animation-parts-rig/tests/fixtures/features.model.json");
const PHYSICS: &str =
    include_str!("../../../../animation-parts-rig/tests/fixtures/physics.model.json");
/// Any page will do: the services admit it and the renderer draws it.
const PAGE: &[u8] = include_bytes!("../../../../render-wgpu/tests/parts_rigs/squash.png");
const MODEL: &str = "people/features/skeleton.model.json";
const OBJECT: u64 = 7;
const STEP: f64 = 0.1;

fn bridge() -> RuntimeAppearanceBridge {
    let content = BTreeMap::from([
        (
            MODEL.to_owned(),
            crate::ContentBytes::from(FEATURES.as_bytes()),
        ),
        (
            "people/features/skeleton.png".to_owned(),
            crate::ContentBytes::from(PAGE),
        ),
        (
            "people/physics/skeleton.model.json".to_owned(),
            crate::ContentBytes::from(PHYSICS.as_bytes()),
        ),
        (
            "people/physics/skeleton.png".to_owned(),
            crate::ContentBytes::from(PAGE),
        ),
    ]);
    RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), content)
}

fn utf8(value: &str) -> NativeUtf8Slice {
    NativeUtf8Slice {
        bytes: value.as_ptr(),
        len: value.len(),
    }
}

fn load(bridge: &mut RuntimeAppearanceBridge) -> NativePartsRigInfo {
    bridge
        .load_parts_rig(NativePartsRigLoadRequest {
            path: utf8(MODEL),
            filter: NativeTextureFilter::Linear,
        })
        .expect("the rig loads")
}

fn appearance(
    bridge: &mut RuntimeAppearanceBridge,
    views: &[NativePartsRigView],
) -> Result<NativeAppearanceHandle, CsharpEngineServicesError> {
    unsafe {
        bridge.create_parts_rig_appearance(&NativePartsRigAppearanceRequest {
            views: views.as_ptr(),
            views_len: views.len(),
            scale: 0.01,
            size_mode: NativeSpriteSizeMode::World,
            billboard: NativeBillboardMode::Cylindrical,
            render_order: 0,
            depth: NativeSpriteDepthPolicy::Default,
            tint: NativeColor {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
            material: NativeSpriteMaterialDescriptor::default(),
            texel_snap: 0.0,
        })
    }
}

fn fact(appearance: NativeAppearanceHandle, layer: NativeRenderLayer) -> NativeAppearanceFact {
    placed(OBJECT, appearance, layer, 0.0)
}

fn placed(
    object_id: u64,
    appearance: NativeAppearanceHandle,
    layer: NativeRenderLayer,
    x: f32,
) -> NativeAppearanceFact {
    NativeAppearanceFact {
        object_id,
        has_parent_object: false,
        parent_object_id: 0,
        transform: NativeTransform {
            translation: NativeVec3 { x, y: 0.0, z: 0.0 },
            rotation: NativeQuat {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 1.0,
            },
            scale: NativeVec3 {
                x: 1.0,
                y: 1.0,
                z: 1.0,
            },
        },
        appearance,
        visible: true,
        layer,
        shadow_casting: Default::default(),
    }
}

fn play(clip: &str, crossfade: f32, speed: f32) -> NativePartsRigPlayRequest {
    NativePartsRigPlayRequest {
        object_id: OBJECT,
        clip: utf8(clip),
        looping: true,
        speed,
        crossfade_seconds: crossfade,
    }
}

/// Ends the call, settles it into `world` `advance` seconds on, and gives
/// the views of the poses it published.
fn end(
    bridge: &mut RuntimeAppearanceBridge,
    world: &mut PresentationWorld,
    advance: f64,
) -> Vec<u32> {
    let mut call = bridge.take_staged_call();
    for output in std::mem::take(&mut call.outputs) {
        if let RuntimeAppearanceCallOutput::Frame(frame) = output {
            world.apply(frame).expect("the world admits the call");
        }
    }
    world.advance_elapsed(advance);
    let settled = evaluate_poses(&mut call, world);
    let views = settled
        .poses
        .ops
        .iter()
        .filter_map(|op| match op {
            RenderDiff::SetPartsRigPose { view, .. } => Some(*view),
            _ => None,
        })
        .collect();
    world
        .apply(settled.poses)
        .expect("the world retains the poses");
    bridge.commit(call);
    views
}

fn update(step: u64) -> NativeProductUpdateFacts {
    NativeProductUpdateFacts {
        admitted_step_count: 1,
        ..realtime_sprite_update(step, STEP)
    }
}

fn read(bridge: &RuntimeAppearanceBridge) -> NativePartsRigReadout {
    bridge
        .staged_ref()
        .unwrap()
        .state
        .parts_rigs
        .read(OBJECT)
        .unwrap()
}

fn events(bridge: &mut RuntimeAppearanceBridge) -> Vec<(String, f64, i64)> {
    let result = bridge.read_parts_rig_events().unwrap();
    if result.events_len == 0 {
        return Vec::new();
    }
    unsafe { std::slice::from_raw_parts(result.events, result.events_len) }
        .iter()
        .map(|event| {
            assert_eq!(event.object_id, OBJECT);
            let name = &event.name.bytes[..event.name.len as usize];
            (
                String::from_utf8(name.to_vec()).unwrap(),
                event.clip_seconds,
                event.int_value,
            )
        })
        .collect()
}

fn scene_vertices(bridge: &RuntimeAppearanceBridge, view: usize) -> Vec<[f32; 2]> {
    bridge.staged_ref().unwrap().state.parts_rigs.instances[&OBJECT].views[view]
        .pose
        .vertices
        .clone()
}

fn drive(bone: &str) -> NativePartsRigDriveRequest {
    NativePartsRigDriveRequest {
        object_id: OBJECT,
        bone: utf8(bone),
        channel: NativePartsRigBoneChannel::Rotation,
        held: true,
        value: 30.0,
    }
}

#[test]
fn views_share_one_clip_time_and_each_view_poses_once_per_change() {
    let (mut bridge, mut world) = (bridge(), PresentationWorld::default());
    bridge.begin_call();
    let front = load(&mut bridge);
    let side = load(&mut bridge);
    assert_eq!((front.animations, front.skins, front.pages), (3, 2, 1));
    let figure = appearance(
        &mut bridge,
        &[
            NativePartsRigView {
                rig: front.rig,
                heading: 0.0,
                mirror: false,
            },
            NativePartsRigView {
                rig: side.rig,
                heading: std::f32::consts::FRAC_PI_2,
                mirror: true,
            },
        ],
    )
    .unwrap();
    unsafe { bridge.stage_snapshot(&fact(figure, NativeRenderLayer::Scene), 1) }.unwrap();
    assert_eq!(
        end(&mut bridge, &mut world, 0.0),
        [0, 1],
        "each view's setup pose"
    );
    bridge.begin_call();
    assert!(read(&bridge).posed);
    assert!(!read(&bridge).playing);
    assert_eq!(end(&mut bridge, &mut world, STEP), [0u32; 0], "a held pose");

    // Both views play from the same time, at the object's speed.
    bridge.begin_call();
    bridge.play_parts_rig(play("a", 0.0, 2.0)).unwrap();
    assert_eq!(end(&mut bridge, &mut world, 0.0), [0, 1]);
    bridge.begin_call();
    assert_eq!(end(&mut bridge, &mut world, STEP), [0, 1]);
    bridge.begin_call();
    let readout = read(&bridge);
    assert!(readout.playing);
    assert_eq!(&readout.clip.bytes[..readout.clip.len as usize], b"a");
    assert!((readout.clip_seconds - 2.0 * STEP).abs() < 1e-9);
    {
        let scene = &bridge.staged_ref().unwrap().state.parts_rigs;
        let instance = &scene.instances[&OBJECT];
        assert_eq!(
            instance.views[0].pose.animation_time,
            instance.views[1].pose.animation_time
        );
        assert_eq!(
            instance.views[0].pose.vertices, instance.views[1].pose.vertices,
            "one rig twice: the same pose"
        );
        // Each camera reads the bone in the view it draws, where it draws
        // it: from the front, view 0 on a plane facing +z; from +x, the
        // mirrored view 1 on a plane facing +x.
        let camera = |eye: [f32; 3], forward: [f32; 3]| PartsRigCamera {
            eye,
            right: [0.0; 3],
            up: [0.0; 3],
            forward,
            orthographic: false,
        };
        let front = scene
            .read_bone(OBJECT, "w1", &camera([0.0, 1.0, 5.0], [0.0, 0.0, -1.0]))
            .unwrap();
        let side = scene
            .read_bone(OBJECT, "w1", &camera([5.0, 1.0, 0.0], [-1.0, 0.0, 0.0]))
            .unwrap();
        assert_eq!((front.view, side.view), (0, 1));
        let [x, y] = [front.position.x, front.position.y];
        assert_ne!(x, 0.0);
        let near = |a: NativeVec3, b: [f32; 3]| {
            (a.x - b[0]).abs() + (a.y - b[1]).abs() + (a.z - b[2]).abs() < 1e-5
        };
        assert!(
            near(front.world, [x * 0.01, y * 0.01, 0.0]),
            "{:?}",
            front.world
        );
        // Mirrored and turned to face +x: rig x runs along +z.
        assert!(
            near(side.world, [0.0, y * 0.01, x * 0.01]),
            "{:?}",
            side.world
        );
    }

    // Refusals leave it playing.
    assert_eq!(
        bridge
            .play_parts_rig(play("walk", 0.0, 1.0))
            .unwrap_err()
            .code(),
        "CSHARP_PARTS_RIG_CLIP"
    );
    assert_eq!(
        bridge
            .drive_parts_rig_bone(drive("turn"))
            .unwrap_err()
            .code(),
        "CSHARP_PARTS_RIG_BONE"
    );
    assert_eq!(
        bridge.destroy_parts_rig(front.rig).unwrap_err().code(),
        "CSHARP_PARTS_RIG_IN_USE"
    );
    // A driven bone moves the pose without moving the clip.
    let before = scene_vertices(&bridge, 0);
    bridge.drive_parts_rig_bone(drive("base")).unwrap();
    assert_eq!(end(&mut bridge, &mut world, 0.0), [0, 1]);
    bridge.begin_call();
    assert_ne!(scene_vertices(&bridge, 0), before);
    assert!((read(&bridge).clip_seconds - 2.0 * STEP).abs() < 1e-9);
    end(&mut bridge, &mut world, 0.0);

    // Published again on another layer, the object's node is made again
    // and goes on playing where it was.
    bridge.begin_call();
    unsafe { bridge.stage_snapshot(&fact(figure, NativeRenderLayer::Viewmodel), 1) }.unwrap();
    assert_eq!(
        end(&mut bridge, &mut world, STEP),
        [0, 1],
        "the new node's poses"
    );
    bridge.begin_call();
    assert!((read(&bridge).clip_seconds - 4.0 * STEP).abs() < 1e-9);

    // Removed, it is forgotten; then the rigs can go.
    unsafe { bridge.stage_snapshot(std::ptr::null(), 0) }.unwrap();
    end(&mut bridge, &mut world, STEP);
    bridge.begin_call();
    assert_eq!(
        bridge
            .play_parts_rig(play("a", 0.0, 1.0))
            .unwrap_err()
            .code(),
        "CSHARP_PARTS_RIG_OBJECT"
    );
    bridge.destroy_appearance(figure).unwrap();
    bridge.destroy_parts_rig(front.rig).unwrap();
    bridge.destroy_parts_rig(side.rig).unwrap();
    let state = &bridge.staged_ref().unwrap().state;
    assert!(state.parts_rigs.loaded.is_empty());
    assert!(state.parts_rigs.rigs.is_empty());
    bridge.end_call();
}

#[test]
fn events_a_clip_passes_are_read_in_the_next_update() {
    let (mut bridge, mut world) = (bridge(), PresentationWorld::default());
    bridge.begin_call();
    let rig = load(&mut bridge);
    let figure = appearance(
        &mut bridge,
        &[NativePartsRigView {
            rig: rig.rig,
            heading: 0.0,
            mirror: false,
        }],
    )
    .unwrap();
    unsafe { bridge.stage_snapshot(&fact(figure, NativeRenderLayer::Scene), 1) }.unwrap();
    end(&mut bridge, &mut world, 0.0);
    bridge.begin_call();
    bridge.play_parts_rig(play("a", 0.0, 1.0)).unwrap();
    end(&mut bridge, &mut world, 0.0);
    // `a` keys `step` at 0 and `cue` (int 7) at 0.5.
    bridge.begin_update_call(update(1));
    assert_eq!(events(&mut bridge), [("step".to_owned(), 0.0, 0)]);
    end(&mut bridge, &mut world, 0.6);
    // A call that is not an update keeps them for the next.
    bridge.begin_call();
    assert_eq!(events(&mut bridge), [("step".to_owned(), 0.0, 0)]);
    end(&mut bridge, &mut world, 0.0);
    bridge.begin_update_call(update(2));
    assert_eq!(events(&mut bridge), [("cue".to_owned(), 0.5, 7)]);
    end(&mut bridge, &mut world, 0.0);
    bridge.begin_update_call(update(3));
    assert_eq!(events(&mut bridge), []);
    bridge.end_call();
}

#[test]
fn a_document_that_is_not_a_rig_c_model_is_refused_by_name() {
    let mut bridge = RuntimeAppearanceBridge::new(
        RuntimeAppearanceCatalog::default(),
        BTreeMap::from([(
            "rig/skeleton.model.json".to_owned(),
            crate::ContentBytes::from(&br#"{"spec":"rigc-compiled/2"}"#[..]),
        )]),
    );
    bridge.begin_call();
    let error = bridge
        .load_parts_rig(NativePartsRigLoadRequest {
            path: utf8("rig/skeleton.model.json"),
            filter: NativeTextureFilter::Nearest,
        })
        .unwrap_err();
    assert_eq!(error.code(), "CSHARP_PARTS_RIG_MODEL");
    assert!(
        error.detail().contains("rigc-compiled"),
        "{}",
        error.detail()
    );
    bridge.end_call();
}

/// An object carried across the world swings its rig's physics, in each of
/// its views; one left standing beside it does not swing.
#[test]
fn a_moving_object_swings_the_physics_of_each_view() {
    const STILL: u64 = OBJECT + 1;
    let (mut bridge, mut world) = (bridge(), PresentationWorld::default());
    bridge.begin_call();
    let rig = bridge
        .load_parts_rig(NativePartsRigLoadRequest {
            path: utf8("people/physics/skeleton.model.json"),
            filter: NativeTextureFilter::Linear,
        })
        .expect("the physics rig loads");
    let view = |mirror| NativePartsRigView {
        rig: rig.rig,
        heading: 0.0,
        mirror,
    };
    let look = appearance(&mut bridge, &[view(false), view(true)]).unwrap();
    let publish = |bridge: &mut RuntimeAppearanceBridge, x: f32| {
        let facts = [
            placed(OBJECT, look, NativeRenderLayer::Scene, x),
            placed(STILL, look, NativeRenderLayer::Scene, -1.0),
        ];
        unsafe { bridge.stage_snapshot(facts.as_ptr(), facts.len()) }.unwrap();
    };
    publish(&mut bridge, 0.0);
    end(&mut bridge, &mut world, 0.0);
    let link = Rig::from_json(PHYSICS).unwrap().find_bone("h1").unwrap();
    let angle = |bridge: &RuntimeAppearanceBridge, object: u64, view: usize| {
        let b = bridge.staged_ref().unwrap().state.parts_rigs.instances[&object].views[view]
            .pose
            .bones[link];
        b.c.atan2(b.a).to_degrees()
    };
    let (mut widest, mut mirrored) = (0.0f64, 0.0f64);
    for n in 1..=60 {
        bridge.begin_call();
        if n > 1 {
            widest = widest.max((angle(&bridge, OBJECT, 0) - angle(&bridge, STILL, 0)).abs());
            mirrored = mirrored.max((angle(&bridge, OBJECT, 1) - angle(&bridge, STILL, 1)).abs());
        }
        // 0.01 m per rig unit: 0.04 m a step is 4 rig units.
        publish(&mut bridge, 0.04 * n.min(30) as f32);
        end(&mut bridge, &mut world, 1.0 / 60.0);
    }
    assert!(
        widest > 10.0,
        "the moved rig's hair swings {widest} degrees"
    );
    assert!(
        mirrored > 10.0,
        "and in its mirrored view {mirrored} degrees"
    );
}
