//! Ghost plate fixtures (#8788): a hidden source captured into 8 sectors,
//! presented at a plate, snapping across a sector boundary with hysteresis.

mod support;

use render_model::*;
use render_presentation::*;
use render_wgpu::RendererOptions;
use support::*;

const SOURCE: u64 = 50;
const PLATE: GhostPlateHandle = GhostPlateHandle::new(7);
const PLATE_CENTER: [f32; 3] = [0.0, 1.0, -4.0];
const NO_ENTITIES: &dyn Fn(u64) -> Option<[f32; 3]> = &|_| None;

/// An asymmetric source far from the plate, hidden as CraftSurvive publishes
/// its ghost source: a tall red box on its left, a short blue box on its
/// right.
fn source_scene(harness: &mut Harness) {
    harness.apply(vec![
        RenderDiff::DefineMaterial {
            material: material("material/red", [0.85, 0.2, 0.15, 1.0], None),
        },
        RenderDiff::DefineMaterial {
            material: material("material/blue", [0.15, 0.3, 0.9, 1.0], None),
        },
        static_mesh(
            "mesh/tall",
            box_mesh([-0.6, 0.0, -0.3], [0.0, 1.6, 0.3], |_| 0),
            "material/red",
        ),
        static_mesh(
            "mesh/short",
            box_mesh([0.0, 0.0, -0.3], [0.6, 0.8, 0.3], |_| 0),
            "material/blue",
        ),
        {
            let mut node = RenderNode::new(Geometry::Group);
            node.transform = transform([20.0, 0.0, 0.0], 0.0, [1.0; 3]);
            node.visible = false;
            RenderDiff::Create {
                handle: RenderHandle::new(SOURCE),
                parent: None,
                node,
            }
        },
        instance(51, Some(SOURCE), "mesh/tall", Transform::IDENTITY),
        instance(52, Some(SOURCE), "mesh/short", Transform::IDENTITY),
    ]);
}

fn descriptor(sectors: u8) -> GhostPlateDescriptor {
    GhostPlateDescriptor {
        source: RenderHandle::new(SOURCE),
        captured_scene: None,
        placement: GhostPlatePlacement {
            transform: Transform {
                translation: PLATE_CENTER,
                rotation: [0.0, 0.0, 0.0, 1.0],
                scale: [1.0; 3],
            },
            width: 2.0,
            height: 2.0,
        },
        capture: GhostPlateCaptureSettings {
            resolution: 128,
            azimuth_degrees: 0.0,
            elevation_degrees: 15.0,
            near: 0.1,
            far: 40.0,
            field_of_view_degrees: 45.0,
            lighting: GhostPlateCaptureLighting {
                mode: GhostPlateCaptureLightingMode::Isolated,
                ambient_color: [0.25, 0.25, 0.25],
                ambient_intensity: 1.8,
                key_direction: [0.5, 1.0, 0.25],
                key_color: [1.0; 3],
                key_intensity: 3.0,
                fill_direction: [-0.5, 0.25, -1.0],
                fill_color: [0.5, 0.5, 0.5],
                fill_intensity: 1.4,
            },
        },
        config: GhostPlateConfig {
            depth_retention: 0.15,
            anchor_policy: GhostPlateAnchorPolicy::BoundsCenter,
            anchor_value: 0.5,
            plate_mapping: GhostPlateMapping::PlateLocked,
            shell_mode: GhostPlateShellMode::WholeMesh,
            shell_depth_epsilon: 0.12,
            sector_count: sectors,
            sector_hysteresis_degrees: 3.0,
        },
    }
}

/// Apply a ghost plate op through the retained world (which freezes the
/// capture scene) and hand the renderer its delta.
fn ghost(harness: &mut Harness, op: GhostPlateProjectionOp) {
    let delta = harness
        .world
        .apply_presentation(PresentationFrameDiff::from_ops(vec![
            PresentationOp::GhostPlate {
                meta: PresentationOpMeta::new(0),
                op,
            },
        ]))
        .expect("retained world admits the plate");
    let issues = harness
        .renderer
        .apply_presentation(&delta, &harness.resources, NO_ENTITIES);
    assert!(issues.is_empty(), "{issues:?}");
}

