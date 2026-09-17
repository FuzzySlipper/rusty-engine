using Rusty.Engine.Entities;

namespace Rusty.Engine.Mechanics;

/// <summary>One positive quantity of a fungible item definition.</summary>
public readonly record struct InventoryStack
{
    public InventoryStack(ItemDefinitionId definition, ulong quantity)
    {
        Definition = definition ?? throw new ArgumentNullException(nameof(definition));
        if (quantity == 0 || quantity > ManagedInventoryLimits.MaximumStackQuantity)
        {
            throw new ArgumentOutOfRangeException(nameof(quantity));
        }

        Quantity = quantity;
    }

    public ItemDefinitionId Definition { get; }

    public ulong Quantity { get; }
}

/// <summary>One typed maximum for an inventory capacity dimension.</summary>
public readonly record struct InventoryCapacityLimit
{
    public InventoryCapacityLimit(CapacityMetricId metric, ulong maximum)
    {
        Metric = metric ?? throw new ArgumentNullException(nameof(metric));
        if (maximum > ManagedInventoryLimits.MaximumCapacityUnits)
        {
            throw new ArgumentOutOfRangeException(
                nameof(maximum),
                $"Capacity limits cannot exceed {ManagedInventoryLimits.MaximumCapacityUnits} units.");
        }

        Maximum = maximum;
    }

    public CapacityMetricId Metric { get; }

    public ulong Maximum { get; }
}

/// <summary>Computed usage for one capacity dimension.</summary>
public readonly record struct CapacityUsage(
    CapacityMetricId Metric,
    ulong Used,
    ulong? Maximum);

/// <summary>One unique item entity directly contained by an inventory owner.</summary>
public readonly record struct UniqueInventoryItem(EntityId Entity, ItemDefinitionId Definition);

/// <summary>Copied read model for one inventory owner.</summary>
public sealed class InventoryView
{
    internal InventoryView(
        EntityId owner,
        ulong storeRevision,
        ulong inventoryRevision,
        IReadOnlyList<InventoryStack> stacks,
        IReadOnlyList<UniqueInventoryItem> uniqueItems,
        IReadOnlyList<CapacityUsage> capacity)
    {
        Owner = owner;
        StoreRevision = storeRevision;
        InventoryRevision = inventoryRevision;
        Stacks = stacks;
        UniqueItems = uniqueItems;
        Capacity = capacity;
    }

    public EntityId Owner { get; }

    public ulong StoreRevision { get; }

    public ulong InventoryRevision { get; }

    public IReadOnlyList<InventoryStack> Stacks { get; }

    public IReadOnlyList<UniqueInventoryItem> UniqueItems { get; }

    public IReadOnlyList<CapacityUsage> Capacity { get; }
}

/// <summary>Kind of one fungible stack mutation.</summary>
public enum InventoryMutationKind
{
    Grant,
    Consume,
}

/// <summary>Evidence for one committed fungible stack mutation.</summary>
public sealed class InventoryMutationReceipt
{
    internal InventoryMutationReceipt(
        InventoryMutationKind kind,
        EntityId owner,
        ItemDefinitionId item,
        ulong requestedQuantity,
        ulong beforeQuantity,
        ulong afterQuantity,
        ulong inventoryRevisionBefore,
        ulong inventoryRevisionAfter,
        IReadOnlyList<CapacityUsage> capacityBefore,
        IReadOnlyList<CapacityUsage> capacityAfter)
    {
        Kind = kind;
        Owner = owner;
        Item = item;
        RequestedQuantity = requestedQuantity;
        BeforeQuantity = beforeQuantity;
        AfterQuantity = afterQuantity;
        InventoryRevisionBefore = inventoryRevisionBefore;
        InventoryRevisionAfter = inventoryRevisionAfter;
        CapacityBefore = capacityBefore;
        CapacityAfter = capacityAfter;
    }

    public InventoryMutationKind Kind { get; }

    public EntityId Owner { get; }

    public ItemDefinitionId Item { get; }

    public ulong RequestedQuantity { get; }

    public ulong BeforeQuantity { get; }

    public ulong AfterQuantity { get; }

    public ulong InventoryRevisionBefore { get; }

    public ulong InventoryRevisionAfter { get; }

    public IReadOnlyList<CapacityUsage> CapacityBefore { get; }

