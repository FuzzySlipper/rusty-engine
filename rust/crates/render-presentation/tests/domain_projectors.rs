use std::collections::BTreeMap;

use render_model::{RenderAssetKind, ResolvedRenderAsset};
use render_presentation::*;

fn assets() -> BTreeMap<String, ResolvedRenderAsset> {
    [
        asset("audio/pulse", RenderAssetKind::Audio, "aa"),
        asset("font/ui", RenderAssetKind::Font, "bb"),
        asset("texture/alert", RenderAssetKind::Texture, "cc"),
        asset("sprite-sheet/sparks", RenderAssetKind::SpriteAtlas, "dd"),
    ]
    .into_iter()
    .map(|asset| (asset.id.clone(), asset))
    .collect()
}

fn asset(id: &str, kind: RenderAssetKind, hash: &str) -> ResolvedRenderAsset {
    ResolvedRenderAsset {
        id: id.to_string(),
        kind,
        content_hash: Some(hash.to_string()),
        version: 1,
    }
}

fn audio_descriptor() -> AudioSourceDescriptor {
    AudioSourceDescriptor {
        clip: AudioClipRef {
            asset: "audio/pulse".into(),
            content_hash: "aa".into(),
        },
        bus: AudioBus::Sfx,
        volume: 0.8,
        pitch: 1.0,
        looping: true,
        spatial_blend: 1.0,
        attenuation: 12.0,
        pan: 0.0,
        emitter: AudioEmitter::World3d {
            position: [1.0, 2.0, 3.0],
        },
    }
}

fn billboard_descriptor() -> BillboardDescriptor {
    BillboardDescriptor {
        anchor: BillboardAnchor::EntityAttached {
            entity: 42,
            offset: [0.0, 1.8, 0.0],
        },
        content: BillboardContent::Icon {
            texture: BillboardTextureRef {
                asset: "texture/alert".into(),
                content_hash: "cc".into(),
            },
            alt_key: "warning".into(),
            fallback_alt: "Warning".into(),
        },
        font: BillboardFontRef::Asset {
            asset: "font/ui".into(),
            content_hash: "bb".into(),
            family: "Rusty UI".into(),
        },
        height_pixels: 24.0,
        color: [1.0, 1.0, 1.0, 1.0],
        background: [0.0, 0.0, 0.0, 0.7],
        max_distance: 40.0,
        layer: BillboardLayer::Occluded,
        visible: true,
        layout: None,
    }
}

