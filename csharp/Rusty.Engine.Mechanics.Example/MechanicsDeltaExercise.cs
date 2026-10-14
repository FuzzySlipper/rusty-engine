using Rusty.Engine.Entities;
using Rusty.Engine.Mechanics;

/// <summary>
/// Inventory, equipment and effect mutations apply only their own change. A seeded
/// sequence of every mutation kind, refusals and grouped edits runs against a populated
/// store; after each step every derived readout must equal a fresh count from the
/// store's plain contents, and a refusal must leave everything as it was.
/// </summary>
internal static class MechanicsDeltaExercise
{
    private const int Seed = 9936;
    private const int InventorySteps = 6000;
    private const int EffectSteps = 4000;
    private const int StackIdsPerOwner = 6;
    private const int UniqueItemCount = 14;
    private const ulong FirstItemEntity = 700;

    private static readonly CapacityMetricId Mass = CapacityMetricId.Parse("delta-mass");
    private static readonly CapacityMetricId Bulk = CapacityMetricId.Parse("delta-bulk");
    private static readonly EntityId[] Owners = [new(601), new(602), new(603)];

    public static void Run()
    {
        ExerciseInventory();
        ExerciseEffects();
        Console.WriteLine("passed: inventory, equipment and effect deltas match a fresh count after every mutation, refusal and edit");
    }

