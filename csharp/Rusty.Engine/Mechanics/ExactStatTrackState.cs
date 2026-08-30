using System.Numerics;

namespace Rusty.Engine.Mechanics;

/// <summary>How a dependent track current value follows a changed stat maximum.</summary>
public enum ExactStatTrackCurrentPolicy
{
    PreserveCurrent,
    PreserveDistanceFromMaximum,
}

/// <summary>One immutable view of an exact stat and its dependent track.</summary>
public sealed record ExactStatTrackSnapshot(
    ulong Revision,
    ExactStatEvaluation Stat,
    ExactValue TrackCurrent,
    ExactTrackBounds TrackBounds);

/// <summary>Read-only result of preparing a stat-source and dependent-track change.</summary>
public sealed record ExactStatTrackChangePreview(
    ExactStatTrackSnapshot Before,
    ExactStatTrackSnapshot After,
    ExactStatTrackCurrentPolicy CurrentPolicy);

/// <summary>Committed before/after evidence for one stat-source change.</summary>
public sealed record ExactStatTrackChangeReceipt(
    ExactStatTrackSnapshot Before,
    ExactStatTrackSnapshot After,
    ExactStatTrackCurrentPolicy CurrentPolicy);

/// <summary>Typed description of one dependent-track current mutation.</summary>
public abstract record ExactStatTrackCurrentMutation
{
    public sealed record Set(
        ExactValue Requested,
        ExactValue Resolved,
        ExactTrackSetPolicy Policy) : ExactStatTrackCurrentMutation;

    public sealed record Spend(
        ExactValue RequestedAmount,
        ExactValue AppliedAmount) : ExactStatTrackCurrentMutation;

    public sealed record Restore(
        ExactValue RequestedAmount,
        ExactValue AppliedAmount) : ExactStatTrackCurrentMutation;
}

/// <summary>Read-only result of preparing one dependent-track current mutation.</summary>
public sealed record ExactStatTrackCurrentMutationPreview(
    ExactStatTrackSnapshot Before,
    ExactStatTrackSnapshot After,
    ExactStatTrackCurrentMutation Mutation);

/// <summary>Committed evidence for one dependent-track current mutation.</summary>
public sealed record ExactStatTrackCurrentMutationReceipt(
    ExactStatTrackSnapshot Before,
    ExactStatTrackSnapshot After,
    ExactStatTrackCurrentMutation Mutation);

/// <summary>
/// Engine-owned exact state for one stat and one track whose maximum derives
/// from that stat. Product code supplies the base, sources, and current-value
/// policy; the Engine evaluates and publishes the complete pair atomically.
/// </summary>
public sealed class ExactStatTrackState
{
    private ExactValue _base;
    private IReadOnlyList<ExactSource> _sources;
    private ExactStatEvaluation _stat;
    private ExactValue _trackCurrent;
    private ExactTrackBounds _trackBounds;

    public ExactStatTrackState(
        ExactStatDefinition statDefinition,
        ExactValue baseValue,
        IEnumerable<ExactSource> sources,
        ExactTrackDefinition trackDefinition,
        ExactValue trackCurrent,
        ulong revision = 0)
    {
        StatDefinition = statDefinition ?? throw new ArgumentNullException(nameof(statDefinition));
        TrackDefinition = trackDefinition ?? throw new ArgumentNullException(nameof(trackDefinition));
        EnsureDependency(StatDefinition, TrackDefinition);

        _sources = CopySources(sources);
        _stat = ExactStatEvaluator.Evaluate(StatDefinition, baseValue, _sources);
        _trackBounds = TrackDefinition.ResolveBounds(_stat.Value);
        EnsureInBounds(trackCurrent, _trackBounds);
        _base = baseValue;
        _trackCurrent = trackCurrent;
        Revision = revision;
    }

    public ExactStatDefinition StatDefinition { get; }

    public ExactTrackDefinition TrackDefinition { get; }

    public ExactValue Base => _base;

    public IReadOnlyList<ExactSource> Sources => _sources;

    public ulong Revision { get; private set; }

    public ExactStatTrackSnapshot Read() => Snapshot(
        Revision,
        _stat,
        _trackCurrent,
        _trackBounds);

