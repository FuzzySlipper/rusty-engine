namespace Rusty.Engine.StateMachine;

/// <summary>A caller-owned state-machine instance. The Engine never stores this value.</summary>
public readonly record struct StateMachineInstance(ulong Machine, ulong Current, ulong Revision);

/// <summary>One state identity in a state-machine definition.</summary>
public readonly record struct StateMachineState(ulong Value);

/// <summary>One directed edge in a state-machine definition.</summary>
public readonly record struct StateMachineTransition(ulong From, ulong To);

/// <summary>A guarded state transition supplied by product/domain code.</summary>
public readonly record struct StateMachineTransitionRequest(
    ulong Expected,
    ulong Next,
    ulong? ExpectedRevision = null);

/// <summary>Fixed receipt for one successful caller-owned transition.</summary>
public readonly record struct StateMachineTransitionReceipt(
    StateMachineInstance Instance,
    ulong Previous,
    ulong Revision);
