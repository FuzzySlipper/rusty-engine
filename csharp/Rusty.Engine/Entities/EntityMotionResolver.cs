using Rusty.Engine;

namespace Rusty.Engine.Entities;

/// <summary>One pure Engine resolution and the managed write that applied it.</summary>
public readonly record struct EntityMotionResolverReceipt(
    MotionResolveReceipt Resolution,
    EntityBatchReceipt Managed);

/// <summary>
/// Projects product-owned Transform and SpatialCollider components into the
/// pure generated Motion service. It never retains a native entity state and
/// leaves movement intent, speed, and Look-derived direction in product code.
/// </summary>
public sealed class EntityMotionResolver
{
    private readonly EntityStore _entities;
    private readonly IMotionService _motion;
    private readonly ComponentType<SpatialCollider> _colliders;

    public EntityMotionResolver(
        EntityStore entities,
        IMotionService motion,
        ComponentType<SpatialCollider> colliders)
    {
        _entities = entities ?? throw new ArgumentNullException(nameof(entities));
        _motion = motion ?? throw new ArgumentNullException(nameof(motion));
        _colliders = colliders ?? throw new ArgumentNullException(nameof(colliders));
    }

    /// <summary>
    /// Resolves one caller-chosen local delta against the active Transform/collider
    /// entities, then writes the returned transform to the target.
    /// </summary>
    public EntityMotionResolverReceipt Resolve(EntityId target, System.Numerics.Vector3 delta)
    {
        IReadOnlyList<EntityComponents<Transform, SpatialCollider>> joined = _entities.Query(
            EngineComponentTypes.Transform,
            _colliders);
        var rows = new MotionSpatialEntity[joined.Count];
        bool targetFound = false;
        for (int index = 0; index < joined.Count; index++)
        {
            (EntityId entity, Transform transform, SpatialCollider collider) = joined[index];
            targetFound |= entity == target;
            // EntityStore deliberately has no transform-parent component today,
            // so this managed projection truthfully supplies unparented roots.
            rows[index] = new MotionSpatialEntity(
                entity.Value,
                transform,
                collider.Min,
                collider.Max,
                collider.Enabled,
                collider.StaticCollider,
                false);
        }
        if (!targetFound)
        {
            throw new InvalidOperationException($"Motion target {target.Value} is not an active Transform/collider entity.");
        }

        MotionResolveReceipt resolution = _motion.Resolve(new MotionResolveRequest(target.Value, delta, rows));
        EntityBatchReceipt managed = _entities.Commit(new EntityBatch().Set(
            target,
            EngineComponentTypes.Transform,
            resolution.CandidateTransform));
        return new EntityMotionResolverReceipt(resolution, managed);
    }
}
