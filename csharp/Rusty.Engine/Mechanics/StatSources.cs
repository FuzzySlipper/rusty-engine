namespace Rusty.Engine.Mechanics;

/// <summary>Optional authored contribution; ordinary modifiers only need a number and kind.</summary>
public abstract record StatContribution
{
    private StatContribution() { }
    public abstract StatModifierKind Kind { get; }
    internal abstract double Magnitude { get; }
    public sealed record Add(double Amount) : StatContribution
    {
        public override StatModifierKind Kind => StatModifierKind.Add;
        internal override double Magnitude => Amount;
    }
    public sealed record Multiply(double Factor) : StatContribution
    {
        public override StatModifierKind Kind => StatModifierKind.Multiply;
        internal override double Magnitude => Factor;
    }
    public sealed record Minimum(double Value) : StatContribution
    {
        public override StatModifierKind Kind => StatModifierKind.Minimum;
        internal override double Magnitude => Value;
    }
    public sealed record Maximum(double Value) : StatContribution
    {
        public override StatModifierKind Kind => StatModifierKind.Maximum;
        internal override double Magnitude => Value;
    }
}

public sealed record StatContributionDefinition
{
    public StatContributionDefinition(StatId stat, StackingGroupId group,
        MechanicsStackingPolicy stacking, StatContribution contribution)
    {
        Stat = stat ?? throw new ArgumentNullException(nameof(stat));
        Group = group ?? throw new ArgumentNullException(nameof(group));
        Contribution = contribution ?? throw new ArgumentNullException(nameof(contribution));
        global::Rusty.Engine.Mechanics.Stat.Finite(contribution.Magnitude, nameof(contribution));
        if (!Enum.IsDefined(stacking)) throw new ArgumentOutOfRangeException(nameof(stacking));
        Stacking = stacking;
    }
    public StatId Stat { get; }
    public StackingGroupId Group { get; }
    public MechanicsStackingPolicy Stacking { get; }
    public StatContribution Contribution { get; }
}

public sealed class StatSource
{
    public StatSource(MechanicsSourceIdentity identity, SourceDefinitionId definition, short priority,
        IEnumerable<StatContributionDefinition> contributions)
    {
        Identity = identity ?? throw new ArgumentNullException(nameof(identity));
        Definition = definition ?? throw new ArgumentNullException(nameof(definition));
        ArgumentNullException.ThrowIfNull(contributions);
        var copied = contributions.ToArray();
        if (copied.Any(item => item is null)) throw new ArgumentException("A contribution cannot be null.", nameof(contributions));
        Contributions = Array.AsReadOnly(copied);
        Priority = priority;
    }
    public MechanicsSourceIdentity Identity { get; }
    public SourceDefinitionId Definition { get; }
    public short Priority { get; }
    public IReadOnlyList<StatContributionDefinition> Contributions { get; }
}

public readonly record struct StatDecision(MechanicsSourceIdentity Source, SourceDefinitionId SourceDefinition,
    int? ContributionIndex, MechanicsDecisionOutcome Outcome, StackingGroupId? StackingGroup,
    MechanicsStackingPolicy? Stacking, StatContribution? Contribution);
