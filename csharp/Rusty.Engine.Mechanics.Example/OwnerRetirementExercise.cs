using Rusty.Engine.Entities;
using Rusty.Engine.Mechanics;

internal static class OwnerRetirementExercise
{
    private const ulong StackQuantity = 4;

    public static void Run()
    {
        EntityId retiring = new(61);
        EntityId keeper = new(62);
        EntityId inventoryOnly = new(63);
        EntityId never = new(64);
        EntityId blade = new(70);
        EntityId token = new(71);
        ItemDefinition supply = new(ItemDefinitionId.Parse("retirement-supply"), ItemKind.Fungible, maximumQuantity: 10);
        ItemDefinition wieldable = new(
            ItemDefinitionId.Parse("retirement-blade"),
            ItemKind.Unique,
            maximumQuantity: 1,
            equipment: new ItemEquipmentPolicy(requiredSlots: 1));
        ItemDefinition keepsake = new(ItemDefinitionId.Parse("retirement-token"), ItemKind.Unique, maximumQuantity: 1);
        EquipmentSlotDefinition hand = new(EquipmentSlotId.Parse("retirement-hand"));
        InventoryStackId carried = InventoryStackId.Parse("retirement-carried");

        var store = new InventoryStore();
        store.RegisterInventory(new InventoryState(retiring));
        store.RegisterEquipment(new EquipmentState(retiring));
        store.RegisterInventory(new InventoryState(keeper));
        store.RegisterEquipment(new EquipmentState(keeper));
        store.RegisterInventory(new InventoryState(inventoryOnly));
        var inventory = new InventoryComponent(store, retiring);
        var equipment = new EquipmentComponent(store, retiring);

        inventory.Grant(supply, carried, StackQuantity);
        inventory.MaterializeUnique(new ItemState(blade, wieldable));
        inventory.MaterializeUnique(new ItemState(token, keepsake));
        equipment.Equip(blade, [hand]);

        // Refused while anything remains: an equipped item, an unequipped item, or only a stack.
        ExpectUnchangedRefusal(store, MechanicsRefusal.AlreadyPresent, () => store.RetireOwner(retiring),
            "an owner with equipped items, unique items and stacks was retired");
        store.Unequip(retiring, blade);
        store.TransferUnique(blade, retiring, keeper);
        ExpectUnchangedRefusal(store, MechanicsRefusal.AlreadyPresent, () => store.RetireOwner(retiring),
            "an owner containing a unique item was retired");
        store.DestroyUnique(token);
        ExpectUnchangedRefusal(store, MechanicsRefusal.AlreadyPresent, () => store.RetireOwner(retiring),
            "an owner holding a stack was retired");
        Require(store.InventoryOwners.Contains(retiring) && store.EquipmentOwners.Contains(retiring)
            && inventory.Stacks.Single().Quantity == StackQuantity,
            "a refused retire changed the owner");

        // Emptying and retiring apply together in one edit, and only on publish.
        using (InventoryEdit edit = store.Prepare())
        {
            edit.Consume(retiring, carried, StackQuantity);
            edit.RetireOwner(retiring);
            Require(store.InventoryOwners.Contains(retiring), "an unpublished retire changed the store");
            edit.Publish();
        }

        Require(!store.InventoryOwners.Contains(retiring) && !store.EquipmentOwners.Contains(retiring)
            && !store.TryGetInventory(retiring, out _) && !store.TryGetEquipment(retiring, out _),
            "a retired owner was still registered");
        ExpectRefusal(MechanicsRefusal.NotFound, () => store.View(retiring), "a retired owner still had a view");
        ExpectRefusal(MechanicsRefusal.NotFound, () => _ = inventory.Stacks, "a retained inventory component read a retired owner");
        ExpectRefusal(MechanicsRefusal.NotFound, () => _ = equipment.Assignments, "a retained equipment component read a retired owner");
        Require(store.TryGetContainer(blade, out EntityId bladeContainer) && bladeContainer == keeper
            && !store.TryGetItem(token, out _)
            && store.View(keeper).UniqueItems.Single().Entity == blade,
            "retiring one owner changed another owner's items");

        ExpectUnchangedRefusal(store, MechanicsRefusal.NotFound, () => store.RetireOwner(retiring),
            "an owner was retired twice");
        ExpectUnchangedRefusal(store, MechanicsRefusal.NotFound, () => store.RetireOwner(never),
            "an unregistered owner was retired");

        // A direct retire invalidates an edit prepared before it.
        InventoryEdit stale = store.Prepare();
        stale.Grant(keeper, supply, carried, 1);
        store.RetireOwner(inventoryOnly);
        Require(!store.InventoryOwners.Contains(inventoryOnly), "an inventory-only owner was not retired");
        ExpectRefusal(MechanicsRefusal.RevisionConflict, stale.Publish, "an edit prepared before a retire was published");
        Require(store.View(keeper).Stacks.Count == 0 && !store.InventoryOwners.Contains(inventoryOnly),
            "a refused stale edit changed the store");

        // A retired owner may be registered again and starts from its new state.
        store.RegisterInventory(new InventoryState(retiring));
        Require(store.View(retiring).Stacks.Count == 0 && store.View(retiring).UniqueItems.Count == 0
            && !store.EquipmentOwners.Contains(retiring),
            "a re-registered owner kept retired state");

        Console.WriteLine("passed: empty owners retire with their equipment registration; non-empty, unknown and repeated retires refuse");
    }

    private static void ExpectUnchangedRefusal(InventoryStore store, MechanicsRefusal reason, Action action, string message)
    {
        ulong revision = store.Revision;
        ExpectRefusal(reason, action, message);
        Require(store.Revision == revision, $"{message} (the store revision changed)");
    }

    private static void ExpectRefusal(MechanicsRefusal reason, Action action, string message)
    {
        try
        {
            action();
        }
        catch (MechanicsException exception) when (exception.Reason == reason)
        {
            return;
        }

        throw new InvalidOperationException(message);
    }

    private static void Require(bool condition, string message)
    {
        if (!condition)
        {
            throw new InvalidOperationException(message);
        }
    }
}
