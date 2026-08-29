using System.Numerics;
using Rusty.Engine;

namespace Rusty.Engine.Entities;

/// <summary>
/// The minimal managed motion facts copied from one retained Dynamics body.
/// Shape, mass, contact facts, and body lifetime remain with Dynamics.
/// </summary>
public readonly record struct DynamicsMotion(
    Vector3 LinearVelocity,
    Vector3 AngularVelocity,
    bool Sleeping);

/// <summary>
/// An explicit product-owned association between one canonical entity and one
/// retained Dynamics owner. The adapter does not store or infer this mapping.
/// </summary>
public readonly record struct DynamicsEntityBinding(EntityId Entity, DynamicsBody Body);

/// <summary>One product-selected wrench for an explicitly bound entity.</summary>
public readonly record struct DynamicsEntityAction(
    EntityId Entity,
    Vector3 Force,
    Vector3 Torque,
    Vector3 Impulse,
    Vector3 TorqueImpulse,
    bool Wake);

/// <summary>Exact managed revision evidence for one Dynamics publication row.</summary>
public readonly record struct DynamicsEntityWorldComponentGuard(
    EntityId Entity,
    ComponentRevision TransformRevision,
    ComponentRevision MotionRevision,
    DynamicsBodyHandle Body);

/// <summary>Copied managed evidence required before and after one Dynamics crossing.</summary>
public readonly record struct DynamicsEntityWorldGuard(
    ulong WorldRevision,
    ReadOnlyMemory<DynamicsEntityWorldComponentGuard> Components);

/// <summary>One native coherent step/read and its one canonical managed batch.</summary>
public readonly record struct DynamicsEntityWorldReceipt(
    DynamicsStepAndReadLeaseReceipt Native,
    EntityBatchReceipt Managed,
    DynamicsEntityWorldGuard Guard);

/// <summary>
/// Composes caller-owned EntityId-to-DynamicsBody bindings with canonical
/// managed Transform and copied DynamicsMotion values. It retains neither a
/// native entity mirror nor a parallel physics state.
/// </summary>
public sealed class DynamicsEntityWorld
{
    private readonly EntityWorld _entities;
    private readonly IDynamicsService _dynamics;
    private readonly DynamicsWorld _world;

    public DynamicsEntityWorld(
        EntityWorld entities,
        IDynamicsService dynamics,
        DynamicsWorld world)
    {
        _entities = entities ?? throw new ArgumentNullException(nameof(entities));
        _dynamics = dynamics ?? throw new ArgumentNullException(nameof(dynamics));
        _world = world ?? throw new ArgumentNullException(nameof(world));
    }

    /// <summary>
    /// Preflights the exact participating component revisions, runs one
    /// bounded typed Dynamics operation, then rechecks and publishes every
    /// returned Transform/DynamicsMotion pair in exactly one EntityBatch.
    /// Binding and action order are explicit and preserved by the native lease.
    /// </summary>
    public DynamicsEntityWorldReceipt Step(
        float stepSeconds,
        uint steps,
        ReadOnlyMemory<DynamicsEntityBinding> bindings,
        ReadOnlyMemory<DynamicsEntityAction> actions,
        int maximumBodies,
        int maximumActions,
        DynamicsEntityWorldGuard? expectedGuard = null)
    {
        if (maximumBodies < 0)
        {
            throw new ArgumentOutOfRangeException(nameof(maximumBodies));
        }
        if (maximumActions < 0)
        {
            throw new ArgumentOutOfRangeException(nameof(maximumActions));
        }
        if (bindings.Length > maximumBodies)
        {
            throw new InvalidOperationException(
                $"Dynamics has {bindings.Length} bindings, exceeding its explicit batch bound {maximumBodies}.");
        }
        if (actions.Length > maximumActions)
        {
            throw new InvalidOperationException(
                $"Dynamics has {actions.Length} actions, exceeding its explicit action bound {maximumActions}.");
        }

        DynamicsEntityBinding[] projectedBindings = bindings.ToArray();
        DynamicsEntityWorldGuard guard = CaptureGuard(projectedBindings);
        if (expectedGuard is DynamicsEntityWorldGuard expected)
        {
            ValidateGuard(expected, guard);
        }

        DynamicsAction[] projectedActions = ProjectActions(projectedBindings, actions.Span);
        DynamicsBody[] selectedBodies = projectedBindings.Select(binding => binding.Body).ToArray();
        DynamicsStepAndReadLeaseReceipt native = _dynamics.StepAndRead(new DynamicsStepAndReadRequest(
            _world,
            stepSeconds,
            steps,
            projectedActions,
            selectedBodies));

        ValidateGuard(guard, CaptureGuard(projectedBindings));
        ValidateNativeReceipt(projectedBindings, native);
        EntityWorldBatchCandidate managed = _entities.PrepareBatch(BuildBatch(guard, native), guard.WorldRevision);
        managed.Publish();
        return new DynamicsEntityWorldReceipt(native, managed.Receipt, guard);
    }

