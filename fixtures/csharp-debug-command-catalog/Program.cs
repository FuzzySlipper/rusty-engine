using System;
using System.Linq;
using Rusty.Engine;
using Rusty.Engine.Debugging;
using Rusty.Engine.NativeProduct;

[assembly: EngineProduct(typeof(DebugCommandCatalogFixture.Product))]

namespace DebugCommandCatalogFixture;

public sealed class Product : IEngineProduct, IDebugCommandModuleSource
{
    private readonly FixtureModule _module = new();

    internal Product()
    {
    }

    public Product(ProductCreateContext context)
        : this()
    {
    }

    public void RegisterDebugCommands(IDebugCommandModuleRegistrar registrar)
    {
        DebugCommandRegistrationResult registration = registrar.Register(_module);
        if (!registration.Succeeded)
        {
            throw new InvalidOperationException(registration.Message);
        }
    }

    public void Start() { }
    public ProductUpdateResult Update(ProductUpdate update) => ProductUpdateResult.None;
    public void Pause() { }
    public void Resume() { }
    public void Restart() { }
    public void Shutdown() { }
    public void Dispose() { }
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
        IDebugCommandCatalog catalog = GeneratedDebugCommandCatalogFactory.Create(new Product());
        Expect(catalog.Commands.Select(command => command.Name).SequenceEqual([
            "fixture.add", "fixture.count", "fixture.reset"
        ]), "catalog descriptors were not deterministic");

        Guid key = Guid.Parse("1e92d98d-2be3-4b8f-bec4-5b34fe696a23");
        Expect(catalog.Execute($"fixture.add 7 Beta {key}") is { Succeeded: true, Message: var add } && add.EndsWith(":7", StringComparison.Ordinal), "typed primitive, enum, and ISpanParsable parsing failed");
        Expect(catalog.Execute("fixture.count") == DebugCommandResult.Success("7"), "first direct invocation failed");
        Expect(catalog.Execute("fixture.reset quoted-value") is { Succeeded: true }, "void command did not return success");
        Expect(catalog.Execute("fixture.count") == DebugCommandResult.Success("12"), "repeated invocation did not preserve the live module");
        Expect(catalog.Execute("fixture.add wrong Beta not-a-guid").Status == DebugCommandStatus.InvalidArguments, "invalid parsing did not return an explicit failure");
        Expect(catalog.Execute("fixture.unknown").Status == DebugCommandStatus.UnknownCommand, "unknown command did not return an explicit failure");

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