    public IReadOnlyList<CapacityUsage> CapacityAfter { get; }
}

/// <summary>Evidence for one atomic fungible stack transfer.</summary>
public sealed class InventoryTransferReceipt
{
    internal InventoryTransferReceipt(
        EntityId fromOwner,
        EntityId toOwner,
        ItemDefinitionId item,
        ulong quantity,
        ulong fromBefore,
        ulong fromAfter,
        ulong toBefore,
        ulong toAfter,
        ulong fromInventoryRevisionBefore,
        ulong fromInventoryRevisionAfter,
        ulong toInventoryRevisionBefore,
        ulong toInventoryRevisionAfter,
        IReadOnlyList<CapacityUsage> fromCapacityBefore,
        IReadOnlyList<CapacityUsage> fromCapacityAfter,
        IReadOnlyList<CapacityUsage> toCapacityBefore,
        IReadOnlyList<CapacityUsage> toCapacityAfter)
    {
        FromOwner = fromOwner;
        ToOwner = toOwner;
        Item = item;
        Quantity = quantity;
        FromBefore = fromBefore;
        FromAfter = fromAfter;
        ToBefore = toBefore;
        ToAfter = toAfter;
        FromInventoryRevisionBefore = fromInventoryRevisionBefore;
        FromInventoryRevisionAfter = fromInventoryRevisionAfter;
        ToInventoryRevisionBefore = toInventoryRevisionBefore;
        ToInventoryRevisionAfter = toInventoryRevisionAfter;
        FromCapacityBefore = fromCapacityBefore;
        FromCapacityAfter = fromCapacityAfter;
        ToCapacityBefore = toCapacityBefore;
        ToCapacityAfter = toCapacityAfter;
    }

    public EntityId FromOwner { get; }

    public EntityId ToOwner { get; }

    public ItemDefinitionId Item { get; }

    public ulong Quantity { get; }

    public ulong FromBefore { get; }

    public ulong FromAfter { get; }

    public ulong ToBefore { get; }

    public ulong ToAfter { get; }

    public ulong FromInventoryRevisionBefore { get; }

    public ulong FromInventoryRevisionAfter { get; }

    public ulong ToInventoryRevisionBefore { get; }

    public ulong ToInventoryRevisionAfter { get; }

    public IReadOnlyList<CapacityUsage> FromCapacityBefore { get; }

    public IReadOnlyList<CapacityUsage> FromCapacityAfter { get; }

    public IReadOnlyList<CapacityUsage> ToCapacityBefore { get; }

    public IReadOnlyList<CapacityUsage> ToCapacityAfter { get; }
}

/// <summary>
/// Product-owned fungible inventory state. It is useful on its own when a
/// product does not need the composed <see cref="InventoryStore"/> helper.
/// </summary>
public sealed class InventoryState
{
    private readonly Dictionary<ItemDefinitionId, InventoryStackEntry> _stacks = [];
    private readonly Dictionary<CapacityMetricId, InventoryCapacityLimit> _capacityLimits = [];

    public InventoryState(
        EntityId owner,
        IEnumerable<InventoryCapacityLimit>? capacityLimits = null)
    {
        Owner = owner;
        if (capacityLimits is null)
        {
            return;
        }

        foreach (InventoryCapacityLimit limit in capacityLimits)
        {
            if (!_capacityLimits.TryAdd(limit.Metric, limit))
            {
                throw new ArgumentException(
                    $"Capacity metric {limit.Metric} was declared more than once.",
                    nameof(capacityLimits));
            }
        }

        if (_capacityLimits.Count > ManagedInventoryLimits.MaximumCapacityLimitsPerInventory)
        {
            throw new ArgumentException(
                $"An inventory cannot have more than {ManagedInventoryLimits.MaximumCapacityLimitsPerInventory} capacity limits.",
                nameof(capacityLimits));
        }
    }

    public EntityId Owner { get; }

    public ulong Revision { get; private set; }

    public IReadOnlyList<InventoryStack> Stacks => _stacks.Values
        .OrderBy(entry => entry.Definition.Id.Value, StringComparer.Ordinal)
        .Select(entry => new InventoryStack(entry.Definition.Id, entry.Quantity))
        .ToArray();

