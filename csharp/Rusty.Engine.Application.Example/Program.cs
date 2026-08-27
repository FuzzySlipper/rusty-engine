using Rusty.Engine;
using Rusty.Engine.Application;

var engine = new ExampleEngineContext();
var defaultOrder = new List<string>();
var defaultPipeline = new UpdatePipeline(engine);

Register(defaultPipeline, UpdatePhase.Input, "input", defaultOrder, engine);
Register(defaultPipeline, UpdatePhase.Update, "update-one", defaultOrder, engine);
Register(defaultPipeline, UpdatePhase.Update, "update-two", defaultOrder, engine);
Register(defaultPipeline, UpdatePhase.LateUpdate, "late-update", defaultOrder, engine);
Register(defaultPipeline, UpdatePhase.Presentation, "presentation", defaultOrder, engine);
defaultPipeline.Run(new ProductUpdate(ProductTurnKind.Realtime, ReadOnlySpan<ProductInputEvent>.Empty, 0));

Require(string.Join(',', defaultOrder) == "input,update-one,update-two,late-update,presentation", "the default pass order is not deterministic");

var customOrder = new List<string>();
var customPipeline = new UpdatePipeline(engine, [UpdatePhase.Presentation, UpdatePhase.Input]);
Register(customPipeline, UpdatePhase.Input, "input", customOrder, engine);
Register(customPipeline, UpdatePhase.Presentation, "presentation", customOrder, engine);
customPipeline.Run(new ProductUpdate(ProductTurnKind.Realtime, ReadOnlySpan<ProductInputEvent>.Empty, 0));

Require(string.Join(',', customOrder) == "presentation,input", "the supplied phase order was not used");

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
    public ISpatialService Spatial => throw new NotSupportedException();
    public IAppearanceService Appearance => throw new NotSupportedException();
    public IAnimationService Animation => throw new NotSupportedException();
    public IRandomService Random => throw new NotSupportedException();
    public IMechanicsService Mechanics => throw new NotSupportedException();
    public IUiService Ui => throw new NotSupportedException();
}
