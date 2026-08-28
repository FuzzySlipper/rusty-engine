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
/// The constructor receives a relative product scope; the developer host
/// selects the absolute persistence root before product creation.
/// </summary>
public sealed class ProductStateStore<TState> : IDisposable
{
    private readonly IPersistenceService _persistence;
    private readonly PersistenceStore _store;
    private readonly IProductStateCodec<TState> _codec;
    private readonly IReadOnlyList<IProductStateMigration> _migrations;

    public ProductStateStore(
        IEngineContext engine,
        string scope,
        IProductStateCodec<TState> codec,
        IReadOnlyList<IProductStateMigration>? migrations = null)
    {
        ArgumentNullException.ThrowIfNull(engine);
        ArgumentException.ThrowIfNullOrWhiteSpace(scope);
        _codec = codec ?? throw new ArgumentNullException(nameof(codec));
        _migrations = migrations ?? [];
        _persistence = engine.Persistence;
        _store = _persistence.OpenStore(new PersistenceOpenRequest(scope));
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
        string scope,
        IProductStateCodec<TState> codec,
        IReadOnlyList<IProductStateMigration>? migrations = null)
    {
        _world = world ?? throw new ArgumentNullException(nameof(world));
        _capture = capture ?? throw new ArgumentNullException(nameof(capture));
        _restore = restore ?? throw new ArgumentNullException(nameof(restore));
        _state = new ProductStateStore<TState>(engine, scope, codec, migrations);
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

/// <summary>
/// Product-decoded transient state for one paired managed Entity and Mechanics import.
/// This is deliberately not an archive record: the product selects the durable state type,
/// codec, migrations, and the mapping that constructs these typed Engine inputs.
/// </summary>
public sealed class MechanicsProductStateRestorePlan
{
    public MechanicsProductStateRestorePlan(
        EntityWorldRestorePlan entities,
        MechanicsWorldImportRequest mechanics)
    {
        Entities = entities ?? throw new ArgumentNullException(nameof(entities));
        Mechanics = mechanics;
    }

    public EntityWorldRestorePlan Entities { get; }

    public MechanicsWorldImportRequest Mechanics { get; }
}

/// <summary>
/// Product-owned durable state composition for an admitted <see cref="MechanicsEntityWorld"/>.
/// The product owns <typeparamref name="TState"/>, its codec/migrations, and both mapping
/// delegates. This class only coordinates opaque Persistence bytes with the existing typed,
/// paired Engine prepare/publish mechanism.
/// </summary>
public sealed class MechanicsEntityWorldProductStateStore<TState> : IDisposable
{
    private readonly MechanicsEntityWorld _world;
    private readonly ProductStateStore<TState> _state;
    private readonly Func<MechanicsWorldExportLeaseReceipt, TState> _capture;
    private readonly Func<TState, MechanicsProductStateRestorePlan> _restore;

    public MechanicsEntityWorldProductStateStore(
        MechanicsEntityWorld world,
        IEngineContext engine,
        string scope,
        IProductStateCodec<TState> codec,
        Func<MechanicsWorldExportLeaseReceipt, TState> capture,
        Func<TState, MechanicsProductStateRestorePlan> restore,
        IReadOnlyList<IProductStateMigration>? migrations = null)
    {
        _world = world ?? throw new ArgumentNullException(nameof(world));
        ArgumentNullException.ThrowIfNull(engine);
        ArgumentException.ThrowIfNullOrWhiteSpace(scope);
        ArgumentNullException.ThrowIfNull(codec);
        _capture = capture ?? throw new ArgumentNullException(nameof(capture));
        _restore = restore ?? throw new ArgumentNullException(nameof(restore));
        _state = new ProductStateStore<TState>(engine, scope, codec, migrations);
    }

    /// <summary>
    /// Publishes the current copied typed Mechanics export to the product capture callback, then
    /// persists only the product-selected opaque state.
    /// </summary>
    public PersistenceSaveReceipt Save(
        string key,
        PersistenceRevisionGuard guard = PersistenceRevisionGuard.Any,
        ulong expectedRevision = 0)
    {
        MechanicsWorldExportLeaseReceipt export = _world.Export();
        return _state.Save(key, _capture(export), guard, expectedRevision);
    }

    /// <summary>
    /// Loads product-owned state and prepares the paired managed/native import without changing
    /// either live world. Dispose the returned result to cancel; call its explicit
    /// <see cref="MechanicsEntityWorldProductStateLoad{TState}.Publish"/> to commit.
    /// </summary>
    public MechanicsEntityWorldProductStateLoad<TState> LoadPrepared(
        string key,
        ulong? expectedManagedRevision = null)
    {
        ProductStateLoad<TState> loaded = _state.Load(key);
        if (!loaded.Present)
        {
            return new MechanicsEntityWorldProductStateLoad<TState>(loaded, null);
        }

        MechanicsProductStateRestorePlan plan = _restore(loaded.State!)
            ?? throw new InvalidOperationException("The product restore mapper returned no paired Mechanics plan.");
        MechanicsEntityWorldImportCandidate candidate = _world.PrepareImport(
            plan.Entities,
            plan.Mechanics,
            expectedManagedRevision);
        return new MechanicsEntityWorldProductStateLoad<TState>(loaded, candidate);
    }

    /// <summary>Convenience for products that do not need to inspect a prepared candidate.</summary>
    public ProductStateLoad<TState> LoadAndPublish(string key, ulong? expectedManagedRevision = null)
    {
        using var loaded = LoadPrepared(key, expectedManagedRevision);
        loaded.Publish();
        return new ProductStateLoad<TState>(loaded.Present, loaded.PersistenceRevision, loaded.State);
    }

    public void Dispose() => _state.Dispose();
}

/// <summary>
/// One loaded product state and, when present, its already-prepared paired Engine candidate.
/// Publication never invokes product code: mapping and validation have already completed.
/// </summary>
public sealed class MechanicsEntityWorldProductStateLoad<TState> : IDisposable
{
    private MechanicsEntityWorldImportCandidate? _candidate;
    private int _published;

    internal MechanicsEntityWorldProductStateLoad(
        ProductStateLoad<TState> loaded,
        MechanicsEntityWorldImportCandidate? candidate)
    {
        Present = loaded.Present;
        PersistenceRevision = loaded.Revision;
        State = loaded.State;
        _candidate = candidate;
    }

    public bool Present { get; }

    public ulong PersistenceRevision { get; }

    public TState? State { get; }

    /// <summary>Copied Engine evidence for a present, successfully prepared import.</summary>
    public MechanicsWorldImportLeaseReceipt? PreparedImport => _candidate?.Receipt;

    /// <summary>
    /// Commits the already validated pair. It is idempotent; an absent load is a no-op.
    /// </summary>
    public void Publish()
    {
        if (!Present || Interlocked.Exchange(ref _published, 1) != 0)
        {
            return;
        }

        MechanicsEntityWorldImportCandidate candidate = _candidate
            ?? throw new ObjectDisposedException(nameof(MechanicsEntityWorldProductStateLoad<TState>));
        candidate.Publish();
    }

    /// <summary>Cancels a present candidate that has not yet been published.</summary>
    public void Dispose()
    {
        MechanicsEntityWorldImportCandidate? candidate = Interlocked.Exchange(ref _candidate, null);
        candidate?.Dispose();
    }
}

/// <summary>
/// Product-decoded transient state for the canonical three-part restore: managed entity facts,
/// exact Mechanics facts, and handle-free continuous Mechanics facts. It is not a persistence
/// format; products retain ownership of the durable DTO, codec, and migration policy.
/// </summary>
public sealed class ContinuousMechanicsEntityWorldProductStateRestorePlan
{
    public ContinuousMechanicsEntityWorldProductStateRestorePlan(
        EntityWorldRestorePlan entities,
        MechanicsWorldImportRequest mechanics,
        ContinuousMechanicsWorldImportImage continuous)
    {
        Entities = entities ?? throw new ArgumentNullException(nameof(entities));
        Mechanics = mechanics;
        Continuous = continuous ?? throw new ArgumentNullException(nameof(continuous));
    }

    public EntityWorldRestorePlan Entities { get; }
    public MechanicsWorldImportRequest Mechanics { get; }
    public ContinuousMechanicsWorldImportImage Continuous { get; }
}

/// <summary>
/// Product-owned persistence composition for the canonical paired exact/continuous Mechanics
/// entity world. Product callbacks map before prepare; explicit publication never calls them.
/// </summary>
public sealed class ContinuousMechanicsEntityWorldProductStateStore<TState> : IDisposable
{
    private readonly ContinuousMechanicsEntityWorld _world;
    private readonly ProductStateStore<TState> _state;
    private readonly Func<ContinuousMechanicsEntityWorldExport, TState> _capture;
    private readonly Func<TState, ContinuousMechanicsEntityWorldProductStateRestorePlan> _restore;

    public ContinuousMechanicsEntityWorldProductStateStore(
        ContinuousMechanicsEntityWorld world,
        IEngineContext engine,
        string scope,
        IProductStateCodec<TState> codec,
        Func<ContinuousMechanicsEntityWorldExport, TState> capture,
        Func<TState, ContinuousMechanicsEntityWorldProductStateRestorePlan> restore,
        IReadOnlyList<IProductStateMigration>? migrations = null)
    {
        _world = world ?? throw new ArgumentNullException(nameof(world));
        ArgumentNullException.ThrowIfNull(engine);
        ArgumentException.ThrowIfNullOrWhiteSpace(scope);
        ArgumentNullException.ThrowIfNull(codec);
        _capture = capture ?? throw new ArgumentNullException(nameof(capture));
        _restore = restore ?? throw new ArgumentNullException(nameof(restore));
        _state = new ProductStateStore<TState>(engine, scope, codec, migrations);
    }

    public PersistenceSaveReceipt Save(
        string key,
        PersistenceRevisionGuard guard = PersistenceRevisionGuard.Any,
        ulong expectedRevision = 0)
    {
        ContinuousMechanicsEntityWorldExport export = _world.Export();
        return _state.Save(key, _capture(export), guard, expectedRevision);
    }

    public ContinuousMechanicsEntityWorldProductStateLoad<TState> LoadPrepared(
        string key,
        ulong? expectedManagedRevision = null)
    {
        ProductStateLoad<TState> loaded = _state.Load(key);
        if (!loaded.Present)
        {
            return new ContinuousMechanicsEntityWorldProductStateLoad<TState>(loaded, null);
        }

        ContinuousMechanicsEntityWorldProductStateRestorePlan plan = _restore(loaded.State!)
            ?? throw new InvalidOperationException("The product restore mapper returned no paired continuous Mechanics plan.");
        ContinuousMechanicsEntityWorldImportCandidate candidate = _world.PrepareImport(
            plan.Entities,
            plan.Mechanics,
            plan.Continuous,
            expectedManagedRevision);
        return new ContinuousMechanicsEntityWorldProductStateLoad<TState>(loaded, candidate);
    }

    public ProductStateLoad<TState> LoadAndPublish(string key, ulong? expectedManagedRevision = null)
    {
        using var loaded = LoadPrepared(key, expectedManagedRevision);
        loaded.Publish();
        return new ProductStateLoad<TState>(loaded.Present, loaded.PersistenceRevision, loaded.State);
    }

    public void Dispose() => _state.Dispose();
}

/// <summary>
/// A present decoded product state and its one prepared paired candidate. Dispose cancels the
/// exact candidate; absent state is deliberately an honest no-op.
/// </summary>
public sealed class ContinuousMechanicsEntityWorldProductStateLoad<TState> : IDisposable
{
    private ContinuousMechanicsEntityWorldImportCandidate? _candidate;
    private int _published;

    internal ContinuousMechanicsEntityWorldProductStateLoad(
        ProductStateLoad<TState> loaded,
        ContinuousMechanicsEntityWorldImportCandidate? candidate)
    {
        Present = loaded.Present;
        PersistenceRevision = loaded.Revision;
        State = loaded.State;
        _candidate = candidate;
    }

    public bool Present { get; }
    public ulong PersistenceRevision { get; }
    public TState? State { get; }
    public MechanicsWorldImportLeaseReceipt? PreparedExactImport => _candidate?.ExactReceipt;
    public ContinuousMechanicsWorldImportLeaseReceipt? PreparedContinuousImport => _candidate?.ContinuousReceipt;

    public void Publish()
    {
        if (!Present || Interlocked.Exchange(ref _published, 1) != 0)
        {
            return;
        }

        ContinuousMechanicsEntityWorldImportCandidate candidate = _candidate
            ?? throw new ObjectDisposedException(nameof(ContinuousMechanicsEntityWorldProductStateLoad<TState>));
        candidate.Publish();
    }

    public void Dispose()
    {
        ContinuousMechanicsEntityWorldImportCandidate? candidate = Interlocked.Exchange(ref _candidate, null);
        candidate?.Dispose();
    }
}
