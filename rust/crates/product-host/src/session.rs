use std::sync::{LockResult, Mutex, MutexGuard, PoisonError};

use crate::{
    CanonicalU64, ProductHostDebugResult, ProductHostError, ProductHostInputBatch,
    ProductHostLifecycleOperation, ProductHostOperationResult, ProductHostRuntime,
    ProductHostRuntimeBinding, ProductHostRuntimeError, ProductHostRuntimeReceipt,
    ProductHostRuntimeScheduleState, ProductHostTimelineCompletion,
    ProductHostTimelineCompletionResult, ProductHostUpdateAttribution,
};

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

/// One serialized, transport-neutral session over a generated Product
/// Runtime. The session owns no output subscription, callbacks, registry, or
/// product state; each operation directly returns the runtime owner's bounded
/// result and output batch.
pub struct ProductHostOperationOwner<R> {
    session: RuntimeSession<R>,
}

impl<R> ProductHostOperationOwner<R> {
    pub fn new(runtime: R) -> Self {
        Self {
            session: RuntimeSession::new(runtime),
        }
    }

    /// Exposes the host-neutral serialized scope for host-owned work that must
    /// remain ordered with a direct product operation.
    pub(crate) fn session(&self) -> &RuntimeSession<R> {
        &self.session
    }
}

impl<R: ProductHostRuntime> ProductHostOperationOwner<R> {
    /// Reloads staged content under the same serialization guard as every
    /// product operation, so no callback observes a half-swapped inventory.
    pub fn reload_content(&self) -> Result<(), ProductHostRuntimeError> {
        self.session
            .with_locked(|runtime| runtime.reload_content())
            .map_err(|_| runtime_poisoned())?
    }

    /// Drains call-local attribution while the outer publisher holds its
    /// operation/publication order (used by the disposable worker adapter).
    pub fn take_update_attribution(
        &self,
    ) -> Result<Option<ProductHostUpdateAttribution>, ProductHostRuntimeError> {
        self.session
            .with_locked(|runtime| runtime.take_update_attribution())
            .map_err(|_| runtime_poisoned())
    }

    /// Reads the runtime's explicit scheduler posture while holding the same
    /// serialization guard as every mutating operation.
    pub fn realtime_schedule_state(
        &self,
    ) -> Result<ProductHostRuntimeScheduleState, ProductHostRuntimeError> {
        self.session
            .with_locked(|runtime| runtime.realtime_schedule_state())
            .map_err(|_| runtime_poisoned())
    }

    /// Reads the runtime's admitted realtime observation interval under the
    /// same owner lock used by lifecycle and update operations.
    pub fn realtime_schedule_interval(
        &self,
    ) -> Result<Option<std::time::Duration>, ProductHostRuntimeError> {
        self.session
            .with_locked(|runtime| runtime.realtime_schedule_interval())
            .map_err(|_| runtime_poisoned())
    }

    /// Reads how often the runtime wants presentation between observations.
    pub fn presentation_interval(
        &self,
    ) -> Result<Option<std::time::Duration>, ProductHostRuntimeError> {
        self.session
            .with_locked(|runtime| runtime.presentation_interval())
            .map_err(|_| runtime_poisoned())
    }

    /// Presents Engine-owned motion between realtime observations.
    pub fn present_realtime(
        &self,
        observed_time_ns: CanonicalU64,
    ) -> Result<
        Option<ProductHostRuntimeReceipt<ProductHostOperationResult>>,
        ProductHostRuntimeError,
    > {
        self.session
            .with_locked(|runtime| runtime.present_realtime(observed_time_ns))
            .map_err(|_| runtime_poisoned())?
    }

    /// Runs one explicit lifecycle operation while holding the session's
    /// serialization guard for the complete owner call.
    pub fn lifecycle(
        &self,
        operation: ProductHostLifecycleOperation,
    ) -> Result<ProductHostRuntimeReceipt<ProductHostOperationResult>, ProductHostRuntimeError>
    {
        self.with_runtime(|runtime| runtime.lifecycle(operation))
    }

    pub fn connect(
        &self,
    ) -> Result<ProductHostRuntimeReceipt<ProductHostOperationResult>, ProductHostRuntimeError>
    {
        self.with_runtime(|runtime| runtime.connect())
    }

