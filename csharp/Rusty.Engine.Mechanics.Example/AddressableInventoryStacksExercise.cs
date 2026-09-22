using Rusty.Engine.Entities;
using Rusty.Engine.Mechanics;

internal static class AddressableInventoryStacksExercise
{
    public static void Run()
    {
        ExerciseIndependentStackAndDefinitionIdentities();
        EntityId owner = new(41);
        EntityId destination = new(42);
        CapacityMetricId mass = CapacityMetricId.Parse("addressable-stack-mass");
        ItemDefinition supply = new(
            ItemDefinitionId.Parse("addressable-supply"),
            ItemKind.Fungible,
            maximumQuantity: 10,
            capacityCosts: [new ItemCapacityCost(mass, 1)]);
        InventoryStackId first = InventoryStackId.Parse("supply-first");
        InventoryStackId second = InventoryStackId.Parse("supply-second");
        InventoryStackId split = InventoryStackId.Parse("supply-split");
        InventoryStackId destinationStack = InventoryStackId.Parse("destination-supply");
        InventoryStackId rejectedStack = InventoryStackId.Parse("rejected-supply");

        var store = new InventoryStore();
        store.RegisterInventory(new InventoryState(owner, [new InventoryCapacityLimit(mass, 10)]));
        store.RegisterInventory(new InventoryState(destination, [new InventoryCapacityLimit(mass, 3)]));

        store.Grant(owner, supply, first, 6);
        store.Grant(owner, supply, second, 3);
        RequireStacks(store.View(owner), (first, 6), (second, 3));

        store.Consume(owner, first, 1);
        InventorySplitReceipt splitReceipt = store.SplitFungible(owner, second, split, 1);
        Require(splitReceipt.SourceStack == second && splitReceipt.SplitStack == split,
            "split did not report its source and product-selected identity");
        RequireStacks(store.View(owner), (first, 5), (second, 2), (split, 1));

        InventoryMergeReceipt merge = store.MergeFungible(owner, split, second);
        Require(merge.SourceStack == split && merge.DestinationStack == second,
            "merge did not report the retired and retained identities");
        RequireStacks(store.View(owner), (first, 5), (second, 3));

        InventoryTransferReceipt partial = store.TransferFungible(owner, destination, first, destinationStack, 2);
        Require(partial.SourceStack == first && partial.DestinationStack == destinationStack,
            "partial transfer did not retain selected stack identities");
        RequireStacks(store.View(owner), (first, 3), (second, 3));
        RequireStacks(store.View(destination), (destinationStack, 2));

        store.TransferFungible(owner, destination, second, destinationStack, 1);
        RequireStacks(store.View(owner), (first, 3), (second, 2));
        RequireStacks(store.View(destination), (destinationStack, 3));

        InventoryView beforeInsufficient = store.View(owner);
        ulong beforeInsufficientRevision = store.Revision;
        ExpectMechanicsError(
            () => store.TransferFungible(owner, destination, second, destinationStack, 3),
            "insufficient selected-stack transfer succeeded");
        Require(store.Revision == beforeInsufficientRevision && SameStacks(beforeInsufficient, store.View(owner)),
            "insufficient selected-stack transfer changed the ledger");

        InventoryView beforeCapacity = store.View(destination);
        ulong beforeCapacityRevision = store.Revision;
        ExpectMechanicsError(
            () => store.TransferFungible(owner, destination, second, destinationStack, 2),
            "capacity-rejected selected-stack transfer succeeded");
        Require(store.Revision == beforeCapacityRevision && SameStacks(beforeCapacity, store.View(destination)),
            "capacity-rejected selected-stack transfer changed the ledger");

        ulong beforeRejectedRestore = store.Revision;
        ExpectMechanicsError(
            () => InventoryState.Restore(
                new EntityId(43),
                [new InventoryStackCapture(rejectedStack, supply, 4)],
                [new InventoryCapacityLimit(mass, 3)]),
            "over-capacity stack capture was restored");
        Require(store.Revision == beforeRejectedRestore && !store.TryGetInventory(new EntityId(43), out _),
            "rejected stack restore changed the existing store");

        InventoryState registrationCandidate = InventoryState.Restore(
            new EntityId(43),
            [new InventoryStackCapture(rejectedStack, supply, 4)],
            [new InventoryCapacityLimit(mass, 4)]);
        registrationCandidate.SetCapacityLimit(new InventoryCapacityLimit(mass, 3));
        ExpectMechanicsError(
            () => store.RegisterInventory(registrationCandidate),
            "over-capacity standalone inventory was registered");
        Require(store.Revision == beforeRejectedRestore && !store.TryGetInventory(new EntityId(43), out _),
            "rejected inventory registration changed the store");

        InventoryState ownerCapture = RequireInventory(store, owner);
        InventoryState destinationCapture = RequireInventory(store, destination);
        var restored = new InventoryStore();
        restored.RegisterInventory(InventoryState.Restore(owner, ownerCapture.CaptureStacks(), ownerCapture.CapacityLimits));
        restored.RegisterInventory(InventoryState.Restore(destination, destinationCapture.CaptureStacks(), destinationCapture.CapacityLimits));
        RequireStacks(restored.View(owner), (first, 3), (second, 2));
        RequireStacks(restored.View(destination), (destinationStack, 3));

        using InventoryEdit stale = restored.Prepare();
        stale.SplitFungible(owner, first, split, 1);
        restored.Consume(owner, second, 1);
        ExpectMechanicsError(stale.Publish, "stale selected-stack edit was published");
        RequireStacks(restored.View(owner), (first, 3), (second, 1));

        Console.WriteLine("passed: addressable inventory stacks preserve identity, quantity, capacity, and restore relationships");
    }