    public ExactStatTrackChangeCandidate PrepareSourceChange(
        ExactValue prospectiveBase,
        IEnumerable<ExactSource> prospectiveSources,
        ExactStatTrackCurrentPolicy currentPolicy,
        ulong? expectedRevision = null)
    {
        EnsureRevision(expectedRevision);
        ulong revisionAfter = NextRevision();
        IReadOnlyList<ExactSource> copiedSources = CopySources(prospectiveSources);
        ExactStatEvaluation prospectiveStat = ExactStatEvaluator.Evaluate(
            StatDefinition,
            prospectiveBase,
            copiedSources);
        ExactTrackBounds prospectiveBounds = TrackDefinition.ResolveBounds(prospectiveStat.Value);
        ExactValue prospectiveCurrent = ResolveCurrent(prospectiveBounds, currentPolicy);
        ExactStatTrackSnapshot before = Read();
        ExactStatTrackSnapshot after = Snapshot(
            revisionAfter,
            prospectiveStat,
            prospectiveCurrent,
            prospectiveBounds);

        return new ExactStatTrackChangeCandidate(
            this,
            Revision,
            prospectiveBase,
            copiedSources,
            prospectiveStat,
            prospectiveCurrent,
            prospectiveBounds,
            new ExactStatTrackChangePreview(before, after, currentPolicy));
    }

    public ExactStatTrackChangeReceipt ApplySourceChange(
        ExactValue prospectiveBase,
        IEnumerable<ExactSource> prospectiveSources,
        ExactStatTrackCurrentPolicy currentPolicy,
        ulong? expectedRevision = null) =>
        PrepareSourceChange(
            prospectiveBase,
            prospectiveSources,
            currentPolicy,
            expectedRevision).Publish();

    public ExactStatTrackCurrentMutationCandidate PrepareSetCurrent(
        ExactValue requested,
        ExactTrackSetPolicy policy,
        ulong? expectedRevision = null)
    {
        EnsureRevision(expectedRevision);
        ExactValue resolved = policy switch
        {
            ExactTrackSetPolicy.RejectOutOfBounds => RequireInBounds(requested),
            ExactTrackSetPolicy.ClampToBounds => requested.Clamp(
                _trackBounds.Minimum,
                _trackBounds.Maximum),
            _ => throw new MechanicsException("Unknown exact stat-track set policy."),
        };
        return PrepareCurrentMutation(
            resolved,
            new ExactStatTrackCurrentMutation.Set(requested, resolved, policy),
            expectedRevision);
    }

    public ExactStatTrackCurrentMutationReceipt SetCurrent(
        ExactValue requested,
        ExactTrackSetPolicy policy,
        ulong? expectedRevision = null) =>
        PrepareSetCurrent(requested, policy, expectedRevision).Publish();

    public ExactStatTrackCurrentMutationCandidate PrepareSpend(
        ExactValue requestedAmount,
        ulong? expectedRevision = null)
    {
        EnsureRevision(expectedRevision);
        ExactValue amount = requestedAmount.RequireNonNegative();
        long availableRaw = _trackCurrent.Raw - _trackBounds.Minimum.Raw;
        if (amount.Raw > availableRaw)
        {
            throw new MechanicsException("The dependent exact track does not have enough value to spend.");
        }
        ExactValue after = _trackCurrent.CheckedSubtract(amount);
        return PrepareCurrentMutation(
            after,
            new ExactStatTrackCurrentMutation.Spend(amount, amount),
            expectedRevision);
    }

    public ExactStatTrackCurrentMutationReceipt Spend(
        ExactValue amount,
        ulong? expectedRevision = null) =>
        PrepareSpend(amount, expectedRevision).Publish();

    public ExactStatTrackCurrentMutationCandidate PrepareRestore(
        ExactValue requestedAmount,
        ulong? expectedRevision = null)
    {
        EnsureRevision(expectedRevision);
        ExactValue amount = requestedAmount.RequireNonNegative();
        long availableRaw = _trackBounds.Maximum.Raw - _trackCurrent.Raw;
        ExactValue applied = amount.Raw > availableRaw
            ? new ExactValue(availableRaw)
            : amount;
        ExactValue after = _trackCurrent.CheckedAdd(applied);
        return PrepareCurrentMutation(
            after,
            new ExactStatTrackCurrentMutation.Restore(amount, applied),
            expectedRevision);
    }

