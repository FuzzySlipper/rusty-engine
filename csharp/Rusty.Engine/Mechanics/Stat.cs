namespace Rusty.Engine.Mechanics;

public enum StatModifierKind { Add, Multiply, Minimum, Maximum }

/// <summary>A handle for removing one modifier from the Stat that created it.</summary>
public sealed class StatModifierHandle
{
    internal StatModifierHandle() { }
}

/// <summary>
/// An ordinary mutable numeric stat. Changes validate before replacing its value.
/// Integer getters round explicitly; evaluation keeps fractions unless quantum is set.
/// </summary>
public sealed class Stat
{
    private double _baseValue;
    private double _minimum;
    private double _maximum;
    private double _quantum;
    private MidpointRounding _rounding;
    private MidpointRounding _integerRounding;
    private List<Modifier> _modifiers = [];
    private StatId? _sourceStat;
    private StatSource[] _sources = [];
    private StatEvaluation _evaluation;

    public Stat(double baseValue, double minimum = double.MinValue, double maximum = double.MaxValue,
        double quantum = 0, MidpointRounding rounding = MidpointRounding.AwayFromZero,
        MidpointRounding integerRounding = MidpointRounding.AwayFromZero)
    {
        ValidateRounding(integerRounding);
        _evaluation = Evaluate(baseValue, minimum, maximum, quantum, rounding, _modifiers, null, _sources);
        _baseValue = baseValue;
        _minimum = minimum;
        _maximum = maximum;
        _quantum = quantum;
        _rounding = rounding;
        _integerRounding = integerRounding;
    }

    public double BaseValue
    {
        get => _baseValue;
        set
        {
            var next = Evaluate(value, _minimum, _maximum, _quantum, _rounding, _modifiers, _sourceStat, _sources);
            _baseValue = value;
            Commit(next);
        }
    }
    public double Minimum { get => _minimum; set => SetBounds(value, _maximum); }
    public double Maximum { get => _maximum; set => SetBounds(_minimum, value); }
    public double Quantum { get => _quantum; set => SetQuantization(value, _rounding); }
    public MidpointRounding Rounding { get => _rounding; set => SetQuantization(_quantum, value); }
    public MidpointRounding IntegerRounding
    {
        get => _integerRounding;
        set { ValidateRounding(value); _integerRounding = value; }
    }
    public double Value => _evaluation.Value;
    public float ValueFloat => ToFloat(Value);
    public int ValueInt => checked((int)Math.Round(Value, _integerRounding));
    public long ValueInt64 => checked((long)Math.Round(Value, _integerRounding));

    public void SetBounds(double minimum, double maximum)
    {
        var next = Evaluate(_baseValue, minimum, maximum, _quantum, _rounding, _modifiers, _sourceStat, _sources);
        _minimum = minimum;
        _maximum = maximum;
        Commit(next);
    }

    /// <summary>Zero disables quantization. Resolved bounds win over off-grid rounded endpoints.</summary>
    public void SetQuantization(double quantum, MidpointRounding rounding = MidpointRounding.AwayFromZero)
    {
        var next = Evaluate(_baseValue, _minimum, _maximum, quantum, rounding, _modifiers, _sourceStat, _sources);
        _quantum = quantum;
        _rounding = rounding;
        Commit(next);
    }

    public StatModifierHandle AddModifier(double amount, StatModifierKind kind = StatModifierKind.Add)
    {
        Finite(amount, nameof(amount));
        if (!Enum.IsDefined(kind)) throw new ArgumentOutOfRangeException(nameof(kind));
        var handle = new StatModifierHandle();
        List<Modifier> modifiers = [.. _modifiers, new(handle, kind, amount)];
        var next = Evaluate(_baseValue, _minimum, _maximum, _quantum, _rounding, modifiers, _sourceStat, _sources);
        _modifiers = modifiers;
        Commit(next);
        return handle;
    }

    public bool RemoveModifier(StatModifierHandle handle)
    {
        ArgumentNullException.ThrowIfNull(handle);
        int index = _modifiers.FindIndex(modifier => ReferenceEquals(modifier.Handle, handle));
        if (index < 0) return false;
        List<Modifier> modifiers = [.. _modifiers];
        modifiers.RemoveAt(index);
        var next = Evaluate(_baseValue, _minimum, _maximum, _quantum, _rounding, modifiers, _sourceStat, _sources);
        _modifiers = modifiers;
        Commit(next);
        return true;
    }

    /// <summary>Optional authored provenance and stacking. Replaces sources, preserving local modifiers.</summary>
    public void SetSources(StatId stat, IEnumerable<StatSource> sources)
    {
        ArgumentNullException.ThrowIfNull(stat);
        ArgumentNullException.ThrowIfNull(sources);
        var ordered = MechanicsSourceOrdering.Order(sources,
            source => source.Identity, source => source.Definition, source => source.Priority).ToArray();
        var next = Evaluate(_baseValue, _minimum, _maximum, _quantum, _rounding, _modifiers, stat, ordered);
        _sourceStat = stat;
        _sources = ordered;
        Commit(next);
    }

    public bool RemoveSource(MechanicsSourceIdentity identity)
    {
        ArgumentNullException.ThrowIfNull(identity);
        if (!_sources.Any(source => source.Identity == identity)) return false;
        SetSources(_sourceStat!, _sources.Where(source => source.Identity != identity));
        return true;
    }

    /// <summary>A readout of this evaluation, including selected/suppressed authored contributions.</summary>
    public StatEvaluation Explain() => _evaluation;

    private void Commit(StatEvaluation next) => _evaluation = next;

    internal static float ToFloat(double value)
    {
        if (value < -float.MaxValue || value > float.MaxValue) throw new OverflowException("Stat value does not fit Single.");
        return (float)value;
    }

