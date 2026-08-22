//! Standard gameplay command descriptors and direct named-owner adapters.
//!
//! This optional tooling leaf composes `developer-command`, `engine-inspector`,
//! `gameplay-standard`, `gameplay-resolution`, and `gameplay-mechanics`. It
//! owns no game state, policy, transaction, queue, safe point, registry, or
//! transport. A downstream product selects descriptors and binds its existing
//! owners at its composition root.

#![forbid(unsafe_code)]

mod admin;
mod commands;
mod inspect;
mod resolution;

pub use admin::{admin_apply_effect, admin_remove_effect, admin_set_stat_base, admin_set_track};
pub use commands::{
    declare_standard_commands, descriptor_for, AdminApplyEffect, AdminRemoveEffect,
    AdminSetStatBase, AdminSetTrack, InspectEntity, InspectMechanics, InspectStandard,
    PlayStandardAttempt, PreviewStandardAttempt, StandardCommand,
};
pub use inspect::{
    inspect_entity, inspect_mechanics, inspect_standard_evidence, inspect_standard_plan,
};
pub use resolution::{
    execute_standard_attempt, preview_standard_attempt, validate_standard_plan, StandardAttempt,
};

#[cfg(test)]
mod tests {
    use std::convert::Infallible;

    use developer_command::{
        CommandBindings, CommandLane, CommandProfile, CommandRequest, DispatchFacts, ExpectedFacts,
        HandlerResult, ProfileId, RuntimeInstanceId,
    };
    use gameplay_resolution::{
        PolicyResult, Program, ResolutionId, ResolutionIdentity, ResolutionPlan, ResolutionPolicy,
        ResolutionTraceSink, ResolutionTransaction, StandardResolver,
    };
    use gameplay_standard::{
        CapabilityRequirementId, CapabilityRoleBinding, CapabilityRoleBindings, CapabilityRoleId,
        ExactExpr, ExactInputBundle, StandardOperation, StandardOperationContext,
        STANDARD_TRACK_CAPABILITY,
    };

    use crate::{
        declare_standard_commands, execute_standard_attempt, preview_standard_attempt,
        AdminSetStatBase, InspectEntity, PlayStandardAttempt, PreviewStandardAttempt,
        StandardAttempt,
    };

    fn runtime() -> RuntimeInstanceId {
        RuntimeInstanceId::parse("test-runtime").unwrap()
    }

    fn profile(lanes: impl IntoIterator<Item = CommandLane>) -> CommandProfile {
        CommandProfile::new(ProfileId::parse("test-profile").unwrap(), lanes).unwrap()
    }

    fn facts() -> DispatchFacts {
        DispatchFacts {
            runtime: runtime(),
            revision: 8,
            catalog_epoch: 13,
        }
    }

    fn request<C: developer_command::DeveloperCommand>(
        correlation: &str,
        payload: C::Request,
    ) -> CommandRequest<C::Request> {
        CommandRequest::new(
            C::descriptor().id().clone(),
            developer_command::CorrelationId::parse(correlation).unwrap(),
            runtime(),
            payload,
        )
    }

    #[test]
    fn module_descriptors_are_distinct_and_truthful_about_privilege() {
        let mut commands = CommandBindings::new(
            profile([CommandLane::Inspect, CommandLane::Preview]),
            facts(),
            16,
        )
        .unwrap();
        declare_standard_commands(&mut commands).unwrap();
        let discovery = commands.discover();
        assert_eq!(discovery.commands.len(), 9);
        assert!(discovery.commands.iter().all(|entry| !entry.bound));
        assert!(discovery.commands.iter().any(|entry| {
            entry.descriptor.id().as_str() == "standard.admin.track.set"
                && entry.descriptor.lane() == CommandLane::Admin
        }));
        let effect_apply = discovery
            .commands
            .iter()
            .find(|entry| entry.descriptor.id().as_str() == "standard.admin.effect.apply")
            .unwrap();
        assert!(effect_apply
            .descriptor
            .parameters()
            .iter()
            .any(|parameter| parameter.name == "provenance"));
        assert!(!effect_apply
            .descriptor
            .parameters()
            .iter()
            .any(|parameter| parameter.name == "source"));
    }

