namespace Rusty.Engine.Mechanics;

/// <summary>How a continuous track set handles a value outside its bounds.</summary>
public enum ContinuousTrackSetPolicy
{
    RejectOutOfBounds,
    ClampToBounds,
}

/// <summary>How a continuous track handles a prospective maximum reduction.</summary>
public enum ContinuousTrackReconciliationPolicy
{
    PreserveCurrent,
    ClampToMaximum,
}

/// <summary>Maximum source for one continuous track.</summary>
public abstract record ContinuousTrackMaximum
{
    public sealed record Fixed(ContinuousValue Value) : ContinuousTrackMaximum;

    public sealed record FromStat(StatId Stat) : ContinuousTrackMaximum;
}

/// <summary>Identity and fixed minimum for one continuous track.</summary>
public sealed class ContinuousTrackDefinition
{
    public ContinuousTrackDefinition(
        TrackId id,
        ContinuousValue minimum,
        ContinuousTrackMaximum maximum)
    {
        Id = id ?? throw new ArgumentNullException(nameof(id));
        Maximum = maximum ?? throw new ArgumentNullException(nameof(maximum));
        if (maximum is ContinuousTrackMaximum.Fixed fixedMaximum && minimum > fixedMaximum.Value)
        {
            throw new ArgumentException("Continuous track bounds are inverted.", nameof(maximum));
        }

        Minimum = minimum;
    }

    public TrackId Id { get; }

    public ContinuousValue Minimum { get; }

    public ContinuousTrackMaximum Maximum { get; }

    public ContinuousTrackBounds ResolveBounds(ContinuousValue resolvedMaximum) =>
        new(Minimum, resolvedMaximum);

    public ContinuousTrackBounds FixedBounds => Maximum is ContinuousTrackMaximum.Fixed fixedMaximum
        ? new(Minimum, fixedMaximum.Value)
        : throw new InvalidOperationException(
            $"Track {Id} derives its maximum from stat {((ContinuousTrackMaximum.FromStat)Maximum).Stat}.");
}

/// <summary>Resolved minimum and maximum for one continuous track operation.</summary>
public readonly record struct ContinuousTrackBounds
{
    public ContinuousTrackBounds(ContinuousValue minimum, ContinuousValue maximum)
    {
        if (minimum > maximum)
        {
            throw new ArgumentException("Continuous track bounds are inverted.", nameof(maximum));
        }

        Minimum = minimum;
        Maximum = maximum;
    }

    public ContinuousValue Minimum { get; }

    public ContinuousValue Maximum { get; }
}

/// <summary>Result of one continuous track set operation.</summary>
public readonly record struct ContinuousTrackSetReceipt(
    ContinuousValue Requested,
    ContinuousValue Before,
    ContinuousValue After,
    ContinuousTrackBounds Bounds,
    ContinuousTrackSetPolicy Policy);

/// <summary>Result of one continuous spend or restore operation.</summary>
public readonly record struct ContinuousTrackMutationReceipt(
    ContinuousValue RequestedAmount,
    ContinuousValue AppliedAmount,
    ContinuousValue Before,
    ContinuousValue After,
    ContinuousTrackBounds Bounds,
    bool IsSpend);

/// <summary>Result of one continuous no-stranding reconciliation.</summary>
public readonly record struct ContinuousTrackReconciliationReceipt(
    ContinuousTrackBounds PreviousBounds,
    ContinuousTrackBounds ProspectiveBounds,
    ContinuousValue Before,
    ContinuousValue After,
    ContinuousTrackReconciliationPolicy Policy);

/// <summary>
/// Mutable product-owned continuous track state. Candidate bounds and values
/// are validated before either is published.
/// </summary>
public sealed class ContinuousTrack
{
    public ContinuousTrack(ContinuousTrackDefinition definition, ContinuousValue current)
        : this(definition, current, FixedBoundsFor(definition)) { }

    public ContinuousTrack(
        ContinuousTrackDefinition definition,
        ContinuousValue current,
        ContinuousTrackBounds bounds)
    {
        Definition = definition ?? throw new ArgumentNullException(nameof(definition));
        EnsureBoundsMatchDefinition(bounds);
        EnsureInBounds(current, bounds, "Initial continuous track value");
        Current = current;
        Bounds = bounds;
    }

    public ContinuousTrackDefinition Definition { get; }

    public ContinuousValue Current { get; private set; }

    public ContinuousTrackBounds Bounds { get; private set; }

    public ContinuousTrackSetReceipt Set(
        ContinuousValue requested,
        ContinuousTrackSetPolicy policy,
        ContinuousTrackBounds? resolvedBounds = null)
    {
        ContinuousTrackBounds bounds = ResolveOperationBounds(resolvedBounds);
        EnsureInBounds(Current, bounds, "Current continuous track value");
        ContinuousValue after = policy switch
        {
            ContinuousTrackSetPolicy.RejectOutOfBounds => SetRejected(requested, bounds),
            ContinuousTrackSetPolicy.ClampToBounds => requested.Clamp(bounds.Minimum, bounds.Maximum),
            _ => throw new MechanicsException("Unknown continuous track set policy."),
        };

        ContinuousValue before = Current;
        Current = after;
        Bounds = bounds;
        return new ContinuousTrackSetReceipt(requested, before, after, bounds, policy);
    }

