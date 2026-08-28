namespace Rusty.Engine.Entities;

/// <summary>
/// A validated managed restore candidate. Its publish step is an assignment only; all validation
/// belongs to <see cref="EntityWorld.PrepareRestore"/> before a cross-world composition commits.
/// </summary>
internal sealed class EntityWorldRestoreCandidate
{
    private readonly EntityWorld _world;
    private readonly EntityWorld.WorldState _state;

    internal EntityWorldRestoreCandidate(EntityWorld world, EntityWorld.WorldState state)
    {
        _world = world;
        _state = state;
    }

    internal void Publish() => _world.PublishPreparedRestore(_state);
}