    public ExactStatTrackCurrentMutationReceipt Restore(
        ExactValue amount,
        ulong? expectedRevision = null) =>
        PrepareRestore(amount, expectedRevision).Publish();

    internal ExactStatTrackChangeReceipt Publish(
        ulong expectedRevision,
        ExactValue prospectiveBase,
        IReadOnlyList<ExactSource> prospectiveSources,
        ExactStatEvaluation prospectiveStat,
        ExactValue prospectiveCurrent,
        ExactTrackBounds prospectiveBounds,
        ExactStatTrackChangePreview preview)
    {
        EnsureRevision(expectedRevision);
        EnsureInBounds(prospectiveCurrent, prospectiveBounds);
        _base = prospectiveBase;
        _sources = prospectiveSources;
        _stat = prospectiveStat;
        _trackCurrent = prospectiveCurrent;
        _trackBounds = prospectiveBounds;
        Revision = preview.After.Revision;
        return new ExactStatTrackChangeReceipt(preview.Before, Read(), preview.CurrentPolicy);
    }

    internal ExactStatTrackCurrentMutationReceipt PublishCurrentMutation(
        ulong expectedRevision,
        ExactValue prospectiveCurrent,
        ExactStatTrackCurrentMutationPreview preview)
    {
        EnsureRevision(expectedRevision);
        EnsureInBounds(prospectiveCurrent, _trackBounds);
        _trackCurrent = prospectiveCurrent;
        Revision = preview.After.Revision;
        return new ExactStatTrackCurrentMutationReceipt(
            preview.Before,
            Read(),
            preview.Mutation);
    }

    private ExactStatTrackCurrentMutationCandidate PrepareCurrentMutation(
        ExactValue prospectiveCurrent,
        ExactStatTrackCurrentMutation mutation,
        ulong? expectedRevision)
    {
        EnsureRevision(expectedRevision);
        EnsureInBounds(prospectiveCurrent, _trackBounds);
        ulong revisionAfter = NextRevision();
        ExactStatTrackSnapshot before = Read();
        ExactStatTrackSnapshot after = Snapshot(
            revisionAfter,
            _stat,
            prospectiveCurrent,
            _trackBounds);
        return new ExactStatTrackCurrentMutationCandidate(
            this,
            Revision,
            prospectiveCurrent,
            new ExactStatTrackCurrentMutationPreview(before, after, mutation));
    }

    private ExactValue ResolveCurrent(
        ExactTrackBounds prospectiveBounds,
        ExactStatTrackCurrentPolicy currentPolicy)
    {
        ExactValue current = currentPolicy switch
        {
            ExactStatTrackCurrentPolicy.PreserveCurrent => _trackCurrent,
            ExactStatTrackCurrentPolicy.PreserveDistanceFromMaximum =>
                ResolveDistanceFromMaximum(prospectiveBounds.Maximum),
            _ => throw new MechanicsException("Unknown exact stat-track current policy."),
        };
        EnsureInBounds(current, prospectiveBounds);
        return current;
    }

    private ExactValue ResolveDistanceFromMaximum(ExactValue prospectiveMaximum)
    {
        BigInteger reconciledRaw = (BigInteger)prospectiveMaximum.Raw
            - _trackBounds.Maximum.Raw
            + _trackCurrent.Raw;
        if (reconciledRaw < -ExactValue.MaximumAbsolute
            || reconciledRaw > ExactValue.MaximumAbsolute)
        {
            throw new MechanicsArithmeticException(
                "The distance-preserving exact track current is outside the exact value domain.");
        }

        return new ExactValue((long)reconciledRaw);
    }

    private void EnsureRevision(ulong? expectedRevision)
    {
        if (expectedRevision is ulong expected && expected != Revision)
        {
            throw new MechanicsException(
                $"Exact stat-track revision is stale: expected {expected}, actual {Revision}.");
        }
    }

    private ulong NextRevision()
    {
        try
        {
            return checked(Revision + 1);
        }
        catch (OverflowException exception)
        {
            throw new MechanicsArithmeticException(
                "The exact stat-track revision is exhausted.",
                exception);
        }
    }

    private static IReadOnlyList<ExactSource> CopySources(IEnumerable<ExactSource> sources)
    {
        ArgumentNullException.ThrowIfNull(sources);
        ExactSource[] copied = sources.ToArray();
        if (copied.Any(source => source is null))
        {
            throw new ArgumentException("Exact stat sources cannot contain null.", nameof(sources));
        }
        return Array.AsReadOnly(copied);
    }

