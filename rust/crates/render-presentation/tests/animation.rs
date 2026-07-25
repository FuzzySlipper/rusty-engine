use std::collections::{BTreeMap, BTreeSet};

use render_model::{RenderAssetKind, RenderHandle, ResolvedRenderAsset};
use render_presentation::*;

fn assets() -> BTreeMap<String, ResolvedRenderAsset> {
    let asset = ResolvedRenderAsset {
        id: "animated-mesh/hero".into(),
        kind: RenderAssetKind::AnimatedMesh,
        content_hash: Some("ff".into()),
        version: 3,
    };
    BTreeMap::from([(asset.id.clone(), asset)])
}

fn catalog() -> AnimationCatalog {
    AnimationCatalog {
        schema_version: ANIMATION_CATALOG_SCHEMA_VERSION,
        catalog_id: "hero.animation".into(),
        assets: vec![AnimationClipAsset {
            asset_id: "animated-mesh/hero".into(),
            content_hash: "ff".into(),
            clips: vec!["idle".into(), "walk".into(), "run".into(), "jump".into()],
        }],
        graphs: vec![AnimationGraphDefinition {
            graph_id: "hero.locomotion".into(),
            version: 2,
            asset_id: "animated-mesh/hero".into(),
            initial_state_id: "idle".into(),
            parameters: vec![
                AnimationParameterDefinition {
                    parameter_id: "speed".into(),
                    kind: AnimationParameterKind::Float,
                    default_value: AnimationParameterValue::Float(0),
                },
                AnimationParameterDefinition {
                    parameter_id: "grounded".into(),
                    kind: AnimationParameterKind::Bool,
                    default_value: AnimationParameterValue::Bool(true),
                },
                AnimationParameterDefinition {
                    parameter_id: "jump".into(),
                    kind: AnimationParameterKind::Trigger,
                    default_value: AnimationParameterValue::Trigger(false),
                },
            ],
            states: vec![
                AnimationStateDefinition {
                    state_id: "idle".into(),
                    motion: AnimationMotionDefinition::Clip {
                        clip_id: "idle".into(),
                        speed_milli: 1_000,
                    },
                },
                AnimationStateDefinition {
                    state_id: "move".into(),
                    motion: AnimationMotionDefinition::LinearBlend {
                        parameter_id: "speed".into(),
                        low_clip_id: "walk".into(),
                        high_clip_id: "run".into(),
                        minimum_milli: 0,
                        maximum_milli: 1_000,
                        speed_milli: 1_000,
                    },
                },
                AnimationStateDefinition {
                    state_id: "air".into(),
                    motion: AnimationMotionDefinition::Clip {
                        clip_id: "jump".into(),
                        speed_milli: 1_000,
                    },
                },
            ],
            transitions: vec![
                AnimationTransitionDefinition {
                    transition_id: "idle.jump".into(),
                    from_state_id: "idle".into(),
                    to_state_id: "air".into(),
                    priority: 0,
                    duration_ticks: 0,
                    conditions: vec![AnimationCondition::TriggerSet {
                        parameter_id: "jump".into(),
                    }],
                },
                AnimationTransitionDefinition {
                    transition_id: "idle.move".into(),
                    from_state_id: "idle".into(),
                    to_state_id: "move".into(),
                    priority: 1,
                    duration_ticks: 2,
                    conditions: vec![AnimationCondition::FloatGreaterThan {
                        parameter_id: "speed".into(),
                        threshold_milli: 0,
                    }],
                },
                AnimationTransitionDefinition {
                    transition_id: "move.jump".into(),
                    from_state_id: "move".into(),
                    to_state_id: "air".into(),
                    priority: 0,
                    duration_ticks: 0,
                    conditions: vec![AnimationCondition::TriggerSet {
                        parameter_id: "jump".into(),
                    }],
                },
                AnimationTransitionDefinition {
                    transition_id: "move.idle".into(),
                    from_state_id: "move".into(),
                    to_state_id: "idle".into(),
                    priority: 1,
                    duration_ticks: 1,
                    conditions: vec![AnimationCondition::FloatLessThanOrEqual {
                        parameter_id: "speed".into(),
                        threshold_milli: 0,
                    }],
                },
                AnimationTransitionDefinition {
                    transition_id: "air.idle".into(),
                    from_state_id: "air".into(),
                    to_state_id: "idle".into(),
                    priority: 0,
                    duration_ticks: 1,
                    conditions: vec![AnimationCondition::BoolEquals {
                        parameter_id: "grounded".into(),
                        value: true,
                    }],
                },
            ],
        }],
    }
}

