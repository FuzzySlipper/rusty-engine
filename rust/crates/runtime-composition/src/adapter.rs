use render_model::RenderFrameDiff;
use render_presentation::PresentationFrameDiff;
use runtime_input::{InputFrame, RuntimeIntentEnvelope};
use runtime_lifecycle::{RuntimeLifecycle, RuntimePhaseToken, SimulationStep};
use runtime_mutation::{MutationAuthority, MutationBatch, MutationPlanner};
use runtime_schedule::ScheduleSystemInvocation;
use runtime_timeline::{TimelineOperationSpec, TimelineRelease};
use serde::Serialize;

/// Maximum UI streams a product can publish in one simulation step.
pub const MAX_PRODUCT_RUNTIME_UI_OUTPUTS: usize = 64;

/// Maximum product-owned timeline requests admitted at one Timeline boundary.
/// It is deliberately the same existing release-prefix bound so an adapter
/// cannot create an unbounded per-step staging surface.
pub const MAX_PRODUCT_RUNTIME_TIMELINE_REQUESTS: usize =
    runtime_timeline::MAX_TIMELINE_RELEASE_PREFIX;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductRuntimeOutputError {
    TooManyUiOutputs { received: usize, maximum: usize },
}

impl std::fmt::Display for ProductRuntimeOutputError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "product runtime output rejected: {self:?}")
    }
}

impl std::error::Error for ProductRuntimeOutputError {}

/// One optional typed UI output produced after the Projection phase.
///
/// The UI value is still product-owned typed data at this boundary. The root
/// serializes it once into the validated `runtime-ui` envelope. Render and
/// presentation frames are the existing Rust-owned validated projections.
#[derive(Debug, Clone, PartialEq)]
pub struct ProductRuntimeUi<Ui> {
    stream: String,
    contract: String,
    ui: Ui,
}

impl<Ui> ProductRuntimeUi<Ui> {
    pub fn new(stream: impl Into<String>, contract: impl Into<String>, ui: Ui) -> Self {
        Self {
            stream: stream.into(),
            contract: contract.into(),
            ui,
        }
    }

    pub fn stream(&self) -> &str {
        &self.stream
    }

    pub fn ui(&self) -> &Ui {
        &self.ui
    }

    pub fn into_parts(self) -> (String, String, Ui) {
        (self.stream, self.contract, self.ui)
    }
}

/// Typed product outputs produced after the Projection phase. UI is optional
/// because products may be simulation-only or may publish only renderer or
/// presentation diffs for a particular step.
#[derive(Debug, Clone, PartialEq)]
pub struct ProductRuntimeOutputs<Ui> {
    ui: Vec<ProductRuntimeUi<Ui>>,
    render: Option<RenderFrameDiff>,
    presentation: Option<PresentationFrameDiff>,
}

impl<Ui> ProductRuntimeOutputs<Ui> {
    pub fn new(
        ui: Vec<ProductRuntimeUi<Ui>>,
        render: Option<RenderFrameDiff>,
        presentation: Option<PresentationFrameDiff>,
    ) -> Result<Self, ProductRuntimeOutputError> {
        if ui.len() > MAX_PRODUCT_RUNTIME_UI_OUTPUTS {
            return Err(ProductRuntimeOutputError::TooManyUiOutputs {
                received: ui.len(),
                maximum: MAX_PRODUCT_RUNTIME_UI_OUTPUTS,
            });
        }
        Ok(Self {
            ui,
            render,
            presentation,
        })
    }

    pub fn ui(&self) -> &[ProductRuntimeUi<Ui>] {
        &self.ui
    }

    pub fn render(&self) -> Option<&RenderFrameDiff> {
        self.render.as_ref()
    }

    pub fn presentation(&self) -> Option<&PresentationFrameDiff> {
        self.presentation.as_ref()
    }

    pub fn into_parts(
        self,
    ) -> (
        Vec<ProductRuntimeUi<Ui>>,
        Option<RenderFrameDiff>,
        Option<PresentationFrameDiff>,
    ) {
        (self.ui, self.render, self.presentation)
    }
}

/// The closed downstream seam for one Product Runtime Composition.
///
/// A product implementation normally has generated, direct matches inside
/// these methods which call its `product_kernel_execution_facade!` functions.
/// This trait itself stores no owner table and performs no dynamic dispatch.
/// The associated authority/planner pair is deliberately exposed only as a
/// typed pair for the composition root's one mutation publication call.
pub trait ProductRuntimeAdapter {
    /// Product-owned live authoritative state used by `runtime-mutation`.
    type Authority: MutationAuthority<Guard = Self::Guard>;
    type Guard: Clone + Eq;
    /// Product-owned staging planner. Its error is the same closed adapter
    /// error used by all runtime hooks so lane failures retain their identity.
    type Planner: MutationPlanner<Self::Authority, Self::Evidence, Error = Self::Error>;
    type Evidence: Clone;
    type Error;
    type ScheduleOutput;
    type UiOutput: Serialize;

    /// Receives the one immutable input snapshot for the admitted step.
    fn on_input(
        &mut self,
        frame: &InputFrame,
        intents: &[RuntimeIntentEnvelope],
    ) -> Result<(), Self::Error>;

    /// Dispatches one due authored schedule system. The adapter owns the
    /// concrete Product Kernel snapshot/request construction and may use its
    /// generated static execution facade here.
    fn dispatch_schedule(
        &mut self,
        invocation: ScheduleSystemInvocation<'_>,
        lifecycle: &RuntimeLifecycle,
        token: RuntimePhaseToken,
    ) -> Result<Self::ScheduleOutput, Self::Error>;

    /// Returns inert, bounded timeline operation requests for the current
    /// admitted step. Runtime Composition validates and atomically enqueues
    /// them with the exact Timeline phase token before release; the adapter
    /// cannot reach into the timeline lane or install a callback.
    fn prepare_timeline(
        &mut self,
        _step: SimulationStep,
    ) -> Result<Vec<TimelineOperationSpec>, Self::Error> {
        Ok(Vec::new())
    }

    /// Receives immutable timeline releases before consequence/commit systems.
    fn on_timeline_releases(&mut self, releases: &TimelineRelease) -> Result<(), Self::Error>;

    /// Returns the one non-empty batch for this step, or `None` when the
    /// mutation phase must be explicitly accounted for as empty.
    fn prepare_mutation(
        &mut self,
        step: SimulationStep,
    ) -> Result<Option<MutationBatch>, Self::Error>;

    /// Returns the product-owned authority and planner. The root alone passes
    /// them to `RuntimeMutation`; the adapter never calls the lane itself.
    fn mutation_parts(&mut self) -> (&mut Self::Authority, &mut Self::Planner);

    /// Builds typed UI/render/presentation outputs after the projection token
    /// has been admitted. The root validates every returned frame and emits
    /// the UI value through the instance-owned UI lane.
    fn project(
        &mut self,
        lifecycle: &RuntimeLifecycle,
        token: RuntimePhaseToken,
    ) -> Result<ProductRuntimeOutputs<Self::UiOutput>, Self::Error>;

    /// Rebind product-owned state to a new lifecycle epoch. This hook is kept
    /// explicit even though the initial Demand path does not invoke it yet.
    fn rebind(&mut self, _lifecycle: &RuntimeLifecycle) -> Result<(), Self::Error> {
        Ok(())
    }

    /// Terminal product-owned cleanup hook.
    fn dispose(&mut self) {}
}