    private static void ExerciseInventory()
    {
        var random = new Random(Seed);
        ItemClassificationId weapon = ItemClassificationId.Parse("delta-weapon");
        ItemClassificationId shieldClass = ItemClassificationId.Parse("delta-shield");
        ItemClassificationId ringClass = ItemClassificationId.Parse("delta-ring");
        EquipmentExclusivityId hands = EquipmentExclusivityId.Parse("delta-hands");
        ItemDefinition[] fungible =
        [
            new(ItemDefinitionId.Parse("delta-arrow"), ItemKind.Fungible, 40, capacityCosts: [new ItemCapacityCost(Mass, 1)]),
            new(ItemDefinitionId.Parse("delta-ore"), ItemKind.Fungible, 12,
                capacityCosts: [new ItemCapacityCost(Bulk, 2), new ItemCapacityCost(Mass, 3)]),
            new(ItemDefinitionId.Parse("delta-coin"), ItemKind.Fungible, 500),
            new(ItemDefinitionId.Parse("delta-herb"), ItemKind.Fungible, 20, capacityCosts: [new ItemCapacityCost(Bulk, 1)]),
        ];
        ItemDefinition[] unique =
        [
            new(ItemDefinitionId.Parse("delta-dagger"), ItemKind.Unique, 1, [weapon],
                [new ItemCapacityCost(Mass, 2)], new ItemEquipmentPolicy(1), [SourceDefinitionId.Parse("delta-edge")]),
            new(ItemDefinitionId.Parse("delta-greatsword"), ItemKind.Unique, 1, [weapon],
                [new ItemCapacityCost(Mass, 9), new ItemCapacityCost(Bulk, 4)], new ItemEquipmentPolicy(2, hands),
                [SourceDefinitionId.Parse("delta-reach"), SourceDefinitionId.Parse("delta-weight")]),
            new(ItemDefinitionId.Parse("delta-shield"), ItemKind.Unique, 1, [shieldClass],
                [new ItemCapacityCost(Mass, 6)], new ItemEquipmentPolicy(1, hands)),
            new(ItemDefinitionId.Parse("delta-ring"), ItemKind.Unique, 1, [ringClass],
                equipment: new ItemEquipmentPolicy(1), sourceDefinitions: [SourceDefinitionId.Parse("delta-glow")]),
            new(ItemDefinitionId.Parse("delta-idol"), ItemKind.Unique, 1, capacityCosts: [new ItemCapacityCost(Bulk, 5)]),
        ];
        EquipmentSlotDefinition[] slots =
        [
            new(EquipmentSlotId.Parse("delta-left"), [weapon]),
            new(EquipmentSlotId.Parse("delta-right"), [weapon, shieldClass]),
            new(EquipmentSlotId.Parse("delta-ring-a"), [ringClass]),
            new(EquipmentSlotId.Parse("delta-ring-b"), [ringClass]),
            new(EquipmentSlotId.Parse("delta-belt")),
        ];
        Dictionary<EquipmentSlotId, EquipmentSlotDefinition> slotDefinitions = slots.ToDictionary(slot => slot.Id);

        var store = new InventoryStore();
        store.RegisterInventory(new InventoryState(Owners[0], [new InventoryCapacityLimit(Mass, 120), new InventoryCapacityLimit(Bulk, 40)]));
        store.RegisterInventory(new InventoryState(Owners[1], [new InventoryCapacityLimit(Mass, 60)]));
        store.RegisterInventory(new InventoryState(Owners[2]));
        foreach (EntityId owner in Owners)
        {
            store.RegisterEquipment(new EquipmentState(owner));
        }

        var outcomes = new Dictionary<string, (int Applied, int Refused)>();
        var refusals = new HashSet<MechanicsRefusal>();
        InventoryEdit? edit = null;
        Snapshot? editBase = null;
        for (int step = 0; step < InventorySteps; step++)
        {
            // Grouped edits open now and then; their operations must not reach the live store
            // until Publish, and a live change in between must refuse that Publish.
            if (edit is null && random.Next(8) == 0)
            {
                editBase = Capture(store);
                edit = store.Prepare();
            }

            bool onEdit = edit is not null && random.Next(6) != 0;
            Func<EntityId, InventoryView> view = onEdit ? edit!.View : store.View;
            InventoryStore liveBefore = store;
            Snapshot before = Capture(store);
            Snapshot? editBefore = onEdit ? Capture(edit!) : null;
            int kind = random.Next(10);
            EntityId owner = Owners[random.Next(Owners.Length)];
            EntityId other = Owners[random.Next(Owners.Length)];
            InventoryView ownerView = view(owner);
            InventoryStackId stack = PickStack(random, owner, ownerView);
            InventoryStackId secondStack = PickStack(random, owner, ownerView);
            InventoryStackId otherStack = PickStack(random, other, view(other));
            EntityId item = PickItem(random, ownerView);
            EntityId swapIn = PickItem(random, ownerView);
            string name = kind switch
            {
                0 => "grant",
                1 => "consume",
                2 => "transfer-fungible",
                3 => "split",
                4 => "merge",
                5 => "materialize",
                6 => "transfer-unique",
                7 => random.Next(4) == 0 ? "destroy" : "set-costs",
                8 => "equip",
                _ => random.Next(2) == 0 ? "unequip" : "swap",
            };
            if (name is "swap" or "unequip" && random.Next(4) != 0
                && store.TryGetEquipment(owner, out EquipmentState? worn) && worn!.Assignments.Count != 0)
            {
                // The live assignments; an edit usually still holds them.
                item = worn.Assignments[random.Next(worn.Assignments.Count)].Item;
            }

            if (name == "materialize" && random.Next(4) != 0)
            {
                HashSet<EntityId> used = Owners.SelectMany(value => view(value).UniqueItems).Select(value => value.Entity).ToHashSet();
                item = Enumerable.Range(0, UniqueItemCount).Select(index => new EntityId(FirstItemEntity + (ulong)index))
                    .FirstOrDefault(value => !used.Contains(value), item);
            }

            ulong quantity = (ulong)random.Next(1, 9);
            ItemDefinition grantDefinition = fungible[random.Next(fungible.Length)];
            if (random.Next(4) != 0 && ownerView.Stacks.SingleOrDefault(value => value.Id == stack) is { Id: not null } existing)
            {
                grantDefinition = fungible.Single(definition => definition.Id == existing.Definition);
            }
            ItemDefinition uniqueDefinition = unique[random.Next(unique.Length)];
            ItemDefinition? equipping = ownerView.UniqueItems.Where(value => value.Entity == (name == "swap" ? swapIn : item))
                .Select(value => unique.Single(definition => definition.Id == value.Definition)).FirstOrDefault();
            EquipmentSlotDefinition[] requested = random.Next(4) != 0 && equipping?.Equipment is ItemEquipmentPolicy policy
                ? slots.Where(slot => slot.AllowedClassifications.Count == 0 || slot.AllowedClassifications.Any(equipping.Classifications.Contains))
                    .OrderBy(_ => random.Next()).Take(policy.RequiredSlots).ToArray()
                : slots.OrderBy(_ => random.Next()).Take(random.Next(1, 3)).ToArray();
            IReadOnlyList<ItemCapacityCost>? ownCosts = random.Next(3) switch
            {
                0 => null,
                1 => [],
                _ => [new ItemCapacityCost(random.Next(2) == 0 ? Mass : Bulk, (ulong)random.Next(1, 30))],
            };

            object? receipt;
            try
            {
                receipt = name switch
                {
                    "grant" => onEdit ? edit!.Grant(owner, grantDefinition, stack, quantity) : store.Grant(owner, grantDefinition, stack, quantity),
                    "consume" => onEdit ? edit!.Consume(owner, stack, quantity) : store.Consume(owner, stack, quantity),
                    "transfer-fungible" => onEdit
                        ? edit!.TransferFungible(owner, other, stack, otherStack, quantity)
                        : store.TransferFungible(owner, other, stack, otherStack, quantity),
                    "split" => onEdit
                        ? edit!.SplitFungible(owner, stack, StackId(owner, random.Next(StackIdsPerOwner)), quantity)
                        : store.SplitFungible(owner, stack, StackId(owner, random.Next(StackIdsPerOwner)), quantity),
                    "merge" => onEdit
                        ? edit!.MergeFungible(owner, stack, secondStack)
                        : store.MergeFungible(owner, stack, secondStack),
                    "materialize" => onEdit
                        ? edit!.MaterializeUnique(new ItemState(item, uniqueDefinition, ownCosts), owner)
                        : store.MaterializeUnique(new ItemState(item, uniqueDefinition, ownCosts), owner),
                    "transfer-unique" => onEdit ? edit!.TransferUnique(item, owner, other) : store.TransferUnique(item, owner, other),
                    "destroy" => onEdit ? edit!.DestroyUnique(item) : store.DestroyUnique(item),
                    "set-costs" => onEdit ? edit!.SetCapacityCosts(item, ownCosts) : store.SetCapacityCosts(item, ownCosts),
                    "equip" => onEdit ? edit!.Equip(owner, item, requested) : store.Equip(owner, item, requested),
                    "unequip" => onEdit ? edit!.Unequip(owner, item) : store.Unequip(owner, item),
                    _ => onEdit ? edit!.Swap(owner, item, swapIn, requested) : store.Swap(owner, item, swapIn, requested),
                };
            }
            catch (MechanicsException exception)
            {
                receipt = null;
                refusals.Add(exception.Reason);
                if (onEdit)
                {
                    // A failed operation discards the whole edit.
                    edit = null;
                    editBase = null;
                }
            }

            string key = $"{name}{(onEdit ? "@edit" : string.Empty)}";
            (int applied, int refused) = outcomes.GetValueOrDefault(key);
            outcomes[key] = receipt is null ? (applied, refused + 1) : (applied + 1, refused);
            Snapshot after = Capture(store);
            if (receipt is null || onEdit)
            {
                Require(after.Equals(before) && liveBefore == store,
                    $"step {step}: a refused or edit-only {name} changed the live store");
            }
            else
            {
                Require(after.StoreRevision == before.StoreRevision + 1, $"step {step}: {name} did not advance the store once");
                CheckReceipt(receipt, before, after, step);
            }

            if (receipt is not null && onEdit)
            {
                CheckReceipt(receipt, editBefore!, Capture(edit!), step);
            }

            RequireCoherent(store, slotDefinitions, $"step {step} after {key}");

            if (edit is not null && editBase is not null && random.Next(12) == 0)
            {
                Snapshot staged = Capture(edit);
                Snapshot live = Capture(store);
                bool liveChanged = live.StoreRevision != editBase.StoreRevision;
                Require(liveChanged || live.Equals(editBase), "the live store changed while only the edit was used");
                int close = random.Next(3);
                if (close == 0 && !liveChanged)
                {
                    store.Grant(Owners[2], fungible[2], InventoryStackId.Parse("delta-live-change"), 1);
                    live = Capture(store);
                    liveChanged = true;
                }

                if (close == 2)
                {
                    edit.Cancel();
                    Require(Capture(store).Equals(live), "a cancelled edit changed the live store");
                }
                else if (liveChanged)
                {
                    ExpectRefusal(MechanicsRefusal.RevisionConflict, edit.Publish, "an edit published over a live change");
                    Require(Capture(store).Equals(live), "a refused publish changed the live store");
                    outcomes["publish-conflict"] = (outcomes.GetValueOrDefault("publish-conflict").Applied + 1, 0);
                }
                else
                {
                    edit.Publish();
                    Require(Capture(store).Equals(staged), "a published edit did not leave exactly its staged state");
                    outcomes["publish"] = (outcomes.GetValueOrDefault("publish").Applied + 1, 0);
                }

                RequireCoherent(store, slotDefinitions, $"step {step} after the edit closed");
                edit = null;
                editBase = null;
            }
        }

        foreach (string kind in new[] { "grant", "consume", "transfer-fungible", "split", "merge", "materialize",
            "transfer-unique", "destroy", "set-costs", "equip", "unequip", "swap" })
        {
            Require(outcomes.GetValueOrDefault(kind).Applied > 0, $"no {kind} was applied");
            Require(outcomes.GetValueOrDefault(kind).Refused > 0, $"no {kind} was refused");
            Require(outcomes.GetValueOrDefault(kind + "@edit").Applied > 0, $"no {kind} was applied in an edit");
        }
        Require(outcomes.GetValueOrDefault("publish").Applied > 0 && outcomes.GetValueOrDefault("publish-conflict").Applied > 0,
            "edits were not both published and refused");
        foreach (MechanicsRefusal reason in new[] { MechanicsRefusal.Capacity, MechanicsRefusal.Occupied,
            MechanicsRefusal.Incompatible, MechanicsRefusal.StackMaximum, MechanicsRefusal.Equipped })
        {
            Require(refusals.Contains(reason), $"the sequence never exercised a {reason} refusal");
        }

        RequireRestoredCapacity(store);
        Drain(store, slotDefinitions);
    }

