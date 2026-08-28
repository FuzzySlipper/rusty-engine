using System.Runtime.ExceptionServices;

namespace Rusty.Engine.Resolution;

/// <summary>
/// Product-owned transaction work that is composed locally with an already-planned structural
/// resolution. It intentionally has no rollback guarantee for arbitrary product policy mutation.
/// </summary>
public interface IResolutionProductTransaction
{
    void Stage();
    void Commit();
    void Abort();
}

/// <summary>
/// A copied, bounded native structural readout. Product semantic payloads and policy evidence are
/// deliberately not represented here.
/// </summary>
public readonly record struct ResolutionStructuralReadout(
    ReadOnlyMemory<ResolutionAttemptReadoutRow> Attempts,
    ReadOnlyMemory<ResolutionTraceReadoutRow> Trace);

/// <summary>
/// Managed composition over one retained native structural-resolution session. The generated
/// service remains the sole owner of identity allocation, lineage, sequencing, and quotas.
/// </summary>
public sealed class StructuralResolutionSession : IDisposable
{
    private readonly IResolutionService _service;
    private readonly ResolutionSession _native;
    private readonly ResolutionMode _mode;
    private readonly Stack<ResolutionAttemptScope> _active = new();
    private readonly HashSet<ResolutionPhaseScope> _openPhases = [];
    private bool _prepared;
    private bool _terminal;
    private bool _disposed;

    private StructuralResolutionSession(
        IResolutionService service,
        ResolutionSession native,
        ResolutionSessionCreateRequest request)
    {
        _service = service;
        _native = native;
        _mode = request.Mode;
        Root = new ResolutionAttemptScope(
            this,
            new ResolutionIdentityRow(request.RootResolution, request.Correlation, 0, false, 0));
        _active.Push(Root);
    }

    public ResolutionAttemptScope Root { get; }

    public ResolutionMode Mode => _mode;

    public static StructuralResolutionSession Create(
        IResolutionService service,
        ResolutionSessionCreateRequest request)
    {
        ArgumentNullException.ThrowIfNull(service);
        return new StructuralResolutionSession(service, service.CreateSession(request), request);
    }

    /// <summary>Explicitly prepares a fully planned session for one product transaction outcome.</summary>
    public void PrepareFinalization()
    {
        ThrowIfDisposed();
        if (_terminal)
        {
            throw new InvalidOperationException("The resolution session already has a terminal outcome.");
        }
        if (_prepared)
        {
            return;
        }

        _service.PrepareFinalization(_native);
        _prepared = true;
    }

    /// <summary>
    /// Stages the product transaction once. Preview aborts it before recording the native preview
    /// outcome; Apply commits it once before recording native application. A stage, commit, or
    /// preview-abort failure attempts product abort and native failed finalization before it is
    /// rethrown.
    /// </summary>
    public void FinalizeProductTransaction(IResolutionProductTransaction transaction)
    {
        ThrowIfDisposed();
        ArgumentNullException.ThrowIfNull(transaction);
        PrepareFinalization();

        try
        {
            transaction.Stage();
        }
        catch (Exception failure)
        {
            FailTransaction(transaction, failure);
            ExceptionDispatchInfo.Capture(failure).Throw();
            throw;
        }

        if (_mode == ResolutionMode.Preview)
        {
            try
            {
                transaction.Abort();
            }
            catch (Exception failure)
            {
                FailTransaction(transaction, failure, abortAlreadyAttempted: true);
                ExceptionDispatchInfo.Capture(failure).Throw();
                throw;
            }

            _service.FinalizePreview(_native);
        }
        else
        {
            try
            {
                transaction.Commit();
            }
            catch (Exception failure)
            {
                FailTransaction(transaction, failure);
                ExceptionDispatchInfo.Capture(failure).Throw();
                throw;
            }

            _service.FinalizeApplied(_native);
        }

        _terminal = true;
    }

    /// <summary>Copies the generated bounded receipt and trace collection into managed arrays.</summary>
    public ResolutionStructuralReadout Readout()
    {
        ThrowIfDisposed();
        ResolutionSessionReadoutLeaseReceipt readout = _service.ReadSession(new ResolutionSessionReadRequest(_native));
        return new ResolutionStructuralReadout(readout.Attempts.ToArray(), readout.Traces.ToArray());
    }

