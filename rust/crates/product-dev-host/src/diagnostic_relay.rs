//! Bounded, nonblocking delivery of disposable-worker diagnostics to the shell.
//!
//! Worker socket readers must keep consuming frames even when the stable host's
//! diagnostic writer is slow. This relay retains a bounded FIFO of diagnostic
//! facts and turns any queue overflow into one visible shell-local diagnostic.

use std::{
    cell::Cell,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc::{self, RecvTimeoutError, SyncSender},
        Arc,
    },
    time::Duration,
};

use crate::{
    ProductDevLogDisposition, ProductDevLogSeverity, ProductDevWorkerDiagnostic,
    ProductDevWorkerDiagnosticField,
};

/// Match the stable host's default retained diagnostic history. This is an
/// event-count delivery bound, never a serialized-content-size policy.
pub const WORKER_DIAGNOSTIC_RELAY_CAPACITY: usize = 256;

/// Nonblocking worker-reader side of the shell diagnostic relay.
#[derive(Clone)]
pub struct ProductDevWorkerDiagnosticRelaySender {
    sender: SyncSender<ProductDevWorkerDiagnostic>,
    lost_events: Arc<AtomicU64>,
}

/// Host-writer side of the shell diagnostic relay.
pub struct ProductDevWorkerDiagnosticRelayReceiver {
    receiver: mpsc::Receiver<ProductDevWorkerDiagnostic>,
    last_was_loss: Cell<bool>,
    lost_events: Arc<AtomicU64>,
}

/// Creates the dedicated, bounded diagnostic relay for one disposable worker.
pub fn worker_diagnostic_relay() -> (
    ProductDevWorkerDiagnosticRelaySender,
    ProductDevWorkerDiagnosticRelayReceiver,
) {
    let (sender, receiver) = mpsc::sync_channel(WORKER_DIAGNOSTIC_RELAY_CAPACITY);
    let lost_events = Arc::new(AtomicU64::new(0));
    (
        ProductDevWorkerDiagnosticRelaySender {
            sender,
            lost_events: Arc::clone(&lost_events),
        },
        ProductDevWorkerDiagnosticRelayReceiver {
            receiver,
            last_was_loss: Cell::new(false),
            lost_events,
        },
    )
}

impl ProductDevWorkerDiagnosticRelaySender {
    /// Attempts delivery without ever stalling the worker socket reader.
    ///
    /// `false` means the event was not retained. A full relay records it for
    /// the receiver's explicit loss diagnostic. A disconnected relay has no
    /// receiver left to promise publication.
    pub fn try_send(&self, diagnostic: ProductDevWorkerDiagnostic) -> bool {
        match self.sender.try_send(diagnostic) {
            Ok(()) => true,
            Err(mpsc::TrySendError::Full(_)) => {
                self.lost_events.fetch_add(1, Ordering::Relaxed);
                false
            }
            Err(mpsc::TrySendError::Disconnected(_)) => false,
        }
    }
}

impl ProductDevWorkerDiagnosticRelayReceiver {
    /// Receives the next retained diagnostic or a shell-relay-scoped loss
    /// diagnostic. Alternate pending loss reports with retained events so
    /// sustained overload cannot starve either accounting or retained facts.
    /// The loss record carries only the relay's aggregate count; it makes no
    /// source event ordering or cursor claim.
    pub fn recv_timeout(
        &self,
        timeout: Duration,
    ) -> Result<ProductDevWorkerDiagnostic, RecvTimeoutError> {
        if !self.last_was_loss.get() {
            if let Some(loss) = self.take_loss() {
                return Ok(loss);
            }
        }
        match self.receiver.try_recv() {
            Ok(diagnostic) => {
                self.last_was_loss.set(false);
                return Ok(diagnostic);
            }
            Err(mpsc::TryRecvError::Disconnected) => return self.loss_or_disconnected(),
            Err(mpsc::TryRecvError::Empty) => {}
        }
        match self.receiver.recv_timeout(timeout) {
            Ok(diagnostic) => {
                self.last_was_loss.set(false);
                Ok(diagnostic)
            }
            Err(RecvTimeoutError::Timeout) => self.loss_or_timeout(),
            Err(RecvTimeoutError::Disconnected) => self.loss_or_disconnected(),
        }
    }

    fn loss_or_timeout(&self) -> Result<ProductDevWorkerDiagnostic, RecvTimeoutError> {
        self.take_loss().ok_or(RecvTimeoutError::Timeout)
    }

    fn loss_or_disconnected(&self) -> Result<ProductDevWorkerDiagnostic, RecvTimeoutError> {
        self.take_loss().ok_or(RecvTimeoutError::Disconnected)
    }