    #[test]
    fn declared_unbound_and_preflight_failures_never_enter_inspection_handler() {
        let mut commands =
            CommandBindings::new(profile([CommandLane::Inspect]), facts(), 16).unwrap();
        declare_standard_commands(&mut commands).unwrap();
        let unavailable = commands
            .dispatch::<InspectEntity>(request::<InspectEntity>("one", core_ids::EntityId::new(7)));
        assert!(matches!(unavailable.result, HandlerResult::Rejected(_)));
        assert!(unavailable.provenance.is_none());

        let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let entered = calls.clone();
        commands
            .bind::<InspectEntity, _>(move |_context, _entity| {
                entered.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                Ok::<_, Infallible>(None)
            })
            .unwrap();
        // A handler-bound inspect command still rejects stale runtime facts before entry.
        let mut bad_runtime = request::<InspectEntity>("two", core_ids::EntityId::new(7));
        bad_runtime.runtime = RuntimeInstanceId::parse("other-runtime").unwrap();
        assert!(matches!(
            commands.dispatch::<InspectEntity>(bad_runtime).result,
            HandlerResult::Rejected(_)
        ));
        // The same applies to revision and catalog guards.
        let stale_revision = request::<InspectEntity>("three", core_ids::EntityId::new(7))
            .with_expected(ExpectedFacts {
                profile: None,
                revision: Some(7),
                catalog_epoch: None,
            });
        assert!(matches!(
            commands.dispatch::<InspectEntity>(stale_revision).result,
            HandlerResult::Rejected(_)
        ));
        let stale_catalog = request::<InspectEntity>("four", core_ids::EntityId::new(7))
            .with_expected(ExpectedFacts {
                profile: None,
                revision: None,
                catalog_epoch: Some(12),
            });
        assert!(matches!(
            commands.dispatch::<InspectEntity>(stale_catalog).result,
            HandlerResult::Rejected(_)
        ));
        assert_eq!(commands.history().len(), 0);
        assert_eq!(calls.load(std::sync::atomic::Ordering::Relaxed), 0);
    }

    #[test]
    fn product_attempt_markers_keep_product_types_and_normal_dispatch_correlation_rules() {
        type Preview = PreviewStandardAttempt<u8, u16, &'static str, &'static str>;
        type Play = PlayStandardAttempt<u8, u16, &'static str, &'static str>;
        let mut commands = CommandBindings::new(
            profile([CommandLane::Preview, CommandLane::Play]),
            facts(),
            16,
        )
        .unwrap();
        commands
            .bind::<Preview, _>(|_, attempt: StandardAttempt<u8, u16>| {
                Ok(if attempt.intent == 1 {
                    "preview"
                } else {
                    "other"
                })
            })
            .unwrap();
        commands.bind::<Play, _>(|_, _| Ok("play")).unwrap();
        let attempt = || {
            StandardAttempt::new(
                ResolutionIdentity::root(
                    ResolutionId::new(1).unwrap(),
                    gameplay_resolution::CorrelationId::new(2).unwrap(),
                ),
                1_u8,
                vec![2_u16],
            )
        };
        let first = commands.dispatch::<Preview>(request::<Preview>("attempt", attempt()));
        assert!(matches!(first.result, HandlerResult::Success("preview")));
        let repeated = commands.dispatch::<Preview>(request::<Preview>("attempt", attempt()));
        assert!(matches!(repeated.result, HandlerResult::Rejected(_)));
    }