    public IReadOnlyList<InventoryCapacityLimit> CapacityLimits => _capacityLimits.Values
        .OrderBy(limit => limit.Metric.Value, StringComparer.Ordinal)
        .ToArray();

    public bool TryGetQuantity(ItemDefinitionId definition, out ulong quantity)
    {
        ArgumentNullException.ThrowIfNull(definition);
        if (_stacks.TryGetValue(definition, out InventoryStackEntry? entry))
        {
            quantity = entry.Quantity;
            return true;
        }

        quantity = 0;
        return false;
    }

    /// <summary>Adds or replaces a local capacity limit and advances this standalone state revision.</summary>
    public void SetCapacityLimit(InventoryCapacityLimit limit)
    {
        if (!_capacityLimits.ContainsKey(limit.Metric)
            && _capacityLimits.Count >= ManagedInventoryLimits.MaximumCapacityLimitsPerInventory)
        {
            throw new MechanicsException(
                $"An inventory cannot have more than {ManagedInventoryLimits.MaximumCapacityLimitsPerInventory} capacity limits.");
        }

        _capacityLimits[limit.Metric] = limit;
        Revision = checked(Revision + 1);
    }

    internal InventoryState Clone()
    {
        var result = new InventoryState(Owner, _capacityLimits.Values)
        {
            Revision = Revision,
        };
        foreach ((ItemDefinitionId id, InventoryStackEntry entry) in _stacks)
        {
            result._stacks.Add(id, entry);
        }

        return result;
    }

    internal bool TryGetEntry(ItemDefinitionId definition, out InventoryStackEntry? entry) =>
        _stacks.TryGetValue(definition, out entry);

    internal IEnumerable<InventoryStackEntry> Entries() => _stacks.Values;

    internal void SetEntry(ItemDefinition definition, ulong quantity)
    {
        if (quantity == 0)
        {
            _stacks.Remove(definition.Id);
            return;
        }

        if (!_stacks.ContainsKey(definition.Id)
            && _stacks.Count >= ManagedInventoryLimits.MaximumStacksPerInventory)
        {
            throw new MechanicsException(
                $"An inventory cannot contain more than {ManagedInventoryLimits.MaximumStacksPerInventory} fungible stacks.");
        }

        _stacks[definition.Id] = new InventoryStackEntry(definition, quantity);
    }

    internal void RemoveEntry(ItemDefinitionId definition) => _stacks.Remove(definition);

    internal void SetRevision(ulong revision) => Revision = revision;

    internal sealed record InventoryStackEntry(ItemDefinition Definition, ulong Quantity);
}

/// <summary>
/// Composed product-owned inventory/item/equipment state. Definitions and
/// item meaning remain caller-owned; this helper only maintains the mechanical
/// relationships needed by the selected operations.
/// </summary>
public sealed partial class InventoryStore
{
    private readonly Dictionary<EntityId, InventoryState> _inventories = [];
    private readonly Dictionary<EntityId, ItemState> _items = [];
    private readonly Dictionary<EntityId, EquipmentState> _equipment = [];
    private readonly Dictionary<EntityId, EntityId> _containment = [];
    private readonly Dictionary<EntityId, SortedSet<EntityId>> _containedChildren = [];
    private ulong _revision;

    public ulong Revision => _revision;

    public IReadOnlyList<EntityId> InventoryOwners => _inventories.Keys.OrderBy(value => value).ToArray();

    public IReadOnlyList<EntityId> ItemEntities => _items.Keys.OrderBy(value => value).ToArray();

    public IReadOnlyList<EntityId> EquipmentOwners => _equipment.Keys.OrderBy(value => value).ToArray();

    public void RegisterInventory(InventoryState state)
    {
        ArgumentNullException.ThrowIfNull(state);
        if (!_inventories.TryAdd(state.Owner, state.Clone()))
        {
            throw new MechanicsException($"Inventory owner {state.Owner.Value} is already registered.");
        }

        TouchStore();
    }

    public void RegisterEquipment(EquipmentState state)
    {
        ArgumentNullException.ThrowIfNull(state);
        if (!_inventories.ContainsKey(state.Owner))
        {
            throw new MechanicsException($"Inventory owner {state.Owner.Value} must be registered before equipment.");
        }
        if (!_equipment.TryAdd(state.Owner, state.Clone()))
        {
            throw new MechanicsException($"Equipment owner {state.Owner.Value} is already registered.");
        }

        TouchStore();
    }

