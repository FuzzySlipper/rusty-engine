namespace Rusty.Engine.Mechanics;

/// <summary>Whether an item definition represents a stack or one entity-backed item.</summary>
public enum ItemKind
{
    Fungible,
    Unique,
}

/// <summary>One typed capacity cost applied once per stack quantity or unique item.</summary>
public readonly record struct ItemCapacityCost
{
    public ItemCapacityCost(CapacityMetricId metric, ulong units)
    {
        Metric = metric ?? throw new ArgumentNullException(nameof(metric));
        if (units == 0)
        {
            throw new ArgumentOutOfRangeException(
                nameof(units),
                "Capacity costs must be positive.");
        }

        Units = units;
    }

    public CapacityMetricId Metric { get; }

    public ulong Units { get; }
}

/// <summary>Equipment requirements attached to one unique item definition.</summary>
public sealed class ItemEquipmentPolicy
{
    public ItemEquipmentPolicy(
        ushort requiredSlots,
        EquipmentExclusivityId? exclusiveGroup = null)
    {
        if (requiredSlots == 0)
        {
            throw new ArgumentOutOfRangeException(
                nameof(requiredSlots),
                "An item must require at least one slot.");
        }

        RequiredSlots = requiredSlots;
        ExclusiveGroup = exclusiveGroup;
    }

    public ushort RequiredSlots { get; }

    public EquipmentExclusivityId? ExclusiveGroup { get; }
}

/// <summary>
/// Immutable product-authored item metadata. It intentionally contains no
/// catalog registration or product meaning beyond the mechanical facts needed
/// by inventory and equipment operations.
/// </summary>
public sealed class ItemDefinition
{
    public ItemDefinition(
        ItemDefinitionId id,
        ItemKind kind,
        ulong maximumQuantity,
        IEnumerable<ItemClassificationId>? classifications = null,
        IEnumerable<ItemCapacityCost>? capacityCosts = null,
        ItemEquipmentPolicy? equipment = null,
        IEnumerable<SourceDefinitionId>? sourceDefinitions = null)
    {
        Id = id ?? throw new ArgumentNullException(nameof(id));
        if (maximumQuantity == 0)
        {
            throw new ArgumentOutOfRangeException(
                nameof(maximumQuantity),
                "Item quantities must be positive.");
        }

        if (kind == ItemKind.Unique && maximumQuantity != 1)
        {
            throw new ArgumentException("Unique item definitions must have a quantity of one.", nameof(maximumQuantity));
        }

        Classifications = CopySortedDistinct(classifications, "item classifications");
        CapacityCosts = CopyCosts(capacityCosts);
        Equipment = equipment;
        SourceDefinitions = CopySortedDistinct(sourceDefinitions, "item source definitions");
        Kind = kind;
        MaximumQuantity = maximumQuantity;
    }

    public ItemDefinitionId Id { get; }

    public ItemKind Kind { get; }

    public ulong MaximumQuantity { get; }

    public IReadOnlyList<ItemClassificationId> Classifications { get; }

    public IReadOnlyList<ItemCapacityCost> CapacityCosts { get; }

    public ItemEquipmentPolicy? Equipment { get; }

    public IReadOnlyList<SourceDefinitionId> SourceDefinitions { get; }

    internal bool Matches(ItemDefinition other)
    {
        ArgumentNullException.ThrowIfNull(other);
        return Id == other.Id
            && Kind == other.Kind
            && MaximumQuantity == other.MaximumQuantity
            && Classifications.SequenceEqual(other.Classifications)
            && CapacityCosts.SequenceEqual(other.CapacityCosts)
            && EquipmentMatches(Equipment, other.Equipment)
            && SourceDefinitions.SequenceEqual(other.SourceDefinitions);
    }

    private static bool EquipmentMatches(ItemEquipmentPolicy? left, ItemEquipmentPolicy? right) =>
        left is null && right is null
        || left is not null && right is not null
            && left.RequiredSlots == right.RequiredSlots
            && left.ExclusiveGroup == right.ExclusiveGroup;

    private static IReadOnlyList<ItemCapacityCost> CopyCosts(IEnumerable<ItemCapacityCost>? values)
    {
        if (values is null)
        {
            return Array.Empty<ItemCapacityCost>();
        }

        ItemCapacityCost[] costs = values
            .OrderBy(value => value.Metric.Value, StringComparer.Ordinal)
            .ToArray();
        for (int index = 1; index < costs.Length; index++)
        {
            if (costs[index - 1].Metric == costs[index].Metric)
            {
                throw new ArgumentException(
                    $"Item capacity metric {costs[index].Metric} was declared more than once.",
                    nameof(values));
            }
        }

        return Array.AsReadOnly(costs);
    }

    private static IReadOnlyList<T> CopySortedDistinct<T>(
        IEnumerable<T>? values,
        string name)
        where T : MechanicsIdentity
    {
        if (values is null)
        {
            return Array.Empty<T>();
        }

        T[] copied = values
            .Select(value => value ?? throw new ArgumentException($"{name} cannot contain null values.", nameof(values)))
            .OrderBy(value => value.Value, StringComparer.Ordinal)
            .ToArray();
        for (int index = 1; index < copied.Length; index++)
        {
            if (copied[index - 1] == copied[index])
            {
                throw new ArgumentException($"{name} must be unique.", nameof(values));
            }
        }

        return Array.AsReadOnly(copied);
    }
}