    /// <summary>Emptying every owner leaves only its limited metrics, each at zero.</summary>
    private static void Drain(InventoryStore store, IReadOnlyDictionary<EquipmentSlotId, EquipmentSlotDefinition> slots)
    {
        foreach (EntityId owner in Owners)
        {
            Require(store.TryGetEquipment(owner, out EquipmentState? equipment) && equipment is not null, "equipment");
            foreach (EntityId item in equipment!.Assignments.Select(value => value.Item).Distinct())
            {
                store.Unequip(owner, item);
                RequireCoherent(store, slots, "draining equipment");
            }
            foreach (UniqueInventoryItem item in store.View(owner).UniqueItems)
            {
                store.DestroyUnique(item.Entity);
                RequireCoherent(store, slots, "draining unique items");
            }
            foreach (InventoryStack stack in store.View(owner).Stacks)
            {
                store.Consume(owner, stack.Id, stack.Quantity);
                RequireCoherent(store, slots, "draining stacks");
            }

            Require(store.TryGetInventory(owner, out InventoryState? inventory) && inventory is not null, "inventory");
            Require(store.View(owner).Capacity.SequenceEqual(inventory!.CapacityLimits
                    .Select(limit => new CapacityUsage(limit.Metric, 0, limit.Maximum))),
                $"an emptied owner {owner.Value} still reports used capacity");
        }
    }