    public bool TryGetInventory(EntityId owner, out InventoryState? state)
    {
        if (_inventories.TryGetValue(owner, out InventoryState? value))
        {
            state = value.Clone();
            return true;
        }

        state = null;
        return false;
    }

    public bool TryGetItem(EntityId item, out ItemState? state)
    {
        if (_items.TryGetValue(item, out ItemState? value))
        {
            state = value;
            return true;
        }

        state = null;
        return false;
    }

    public bool TryGetEquipment(EntityId owner, out EquipmentState? state)
    {
        if (_equipment.TryGetValue(owner, out EquipmentState? value))
        {
            state = value.Clone();
            return true;
        }

        state = null;
        return false;
    }

    public bool TryGetContainer(EntityId child, out EntityId container) =>
        _containment.TryGetValue(child, out container);

    public IReadOnlyList<EntityId> ContainedEntities(EntityId container) =>
        _containedChildren.TryGetValue(container, out SortedSet<EntityId>? children)
            ? children.ToArray()
            : Array.Empty<EntityId>();

    public InventoryView View(EntityId owner)
    {
        InventoryState inventory = RequireInventory(owner);
        return BuildView(owner, inventory, ComputeCapacity(owner, inventory));
    }

    public InventoryEdit Prepare(ulong? expectedRevision = null)
    {
        if (expectedRevision is ulong expected && expected != _revision)
        {
            throw new MechanicsException(
                $"Inventory store revision is stale: expected {expected}, actual {_revision}.");
        }

        return new InventoryEdit(this, Clone(), _revision);
    }

    public InventoryMutationReceipt Grant(EntityId owner, ItemDefinition definition, ulong quantity) =>
        Commit(candidate => candidate.Grant(owner, definition, quantity));

    public InventoryMutationReceipt Consume(EntityId owner, ItemDefinition definition, ulong quantity) =>
        Commit(candidate => candidate.Consume(owner, definition, quantity));

    public InventoryTransferReceipt TransferFungible(
        EntityId fromOwner,
        EntityId toOwner,
        ItemDefinition definition,
        ulong quantity) =>
        Commit(candidate => candidate.TransferFungible(fromOwner, toOwner, definition, quantity));

    public InventoryView Read(EntityId owner) => View(owner);

    internal InventoryMutationReceipt GrantCore(
        EntityId owner,
        ItemDefinition definition,
        ulong quantity)
    {
        ArgumentNullException.ThrowIfNull(definition);
        EnsureFungible(definition);
        EnsurePositiveQuantity(quantity);
        InventoryState inventory = RequireInventory(owner);
        EnsureNoDefinitionConflict(inventory, definition);
        ulong before = inventory.TryGetQuantity(definition.Id, out ulong current) ? current : 0;
        ulong after;
        try
        {
            after = checked(before + quantity);
        }
        catch (OverflowException)
        {
            throw new MechanicsException($"Quantity for item {definition.Id} overflowed.");
        }
        if (after > definition.MaximumQuantity)
        {
            throw new MechanicsException(
                $"Item {definition.Id} cannot exceed quantity {definition.MaximumQuantity}; attempted {after}.");
        }

        ulong inventoryRevisionBefore = _revision;
        IReadOnlyList<CapacityUsage> capacityBefore = ComputeCapacity(owner, inventory);
        InventoryState candidate = inventory.Clone();
        candidate.SetEntry(definition, after);
        IReadOnlyList<CapacityUsage> capacityAfter = ComputeCapacity(owner, candidate);
        candidate.SetRevision(checked(candidate.Revision + 1));
        _inventories[owner] = candidate;
        TouchStore();
        return new InventoryMutationReceipt(
            InventoryMutationKind.Grant,
            owner,
            definition.Id,
            quantity,
            before,
            after,
            inventory.Revision,
            candidate.Revision,
            capacityBefore,
            capacityAfter);
    }

