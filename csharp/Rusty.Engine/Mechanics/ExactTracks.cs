namespace Rusty.Engine.Mechanics;

/// <summary>How an exact track set handles a value outside its bounds.</summary>
public enum ExactTrackSetPolicy
{
    RejectOutOfBounds,
    ClampToBounds,
}

/// <summary>How an exact track handles a prospective maximum reduction.</summary>
public enum ExactTrackReconciliationPolicy
{
    PreserveCurrent,
    ClampToMaximum,
}

/// <summary>Maximum source for one exact track.</summary>
public abstract record ExactTrackMaximum
{
    public sealed record Fixed(ExactValue Value) : ExactTrackMaximum;

    public sealed record FromStat(StatId Stat) : ExactTrackMaximum;
}

/// <summary>Identity and fixed minimum for one exact track.</summary>
public sealed class ExactTrackDefinition
{
    public ExactTrackDefinition(TrackId id, ExactValue minimum, ExactTrackMaximum maximum)
    {
        Id = id ?? throw new ArgumentNullException(nameof(id));
        Maximum = maximum ?? throw new ArgumentNullException(nameof(maximum));
        if (maximum is ExactTrackMaximum.Fixed fixedMaximum && minimum > fixedMaximum.Value)
        {
            throw new ArgumentException("Exact track bounds are inverted.", nameof(maximum));
        }

        Minimum = minimum;
    }

    public TrackId Id { get; }

    public ExactValue Minimum { get; }

    public ExactTrackMaximum Maximum { get; }

    public ExactTrackBounds ResolveBounds(ExactValue resolvedMaximum) =>
        new(Minimum, resolvedMaximum);

    public ExactTrackBounds FixedBounds => Maximum is ExactTrackMaximum.Fixed fixedMaximum
        ? new(Minimum, fixedMaximum.Value)
        : throw new InvalidOperationException(
            $"Track {Id} derives its maximum from stat {((ExactTrackMaximum.FromStat)Maximum).Stat}.");
}

/// <summary>Resolved minimum and maximum for one exact track operation.</summary>
public readonly record struct ExactTrackBounds
{
    public ExactTrackBounds(ExactValue minimum, ExactValue maximum)
    {
        if (minimum > maximum)
        {
            throw new ArgumentException("Exact track bounds are inverted.", nameof(maximum));
        }

        Minimum = minimum;
        Maximum = maximum;
    }

    public ExactValue Minimum { get; }

    public ExactValue Maximum { get; }
}

/// <summary>Result of one exact track set operation.</summary>
public readonly record struct ExactTrackSetReceipt(
    ExactValue Requested,
    ExactValue Before,
    ExactValue After,
    ExactTrackBounds Bounds,
    ExactTrackSetPolicy Policy);

/// <summary>Result of one exact spend or restore operation.</summary>
public readonly record struct ExactTrackMutationReceipt(
    ExactValue RequestedAmount,
    ExactValue AppliedAmount,
    ExactValue Before,
    ExactValue After,
    ExactTrackBounds Bounds,
    bool IsSpend);

/// <summary>Result of one exact no-stranding reconciliation.</summary>
public readonly record struct ExactTrackReconciliationReceipt(
    ExactTrackBounds PreviousBounds,
    ExactTrackBounds ProspectiveBounds,
    ExactValue Before,
    ExactValue After,
    ExactTrackReconciliationPolicy Policy);

/// <summary>
/// Mutable product-owned exact track state. All methods validate the complete
/// candidate before changing the current value or resolved bounds, so a
/// rejected mutation leaves the track untouched.
/// </summary>
public sealed class ExactTrack
{
    public ExactTrack(ExactTrackDefinition definition, ExactValue current)
        : this(definition, current, FixedBoundsFor(definition)) { }

    public ExactTrack(ExactTrackDefinition definition, ExactValue current, ExactTrackBounds bounds)
    {
        Definition = definition ?? throw new ArgumentNullException(nameof(definition));
        EnsureBoundsMatchDefinition(bounds);
        EnsureInBounds(current, bounds, "Initial exact track value");
        Current = current;
        Bounds = bounds;
    }

    public ExactTrackDefinition Definition { get; }

    public ExactValue Current { get; private set; }

    public ExactTrackBounds Bounds { get; private set; }

    public ExactTrackSetReceipt Set(
        ExactValue requested,
        ExactTrackSetPolicy policy,
        ExactTrackBounds? resolvedBounds = null)
    {
        ExactTrackBounds bounds = ResolveOperationBounds(resolvedBounds);
        EnsureInBounds(Current, bounds, "Current exact track value");
        ExactValue after = policy switch
        {
            ExactTrackSetPolicy.RejectOutOfBounds => SetRejected(requested, bounds),
            ExactTrackSetPolicy.ClampToBounds => requested.Clamp(bounds.Minimum, bounds.Maximum),
            _ => throw new MechanicsException("Unknown exact track set policy."),
        };

        ExactValue before = Current;
        Current = after;
        Bounds = bounds;
        return new ExactTrackSetReceipt(requested, before, after, bounds, policy);
    }

