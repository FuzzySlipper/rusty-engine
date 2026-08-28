namespace Rusty.Engine.Entities;

/// <summary>
/// A validated managed restore candidate. Its publish step is an assignment only; all validation
/// belongs to <see cref="EntityWorld.PrepareRestore"/> before a cross-world composition commits.
/// </summary>
public sealed class EntityWorldRestoreCandidate
{
    private readonly EntityWorld _world;
    private readonly EntityWorld.WorldState _state;
    private int _published;

    internal EntityWorldRestoreCandidate(EntityWorld world, EntityWorld.WorldState state)
    {
        _world = world;
        _state = state;
    }

    /// <summary>
    /// Publishes the already validated state by assignment. Repeated calls are harmless and do
    /// not revalidate, allocate, invoke product code, or perform any other fallible work.
    /// </summary>
    public void Publish()
    {
        if (Interlocked.Exchange(ref _published, 1) != 0)
        {
            return;
        }
        _world.PublishPreparedRestore(_state);
    }
}