    private DynamicsEntityWorldGuard CaptureGuard(ReadOnlySpan<DynamicsEntityBinding> bindings)
    {
        var active = new HashSet<EntityId>(_entities.Query(
            EngineComponentTypes.Transform,
            EngineComponentTypes.DynamicsMotion).Select(row => row.Entity));
        var entities = new HashSet<ulong>();
        var bodies = new HashSet<ulong>();
        var guards = new DynamicsEntityWorldComponentGuard[bindings.Length];
        for (int index = 0; index < bindings.Length; index++)
        {
            DynamicsEntityBinding binding = bindings[index];
            if (binding.Body is null)
            {
                throw new ArgumentNullException(nameof(bindings), $"Dynamics entity {binding.Entity.Value} has no caller-owned body.");
            }
            if (!entities.Add(binding.Entity.Value))
            {
                throw new ArgumentException($"Dynamics bindings contain duplicate entity {binding.Entity.Value}.", nameof(bindings));
            }
            if (!bodies.Add(binding.Body.Handle.Value))
            {
                throw new ArgumentException($"Dynamics bindings contain duplicate body {binding.Body.Handle.Value}.", nameof(bindings));
            }
            if (!active.Contains(binding.Entity))
            {
                throw new InvalidOperationException(
                    $"Dynamics entity {binding.Entity.Value} must be active with Transform and DynamicsMotion components.");
            }
            guards[index] = new DynamicsEntityWorldComponentGuard(
                binding.Entity,
                _entities.GetComponentRevision(binding.Entity, EngineComponentTypes.Transform),
                _entities.GetComponentRevision(binding.Entity, EngineComponentTypes.DynamicsMotion),
                binding.Body.Handle);
        }
        return new DynamicsEntityWorldGuard(_entities.Revision, guards);
    }

    private static DynamicsAction[] ProjectActions(
        ReadOnlySpan<DynamicsEntityBinding> bindings,
        ReadOnlySpan<DynamicsEntityAction> actions)
    {
        var bodies = new Dictionary<EntityId, DynamicsBody>(bindings.Length);
        foreach (DynamicsEntityBinding binding in bindings)
        {
            bodies.Add(binding.Entity, binding.Body);
        }
        var projected = new DynamicsAction[actions.Length];
        for (int index = 0; index < actions.Length; index++)
        {
            DynamicsEntityAction action = actions[index];
            if (!bodies.TryGetValue(action.Entity, out DynamicsBody? body))
            {
                throw new InvalidOperationException(
                    $"Dynamics action {index} references unbound entity {action.Entity.Value}.");
            }
            projected[index] = new DynamicsAction(
                body,
                action.Force,
                action.Torque,
                action.Impulse,
                action.TorqueImpulse,
                action.Wake);
        }
        return projected;
    }

    private static void ValidateGuard(
        DynamicsEntityWorldGuard expected,
        DynamicsEntityWorldGuard observed)
    {
        if (expected.WorldRevision != observed.WorldRevision)
        {
            throw new InvalidOperationException(
                $"Dynamics managed world revision is stale: expected {expected.WorldRevision}, actual {observed.WorldRevision}.");
        }
        ReadOnlySpan<DynamicsEntityWorldComponentGuard> expectedRows = expected.Components.Span;
        ReadOnlySpan<DynamicsEntityWorldComponentGuard> observedRows = observed.Components.Span;
        if (expectedRows.Length != observedRows.Length)
        {
            throw new InvalidOperationException("Dynamics managed binding set is stale.");
        }
        for (int index = 0; index < expectedRows.Length; index++)
        {
            if (expectedRows[index] != observedRows[index])
            {
                throw new InvalidOperationException(
                    $"Dynamics managed component or binding is stale for entity {expectedRows[index].Entity.Value}.");
            }
        }
    }

    private static void ValidateNativeReceipt(
        ReadOnlySpan<DynamicsEntityBinding> bindings,
        DynamicsStepAndReadLeaseReceipt native)
    {
        ReadOnlySpan<DynamicsStepAndReadBody> rows = native.Bodies.Span;
        if (rows.Length != bindings.Length || native.BodyCount < rows.Length)
        {
            throw new InvalidOperationException("Dynamics step/read receipt did not contain the requested body set.");
        }
        for (int index = 0; index < rows.Length; index++)
        {
            if (rows[index].Body.Value != bindings[index].Body.Handle.Value)
            {
                throw new InvalidOperationException("Dynamics step/read receipt did not preserve explicit binding order.");
            }
        }
    }

    private EntityBatch BuildBatch(
        DynamicsEntityWorldGuard guard,
        DynamicsStepAndReadLeaseReceipt native)
    {
        var batch = new EntityBatch();
        ReadOnlySpan<DynamicsEntityWorldComponentGuard> components = guard.Components.Span;
        ReadOnlySpan<DynamicsStepAndReadBody> rows = native.Bodies.Span;
        for (int index = 0; index < rows.Length; index++)
        {
            DynamicsEntityWorldComponentGuard component = components[index];
            DynamicsReadout readout = rows[index].Readout;
            Transform transform = readout.Transform with
            {
                // Rigid dynamics owns translation and rotation. Scale is a
                // canonical product render/layout fact and remains outside
                // the retained unit-scale body representation.
                Scale = _entities.Get(component.Entity, EngineComponentTypes.Transform).Scale,
            };
            DynamicsMotion motion = new(
                readout.LinearVelocity,
                readout.AngularVelocity,
                readout.Sleeping);
            batch.Mutate(world =>
            {
                world.Set(
                    component.Entity,
                    EngineComponentTypes.Transform,
                    transform,
                    component.TransformRevision);
                world.Set(
                    component.Entity,
                    EngineComponentTypes.DynamicsMotion,
                    motion,
                    component.MotionRevision);
            });
        }
        return batch;
    }
}
