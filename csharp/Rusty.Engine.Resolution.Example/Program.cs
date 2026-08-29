using System;
using System.Linq;

using Rusty.Engine.Resolution;
using Rusty.Engine.StateMachine;

const int MaxEvidence = 8;
const int MaxWork = 8;
const int MaxEffects = 4;
const int MaxEvents = 4;
const int MaxChildren = 2;
const int MaxChildDepth = 2;
const int SingleChild = 1;
const ulong MachineId = 10;
const ulong RootResolution = 100;
const ulong Correlation = 900;
const int RootEvidence = 1;
const int TransactionAmount = 5;
const int InitialState = 1;
const int MovingState = 2;
const int StoppedState = 3;

ExerciseManagedStateMachine();
ExercisePreviewAndApply();
ExerciseChildFailureAndBounds();
ExerciseTransactionFailure();

static void ExerciseManagedStateMachine()
{
    var definition = new StateMachineDefinition(
        machine: MachineId,
        states: [StoppedState, InitialState, MovingState],
        transitions:
        [
            new StateMachineTransition(MovingState, StoppedState),
            new StateMachineTransition(InitialState, MovingState),
        ]);

    Require(definition.States.Select(state => state.Value).SequenceEqual(new ulong[] { InitialState, MovingState, StoppedState }),
        "state-machine states were not deterministic");
    Require(definition.AllowsTransition(InitialState, MovingState),
        "state-machine transition was not admitted");

    StateMachineInstance initial = definition.CreateInstance(InitialState);
    StateMachineTransitionReceipt applied = definition.Transition(
        initial,
        new StateMachineTransitionRequest(InitialState, MovingState, ExpectedRevision: 0));
    Require(applied.Instance == new StateMachineInstance(MachineId, MovingState, 1)
        && initial == definition.CreateInstance(InitialState),
        "state-machine transition did not return a new caller-owned value");

    Throws(() => definition.Transition(
        applied.Instance,
        new StateMachineTransitionRequest(InitialState, MovingState, ExpectedRevision: 0)),
        "stale state-machine transition was accepted");
    Require(applied.Instance.Current == MovingState && applied.Instance.Revision == 1,
        "rejected state-machine transition mutated the instance");
}

static void ExercisePreviewAndApply()
{
    EncounterState previewState = new();
    StructuralResolutionSession preview = CreateSession(ResolutionMode.Preview);
    preview.Root.Record(work: 1, effects: 1, events: 1);
    ResolutionAttemptScope child = preview.Root.BeginChild(ChildBudget(), evidence: 1);
    child.Record(work: 1, effects: 1, events: 1);
    child.Complete();
    preview.Root.Complete();

    var previewTransaction = new EncounterTransaction(previewState, TransactionAmount);
    ResolutionReceipt previewReceipt = preview.Finalize(previewTransaction);
    Require(previewReceipt.Commit == ResolutionCommitStatus.Previewed
        && previewState.HitPoints == 0
        && previewTransaction.StageCalls == 1
        && previewTransaction.CommitCalls == 0
        && previewTransaction.AbortCalls == 1,
        "preview did not leave product state unchanged");
    Require(previewReceipt.Attempts.Length == 2
        && previewReceipt.Attempts.Span[1].Identity.Parent == previewReceipt.Attempts.Span[0].Identity.Resolution,
        "child lineage was not preserved in the managed receipt");

    EncounterState applyState = new();
    StructuralResolutionSession apply = CreateSession(ResolutionMode.Apply);
    apply.Root.Record(work: 1, effects: 1, events: 1);
    apply.Root.Complete();
    var applyTransaction = new EncounterTransaction(applyState, TransactionAmount);
    ResolutionReceipt applyReceipt = apply.Finalize(applyTransaction);
    Require(applyReceipt.Commit == ResolutionCommitStatus.Applied
        && applyState.HitPoints == 5
        && applyTransaction.StageCalls == 1
        && applyTransaction.CommitCalls == 1
        && applyTransaction.AbortCalls == 0,
        "apply did not commit the product transaction once");
}

static void ExerciseChildFailureAndBounds()
{
    StructuralResolutionSession failed = CreateSession(ResolutionMode.Apply);
    ResolutionAttemptScope child = failed.Root.BeginChild(ChildBudget(), evidence: 1);
    child.Complete(ResolutionAttemptStatus.Rejected);
    ResolutionReceipt failedReceipt = failed.Readout();
    Require(failedReceipt.Attempts.Span[0].Status == ResolutionAttemptStatus.ChildFailed,
        "child failure did not close the managed parent");
    Throws(() => failed.Root.Complete(), "child-failed root could still be completed");

    StructuralResolutionSession bounded = CreateSession(ResolutionMode.Apply);
    ResolutionAttemptScope firstChild = bounded.Root.BeginChild(ChildBudget(), evidence: 1);
    firstChild.Complete();
    int admittedBeforeRejectedChild = bounded.Readout().Attempts.Length;
    Throws(() => bounded.Root.BeginChild(
        ChildBudget() with { MaxChildren = SingleChild },
        evidence: MaxEvidence + 1),
        "over-limit child admission was accepted");
    Require(bounded.Readout().Attempts.Length == admittedBeforeRejectedChild,
        "rejected child admission mutated the session");
}

static void ExerciseTransactionFailure()
{
    StructuralResolutionSession session = CreateSession(ResolutionMode.Apply);
    session.Root.Complete();
    EncounterTransaction transaction = new(new EncounterState(), TransactionAmount) { ThrowDuringStage = true };

    ResolutionReceipt receipt = session.Finalize(transaction);
    Require(receipt.Commit == ResolutionCommitStatus.TransactionFailed
        && transaction.StageCalls == 1
        && transaction.AbortCalls == 1
        && transaction.CommitCalls == 0,
        "failed transaction did not report failure and attempt cleanup");
}

static StructuralResolutionSession CreateSession(ResolutionMode mode) =>
    new(
        rootResolution: RootResolution,
        correlation: Correlation,
        mode,
        new ResolutionLimits(MaxEvidence, MaxWork, MaxEffects, MaxEvents, MaxChildren, MaxChildDepth),
        RootBudget(),
        rootEvidence: RootEvidence);

static ResolutionBudget RootBudget() => new(MaxEvidence, MaxWork, MaxEffects, MaxEvents, MaxChildren);
static ResolutionBudget ChildBudget() => new(MaxEvidence, MaxWork, MaxEffects, MaxEvents, MaxChildren);

static void Require(bool condition, string message)
{
    if (!condition)
    {
        throw new InvalidOperationException(message);
    }
}

static void Throws(Action action, string message)
{
    try
    {
        action();
    }
    catch (Exception)
    {
        return;
    }

    throw new InvalidOperationException(message);
}

sealed class EncounterState
{
    public int HitPoints { get; set; }
}

sealed class EncounterTransaction(EncounterState state, int amount) : IResolutionTransaction
{
    private int _pending;

    public bool ThrowDuringStage { get; init; }
    public int StageCalls { get; private set; }
    public int CommitCalls { get; private set; }
    public int AbortCalls { get; private set; }

    public void Stage()
    {
        StageCalls++;
        if (ThrowDuringStage)
        {
            throw new InvalidOperationException("staging failed");
        }
        _pending = amount;
    }

    public void Commit()
    {
        CommitCalls++;
        state.HitPoints += _pending;
        _pending = 0;
    }

    public void Abort()
    {
        AbortCalls++;
        _pending = 0;
    }
}
