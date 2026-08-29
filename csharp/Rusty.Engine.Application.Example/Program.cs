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

var customOrder = new List<string>();
var customPipeline = new UpdatePipeline(engine, [UpdatePhase.Presentation, UpdatePhase.Input]);
Register(customPipeline, UpdatePhase.Input, "input", customOrder, engine);
Register(customPipeline, UpdatePhase.Presentation, "presentation", customOrder, engine);
customPipeline.Run(new ProductUpdate(new ProductUpdateFacts(ProductUpdateMode.Realtime, ProductLifecycleState.Running, 1, 1, 0, 0, 60, 0, 0, 1.0 / 60.0), ReadOnlySpan<ProductInputEvent>.Empty));

Require(string.Join(',', customOrder) == "presentation,input", "the supplied phase order was not used");

static ProductUpdate UpdateAt(ulong firstStep, uint admittedStepCount, ulong generation = 1)
{
    return new ProductUpdate(
        new ProductUpdateFacts(
            ProductUpdateMode.Realtime,
            ProductLifecycleState.Running,
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
    public IMechanicsService Mechanics => throw new NotSupportedException();
    public IContinuousMechanicsService ContinuousMechanics => throw new NotSupportedException();
    public ICameraViewService CameraView => throw new NotSupportedException();
    public IPersistenceService Persistence => throw new NotSupportedException();
    public IRulesService Rules => throw new NotSupportedException();
    public IResolutionService Resolution => throw new NotSupportedException();
    public IStateMachineService StateMachine => throw new NotSupportedException();
    public IStandardExactService StandardExact => throw new NotSupportedException();
    public IStandardContinuousService StandardContinuous => throw new NotSupportedException();
    public IUiService Ui => throw new NotSupportedException();
}
