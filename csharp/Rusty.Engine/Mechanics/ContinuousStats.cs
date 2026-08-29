namespace Rusty.Engine.Mechanics;

/// <summary>Bounds and identity for one finite continuous stat.</summary>
public sealed class ContinuousStatDefinition
{
    public ContinuousStatDefinition(
        StatId id,
        ContinuousValue minimum,
        ContinuousValue maximum)
    {
        Id = id ?? throw new ArgumentNullException(nameof(id));
        if (minimum > maximum)
        {
            throw new ArgumentException("Continuous stat bounds are inverted.", nameof(maximum));
        }

        Minimum = minimum;
        Maximum = maximum;
    }

    public StatId Id { get; }

    public ContinuousValue Minimum { get; }

    public ContinuousValue Maximum { get; }
}

/// <summary>Continuous contribution operation applied to one stat.</summary>
public abstract record ContinuousStatContribution
{
    public abstract ContinuousStatContributionKind Kind { get; }

    /// <summary>Returns the magnitude used by highest/lowest stacking.</summary>
    public int CompareMagnitude(ContinuousStatContribution other)
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
            (Minimum left, Minimum right) => left.Value.CompareTo(right.Value),
            (Maximum left, Maximum right) => left.Value.CompareTo(right.Value),
            _ => throw new MechanicsException("Unknown continuous contribution kind."),
        };
    }

    public sealed record Add(ContinuousValue Amount) : ContinuousStatContribution
    {
        public override ContinuousStatContributionKind Kind => ContinuousStatContributionKind.Add;
    }

    public sealed record Minimum(ContinuousValue Value) : ContinuousStatContribution
    {
        public override ContinuousStatContributionKind Kind => ContinuousStatContributionKind.Minimum;
    }

    public sealed record Maximum(ContinuousValue Value) : ContinuousStatContribution
    {
        public override ContinuousStatContributionKind Kind => ContinuousStatContributionKind.Maximum;
    }
}

/// <summary>Kind of a continuous stat contribution.</summary>
public enum ContinuousStatContributionKind
{
    Add,
    Minimum,
    Maximum,
}

/// <summary>One typed continuous contribution from one source to one stat.</summary>
public sealed record ContinuousStatContributionDefinition
{
    public ContinuousStatContributionDefinition(
        StatId stat,
        StackingGroupId group,
        MechanicsStackingPolicy stacking,
        ContinuousStatContribution contribution)
    {
        Stat = stat ?? throw new ArgumentNullException(nameof(stat));
        Group = group ?? throw new ArgumentNullException(nameof(group));
        Contribution = contribution ?? throw new ArgumentNullException(nameof(contribution));
        Stacking = stacking;
    }

    public StatId Stat { get; }

    public StackingGroupId Group { get; }

    public MechanicsStackingPolicy Stacking { get; }

    public ContinuousStatContribution Contribution { get; }
}

/// <summary>One source decision retained in a continuous stat evaluation.</summary>
public readonly record struct ContinuousStatDecision(
    MechanicsSourceIdentity Source,
    SourceDefinitionId SourceDefinition,
    int? ContributionIndex,
    MechanicsDecisionOutcome Outcome,
    StackingGroupId? StackingGroup,
    MechanicsStackingPolicy? Stacking,
    ContinuousStatContribution? Contribution);

/// <summary>Copied result of one continuous stat evaluation.</summary>
public sealed class ContinuousStatEvaluation
{
    internal ContinuousStatEvaluation(
        ContinuousStatDefinition definition,
        ContinuousValue baseValue,
        ContinuousValue afterAdditions,
        ContinuousValue minimum,
        ContinuousValue maximum,
        IReadOnlyList<ContinuousStatDecision> decisions)
    {
        Definition = definition;
        Base = baseValue;
        AfterAdditions = afterAdditions;
        Unconstrained = afterAdditions;
        Minimum = minimum;
        Maximum = maximum;
        Value = afterAdditions.Clamp(minimum, maximum);
        Decisions = decisions;
    }

