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
    public InventoryMutationReceipt Grant(ItemDefinition definition, InventoryStackId stack, ulong quantity) => Store.Grant(Owner, definition, stack, quantity);
    public InventoryMutationReceipt Consume(InventoryStackId stack, ulong quantity) => Store.Consume(Owner, stack, quantity);
    public InventoryTransferReceipt TransferFungible(EntityId destination, InventoryStackId stack, ulong quantity) => Store.TransferFungible(Owner, destination, stack, quantity);
    public InventoryTransferReceipt TransferFungible(EntityId destination, InventoryStackId sourceStack, InventoryStackId destinationStack, ulong quantity) => Store.TransferFungible(Owner, destination, sourceStack, destinationStack, quantity);
    public InventorySplitReceipt SplitFungible(InventoryStackId sourceStack, InventoryStackId splitStack, ulong quantity) => Store.SplitFungible(Owner, sourceStack, splitStack, quantity);
    public InventoryMergeReceipt MergeFungible(InventoryStackId sourceStack, InventoryStackId destinationStack) => Store.MergeFungible(Owner, sourceStack, destinationStack);
    public ItemMaterializationReceipt MaterializeUnique(ItemState item) => Store.MaterializeUnique(item, Owner);
    public ItemTransferReceipt TransferUnique(EntityId item, EntityId destination) => Store.TransferUnique(item, Owner, destination);
}

public sealed partial class InventoryStore
{
    internal InventoryState InventoryForComponent(EntityId owner) => RequireInventory(owner);
}
