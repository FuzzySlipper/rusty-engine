using Rusty.Engine.Entities;
using Rusty.Engine.StateMachine;

namespace Rusty.Engine.Mechanics;

internal static class ManagedLimitsExercise
{
    internal static void Run()
    {
        AcceptsProductSelectedIdentities();
        AcceptsEffectsBeyondFormerCollectionLimit();
        AcceptsInventoryDefinitionsAndCollectionsBeyondFormerLimits();
        RejectsInventoryArithmeticOverflow();
        AcceptsStateMachineBeyondFormerGraphLimits();
    }

    private static void AcceptsProductSelectedIdentities()
    {
        string longValue = $"Health Ω {new string('x', 96)}";
        StatId identity = StatId.Parse(longValue);
        Check(identity.Value == longValue, "managed identity was normalized or truncated");
        Check(StatId.Parse("Health") != StatId.Parse("health"), "managed identity equality is not ordinal and case-sensitive");
        ExpectArgument(() => StatId.Parse(string.Empty), "empty managed identity was admitted");
    }

    private static void AcceptsEffectsBeyondFormerCollectionLimit()
    {
        var definition = new EffectDefinition(
            EffectDefinitionId.Parse("many-effects"),
            StackingGroupId.Parse("many-effects"),
            EffectStackingPolicy.IndependentByProvenance,
            maximumInstances: 65,
            maximumStacks: ushort.MaxValue);
        var effects = new EffectsComponent(new EntityId(10));

        for (int index = 0; index < 65; index++)
        {
            effects.Apply(
                definition,
                EffectInstanceId.Parse($"effect-{index}"),
                new IntrinsicSourceIdentity(new EntityId(10), SourceInstanceId.Parse($"source-{index}")),
                stacks: 1);
        }

        Check(effects.Effects.Count == 65, "effects beyond the former managed cap were not retained");

        SourceDefinitionId source = SourceDefinitionId.Parse("max-stack-source");
        var maxStackDefinition = new EffectDefinition(
            EffectDefinitionId.Parse("max-stack"),
            StackingGroupId.Parse("max-stack"),
            EffectStackingPolicy.IndependentByProvenance,
            maximumInstances: 1,
            maximumStacks: ushort.MaxValue,
            [source]);
        EffectMutationReceipt maxStack = new EffectsComponent().Apply(
            maxStackDefinition,
            EffectInstanceId.Parse("max-stack-effect"),
            new IntrinsicSourceIdentity(new EntityId(10), SourceInstanceId.Parse("max-stack-item")),
            ushort.MaxValue);
        Check(maxStack.ActivatedSources.Count == ushort.MaxValue
            && ((EffectSourceIdentity)maxStack.ActivatedSources[^1].Identity).Stack == ushort.MaxValue,
            "maximum representable effect stacks did not activate once per stack");
    }

    private static void AcceptsInventoryDefinitionsAndCollectionsBeyondFormerLimits()
    {
        ItemClassificationId[] classifications = Enumerable.Range(0, 17)
            .Select(index => ItemClassificationId.Parse($"classification-{index}"))
            .ToArray();
        ItemCapacityCost[] costs = Enumerable.Range(0, 33)
            .Select(index => new ItemCapacityCost(CapacityMetricId.Parse($"cost-{index}"), 1))
            .ToArray();
        ItemDefinition definition = new(
            ItemDefinitionId.Parse("large-definition"),
            ItemKind.Fungible,
            maximumQuantity: 1_000_000_001,
            classifications: classifications,
            capacityCosts: costs,
            equipment: new ItemEquipmentPolicy(requiredSlots: 9));
        Check(definition.Classifications.Count == classifications.Length
            && definition.CapacityCosts.Count == costs.Length
            && definition.MaximumQuantity == 1_000_000_001,
            "item definition retained a former managed policy ceiling");

        EntityId owner = new(11);
        InventoryCapacityLimit[] limits = Enumerable.Range(0, 33)
            .Select(index => new InventoryCapacityLimit(CapacityMetricId.Parse($"limit-{index}"), 1))
            .ToArray();
        var inventory = new InventoryStore();
        inventory.RegisterInventory(new InventoryState(owner, limits));
        for (int index = 0; index < 129; index++)
        {
            inventory.Grant(
                owner,
                new ItemDefinition(ItemDefinitionId.Parse($"stack-{index}"), ItemKind.Fungible, 1),
                InventoryStackId.Parse($"instance-{index}"),
                1);
        }

        Check(inventory.View(owner).Stacks.Count == 129
            && inventory.View(owner).Capacity.Count == limits.Length,
            "inventory retained a former managed collection ceiling");
    }

    private static void RejectsInventoryArithmeticOverflow()
    {
        CapacityMetricId metric = CapacityMetricId.Parse("overflow");
        EntityId owner = new(12);
        var inventory = new InventoryStore();
        inventory.RegisterInventory(new InventoryState(owner, [new InventoryCapacityLimit(metric, ulong.MaxValue)]));
        ItemDefinition definition = new(
            ItemDefinitionId.Parse("overflowing-item"),
            ItemKind.Fungible,
            ulong.MaxValue,
            capacityCosts: [new ItemCapacityCost(metric, ulong.MaxValue)]);

        InventoryStackId stack = InventoryStackId.Parse("overflowing-stack");
        ExpectMechanicsError(
            () => inventory.Grant(owner, definition, stack, 2),
            "overflowing capacity arithmetic was admitted");
        Check(inventory.View(owner).Stacks.Count == 0, "overflowing grant changed inventory state");
    }

    private static void AcceptsStateMachineBeyondFormerGraphLimits()
    {
        ulong[] states = Enumerable.Range(0, 257).Select(index => (ulong)index).ToArray();
        StateMachineTransition[] transitions = Enumerable.Range(0, 1_025)
            .Select(index => new StateMachineTransition((ulong)(index % states.Length), (ulong)(index / states.Length)))
            .ToArray();
        var definition = new StateMachineDefinition(1, states, transitions);

        Check(definition.States.Count == states.Length
            && definition.Transitions.Count == transitions.Length,
            "state-machine graph retained a former managed policy ceiling");
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

    private static void Check(bool condition, string message)
    {
        if (!condition)
        {
            throw new InvalidOperationException(message);
        }
    }
}