    private static InventoryStackId PickStack(Random random, EntityId owner, InventoryView view) =>
        random.Next(4) != 0 && view.Stacks.Count != 0
            ? view.Stacks[random.Next(view.Stacks.Count)].Id
            : StackId(owner, random.Next(StackIdsPerOwner));

    private static EntityId PickItem(Random random, InventoryView view) =>
        random.Next(4) != 0 && view.UniqueItems.Count != 0
            ? view.UniqueItems[random.Next(view.UniqueItems.Count)].Entity
            : new EntityId(FirstItemEntity + (ulong)random.Next(UniqueItemCount));

    private static InventoryStackId StackId(EntityId owner, int index) =>
        InventoryStackId.Parse($"delta-{owner.Value}-{index}");

    private static void CheckReceipt(object receipt, Snapshot before, Snapshot after, int step)
    {
        string at = $"step {step}";
        switch (receipt)
        {
            case InventoryMutationReceipt mutation:
                RequireCapacity(mutation.CapacityBefore, before.Inventories[mutation.Owner].Capacity, at);
                RequireCapacity(mutation.CapacityAfter, after.Inventories[mutation.Owner].Capacity, at);
                Require(mutation.InventoryRevisionAfter == mutation.InventoryRevisionBefore + 1
                    && mutation.InventoryRevisionAfter == after.Inventories[mutation.Owner].Revision, $"{at}: revision");
                break;
            case InventoryTransferReceipt transfer:
                RequireCapacity(transfer.FromCapacityBefore, before.Inventories[transfer.FromOwner].Capacity, at);
                RequireCapacity(transfer.FromCapacityAfter, after.Inventories[transfer.FromOwner].Capacity, at);
                RequireCapacity(transfer.ToCapacityBefore, before.Inventories[transfer.ToOwner].Capacity, at);
                RequireCapacity(transfer.ToCapacityAfter, after.Inventories[transfer.ToOwner].Capacity, at);
                break;
            case InventorySplitReceipt split:
                RequireCapacity(after.Inventories[split.Owner].Capacity, before.Inventories[split.Owner].Capacity, at);
                break;
            case InventoryMergeReceipt merge:
                RequireCapacity(after.Inventories[merge.Owner].Capacity, before.Inventories[merge.Owner].Capacity, at);
                break;
            case ItemMaterializationReceipt materialized:
                RequireCapacity(materialized.CapacityBefore, before.Inventories[materialized.Container].Capacity, at);
                RequireCapacity(materialized.CapacityAfter, after.Inventories[materialized.Container].Capacity, at);
                break;
            case ItemTransferReceipt moved:
                RequireCapacity(moved.FromCapacityBefore, before.Inventories[moved.FromOwner].Capacity, at);
                RequireCapacity(moved.FromCapacityAfter, after.Inventories[moved.FromOwner].Capacity, at);
                RequireCapacity(moved.ToCapacityBefore, before.Inventories[moved.ToOwner].Capacity, at);
                RequireCapacity(moved.ToCapacityAfter, after.Inventories[moved.ToOwner].Capacity, at);
                break;
            case ItemCapacityCostReceipt costs when costs.Container is EntityId container:
                RequireCapacity(costs.CapacityBefore, before.Inventories[container].Capacity, at);
                RequireCapacity(costs.CapacityAfter, after.Inventories[container].Capacity, at);
                break;
            case EquipmentMutationReceipt equipment when before.Equipment.ContainsKey(equipment.Owner):
                EquipmentAssignment[] previous = before.Equipment[equipment.Owner].Assignments;
                EquipmentAssignment[] current = after.Equipment[equipment.Owner].Assignments;
                Require(equipment.Changes.SequenceEqual(ReferenceChanges(previous, current)), $"{at}: slot changes");
                Require(equipment.EquipmentRevisionBefore == before.Equipment[equipment.Owner].Revision
                    && equipment.EquipmentRevisionAfter == after.Equipment[equipment.Owner].Revision, $"{at}: equipment revision");
                break;
        }
    }

