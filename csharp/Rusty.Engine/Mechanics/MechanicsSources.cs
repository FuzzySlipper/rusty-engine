namespace Rusty.Engine.Mechanics;

/// <summary>How contributions in one stat group combine.</summary>
public enum MechanicsStackingPolicy
{
    Sum,
    Highest,
    Lowest,
    UniqueByDefinition,
}

/// <summary>Outcome recorded for a source contribution during evaluation.</summary>
public enum MechanicsDecisionOutcome
{
    Applied,
    Suppressed,
    Inapplicable,
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

        var identities = new HashSet<MechanicsSourceIdentity>();
        foreach (var entry in ordered)
        {
            if (!identities.Add(entry.Identity))
                throw new MechanicsException(MechanicsRefusal.AlreadyPresent, $"Source identity {entry.Identity} was activated more than once.");
        }

        return ordered.Select(entry => entry.Value).ToArray();
    }

    private readonly record struct SourceOrderEntry<T>(
        T Value,
        MechanicsSourceIdentity Identity,
        SourceDefinitionId Definition,
        short Priority);
}

/// <summary>Why a mechanics operation was refused.</summary>
public enum MechanicsRefusal
{
    /// <summary>A fungible stack would exceed its item's maximum quantity.</summary>
    StackMaximum,
    /// <summary>An inventory capacity limit or an effect group's instance limit would be exceeded.</summary>
    Capacity,
    /// <summary>Quantity or capacity arithmetic overflowed.</summary>
    Overflow,
    /// <summary>A stack or track holds less than the requested amount.</summary>
    Insufficient,
    /// <summary>The owner, stack, item, effect, equipped item or containment named is not present.</summary>
    NotFound,
    /// <summary>The identity, stack, registration, container, equipped item or effect is already present, or an owner being retired still holds stacks or unique items.</summary>
    AlreadyPresent,
    /// <summary>An equipment slot or exclusivity group already holds another item.</summary>
    Occupied,
    /// <summary>A unique item must be unequipped before it is transferred or destroyed.</summary>
    Equipped,
    /// <summary>The item or effect definition's kind, equipment policy or stacking policy does not allow the operation.</summary>
    Incompatible,
    /// <summary>The request itself is malformed, such as a zero quantity or the same source and destination.</summary>
    InvalidRequest,
    /// <summary>Definitions, stacking policies, modifiers or restored effects are invalid or contradict each other.</summary>
    Inconsistent,
    /// <summary>The inventory store changed after the edit began.</summary>
    RevisionConflict,
}

/// <summary>Common managed mechanics operation error.</summary>
public sealed class MechanicsException : InvalidOperationException
{
    public MechanicsException(MechanicsRefusal reason, string message) : base(message) => Reason = reason;

    /// <summary>Why the operation was refused.</summary>
    public MechanicsRefusal Reason { get; }
}