/// A camera 4 units from the plate at `azimuth` degrees around it, looking
/// at it.
fn orbit(azimuth: f32) -> render_host_contracts::RendererCompositionCamera {
    let radians = azimuth.to_radians();
    let center = glam::Vec3::from(PLATE_CENTER);
    let eye = center + glam::Vec3::new(radians.sin() * 4.0, 1.2, radians.cos() * 4.0);
    let direction = (center - eye).normalize();
    camera(
        [f64::from(eye.x), f64::from(eye.y), f64::from(eye.z)],
        f64::from(direction.x.atan2(-direction.z).to_degrees()),
        f64::from(direction.y.asin().to_degrees()),
    )
}

fn sector(harness: &Harness) -> u32 {
    harness.renderer.ghost_plate_readouts()[0].current_sector
}

#[test]
fn a_ghost_plate_snaps_between_captured_sectors_with_hysteresis() {
    let mut harness = Harness::new(RendererOptions::default());
    source_scene(&mut harness);
    ghost(
        &mut harness,
        GhostPlateProjectionOp::Create {
            handle: PLATE,
            descriptor: descriptor(8),
        },
    );
    let (front, pixels) = harness.render(&orbit(20.0));
    assert!(
        front.draws >= 2,
        "the plate draws its frozen parts: {front:?}"
    );
    assert_eq!(sector(&harness), 0);
    assert_screenshot("ghost-plate-front", &pixels);

    // 22.5° is the boundary; 3° of hysteresis holds the current sector.
    let mut path = Vec::new();
    for azimuth in [24.0, 27.0, 24.0, 19.0] {
        harness.render(&orbit(azimuth));
        path.push(sector(&harness));
    }
    assert_eq!(path, vec![0, 1, 1, 0]);

    let (_, side) = harness.render(&orbit(95.0));
    assert_eq!(sector(&harness), 2);
    assert_screenshot("ghost-plate-side", &side);
}

#[test]
fn placement_updates_reuse_captures_and_sector_changes_recapture() {
    let mut harness = Harness::new(RendererOptions::default());
    source_scene(&mut harness);
    ghost(
        &mut harness,
        GhostPlateProjectionOp::Create {
            handle: PLATE,
            descriptor: descriptor(8),
        },
    );
    harness.render(&orbit(0.0));
    let captured = harness.renderer.ghost_plate_readouts()[0].capture_milliseconds;

    let mut wider = descriptor(8);
    wider.placement.width = 3.0;
    wider.config.depth_retention = 0.5;
    ghost(
        &mut harness,
        GhostPlateProjectionOp::Update {
            handle: PLATE,
            patch: GhostPlatePatch {
                placement: Some(wider.placement.clone()),
                config: Some(wider.config.clone()),
            },
        },
    );
    harness.render(&orbit(0.0));
    let readout = &harness.renderer.ghost_plate_readouts()[0];
    assert_eq!(
        (readout.sector_count, readout.capture_milliseconds),
        (8, captured),
        "placement and relief changes keep the captures"
    );

    wider.config.sector_count = 4;
    ghost(
        &mut harness,
        GhostPlateProjectionOp::Update {
            handle: PLATE,
            patch: GhostPlatePatch {
                placement: None,
                config: Some(wider.config),
            },
        },
    );
    harness.render(&orbit(95.0));
    let readout = &harness.renderer.ghost_plate_readouts()[0];
    assert_eq!((readout.sector_count, readout.current_sector), (4, 1));

    ghost(
        &mut harness,
        GhostPlateProjectionOp::Destroy { handle: PLATE },
    );
    assert!(harness.renderer.ghost_plate_readouts().is_empty());
}