    #[test]
    fn exact_admin_marker_cannot_hide_owner_types() {
        let _adapter: fn(
            &mut entity_state::EntityState,
            &gameplay_mechanics::MechanicsCatalog,
            gameplay_mechanics::StatBaseMutationRequest,
        ) -> Result<
            gameplay_mechanics::StatBaseMutationReceipt,
            gameplay_mechanics::MechanicsError,
        > = crate::admin_set_stat_base;
        let _track_adapter: fn(
            &mut entity_state::EntityState,
            &gameplay_mechanics::MechanicsCatalog,
            gameplay_mechanics::TrackSetRequest,
        ) -> Result<
            gameplay_mechanics::TrackSetReceipt,
            gameplay_mechanics::MechanicsError,
        > = crate::admin_set_track;
        let _effect_apply_adapter: fn(
            &mut entity_state::EntityState,
            &gameplay_mechanics::MechanicsCatalog,
            gameplay_mechanics::EffectApplyRequest,
        ) -> Result<
            gameplay_mechanics::EffectMutationReceipt,
            gameplay_mechanics::MechanicsError,
        > = crate::admin_apply_effect;
        let _effect_remove_adapter: fn(
            &mut entity_state::EntityState,
            &gameplay_mechanics::MechanicsCatalog,
            gameplay_mechanics::EffectRemovalRequest,
        ) -> Result<
            gameplay_mechanics::EffectMutationReceipt,
            gameplay_mechanics::MechanicsError,
        > = crate::admin_remove_effect;
        fn exact<
            C: developer_command::DeveloperCommand<
                Request = gameplay_mechanics::StatBaseMutationRequest,
                Reply = gameplay_mechanics::StatBaseMutationReceipt,
                Error = gameplay_mechanics::MechanicsError,
            >,
        >() {
        }
        exact::<AdminSetStatBase>();
        fn exact_track<
            C: developer_command::DeveloperCommand<
                Request = gameplay_mechanics::TrackSetRequest,
                Reply = gameplay_mechanics::TrackSetReceipt,
                Error = gameplay_mechanics::MechanicsError,
            >,
        >() {
        }
        exact_track::<crate::AdminSetTrack>();
        fn exact_effect_apply<
            C: developer_command::DeveloperCommand<
                Request = gameplay_mechanics::EffectApplyRequest,
                Reply = gameplay_mechanics::EffectMutationReceipt,
                Error = gameplay_mechanics::MechanicsError,
            >,
        >() {
        }
        exact_effect_apply::<crate::AdminApplyEffect>();
        fn exact_effect_remove<
            C: developer_command::DeveloperCommand<
                Request = gameplay_mechanics::EffectRemovalRequest,
                Reply = gameplay_mechanics::EffectMutationReceipt,
                Error = gameplay_mechanics::MechanicsError,
            >,
        >() {
        }
        exact_effect_remove::<crate::AdminRemoveEffect>();
    }

    fn scalar(value: i64) -> gameplay_mechanics::MechanicsScalar {
        gameplay_mechanics::MechanicsScalar::new(value).unwrap()
    }

    fn mechanics_catalog() -> gameplay_mechanics::MechanicsCatalog {
        gameplay_mechanics::MechanicsCatalog::admit(
            gameplay_mechanics::MechanicsCatalogDefinition {
                version: gameplay_mechanics::CatalogVersion::parse("commands.v1").unwrap(),
                stats: vec![gameplay_mechanics::StatDefinition {
                    id: gameplay_mechanics::StatId::parse("vitality").unwrap(),
                    minimum: scalar(0),
                    maximum: scalar(100),
                }],
                tracks: vec![gameplay_mechanics::TrackDefinition {
                    id: gameplay_mechanics::TrackId::parse("health").unwrap(),
                    minimum: scalar(0),
                    maximum: gameplay_mechanics::TrackMaximum::Fixed { value: scalar(20) },
                }],
                sources: vec![],
                damage_kinds: vec![],
                effects: vec![],
                capacity_metrics: vec![],
                items: vec![],
                equipment_slots: vec![],
            },
        )
        .unwrap()
    }

    fn mechanics_state() -> entity_state::EntityState {
        let entity = core_ids::EntityId::new(70);
        let mut state = entity_state::EntityState::from_definitions_with_registry(
            gameplay_mechanics::gameplay_component_registry().unwrap(),
            [entity_state::EntityDefinition::new(entity, "fixture")],
        )
        .unwrap();
        let revision = state
            .component_revision::<gameplay_mechanics::StatsComponent>(entity)
            .unwrap();
        entity_state::EntityAuthoringService
            .attach_component(
                &mut state,
                revision,
                entity,
                gameplay_mechanics::StatsComponent::new(
                    gameplay_mechanics::CatalogVersion::parse("commands.v1").unwrap(),
                    vec![gameplay_mechanics::StatValue::new(
                        gameplay_mechanics::StatId::parse("vitality").unwrap(),
                        scalar(10),
                    )],
                )
                .unwrap(),
            )
            .unwrap();
        let revision = state
            .component_revision::<gameplay_mechanics::TracksComponent>(entity)
            .unwrap();
        entity_state::EntityAuthoringService
            .attach_component(
                &mut state,
                revision,
                entity,
                gameplay_mechanics::TracksComponent::new(
                    gameplay_mechanics::CatalogVersion::parse("commands.v1").unwrap(),
                    vec![gameplay_mechanics::TrackValue::new(
                        gameplay_mechanics::TrackId::parse("health").unwrap(),
                        scalar(10),
                    )],
                )
                .unwrap(),
            )
            .unwrap();
        state
    }

