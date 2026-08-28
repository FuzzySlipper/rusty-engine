using Rusty.Engine;
using Rusty.Engine.Resolution;

ExercisePreview();
ExerciseApplyWithChildLineage();
ExerciseRejectionAndInvalidOrdering();
ExerciseLocalPolicyFaultAndChildFailure();
ExerciseTransactionFailure();

static void ExercisePreview()
{
    var service = new ResolutionServiceFixture();
    var state = new EncounterState();
    using StructuralResolutionSession session = CreateSession(service, ResolutionMode.Preview);
    PlanRoot(session.Root, includeChild: false);

    var transaction = new EncounterTransaction(state, [new HealEffect("cleric", 3)]);
    session.FinalizeProductTransaction(transaction);
    ResolutionStructuralReadout readout = session.Readout();

    Require(state.HitPoints == 0, "preview changed live product state");
    Require(transaction.StageCalls == 1 && transaction.CommitCalls == 0 && transaction.AbortCalls == 1,
        "preview did not stage once and abort once");
    Require(readout.Attempts.Span[0].Commit == ResolutionCommitStatus.Previewed,
        "preview receipt did not report the native preview terminal state");
}

static void ExerciseApplyWithChildLineage()
{
    var service = new ResolutionServiceFixture();
    var state = new EncounterState();
    using StructuralResolutionSession session = CreateSession(service, ResolutionMode.Apply);
    PlanRoot(session.Root, includeChild: true);

    var transaction = new EncounterTransaction(state, [new HealEffect("cleric", 3), new HealEffect("sprite", 2)]);
    session.FinalizeProductTransaction(transaction);
    ResolutionStructuralReadout readout = session.Readout();

    Require(state.HitPoints == 5, "apply did not publish the typed product effects");
    Require(transaction.StageCalls == 1 && transaction.CommitCalls == 1 && transaction.AbortCalls == 0,
        "apply did not stage and commit exactly once");
    Require(readout.Attempts.Length == 2, "child attempt was absent from the bounded receipt");
    ResolutionIdentityRow root = readout.Attempts.Span[0].Identity;
    ResolutionIdentityRow child = readout.Attempts.Span[1].Identity;
    Require(child.Correlation == root.Correlation && child.HasParent && child.Parent == root.Resolution && child.Depth == 1,
        "native child identity did not preserve correlation and parent lineage");
    Require(readout.Attempts.Span[0].Effects == 2 && readout.Attempts.Span[0].Events == 2,
        "child totals did not aggregate into the root receipt");
    Require(readout.Attempts.Span[0].Commit == ResolutionCommitStatus.Applied,
        "apply receipt did not report native application");
}

static void ExerciseRejectionAndInvalidOrdering()
{
    var state = new EncounterState();
    var rejectedService = new ResolutionServiceFixture();
    using (StructuralResolutionSession rejected = CreateSession(rejectedService, ResolutionMode.Apply))
    {
        rejected.Root.ExecutePhase(ResolutionPhase.Admit, _ => { });
        rejected.Root.Reject();
        Require(rejected.Readout().Attempts.Span[0].Status == ResolutionAttemptStatus.Rejected,
            "rejection did not reach the structural receipt");
    }

    var invalidService = new ResolutionServiceFixture();
    using (StructuralResolutionSession invalid = CreateSession(invalidService, ResolutionMode.Apply))
    {
        Throws(() => invalid.Root.BeginPhase(ResolutionPhase.Plan), "invalid phase order was admitted");
    }

    Require(state.HitPoints == 0, "rejection or invalid ordering changed product state");
}

static void ExerciseTransactionFailure()
{
    var service = new ResolutionServiceFixture();
    var state = new EncounterState();
    using StructuralResolutionSession session = CreateSession(service, ResolutionMode.Apply);
    PlanRoot(session.Root, includeChild: false);
    var transaction = new EncounterTransaction(state, [new HealEffect("cleric", 3)]) { ThrowDuringStage = true };

    Throws(() => session.FinalizeProductTransaction(transaction), "stage failure did not reach the caller");
    ResolutionStructuralReadout readout = session.Readout();
    Require(state.HitPoints == 0 && transaction.AbortCalls == 1,
        "failed product transaction left live state or skipped abort");
    Require(readout.Attempts.Span[0].Commit == ResolutionCommitStatus.TransactionFailed,
        "failed product transaction was not recorded in the native receipt");
}

