namespace Rusty.Engine.Entities;

/// <summary>
/// Explicit preparation of membership and value-fact replacements. Typed operations preserve
/// attached class references; they do not snapshot or roll back mutable object graphs.
/// </summary>
public sealed class EntityBatch
{
    private readonly List<Action<EntityStore>> _mutations = [];

    /// <summary>Stages one value-fact replacement without running a caller callback.</summary>
    public EntityBatch Set<T>(EntityId entity, ComponentType<T> componentType, T value, ComponentRevision? expectedRevision = null)
        where T : struct
    {
        ArgumentNullException.ThrowIfNull(componentType);
        _mutations.Add(store => store.Set(entity, componentType, value, expectedRevision));
        return this;
    }

    /// <summary>Creates the next local ID, rejecting a changed allocator before publication.</summary>
    public EntityBatch Create(EntityId expectedId, EntityLifecycle lifecycle = EntityLifecycle.Active)
    {
        _mutations.Add(store =>
        {
            if (store.NextEntityValue != expectedId.Value)
            {
                throw new InvalidOperationException("Entity identity changed while preparing creation.");
            }
            store.Create(lifecycle);
        });
        return this;
    }

    internal IReadOnlyList<Action<EntityStore>> Mutations => _mutations;
}

public readonly record struct EntityBatchReceipt(ulong RevisionBefore, ulong RevisionAfter);
