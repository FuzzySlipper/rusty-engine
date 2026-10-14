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
    let frame = PresentationFrameDiff::from_ops(vec![
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
    ]);
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