static void ExerciseLocalPolicyFaultAndChildFailure()
{
    var faultService = new ResolutionServiceFixture();
    using (StructuralResolutionSession faulted = CreateSession(faultService, ResolutionMode.Apply))
    {
        Throws(
            () => faulted.Root.ExecutePhase(ResolutionPhase.Admit, _ => throw new InvalidOperationException("typed policy fault")),
            "typed policy fault did not return to the product");
        ResolutionStructuralReadout readout = faulted.Readout();
        Require(readout.Attempts.Span[0].Status == ResolutionAttemptStatus.Faulted,
            $"active policy fault was not recorded as a faulted attempt ({readout.Attempts.Span[0].Status})");
        Require(readout.Trace.ToArray().Any(row => row.Phase == ResolutionPhase.Admit && row.Kind == ResolutionTraceKind.Faulted),
            "active policy fault did not produce a faulted trace");
        Require(!readout.Trace.ToArray().Any(row => row.Phase == ResolutionPhase.Admit && row.Kind == ResolutionTraceKind.PhaseCompleted),
            "active policy fault incorrectly completed its phase");
    }

    var childService = new ResolutionServiceFixture();
    using (StructuralResolutionSession childFailure = CreateSession(childService, ResolutionMode.Apply))
    {
        ResolutionAttemptScope root = childFailure.Root;
        root.ExecutePhase(ResolutionPhase.Admit, _ => { });
        root.ExecutePhase(ResolutionPhase.Gather, _ => { });
        root.ExecutePhase(ResolutionPhase.Check, _ => { });
        root.ExecutePhase(ResolutionPhase.Plan, phase =>
        {
            phase.RecordSequence(1);
            phase.RecordPredicate(2, true);
            phase.RecordOperation(2, 1, 1);
        });
        using (ResolutionPhaseScope beforeCommit = root.BeginPhase(ResolutionPhase.BeforeCommit))
        {
            ResolutionAttemptScope child = root.BeginChild(Budget(maxChildren: 1), evidence: 1);
            child.ExecutePhase(ResolutionPhase.Admit, phase => phase.Reject());
        }

        Throws(() => root.BeginPhase(ResolutionPhase.BeforeCommit), "failed child left a usable managed parent continuation");
        Require(childFailure.Readout().Attempts.Span[0].Status == ResolutionAttemptStatus.ChildFailed,
            "child rejection did not propagate to the native root receipt");
    }
}

static StructuralResolutionSession CreateSession(ResolutionServiceFixture service, ResolutionMode mode) =>
    StructuralResolutionSession.Create(service, new ResolutionSessionCreateRequest(
        RootResolution: 100,
        Correlation: 900,
        Mode: mode,
        Limits: new ResolutionLimits(8, 8, 4, 4, 8, 8, 64, 4, 2),
        RootBudget: Budget(maxChildren: 2),
        RootEvidence: 1));

static ResolutionStructuralBudget Budget(uint maxChildren) => new(8, 8, 4, 4, 8, 8, 64, maxChildren);

static void PlanRoot(ResolutionAttemptScope root, bool includeChild)
{
    root.ExecutePhase(ResolutionPhase.Admit, _ => { });
    root.ExecutePhase(ResolutionPhase.Gather, _ => { });
    root.ExecutePhase(ResolutionPhase.Check, _ => { });
    root.ExecutePhase(ResolutionPhase.Plan, phase =>
    {
        phase.RecordSequence(programDepth: 1);
        phase.RecordPredicate(programDepth: 2, passed: true);
        phase.RecordOperation(programDepth: 1, effects: 1, events: 1);
    });
    using (ResolutionPhaseScope beforeCommit = root.BeginPhase(ResolutionPhase.BeforeCommit))
    {
        // The structural owner applies interceptors before child traversal. A successful child
        // contributes its totals to this root receipt; product code must not rewrite totals after.
        beforeCommit.RecordInterceptor(effects: 1, events: 1);
        if (includeChild)
        {
            ResolutionAttemptScope child = root.BeginChild(Budget(maxChildren: 1), evidence: 1);
            PlanChild(child);
        }
    }
    root.Plan();
}

