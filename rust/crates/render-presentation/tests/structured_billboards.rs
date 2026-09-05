use std::collections::{BTreeMap, BTreeSet};

use render_model::{RenderAssetKind, RenderHandle, ResolvedRenderAsset};
use render_presentation::*;

const WORLD_INDICATOR_FIXTURE: &str =
    include_str!("../../../../fixtures/render/world-indicator-frame-v1.json");

fn asset(id: &str, kind: RenderAssetKind, hash: &str) -> ResolvedRenderAsset {
    ResolvedRenderAsset {
        id: id.into(),
        kind,
        content_hash: Some(hash.into()),
        version: 1,
    }
}

fn assets() -> BTreeMap<String, ResolvedRenderAsset> {
    [
        asset("font/ui", RenderAssetKind::Font, "font-hash"),
        asset(
            "texture/indicator",
            RenderAssetKind::Texture,
            "indicator-hash",
        ),
        asset("texture/ready", RenderAssetKind::Texture, "ready-hash"),
        asset("texture/locked", RenderAssetKind::Texture, "locked-hash"),
        asset("audio/pulse", RenderAssetKind::Audio, "audio-hash"),
    ]
    .into_iter()
    .map(|asset| (asset.id.clone(), asset))
    .collect()
}

fn text(key: &str, fallback: &str) -> BillboardLocalizedText {
    BillboardLocalizedText {
        localization_key: key.into(),
        fallback_text: fallback.into(),
    }
}

fn meter(id: &str) -> BillboardMeter {
    BillboardMeter {
        id: id.into(),
        accessible_label: text("meter.value", "Value"),
        current: 40.0,
        min: 0.0,
        max: 100.0,
        preview: Some(70.0),
        fill_direction: BillboardMeterFillDirection::LeftToRight,
        segments: 8,
        fill: [0.2, 0.8, 0.3, 1.0],
        preview_fill: [0.9, 0.7, 0.1, 1.0],
        back: [0.05, 0.05, 0.05, 0.9],
        border: [0.0, 0.0, 0.0, 1.0],
    }
}

fn cue(id: &str, icon: Option<BillboardTextureRef>) -> BillboardStatusCue {
    BillboardStatusCue {
        id: id.into(),
        label: text("status.ready", "Ready"),
        icon,
    }
}

fn texture(asset: &str, content_hash: &str) -> BillboardTextureRef {
    BillboardTextureRef {
        asset: asset.into(),
        content_hash: content_hash.into(),
    }
}

fn layout() -> BillboardLayoutPolicy {
    BillboardLayoutPolicy {
        priority: -7,
        sizing: BillboardLayoutSizing::DistanceScaled {
            reference_distance: 20.0,
            min_scale: 0.75,
            max_scale: 1.25,
        },
        safe_area: BillboardSafeArea {
            top_pixels: 8.0,
            right_pixels: 12.0,
            bottom_pixels: 8.0,
            left_pixels: 12.0,
        },
        edge_behavior: BillboardEdgeBehavior::Clamp,
        overlap_behavior: BillboardOverlapBehavior::Suppress,
    }
}

fn indicator() -> BillboardIndicator {
    BillboardIndicator {
        label: Some(text("actor.name", "Sentinel")),
        icon: Some(texture("texture/indicator", "indicator-hash")),
        accessible_label: text("actor.indicator", "Sentinel indicator"),
        meters: vec![meter("primary")],
        status_cues: vec![cue("ready", Some(texture("texture/ready", "ready-hash")))],
        width_pixels: 220.0,
        spacing_pixels: 6.0,
        alignment: BillboardAlignment::Center,
        style: BillboardStyle {
            opacity: 0.95,
            backing: [0.0, 0.0, 0.0, 0.65],
            border: [0.2, 0.2, 0.2, 1.0],
            radius_pixels: 6.0,
        },
    }
}

fn descriptor(
    content: BillboardContent,
    layout: Option<BillboardLayoutPolicy>,
) -> BillboardDescriptor {
    BillboardDescriptor {
        anchor: BillboardAnchor::EntityAttached {
            entity: 42,
            offset: [0.0, 1.8, 0.0],
        },
        content,
        font: BillboardFontRef::Asset {
            asset: "font/ui".into(),
            content_hash: "font-hash".into(),
            family: "Rusty UI".into(),
        },
        height_pixels: 24.0,
        color: [1.0; 4],
        background: [0.0, 0.0, 0.0, 0.35],
        max_distance: 80.0,
        layer: BillboardLayer::Occluded,
        visible: true,
        layout,
    }
}

