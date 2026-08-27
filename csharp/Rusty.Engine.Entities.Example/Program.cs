using Rusty.Engine.Entities;

const uint HealthLocalComponentId = 1;
const uint ArmorLocalComponentId = 2;
var health = ComponentType<Health>.Create(ProductComponentKeys.Create(HealthLocalComponentId), validator: ValidateHealth);
var armor = ComponentType<Armor>.Create(ProductComponentKeys.Create(ArmorLocalComponentId));
using var world = new EntityWorld([EngineComponentTypes.Transform, EngineComponentTypes.CharacterMotion, health, armor]);

EntityId actor = world.Create();
world.Set(actor, health, new Health(10));
Throws(() => world.Set(actor, health, new Health(-1)), "typed component validator did not reject invalid state");
world.Set(actor, armor, new Armor(3));
ComponentRevision healthRevision = world.GetComponentRevision(actor, health);

EntityWorldSnapshot snapshot = world.Snapshot();
EntityBatchReceipt receipt = world.Commit(new EntityBatch()
    .Mutate(staged => staged.Set(actor, health, new Health(6), healthRevision))
    .Mutate(staged => staged.SetLifecycle(actor, EntityLifecycle.Disabled)), expectedRevision: snapshot.Revision);

Require(receipt.RevisionAfter == snapshot.Revision + 1, "a successful batch must advance the world revision exactly once");
Require(world.Query(health).Count == 0, "disabled entities are omitted from normal queries");
Require(world.Query(health, includeDisabled: true).Single().Value.Current == 6, "typed batch mutation did not commit");
Require(world.Query(health, armor, includeDisabled: true).Single().Second.Current == 3, "two-component query did not join typed columns");

Throws(
    () => world.Commit(new EntityBatch()
        .Mutate(staged => staged.Set(actor, health, new Health(4)))
        .Mutate(staged => staged.Set(new EntityId(999), health, new Health(1))), expectedRevision: receipt.RevisionAfter),
    "a rejected batch must report its invalid staged mutation");
Require(world.Get(actor, health).Current == 6 && world.Revision == receipt.RevisionAfter, "a rejected batch changed live state");

world.Restore(snapshot, expectedRevision: receipt.RevisionAfter);
Require(world.Get(actor, health).Current == 10, "in-memory snapshot restore did not recover the typed value");
Throws(() => world.Set(actor, health, new Health(9), healthRevision), "snapshot restore must invalidate old component guards");
Require(world.Diagnostics().Components.Single(component => component.Key == health.Key).ValueCount == 1, "diagnostics lost the component table");

static void Require(bool condition, string message)
{
    if (!condition)
    {
        throw new InvalidOperationException(message);
    }
}

static void Throws(Action action, string message)
{
    try
    {
        action();
    }
    catch (Exception exception) when (exception is InvalidOperationException or ArgumentException)
    {
        return;
    }
    throw new InvalidOperationException(message);
}

static void ValidateHealth(in Health health)
{
    if (health.Current < 0)
    {
        throw new ArgumentOutOfRangeException(nameof(health), "Health cannot be negative.");
    }
}

readonly record struct Health(int Current);

readonly record struct Armor(int Current);
