using Rusty.Engine;

namespace Rusty.Engine.Entities;

/// <summary>
/// Composes caller-owned state-machine values with the canonical managed entity world.
///
/// Each component descriptor represents one explicit machine lane. EntityWorld owns attachment
/// presence, entity lifecycle, snapshots, and component revisions; the generated native service
/// validates only the detached machine definition and transition.
/// </summary>
public sealed class StateMachineEntityWorld
{
    public const int MaximumInspectionCount = 256;

    private readonly EntityWorld _entities;
    private readonly IStateMachineService _stateMachines;

    public StateMachineEntityWorld(EntityWorld entities, IStateMachineService stateMachines)
    {
        _entities = entities ?? throw new ArgumentNullException(nameof(entities));
        _stateMachines = stateMachines ?? throw new ArgumentNullException(nameof(stateMachines));
    }

    public StateMachineEntityAttachmentReceipt Attach(
        EntityId entity,
        ComponentType<StateMachineInstance> componentType,
        StateMachineDefinition definition,
        ulong initialState,
        EntityRevision? expectedEntityRevision = null,
        ComponentRevision? expectedComponentRevision = null)
    {
        ArgumentNullException.ThrowIfNull(componentType);
        ArgumentNullException.ThrowIfNull(definition);
        _ = RequireActive(entity, expectedEntityRevision);
        ComponentRevision componentRevision = RequireComponentRevision(
            entity,
            componentType,
            expectedComponentRevision);
        if (_entities.Has(entity, componentType))
        {
            throw new InvalidOperationException(
                $"Entity {entity.Value} already has state-machine component {componentType.Key.Value}.");
        }

        StateMachineDefinitionReadoutLeaseReceipt readout = ReadDefinition(definition);
        StateMachineDefinitionReadoutRow row = readout.Definitions.Span[0];
        ReadOnlySpan<StateMachineState> states = SliceStates(readout, row);
        if (!states.Contains(new StateMachineState(initialState)))
        {
            throw new InvalidOperationException(
                $"State {initialState} is not declared by machine {row.Machine}.");
        }

        StateMachineInstance instance = new(row.Machine, initialState, 0);
        _entities.Set(entity, componentType, instance, componentRevision);
        return new StateMachineEntityAttachmentReceipt(
            entity,
            _entities.GetEntityRevision(entity),
            _entities.GetComponentRevision(entity, componentType),
            instance);
    }

    public StateMachineEntityTransitionReceipt Transition(
        EntityId entity,
        ComponentType<StateMachineInstance> componentType,
        StateMachineDefinition definition,
        ulong expectedState,
        ulong nextState,
        ulong? expectedInstanceRevision = null,
        EntityRevision? expectedEntityRevision = null,
        ComponentRevision? expectedComponentRevision = null)
    {
        ArgumentNullException.ThrowIfNull(componentType);
        ArgumentNullException.ThrowIfNull(definition);
        RequireActive(entity, expectedEntityRevision);
        ComponentRevision componentRevision = RequireComponentRevision(
            entity,
            componentType,
            expectedComponentRevision);
        StateMachineInstance before = _entities.Get(entity, componentType);
        StateMachineDefinitionReadoutRow definitionRow = ReadDefinition(definition).Definitions.Span[0];
        if (before.Machine != definitionRow.Machine)
        {
            throw new InvalidOperationException(
                $"State-machine component {componentType.Key.Value} belongs to machine {before.Machine}, not {definitionRow.Machine}.");
        }

        ulong instanceRevision = expectedInstanceRevision ?? before.Revision;
        StateMachineTransitionReceipt applied = _stateMachines.ApplyTransition(
            new StateMachineTransitionRequest(
                definition,
                before,
                expectedState,
                nextState,
                true,
                instanceRevision));

        // The detached native call returns a value and owns no entity state. This exact component
        // guard is therefore the sole authority publication point.
        _entities.Set(entity, componentType, applied.Instance, componentRevision);
        return new StateMachineEntityTransitionReceipt(
            entity,
            _entities.GetEntityRevision(entity),
            _entities.GetComponentRevision(entity, componentType),
            before,
            applied.Instance,
            applied.Previous);
    }

    public StateMachineEntityDetachReceipt Detach(
        EntityId entity,
        ComponentType<StateMachineInstance> componentType,
        EntityRevision? expectedEntityRevision = null,
        ComponentRevision? expectedComponentRevision = null)
    {
        ArgumentNullException.ThrowIfNull(componentType);
        EntityRevision entityRevision = RequireAlive(entity, expectedEntityRevision);
        ComponentRevision componentRevision = RequireComponentRevision(
            entity,
            componentType,
            expectedComponentRevision);
        bool removed = _entities.Remove(entity, componentType, componentRevision);
        return new StateMachineEntityDetachReceipt(
            entity,
            removed ? _entities.GetEntityRevision(entity) : entityRevision,
            _entities.GetComponentRevision(entity, componentType),
            removed);
    }

