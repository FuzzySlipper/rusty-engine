using Rusty.Engine;

namespace Rusty.Engine.Entities;

/// <summary>Exact managed revision evidence for one active motion row.</summary>
public readonly record struct EntityMotionResolverComponentGuard(
    EntityId Entity,
    ComponentRevision TransformRevision,
    ComponentRevision ColliderRevision);

/// <summary>Copied managed evidence required before applying a pure Motion result.</summary>
public readonly record struct EntityMotionResolverGuard(
    ulong StoreRevision,
    ReadOnlyMemory<EntityMotionResolverComponentGuard> Components);

/// <summary>One pure Engine resolution plus the one managed publication receipt.</summary>
public readonly record struct EntityMotionResolverReceipt(
    MotionResolveReceipt Resolution,
    EntityBatchReceipt Managed,
    EntityMotionResolverGuard Guard);

/// <summary>
/// Projects product-owned Transform and SpatialCollider components into the
/// pure generated Motion service. It never retains a native entity state and
/// leaves movement intent, speed, and Look-derived direction in product code.
/// </summary>
public sealed class EntityMotionResolver
{
    private const int MinimumMaximumEntities = 1;

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
    /// Resolves one caller-chosen local delta against the deterministic active
    /// Transform/collider projection, then applies the returned candidate
    /// transform in exactly one guarded EntityStore batch.
    /// </summary>
    public EntityMotionResolverReceipt Resolve(
        EntityId target,
        System.Numerics.Vector3 delta,
        int maximumEntities,
        EntityMotionResolverGuard? expectedGuard = null)
    {
        if (maximumEntities < MinimumMaximumEntities)
        {
            throw new ArgumentOutOfRangeException(nameof(maximumEntities));
        }

        EntityMotionResolverGuard guard = CaptureGuard(maximumEntities);
        if (expectedGuard is EntityMotionResolverGuard expected)
        {
            ValidateGuard(expected, guard);
        }

        MotionSpatialEntity[] rows = ProjectRows(guard.Components.Span);
        MotionResolveReceipt resolution = _motion.Resolve(new MotionResolveRequest(target.Value, delta, rows));

        // Resolve is pure. Rechecking managed facts after it returns prevents
        // an old candidate transform from being published into changed product
        // state without inventing a native EntityStore revision.
        ValidateGuard(guard, CaptureGuard(maximumEntities));
        if (!TryFindTargetGuard(guard.Components.Span, target, out EntityMotionResolverComponentGuard targetGuard))
        {
            throw new InvalidOperationException($"Motion target {target.Value} is not an active Transform/collider entity.");
        }

        var batch = new EntityBatch().Set(
            target,
            EngineComponentTypes.Transform,
            resolution.CandidateTransform,
            targetGuard.TransformRevision);
        EntityEdit prepared = _entities.PrepareBatch(batch, guard.StoreRevision);
        prepared.Publish();
        return new EntityMotionResolverReceipt(resolution, prepared.Receipt, guard);
    }

    private EntityMotionResolverGuard CaptureGuard(int maximumEntities)
    {
        IReadOnlyList<EntityComponents<Transform, SpatialCollider>> joined = _entities.Query(
            EngineComponentTypes.Transform,
            _colliders);
        if (joined.Count > maximumEntities)
        {
            throw new InvalidOperationException(
                $"Motion has {joined.Count} active rows, exceeding its explicit batch bound {maximumEntities}.");
        }
        var guards = new EntityMotionResolverComponentGuard[joined.Count];
        for (int index = 0; index < joined.Count; index++)
        {
            EntityComponents<Transform, SpatialCollider> row = joined[index];
            guards[index] = new EntityMotionResolverComponentGuard(
                row.Entity,
                _entities.GetComponentRevision(row.Entity, EngineComponentTypes.Transform),
                _entities.GetComponentRevision(row.Entity, _colliders));
        }
        return new EntityMotionResolverGuard(_entities.Revision, guards);
    }

    private MotionSpatialEntity[] ProjectRows(ReadOnlySpan<EntityMotionResolverComponentGuard> guards)
    {
        var rows = new MotionSpatialEntity[guards.Length];
        for (int index = 0; index < guards.Length; index++)
        {
            EntityId entity = guards[index].Entity;
            Transform transform = _entities.Get(entity, EngineComponentTypes.Transform);
            SpatialCollider collider = _entities.Get(entity, _colliders);
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
        return rows;
    }

    private static bool TryFindTargetGuard(
        ReadOnlySpan<EntityMotionResolverComponentGuard> guards,
        EntityId target,
        out EntityMotionResolverComponentGuard targetGuard)
    {
        foreach (EntityMotionResolverComponentGuard guard in guards)
        {
            if (guard.Entity == target)
            {
                targetGuard = guard;
                return true;
            }
        }
        targetGuard = default;
        return false;
    }

    private static void ValidateGuard(EntityMotionResolverGuard expected, EntityMotionResolverGuard observed)
    {
        if (expected.StoreRevision != observed.StoreRevision)
        {
            throw new InvalidOperationException(
                $"Motion managed store revision is stale: expected {expected.StoreRevision}, actual {observed.StoreRevision}.");
        }
        ReadOnlySpan<EntityMotionResolverComponentGuard> expectedComponents = expected.Components.Span;
        ReadOnlySpan<EntityMotionResolverComponentGuard> observedComponents = observed.Components.Span;
        if (expectedComponents.Length != observedComponents.Length)
        {
            throw new InvalidOperationException("Motion managed row set is stale.");
        }
        for (int index = 0; index < expectedComponents.Length; index++)
        {
            if (expectedComponents[index] != observedComponents[index])
            {
                throw new InvalidOperationException(
                    $"Motion managed component revision is stale for entity {observedComponents[index].Entity.Value}.");
            }
        }
    }
}