/// A capture carries its materials and atlases whole: once its source is
/// gone, the main world may release them, and the plate still builds again
/// for a new sector count and for a fresh attachment.
#[test]
fn a_plate_rebuilds_after_its_material_and_atlas_are_released() {
    let mut harness = Harness::new(RendererOptions::default());
    source_scene(&mut harness);
    let sheet: Vec<u8> = (0..16)
        .flat_map(|index| {
            if index % 2 == 0 {
                [230, 200, 40, 255]
            } else {
                [40, 200, 90, 255]
            }
        })
        .collect();
    let atlas_texture =
        harness
            .resources
            .texture("texture/ghost-sheet", 4, 4, &sheet, TextureWrap::Clamp);
    harness.apply(vec![
        RenderDiff::DefineTexture {
            texture: atlas_texture,
        },
        RenderDiff::DefineSpriteAtlas {
            atlas: SpriteAtlasDescriptor {
                id: "sprite/ghost".to_owned(),
                texture: "texture/ghost-sheet".to_owned(),
                frames: vec![SpriteFrameRect {
                    frame: 0,
                    uv_min: [0.0, 0.0],
                    uv_max: [1.0, 1.0],
                    size: None,
                }],
            },
        },
        RenderDiff::CreateSprite {
            handle: RenderHandle::new(53),
            parent: Some(RenderHandle::new(SOURCE)),
            sprite: SpriteInstanceDescriptor {
                asset: "sprite/ghost".to_owned(),
                frame: 0,
                pivot: [0.5, 0.0],
                size: [0.5, 0.5],
                size_mode: SpriteSizeMode::World,
                billboard: BillboardMode::None,
                tint: [1.0; 4],
                render_order: 0,
                depth: SpriteDepthPolicy::Default,
                layer: RenderLayer::Scene,
                viewport_placement: None,
                shading: SpriteShading::Unlit,
                material: SpriteMaterialDescriptor::default(),
                visible: true,
                transform: transform([0.0, 1.6, 0.0], 0.0, [1.0; 3]),
                attachment: SpriteAttachment::default(),
                metadata: RenderMetadata::default(),
            },
        },
    ]);
    ghost(
        &mut harness,
        GhostPlateProjectionOp::Create {
            handle: PLATE,
            descriptor: descriptor(8),
        },
    );
    let (captured, _) = harness.render(&orbit(0.0));
    let parts = harness.renderer.ghost_plate_readouts()[0].parts;
    assert!(parts >= 2, "the plate captures both boxes: {parts}");

    // The source goes, then the resources only it drew from.
    harness.apply(vec![RenderDiff::Destroy {
        handle: RenderHandle::new(SOURCE),
    }]);
    harness.apply(vec![
        RenderDiff::ReleaseStaticMesh {
            asset: "mesh/tall".to_owned(),
        },
        RenderDiff::ReleaseMaterial {
            id: "material/red".to_owned(),
        },
        RenderDiff::ReleaseSpriteAtlas {
            id: "sprite/ghost".to_owned(),
        },
    ]);
    // The texture's bytes are read again on every rebuild, so it stays.
    assert!(harness
        .world
        .clone()
        .apply(RenderFrameDiff {
            publication: None,
            ops: vec![RenderDiff::ReleaseTexture {
                id: "texture/ghost-sheet".to_owned(),
            }],
        })
        .is_err());

    let mut fewer = descriptor(4);
    ghost(
        &mut harness,
        GhostPlateProjectionOp::Update {
            handle: PLATE,
            patch: GhostPlatePatch {
                placement: None,
                config: Some(fewer.config.clone()),
            },
        },
    );
    let (rebuilt, _) = harness.render(&orbit(0.0));
    let readout = &harness.renderer.ghost_plate_readouts()[0];
    assert_eq!((readout.sector_count, readout.parts), (4, parts));
    assert_eq!(rebuilt.draws, captured.draws);

    // A fresh attachment: the retained world, then its plate's baseline.
    fewer.captured_scene = None;
    let baseline = harness
        .world
        .with_retained_captures(vec![PresentationFrameDiff::from_ops(vec![
            PresentationOp::GhostPlate {
                meta: PresentationOpMeta::new(0),
                op: GhostPlateProjectionOp::Create {
                    handle: PLATE,
                    descriptor: fewer,
                },
            },
        ])]);
    let mut fresh = render_wgpu::Renderer::new(&harness.gpu, harness.renderer.options());
    let issues = fresh.apply(&harness.world.snapshot().frame, &harness.resources);
    assert!(issues.is_empty(), "{issues:?}");
    for frame in &baseline {
        let issues = fresh.apply_presentation(frame, &harness.resources, NO_ENTITIES);
        assert!(issues.is_empty(), "{issues:?}");
    }
    let readout = &fresh.ghost_plate_readouts()[0];
    assert_eq!((readout.sector_count, readout.parts), (4, parts));
    let attached = fresh.render_offscreen(&orbit(0.0), &harness.target);
    assert_eq!(attached.draws, captured.draws);
}
