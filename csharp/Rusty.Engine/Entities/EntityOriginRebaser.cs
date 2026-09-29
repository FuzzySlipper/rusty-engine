using System.Numerics;
using Rusty.Engine;

namespace Rusty.Engine.Entities;

/// <summary>One paired native-origin and managed-transform publication result.</summary>
public readonly record struct EntityOriginRebaserCommitReceipt(
    WorldOriginCommitReceipt Native,
    EntityBatchReceipt Managed);

/// <summary>
/// Explicitly composes product-owned <see cref="EntityStore"/> transform and
/// global-position facts with the generated Engine WorldOrigin service. It is
/// a call-time projection only: global-position policy remains in C#, and no
/// native entity-world mirror is retained.
/// </summary>
public sealed class EntityOriginRebaser
{
    private readonly EntityStore _entities;
    private readonly IWorldOriginService _worldOrigins;
    private readonly SpatialSession _session;
    private readonly ComponentType<WorldOriginGlobalPosition> _globalPositions;

    public EntityOriginRebaser(
        EntityStore entities,
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
    /// Captures every active Transform/global-position root and asks Engine to
    /// prepare their rebased local transforms. Commit moves the origin and
    /// rebases the live collision scene. Product code chooses when and where to
    /// rebase by passing the target cell explicitly.
    /// </summary>
    public EntityOriginRebaserPrepared Prepare(long targetCellX, long targetCellY, long targetCellZ)
    {
        IReadOnlyList<EntityComponents<Transform, WorldOriginGlobalPosition>> joined = _entities.Query(
            EngineComponentTypes.Transform,
            _globalPositions);
        var rows = new WorldOriginEntityRow[joined.Count];
        for (int index = 0; index < joined.Count; index++)
        {
            EntityComponents<Transform, WorldOriginGlobalPosition> row = joined[index];
            rows[index] = new WorldOriginEntityRow(row.Entity.Value, row.First, row.Second);
        }

        WorldOriginPrepared native = _worldOrigins.Prepare(new WorldOriginPrepareRequest(
            _session,
            targetCellX,
            targetCellY,
            targetCellZ,
            rows));
        try
        {
            WorldOriginPreparedResult prepared = _worldOrigins.ReadPrepared(new WorldOriginPreparedReadRequest(native));
            return new EntityOriginRebaserPrepared(this, native, prepared);
        }
        catch
        {
            native.Dispose();
            throw;
        }
    }

    internal EntityOriginRebaserCommitReceipt CommitPrepared(
        WorldOriginPrepared native,
        WorldOriginPreparedResult prepared)
    {
        WorldOriginCommitReceipt nativeReceipt = _worldOrigins.Commit(new WorldOriginCommitRequest(native));
        var batch = new EntityBatch();
        foreach (WorldOriginAffectedTransform fact in prepared.Affected.Span)
        {
            batch.Set(new EntityId(fact.EntityId), EngineComponentTypes.Transform, fact.LocalTransform);
        }
        // Stored character motion holds local-frame anchors and heights; without
        // this the next step carries the character back by the whole delta.
        Vector3 delta = nativeReceipt.LocalDelta;
        foreach ((EntityId entity, CharacterMotion motion) in _entities.Query<CharacterMotion>(includeDisabled: true))
        {
            batch.Set(entity, EngineComponentTypes.CharacterMotion, motion.Rebased(delta));
        }
        return new EntityOriginRebaserCommitReceipt(nativeReceipt, _entities.Commit(batch));
    }
}

/// <summary>
/// Owns one native prepared WorldOrigin handle plus copied managed evidence.
/// Disposing before <see cref="Commit"/> cancels the Engine candidate without
/// changing either live owner.
/// </summary>
public sealed class EntityOriginRebaserPrepared : IDisposable
{
    private readonly EntityOriginRebaser _owner;
    private WorldOriginPrepared? _native;

    internal EntityOriginRebaserPrepared(
        EntityOriginRebaser owner,
        WorldOriginPrepared native,
        WorldOriginPreparedResult receipt)
    {
        _owner = owner;
        _native = native;
        Receipt = receipt;
    }

    /// <summary>The copied target and rebased root transforms, before either owner applies them.</summary>
    public WorldOriginPreparedResult Receipt { get; }

    /// <summary>
    /// Commits Engine's prepared origin/scene, then writes the rebased transforms
    /// and the rebased stored character motion.
    /// </summary>
    public EntityOriginRebaserCommitReceipt Commit()
    {
        using WorldOriginPrepared native = Interlocked.Exchange(ref _native, null)
            ?? throw new ObjectDisposedException(nameof(EntityOriginRebaserPrepared));
        return _owner.CommitPrepared(native, Receipt);
    }

    public void Dispose()
    {
        WorldOriginPrepared? native = Interlocked.Exchange(ref _native, null);
        native?.Dispose();
    }
}