    fn admin_request(
        state: &entity_state::EntityState,
        operation: &str,
    ) -> gameplay_mechanics::StatBaseMutationRequest {
        let operation = gameplay_mechanics::OperationId::parse(operation).unwrap();
        gameplay_mechanics::StatBaseMutationRequest {
            source: gameplay_mechanics::SourceInstanceIdentity::Request {
                operation: operation.clone(),
                instance: gameplay_mechanics::SourceInstanceId::parse("admin").unwrap(),
            },
            operation,
            entity: core_ids::EntityId::new(70),
            stat: gameplay_mechanics::StatId::parse("vitality").unwrap(),
            base: scalar(20),
            expected_revision: Some(
                state
                    .component_revision::<gameplay_mechanics::StatsComponent>(
                        core_ids::EntityId::new(70),
                    )
                    .unwrap(),
            ),
        }
    }

    #[test]
    fn admin_command_mutates_through_named_service_and_preserves_stale_owner_error() {
        let state = std::sync::Arc::new(std::sync::Mutex::new(mechanics_state()));
        let catalog = mechanics_catalog();
        let success_request = admin_request(&state.lock().unwrap(), "admin-set-one");
        let stale_request = success_request.clone();
        let mut commands =
            CommandBindings::new(profile([CommandLane::Admin]), facts(), 16).unwrap();
        let authority = state.clone();
        commands
            .bind::<AdminSetStatBase, _>(move |_context, request| {
                crate::admin_set_stat_base(&mut authority.lock().unwrap(), &catalog, request)
            })
            .unwrap();
        let success = commands.dispatch::<AdminSetStatBase>(request::<AdminSetStatBase>(
            "admin-success",
            success_request,
        ));
        match success.result {
            HandlerResult::Success(receipt) => assert_eq!(receipt.after, scalar(20)),
            other => panic!("unexpected admin success response: {other:?}"),
        }
        let after_success_revision = state
            .lock()
            .unwrap()
            .component_revision::<gameplay_mechanics::StatsComponent>(core_ids::EntityId::new(70))
            .unwrap();
        let rejected = commands.dispatch::<AdminSetStatBase>(request::<AdminSetStatBase>(
            "admin-stale",
            stale_request,
        ));
        assert!(matches!(
            rejected.result,
            HandlerResult::Rejected(developer_command::DispatchError::Command(
                gameplay_mechanics::MechanicsError::StaleComponentRevision { .. }
            ))
        ));
        let state = state.lock().unwrap();
        assert_eq!(
            state
                .component::<gameplay_mechanics::StatsComponent>(core_ids::EntityId::new(70))
                .unwrap()
                .unwrap()
                .base(&gameplay_mechanics::StatId::parse("vitality").unwrap()),
            Some(scalar(20)),
        );
        assert_eq!(
            state
                .component_revision::<gameplay_mechanics::StatsComponent>(core_ids::EntityId::new(
                    70
                ))
                .unwrap(),
            after_success_revision,
        );
        assert_eq!(commands.history().len(), 2);
    }

