//! Host-neutral runtime-session ownership.
//!
//! A runtime session serializes access to one concrete runtime instance and
//! carries an operation's result with its logical outputs. It does not define
//! operation dispatch, callbacks, output subscriptions, scheduling or
//! transport policy. Hosts can keep a session lock across an atomic
//! snapshot/cursor handover when their own publication contract requires it.

#![forbid(unsafe_code)]

use std::sync::{LockResult, Mutex, MutexGuard, PoisonError};

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

    /// Runs one typed owner operation under the serialization guard.
    ///
    /// The closure runs while the guard is held. This lets a host keep a
    /// runtime mutation and its own immediate publication or cursor handover
    /// in one ordered scope without teaching the session about either policy.
    /// Standard mutex poisoning remains visible to the host unchanged.
    pub fn with_locked<T, F>(&self, call: F) -> Result<T, PoisonError<MutexGuard<'_, R>>>
    where
        F: FnOnce(&mut R) -> T,
    {
        let mut runtime = self.runtime.lock()?;
        Ok(call(&mut runtime))
    }

    /// Runs one typed owner operation with host lifecycle callbacks inside the
    /// serialization guard. `begin` runs after lock acquisition and `finish`
    /// runs before the guard is released, so a waiting contender cannot appear
    /// to execute while the current operation is still being observed.
    ///
    /// Standard mutex poisoning remains visible to the host unchanged.
    pub fn with_locked_timed<T, F, B, E>(
        &self,
        begin: B,
        call: F,
        finish: E,
    ) -> Result<T, PoisonError<MutexGuard<'_, R>>>
    where
        F: FnOnce(&mut R) -> T,
        B: FnOnce(),
        E: FnOnce(),
    {
        let mut runtime = self.runtime.lock()?;
        begin();
        let result = call(&mut runtime);
        finish();
        Ok(result)
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

#[cfg(test)]
mod tests {
    use std::{
        sync::{mpsc, Arc},
        thread,
        time::Duration,
    };

    use super::*;

    #[test]
    fn owner_calls_share_one_serialization_guard() {
        let session = Arc::new(RuntimeSession::new(0_u8));
        let guard = session.lock().expect("session lock");
        let blocked = Arc::clone(&session);
        let (sent, received) = mpsc::channel();
        let join = thread::spawn(move || {
            blocked
                .with_locked(|value| *value += 1)
                .expect("serialized owner call");
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
    fn timed_owner_scope_keeps_callbacks_inside_the_lock() {
        let session = Arc::new(RuntimeSession::new(0_u8));
        let (began, began_ready) = mpsc::channel();
        let (release, release_owner) = mpsc::channel();
        let (finished, finished_ready) = mpsc::channel();
        let scoped = Arc::clone(&session);
        let owner = thread::spawn(move || {
            scoped
                .with_locked_timed(
                    || began.send(()).expect("begin marker"),
                    |value| {
                        *value += 1;
                        release_owner.recv().expect("release owner scope");
                    },
                    || finished.send(()).expect("finish marker"),
                )
                .expect("timed owner scope");
        });
        began_ready.recv().expect("owner scope began");

        let blocked = Arc::clone(&session);
        let (contender_done, contender_ready) = mpsc::channel();
        let contender = thread::spawn(move || {
            blocked.with_locked(|_| ()).expect("contending owner scope");
            contender_done.send(()).expect("contender marker");
        });
        assert!(
            contender_ready
                .recv_timeout(Duration::from_millis(25))
                .is_err(),
            "a contender bypassed the timed owner scope"
        );
        release.send(()).expect("release timed owner scope");
        finished_ready
            .recv_timeout(Duration::from_secs(1))
            .expect("finish ran before guard release");
        contender_ready
            .recv_timeout(Duration::from_secs(1))
            .expect("contender ran after owner scope");
        owner.join().expect("timed owner worker");
        contender.join().expect("contending owner worker");
        assert_eq!(*session.lock().expect("final session lock"), 1);
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
        assert!(session.with_locked(|_| ()).is_err());
    }
}