    internal InventoryMutationReceipt ConsumeCore(
        EntityId owner,
        ItemDefinition definition,
        ulong quantity)
    {
        ArgumentNullException.ThrowIfNull(definition);
        EnsureFungible(definition);
        EnsurePositiveQuantity(quantity);
        InventoryState inventory = RequireInventory(owner);
        EnsureNoDefinitionConflict(inventory, definition);
        ulong before = inventory.TryGetQuantity(definition.Id, out ulong current) ? current : 0;
        if (quantity > before)
        {
            throw new MechanicsException(
                $"Inventory {owner.Value} has only {before} of item {definition.Id}, requested {quantity}.");
        }

        ulong after = before - quantity;
        ulong inventoryRevisionBefore = _revision;
        IReadOnlyList<CapacityUsage> capacityBefore = ComputeCapacity(owner, inventory);
        InventoryState candidate = inventory.Clone();
        candidate.SetEntry(definition, after);
        IReadOnlyList<CapacityUsage> capacityAfter = ComputeCapacity(owner, candidate);
        candidate.SetRevision(checked(candidate.Revision + 1));
        _inventories[owner] = candidate;
        TouchStore();
        return new InventoryMutationReceipt(
            InventoryMutationKind.Consume,
            owner,
            definition.Id,
            quantity,
            before,
            after,
            inventory.Revision,
            candidate.Revision,
            capacityBefore,
            capacityAfter);
    }

    internal InventoryTransferReceipt TransferFungibleCore(
        EntityId fromOwner,
        EntityId toOwner,
        ItemDefinition definition,
        ulong quantity)
    {
        ArgumentNullException.ThrowIfNull(definition);
        EnsureFungible(definition);
        EnsurePositiveQuantity(quantity);
        if (fromOwner == toOwner)
        {
            throw new MechanicsException("A fungible transfer requires distinct owners.");
        }

        InventoryState from = RequireInventory(fromOwner);
        InventoryState to = RequireInventory(toOwner);
        EnsureNoDefinitionConflict(from, definition);
        EnsureNoDefinitionConflict(to, definition);
        ulong fromBefore = from.TryGetQuantity(definition.Id, out ulong fromQuantity) ? fromQuantity : 0;
        if (quantity > fromBefore)
        {
            throw new MechanicsException(
                $"Inventory {fromOwner.Value} has only {fromBefore} of item {definition.Id}, requested {quantity}.");
        }
        ulong toBefore = to.TryGetQuantity(definition.Id, out ulong toQuantity) ? toQuantity : 0;
        ulong toAfter;
        try
        {
            toAfter = checked(toBefore + quantity);
        }
        catch (OverflowException)
        {
            throw new MechanicsException($"Quantity for item {definition.Id} overflowed.");
        }
        if (toAfter > definition.MaximumQuantity)
        {
            throw new MechanicsException(
                $"Item {definition.Id} cannot exceed quantity {definition.MaximumQuantity}; attempted {toAfter}.");
        }

        ulong fromAfter = fromBefore - quantity;
        ulong inventoryRevisionBefore = _revision;
        IReadOnlyList<CapacityUsage> fromCapacityBefore = ComputeCapacity(fromOwner, from);
        IReadOnlyList<CapacityUsage> toCapacityBefore = ComputeCapacity(toOwner, to);
        InventoryState fromCandidate = from.Clone();
        InventoryState toCandidate = to.Clone();
        fromCandidate.SetEntry(definition, fromAfter);
        toCandidate.SetEntry(definition, toAfter);
        IReadOnlyList<CapacityUsage> fromCapacityAfter = ComputeCapacity(fromOwner, fromCandidate);
        IReadOnlyList<CapacityUsage> toCapacityAfter = ComputeCapacity(toOwner, toCandidate);
        fromCandidate.SetRevision(checked(fromCandidate.Revision + 1));
        toCandidate.SetRevision(checked(toCandidate.Revision + 1));
        _inventories[fromOwner] = fromCandidate;
        _inventories[toOwner] = toCandidate;
        TouchStore();
        return new InventoryTransferReceipt(
            fromOwner,
            toOwner,
            definition.Id,
            quantity,
            fromBefore,
            fromAfter,
            toBefore,
            toAfter,
            from.Revision,
            fromCandidate.Revision,
            to.Revision,
            toCandidate.Revision,
            fromCapacityBefore,
            fromCapacityAfter,
            toCapacityBefore,
            toCapacityAfter);
    }