static void PlanChild(ResolutionAttemptScope child)
{
    child.ExecutePhase(ResolutionPhase.Admit, _ => { });
    child.ExecutePhase(ResolutionPhase.Gather, _ => { });
    child.ExecutePhase(ResolutionPhase.Check, _ => { });
    child.ExecutePhase(ResolutionPhase.Plan, phase =>
    {
        phase.RecordSequence(programDepth: 1);
        phase.RecordPredicate(programDepth: 2, passed: true);
        phase.RecordOperation(programDepth: 1, effects: 1, events: 1);
    });
    child.ExecutePhase(ResolutionPhase.BeforeCommit, phase => phase.RecordInterceptor(effects: 1, events: 1));
    child.Plan();
}

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
    catch (InvalidOperationException)
    {
        return;
    }
    throw new InvalidOperationException(message);
}

sealed record HealEffect(string Source, int Amount);

sealed class EncounterState
{
    public int HitPoints { get; set; }
}

sealed class EncounterTransaction(EncounterState state, IReadOnlyList<HealEffect> effects) : IResolutionProductTransaction
{
    private List<HealEffect>? _staged;

    public bool ThrowDuringStage { get; init; }
    public int StageCalls { get; private set; }
    public int CommitCalls { get; private set; }
    public int AbortCalls { get; private set; }

    public void Stage()
    {
        StageCalls++;
        if (ThrowDuringStage)
        {
            throw new InvalidOperationException("typed product candidate rejected staging");
        }
        _staged = effects.ToList();
    }

    public void Commit()
    {
        CommitCalls++;
        foreach (HealEffect effect in _staged ?? throw new InvalidOperationException("commit without staging"))
        {
            state.HitPoints += effect.Amount;
        }
        _staged = null;
    }

    public void Abort()
    {
        AbortCalls++;
        _staged = null;
    }
}

sealed class ResolutionServiceFixture : IResolutionService
{
    private readonly Dictionary<ulong, SessionState> _sessions = [];
    private ulong _nextSession = 1;

    public ResolutionSession CreateSession(ResolutionSessionCreateRequest request)
    {
        var session = new ResolutionSession(new ResolutionSessionHandle(_nextSession++), () => { });
        _sessions.Add(session.Handle.Value, new SessionState(request));
        return session;
    }

    public void BeginPhase(ResolutionBeginPhaseRequest request)
    {
        AttemptState attempt = Current(request.Session);
        if (attempt.ActivePhase is not null || PlanningPhases[attempt.NextPhase] != request.Phase)
        {
            throw new InvalidOperationException("native fixture rejected the structural phase ordering");
        }
        attempt.ActivePhase = request.Phase;
        State(request.Session).Trace.Add(new ResolutionTraceReadoutRow(attempt.Identity, request.Phase, ResolutionTraceKind.PhaseStarted, 0, false));
    }

    public void CompletePhase(ResolutionBeginPhaseRequest request)
    {
        AttemptState attempt = Current(request.Session);
        if (attempt.ActivePhase != request.Phase)
        {
            throw new InvalidOperationException("native fixture rejected completion of an inactive phase");
        }
        attempt.ActivePhase = null;
        attempt.NextPhase++;
        State(request.Session).Trace.Add(new ResolutionTraceReadoutRow(attempt.Identity, request.Phase, ResolutionTraceKind.PhaseCompleted, 0, false));
    }

