using System.Security.Cryptography;
using Rusty.Engine;
using Rusty.Engine.Debugging;

namespace SourceRootReload;

public sealed class Product : IEngineProduct, IDebugCommandModuleSource, IDebugCommandModule
{
    private const string Bundle = "notes";
    private readonly ProductContent content;
    private readonly Guid instance = Guid.NewGuid();
    private ProductContentBundle? held;

    public Product(ProductCreateContext context) => content = context.Content;

    [DebugCommand("exercise.identity", Description = "Process and product instance, to show no restart.")]
    public string Identity() => $"pid={Environment.ProcessId} instance={instance:N}";

    [DebugCommand("exercise.read", Description = "Opens the bundle now and reads one file.")]
    public string Read(string path)
    {
        using ProductContentBundle bundle = content.OpenBundle(Bundle);
        return Describe(bundle, path);
    }

    [DebugCommand("exercise.hold", Description = "Keeps the bundle opened now alive.")]
    public string Hold()
    {
        held?.Dispose();
        held = content.OpenBundle(Bundle);
        return "held";
    }

    [DebugCommand("exercise.held", Description = "Reads from the bundle kept by exercise.hold.")]
    public string ReadHeld(string path) => held is null ? "nothing held" : Describe(held, path);

    [DebugCommand("exercise.loose", Description = "Reads the create-time loose snapshot.")]
    public string Loose() => content.ReadText("loose.txt").Trim();

    private static string Describe(ProductContentBundle bundle, string path)
    {
        if (!bundle.TryReadFile(path, out ProductContentFile file)) return "missing";
        string sha = Convert.ToHexString(SHA256.HashData(file.Bytes.Span))[..12].ToLowerInvariant();
        return $"{file.ReadText().Trim()} sha={sha}";
    }

    public void RegisterDebugCommands(IDebugCommandModuleRegistrar registrar) => registrar.Register(this);
    public void Start() { }
    public ProductUpdateResult Update(ProductUpdate update) => ProductUpdateResult.None;
    public void Pause() { }
    public void Resume() { }
    public void Restart() { }
    public void Shutdown() { }
    public void Dispose() => held?.Dispose();
}