    public StateMachineEntityInspectionPage Inspect(
        ComponentType<StateMachineInstance> componentType,
        int offset = 0,
        int maximum = 64,
        bool includeDisabled = false)
    {
        ArgumentNullException.ThrowIfNull(componentType);
        if (offset < 0)
        {
            throw new ArgumentOutOfRangeException(nameof(offset));
        }
        if (maximum is < 1 or > MaximumInspectionCount)
        {
            throw new ArgumentOutOfRangeException(
                nameof(maximum),
                $"Inspection count must be between 1 and {MaximumInspectionCount}.");
        }

        IReadOnlyList<EntityComponent<StateMachineInstance>> attached =
            _entities.Query(componentType, includeDisabled);
        StateMachineEntityInspectionRow[] rows = attached
            .Skip(offset)
            .Take(maximum)
            .Select(item => new StateMachineEntityInspectionRow(
                item.Entity,
                _entities.GetLifecycle(item.Entity),
                _entities.GetEntityRevision(item.Entity),
                _entities.GetComponentRevision(item.Entity, componentType),
                item.Value))
            .ToArray();
        return new StateMachineEntityInspectionPage(offset, attached.Count, rows);
    }

    private EntityRevision RequireActive(EntityId entity, EntityRevision? expectedRevision)
    {
        EntityRevision observed = RequireAlive(entity, expectedRevision);
        if (_entities.GetLifecycle(entity) != EntityLifecycle.Active)
        {
            throw new InvalidOperationException($"Entity {entity.Value} is not active.");
        }
        return observed;
    }

    private EntityRevision RequireAlive(EntityId entity, EntityRevision? expectedRevision)
    {
        EntityRevision observed = _entities.GetEntityRevision(entity);
        if (expectedRevision is EntityRevision expected && observed != expected)
        {
            throw new InvalidOperationException($"Entity {entity.Value} has a stale entity revision.");
        }
        if (_entities.GetLifecycle(entity) == EntityLifecycle.Tombstoned)
        {
            throw new InvalidOperationException($"Entity {entity.Value} has been tombstoned.");
        }
        return observed;
    }

    private ComponentRevision RequireComponentRevision(
        EntityId entity,
        ComponentType<StateMachineInstance> componentType,
        ComponentRevision? expectedRevision)
    {
        ComponentRevision observed = _entities.GetComponentRevision(entity, componentType);
        if (expectedRevision is ComponentRevision expected && observed != expected)
        {
            throw new InvalidOperationException(
                $"Entity {entity.Value} has a stale state-machine component revision.");
        }
        return observed;
    }

    private StateMachineDefinitionReadoutLeaseReceipt ReadDefinition(StateMachineDefinition definition)
    {
        StateMachineDefinitionReadoutLeaseReceipt readout = _stateMachines.ReadDefinition(definition);
        if (readout.Definitions.Length != 1)
        {
            throw new InvalidOperationException("A retained state-machine definition must have one readout row.");
        }
        StateMachineDefinitionReadoutRow row = readout.Definitions.Span[0];
        _ = SliceStates(readout, row);
        _ = SliceTransitions(readout, row);
        return readout;
    }

    private static ReadOnlySpan<StateMachineState> SliceStates(
        StateMachineDefinitionReadoutLeaseReceipt readout,
        StateMachineDefinitionReadoutRow row)
    {
        int start = checked((int)row.StatesStart);
        int length = checked((int)row.StatesLen);
        if (start > readout.States.Length || length > readout.States.Length - start)
        {
            throw new InvalidOperationException("State-machine definition returned an invalid state range.");
        }
        return readout.States.Span.Slice(start, length);
    }

    private static ReadOnlySpan<StateMachineTransition> SliceTransitions(
        StateMachineDefinitionReadoutLeaseReceipt readout,
        StateMachineDefinitionReadoutRow row)
    {
        int start = checked((int)row.TransitionsStart);
        int length = checked((int)row.TransitionsLen);
        if (start > readout.Transitions.Length || length > readout.Transitions.Length - start)
        {
            throw new InvalidOperationException("State-machine definition returned an invalid transition range.");
        }
        return readout.Transitions.Span.Slice(start, length);
    }
}

public readonly record struct StateMachineEntityAttachmentReceipt(
    EntityId Entity,
    EntityRevision EntityRevision,
    ComponentRevision ComponentRevision,
    StateMachineInstance Instance);

public readonly record struct StateMachineEntityTransitionReceipt(
    EntityId Entity,
    EntityRevision EntityRevision,
    ComponentRevision ComponentRevision,
    StateMachineInstance Before,
    StateMachineInstance After,
    ulong PreviousState);

public readonly record struct StateMachineEntityDetachReceipt(
    EntityId Entity,
    EntityRevision EntityRevision,
    ComponentRevision ComponentRevision,
    bool Removed);

public readonly record struct StateMachineEntityInspectionRow(
    EntityId Entity,
    EntityLifecycle Lifecycle,
    EntityRevision EntityRevision,
    ComponentRevision ComponentRevision,
    StateMachineInstance Instance);

public readonly record struct StateMachineEntityInspectionPage(
    int Offset,
    int TotalCount,
    ReadOnlyMemory<StateMachineEntityInspectionRow> Items);