fn validated_catalog() -> ValidatedAnimationCatalog {
    validate_animation_catalog(catalog(), &assets()).expect("fixture catalog validates")
}

#[test]
fn catalog_rejects_invalid_graphs_speeds_and_asset_identity_drift() {
    let mut ambiguous = catalog();
    ambiguous.graphs[0].transitions[1].priority = 0;
    let error = validate_animation_catalog(ambiguous, &assets()).unwrap_err();
    assert!(error
        .diagnostics
        .iter()
        .any(|item| item.code == AnimationCatalogDiagnosticCode::AmbiguousTransition));

    let mut changed_assets = assets();
    changed_assets
        .get_mut("animated-mesh/hero")
        .unwrap()
        .content_hash = Some("changed".into());
    let error = validate_animation_catalog(catalog(), &changed_assets).unwrap_err();
    assert!(error
        .diagnostics
        .iter()
        .any(|item| item.code == AnimationCatalogDiagnosticCode::ContentHashMismatch));

    let mut invalid_graph = catalog();
    invalid_graph.graphs[0].states[0].motion = AnimationMotionDefinition::Clip {
        clip_id: "missing".into(),
        speed_milli: 1_000,
    };
    invalid_graph.graphs[0]
        .states
        .push(AnimationStateDefinition {
            state_id: "orphan".into(),
            motion: AnimationMotionDefinition::Clip {
                clip_id: "idle".into(),
                speed_milli: 1_000,
            },
        });
    invalid_graph.graphs[0]
        .transitions
        .push(AnimationTransitionDefinition {
            transition_id: "idle.invalid".into(),
            from_state_id: "idle".into(),
            to_state_id: "move".into(),
            priority: 1,
            duration_ticks: 1,
            conditions: vec![AnimationCondition::FloatGreaterThan {
                parameter_id: "grounded".into(),
                threshold_milli: 0,
            }],
        });
    let error = validate_animation_catalog(invalid_graph, &assets()).unwrap_err();
    for expected in [
        AnimationCatalogDiagnosticCode::MissingClip,
        AnimationCatalogDiagnosticCode::UnreachableState,
        AnimationCatalogDiagnosticCode::AmbiguousTransition,
        AnimationCatalogDiagnosticCode::ParameterTypeMismatch,
    ] {
        assert!(
            error.diagnostics.iter().any(|item| item.code == expected),
            "missing diagnostic {expected:?}"
        );
    }

    for motion in [
        AnimationMotionDefinition::Clip {
            clip_id: "idle".into(),
            speed_milli: 0,
        },
        AnimationMotionDefinition::LinearBlend {
            parameter_id: "speed".into(),
            low_clip_id: "walk".into(),
            high_clip_id: "run".into(),
            minimum_milli: 0,
            maximum_milli: 1_000,
            speed_milli: -1,
        },
    ] {
        let mut invalid_speed = catalog();
        invalid_speed.graphs[0].states[0].motion = motion;
        let error = validate_animation_catalog(invalid_speed, &assets()).unwrap_err();
        assert!(error
            .diagnostics
            .iter()
            .any(|item| item.code == AnimationCatalogDiagnosticCode::InvalidPlaybackSpeed));
    }
}

