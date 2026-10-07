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
/// A copied result from one trigger reconciliation, with every collider it was given: the
/// projected store entities followed by any caller-supplied subjects.
/// </summary>
public readonly record struct EntityTriggerProjectionReconcileReceipt(
    SpatialTriggerReconcileResult Trigger,
    ReadOnlyMemory<SpatialEntityCollider> Entities)
{
    /// <summary>Every enter and exit edge the reconciliation produced.</summary>
    public ReadOnlyMemory<SpatialTriggerFact> Facts => Trigger.Facts;
}

/// <summary>
/// A copied result from one trigger restore, with every collider the baseline was built from:
/// the projected store entities followed by any caller-supplied subjects.
/// </summary>
public readonly record struct EntityTriggerProjectionRestoreReceipt(
    SpatialTriggerRestoreReceipt Trigger,
    ReadOnlyMemory<SpatialEntityCollider> Entities);

/// <summary>
/// Explicitly projects the managed Transform and SpatialCollider built-ins into one generated
/// Spatial trigger reconciliation. It is a call-time projection only; it retains no second
/// spatial world or product component mirror.
/// </summary>
/// <remarks>
/// The projection of one entity is <see cref="Project"/>, so a product that moves a collider
/// outside the store (a character the movement system owns) projects it the same way and
/// passes it as a subject of <see cref="ReconcileTriggers(ulong, SpatialTriggerCause, ReadOnlySpan{SpatialEntityCollider})"/>
/// or <see cref="RestoreTriggers"/>, instead of keeping a copy of the bounds arithmetic.
/// </remarks>
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
        => ReconcileTriggers(tick, cause, ReadOnlySpan<SpatialEntityCollider>.Empty);

    /// <summary>
    /// Projects the active Transform/collider entities, appends <paramref name="subjects"/>
    /// (world-space colliders the caller projected itself, with ids distinct from every store
    /// entity's), reconciles them as one generated batch and reads back every trigger fact.
    /// </summary>
    public EntityTriggerProjectionReconcileReceipt ReconcileTriggers(
        ulong tick,
        SpatialTriggerCause cause,
        ReadOnlySpan<SpatialEntityCollider> subjects)
    {
        SpatialEntityCollider[] projected = ProjectEntities(subjects);
        SpatialTriggerReconcileResult trigger = _spatial.ReconcileTriggers(
            new SpatialTriggerReconcileRequest(_session, tick, cause, projected));
        return new EntityTriggerProjectionReconcileReceipt(trigger, projected);
    }

    /// <summary>
    /// Replaces the session's active-trigger and overlap baseline from the same projection a
    /// reconcile uses: the active Transform/collider entities plus <paramref name="subjects"/>.
    /// <paramref name="activeTriggers"/> is the complete active set among the registered
    /// triggers. No enter or exit facts are produced.
    /// </summary>
    public EntityTriggerProjectionRestoreReceipt RestoreTriggers(
        ReadOnlyMemory<ulong> activeTriggers,
        ReadOnlySpan<SpatialEntityCollider> subjects)
    {
        SpatialEntityCollider[] projected = ProjectEntities(subjects);
        SpatialTriggerRestoreReceipt trigger = _spatial.RestoreTriggers(
            new SpatialTriggerRestoreRequest(_session, activeTriggers, projected));
        return new EntityTriggerProjectionRestoreReceipt(trigger, projected);
    }

    /// <summary>
    /// The world-space colliders of every active Transform/collider entity, as a reconcile or
    /// restore would be given them.
    /// </summary>
    public SpatialEntityCollider[] ProjectEntities() => ProjectEntities(ReadOnlySpan<SpatialEntityCollider>.Empty);

    /// <summary>
    /// One entity's collider in world space: the local box's eight corners scaled, rotated and
    /// translated by <paramref name="transform"/>, then bounded. The same projection every
    /// reconcile and restore applies to the store's entities.
    /// </summary>
    public static SpatialEntityCollider Project(EntityId entity, Transform transform, SpatialCollider collider)
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

    private SpatialEntityCollider[] ProjectEntities(ReadOnlySpan<SpatialEntityCollider> subjects)
    {
        IReadOnlyList<EntityComponents<Transform, SpatialCollider>> joined = _entities.Query(
            EngineComponentTypes.Transform,
            _colliders);
        var projected = new SpatialEntityCollider[joined.Count + subjects.Length];
        for (int index = 0; index < joined.Count; index++)
        {
            EntityComponents<Transform, SpatialCollider> row = joined[index];
            projected[index] = Project(row.Entity, row.First, row.Second);
        }
        subjects.CopyTo(projected.AsSpan(joined.Count));
        return projected;
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
