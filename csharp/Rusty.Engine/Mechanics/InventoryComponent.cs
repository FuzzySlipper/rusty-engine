using Rusty.Engine.Entities;

namespace Rusty.Engine.Mechanics;

/// <summary>Live owner-scoped access to one InventoryStore. No inventory ledger is copied.</summary>
/// <remarks>Each read resolves the current owner record, including after an InventoryEdit publishes.
/// Returned views and lists describe that read; retain this component for subsequent live reads.</remarks>
public sealed class InventoryComponent
{
    public InventoryComponent(InventoryStore store, EntityId owner)
    {
        Store = store ?? throw new ArgumentNullException(nameof(store));
        Owner = owner;
        _ = State;
    }

    public InventoryStore Store { get; }
    public EntityId Owner { get; }
    private InventoryState State => Store.InventoryForComponent(Owner);
    public ulong Revision => State.Revision;
    public IReadOnlyList<InventoryStack> Stacks => State.Stacks;
    public IReadOnlyList<InventoryCapacityLimit> CapacityLimits => State.CapacityLimits;
    public IReadOnlyList<UniqueInventoryItem> UniqueItems => Store.View(Owner).UniqueItems;
    public InventoryView View() => Store.View(Owner);
    public bool TryGetQuantity(ItemDefinitionId id, out ulong quantity) => State.TryGetQuantity(id, out quantity);
    public bool Contains(EntityId item) => Store.TryGetContainer(item, out var owner) && owner == Owner;
    public InventoryMutationReceipt Grant(ItemDefinition definition, ulong quantity) => Store.Grant(Owner, definition, quantity);
    public InventoryMutationReceipt Consume(ItemDefinition definition, ulong quantity) => Store.Consume(Owner, definition, quantity);
    public InventoryTransferReceipt TransferFungible(EntityId destination, ItemDefinition definition, ulong quantity) => Store.TransferFungible(Owner, destination, definition, quantity);
    public ItemMaterializationReceipt MaterializeUnique(ItemState item) => Store.MaterializeUnique(item, Owner);
    public ItemTransferReceipt TransferUnique(EntityId item, EntityId destination) => Store.TransferUnique(item, Owner, destination);
}

public sealed partial class InventoryStore
{
    internal InventoryState InventoryForComponent(EntityId owner) => RequireInventory(owner);
}
