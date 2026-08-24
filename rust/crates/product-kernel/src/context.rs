use std::fmt;

use runtime_lifecycle::{
    RuntimeLifecycle, RuntimeLifecycleError, RuntimePhase, RuntimePhaseToken, SimulationStep,
};

/// A schedule-phase context for one downstream closed system function.
/// Construction validates the exact lifecycle token and retains only the
/// immutable closed snapshot/request borrows supplied by the product.
#[derive(Debug, Clone, Copy)]
pub struct ProductSystemContext<'a, Snapshot, Request> {
    token: RuntimePhaseToken,
    snapshot: &'a Snapshot,
    request: &'a Request,
}

impl<'a, Snapshot, Request> ProductSystemContext<'a, Snapshot, Request> {
    pub fn new(
        lifecycle: &RuntimeLifecycle,
        token: RuntimePhaseToken,
        snapshot: &'a Snapshot,
        request: &'a Request,
    ) -> Result<Self, ProductKernelContextError> {
        validate_phase(lifecycle, token, RuntimePhase::Schedule)?;
        Ok(Self {
            token,
            snapshot,
            request,
        })
    }

    pub const fn token(self) -> RuntimePhaseToken {
        self.token
    }

    pub const fn step(self) -> SimulationStep {
        self.token.simulation().step()
    }

    pub fn snapshot(&self) -> &Snapshot {
        self.snapshot
    }

    pub fn request(&self) -> &Request {
        self.request
    }
}

/// A mutation-phase context for one downstream closed operation function.
/// It deliberately has a separate type from [`ProductSystemContext`] so a
/// system snapshot cannot be mistaken for an operation publication request.
#[derive(Debug, Clone, Copy)]
pub struct ProductOperationContext<'a, Snapshot, Request> {
    token: RuntimePhaseToken,
    snapshot: &'a Snapshot,
    request: &'a Request,
}

impl<'a, Snapshot, Request> ProductOperationContext<'a, Snapshot, Request> {
    pub fn new(
        lifecycle: &RuntimeLifecycle,
        token: RuntimePhaseToken,
        snapshot: &'a Snapshot,
        request: &'a Request,
    ) -> Result<Self, ProductKernelContextError> {
        validate_phase(lifecycle, token, RuntimePhase::Mutation)?;
        Ok(Self {
            token,
            snapshot,
            request,
        })
    }

    pub const fn token(self) -> RuntimePhaseToken {
        self.token
    }

    pub const fn step(self) -> SimulationStep {
        self.token.simulation().step()
    }

    pub fn snapshot(&self) -> &Snapshot {
        self.snapshot
    }

    pub fn request(&self) -> &Request {
        self.request
    }
}

fn validate_phase(
    lifecycle: &RuntimeLifecycle,
    token: RuntimePhaseToken,
    expected: RuntimePhase,
) -> Result<(), ProductKernelContextError> {
    if token.phase() != expected {
        return Err(ProductKernelContextError::WrongPhase {
            expected,
            received: token.phase(),
        });
    }
    lifecycle
        .validate_phase_token(token, expected)
        .map_err(ProductKernelContextError::Lifecycle)
}

/// Context construction failures. They are deliberately separate from
/// Product Model/linkage failures because context construction is a live
/// lifecycle-token boundary while assembly linking is pre-start admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductKernelContextError {
    WrongPhase {
        expected: RuntimePhase,
        received: RuntimePhase,
    },
    Lifecycle(RuntimeLifecycleError),
}

impl fmt::Display for ProductKernelContextError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "product kernel context rejected: {self:?}")
    }
}

impl std::error::Error for ProductKernelContextError {}
