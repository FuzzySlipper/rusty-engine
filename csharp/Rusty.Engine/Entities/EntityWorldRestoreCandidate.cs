namespace Rusty.Engine.Entities;

/// <summary>
/// A validated managed restore candidate. It may publish only while the live world remains at
/// the revision from which it was prepared, so it cannot discard intervening mutations.
/// </summary>
public sealed class EntityWorldRestoreCandidate
{
    private readonly EntityWorld _world;
    private readonly EntityWorld.WorldState _state;
    private readonly ulong _preparedRevision;
    // 0 is unconsumed, 1 is one caller publishing, and 2 is successfully published.
    private int _publicationState;

    internal EntityWorldRestoreCandidate(EntityWorld world, EntityWorld.WorldState state, ulong preparedRevision)
    {
        _world = world;
        _state = state;
        _preparedRevision = preparedRevision;
    }

    /// <summary>
    /// Publishes the already validated state if the live world has not changed. Repeated calls
    /// after success are harmless. A stale failure leaves this candidate unconsumed, although a
    /// normally monotonic world revision means it cannot become current again without an
    /// owner-controlled replacement.
    /// </summary>
    public void Publish()
    {
        var spinner = new SpinWait();
        while (true)
        {
            int state = Volatile.Read(ref _publicationState);
            if (state == 2)
            {
                return;
            }
            if (state == 0 && Interlocked.CompareExchange(ref _publicationState, 1, 0) == 0)
            {
                try
                {
                    _world.PublishPreparedRestore(_state, _preparedRevision);
                    Volatile.Write(ref _publicationState, 2);
                    return;
                }
                catch
                {
                    Volatile.Write(ref _publicationState, 0);
                    throw;
                }
            }
            spinner.SpinOnce();
        }
    }
}
