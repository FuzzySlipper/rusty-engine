using System;
using System.Globalization;
using System.Text.RegularExpressions;
using Rusty.Engine.Debugging;
using Rusty.Engine.Entities;
using Rusty.Engine.Mechanics;

namespace EntityStoreDebugFixture;

internal static class MechanicsDebugExercise
{
    internal static void Run()
    {
        using var original = new EntityStore();
        var entity = original.Create();
        var maximum = new Stat(100);
        var track = new Track(maximum, current: 70);
        var stats = new StatsComponent();
        stats.AddStat(StatId.Parse("health-max"), maximum);
        stats.AddTrack(TrackId.Parse("health"), track);
        original.Add(entity, stats);
        var effects = new EffectsComponent(entity);
        original.Add(entity, effects);
        var inventoryStore = new InventoryStore();
        inventoryStore.RegisterInventory(new InventoryState(entity));
        inventoryStore.RegisterEquipment(new EquipmentState(entity));
        var inventory = new InventoryComponent(inventoryStore, entity);
        var equipment = new EquipmentComponent(inventoryStore, entity);
        original.Add(entity, inventory);
        original.Add(entity, equipment);

        var debug = new EntityStoreDebugModule();
        debug.RegisterStore("game", original);
        debug.RegisterMechanicsProjections(maximumEntries: 1);
        string Read(string type) => ReadComponent(debug, entity, type);
        ulong revision = original.Revision;
        Require(Read(nameof(StatsComponent)).Contains("current=70"), "initial stats projection");
        maximum.BaseValue = 50;
        Require(original.Revision == revision && Read(nameof(StatsComponent)).Contains("current=50"), "in-place changes must bypass structural revision caching");
        var definition = new EffectDefinition(EffectDefinitionId.Parse("ward"), StackingGroupId.Parse("ward"), EffectStackingPolicy.Refresh, 1, 2);
        var effect = EffectInstanceId.Parse("ward-one");
        effects.Apply(definition, effect, new IntrinsicSourceIdentity(entity, SourceInstanceId.Parse("self")), 1);
        Require(Read(nameof(EffectsComponent)).Contains("stacks=1"), "live effect apply");
        effects.Expire(effect);
        Require(Read(nameof(EffectsComponent)).Contains("effects=0"), "live effect expiry");
        var item = new ItemDefinition(ItemDefinitionId.Parse("potion"), ItemKind.Fungible, 10);
        var potionStack = InventoryStackId.Parse("carried-potions");
        inventory.Grant(item, potionStack, 3);
        Require(Read(nameof(InventoryComponent)).Contains("quantity=3"), "live inventory grant");
        using (var edit = inventoryStore.Prepare()) { edit.Consume(entity, potionStack, 2); edit.Publish(); }
        Require(Read(nameof(InventoryComponent)).Contains("quantity=1"), "live facade after edit publication");
        var weaponEntity = new EntityId(100);
        var weapon = new ItemDefinition(ItemDefinitionId.Parse("blade"), ItemKind.Unique, 1, equipment: new ItemEquipmentPolicy(1));
        inventory.MaterializeUnique(new ItemState(weaponEntity, weapon));
        equipment.Equip(weaponEntity, [new EquipmentSlotDefinition(EquipmentSlotId.Parse("hand"))]);
        Require(Read(nameof(EquipmentComponent)).Contains("slot=hand:item=100"), "live equipment projection");
        stats.AddStat(StatId.Parse("speed"), new Stat(2));
        Require(Read(nameof(StatsComponent)).Contains("entries-truncated=true"), "mechanics entry bounds");

        using var replacement = new EntityStore();
        var next = replacement.Create();
        // Different automatic family order: the projection must follow the CLR type, not the old key.
        replacement.Add(next, new EffectsComponent(next));
        var nextStats = new StatsComponent();
        nextStats.AddStat(StatId.Parse("health-max"), new Stat(9));
        replacement.Add(next, nextStats);
        debug.ReplaceStore("game", replacement);
        original.Dispose();
        Require(ReadComponent(debug, next, nameof(StatsComponent)).Contains("value=9"), "replacement store with different keys");
        replacement.Remove<StatsComponent>(next);
        Require(!debug.GetEntity("game", next.Value).Message.Contains("type=StatsComponent"), "removed component not inspected");
        debug.UnregisterStore("game");
        Require(debug.GetEntity("game", next.Value).Status == DebugCommandStatus.InvalidArguments, "unregistered store not inspected");
    }

    private static string ReadComponent(EntityStoreDebugModule debug, EntityId entity, string type)
    {
        string metadata = debug.GetEntity("game", entity.Value).Message;
        var match = Regex.Match(metadata, @"component=(\d+):revision=\d+:type=" + type);
        Require(match.Success, "component metadata missing for " + type);
        var result = debug.GetComponent("game", entity.Value, uint.Parse(match.Groups[1].Value, CultureInfo.InvariantCulture));
        Require(result.Succeeded && result.Message.Length <= EntityStoreDebugModule.MaximumResultLength, "bounded projection failed");
        return result.Message;
    }

    private static void Require(bool condition, string message)
    {
        if (!condition) throw new InvalidOperationException(message);
    }
}
