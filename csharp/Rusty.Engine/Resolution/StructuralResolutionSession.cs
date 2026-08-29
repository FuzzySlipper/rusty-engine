using System;
using System.Collections.Generic;
using System.Linq;

namespace Rusty.Engine.Resolution;

/// <summary>
/// Small managed coordinator for structural admission and transaction finalization.
///
/// Product code owns intent, facts, policy, and meaning. This class only keeps bounded attempt
/// lineage, guarded terminal outcomes, and an optional product transaction boundary.
/// </summary>
public sealed class StructuralResolutionSession
{
    private readonly ResolutionLimits _limits;
    private readonly List<ResolutionAttemptState> _attempts;
    private readonly Stack<ResolutionAttemptState> _active;
    private ulong _nextResolution;
    private ResolutionCommitStatus _commit = ResolutionCommitStatus.NotAttempted;

    public StructuralResolutionSession(
        ulong rootResolution,
        ulong correlation,
        ResolutionMode mode,
        ResolutionLimits? limits = null,
        ResolutionBudget? rootBudget = null,
        int rootEvidence = 0)
    {
        _limits = limits ?? ResolutionLimits.Default;
        _limits.Validate();
        ResolutionBudget budget = rootBudget ?? ResolutionBudget.From(_limits);
        budget.Validate(_limits);
        EnsureWithin(rootEvidence, budget.MaxEvidence, nameof(rootEvidence));

        if (rootResolution == ulong.MaxValue)
        {
            throw new ArgumentOutOfRangeException(nameof(rootResolution), "The resolution identity space is exhausted.");
        }

        Root = new ResolutionAttemptScope(this, new ResolutionAttemptState
        {
            Identity = ResolutionIdentity.Root(rootResolution, correlation),
            Budget = budget,
            IsRoot = true,
            Status = ResolutionAttemptStatus.Open,
            Counts = new ResolutionAttemptCounts(rootEvidence, 0, 0, 0, 0),
        });
        _attempts = [Root.State];
        _active = new Stack<ResolutionAttemptState>();
        _active.Push(Root.State);
        _nextResolution = rootResolution + 1;
        Mode = mode;
    }

    public ResolutionMode Mode { get; }

    public ResolutionLimits Limits => _limits;

    public ResolutionAttemptScope Root { get; }

    public ResolutionCommitStatus CommitStatus => _commit;

    /// <summary>Returns the current bounded structural state in admission order.</summary>
    public ResolutionReceipt Readout() =>
        new(
            _attempts.Select(attempt => new ResolutionAttemptReceipt(
                attempt.Identity,
                attempt.IsRoot,
                attempt.Status,
                attempt.Counts)).ToArray(),
            _commit);

    /// <summary>
    /// Finalizes one fully planned root transaction. Preview stages and aborts; Apply stages and
    /// commits. A stage, commit, or abort failure records TransactionFailed and makes no second
    /// finalization possible.
    /// </summary>
    public ResolutionReceipt Finalize(IResolutionTransaction transaction)
    {
        ArgumentNullException.ThrowIfNull(transaction);
        EnsureNotFinalized();
        if (_active.Count != 0 || Root.Status != ResolutionAttemptStatus.Planned)
        {
            throw new InvalidOperationException("A resolution session can finalize only after its root is planned.");
        }

        bool abortAttempted = false;
        try
        {
            transaction.Stage();
            if (Mode == ResolutionMode.Preview)
            {
                abortAttempted = true;
                transaction.Abort();
                _commit = ResolutionCommitStatus.Previewed;
            }
            else
            {
                transaction.Commit();
                _commit = ResolutionCommitStatus.Applied;
            }
        }
        catch
        {
            if (!abortAttempted)
            {
                TryAbort(transaction);
            }
            _commit = ResolutionCommitStatus.TransactionFailed;
        }

        return Readout();
    }

