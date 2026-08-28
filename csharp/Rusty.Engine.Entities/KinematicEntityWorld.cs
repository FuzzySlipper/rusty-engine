using System.Numerics;
using Rusty.Engine;

namespace Rusty.Engine.Entities;

/// <summary>
/// Product-owned bounds and velocity projected into one call-local Kinematic
/// motion phase. It is distinct from the generated detached Physics
/// <c>KinematicBody</c> value.
/// </summary>
public readonly record struct Kinematic(Vector3 HalfExtents, Vector3 Velocity);

/// <summary>Exact managed revision evidence for one projected Kinematic row.</summary>
public readonly record struct KinematicEntityWorldComponentGuard(
    EntityId Entity,
    ComponentRevision TransformRevision,
    ComponentRevision KinematicRevision,
    ComponentRevision ColliderRevision);

/// <summary>Copied managed evidence that must still hold before applying a phase candidate.</summary>
public readonly record struct KinematicEntityWorldGuard(
    ulong WorldRevision,
    ReadOnlyMemory<KinematicEntityWorldComponentGuard> Components);

/// <summary>One completed pure phase plus its one managed publication receipt.</summary>
public readonly record struct KinematicEntityWorldReceipt(
    KinematicMotionLeaseReceipt Motion,
    EntityBatchReceipt Managed,
    KinematicEntityWorldGuard Guard);

/// <summary>
/// A copied, call-local Kinematic phase result. No native state remains after
/// <see cref="KinematicEntityWorld.Prepare"/> returns; <see cref="Apply"/>
/// only rechecks and assigns canonical managed component state.
/// </summary>
public sealed class KinematicEntityWorldPrepared
{
    private readonly EntityWorld _entities;
    private readonly ComponentType<SpatialCollider> _colliders;
    private bool _applied;

    internal KinematicEntityWorldPrepared(
        EntityWorld entities,
        ComponentType<SpatialCollider> colliders,
        KinematicEntityWorldGuard guard,
        KinematicMotionLeaseReceipt motion)
    {
        _entities = entities;
        _colliders = colliders;
        Guard = guard;
        Motion = motion;
    }

    public KinematicEntityWorldGuard Guard { get; }

    public KinematicMotionLeaseReceipt Motion { get; }

    /// <summary>
    /// Rechecks the copied projection, then publishes all changed Transform
    /// and Kinematic values in exactly one assignment-only EntityWorld batch.
    /// </summary>
    public KinematicEntityWorldReceipt Apply()
    {
        if (_applied)
        {
            throw new InvalidOperationException("A Kinematic phase candidate can only be applied once.");
        }
        KinematicEntityWorld.ValidateGuard(Guard, KinematicEntityWorld.CaptureGuard(_entities, _colliders, checked((int)Guard.Components.Length)));
        EntityBatch batch = KinematicEntityWorld.BuildBatch(Guard, Motion);
        EntityWorldBatchCandidate staged = _entities.PrepareBatch(batch, Guard.WorldRevision);
        staged.Publish();
        _applied = true;
        return new KinematicEntityWorldReceipt(Motion, staged.Receipt, Guard);
    }
}

/// <summary>
/// Projects active managed Transform and Kinematic values into the generated
/// Kinematic motion family. Optional SpatialCollider values control dynamic
/// blocking; an absent collider truthfully becomes disabled for that call.
/// </summary>
public sealed class KinematicEntityWorld
{
    private const int MinimumMaximumEntities = 1;

    private readonly EntityWorld _entities;
    private readonly IKinematicService _kinematic;
    private readonly ComponentType<SpatialCollider> _colliders;

    public KinematicEntityWorld(
        EntityWorld entities,
        IKinematicService kinematic,
        ComponentType<SpatialCollider> colliders)
    {
        _entities = entities ?? throw new ArgumentNullException(nameof(entities));
        _kinematic = kinematic ?? throw new ArgumentNullException(nameof(kinematic));
        _colliders = colliders ?? throw new ArgumentNullException(nameof(colliders));
    }