    public void RecordPredicate(ResolutionRecordPredicateRequest request)
    {
        AttemptState attempt = RequirePhase(request.Session, ResolutionPhase.Plan);
        attempt.ProgramNodes++;
        attempt.ProgramDepth = Math.Max(attempt.ProgramDepth, request.ProgramDepth);
    }

    public void RecordSequence(ResolutionRecordSequenceRequest request)
    {
        AttemptState attempt = RequirePhase(request.Session, ResolutionPhase.Plan);
        attempt.ProgramNodes++;
        attempt.ProgramDepth = Math.Max(attempt.ProgramDepth, request.ProgramDepth);
    }

    public void RecordOperation(ResolutionRecordOperationRequest request)
    {
        AttemptState attempt = RequirePhase(request.Session, ResolutionPhase.Plan);
        attempt.ProgramNodes++;
        attempt.ProgramDepth = Math.Max(attempt.ProgramDepth, request.ProgramDepth);
        attempt.Effects += request.Effects;
        attempt.Events += request.Events;
    }

    public void RecordInterceptor(ResolutionRecordInterceptorRequest request)
    {
        AttemptState attempt = RequirePhase(request.Session, ResolutionPhase.BeforeCommit);
        attempt.Interceptors++;
        attempt.Effects = request.Effects;
        attempt.Events = request.Events;
    }

    public ResolutionChildReceipt BeginChild(ResolutionBeginChildRequest request)
    {
        SessionState state = State(request.Session);
        AttemptState parent = RequirePhase(request.Session, ResolutionPhase.BeforeCommit);
        ulong childResolution = state.Attempts.Max(attempt => attempt.Identity.Resolution) + 1;
        var identity = new ResolutionIdentityRow(childResolution, parent.Identity.Correlation, parent.Identity.Resolution, true,
            checked((ushort)(parent.Identity.Depth + 1)));
        state.Attempts.Add(new AttemptState(identity, isRoot: false, request.Evidence));
        state.Active.Push(state.Attempts.Count - 1);
        parent.Children++;
        return new ResolutionChildReceipt(identity);
    }

    public void CompleteAttempt(ResolutionCompleteAttemptRequest request)
    {
        SessionState state = State(request.Session);
        AttemptState attempt = Current(request.Session);
        if (request.Status == ResolutionAttemptStatus.Open ||
            (request.Status == ResolutionAttemptStatus.Planned && attempt.ActivePhase is not null))
        {
            throw new InvalidOperationException("native fixture rejected incomplete attempt completion");
        }
        attempt.Status = request.Status;
        ResolutionPhase tracePhase = attempt.ActivePhase ?? PlanningPhases[Math.Max(0, attempt.NextPhase - 1)];
        if (request.Status != ResolutionAttemptStatus.Planned)
        {
            state.Trace.Add(new ResolutionTraceReadoutRow(attempt.Identity, tracePhase, TraceKind(request.Status), 0, false));
            foreach (int active in state.Active)
            {
                state.Attempts[active].Status = active == state.Active.Peek()
                    ? request.Status
                    : ResolutionAttemptStatus.ChildFailed;
            }
            state.Active.Clear();
            return;
        }

        state.Active.Pop();
        if (state.Active.Count > 0)
        {
            AttemptState parent = state.Attempts[state.Active.Peek()];
            parent.Effects += attempt.Effects;
            parent.Events += attempt.Events;
        }
    }

    public void PrepareFinalization(ResolutionSession session)
    {
        SessionState state = State(session);
        if (state.Active.Count != 0 || state.Attempts[0].Status != ResolutionAttemptStatus.Planned)
        {
            throw new InvalidOperationException("native fixture rejected finalization before the root plan");
        }
        state.Commit = ResolutionCommitStatus.Prepared;
    }

    public void FinalizePreview(ResolutionSession session) => Finalize(session, ResolutionMode.Preview, ResolutionCommitStatus.Previewed);
    public void FinalizeApplied(ResolutionSession session) => Finalize(session, ResolutionMode.Apply, ResolutionCommitStatus.Applied);