#[test]
fn identical_explicit_inputs_produce_identical_state_without_replay_bookkeeping() {
    let mut left = AnimationControllerService::new(validated_catalog());
    let mut right = AnimationControllerService::new(validated_catalog());
    for controller in [&mut left, &mut right] {
        controller.attach(7, "hero.locomotion").unwrap();
        controller.set_float(7, "speed", 500).unwrap();
        controller.tick(7, 1).unwrap();
        controller.tick(7, 2).unwrap();
        controller.tick(7, 3).unwrap();
    }
    assert_eq!(left.state(7).unwrap(), right.state(7).unwrap());
}

#[test]
fn controller_resolves_blends_and_transition_timing_deterministically() {
    let mut controller = AnimationControllerService::new(validated_catalog());
    let attached = controller.attach(1, "hero.locomotion").unwrap();
    assert_eq!(attached.state.unwrap().motion.clip_a, "idle");

    controller.set_float(1, "speed", 500).unwrap();
    let started = controller.tick(1, 1).unwrap().state.unwrap();
    let transition = started.transition.as_ref().unwrap();
    assert_eq!(transition.transition_id, "idle.move");
    assert_eq!(transition.target_motion.blend_weight_milli, 500);
    let started_fact = started.transition_fact.as_ref().unwrap();
    assert_eq!(started_fact.controller_tick, 1);
    assert_eq!(started_fact.transition_id, "idle.move");
    assert_eq!(started_fact.from_state_id, "idle");
    assert_eq!(started_fact.to_state_id, "move");
    assert_eq!(started_fact.moment, AnimationTransitionFactMoment::Started);
    assert_eq!(started_fact.duration_ticks, 2);

    let advancing = controller.tick(1, 2).unwrap().state.unwrap();
    assert_eq!(advancing.transition.unwrap().elapsed_ticks, 1);
    let completed = controller.tick(1, 3).unwrap().state.unwrap();
    assert_eq!(completed.current_state_id, "move");
    assert_eq!(completed.motion.clip_a, "walk");
    assert_eq!(completed.motion.clip_b.as_deref(), Some("run"));
    assert_eq!(completed.motion.blend_weight_milli, 500);
    let completed_fact = completed.transition_fact.as_ref().unwrap();
    assert_eq!(completed_fact.controller_tick, 3);
    assert_eq!(completed_fact.transition_id, "idle.move");
    assert_eq!(completed_fact.from_state_id, "idle");
    assert_eq!(completed_fact.to_state_id, "move");
    assert_eq!(
        completed_fact.moment,
        AnimationTransitionFactMoment::Completed
    );
    assert_eq!(completed_fact.duration_ticks, 2);

    let error = controller.tick(1, 5).unwrap_err();
    assert_eq!(
        error,
        AnimationControllerError::TickNotContiguous {
            expected: 4,
            actual: 5
        }
    );
    assert_eq!(controller.state(1).unwrap().controller_tick, 3);
}

#[test]
fn controller_priority_trigger_consumption_batch_rollback_and_reset_are_explicit() {
    let mut controller = AnimationControllerService::new(validated_catalog());
    controller.attach(2, "hero.locomotion").unwrap();
    controller.set_float(2, "speed", 500).unwrap();
    controller.fire_trigger(2, "jump").unwrap();
    let jumped = controller.tick(2, 1).unwrap().state.unwrap();
    assert_eq!(jumped.current_state_id, "air");
    assert_eq!(
        jumped.parameters.get("jump"),
        Some(&AnimationParameterValue::Trigger(false))
    );
    assert_eq!(jumped.transition_fact.unwrap().transition_id, "idle.jump");

    let error = controller
        .apply_batch(vec![
            (
                99,
                AnimationControllerInput::Attach {
                    graph_id: "hero.locomotion".into(),
                },
            ),
            (
                99,
                AnimationControllerInput::SetFloat {
                    parameter_id: "missing".into(),
                    value_milli: 1,
                },
            ),
        ])
        .unwrap_err();
    assert_eq!(
        error,
        AnimationControllerError::UnknownParameter("missing".into())
    );
    assert_eq!(
        controller.state(99).unwrap_err(),
        AnimationControllerError::ControllerMissing(99)
    );

    controller.reset();
    controller
        .attach(2, "hero.locomotion")
        .expect("controller may reopen after reset");
    assert_eq!(controller.state(2).unwrap().revision, 0);
}

