using Rusty.Engine.Entities;

namespace Rusty.Engine.Mechanics;

/// <summary>
/// Product-owned state for one unique item entity. Product-specific fields
/// remain in the product and can be keyed by <see cref="Entity"/>.
/// </summary>
public sealed class ItemState
{
    public ItemState(EntityId entity, ItemDefinition definition)
    {
        if (entity.Value == 0)
        {
            throw new ArgumentOutOfRangeException(nameof(entity), "Unique item entities must be non-zero.");
        }
        ArgumentNullException.ThrowIfNull(definition);
        if (definition.Kind != ItemKind.Unique)
        {
            throw new MechanicsException($"Item {definition.Id} is not a unique item definition.");
        }

        Entity = entity;
        Definition = definition;
    }

    public EntityId Entity { get; }

    public ItemDefinition Definition { get; }
}

/// <summary>Evidence for one atomically materialized unique item.</summary>
public sealed class ItemMaterializationReceipt
{
    internal ItemMaterializationReceipt(
        EntityId item,
        ItemDefinitionId definition,
        EntityId container,
        ulong inventoryRevisionBefore,
        ulong inventoryRevisionAfter,
        IReadOnlyList<CapacityUsage> capacityBefore,
        IReadOnlyList<CapacityUsage> capacityAfter)
    {
        Item = item;
        Definition = definition;
        Container = container;
        InventoryRevisionBefore = inventoryRevisionBefore;
        InventoryRevisionAfter = inventoryRevisionAfter;
        CapacityBefore = capacityBefore;
        CapacityAfter = capacityAfter;
    }

    public EntityId Item { get; }

    public ItemDefinitionId Definition { get; }

    public EntityId Container { get; }

    public ulong InventoryRevisionBefore { get; }

    public ulong InventoryRevisionAfter { get; }

    public IReadOnlyList<CapacityUsage> CapacityBefore { get; }

    public IReadOnlyList<CapacityUsage> CapacityAfter { get; }
}

/// <summary>Evidence for one atomic unique-item transfer.</summary>
public sealed class ItemTransferReceipt
{
    internal ItemTransferReceipt(
        EntityId item,
        ItemDefinitionId definition,
        EntityId fromOwner,
        EntityId toOwner,
        ulong inventoryRevisionBefore,
        ulong inventoryRevisionAfter,
        IReadOnlyList<CapacityUsage> fromCapacityBefore,
        IReadOnlyList<CapacityUsage> fromCapacityAfter,
        IReadOnlyList<CapacityUsage> toCapacityBefore,
        IReadOnlyList<CapacityUsage> toCapacityAfter)
    {
        Item = item;
        Definition = definition;
        FromOwner = fromOwner;
        ToOwner = toOwner;
        InventoryRevisionBefore = inventoryRevisionBefore;
        InventoryRevisionAfter = inventoryRevisionAfter;
        FromCapacityBefore = fromCapacityBefore;
        FromCapacityAfter = fromCapacityAfter;
        ToCapacityBefore = toCapacityBefore;
        ToCapacityAfter = toCapacityAfter;
    }

    public EntityId Item { get; }

    public ItemDefinitionId Definition { get; }

    public EntityId FromOwner { get; }

    public EntityId ToOwner { get; }

    public ulong InventoryRevisionBefore { get; }

    public ulong InventoryRevisionAfter { get; }

    public IReadOnlyList<CapacityUsage> FromCapacityBefore { get; }

    public IReadOnlyList<CapacityUsage> FromCapacityAfter { get; }

    public IReadOnlyList<CapacityUsage> ToCapacityBefore { get; }

    public IReadOnlyList<CapacityUsage> ToCapacityAfter { get; }
}

/// <summary>Evidence for one caller-approved unique-item destruction.</summary>
public sealed class ItemDestroyReceipt
{
    internal ItemDestroyReceipt(
        EntityId item,
        ItemDefinitionId definition,
        EntityId? formerOwner,
        ulong inventoryRevisionBefore,
        ulong inventoryRevisionAfter)
    {
        Item = item;
        Definition = definition;
        FormerOwner = formerOwner;
        InventoryRevisionBefore = inventoryRevisionBefore;
        InventoryRevisionAfter = inventoryRevisionAfter;
    }

    public EntityId Item { get; }

    public ItemDefinitionId Definition { get; }

    public EntityId? FormerOwner { get; }

    public ulong InventoryRevisionBefore { get; }

    public ulong InventoryRevisionAfter { get; }
}

public sealed partial class InventoryStore
{
    public ItemMaterializationReceipt MaterializeUnique(ItemState item, EntityId owner) =>
        Commit(candidate => candidate.MaterializeUnique(item, owner));

    public ItemTransferReceipt TransferUnique(EntityId item, EntityId fromOwner, EntityId toOwner) =>
        Commit(candidate => candidate.TransferUnique(item, fromOwner, toOwner));

    public ItemDestroyReceipt DestroyUnique(EntityId item) =>
        Commit(candidate => candidate.DestroyUnique(item));

