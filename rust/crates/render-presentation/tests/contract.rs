use render_model::RenderHandle;
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
        max_distance: 10.0,
        rolloff: AudioRolloff::Linear,
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
        size_mode: Default::default(),
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

#[test]
fn every_presentation_operation_validates_in_one_frame() {
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
        PresentationOp::Particle {
            meta: PresentationOpMeta::new(11),
            op: ParticleProjectionOp::Destroy {
                handle: ParticleEmitterHandle::new(4),
            },
        },
        PresentationOp::Particle {
            meta: PresentationOpMeta::new(12),
            op: ParticleProjectionOp::Destroy {
                handle: ParticleEmitterHandle::new(5),
            },
        },
        PresentationOp::Particle {
            meta: PresentationOpMeta::new(13),
            op: ParticleProjectionOp::Destroy {
                handle: ParticleEmitterHandle::new(6),
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
    PresentationFrameDiff::try_from_ops(ops).unwrap();
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
fn retired_audio_signals_cross_a_rebind_as_transient_events() {
    let frame = PresentationFrameDiff::try_from_ops(vec![
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
            op: AudioProjectionOp::Retire {
                signal_handle: AudioSignalHandle::new(1),
            },
        },
    ])
    .expect("retirement frame validates");
    let transient = frame.transient_events();
    assert_eq!(transient.ops.len(), 2);
    assert!(matches!(
        transient.ops[1],
        PresentationOp::Audio {
            op: AudioProjectionOp::Retire { signal_handle },
            ..
        } if signal_handle.raw() == 1
    ));
}

#[test]
fn frame_rejects_sequence_gaps() {
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
