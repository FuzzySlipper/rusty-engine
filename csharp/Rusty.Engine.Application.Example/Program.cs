using Rusty.Engine;
using Rusty.Engine.Application;

var engine = new ExampleEngineContext();
var scheduler = new SimulationScheduler();
var defaultOrder = new List<string>();
var defaultPipeline = new UpdatePipeline(engine);

Register(defaultPipeline, UpdatePhase.Input, "input", defaultOrder, engine);
Register(defaultPipeline, UpdatePhase.Update, "update-one", defaultOrder, engine);
Register(defaultPipeline, UpdatePhase.Update, "update-two", defaultOrder, engine);
Register(defaultPipeline, UpdatePhase.LateUpdate, "late-update", defaultOrder, engine);
Register(defaultPipeline, UpdatePhase.Presentation, "presentation", defaultOrder, engine);
defaultPipeline.Run(UpdateAt(1, 1));

var schedulerPipeline = new UpdatePipeline(engine);
scheduler.Attach(schedulerPipeline, UpdatePhase.Update);
var scheduledOrder = new List<string>();
var scheduledSteps = new List<ulong>();
var cancelled = scheduler.ScheduleAt(2, _ => scheduledOrder.Add("cancelled"));
scheduler.ScheduleAt(2, context =>
{
    scheduledOrder.Add("one");
    scheduledSteps.Add(context.SimulationStep);
});
var repeating = scheduler.ScheduleRepeatingAt(2, 2, context =>
{
    scheduledOrder.Add("repeat");
    scheduledSteps.Add(context.SimulationStep);
});
Require(scheduler.Cancel(cancelled), "the pending callback did not cancel");

schedulerPipeline.Run(UpdateAt(1, 1));
schedulerPipeline.Run(UpdateAt(2, 2));
Require(scheduler.RescheduleAfter(repeating, 1), "the repeating callback did not reschedule");
schedulerPipeline.Run(UpdateAt(4, 2));
scheduler.ScheduleAt(10, _ => scheduledOrder.Add("stale"));
schedulerPipeline.Run(UpdateAt(10, 1, generation: 2));

Require(string.Join(',', defaultOrder) == "input,update-one,update-two,late-update,presentation", "the default pass order is not deterministic");
Require(string.Join(',', scheduledOrder) == "one,repeat,repeat", "the scheduler did not preserve admitted-step order");
Require(scheduledSteps.SequenceEqual([2UL, 2UL, 5UL]), "the scheduler did not report the exact dispatched batch steps");
Require(scheduler.Readout == new SchedulerReadout(0, 1, 3), "the scheduler readout was not bounded and accurate");

var beforeFirstStep = new SimulationScheduler();
var relativeSchedulingRejected = false;
try
{
    beforeFirstStep.WaitSteps(0, _ => { });
}
catch (InvalidOperationException exception) when (exception.Message.StartsWith("Relative scheduling needs an Engine-admitted simulation step."))
{
    relativeSchedulingRejected = true;
}

Require(relativeSchedulingRejected, "relative continuation scheduling before the first admitted step did not fail");

var continuations = new SimulationScheduler();
continuations.Advance(UpdateAt(10, 1));
var continuationOrder = new List<string>();
var conditionReady = false;
var conditionChecks = 0;
continuations.ResumeNextStep(context => continuationOrder.Add($"resume-{context.SimulationStep}"));
continuations.WaitSteps(0, context => continuationOrder.Add($"zero-{context.SimulationStep}"));
continuations.WaitSteps(2, context =>
{
    conditionReady = true;
    continuationOrder.Add($"fixed-{context.SimulationStep}");
});
continuations.WaitUntil(
    () =>
    {
        conditionChecks++;
        return conditionReady;
    },
    context => continuationOrder.Add($"condition-{context.SimulationStep}"));
continuations.Advance(UpdateAt(11, 3));

Require(string.Join(',', continuationOrder) == "resume-11,zero-11,fixed-13,condition-13", "the continuation helpers did not follow admitted steps");
Require(conditionChecks == 3, "the false completion condition did not retry once per admitted step");
Require(continuations.Readout == new SchedulerReadout(0, 4, 0), "the completed continuations were not released");

var dispatchMutation = new SimulationScheduler();
dispatchMutation.Advance(UpdateAt(15, 1));
var dispatchOrder = new List<string>();
ScheduledWorkHandle cancelledDuringDispatch = default;
dispatchMutation.ResumeNextStep(_ =>
{
    dispatchOrder.Add("first");
    Require(dispatchMutation.Cancel(cancelledDuringDispatch), "a callback did not cancel pending work during dispatch");
    dispatchMutation.ResumeNextStep(_ => dispatchOrder.Add("next"));
});
cancelledDuringDispatch = dispatchMutation.ResumeNextStep(_ => dispatchOrder.Add("cancelled"));
dispatchMutation.Advance(UpdateAt(16, 1));
dispatchMutation.Advance(UpdateAt(17, 1));
Require(string.Join(',', dispatchOrder) == "first,next", "dispatch-time scheduling or cancellation was not revision safe");