    public void FinalizeFailed(ResolutionSession session)
    {
        SessionState state = State(session);
        if (state.Commit != ResolutionCommitStatus.Prepared)
        {
            throw new InvalidOperationException("native fixture rejected failure finalization before prepare");
        }
        state.Commit = ResolutionCommitStatus.TransactionFailed;
    }

    public ResolutionSessionReadoutLeaseReceipt ReadSession(ResolutionSessionReadRequest request)
    {
        SessionState state = State(request.Session);
        ResolutionAttemptReadoutRow[] attempts = state.Attempts.Select(attempt => new ResolutionAttemptReadoutRow(
            attempt.Identity, state.Mode, attempt.IsRoot, attempt.Status, state.Commit, attempt.Evidence,
            attempt.ProgramNodes, attempt.ProgramDepth, attempt.Interceptors, attempt.Effects, attempt.Events, attempt.Children)).ToArray();
        return new ResolutionSessionReadoutLeaseReceipt(attempts, state.Trace.ToArray());
    }

    private static readonly ResolutionPhase[] PlanningPhases =
        [ResolutionPhase.Admit, ResolutionPhase.Gather, ResolutionPhase.Check, ResolutionPhase.Plan, ResolutionPhase.BeforeCommit];

    private static ResolutionTraceKind TraceKind(ResolutionAttemptStatus status) => status switch
    {
        ResolutionAttemptStatus.Rejected => ResolutionTraceKind.Rejected,
        ResolutionAttemptStatus.Suspended => ResolutionTraceKind.Suspended,
        ResolutionAttemptStatus.Faulted => ResolutionTraceKind.Faulted,
        ResolutionAttemptStatus.LimitExceeded => ResolutionTraceKind.LimitExceeded,
        ResolutionAttemptStatus.ChildFailed => ResolutionTraceKind.ChildFailed,
        _ => throw new InvalidOperationException("planned attempts have no failure trace"),
    };

    private void Finalize(ResolutionSession session, ResolutionMode expectedMode, ResolutionCommitStatus status)
    {
        SessionState state = State(session);
        if (state.Mode != expectedMode || state.Commit != ResolutionCommitStatus.Prepared)
        {
            throw new InvalidOperationException("native fixture rejected the terminal mode");
        }
        state.Commit = status;
    }

    private AttemptState RequirePhase(ResolutionSession session, ResolutionPhase phase)
    {
        AttemptState attempt = Current(session);
        if (attempt.ActivePhase != phase)
        {
            throw new InvalidOperationException($"native fixture requires {phase}");
        }
        return attempt;
    }

    private AttemptState Current(ResolutionSession session) => State(session).Attempts[State(session).Active.Peek()];
    private SessionState State(ResolutionSession session) => _sessions[session.Handle.Value];

    private sealed class SessionState(ResolutionSessionCreateRequest request)
    {
        public ResolutionMode Mode { get; } = request.Mode;
        public ResolutionCommitStatus Commit { get; set; } = ResolutionCommitStatus.NotAttempted;
        public List<AttemptState> Attempts { get; } =
            [new AttemptState(new ResolutionIdentityRow(request.RootResolution, request.Correlation, 0, false, 0), true, request.RootEvidence)];
        public Stack<int> Active { get; } = new([0]);
        public List<ResolutionTraceReadoutRow> Trace { get; } = [];
    }

    private sealed class AttemptState(ResolutionIdentityRow identity, bool isRoot, uint evidence)
    {
        public ResolutionIdentityRow Identity { get; } = identity;
        public bool IsRoot { get; } = isRoot;
        public uint Evidence { get; } = evidence;
        public ResolutionAttemptStatus Status { get; set; } = ResolutionAttemptStatus.Open;
        public ResolutionPhase? ActivePhase { get; set; }
        public int NextPhase { get; set; }
        public uint ProgramNodes { get; set; }
        public ushort ProgramDepth { get; set; }
        public uint Interceptors { get; set; }
        public uint Effects { get; set; }
        public uint Events { get; set; }
        public uint Children { get; set; }
    }
}