    private static void ExerciseIndependentStackAndDefinitionIdentities()
    {
        EntityId owner = new(51);
        EntityId destination = new(52);
        var firstDefinition = new ItemDefinition(ItemDefinitionId.Parse("first-definition"), ItemKind.Fungible, 10);
        var secondDefinition = new ItemDefinition(ItemDefinitionId.Parse("second-definition"), ItemKind.Fungible, 10);
        // Product stack names may equal any definition name without reserving that name.
        InventoryStackId secondStack = InventoryStackId.Parse(firstDefinition.Id.Value);
        InventoryStackId firstStack = InventoryStackId.Parse("first-selected-stack");
        InventoryStackId destinationStack = InventoryStackId.Parse("received-first-stack");
        var store = new InventoryStore();
        store.RegisterInventory(new InventoryState(owner));
        store.RegisterInventory(new InventoryState(destination));
        store.Grant(owner, secondDefinition, secondStack, 2);
        store.Grant(destination, secondDefinition, secondStack, 4);
        store.Grant(owner, firstDefinition, firstStack, 3);
        store.TransferFungible(owner, destination, firstStack, destinationStack, 1);
        store.Consume(owner, firstStack, 1);
        RequireStacks(store.View(owner), (secondStack, 2), (firstStack, 1));
        RequireStacks(store.View(destination), (secondStack, 4), (destinationStack, 1));
        Require(store.View(owner).Stacks.Single(stack => stack.Id == secondStack).Definition == secondDefinition.Id,
            "a stack name matching another definition changed the selected definition");
    }

    private static InventoryState RequireInventory(InventoryStore store, EntityId owner)
    {
        if (!store.TryGetInventory(owner, out InventoryState? inventory) || inventory is null)
        {
            throw new InvalidOperationException("captured inventory was missing");
        }

        return inventory;
    }

    private static void RequireStacks(InventoryView view, params (InventoryStackId Id, ulong Quantity)[] expected)
    {
        Require(view.Stacks.Count == expected.Length, "stack count was incorrect");
        foreach ((InventoryStackId id, ulong quantity) in expected)
        {
            InventoryStack stack = view.Stacks.SingleOrDefault(stack => stack.Id == id);
            Require(stack.Id == id && stack.Quantity == quantity, $"stack {id} had the wrong quantity");
        }
    }

    private static bool SameStacks(InventoryView left, InventoryView right) =>
        left.Stacks.Count == right.Stacks.Count
        && left.Stacks.OrderBy(stack => stack.Id.Value, StringComparer.Ordinal)
            .SequenceEqual(right.Stacks.OrderBy(stack => stack.Id.Value, StringComparer.Ordinal));

    private static void ExpectMechanicsError(Action action, string message)
    {
        try
        {
            action();
        }
        catch (MechanicsException)
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
