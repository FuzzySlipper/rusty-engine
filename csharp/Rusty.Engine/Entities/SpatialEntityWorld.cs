using System.Numerics;
using Rusty.Engine;

namespace Rusty.Engine.Entities;

/// <summary>
/// Local-space collision facts for one canonical managed entity. The generated Spatial value
/// carries an entity id, but that id is deliberately omitted here: <see cref="EntityWorld"/>
/// supplies it when a named Spatial projection is requested.
/// </summary>
public readonly record struct SpatialCollider(
    Vector3 Min,
    Vector3 Max,
    uint CollisionGroup,
    uint CollisionMask,
    bool Enabled,
    bool StaticCollider,
    bool Trigger);

/// <summary>One exact managed-state guard for a Spatial projection row.</summary>
public readonly record struct SpatialEntityWorldComponentGuard(
    EntityId Entity,
    ComponentRevision TransformRevision,
    ComponentRevision ColliderRevision);

/// <summary>
/// Copied managed revision evidence for a Spatial projection. Supplying it on a later call
/// rejects a changed world or changed participating component before crossing into Spatial.
/// </summary>
public readonly record struct SpatialEntityWorldGuard(
    ulong WorldRevision,
    ReadOnlyMemory<SpatialEntityWorldComponentGuard> Components);

/// <summary>
/// A copied result from one coherent trigger reconciliation. Facts are read immediately from the
/// generated bounded indexed readback while the reconciliation result is still current.
/// </summary>
public readonly record struct SpatialEntityWorldReconcileReceipt(
    SpatialTriggerReceipt Trigger,
    SpatialEntityWorldGuard Guard,
    ReadOnlyMemory<SpatialEntityCollider> Entities,
    ReadOnlyMemory<SpatialTriggerFactAtReceipt> Facts,
    bool FactsTruncated);

/// <summary>
/// Explicitly projects the managed Transform and SpatialCollider built-ins into one generated
/// Spatial trigger reconciliation. It is a call-time projection only; it retains no second
/// spatial world or product component mirror.
/// </summary>
public sealed class SpatialEntityWorld
{
    private readonly EntityWorld _entities;
    private readonly ISpatialService _spatial;
    private readonly SpatialSession _session;
    private readonly ComponentType<SpatialCollider> _colliders;

    public SpatialEntityWorld(
        EntityWorld entities,
        ISpatialService spatial,
        SpatialSession session,
        ComponentType<SpatialCollider> colliders)
    {
        _entities = entities ?? throw new ArgumentNullException(nameof(entities));
        _spatial = spatial ?? throw new ArgumentNullException(nameof(spatial));
        _session = session ?? throw new ArgumentNullException(nameof(session));
        _colliders = colliders ?? throw new ArgumentNullException(nameof(colliders));
    }