    /// <summary>
    /// Captures a bounded active Transform/Kinematic projection and resolves
    /// one product-selected phase through the existing Spatial-session snapshot.
    /// A null selection runs every projected body; an empty selection is an
    /// explicit selected phase that advances none.
    /// </summary>
    public KinematicEntityWorldPrepared Prepare(
        SpatialSession session,
        float deltaSeconds,
        int maximumEntities,
        ReadOnlyMemory<EntityId>? selection = null,
        KinematicEntityWorldGuard? expectedGuard = null)
    {
        ArgumentNullException.ThrowIfNull(session);
        if (maximumEntities < MinimumMaximumEntities)
        {
            throw new ArgumentOutOfRangeException(nameof(maximumEntities));
        }

        KinematicEntityWorldGuard guard = CaptureGuard(_entities, _colliders, maximumEntities);
        if (expectedGuard is KinematicEntityWorldGuard expected)
        {
            ValidateGuard(expected, guard);
        }

        ReadOnlyMemory<EntityId> selected = selection.GetValueOrDefault();
        if (selection.HasValue && selected.Length > maximumEntities)
        {
            throw new InvalidOperationException(
                $"Kinematic selection has {selected.Length} ids, exceeding its explicit bound {maximumEntities}.");
        }
        ulong[] selectedIds = ProjectSelection(selected, selection.HasValue);
        KinematicMotionEntityRow[] rows = ProjectRows(guard.Components.Span);
        KinematicMotionLeaseReceipt motion = _kinematic.RunMotion(new KinematicMotionRequest(
            session,
            deltaSeconds,
            rows,
            selection.HasValue,
            selectedIds));
        ValidateGuard(guard, CaptureGuard(_entities, _colliders, maximumEntities));
        ValidateMotionReceipt(guard, motion, selectedIds, selection.HasValue);
        return new KinematicEntityWorldPrepared(_entities, _colliders, guard, motion);
    }

    internal static KinematicEntityWorldGuard CaptureGuard(
        EntityWorld entities,
        ComponentType<SpatialCollider> colliders,
        int maximumEntities)
    {
        IReadOnlyList<EntityComponents<Transform, Kinematic>> joined = entities.Query(
            EngineComponentTypes.Transform,
            EngineComponentTypes.Kinematic);
        if (joined.Count > maximumEntities)
        {
            throw new InvalidOperationException(
                $"Kinematic has {joined.Count} active rows, exceeding its explicit batch bound {maximumEntities}.");
        }
        var guards = new KinematicEntityWorldComponentGuard[joined.Count];
        for (int index = 0; index < joined.Count; index++)
        {
            EntityId entity = joined[index].Entity;
            guards[index] = new KinematicEntityWorldComponentGuard(
                entity,
                entities.GetComponentRevision(entity, EngineComponentTypes.Transform),
                entities.GetComponentRevision(entity, EngineComponentTypes.Kinematic),
                entities.GetComponentRevision(entity, colliders));
        }
        return new KinematicEntityWorldGuard(entities.Revision, guards);
    }

    internal static void ValidateGuard(KinematicEntityWorldGuard expected, KinematicEntityWorldGuard observed)
    {
        if (expected.WorldRevision != observed.WorldRevision)
        {
            throw new InvalidOperationException(
                $"Kinematic managed world revision is stale: expected {expected.WorldRevision}, actual {observed.WorldRevision}.");
        }
        ReadOnlySpan<KinematicEntityWorldComponentGuard> expectedComponents = expected.Components.Span;
        ReadOnlySpan<KinematicEntityWorldComponentGuard> observedComponents = observed.Components.Span;
        if (expectedComponents.Length != observedComponents.Length)
        {
            throw new InvalidOperationException("Kinematic managed row set is stale.");
        }
        for (int index = 0; index < expectedComponents.Length; index++)
        {
            if (expectedComponents[index] != observedComponents[index])
            {
                throw new InvalidOperationException(
                    $"Kinematic managed component revision is stale for entity {observedComponents[index].Entity.Value}.");
            }
        }
    }

    internal static EntityBatch BuildBatch(KinematicEntityWorldGuard guard, KinematicMotionLeaseReceipt motion)
    {
        var batch = new EntityBatch();
        foreach (KinematicMotionCandidate candidate in motion.Candidates.Span)
        {
            KinematicEntityWorldComponentGuard component = FindGuard(guard.Components.Span, new EntityId(candidate.EntityId));
            bool transformChanged = candidate.BeforeTransform != candidate.AfterTransform;
            bool velocityChanged = candidate.BeforeVelocity != candidate.AfterVelocity;
            if (!transformChanged && !velocityChanged)
            {
                throw new InvalidOperationException($"Kinematic candidate {candidate.EntityId} did not change managed state.");
            }
            batch.Mutate(world =>
            {
                if (transformChanged)
                {
                    world.Set(component.Entity, EngineComponentTypes.Transform, candidate.AfterTransform, component.TransformRevision);
                }
                if (velocityChanged)
                {
                    Kinematic current = world.Get(component.Entity, EngineComponentTypes.Kinematic);
                    world.Set(
                        component.Entity,
                        EngineComponentTypes.Kinematic,
                        new Kinematic(current.HalfExtents, candidate.AfterVelocity),
                        component.KinematicRevision);
                }
            });
        }
        return batch;
    }

