namespace Rusty.Engine.Mechanics;

/// <summary>How contributions in one stat group combine.</summary>
public enum MechanicsStackingPolicy
{
    Sum,
    Highest,
    Lowest,
    UniqueBySource,
}

/// <summary>Outcome recorded for a source contribution during evaluation.</summary>
public enum MechanicsDecisionOutcome
{
    Applied,
    Suppressed,
    Inapplicable,
}

/// <summary>
/// One exact source activation and its typed contributions. The caller owns
/// the definition collection; this value is just an explicit activation and
/// never consults a global registry.
/// </summary>
public sealed class ExactSource
{
    public ExactSource(
        MechanicsSourceIdentity identity,
        SourceDefinitionId definition,
        short priority,
        IEnumerable<ExactStatContributionDefinition> contributions)
    {
        Identity = identity ?? throw new ArgumentNullException(nameof(identity));
        Definition = definition ?? throw new ArgumentNullException(nameof(definition));
        Contributions = Copy(contributions, nameof(contributions));
        Priority = priority;
    }

    public MechanicsSourceIdentity Identity { get; }

    public SourceDefinitionId Definition { get; }

    public short Priority { get; }

    public IReadOnlyList<ExactStatContributionDefinition> Contributions { get; }

    private static IReadOnlyList<T> Copy<T>(IEnumerable<T> values, string name)
    {
        ArgumentNullException.ThrowIfNull(values, name);
        return Array.AsReadOnly(values.ToArray());
    }
}

/// <summary>One continuous source activation and its typed contributions.</summary>
public sealed class ContinuousSource
{
    public ContinuousSource(
        MechanicsSourceIdentity identity,
        SourceDefinitionId definition,
        short priority,
        IEnumerable<ContinuousStatContributionDefinition> contributions)
    {
        Identity = identity ?? throw new ArgumentNullException(nameof(identity));
        Definition = definition ?? throw new ArgumentNullException(nameof(definition));
        Contributions = Copy(contributions, nameof(contributions));
        Priority = priority;
    }

    public MechanicsSourceIdentity Identity { get; }

    public SourceDefinitionId Definition { get; }

    public short Priority { get; }

    public IReadOnlyList<ContinuousStatContributionDefinition> Contributions { get; }

    private static IReadOnlyList<T> Copy<T>(IEnumerable<T> values, string name)
    {
        ArgumentNullException.ThrowIfNull(values, name);
        return Array.AsReadOnly(values.ToArray());
    }
}

/// <summary>One source activation emitted by an active effect.</summary>
public readonly record struct EffectSourceActivation(
    MechanicsSourceIdentity Identity,
    SourceDefinitionId Definition);

/// <summary>
/// Orders source activations by the same stable tuple used by the Rust
/// mechanics donor: priority, provenance identity, then source definition.
/// Duplicate provenance is rejected instead of silently overwriting a source.
/// </summary>
public static class MechanicsSourceOrdering
{
    public static IReadOnlyList<T> Order<T>(
        IEnumerable<T> values,
        Func<T, MechanicsSourceIdentity> identity,
        Func<T, SourceDefinitionId> definition,
        Func<T, short> priority)
    {
        ArgumentNullException.ThrowIfNull(values);
        ArgumentNullException.ThrowIfNull(identity);
        ArgumentNullException.ThrowIfNull(definition);
        ArgumentNullException.ThrowIfNull(priority);

        SourceOrderEntry<T>[] ordered = values
            .Select(value => new SourceOrderEntry<T>(
                value,
                identity(value) ?? throw new ArgumentException("A source identity cannot be null."),
                definition(value) ?? throw new ArgumentException("A source definition cannot be null."),
                priority(value)))
            .OrderBy(entry => entry.Priority)
            .ThenBy(entry => entry.Identity)
            .ThenBy(entry => entry.Definition.Value, StringComparer.Ordinal)
            .ToArray();

        for (int index = 1; index < ordered.Length; index++)
        {
            if (ordered[index - 1].Identity == ordered[index].Identity)
            {
                throw new MechanicsException(
                    $"Source identity {ordered[index].Identity} was activated more than once.");
            }
        }

        return ordered.Select(entry => entry.Value).ToArray();
    }

    private readonly record struct SourceOrderEntry<T>(
        T Value,
        MechanicsSourceIdentity Identity,
        SourceDefinitionId Definition,
        short Priority);
}

/// <summary>Common managed mechanics operation error.</summary>
public sealed class MechanicsException : InvalidOperationException
{
    public MechanicsException(string message) : base(message) { }
}