fn structured_descriptor() -> BillboardDescriptor {
    descriptor(
        BillboardContent::Structured {
            indicator: indicator(),
        },
        Some(layout()),
    )
}

fn project(
    projector: &mut BillboardProjector,
    handle: u64,
    descriptor: BillboardDescriptor,
) -> Result<(), BillboardProjectionDiagnostic> {
    projector
        .project(
            &assets(),
            PresentationOpMeta::new(0),
            BillboardProjectionOp::Create {
                handle: BillboardHandle::new(handle),
                descriptor,
            },
        )
        .map(|_| ())
}

#[test]
fn structured_world_indicator_fixture_decodes_at_the_rust_border() {
    let frame: PresentationFrameDiff = serde_json::from_str(WORLD_INDICATOR_FIXTURE).unwrap();
    frame.validate().unwrap();
    let PresentationOp::Billboard {
        op: BillboardProjectionOp::Create { descriptor, .. },
        ..
    } = &frame.ops[0]
    else {
        panic!("fixture must contain one structured billboard create");
    };
    let BillboardContent::Structured { indicator } = &descriptor.content else {
        panic!("fixture billboard must use structured content");
    };
    assert_eq!(indicator.meters[0].id, "health");
    assert_eq!(indicator.status_cues[0].id, "interact");
    assert_eq!(indicator.meters[0].current_fraction(), Some(0.72));
}

#[test]
fn frame_border_rejects_structured_descriptor_bounds_without_projector_state() {
    for mutate in [
        |descriptor: &mut BillboardDescriptor| descriptor.layout = None,
        |descriptor: &mut BillboardDescriptor| {
            if let BillboardContent::Structured { indicator } = &mut descriptor.content {
                indicator.width_pixels = 0.0;
            }
        },
    ] {
        let mut descriptor = structured_descriptor();
        mutate(&mut descriptor);
        let frame = PresentationFrameDiff {
            publication: None,
            schema_version: PRESENTATION_FRAME_SCHEMA_VERSION,
            ops: vec![PresentationOp::Billboard {
                meta: PresentationOpMeta::new(0),
                op: BillboardProjectionOp::Create {
                    handle: BillboardHandle::new(1),
                    descriptor,
                },
            }],
        };
        assert!(matches!(
            frame.validate(),
            Err(PresentationFrameError::InvalidDescriptor { .. })
        ));
    }
}

#[test]
fn structured_indicator_round_trips_and_derives_meter_fractions() {
    let descriptor = structured_descriptor();
    let meter = match &descriptor.content {
        BillboardContent::Structured { indicator } => &indicator.meters[0],
        _ => unreachable!("fixture is structured"),
    };
    assert_eq!(meter.current_fraction(), Some(0.4));
    assert_eq!(meter.preview_fraction(), Some(0.7));

    let encoded = serde_json::to_string(&descriptor).unwrap();
    let decoded: BillboardDescriptor = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, descriptor);
    assert!(encoded.contains("structured"));
    assert!(encoded.contains("distanceScaled"));
    assert!(encoded.contains("referenceDistance"));
    assert!(!encoded.contains("reference_distance"));
    assert!(!encoded.contains("normalized"));
}

#[test]
fn legacy_descriptor_omits_layout_and_remains_compatible() {
    let legacy = descriptor(
        BillboardContent::Text {
            localization_key: "actor.name".into(),
            fallback_text: "Sentinel".into(),
            arguments: Vec::new(),
        },
        None,
    );
    let encoded = serde_json::to_string(&legacy).unwrap();
    assert!(!encoded.contains("layout"));
    assert_eq!(
        serde_json::from_str::<BillboardDescriptor>(&encoded).unwrap(),
        legacy
    );

    let mut projector = BillboardProjector::default();
    project(&mut projector, 1, legacy).unwrap();
}

