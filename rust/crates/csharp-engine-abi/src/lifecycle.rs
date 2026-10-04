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
///
/// Once a product selects gameplay time, every realtime observation delivers
/// an update, and one that admits no step has `admitted_step_count` zero with
/// `simulation_step` the next step to be admitted. The `gameplay_*` fields are
/// the selection this observation was admitted under.
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
    pub gameplay_time_selected: bool,
    pub gameplay_rate: f64,
    pub gameplay_advance_remaining_steps: u32,
    /// Unscaled host seconds since the previous update, whatever the gameplay
    /// rate: for look, camera smoothing and other presentation that must not
    /// slow with the world. Zero after a new baseline (start, resume, restart,
    /// an inspection change) and for inspection's manual steps.
    pub host_elapsed_seconds: f64,
}

/// Selects how fast realtime gameplay simulation follows host time: 0 holds
/// the world, 1 is realtime, and anything between is slow motion. It takes
/// effect from the next host observation and ends any bounded advance.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NativeGameplayTimeRateRequest {
    pub rate: f64,
}

/// Runs the world for `seconds` of simulation time, rounded up to whole
/// fixed steps, at `rate` (above 0, at most 1), and then holds it.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NativeGameplayTimeAdvanceRequest {
    pub seconds: f64,
    pub rate: f64,
}

/// The gameplay time selected for the next host observation. `selected` is
/// false until the product's first selection in this generation; a held
/// world has `rate` 0. `advance_remaining_steps` counts the whole steps a
/// bounded advance still has to admit.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct NativeGameplayTimeReadout {
    pub selected: bool,
    pub rate: f64,
    pub advance_remaining_steps: u32,
    pub fixed_step_hz: u32,
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
