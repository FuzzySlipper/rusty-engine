namespace Rusty.Engine.Entities;

/// <summary>
/// Prepared entity creation/value replacements. Publication checks structural revision;
/// it does not freeze or roll back fields inside attached objects. A failed or disposed
/// edit is terminal and releases its staged state. Successful publication is idempotent.
/// </summary>
public sealed class EntityEdit : IDisposable
{
    private readonly EntityStore _store;
    private readonly object _publication = new();
    private EntityStore.StoreState? _state;
    private readonly ulong _preparedRevision;
    private bool _published;

    internal EntityEdit(EntityStore store, EntityStore.StoreState state, ulong preparedRevision, EntityBatchReceipt receipt)
    {
        _store = store;
        _state = state;
        _preparedRevision = preparedRevision;
        Receipt = receipt;
    }

    public EntityBatchReceipt Receipt { get; }

    public void Publish()
    {
        lock (_publication)
        {
            if (_published) return;
            EntityStore.StoreState state = _state
                ?? throw new InvalidOperationException("This entity edit has failed or been disposed.");
            try
            {
                _store.PublishPreparedBatch(state, _preparedRevision);
                _published = true;
            }
            finally
            {
                _state = null;
            }
        }
    }

    public void Dispose()
    {
        lock (_publication) _state = null;
    }
}
