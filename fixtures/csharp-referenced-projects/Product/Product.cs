using Rusty.Engine;
using Rusty.Engine.Debugging;
using ReferencedProjects.Library;

namespace ReferencedProjects;

public sealed class Product : IEngineProduct, IDebugCommandModuleSource, IDebugCommandModule
{
    public Product(ProductCreateContext context) { }

    [DebugCommand("referenced.revision")]
    public string Revision() => LibraryFacts.Describe();

    public void RegisterDebugCommands(IDebugCommandModuleRegistrar registrar) => registrar.Register(this);
    public void Start() { }
    public ProductUpdateResult Update(ProductUpdate update) => ProductUpdateResult.None;
    public void Pause() { }
    public void Resume() { }
    public void Restart() { }
    public void Shutdown() { }
    public void Dispose() { }
}
