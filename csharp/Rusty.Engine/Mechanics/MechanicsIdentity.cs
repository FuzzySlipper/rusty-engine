using System.Text;
using Rusty.Engine.Entities;

namespace Rusty.Engine.Mechanics;

/// <summary>
/// Base type for the small, domain-specific identities used by managed
/// mechanics. Identities are deliberately not interchangeable strings.
/// </summary>
public abstract record MechanicsIdentity
{
    public const int MaximumBytes = 96;

    protected MechanicsIdentity(string value)
    {
        Value = MechanicsIdentityValidation.Validate(value, GetType().Name);
    }

    public string Value { get; }

    public override string ToString() => Value;
}

internal static class MechanicsIdentityValidation
{
    internal static string Validate(string? value, string kind)
    {
        ArgumentNullException.ThrowIfNull(value);
        if (value.Length == 0)
        {
            throw new ArgumentException($"{kind} identities cannot be empty.", nameof(value));
        }

        if (Encoding.UTF8.GetByteCount(value) > MechanicsIdentity.MaximumBytes)
        {
            throw new ArgumentException(
                $"{kind} identities cannot exceed {MechanicsIdentity.MaximumBytes} UTF-8 bytes.",
                nameof(value));
        }

        if (value[0] is < 'a' or > 'z')
        {
            throw new ArgumentException(
                $"{kind} identities must start with a lowercase ASCII letter.",
                nameof(value));
        }

        foreach (char character in value)
        {
            bool supported = character is >= 'a' and <= 'z'
                || character is >= '0' and <= '9'
                || character is '.' or '-' or '_';
            if (!supported)
            {
                throw new ArgumentException(
                    $"{kind} identities contain unsupported characters.",
                    nameof(value));
            }
        }

        return value;
    }
}

/// <summary>Identity for one evaluated exact or continuous stat.</summary>
public sealed record StatId : MechanicsIdentity
{
    private StatId(string value) : base(value) { }

    public static StatId Parse(string value) => new(value);

    public static bool TryParse(string? value, out StatId? result)
    {
        try
        {
            result = value is null ? null : Parse(value);
            return result is not null;
        }
        catch (ArgumentException)
        {
            result = null;
            return false;
        }
    }
}

/// <summary>Identity for one bounded current-value track.</summary>
public sealed record TrackId : MechanicsIdentity
{
    private TrackId(string value) : base(value) { }

    public static TrackId Parse(string value) => new(value);

    public static bool TryParse(string? value, out TrackId? result)
    {
        try
        {
            result = value is null ? null : Parse(value);
            return result is not null;
        }
        catch (ArgumentException)
        {
            result = null;
            return false;
        }
    }
}

/// <summary>Identity for one source definition.</summary>
public sealed record SourceDefinitionId : MechanicsIdentity
{
    private SourceDefinitionId(string value) : base(value) { }

    public static SourceDefinitionId Parse(string value) => new(value);

    public static bool TryParse(string? value, out SourceDefinitionId? result)
    {
        try
        {
            result = value is null ? null : Parse(value);
            return result is not null;
        }
        catch (ArgumentException)
        {
            result = null;
            return false;
        }
    }
}

/// <summary>Identity for one source activation instance.</summary>
public sealed record SourceInstanceId : MechanicsIdentity
{
    private SourceInstanceId(string value) : base(value) { }

    public static SourceInstanceId Parse(string value) => new(value);

    public static bool TryParse(string? value, out SourceInstanceId? result)
    {
        try
        {
            result = value is null ? null : Parse(value);
            return result is not null;
        }
        catch (ArgumentException)
        {
            result = null;
            return false;
        }
    }
}

/// <summary>Identity for one source/effect stacking group.</summary>
public sealed record StackingGroupId : MechanicsIdentity
{
    private StackingGroupId(string value) : base(value) { }

    public static StackingGroupId Parse(string value) => new(value);

    public static bool TryParse(string? value, out StackingGroupId? result)
    {
        try
        {
            result = value is null ? null : Parse(value);
            return result is not null;
        }
        catch (ArgumentException)
        {
            result = null;
            return false;
        }
    }
}

/// <summary>Identity for one authored effect definition.</summary>
public sealed record EffectDefinitionId : MechanicsIdentity
{
    private EffectDefinitionId(string value) : base(value) { }

    public static EffectDefinitionId Parse(string value) => new(value);

    public static bool TryParse(string? value, out EffectDefinitionId? result)
    {
        try
        {
            result = value is null ? null : Parse(value);
            return result is not null;
        }
        catch (ArgumentException)
        {
            result = null;
            return false;
        }
    }
}

