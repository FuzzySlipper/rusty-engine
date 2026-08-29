using Rusty.Engine.Entities;

namespace Rusty.Engine.Mechanics;

/// <summary>One caller-authored slot and the classifications it accepts.</summary>
public sealed class EquipmentSlotDefinition
{
    public EquipmentSlotDefinition(
        EquipmentSlotId id,
        IEnumerable<ItemClassificationId>? allowedClassifications = null)
    {
        Id = id ?? throw new ArgumentNullException(nameof(id));
        AllowedClassifications = CopyClassifications(allowedClassifications);
    }

    public EquipmentSlotId Id { get; }

    public IReadOnlyList<ItemClassificationId> AllowedClassifications { get; }

    private static IReadOnlyList<ItemClassificationId> CopyClassifications(
        IEnumerable<ItemClassificationId>? values)
    {
        if (values is null)
        {
            return Array.Empty<ItemClassificationId>();
        }

        ItemClassificationId[] copied = values
            .Select(value => value ?? throw new ArgumentException("Slot classifications cannot be null.", nameof(values)))
            .OrderBy(value => value.Value, StringComparer.Ordinal)
            .ToArray();
        if (copied.Length > ManagedInventoryLimits.MaximumClassificationsPerItem)
        {
            throw new ArgumentException(
                $"A slot cannot admit more than {ManagedInventoryLimits.MaximumClassificationsPerItem} classifications.",
                nameof(values));
        }

        if (copied.Distinct().Count() != copied.Length)
        {
            throw new ArgumentException("Slot classifications must be unique.", nameof(values));
        }

        return Array.AsReadOnly(copied);
    }
}

/// <summary>A slot-to-item assignment in one product equipment state.</summary>
public readonly record struct EquipmentAssignment(EquipmentSlotId Slot, EntityId Item);

/// <summary>Product-owned equipment assignment state for one owner.</summary>
public sealed class EquipmentState
{
    private readonly Dictionary<EquipmentSlotId, EquipmentSlotEntry> _assignments = [];

    public EquipmentState(EntityId owner)
    {
        Owner = owner;
    }

    public EntityId Owner { get; }

    public ulong Revision { get; private set; }

    public IReadOnlyList<EquipmentAssignment> Assignments => _assignments.Values
        .OrderBy(entry => entry.Slot.Id.Value, StringComparer.Ordinal)
        .Select(entry => new EquipmentAssignment(entry.Slot.Id, entry.Item))
        .ToArray();

    public bool ContainsItem(EntityId item) => _assignments.Values.Any(entry => entry.Item == item);

    internal EquipmentState Clone()
    {
        var result = new EquipmentState(Owner)
        {
            Revision = Revision,
        };
        foreach ((EquipmentSlotId id, EquipmentSlotEntry entry) in _assignments)
        {
            result._assignments.Add(id, entry);
        }

        return result;
    }

    internal IEnumerable<EquipmentSlotEntry> Entries() => _assignments.Values;

    internal bool TryGet(EquipmentSlotId slot, out EquipmentSlotEntry? entry) =>
        _assignments.TryGetValue(slot, out entry);

    internal void Set(EquipmentSlotDefinition slot, EntityId item) =>
        _assignments[slot.Id] = new EquipmentSlotEntry(slot, item);

    internal bool Remove(EquipmentSlotId slot) => _assignments.Remove(slot);

    internal void SetRevision(ulong revision) => Revision = revision;

    internal sealed record EquipmentSlotEntry(EquipmentSlotDefinition Slot, EntityId Item);
}

/// <summary>Kind of one equipment mutation.</summary>
public enum EquipmentMutationKind
{
    Equip,
    Unequip,
    Swap,
}

/// <summary>One changed slot in an equipment mutation receipt.</summary>
public readonly record struct EquipmentSlotChange(
    EquipmentSlotId Slot,
    EntityId? Before,
    EntityId? After);

/// <summary>One deterministic source activated by an equipped item.</summary>
public readonly record struct EquipmentSourceActivation(
    EquippedItemSourceIdentity Identity,
    SourceDefinitionId Definition);

