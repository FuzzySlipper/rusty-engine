namespace Rusty.Engine.Entities;

/// <summary>
/// An immutable, in-process checkpoint made by <see cref="EntityWorld.Snapshot"/>.
/// It deliberately has no serialized schema; persistence owns its own future boundary.
/// </summary>
public sealed class EntityWorldSnapshot
{
    internal EntityWorldSnapshot(EntityWorld.WorldState state)
    {
        State = state;
    }

    internal EntityWorld.WorldState State { get; }

    public ulong Revision => State.Revision;

    public int EntityCount => State.Entities.Count;
}