fn descriptor(
    target: RenderHandle,
    state: &AnimationControllerState,
) -> AnimationProjectionDescriptor {
    AnimationProjectionDescriptor {
        target,
        asset: state.asset_id.clone(),
        content_hash: "ff".into(),
        tick_duration_millis: 16,
        controller: state.into(),
    }
}

#[test]
fn animation_projection_validates_handles_targets_revisions_and_batches_atomically() {
    let assets = assets();
    let target = RenderHandle::new(41);
    let targets = BTreeSet::from([target]);
    let mut controller = AnimationControllerService::new(validated_catalog());
    let initial = controller
        .attach(7, "hero.locomotion")
        .unwrap()
        .state
        .unwrap();
    let mut projector = AnimationProjector::new();

    let error = projector
        .project_batch(
            &assets,
            &targets,
            vec![
                (
                    PresentationOpMeta::new(0),
                    AnimationProjectionOp::Create {
                        handle: AnimationProjectionHandle::new(10),
                        descriptor: descriptor(target, &initial),
                    },
                ),
                (
                    PresentationOpMeta::new(1),
                    AnimationProjectionOp::Create {
                        handle: AnimationProjectionHandle::new(11),
                        descriptor: descriptor(
                            RenderHandle::new(999),
                            &AnimationControllerState {
                                entity: 8,
                                ..initial.clone()
                            },
                        ),
                    },
                ),
            ],
        )
        .unwrap_err();
    assert_eq!(error.code, AnimationProjectionDiagnosticCode::UnknownTarget);
    assert_eq!(projector.readout().active_controllers, 0);

    projector
        .create_for_state(
            &assets,
            &targets,
            AnimationProjectionTarget {
                target,
                content_hash: "ff".into(),
                tick_duration_millis: 16,
            },
            &initial,
            PresentationOpMeta::new(0),
        )
        .unwrap();
    assert_eq!(projector.handle(7), Some(AnimationProjectionHandle::new(1)));

    assert_eq!(
        projector
            .project(
                &assets,
                &targets,
                PresentationOpMeta::new(1),
                AnimationProjectionOp::Create {
                    handle: AnimationProjectionHandle::new(1),
                    descriptor: descriptor(
                        target,
                        &AnimationControllerState {
                            entity: 8,
                            ..initial.clone()
                        }
                    ),
                },
            )
            .unwrap_err()
            .code,
        AnimationProjectionDiagnosticCode::DuplicateHandle
    );

    let changed = controller
        .set_float(7, "speed", 500)
        .unwrap()
        .state
        .unwrap();
    projector
        .update_for_state(&assets, &targets, &changed, PresentationOpMeta::new(2))
        .unwrap();
    assert_eq!(
        projector
            .update_for_state(&assets, &targets, &changed, PresentationOpMeta::new(3),)
            .unwrap_err()
            .code,
        AnimationProjectionDiagnosticCode::StaleRevision
    );

    projector
        .destroy_entity(7, PresentationOpMeta::new(4))
        .unwrap();
    assert_eq!(projector.readout().active_controllers, 0);
    projector.reset();
    projector
        .create_for_state(
            &assets,
            &targets,
            AnimationProjectionTarget {
                target,
                content_hash: "ff".into(),
                tick_duration_millis: 16,
            },
            &changed,
            PresentationOpMeta::new(0),
        )
        .unwrap();
    assert_eq!(projector.handle(7), Some(AnimationProjectionHandle::new(1)));
}