    internal static void Finite(double value, string name)
    {
        if (!double.IsFinite(value)) throw new ArgumentOutOfRangeException(name, "Stat numbers must be finite.");
    }

    private static double Result(double value)
    {
        if (!double.IsFinite(value)) throw new OverflowException("Stat arithmetic produced a non-finite result.");
        return value;
    }

    private static void ValidateRounding(MidpointRounding mode)
    {
        if (!Enum.IsDefined(mode)) throw new ArgumentOutOfRangeException(nameof(mode));
    }

    private static double Quantize(double value, double quantum, MidpointRounding rounding)
    {
        if (quantum == 0) return value;
        double units = value / quantum;
        // Preserve the sign for directed rounding if division underflows to zero.
        if (units == 0 && value != 0) units = Math.CopySign(double.Epsilon, value);
        // Beyond double's integral precision a grid step cannot change this value.
        const double IntegralPrecisionBoundary = 4503599627370496d; // 2^52
        if (!double.IsFinite(units) || Math.Abs(units) >= IntegralPrecisionBoundary) return value;
        return Result(Math.Round(units, rounding) * quantum);
    }

    private static StatEvaluation Evaluate(double baseValue, double minimum, double maximum, double quantum,
        MidpointRounding rounding, IReadOnlyList<Modifier> modifiers, StatId? stat, IReadOnlyList<StatSource> sources)
    {
        Finite(baseValue, nameof(baseValue));
        Finite(minimum, nameof(minimum));
        Finite(maximum, nameof(maximum));
        Finite(quantum, nameof(quantum));
        ValidateRounding(rounding);
        if (minimum > maximum) throw new ArgumentException("Stat bounds are inverted.");
        if (quantum < 0) throw new ArgumentOutOfRangeException(nameof(quantum));
        var decisions = new List<StatDecision>();
        var candidates = new List<Candidate>();
        foreach (var source in sources)
        {
            bool matched = false;
            for (int i = 0; i < source.Contributions.Count; i++)
            {
                var contribution = source.Contributions[i];
                if (contribution.Stat != stat) continue;
                matched = true;
                candidates.Add(new(decisions.Count, source.Definition, contribution));
                decisions.Add(new(source.Identity, source.Definition, i, MechanicsDecisionOutcome.Suppressed,
                    contribution.Group, contribution.Stacking, contribution.Contribution));
            }
            if (!matched) decisions.Add(new(source.Identity, source.Definition, null,
                MechanicsDecisionOutcome.Inapplicable, null, null, null));
        }
        foreach (var group in candidates.GroupBy(candidate => candidate.Definition.Group))
        {
            var members = group.ToArray();
            var policy = members[0].Definition.Stacking;
            if (members.Any(member => member.Definition.Stacking != policy))
                throw new MechanicsException($"Stacking group {group.Key.Value} uses more than one policy.");
            IEnumerable<Candidate> selected;
            switch (policy)
            {
                case MechanicsStackingPolicy.Sum: selected = members; break;
                case MechanicsStackingPolicy.UniqueByDefinition:
                    selected = members.DistinctBy(member => member.SourceDefinition); break;
                case MechanicsStackingPolicy.Highest:
                case MechanicsStackingPolicy.Lowest:
                    var kind = members[0].Definition.Contribution.Kind;
                    if (members.Any(member => member.Definition.Contribution.Kind != kind))
                        throw new MechanicsException("Highest/lowest stacking requires one contribution kind.");
                    selected = [policy == MechanicsStackingPolicy.Highest
                        ? members.MaxBy(member => member.Definition.Contribution.Magnitude)!
                        : members.MinBy(member => member.Definition.Contribution.Magnitude)!];
                    break;
                default: throw new ArgumentOutOfRangeException(nameof(policy));
            }
            foreach (var member in selected)
                decisions[member.Index] = decisions[member.Index] with { Outcome = MechanicsDecisionOutcome.Applied };
        }
        var operations = modifiers.Select(modifier => (modifier.Kind, modifier.Amount)).ToList();
        operations.AddRange(candidates.Where(candidate => decisions[candidate.Index].Outcome == MechanicsDecisionOutcome.Applied)
            .Select(candidate => (candidate.Definition.Contribution.Kind, candidate.Definition.Contribution.Magnitude)));
        double additions = baseValue;
        foreach (var (kind, amount) in operations)
        {
            Finite(amount, nameof(amount));
            if (kind == StatModifierKind.Add) additions = Result(additions + amount);
            else if (kind == StatModifierKind.Minimum) minimum = Math.Max(minimum, amount);
            else if (kind == StatModifierKind.Maximum) maximum = Math.Min(maximum, amount);
        }
        if (minimum > maximum) throw new MechanicsException("Stat modifiers produced inverted bounds.");
        double scaled = additions;
        foreach (var (kind, amount) in operations)
            if (kind == StatModifierKind.Multiply) scaled = Result(scaled * amount);
        double quantized = Quantize(scaled, quantum, rounding);
        return new(baseValue, additions, scaled, minimum, maximum, quantized,
            Math.Clamp(quantized, minimum, maximum), Array.AsReadOnly(decisions.ToArray()));
    }

    private sealed record Modifier(StatModifierHandle Handle, StatModifierKind Kind, double Amount);
    private sealed record Candidate(int Index, SourceDefinitionId SourceDefinition, StatContributionDefinition Definition);
}

/// <summary>Immutable explanation; ordinary reads use Stat.Value and its numeric accessors.</summary>
public sealed record StatEvaluation(double Base, double AfterAdditions, double AfterScaling,
    double Minimum, double Maximum, double Quantized, double Value, IReadOnlyList<StatDecision> Decisions);