    #[test]
    fn stale_standard_plan_is_rejected_by_the_existing_plan_validator() {
        let catalog = mechanics_catalog();
        let mut state = mechanics_state();
        let role = CapabilityRoleId::parse("actor").unwrap();
        let operation = StandardOperation::SpendTrack {
            role: role.clone(),
            track: gameplay_mechanics::TrackId::parse("health").unwrap(),
            amount: ExactExpr::Literal(scalar(1)).into(),
        };
        let bindings = CapabilityRoleBindings::admit(
            &operation.requirements(),
            vec![CapabilityRoleBinding::new(
                role,
                core_ids::EntityId::new(70),
                vec![CapabilityRequirementId::parse(STANDARD_TRACK_CAPABILITY).unwrap()],
            )
            .unwrap()],
        )
        .unwrap();
        let operation_id = gameplay_mechanics::OperationId::parse("planned-spend").unwrap();
        let context = StandardOperationContext::new(
            operation_id.clone(),
            gameplay_mechanics::SourceInstanceIdentity::Request {
                operation: operation_id,
                instance: gameplay_mechanics::SourceInstanceId::parse("plan").unwrap(),
            },
        )
        .unwrap();
        let plan = operation
            .plan(
                &bindings,
                &ExactInputBundle::new(vec![]),
                &state,
                &catalog,
                &context,
            )
            .unwrap();
        let revision = state
            .component_revision::<gameplay_mechanics::TracksComponent>(core_ids::EntityId::new(70))
            .unwrap();
        entity_state::EntityAuthoringService
            .replace_component(
                &mut state,
                revision,
                core_ids::EntityId::new(70),
                gameplay_mechanics::TracksComponent::new(
                    gameplay_mechanics::CatalogVersion::parse("commands.v1").unwrap(),
                    vec![gameplay_mechanics::TrackValue::new(
                        gameplay_mechanics::TrackId::parse("health").unwrap(),
                        scalar(9),
                    )],
                )
                .unwrap(),
            )
            .unwrap();
        assert!(matches!(
            crate::validate_standard_plan(&plan, &state, &catalog),
            Err(gameplay_standard::StandardPlanValidationError::StaleComponentRevision { .. })
        ));
    }

    type EmptyStandardEvidenceParts<'a> = engine_inspector::StandardBorrowedEvidenceParts<
        'a,
        (),
        (),
        (),
        (),
        (),
        (),
        (),
        (),
        (),
        (),
        (),
        &'a str,
    >;

    #[test]
    fn standard_inspection_composes_supplied_plan_receipt_and_product_explanation() {
        let catalog = mechanics_catalog();
        let state = mechanics_state();
        let role = CapabilityRoleId::parse("actor").unwrap();
        let operation = StandardOperation::SpendTrack {
            role: role.clone(),
            track: gameplay_mechanics::TrackId::parse("health").unwrap(),
            amount: ExactExpr::Literal(scalar(1)).into(),
        };
        let bindings = CapabilityRoleBindings::admit(
            &operation.requirements(),
            vec![CapabilityRoleBinding::new(
                role,
                core_ids::EntityId::new(70),
                vec![CapabilityRequirementId::parse(STANDARD_TRACK_CAPABILITY).unwrap()],
            )
            .unwrap()],
        )
        .unwrap();
        let operation_id = gameplay_mechanics::OperationId::parse("inspect-plan").unwrap();
        let context = StandardOperationContext::new(
            operation_id.clone(),
            gameplay_mechanics::SourceInstanceIdentity::Request {
                operation: operation_id,
                instance: gameplay_mechanics::SourceInstanceId::parse("inspect").unwrap(),
            },
        )
        .unwrap();
        let plan = operation
            .plan(
                &bindings,
                &ExactInputBundle::new(vec![]),
                &state,
                &catalog,
                &context,
            )
            .unwrap();
        let mut candidate = state.clone();
        let receipt = plan
            .effect()
            .apply_to_candidate(&mut candidate, &catalog)
            .unwrap();
        let mechanics =
            crate::inspect_mechanics(&state, &catalog, core_ids::EntityId::new(70)).unwrap();
        let explanation = "product trace stays typed";
        let parts: EmptyStandardEvidenceParts<'_> =
            engine_inspector::StandardBorrowedEvidenceParts {
                plan: Some(gameplay_standard::StandardOperationPlanProjection(&plan)),
                operation: Some(gameplay_standard::StandardOperationProjection(&operation)),
                definition: None,
                mechanics_receipt: Some(gameplay_standard::StandardMechanicsReceiptProjection(
                    &receipt,
                )),
                resolution: None,
                explanation: &explanation,
            };
        let evidence = crate::inspect_standard_evidence(&mechanics, None, parts);
        assert_eq!(
            evidence.plan().unwrap().plan().catalog().fingerprint(),
            catalog.fingerprint()
        );
        assert_eq!(
            evidence.operation().unwrap().requirements(),
            operation.requirements()
        );
        assert!(evidence.mechanics_receipt().unwrap().track().is_some());
        assert_eq!(evidence.explanation(), &explanation);
    }

