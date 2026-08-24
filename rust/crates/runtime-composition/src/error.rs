use std::fmt;

use runtime_input::RuntimeInputError;
use runtime_lifecycle::RuntimeLifecycleError;
use runtime_mutation::RuntimeMutationError;
use runtime_schedule::RuntimeScheduleError;
use runtime_timeline::RuntimeTimelineError;
use runtime_ui::RuntimeUiProjectionError;

/// Failure while binding the five lanes to an already-running lifecycle.
#[derive(Debug)]
pub enum RuntimeCompositionBindError {
    Lifecycle(RuntimeLifecycleError),
    LifecycleNotRunning,
    Schedule(RuntimeScheduleError),
    Timeline(RuntimeTimelineError),
    Mutation(RuntimeMutationError<()>),
    Ui(RuntimeUiProjectionError),
}

impl fmt::Display for RuntimeCompositionBindError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "runtime composition bind rejected: {self:?}")
    }
}

impl std::error::Error for RuntimeCompositionBindError {}

/// Failure while admitting or executing one composition step.
#[derive(Debug)]
pub enum RuntimeCompositionError<E> {
    NotStarted,
    Disposed,
    TerminalFailure,
    Lifecycle(RuntimeLifecycleError),
    Input(RuntimeInputError),
    Schedule(RuntimeScheduleError<E>),
    ScheduleStatic(RuntimeScheduleError),
    Timeline(RuntimeTimelineError),
    Mutation(RuntimeMutationError<E>),
    MutationStatic(RuntimeMutationError<()>),
    Ui(RuntimeUiProjectionError),
    Render(render_model::RenderFrameError),
    Presentation(render_presentation::PresentationFrameError),
    Adapter(E),
    UiEncoding(serde_json::Error),
}

impl<E: fmt::Debug> fmt::Display for RuntimeCompositionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "runtime composition rejected: {self:?}")
    }
}

impl<E: fmt::Debug + 'static> std::error::Error for RuntimeCompositionError<E> {}