    internal ResolutionAttemptScope BeginChild(
        ResolutionAttemptScope parent,
        ResolutionBudget budget,
        int evidence)
    {
        EnsureNotFinalized();
        RequireActive(parent);
        budget.Validate(_limits);
        EnsureWithin(evidence, budget.MaxEvidence, nameof(evidence));

        ResolutionAttemptState parentState = parent.State;
        if (parentState.Status != ResolutionAttemptStatus.Open)
        {
            throw new InvalidOperationException("Only an open attempt can admit a child.");
        }
        if (parentState.Identity.Depth >= _limits.MaxChildDepth)
        {
            throw new InvalidOperationException($"Resolution child depth exceeds {_limits.MaxChildDepth}.");
        }
        if (parentState.Counts.Children >= parentState.Budget.MaxChildren)
        {
            throw new InvalidOperationException($"Resolution child count exceeds {parentState.Budget.MaxChildren}.");
        }
        if (_attempts.Count - 1 >= _limits.MaxChildResolutions)
        {
            throw new InvalidOperationException($"Resolution child count exceeds {_limits.MaxChildResolutions}.");
        }

        ResolutionIdentity identity = parentState.Identity.Child(_nextResolution);
        if (_nextResolution == ulong.MaxValue)
        {
            throw new InvalidOperationException("Resolution identity space is exhausted.");
        }

        ResolutionAttemptState childState = new()
        {
            Identity = identity,
            Budget = budget,
            IsRoot = false,
            Status = ResolutionAttemptStatus.Open,
            Counts = new ResolutionAttemptCounts(evidence, 0, 0, 0, 0),
        };
        parentState.Counts = parentState.Counts with { Children = parentState.Counts.Children + 1 };
        _attempts.Add(childState);
        _active.Push(childState);
        _nextResolution++;
        return new ResolutionAttemptScope(this, childState);
    }

    internal void Record(
        ResolutionAttemptScope attempt,
        int work,
        int effects,
        int events)
    {
        EnsureNotFinalized();
        RequireActive(attempt);
        if (work < 0 || effects < 0 || events < 0)
        {
            throw new ArgumentOutOfRangeException(nameof(work), "Resolution counters cannot be negative.");
        }

        ResolutionAttemptState state = attempt.State;
        if (state.Status != ResolutionAttemptStatus.Open)
        {
            throw new InvalidOperationException("Only an open attempt can record structural work.");
        }
        int nextWork = CheckedAdd(state.Counts.Work, work, nameof(work));
        int nextEffects = CheckedAdd(state.Counts.Effects, effects, nameof(effects));
        int nextEvents = CheckedAdd(state.Counts.Events, events, nameof(events));
        EnsureWithin(nextWork, state.Budget.MaxWork, nameof(work));
        EnsureWithin(nextEffects, state.Budget.MaxEffects, nameof(effects));
        EnsureWithin(nextEvents, state.Budget.MaxEvents, nameof(events));
        state.Counts = state.Counts with { Work = nextWork, Effects = nextEffects, Events = nextEvents };
    }