fn particle_descriptor() -> ParticleEmitterDescriptor {
    ParticleEmitterDescriptor {
        anchor: ParticleAnchor::World {
            position: [1.0, 2.0, 3.0],
        },
        visual: ParticleVisual::Billboard {
            sprite: ParticleSpriteRef {
                asset: "sprite-sheet/sparks".into(),
                content_hash: "dd".into(),
                frame_count: 4,
            },
        },
        rate_per_second: 12.0,
        burst_count: 8,
        lifetime_seconds: [0.2, 0.6],
        velocity_min: [-1.0, 1.0, -1.0],
        velocity_max: [1.0, 3.0, 1.0],
        acceleration: [0.0, -4.0, 0.0],
        size_curve: vec![
            ParticleScalarKey {
                age: 0.0,
                value: 0.2,
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
        flipbook_frames_per_second: 16.0,
        seed: 44,
        max_particles: 64,
        visible: true,
        collision: None,
    }
}

fn telemetry_descriptor() -> TelemetryOverlayDescriptor {
    TelemetryOverlayDescriptor {
        title: "Runtime".into(),
        corner: TelemetryOverlayCorner::TopRight,
        refresh_interval_ms: 250,
        max_frame_time_samples: 60,
        visible: true,
    }
}

#[test]
fn audio_batch_is_atomic_and_reset_clears_retained_and_impulse_state() {
    let assets = assets();
    let handle = AudioHandle::new(1);
    let mut projector = AudioProjector::default();
    let error = projector
        .project_batch(
            &assets,
            vec![
                (
                    PresentationOpMeta::new(0),
                    AudioProjectionOp::Create {
                        handle,
                        descriptor: audio_descriptor(),
                    },
                ),
                (
                    PresentationOpMeta::new(1),
                    AudioProjectionOp::Update {
                        handle,
                        patch: AudioSourcePatch {
                            pitch: Some(0.0),
                            ..AudioSourcePatch::default()
                        },
                    },
                ),
            ],
        )
        .expect_err("invalid later update rejects the complete batch");
    assert_eq!(error.code, AudioProjectionDiagnosticCode::InvalidDescriptor);
    assert_eq!(projector.readout().active_sources, 0);
    assert_eq!(projector.readout().diagnostics.len(), 1);

    projector
        .project(
            &assets,
            PresentationOpMeta::new(0),
            AudioProjectionOp::Emit {
                signal_id: "shot:1".into(),
                descriptor: AudioSourceDescriptor {
                    looping: false,
                    ..audio_descriptor()
                },
            },
        )
        .unwrap();
    let duplicate = projector
        .project(
            &assets,
            PresentationOpMeta::new(1),
            AudioProjectionOp::Emit {
                signal_id: "shot:1".into(),
                descriptor: AudioSourceDescriptor {
                    looping: false,
                    ..audio_descriptor()
                },
            },
        )
        .unwrap_err();
    assert_eq!(
        duplicate.code,
        AudioProjectionDiagnosticCode::DuplicateSignal
    );
    projector.reset();
    assert_eq!(projector.readout().emitted_signals, 0);
    assert!(projector.readout().diagnostics.is_empty());
}

#[test]
fn audio_diagnostics_are_bounded_oldest_first_and_reset_eviction_state() {
    let assets = assets();
    let mut projector = AudioProjector::default();
    let total = MAX_AUDIO_DIAGNOSTICS + 2;

    for sequence in 0..total {
        let diagnostic = projector
            .project(
                &assets,
                PresentationOpMeta::new(sequence as u32),
                AudioProjectionOp::Destroy {
                    handle: AudioHandle::new(sequence as u64 + 1),
                },
            )
            .expect_err("unknown handles produce retained diagnostics");
        assert_eq!(diagnostic.code, AudioProjectionDiagnosticCode::UnknownHandle);
    }

    let readout = projector.readout();
    assert_eq!(readout.retained_diagnostic_count, MAX_AUDIO_DIAGNOSTICS as u32);
    assert_eq!(readout.evicted_diagnostic_count, 2);
    assert_eq!(readout.diagnostics.len(), MAX_AUDIO_DIAGNOSTICS);
    assert_eq!(readout.diagnostics.first().unwrap().sequence, 2);
    assert_eq!(readout.diagnostics.last().unwrap().sequence, (total - 1) as u32);

    projector.reset();
    let readout = projector.readout();
    assert_eq!(readout.retained_diagnostic_count, 0);
    assert_eq!(readout.evicted_diagnostic_count, 0);
    assert!(readout.diagnostics.is_empty());
}

#[test]
fn audio_retained_voice_controls_and_fixed_bus_state_are_owner_truth() {
    let assets = assets();
    let handle = AudioHandle::new(7);
    let descriptor = audio_descriptor();
    let mut projector = AudioProjector::default();

    projector
        .project(
            &assets,
            PresentationOpMeta::new(0),
            AudioProjectionOp::Create {
                handle,
                descriptor: descriptor.clone(),
            },
        )
        .unwrap();
    assert_eq!(
        projector.voice(handle).unwrap().desired_state,
        AudioVoiceDesiredState::Playing
    );

    for (sequence, control, expected) in [
        (1, AudioVoiceControl::Pause, AudioVoiceDesiredState::Paused),
        // Repeating the same desired state is intentionally idempotent.
        (2, AudioVoiceControl::Pause, AudioVoiceDesiredState::Paused),
        (3, AudioVoiceControl::Resume, AudioVoiceDesiredState::Playing),
        (4, AudioVoiceControl::Resume, AudioVoiceDesiredState::Playing),
        // Retrigger stays on the same retained handle and has the same
        // descriptor; the wire operation directs host realization to offset 0.
        (5, AudioVoiceControl::Retrigger, AudioVoiceDesiredState::Playing),
    ] {
        projector
            .project(
                &assets,
                PresentationOpMeta::new(sequence),
                AudioProjectionOp::VoiceControl { handle, control },
            )
            .unwrap();
        let voice = projector.voice(handle).unwrap();
        assert_eq!(voice.desired_state, expected);
        assert_eq!(voice.descriptor, descriptor);
        assert_eq!(
            projector.readout().paused_sources,
            u32::from(expected == AudioVoiceDesiredState::Paused)
        );
    }

    let unknown = projector
        .project(
            &assets,
            PresentationOpMeta::new(6),
            AudioProjectionOp::VoiceControl {
                handle: AudioHandle::new(99),
                control: AudioVoiceControl::Pause,
            },
        )
        .unwrap_err();
    assert_eq!(unknown.code, AudioProjectionDiagnosticCode::UnknownHandle);

    assert_eq!(projector.bus(AudioBus::Ambient).volume, 1.0);
    assert!(!projector.bus(AudioBus::Ambient).muted);
    projector
        .project(
            &assets,
            PresentationOpMeta::new(7),
            AudioProjectionOp::BusControl {
                bus: AudioBus::Sfx,
                control: AudioBusControl::SetVolume { volume: 0.25 },
            },
        )
        .unwrap();
    projector
        .project(
            &assets,
            PresentationOpMeta::new(8),
            AudioProjectionOp::BusControl {
                bus: AudioBus::Sfx,
                control: AudioBusControl::SetMuted { muted: true },
            },
        )
        .unwrap();
    assert_eq!(
        projector.bus(AudioBus::Sfx),
        AudioBusReadout {
            bus: AudioBus::Sfx,
            volume: 0.25,
            muted: true,
        }
    );
    assert_eq!(projector.readout().active_sources, 1);
    assert_eq!(projector.readout().paused_sources, 0);

    let error = projector
        .project_batch(
            &assets,
            vec![
                (
                    PresentationOpMeta::new(9),
                    AudioProjectionOp::BusControl {
                        bus: AudioBus::Ui,
                        control: AudioBusControl::SetMuted { muted: true },
                    },
                ),
                (
                    PresentationOpMeta::new(10),
                    AudioProjectionOp::BusControl {
                        bus: AudioBus::Sfx,
                        control: AudioBusControl::SetVolume { volume: 2.0 },
                    },
                ),
            ],
        )
        .unwrap_err();
    assert_eq!(error.code, AudioProjectionDiagnosticCode::InvalidControl);
    assert!(!projector.bus(AudioBus::Ui).muted);

    projector.reset();
    assert!(projector.voice(handle).is_none());
    assert_eq!(projector.bus(AudioBus::Sfx).volume, 1.0);
    assert!(!projector.bus(AudioBus::Sfx).muted);
}

#[test]
fn audio_rejects_wrong_kind_and_content_identity() {
    let mut wrong_kind_assets = assets();
    wrong_kind_assets.get_mut("audio/pulse").unwrap().kind = RenderAssetKind::Font;
    let mut projector = AudioProjector::default();
    assert_eq!(
        projector
            .project(
                &wrong_kind_assets,
                PresentationOpMeta::new(0),
                AudioProjectionOp::Create {
                    handle: AudioHandle::new(1),
                    descriptor: audio_descriptor(),
                },
            )
            .unwrap_err()
            .code,
        AudioProjectionDiagnosticCode::AssetKindMismatch
    );

    let mut changed_assets = assets();
    changed_assets.get_mut("audio/pulse").unwrap().content_hash = Some("changed".into());
    assert_eq!(
        projector
            .project(
                &changed_assets,
                PresentationOpMeta::new(1),
                AudioProjectionOp::Create {
                    handle: AudioHandle::new(1),
                    descriptor: audio_descriptor(),
                },
            )
            .unwrap_err()
            .code,
        AudioProjectionDiagnosticCode::ContentHashMismatch
    );
}

#[test]
fn billboard_assets_bounds_and_retained_lifecycle_are_checked() {
    let assets = assets();
    let handle = BillboardHandle::new(7);
    let mut projector = BillboardProjector::default();
    projector
        .project(
            &assets,
            PresentationOpMeta::new(0),
            BillboardProjectionOp::Create {
                handle,
                descriptor: billboard_descriptor(),
            },
        )
        .unwrap();
    assert_eq!(projector.readout().referenced_fonts, 1);
    assert_eq!(projector.readout().referenced_icons, 1);

    let error = projector
        .project(
            &assets,
            PresentationOpMeta::new(1),
            BillboardProjectionOp::Update {
                handle,
                patch: BillboardPatch {
                    height_pixels: Some(2.0),
                    ..BillboardPatch::default()
                },
            },
        )
        .unwrap_err();
    assert_eq!(
        error.code,
        BillboardProjectionDiagnosticCode::InvalidDescriptor
    );
    assert_eq!(projector.descriptor(handle), Some(&billboard_descriptor()));

    assert_eq!(
        projector
            .project(
                &assets,
                PresentationOpMeta::new(2),
                BillboardProjectionOp::Create {
                    handle,
                    descriptor: billboard_descriptor(),
                },
            )
            .unwrap_err()
            .code,
        BillboardProjectionDiagnosticCode::DuplicateHandle
    );
    projector
        .project(
            &assets,
            PresentationOpMeta::new(3),
            BillboardProjectionOp::Destroy { handle },
        )
        .unwrap();
    assert_eq!(projector.readout().active_billboards, 0);
}

#[test]
fn particle_curves_signal_ids_and_reservation_budget_fail_closed() {
    let assets = assets();
    let limits = ParticleProjectionLimits {
        max_active_emitters: 1,
        max_particles_per_emitter: 64,
        max_reserved_particles: 64,
    };
    let mut projector = ParticleProjector::new(limits);
    let mut invalid = particle_descriptor();
    invalid.size_curve[1].age = 0.0;
    assert_eq!(
        projector
            .project(
                &assets,
                PresentationOpMeta::new(0),
                ParticleProjectionOp::Emit {
                    signal_id: "invalid".into(),
                    descriptor: invalid,
                },
            )
            .unwrap_err()
            .code,
        ParticleProjectionDiagnosticCode::InvalidDescriptor
    );

    let handle = ParticleEmitterHandle::new(1);
    let error = projector
        .project_batch(
            &assets,
            vec![
                (
                    PresentationOpMeta::new(0),
                    ParticleProjectionOp::Create {
                        handle,
                        descriptor: particle_descriptor(),
                    },
                ),
                (
                    PresentationOpMeta::new(1),
                    ParticleProjectionOp::Create {
                        handle: ParticleEmitterHandle::new(2),
                        descriptor: particle_descriptor(),
                    },
                ),
            ],
        )
        .unwrap_err();
    assert_eq!(error.code, ParticleProjectionDiagnosticCode::BudgetExceeded);
    assert_eq!(projector.readout().active_emitters, 0);

    projector
        .project(
            &assets,
            PresentationOpMeta::new(0),
            ParticleProjectionOp::Emit {
                signal_id: "impact:1".into(),
                descriptor: particle_descriptor(),
            },
        )
        .unwrap();
    assert_eq!(
        projector
            .project(
                &assets,
                PresentationOpMeta::new(1),
                ParticleProjectionOp::Emit {
                    signal_id: "impact:1".into(),
                    descriptor: particle_descriptor(),
                },
            )
            .unwrap_err()
            .code,
        ParticleProjectionDiagnosticCode::DuplicateSignal
    );
    projector.reset();
    assert_eq!(projector.readout().emitted_bursts, 0);
}

#[test]
fn cube_particles_validate_local_collision_without_an_asset_reference() {
    let mut descriptor = particle_descriptor();
    descriptor.visual = ParticleVisual::Cube;
    descriptor.flipbook_frames_per_second = 0.0;
    descriptor.collision = Some(ParticleCollisionDescriptor {
        radius: 0.1,
        restitution: 0.5,
        friction: 0.25,
        maximum_impacts: 4,
        sleep_speed: 0.1,
        limit_behavior: ParticleCollisionLimitBehavior::Sleep,
        volumes: vec![ParticleCollisionVolume::Plane {
            normal: [0.0, 1.0, 0.0],
            offset: -1.0,
        }],
    });
    let mut projector = ParticleProjector::default();
    projector
        .project(
            &BTreeMap::<String, ResolvedRenderAsset>::new(),
            PresentationOpMeta::new(0),
            ParticleProjectionOp::Emit {
                signal_id: "cube:1".into(),
                descriptor: descriptor.clone(),
            },
        )
        .unwrap();
    assert_eq!(projector.readout().referenced_sprites, 0);

    descriptor.collision.as_mut().unwrap().volumes[0] = ParticleCollisionVolume::Plane {
        normal: [0.0, 2.0, 0.0],
        offset: 0.0,
    };
    assert_eq!(
        projector
            .project(
                &BTreeMap::<String, ResolvedRenderAsset>::new(),
                PresentationOpMeta::new(1),
                ParticleProjectionOp::Emit {
                    signal_id: "cube:invalid".into(),
                    descriptor,
                },
            )
            .unwrap_err()
            .code,
        ParticleProjectionDiagnosticCode::InvalidDescriptor
    );
}

#[test]
fn telemetry_batch_and_reopen_have_no_hidden_state() {
    let handle = TelemetryOverlayHandle::new(9);
    let mut projector = TelemetryOverlayProjector::default();
    let error = projector
        .project_batch(vec![
            (
                PresentationOpMeta::new(0),
                TelemetryOverlayProjectionOp::Create {
                    handle,
                    descriptor: telemetry_descriptor(),
                },
            ),
            (
                PresentationOpMeta::new(1),
                TelemetryOverlayProjectionOp::Update {
                    handle,
                    patch: TelemetryOverlayPatch {
                        refresh_interval_ms: Some(1),
                        ..TelemetryOverlayPatch::default()
                    },
                },
            ),
        ])
        .unwrap_err();
    assert_eq!(
        error.code,
        TelemetryOverlayDiagnosticCode::InvalidDescriptor
    );
    assert_eq!(projector.readout().active_overlays, 0);

    projector
        .project(
            PresentationOpMeta::new(0),
            TelemetryOverlayProjectionOp::Create {
                handle,
                descriptor: telemetry_descriptor(),
            },
        )
        .unwrap();
    projector.reset();
    projector
        .project(
            PresentationOpMeta::new(0),
            TelemetryOverlayProjectionOp::Create {
                handle,
                descriptor: telemetry_descriptor(),
            },
        )
        .expect("same handle may be reopened after reset");
}

#[test]
fn every_retained_domain_rejects_unknown_handles() {
    let assets = assets();
    assert_eq!(
        AudioProjector::default()
            .project(
                &assets,
                PresentationOpMeta::new(0),
                AudioProjectionOp::Destroy {
                    handle: AudioHandle::new(99),
                },
            )
            .unwrap_err()
            .code,
        AudioProjectionDiagnosticCode::UnknownHandle
    );
    assert_eq!(
        BillboardProjector::default()
            .project(
                &assets,
                PresentationOpMeta::new(0),
                BillboardProjectionOp::Destroy {
                    handle: BillboardHandle::new(99),
                },
            )
            .unwrap_err()
            .code,
        BillboardProjectionDiagnosticCode::UnknownHandle
    );
    assert_eq!(
        ParticleProjector::default()
            .project(
                &assets,
                PresentationOpMeta::new(0),
                ParticleProjectionOp::Destroy {
                    handle: ParticleEmitterHandle::new(99),
                },
            )
            .unwrap_err()
            .code,
        ParticleProjectionDiagnosticCode::UnknownHandle
    );
    assert_eq!(
        TelemetryOverlayProjector::default()
            .project(
                PresentationOpMeta::new(0),
                TelemetryOverlayProjectionOp::Destroy {
                    handle: TelemetryOverlayHandle::new(99),
                },
            )
            .unwrap_err()
            .code,
        TelemetryOverlayDiagnosticCode::UnknownHandle
    );
}