    pub fn lifecycle_with_binding(
        &self,
        operation: ProductHostLifecycleOperation,
        binding: Option<ProductHostRuntimeBinding>,
    ) -> Result<ProductHostRuntimeReceipt<ProductHostOperationResult>, ProductHostRuntimeError>
    {
        self.with_runtime(|runtime| runtime.lifecycle_with_binding(operation, binding))
    }

    /// Retire queued transport input only after the runtime admits the fence,
    /// while still holding the same owner lock used by scheduler draining.
    pub fn lifecycle_with_input_fence(
        &self,
        operation: ProductHostLifecycleOperation,
        binding: Option<ProductHostRuntimeBinding>,
        clear_admitted: impl FnOnce(),
    ) -> Result<ProductHostRuntimeReceipt<ProductHostOperationResult>, ProductHostRuntimeError>
    {
        self.with_runtime(|runtime| {
            let receipt = runtime.lifecycle_with_binding(operation, binding)?;
            if receipt.result().is_accepted() {
                clear_admitted();
            }
            Ok(receipt)
        })
    }

    pub fn control_with_input_fence(
        &self,
        operation: crate::ProductHostControlOperation,
        binding: ProductHostRuntimeBinding,
        clear_admitted: impl FnOnce(),
    ) -> Result<ProductHostRuntimeReceipt<ProductHostOperationResult>, ProductHostRuntimeError>
    {
        self.with_runtime(|runtime| {
            let receipt = runtime.control(operation, binding)?;
            if receipt.result().is_accepted() {
                clear_admitted();
            }
            Ok(receipt)
        })
    }

    pub fn control(
        &self,
        operation: crate::ProductHostControlOperation,
        binding: ProductHostRuntimeBinding,
    ) -> Result<ProductHostRuntimeReceipt<ProductHostOperationResult>, ProductHostRuntimeError>
    {
        self.with_runtime(|runtime| runtime.control(operation, binding))
    }

    /// Reestablishes input from the loaded runtime's current authoritative binding.
    pub fn recover_input_overflow(
        &self,
    ) -> Result<ProductHostRuntimeReceipt<ProductHostOperationResult>, ProductHostRuntimeError>
    {
        self.with_runtime(|runtime| runtime.recover_input_overflow())
    }

    /// Admits one already validated input batch through the runtime owner.
    pub fn input(
        &self,
        batch: ProductHostInputBatch,
    ) -> Result<ProductHostRuntimeReceipt<crate::ProductHostInputResult>, ProductHostRuntimeError>
    {
        self.with_runtime(|runtime| runtime.input(batch))
    }

    /// Executes a product-owned generated debug command under the same mutex
    /// as lifecycle and update work. This is the initial direct safe point;
    /// no separate debug runtime or scheduler is introduced.
    pub fn execute_debug(
        &self,
        command: &str,
    ) -> Result<ProductHostRuntimeReceipt<ProductHostDebugResult>, ProductHostRuntimeError> {
        self.with_runtime(|runtime| runtime.execute_debug(command))
    }

    /// Bring already queued input into the same owner scope before a time command.
    pub fn execute_debug_with_input<F>(
        &self,
        command: &str,
        drain: F,
    ) -> Result<ProductHostRuntimeReceipt<ProductHostDebugResult>, ProductHostRuntimeError>
    where
        F: FnOnce() -> (Vec<ProductHostInputBatch>, bool),
    {
        self.with_runtime(|runtime| {
            let (batches, overflowed) = drain();
            let mut outputs = Vec::new();
            if overflowed {
                outputs.extend(runtime.recover_input_overflow()?.into_parts().1);
            }
            for batch in batches {
                outputs.extend(runtime.input(batch)?.into_parts().1);
            }
            let (result, debug_outputs) = runtime.execute_debug(command)?.into_parts();
            outputs.extend(debug_outputs);
            ProductHostRuntimeReceipt::new(result, outputs).map_err(host_error_to_runtime)
        })
    }

    pub fn describe_debug(
        &self,
    ) -> Result<ProductHostRuntimeReceipt<crate::ProductHostDebugCatalog>, ProductHostRuntimeError>
    {
        self.with_runtime(|runtime| runtime.describe_debug())
    }

    /// Strictly admits an input wire array, then forwards the validated batch
    /// through the same direct owner path as [`Self::input`].
    pub fn input_json(
        &self,
        bytes: &[u8],
    ) -> Result<ProductHostRuntimeReceipt<crate::ProductHostInputResult>, ProductHostRuntimeError>
    {
        let batch = ProductHostInputBatch::decode_json(bytes).map_err(host_error_to_runtime)?;
        self.input(batch)
    }