    internal void Complete(ResolutionAttemptScope attempt, ResolutionAttemptStatus status)
    {
        EnsureNotFinalized();
        RequireActive(attempt);
        if (status == ResolutionAttemptStatus.Open)
        {
            throw new ArgumentOutOfRangeException(nameof(status), "An attempt must complete with a terminal status.");
        }

        ResolutionAttemptState state = attempt.State;
        if (state.Status != ResolutionAttemptStatus.Open)
        {
            throw new InvalidOperationException("The resolution attempt has already completed.");
        }
        if (status == ResolutionAttemptStatus.Planned)
        {
            ResolutionAttemptState? child = _active.Count > 0 ? _active.Peek() : null;
            if (!ReferenceEquals(child, state))
            {
                throw new InvalidOperationException("An attempt cannot be planned while a child remains active.");
            }
            if (state.IsRoot && _active.Count != 1)
            {
                throw new InvalidOperationException("The root cannot be planned while a child remains active.");
            }
        }

        if (status == ResolutionAttemptStatus.Planned)
        {
            ResolutionAttemptState? parentBeforePop = _active.Count > 1 ? _active.Skip(1).First() : null;
            int work = 0;
            int effects = 0;
            int events = 0;
            if (parentBeforePop is not null)
            {
                work = CheckedAdd(parentBeforePop.Counts.Work, state.Counts.Work, nameof(ResolutionAttemptCounts.Work));
                effects = CheckedAdd(parentBeforePop.Counts.Effects, state.Counts.Effects, nameof(ResolutionAttemptCounts.Effects));
                events = CheckedAdd(parentBeforePop.Counts.Events, state.Counts.Events, nameof(ResolutionAttemptCounts.Events));
                EnsureWithin(work, parentBeforePop.Budget.MaxWork, nameof(ResolutionAttemptCounts.Work));
                EnsureWithin(effects, parentBeforePop.Budget.MaxEffects, nameof(ResolutionAttemptCounts.Effects));
                EnsureWithin(events, parentBeforePop.Budget.MaxEvents, nameof(ResolutionAttemptCounts.Events));
            }

            state.Status = status;
            _active.Pop();
            if (_active.TryPeek(out ResolutionAttemptState? parentAfterPop))
            {
                parentAfterPop.Counts = parentAfterPop.Counts with { Work = work, Effects = effects, Events = events };
            }
            return;
        }

        state.Status = status;
        _active.Pop();

        if (_active.Count != 0)
        {
            foreach (ResolutionAttemptState ancestor in _active)
            {
                ancestor.Status = ResolutionAttemptStatus.ChildFailed;
            }
            _active.Clear();
        }
    }

    private void RequireActive(ResolutionAttemptScope attempt)
    {
        if (_active.Count == 0 || !ReferenceEquals(_active.Peek(), attempt.State))
        {
            throw new InvalidOperationException("The attempt is not the active resolution scope.");
        }
    }

    private void EnsureNotFinalized()
    {
        if (_commit != ResolutionCommitStatus.NotAttempted)
        {
            throw new InvalidOperationException("The resolution session already has a terminal commit outcome.");
        }
    }

    private static void TryAbort(IResolutionTransaction transaction)
    {
        try
        {
            transaction.Abort();
        }
        catch
        {
            // A transaction's cleanup is best effort; the structural outcome remains failed.
        }
    }

    private static int CheckedAdd(int left, int right, string name)
    {
        try
        {
            return checked(left + right);
        }
        catch (OverflowException)
        {
            throw new InvalidOperationException($"Resolution counter {name} overflowed.");
        }
    }

    private static void EnsureWithin(int value, int maximum, string name)
    {
        if (value < 0 || value > maximum)
        {
            throw new InvalidOperationException($"Resolution counter {name} value {value} exceeds maximum {maximum}.");
        }
    }
}

/// <summary>One product-owned root or child structural attempt.</summary>
public sealed class ResolutionAttemptScope
{
    private readonly StructuralResolutionSession _session;

    internal ResolutionAttemptScope(StructuralResolutionSession session, ResolutionAttemptState state)
    {
        _session = session;
        State = state;
    }

    internal ResolutionAttemptState State { get; }

    public ResolutionIdentity Identity => State.Identity;

    public ResolutionAttemptStatus Status => State.Status;

    public ResolutionAttemptCounts Counts => State.Counts;

    public ResolutionAttemptScope BeginChild(
        ResolutionBudget budget,
        int evidence = 0) =>
        _session.BeginChild(this, budget, evidence);

    public void Record(int work = 1, int effects = 0, int events = 0) =>
        _session.Record(this, work, effects, events);

    public void Complete(ResolutionAttemptStatus status = ResolutionAttemptStatus.Planned) =>
        _session.Complete(this, status);
}

internal sealed class ResolutionAttemptState
{
    public required ResolutionIdentity Identity { get; init; }
    public required ResolutionBudget Budget { get; init; }
    public bool IsRoot { get; init; }
    public ResolutionAttemptStatus Status { get; set; }
    public ResolutionAttemptCounts Counts { get; set; }
}
