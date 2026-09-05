use runtime_session::RuntimeSession;

use crate::{
    CanonicalU64, ProductDevDebugResult, ProductDevHostError, ProductDevInputBatch,
    ProductDevLifecycleOperation, ProductDevOperationResult, ProductDevRuntime,
    ProductDevRuntimeBinding, ProductDevRuntimeError, ProductDevRuntimeReceipt,
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

    /// Exposes the host-neutral serialized scope for host-owned work that must
    /// remain ordered with a direct product operation.
    pub(crate) fn session(&self) -> &RuntimeSession<R> {
        &self.session
    }
}

impl<R: ProductDevRuntime> ProductDevOperationOwner<R> {
    /// Drains call-local attribution while the outer publisher holds its
    /// operation/publication order (used by the disposable worker adapter).
    pub fn take_update_attribution(
        &self,
    ) -> Result<Option<ProductDevUpdateAttribution>, ProductDevRuntimeError> {
        self.session
            .with_locked(|runtime| runtime.take_update_attribution())
            .map_err(|_| runtime_poisoned())
    }

    /// Reads the runtime's explicit scheduler posture while holding the same
    /// serialization guard as every mutating operation.
    pub fn realtime_schedule_state(
        &self,
    ) -> Result<ProductDevRuntimeScheduleState, ProductDevRuntimeError> {
        self.session
            .with_locked(|runtime| runtime.realtime_schedule_state())
            .map_err(|_| runtime_poisoned())
    }

    /// Reads the runtime's admitted realtime observation interval under the
    /// same owner lock used by lifecycle and update operations.
    pub fn realtime_schedule_interval(
        &self,
    ) -> Result<Option<std::time::Duration>, ProductDevRuntimeError> {
        self.session
            .with_locked(|runtime| runtime.realtime_schedule_interval())
            .map_err(|_| runtime_poisoned())
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
        self.session
            .with_locked(call)
            .map_err(|_| runtime_poisoned())
            .and_then(|result| result)
    }
}

pub(crate) fn runtime_poisoned() -> ProductDevRuntimeError {
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
    use super::*;
    use crate::ProductDevOperationKind;
    use runtime_input::RuntimeInputBinding;
    use runtime_lifecycle::{RuntimeControlRevision, RuntimeGeneration, RuntimeInstanceId};
    use runtime_publication::RuntimePublication;

    #[derive(Default)]
    struct FixtureRuntime;

    impl FixtureRuntime {
        fn operation(
            operation: crate::ProductDevOperationKind,
        ) -> ProductDevRuntimeReceipt<ProductDevOperationResult> {
            ProductDevRuntimeReceipt::new(
                ProductDevOperationResult::rejected(operation, "fixture").unwrap(),
                publications(),
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
                publications(),
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
                publications(),
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
}
