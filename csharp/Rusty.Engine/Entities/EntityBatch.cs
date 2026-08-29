namespace Rusty.Engine.Entities;

/// <summary>
/// Stages mutations that <see cref="EntityWorld"/> validates and commits atomically.
/// Atomicity covers only the world's state: delegates must not rely on rollback of external captures.
/// </summary>
public sealed class EntityBatch
{
    private readonly List<Action<EntityWorld>> _mutations = [];

    public EntityBatch Mutate(Action<EntityWorld> mutation)
    {
        ArgumentNullException.ThrowIfNull(mutation);
        _mutations.Add(mutation);
        return this;
    }

    internal IReadOnlyList<Action<EntityWorld>> Mutations => _mutations;
}

public readonly record struct EntityBatchReceipt(ulong RevisionBefore, ulong RevisionAfter, int MutationCount);
