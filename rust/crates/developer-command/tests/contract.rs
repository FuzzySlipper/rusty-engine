use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use developer_command::{
    map_command_response, CommandAlias, CommandBindings, CommandDescriptor, CommandId, CommandLane,
    CommandProfile, CommandRequest, CorrelationId, DeveloperCommand, DispatchError, DispatchFacts,
    EnvelopeError, ExpectedFacts, HandlerResult, HostCommandDiscovery, HostCommandRequest,
    HostDecimalU64, HostErrorBody, HostErrorCode, HostErrorMessage, HostErrorPhase, HostReceiptRef,
    HostReceiptRefs, HostResponseContext, ParameterDescriptor, ProfileId, RuntimeInstanceId,
    TypeDescriptor, CURRENT_PROTOCOL_VERSION, MAX_COMMAND_ALIASES, MAX_DESCRIPTOR_COLLECTION_ITEMS,
    MAX_DESCRIPTOR_STRING_BYTES, MAX_HOST_ERROR_MESSAGE_BYTES, MAX_HOST_RECEIPT_REFS,
};

struct Inspect;
struct Play;
struct Admin;
struct Unbound;
struct ConflictingInspect;

#[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

#[test]
fn borrowed_dispatch_requires_explicit_exposure_and_shares_preflight_state() {
    let mut port = bindings();
    port.expose_borrowed::<Inspect>().unwrap();
    port.declare::<Play>().unwrap();
    let discovery = port.discover();
    let inspect = discovery
        .commands
        .iter()
        .find(|entry| entry.descriptor.id().as_str() == "dev.inspect.entity")
        .unwrap();
    assert!(inspect.bound);
    assert!(!inspect.stored_bound);
    assert!(inspect.borrowed_bound);

    let calls = std::cell::Cell::new(0);
    let mut owner = |context: developer_command::CommandContext, value| {
        assert_eq!(context.provenance().sequence, 1);
        calls.set(calls.get() + 1);
        Ok::<_, OwnerError>(value + 10)
    };
    let response = port.dispatch_borrowed::<Inspect, _>(
        request::<Inspect>("dev.inspect.entity", "borrowed.1", 2),
        &mut owner,
    );
    assert_eq!(response.result, HandlerResult::Success(12));
    assert_eq!(calls.get(), 1);
    assert_eq!(port.history().len(), 1);

    // Stale and cancelled requests are rejected before the borrowed owner is
    // entered, and preserve the same no-correlation/no-history rule as stored
    // dispatch.
    let stale = request::<Inspect>("dev.inspect.entity", "borrowed.stale", 2).with_expected(
        ExpectedFacts {
            profile: None,
            revision: Some(8),
            catalog_epoch: None,
        },
    );
    let rejected = port.dispatch_borrowed::<Inspect, _>(stale, &mut owner);
    assert!(matches!(
        rejected.result,
        HandlerResult::Rejected(DispatchError::Envelope(EnvelopeError::StaleRevision { .. }))
    ));
    assert!(rejected.provenance.is_none());
    assert_eq!(calls.get(), 1);
    assert_eq!(port.history().len(), 1);

    let unavailable = port.dispatch_borrowed::<Play, _>(
        request::<Play>("dev.play.action", "borrowed.unexposed", 2),
        &mut owner,
    );
    assert!(matches!(
        unavailable.result,
        HandlerResult::Rejected(DispatchError::Envelope(
            EnvelopeError::CommandUnavailable { .. }
        ))
    ));
    assert_eq!(calls.get(), 1);
    assert_eq!(port.history().len(), 1);
}

#[test]
fn borrowed_dispatch_preserves_exact_owner_error_and_history_provenance() {
    let mut port = bindings();
    port.expose_borrowed::<Admin>().unwrap();
    let mut owner =
        |_context: developer_command::CommandContext, _value| Err::<u32, _>(OwnerError::Rejected);
    let response = port.dispatch_borrowed::<Admin, _>(
        request::<Admin>("dev.admin.reset", "borrowed.error", 4),
        &mut owner,
    );
    assert_eq!(
        response.result,
        HandlerResult::Rejected(DispatchError::Command(OwnerError::Rejected))
    );
    assert_eq!(response.provenance.as_ref().unwrap().sequence, 1);
    assert_eq!(port.history().len(), 1);
}