/// <summary>Evidence for one validated equipment mutation.</summary>
public sealed class EquipmentMutationReceipt
{
    internal EquipmentMutationReceipt(
        EquipmentMutationKind kind,
        EntityId owner,
        EntityId item,
        EntityId? replacedItem,
        ulong equipmentRevisionBefore,
        ulong equipmentRevisionAfter,
        IReadOnlyList<EquipmentSlotChange> changes,
        IReadOnlyList<EquipmentSourceActivation> sourceActivations)
    {
        Kind = kind;
        Owner = owner;
        Item = item;
        ReplacedItem = replacedItem;
        EquipmentRevisionBefore = equipmentRevisionBefore;
        EquipmentRevisionAfter = equipmentRevisionAfter;
        Changes = changes;
        SourceActivations = sourceActivations;
    }

    public EquipmentMutationKind Kind { get; }

    public EntityId Owner { get; }

    public EntityId Item { get; }

    public EntityId? ReplacedItem { get; }

    public ulong EquipmentRevisionBefore { get; }

    public ulong EquipmentRevisionAfter { get; }

    public IReadOnlyList<EquipmentSlotChange> Changes { get; }

    public IReadOnlyList<EquipmentSourceActivation> SourceActivations { get; }
}

public sealed partial class InventoryWorld
{
    internal EquipmentMutationReceipt EquipCore(
        EntityId owner,
        EntityId item,
        IEnumerable<EquipmentSlotDefinition> slots)
    {
        ArgumentNullException.ThrowIfNull(slots);
        EquipmentState equipment = RequireEquipment(owner);
        ItemState itemState = RequireItem(item);
        EnsureContainedBy(item, owner);
        if (equipment.ContainsItem(item))
        {
            throw new MechanicsException($"Unique item {item.Value} is already equipped by {owner.Value}.");
        }

        EquipmentSlotDefinition[] requested = CopyRequestedSlots(slots);
        EnsureRequestedSlotCount(itemState.Definition, requested);
        EquipmentState candidate = equipment.Clone();
        foreach (EquipmentSlotDefinition slot in requested)
        {
            if (candidate.TryGet(slot.Id, out EquipmentState.EquipmentSlotEntry? occupied)
                && occupied is not null)
            {
                throw new MechanicsException(
                    $"Equipment slot {slot.Id} is occupied by item {occupied.Item.Value}.");
            }
            candidate.Set(slot, item);
        }

        IReadOnlyList<EquipmentSourceActivation> activations = ValidateEquipment(owner, candidate, out _);
        IReadOnlyList<EquipmentSlotChange> changes = Changes(equipment, candidate);
        candidate.SetRevision(checked(candidate.Revision + 1));
        _equipment[owner] = candidate;
        TouchWorld();
        return new EquipmentMutationReceipt(
            EquipmentMutationKind.Equip,
            owner,
            item,
            null,
            equipment.Revision,
            candidate.Revision,
            changes,
            activations);
    }

    internal EquipmentMutationReceipt UnequipCore(EntityId owner, EntityId item)
    {
        EquipmentState equipment = RequireEquipment(owner);
        if (!equipment.ContainsItem(item))
        {
            throw new MechanicsException($"Unique item {item.Value} is not equipped by {owner.Value}.");
        }

        EquipmentState candidate = equipment.Clone();
        foreach (EquipmentAssignment assignment in equipment.Assignments.Where(value => value.Item == item))
        {
            candidate.Remove(assignment.Slot);
        }

        IReadOnlyList<EquipmentSourceActivation> activations = ValidateEquipment(owner, candidate, out _);
        IReadOnlyList<EquipmentSlotChange> changes = Changes(equipment, candidate);
        candidate.SetRevision(checked(candidate.Revision + 1));
        _equipment[owner] = candidate;
        TouchWorld();
        return new EquipmentMutationReceipt(
            EquipmentMutationKind.Unequip,
            owner,
            item,
            null,
            equipment.Revision,
            candidate.Revision,
            changes,
            activations);
    }

