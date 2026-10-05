use super::*;
use std::time::Instant;

/// Drains `session` into `seen` until `done` holds for the latest drain.
fn until(
    session: &Session,
    seen: &mut Vec<Event>,
    mut done: impl FnMut(&Drained, &[Event]) -> bool,
) -> Drained {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let drained = session.drain();
        seen.extend(drained.events.iter().cloned());
        if done(&drained, seen) {
            return drained;
        }
        assert!(
            Instant::now() < deadline,
            "timed out; events so far: {seen:?}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn host(runtime: &SessionRuntime) -> (Session, Vec<Event>, Invitation) {
    let session = Session::host(
        runtime,
        HostConfig {
            identity: Identity::ephemeral(),
            application: "fixture/1".into(),
            relay: Relay::None,
            relay_token: String::new(),
            relay_only: false,
        },
    );
    let mut events = Vec::new();
    let drained = until(&session, &mut events, |drained, _| {
        drained.state != State::Starting
    });
    assert_eq!(drained.state, State::Open, "{}", drained.diagnostic);
    let invitation = Invitation::decode(&drained.invitation).unwrap();
    (session, events, invitation)
}

fn join(
    runtime: &SessionRuntime,
    invitation: &Invitation,
    identity: &Identity,
) -> (Session, Vec<Event>) {
    let session = Session::join(
        runtime,
        JoinConfig {
            identity: identity.clone(),
            application: "fixture/1".into(),
            invitation: invitation.clone(),
            relay_only: false,
        },
    );
    let mut events = Vec::new();
    let drained = until(&session, &mut events, |drained, _| {
        drained.state != State::Starting
    });
    assert_eq!(drained.state, State::Open, "{}", drained.diagnostic);
    (session, events)
}

fn messages(events: &[Event]) -> Vec<(u32, u64, Vec<u8>)> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::Message {
                member,
                seen,
                payload,
                ..
            } => Some((*member, *seen, payload.clone())),
            _ => None,
        })
        .collect()
}

