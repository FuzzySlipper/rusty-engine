use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use developer_command::{
    CommandAlias, CommandBindings, CommandDescriptor, CommandId, CommandLane, CommandProfile,
    CommandRequest, CorrelationId, DeveloperCommand, DispatchError, DispatchFacts, EnvelopeError,
    ExpectedFacts, HandlerResult, ParameterDescriptor, ProfileId, RuntimeInstanceId,
    TypeDescriptor, CURRENT_PROTOCOL_VERSION, MAX_COMMAND_ALIASES, MAX_DESCRIPTOR_COLLECTION_ITEMS,
    MAX_DESCRIPTOR_STRING_BYTES,
};

struct Inspect;
struct Play;
struct Admin;
struct Unbound;
struct ConflictingInspect;

#[derive(Debug, PartialEq, Eq)]
enum OwnerError {
    Rejected,
}

impl DeveloperCommand for Inspect {
    type Request = u32;
    type Reply = u32;
    type Error = OwnerError;

    fn descriptor() -> CommandDescriptor {
        descriptor("dev.inspect.entity", CommandLane::Inspect, &[])
    }
}
impl DeveloperCommand for Play {
    type Request = u32;
    type Reply = u32;
    type Error = OwnerError;

    fn descriptor() -> CommandDescriptor {
        descriptor("dev.play.action", CommandLane::Play, &["play"])
    }
}
impl DeveloperCommand for Admin {
    type Request = u32;
    type Reply = u32;
    type Error = OwnerError;

    fn descriptor() -> CommandDescriptor {
        descriptor("dev.admin.reset", CommandLane::Admin, &[])
    }
}
impl DeveloperCommand for Unbound {
    type Request = u32;
    type Reply = u32;
    type Error = OwnerError;

    fn descriptor() -> CommandDescriptor {
        descriptor("dev.session.unbound", CommandLane::Session, &[])
    }
}
impl DeveloperCommand for ConflictingInspect {
    type Request = u32;
    type Reply = u32;
    type Error = OwnerError;

    fn descriptor() -> CommandDescriptor {
        descriptor("dev.inspect.entity", CommandLane::Fault, &[])
    }
}

fn descriptor(id: &str, lane: CommandLane, aliases: &[&str]) -> CommandDescriptor {
    CommandDescriptor::new(
        CommandId::parse(id).unwrap(),
        aliases
            .iter()
            .map(|alias| CommandAlias::parse(*alias).unwrap())
            .collect(),
        lane,
        "A bounded product-owned command.",
        vec![ParameterDescriptor::new(
            "value",
            "A value.",
            true,
            TypeDescriptor::UnsignedInteger,
        )],
        TypeDescriptor::UnsignedInteger,
        TypeDescriptor::Identifier { maximum_bytes: 32 },
    )
    .unwrap()
}

fn facts() -> DispatchFacts {
    DispatchFacts {
        runtime: RuntimeInstanceId::parse("runtime.test").unwrap(),
        revision: 7,
        catalog_epoch: 3,
    }
}

fn bindings() -> CommandBindings {
    CommandBindings::new(
        CommandProfile::broad(ProfileId::parse("profile.test").unwrap()),
        facts(),
        16,
    )
    .unwrap()
}

fn request<C: DeveloperCommand<Request = u32>>(
    id: &str,
    correlation: &str,
    value: u32,
) -> CommandRequest<u32> {
    let _ = std::marker::PhantomData::<C>;
    CommandRequest::new(
        CommandId::parse(id).unwrap(),
        CorrelationId::parse(correlation).unwrap(),
        facts().runtime,
        value,
    )
}