    internal InventoryStore Clone()
    {
        var result = new InventoryStore { _revision = _revision };
        foreach ((EntityId owner, InventoryState state) in _inventories)
        {
            result._inventories.Add(owner, state.Clone());
        }
        foreach ((EntityId item, ItemState state) in _items)
        {
            result._items.Add(item, state);
        }
        foreach ((EntityId owner, EquipmentState state) in _equipment)
        {
            result._equipment.Add(owner, state.Clone());
        }
        foreach ((EntityId child, EntityId container) in _containment)
        {
            result._containment.Add(child, container);
        }
        foreach ((EntityId container, SortedSet<EntityId> children) in _containedChildren)
        {
            result._containedChildren.Add(container, [.. children]);
        }

        return result;
    }

    internal void PublishCandidate(InventoryStore candidate, ulong expectedRevision)
    {
        if (_revision != expectedRevision)
        {
            throw new MechanicsException(
                $"Inventory store changed while a candidate was prepared: expected {expectedRevision}, actual {_revision}.");
        }

        candidate.ValidateStore();
        _inventories.Clear();
        foreach ((EntityId owner, InventoryState state) in candidate._inventories)
        {
            _inventories.Add(owner, state);
        }
        _items.Clear();
        foreach ((EntityId item, ItemState state) in candidate._items)
        {
            _items.Add(item, state);
        }
        _equipment.Clear();
        foreach ((EntityId owner, EquipmentState state) in candidate._equipment)
        {
            _equipment.Add(owner, state);
        }
        _containment.Clear();
        foreach ((EntityId child, EntityId container) in candidate._containment)
        {
            _containment.Add(child, container);
        }
        _containedChildren.Clear();
        foreach ((EntityId container, SortedSet<EntityId> children) in candidate._containedChildren)
        {
            _containedChildren.Add(container, children);
        }
        _revision = candidate._revision;
    }

    private T Commit<T>(Func<InventoryEdit, T> operation)
    {
        InventoryEdit candidate = Prepare();
        T receipt = operation(candidate);
        candidate.Publish();
        return receipt;
    }

    private void TouchStore() => _revision = checked(_revision + 1);

    private InventoryState RequireInventory(EntityId owner) =>
        _inventories.TryGetValue(owner, out InventoryState? state)
            ? state
            : throw new MechanicsException($"Inventory owner {owner.Value} is not registered.");

    private ItemState RequireItem(EntityId item) =>
        _items.TryGetValue(item, out ItemState? state)
            ? state
            : throw new MechanicsException($"Unique item entity {item.Value} is not registered.");

    private EquipmentState RequireEquipment(EntityId owner) =>
        _equipment.TryGetValue(owner, out EquipmentState? state)
            ? state
            : throw new MechanicsException($"Equipment owner {owner.Value} is not registered.");

    private InventoryView BuildView(
        EntityId owner,
        InventoryState inventory,
        IReadOnlyList<CapacityUsage> capacity)
    {
        UniqueInventoryItem[] uniqueItems = _containedChildren.TryGetValue(owner, out SortedSet<EntityId>? children)
            ? children.Select(item =>
                    new UniqueInventoryItem(item, RequireItem(item).Definition.Id))
                .ToArray()
            : [];
        return new InventoryView(
            owner,
            _revision,
            inventory.Revision,
            inventory.Stacks,
            Array.AsReadOnly(uniqueItems),
            capacity);
    }