    internal ItemMaterializationReceipt MaterializeUniqueCore(ItemState item, EntityId owner)
    {
        ArgumentNullException.ThrowIfNull(item);
        InventoryState inventory = RequireInventory(owner);
        if (_items.ContainsKey(item.Entity))
        {
            throw new MechanicsException($"Unique item entity {item.Entity.Value} is already registered.");
        }
        if (_containment.ContainsKey(item.Entity))
        {
            throw new MechanicsException($"Unique item entity {item.Entity.Value} already has a container.");
        }
        ulong inventoryRevisionBefore = _revision;
        IReadOnlyList<CapacityUsage> before = ComputeCapacity(owner, inventory);
        IReadOnlyList<CapacityUsage> after = ComputeCapacity(owner, inventory, includedItem: item.Entity, includedState: item);

        _items.Add(item.Entity, item);
        SetContainment(item.Entity, owner);
        InventoryState updated = inventory.Clone();
        updated.SetRevision(checked(updated.Revision + 1));
        _inventories[owner] = updated;
        TouchStore();
        return new ItemMaterializationReceipt(
            item.Entity,
            item.Definition.Id,
            owner,
            inventoryRevisionBefore,
            _revision,
            before,
            after);
    }

    internal ItemTransferReceipt TransferUniqueCore(
        EntityId item,
        EntityId fromOwner,
        EntityId toOwner)
    {
        if (fromOwner == toOwner)
        {
            throw new MechanicsException("A unique item transfer requires distinct owners.");
        }

        ItemState itemState = RequireItem(item);
        InventoryState fromInventory = RequireInventory(fromOwner);
        InventoryState toInventory = RequireInventory(toOwner);
        if (!_containment.TryGetValue(item, out EntityId actualOwner) || actualOwner != fromOwner)
        {
            string actual = _containment.TryGetValue(item, out EntityId value)
                ? value.Value.ToString()
                : "none";
            throw new MechanicsException(
                $"Unique item {item.Value} is contained by {actual}, not {fromOwner.Value}.");
        }
        if (IsEquipped(fromOwner, item))
        {
            throw new MechanicsException(
                $"Unique item {item.Value} must be unequipped before transfer.");
        }
        ulong inventoryRevisionBefore = _revision;
        IReadOnlyList<CapacityUsage> fromBefore = ComputeCapacity(fromOwner, fromInventory);
        IReadOnlyList<CapacityUsage> toBefore = ComputeCapacity(toOwner, toInventory);
        IReadOnlyList<CapacityUsage> fromAfter = ComputeCapacity(
            fromOwner,
            fromInventory,
            excludedItem: item);
        IReadOnlyList<CapacityUsage> toAfter = ComputeCapacity(
            toOwner,
            toInventory,
            includedItem: item);

        RemoveContainment(item, fromOwner);
        SetContainment(item, toOwner);
        InventoryState updatedFrom = fromInventory.Clone();
        updatedFrom.SetRevision(checked(updatedFrom.Revision + 1));
        InventoryState updatedTo = toInventory.Clone();
        updatedTo.SetRevision(checked(updatedTo.Revision + 1));
        _inventories[fromOwner] = updatedFrom;
        _inventories[toOwner] = updatedTo;
        TouchStore();
        return new ItemTransferReceipt(
            item,
            itemState.Definition.Id,
            fromOwner,
            toOwner,
            inventoryRevisionBefore,
            _revision,
            fromBefore,
            fromAfter,
            toBefore,
            toAfter);
    }

    internal ItemDestroyReceipt DestroyUniqueCore(EntityId item)
    {
        ItemState itemState = RequireItem(item);
        if (IsEquippedAnywhere(item))
        {
            throw new MechanicsException(
                $"Unique item {item.Value} must be unequipped before destruction.");
        }

        ulong inventoryRevisionBefore = _revision;
        EntityId? formerOwner = null;
        if (_containment.TryGetValue(item, out EntityId owner))
        {
            formerOwner = owner;
            RemoveContainment(item, owner);
            InventoryState inventory = RequireInventory(owner).Clone();
            inventory.SetRevision(checked(inventory.Revision + 1));
            _inventories[owner] = inventory;
        }

        _items.Remove(item);
        TouchStore();
        return new ItemDestroyReceipt(
            item,
            itemState.Definition.Id,
            formerOwner,
            inventoryRevisionBefore,
            _revision);
    }

    private void SetContainment(EntityId child, EntityId container)
    {
        if (_containment.ContainsKey(child))
        {
            throw new MechanicsException($"Unique item {child.Value} already has a container.");
        }

        _containment.Add(child, container);
        if (!_containedChildren.TryGetValue(container, out SortedSet<EntityId>? children))
        {
            children = [];
            _containedChildren.Add(container, children);
        }
        children.Add(child);
    }

    private void RemoveContainment(EntityId child, EntityId container)
    {
        _containment.Remove(child);
        if (_containedChildren.TryGetValue(container, out SortedSet<EntityId>? children))
        {
            children.Remove(child);
            if (children.Count == 0)
            {
                _containedChildren.Remove(container);
            }
        }
    }

    private bool IsEquipped(EntityId owner, EntityId item) =>
        _equipment.TryGetValue(owner, out EquipmentState? state)
        && state.ContainsItem(item);

    private bool IsEquippedAnywhere(EntityId item) =>
        _equipment.Values.Any(state => state.ContainsItem(item));
}
