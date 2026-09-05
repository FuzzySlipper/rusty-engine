#[cfg(test)]
use std::sync::MutexGuard;

use runtime_session::RuntimeSession;

use crate::{
    CanonicalU64, ProductDevDebugResult, ProductDevHostError, ProductDevInputBatch,
    ProductDevInputResult, ProductDevLifecycleOperation, ProductDevOperationResult,
    ProductDevRuntime, ProductDevRuntimeBinding, ProductDevRuntimeError, ProductDevRuntimeReceipt,
    ProductDevRuntimeScheduleState, ProductDevTimelineCompletion,
    ProductDevTimelineCompletionResult, ProductDevUpdateAttribution,
};

/// One serialized, transport-neutral session over a generated Product
/// Runtime. The session owns no output subscription, callbacks, registry, or
/// product state; each operation directly returns the runtime owner's bounded
/// result and output batch.
pub struct ProductDevOperationOwner<R> {
    session: RuntimeSession<R>,
}

impl<R> ProductDevOperationOwner<R> {
    pub fn new(runtime: R) -> Self {
        Self {
            session: RuntimeSession::new(runtime),
        }
    }
}

impl<R: ProductDevRuntime> ProductDevOperationOwner<R> {
    /// Drains call-local attribution while the outer publisher holds its
    /// operation/publication order (used by the disposable worker adapter).
    pub fn take_update_attribution(
        &self,
    ) -> Result<Option<ProductDevUpdateAttribution>, ProductDevRuntimeError> {
        let mut runtime = self.session.lock().map_err(|_| runtime_poisoned())?;
        Ok(runtime.take_update_attribution())
    }

    /// Reads the runtime's explicit scheduler posture while holding the same
    /// serialization guard as every mutating operation.
    pub fn realtime_schedule_state(
        &self,
    ) -> Result<ProductDevRuntimeScheduleState, ProductDevRuntimeError> {
        let runtime = self.session.lock().map_err(|_| runtime_poisoned())?;
        Ok(runtime.realtime_schedule_state())
    }

    /// Reads the runtime's admitted realtime observation interval under the
    /// same owner lock used by lifecycle and update operations.
    pub fn realtime_schedule_interval(
        &self,
    ) -> Result<Option<std::time::Duration>, ProductDevRuntimeError> {
        let runtime = self.session.lock().map_err(|_| runtime_poisoned())?;
        Ok(runtime.realtime_schedule_interval())
    }