/// <summary>Identity for one live effect activation.</summary>
public sealed record EffectInstanceId : MechanicsIdentity
{
    private EffectInstanceId(string value) : base(value) { }

    public static EffectInstanceId Parse(string value) => new(value);

    public static bool TryParse(string? value, out EffectInstanceId? result)
    {
        try
        {
            result = value is null ? null : Parse(value);
            return result is not null;
        }
        catch (ArgumentException)
        {
            result = null;
            return false;
        }
    }
}

/// <summary>Identity for one caller-owned operation correlation.</summary>
public sealed record OperationId : MechanicsIdentity
{
    private OperationId(string value) : base(value) { }

    public static OperationId Parse(string value) => new(value);

    public static bool TryParse(string? value, out OperationId? result)
    {
        try
        {
            result = value is null ? null : Parse(value);
            return result is not null;
        }
        catch (ArgumentException)
        {
            result = null;
            return false;
        }
    }
}

/// <summary>
/// Describes where one source activation came from. The optional entity values
/// let the same managed mechanism be used for entity-backed and standalone
/// product state without inventing an entity registry here.
/// </summary>
public abstract record MechanicsSourceIdentity : IComparable<MechanicsSourceIdentity>
{
    protected abstract int KindOrder { get; }

    public int CompareTo(MechanicsSourceIdentity? other)
    {
        if (other is null)
        {
            return 1;
        }

        int kind = KindOrder.CompareTo(other.KindOrder);
        return kind != 0 ? kind : ComparePayload(other);
    }

    protected abstract int ComparePayload(MechanicsSourceIdentity other);
}

/// <summary>One intrinsic source activation.</summary>
public sealed record IntrinsicSourceIdentity(EntityId? Entity, SourceInstanceId Instance)
    : MechanicsSourceIdentity
{
    protected override int KindOrder => 0;

    protected override int ComparePayload(MechanicsSourceIdentity other)
    {
        IntrinsicSourceIdentity value = (IntrinsicSourceIdentity)other;
        return CompareNullableEntity(Entity, value.Entity, Instance.Value, value.Instance.Value);
    }

    private static int CompareNullableEntity(
        EntityId? leftEntity,
        EntityId? rightEntity,
        string leftValue,
        string rightValue)
    {
        int entity = Nullable.Compare(leftEntity, rightEntity);
        return entity != 0 ? entity : StringComparer.Ordinal.Compare(leftValue, rightValue);
    }
}

/// <summary>One effect-stack source activation.</summary>
public sealed record EffectSourceIdentity(
    EntityId? Entity,
    EffectInstanceId Effect,
    ushort Stack,
    SourceDefinitionId Source) : MechanicsSourceIdentity
{
    protected override int KindOrder => 1;

    protected override int ComparePayload(MechanicsSourceIdentity other)
    {
        EffectSourceIdentity value = (EffectSourceIdentity)other;
        int entity = Nullable.Compare(Entity, value.Entity);
        if (entity != 0)
        {
            return entity;
        }

        int effect = StringComparer.Ordinal.Compare(Effect.Value, value.Effect.Value);
        if (effect != 0)
        {
            return effect;
        }

        int stack = Stack.CompareTo(value.Stack);
        return stack != 0
            ? stack
            : StringComparer.Ordinal.Compare(Source.Value, value.Source.Value);
    }
}

/// <summary>One source activation supplied by an equipped item.</summary>
public sealed record EquippedItemSourceIdentity(
    EntityId? Owner,
    EntityId? Item,
    SourceDefinitionId Source) : MechanicsSourceIdentity
{
    protected override int KindOrder => 2;

    protected override int ComparePayload(MechanicsSourceIdentity other)
    {
        EquippedItemSourceIdentity value = (EquippedItemSourceIdentity)other;
        int owner = Nullable.Compare(Owner, value.Owner);
        if (owner != 0)
        {
            return owner;
        }

        int item = Nullable.Compare(Item, value.Item);
        return item != 0
            ? item
            : StringComparer.Ordinal.Compare(Source.Value, value.Source.Value);
    }
}

/// <summary>One request-local source activation.</summary>
public sealed record RequestSourceIdentity(OperationId Operation, SourceInstanceId Instance)
    : MechanicsSourceIdentity
{
    protected override int KindOrder => 3;

    protected override int ComparePayload(MechanicsSourceIdentity other)
    {
        RequestSourceIdentity value = (RequestSourceIdentity)other;
        int operation = StringComparer.Ordinal.Compare(Operation.Value, value.Operation.Value);
        return operation != 0
            ? operation
            : StringComparer.Ordinal.Compare(Instance.Value, value.Instance.Value);
    }
}
