use std::collections::{BTreeMap, BTreeSet};

use render_model::{RenderAssetKind, RenderHandle, ResolvedRenderAsset};
use render_presentation::*;

fn audio() -> AudioSourceDescriptor {
    AudioSourceDescriptor {
        clip: AudioClipRef {
            asset: "audio/pulse".into(),
            content_hash: "aa".into(),
            duration_seconds: None,
        },
        bus: AudioBus::Sfx,
        volume: 0.8,
        pitch: 1.0,
        looping: false,
        spatial_blend: 0.0,
        attenuation: 10.0,
        pan: 0.0,
        emitter: AudioEmitter::Global2d,
    }
}

fn billboard() -> BillboardDescriptor {
    BillboardDescriptor {
        anchor: BillboardAnchor::World {
            position: [1.0, 2.0, 3.0],
        },
        content: BillboardContent::Text {
            localization_key: "fixture.label".into(),
            fallback_text: "Fixture".into(),
            arguments: vec![BillboardTemplateArgument {
                name: "value".into(),
                value: "42".into(),
            }],
        },
        font: BillboardFontRef::System {
            family: "sans-serif".into(),
        },
        height_pixels: 24.0,
        color: [1.0; 4],
        background: [0.0, 0.0, 0.0, 0.5],
        max_distance: 50.0,
        layer: BillboardLayer::DepthTested,
        visible: true,
        layout: None,
    }
}

fn particle() -> ParticleEmitterDescriptor {
    ParticleEmitterDescriptor {
        anchor: ParticleAnchor::World {
            position: [0.0, 1.0, 0.0],
        },
        visual: ParticleVisual::Billboard {
            sprite: ParticleSpriteRef {
                asset: "sprite-sheet/sparks".into(),
                content_hash: "dd".into(),
                frame_count: 4,
            },
        },
        rate_per_second: 8.0,
        burst_count: 4,
        lifetime_seconds: [0.2, 0.6],
        velocity_min: [-1.0, 1.0, -1.0],
        velocity_max: [1.0, 2.0, 1.0],
        acceleration: [0.0, -4.0, 0.0],
        size_curve: vec![
            ParticleScalarKey {
                age: 0.0,
                value: 0.25,
            },
            ParticleScalarKey {
                age: 1.0,
                value: 0.0,
            },
        ],
        color_curve: vec![
            ParticleColorKey {
                age: 0.0,
                color: [1.0, 0.8, 0.2, 1.0],
            },
            ParticleColorKey {
                age: 1.0,
                color: [1.0, 0.2, 0.0, 0.0],
            },
        ],
        flipbook_frames_per_second: 12.0,
        seed: 7,
        max_particles: 32,
        visible: true,
        collision: None,
    }
}

fn telemetry() -> TelemetryOverlayDescriptor {
    TelemetryOverlayDescriptor {
        title: "Frame telemetry".into(),
        corner: TelemetryOverlayCorner::TopRight,
        refresh_interval_ms: 250,
        max_frame_time_samples: 60,
        visible: true,
    }
}

fn animation_state(revision: u64) -> AnimationControllerProjectionState {
    AnimationControllerProjectionState {
        entity: 42,
        graph_id: "hero.locomotion".into(),
        graph_version: 2,
        state_id: "idle".into(),
        revision,
        controller_tick: revision,
        phase_seconds: revision as f64 * 0.016,
        clip_phases: vec![],
        motion: ResolvedAnimationMotion {
            clip_a: "idle".into(),
            clip_b: None,
            blend_weight_milli: 0,
            speed_milli: 1_000,
        },
        transition: None,
        transition_fact: None,
    }
}