    /// Runs one explicit lifecycle operation while holding the session's
    /// serialization guard for the complete owner call.
    pub fn lifecycle(
        &self,
        operation: ProductDevLifecycleOperation,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevOperationResult>, ProductDevRuntimeError> {
        self.with_runtime(|runtime| runtime.lifecycle(operation))
    }

    pub fn connect(
        &self,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevOperationResult>, ProductDevRuntimeError> {
        self.with_runtime(|runtime| runtime.connect())
    }

    pub fn lifecycle_with_binding(
        &self,
        operation: ProductDevLifecycleOperation,
        binding: Option<ProductDevRuntimeBinding>,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevOperationResult>, ProductDevRuntimeError> {
        self.with_runtime(|runtime| runtime.lifecycle_with_binding(operation, binding))
    }

    pub fn control(
        &self,
        operation: crate::ProductDevControlOperation,
        binding: ProductDevRuntimeBinding,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevOperationResult>, ProductDevRuntimeError> {
        self.with_runtime(|runtime| runtime.control(operation, binding))
    }

    /// Admits one already validated input batch through the runtime owner.
    pub fn input(
        &self,
        batch: ProductDevInputBatch,
    ) -> Result<ProductDevRuntimeReceipt<crate::ProductDevInputResult>, ProductDevRuntimeError>
    {
        self.with_runtime(|runtime| runtime.input(batch))
    }

    /// Executes a product-owned generated debug command under the same mutex
    /// as lifecycle and update work. This is the initial direct safe point;
    /// no separate debug runtime or scheduler is introduced.
    pub fn execute_debug(
        &self,
        command: &str,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevDebugResult>, ProductDevRuntimeError> {
        self.with_runtime(|runtime| runtime.execute_debug(command))
    }

    pub fn describe_debug(
        &self,
    ) -> Result<ProductDevRuntimeReceipt<crate::ProductDevDebugCatalog>, ProductDevRuntimeError>
    {
        self.with_runtime(|runtime| runtime.describe_debug())
    }

    pub fn report_audio_feedback(
        &self,
        feedback: crate::ProductDevAudioFeedback,
    ) -> Result<
        ProductDevRuntimeReceipt<crate::ProductDevAudioFeedbackResult>,
        ProductDevRuntimeError,
    > {
        self.with_runtime(|runtime| runtime.report_audio_feedback(feedback))
    }

    pub fn report_animation_feedback(
        &self,
        feedback: crate::ProductDevAnimationFeedback,
    ) -> Result<
        ProductDevRuntimeReceipt<crate::ProductDevAnimationFeedbackResult>,
        ProductDevRuntimeError,
    > {
        self.with_runtime(|runtime| runtime.report_animation_feedback(feedback))
    }

    pub fn report_ghost_plate_feedback(
        &self,
        feedback: crate::ProductDevGhostPlateFeedback,
    ) -> Result<
        ProductDevRuntimeReceipt<crate::ProductDevGhostPlateFeedbackResult>,
        ProductDevRuntimeError,
    > {
        self.with_runtime(|runtime| runtime.report_ghost_plate_feedback(feedback))
    }

    pub fn report_renderer_diagnostics(
        &self,
        feedback: crate::ProductDevRendererDiagnosticsFeedback,
    ) -> Result<
        ProductDevRuntimeReceipt<crate::ProductDevRendererDiagnosticsFeedbackResult>,
        ProductDevRuntimeError,
    > {
        self.with_runtime(|runtime| runtime.report_renderer_diagnostics(feedback))
    }

    /// Strictly admits an input wire array, then forwards the validated batch
    /// through the same direct owner path as [`Self::input`].
    pub fn input_json(
        &self,
        bytes: &[u8],
    ) -> Result<ProductDevRuntimeReceipt<crate::ProductDevInputResult>, ProductDevRuntimeError>
    {
        let batch = ProductDevInputBatch::decode_json(bytes).map_err(host_error_to_runtime)?;
        self.input(batch)
    }

    /// Advances the realtime lane with one canonical host time.
    pub fn advance_realtime(
        &self,
        observed_time_ns: CanonicalU64,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevOperationResult>, ProductDevRuntimeError> {
        self.with_runtime(|runtime| runtime.advance_realtime(observed_time_ns))
    }

    /// Atomically drains one host-mailbox snapshot through the runtime input
    /// owner immediately before one realtime advance. Each successful input
    /// receipt is delivered to `publish_input` before the update receipt;
    /// input admission errors are returned as recoverable observations while
    /// the scheduled advance still proceeds. Lifecycle/update serialization
    /// never has a second owner or an input queue hidden inside the browser
    /// route. Both publication callbacks run while this owner lock is held, so
    /// output ordering cannot race a lifecycle/control operation.
    pub fn advance_realtime_with_input_and_publish<F, I, P, B, E>(
        &self,
        drain: F,
        observed_time_ns: CanonicalU64,
        mut publish_input: I,
        mut publish: P,
        begin: B,
        finish: E,
    ) -> Result<
        (
            Vec<ProductDevRuntimeError>,
            Option<ProductDevUpdateAttribution>,
        ),
        ProductDevRuntimeError,
    >
    where
        F: FnOnce() -> (Vec<ProductDevInputBatch>, bool),
        I: FnMut(ProductDevRuntimeReceipt<ProductDevInputResult>),
        P: FnMut(ProductDevRuntimeReceipt<ProductDevOperationResult>),
        B: FnOnce(),
        E: FnOnce(),
    {
        let mut runtime = self.session.lock().map_err(|_| runtime_poisoned())?;
        begin();
        let (batches, overflowed) = drain();
        let mut input_errors = Vec::new();
        if overflowed {
            match runtime.recover_input_overflow() {
                Ok(receipt) => publish(receipt),
                Err(error) => input_errors.push(error),
            }
        }
        for batch in batches {
            match runtime.input(batch) {
                Ok(receipt) => publish_input(receipt),
                Err(error) => input_errors.push(error),
            }
        }
        let result = runtime.advance_realtime(observed_time_ns);
        let attribution = runtime.take_update_attribution();
        let result = match result {
            Ok(receipt) => {
                publish(receipt);
                Ok((input_errors, attribution))
            }
            Err(error) => Err(error),
        };
        finish();
        result
    }

    /// Strictly admits a canonical JSON u64 and advances the realtime lane.
    pub fn advance_realtime_json(
        &self,
        bytes: &[u8],
    ) -> Result<ProductDevRuntimeReceipt<ProductDevOperationResult>, ProductDevRuntimeError> {
        let observed_time_ns = decode_canonical_u64(bytes)?;
        self.advance_realtime(observed_time_ns)
    }

    /// Admits one demand step through the runtime owner.
    pub fn admit_demand_step(
        &self,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevOperationResult>, ProductDevRuntimeError> {
        self.with_runtime(|runtime| runtime.admit_demand_step())
    }

    /// Admits one canonical external step through the runtime owner.
    pub fn admit_external_step(
        &self,
        step: CanonicalU64,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevOperationResult>, ProductDevRuntimeError> {
        self.with_runtime(|runtime| runtime.admit_external_step(step))
    }

    /// Strictly admits a canonical JSON u64 and forwards an external step.
    pub fn admit_external_step_json(
        &self,
        bytes: &[u8],
    ) -> Result<ProductDevRuntimeReceipt<ProductDevOperationResult>, ProductDevRuntimeError> {
        let step = decode_canonical_u64(bytes)?;
        self.admit_external_step(step)
    }

    /// Admits one already validated timeline completion through the runtime
    /// owner.
    pub fn complete_timeline(
        &self,
        completion: ProductDevTimelineCompletion,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevTimelineCompletionResult>, ProductDevRuntimeError>
    {
        self.with_runtime(|runtime| runtime.complete_timeline(completion))
    }

    /// Strictly admits a timeline completion wire object and forwards it
    /// through the same direct owner path as [`Self::complete_timeline`].
    pub fn complete_timeline_json(
        &self,
        bytes: &[u8],
    ) -> Result<ProductDevRuntimeReceipt<ProductDevTimelineCompletionResult>, ProductDevRuntimeError>
    {
        let completion =
            ProductDevTimelineCompletion::decode_json(bytes).map_err(host_error_to_runtime)?;
        self.complete_timeline(completion)
    }

    pub(crate) fn with_runtime<T, F>(
        &self,
        call: F,
    ) -> Result<ProductDevRuntimeReceipt<T>, ProductDevRuntimeError>
    where
        F: FnOnce(&mut R) -> Result<ProductDevRuntimeReceipt<T>, ProductDevRuntimeError>,
    {
        self.with_locked_runtime(call)
    }

    /// Runs an owner operation while retaining the serialization guard for
    /// the complete callback. Host routes use this when they must publish a
    /// receipt before another runtime mutation can begin.
    pub(crate) fn with_locked_runtime<T, F>(&self, call: F) -> Result<T, ProductDevRuntimeError>
    where
        F: FnOnce(&mut R) -> Result<T, ProductDevRuntimeError>,
    {
        let mut runtime = self.session.lock().map_err(|_| runtime_poisoned())?;
        call(&mut runtime)
    }

    /// Runs a serialized owner operation with lifecycle callbacks inside the
    /// owner lock. Host telemetry uses this to represent the operation that is
    /// actually executing, rather than a contender merely waiting for it.
    pub(crate) fn with_locked_runtime_timed<T, F, B, E>(
        &self,
        begin: B,
        call: F,
        finish: E,
    ) -> Result<T, ProductDevRuntimeError>
    where
        F: FnOnce(&mut R) -> Result<T, ProductDevRuntimeError>,
        B: FnOnce(),
        E: FnOnce(),
    {
        let mut runtime = self.session.lock().map_err(|_| runtime_poisoned())?;
        begin();
        let result = call(&mut runtime);
        finish();
        result
    }

    #[cfg(test)]
    fn lock_for_test(&self) -> MutexGuard<'_, R> {
        self.session.lock().expect("fixture session lock")
    }
}

fn runtime_poisoned() -> ProductDevRuntimeError {
    ProductDevRuntimeError::new(
        "DEV_HOST_RUNTIME_POISONED",
        "runtime serialization lock is poisoned",
    )
    .expect("fixed runtime poison diagnostic is valid")
}

fn decode_canonical_u64(bytes: &[u8]) -> Result<CanonicalU64, ProductDevRuntimeError> {
    if bytes.len() > crate::MAX_REQUEST_BODY_BYTES {
        return Err(host_error_to_runtime(ProductDevHostError::new(
            "DEV_HOST_BODY_BOUNDS",
            "JSON payload exceeds the host body bound",
        )));
    }
    CanonicalU64::decode_json(bytes)
        .map_err(|_| {
            ProductDevHostError::new("DEV_HOST_CANONICAL_U64", "canonical u64 JSON is invalid")
        })
        .map_err(host_error_to_runtime)
}

fn host_error_to_runtime(error: ProductDevHostError) -> ProductDevRuntimeError {
    ProductDevRuntimeError::new(error.code(), error.detail())
        .expect("bounded host error has a valid runtime diagnostic")
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{mpsc, Arc, Mutex},
        thread,
        time::Duration,
    };

    use super::*;
    use crate::ProductDevOperationKind;

    #[derive(Default)]
    struct FixtureRuntime;

    impl FixtureRuntime {
        fn operation(
            operation: crate::ProductDevOperationKind,
        ) -> ProductDevRuntimeReceipt<ProductDevOperationResult> {
            ProductDevRuntimeReceipt::new(
                ProductDevOperationResult::rejected(operation, "fixture").unwrap(),
                vec![crate::ProductDevRuntimeOutput::binding(
                    binding(),
                    CanonicalU64::new(0),
                )],
            )
            .unwrap()
        }
    }

    fn binding() -> crate::ProductDevRuntimeBinding {
        crate::ProductDevRuntimeBinding {
            instance_id: CanonicalU64::new(1),
            generation: CanonicalU64::new(1),
            control_revision: CanonicalU64::new(1),
        }
    }

    fn readout() -> crate::ProductDevRuntimeReadout {
        crate::ProductDevRuntimeReadout::new(
            binding(),
            crate::ProductDevRuntimeMode::Demand,
            crate::ProductDevRuntimeState::Running,
        )
    }

    impl ProductDevRuntime for FixtureRuntime {
        fn lifecycle(
            &mut self,
            operation: ProductDevLifecycleOperation,
        ) -> Result<ProductDevRuntimeReceipt<ProductDevOperationResult>, ProductDevRuntimeError>
        {
            Ok(Self::operation(operation.operation_kind()))
        }

        fn input(
            &mut self,
            batch: ProductDevInputBatch,
        ) -> Result<ProductDevRuntimeReceipt<crate::ProductDevInputResult>, ProductDevRuntimeError>
        {
            let accepted_through = batch
                .events()
                .last()
                .map(|event| CanonicalU64::new(event.sequence()));
            Ok(ProductDevRuntimeReceipt::new(
                crate::ProductDevInputResult::with_progress(
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
                vec![crate::ProductDevRuntimeOutput::binding(
                    binding(),
                    CanonicalU64::new(0),
                )],
            )
            .unwrap())
        }

        fn advance_realtime(
            &mut self,
            _observed_time_ns: CanonicalU64,
        ) -> Result<ProductDevRuntimeReceipt<ProductDevOperationResult>, ProductDevRuntimeError>
        {
            Ok(Self::operation(ProductDevOperationKind::AdvanceRealtime))
        }

        fn admit_demand_step(
            &mut self,
        ) -> Result<ProductDevRuntimeReceipt<ProductDevOperationResult>, ProductDevRuntimeError>
        {
            Ok(Self::operation(ProductDevOperationKind::AdmitDemandStep))
        }

        fn admit_external_step(
            &mut self,
            _step: CanonicalU64,
        ) -> Result<ProductDevRuntimeReceipt<ProductDevOperationResult>, ProductDevRuntimeError>
        {
            Ok(Self::operation(ProductDevOperationKind::AdmitExternalStep))
        }

        fn complete_timeline(
            &mut self,
            completion: ProductDevTimelineCompletion,
        ) -> Result<
            ProductDevRuntimeReceipt<ProductDevTimelineCompletionResult>,
            ProductDevRuntimeError,
        > {
            Ok(ProductDevRuntimeReceipt::new(
                ProductDevTimelineCompletionResult::rejected(
                    CanonicalU64::new(completion.envelope().ticket().value()),
                    "fixture",
                )
                .unwrap(),
                vec![crate::ProductDevRuntimeOutput::binding(
                    binding(),
                    CanonicalU64::new(0),
                )],
            )
            .unwrap())
        }
    }

    #[test]
    fn direct_and_json_operations_return_owner_receipts() {
        let session = ProductDevOperationOwner::new(FixtureRuntime);
        assert_eq!(
            session
                .lifecycle(ProductDevLifecycleOperation::Start)
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
        assert_eq!(session.admit_demand_step().unwrap().into_parts().1.len(), 1);
        assert_eq!(
            session
                .admit_external_step_json(br#""3""#)
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
        let session = ProductDevOperationOwner::new(FixtureRuntime);
        let input = session.input_json(br#"[] trailing"#).unwrap_err();
        assert_eq!(input.code(), "DEV_HOST_INPUT_DECODE");
        let time = session.advance_realtime_json(br#"01"#).unwrap_err();
        assert_eq!(time.code(), "DEV_HOST_CANONICAL_U64");
        let timeline = session.complete_timeline_json(br#"{}"#).unwrap_err();
        assert_eq!(timeline.code(), "DEV_HOST_TIMELINE_DECODE");
    }

    #[test]
    fn poisoned_runtime_lock_is_reported_without_dispatch() {
        let session = Arc::new(ProductDevOperationOwner::new(FixtureRuntime));
        let poisoned = Arc::clone(&session);
        let _ = thread::spawn(move || {
            let _guard = poisoned.lock_for_test();
            panic!("poison fixture lock");
        })
        .join();
        let error = session.admit_demand_step().unwrap_err();
        assert_eq!(error.code(), "DEV_HOST_RUNTIME_POISONED");
    }

    #[test]
    fn debug_execution_uses_the_same_runtime_serialization_lock() {
        let session = Arc::new(ProductDevOperationOwner::new(FixtureRuntime));
        let guard = session.lock_for_test();
        let (sent, received) = mpsc::channel();
        let blocked = Arc::clone(&session);
        let join = thread::spawn(move || {
            let result = blocked.execute_debug("fixture.count");
            sent.send(result.map(|_| ())).expect("send debug result");
        });
        assert!(
            received.recv_timeout(Duration::from_millis(25)).is_err(),
            "debug operation bypassed the lifecycle/update serialization lock"
        );
        drop(guard);
        let error = received
            .recv_timeout(Duration::from_secs(1))
            .expect("debug operation completed after lock release")
            .expect_err("fixture does not implement debug execution");
        assert_eq!(error.code(), "DEV_HOST_DEBUG_UNSUPPORTED");
        join.join().expect("debug worker");
    }

    #[test]
    fn scheduled_publication_stays_inside_owner_serialization() {
        let session = Arc::new(ProductDevOperationOwner::new(FixtureRuntime));
        let (published, published_ready) = mpsc::channel();
        let (release, release_publication) = mpsc::channel();
        let order = Arc::new(Mutex::new(Vec::new()));
        let input_order = Arc::clone(&order);
        let update_order = Arc::clone(&order);
        let scheduled_session = Arc::clone(&session);
        let scheduled = thread::spawn(move || {
            scheduled_session
                .advance_realtime_with_input_and_publish(
                    || (vec![ProductDevInputBatch::new(Vec::new())], false),
                    CanonicalU64::new(1),
                    |receipt| {
                        let _ = receipt;
                        input_order.lock().expect("input order lock").push("input");
                    },
                    |receipt| {
                        let _ = receipt;
                        update_order
                            .lock()
                            .expect("update order lock")
                            .push("advance");
                        published.send(()).expect("publication marker");
                        release_publication
                            .recv_timeout(Duration::from_secs(1))
                            .expect("publication release");
                    },
                    || {},
                    || {},
                )
                .expect("scheduled fixture advance");
        });
        published_ready
            .recv_timeout(Duration::from_secs(1))
            .expect("scheduled publication started");

        let competing_session = Arc::clone(&session);
        let (finished, competing_finished) = mpsc::channel();
        let competing = thread::spawn(move || {
            let result = competing_session.lifecycle(ProductDevLifecycleOperation::Start);
            finished.send(result).expect("competing result");
        });
        assert!(
            competing_finished
                .recv_timeout(Duration::from_millis(25))
                .is_err(),
            "a later runtime operation overtook scheduled output publication"
        );
        release.send(()).expect("release scheduled publication");
        scheduled.join().expect("scheduled worker");
        competing.join().expect("competing worker");
        assert_eq!(
            *order.lock().expect("final order lock"),
            vec!["input", "advance"],
            "runtime input receipts must publish before the scheduled advance receipt"
        );
    }
}