    private IReadOnlyList<CapacityUsage> ComputeCapacity(
        EntityId owner,
        InventoryState inventory,
        EntityId? excludedItem = null,
        EntityId? includedItem = null,
        ItemState? includedState = null)
    {
        var used = new Dictionary<CapacityMetricId, ulong>();
        foreach (InventoryCapacityLimit limit in inventory.CapacityLimits)
        {
            used.TryAdd(limit.Metric, 0);
        }

        foreach (InventoryState.InventoryStackEntry entry in inventory.Entries())
        {
            AddCapacityCosts(used, entry.Definition, entry.Quantity);
        }

        if (_containedChildren.TryGetValue(owner, out SortedSet<EntityId>? children))
        {
            foreach (EntityId child in children)
            {
                if (excludedItem == child)
                {
                    continue;
                }
                AddCapacityCosts(used, RequireItem(child).Definition, 1);
            }
        }

        if (includedItem is EntityId included
            && (!_containment.TryGetValue(included, out EntityId current) || current != owner))
        {
            ItemDefinition definition = _items.TryGetValue(included, out ItemState? stored)
                ? stored.Definition
                : includedState?.Definition
                    ?? throw new MechanicsException($"Unique item entity {included.Value} is not registered.");
            AddCapacityCosts(used, definition, 1);
        }

        Dictionary<CapacityMetricId, ulong> limits = inventory.CapacityLimits
            .ToDictionary(limit => limit.Metric, limit => limit.Maximum);
        var result = new List<CapacityUsage>(used.Count);
        foreach ((CapacityMetricId metric, ulong amount) in used.OrderBy(entry => entry.Key.Value, StringComparer.Ordinal))
        {
            ulong? maximum = limits.TryGetValue(metric, out ulong maximumValue) ? maximumValue : null;
            if (maximum is ulong admittedMaximum && amount > admittedMaximum)
            {
                throw new MechanicsException(
                    $"Inventory {owner.Value} exceeds capacity {metric}: attempted {amount}, maximum {admittedMaximum}.");
            }
            result.Add(new CapacityUsage(metric, amount, maximum));
        }

        return Array.AsReadOnly(result.ToArray());
    }

    private static void AddCapacityCosts(
        Dictionary<CapacityMetricId, ulong> used,
        ItemDefinition definition,
        ulong quantity)
    {
        foreach (ItemCapacityCost cost in definition.CapacityCosts)
        {
            ulong amount;
            ulong total;
            try
            {
                amount = checked(cost.Units * quantity);
                total = checked(used.TryGetValue(cost.Metric, out ulong current) ? current + amount : amount);
            }
            catch (OverflowException)
            {
                throw new MechanicsException(
                    $"Capacity arithmetic overflowed for metric {cost.Metric}.");
            }
            used[cost.Metric] = total;
        }
    }

    internal void ValidateStore()
    {
        foreach ((EntityId owner, InventoryState inventory) in _inventories)
        {
            _ = ComputeCapacity(owner, inventory);
            if (_containedChildren.TryGetValue(owner, out SortedSet<EntityId>? children)
                && children.Count > ManagedInventoryLimits.MaximumContainedEntitiesPerInventory)
            {
                throw new MechanicsException(
                    $"Inventory {owner.Value} contains more than {ManagedInventoryLimits.MaximumContainedEntitiesPerInventory} entities.");
            }
        }

        foreach ((EntityId child, EntityId container) in _containment)
        {
            if (!_items.ContainsKey(child) || !_inventories.ContainsKey(container)
                || !_containedChildren.TryGetValue(container, out SortedSet<EntityId>? children)
                || !children.Contains(child))
            {
                throw new MechanicsException("Inventory containment indexes are inconsistent.");
            }
        }

        foreach ((EntityId container, SortedSet<EntityId> children) in _containedChildren)
        {
            if (!_inventories.ContainsKey(container)
                || children.Any(child => !_items.ContainsKey(child)
                    || !_containment.TryGetValue(child, out EntityId actual)
                    || actual != container))
            {
                throw new MechanicsException("Inventory containment reverse indexes are inconsistent.");
            }
        }

        foreach ((EntityId owner, EquipmentState equipment) in _equipment)
        {
            ValidateEquipment(owner, equipment, out _);
        }
    }

    private static void EnsureFungible(ItemDefinition definition)
    {
        if (definition.Kind != ItemKind.Fungible)
        {
            throw new MechanicsException(
                $"Item {definition.Id} is {definition.Kind}, but a fungible stack was required.");
        }
    }

    private static void EnsureDefinitionMatches(
        ItemDefinition expected,
        ItemDefinition actual)
    {
        if (!expected.Matches(actual))
        {
            throw new MechanicsException(
                $"Item definition {expected.Id} conflicts with the definition already stored in this store.");
        }
    }

    private static void EnsurePositiveQuantity(ulong quantity)
    {
        if (quantity == 0)
        {
            throw new MechanicsException("Inventory quantities must be positive.");
        }
    }

    private static void EnsureContainmentQuota(
        SortedSet<EntityId>? children,
        EntityId owner,
        bool adding)
    {
        int count = children?.Count ?? 0;
        if (adding && count >= ManagedInventoryLimits.MaximumContainedEntitiesPerInventory)
        {
            throw new MechanicsException(
                $"Inventory {owner.Value} cannot contain more than {ManagedInventoryLimits.MaximumContainedEntitiesPerInventory} entities.");
        }
    }

