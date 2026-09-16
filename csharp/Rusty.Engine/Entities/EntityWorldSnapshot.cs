namespace Rusty.Engine.Entities;

/// <summary>
/// An immutable, in-process checkpoint made by <see cref="EntityStore.Snapshot"/>.
/// It deliberately has no serialized schema; persistence owns its own future boundary.
/// </summary>
public sealed class EntityWorldSnapshot
{
    internal EntityWorldSnapshot(EntityStore.StoreState state)
    {
        State = state;
    }

    internal EntityStore.StoreState State { get; }

    public ulong Revision => State.Revision;

    public int EntityCount => State.Entities.Count;
}