    private KinematicMotionEntityRow[] ProjectRows(ReadOnlySpan<KinematicEntityWorldComponentGuard> guards)
    {
        var rows = new KinematicMotionEntityRow[guards.Length];
        for (int index = 0; index < guards.Length; index++)
        {
            EntityId entity = guards[index].Entity;
            Transform transform = _entities.Get(entity, EngineComponentTypes.Transform);
            Kinematic kinematic = _entities.Get(entity, EngineComponentTypes.Kinematic);
            bool collisionEnabled = _entities.TryGet(entity, _colliders, out SpatialCollider collider) && collider.Enabled;
            bool collisionStatic = collisionEnabled && collider.StaticCollider;
            rows[index] = new KinematicMotionEntityRow(
                entity.Value,
                transform,
                kinematic.HalfExtents,
                kinematic.Velocity,
                collisionEnabled,
                collisionStatic);
        }
        return rows;
    }

    private static ulong[] ProjectSelection(ReadOnlyMemory<EntityId> selection, bool selectionPresent)
    {
        if (!selectionPresent)
        {
            return [];
        }
        var ids = new ulong[selection.Length];
        var seen = new HashSet<ulong>();
        for (int index = 0; index < selection.Length; index++)
        {
            ulong id = selection.Span[index].Value;
            if (!seen.Add(id))
            {
                throw new ArgumentException($"Kinematic selection contains duplicate entity {id}.", nameof(selection));
            }
            ids[index] = id;
        }
        return ids;
    }

    private void ValidateMotionReceipt(
        KinematicEntityWorldGuard guard,
        KinematicMotionLeaseReceipt motion,
        ReadOnlySpan<ulong> selectedIds,
        bool selectionPresent)
    {
        ReadOnlySpan<KinematicEntityWorldComponentGuard> components = guard.Components.Span;
        if (motion.BodiesConsidered > (ulong)components.Length
            || motion.MovedBodies > motion.BodiesConsidered
            || motion.BlockedAxes > (ulong)motion.Facts.Length
            || motion.RevisionBefore != 0
            || motion.RevisionAfter != (motion.Candidates.Length == 0 ? 0UL : 1UL)
            || motion.MovedBodies > (ulong)motion.Candidates.Length)
        {
            throw new InvalidOperationException("Kinematic native phase receipt had invalid call-local metadata.");
        }
        var allowed = new HashSet<ulong>(selectionPresent ? selectedIds.ToArray() : components.ToArray().Select(component => component.Entity.Value));
        ulong expectedBodies = 0;
        foreach (KinematicEntityWorldComponentGuard component in components)
        {
            if (allowed.Contains(component.Entity.Value))
            {
                expectedBodies++;
            }
        }
        if (motion.BodiesConsidered != expectedBodies)
        {
            throw new InvalidOperationException("Kinematic native phase receipt considered an unexpected entity set.");
        }

        var candidates = new HashSet<ulong>();
        ulong previousCandidate = 0;
        foreach (KinematicMotionCandidate candidate in motion.Candidates.Span)
        {
            if ((candidates.Count != 0 && candidate.EntityId <= previousCandidate)
                || !allowed.Contains(candidate.EntityId)
                || !TryFindGuard(components, new EntityId(candidate.EntityId), out KinematicEntityWorldComponentGuard component)
                || candidate.BeforeTransform != CurrentTransform(component)
                || candidate.BeforeVelocity != CurrentVelocity(component)
                || (candidate.BeforeTransform == candidate.AfterTransform && candidate.BeforeVelocity == candidate.AfterVelocity)
                || !candidates.Add(candidate.EntityId))
            {
                throw new InvalidOperationException("Kinematic native candidate did not match the guarded managed projection.");
            }
            previousCandidate = candidate.EntityId;
        }

        Transform CurrentTransform(KinematicEntityWorldComponentGuard component)
            => _entities.Get(component.Entity, EngineComponentTypes.Transform);
        Vector3 CurrentVelocity(KinematicEntityWorldComponentGuard component)
            => _entities.Get(component.Entity, EngineComponentTypes.Kinematic).Velocity;
    }

    private static KinematicEntityWorldComponentGuard FindGuard(
        ReadOnlySpan<KinematicEntityWorldComponentGuard> guards,
        EntityId entity)
    {
        if (TryFindGuard(guards, entity, out KinematicEntityWorldComponentGuard result))
        {
            return result;
        }
        throw new InvalidOperationException($"Kinematic candidate referenced unknown entity {entity.Value}.");
    }

    private static bool TryFindGuard(
        ReadOnlySpan<KinematicEntityWorldComponentGuard> guards,
        EntityId entity,
        out KinematicEntityWorldComponentGuard result)
    {
        foreach (KinematicEntityWorldComponentGuard guard in guards)
        {
            if (guard.Entity == entity)
            {
                result = guard;
                return true;
            }
        }
        result = default;
        return false;
    }
}