    private static void EnsureDependency(
        ExactStatDefinition statDefinition,
        ExactTrackDefinition trackDefinition)
    {
        if (trackDefinition.Maximum is not ExactTrackMaximum.FromStat fromStat
            || fromStat.Stat != statDefinition.Id)
        {
            throw new MechanicsException(
                $"Track {trackDefinition.Id} must derive its maximum from stat {statDefinition.Id}.");
        }
    }

    private static void EnsureInBounds(ExactValue current, ExactTrackBounds bounds)
    {
        if (current < bounds.Minimum || current > bounds.Maximum)
        {
            throw new MechanicsException("The dependent exact track current value is outside its resolved bounds.");
        }
    }

    private ExactValue RequireInBounds(ExactValue current)
    {
        EnsureInBounds(current, _trackBounds);
        return current;
    }

    private static ExactStatTrackSnapshot Snapshot(
        ulong revision,
        ExactStatEvaluation stat,
        ExactValue trackCurrent,
        ExactTrackBounds trackBounds) =>
        new(revision, stat, trackCurrent, trackBounds);
}

/// <summary>Detached current mutation that publishes only at its captured pair revision.</summary>
public sealed class ExactStatTrackCurrentMutationCandidate
{
    private readonly ExactStatTrackState _owner;
    private readonly ulong _expectedRevision;
    private readonly ExactValue _prospectiveCurrent;
    private bool _published;

    internal ExactStatTrackCurrentMutationCandidate(
        ExactStatTrackState owner,
        ulong expectedRevision,
        ExactValue prospectiveCurrent,
        ExactStatTrackCurrentMutationPreview preview)
    {
        _owner = owner;
        _expectedRevision = expectedRevision;
        _prospectiveCurrent = prospectiveCurrent;
        Preview = preview;
    }

    public ExactStatTrackCurrentMutationPreview Preview { get; }

    public ExactStatTrackCurrentMutationReceipt Publish()
    {
        if (_published)
        {
            throw new InvalidOperationException(
                "The exact stat-track current mutation candidate was already published.");
        }
        ExactStatTrackCurrentMutationReceipt receipt = _owner.PublishCurrentMutation(
            _expectedRevision,
            _prospectiveCurrent,
            Preview);
        _published = true;
        return receipt;
    }
}

/// <summary>Detached stat/track candidate that publishes only at its captured owner revision.</summary>
public sealed class ExactStatTrackChangeCandidate
{
    private readonly ExactStatTrackState _owner;
    private readonly ulong _expectedRevision;
    private readonly ExactValue _prospectiveBase;
    private readonly IReadOnlyList<ExactSource> _prospectiveSources;
    private readonly ExactStatEvaluation _prospectiveStat;
    private readonly ExactValue _prospectiveCurrent;
    private readonly ExactTrackBounds _prospectiveBounds;
    private bool _published;

    internal ExactStatTrackChangeCandidate(
        ExactStatTrackState owner,
        ulong expectedRevision,
        ExactValue prospectiveBase,
        IReadOnlyList<ExactSource> prospectiveSources,
        ExactStatEvaluation prospectiveStat,
        ExactValue prospectiveCurrent,
        ExactTrackBounds prospectiveBounds,
        ExactStatTrackChangePreview preview)
    {
        _owner = owner;
        _expectedRevision = expectedRevision;
        _prospectiveBase = prospectiveBase;
        _prospectiveSources = prospectiveSources;
        _prospectiveStat = prospectiveStat;
        _prospectiveCurrent = prospectiveCurrent;
        _prospectiveBounds = prospectiveBounds;
        Preview = preview;
    }

    public ExactStatTrackChangePreview Preview { get; }

    public ExactStatTrackChangeReceipt Publish()
    {
        if (_published)
        {
            throw new InvalidOperationException("The exact stat-track candidate was already published.");
        }
        ExactStatTrackChangeReceipt receipt = _owner.Publish(
            _expectedRevision,
            _prospectiveBase,
            _prospectiveSources,
            _prospectiveStat,
            _prospectiveCurrent,
            _prospectiveBounds,
            Preview);
        _published = true;
        return receipt;
    }
}