    private static void RequireCapacity(IReadOnlyList<CapacityUsage> actual, CapacityUsage[] expected, string at) =>
        Require(actual.SequenceEqual(expected), $"{at}: receipt capacity differed from the store's");

    private static EquipmentSlotChange[] ReferenceChanges(EquipmentAssignment[] before, EquipmentAssignment[] after)
    {
        Dictionary<EquipmentSlotId, EntityId> old = before.ToDictionary(value => value.Slot, value => value.Item);
        Dictionary<EquipmentSlotId, EntityId> current = after.ToDictionary(value => value.Slot, value => value.Item);
        return old.Keys.Union(current.Keys)
            .OrderBy(slot => slot.Value, StringComparer.Ordinal)
            .Select(slot => new EquipmentSlotChange(
                slot,
                old.TryGetValue(slot, out EntityId was) ? was : null,
                current.TryGetValue(slot, out EntityId now) ? now : null))
            .Where(change => change.Before != change.After)
            .ToArray();
    }

    /// <summary>Every readout equals a fresh count from the store's plain contents, and every rule holds.</summary>
    private static void RequireCoherent(
        InventoryStore store,
        IReadOnlyDictionary<EquipmentSlotId, EquipmentSlotDefinition> slots,
        string at)
    {
        foreach (EntityId owner in Owners)
        {
            InventoryView view = store.View(owner);
            CapacityUsage[] expected = ReferenceCapacity(store, owner);
            Require(view.Capacity.SequenceEqual(expected), $"{at}: owner {owner.Value} capacity differed from a fresh count");
            Require(expected.All(usage => usage.Maximum is not ulong maximum || usage.Used <= maximum),
                $"{at}: owner {owner.Value} exceeded a capacity limit");

            Require(store.TryGetEquipment(owner, out EquipmentState? equipment) && equipment is not null, $"{at}: equipment");
            var byItem = equipment!.Assignments.GroupBy(value => value.Item).ToArray();
            var groups = new HashSet<EquipmentExclusivityId>();
            foreach (var assigned in byItem)
            {
                Require(store.TryGetContainer(assigned.Key, out EntityId container) && container == owner,
                    $"{at}: an equipped item is not contained by its owner");
                Require(store.TryGetItem(assigned.Key, out ItemState? item) && item!.Definition.Equipment is not null,
                    $"{at}: an equipped item is missing or unequippable");
                ItemEquipmentPolicy policy = item!.Definition.Equipment!;
                Require(assigned.Count() == policy.RequiredSlots, $"{at}: an equipped item holds the wrong slot count");
                foreach (EquipmentAssignment assignment in assigned)
                {
                    EquipmentSlotDefinition slot = slots[assignment.Slot];
                    Require(slot.AllowedClassifications.Count == 0
                        || slot.AllowedClassifications.Any(item.Definition.Classifications.Contains),
                        $"{at}: an equipped item sits in a slot that does not accept it");
                }
                Require(policy.ExclusiveGroup is not EquipmentExclusivityId group || groups.Add(group),
                    $"{at}: two equipped items share an exclusivity group");
            }
        }
    }

