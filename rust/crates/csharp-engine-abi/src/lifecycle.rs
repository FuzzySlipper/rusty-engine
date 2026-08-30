use crate::NativeInputEvent;

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeProductUpdateMode {
    Realtime = 1,
    Demand = 2,
    External = 3,
}

/// One typed result a trusted product may return after an admitted update.
///
/// The result is copied across the product callback boundary and applied by
/// the Rust runtime only after the completed update has been staged and
/// committed. It is deliberately not a general command or event channel.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeProductUpdateResult {
    None = 0,
    ReportFault = 1,
}

/// Lifecycle state accompanying a product update.
///
/// This is a snapshot of the Rust-owned lifecycle at the point an update was
/// admitted. In particular, `Paused` remains an explicit state even though a
/// paused lifecycle does not admit product updates.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeProductLifecycleState {
    Created = 1,
    Running = 2,
    Paused = 3,
    Faulted = 4,
    Shutdown = 5,
}

/// Typed facts for one Rust-admitted product update.
///
/// The lifecycle remains the sole host clock and simulation-admission owner.
/// Realtime observations carry host monotonic nanoseconds and fixed-step
/// facts; demand/external updates carry zero for fields that do not apply.
/// `simulation_step` is the first step in the admitted batch and
/// `admitted_step_count` describes the complete batch. Dropped steps are the
/// whole steps dropped from this realtime observation, not a product-owned
/// counter or scheduling command.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NativeProductUpdateFacts {
    pub mode: NativeProductUpdateMode,
    pub lifecycle_state: NativeProductLifecycleState,
    pub generation: u64,
    pub control_revision: u64,
    pub observed_host_time_nanoseconds: u64,
    pub simulation_step: u64,
    pub fixed_step_hz: u32,
    pub admitted_step_count: u32,
    pub dropped_step_count: u64,
    pub fixed_delta_seconds: f64,
}

/// Rust-owned lifecycle facts published only after a host transition commits.
///
/// Unlike update facts, this snapshot also covers host-only operations such as
/// fault reporting and control replacement. The managed product copies it and
/// must not retain the borrowed pointer supplied to its callback.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeProductRuntimeFacts {
    pub lifecycle_state: NativeProductLifecycleState,
    pub instance_id: u64,
    pub generation: u64,
    pub control_revision: u64,
}

/// Explicit typed update facts and its borrowed input slice.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeProductUpdateArgs {
    pub facts: NativeProductUpdateFacts,
    pub events: *const NativeInputEvent,
    pub event_count: usize,
}