#[test]
fn structured_composition_bounds_and_identity_rules_are_enforced() {
    let mut too_many_meters = structured_descriptor();
    if let BillboardContent::Structured { indicator } = &mut too_many_meters.content {
        indicator.meters = (0..5)
            .map(|index| meter(&format!("meter-{index}")))
            .collect();
    }
    assert_eq!(
        project(&mut BillboardProjector::default(), 1, too_many_meters)
            .unwrap_err()
            .code,
        BillboardProjectionDiagnosticCode::InvalidDescriptor
    );

    let mut too_many_cues = structured_descriptor();
    if let BillboardContent::Structured { indicator } = &mut too_many_cues.content {
        indicator.status_cues = (0..9)
            .map(|index| cue(&format!("cue-{index}"), None))
            .collect();
    }
    assert_eq!(
        project(&mut BillboardProjector::default(), 2, too_many_cues)
            .unwrap_err()
            .code,
        BillboardProjectionDiagnosticCode::InvalidDescriptor
    );

    let mut duplicate_ids = structured_descriptor();
    if let BillboardContent::Structured { indicator } = &mut duplicate_ids.content {
        indicator.meters.push(meter("primary"));
    }
    assert_eq!(
        project(&mut BillboardProjector::default(), 3, duplicate_ids)
            .unwrap_err()
            .code,
        BillboardProjectionDiagnosticCode::InvalidDescriptor
    );

    let mut long_text = structured_descriptor();
    if let BillboardContent::Structured { indicator } = &mut long_text.content {
        indicator.accessible_label.fallback_text = "x".repeat(257);
    }
    assert_eq!(
        project(&mut BillboardProjector::default(), 4, long_text)
            .unwrap_err()
            .code,
        BillboardProjectionDiagnosticCode::InvalidDescriptor
    );
}

#[test]
fn meters_require_finite_ordered_ranges_in_range_preview_and_segment_bounds() {
    type MeterMutation = fn(&mut BillboardMeter);
    let cases: [(&str, MeterMutation); 8] = [
        ("reversed range", |meter: &mut BillboardMeter| {
            meter.min = 100.0;
            meter.max = 0.0;
        }),
        ("current below min", |meter: &mut BillboardMeter| {
            meter.current = -1.0;
        }),
        ("preview above max", |meter: &mut BillboardMeter| {
            meter.preview = Some(101.0);
        }),
        ("zero segments", |meter: &mut BillboardMeter| {
            meter.segments = 0;
        }),
        ("too many segments", |meter: &mut BillboardMeter| {
            meter.segments = 33;
        }),
        ("nonfinite current", |meter: &mut BillboardMeter| {
            meter.current = f32::NAN;
        }),
        ("nonfinite range", |meter: &mut BillboardMeter| {
            meter.max = f32::INFINITY;
        }),
        ("extreme magnitude", |meter: &mut BillboardMeter| {
            meter.current = 1_500_000_000_000.0;
            meter.max = 2_000_000_000_000.0;
        }),
    ];

    for (name, mutate) in cases {
        let mut invalid = structured_descriptor();
        if let BillboardContent::Structured { indicator } = &mut invalid.content {
            mutate(&mut indicator.meters[0]);
        }
        assert_eq!(
            project(&mut BillboardProjector::default(), 1, invalid)
                .unwrap_err()
                .code,
            BillboardProjectionDiagnosticCode::InvalidDescriptor,
            "case {name}"
        );
    }
}