fn fixture_frame() -> PresentationFrameDiff {
    PresentationFrameDiff::try_from_ops(vec![
        PresentationOp::Audio {
            meta: PresentationOpMeta::new(0),
            op: AudioProjectionOp::Emit {
                signal_handle: AudioSignalHandle::new(1),
                signal_id: "fixture:pulse".into(),
                descriptor: audio(),
            },
        },
        PresentationOp::Billboard {
            meta: PresentationOpMeta::new(1),
            op: BillboardProjectionOp::Create {
                handle: BillboardHandle::new(1),
                descriptor: billboard(),
            },
        },
        PresentationOp::Particle {
            meta: PresentationOpMeta::new(2),
            op: ParticleProjectionOp::Emit {
                signal_id: "fixture:sparks".into(),
                descriptor: particle(),
            },
        },
        PresentationOp::TelemetryOverlay {
            meta: PresentationOpMeta::new(3),
            op: TelemetryOverlayProjectionOp::Create {
                handle: TelemetryOverlayHandle::new(1),
                descriptor: telemetry(),
            },
        },
        PresentationOp::Animation {
            meta: PresentationOpMeta::new(4),
            op: AnimationProjectionOp::Create {
                handle: AnimationProjectionHandle::new(1),
                descriptor: AnimationProjectionDescriptor {
                    target: RenderHandle::new(42),
                    asset: "animated-mesh/hero".into(),
                    content_hash: "ff".into(),
                    tick_duration_millis: 16,
                    controller: animation_state(0),
                },
            },
        },
    ])
    .unwrap()
}

#[test]
fn every_presentation_operation_survives_the_json_border() {
    let animation_descriptor = AnimationProjectionDescriptor {
        target: RenderHandle::new(42),
        asset: "animated-mesh/hero".into(),
        content_hash: "ff".into(),
        tick_duration_millis: 16,
        controller: animation_state(0),
    };
    let ops = vec![
        PresentationOp::Audio {
            meta: PresentationOpMeta::new(0),
            op: AudioProjectionOp::Emit {
                signal_handle: AudioSignalHandle::new(1),
                signal_id: "pulse".into(),
                descriptor: audio(),
            },
        },
        PresentationOp::Audio {
            meta: PresentationOpMeta::new(1),
            op: AudioProjectionOp::Create {
                handle: AudioHandle::new(1),
                descriptor: audio(),
            },
        },
        PresentationOp::Audio {
            meta: PresentationOpMeta::new(2),
            op: AudioProjectionOp::Update {
                handle: AudioHandle::new(1),
                patch: AudioSourcePatch {
                    volume: Some(0.5),
                    ..AudioSourcePatch::default()
                },
            },
        },
        PresentationOp::Audio {
            meta: PresentationOpMeta::new(3),
            op: AudioProjectionOp::Destroy {
                handle: AudioHandle::new(1),
            },
        },
        PresentationOp::Billboard {
            meta: PresentationOpMeta::new(4),
            op: BillboardProjectionOp::Create {
                handle: BillboardHandle::new(2),
                descriptor: billboard(),
            },
        },
        PresentationOp::Billboard {
            meta: PresentationOpMeta::new(5),
            op: BillboardProjectionOp::Update {
                handle: BillboardHandle::new(2),
                patch: BillboardPatch {
                    visible: Some(false),
                    ..BillboardPatch::default()
                },
            },
        },
        PresentationOp::Billboard {
            meta: PresentationOpMeta::new(6),
            op: BillboardProjectionOp::Destroy {
                handle: BillboardHandle::new(2),
            },
        },
        PresentationOp::Particle {
            meta: PresentationOpMeta::new(7),
            op: ParticleProjectionOp::Emit {
                signal_id: "sparks".into(),
                descriptor: particle(),
            },
        },
        PresentationOp::Particle {
            meta: PresentationOpMeta::new(8),
            op: ParticleProjectionOp::Create {
                handle: ParticleEmitterHandle::new(3),
                descriptor: particle(),
            },
        },
        PresentationOp::Particle {
            meta: PresentationOpMeta::new(9),
            op: ParticleProjectionOp::Update {
                handle: ParticleEmitterHandle::new(3),
                patch: ParticleEmitterPatch {
                    visible: Some(false),
                    ..ParticleEmitterPatch::default()
                },
            },
        },
        PresentationOp::Particle {
            meta: PresentationOpMeta::new(10),
            op: ParticleProjectionOp::Destroy {
                handle: ParticleEmitterHandle::new(3),
            },
        },
        PresentationOp::TelemetryOverlay {
            meta: PresentationOpMeta::new(11),
            op: TelemetryOverlayProjectionOp::Create {
                handle: TelemetryOverlayHandle::new(4),
                descriptor: telemetry(),
            },
        },
        PresentationOp::TelemetryOverlay {
            meta: PresentationOpMeta::new(12),
            op: TelemetryOverlayProjectionOp::Update {
                handle: TelemetryOverlayHandle::new(4),
                patch: TelemetryOverlayPatch {
                    visible: Some(false),
                    ..TelemetryOverlayPatch::default()
                },
            },
        },
        PresentationOp::TelemetryOverlay {
            meta: PresentationOpMeta::new(13),
            op: TelemetryOverlayProjectionOp::Destroy {
                handle: TelemetryOverlayHandle::new(4),
            },
        },
        PresentationOp::Animation {
            meta: PresentationOpMeta::new(14),
            op: AnimationProjectionOp::Create {
                handle: AnimationProjectionHandle::new(5),
                descriptor: animation_descriptor,
            },
        },
        PresentationOp::Animation {
            meta: PresentationOpMeta::new(15),
            op: AnimationProjectionOp::Update {
                handle: AnimationProjectionHandle::new(5),
                controller: animation_state(1),
            },
        },
        PresentationOp::Animation {
            meta: PresentationOpMeta::new(16),
            op: AnimationProjectionOp::Destroy {
                handle: AnimationProjectionHandle::new(5),
            },
        },
        PresentationOp::Audio {
            meta: PresentationOpMeta::new(17),
            op: AudioProjectionOp::VoiceControl {
                handle: AudioHandle::new(1),
                control: AudioVoiceControl::Retrigger,
            },
        },
        PresentationOp::Audio {
            meta: PresentationOpMeta::new(18),
            op: AudioProjectionOp::BusControl {
                bus: AudioBus::Ambient,
                control: AudioBusControl::SetVolume { volume: 0.5 },
            },
        },
        PresentationOp::Audio {
            meta: PresentationOpMeta::new(19),
            op: AudioProjectionOp::BusControl {
                bus: AudioBus::Ui,
                control: AudioBusControl::SetMuted { muted: true },
            },
        },
    ];
    let frame = PresentationFrameDiff::try_from_ops(ops).unwrap();
    let encoded = frame.encode_json().unwrap();
    assert_eq!(PresentationFrameDiff::decode_json(&encoded).unwrap(), frame);
    for marker in [
        "emit",
        "create",
        "update",
        "destroy",
        "voiceControl",
        "busControl",
        "retrigger",
        "setVolume",
        "setMuted",
        "audio",
        "billboard",
        "particle",
        "telemetryOverlay",
        "animation",
    ] {
        assert!(
            encoded.contains(marker),
            "missing operation marker {marker}"
        );
    }
}