    /// Advances the realtime lane with one canonical host time.
    pub fn advance_realtime(
        &self,
        observed_time_ns: CanonicalU64,
    ) -> Result<ProductHostRuntimeReceipt<ProductHostOperationResult>, ProductHostRuntimeError>
    {
        self.with_runtime(|runtime| runtime.advance_realtime(observed_time_ns))
    }

    /// Strictly admits a canonical JSON u64 and advances the realtime lane.
    pub fn advance_realtime_json(
        &self,
        bytes: &[u8],
    ) -> Result<ProductHostRuntimeReceipt<ProductHostOperationResult>, ProductHostRuntimeError>
    {
        let observed_time_ns = decode_canonical_u64(bytes)?;
        self.advance_realtime(observed_time_ns)
    }

    /// Admits one already validated timeline completion through the runtime
    /// owner.
    pub fn complete_timeline(
        &self,
        completion: ProductHostTimelineCompletion,
    ) -> Result<
        ProductHostRuntimeReceipt<ProductHostTimelineCompletionResult>,
        ProductHostRuntimeError,
    > {
        self.with_runtime(|runtime| runtime.complete_timeline(completion))
    }

    /// Strictly admits a timeline completion wire object and forwards it
    /// through the same direct owner path as [`Self::complete_timeline`].
    pub fn complete_timeline_json(
        &self,
        bytes: &[u8],
    ) -> Result<
        ProductHostRuntimeReceipt<ProductHostTimelineCompletionResult>,
        ProductHostRuntimeError,
    > {
        let completion =
            ProductHostTimelineCompletion::decode_json(bytes).map_err(host_error_to_runtime)?;
        self.complete_timeline(completion)
    }

    pub(crate) fn with_runtime<T, F>(
        &self,
        call: F,
    ) -> Result<ProductHostRuntimeReceipt<T>, ProductHostRuntimeError>
    where
        F: FnOnce(&mut R) -> Result<ProductHostRuntimeReceipt<T>, ProductHostRuntimeError>,
    {
        self.session
            .with_locked(call)
            .map_err(|_| runtime_poisoned())
            .and_then(|result| result)
    }
}

pub(crate) fn runtime_poisoned() -> ProductHostRuntimeError {
    ProductHostRuntimeError::new(
        "PRODUCT_HOST_RUNTIME_POISONED",
        "runtime serialization lock is poisoned",
    )
}

fn decode_canonical_u64(bytes: &[u8]) -> Result<CanonicalU64, ProductHostRuntimeError> {
    if bytes.len() > crate::MAX_REQUEST_BODY_BYTES {
        return Err(host_error_to_runtime(ProductHostError::new(
            "PRODUCT_HOST_BODY_BOUNDS",
            "JSON payload exceeds the host body bound",
        )));
    }
    CanonicalU64::decode_json(bytes)
        .map_err(|_| {
            ProductHostError::new(
                "PRODUCT_HOST_CANONICAL_U64",
                "canonical u64 JSON is invalid",
            )
        })
        .map_err(host_error_to_runtime)
}