#[test]
fn indicator_style_and_layout_policy_are_bounded_and_structured_only() {
    let mut missing_layout = structured_descriptor();
    missing_layout.layout = None;
    assert_eq!(
        project(&mut BillboardProjector::default(), 1, missing_layout)
            .unwrap_err()
            .code,
        BillboardProjectionDiagnosticCode::InvalidDescriptor
    );

    let mut legacy_with_layout = descriptor(
        BillboardContent::Value {
            label_key: "actor.value".into(),
            fallback_label: "Value".into(),
            value: "1".into(),
            unit_key: None,
            fallback_unit: None,
        },
        Some(layout()),
    );
    assert_eq!(
        project(
            &mut BillboardProjector::default(),
            2,
            legacy_with_layout.clone()
        )
        .unwrap_err()
        .code,
        BillboardProjectionDiagnosticCode::InvalidDescriptor
    );

    let mut invalid_style = structured_descriptor();
    if let BillboardContent::Structured { indicator } = &mut invalid_style.content {
        indicator.style.opacity = 1.1;
        indicator.width_pixels = 0.0;
    }
    assert_eq!(
        project(&mut BillboardProjector::default(), 3, invalid_style)
            .unwrap_err()
            .code,
        BillboardProjectionDiagnosticCode::InvalidDescriptor
    );

    legacy_with_layout.layout = None;
    let mut invalid_scale = structured_descriptor();
    invalid_scale.layout = Some(BillboardLayoutPolicy {
        sizing: BillboardLayoutSizing::DistanceScaled {
            reference_distance: 0.0,
            min_scale: 2.0,
            max_scale: 1.0,
        },
        ..layout()
    });
    assert_eq!(
        project(&mut BillboardProjector::default(), 4, invalid_scale)
            .unwrap_err()
            .code,
        BillboardProjectionDiagnosticCode::InvalidDescriptor
    );
}

#[test]
fn structured_icons_require_exact_assets_and_are_accounted_once() {
    let mut projector = BillboardProjector::default();
    let mut descriptor = structured_descriptor();
    if let BillboardContent::Structured { indicator } = &mut descriptor.content {
        indicator.status_cues.push(cue(
            "locked",
            Some(texture("texture/locked", "locked-hash")),
        ));
    }
    project(&mut projector, 1, descriptor.clone()).unwrap();
    let readout = projector.readout();
    assert_eq!(readout.referenced_fonts, 1);
    assert_eq!(readout.referenced_icons, 3);

    let mut missing = descriptor.clone();
    if let BillboardContent::Structured { indicator } = &mut missing.content {
        indicator.icon = Some(texture("texture/missing", "missing-hash"));
    }
    assert_eq!(
        project(&mut BillboardProjector::default(), 2, missing)
            .unwrap_err()
            .code,
        BillboardProjectionDiagnosticCode::AssetMissing
    );

    let mut wrong_hash_assets = assets();
    wrong_hash_assets
        .get_mut("texture/indicator")
        .unwrap()
        .content_hash = Some("changed".into());
    let mut wrong_hash_projector = BillboardProjector::default();
    let wrong_hash_error = wrong_hash_projector
        .project(
            &wrong_hash_assets,
            PresentationOpMeta::new(0),
            BillboardProjectionOp::Create {
                handle: BillboardHandle::new(3),
                descriptor,
            },
        )
        .unwrap_err();
    assert_eq!(
        wrong_hash_error.code,
        BillboardProjectionDiagnosticCode::ContentHashMismatch
    );
}

#[test]
fn structured_create_update_destroy_duplicate_unknown_and_batch_rollback_are_atomic() {
    let assets = assets();
    let handle = BillboardHandle::new(7);
    let mut projector = BillboardProjector::default();
    let error = projector
        .project_batch(
            &assets,
            vec![
                (
                    PresentationOpMeta::new(0),
                    BillboardProjectionOp::Create {
                        handle,
                        descriptor: structured_descriptor(),
                    },
                ),
                (
                    PresentationOpMeta::new(1),
                    BillboardProjectionOp::Update {
                        handle,
                        patch: BillboardPatch {
                            content: Some(BillboardContent::Structured {
                                indicator: BillboardIndicator {
                                    width_pixels: 0.0,
                                    ..indicator()
                                },
                            }),
                            ..BillboardPatch::default()
                        },
                    },
                ),
            ],
        )
        .unwrap_err();
    assert_eq!(
        error.code,
        BillboardProjectionDiagnosticCode::InvalidDescriptor
    );
    assert_eq!(projector.readout().active_billboards, 0);

    project(&mut projector, handle.raw(), structured_descriptor()).unwrap();
    assert_eq!(
        projector
            .project(
                &assets,
                PresentationOpMeta::new(1),
                BillboardProjectionOp::Create {
                    handle,
                    descriptor: structured_descriptor(),
                },
            )
            .unwrap_err()
            .code,
        BillboardProjectionDiagnosticCode::DuplicateHandle
    );

    let updated_layout = BillboardLayoutPolicy {
        priority: 11,
        sizing: BillboardLayoutSizing::ConstantPixels,
        ..layout()
    };
    projector
        .project(
            &assets,
            PresentationOpMeta::new(2),
            BillboardProjectionOp::Update {
                handle,
                patch: BillboardPatch {
                    layout: Some(updated_layout.clone()),
                    ..BillboardPatch::default()
                },
            },
        )
        .unwrap();
    assert_eq!(
        projector.descriptor(handle).unwrap().layout,
        Some(updated_layout)
    );

    projector
        .project(
            &assets,
            PresentationOpMeta::new(3),
            BillboardProjectionOp::Update {
                handle,
                patch: BillboardPatch {
                    content: Some(BillboardContent::Text {
                        localization_key: "actor.name".into(),
                        fallback_text: "Sentinel".into(),
                        arguments: Vec::new(),
                    }),
                    ..BillboardPatch::default()
                },
            },
        )
        .unwrap();
    assert_eq!(projector.descriptor(handle).unwrap().layout, None);

    projector
        .project(
            &assets,
            PresentationOpMeta::new(4),
            BillboardProjectionOp::Destroy { handle },
        )
        .unwrap();
    assert_eq!(projector.readout().active_billboards, 0);
    assert_eq!(
        projector
            .project(
                &assets,
                PresentationOpMeta::new(5),
                BillboardProjectionOp::Destroy { handle },
            )
            .unwrap_err()
            .code,
        BillboardProjectionDiagnosticCode::UnknownHandle
    );
}