    #[derive(Default)]
    struct Policy;

    impl ResolutionPolicy for Policy {
        type RawIntent = u8;
        type Intent = u8;
        type Facts = ();
        type Predicate = ();
        type Operation = ();
        type Effect = u8;
        type Event = ();
        type Evidence = u16;
        type Interceptor = ();
        type TraceDetail = ();
        type Rejection = ();
        type Fault = ();
        type Suspension = ();

        fn admit(
            &mut self,
            intent: &u8,
            _: &[u16],
            _: &mut dyn ResolutionTraceSink<()>,
        ) -> PolicyResult<u8, (), (), ()> {
            Ok(*intent)
        }
        fn gather(
            &mut self,
            _: &u8,
            _: &[u16],
            _: &mut dyn ResolutionTraceSink<()>,
        ) -> PolicyResult<(), (), (), ()> {
            Ok(())
        }
        fn check(
            &mut self,
            _: &u8,
            _: &(),
            _: &[u16],
            _: &mut dyn ResolutionTraceSink<()>,
        ) -> PolicyResult<(), (), (), ()> {
            Ok(())
        }
        fn plan(
            &mut self,
            _: &u8,
            _: &(),
            _: &[u16],
            _: &mut dyn ResolutionTraceSink<()>,
        ) -> PolicyResult<Program<(), ()>, (), (), ()> {
            Ok(Program::Operation(()))
        }
        fn evaluate_predicate(
            &mut self,
            _: &(),
            _: &u8,
            _: &(),
            _: &[u16],
            _: &mut dyn ResolutionTraceSink<()>,
        ) -> PolicyResult<bool, (), (), ()> {
            Ok(true)
        }
        fn plan_operation(
            &mut self,
            _: &(),
            _: &u8,
            _: &(),
            _: &[u16],
            _: &mut dyn ResolutionTraceSink<()>,
        ) -> PolicyResult<ResolutionPlan<u8, (), u8, u16>, (), (), ()> {
            let mut plan = ResolutionPlan::new();
            plan.push_effect(7);
            Ok(plan)
        }
    }

    #[derive(Default)]
    struct Transaction {
        authority: Vec<u8>,
        staged: Vec<u8>,
        commits: usize,
        aborts: usize,
    }

    impl ResolutionTransaction for Transaction {
        type Effect = u8;
        type Error = Infallible;
        fn stage(&mut self, effect: &u8) -> Result<(), Self::Error> {
            self.staged.push(*effect);
            Ok(())
        }
        fn commit(&mut self) -> Result<(), Self::Error> {
            self.authority.append(&mut self.staged);
            self.commits += 1;
            Ok(())
        }
        fn abort(&mut self) {
            self.staged.clear();
            self.aborts += 1;
        }
    }

    fn attempt() -> StandardAttempt<u8, u16> {
        StandardAttempt::new(
            ResolutionIdentity::root(
                ResolutionId::new(3).unwrap(),
                gameplay_resolution::CorrelationId::new(4).unwrap(),
            ),
            1,
            vec![5],
        )
    }

    #[test]
    fn preview_aborts_and_apply_publishes_once_through_the_same_standard_resolver() {
        let resolver = StandardResolver::default();
        let mut preview_transaction = Transaction::default();
        let preview =
            preview_standard_attempt(&resolver, &mut Policy, &mut preview_transaction, attempt());
        assert!(preview.succeeded());
        assert!(preview_transaction.authority.is_empty());
        assert_eq!(preview_transaction.commits, 0);
        assert_eq!(preview_transaction.aborts, 1);

        let mut apply_transaction = Transaction::default();
        let applied =
            execute_standard_attempt(&resolver, &mut Policy, &mut apply_transaction, attempt());
        assert!(applied.succeeded());
        assert_eq!(apply_transaction.authority, vec![7]);
        assert_eq!(apply_transaction.commits, 1);
        assert_eq!(apply_transaction.aborts, 0);
    }
}
