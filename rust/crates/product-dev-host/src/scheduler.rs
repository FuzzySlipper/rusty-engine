//! Realtime scheduler sequencing for one Product development host session.
//!
//! The scheduler owns mailbox draining and publication order. The runtime
//! session remains neutral: it only supplies the serialized owner scope that
//! keeps this host policy atomic with every other runtime operation.

use crate::{
    session::{runtime_poisoned, ProductDevOperationOwner},
    CanonicalU64, ProductDevInputBatch, ProductDevInputResult, ProductDevOperationResult,
    ProductDevRuntime, ProductDevRuntimeError, ProductDevRuntimeReceipt,
    ProductDevUpdateAttribution,
};

/// Drains one host-mailbox snapshot through the runtime input owner immediately
/// before one realtime advance. Successful input receipts publish before the
/// update receipt; input admission errors remain recoverable observations while
/// the scheduled advance proceeds. All callbacks execute inside the same
/// runtime owner scope, preserving publication order with lifecycle and control
/// operations.
pub fn advance_realtime_with_input_and_publish<R, F, I, P, B, E>(
    owner: &ProductDevOperationOwner<R>,
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
    R: ProductDevRuntime,
    F: FnOnce() -> (Vec<ProductDevInputBatch>, bool),
    I: FnMut(ProductDevRuntimeReceipt<ProductDevInputResult>),
    P: FnMut(ProductDevRuntimeReceipt<ProductDevOperationResult>),
    B: FnOnce(),
    E: FnOnce(),
{
    owner
        .session()
        .with_locked_timed(
            begin,
            |runtime| {
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
                match result {
                    Ok(receipt) => {
                        publish(receipt);
                        Ok((input_errors, attribution))
                    }
                    Err(error) => Err(error),
                }
            },
            finish,
        )
        .map_err(|_| runtime_poisoned())?
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{mpsc, Arc, Mutex},
        thread,
        time::Duration,
    };

    use super::*;
    use crate::{
        ProductDevLifecycleOperation, ProductDevOperationKind, ProductDevRuntimeBinding,
        ProductDevRuntimeMode, ProductDevRuntimeReadout, ProductDevRuntimeState,
        ProductDevTimelineCompletion, ProductDevTimelineCompletionResult,
    };
    use runtime_input::RuntimeInputBinding;
    use runtime_lifecycle::{RuntimeControlRevision, RuntimeGeneration, RuntimeInstanceId};
    use runtime_publication::RuntimePublication;

    #[derive(Default)]
    struct FixtureRuntime;

    impl FixtureRuntime {
        fn binding() -> ProductDevRuntimeBinding {
            ProductDevRuntimeBinding {
                instance_id: CanonicalU64::new(1),
                generation: CanonicalU64::new(1),
                control_revision: CanonicalU64::new(1),
            }
        }

        fn readout() -> ProductDevRuntimeReadout {
            ProductDevRuntimeReadout::new(
                Self::binding(),
                ProductDevRuntimeMode::Demand,
                ProductDevRuntimeState::Running,
            )
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

        fn operation(
            operation: ProductDevOperationKind,
        ) -> ProductDevRuntimeReceipt<ProductDevOperationResult> {
            ProductDevRuntimeReceipt::new(
                ProductDevOperationResult::rejected(operation, "fixture").unwrap(),
                Self::publications(),
            )
            .unwrap()
        }
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
        ) -> Result<ProductDevRuntimeReceipt<ProductDevInputResult>, ProductDevRuntimeError>
        {
            let accepted_through = batch
                .events()
                .last()
                .map(|event| CanonicalU64::new(event.sequence()));
            Ok(ProductDevRuntimeReceipt::new(
                ProductDevInputResult::with_progress(
                    batch.events().len(),
                    batch.events().len(),
                    0,
                    accepted_through,
                    accepted_through,
                    CanonicalU64::new(2),
                    Self::binding(),
                    Self::readout(),
                )
                .unwrap(),
                Self::publications(),
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
                Self::publications(),
            )
            .unwrap())
        }
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
            advance_realtime_with_input_and_publish(
                &scheduled_session,
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
