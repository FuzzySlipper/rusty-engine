using Rusty.Engine.Entities;
using Rusty.Engine.Mechanics;

internal static class MechanicsComponentsExercise
{
    public static void Run()
    {
        using var entities = new EntityStore();
        var owner = entities.Create();
        var destination = entities.Create();
        var store = new InventoryStore();
        store.RegisterInventory(new InventoryState(owner));
        store.RegisterInventory(new InventoryState(destination));
        store.RegisterEquipment(new EquipmentState(owner));
        store.RegisterEquipment(new EquipmentState(destination));
        var inventory = new InventoryComponent(store, owner);
        var equipment = new EquipmentComponent(store, owner);
        var other = new InventoryComponent(store, destination);
        var otherEquipment = new EquipmentComponent(store, destination);
        entities.Add(owner, inventory);
        entities.Add(owner, equipment);
        var ammunition = new ItemDefinition(ItemDefinitionId.Parse("ammunition"), ItemKind.Fungible, 100);
        var carriedStack = InventoryStackId.Parse("carried-ammunition");
        var transferredStack = InventoryStackId.Parse("transferred-ammunition");
        inventory.Grant(ammunition, carriedStack, 5);
        inventory.Consume(carriedStack, 1);
        inventory.TransferFungible(destination, carriedStack, transferredStack, 2);
        Check(inventory.Stacks.Single().Quantity == 2 && other.Stacks.Single().Quantity == 2, "facade quantity operations conserve items");
        var weapon = new ItemDefinition(ItemDefinitionId.Parse("blade"), ItemKind.Unique, 1, equipment: new ItemEquipmentPolicy(1));
        var item = entities.Create();
        var slot = new EquipmentSlotDefinition(EquipmentSlotId.Parse("hand"));
        inventory.MaterializeUnique(new ItemState(item, weapon));
        equipment.Equip(item, [slot]);
        using (var edit = store.Prepare())
        {
            edit.Unequip(owner, item);
            edit.TransferUnique(item, owner, destination);
            edit.Equip(destination, item, [slot]);
            Check(equipment.ContainsItem(item) && inventory.Contains(item), "unpublished edits do not change live facades");
            edit.Publish();
        }
        Check(!equipment.ContainsItem(item) && !inventory.Contains(item) && other.Contains(item)
            && otherEquipment.Assignments.Single().Item == item, "retained facades follow published canonical records");
        Check(ReferenceEquals(entities.Get<InventoryComponent>(owner), inventory), "entity returns the retained facade");
        Check(!entities.TryGetContainedIn(item, out _), "inventory transfers do not change entity relationships");

        var effects = new EffectsComponent(owner);
        entities.Add(owner, effects);
        var definition = new EffectDefinition(EffectDefinitionId.Parse("ward"), StackingGroupId.Parse("ward"), EffectStackingPolicy.Refresh, 1, 3,
            [SourceDefinitionId.Parse("ward-bonus")]);
        var instance = EffectInstanceId.Parse("ward-instance");
        var source = new IntrinsicSourceIdentity(owner, SourceInstanceId.Parse("caster"));
        effects.Apply(definition, instance, source, 1);
        var copy = effects.Copy();
        var receipt = copy.Refresh(instance, source, 2);
        Check(copy.Owner == owner && receipt.ActivatedSources.Count == 2 && effects.Effects.Single().Stacks == 1,
            "detached copy preserves provenance and cannot mutate the live collection");
        effects.Expire(instance);
        Check(copy.Effects.Single().Stacks == 2 && effects.Effects.Count == 0, "live expiry cannot mutate the detached copy");
        Console.WriteLine("passed: live inventory/equipment components and detached effects copies");
    }

    private static void Check(bool condition, string message)
    {
        if (!condition) throw new InvalidOperationException(message);
    }
}
