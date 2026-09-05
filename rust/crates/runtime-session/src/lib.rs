//! Host-neutral runtime-session ownership and recovery facts.
//!
//! A runtime session serializes access to one concrete runtime instance. It
//! owns logical receipts and prepared replacement boundaries, but does not
//! define operation dispatch, callbacks, output subscriptions, registries,
//! scheduling, or transport policy. Hosts retain those responsibilities and
//! can keep a session lock across an atomic snapshot/cursor handover when
//! their own publication contract requires it.

#![forbid(unsafe_code)]

use std::sync::{LockResult, Mutex, MutexGuard};

use serde::{Deserialize, Serialize};

/// Whether an operation changed the authoritative state it owns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeMutationCertainty {
    /// The condition was discovered before the operation could mutate state.
    NotApplied,
    /// The operation and its owned effects were committed.
    Committed,
    /// The operation crossed an ownership boundary and its effect is unknown.
    Unknown,
}

/// A host-owned projection scope that must be re-established before dependent
/// work continues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeInvalidatedScope {
    /// No host-visible state needs to be re-established.
    None,
    /// The input cursor or held-input projection is no longer authoritative.
    Input,
    /// Retained output/projection state needs a fresh baseline.
    Outputs,
    /// The loaded runtime incarnation cannot be trusted to continue.
    Incarnation,
}

/// The safe next action for a host after an operation's recovery facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeNextAction {
    /// Continue using the current runtime and its current binding.
    Continue,
    /// Re-establish the invalidated scope before issuing dependent work.
    Rebaseline,
    /// Replace the runtime incarnation; the developer session may remain.
    ReplaceIncarnation,
}

/// Source-owned facts that let a host select recovery without parsing an error
/// code or diagnostic. This vocabulary describes recovery only; it does not
/// make an uncertain gameplay callback safe to replay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeRecovery {
    pub mutation: RuntimeMutationCertainty,
    pub invalidated_scope: RuntimeInvalidatedScope,
    pub next_action: RuntimeNextAction,
}

impl RuntimeRecovery {
    pub const fn committed() -> Self {
        Self {
            mutation: RuntimeMutationCertainty::Committed,
            invalidated_scope: RuntimeInvalidatedScope::None,
            next_action: RuntimeNextAction::Continue,
        }
    }

    pub const fn not_applied() -> Self {
        Self {
            mutation: RuntimeMutationCertainty::NotApplied,
            invalidated_scope: RuntimeInvalidatedScope::None,
            next_action: RuntimeNextAction::Continue,
        }
    }

    pub const fn output_rebaseline() -> Self {
        Self {
            mutation: RuntimeMutationCertainty::Unknown,
            invalidated_scope: RuntimeInvalidatedScope::Outputs,
            next_action: RuntimeNextAction::Rebaseline,
        }
    }

    pub const fn incarnation_tainted() -> Self {
        Self {
            mutation: RuntimeMutationCertainty::Unknown,
            invalidated_scope: RuntimeInvalidatedScope::Incarnation,
            next_action: RuntimeNextAction::ReplaceIncarnation,
        }
    }

    pub const fn mutation(self) -> RuntimeMutationCertainty {
        self.mutation
    }

    pub const fn invalidated_scope(self) -> RuntimeInvalidatedScope {
        self.invalidated_scope
    }

    pub const fn next_action(self) -> RuntimeNextAction {
        self.next_action
    }
}

/// One serialized owner for a concrete runtime instance.
///
/// The lock result deliberately retains the standard poisoning information.
/// An owning host maps it to its own diagnostic vocabulary and decides how to
/// replace or recover its runtime; this type never selects that policy.
pub struct RuntimeSession<R> {
    runtime: Mutex<R>,
}

impl<R> RuntimeSession<R> {
    pub fn new(runtime: R) -> Self {
        Self {
            runtime: Mutex::new(runtime),
        }
    }

    /// Acquires the serialization guard. Holding the returned guard permits a
    /// host to make a snapshot and output cursor capture one atomic handover.
    pub fn lock(&self) -> LockResult<MutexGuard<'_, R>> {
        self.runtime.lock()
    }
}

/// An operation's owned result and logical outputs, independent of transport
/// encoding, delivery cursors, queue bounds, and host DTOs.
#[derive(Debug, Clone)]
pub struct RuntimeReceipt<T, O> {
    result: T,
    outputs: Vec<O>,
}

impl<T, O> RuntimeReceipt<T, O> {
    pub fn new(result: T, outputs: Vec<O>) -> Self {
        Self { result, outputs }
    }
    pub fn result(&self) -> &T {
        &self.result
    }
    pub fn into_parts(self) -> (T, Vec<O>) {
        (self.result, self.outputs)
    }
}

/// A replacement whose producer has been quiesced before projection install.
/// Preparation must stop/join the previous publisher under its own session
/// serialization. Only after that completes may a host acquire projection
/// write ownership; an old reader may still need a projection read to finish
/// acknowledging a snapshot boundary during preparation.
pub struct PreparedRuntimeReplacement<T>(T);

impl<T> PreparedRuntimeReplacement<T> {
    pub fn prepare<E>(prepare: impl FnOnce() -> Result<T, E>) -> Result<Self, E> {
        prepare().map(Self)
    }

    pub fn into_inner(self) -> T {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{mpsc, Arc},
        thread,
        time::Duration,
    };

    use super::*;

    #[test]
    fn recovery_facts_preserve_the_stable_wire_shape() {
        let recovery = RuntimeRecovery::output_rebaseline();
        assert_eq!(
            serde_json::to_string(&recovery).expect("recovery serializes"),
            r#"{"mutation":"unknown","invalidatedScope":"outputs","nextAction":"rebaseline"}"#
        );
        assert_eq!(
            RuntimeRecovery::incarnation_tainted().next_action(),
            RuntimeNextAction::ReplaceIncarnation
        );
    }

    #[test]
    fn owner_calls_share_one_serialization_guard() {
        let session = Arc::new(RuntimeSession::new(0_u8));
        let guard = session.lock().expect("session lock");
        let blocked = Arc::clone(&session);
        let (sent, received) = mpsc::channel();
        let join = thread::spawn(move || {
            let mut value = blocked.lock().expect("serialized owner call");
            *value += 1;
            sent.send(()).expect("completion marker");
        });
        assert!(
            received.recv_timeout(Duration::from_millis(25)).is_err(),
            "owner call bypassed the session serialization guard"
        );
        drop(guard);
        received
            .recv_timeout(Duration::from_secs(1))
            .expect("owner call completed after guard release");
        join.join().expect("owner worker");
        assert_eq!(*session.lock().unwrap(), 1);
    }

    #[test]
    fn poisoned_owner_lock_remains_observable() {
        let session = Arc::new(RuntimeSession::new(()));
        let poisoned = Arc::clone(&session);
        let _ = thread::spawn(move || {
            let _guard = poisoned.lock().expect("fixture session lock");
            panic!("poison fixture lock");
        })
        .join();
        assert!(session.lock().is_err());
    }
}