    public void Dispose()
    {
        if (_disposed)
        {
            return;
        }

        _disposed = true;
        _native.Dispose();
    }

    internal ResolutionAttemptScope BeginChild(
        ResolutionAttemptScope parent,
        ResolutionStructuralBudget budget,
        uint evidence)
    {
        RequireActive(parent);
        ResolutionChildReceipt child = _service.BeginChild(new ResolutionBeginChildRequest(_native, budget, evidence));
        var scope = new ResolutionAttemptScope(this, child.Identity);
        _active.Push(scope);
        return scope;
    }

    internal ResolutionPhaseScope BeginPhase(ResolutionAttemptScope attempt, ResolutionPhase phase)
    {
        RequireActive(attempt);
        _service.BeginPhase(new ResolutionBeginPhaseRequest(_native, phase));
        var scope = new ResolutionPhaseScope(this, attempt, phase);
        _openPhases.Add(scope);
        return scope;
    }

    internal void CompletePhase(ResolutionAttemptScope attempt, ResolutionPhase phase)
    {
        RequireActive(attempt);
        _service.CompletePhase(new ResolutionBeginPhaseRequest(_native, phase));
        _openPhases.RemoveWhere(scope => scope.IsFor(attempt, phase));
    }

    internal void RecordPredicate(ResolutionAttemptScope attempt, ushort programDepth, bool passed)
    {
        RequireActive(attempt);
        _service.RecordPredicate(new ResolutionRecordPredicateRequest(_native, programDepth, passed));
    }

    internal void RecordSequence(ResolutionAttemptScope attempt, ushort programDepth)
    {
        RequireActive(attempt);
        _service.RecordSequence(new ResolutionRecordSequenceRequest(_native, programDepth));
    }

    internal void RecordOperation(ResolutionAttemptScope attempt, ushort programDepth, uint effects, uint events)
    {
        RequireActive(attempt);
        _service.RecordOperation(new ResolutionRecordOperationRequest(_native, programDepth, effects, events));
    }

    internal void RecordInterceptor(ResolutionAttemptScope attempt, uint effects, uint events)
    {
        RequireActive(attempt);
        _service.RecordInterceptor(new ResolutionRecordInterceptorRequest(_native, effects, events));
    }

    internal void CompleteAttempt(
        ResolutionAttemptScope attempt,
        ResolutionAttemptStatus status)
    {
        RequireActive(attempt);
        _service.CompleteAttempt(new ResolutionCompleteAttemptRequest(_native, status));
        if (status == ResolutionAttemptStatus.Planned)
        {
            _active.Pop();
            attempt.MarkCompleted();
            return;
        }

        foreach (ResolutionAttemptScope active in _active)
        {
            active.MarkCompleted();
        }
        foreach (ResolutionPhaseScope phase in _openPhases)
        {
            phase.MarkTerminated();
        }
        _openPhases.Clear();
        _active.Clear();
    }

    private void FailTransaction(
        IResolutionProductTransaction transaction,
        Exception primaryFailure,
        bool abortAlreadyAttempted = false)
    {
        List<Exception>? cleanupFailures = null;
        if (!abortAlreadyAttempted)
        {
            try
            {
                transaction.Abort();
            }
            catch (Exception abortFailure)
            {
                cleanupFailures = [abortFailure];
            }
        }

        try
        {
            _service.FinalizeFailed(_native);
            _terminal = true;
        }
        catch (Exception finalizeFailure)
        {
            (cleanupFailures ??= []).Add(finalizeFailure);
        }

        if (cleanupFailures is { Count: > 0 })
        {
            cleanupFailures.Insert(0, primaryFailure);
            throw new AggregateException("Resolution transaction failed and cleanup did not fully complete.", cleanupFailures);
        }
    }

    private void RequireActive(ResolutionAttemptScope attempt)
    {
        ThrowIfDisposed();
        if (_active.Count == 0 || !ReferenceEquals(_active.Peek(), attempt))
        {
            throw new InvalidOperationException("The attempt is not the active native resolution scope.");
        }
    }

    private void ThrowIfDisposed()
    {
        ObjectDisposedException.ThrowIf(_disposed, this);
    }
}

/// <summary>One root or native-allocated child attempt. Its identity is native evidence, not a managed allocation.</summary>
public sealed class ResolutionAttemptScope
{
    private readonly StructuralResolutionSession _session;
    private bool _completed;

