using System.Buffers;

namespace Rusty.Engine.Persistence;

/// <summary>
/// Product-owned meaning for one durable state value: the current shape, encoded and decoded
/// as-is. The Engine never inspects these bytes and assigns them no schema number, migration
/// path, or compatibility meaning. Changing the shape breaks old saves; that is the product's
/// explicit choice, not a condition the store detects or repairs.
/// </summary>
public interface IProductStateCodec<TState>
{
    void Encode(in TState state, IBufferWriter<byte> destination);

    TState Decode(ReadOnlySpan<byte> payload);
}

public readonly record struct ProductStateLoad<TState>(bool Present, ulong Revision, TState? State);

/// <summary>
/// Managed composition around the generated direct Persistence service. It is
/// intentionally the place where a C# product selects its codec. The constructor
/// receives a relative product scope; the developer host selects the absolute
/// persistence root before product creation.
/// </summary>
public sealed class ProductStateStore<TState> : IDisposable
{
    private readonly IPersistenceService _persistence;
    private readonly PersistenceStore _store;
    private readonly IProductStateCodec<TState> _codec;

    public ProductStateStore(
        IEngineContext engine,
        string scope,
        IProductStateCodec<TState> codec)
    {
        ArgumentNullException.ThrowIfNull(engine);
        ArgumentException.ThrowIfNullOrWhiteSpace(scope);
        _codec = codec ?? throw new ArgumentNullException(nameof(codec));
        _persistence = engine.Persistence;
        _store = _persistence.OpenStore(new PersistenceOpenRequest(scope));
    }

    /// <summary>Returns RevisionConflict with the current stored revision when
    /// the guard does not match; no bytes are written in that case.</summary>
    public PersistenceSaveReceipt Save(
        string key,
        in TState state,
        PersistenceRevisionGuard guard = PersistenceRevisionGuard.Any,
        ulong expectedRevision = 0)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(key);
        var payload = new ArrayBufferWriter<byte>();
        _codec.Encode(in state, payload);
        return _persistence.Save(new PersistenceSaveRequest(
            _store,
            key,
            guard,
            expectedRevision,
            payload.WrittenMemory));
    }

    /// <summary>
    /// Reads and decodes the current shape directly. A missing key reports absent; malformed
    /// bytes fail in the product codec. There is no partial-load success: Decode either
    /// returns the whole value or throws.
    /// </summary>
    public ProductStateLoad<TState> Load(string key)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(key);
        using PersistenceBlob blob = _persistence.Load(new PersistenceLoadRequest(_store, key));
        PersistenceBlobInfo info = _persistence.DescribeBlob(blob);
        if (!info.Present)
        {
            return new ProductStateLoad<TState>(false, 0, default);
        }

        int length = checked((int)info.PayloadLen);
        byte[] payload = new byte[length];
        _persistence.CopyBlob(new PersistenceCopyBlobRequest(blob, payload));
        return new ProductStateLoad<TState>(true, info.Revision, _codec.Decode(payload));
    }

    public void Dispose() => _store.Dispose();
}
