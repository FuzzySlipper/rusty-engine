using System;
using Rusty.Engine;
using Rusty.Engine.Debugging;
using Rusty.Engine.Entities;
using Rusty.Engine.NativeProduct;

[assembly: EngineProduct(typeof(EntityWorldDebugFixture.Product))]

namespace EntityWorldDebugFixture;

public sealed class Product : IEngineProduct, IDebugCommandModuleSource
{
    private static readonly ComponentType<Health> Health = ComponentType<Health>.Create(ProductComponentKeys.Create(1));
    private static readonly ComponentType<HiddenFact> Hidden = ComponentType<HiddenFact>.Create(ProductComponentKeys.Create(2));
    private static readonly ComponentType<ExplosiveFact> Explosive = ComponentType<ExplosiveFact>.Create(ProductComponentKeys.Create(3));
    private readonly EntityWorld _world = new([Health, Hidden, Explosive]);
    private readonly EntityWorld _secondary = new([Health]);
    private readonly EntityWorldDebugModule _entities = new();
    private readonly MutationModule _mutation;
    private readonly EntityId _actor;

    public static ComponentType<Health> HealthForFixture => Health;

    internal Product()
    {
        _actor = _world.Create();
        EntityId disabled = _world.Create(EntityLifecycle.Disabled);
        EntityId tombstoned = _world.Create();
        _world.Set(_actor, Health, new Health(8));
        _world.Set(_actor, Hidden, new HiddenFact(3));
        _world.Set(_actor, Explosive, new ExplosiveFact(1));
        _world.SetContainment(disabled, _actor);
        _world.Destroy(tombstoned);

        _entities.RegisterWorld("alpha", _world);
        _entities.RegisterWorld("beta", _secondary);
        _entities.RegisterProjection(Health, static (in Health value) => $"current={value.Current}");
        _entities.RegisterProjection(Explosive, static (in ExplosiveFact _) => throw new InvalidOperationException("formatter must be contained"));
        _mutation = new MutationModule(_world, _actor);
    }

    public Product(ProductCreateContext context) : this()
    {
    }

    public void RegisterDebugCommands(IDebugCommandModuleRegistrar registrar)
    {
        Require(registrar.Register(_entities).Succeeded, "entity debug module did not register");
        Require(registrar.Register(_mutation).Succeeded, "mutation module did not register");
    }

    public void Start() { }
    public void Attach() { }
    public ProductUpdateResult Update(ProductUpdate update) => ProductUpdateResult.None;
    public void Pause() { }
    public void Resume() { }
    public void Restart() { }
    public void Shutdown() { }
    public void Dispose() { }

    private static void Require(bool condition, string message)
    {
        if (!condition)
        {
            throw new InvalidOperationException(message);
        }
    }
}

public readonly record struct Health(int Current);
public readonly record struct HiddenFact(int Value);
public readonly record struct ExplosiveFact(int Value);

public sealed class MutationModule(EntityWorld world, EntityId actor) : IDebugCommandModule
{
    [DebugCommand("fixture.damage")]
    public DebugCommandResult Damage(int amount)
    {
        Health health = world.Get(actor, Product.HealthForFixture);
        world.Set(actor, Product.HealthForFixture, new Health(health.Current - amount));
        return DebugCommandResult.Success($"health={health.Current - amount}");
    }
}

