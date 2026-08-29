using Rusty.Engine;

namespace Rusty.Engine.Entities;

/// <summary>Exact managed revision evidence for one root supplied to WorldOrigin.</summary>
public readonly record struct WorldOriginEntityWorldComponentGuard(
    EntityId Entity,
    ComponentRevision TransformRevision,
    ComponentRevision GlobalPositionRevision);

/// <summary>
/// A deterministic product-world snapshot guard. The native WorldOrigin
/// service owns only origin and collision-scene guards; this guard remains
/// managed because <see cref="EntityWorld"/> is the product's canonical state.
/// </summary>
public readonly record struct WorldOriginEntityWorldGuard(
    ulong WorldRevision,
    ReadOnlyMemory<WorldOriginEntityWorldComponentGuard> Components);

/// <summary>Copied bounded facts from a prepared rebase before either owner publishes it.</summary>
public readonly record struct WorldOriginEntityWorldPrepareReceipt(
    WorldOriginPreparedReadout Native,
    WorldOriginEntityWorldGuard Guard,
    ReadOnlyMemory<WorldOriginAffectedAtReceipt> Affected);

/// <summary>One paired native-origin and managed-transform publication result.</summary>
public readonly record struct WorldOriginEntityWorldCommitReceipt(
    WorldOriginCommitReceipt Native,
    EntityBatchReceipt Managed);

/// <summary>
/// Explicitly composes product-owned <see cref="EntityWorld"/> transform and
/// global-position facts with the generated Engine WorldOrigin service. It is
/// a call-time projection only: global-position policy remains in C#, and no
/// native entity-world mirror is retained.
/// </summary>
public sealed class WorldOriginEntityWorld
{
    private const int MinimumMaximumEntities = 1;

    private readonly EntityWorld _entities;
    private readonly IWorldOriginService _worldOrigins;
    private readonly SpatialSession _session;
    private readonly ComponentType<WorldOriginGlobalPosition> _globalPositions;

    public WorldOriginEntityWorld(
        EntityWorld entities,
        IWorldOriginService worldOrigins,
        SpatialSession session,
        ComponentType<WorldOriginGlobalPosition> globalPositions)
    {
        _entities = entities ?? throw new ArgumentNullException(nameof(entities));
        _worldOrigins = worldOrigins ?? throw new ArgumentNullException(nameof(worldOrigins));
        _session = session ?? throw new ArgumentNullException(nameof(session));
        _globalPositions = globalPositions ?? throw new ArgumentNullException(nameof(globalPositions));
    }

    /// <summary>
    /// Captures every active Transform/global-position root in deterministic
    /// entity order and asks Engine to prepare, but not publish, a rebased
    /// origin and collision scene. Product code chooses when and where to
    /// rebase by passing the target cell explicitly.
    /// </summary>
    public WorldOriginEntityWorldPrepared Prepare(
        long targetCellX,
        long targetCellY,
        long targetCellZ,
        int maximumEntities,
        WorldOriginEntityWorldGuard? expectedGuard = null)
    {
        if (maximumEntities < MinimumMaximumEntities)
        {
            throw new ArgumentOutOfRangeException(nameof(maximumEntities));
        }

        WorldOriginReadout origin = _worldOrigins.Read(new WorldOriginReadRequest(_session));
        ulong worldRevision = _entities.Revision;
        if (expectedGuard is WorldOriginEntityWorldGuard expected && expected.WorldRevision != worldRevision)
        {
            throw new InvalidOperationException(
                $"WorldOrigin managed world revision is stale: expected {expected.WorldRevision}, actual {worldRevision}.");
        }

        IReadOnlyList<EntityComponents<Transform, WorldOriginGlobalPosition>> joined = _entities.Query(
            EngineComponentTypes.Transform,
            _globalPositions);
        if (joined.Count > maximumEntities)
        {
            throw new InvalidOperationException(
                $"WorldOrigin has {joined.Count} roots, exceeding its explicit batch bound {maximumEntities}.");
        }

        var rows = new WorldOriginEntityRow[joined.Count];
        var guards = new WorldOriginEntityWorldComponentGuard[joined.Count];
        for (int index = 0; index < joined.Count; index++)
        {
            EntityComponents<Transform, WorldOriginGlobalPosition> row = joined[index];
            rows[index] = new WorldOriginEntityRow(row.Entity.Value, row.First, row.Second);
            guards[index] = new WorldOriginEntityWorldComponentGuard(
                row.Entity,
                _entities.GetComponentRevision(row.Entity, EngineComponentTypes.Transform),
                _entities.GetComponentRevision(row.Entity, _globalPositions));
        }

        var guard = new WorldOriginEntityWorldGuard(worldRevision, guards);
        if (expectedGuard is WorldOriginEntityWorldGuard supplied)
        {
            ValidateGuard(supplied, guard);
        }

        WorldOriginPrepared native = _worldOrigins.Prepare(new WorldOriginPrepareRequest(
            _session,
            origin.Revision,
            origin.VoxelSourceRevision,
            origin.StaticMeshRevision,
            targetCellX,
            targetCellY,
            targetCellZ,
            rows));
        try
        {
            WorldOriginPreparedReadout summary = _worldOrigins.ReadPrepared(
                new WorldOriginPreparedReadRequest(native));
            if (!summary.Present || summary.AffectedEntityCount != (uint)rows.Length)
            {
                throw new InvalidOperationException("WorldOrigin prepared facts did not preserve the complete managed root set.");
            }

            var affected = new WorldOriginAffectedAtReceipt[rows.Length];
            for (uint index = 0; index < (uint)affected.Length; index++)
            {
                WorldOriginAffectedAtReceipt fact = _worldOrigins.ReadAffectedAt(
                    new WorldOriginAffectedAtRequest(native, index));
                if (!fact.Present || fact.EntityId != rows[index].EntityId)
                {
                    throw new InvalidOperationException("WorldOrigin affected-fact order does not match the deterministic managed root set.");
                }
                affected[index] = fact;
            }
            return new WorldOriginEntityWorldPrepared(this, native, new WorldOriginEntityWorldPrepareReceipt(summary, guard, affected));
        }
        catch
        {
            native.Dispose();
            throw;
        }
    }

