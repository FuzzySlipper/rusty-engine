using System.Numerics;
using Rusty.Engine;

namespace Rusty.Engine.Entities;

/// <summary>
/// Local-space collision facts for one canonical managed entity. The generated Spatial value
/// carries an entity id, but that id is deliberately omitted here: <see cref="EntityStore"/>
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

/// <summary>
/// A copied result from one trigger reconciliation, with every fact it produced.
/// </summary>
public readonly record struct EntityTriggerProjectionReconcileReceipt(
    SpatialTriggerReconcileResult Trigger,
    ReadOnlyMemory<SpatialEntityCollider> Entities)
{
    /// <summary>Every enter and exit edge the reconciliation produced.</summary>
    public ReadOnlyMemory<SpatialTriggerFact> Facts => Trigger.Facts;
}

/// <summary>
/// Explicitly projects the managed Transform and SpatialCollider built-ins into one generated
/// Spatial trigger reconciliation. It is a call-time projection only; it retains no second
/// spatial world or product component mirror.
/// </summary>
public sealed class EntityTriggerProjection
{
    private readonly EntityStore _entities;
    private readonly ISpatialService _spatial;
    private readonly SpatialSession _session;
    private readonly ComponentType<SpatialCollider> _colliders;

    public EntityTriggerProjection(
        EntityStore entities,
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
    /// Projects the active Transform/collider entities as one generated batch and reads back
    /// every trigger fact the reconciliation produced.
    /// </summary>
    public EntityTriggerProjectionReconcileReceipt ReconcileTriggers(ulong tick, SpatialTriggerCause cause)
    {
        IReadOnlyList<EntityComponents<Transform, SpatialCollider>> joined = _entities.Query(
            EngineComponentTypes.Transform,
            _colliders);
        var projected = new SpatialEntityCollider[joined.Count];
        for (int index = 0; index < joined.Count; index++)
        {
            EntityComponents<Transform, SpatialCollider> row = joined[index];
            projected[index] = Project(row.Entity, row.First, row.Second);
        }

        SpatialTriggerReconcileResult trigger = _spatial.ReconcileTriggers(
            new SpatialTriggerReconcileRequest(_session, tick, cause, projected));
        return new EntityTriggerProjectionReconcileReceipt(trigger, projected);
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
