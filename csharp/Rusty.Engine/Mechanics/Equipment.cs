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
    private readonly SortedDictionary<EntityId, EquippedItem> _items = [];

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

    public bool ContainsItem(EntityId item) => _items.ContainsKey(item);

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
        foreach ((EntityId item, EquippedItem equipped) in _items)
        {
            result._items.Add(item, equipped);
        }

        return result;
    }

    /// <summary>Equipped items in entity order, each with its definition and slots.</summary>
    internal IEnumerable<KeyValuePair<EntityId, EquippedItem>> Items() => _items;

    internal bool TryGet(EquipmentSlotId slot, out EquipmentSlotEntry? entry) =>
        _assignments.TryGetValue(slot, out entry);

    internal bool TryGetItem(EntityId item, out EquippedItem? equipped) =>
        _items.TryGetValue(item, out equipped);

    internal void Add(EntityId item, ItemDefinition definition, EquipmentSlotDefinition[] slots)
    {
        foreach (EquipmentSlotDefinition slot in slots)
        {
            _assignments[slot.Id] = new EquipmentSlotEntry(slot, item);
        }
        _items.Add(item, new EquippedItem(definition, Array.ConvertAll(slots, slot => slot.Id)));
    }

    internal EquipmentSlotId[] Remove(EntityId item)
    {
        EquipmentSlotId[] slots = _items[item].Slots;
        foreach (EquipmentSlotId slot in slots)
        {
            _assignments.Remove(slot);
        }
        _items.Remove(item);
        return slots;
    }

    internal void SetRevision(ulong revision) => Revision = revision;

    internal sealed record EquipmentSlotEntry(EquipmentSlotDefinition Slot, EntityId Item);

    internal sealed record EquippedItem(ItemDefinition Definition, EquipmentSlotId[] Slots);
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

public sealed partial class InventoryStore
{
    public EquipmentMutationReceipt Equip(
        EntityId owner,
        EntityId item,
        IEnumerable<EquipmentSlotDefinition> slots) =>
        EquipCore(owner, item, slots);

    public EquipmentMutationReceipt Unequip(EntityId owner, EntityId item) =>
        UnequipCore(owner, item);