    fn take_loss(&self) -> Option<ProductDevWorkerDiagnostic> {
        let count = self.lost_events.swap(0, Ordering::AcqRel);
        if count == 0 {
            return None;
        }
        self.last_was_loss.set(true);
        Some(ProductDevWorkerDiagnostic {
            severity: ProductDevLogSeverity::Warning,
            disposition: ProductDevLogDisposition::Degraded,
            source: "shell-relay".to_owned(),
            code: "DEV_HOST_WORKER_DIAGNOSTIC_DROPPED".to_owned(),
            message: "shell diagnostic relay dropped worker diagnostic events after its bounded queue filled"
                .to_owned(),
            runtime: None,
            correlation: None,
            fields: vec![ProductDevWorkerDiagnosticField {
                key: "dropped-count".to_owned(),
                value: count.to_string(),
            }],
        })
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn diagnostic(index: usize) -> ProductDevWorkerDiagnostic {
        ProductDevWorkerDiagnostic {
            severity: ProductDevLogSeverity::Warning,
            disposition: ProductDevLogDisposition::Degraded,
            source: "worker".to_owned(),
            code: format!("WORKER_{index:03}"),
            message: "fixture".to_owned(),
            runtime: None,
            correlation: None,
            fields: Vec::new(),
        }
    }

    fn dropped_count(diagnostic: ProductDevWorkerDiagnostic) -> u64 {
        assert_eq!(diagnostic.source, "shell-relay");
        assert_eq!(diagnostic.code, "DEV_HOST_WORKER_DIAGNOSTIC_DROPPED");
        diagnostic
            .fields
            .iter()
            .find(|field| field.key == "dropped-count")
            .expect("loss diagnostic names the exact dropped count")
            .value
            .parse()
            .unwrap()
    }

    #[test]
    fn startup_page_queues_all_sixty_four_events_before_the_receiver_starts() {
        let (sender, receiver) = worker_diagnostic_relay();
        for index in 0..64 {
            assert!(sender.try_send(diagnostic(index)));
        }

        let delivered = (0..64)
            .map(|_| receiver.recv_timeout(Duration::ZERO).unwrap().code)
            .collect::<Vec<_>>();
        assert_eq!(
            delivered,
            (0..64)
                .map(|index| format!("WORKER_{index:03}"))
                .collect::<Vec<_>>()
        );
        assert!(matches!(
            receiver.recv_timeout(Duration::ZERO),
            Err(RecvTimeoutError::Timeout)
        ));
    }

    #[test]
    fn normal_page_preserves_all_sixty_four_events_in_fifo_order() {
        let (sender, receiver) = worker_diagnostic_relay();
        for index in 0..64 {
            assert!(sender.try_send(diagnostic(index)));
            assert_eq!(
                receiver.recv_timeout(Duration::ZERO).unwrap().code,
                format!("WORKER_{index:03}")
            );
        }
    }

    #[test]
    fn final_overflow_reports_its_exact_count_before_retained_fifo_events() {
        let (sender, receiver) = worker_diagnostic_relay();
        for index in 0..WORKER_DIAGNOSTIC_RELAY_CAPACITY {
            assert!(sender.try_send(diagnostic(index)));
        }
        for index in WORKER_DIAGNOSTIC_RELAY_CAPACITY..WORKER_DIAGNOSTIC_RELAY_CAPACITY + 3 {
            assert!(!sender.try_send(diagnostic(index)));
        }
        drop(sender);

        assert_eq!(
            dropped_count(receiver.recv_timeout(Duration::ZERO).unwrap()),
            3
        );
        let delivered = (0..WORKER_DIAGNOSTIC_RELAY_CAPACITY)
            .map(|_| receiver.recv_timeout(Duration::ZERO).unwrap().code)
            .collect::<Vec<_>>();
        assert_eq!(
            delivered,
            (0..WORKER_DIAGNOSTIC_RELAY_CAPACITY)
                .map(|index| format!("WORKER_{index:03}"))
                .collect::<Vec<_>>()
        );
        assert!(matches!(
            receiver.recv_timeout(Duration::ZERO),
            Err(RecvTimeoutError::Disconnected)
        ));
    }

    #[test]
    fn sustained_nonempty_relay_reports_each_overflow_without_reordering_retained_events() {
        let (sender, receiver) = worker_diagnostic_relay();
        for index in 0..WORKER_DIAGNOSTIC_RELAY_CAPACITY {
            assert!(sender.try_send(diagnostic(index)));
        }
        assert!(!sender.try_send(diagnostic(WORKER_DIAGNOSTIC_RELAY_CAPACITY)));
        assert_eq!(
            dropped_count(receiver.recv_timeout(Duration::ZERO).unwrap()),
            1
        );

        // Overflow continues while the retained queue is full. Retained facts
        // must also advance, instead of producing loss reports forever.
        assert!(!sender.try_send(diagnostic(WORKER_DIAGNOSTIC_RELAY_CAPACITY + 1)));
        assert_eq!(
            receiver.recv_timeout(Duration::ZERO).unwrap().code,
            "WORKER_000"
        );
        assert_eq!(
            dropped_count(receiver.recv_timeout(Duration::ZERO).unwrap()),
            1
        );

        let delivered = (1..WORKER_DIAGNOSTIC_RELAY_CAPACITY)
            .map(|_| receiver.recv_timeout(Duration::ZERO).unwrap().code)
            .collect::<Vec<_>>();
        assert_eq!(
            delivered,
            (1..WORKER_DIAGNOSTIC_RELAY_CAPACITY)
                .map(|index| format!("WORKER_{index:03}"))
                .collect::<Vec<_>>()
        );
    }
}
