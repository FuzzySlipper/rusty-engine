using System;
using System.Linq;
using Rusty.Engine;
using Rusty.Engine.Debugging;
using Rusty.Engine.NativeProduct;

namespace DebugCommandCatalogFixture;

public sealed class Product : IEngineProduct, IDebugCommandModuleSource
{
    private readonly FixtureModule _module = new();
    private readonly FixtureDiagnosticsModule _diagnostics = new();
    private IDebugCommandModuleRegistrar? _registrar;

    internal Product()
    {
    }

    public Product(ProductCreateContext context)
        : this()
    {
    }

    public void RegisterDebugCommands(IDebugCommandModuleRegistrar registrar)
    {
        Register(registrar.Register(_module));
        Register(registrar.Register(_diagnostics));
        Register(registrar.Register(new FixtureSession("first", 1)));
        _registrar = registrar;
    }

    /// <summary>Replaces the session the way a product does on new game or load.</summary>
    internal void ReplaceSession(FixtureSession session) => Register(_registrar!.Register(session));

    internal DebugCommandRegistrationResult RegisterWithoutCommands() => _registrar!.Register(new FixtureModuleWithoutCommands());

    public void Start() { }
    public ProductUpdateResult Update(ProductUpdate update) => ProductUpdateResult.None;
    public void Pause() { }
    public void Resume() { }
    public void Restart() { }
    public void Shutdown() { }
    public void Dispose() { }

    private static void Register(DebugCommandRegistrationResult registration)
    {
        if (!registration.Succeeded) throw new InvalidOperationException(registration.Message);
    }
}

internal sealed class ProductWithoutModules : IEngineProduct
{
    public void Start() { }
    public ProductUpdateResult Update(ProductUpdate update) => ProductUpdateResult.None;
    public void Pause() { }
    public void Resume() { }
    public void Restart() { }
    public void Shutdown() { }
    public void Dispose() { }
}

internal static class Program
{
    private static int Main()
    {
        Product product = new();
        IDebugCommandCatalog catalog = GeneratedDebugCommandCatalogFactory.Create(product);
        Expect(catalog.Commands.Select(command => command.Name).Where(name => name.StartsWith("fixture.", StringComparison.Ordinal)).SequenceEqual([
            "fixture.add", "fixture.count", "fixture.ping", "fixture.reset", "fixture.session", "fixture.turns"
        ]), "catalog descriptors were not deterministic");

        Guid key = Guid.Parse("1e92d98d-2be3-4b8f-bec4-5b34fe696a23");
        Expect(catalog.Execute($"fixture.add 7 Beta {key}") is { Succeeded: true, Message: var add } && add.EndsWith(":7", StringComparison.Ordinal), "typed primitive, enum, and ISpanParsable parsing failed");
        Expect(catalog.Execute("fixture.count") == DebugCommandResult.Success("7"), "first direct invocation failed");
        Expect(catalog.Execute("fixture.reset quoted-value") is { Succeeded: true }, "void command did not return success");
        Expect(catalog.Execute("fixture.count") == DebugCommandResult.Success("12"), "repeated invocation did not preserve the live module");
        Expect(catalog.Execute("fixture.ping 42") == DebugCommandResult.Success("pong:42"), "second live module did not register and dispatch");
        Expect(catalog.Execute("fixture.add wrong Beta not-a-guid").Status == DebugCommandStatus.InvalidArguments, "invalid parsing did not return an explicit failure");
        Expect(catalog.Execute("fixture.unknown").Status == DebugCommandStatus.UnknownCommand, "unknown command did not return an explicit failure");
        Expect(catalog.Execute("fixture.session") == DebugCommandResult.Success("first")
            && catalog.Execute("fixture.turns") == DebugCommandResult.Success("1"), "one object did not serve both of its command interfaces");
        product.ReplaceSession(new FixtureSession("second", 2));
        Expect(catalog.Execute("fixture.session") == DebugCommandResult.Success("second")
            && catalog.Execute("fixture.turns") == DebugCommandResult.Success("2"), "a later registration did not replace the live instance for each interface");
        Expect(product.RegisterWithoutCommands().Status == DebugCommandRegistrationStatus.UnsupportedModule, "a module without generated commands registered");

        IDebugCommandCatalog absent = GeneratedDebugCommandCatalogFactory.Create(new ProductWithoutModules());
        Expect(absent.Execute("fixture.count").Status == DebugCommandStatus.ModuleUnavailable, "missing module did not return an explicit failure");
        return 0;
    }

    private static void Expect(bool condition, string message)
    {
        if (!condition)
        {
            throw new InvalidOperationException(message);
        }
    }
}
