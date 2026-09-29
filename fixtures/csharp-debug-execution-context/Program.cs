using Rusty.Engine;
using Rusty.Engine.Debugging;

namespace DebugExecutionContextFixture;

// The runtime-pack proof loads this product through the CoreCLR host.
public sealed class FixtureProduct : IEngineProduct
{
    public FixtureProduct(ProductCreateContext context)
    {
        Debugging = context.Debugging;
    }

    internal DebugExecutionContext Debugging { get; }

    public void Start() { }

    public void Attach() { }

    public ProductUpdateResult Update(ProductUpdate update) => ProductUpdateResult.None;

    public void Pause() { }

    public void Resume() { }

    public void Restart() { }

    public void Shutdown() { }

    public void Dispose() { }
}
