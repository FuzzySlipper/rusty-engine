using System;
using Rusty.Engine;
using Rusty.Engine.Debugging;
using Rusty.Engine.Entities;
using Rusty.Engine.NativeProduct;

namespace EntityStoreDebugFixture;

public sealed class Product : IEngineProduct, IDebugCommandModuleSource
{
    private static readonly ComponentType<Health> Health = ComponentType<Health>.Create(ProductComponentKeys.Create(1));
    private static readonly ComponentType<HiddenFact> Hidden = ComponentType<HiddenFact>.Create(ProductComponentKeys.Create(2));
    private static readonly ComponentType<ExplosiveFact> Explosive = ComponentType<ExplosiveFact>.Create(ProductComponentKeys.Create(3));
    private readonly EntityStore _world = new([Health, Hidden, Explosive]);
    private readonly EntityStore _secondary = new([Health]);
    private readonly EntityStoreDebugModule _entities = new();
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

        _entities.RegisterStore("alpha", _world);
        _entities.RegisterStore("beta", _secondary);
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

public sealed class MutationModule(EntityStore world, EntityId actor) : IDebugCommandModule
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
        MechanicsDebugExercise.Run();
        var product = new Product();
        IDebugCommandCatalog catalog = GeneratedDebugCommandCatalogFactory.Create(product);

        Require(catalog.Execute("entity.stores") is { Succeeded: true, Message: var worlds }
            && worlds == "stores=2;name=alpha;name=beta", "world registration did not reach the generated catalog deterministically");
        Require(catalog.Execute("entity.summary alpha") is { Succeeded: true, Message: var summary }
            && summary.Contains("active=1", StringComparison.Ordinal)
            && summary.Contains("disabled=1", StringComparison.Ordinal)
            && summary.Contains("tombstoned=0", StringComparison.Ordinal), "world lifecycle summary was incomplete");
        Require(catalog.Execute("entity.list alpha All 0 2") is { Succeeded: true, Message: var page }
            && page.Contains("entity=1:lifecycle=Active", StringComparison.Ordinal)
            && page.Contains("entity=2:lifecycle=Disabled", StringComparison.Ordinal), "entity page was not deterministic");
        Require(catalog.Execute("entity.list alpha All 18446744073709551615 1") is { Succeeded: true, Message: var terminal }
            && terminal.Contains("count=0", StringComparison.Ordinal), "cursor overflow did not produce an empty page");
        Require(catalog.Execute("entity.list alpha All 0 1000") is { Succeeded: true, Message: var wide }
            && wide.Contains("count=2", StringComparison.Ordinal), "a page larger than the store did not list every entity");
        Require(catalog.Execute("entity.list alpha All 0 0") is { Succeeded: true, Message: var empty }
            && empty.Contains("count=0", StringComparison.Ordinal), "a zero page limit did not give an empty page");
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

        var direct = new EntityStoreDebugModule();
        var standalone = new EntityStore([Product.HealthForFixture]);
        direct.RegisterStore("standalone", standalone);
        direct.RegisterProjection(Product.HealthForFixture, static (in Health value) => new string('x', value.Current));
        direct.RegisterStore("two words", standalone);
        Require(direct.Summary("two words").Succeeded, "a store name with a space was not inspectable");
        const int LongProjection = 10_000;
        EntityId longEntity = standalone.Create();
        standalone.Set(longEntity, Product.HealthForFixture, new Health(LongProjection));
        Require(direct.GetComponent("standalone", longEntity.Value, Product.HealthForFixture.Key.Value) is { Succeeded: true, Message: var whole }
            && whole.Contains(new string('x', LongProjection), StringComparison.Ordinal), "a long projection was not returned whole");
        standalone.Dispose();
        Require(direct.Summary("standalone").Status == DebugCommandStatus.Failed, "disposed world did not report a bounded failure");

        var generations = new EntityStoreDebugModule();
        var original = new EntityStore([Product.HealthForFixture]);
        EntityId originalEntity = original.Create();
        original.Set(originalEntity, Product.HealthForFixture, new Health(11));
        generations.RegisterStore("current", original);
        generations.RegisterProjection(Product.HealthForFixture, static (in Health value) => $"current={value.Current}");

        var disposedReplacement = new EntityStore([Product.HealthForFixture]);
        disposedReplacement.Dispose();
        Require(generations.GetComponent("current", originalEntity.Value, Product.HealthForFixture.Key.Value) is { Succeeded: true, Message: var preserved }
            && preserved.Contains("value=current=11", StringComparison.Ordinal), "the registered store was not read");
        generations.RegisterStore("current", disposedReplacement);
        Require(generations.Summary("current").Status == DebugCommandStatus.Failed, "a disposed registered store was not reported");

        var replacement = new EntityStore([Product.HealthForFixture]);
        EntityId replacementEntity = replacement.Create();
        EntityId replacementOnlyEntity = replacement.Create();
        replacement.Set(replacementEntity, Product.HealthForFixture, new Health(22));
        generations.RegisterStore("current", replacement);
        Require(generations.GetComponent("current", replacementEntity.Value, Product.HealthForFixture.Key.Value) is { Succeeded: true, Message: var replaced }
            && replaced.Contains("value=current=22", StringComparison.Ordinal)
            && !replaced.Contains("value=current=11", StringComparison.Ordinal), "replacement reads did not use only the current generation");
        Require(generations.GetEntity("current", replacementOnlyEntity.Value).Succeeded,
            "replacement-only entity was not discoverable through the stable world name");
        Require(generations.Summary("current") is { Succeeded: true, Message: var replacementSummary }
            && replacementSummary.Contains("entities=2", StringComparison.Ordinal), "replacement summary retained the prior generation");
        original.Dispose();
        Require(generations.Summary("current").Succeeded, "retired world disposal affected the replacement registration");

        RequireThrows(() => generations.UnregisterStore("missing"), "unknown unregister name was accepted");
        Require(generations.Summary("current").Succeeded, "failed unregister corrupted the existing registration");
        generations.UnregisterStore("current");
        Require(generations.ListStores() == DebugCommandResult.Success("stores=0"), "unregister did not remove the world deterministically");
        Require(generations.Summary("current").Status == DebugCommandStatus.InvalidArguments, "unregistered world remained queryable");
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
