//! Realtime scheduler sequencing for one product host session.
//!
//! The scheduler owns mailbox draining and publication order. The runtime
//! session remains neutral: it only supplies the serialized owner scope that
//! keeps this host policy atomic with every other runtime operation.

use crate::{
    session::{runtime_poisoned, ProductHostOperationOwner},
    CanonicalU64, ProductHostInputBatch, ProductHostInputResult, ProductHostOperationResult,
    ProductHostRuntime, ProductHostRuntimeError, ProductHostRuntimeReceipt,
    ProductHostUpdateAttribution,
};

/// Drains one host-mailbox snapshot through the runtime input owner immediately
/// before one realtime advance. Successful input receipts publish before the
/// update receipt; input admission errors remain recoverable observations while
/// the scheduled advance proceeds. All callbacks execute inside the same
/// runtime owner scope, preserving publication order with lifecycle and control
/// operations.
pub fn advance_realtime_with_input_and_publish<R, F, I, P, B, E>(
    owner: &ProductHostOperationOwner<R>,
    drain: F,
    observed_time_ns: CanonicalU64,
    mut publish_input: I,
    mut publish: P,
    begin: B,
    finish: E,
) -> Result<
    (
        Vec<ProductHostRuntimeError>,
        Option<ProductHostUpdateAttribution>,
    ),
    ProductHostRuntimeError,
>
where
    R: ProductHostRuntime,
    F: FnOnce() -> (Vec<ProductHostInputBatch>, bool),
    I: FnMut(ProductHostRuntimeReceipt<ProductHostInputResult>),
    P: FnMut(ProductHostRuntimeReceipt<ProductHostOperationResult>),
    B: FnOnce(),
    E: FnOnce(),
{
    owner
        .session()
        .with_locked_timed(
            begin,
            |runtime| {
                let input_errors =
                    deliver_queued_input(runtime, drain(), &mut publish_input, &mut publish);
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

/// Hands one host-mailbox snapshot to the runtime input owner in arrival
/// order, inside the caller's runtime owner scope. A realtime advance does this
/// first, and so does a debug command: held playtest time advances only through
/// debug commands, so input accepted before one must reach the steps it runs.
pub(crate) fn deliver_queued_input<R, I, P>(
    runtime: &mut R,
    (batches, overflowed): (Vec<ProductHostInputBatch>, bool),
    publish_input: &mut I,
    publish: &mut P,
) -> Vec<ProductHostRuntimeError>
where
    R: ProductHostRuntime,
    I: FnMut(ProductHostRuntimeReceipt<ProductHostInputResult>),
    P: FnMut(ProductHostRuntimeReceipt<ProductHostOperationResult>),
{
    let mut input_errors = Vec::new();
    if overflowed {
        let result = runtime.recover_input_overflow();
        match result {
            Ok(receipt) => publish(receipt),
            Err(error) => input_errors.push(error),
        }
    }
    for batch in batches {
        let result = runtime.input(batch);
        match result {
            Ok(receipt) => publish_input(receipt),
            Err(error) => input_errors.push(error),
        }
    }
    input_errors
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{mpsc, Arc, Mutex},
        thread,
        time::Duration,
    };

    use super::*;
    use crate::publication::RuntimePublication;
    use crate::{
        ProductHostLifecycleOperation, ProductHostOperationKind, ProductHostRuntimeBinding,
        ProductHostRuntimeReadout, ProductHostRuntimeState, ProductHostTimelineCompletion,
        ProductHostTimelineCompletionResult,
    };
    use runtime_input::RuntimeInputBinding;
    use runtime_lifecycle::{RuntimeControlRevision, RuntimeGeneration, RuntimeInstanceId};

    struct FixtureRuntime;

    impl FixtureRuntime {
        fn binding() -> ProductHostRuntimeBinding {
            ProductHostRuntimeBinding {
                instance_id: CanonicalU64::new(1),
                generation: CanonicalU64::new(1),
                control_revision: CanonicalU64::new(1),
            }
        }

        fn readout() -> ProductHostRuntimeReadout {
            ProductHostRuntimeReadout::new(Self::binding(), ProductHostRuntimeState::Running)
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
            operation: ProductHostOperationKind,
        ) -> ProductHostRuntimeReceipt<ProductHostOperationResult> {
            ProductHostRuntimeReceipt::new(
                ProductHostOperationResult::rejected(operation, "fixture").unwrap(),
                Self::publications(),
            )
            .unwrap()
        }
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
        ) -> Result<ProductHostRuntimeReceipt<ProductHostInputResult>, ProductHostRuntimeError>
        {
            let accepted_through = batch
                .events()
                .last()
                .map(|event| CanonicalU64::new(event.sequence()));
            Ok(ProductHostRuntimeReceipt::new(
                ProductHostInputResult::with_progress(
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
                Self::publications(),
            )
            .unwrap())
        }
    }

    #[test]
    fn scheduled_publication_stays_inside_owner_serialization() {
        let session = Arc::new(ProductHostOperationOwner::new(FixtureRuntime));
        let (published, published_ready) = mpsc::channel();
        let (release, release_publication) = mpsc::channel();
        let order = Arc::new(Mutex::new(Vec::new()));
        let input_order = Arc::clone(&order);
        let update_order = Arc::clone(&order);
        let scheduled_session = Arc::clone(&session);
        let scheduled = thread::spawn(move || {
            advance_realtime_with_input_and_publish(
                &scheduled_session,
                || (vec![ProductHostInputBatch::new(Vec::new())], false),
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
            let result = competing_session.lifecycle(ProductHostLifecycleOperation::Start);
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