    /// <summary>
    /// Projects the deterministic active Transform/collider join as one generated batch. The
    /// caller supplies explicit bounds for both admission and copied fact readback; oversized
    /// projections are rejected before the native call instead of silently becoming partial.
    /// </summary>
    public SpatialEntityWorldReconcileReceipt ReconcileTriggers(
        ulong tick,
        SpatialTriggerCause cause,
        int maximumEntities,
        int maximumFactReadback,
        SpatialEntityWorldGuard? expectedGuard = null)
    {
        if (maximumEntities < 1)
        {
            throw new ArgumentOutOfRangeException(nameof(maximumEntities));
        }
        if (maximumFactReadback < 0)
        {
            throw new ArgumentOutOfRangeException(nameof(maximumFactReadback));
        }

        ulong worldRevision = _entities.Revision;
        if (expectedGuard is SpatialEntityWorldGuard expected && expected.WorldRevision != worldRevision)
        {
            throw new InvalidOperationException(
                $"Spatial projection world revision is stale: expected {expected.WorldRevision}, actual {worldRevision}.");
        }

        IReadOnlyList<EntityComponents<Transform, SpatialCollider>> joined = _entities.Query(
            EngineComponentTypes.Transform,
            _colliders);
        if (joined.Count > maximumEntities)
        {
            throw new InvalidOperationException(
                $"Spatial projection has {joined.Count} entities, exceeding its explicit batch bound {maximumEntities}.");
        }

        var projected = new SpatialEntityCollider[joined.Count];
        var guards = new SpatialEntityWorldComponentGuard[joined.Count];
        for (int index = 0; index < joined.Count; index++)
        {
            EntityComponents<Transform, SpatialCollider> row = joined[index];
            projected[index] = Project(row.Entity, row.First, row.Second);
            guards[index] = new SpatialEntityWorldComponentGuard(
                row.Entity,
                _entities.GetComponentRevision(row.Entity, EngineComponentTypes.Transform),
                _entities.GetComponentRevision(row.Entity, _colliders));
        }

        if (expectedGuard is SpatialEntityWorldGuard supplied)
        {
            ValidateComponentGuards(supplied, guards);
        }

        // ReconcileTriggers owns atomic trigger-state publication. The managed side is only the
        // immutable call input, so all stale checks happen before that single service crossing.
        SpatialTriggerReceipt trigger = _spatial.ReconcileTriggers(
            new SpatialTriggerReconcileRequest(_session, tick, cause, projected));
        int factCount = checked((int)trigger.FactCount);
        int readCount = Math.Min(factCount, maximumFactReadback);
        var facts = new SpatialTriggerFactAtReceipt[readCount];
        for (uint index = 0; index < (uint)readCount; index++)
        {
            facts[index] = _spatial.ReadTriggerFactAt(new SpatialTriggerFactAtRequest(_session, index));
        }

        return new SpatialEntityWorldReconcileReceipt(
            trigger,
            new SpatialEntityWorldGuard(worldRevision, guards),
            projected,
            facts,
            factCount > readCount);
    }

    private static void ValidateComponentGuards(
        SpatialEntityWorldGuard expected,
        IReadOnlyList<SpatialEntityWorldComponentGuard> observed)
    {
        ReadOnlySpan<SpatialEntityWorldComponentGuard> supplied = expected.Components.Span;
        if (supplied.Length != observed.Count)
        {
            throw new InvalidOperationException("Spatial projection component set is stale.");
        }
        for (int index = 0; index < observed.Count; index++)
        {
            if (supplied[index] != observed[index])
            {
                throw new InvalidOperationException(
                    $"Spatial projection component revision is stale for entity {observed[index].Entity.Value}.");
            }
        }
    }

    private static SpatialEntityCollider Project(EntityId entity, Transform transform, SpatialCollider collider)
    {
        Vector3 min = TransformPoint(collider.Min, transform);
        Vector3 max = min;
        foreach (Vector3 corner in Corners(collider.Min, collider.Max))
        {
            Vector3 point = TransformPoint(corner, transform);
            min = Vector3.Min(min, point);
            max = Vector3.Max(max, point);
        }
        return new SpatialEntityCollider(
            entity.Value,
            min,
            max,
            collider.CollisionGroup,
            collider.CollisionMask,
            collider.Enabled,
            collider.StaticCollider,
            collider.Trigger);
    }

    private static Vector3 TransformPoint(Vector3 point, Transform transform)
        => Vector3.Transform(point * transform.Scale, transform.Rotation) + transform.Translation;

    private static IEnumerable<Vector3> Corners(Vector3 min, Vector3 max)
    {
        yield return new Vector3(min.X, min.Y, min.Z);
        yield return new Vector3(min.X, min.Y, max.Z);
        yield return new Vector3(min.X, max.Y, min.Z);
        yield return new Vector3(min.X, max.Y, max.Z);
        yield return new Vector3(max.X, min.Y, min.Z);
        yield return new Vector3(max.X, min.Y, max.Z);
        yield return new Vector3(max.X, max.Y, min.Z);
        yield return new Vector3(max.X, max.Y, max.Z);
    }
}
