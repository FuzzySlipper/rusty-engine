using Rusty.Engine;
using Rusty.Engine.Entities;
using Rusty.Engine.Mechanics;
using Rusty.Engine.Persistence;
using System.Text.Json.Serialization;

/// <summary>
/// Engine-owned proof for mechanics capture/rebuild: representative stats, effects, and
/// inventory survive a JSON/current-state roundtrip and rebuild into live, aliased behavior.
/// </summary>
/// <remarks>
/// Explicit product choices demonstrated here, not Engine policy: authored definitions
/// (items, slots, effects) are re-supplied; effect instances and provenance are fresh;
/// runtime-to-fresh identity mapping is a product-owned table (a save-local instance-key map and a
/// fresh-id counter stand in for it); stat sources are not captured. What the Engine owns:
/// shared Stat identity for track maximums, modifier values, and validation on rebuild.
/// </remarks>
internal static class MechanicsPersistenceExercise
{
    public static void Run()
    {
        ExerciseSharedRestore();
        // Live source session: one stat with a modifier, one track over it, one effect,
        // one stack grant, two identical swords and one multi-slot item.
        CapacityMetricId weight = CapacityMetricId.Parse("weight");
        ItemClassificationId weapon = ItemClassificationId.Parse("weapon");
        var potion = new ItemDefinition(
            ItemDefinitionId.Parse("potion"), ItemKind.Fungible, maximumQuantity: 20,
            capacityCosts: [new ItemCapacityCost(weight, 1)]);
        var sword = new ItemDefinition(
            ItemDefinitionId.Parse("sword"), ItemKind.Unique, maximumQuantity: 1,
            classifications: [weapon],
            capacityCosts: [new ItemCapacityCost(weight, 3)],
            equipment: new ItemEquipmentPolicy(1));
        var mainHand = new EquipmentSlotDefinition(EquipmentSlotId.Parse("main-hand"), [weapon]);
        var offHand = new EquipmentSlotDefinition(EquipmentSlotId.Parse("off-hand"), [weapon]);
        var backLeft = new EquipmentSlotDefinition(EquipmentSlotId.Parse("back-left"), [weapon]);
        var backRight = new EquipmentSlotDefinition(EquipmentSlotId.Parse("back-right"), [weapon]);
        var greatsword = new ItemDefinition(
            ItemDefinitionId.Parse("greatsword"), ItemKind.Unique, maximumQuantity: 1,
            classifications: [weapon], capacityCosts: [new ItemCapacityCost(weight, 5)],
            equipment: new ItemEquipmentPolicy(2));
        var burning = new EffectDefinition(
            EffectDefinitionId.Parse("burning"), StackingGroupId.Parse("fire"),
            EffectStackingPolicy.IndependentByProvenance, maximumInstances: 4, maximumStacks: 3);

        StatId mightId = StatId.Parse("might");
        TrackId healthId = TrackId.Parse("health");
        var might = new Stat(100, minimum: 20, maximum: 200);
        might.AddModifier(10);
        might.AddModifier(1.0, StatModifierKind.Multiply);
        var health = new Track(might, current: 80, minimum: 10,
            maximumChangePolicy: TrackMaximumChangePolicy.PreserveMissingAmount);
        var stats = new StatsComponent();
        stats.AddStat(mightId, might);
        stats.AddTrack(healthId, health);

        var effects = new EffectsComponent();
        EffectMutationReceipt applied = effects.Apply(
            burning, EffectInstanceId.Parse("burn-1"),
            new RequestSourceIdentity(OperationId.Parse("expedition-start"), SourceInstanceId.Parse("campfire")),
            stacks: 2);

        EntityId owner = new(501);
        var inventory = new InventoryStore();
        inventory.RegisterInventory(new InventoryState(owner, [new InventoryCapacityLimit(weight, 20)]));
        inventory.RegisterEquipment(new EquipmentState(owner));
        inventory.Grant(owner, potion, 3);
        EntityId swordEntity = new(511);
        inventory.MaterializeUnique(new ItemState(swordEntity, sword), owner);
        inventory.Equip(owner, swordEntity, [mainHand]);
        EntityId secondSwordEntity = new(512);
        EntityId greatswordEntity = new(513);
        inventory.MaterializeUnique(new ItemState(secondSwordEntity, sword), owner);
        inventory.MaterializeUnique(new ItemState(greatswordEntity, greatsword), owner);
        inventory.Equip(owner, secondSwordEntity, [offHand]);
        inventory.Equip(owner, greatswordEntity, [backLeft, backRight]);

        // Capture selected values: stats via the Engine helper; effects and inventory via
        // their existing read paths (Effects list, inventory View, equipment assignments).
        StatsComponentSnapshot statsCapture = StatsComponentCapture.Capture(stats);
        List<ActiveEffectCapture> effectCaptures = effects.Effects
            .Select(effect => new ActiveEffectCapture(effect.DefinitionId.Value, effect.Stacks))
            .ToList();
        InventoryView view = inventory.View(owner);
        if (!inventory.TryGetEquipment(owner, out EquipmentState? foundEquipment) || foundEquipment is null)
            throw new InvalidOperationException("equipment read did not resolve");
        if (!inventory.TryGetInventory(owner, out InventoryState? foundInventory) || foundInventory is null)
            throw new InvalidOperationException("inventory read did not resolve");
        EquipmentState equipment = foundEquipment;
        InventoryState state = foundInventory;
        var slotsByItem = equipment.Assignments.ToLookup(assignment => assignment.Item);
        // These keys only identify instances within this save, not definitions or new runtime IDs.
        var savedItemIds = view.UniqueItems.OrderBy(item => item.Entity.Value)
            .Select((item, index) => (item.Entity, Id: index + 1))
            .ToDictionary(item => item.Entity, item => item.Id);
        var save = new MechanicsSave(
            statsCapture,
            effectCaptures,
            new InventoryCapture(
                view.Stacks
                    .OrderBy(stack => stack.Definition.Value, StringComparer.Ordinal)
                    .Select(stack => new InventoryStackCapture(stack.Definition.Value, stack.Quantity))
                    .ToList(),
                view.UniqueItems
                    .OrderBy(item => item.Definition.Value, StringComparer.Ordinal)
                    .Select(item => new UniqueItemCapture(
                        savedItemIds[item.Entity], item.Definition.Value,
                        slotsByItem[item.Entity].Select(assignment => assignment.Slot.Value)
                            .OrderBy(slot => slot, StringComparer.Ordinal).ToList()))
                    .ToList(),
                state.CapacityLimits
                    .OrderBy(limit => limit.Metric.Value, StringComparer.Ordinal)
                    .Select(limit => new CapacityLimitCapture(limit.Metric.Value, limit.Maximum))
                    .ToList()));

        var persistence = new InMemoryPersistenceService();
        using var store = new ProductStateStore<MechanicsSave>(
            new PersistenceEngineContext(persistence), "mechanics-example",
            new JsonProductStateCodec<MechanicsSave>(MechanicsPersistenceJsonContext.Default.MechanicsSave));
        PersistenceSaveReceipt saved = store.Save("session", save);
        Require(saved.Outcome == PersistenceSaveOutcome.Saved,
            "the mechanics save did not report success");
        ProductStateLoad<MechanicsSave> loaded = store.Load("session");
        if (!loaded.Present || loaded.State is null)
            throw new InvalidOperationException("the mechanics save did not load");
        MechanicsSave saveState = loaded.State;

        // Fresh session: rebuild everything from the loaded values. Definitions are
        // re-supplied product data; instances, provenance, owners, and item entities are fresh.
        StatsComponent rebuiltStats = StatsComponentCapture.Rebuild(saveState.Stats);
        Stat rebuiltMight = rebuiltStats.GetStat(mightId);
        Track rebuiltHealth = rebuiltStats.GetTrack(healthId);
        Require(ReferenceEquals(rebuiltHealth.Maximum, rebuiltMight),
            "the rebuilt track does not share the rebuilt maximum stat");
        Require(rebuiltMight.Value == 110 && rebuiltHealth.Value == 80
            && rebuiltMight.Minimum == 20 && rebuiltMight.Maximum == 200
            && rebuiltMight.Modifiers.Count == 2
            && rebuiltMight.Modifiers[0].Amount == 10 && rebuiltMight.Modifiers[0].Kind == StatModifierKind.Add
            && rebuiltMight.Modifiers[1].Amount == 1.0 && rebuiltMight.Modifiers[1].Kind == StatModifierKind.Multiply
            && rebuiltHealth.Minimum == 10
            && rebuiltHealth.MaximumChangePolicy == TrackMaximumChangePolicy.PreserveMissingAmount,
            "the rebuilt stats did not preserve values, bounds, modifiers, and policy");

        var rebuiltEffects = new EffectsComponent();
        Require(saveState.Effects.Count == 1, "the effect capture did not survive");
        rebuiltEffects.Apply(
            burning, EffectInstanceId.Parse("burn-2"),
            new RequestSourceIdentity(OperationId.Parse("rebuild-op"), SourceInstanceId.Parse("rebuild-source")),
            saveState.Effects[0].Stacks);
        ActiveEffect rebuiltEffect = rebuiltEffects.Effects.Single();
        Require(rebuiltEffect.DefinitionId == EffectDefinitionId.Parse("burning") && rebuiltEffect.Stacks == 2
            && rebuiltEffect.Instance != applied.Current!.Instance,
            "the rebuilt effect did not re-apply with a fresh instance identity");

        EntityId freshOwner = new(502);
        var freshInventory = new InventoryStore();
        freshInventory.RegisterInventory(new InventoryState(
            freshOwner,
            saveState.Inventory.Limits
                .Select(limit => new InventoryCapacityLimit(CapacityMetricId.Parse(limit.Metric), limit.Maximum))
                .ToArray()));
        freshInventory.RegisterEquipment(new EquipmentState(freshOwner));
        Dictionary<string, ItemDefinition> definitions = new(StringComparer.Ordinal)
        {
            [potion.Id.Value] = potion,
            [sword.Id.Value] = sword,
            [greatsword.Id.Value] = greatsword,
        };
        foreach (InventoryStackCapture stack in saveState.Inventory.Stacks)
        {
            freshInventory.Grant(freshOwner, definitions[stack.Definition], stack.Quantity);
        }
        // Map saved instances to fresh runtime IDs; definitions are looked up separately.
        ulong nextItemValue = 601;
        var freshItems = new Dictionary<int, EntityId>();
        foreach (UniqueItemCapture unique in saveState.Inventory.Uniques)
        {
            EntityId fresh = new(nextItemValue++);
            freshItems.Add(unique.Id, fresh);
            freshInventory.MaterializeUnique(new ItemState(fresh, definitions[unique.Definition]), freshOwner);
        }
        Dictionary<string, EquipmentSlotDefinition> slots = new(StringComparer.Ordinal)
        {
            [mainHand.Id.Value] = mainHand,
            [offHand.Id.Value] = offHand,
            [backLeft.Id.Value] = backLeft,
            [backRight.Id.Value] = backRight,
        };
        foreach (UniqueItemCapture unique in saveState.Inventory.Uniques)
        {
            if (unique.Slots.Count > 0)
            {
                freshInventory.Equip(freshOwner, freshItems[unique.Id],
                    unique.Slots.Select(slot => slots[slot]).ToArray());
            }
        }
        InventoryView freshView = freshInventory.View(freshOwner);
        Require(freshView.Stacks.Single().Definition == ItemDefinitionId.Parse("potion")
            && freshView.Stacks.Single().Quantity == 3, "granted stacks did not rebuild");
        EntityId freshSword = freshItems[savedItemIds[swordEntity]];
        EntityId freshSecondSword = freshItems[savedItemIds[secondSwordEntity]];
        EntityId freshGreatsword = freshItems[savedItemIds[greatswordEntity]];
        Require(freshView.UniqueItems.Count == 3
            && freshView.UniqueItems.Count(item => item.Definition == sword.Id) == 2
            && freshSword != freshSecondSword
            && freshView.UniqueItems.All(item => !savedItemIds.ContainsKey(item.Entity)),
            "unique instances did not receive distinct fresh runtime identities");
        // Use the live facade for reads across mutations; EquipmentState readouts are detached.
        var freshEquipment = new EquipmentComponent(freshInventory, freshOwner);
        var assignments = freshEquipment.Assignments.ToDictionary(item => item.Slot, item => item.Item);
        Require(assignments.Count == 4 && assignments[mainHand.Id] == freshSword
            && assignments[offHand.Id] == freshSecondSword
            && assignments[backLeft.Id] == freshGreatsword
            && assignments[backRight.Id] == freshGreatsword,
            "restored assignments lost instance identity or multi-slot membership");
        freshEquipment.Unequip(freshSword);
        Require(freshEquipment.Assignments.Count == 3
            && freshEquipment.Assignments.Single(item => item.Slot == offHand.Id).Item == freshSecondSword,
            "unequipping one sword changed the other instance");
        freshEquipment.Unequip(freshGreatsword);
        Require(freshEquipment.Assignments.Count == 1,
            "unequipping the multi-slot item did not free both slots");
        freshEquipment.Equip(freshGreatsword, [mainHand, backLeft]);
        Require(freshEquipment.Assignments.Count == 3
            && freshEquipment.Assignments.Count(item => item.Item == freshGreatsword) == 2
            && equipment.Assignments.Count == 4,
            "restored equipment could not be used independently of the source inventory");

        // Reattach to a fresh entity and wrap: subsequent behavior proves live aliasing,
        // not byte equality.
        using var world = new EntityStore();
        EntityId hero = world.Create(new EntityTypeId("code:rebuilt/hero"));
        world.Add(hero, rebuiltStats);
        world.Add(hero, rebuiltEffects);
        var actor = new Actor(world, hero);
        Require(ReferenceEquals(actor.Get<StatsComponent>(), rebuiltStats), "the actor did not see the rebuilt stats");
        actor.Get<StatsComponent>().GetStat(mightId).BaseValue = 150;
        Require(actor.Get<StatsComponent>().GetTrack(healthId).MaximumValue == 160
            && actor.Get<StatsComponent>().GetTrack(healthId).Value == 130,
            "a rebuilt stat change did not reach the same rebuilt track with its policy");
        double spent = actor.Get<StatsComponent>().GetTrack(healthId).Spend(15);
        Require(spent == 15 && actor.Get<StatsComponent>().GetTrack(healthId).Value == 115
            && !actor.Get<StatsComponent>().GetTrack(healthId).TrySpend(1000),
            "the rebuilt track did not behave live after restore");
        Require(actor.Get<EffectsComponent>().Effects.Single().Stacks == 2,
            "the reattached effects were not visible through the actor");

        // A track whose maximum lives outside the captured component cannot rebuild shared
        // identity: capture fails loudly instead of duplicating the maximum.
        var outside = new Stat(50, minimum: 0, maximum: 100);
        var mixed = new StatsComponent();
        mixed.AddStat(StatId.Parse("other"), new Stat(5, minimum: 0, maximum: 10));
        mixed.AddTrack(TrackId.Parse("outsider"), new Track(outside, current: 10));
        ThrowsInvalidOperation(() => StatsComponentCapture.Capture(mixed),
            "an externally-owned track maximum did not fail capture");

        // A hand-edited snapshot naming an unknown maximum fails at rebuild, not silently.
        var unknownMaximum = new StatsComponentSnapshot(
            [new StatCapture("might", 100, 0, 200, 0,
                MidpointRounding.AwayFromZero, MidpointRounding.AwayFromZero, [])],
            [new TrackCapture("ghost", "no-such-stat", 10, 0,
                TrackMaximumChangePolicy.PreserveCurrent, 0,
                MidpointRounding.AwayFromZero, MidpointRounding.AwayFromZero)]);
        ThrowsInvalidOperation(() => StatsComponentCapture.Rebuild(unknownMaximum),
            "an unknown track maximum did not fail rebuild");
    }

