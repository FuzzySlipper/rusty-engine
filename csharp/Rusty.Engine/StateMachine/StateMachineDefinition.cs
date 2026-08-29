using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.Linq;

namespace Rusty.Engine.StateMachine;

/// <summary>
/// An immutable, product-owned state-machine graph.
///
/// The Engine does not retain a machine registry. A product creates a definition and passes that
/// value to the domain code that owns its instances and meaning.
/// </summary>
public sealed class StateMachineDefinition
{
    public const int MaximumStates = 256;
    public const int MaximumTransitions = 1_024;

    private readonly HashSet<ulong> _stateValues;
    private readonly HashSet<StateMachineTransition> _transitionValues;

    public StateMachineDefinition(
        ulong machine,
        IEnumerable<ulong> states,
        IEnumerable<StateMachineTransition> transitions)
    {
        ArgumentNullException.ThrowIfNull(states);
        ArgumentNullException.ThrowIfNull(transitions);

        ulong[] stateValues = states.ToArray();
        if (stateValues.Length == 0)
        {
            throw new ArgumentException("A state-machine definition must declare at least one state.", nameof(states));
        }
        if (stateValues.Length > MaximumStates)
        {
            throw new ArgumentOutOfRangeException(
                nameof(states),
                stateValues.Length,
                $"A state-machine definition may declare at most {MaximumStates} states.");
        }

        _stateValues = new HashSet<ulong>(stateValues.Length);
        foreach (ulong state in stateValues)
        {
            if (!_stateValues.Add(state))
            {
                throw new ArgumentException(
                    $"State-machine definition {machine} declares state {state} more than once.",
                    nameof(states));
            }
        }

        StateMachineTransition[] transitionValues = transitions.ToArray();
        if (transitionValues.Length > MaximumTransitions)
        {
            throw new ArgumentOutOfRangeException(
                nameof(transitions),
                transitionValues.Length,
                $"A state-machine definition may declare at most {MaximumTransitions} transitions.");
        }

        _transitionValues = new HashSet<StateMachineTransition>(transitionValues.Length);
        foreach (StateMachineTransition transition in transitionValues)
        {
            if (!_stateValues.Contains(transition.From) || !_stateValues.Contains(transition.To))
            {
                throw new ArgumentException(
                    $"Transition {transition.From}->{transition.To} refers to an undeclared state.",
                    nameof(transitions));
            }
            if (!_transitionValues.Add(transition))
            {
                throw new ArgumentException(
                    $"State-machine definition {machine} declares transition {transition.From}->{transition.To} more than once.",
                    nameof(transitions));
            }
        }

        Machine = machine;
        States = Array.AsReadOnly(stateValues.OrderBy(value => value).Select(value => new StateMachineState(value)).ToArray());
        Transitions = Array.AsReadOnly(_transitionValues
            .OrderBy(transition => transition.From)
            .ThenBy(transition => transition.To)
            .ToArray());
    }

    public ulong Machine { get; }

    /// <summary>Returns states in ascending value order for deterministic inspection.</summary>
    public IReadOnlyList<StateMachineState> States { get; }

    /// <summary>Returns directed edges in ascending from/to order for deterministic inspection.</summary>
    public IReadOnlyList<StateMachineTransition> Transitions { get; }

    public bool ContainsState(ulong state) => _stateValues.Contains(state);

    public bool AllowsTransition(ulong from, ulong to) =>
        _transitionValues.Contains(new StateMachineTransition(from, to));

    public StateMachineInstance CreateInstance(ulong current, ulong revision = 0)
    {
        EnsureState(current);
        return new StateMachineInstance(Machine, current, revision);
    }

    /// <summary>
    /// Applies one guarded transition to a caller-owned instance and returns a new value.
    ///
    /// No instance is retained or mutated. Every rejected request throws before a new value is
    /// produced, so the caller's state remains unchanged.
    /// </summary>
    public StateMachineTransitionReceipt Transition(
        StateMachineInstance instance,
        ulong expected,
        ulong next,
        ulong? expectedRevision = null)
    {
        if (instance.Machine != Machine)
        {
            throw new InvalidOperationException(
                $"State-machine instance belongs to machine {instance.Machine}, not {Machine}.");
        }
        EnsureState(next);
        if (!AllowsTransition(expected, next))
        {
            throw new InvalidOperationException(
                $"State-machine {Machine} does not allow transition {expected}->{next}.");
        }
        if (instance.Current != expected)
        {
            throw new InvalidOperationException(
                $"State-machine {Machine} expected state {expected}, but instance is in {instance.Current}.");
        }
        if (expectedRevision is ulong expectedValue && instance.Revision != expectedValue)
        {
            throw new InvalidOperationException(
                $"State-machine {Machine} expected revision {expectedValue}, but instance is at {instance.Revision}.");
        }
        if (instance.Revision == ulong.MaxValue)
        {
            throw new InvalidOperationException($"State-machine {Machine} revision is exhausted.");
        }

        ulong revision = instance.Revision + 1;
        StateMachineInstance updated = new(Machine, next, revision);
        return new StateMachineTransitionReceipt(updated, instance.Current, revision);
    }

    public StateMachineTransitionReceipt Transition(
        StateMachineInstance instance,
        StateMachineTransitionRequest request) =>
        Transition(instance, request.Expected, request.Next, request.ExpectedRevision);

    private void EnsureState(ulong state)
    {
        if (!_stateValues.Contains(state))
        {
            throw new InvalidOperationException($"State {state} is not declared by machine {Machine}.");
        }
    }
}