#[test]
fn particle_patch_distinguishes_omitted_collision_from_explicit_clear() {
    let omitted = ParticleEmitterPatch::default();
    let omitted_json = serde_json::to_value(&omitted).unwrap();
    assert!(omitted_json.get("collision").is_none());

    let clear = ParticleEmitterPatch {
        collision: Some(None),
        ..ParticleEmitterPatch::default()
    };
    let clear_json = serde_json::to_value(&clear).unwrap();
    assert_eq!(clear_json.get("collision"), Some(&serde_json::Value::Null));
    assert_eq!(
        serde_json::from_value::<ParticleEmitterPatch>(clear_json).unwrap(),
        clear
    );
}

#[test]
fn frame_rejects_sequence_gaps_and_unknown_fields() {
    let error = PresentationFrameDiff::try_from_ops(vec![PresentationOp::Audio {
        meta: PresentationOpMeta::new(1),
        op: AudioProjectionOp::Emit {
            signal_handle: AudioSignalHandle::new(1),
            signal_id: "late".into(),
            descriptor: audio(),
        },
    }])
    .unwrap_err();
    assert_eq!(
        error,
        PresentationFrameError::NonContiguousSequence {
            expected: 0,
            actual: 1
        }
    );

    let json = fixture_frame().encode_json().unwrap();
    let with_unknown = json.replacen(
        "\"schemaVersion\": 1,",
        "\"schemaVersion\": 1,\n  \"unexpected\": true,",
        1,
    );
    assert!(matches!(
        PresentationFrameDiff::decode_json(&with_unknown),
        Err(PresentationJsonError::Decode(_))
    ));
}

