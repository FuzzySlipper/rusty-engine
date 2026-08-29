using System.Numerics;

namespace Rusty.Engine.Mechanics;

/// <summary>Bounds and identity for one exact stat.</summary>
public sealed class ExactStatDefinition
{
    public ExactStatDefinition(StatId id, ExactValue minimum, ExactValue maximum)
    {
        Id = id ?? throw new ArgumentNullException(nameof(id));
        if (minimum > maximum)
        {
            throw new ArgumentException("Exact stat bounds are inverted.", nameof(maximum));
        }

        Minimum = minimum;
        Maximum = maximum;
    }

    public StatId Id { get; }

    public ExactValue Minimum { get; }

    public ExactValue Maximum { get; }
}

/// <summary>Exact contribution operation applied to one stat.</summary>
public abstract record ExactStatContribution
{
    public abstract ExactStatContributionKind Kind { get; }

    /// <summary>Returns the magnitude used by highest/lowest stacking.</summary>
    public int CompareMagnitude(ExactStatContribution other)
    {
        ArgumentNullException.ThrowIfNull(other);
        if (Kind != other.Kind)
        {
            throw new MechanicsException(
                "Contributions in one stacking group must use one contribution kind.");
        }

        return (this, other) switch
        {
            (Add left, Add right) => left.Amount.CompareTo(right.Amount),
            (Scale left, Scale right) => left.Ratio.CompareTo(right.Ratio),
            (Minimum left, Minimum right) => left.Value.CompareTo(right.Value),
            (Maximum left, Maximum right) => left.Value.CompareTo(right.Value),
            _ => throw new MechanicsException("Unknown exact contribution kind."),
        };
    }

    public sealed record Add(ExactValue Amount) : ExactStatContribution
    {
        public override ExactStatContributionKind Kind => ExactStatContributionKind.Add;
    }

    public sealed record Scale(ExactRatio Ratio) : ExactStatContribution
    {
        public override ExactStatContributionKind Kind => ExactStatContributionKind.Scale;
    }

    public sealed record Minimum(ExactValue Value) : ExactStatContribution
    {
        public override ExactStatContributionKind Kind => ExactStatContributionKind.Minimum;
    }

    public sealed record Maximum(ExactValue Value) : ExactStatContribution
    {
        public override ExactStatContributionKind Kind => ExactStatContributionKind.Maximum;
    }
}

/// <summary>Kind of an exact stat contribution.</summary>
public enum ExactStatContributionKind
{
    Add,
    Scale,
    Minimum,
    Maximum,
}

/// <summary>One typed contribution from one source to one stat.</summary>
public sealed record ExactStatContributionDefinition
{
    public ExactStatContributionDefinition(
        StatId stat,
        StackingGroupId group,
        MechanicsStackingPolicy stacking,
        ExactStatContribution contribution)
    {
        Stat = stat ?? throw new ArgumentNullException(nameof(stat));
        Group = group ?? throw new ArgumentNullException(nameof(group));
        Contribution = contribution ?? throw new ArgumentNullException(nameof(contribution));
        Stacking = stacking;
    }

    public StatId Stat { get; }

    public StackingGroupId Group { get; }

    public MechanicsStackingPolicy Stacking { get; }

    public ExactStatContribution Contribution { get; }
}

/// <summary>One source decision retained in an exact stat evaluation.</summary>
public readonly record struct ExactStatDecision(
    MechanicsSourceIdentity Source,
    SourceDefinitionId SourceDefinition,
    int? ContributionIndex,
    MechanicsDecisionOutcome Outcome,
    StackingGroupId? StackingGroup,
    MechanicsStackingPolicy? Stacking,
    ExactStatContribution? Contribution);

/// <summary>Copied result of one exact stat evaluation.</summary>
public sealed class ExactStatEvaluation
{
    internal ExactStatEvaluation(
        ExactStatDefinition definition,
        ExactValue baseValue,
        ExactValue afterAdditions,
        ExactRatioProduct scale,
        ExactValue afterScaling,
        ExactValue minimum,
        ExactValue maximum,
        IReadOnlyList<ExactStatDecision> decisions)
    {
        Definition = definition;
        Base = baseValue;
        AfterAdditions = afterAdditions;
        Scale = scale;
        AfterScaling = afterScaling;
        Unconstrained = afterScaling;
        Minimum = minimum;
        Maximum = maximum;
        Value = afterScaling.Clamp(minimum, maximum);
        Decisions = decisions;
    }

    public ExactStatDefinition Definition { get; }

    public ExactValue Base { get; }

    public ExactValue AfterAdditions { get; }

    public ExactRatioProduct Scale { get; }

    public ExactValue AfterScaling { get; }

    public ExactValue Unconstrained { get; }

    public ExactValue Minimum { get; }

    public ExactValue Maximum { get; }

    public ExactValue Value { get; }

    public IReadOnlyList<ExactStatDecision> Decisions { get; }
}