    internal EquipmentMutationReceipt SwapCore(
        EntityId owner,
        EntityId outgoingItem,
        EntityId incomingItem,
        IEnumerable<EquipmentSlotDefinition> slots)
    {
        ArgumentNullException.ThrowIfNull(slots);
        if (outgoingItem == incomingItem)
        {
            throw new MechanicsException("An equipment swap requires distinct items.");
        }

        EquipmentState equipment = RequireEquipment(owner);
        ItemState incoming = RequireItem(incomingItem);
        EnsureContainedBy(outgoingItem, owner);
        EnsureContainedBy(incomingItem, owner);
        if (!equipment.ContainsItem(outgoingItem))
        {
            throw new MechanicsException($"Unique item {outgoingItem.Value} is not equipped by {owner.Value}.");
        }
        if (equipment.ContainsItem(incomingItem))
        {
            throw new MechanicsException($"Unique item {incomingItem.Value} is already equipped by {owner.Value}.");
        }

        EquipmentSlotDefinition[] requested = CopyRequestedSlots(slots);
        EnsureRequestedSlotCount(incoming.Definition, requested);
        EquipmentState candidate = equipment.Clone();
        foreach (EquipmentAssignment assignment in equipment.Assignments.Where(value => value.Item == outgoingItem))
        {
            candidate.Remove(assignment.Slot);
        }
        foreach (EquipmentSlotDefinition slot in requested)
        {
            if (candidate.TryGet(slot.Id, out EquipmentState.EquipmentSlotEntry? occupied)
                && occupied is not null)
            {
                throw new MechanicsException(
                    $"Equipment slot {slot.Id} is occupied by item {occupied.Item.Value}.");
            }
            candidate.Set(slot, incomingItem);
        }

        IReadOnlyList<EquipmentSourceActivation> activations = ValidateEquipment(owner, candidate, out _);
        IReadOnlyList<EquipmentSlotChange> changes = Changes(equipment, candidate);
        candidate.SetRevision(checked(candidate.Revision + 1));
        _equipment[owner] = candidate;
        TouchWorld();
        return new EquipmentMutationReceipt(
            EquipmentMutationKind.Swap,
            owner,
            incomingItem,
            outgoingItem,
            equipment.Revision,
            candidate.Revision,
            changes,
            activations);
    }

    private IReadOnlyList<EquipmentSourceActivation> ValidateEquipment(
        EntityId owner,
        EquipmentState equipment,
        out IReadOnlyList<EntityId> observedItems)
    {
        if (equipment.Assignments.Count > ManagedInventoryLimits.MaximumEquipmentAssignments)
        {
            throw new MechanicsException(
                $"Equipment cannot contain more than {ManagedInventoryLimits.MaximumEquipmentAssignments} assignments.");
        }

        Dictionary<EntityId, List<EquipmentState.EquipmentSlotEntry>> byItem = [];
        foreach (EquipmentState.EquipmentSlotEntry assignment in equipment.Entries())
        {
            if (!byItem.TryGetValue(assignment.Item, out List<EquipmentState.EquipmentSlotEntry>? itemSlots))
            {
                itemSlots = [];
                byItem.Add(assignment.Item, itemSlots);
            }
            itemSlots.Add(assignment);
        }

        Dictionary<EquipmentExclusivityId, EntityId> exclusiveItems = [];
        List<EntityId> itemIds = byItem.Keys.OrderBy(value => value).ToList();
        List<EquipmentSourceActivation> activations = [];
        foreach (EntityId item in itemIds)
        {
            ItemState itemState = RequireItem(item);
            EnsureContainedBy(item, owner);
            ItemEquipmentPolicy? policy = itemState.Definition.Equipment
                ?? throw new MechanicsException($"Item {item.Value} is not equippable.");
            List<EquipmentState.EquipmentSlotEntry> assignments = byItem[item];
            if (assignments.Count != policy.RequiredSlots)
            {
                throw new MechanicsException(
                    $"Item {item.Value} requires {policy.RequiredSlots} equipment slots but has {assignments.Count}.");
            }

            foreach (EquipmentState.EquipmentSlotEntry assignment in assignments)
            {
                if (assignment.Slot.AllowedClassifications.Count != 0
                    && !assignment.Slot.AllowedClassifications.Any(
                        classification => itemState.Definition.Classifications.Contains(classification)))
                {
                    throw new MechanicsException(
                        $"Item {item.Value} does not match equipment slot {assignment.Slot.Id} classifications.");
                }
            }

            if (policy.ExclusiveGroup is EquipmentExclusivityId exclusiveGroup)
            {
                if (exclusiveItems.TryGetValue(exclusiveGroup, out EntityId existing)
                    && existing != item)
                {
                    throw new MechanicsException(
                        $"Equipment exclusivity group {exclusiveGroup} already contains item {existing.Value}.");
                }
                exclusiveItems[exclusiveGroup] = item;
            }

            foreach (SourceDefinitionId source in itemState.Definition.SourceDefinitions)
            {
                activations.Add(new EquipmentSourceActivation(
                    new EquippedItemSourceIdentity(owner, item, source),
                    source));
            }
        }

        if (activations.Count > ManagedInventoryLimits.MaximumEquipmentSourceActivations)
        {
            throw new MechanicsException(
                $"Equipment source activations cannot exceed {ManagedInventoryLimits.MaximumEquipmentSourceActivations}.");
        }

        observedItems = itemIds;
        return Array.AsReadOnly(activations.ToArray());
    }