    private static CapacityUsage[] ReferenceCapacity(InventoryStore store, EntityId owner)
    {
        Require(store.TryGetInventory(owner, out InventoryState? inventory) && inventory is not null, "inventory");
        Dictionary<CapacityMetricId, ulong> used = [];
        foreach (InventoryStackCapture stack in inventory!.CaptureStacks())
        {
            Add(used, stack.Definition.CapacityCosts, stack.Quantity);
        }
        foreach (UniqueInventoryItem unique in store.View(owner).UniqueItems)
        {
            Require(store.TryGetItem(unique.Entity, out ItemState? item) && item is not null, "contained item");
            Add(used, item!.CapacityCosts, 1);
        }

        Dictionary<CapacityMetricId, ulong> limits = inventory.CapacityLimits.ToDictionary(limit => limit.Metric, limit => limit.Maximum);
        return used.Keys.Union(limits.Keys)
            .OrderBy(metric => metric.Value, StringComparer.Ordinal)
            .Select(metric => new CapacityUsage(
                metric,
                used.GetValueOrDefault(metric),
                limits.TryGetValue(metric, out ulong maximum) ? maximum : null))
            .ToArray();
    }

    private static void Add(Dictionary<CapacityMetricId, ulong> used, IReadOnlyList<ItemCapacityCost> costs, ulong quantity)
    {
        foreach (ItemCapacityCost cost in costs)
        {
            used[cost.Metric] = used.GetValueOrDefault(cost.Metric) + cost.Units * quantity;
        }
    }