    public EquipmentMutationReceipt Swap(
        EntityId owner,
        EntityId outgoingItem,
        EntityId incomingItem,
        IEnumerable<EquipmentSlotDefinition> slots) =>
        SwapCore(owner, outgoingItem, incomingItem, slots);

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
            throw new MechanicsException(MechanicsRefusal.AlreadyPresent, $"Unique item {item.Value} is already equipped by {owner.Value}.");
        }

        EquipmentSlotDefinition[] requested = CopyRequestedSlots(slots);
        EnsureRequestedSlotCount(itemState.Definition, requested);
        EnsureSlotsAccept(equipment, itemState, requested, outgoing: null);
        ulong revisionBefore = equipment.Revision;
        ulong revisionAfter = checked(revisionBefore + 1);
        equipment.Add(item, itemState.Definition, requested);
        equipment.SetRevision(revisionAfter);
        TouchStore();
        return new EquipmentMutationReceipt(
            EquipmentMutationKind.Equip,
            owner,
            item,
            null,
            revisionBefore,
            revisionAfter,
            Changes([], null, Array.ConvertAll(requested, slot => slot.Id), item),
            SourceActivations(owner, equipment));
    }

    internal EquipmentMutationReceipt UnequipCore(EntityId owner, EntityId item)
    {
        EquipmentState equipment = RequireEquipment(owner);
        if (!equipment.ContainsItem(item))
        {
            throw new MechanicsException(MechanicsRefusal.NotFound, $"Unique item {item.Value} is not equipped by {owner.Value}.");
        }

        ulong revisionBefore = equipment.Revision;
        ulong revisionAfter = checked(revisionBefore + 1);
        EquipmentSlotId[] removed = equipment.Remove(item);
        equipment.SetRevision(revisionAfter);
        TouchStore();
        return new EquipmentMutationReceipt(
            EquipmentMutationKind.Unequip,
            owner,
            item,
            null,
            revisionBefore,
            revisionAfter,
            Changes(removed, item, [], null),
            SourceActivations(owner, equipment));
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
            throw new MechanicsException(MechanicsRefusal.InvalidRequest, "An equipment swap requires distinct items.");
        }

        EquipmentState equipment = RequireEquipment(owner);
        ItemState incoming = RequireItem(incomingItem);
        EnsureContainedBy(outgoingItem, owner);
        EnsureContainedBy(incomingItem, owner);
        if (!equipment.ContainsItem(outgoingItem))
        {
            throw new MechanicsException(MechanicsRefusal.NotFound, $"Unique item {outgoingItem.Value} is not equipped by {owner.Value}.");
        }
        if (equipment.ContainsItem(incomingItem))
        {
            throw new MechanicsException(MechanicsRefusal.AlreadyPresent, $"Unique item {incomingItem.Value} is already equipped by {owner.Value}.");
        }

        EquipmentSlotDefinition[] requested = CopyRequestedSlots(slots);
        EnsureRequestedSlotCount(incoming.Definition, requested);
        EnsureSlotsAccept(equipment, incoming, requested, outgoingItem);
        ulong revisionBefore = equipment.Revision;
        ulong revisionAfter = checked(revisionBefore + 1);
        EquipmentSlotId[] removed = equipment.Remove(outgoingItem);
        equipment.Add(incomingItem, incoming.Definition, requested);
        equipment.SetRevision(revisionAfter);
        TouchStore();
        return new EquipmentMutationReceipt(
            EquipmentMutationKind.Swap,
            owner,
            incomingItem,
            outgoingItem,
            revisionBefore,
            revisionAfter,
            Changes(removed, outgoingItem, Array.ConvertAll(requested, slot => slot.Id), incomingItem),
            SourceActivations(owner, equipment));
    }

    /// <summary>
    /// Admits an incoming item to its requested slots: each must be free (or held by the outgoing
    /// item of a swap) and accept the item's classifications, and no other equipped item may hold
    /// its exclusivity group. Items already equipped were admitted when they were equipped.
    /// </summary>
    private static void EnsureSlotsAccept(
        EquipmentState equipment,
        ItemState incoming,
        EquipmentSlotDefinition[] requested,
        EntityId? outgoing)
    {
        foreach (EquipmentSlotDefinition slot in requested)
        {
            if (equipment.TryGet(slot.Id, out EquipmentState.EquipmentSlotEntry? occupied)
                && occupied is not null
                && occupied.Item != outgoing)
            {
                throw new MechanicsException(
                    MechanicsRefusal.Occupied,
                    $"Equipment slot {slot.Id} is occupied by item {occupied.Item.Value}.");
            }
        }

        foreach (EquipmentSlotDefinition slot in requested)
        {
            if (slot.AllowedClassifications.Count != 0
                && !slot.AllowedClassifications.Any(
                    classification => incoming.Definition.Classifications.Contains(classification)))
            {
                throw new MechanicsException(
                    MechanicsRefusal.Incompatible,
                    $"Item {incoming.Entity.Value} does not match equipment slot {slot.Id} classifications.");
            }
        }

        if (incoming.Definition.Equipment?.ExclusiveGroup is EquipmentExclusivityId exclusiveGroup)
        {
            foreach ((EntityId item, EquipmentState.EquippedItem equipped) in equipment.Items())
            {
                if (item != outgoing && equipped.Definition.Equipment?.ExclusiveGroup == exclusiveGroup)
                {
                    throw new MechanicsException(
                        MechanicsRefusal.Occupied,
                        $"Equipment exclusivity group {exclusiveGroup} already contains item {item.Value}.");
                }
            }
        }
    }

    /// <summary>Every source of every item the owner has equipped, in item then definition order.</summary>
    private static IReadOnlyList<EquipmentSourceActivation> SourceActivations(EntityId owner, EquipmentState equipment)
    {
        List<EquipmentSourceActivation> activations = [];
        foreach ((EntityId item, EquipmentState.EquippedItem equipped) in equipment.Items())
        {
            foreach (SourceDefinitionId source in equipped.Definition.SourceDefinitions)
            {
                activations.Add(new EquipmentSourceActivation(
                    new EquippedItemSourceIdentity(owner, item, source),
                    source));
            }
        }

        return activations.AsReadOnly();
    }

    private static EquipmentSlotDefinition[] CopyRequestedSlots(
        IEnumerable<EquipmentSlotDefinition> slots)
    {
        EquipmentSlotDefinition[] requested = slots
            .Select(slot => slot ?? throw new ArgumentException("Equipment slots cannot be null.", nameof(slots)))
            .ToArray();
        if (requested.Select(slot => slot.Id).Distinct().Count() != requested.Length)
        {
            throw new MechanicsException(MechanicsRefusal.InvalidRequest, "An equipment request cannot repeat a slot.");
        }
        return requested;
    }

    private static void EnsureRequestedSlotCount(
        ItemDefinition definition,
        IReadOnlyList<EquipmentSlotDefinition> slots)
    {
        ItemEquipmentPolicy policy = definition.Equipment
            ?? throw new MechanicsException(MechanicsRefusal.Incompatible, $"Item {definition.Id} is not equippable.");
        if (slots.Count != policy.RequiredSlots)
        {
            throw new MechanicsException(
                MechanicsRefusal.Incompatible,
                $"Item {definition.Id} requires {policy.RequiredSlots} equipment slots but requested {slots.Count}.");
        }
    }

    private void EnsureContainedBy(EntityId item, EntityId owner)
    {
        RequireItem(item);
        if (!_containment.TryGetValue(item, out EntityId actual) || actual != owner)
        {
            throw new MechanicsException(
                MechanicsRefusal.NotFound,
                $"Unique item {item.Value} is not contained by inventory owner {owner.Value}.");
        }
    }

    /// <summary>The slots one mutation touched, in slot order: those the outgoing item left and the incoming item took.</summary>
    private static IReadOnlyList<EquipmentSlotChange> Changes(
        EquipmentSlotId[] vacated,
        EntityId? outgoing,
        EquipmentSlotId[] taken,
        EntityId? incoming)
    {
        var changes = new List<EquipmentSlotChange>(vacated.Length + taken.Length);
        foreach (EquipmentSlotId slot in vacated)
        {
            changes.Add(new EquipmentSlotChange(slot, outgoing, Array.IndexOf(taken, slot) >= 0 ? incoming : null));
        }
        foreach (EquipmentSlotId slot in taken)
        {
            if (Array.IndexOf(vacated, slot) < 0)
            {
                changes.Add(new EquipmentSlotChange(slot, null, incoming));
            }
        }

        changes.Sort((left, right) => string.CompareOrdinal(left.Slot.Value, right.Slot.Value));
        return changes.AsReadOnly();
    }
}