/// <summary>
/// Evaluates direct caller-supplied exact sources. It is intentionally a
/// calculation helper, not a catalog, registry, scheduler, or state owner.
/// </summary>
public static class ExactStatEvaluator
{
    public static ExactStatEvaluation Evaluate(
        ExactStatDefinition definition,
        ExactValue baseValue,
        IEnumerable<ExactSource> sources)
    {
        ArgumentNullException.ThrowIfNull(definition);
        ArgumentNullException.ThrowIfNull(sources);
        EnsureWithinBounds(baseValue, definition.Minimum, definition.Maximum, "base stat");

        ExactSource[] ordered = MechanicsSourceOrdering.Order(
            sources,
            source => source.Identity,
            source => source.Definition,
            source => source.Priority).ToArray();

        var decisions = new List<ExactStatDecision>();
        var candidates = new List<Candidate>();
        foreach (ExactSource source in ordered)
        {
            bool matched = false;
            for (int index = 0; index < source.Contributions.Count; index++)
            {
                ExactStatContributionDefinition contribution = source.Contributions[index];
                if (contribution.Stat != definition.Id)
                {
                    continue;
                }

                matched = true;
                int decisionIndex = decisions.Count;
                decisions.Add(new ExactStatDecision(
                    source.Identity,
                    source.Definition,
                    index,
                    MechanicsDecisionOutcome.Suppressed,
                    contribution.Group,
                    contribution.Stacking,
                    contribution.Contribution));
                candidates.Add(new Candidate(
                    decisionIndex,
                    source.Definition,
                    contribution.Group,
                    contribution.Stacking,
                    contribution.Contribution));
            }

            if (!matched)
            {
                decisions.Add(new ExactStatDecision(
                    source.Identity,
                    source.Definition,
                    null,
                    MechanicsDecisionOutcome.Inapplicable,
                    null,
                    null,
                    null));
            }
        }

        SelectCandidates(candidates, decisions);

        BigInteger additive = BigInteger.Zero;
        ExactRatioProduct scale = ExactRatioProduct.One;
        ExactValue minimum = definition.Minimum;
        ExactValue maximum = definition.Maximum;
        foreach (Candidate candidate in candidates)
        {
            if (decisions[candidate.DecisionIndex].Outcome != MechanicsDecisionOutcome.Applied)
            {
                continue;
            }

            switch (candidate.Contribution)
            {
                case ExactStatContribution.Add add:
                    additive += add.Amount.Raw;
                    break;
                case ExactStatContribution.Scale scaleContribution:
                    scale = scale.Include(scaleContribution.Ratio);
                    break;
                case ExactStatContribution.Minimum lower:
                    minimum = minimum > lower.Value ? minimum : lower.Value;
                    break;
                case ExactStatContribution.Maximum upper:
                    maximum = maximum < upper.Value ? maximum : upper.Value;
                    break;
                default:
                    throw new MechanicsException("Unknown exact contribution kind.");
            }
        }

        if (minimum > maximum)
        {
            throw new MechanicsException("Exact stat modifiers produced inverted bounds.");
        }

        BigInteger afterAdditionsRaw = baseValue.Raw + additive;
        EnsureExactRange(afterAdditionsRaw, "Exact stat additions");
        ExactValue afterAdditions = new((long)afterAdditionsRaw);
        ExactValue afterScaling = scale.Apply(afterAdditions);
        return new ExactStatEvaluation(
            definition,
            baseValue,
            afterAdditions,
            scale,
            afterScaling,
            minimum,
            maximum,
            Array.AsReadOnly(decisions.ToArray()));
    }

    private static void SelectCandidates(
        IReadOnlyList<Candidate> candidates,
        IList<ExactStatDecision> decisions)
    {
        foreach (IGrouping<StackingGroupId, Candidate> group in candidates.GroupBy(candidate => candidate.Group))
        {
            Candidate[] members = group.ToArray();
            MechanicsStackingPolicy stacking = members[0].Stacking;
            if (members.Any(member => member.Stacking != stacking))
            {
                throw new MechanicsException(
                    $"Stacking group {group.Key.Value} uses more than one policy.");
            }

            switch (stacking)
            {
                case MechanicsStackingPolicy.Sum:
                    foreach (Candidate member in members)
                    {
                        Apply(member, decisions);
                    }
                    break;
                case MechanicsStackingPolicy.Highest:
                case MechanicsStackingPolicy.Lowest:
                    Candidate selected = members[0];
                    foreach (Candidate member in members.Skip(1))
                    {
                        int ordering = member.Contribution.CompareMagnitude(selected.Contribution);
                        if (stacking == MechanicsStackingPolicy.Highest && ordering > 0
                            || stacking == MechanicsStackingPolicy.Lowest && ordering < 0)
                        {
                            selected = member;
                        }
                    }

                    Apply(selected, decisions);
                    break;
                case MechanicsStackingPolicy.UniqueBySource:
                    var definitions = new HashSet<SourceDefinitionId>();
                    foreach (Candidate member in members)
                    {
                        if (definitions.Add(member.SourceDefinition))
                        {
                            Apply(member, decisions);
                        }
                    }
                    break;
                default:
                    throw new MechanicsException("Unknown stat stacking policy.");
            }
        }
    }

    private static void Apply(Candidate candidate, IList<ExactStatDecision> decisions)
    {
        decisions[candidate.DecisionIndex] = decisions[candidate.DecisionIndex] with
        {
            Outcome = MechanicsDecisionOutcome.Applied,
        };
    }

    private static void EnsureWithinBounds(
        ExactValue value,
        ExactValue minimum,
        ExactValue maximum,
        string name)
    {
        if (value < minimum || value > maximum)
        {
            throw new ArgumentOutOfRangeException(nameof(value), $"The {name} is outside its bounds.");
        }
    }

    private static void EnsureExactRange(BigInteger value, string description)
    {
        if (value < -ExactValue.MaximumAbsolute || value > ExactValue.MaximumAbsolute)
        {
            throw new MechanicsArithmeticException($"{description} exceeded the admitted value range.");
        }
    }

    private sealed record Candidate(
        int DecisionIndex,
        SourceDefinitionId SourceDefinition,
        StackingGroupId Group,
        MechanicsStackingPolicy Stacking,
        ExactStatContribution Contribution);
}