#[test]
fn a_host_and_two_guests_exchange_views_messages_and_chat() {
    let runtime = SessionRuntime::new().unwrap();
    let (host, mut host_events, invitation) = host(&runtime);
    let (alice, mut alice_events) = join(&runtime, &invitation, &Identity::ephemeral());
    let (bob, mut bob_events) = join(&runtime, &invitation, &Identity::ephemeral());
    let roster = until(&host, &mut host_events, |_, seen| {
        seen.iter()
            .filter(|event| matches!(event, Event::Joined { .. }))
            .count()
            == 2
    });
    let alice_member = alice.drain().local_member;
    let bob_member = bob.drain().local_member;
    assert_eq!((alice_member, bob_member), (2, 3));
    assert!(roster.members.iter().all(|member| member.connected));
    assert!(roster
        .members
        .iter()
        .filter(|member| !member.is_host)
        .all(|member| member.awaiting_view));

    // A broadcast skips members still awaiting their view.
    assert_eq!(host.broadcast(b"too early").unwrap().recipients, 0);
    let alice_view = host.send_view(alice_member, b"view for alice").unwrap();
    let after = host.broadcast(b"turn 1").unwrap();
    assert_eq!(after.recipients, 1, "only alice has her view");
    assert!(after.sequence > alice_view.sequence);
    let alice_seen = until(&alice, &mut alice_events, |_, seen| {
        messages(seen)
            .iter()
            .any(|(_, _, payload)| payload == b"turn 1")
    });
    assert_eq!(alice_seen.host_sequence, after.sequence);
    let order: Vec<_> = alice_events
        .iter()
        .filter_map(|event| match event {
            Event::View { payload, .. } | Event::Message { payload, .. } => Some(payload.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(order, [b"view for alice".to_vec(), b"turn 1".to_vec()]);

    // The host learns which host sequence an action answered.
    alice.send(HOST_MEMBER, b"move north").unwrap();
    until(&host, &mut host_events, |_, seen| {
        !messages(seen).is_empty()
    });
    assert_eq!(
        messages(&host_events),
        [(alice_member, after.sequence, b"move north".to_vec())]
    );
    assert!(
        alice.send(bob_member, b"x").is_err(),
        "guests talk only to the host"
    );
    assert!(alice.broadcast(b"x").is_err());

    // Chat reaches everyone in transcript order, attributed by connection.
    alice.send_chat("hello").unwrap();
    until(&host, &mut host_events, |drained, _| {
        drained.chat.len() == 1
    });
    host.send_chat("welcome").unwrap();
    let bob_chat = until(&bob, &mut bob_events, |drained, _| drained.chat.len() == 2);
    let lines: Vec<_> = bob_chat
        .chat
        .iter()
        .map(|line| (line.member, line.text.as_str(), line.state))
        .collect();
    assert_eq!(
        lines,
        [
            (alice_member, "hello", ChatState::Delivered),
            (HOST_MEMBER, "welcome", ChatState::Delivered)
        ]
    );
    let alice_chat = until(&alice, &mut alice_events, |drained, _| {
        drained.chat.len() == 2
            && drained
                .chat
                .iter()
                .all(|line| line.state == ChatState::Delivered)
    });
    assert_eq!(
        alice_chat.chat[0].sequence, 1,
        "the pending line became the host's line"
    );

    // A graceful leave is reported as such, and the roster follows.
    drop(bob);
    until(&host, &mut host_events, |_, seen| {
        seen.contains(&Event::Left {
            member: bob_member,
            reason: LeaveReason::Left,
        })
    });
    until(&alice, &mut alice_events, |drained, _| {
        drained
            .members
            .iter()
            .any(|member| member.member == bob_member && !member.connected)
    });

    // The host leaving ends the session for its guests.
    drop(host);
    let ended = until(&alice, &mut alice_events, |drained, _| {
        drained.state == State::Ended
    });
    assert_eq!(
        ended.end_reason,
        Some(EndReason::HostClosed),
        "{}",
        ended.diagnostic
    );
    assert_eq!(alice.send_chat("anyone?").unwrap(), 2);
    assert_eq!(alice.drain().chat.last().unwrap().state, ChatState::Failed);
}

#[test]
fn joins_are_refused_for_another_session_or_application() {
    let runtime = SessionRuntime::new().unwrap();
    let (_host, _, invitation) = host(&runtime);
    let mut forged = invitation.clone();
    forged.secret = "0".repeat(32);
    let wrong_app = Session::join(
        &runtime,
        JoinConfig {
            identity: Identity::ephemeral(),
            application: "another-game/1".into(),
            invitation: invitation.clone(),
            relay_only: false,
        },
    );
    let wrong_secret = Session::join(
        &runtime,
        JoinConfig {
            identity: Identity::ephemeral(),
            application: "fixture/1".into(),
            invitation: forged,
            relay_only: false,
        },
    );
    for (session, expected) in [
        (wrong_app, "another-game/1"),
        (wrong_secret, "not for this session"),
    ] {
        let ended = until(&session, &mut Vec::new(), |drained, _| {
            drained.state == State::Ended
        });
        assert_eq!(ended.end_reason, Some(EndReason::Refused));
        assert!(ended.diagnostic.contains(expected), "{}", ended.diagnostic);
    }
    assert!(Invitation::decode("hello").is_err());
}

#[test]
fn a_rejoining_identity_keeps_its_member_and_awaits_a_fresh_view() {
    let runtime = SessionRuntime::new().unwrap();
    let (host, mut host_events, invitation) = host(&runtime);
    let identity = Identity::ephemeral();
    let (first, _) = join(&runtime, &invitation, &identity);
    let member = first.drain().local_member;
    host.send_view(member, b"v1").unwrap();
    drop(first);
    until(&host, &mut host_events, |_, seen| {
        seen.iter().any(|event| matches!(event, Event::Left { .. }))
    });
    let (second, mut events) = join(&runtime, &invitation, &identity);
    assert_eq!(second.drain().local_member, member);
    let drained = until(&host, &mut host_events, |_, seen| {
        seen.contains(&Event::Rejoined { member })
    });
    let entry = drained
        .members
        .iter()
        .find(|entry| entry.member == member)
        .unwrap();
    assert!(entry.connected && entry.awaiting_view);
    assert_eq!(host.broadcast(b"stale").unwrap().recipients, 0);
    host.send_view(member, b"v2").unwrap();
    until(&second, &mut events, |_, seen| {
        seen.iter()
            .any(|event| matches!(event, Event::View { payload, .. } if payload == b"v2"))
    });
}

#[test]
fn a_vanished_guest_is_reported_lost_after_the_idle_timeout() {
    let runtime = SessionRuntime::new().unwrap();
    let (host, mut host_events, invitation) = host(&runtime);
    let guest_runtime = SessionRuntime::new().unwrap();
    let (guest, _) = join(&guest_runtime, &invitation, &Identity::ephemeral());
    let member = guest.drain().local_member;
    // Stopping the guest's runtime silences it without a goodbye.
    std::mem::forget(guest);
    drop(guest_runtime);
    let started = Instant::now();
    until(&host, &mut host_events, |_, seen| {
        seen.contains(&Event::Left {
            member,
            reason: LeaveReason::Lost,
        })
    });
    assert!(started.elapsed() >= IDLE_TIMEOUT - Duration::from_secs(2));
}

#[test]
fn identities_are_stored_once_and_reused() {
    let directory =
        std::env::temp_dir().join(format!("svc-session-identity-{}", std::process::id()));
    let path = directory.join("player.key");
    let first = Identity::stored(&path).unwrap();
    assert_eq!(Identity::stored(&path).unwrap().key(), first.key());
    assert_ne!(Identity::ephemeral().key(), first.key());
    let _ = std::fs::remove_dir_all(directory);
}

/// Sends `count` 64 KiB messages from a fresh guest while the host drains
/// after each, releasing what it drained only when `release` is set.
fn flood(release: bool, count: usize) -> (Vec<Event>, u32) {
    let runtime = SessionRuntime::new().unwrap();
    let (host, mut host_events, invitation) = host(&runtime);
    let (guest, _) = join(&runtime, &invitation, &Identity::ephemeral());
    let member = guest.drain().local_member;
    let chunk = vec![7u8; 64 * 1024];
    for _ in 0..count {
        if host_events
            .iter()
            .any(|event| matches!(event, Event::Left { .. }))
        {
            break;
        }
        guest.send(HOST_MEMBER, &chunk).unwrap();
        let before = host_events.len();
        let deadline = Instant::now() + Duration::from_secs(10);
        while host_events.len() == before && Instant::now() < deadline {
            let drained = host.drain();
            if release {
                let bytes = drained
                    .events
                    .iter()
                    .map(|event| match event {
                        Event::Message { payload, .. } => payload.len(),
                        _ => 0,
                    })
                    .sum();
                host.release(bytes);
            }
            host_events.extend(drained.events);
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    std::thread::sleep(Duration::from_millis(300));
    host_events.extend(host.drain().events);
    (host_events, member)
}

#[test]
fn drained_but_untaken_payload_still_counts_against_the_flood_limit() {
    // Eight drains of 64 KiB each, never released: past the 256 KiB test limit.
    let (events, member) = flood(false, 8);
    assert!(
        events.contains(&Event::Left {
            member,
            reason: LeaveReason::Flooded
        }),
        "{events:?}"
    );
}

#[test]
fn released_payload_never_floods() {
    let (events, member) = flood(true, 8);
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, Event::Left { member: m, .. } if *m == member)),
        "{events:?}"
    );
}