fn host_error_to_runtime(error: ProductHostError) -> ProductHostRuntimeError {
    ProductHostRuntimeError::new(error.code(), error.detail())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::publication::RuntimePublication;
    use crate::ProductHostOperationKind;
    use runtime_input::RuntimeInputBinding;
    use runtime_lifecycle::{RuntimeControlRevision, RuntimeGeneration, RuntimeInstanceId};

    struct FixtureRuntime;

    impl FixtureRuntime {
        fn operation(
            operation: crate::ProductHostOperationKind,
        ) -> ProductHostRuntimeReceipt<ProductHostOperationResult> {
            ProductHostRuntimeReceipt::new(
                ProductHostOperationResult::rejected(operation, "fixture").unwrap(),
                publications(),
            )
            .unwrap()
        }
    }

    fn binding() -> crate::ProductHostRuntimeBinding {
        crate::ProductHostRuntimeBinding {
            instance_id: CanonicalU64::new(1),
            generation: CanonicalU64::new(1),
            control_revision: CanonicalU64::new(1),
        }
    }

    fn publications() -> Vec<RuntimePublication> {
        vec![RuntimePublication::binding(
            RuntimeInputBinding::new(
                RuntimeInstanceId::new(1),
                RuntimeGeneration::new(1),
                RuntimeControlRevision::new(1),
            ),
            0,
        )]
    }

    fn readout() -> crate::ProductHostRuntimeReadout {
        crate::ProductHostRuntimeReadout::new(binding(), crate::ProductHostRuntimeState::Running)
    }

    impl ProductHostRuntime for FixtureRuntime {
        fn lifecycle(
            &mut self,
            operation: ProductHostLifecycleOperation,
        ) -> Result<ProductHostRuntimeReceipt<ProductHostOperationResult>, ProductHostRuntimeError>
        {
            Ok(Self::operation(operation.operation_kind()))
        }

        fn input(
            &mut self,
            batch: ProductHostInputBatch,
        ) -> Result<ProductHostRuntimeReceipt<crate::ProductHostInputResult>, ProductHostRuntimeError>
        {
            let accepted_through = batch
                .events()
                .last()
                .map(|event| CanonicalU64::new(event.sequence()));
            Ok(ProductHostRuntimeReceipt::new(
                crate::ProductHostInputResult::with_progress(
                    batch.events().len(),
                    batch.events().len(),
                    0,
                    accepted_through,
                    accepted_through,
                    CanonicalU64::new(2),
                    binding(),
                    readout(),
                )
                .unwrap(),
                publications(),
            )
            .unwrap())
        }

        fn advance_realtime(
            &mut self,
            _observed_time_ns: CanonicalU64,
        ) -> Result<ProductHostRuntimeReceipt<ProductHostOperationResult>, ProductHostRuntimeError>
        {
            Ok(Self::operation(ProductHostOperationKind::AdvanceRealtime))
        }

        fn complete_timeline(
            &mut self,
            completion: ProductHostTimelineCompletion,
        ) -> Result<
            ProductHostRuntimeReceipt<ProductHostTimelineCompletionResult>,
            ProductHostRuntimeError,
        > {
            Ok(ProductHostRuntimeReceipt::new(
                ProductHostTimelineCompletionResult::rejected(
                    CanonicalU64::new(completion.envelope().ticket().value()),
                    "fixture",
                )
                .unwrap(),
                publications(),
            )
            .unwrap())
        }
    }

    #[test]
    fn rejected_lifecycle_and_control_preserve_queued_input_fence() {
        let owner = ProductHostOperationOwner::new(FixtureRuntime);
        let cleared = std::cell::Cell::new(false);
        let result = owner
            .lifecycle_with_input_fence(
                ProductHostLifecycleOperation::Pause,
                Some(binding()),
                || cleared.set(true),
            )
            .unwrap();
        assert!(!result.result().is_accepted());
        assert!(!cleared.get());
        let _ = owner.control_with_input_fence(
            crate::ProductHostControlOperation::Replace,
            binding(),
            || cleared.set(true),
        );
        assert!(!cleared.get());
    }

    #[test]
    fn direct_and_json_operations_return_owner_receipts() {
        let session = ProductHostOperationOwner::new(FixtureRuntime);
        assert_eq!(
            session
                .lifecycle(ProductHostLifecycleOperation::Start)
                .unwrap()
                .into_parts()
                .1
                .len(),
            1
        );
        assert_eq!(session.input_json(b"[]").unwrap().into_parts().1.len(), 1);
        assert_eq!(
            session
                .advance_realtime_json(br#""2""#)
                .unwrap()
                .into_parts()
                .1
                .len(),
            1
        );
        let completion = br#"{
            "ticket":"4",
            "runtime":{"instanceId":"1","generation":"1","controlRevision":"1"},
            "correlation":"fixture",
            "outcome":{"kind":"success"},
            "provenance":{"correlation":"fixture"}
        }"#;
        assert_eq!(
            session
                .complete_timeline_json(completion)
                .unwrap()
                .into_parts()
                .1
                .len(),
            1
        );
    }

    #[test]
    fn json_admission_rejects_malformed_and_trailing_payloads() {
        let session = ProductHostOperationOwner::new(FixtureRuntime);
        let input = session.input_json(br#"[] trailing"#).unwrap_err();
        assert_eq!(input.code(), "PRODUCT_HOST_INPUT_DECODE");
        let time = session.advance_realtime_json(br#"01"#).unwrap_err();
        assert_eq!(time.code(), "PRODUCT_HOST_CANONICAL_U64");
        let timeline = session.complete_timeline_json(br#"{}"#).unwrap_err();
        assert_eq!(timeline.code(), "PRODUCT_HOST_TIMELINE_DECODE");
    }
}

#[cfg(test)]
mod serialization_tests {
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