    public ExactTrackMutationReceipt Spend(
        ExactValue amount,
        ExactTrackBounds? resolvedBounds = null)
    {
        return Adjust(amount, isSpend: true, resolvedBounds);
    }

    public ExactTrackMutationReceipt Restore(
        ExactValue amount,
        ExactTrackBounds? resolvedBounds = null)
    {
        return Adjust(amount, isSpend: false, resolvedBounds);
    }

    /// <summary>
    /// Applies a caller-staged bound before the source or stat change that will
    /// cause it. Preserve fails if the current value would be stranded; clamp
    /// lowers the value atomically with the new bound.
    /// </summary>
    public ExactTrackReconciliationReceipt Reconcile(
        ExactTrackBounds prospectiveBounds,
        ExactTrackReconciliationPolicy policy)
    {
        EnsureBoundsMatchDefinition(prospectiveBounds);
        EnsureInBounds(Current, Bounds, "Current exact track value");
        if (prospectiveBounds.Maximum > Bounds.Maximum)
        {
            throw new MechanicsException("Exact track reconciliation cannot expand its current maximum.");
        }

        ExactValue before = Current;
        ExactValue after = policy switch
        {
            ExactTrackReconciliationPolicy.PreserveCurrent when Current > prospectiveBounds.Maximum =>
                throw new MechanicsException("The prospective exact track maximum would strand its current value."),
            ExactTrackReconciliationPolicy.PreserveCurrent => Current,
            ExactTrackReconciliationPolicy.ClampToMaximum => Current.Clamp(
                prospectiveBounds.Minimum,
                prospectiveBounds.Maximum),
            _ => throw new MechanicsException("Unknown exact track reconciliation policy."),
        };

        ExactTrackBounds previousBounds = Bounds;
        Current = after;
        Bounds = prospectiveBounds;
        return new ExactTrackReconciliationReceipt(
            previousBounds,
            prospectiveBounds,
            before,
            after,
            policy);
    }

    private ExactTrackMutationReceipt Adjust(
        ExactValue requestedAmount,
        bool isSpend,
        ExactTrackBounds? resolvedBounds)
    {
        ExactValue amount = requestedAmount.RequireNonNegative();
        ExactTrackBounds bounds = ResolveOperationBounds(resolvedBounds);
        EnsureInBounds(Current, bounds, "Current exact track value");

        ExactValue before = Current;
        ExactValue after;
        ExactValue applied;
        if (isSpend)
        {
            long availableRaw = Current.Raw - bounds.Minimum.Raw;
            if (amount.Raw > availableRaw)
            {
                throw new MechanicsException("The exact track does not have enough value to spend.");
            }

            after = Current.CheckedSubtract(amount);
            applied = amount;
        }
        else
        {
            long availableRaw = bounds.Maximum.Raw - Current.Raw;
            applied = amount.Raw > availableRaw ? new ExactValue(availableRaw) : amount;
            after = Current.CheckedAdd(applied);
        }

        Current = after;
        Bounds = bounds;
        return new ExactTrackMutationReceipt(amount, applied, before, after, bounds, isSpend);
    }

    private ExactTrackBounds ResolveOperationBounds(ExactTrackBounds? resolvedBounds)
    {
        ExactTrackBounds bounds = resolvedBounds ?? Bounds;
        EnsureBoundsMatchDefinition(bounds);
        return bounds;
    }

    private static ExactTrackBounds FixedBoundsFor(ExactTrackDefinition definition)
    {
        ArgumentNullException.ThrowIfNull(definition);
        return definition.FixedBounds;
    }

    private void EnsureBoundsMatchDefinition(ExactTrackBounds bounds)
    {
        if (bounds.Minimum != Definition.Minimum)
        {
            throw new MechanicsException(
                $"Track {Definition.Id} cannot change its declared minimum during reconciliation.");
        }
    }

    private static ExactValue SetRejected(ExactValue requested, ExactTrackBounds bounds)
    {
        EnsureInBounds(requested, bounds, "Requested exact track value");
        return requested;
    }

    private static void EnsureInBounds(ExactValue value, ExactTrackBounds bounds, string name)
    {
        if (value < bounds.Minimum || value > bounds.Maximum)
        {
            throw new MechanicsException($"{name} is outside the exact track bounds.");
        }
    }
}