    /// <summary>A save of the final store restores to the same stacks and capacity.</summary>
    private static void RequireRestoredCapacity(InventoryStore store)
    {
        var restored = new InventoryStore();
        foreach (EntityId owner in Owners)
        {
            Require(store.TryGetInventory(owner, out InventoryState? saved) && saved is not null, "saved inventory");
            restored.RegisterInventory(InventoryState.Restore(owner, saved!.CaptureStacks(), saved.CapacityLimits));
        }
        foreach (EntityId item in store.ItemEntities)
        {
            Require(store.TryGetItem(item, out ItemState? state) && state is not null, "saved item");
            if (store.TryGetContainer(item, out EntityId container))
            {
                restored.MaterializeUnique(new ItemState(item, state!.Definition, state.CapacityCostOverride), container);
            }
        }
        foreach (EntityId owner in Owners)
        {
            Require(restored.View(owner).Stacks.SequenceEqual(store.View(owner).Stacks)
                && restored.View(owner).Capacity.SequenceEqual(store.View(owner).Capacity),
                "a restored save did not reproduce the stacks and capacity");
        }
    }

    private static Snapshot Capture(InventoryStore store) => Capture(owner => store.View(owner), store.Revision,
        owner => store.TryGetEquipment(owner, out EquipmentState? state) ? state! : throw new InvalidOperationException("equipment"));

    private static Snapshot Capture(InventoryEdit edit) => Capture(edit.View, edit.Revision, null);

    private static Snapshot Capture(Func<EntityId, InventoryView> view, ulong revision, Func<EntityId, EquipmentState>? equipment)
    {
        var inventories = new Dictionary<EntityId, InventorySnapshot>();
        var equipped = new Dictionary<EntityId, EquipmentSnapshot>();
        foreach (EntityId owner in Owners)
        {
            InventoryView value = view(owner);
            inventories[owner] = new InventorySnapshot(value.InventoryRevision, [.. value.Stacks], [.. value.UniqueItems], [.. value.Capacity]);
            if (equipment is not null)
            {
                EquipmentState state = equipment(owner);
                equipped[owner] = new EquipmentSnapshot(state.Revision, [.. state.Assignments]);
            }
        }

        return new Snapshot(revision, inventories, equipped);
    }

    private sealed record InventorySnapshot(ulong Revision, InventoryStack[] Stacks, UniqueInventoryItem[] UniqueItems, CapacityUsage[] Capacity)
    {
        public bool Equals(InventorySnapshot? other) => other is not null && Revision == other.Revision
            && Stacks.SequenceEqual(other.Stacks) && UniqueItems.SequenceEqual(other.UniqueItems) && Capacity.SequenceEqual(other.Capacity);

        public override int GetHashCode() => Revision.GetHashCode();
    }

    private sealed record EquipmentSnapshot(ulong Revision, EquipmentAssignment[] Assignments)
    {
        public bool Equals(EquipmentSnapshot? other) => other is not null && Revision == other.Revision
            && Assignments.SequenceEqual(other.Assignments);

        public override int GetHashCode() => Revision.GetHashCode();
    }

    private sealed record Snapshot(
        ulong StoreRevision,
        Dictionary<EntityId, InventorySnapshot> Inventories,
        Dictionary<EntityId, EquipmentSnapshot> Equipment)
    {
        public bool Equals(Snapshot? other) => other is not null && StoreRevision == other.StoreRevision
            && Owners.All(owner => Inventories[owner].Equals(other.Inventories[owner]))
            // An edit's snapshot has no equipment readout; compare equipment when both have one.
            && (Equipment.Count == 0 || other.Equipment.Count == 0
                || Equipment.All(entry => entry.Value.Equals(other.Equipment[entry.Key])));

        public override int GetHashCode() => StoreRevision.GetHashCode();
    }

