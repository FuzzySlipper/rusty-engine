use std::collections::BTreeMap;

use product_kernel::ProductProjectionContext;
use runtime_lifecycle::{RuntimeLifecycle, RuntimePhaseToken, RuntimeState};
use serde::Serialize;
use serde_json::Value;

use crate::model::{
    bound_error, map_context_error, validate_identity, validate_value, RuntimeUiProjectionEnvelope,
    RuntimeUiProjectionError, RuntimeUiProjectionReadout, RuntimeUiRuntimeBinding,
    MAX_RUNTIME_UI_PROJECTION_STREAMS,
};

/// One explicit UI projection lane bound to one lifecycle epoch.
///
/// The lane retains only per-stream sequence evidence. It stores no DTO,
/// product snapshot, callback, host object, scheduler, clock, or renderer.
/// Rebinding clears all sequence progress for the new lifecycle epoch.
#[derive(Debug)]
pub struct RuntimeUiProjection {
    runtime: RuntimeUiRuntimeBinding,
    last_sequence_by_stream: BTreeMap<String, u64>,
    last_contract_by_stream: BTreeMap<String, String>,
    disposed: bool,
}

impl RuntimeUiProjection {
    /// Binds a fresh lane to a running lifecycle. A host/product may bind this
    /// lane at any point in a running generation; projection tokens still have
    /// to be admitted and validated by the lifecycle for every emission.
    pub fn bind(lifecycle: &RuntimeLifecycle) -> Result<Self, RuntimeUiProjectionError> {
        if lifecycle.state() != RuntimeState::Running {
            return Err(RuntimeUiProjectionError::LifecycleNotRunning {
                state: lifecycle.state(),
            });
        }
        Ok(Self {
            runtime: lifecycle.into(),
            last_sequence_by_stream: BTreeMap::new(),
            last_contract_by_stream: BTreeMap::new(),
            disposed: false,
        })
    }

    pub fn runtime(&self) -> RuntimeUiRuntimeBinding {
        self.runtime
    }

    pub fn readout(&self) -> RuntimeUiProjectionReadout {
        RuntimeUiProjectionReadout {
            runtime: self.runtime,
            stream_count: self.last_sequence_by_stream.len(),
            disposed: self.disposed,
        }
    }

    /// Terminally disposes the lane and drops all per-stream evidence.
    pub fn dispose(&mut self) {
        self.last_sequence_by_stream.clear();
        self.last_contract_by_stream.clear();
        self.disposed = true;
    }

    /// Rebinds this lane to the current running lifecycle epoch. Rebinding is
    /// explicit even when only the control revision changed, and always clears
    /// per-epoch stream sequence progress.
    pub fn rebind(&mut self, lifecycle: &RuntimeLifecycle) -> Result<(), RuntimeUiProjectionError> {
        if self.disposed {
            return Err(RuntimeUiProjectionError::Disposed);
        }
        if lifecycle.instance_id() != self.runtime.instance_id() {
            return Err(RuntimeUiProjectionError::RebindForeignInstance {
                expected: self.runtime.instance_id(),
                received: lifecycle.instance_id(),
            });
        }
        if lifecycle.state() != RuntimeState::Running {
            return Err(RuntimeUiProjectionError::RebindNotRunning {
                state: lifecycle.state(),
            });
        }
        let next_runtime = RuntimeUiRuntimeBinding::from(lifecycle);
        if next_runtime == self.runtime {
            return Ok(());
        }
        if next_runtime.generation() < self.runtime.generation()
            || next_runtime.control_revision() <= self.runtime.control_revision()
        {
            return Err(RuntimeUiProjectionError::RebindRegression {
                expected: self.runtime,
                received: next_runtime,
            });
        }
        self.runtime = next_runtime;
        self.last_sequence_by_stream.clear();
        self.last_contract_by_stream.clear();
        Ok(())
    }

