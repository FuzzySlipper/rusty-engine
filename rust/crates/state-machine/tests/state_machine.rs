use core_ids::{EntityId, ModeId, ProcessId};
use entity_state::{EntityAuthoringService, EntityDefinition, EntityState};
use state_machine::{
    apply_detached_transition, apply_transition_to_instance, DetachedMachineInstance,
    DetachedTransitionRequest, MachineInstance, StateMachineError, StateMachineFact,
    StateMachineSpec, StateMachineStore, TransitionRequest,
};

#[test]
fn entity_owned_transition_is_explicit_revisioned_and_typed() {
    let entities = entities();
    let mut machines = StateMachineStore::new();
    machines.define_machine(spec()).unwrap();
    let attached = machines
        .attach(&entities, entity(), machine(), idle())
        .unwrap();
    assert_eq!(
        attached,
        StateMachineFact::Attached {
            entity: entity(),
            machine: machine(),
            state: idle(),
            revision: 0,
        }
    );

    let applied = machines
        .apply_transition(
            &entities,
            TransitionRequest::new(entity(), machine(), idle(), moving()).expecting_revision(0),
        )
        .unwrap();
    assert_eq!(applied.previous, idle());
    assert_eq!(applied.instance.current, moving());
    assert_eq!(applied.instance.revision, 1);
    assert_eq!(
        applied.fact,
        StateMachineFact::Transitioned {
            entity: entity(),
            machine: machine(),
            from: idle(),
            to: moving(),
            revision: 1,
        }
    );
}

#[test]
fn invalid_and_stale_requests_do_not_mutate_the_instance() {
    let entities = entities();
    let mut machines = StateMachineStore::new();
    machines.define_machine(spec()).unwrap();
    machines
        .attach(&entities, entity(), machine(), idle())
        .unwrap();
    let original = machines.clone();

    let invalid = machines
        .apply_transition(
            &entities,
            TransitionRequest::new(entity(), machine(), idle(), stopped()),
        )
        .unwrap_err();
    assert_eq!(invalid.code(), "invalid-transition");
    assert_eq!(machines, original);

    let stale = machines
        .apply_transition(
            &entities,
            TransitionRequest::new(entity(), machine(), idle(), moving()).expecting_revision(9),
        )
        .unwrap_err();
    assert!(matches!(stale, StateMachineError::StaleRevision { .. }));
    assert_eq!(machines, original);
}

#[test]
fn entity_state_remains_the_lifecycle_authority() {
    let mut entities = entities();
    let mut machines = StateMachineStore::new();
    machines.define_machine(spec()).unwrap();
    machines
        .attach(&entities, entity(), machine(), idle())
        .unwrap();
    EntityAuthoringService
        .disable(&mut entities, 0, entity())
        .unwrap();

    let before = machines.clone();
    let error = machines
        .apply_transition(
            &entities,
            TransitionRequest::new(entity(), machine(), idle(), moving()),
        )
        .unwrap_err();
    assert!(matches!(error, StateMachineError::EntityInactive { .. }));
    assert_eq!(machines, before);

    let missing = machines
        .attach(&entities, EntityId::new(99), machine(), idle())
        .unwrap_err();
    assert_eq!(missing.code(), "entity-missing");
}

#[test]
fn detached_transition_uses_the_same_checks_without_a_second_store() {
    let instance = MachineInstance {
        entity: entity(),
        machine: machine(),
        current: idle(),
        revision: 4,
    };
    let applied = apply_transition_to_instance(
        &spec(),
        instance,
        TransitionRequest::new(entity(), machine(), idle(), moving()).expecting_revision(4),
    )
    .unwrap();

    assert_eq!(applied.instance.current, moving());
    assert_eq!(applied.instance.revision, 5);

    let exhausted = MachineInstance {
        revision: u64::MAX,
        ..instance
    };
    let error = apply_transition_to_instance(
        &spec(),
        exhausted,
        TransitionRequest::new(entity(), machine(), idle(), moving()).expecting_revision(u64::MAX),
    )
    .unwrap_err();
    assert!(matches!(error, StateMachineError::RevisionOverflow { .. }));
    assert_eq!(exhausted.current, idle());
}