    private static EquipmentSlotDefinition[] CopyRequestedSlots(
        IEnumerable<EquipmentSlotDefinition> slots)
    {
        EquipmentSlotDefinition[] requested = slots
            .Select(slot => slot ?? throw new ArgumentException("Equipment slots cannot be null.", nameof(slots)))
            .ToArray();
        if (requested.Length > ManagedInventoryLimits.MaximumEquipmentAssignments)
        {
            throw new MechanicsException(
                $"An equipment request cannot contain more than {ManagedInventoryLimits.MaximumEquipmentAssignments} slots.");
        }
        if (requested.Select(slot => slot.Id).Distinct().Count() != requested.Length)
        {
            throw new MechanicsException("An equipment request cannot repeat a slot.");
        }
        return requested;
    }

    private static void EnsureRequestedSlotCount(
        ItemDefinition definition,
        IReadOnlyList<EquipmentSlotDefinition> slots)
    {
        ItemEquipmentPolicy policy = definition.Equipment
            ?? throw new MechanicsException($"Item {definition.Id} is not equippable.");
        if (slots.Count != policy.RequiredSlots)
        {
            throw new MechanicsException(
                $"Item {definition.Id} requires {policy.RequiredSlots} equipment slots but requested {slots.Count}.");
        }
    }

    private void EnsureContainedBy(EntityId item, EntityId owner)
    {
        RequireItem(item);
        if (!_containment.TryGetValue(item, out EntityId actual) || actual != owner)
        {
            throw new MechanicsException(
                $"Unique item {item.Value} is not contained by inventory owner {owner.Value}.");
        }
    }

    private static IReadOnlyList<EquipmentSlotChange> Changes(
        EquipmentState before,
        EquipmentState after)
    {
        Dictionary<EquipmentSlotId, EntityId> oldValues = before.Assignments
            .ToDictionary(value => value.Slot, value => value.Item);
        Dictionary<EquipmentSlotId, EntityId> newValues = after.Assignments
            .ToDictionary(value => value.Slot, value => value.Item);
        EquipmentSlotId[] slots = oldValues.Keys
            .Concat(newValues.Keys)
            .Distinct()
            .OrderBy(value => value.Value, StringComparer.Ordinal)
            .ToArray();
        return Array.AsReadOnly(slots
            .Where(slot => !oldValues.TryGetValue(slot, out EntityId oldItem)
                || !newValues.TryGetValue(slot, out EntityId newItem)
                || oldItem != newItem)
            .Select(slot => new EquipmentSlotChange(
                slot,
                oldValues.TryGetValue(slot, out EntityId oldItem) ? oldItem : null,
                newValues.TryGetValue(slot, out EntityId newItem) ? newItem : null))
            .ToArray());
    }
}

public static class EquipmentService
{
    public static EquipmentMutationReceipt Equip(
        InventoryWorld world,
        EntityId owner,
        EntityId item,
        IEnumerable<EquipmentSlotDefinition> slots)
    {
        ArgumentNullException.ThrowIfNull(world);
        InventoryWorldCandidate candidate = world.Prepare();
        EquipmentMutationReceipt receipt = candidate.Equip(owner, item, slots);
        candidate.Publish();
        return receipt;
    }

    public static EquipmentMutationReceipt Unequip(
        InventoryWorld world,
        EntityId owner,
        EntityId item)
    {
        ArgumentNullException.ThrowIfNull(world);
        InventoryWorldCandidate candidate = world.Prepare();
        EquipmentMutationReceipt receipt = candidate.Unequip(owner, item);
        candidate.Publish();
        return receipt;
    }

    public static EquipmentMutationReceipt Swap(
        InventoryWorld world,
        EntityId owner,
        EntityId outgoingItem,
        EntityId incomingItem,
        IEnumerable<EquipmentSlotDefinition> slots)
    {
        ArgumentNullException.ThrowIfNull(world);
        InventoryWorldCandidate candidate = world.Prepare();
        EquipmentMutationReceipt receipt = candidate.Swap(owner, outgoingItem, incomingItem, slots);
        candidate.Publish();
        return receipt;
    }
}
