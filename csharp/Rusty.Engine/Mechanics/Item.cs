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
        ulong worldRevisionBefore,
        ulong worldRevisionAfter,
        IReadOnlyList<CapacityUsage> capacityBefore,
        IReadOnlyList<CapacityUsage> capacityAfter)
    {
        Item = item;
        Definition = definition;
        Container = container;
        WorldRevisionBefore = worldRevisionBefore;
        WorldRevisionAfter = worldRevisionAfter;
        CapacityBefore = capacityBefore;
        CapacityAfter = capacityAfter;
    }

    public EntityId Item { get; }

    public ItemDefinitionId Definition { get; }

    public EntityId Container { get; }

    public ulong WorldRevisionBefore { get; }

    public ulong WorldRevisionAfter { get; }

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
        ulong worldRevisionBefore,
        ulong worldRevisionAfter,
        IReadOnlyList<CapacityUsage> fromCapacityBefore,
        IReadOnlyList<CapacityUsage> fromCapacityAfter,
        IReadOnlyList<CapacityUsage> toCapacityBefore,
        IReadOnlyList<CapacityUsage> toCapacityAfter)
    {
        Item = item;
        Definition = definition;
        FromOwner = fromOwner;
        ToOwner = toOwner;
        WorldRevisionBefore = worldRevisionBefore;
        WorldRevisionAfter = worldRevisionAfter;
        FromCapacityBefore = fromCapacityBefore;
        FromCapacityAfter = fromCapacityAfter;
        ToCapacityBefore = toCapacityBefore;
        ToCapacityAfter = toCapacityAfter;
    }

    public EntityId Item { get; }

    public ItemDefinitionId Definition { get; }

    public EntityId FromOwner { get; }

    public EntityId ToOwner { get; }

    public ulong WorldRevisionBefore { get; }

    public ulong WorldRevisionAfter { get; }

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
        ulong worldRevisionBefore,
        ulong worldRevisionAfter)
    {
        Item = item;
        Definition = definition;
        FormerOwner = formerOwner;
        WorldRevisionBefore = worldRevisionBefore;
        WorldRevisionAfter = worldRevisionAfter;
    }

    public EntityId Item { get; }

    public ItemDefinitionId Definition { get; }

    public EntityId? FormerOwner { get; }

    public ulong WorldRevisionBefore { get; }

    public ulong WorldRevisionAfter { get; }
}

public sealed partial class InventoryWorld
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
        EnsureContainmentQuota(
            _containedChildren.TryGetValue(owner, out SortedSet<EntityId>? children) ? children : null,
            owner,
            adding: true);

        ulong worldRevisionBefore = _revision;
        IReadOnlyList<CapacityUsage> before = ComputeCapacity(owner, inventory);
        IReadOnlyList<CapacityUsage> after = ComputeCapacity(owner, inventory, includedItem: item.Entity, includedState: item);

        _items.Add(item.Entity, item);
        SetContainment(item.Entity, owner);
        InventoryState updated = inventory.Clone();
        updated.SetRevision(checked(updated.Revision + 1));
        _inventories[owner] = updated;
        TouchWorld();
        return new ItemMaterializationReceipt(
            item.Entity,
            item.Definition.Id,
            owner,
            worldRevisionBefore,
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
        EnsureContainmentQuota(
            _containedChildren.TryGetValue(toOwner, out SortedSet<EntityId>? children) ? children : null,
            toOwner,
            adding: true);

        ulong worldRevisionBefore = _revision;
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
        TouchWorld();
        return new ItemTransferReceipt(
            item,
            itemState.Definition.Id,
            fromOwner,
            toOwner,
            worldRevisionBefore,
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

        ulong worldRevisionBefore = _revision;
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
        TouchWorld();
        return new ItemDestroyReceipt(
            item,
            itemState.Definition.Id,
            formerOwner,
            worldRevisionBefore,
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

/// <summary>Convenience entry points for unique-item lifecycle operations.</summary>
public static class ItemService
{
    public static ItemMaterializationReceipt MaterializeUnique(
        InventoryWorld world,
        ItemState item,
        EntityId owner)
    {
        ArgumentNullException.ThrowIfNull(world);
        return world.MaterializeUnique(item, owner);
    }

    public static ItemTransferReceipt TransferUnique(
        InventoryWorld world,
        EntityId item,
        EntityId fromOwner,
        EntityId toOwner)
    {
        ArgumentNullException.ThrowIfNull(world);
        return world.TransferUnique(item, fromOwner, toOwner);
    }

    public static ItemDestroyReceipt DestroyUnique(InventoryWorld world, EntityId item)
    {
        ArgumentNullException.ThrowIfNull(world);
        return world.DestroyUnique(item);
    }
}
