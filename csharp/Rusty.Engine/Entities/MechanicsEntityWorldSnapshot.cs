using Rusty.Engine;

namespace Rusty.Engine.Entities;

/// <summary>
/// One retained, process-local checkpoint for a canonical managed world and its matching native
/// Mechanics world. It is deliberately not serializable: product-owned codecs and migrations stay
/// outside this composition surface.
/// </summary>
public sealed class MechanicsEntityWorldSnapshot : IDisposable
{
    private bool _disposed;

    internal MechanicsEntityWorldSnapshot(
        EntityWorldSnapshot entities,
        MechanicsWorldSnapshot mechanics,
        ulong nativeStateRevision,
        IReadOnlyDictionary<EntityId, MechanicsBindingSnapshot> bindings)
    {
        Entities = entities;
        Mechanics = mechanics;
        NativeStateRevision = nativeStateRevision;
        Bindings = bindings;
    }

    internal EntityWorldSnapshot Entities { get; }
    internal MechanicsWorldSnapshot Mechanics { get; }
    internal IReadOnlyDictionary<EntityId, MechanicsBindingSnapshot> Bindings { get; }

    public ulong ManagedRevision => Entities.Revision;
    public ulong NativeStateRevision { get; }

    public void Dispose()
    {
        if (_disposed)
        {
            return;
        }
        _disposed = true;
        Mechanics.Dispose();
    }
}

internal readonly record struct MechanicsBindingSnapshot(ulong NativeHandle, bool Committed, ulong LifecycleStamp);
