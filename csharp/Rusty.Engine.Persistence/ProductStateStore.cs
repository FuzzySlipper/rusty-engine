using System.Buffers;
using Rusty.Engine.Entities;

namespace Rusty.Engine.Persistence;

/// <summary>
/// Product-owned meaning for one durable state value. The Engine never
/// inspects these bytes or decides how their versions migrate.
/// </summary>
public interface IProductStateCodec<TState>
{
    uint SchemaVersion { get; }

    void Encode(in TState state, IBufferWriter<byte> destination);

    TState Decode(ReadOnlySpan<byte> payload);
}

/// <summary>One explicit product migration edge, selected by concrete schema versions.</summary>
public interface IProductStateMigration
{
    uint FromSchemaVersion { get; }

    uint ToSchemaVersion { get; }

    byte[] Migrate(ReadOnlySpan<byte> payload);
}

public readonly record struct ProductStateLoad<TState>(bool Present, ulong Revision, TState? State);

/// <summary>
/// Managed composition around the generated direct Persistence service. It is
/// intentionally the place where a C# product selects codecs and migrations.
/// </summary>
public sealed class ProductStateStore<TState> : IDisposable
{
    private readonly IPersistenceService _persistence;
    private readonly PersistenceStore _store;
    private readonly IProductStateCodec<TState> _codec;
    private readonly IReadOnlyList<IProductStateMigration> _migrations;

    public ProductStateStore(
        IEngineContext engine,
        string root,
        IProductStateCodec<TState> codec,
        IReadOnlyList<IProductStateMigration>? migrations = null)
    {
        ArgumentNullException.ThrowIfNull(engine);
        ArgumentException.ThrowIfNullOrWhiteSpace(root);
        _codec = codec ?? throw new ArgumentNullException(nameof(codec));
        _migrations = migrations ?? [];
        _persistence = engine.Persistence;
        _store = _persistence.OpenStore(new PersistenceOpenRequest(root));
    }

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
            _codec.SchemaVersion,
            guard,
            expectedRevision,
            payload.WrittenMemory));
    }

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
        TState state = DecodeAndMigrate(info.SchemaVersion, payload);
        return new ProductStateLoad<TState>(true, info.Revision, state);
    }

    public void Dispose() => _store.Dispose();

    private TState DecodeAndMigrate(uint storedSchemaVersion, byte[] payload)
    {
        uint version = storedSchemaVersion;
        int remainingEdges = _migrations.Count;
        while (version != _codec.SchemaVersion)
        {
            if (remainingEdges-- == 0)
            {
                throw new InvalidOperationException($"No finite migration path from schema {storedSchemaVersion} to {_codec.SchemaVersion}.");
            }

            IProductStateMigration? edge = _migrations.SingleOrDefault(candidate => candidate.FromSchemaVersion == version);
            if (edge is null || edge.ToSchemaVersion == version)
            {
                throw new InvalidOperationException($"No migration from schema {version} to {_codec.SchemaVersion}.");
            }

            payload = edge.Migrate(payload) ?? throw new InvalidOperationException("A product migration returned null payload bytes.");
            version = edge.ToSchemaVersion;
        }
        return _codec.Decode(payload);
    }
}

/// <summary>
/// Product composition that checkpoints an in-process <see cref="EntityWorld"/>
/// only through product-supplied capture/restore delegates. It intentionally
/// adds no native entity authority and no generic entity serialization schema.
/// </summary>
public sealed class EntityWorldProductStateStore<TState> : IDisposable
{
    private readonly EntityWorld _world;
    private readonly Func<EntityWorld, TState> _capture;
    private readonly Action<EntityWorld, TState> _restore;
    private readonly ProductStateStore<TState> _state;

    public EntityWorldProductStateStore(
        EntityWorld world,
        Func<EntityWorld, TState> capture,
        Action<EntityWorld, TState> restore,
        IEngineContext engine,
        string root,
        IProductStateCodec<TState> codec,
        IReadOnlyList<IProductStateMigration>? migrations = null)
    {
        _world = world ?? throw new ArgumentNullException(nameof(world));
        _capture = capture ?? throw new ArgumentNullException(nameof(capture));
        _restore = restore ?? throw new ArgumentNullException(nameof(restore));
        _state = new ProductStateStore<TState>(engine, root, codec, migrations);
    }

    public PersistenceSaveReceipt Save(string key, PersistenceRevisionGuard guard = PersistenceRevisionGuard.Any, ulong expectedRevision = 0) =>
        _state.Save(key, _capture(_world), guard, expectedRevision);

    public ProductStateLoad<TState> LoadAndRestore(string key)
    {
        ProductStateLoad<TState> loaded = _state.Load(key);
        if (loaded.Present && loaded.State is not null)
        {
            _restore(_world, loaded.State);
        }
        return loaded;
    }

    public void Dispose() => _state.Dispose();
}
