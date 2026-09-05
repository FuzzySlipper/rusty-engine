using Rusty.Engine;

namespace Rusty.Engine.Entities;

/// <summary>Exact managed revision evidence for one active motion row.</summary>
public readonly record struct MotionEntityWorldComponentGuard(
    EntityId Entity,
    ComponentRevision TransformRevision,
    ComponentRevision ColliderRevision);

/// <summary>Copied managed evidence required before applying a pure Motion result.</summary>
public readonly record struct MotionEntityWorldGuard(
    ulong WorldRevision,
    ReadOnlyMemory<MotionEntityWorldComponentGuard> Components);

/// <summary>One pure Engine resolution plus the one managed publication receipt.</summary>
public readonly record struct MotionEntityWorldReceipt(
    MotionResolveReceipt Resolution,
    EntityBatchReceipt Managed,
    MotionEntityWorldGuard Guard);

/// <summary>
/// Projects product-owned Transform and SpatialCollider components into the
/// pure generated Motion service. It never retains a native entity state and
/// leaves movement intent, speed, and Look-derived direction in product code.
/// </summary>
public sealed class MotionEntityWorld
{
    private const int MinimumMaximumEntities = 1;

    private readonly EntityWorld _entities;
    private readonly IMotionService _motion;
    private readonly ComponentType<SpatialCollider> _colliders;

    public MotionEntityWorld(
        EntityWorld entities,
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
    /// transform in exactly one guarded EntityWorld batch.
    /// </summary>
    public MotionEntityWorldReceipt Resolve(
        EntityId target,
        System.Numerics.Vector3 delta,
        int maximumEntities,
        MotionEntityWorldGuard? expectedGuard = null)
    {
        if (maximumEntities < MinimumMaximumEntities)
        {
            throw new ArgumentOutOfRangeException(nameof(maximumEntities));
        }

        MotionEntityWorldGuard guard = CaptureGuard(maximumEntities);
        if (expectedGuard is MotionEntityWorldGuard expected)
        {
            ValidateGuard(expected, guard);
        }

        MotionSpatialEntity[] rows = ProjectRows(guard.Components.Span);
        MotionResolveReceipt resolution = _motion.Resolve(new MotionResolveRequest(target.Value, delta, rows));

        // Resolve is pure. Rechecking managed facts after it returns prevents
        // an old candidate transform from being published into changed product
        // state without inventing a native EntityWorld revision.
        ValidateGuard(guard, CaptureGuard(maximumEntities));
        if (!TryFindTargetGuard(guard.Components.Span, target, out MotionEntityWorldComponentGuard targetGuard))
        {
            throw new InvalidOperationException($"Motion target {target.Value} is not an active Transform/collider entity.");
        }

        var batch = new EntityBatch().Mutate(world => world.Set(
            target,
            EngineComponentTypes.Transform,
            resolution.CandidateTransform,
            targetGuard.TransformRevision));
        EntityWorldBatchCandidate prepared = _entities.PrepareBatch(batch, guard.WorldRevision);
        prepared.Publish();
        return new MotionEntityWorldReceipt(resolution, prepared.Receipt, guard);
    }

    private MotionEntityWorldGuard CaptureGuard(int maximumEntities)
    {
        IReadOnlyList<EntityComponents<Transform, SpatialCollider>> joined = _entities.Query(
            EngineComponentTypes.Transform,
            _colliders);
        if (joined.Count > maximumEntities)
        {
            throw new InvalidOperationException(
                $"Motion has {joined.Count} active rows, exceeding its explicit batch bound {maximumEntities}.");
        }
        var guards = new MotionEntityWorldComponentGuard[joined.Count];
        for (int index = 0; index < joined.Count; index++)
        {
            EntityComponents<Transform, SpatialCollider> row = joined[index];
            guards[index] = new MotionEntityWorldComponentGuard(
                row.Entity,
                _entities.GetComponentRevision(row.Entity, EngineComponentTypes.Transform),
                _entities.GetComponentRevision(row.Entity, _colliders));
        }
        return new MotionEntityWorldGuard(_entities.Revision, guards);
    }

    private MotionSpatialEntity[] ProjectRows(ReadOnlySpan<MotionEntityWorldComponentGuard> guards)
    {
        var rows = new MotionSpatialEntity[guards.Length];
        for (int index = 0; index < guards.Length; index++)
        {
            EntityId entity = guards[index].Entity;
            Transform transform = _entities.Get(entity, EngineComponentTypes.Transform);
            SpatialCollider collider = _entities.Get(entity, _colliders);
            // EntityWorld deliberately has no transform-parent component today,
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
        ReadOnlySpan<MotionEntityWorldComponentGuard> guards,
        EntityId target,
        out MotionEntityWorldComponentGuard targetGuard)
    {
        foreach (MotionEntityWorldComponentGuard guard in guards)
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

    private static void ValidateGuard(MotionEntityWorldGuard expected, MotionEntityWorldGuard observed)
    {
        if (expected.WorldRevision != observed.WorldRevision)
        {
            throw new InvalidOperationException(
                $"Motion managed world revision is stale: expected {expected.WorldRevision}, actual {observed.WorldRevision}.");
        }
        ReadOnlySpan<MotionEntityWorldComponentGuard> expectedComponents = expected.Components.Span;
        ReadOnlySpan<MotionEntityWorldComponentGuard> observedComponents = observed.Components.Span;
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