    private static void ExerciseSharedRestore()
    {
        StatId primary = StatId.Parse("health-max");
        StatId alias = StatId.Parse("vitality");
        TrackId health = TrackId.Parse("health");
        TrackId healthAlias = TrackId.Parse("life");
        var source = new StatSource(
            new RequestSourceIdentity(OperationId.Parse("equip"), SourceInstanceId.Parse("belt")),
            SourceDefinitionId.Parse("belt"), 0,
            [new StatContributionDefinition(primary, StackingGroupId.Parse("bonus"),
                MechanicsStackingPolicy.Sum, new StatContribution.Add(50))]);

        // Test both previously throwing current > bare maximum and the subtler case
        // where applying sources after track creation would change an in-range current.
        foreach (double current in new[] { 140d, 80d })
        {
            var stat = new Stat(100);
            StatModifierHandle originalHandle = stat.AddModifier(10);
            stat.SetSources(primary, [source]);
            var track = new Track(stat, current: current,
                maximumChangePolicy: TrackMaximumChangePolicy.PreserveMissingAmount);
            var component = new StatsComponent();
            // Insert aliases first: selection and preservation must not depend on insertion order.
            component.AddStat(alias, stat);
            component.AddStat(primary, stat);
            component.AddTrack(healthAlias, track);
            component.AddTrack(health, track);

            StatsComponentSnapshot captured = StatsComponentCapture.Capture(component);
            var codec = new JsonProductStateCodec<StatsComponentSnapshot>(
                MechanicsPersistenceJsonContext.Default.StatsComponentSnapshot);
            var bytes = new System.Buffers.ArrayBufferWriter<byte>();
            codec.Encode(captured, bytes);
            StatsComponentSnapshot loaded = codec.Decode(bytes.WrittenSpan);
            StatModifierHandle? restoredHandle = null;
            int callbacks = 0;
            StatsComponent restored = StatsComponentCapture.Rebuild(loaded, (saved, rebuilt, handles) =>
            {
                callbacks++;
                Require(saved.Id == primary.Value && handles.Count == 1,
                    "restore callback must run once for the canonical stat with ordered handles");
                rebuilt.SetSources(primary, [source]);
                restoredHandle = handles[0];
            });
            Stat restoredStat = restored.GetStat(alias);
            Track restoredTrack = restored.GetTrack(healthAlias);
            Require(callbacks == 1 && ReferenceEquals(restoredStat, restored.GetStat(primary))
                && ReferenceEquals(restoredTrack, restored.GetTrack(health))
                && ReferenceEquals(restoredTrack.Maximum, restoredStat),
                "JSON restore broke stat, track, or maximum aliases");
            Require(restoredTrack.Current == current && restoredTrack.MaximumValue == 160,
                "restoring sources must not change captured current");
            restoredStat.BaseValue = 120;
            Require(restoredTrack.Current == current + 20 && track.Current == current,
                "restored aliases must share subsequent changes without changing the original");
            Require(!restoredStat.RemoveModifier(originalHandle)
                && restoredStat.RemoveModifier(restoredHandle!)
                && !restoredStat.RemoveModifier(restoredHandle!),
                "product must be able to remove the restored modifier with its fresh handle");
            Require(restoredTrack.MaximumValue == 170 && restoredTrack.Current == current + 10,
                "removing a restored modifier must update the same shared track exactly once");
            restoredTrack.Spend(5);
            Require(restored.GetTrack(health).Current == current + 5,
                "track aliases must share current-value changes");
        }
    }

    private static void Require(bool condition, string message)
    {
        if (!condition) throw new InvalidOperationException(message);
    }

    private static void ThrowsInvalidOperation(Action action, string message)
    {
        try { action(); }
        catch (Exception error) when (error.GetType() == typeof(InvalidOperationException)) { return; }
        throw new InvalidOperationException(message);
    }
}

internal sealed record ActiveEffectCapture(string Definition, ushort Stacks);

internal sealed record InventoryStackCapture(string Definition, ulong Quantity);

internal sealed record UniqueItemCapture(int Id, string Definition, List<string> Slots);

internal sealed record CapacityLimitCapture(string Metric, ulong Maximum);

internal sealed record InventoryCapture(
    List<InventoryStackCapture> Stacks,
    List<UniqueItemCapture> Uniques,
    List<CapacityLimitCapture> Limits);

internal sealed record MechanicsSave(
    StatsComponentSnapshot Stats,
    List<ActiveEffectCapture> Effects,
    InventoryCapture Inventory);

[JsonSerializable(typeof(MechanicsSave))]
internal partial class MechanicsPersistenceJsonContext : JsonSerializerContext
{
}
