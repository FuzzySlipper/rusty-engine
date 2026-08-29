using Rusty.Engine;

namespace Rusty.Engine.Entities;

/// <summary>
/// A paired, validated import candidate. Before publication it owns only the prepared native
/// import handle; disposing it cancels that prepared native candidate and leaves both live worlds
/// unchanged. Publication is intentionally idempotent.
/// </summary>
public sealed class MechanicsEntityWorldImportCandidate : IDisposable
{
    private readonly MechanicsEntityWorld _world;
    private readonly EntityWorldRestoreCandidate _managed;
    private MechanicsWorldImport? _native;
    private int _published;

    internal MechanicsEntityWorldImportCandidate(
        MechanicsEntityWorld world,
        MechanicsWorldImport native,
        EntityWorldRestoreCandidate managed,
        MechanicsWorldImportLeaseReceipt receipt)
    {
        _world = world;
        _native = native;
        _managed = managed;
        Receipt = receipt;
    }

    /// <summary>Copied typed evidence for the already prepared native import.</summary>
    public MechanicsWorldImportLeaseReceipt Receipt { get; }

    /// <summary>
    /// Publishes the native candidate, claims every replacement binding, then publishes the
    /// managed candidate and swaps the adapter map. Repeated successful calls are harmless.
    /// </summary>
    public void Publish()
    {
        if (Volatile.Read(ref _published) != 0)
        {
            return;
        }

        MechanicsWorldImport native = _native
            ?? throw new ObjectDisposedException(nameof(MechanicsEntityWorldImportCandidate));
        try
        {
            _world.PublishPreparedImport(native, _managed, Receipt);
            Volatile.Write(ref _published, 1);
        }
        finally
        {
            // A successful candidate retires its import handle here. A failed candidate is also
            // cancelled here; its detached managed candidate owns no resources or live state.
            native.Dispose();
            _native = null;
        }
    }

    /// <summary>
    /// Cancels an unpublished candidate. It deliberately releases only the native preparation;
    /// the managed candidate is a detached value until <see cref="Publish"/> assigns it.
    /// </summary>
    public void Dispose()
    {
        MechanicsWorldImport? native = Interlocked.Exchange(ref _native, null);
        native?.Dispose();
    }
}