#[test]
fn frame_rejects_presentation_identities_outside_the_json_safe_range() {
    let unsafe_value = (1_u64 << 53) + 1;
    let frame = PresentationFrameDiff {
        publication: None,
        schema_version: PRESENTATION_FRAME_SCHEMA_VERSION,
        ops: vec![PresentationOp::Billboard {
            meta: PresentationOpMeta::new(0),
            op: BillboardProjectionOp::Create {
                handle: BillboardHandle::new(unsafe_value),
                descriptor: billboard(),
            },
        }],
    };
    assert_eq!(
        frame.validate(),
        Err(PresentationFrameError::UnsafeJsonInteger {
            sequence: 0,
            field: "billboard.handle",
            value: unsafe_value,
        })
    );
    assert!(matches!(
        frame.encode_json(),
        Err(PresentationJsonError::InvalidFrame(
            PresentationFrameError::UnsafeJsonInteger { .. }
        ))
    ));
}

#[test]
fn checked_in_fixture_is_the_canonical_cross_language_frame() {
    let fixture = include_str!("../../../../fixtures/render/presentation-frame-v1.json");
    let decoded = PresentationFrameDiff::decode_json(fixture).unwrap();
    assert_eq!(decoded, fixture_frame());
    assert_eq!(decoded.encode_json().unwrap(), fixture.trim_end());
}

#[test]
fn legacy_sprite_descriptor_decodes_and_new_writers_emit_visual() {
    let legacy = r#"{
      "anchor":{"kind":"world","position":[0.0,1.0,0.0]},
      "sprite":{"asset":"sprite-sheet/sparks","contentHash":"dd","frameCount":4},
      "ratePerSecond":8.0,"burstCount":4,"lifetimeSeconds":[0.2,0.6],
      "velocityMin":[-1.0,1.0,-1.0],"velocityMax":[1.0,2.0,1.0],
      "acceleration":[0.0,-4.0,0.0],
      "sizeCurve":[{"age":0.0,"value":0.25},{"age":1.0,"value":0.0}],
      "colorCurve":[{"age":0.0,"color":[1.0,0.8,0.2,1.0]},{"age":1.0,"color":[1.0,0.2,0.0,0.0]}],
      "flipbookFramesPerSecond":12.0,"seed":7,"maxParticles":32,"visible":true
    }"#;
    let descriptor: ParticleEmitterDescriptor = serde_json::from_str(legacy).unwrap();
    assert!(matches!(
        descriptor.visual,
        ParticleVisual::Billboard { .. }
    ));
    let encoded = serde_json::to_value(descriptor).unwrap();
    assert!(encoded.get("visual").is_some());
    assert!(encoded.get("sprite").is_none());
}

#[test]
fn mixed_domain_frame_rejection_does_not_commit_earlier_operations() {
    let audio_asset = ResolvedRenderAsset {
        id: "audio/pulse".into(),
        kind: RenderAssetKind::Audio,
        content_hash: Some("aa".into()),
        version: 1,
    };
    let assets = BTreeMap::from([(audio_asset.id.clone(), audio_asset)]);
    let targets = BTreeSet::<RenderHandle>::new();
    let frame = PresentationFrameDiff::try_from_ops(vec![
        PresentationOp::Audio {
            meta: PresentationOpMeta::new(0),
            op: AudioProjectionOp::Create {
                handle: AudioHandle::new(1),
                descriptor: audio(),
            },
        },
        PresentationOp::TelemetryOverlay {
            meta: PresentationOpMeta::new(1),
            op: TelemetryOverlayProjectionOp::Create {
                handle: TelemetryOverlayHandle::new(1),
                descriptor: TelemetryOverlayDescriptor {
                    refresh_interval_ms: 1,
                    ..telemetry()
                },
            },
        },
    ])
    .unwrap();
    let mut projectors = PresentationProjectorSet::default();
    assert!(matches!(
        projectors.project_frame(&assets, &targets, frame),
        Err(PresentationProjectionError::Telemetry(_))
    ));
    let readout = projectors.readout();
    assert_eq!(readout.audio.active_sources, 0);
    assert_eq!(readout.telemetry.active_overlays, 0);
}