#[test]
fn detached_transition_rejects_an_invalid_machine_spec() {
    let invalid = StateMachineSpec::new(machine(), [moving()]).allow(idle(), moving());
    let instance = MachineInstance {
        entity: entity(),
        machine: machine(),
        current: idle(),
        revision: 4,
    };

    let error = apply_transition_to_instance(
        &invalid,
        instance,
        TransitionRequest::new(entity(), machine(), idle(), moving()),
    )
    .unwrap_err();

    assert!(matches!(
        error,
        StateMachineError::InvalidState { state, .. } if state == idle()
    ));
    assert_eq!(instance.current, idle());
    assert_eq!(instance.revision, 4);
}

#[test]
fn detached_transition_is_a_purpose_neutral_value_operation() {
    let instance = DetachedMachineInstance::new(machine(), idle(), 4);
    let applied = apply_detached_transition(
        &spec(),
        instance,
        DetachedTransitionRequest::new(idle(), moving()).expecting_revision(4),
    )
    .unwrap();

    assert_eq!(applied.previous, idle());
    assert_eq!(applied.revision, 5);
    assert_eq!(
        applied.instance,
        DetachedMachineInstance::new(machine(), moving(), 5)
    );
    assert_eq!(instance, DetachedMachineInstance::new(machine(), idle(), 4));

    let stale_state = apply_detached_transition(
        &spec(),
        applied.instance,
        DetachedTransitionRequest::new(idle(), moving()).expecting_revision(5),
    )
    .unwrap_err();
    assert!(matches!(
        stale_state,
        StateMachineError::DetachedStaleCurrentState { .. }
    ));
    assert_eq!(applied.instance.current, moving());
    assert_eq!(applied.instance.revision, 5);

    let stale_revision = apply_detached_transition(
        &spec(),
        applied.instance,
        DetachedTransitionRequest::new(moving(), stopped()).expecting_revision(4),
    )
    .unwrap_err();
    assert!(matches!(
        stale_revision,
        StateMachineError::DetachedStaleRevision { .. }
    ));
}

#[test]
fn detached_transition_checks_bounded_definition_shape() {
    let states = (0..=state_machine::MAX_DETACHED_DEFINITION_STATES)
        .map(|state| ModeId::new(state as u64))
        .collect::<Vec<_>>();
    let spec = StateMachineSpec::new(machine(), states);
    let instance = DetachedMachineInstance::new(machine(), ModeId::new(0), 0);
    let error = apply_detached_transition(
        &spec,
        instance,
        DetachedTransitionRequest::new(ModeId::new(0), ModeId::new(0)),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        StateMachineError::DefinitionStateLimitExceeded { .. }
    ));
    assert_eq!(instance.current, ModeId::new(0));
    assert_eq!(instance.revision, 0);
}

#[test]
fn definitions_are_validated_and_iterate_deterministically() {
    let ordered = StateMachineSpec::new(machine(), [stopped(), idle(), moving()])
        .allow(moving(), stopped())
        .allow(idle(), moving());
    assert_eq!(
        ordered.states().collect::<Vec<_>>(),
        vec![idle(), moving(), stopped()]
    );
    assert_eq!(
        ordered.transitions().collect::<Vec<_>>(),
        vec![(idle(), moving()), (moving(), stopped())]
    );

    let mut store = StateMachineStore::new();
    let invalid = StateMachineSpec::new(machine(), [idle()]).allow(idle(), moving());
    assert!(matches!(
        store.define_machine(invalid),
        Err(StateMachineError::InvalidState { state, .. }) if state == moving()
    ));
    assert!(matches!(
        store.define_machine(StateMachineSpec::new(ProcessId::new(11), [])),
        Err(StateMachineError::EmptyMachine { .. })
    ));
}

fn entities() -> EntityState {
    EntityState::from_definitions([EntityDefinition::new(entity(), "moving platform")]).unwrap()
}

const fn entity() -> EntityId {
    EntityId::new(7)
}

const fn machine() -> ProcessId {
    ProcessId::new(10)
}

const fn idle() -> ModeId {
    ModeId::new(1)
}

const fn moving() -> ModeId {
    ModeId::new(2)
}

const fn stopped() -> ModeId {
    ModeId::new(3)
}

fn spec() -> StateMachineSpec {
    StateMachineSpec::new(machine(), [idle(), moving(), stopped()])
        .allow(idle(), moving())
        .allow(moving(), stopped())
}
