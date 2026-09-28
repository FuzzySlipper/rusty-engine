using System.Numerics;
using Rusty.Engine;

namespace Rusty.Engine.Entities;

/// <summary>
/// Product-owned bounds and velocity projected into one call-local Kinematic
/// motion phase. It is distinct from the generated detached Physics
/// <c>KinematicBody</c> value.
/// </summary>
public readonly record struct Kinematic(Vector3 HalfExtents, Vector3 Velocity);

/// <summary>One Kinematic motion phase and the managed writes that applied it.</summary>
public readonly record struct EntityKinematicMotionReceipt(
    KinematicMotionLeaseReceipt Motion,
    EntityBatchReceipt Managed);

/// <summary>
/// Projects active managed Transform and Kinematic values into the generated
/// Kinematic motion family. Optional SpatialCollider values control dynamic
/// blocking; an absent collider truthfully becomes disabled for that call.
/// </summary>
public sealed class EntityKinematicMotion
{
    private readonly EntityStore _entities;
    private readonly IKinematicService _kinematic;
    private readonly ComponentType<SpatialCollider> _colliders;

    public EntityKinematicMotion(
        EntityStore entities,
        IKinematicService kinematic,
        ComponentType<SpatialCollider> colliders)
    {
        _entities = entities ?? throw new ArgumentNullException(nameof(entities));
        _kinematic = kinematic ?? throw new ArgumentNullException(nameof(kinematic));
        _colliders = colliders ?? throw new ArgumentNullException(nameof(colliders));
    }

    /// <summary>
    /// Runs one Kinematic motion phase over every active Transform/Kinematic entity and
    /// writes the changed Transform and Kinematic values. A null selection runs every
    /// projected body; an empty selection is an explicit selected phase that advances none.
    /// </summary>
    public EntityKinematicMotionReceipt Step(
        SpatialSession session,
        float deltaSeconds,
        ReadOnlyMemory<EntityId>? selection = null)
    {
        ArgumentNullException.ThrowIfNull(session);
        IReadOnlyList<EntityComponents<Transform, Kinematic>> joined = _entities.Query(
            EngineComponentTypes.Transform,
            EngineComponentTypes.Kinematic);
        var rows = new KinematicMotionEntityRow[joined.Count];
        var halfExtents = new Dictionary<ulong, Vector3>(joined.Count);
        for (int index = 0; index < joined.Count; index++)
        {
            (EntityId entity, Transform transform, Kinematic kinematic) = joined[index];
            bool collisionEnabled = _entities.TryGet(entity, _colliders, out SpatialCollider collider) && collider.Enabled;
            rows[index] = new KinematicMotionEntityRow(
                entity.Value,
                transform,
                kinematic.HalfExtents,
                kinematic.Velocity,
                collisionEnabled,
                collisionEnabled && collider.StaticCollider);
            halfExtents.Add(entity.Value, kinematic.HalfExtents);
        }
        ulong[] selectedIds = selection is ReadOnlyMemory<EntityId> selected
            ? selected.ToArray().Select(entity => entity.Value).ToArray()
            : [];
        KinematicMotionLeaseReceipt motion = _kinematic.RunMotion(new KinematicMotionRequest(
            session,
            deltaSeconds,
            rows,
            selection.HasValue,
            selectedIds));

        var batch = new EntityBatch();
        foreach (KinematicMotionCandidate candidate in motion.Candidates.Span)
        {
            EntityId entity = new(candidate.EntityId);
            if (candidate.BeforeTransform != candidate.AfterTransform)
            {
                batch.Set(entity, EngineComponentTypes.Transform, candidate.AfterTransform);
            }
            if (candidate.BeforeVelocity != candidate.AfterVelocity)
            {
                batch.Set(entity, EngineComponentTypes.Kinematic,
                    new Kinematic(halfExtents[candidate.EntityId], candidate.AfterVelocity));
            }
        }
        return new EntityKinematicMotionReceipt(motion, _entities.Commit(batch));
    }
}