var throwingContinuation = new SimulationScheduler();
throwingContinuation.Advance(UpdateAt(20, 1));
throwingContinuation.WaitUntil(
    () => throw new InvalidOperationException("expected condition failure"),
    _ => throw new InvalidOperationException("a throwing condition must not resume"));
var conditionFailurePropagated = false;
try
{
    throwingContinuation.Advance(UpdateAt(21, 1));
}
catch (InvalidOperationException exception) when (exception.Message == "expected condition failure")
{
    conditionFailurePropagated = true;
}

Require(conditionFailurePropagated, "a condition failure did not propagate");
Require(throwingContinuation.Readout == new SchedulerReadout(1, 0, 0), "a throwing condition did not remain pending");

var staleContinuation = new SimulationScheduler();
staleContinuation.Advance(UpdateAt(30, 1));
staleContinuation.WaitUntil(() => false, _ => throw new InvalidOperationException("a stale continuation resumed"));
staleContinuation.Advance(UpdateAt(31, 1));
staleContinuation.Advance(UpdateAt(32, 1, generation: 2));
Require(staleContinuation.Readout == new SchedulerReadout(0, 0, 1), "a generation change did not cancel the pending continuation");

var lifecycleScheduler = new SimulationScheduler();
var lifecycleRuns = 0;
lifecycleScheduler.ScheduleAt(40, _ => lifecycleRuns++);
lifecycleScheduler.Advance(UpdateAt(40, 1, state: ProductLifecycleState.Paused));
lifecycleScheduler.Advance(UpdateAt(41, 1, state: ProductLifecycleState.Faulted));
lifecycleScheduler.Advance(UpdateAt(42, 1, state: ProductLifecycleState.Shutdown));
Require(lifecycleRuns == 0, "a non-running lifecycle update dispatched work");
lifecycleScheduler.Advance(UpdateAt(43, 1));
Require(lifecycleRuns == 1, "a retained callback did not dispatch after a running admitted step");

var customOrder = new List<string>();
var customPipeline = new UpdatePipeline(engine, [UpdatePhase.Presentation, UpdatePhase.Input]);
Register(customPipeline, UpdatePhase.Input, "input", customOrder, engine);
Register(customPipeline, UpdatePhase.Presentation, "presentation", customOrder, engine);
customPipeline.Run(new ProductUpdate(new ProductUpdateFacts(ProductUpdateMode.Realtime, ProductLifecycleState.Running, 1, 1, 0, 0, 60, 0, 0, 1.0 / 60.0), ReadOnlySpan<ProductInputEvent>.Empty));

Require(string.Join(',', customOrder) == "presentation,input", "the supplied phase order was not used");

static ProductUpdate UpdateAt(
    ulong firstStep,
    uint admittedStepCount,
    ulong generation = 1,
    ProductLifecycleState state = ProductLifecycleState.Running)
{
    return new ProductUpdate(
        new ProductUpdateFacts(
            ProductUpdateMode.Realtime,
            state,
            generation,
            1,
            0,
            firstStep,
            60,
            admittedStepCount,
            0,
            1.0 / 60.0),
        ReadOnlySpan<ProductInputEvent>.Empty);
}

static void Register(UpdatePipeline pipeline, UpdatePhase phase, string name, List<string> order, IEngineContext expectedEngine)
{
    pipeline.Register(phase, (engine, _) =>
    {
        Require(ReferenceEquals(engine, expectedEngine), "the callback received a different engine context");
        order.Add(name);
    });
}

static void Require(bool condition, string message)
{
    if (!condition)
    {
        throw new InvalidOperationException(message);
    }
}

sealed class ExampleEngineContext : IEngineContext
{
    public ILookService Look => throw new NotSupportedException();
    public IAudioService Audio => throw new NotSupportedException();
    public IDynamicsService Dynamics => throw new NotSupportedException();
    public IMotionService Motion => throw new NotSupportedException();
    public IKinematicService Kinematic => throw new NotSupportedException();
    public ISpatialService Spatial => throw new NotSupportedException();
    public IPerceptionService Perception => throw new NotSupportedException();
    public IWorldOriginService WorldOrigin => throw new NotSupportedException();
    public IVoxelService Voxel => throw new NotSupportedException();
    public IVoxelContentService VoxelContent => throw new NotSupportedException();
    public IVoxelScenePresentationService VoxelScenePresentation => throw new NotSupportedException();
    public IContentService Content => throw new NotSupportedException();
    public IAuthoredContentService AuthoredContent => throw new NotSupportedException();
    public IContentStoreService ContentStore => throw new NotSupportedException();
    public IAppearanceService Appearance => throw new NotSupportedException();
    public IPresentationService Presentation => throw new NotSupportedException();
    public IAnimationService Animation => throw new NotSupportedException();
    public IRandomService Random => throw new NotSupportedException();
    public ICameraViewService CameraView => throw new NotSupportedException();
    public IPersistenceService Persistence => throw new NotSupportedException();
    public IUiService Ui => throw new NotSupportedException();
}