    internal ResolutionAttemptScope(StructuralResolutionSession session, ResolutionIdentityRow identity)
    {
        _session = session;
        Identity = identity;
    }

    public ResolutionIdentityRow Identity { get; }

    public ResolutionAttemptScope BeginChild(ResolutionStructuralBudget budget, uint evidence)
    {
        ThrowIfCompleted();
        return _session.BeginChild(this, budget, evidence);
    }

    public ResolutionPhaseScope BeginPhase(ResolutionPhase phase)
    {
        ThrowIfCompleted();
        return _session.BeginPhase(this, phase);
    }

    public void ExecutePhase(ResolutionPhase phase, Action<ResolutionPhaseScope> action)
    {
        ArgumentNullException.ThrowIfNull(action);
        ResolutionPhaseScope scope = BeginPhase(phase);
        try
        {
            action(scope);
        }
        catch
        {
            scope.FaultIfActive();
            throw;
        }
        scope.Dispose();
    }

    public void Plan() => Complete(ResolutionAttemptStatus.Planned);
    public void Reject() => Complete(ResolutionAttemptStatus.Rejected);
    public void Suspend() => Complete(ResolutionAttemptStatus.Suspended);
    public void Fault() => Complete(ResolutionAttemptStatus.Faulted);

    public void Complete(ResolutionAttemptStatus status)
    {
        ThrowIfCompleted();
        _session.CompleteAttempt(this, status);
    }

    private void ThrowIfCompleted()
    {
        if (_completed)
        {
            throw new InvalidOperationException("The resolution attempt has already completed.");
        }
    }

    internal void MarkCompleted() => _completed = true;
}

/// <summary>Locally executes one native-validated structural phase; no callback crosses the ABI.</summary>
public sealed class ResolutionPhaseScope : IDisposable
{
    private readonly StructuralResolutionSession _session;
    private readonly ResolutionAttemptScope _attempt;
    private bool _completed;

    internal ResolutionPhaseScope(StructuralResolutionSession session, ResolutionAttemptScope attempt, ResolutionPhase phase)
    {
        _session = session;
        _attempt = attempt;
        Phase = phase;
    }

    public ResolutionPhase Phase { get; }

    /// <summary>Records one structural predicate node, without carrying product predicate data.</summary>
    public void RecordPredicate(ushort programDepth, bool passed)
    {
        ThrowIfCompleted();
        _session.RecordPredicate(_attempt, programDepth, passed);
    }

    /// <summary>Records one structural sequence node, without carrying a program tree.</summary>
    public void RecordSequence(ushort programDepth)
    {
        ThrowIfCompleted();
        _session.RecordSequence(_attempt, programDepth);
    }

    public void RecordOperation(ushort programDepth, uint effects, uint events)
    {
        ThrowIfCompleted();
        _session.RecordOperation(_attempt, programDepth, effects, events);
    }

    public void RecordInterceptor(uint effects, uint events)
    {
        ThrowIfCompleted();
        _session.RecordInterceptor(_attempt, effects, events);
    }

    /// <summary>Terminates the active attempt without emitting PhaseCompleted.</summary>
    public void Reject() => CompleteAttempt(ResolutionAttemptStatus.Rejected);

    /// <summary>Terminates the active attempt without emitting PhaseCompleted.</summary>
    public void Suspend() => CompleteAttempt(ResolutionAttemptStatus.Suspended);

    /// <summary>Terminates the active attempt without emitting PhaseCompleted.</summary>
    public void Fault() => CompleteAttempt(ResolutionAttemptStatus.Faulted);

    public void Dispose()
    {
        if (_completed)
        {
            return;
        }

        _session.CompletePhase(_attempt, Phase);
        _completed = true;
    }

    private void ThrowIfCompleted()
    {
        if (_completed)
        {
            throw new ObjectDisposedException(nameof(ResolutionPhaseScope));
        }
    }

    internal bool IsFor(ResolutionAttemptScope attempt, ResolutionPhase phase) =>
        ReferenceEquals(_attempt, attempt) && Phase == phase;

    internal void MarkTerminated() => _completed = true;

    internal void FaultIfActive()
    {
        if (!_completed)
        {
            Fault();
        }
    }

    private void CompleteAttempt(ResolutionAttemptStatus status)
    {
        ThrowIfCompleted();
        _session.CompleteAttempt(_attempt, status);
    }
}
