use std::collections::BTreeMap;

use runtime_lifecycle::{RuntimeLifecycle, RuntimePhaseToken, RuntimeState};
use serde_json::Value;

use crate::model::{
    validate_identity, validate_value, RuntimeUiProjectionEnvelope, RuntimeUiProjectionError,
    RuntimeUiProjectionReadout, RuntimeUiRuntimeBinding, MAX_RUNTIME_UI_PROJECTION_STREAMS,
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

/// One validated UI envelope that has not yet advanced its stream evidence.
///
/// This lets a product runtime stage its UI output beside other Engine-owned
/// projections before publishing the complete admitted turn.
#[derive(Debug)]
pub struct PreparedRuntimeUiProjection {
    token: RuntimePhaseToken,
    emission: PreparedEmission,
    envelope: RuntimeUiProjectionEnvelope,
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

    /// Publishes an already-owned projection value from an Engine runtime
    /// owner. This path has no product projection context:
    /// the caller supplies the exact lifecycle projection token directly.
    ///
    /// This keeps product runtimes independent of projection internals while
    /// retaining the same lifecycle, epoch, stream,
    /// sequence, identity, and bounded-value checks as typed projections.
    pub fn emit_value(
        &mut self,
        lifecycle: &RuntimeLifecycle,
        token: RuntimePhaseToken,
        stream: impl Into<String>,
        contract: impl Into<String>,
        value: Value,
    ) -> Result<RuntimeUiProjectionEnvelope, RuntimeUiProjectionError> {
        let prepared = self.prepare_value(lifecycle, token, stream, contract, value)?;
        self.commit_prepared(lifecycle, prepared)
    }

    /// Validates an Engine-owned projection value without advancing stream
    /// sequence evidence or publishing an envelope.
    pub fn prepare_value(
        &self,
        lifecycle: &RuntimeLifecycle,
        token: RuntimePhaseToken,
        stream: impl Into<String>,
        contract: impl Into<String>,
        value: Value,
    ) -> Result<PreparedRuntimeUiProjection, RuntimeUiProjectionError> {
        let prepared = self.prepare_token(lifecycle, token, stream.into(), contract.into())?;
        validate_value(&value)?;
        let envelope = RuntimeUiProjectionEnvelope::new(
            prepared.runtime,
            prepared.sequence,
            prepared.stream.clone(),
            prepared.contract.clone(),
            value,
        )?;
        Ok(PreparedRuntimeUiProjection {
            token,
            emission: prepared,
            envelope,
        })
    }

    /// Advances stream evidence for a previously validated envelope.
    pub fn commit_prepared(
        &mut self,
        lifecycle: &RuntimeLifecycle,
        prepared: PreparedRuntimeUiProjection,
    ) -> Result<RuntimeUiProjectionEnvelope, RuntimeUiProjectionError> {
        let current = self.prepare_token(
            lifecycle,
            prepared.token,
            prepared.emission.stream.clone(),
            prepared.emission.contract.clone(),
        )?;
        if current.sequence != prepared.emission.sequence
            || current.runtime != prepared.emission.runtime
        {
            return Err(RuntimeUiProjectionError::LifecycleBindingChanged);
        }
        self.last_sequence_by_stream.insert(
            prepared.envelope.stream().to_owned(),
            prepared.envelope.sequence(),
        );
        self.last_contract_by_stream.insert(
            prepared.envelope.stream().to_owned(),
            prepared.envelope.contract().to_owned(),
        );
        Ok(prepared.envelope)
    }

    fn prepare_token(
        &self,
        lifecycle: &RuntimeLifecycle,
        token: RuntimePhaseToken,
        stream: String,
        contract: String,
    ) -> Result<PreparedEmission, RuntimeUiProjectionError> {
        let runtime = RuntimeUiRuntimeBinding::from(lifecycle);
        if self.disposed {
            return Err(RuntimeUiProjectionError::Disposed);
        }
        if token.phase() != runtime_lifecycle::RuntimePhase::Projection {
            return Err(RuntimeUiProjectionError::WrongPhase {
                expected: runtime_lifecycle::RuntimePhase::Projection,
                received: token.phase(),
            });
        }
        lifecycle
            .validate_phase_token(token, runtime_lifecycle::RuntimePhase::Projection)
            .map_err(RuntimeUiProjectionError::Lifecycle)?;
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
        let sequence = token.simulation().step().value();
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
}

#[derive(Debug)]
struct PreparedEmission {
    runtime: RuntimeUiRuntimeBinding,
    sequence: u64,
    stream: String,
    contract: String,
}