    internal WorldOriginEntityWorldCommitReceipt CommitPrepared(
        WorldOriginPrepared native,
        WorldOriginEntityWorldPrepareReceipt prepared)
    {
        // Recheck all product-owned facts immediately before the native call.
        // PrepareBatch then evaluates every managed mutation and validator into
        // a detached state, leaving no fallible action after Engine commits.
        ValidateGuard(prepared.Guard, CaptureGuard());
        EntityWorldBatchCandidate managed = _entities.PrepareBatch(
            TransformBatch(prepared.Affected.Span, prepared.Guard.Components.Span),
            prepared.Guard.WorldRevision);
        WorldOriginCommitReceipt nativeReceipt = _worldOrigins.Commit(new WorldOriginCommitRequest(native));
        managed.Publish();
        return new WorldOriginEntityWorldCommitReceipt(nativeReceipt, managed.Receipt);
    }

    private EntityBatch TransformBatch(
        ReadOnlySpan<WorldOriginAffectedAtReceipt> affected,
        ReadOnlySpan<WorldOriginEntityWorldComponentGuard> guards)
    {
        if (affected.Length != guards.Length)
        {
            throw new InvalidOperationException("WorldOrigin prepared transform facts no longer match their component guards.");
        }
        var batch = new EntityBatch();
        for (int index = 0; index < affected.Length; index++)
        {
            WorldOriginAffectedAtReceipt fact = affected[index];
            WorldOriginEntityWorldComponentGuard guard = guards[index];
            if (!fact.Present || fact.EntityId != guard.Entity.Value)
            {
                throw new InvalidOperationException("WorldOrigin prepared transform facts no longer match their managed entities.");
            }
            batch.Mutate(world => world.Set(
                guard.Entity,
                EngineComponentTypes.Transform,
                fact.LocalTransform,
                guard.TransformRevision));
        }
        return batch;
    }

    private WorldOriginEntityWorldGuard CaptureGuard()
    {
        IReadOnlyList<EntityComponents<Transform, WorldOriginGlobalPosition>> joined = _entities.Query(
            EngineComponentTypes.Transform,
            _globalPositions);
        var guards = new WorldOriginEntityWorldComponentGuard[joined.Count];
        for (int index = 0; index < joined.Count; index++)
        {
            EntityComponents<Transform, WorldOriginGlobalPosition> row = joined[index];
            guards[index] = new WorldOriginEntityWorldComponentGuard(
                row.Entity,
                _entities.GetComponentRevision(row.Entity, EngineComponentTypes.Transform),
                _entities.GetComponentRevision(row.Entity, _globalPositions));
        }
        return new WorldOriginEntityWorldGuard(_entities.Revision, guards);
    }

    private static void ValidateGuard(
        WorldOriginEntityWorldGuard expected,
        WorldOriginEntityWorldGuard observed)
    {
        if (expected.WorldRevision != observed.WorldRevision)
        {
            throw new InvalidOperationException(
                $"WorldOrigin managed world revision is stale: expected {expected.WorldRevision}, actual {observed.WorldRevision}.");
        }
        ReadOnlySpan<WorldOriginEntityWorldComponentGuard> expectedComponents = expected.Components.Span;
        ReadOnlySpan<WorldOriginEntityWorldComponentGuard> observedComponents = observed.Components.Span;
        if (expectedComponents.Length != observedComponents.Length)
        {
            throw new InvalidOperationException("WorldOrigin managed root set is stale.");
        }
        for (int index = 0; index < expectedComponents.Length; index++)
        {
            if (expectedComponents[index] != observedComponents[index])
            {
                throw new InvalidOperationException(
                    $"WorldOrigin managed component revision is stale for entity {observedComponents[index].Entity.Value}.");
            }
        }
    }
}

/// <summary>
/// Owns one native prepared WorldOrigin handle plus copied managed evidence.
/// Disposing before <see cref="Commit"/> cancels the Engine candidate without
/// changing either live owner.
/// </summary>
public sealed class WorldOriginEntityWorldPrepared : IDisposable
{
    private readonly WorldOriginEntityWorld _owner;
    private WorldOriginPrepared? _native;

    internal WorldOriginEntityWorldPrepared(
        WorldOriginEntityWorld owner,
        WorldOriginPrepared native,
        WorldOriginEntityWorldPrepareReceipt receipt)
    {
        _owner = owner;
        _native = native;
        Receipt = receipt;
    }

    public WorldOriginEntityWorldPrepareReceipt Receipt { get; }

    /// <summary>
    /// Commits Engine's prepared origin/scene, then assigns an already
    /// validated managed transform candidate. The adapter's synchronous
    /// contract forbids concurrent EntityWorld mutation during this call.
    /// </summary>
    public WorldOriginEntityWorldCommitReceipt Commit()
    {
        WorldOriginPrepared native = _native
            ?? throw new ObjectDisposedException(nameof(WorldOriginEntityWorldPrepared));
        WorldOriginEntityWorldCommitReceipt receipt = _owner.CommitPrepared(native, Receipt);
        native.Dispose();
        _native = null;
        return receipt;
    }

    public void Dispose()
    {
        WorldOriginPrepared? native = Interlocked.Exchange(ref _native, null);
        native?.Dispose();
    }
}