    public ContinuousTrackMutationReceipt Spend(
        ContinuousValue amount,
        ContinuousTrackBounds? resolvedBounds = null)
    {
        return Adjust(amount, isSpend: true, resolvedBounds);
    }

    public ContinuousTrackMutationReceipt Restore(
        ContinuousValue amount,
        ContinuousTrackBounds? resolvedBounds = null)
    {
        return Adjust(amount, isSpend: false, resolvedBounds);
    }

    /// <summary>
    /// Applies a caller-staged bound before the source or stat change that will
    /// cause it. Preserve fails if the current value would be stranded; clamp
    /// lowers the value atomically with the new bound.
    /// </summary>
    public ContinuousTrackReconciliationReceipt Reconcile(
        ContinuousTrackBounds prospectiveBounds,
        ContinuousTrackReconciliationPolicy policy)
    {
        EnsureBoundsMatchDefinition(prospectiveBounds);
        EnsureInBounds(Current, Bounds, "Current continuous track value");
        if (prospectiveBounds.Maximum > Bounds.Maximum)
        {
            throw new MechanicsException("Continuous track reconciliation cannot expand its current maximum.");
        }

        ContinuousValue before = Current;
        ContinuousValue after = policy switch
        {
            ContinuousTrackReconciliationPolicy.PreserveCurrent when Current > prospectiveBounds.Maximum =>
                throw new MechanicsException("The prospective continuous track maximum would strand its current value."),
            ContinuousTrackReconciliationPolicy.PreserveCurrent => Current,
            ContinuousTrackReconciliationPolicy.ClampToMaximum => Current.Clamp(
                prospectiveBounds.Minimum,
                prospectiveBounds.Maximum),
            _ => throw new MechanicsException("Unknown continuous track reconciliation policy."),
        };

        ContinuousTrackBounds previousBounds = Bounds;
        Current = after;
        Bounds = prospectiveBounds;
        return new ContinuousTrackReconciliationReceipt(
            previousBounds,
            prospectiveBounds,
            before,
            after,
            policy);
    }

    private ContinuousTrackMutationReceipt Adjust(
        ContinuousValue requestedAmount,
        bool isSpend,
        ContinuousTrackBounds? resolvedBounds)
    {
        ContinuousValue amount = requestedAmount.RequireNonNegative();
        ContinuousTrackBounds bounds = ResolveOperationBounds(resolvedBounds);
        EnsureInBounds(Current, bounds, "Current continuous track value");

        ContinuousValue before = Current;
        ContinuousValue after;
        ContinuousValue applied;
        if (isSpend)
        {
            double availableValue = Current.Value - bounds.Minimum.Value;
            if (amount.Value > availableValue)
            {
                throw new MechanicsException("The continuous track does not have enough value to spend.");
            }

            after = Current.CheckedSubtract(amount);
            applied = amount;
        }
        else
        {
            double availableValue = bounds.Maximum.Value - Current.Value;
            applied = amount.Value > availableValue
                ? new ContinuousValue(availableValue)
                : amount;
            after = Current.CheckedAdd(applied);
        }

        Current = after;
        Bounds = bounds;
        return new ContinuousTrackMutationReceipt(amount, applied, before, after, bounds, isSpend);
    }

    private ContinuousTrackBounds ResolveOperationBounds(ContinuousTrackBounds? resolvedBounds)
    {
        ContinuousTrackBounds bounds = resolvedBounds ?? Bounds;
        EnsureBoundsMatchDefinition(bounds);
        return bounds;
    }

    private static ContinuousTrackBounds FixedBoundsFor(ContinuousTrackDefinition definition)
    {
        ArgumentNullException.ThrowIfNull(definition);
        return definition.FixedBounds;
    }

    private void EnsureBoundsMatchDefinition(ContinuousTrackBounds bounds)
    {
        if (bounds.Minimum != Definition.Minimum)
        {
            throw new MechanicsException(
                $"Track {Definition.Id} cannot change its declared minimum during reconciliation.");
        }
    }

    private static ContinuousValue SetRejected(
        ContinuousValue requested,
        ContinuousTrackBounds bounds)
    {
        EnsureInBounds(requested, bounds, "Requested continuous track value");
        return requested;
    }

    private static void EnsureInBounds(
        ContinuousValue value,
        ContinuousTrackBounds bounds,
        string name)
    {
        if (value < bounds.Minimum || value > bounds.Maximum)
        {
            throw new MechanicsException($"{name} is outside the continuous track bounds.");
        }
    }
}