internal static class Program
{
    private static int Main()
    {
        var product = new Product();
        IDebugCommandCatalog catalog = GeneratedDebugCommandCatalogFactory.Create(product);

        Require(catalog.Execute("entity.worlds") is { Succeeded: true, Message: var worlds }
            && worlds == "worlds=2;name=alpha;name=beta", "world registration did not reach the generated catalog deterministically");
        Require(catalog.Execute("entity.summary alpha") is { Succeeded: true, Message: var summary }
            && summary.Contains("active=1", StringComparison.Ordinal)
            && summary.Contains("disabled=1", StringComparison.Ordinal)
            && summary.Contains("tombstoned=1", StringComparison.Ordinal), "world lifecycle summary was incomplete");
        Require(catalog.Execute("entity.list alpha All 0 2") is { Succeeded: true, Message: var page }
            && page.Contains("entity=1:lifecycle=Active", StringComparison.Ordinal)
            && page.Contains("entity=2:lifecycle=Disabled", StringComparison.Ordinal), "entity page was not deterministic");
        Require(catalog.Execute("entity.list alpha All 18446744073709551615 1") is { Succeeded: true, Message: var terminal }
            && terminal.Contains("count=0", StringComparison.Ordinal), "cursor overflow did not produce an empty page");
        Require(catalog.Execute("entity.list alpha All 0 0").Status == DebugCommandStatus.InvalidArguments, "zero page limit was accepted");
        Require(catalog.Execute("entity.list alpha All 0 65").Status == DebugCommandStatus.InvalidArguments, "over-cap page limit was accepted");
        Require(catalog.Execute("entity.list alpha 99 0 1").Status == DebugCommandStatus.InvalidArguments, "unknown selector was accepted");
        Require(catalog.Execute("entity.get alpha 1") is { Succeeded: true, Message: var entity }
            && entity.Contains("children=2", StringComparison.Ordinal)
            && entity.Contains("component=1024", StringComparison.Ordinal)
            && entity.Contains("component=1025", StringComparison.Ordinal), "entity containment or component presence was omitted");
        Require(catalog.Execute("entity.children alpha 1 0 1") is { Succeeded: true, Message: var children }
            && children.Contains("child=2", StringComparison.Ordinal), "contained child page was not ordered");
        Require(catalog.Execute("entity.component alpha 1 1024") is { Succeeded: true, Message: var projected }
            && projected.Contains("value=current=8", StringComparison.Ordinal), "projected component value was not returned");
        Require(catalog.Execute("entity.component alpha 1 1025") is { Succeeded: true, Message: var unprojected }
            && unprojected.Contains("projection-unavailable", StringComparison.Ordinal), "unprojected component did not remain discoverable");
        Require(catalog.Execute("entity.component alpha 2 1024") is { Succeeded: true, Message: var absent }
            && absent.Contains("present=false", StringComparison.Ordinal), "absent component was not distinguished from an unprojected one");
        Require(catalog.Execute("entity.component alpha 1 9999").Status == DebugCommandStatus.InvalidArguments, "unknown component key was accepted");
        Require(catalog.Execute("entity.component alpha 1 1026").Status == DebugCommandStatus.Failed, "formatter exception escaped the projection boundary");
        Require(catalog.Execute("entity.get missing 1").Status == DebugCommandStatus.InvalidArguments, "unknown world was accepted");
        Require(catalog.Execute("entity.get alpha 99").Status == DebugCommandStatus.InvalidArguments, "unknown entity was accepted");
        Require(catalog.Execute("fixture.damage 3") == DebugCommandResult.Success("health=5"), "ordinary named mutation command failed");
        Require(catalog.Execute("entity.component alpha 1 1024") is { Succeeded: true, Message: var after }
            && after.Contains("value=current=5", StringComparison.Ordinal), "inspection did not observe an ordinary named mutation");

        var direct = new EntityWorldDebugModule();
        var standalone = new EntityWorld([Product.HealthForFixture]);
        direct.RegisterWorld("standalone", standalone);
        direct.RegisterProjection(Product.HealthForFixture, static (in Health value) => new string('x', value.Current));
        RequireThrows(() => direct.RegisterWorld("standalone", standalone), "duplicate world name was accepted");
        RequireThrows(() => direct.RegisterWorld("not valid", standalone), "invalid world name was accepted");
        RequireThrows(() => direct.RegisterWorld(new string('x', EntityWorldDebugModule.MaximumWorldNameLength + 1), standalone), "oversized world name was accepted");
        RequireThrows(() => direct.RegisterProjection(Product.HealthForFixture, static (in Health _) => "duplicate"), "duplicate projection key was accepted");
        EntityId oversizedEntity = standalone.Create();
        standalone.Set(oversizedEntity, Product.HealthForFixture, new Health(EntityWorldDebugModule.MaximumResultLength));
        Require(direct.GetComponent("standalone", oversizedEntity.Value, Product.HealthForFixture.Key.Value) is { Succeeded: true, Message: var oversized }
            && oversized.Length == EntityWorldDebugModule.MaximumResultLength, "oversized projection output was not bounded");
        standalone.Dispose();
        Require(direct.Summary("standalone").Status == DebugCommandStatus.Failed, "disposed world did not report a bounded failure");
        return 0;
    }

    private static void Require(bool condition, string message)
    {
        if (!condition)
        {
            throw new InvalidOperationException(message);
        }
    }

    private static void RequireThrows(Action action, string message)
    {
        try
        {
            action();
        }
        catch (InvalidOperationException)
        {
            return;
        }
        catch (ArgumentException)
        {
            return;
        }
        throw new InvalidOperationException(message);
    }
}