#[test]
fn mixed_domain_structured_failure_does_not_commit_earlier_audio() {
    let assets = assets();
    let targets = BTreeSet::<RenderHandle>::new();
    let mut invalid_billboard = structured_descriptor();
    if let BillboardContent::Structured { indicator } = &mut invalid_billboard.content {
        indicator.icon.as_mut().unwrap().content_hash = "wrong-hash".into();
    }
    let frame = PresentationFrameDiff::try_from_ops(vec![
        PresentationOp::Audio {
            meta: PresentationOpMeta::new(0),
            op: AudioProjectionOp::Create {
                handle: AudioHandle::new(1),
                descriptor: AudioSourceDescriptor {
                    clip: AudioClipRef {
                        asset: "audio/pulse".into(),
                        content_hash: "audio-hash".into(),
                        duration_seconds: Some(2.0),
                    },
                    bus: AudioBus::Sfx,
                    volume: 0.8,
                    pitch: 1.0,
                    looping: false,
                    spatial_blend: 0.0,
                    attenuation: 10.0,
                    pan: 0.0,
                    emitter: AudioEmitter::Global2d,
                },
            },
        },
        PresentationOp::Billboard {
            meta: PresentationOpMeta::new(1),
            op: BillboardProjectionOp::Create {
                handle: BillboardHandle::new(1),
                descriptor: invalid_billboard,
            },
        },
    ])
    .unwrap();

    let mut projectors = PresentationProjectorSet::default();
    assert!(matches!(
        projectors.project_frame(&assets, &targets, frame),
        Err(PresentationProjectionError::Billboard(_))
    ));
    let readout = projectors.readout();
    assert_eq!(readout.audio.active_sources, 0);
    assert_eq!(readout.billboards.active_billboards, 0);
}

#[test]
fn frame_rejects_nonfinite_structured_numbers_before_json_encoding() {
    let mut invalid = structured_descriptor();
    if let BillboardContent::Structured { indicator } = &mut invalid.content {
        indicator.meters[0].preview = Some(f32::NAN);
    }
    let frame = PresentationFrameDiff {
        publication: None,
        schema_version: PRESENTATION_FRAME_SCHEMA_VERSION,
        ops: vec![PresentationOp::Billboard {
            meta: PresentationOpMeta::new(0),
            op: BillboardProjectionOp::Create {
                handle: BillboardHandle::new(1),
                descriptor: invalid,
            },
        }],
    };
    assert_eq!(
        frame.validate(),
        Err(PresentationFrameError::NonFiniteNumber {
            sequence: 0,
            field: "billboard.meter.preview",
        })
    );
    assert!(matches!(
        frame.encode_json(),
        Err(PresentationJsonError::InvalidFrame(
            PresentationFrameError::NonFiniteNumber { .. }
        ))
    ));
}
