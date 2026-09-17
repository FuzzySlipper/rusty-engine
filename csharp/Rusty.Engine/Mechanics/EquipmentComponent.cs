using Rusty.Engine.Entities;

namespace Rusty.Engine.Mechanics;

/// <summary>Live equipment access for one owner in an InventoryStore, usable standalone or attached.</summary>
public sealed class EquipmentComponent
{
    public EquipmentComponent(InventoryStore store, EntityId owner)
    {
        Store = store ?? throw new ArgumentNullException(nameof(store));
        Owner = owner;
        _ = State;
    }

    public InventoryStore Store { get; }
    public EntityId Owner { get; }
    private EquipmentState State => Store.EquipmentForComponent(Owner);
    public ulong Revision => State.Revision;
    public IReadOnlyList<EquipmentAssignment> Assignments => State.Assignments;
    public bool ContainsItem(EntityId item) => State.ContainsItem(item);
    public EquipmentMutationReceipt Equip(EntityId item, IEnumerable<EquipmentSlotDefinition> slots) => Store.Equip(Owner, item, slots);
    public EquipmentMutationReceipt Unequip(EntityId item) => Store.Unequip(Owner, item);
    public EquipmentMutationReceipt Swap(EntityId outgoingItem, EntityId incomingItem, IEnumerable<EquipmentSlotDefinition> slots) => Store.Swap(Owner, outgoingItem, incomingItem, slots);
}

public sealed partial class InventoryStore
{
    internal EquipmentState EquipmentForComponent(EntityId owner) => RequireEquipment(owner);
}