    /// Runs a typed downstream projection function with a validated projection
    /// context, then emits its owned DTO. All lane/stream/sequence validation
    /// occurs before the function is invoked, so rejected emissions cannot
    /// execute downstream projection code.
    pub fn project<Snapshot, D, F>(
        &mut self,
        lifecycle: &RuntimeLifecycle,
        token: RuntimePhaseToken,
        stream: impl Into<String>,
        contract: impl Into<String>,
        snapshot: &Snapshot,
        projector: F,
    ) -> Result<RuntimeUiProjectionEnvelope, RuntimeUiProjectionError>
    where
        F: FnOnce(ProductProjectionContext<'_, Snapshot>) -> D,
        D: Serialize,
    {
        let context =
            ProductProjectionContext::new(lifecycle, token, snapshot).map_err(map_context_error)?;
        let prepared = self.prepare(lifecycle, context, stream.into(), contract.into())?;
        let value = serde_json::to_value(projector(context))
            .map_err(|error| RuntimeUiProjectionError::ValueEncoding(error.to_string()))?;
        self.finish(lifecycle, context, prepared, value)
    }

    /// Variant of [`Self::project`] for a typed downstream function that may
    /// report its own product-owned error. The error is copied only into a
    /// bounded diagnostic and is never treated as a runtime authority.
    pub fn project_result<Snapshot, D, E, F>(
        &mut self,
        lifecycle: &RuntimeLifecycle,
        token: RuntimePhaseToken,
        stream: impl Into<String>,
        contract: impl Into<String>,
        snapshot: &Snapshot,
        projector: F,
    ) -> Result<RuntimeUiProjectionEnvelope, RuntimeUiProjectionError>
    where
        F: FnOnce(ProductProjectionContext<'_, Snapshot>) -> Result<D, E>,
        D: Serialize,
        E: std::fmt::Display,
    {
        let context =
            ProductProjectionContext::new(lifecycle, token, snapshot).map_err(map_context_error)?;
        let prepared = self.prepare(lifecycle, context, stream.into(), contract.into())?;
        let dto = projector(context)
            .map_err(|error| RuntimeUiProjectionError::ProjectionFailed(bound_error(error)))?;
        let value = serde_json::to_value(dto)
            .map_err(|error| RuntimeUiProjectionError::ValueEncoding(error.to_string()))?;
        self.finish(lifecycle, context, prepared, value)
    }

    /// Emits an already-owned DTO produced from a Product Projection Context.
    /// The context token is revalidated against the live lifecycle before any
    /// value is copied or retained.
    pub fn emit<Snapshot, D>(
        &mut self,
        lifecycle: &RuntimeLifecycle,
        context: ProductProjectionContext<'_, Snapshot>,
        stream: impl Into<String>,
        contract: impl Into<String>,
        dto: D,
    ) -> Result<RuntimeUiProjectionEnvelope, RuntimeUiProjectionError>
    where
        D: Serialize,
    {
        let prepared = self.prepare(lifecycle, context, stream.into(), contract.into())?;
        let value = serde_json::to_value(dto)
            .map_err(|error| RuntimeUiProjectionError::ValueEncoding(error.to_string()))?;
        self.finish(lifecycle, context, prepared, value)
    }

    fn prepare<Snapshot>(
        &self,
        lifecycle: &RuntimeLifecycle,
        context: ProductProjectionContext<'_, Snapshot>,
        stream: String,
        contract: String,
    ) -> Result<PreparedEmission, RuntimeUiProjectionError> {
        if self.disposed {
            return Err(RuntimeUiProjectionError::Disposed);
        }
        let token = context.token();
        let context = ProductProjectionContext::new(lifecycle, token, context.snapshot())
            .map_err(map_context_error)?;
        let runtime = RuntimeUiRuntimeBinding::from(lifecycle);
        if runtime != self.runtime {
            return Err(RuntimeUiProjectionError::RebindRequired {
                expected: self.runtime,
                received: runtime,
            });
        }
        let stream = validate_identity("stream", stream)?;
        let contract = validate_identity("contract", contract)?;
        if let Some(previous_contract) = self.last_contract_by_stream.get(&stream) {
            if previous_contract != &contract {
                return Err(RuntimeUiProjectionError::ContractChanged {
                    stream,
                    previous: previous_contract.clone(),
                    received: contract,
                });
            }
        }
        let sequence = context.step().value();
        match self.last_sequence_by_stream.get(&stream) {
            Some(previous) if sequence == *previous => {
                return Err(RuntimeUiProjectionError::DuplicateSequence { stream, sequence });
            }
            Some(previous) if sequence < *previous => {
                return Err(RuntimeUiProjectionError::SequenceRegression {
                    stream,
                    previous: *previous,
                    received: sequence,
                });
            }
            _ => {}
        }
        if !self.last_sequence_by_stream.contains_key(&stream)
            && self.last_sequence_by_stream.len() >= MAX_RUNTIME_UI_PROJECTION_STREAMS
        {
            return Err(RuntimeUiProjectionError::StreamLimit {
                maximum: MAX_RUNTIME_UI_PROJECTION_STREAMS,
            });
        }
        Ok(PreparedEmission {
            runtime,
            sequence,
            stream,
            contract,
        })
    }

    fn finish<Snapshot>(
        &mut self,
        lifecycle: &RuntimeLifecycle,
        context: ProductProjectionContext<'_, Snapshot>,
        prepared: PreparedEmission,
        value: Value,
    ) -> Result<RuntimeUiProjectionEnvelope, RuntimeUiProjectionError> {
        // A product closure cannot normally mutate the lifecycle through this
        // API, but revalidation makes the publication boundary explicit if it
        // captures other mutable state or if the API is composed in a larger
        // owner.
        let current = self.prepare(
            lifecycle,
            context,
            prepared.stream.clone(),
            prepared.contract.clone(),
        )?;
        if current.sequence != prepared.sequence || current.runtime != prepared.runtime {
            return Err(RuntimeUiProjectionError::LifecycleBindingChanged);
        }
        validate_value(&value)?;
        let envelope = RuntimeUiProjectionEnvelope::new(
            prepared.runtime,
            prepared.sequence,
            prepared.stream,
            prepared.contract,
            value,
        )?;
        self.last_sequence_by_stream
            .insert(envelope.stream().to_owned(), envelope.sequence());
        self.last_contract_by_stream
            .insert(envelope.stream().to_owned(), envelope.contract().to_owned());
        Ok(envelope)
    }
}

#[derive(Debug)]
struct PreparedEmission {
    runtime: RuntimeUiRuntimeBinding,
    sequence: u64,
    stream: String,
    contract: String,
}
