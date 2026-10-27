using System.Text;
using Rusty.Engine;

namespace CsharpRelease;

/// <summary>
/// Opens the content container its ReleaseImport tool installs beside
/// product.rpak, saves the greeting it holds and quits. Without the container
/// it says how to make one and keeps running.
/// </summary>
public sealed class Product(ProductCreateContext context) : IEngineProduct
{
    private const string ContainerName = "release-data.rpak";
    private const string GreetingPath = "greeting.txt";
    private const string SaveScope = "release";
    private const string SaveKey = "greeting";

    private bool opened;

    public void Start() { }

    public ProductUpdateResult Update(ProductUpdate update)
    {
        if (opened) return ProductUpdateResult.None;
        opened = true;
        string greeting;
        try
        {
            using ProductContentBundle data = ProductContentBundle.OpenContainer(context.Engine.Content, ContainerName);
            greeting = data.ReadText(GreetingPath);
        }
        catch (EngineCallException error)
        {
            Console.Error.WriteLine($"{ContainerName} is not installed: run tools/ReleaseImport/ReleaseImport <folder>. {error.Message}");
            return ProductUpdateResult.None;
        }
        IPersistenceService persistence = context.Engine.Persistence;
        using PersistenceStore store = persistence.OpenStore(new(SaveScope));
        persistence.Save(new(store, SaveKey, PersistenceRevisionGuard.Any, 0, Encoding.UTF8.GetBytes(greeting)));
        Console.WriteLine($"RELEASE_FIXTURE_OPENED {greeting.Trim()}");
        context.Engine.Host.RequestExit();
        return ProductUpdateResult.None;
    }

    public void Pause() { }
    public void Resume() { }
    public void Restart() { }
    public void Shutdown() { }
    public void Dispose() { }
}