    public ContinuousStatDefinition Definition { get; }

    public ContinuousValue Base { get; }

    public ContinuousValue AfterAdditions { get; }

    public ContinuousValue Unconstrained { get; }

    public ContinuousValue Minimum { get; }

    public ContinuousValue Maximum { get; }

    public ContinuousValue Value { get; }

    public IReadOnlyList<ContinuousStatDecision> Decisions { get; }
}

/// <summary>Evaluates direct caller-supplied finite continuous sources.</summary>
public static class ContinuousStatEvaluator
{
    public static ContinuousStatEvaluation Evaluate(
        ContinuousStatDefinition definition,
        ContinuousValue baseValue,
        IEnumerable<ContinuousSource> sources)
    {
        ArgumentNullException.ThrowIfNull(definition);
        ArgumentNullException.ThrowIfNull(sources);
        EnsureWithinBounds(baseValue, definition.Minimum, definition.Maximum, "base stat");

        ContinuousSource[] ordered = MechanicsSourceOrdering.Order(
            sources,
            source => source.Identity,
            source => source.Definition,
            source => source.Priority).ToArray();

        var decisions = new List<ContinuousStatDecision>();
        var candidates = new List<Candidate>();
        foreach (ContinuousSource source in ordered)
        {
            bool matched = false;
            for (int index = 0; index < source.Contributions.Count; index++)
            {
                ContinuousStatContributionDefinition contribution = source.Contributions[index];
                if (contribution.Stat != definition.Id)
                {
                    continue;
                }

                matched = true;
                int decisionIndex = decisions.Count;
                decisions.Add(new ContinuousStatDecision(
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
                decisions.Add(new ContinuousStatDecision(
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

        ContinuousValue afterAdditions = baseValue;
        ContinuousValue minimum = definition.Minimum;
        ContinuousValue maximum = definition.Maximum;
        foreach (Candidate candidate in candidates)
        {
            if (decisions[candidate.DecisionIndex].Outcome != MechanicsDecisionOutcome.Applied)
            {
                continue;
            }

            switch (candidate.Contribution)
            {
                case ContinuousStatContribution.Add add:
                    afterAdditions = afterAdditions.CheckedAdd(add.Amount);
                    break;
                case ContinuousStatContribution.Minimum lower:
                    minimum = minimum > lower.Value ? minimum : lower.Value;
                    break;
                case ContinuousStatContribution.Maximum upper:
                    maximum = maximum < upper.Value ? maximum : upper.Value;
                    break;
                default:
                    throw new MechanicsException("Unknown continuous contribution kind.");
            }
        }

        if (minimum > maximum)
        {
            throw new MechanicsException("Continuous stat modifiers produced inverted bounds.");
        }

        return new ContinuousStatEvaluation(
            definition,
            baseValue,
            afterAdditions,
            minimum,
            maximum,
            Array.AsReadOnly(decisions.ToArray()));
    }

    private static void SelectCandidates(
        IReadOnlyList<Candidate> candidates,
        IList<ContinuousStatDecision> decisions)
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

    private static void Apply(Candidate candidate, IList<ContinuousStatDecision> decisions)
    {
        decisions[candidate.DecisionIndex] = decisions[candidate.DecisionIndex] with
        {
            Outcome = MechanicsDecisionOutcome.Applied,
        };
    }

    private static void EnsureWithinBounds(
        ContinuousValue value,
        ContinuousValue minimum,
        ContinuousValue maximum,
        string name)
    {
        if (value < minimum || value > maximum)
        {
            throw new ArgumentOutOfRangeException(nameof(value), $"The {name} is outside its bounds.");
        }
    }

    private sealed record Candidate(
        int DecisionIndex,
        SourceDefinitionId SourceDefinition,
        StackingGroupId Group,
        MechanicsStackingPolicy Stacking,
        ContinuousStatContribution Contribution);
}