    private static void EnsureNoDefinitionConflict(InventoryState inventory, ItemDefinition definition)
    {
        if (inventory.TryGetEntry(definition.Id, out InventoryState.InventoryStackEntry? entry)
            && entry is not null)
        {
            EnsureDefinitionMatches(definition, entry.Definition);
        }
    }

}

/// <summary>
/// Detached managed inventory candidate. Mutations are applied to the detached
/// copy and become live only when <see cref="Publish"/> succeeds.
/// </summary>
public sealed partial class InventoryEdit : IDisposable
{
    private readonly InventoryStore _owner;
    private InventoryStore? _working;
    private readonly ulong _expectedOwnerRevision;
    private bool _published;

    internal InventoryEdit(
        InventoryStore owner,
        InventoryStore working,
        ulong expectedOwnerRevision)
    {
        _owner = owner;
        _working = working;
        _expectedOwnerRevision = expectedOwnerRevision;
    }

    public ulong Revision
    {
        get => Execute(static working => working.Revision);
    }

    public InventoryView View(EntityId owner) => Execute(working => working.View(owner));

    public void Validate() => Execute(static working => working.ValidateStore());

    public void Publish()
    {
        InventoryStore working = EnsureOpen();
        try
        {
            working.ValidateStore();
            _owner.PublishCandidate(working, _expectedOwnerRevision);
            _published = true;
        }
        finally
        {
            Discard();
        }
    }

    /// <summary>Discards this detached edit without changing its owner.</summary>
    public void Cancel() => Discard();

    /// <summary>Discards this detached edit without changing its owner.</summary>
    public void Dispose() => Discard();

    public InventoryMutationReceipt Grant(EntityId owner, ItemDefinition definition, ulong quantity) =>
        Execute(working => working.GrantCore(owner, definition, quantity));

    public InventoryMutationReceipt Consume(EntityId owner, ItemDefinition definition, ulong quantity) =>
        Execute(working => working.ConsumeCore(owner, definition, quantity));

    public InventoryTransferReceipt TransferFungible(
        EntityId fromOwner,
        EntityId toOwner,
        ItemDefinition definition,
        ulong quantity)
        => Execute(working => working.TransferFungibleCore(fromOwner, toOwner, definition, quantity));

    public ItemMaterializationReceipt MaterializeUnique(ItemState item, EntityId owner) =>
        Execute(working => working.MaterializeUniqueCore(item, owner));

    public ItemTransferReceipt TransferUnique(EntityId item, EntityId fromOwner, EntityId toOwner) =>
        Execute(working => working.TransferUniqueCore(item, fromOwner, toOwner));

    public ItemDestroyReceipt DestroyUnique(EntityId item) =>
        Execute(working => working.DestroyUniqueCore(item));

    public EquipmentMutationReceipt Equip(
        EntityId owner,
        EntityId item,
        IEnumerable<EquipmentSlotDefinition> slots)
        => Execute(working => working.EquipCore(owner, item, slots));

    public EquipmentMutationReceipt Unequip(EntityId owner, EntityId item) =>
        Execute(working => working.UnequipCore(owner, item));

    public EquipmentMutationReceipt Swap(
        EntityId owner,
        EntityId outgoingItem,
        EntityId incomingItem,
        IEnumerable<EquipmentSlotDefinition> slots)
        => Execute(working => working.SwapCore(owner, outgoingItem, incomingItem, slots));

    private T Execute<T>(Func<InventoryStore, T> operation)
    {
        InventoryStore working = EnsureOpen();
        try
        {
            return operation(working);
        }
        catch
        {
            Discard();
            throw;
        }
    }

    private void Execute(Action<InventoryStore> operation)
    {
        InventoryStore working = EnsureOpen();
        try
        {
            operation(working);
        }
        catch
        {
            Discard();
            throw;
        }
    }

    private InventoryStore EnsureOpen() => _working ?? throw new InvalidOperationException(
        _published
            ? "An inventory candidate cannot be used after publication."
            : "An inventory candidate cannot be used after it was discarded.");

    private void Discard() => _working = null;
}
