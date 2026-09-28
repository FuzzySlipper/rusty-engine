namespace Rusty.Engine.Entities;

/// <summary>
/// An ordered list of entity creations and value-fact writes that
/// <see cref="EntityStore.Commit"/> applies directly.
/// </summary>
public sealed class EntityBatch
{
    private readonly List<Action<EntityStore>> _mutations = [];

    /// <summary>Adds one value-fact write.</summary>
    public EntityBatch Set<T>(EntityId entity, ComponentType<T> componentType, T value)
        where T : struct
    {
        ArgumentNullException.ThrowIfNull(componentType);
        _mutations.Add(store => store.Set(entity, componentType, value));
        return this;
    }

    /// <summary>Creates the next entity, which must receive <paramref name="expectedId"/>.</summary>
    public EntityBatch Create(EntityId expectedId, EntityLifecycle lifecycle = EntityLifecycle.Active)
    {
        _mutations.Add(store =>
        {
            if (store.NextEntityValue != expectedId.Value)
            {
                throw new InvalidOperationException($"The next entity is {store.NextEntityValue}, not {expectedId.Value}.");
            }
            store.Create(lifecycle);
        });
        return this;
    }

    internal IReadOnlyList<Action<EntityStore>> Mutations => _mutations;
}

public readonly record struct EntityBatchReceipt(ulong RevisionBefore, ulong RevisionAfter);
