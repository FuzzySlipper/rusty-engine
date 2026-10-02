using Rusty.Engine.Entities;
using Rusty.Engine.Mechanics;

/// <summary>
/// Items of one definition that weigh differently, and an item whose weight
/// changes while carried: inventory capacity follows each item's own costs.
/// </summary>
internal static class ItemCapacityCostExercise
{
    private const ulong LanternWeight = 10;
    private const ulong HeavyLanternWeight = 40;
    private const ulong PackLimit = 60;
    private const ulong WagonLimit = 30;

    public static void Run()
    {
        CapacityMetricId weight = CapacityMetricId.Parse("cost-weight");
        CapacityMetricId bulk = CapacityMetricId.Parse("cost-bulk");
        EntityId pack = new(81);
        EntityId wagon = new(82);
        EntityId heavy = new(90);
        EntityId plain = new(91);
        ItemDefinition lantern = new(
            ItemDefinitionId.Parse("cost-lantern"),
            ItemKind.Unique,
            maximumQuantity: 1,
            capacityCosts: [new ItemCapacityCost(weight, LanternWeight)],
            equipment: new ItemEquipmentPolicy(requiredSlots: 1));
        EquipmentSlotDefinition hand = new(EquipmentSlotId.Parse("cost-hand"));

        var store = new InventoryStore();
        store.RegisterInventory(new InventoryState(pack, [new InventoryCapacityLimit(weight, PackLimit)]));
        store.RegisterEquipment(new EquipmentState(pack));
        store.RegisterInventory(new InventoryState(wagon, [new InventoryCapacityLimit(weight, WagonLimit)]));

        // Same definition, different weights.
        store.MaterializeUnique(new ItemState(heavy, lantern, [new ItemCapacityCost(weight, HeavyLanternWeight)]), pack);
        store.MaterializeUnique(new ItemState(plain, lantern), pack);
        Require(Weight(store, pack, weight) == HeavyLanternWeight + LanternWeight, "per-item costs were not counted");
        Require(store.TryGetItem(heavy, out ItemState? heavyState) && heavyState!.Definition == lantern
            && heavyState.CapacityCosts.Single().Units == HeavyLanternWeight,
            "an item with its own costs lost its definition or costs");

        // Changed while equipped; equipment, containment and definition stay.
        store.Equip(pack, plain, [hand]);
        ulong packRevision = store.View(pack).InventoryRevision;
        ItemCapacityCostReceipt heavier = store.SetCapacityCosts(plain, [new ItemCapacityCost(weight, 20)]);
        Require(heavier.Container == pack && heavier.CapacityBefore.Single().Used == 50
            && heavier.CapacityAfter.Single().Used == PackLimit
            && store.View(pack).InventoryRevision == packRevision + 1,
            "a live cost change was not admitted against its container");
        Require(store.TryGetEquipment(pack, out EquipmentState? equipped) && equipped!.ContainsItem(plain)
            && store.TryGetContainer(plain, out EntityId container) && container == pack,
            "a cost change moved or unequipped the item");

        // A change the container cannot hold is refused and changes nothing.
        ExpectUnchangedRefusal(store, MechanicsRefusal.Capacity,
            () => store.SetCapacityCosts(plain, [new ItemCapacityCost(weight, 21)]),
            "a cost change beyond the container's capacity was admitted");
        Require(Weight(store, pack, weight) == PackLimit, "a refused cost change changed capacity");
        ExpectArgument(() => store.SetCapacityCosts(plain, [new ItemCapacityCost(weight, 1), new ItemCapacityCost(weight, 2)]),
            "a cost list naming a metric twice was admitted");

        // Null returns to the definition's costs; an empty list costs nothing.
        store.SetCapacityCosts(plain, null);
        Require(Weight(store, pack, weight) == HeavyLanternWeight + LanternWeight
            && store.TryGetItem(plain, out ItemState? cleared) && cleared!.CapacityCostOverride is null,
            "clearing did not return to the definition's costs");
        store.SetCapacityCosts(plain, []);
        Require(Weight(store, pack, weight) == HeavyLanternWeight, "an empty cost list was counted");
        store.SetCapacityCosts(plain, [new ItemCapacityCost(bulk, 3)]);
        Require(Weight(store, pack, weight) == HeavyLanternWeight
            && store.View(pack).Capacity.Single(usage => usage.Metric == bulk).Used == 3,
            "an item's own metric was not counted");

        // Transfer admits the item's own weight at the destination.
        ExpectUnchangedRefusal(store, MechanicsRefusal.Capacity, () => store.TransferUnique(heavy, pack, wagon),
            "a transfer beyond the destination's capacity was admitted");
        using (InventoryEdit edit = store.Prepare())
        {
            edit.SetCapacityCosts(heavy, [new ItemCapacityCost(weight, 25)]);
            edit.TransferUnique(heavy, pack, wagon);
            Require(Weight(store, pack, weight) == HeavyLanternWeight, "an unpublished edit changed the store");
            edit.Publish();
        }
        Require(Weight(store, wagon, weight) == 25 && Weight(store, pack, weight) == 0,
            "a cost change and transfer in one edit did not apply together");

        // A product save stores each item's own costs and restores them.
        var saved = store.ItemEntities
            .Select(item => store.TryGetItem(item, out ItemState? state) && store.TryGetContainer(item, out EntityId owner)
                ? (state!, owner)
                : throw new InvalidOperationException("an item lost its state"))
            .ToArray();
        var restored = new InventoryStore();
        restored.RegisterInventory(new InventoryState(pack, [new InventoryCapacityLimit(weight, PackLimit)]));
        restored.RegisterInventory(new InventoryState(wagon, [new InventoryCapacityLimit(weight, WagonLimit)]));
        foreach ((ItemState state, EntityId owner) in saved)
            restored.MaterializeUnique(new ItemState(state.Entity, state.Definition, state.CapacityCostOverride), owner);
        Require(restored.View(pack).Capacity.SequenceEqual(store.View(pack).Capacity)
            && restored.View(wagon).Capacity.SequenceEqual(store.View(wagon).Capacity),
            "restored items did not keep their own costs");

        // Removing an item removes its own weight.
        store.DestroyUnique(heavy);
        Require(Weight(store, wagon, weight) == 0, "a destroyed item still weighed");
        ExpectUnchangedRefusal(store, MechanicsRefusal.NotFound, () => store.SetCapacityCosts(heavy, null),
            "a destroyed item's costs were changed");
        Console.WriteLine("passed: unique items carry their own capacity costs through change, equipment, transfer, refusal and restore");
    }

    private static ulong Weight(InventoryStore store, EntityId owner, CapacityMetricId metric) =>
        store.View(owner).Capacity.Single(usage => usage.Metric == metric).Used;

    private static void ExpectUnchangedRefusal(InventoryStore store, MechanicsRefusal reason, Action action, string message)
    {
        ulong revision = store.Revision;
        try
        {
            action();
        }
        catch (MechanicsException exception) when (exception.Reason == reason)
        {
            Require(store.Revision == revision, $"{message} (the store revision changed)");
            return;
        }

        throw new InvalidOperationException(message);
    }

    private static void ExpectArgument(Action action, string message)
    {
        try
        {
            action();
        }
        catch (ArgumentException)
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