    private static void ExerciseEffects()
    {
        var random = new Random(Seed);
        SourceDefinitionId aura = SourceDefinitionId.Parse("delta-aura");
        StackingGroupId wards = StackingGroupId.Parse("delta-wards");
        StackingGroupId focus = StackingGroupId.Parse("delta-focus");
        StackingGroupId stances = StackingGroupId.Parse("delta-stances");
        EffectDefinition[] definitions =
        [
            new(EffectDefinitionId.Parse("delta-ward"), wards, EffectStackingPolicy.IndependentByProvenance, 3, 4, [aura]),
            new(EffectDefinitionId.Parse("delta-ward-b"), wards, EffectStackingPolicy.IndependentByProvenance, 3, 2),
            new(EffectDefinitionId.Parse("delta-focus"), focus, EffectStackingPolicy.Refresh, 0, 3, [aura]),
            // An independent definition in a refresh group: it may never join an active focus.
            new(EffectDefinitionId.Parse("delta-focus-intruder"), focus, EffectStackingPolicy.IndependentByProvenance, 3, 1),
            new(EffectDefinitionId.Parse("delta-stance"), stances, EffectStackingPolicy.Replace, 0, 2, [aura]),
            new(EffectDefinitionId.Parse("delta-stance-b"), stances, EffectStackingPolicy.Replace, 0, 1),
        ];
        var effects = new EffectsComponent(new EntityId(650));
        var counts = new Dictionary<EffectMutationKind, int>();
        int refused = 0;
        for (int step = 0; step < EffectSteps; step++)
        {
            ActiveEffect[] before = [.. effects.Effects];
            EffectDefinition definition = definitions[random.Next(definitions.Length)];
            EffectInstanceId instance = EffectInstanceId.Parse($"delta-effect-{random.Next(10)}");
            MechanicsSourceIdentity provenance = new IntrinsicSourceIdentity(new EntityId(650), SourceInstanceId.Parse($"delta-source-{random.Next(4)}"));
            ushort stacks = (ushort)random.Next(1, definition.MaximumStacks + 1);
            EffectMutationReceipt? receipt;
            try
            {
                receipt = random.Next(5) switch
                {
                    0 or 1 => effects.Apply(definition, instance, provenance, stacks),
                    2 => effects.Refresh(instance, provenance, 1),
                    3 => effects.Replace(definition, instance, provenance, stacks),
                    _ => random.Next(2) == 0 ? effects.Remove(instance) : effects.Expire(instance),
                };
            }
            catch (MechanicsException)
            {
                receipt = null;
                refused++;
            }

            ActiveEffect[] after = [.. effects.Effects];
            if (receipt is null)
            {
                Require(after.SequenceEqual(before), $"effect step {step}: a refusal changed the collection");
            }
            else
            {
                counts[receipt.Kind] = counts.GetValueOrDefault(receipt.Kind) + 1;
                Require(receipt.Removed.All(removed => before.Contains(removed) && !after.Any(effect => ReferenceEquals(effect, removed))),
                    $"effect step {step}: a removed effect is still active");
                Require(receipt.Current is null || after.Any(effect => ReferenceEquals(effect, receipt.Current)),
                    $"effect step {step}: the current effect is not active");
                Require(receipt.ActivatedSources.Count == (receipt.Current is null
                    ? 0
                    : receipt.Current.Stacks * receipt.Current.Definition.SourceDefinitions.Count),
                    $"effect step {step}: source activations");
                Require(after.Length == before.Length - receipt.Removed.Count + (receipt.Current is null ? 0 : 1),
                    $"effect step {step}: the collection changed beyond its receipt");
            }

            RequireEffectRules(after, $"effect step {step}");
        }

        foreach (EffectMutationKind kind in Enum.GetValues<EffectMutationKind>())
        {
            Require(counts.GetValueOrDefault(kind) > 0, $"no effect {kind} was applied");
        }
        Require(refused > 0, "no effect mutation was refused");
    }

    private static void RequireEffectRules(ActiveEffect[] effects, string at)
    {
        Require(effects.Select(effect => effect.Instance.Value).SequenceEqual(
            effects.Select(effect => effect.Instance.Value).Order(StringComparer.Ordinal).Distinct()),
            $"{at}: effects are not unique and in instance order");
        foreach (var group in effects.GroupBy(effect => effect.Definition.StackingGroup))
        {
            ActiveEffect[] members = [.. group];
            Require(members.All(effect => effect.Definition.Stacking == members[0].Definition.Stacking),
                $"{at}: group {group.Key} mixes stacking policies");
            if (members[0].Definition.Stacking == EffectStackingPolicy.IndependentByProvenance)
            {
                Require(members.Length <= members.Min(effect => effect.Definition.MaximumInstances)
                    && members.Select(effect => effect.Provenance).Distinct().Count() == members.Length,
                    $"{at}: independent group {group.Key} broke its limits");
            }
            else
            {
                Require(members.Length == 1, $"{at}: group {group.Key} holds more than one effect");
            }
        }
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
