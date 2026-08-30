use std::sync::Mutex;

#[cfg(test)]
use std::sync::MutexGuard;

use crate::{
    CanonicalU64, ProductDevDebugResult, ProductDevHostError, ProductDevInputBatch,
    ProductDevLifecycleOperation, ProductDevOperationResult, ProductDevRuntime,
    ProductDevRuntimeError, ProductDevRuntimeReceipt, ProductDevTimelineCompletion,
    ProductDevTimelineCompletionResult,
};

/// One serialized, transport-neutral session over a generated Product
/// Runtime. The session owns no output subscription, callbacks, registry, or
/// product state; each operation directly returns the runtime owner's bounded
/// result and output batch.
pub struct ProductDevOperationOwner<R> {
    runtime: Mutex<R>,
}

impl<R> ProductDevOperationOwner<R> {
    pub fn new(runtime: R) -> Self {
        Self {
            runtime: Mutex::new(runtime),
        }
    }
}

impl<R: ProductDevRuntime> ProductDevOperationOwner<R> {
    /// Runs one explicit lifecycle operation while holding the session's
    /// serialization guard for the complete owner call.
    pub fn lifecycle(
        &self,
        operation: ProductDevLifecycleOperation,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevOperationResult>, ProductDevRuntimeError> {
        self.with_runtime(|runtime| runtime.lifecycle(operation))
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

    /// Strictly admits a canonical JSON u64 and advances the realtime lane.
    pub fn advance_realtime_json(
        &self,
        bytes: &[u8],
    ) -> Result<ProductDevRuntimeReceipt<ProductDevOperationResult>, ProductDevRuntimeError> {
        let observed_time_ns = CanonicalU64::decode_json(bytes).map_err(host_error_to_runtime)?;
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
        let step = CanonicalU64::decode_json(bytes).map_err(host_error_to_runtime)?;
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

    fn with_runtime<T, F>(
        &self,
        call: F,
    ) -> Result<ProductDevRuntimeReceipt<T>, ProductDevRuntimeError>
    where
        F: FnOnce(&mut R) -> Result<ProductDevRuntimeReceipt<T>, ProductDevRuntimeError>,
    {
        let mut runtime = self.runtime.lock().map_err(|_| runtime_poisoned())?;
        call(&mut runtime)
    }

    #[cfg(test)]
    fn lock_for_test(&self) -> MutexGuard<'_, R> {
        self.runtime.lock().expect("fixture session lock")
    }
}

fn runtime_poisoned() -> ProductDevRuntimeError {
    ProductDevRuntimeError::new(
        "DEV_HOST_RUNTIME_POISONED",
        "runtime serialization lock is poisoned",
    )
    .expect("fixed runtime poison diagnostic is valid")
}

fn host_error_to_runtime(error: ProductDevHostError) -> ProductDevRuntimeError {
    ProductDevRuntimeError::new(error.code(), error.detail())
        .expect("bounded host error has a valid runtime diagnostic")
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{mpsc, Arc},
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
            Ok(ProductDevRuntimeReceipt::new(
                crate::ProductDevInputResult::accepted(batch.events().len(), binding(), readout())
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
}