#[test]
fn discovery_and_typed_sync_dispatch_keep_inspect_play_and_admin_distinct() {
    let mut port = bindings();
    port.bind::<Inspect, _>(|_, value| Ok(value + 1)).unwrap();
    port.bind::<Play, _>(|_, value| Ok(value + 2)).unwrap();
    port.bind::<Admin, _>(|_, _| Err(OwnerError::Rejected))
        .unwrap();
    port.declare::<Unbound>().unwrap();

    let discovery = port.discover();
    assert_eq!(discovery.commands.len(), 4);
    assert_eq!(
        discovery
            .commands
            .iter()
            .filter(|entry| entry.bound)
            .count(),
        3
    );
    assert!(
        !discovery
            .commands
            .iter()
            .find(|entry| entry.descriptor.id().as_str() == "dev.session.unbound")
            .unwrap()
            .bound
    );

    let inspect =
        port.dispatch::<Inspect>(request::<Inspect>("dev.inspect.entity", "inspect.1", 10));
    assert_eq!(inspect.result, HandlerResult::Success(11));
    let play = port.dispatch::<Play>(request::<Play>("dev.play.action", "play.1", 10));
    assert_eq!(play.result, HandlerResult::Success(12));
    assert_eq!(
        inspect.provenance.as_ref().unwrap().lane,
        CommandLane::Inspect
    );
    assert_eq!(play.provenance.as_ref().unwrap().lane, CommandLane::Play);
    assert_ne!(
        inspect.provenance.as_ref().unwrap().command,
        play.provenance.as_ref().unwrap().command
    );

    let admin = port.dispatch::<Admin>(request::<Admin>("dev.admin.reset", "admin.1", 10));
    assert_eq!(
        admin.result,
        HandlerResult::Rejected(DispatchError::Command(OwnerError::Rejected))
    );
    assert_eq!(admin.provenance.as_ref().unwrap().lane, CommandLane::Admin);
    let unavailable =
        port.dispatch::<Unbound>(request::<Unbound>("dev.session.unbound", "unbound.1", 10));
    assert!(matches!(
        unavailable.result,
        HandlerResult::Rejected(DispatchError::Envelope(
            EnvelopeError::CommandUnavailable { .. }
        ))
    ));
    assert_eq!(unavailable.provenance, None);
    assert_eq!(port.history().len(), 3);
}

