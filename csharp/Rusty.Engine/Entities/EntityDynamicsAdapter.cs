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

/// <summary>One native step/read and the managed writes that applied it.</summary>
public readonly record struct EntityDynamicsAdapterReceipt(
    DynamicsStepAndReadResult Native,
    EntityBatchReceipt Managed);

/// <summary>
/// Composes caller-owned EntityId-to-DynamicsBody bindings with canonical
/// managed Transform and copied DynamicsMotion values. It retains neither a
/// native entity mirror nor a parallel physics state.
/// </summary>
public sealed class EntityDynamicsAdapter
{
    private readonly EntityStore _entities;
    private readonly IDynamicsService _dynamics;
    private readonly DynamicsWorld _world;

    public EntityDynamicsAdapter(
        EntityStore entities,
        IDynamicsService dynamics,
        DynamicsWorld world)
    {
        _entities = entities ?? throw new ArgumentNullException(nameof(entities));
        _dynamics = dynamics ?? throw new ArgumentNullException(nameof(dynamics));
        _world = world ?? throw new ArgumentNullException(nameof(world));
    }

    /// <summary>
    /// Runs one Dynamics step/read for the bound bodies and writes every returned
    /// Transform/DynamicsMotion pair. Binding and action order are preserved by the
    /// native readout.
    /// </summary>
    public EntityDynamicsAdapterReceipt Step(
        float stepSeconds,
        uint steps,
        ReadOnlyMemory<DynamicsEntityBinding> bindings,
        ReadOnlyMemory<DynamicsEntityAction> actions)
    {
        DynamicsEntityBinding[] bound = bindings.ToArray();
        DynamicsStepAndReadResult native = _dynamics.StepAndRead(new DynamicsStepAndReadRequest(
            _world,
            stepSeconds,
            steps,
            ProjectActions(bound, actions.Span),
            bound.Select(binding => binding.Body).ToArray()));

        var batch = new EntityBatch();
        ReadOnlySpan<DynamicsBodyFact> rows = native.Bodies.Span;
        for (int index = 0; index < rows.Length; index++)
        {
            EntityId entity = bound[index].Entity;
            DynamicsReadout readout = rows[index].Readout;
            // Rigid dynamics owns translation and rotation. Scale is a product
            // render/layout fact outside the unit-scale body representation.
            Vector3 scale = _entities.Get(entity, EngineComponentTypes.Transform).Scale;
            batch.Set(entity, EngineComponentTypes.Transform, readout.Transform with { Scale = scale })
                .Set(entity, EngineComponentTypes.DynamicsMotion, new DynamicsMotion(
                    readout.LinearVelocity,
                    readout.AngularVelocity,
                    readout.Sleeping));
        }
        return new EntityDynamicsAdapterReceipt(native, _entities.Commit(batch));
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
}