#[test]
fn stored_and_borrowed_exposures_coexist_without_crossing_type_or_preflight_state() {
    let mut port = bindings();
    port.bind::<Inspect, _>(|_, value| Ok(value + 1)).unwrap();
    port.expose_borrowed::<Admin>().unwrap();
    let discovery = port.discover();
    let stored = discovery
        .commands
        .iter()
        .find(|entry| entry.descriptor.id().as_str() == "dev.inspect.entity")
        .unwrap();
    let borrowed = discovery
        .commands
        .iter()
        .find(|entry| entry.descriptor.id().as_str() == "dev.admin.reset")
        .unwrap();
    assert_eq!(
        (stored.bound, stored.stored_bound, stored.borrowed_bound),
        (true, true, false)
    );
    assert_eq!(
        (
            borrowed.bound,
            borrowed.stored_bound,
            borrowed.borrowed_bound
        ),
        (true, false, true)
    );

    let stored_response = port.dispatch::<Inspect>(request::<Inspect>(
        "dev.inspect.entity",
        "coexist.stored",
        2,
    ));
    assert_eq!(stored_response.result, HandlerResult::Success(3));
    let mut owner = |_context: developer_command::CommandContext, _value| Ok::<_, OwnerError>(9);
    let borrowed_response = port.dispatch_borrowed::<Admin, _>(
        request::<Admin>("dev.admin.reset", "coexist.borrowed", 2),
        &mut owner,
    );
    assert_eq!(borrowed_response.result, HandlerResult::Success(9));
    assert_eq!(borrowed_response.provenance.as_ref().unwrap().sequence, 2);

    // A marker with a conflicting type for the same command identity cannot
    // enter the borrowed owner even though its request ID matches.
    let mut wrong_port = bindings();
    wrong_port.expose_borrowed::<Inspect>().unwrap();
    let mut calls = 0;
    let mut wrong_owner = |_context: developer_command::CommandContext, _value| {
        calls += 1;
        Ok::<_, OwnerError>(9)
    };
    let wrong = wrong_port.dispatch_borrowed::<ConflictingInspect, _>(
        request::<ConflictingInspect>("dev.inspect.entity", "coexist.wrong-type", 2),
        &mut wrong_owner,
    );
    assert!(matches!(
        wrong.result,
        HandlerResult::Rejected(DispatchError::Envelope(EnvelopeError::BindingInvariant))
    ));
    assert!(wrong.provenance.is_none());
    assert_eq!(calls, 0);
    assert_eq!(port.history().len(), 2);
    assert_eq!(wrong_port.history().len(), 0);
}

#[test]
fn host_wire_request_is_strict_camel_case_and_decimal_u64() {
    let request = serde_json::from_str::<HostCommandRequest<u32>>(
        r#"{
            "protocolVersion":1,
            "command":"dev.inspect.entity",
            "correlation":"wire.1",
            "runtime":"runtime.test",
            "expected":{"profile":"profile.test","revision":"18446744073709551615","catalogEpoch":"3"},
            "payload":7
        }"#,
    )
    .unwrap();
    assert_eq!(request.expected.revision, HostDecimalU64::new(u64::MAX));
    let (mapped, response_context) = request.into_command_parts().unwrap();
    assert_eq!(mapped.expected.revision, Some(u64::MAX));
    assert_eq!(mapped.expected.catalog_epoch, Some(3));
    assert_eq!(response_context.correlation().as_str(), "wire.1");
    assert_eq!(response_context.profile().as_str(), "profile.test");

    assert!(serde_json::from_str::<HostCommandRequest<u32>>(
        r#"{
            "protocolVersion":1,"command":"dev.inspect.entity","correlation":"wire.2",
            "runtime":"runtime.test","expected":{"profile":"profile.test","revision":1,"catalogEpoch":"3"},"payload":7
        }"#
    )
    .is_err());
    assert!(serde_json::from_str::<HostCommandRequest<u32>>(
        r#"{
            "protocolVersion":1,"command":"dev.inspect.entity","correlation":"wire.3",
            "runtime":"runtime.test","expected":{"profile":"profile.test","revision":"01","catalogEpoch":"3"},"payload":7
        }"#
    )
    .is_err());
    assert!(serde_json::from_str::<HostCommandRequest<u32>>(
        r#"{
            "protocolVersion":1,"command":"dev.inspect.entity","correlation":"wire.4",
            "runtime":"runtime.test","expected":{"profile":"profile.test","revision":"1","catalogEpoch":"3"},"payload":7,"extra":true
        }"#
    )
    .is_err());
}