#[test]
fn envelope_failures_do_not_call_handlers_or_change_dispatch_state() {
    let calls = Arc::new(AtomicUsize::new(0));
    let handler_calls = Arc::clone(&calls);
    let mut port = bindings();
    port.bind::<Inspect, _>(move |_, value| {
        handler_calls.fetch_add(1, Ordering::SeqCst);
        Ok(value)
    })
    .unwrap();

    let cases = [
        request::<Inspect>("dev.inspect.entity", "bad.protocol", 1),
        request::<Inspect>("dev.inspect.entity", "bad.cancelled", 1).cancelled(),
        request::<Inspect>("dev.inspect.entity", "bad.timeout", 1).timed_out(),
        CommandRequest::new(
            CommandId::parse("dev.inspect.entity").unwrap(),
            CorrelationId::parse("bad.runtime").unwrap(),
            RuntimeInstanceId::parse("runtime.other").unwrap(),
            1,
        ),
        request::<Inspect>("dev.inspect.entity", "bad.revision", 1).with_expected(ExpectedFacts {
            profile: None,
            revision: Some(8),
            catalog_epoch: None,
        }),
        request::<Inspect>("dev.inspect.entity", "bad.epoch", 1).with_expected(ExpectedFacts {
            profile: None,
            revision: None,
            catalog_epoch: Some(4),
        }),
        request::<Inspect>("dev.inspect.entity", "bad.profile", 1).with_expected(ExpectedFacts {
            profile: Some(ProfileId::parse("profile.other").unwrap()),
            revision: None,
            catalog_epoch: None,
        }),
    ];
    for (index, mut invalid) in cases.into_iter().enumerate() {
        if index == 0 {
            invalid.protocol_version = developer_command::ProtocolVersion::new(2).unwrap();
        }
        let response = port.dispatch::<Inspect>(invalid);
        assert!(matches!(
            response.result,
            HandlerResult::Rejected(DispatchError::Envelope(_))
        ));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert!(port.history().is_empty());
        assert_eq!(response.provenance, None);
    }
    let mismatch =
        port.dispatch::<Inspect>(request::<Inspect>("dev.play.action", "bad.command", 1));
    assert!(matches!(
        mismatch.result,
        HandlerResult::Rejected(DispatchError::Envelope(
            EnvelopeError::CommandMismatch { .. }
        ))
    ));
    let unknown =
        port.dispatch::<Unbound>(request::<Unbound>("dev.session.unbound", "bad.unknown", 1));
    assert!(matches!(
        unknown.result,
        HandlerResult::Rejected(DispatchError::Envelope(
            EnvelopeError::UnknownCommand { .. }
        ))
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(port.history().is_empty());
}

#[test]
fn duplicate_and_mismatched_correlations_are_rejected_before_handler_calls() {
    let calls = Arc::new(AtomicUsize::new(0));
    let inspect_calls = Arc::clone(&calls);
    let play_calls = Arc::clone(&calls);
    let mut port = bindings();
    port.bind::<Inspect, _>(move |_, value| {
        inspect_calls.fetch_add(1, Ordering::SeqCst);
        Ok(value)
    })
    .unwrap();
    port.bind::<Play, _>(move |_, value| {
        play_calls.fetch_add(1, Ordering::SeqCst);
        Ok(value)
    })
    .unwrap();

    assert!(matches!(
        port.dispatch::<Inspect>(request::<Inspect>("dev.inspect.entity", "shared", 1))
            .result,
        HandlerResult::Success(1)
    ));
    assert!(matches!(
        port.dispatch::<Inspect>(request::<Inspect>("dev.inspect.entity", "shared", 1))
            .result,
        HandlerResult::Rejected(DispatchError::Envelope(
            EnvelopeError::DuplicateCorrelation { .. }
        ))
    ));
    assert!(matches!(
        port.dispatch::<Play>(request::<Play>("dev.play.action", "shared", 1))
            .result,
        HandlerResult::Rejected(DispatchError::Envelope(
            EnvelopeError::CorrelationMismatch { .. }
        ))
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(port.history().len(), 1);
}

#[test]
fn typed_owner_error_survives_the_envelope_and_is_historied() {
    let mut port = bindings();
    port.bind::<Inspect, _>(|_, _| Err(OwnerError::Rejected))
        .unwrap();
    let response =
        port.dispatch::<Inspect>(request::<Inspect>("dev.inspect.entity", "owner.error", 1));
    assert_eq!(
        response.result,
        HandlerResult::Rejected(DispatchError::Command(OwnerError::Rejected))
    );
    assert_eq!(
        port.history().front().unwrap().outcome,
        developer_command::CommandHistoryOutcome::CommandRejected
    );
}

#[test]
fn identity_alias_and_descriptor_bounds_are_exact() {
    assert!(CommandId::parse("").is_err());
    assert!(CommandId::parse("UPPER").is_err());
    assert!(CommandId::parse("x".repeat(129)).is_err());

    let aliases = (0..=MAX_COMMAND_ALIASES)
        .map(|index| CommandAlias::parse(format!("alias.{index}")).unwrap())
        .collect();
    assert!(matches!(
        CommandDescriptor::new(
            CommandId::parse("dev.aliases").unwrap(),
            aliases,
            CommandLane::Inspect,
            "summary",
            vec![],
            TypeDescriptor::Unit,
            TypeDescriptor::Unit,
        ),
        Err(developer_command::CommandDescriptorError::TooManyAliases { .. })
    ));
    assert!(matches!(
        TypeDescriptor::String {
            maximum_bytes: MAX_DESCRIPTOR_STRING_BYTES + 1
        }
        .node_count(),
        Err(developer_command::TypeDescriptorError::InvalidStringLimit { .. })
    ));
    assert!(TypeDescriptor::String {
        maximum_bytes: MAX_DESCRIPTOR_STRING_BYTES
    }
    .node_count()
    .is_ok());
    assert!(TypeDescriptor::List {
        item: Box::new(TypeDescriptor::Unit),
        maximum_items: MAX_DESCRIPTOR_COLLECTION_ITEMS
    }
    .node_count()
    .is_ok());
    assert!(matches!(
        TypeDescriptor::Record {
            fields: (0..=MAX_DESCRIPTOR_COLLECTION_ITEMS)
                .map(|index| ParameterDescriptor::new(
                    format!("field.{index}"),
                    "field",
                    true,
                    TypeDescriptor::Unit
                ))
                .collect(),
        }
        .node_count(),
        Err(developer_command::TypeDescriptorError::InvalidCollectionLimit { .. })
    ));
}

#[test]
fn duplicate_commands_and_aliases_are_rejected() {
    let mut port = bindings();
    port.declare::<Play>().unwrap();
    port.declare::<Inspect>().unwrap();
    assert!(port.declare::<Play>().is_err());
    assert!(port
        .declare_descriptor(descriptor("dev.other", CommandLane::Preview, &["play"]))
        .is_err());
    assert!(matches!(
        port.bind::<ConflictingInspect, _>(|_, value| Ok(value)),
        Err(developer_command::CommandBindingsError::DescriptorMismatch { .. })
    ));
}

#[test]
fn aliases_resolve_explicitly_and_descriptor_node_limits_are_exact() {
    let mut port = bindings();
    port.declare::<Play>().unwrap();
    assert_eq!(
        port.resolve_alias(&CommandAlias::parse("play").unwrap())
            .unwrap()
            .as_str(),
        "dev.play.action"
    );

    let nested = |lists| {
        let mut value = TypeDescriptor::Unit;
        for _ in 0..lists {
            value = TypeDescriptor::List {
                item: Box::new(value),
                maximum_items: 1,
            };
        }
        value
    };
    let bounded_record = |last_lists| TypeDescriptor::Record {
        fields: (0..MAX_DESCRIPTOR_COLLECTION_ITEMS)
            .map(|index| {
                ParameterDescriptor::new(
                    format!("field.{index}"),
                    "field",
                    true,
                    nested(if index + 1 == MAX_DESCRIPTOR_COLLECTION_ITEMS {
                        last_lists
                    } else {
                        3
                    }),
                )
            })
            .collect(),
    };
    assert_eq!(
        bounded_record(2).node_count().unwrap(),
        developer_command::MAX_DESCRIPTOR_NODES
    );
    assert!(matches!(
        bounded_record(3).node_count(),
        Err(developer_command::TypeDescriptorError::TooManyNodes { .. })
    ));
    assert!(CommandDescriptor::new(
        CommandId::parse("dev.nodes.exact").unwrap(),
        vec![],
        CommandLane::Inspect,
        "summary",
        vec![],
        bounded_record(1),
        TypeDescriptor::Unit,
    )
    .is_ok());
    assert!(matches!(
        CommandDescriptor::new(
            CommandId::parse("dev.nodes.over").unwrap(),
            vec![],
            CommandLane::Inspect,
            "summary",
            vec![],
            bounded_record(2),
            TypeDescriptor::Unit,
        ),
        Err(developer_command::CommandDescriptorError::TooManyNodes { .. })
    ));
    let deeply_nested = nested(developer_command::MAX_DESCRIPTOR_DEPTH);
    assert!(matches!(
        deeply_nested.node_count(),
        Err(developer_command::TypeDescriptorError::TooDeep { .. })
    ));
    let deep_wire = serde_json::to_value(&deeply_nested).unwrap();
    let error = serde_json::from_value::<TypeDescriptor>(deep_wire).unwrap_err();
    assert!(error.to_string().contains("TooDeep"));
}

#[test]
fn current_protocol_is_version_one() {
    assert_eq!(CURRENT_PROTOCOL_VERSION.value(), 1);
}

#[test]
fn wire_decoding_revalidates_identities_and_public_contract_values() {
    assert!(serde_json::from_str::<developer_command::ProtocolVersion>("0").is_err());
    assert!(serde_json::from_str::<CommandId>("\"UPPER\"").is_err());
    assert!(serde_json::from_str::<CommandAlias>("\"\"").is_err());
    assert!(serde_json::from_str::<CorrelationId>("\"UPPER\"").is_err());
    assert!(serde_json::from_str::<RuntimeInstanceId>("\"runtime/invalid\"").is_err());
    assert!(serde_json::from_str::<ProfileId>("\"profile space\"").is_err());
    assert!(serde_json::from_str::<TypeDescriptor>(r#"{"String":{"maximum_bytes":0}}"#).is_err());
    assert!(serde_json::from_str::<developer_command::CommandProfile>(
        r#"{"id":"profile.empty","permitted_lanes":[]}"#
    )
    .is_err());
    assert!(serde_json::from_str::<CommandDescriptor>(
        r#"{
            "id":"dev.wire","aliases":[],"lane":"Inspect","summary":"summary",
            "parameters":[],"result":"Unit","error":"Unit","unexpected":true
        }"#
    )
    .is_err());
    assert!(serde_json::from_str::<CommandRequest<u32>>(
        r#"{
            "protocol_version":0,"command":"dev.wire","correlation":"wire.1",
            "runtime":"runtime.test","expected":{"profile":null,"revision":null,"catalog_epoch":null},
            "cancelled":false,"timed_out":false,"payload":1
        }"#
    )
    .is_err());
    assert!(serde_json::from_str::<CommandRequest<u32>>(
        r#"{
            "protocol_version":1,"command":"dev.wire","correlation":"wire.2",
            "runtime":"runtime.test","expected":{"profile":null,"revision":null,"catalog_epoch":null,"spoofed":true},
            "cancelled":false,"timed_out":false,"payload":1
        }"#
    )
    .is_err());
}
