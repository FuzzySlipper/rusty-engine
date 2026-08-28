namespace Rusty.Engine.Entities;

/// <summary>
/// A validated managed batch ready for a cross-owner composition. Its publish
/// step is an assignment only; every guard, component validator, and product
/// delegate runs in <see cref="EntityWorld.PrepareBatch"/> before another owner
/// commits.
/// </summary>
public sealed class EntityWorldBatchCandidate
{
    private readonly EntityWorld _world;
    private readonly EntityWorld.WorldState _state;
    private int _published;

    internal EntityWorldBatchCandidate(
        EntityWorld world,
        EntityWorld.WorldState state,
        EntityBatchReceipt receipt)
    {
        _world = world;
        _state = state;
        Receipt = receipt;
    }

    public EntityBatchReceipt Receipt { get; }

    /// <summary>
    /// Publishes the already validated batch by assignment. Repeated calls are
    /// harmless and never revalidate, allocate, or invoke product delegates.
    /// </summary>
    public void Publish()
    {
        if (Interlocked.Exchange(ref _published, 1) != 0)
        {
            return;
        }
        _world.PublishPreparedBatch(_state);
    }
}