#[test]
fn host_wire_response_preserves_facts_and_internal_provenance_sidecar() {
    let mut port = bindings();
    port.expose_borrowed::<Admin>().unwrap();
    let mut owner =
        |_context: developer_command::CommandContext, _value| Err::<u32, _>(OwnerError::Rejected);
    let response = port.dispatch_borrowed::<Admin, _>(
        request::<Admin>("dev.admin.reset", "wire.error", 4),
        &mut owner,
    );
    let mapped = map_command_response(
        response,
        HostResponseContext::new(
            CorrelationId::parse("wire.wrong-context").unwrap(),
            ProfileId::parse("profile.wrong-context").unwrap(),
        ),
        HostReceiptRefs::new(vec![HostReceiptRef::parse("receipt.1").unwrap()]).unwrap(),
        |error| HostErrorBody {
            code: HostErrorCode::parse("owner_rejected").unwrap(),
            message: HostErrorMessage::new(format!("owner rejected: {error:?}")).unwrap(),
            details: Some(error),
        },
    );
    assert_eq!(
        mapped.metadata.error_phase,
        Some(HostErrorPhase::EnteredOwner)
    );
    assert_eq!(mapped.metadata.provenance.unwrap().sequence, 1);
    let json = serde_json::to_value(mapped.wire).unwrap();
    assert_eq!(json["correlation"], "wire.error");
    assert_eq!(json["revision"], "7");
    assert_eq!(json["catalogEpoch"], "3");
    assert_eq!(json["outcome"]["kind"], "error");
    assert_eq!(json["outcome"]["code"], "owner_rejected");
    assert!(json["outcome"].get("phase").is_none());
    assert!(json["outcome"].get("provenance").is_none());
    let keys = json
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(
        keys,
        vec![
            "catalogEpoch".to_owned(),
            "correlation".to_owned(),
            "outcome".to_owned(),
            "profile".to_owned(),
            "revision".to_owned(),
            "runtime".to_owned(),
        ]
    );
    let receipts = (0..MAX_HOST_RECEIPT_REFS)
        .map(|index| HostReceiptRef::parse(format!("receipt.{index}")))
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(
        HostReceiptRefs::new(receipts.clone()).unwrap().as_slice(),
        receipts
    );
    assert!(HostReceiptRefs::new(
        (0..=MAX_HOST_RECEIPT_REFS)
            .map(|index| HostReceiptRef::parse(format!("receipt.over.{index}")))
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    )
    .is_err());
    assert!(HostErrorCode::parse("not a code").is_err());
    assert!(HostErrorMessage::new("x".repeat(MAX_HOST_ERROR_MESSAGE_BYTES)).is_ok());
    assert!(HostErrorMessage::new("x".repeat(MAX_HOST_ERROR_MESSAGE_BYTES + 1)).is_err());

    let mut port = bindings();
    port.expose_borrowed::<Admin>().unwrap();
    let mut owner =
        |_context: developer_command::CommandContext, _value| Err::<u32, _>(OwnerError::Rejected);
    let response = port.dispatch_borrowed::<Admin, _>(
        request::<Admin>("dev.admin.reset", "wire.mismatch", 4),
        &mut owner,
    );
    let mapped = map_command_response(
        response,
        HostResponseContext::new(
            CorrelationId::parse("wire.other").unwrap(),
            ProfileId::parse("profile.other").unwrap(),
        ),
        HostReceiptRefs::empty(),
        |_error| HostErrorBody::<serde_json::Value> {
            code: HostErrorCode::parse("owner_rejected").unwrap(),
            message: HostErrorMessage::new("owner rejected").unwrap(),
            details: None,
        },
    );
    assert_eq!(mapped.wire.correlation.as_str(), "wire.mismatch");
    assert_eq!(mapped.wire.profile.as_str(), "profile.test");

    let mut owner_calls = 0;
    let response = port.dispatch_borrowed::<Admin, _>(
        request::<Admin>("dev.inspect.entity", "wire.pre-dispatch", 4),
        &mut |_context, _value| {
            owner_calls += 1;
            Err::<u32, _>(OwnerError::Rejected)
        },
    );
    let mapped = map_command_response(
        response,
        HostResponseContext::new(
            CorrelationId::parse("wire.pre-dispatch").unwrap(),
            ProfileId::parse("profile.requested").unwrap(),
        ),
        HostReceiptRefs::empty(),
        |_error| HostErrorBody::<serde_json::Value> {
            code: HostErrorCode::parse("owner_rejected").unwrap(),
            message: HostErrorMessage::new("owner rejected").unwrap(),
            details: None,
        },
    );
    assert_eq!(owner_calls, 0);
    assert_eq!(mapped.wire.correlation.as_str(), "wire.pre-dispatch");
    assert_eq!(mapped.wire.profile.as_str(), "profile.requested");
    assert_eq!(
        mapped.metadata.error_phase,
        Some(HostErrorPhase::PreDispatch)
    );
}

#[test]
fn host_wire_discovery_exposes_only_executable_commands_with_lowercase_lanes() {
    let mut port = bindings();
    port.expose_borrowed::<Inspect>().unwrap();
    port.declare::<Play>().unwrap();
    let discovery =
        HostCommandDiscovery::from_bindings(&port, CommandId::parse("contract.v1").unwrap());
    assert_eq!(discovery.protocol_version, CURRENT_PROTOCOL_VERSION);
    assert_eq!(
        discovery.permitted_lanes,
        vec![
            "inspect".to_owned(),
            "preview".to_owned(),
            "play".to_owned(),
            "admin".to_owned(),
            "session".to_owned(),
            "author".to_owned(),
            "fault".to_owned(),
        ]
    );
    assert_eq!(discovery.commands.len(), 1);
    assert_eq!(discovery.commands[0].id.as_str(), "dev.inspect.entity");
    assert_eq!(discovery.commands[0].lane, "inspect");
    let json = serde_json::to_value(discovery).unwrap();
    assert!(json["commands"][0].get("helpOnly").is_none());
    assert_eq!(json["revision"], "7");
}
