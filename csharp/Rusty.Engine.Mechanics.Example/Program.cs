using Rusty.Engine.Entities;
using Rusty.Engine.Mechanics;

ExerciseTypedIds();
StatExercise.Run();
TrackExercise.Run();
StatsComponentExercise.Run();
MechanicsComponentsExercise.Run();
ExerciseEffectPolicies();
ExerciseManagedInventory();

static void ExerciseTypedIds()
{
    StatId strength = StatId.Parse("strength");
    Require(!StatId.TryParse("Strength", out _), "invalid typed identity was admitted");
    Require(strength.Value == "strength", "typed identity lost its value");
}

static void ExerciseEffectPolicies()
{
    SourceDefinitionId auraSource = SourceDefinitionId.Parse("aura-source");
    EffectDefinition independent = new(
        EffectDefinitionId.Parse("ward"),
        StackingGroupId.Parse("wards"),
        EffectStackingPolicy.IndependentByProvenance,
        maximumInstances: 2,
        maximumStacks: 3,
        [auraSource]);
    EffectsComponent independentState = new(new EntityId(1));
    independentState.Apply(
        independent,
        EffectInstanceId.Parse("ward-one"),
        new IntrinsicSourceIdentity(new EntityId(1), SourceInstanceId.Parse("item-one")),
        stacks: 2);
    EffectMutationReceipt second = independentState.Apply(
        independent,
        EffectInstanceId.Parse("ward-two"),
        new IntrinsicSourceIdentity(new EntityId(1), SourceInstanceId.Parse("item-two")),
        stacks: 1);
    Require(second.ActivatedSources.Count == 1, "effect source activation count was incorrect");
    ExpectMechanicsError(
        () => independentState.Apply(
            independent,
            EffectInstanceId.Parse("ward-three"),
            new IntrinsicSourceIdentity(new EntityId(1), SourceInstanceId.Parse("item-one")),
            stacks: 1),
        "independent provenance conflict was ignored");

    EffectDefinition refresh = new(
        EffectDefinitionId.Parse("focus"),
        StackingGroupId.Parse("focus"),
        EffectStackingPolicy.Refresh,
        maximumInstances: 0,
        maximumStacks: 3);
    EffectsComponent refreshState = new();
    refreshState.Apply(
        refresh,
        EffectInstanceId.Parse("focus-instance"),
        new RequestSourceIdentity(OperationId.Parse("cast-one"), SourceInstanceId.Parse("cast")),
        stacks: 1);
    EffectMutationReceipt refreshed = refreshState.Refresh(
        EffectInstanceId.Parse("focus-instance"),
        new RequestSourceIdentity(OperationId.Parse("cast-two"), SourceInstanceId.Parse("cast")),
        stacks: 3);
    Require(refreshed.Kind == EffectMutationKind.Refresh
        && refreshState.Effects.Single().Stacks == 3,
        "refresh did not replace the existing activation");

    EffectDefinition replace = new(
        EffectDefinitionId.Parse("stance"),
        StackingGroupId.Parse("stances"),
        EffectStackingPolicy.Replace,
        maximumInstances: 0,
        maximumStacks: 1);
    EffectsComponent replaceState = new();
    replaceState.Apply(
        replace,
        EffectInstanceId.Parse("stance-old"),
        new RequestSourceIdentity(OperationId.Parse("stance"), SourceInstanceId.Parse("old")),
        stacks: 1);
    EffectMutationReceipt replaced = replaceState.Replace(
        replace,
        EffectInstanceId.Parse("stance-new"),
        new RequestSourceIdentity(OperationId.Parse("stance"), SourceInstanceId.Parse("new")),
        stacks: 1);
    Require(replaced.Removed.Count == 1 && replaceState.Effects.Single().Instance.Value == "stance-new",
        "replace did not remove the prior group member");
    EffectMutationReceipt expired = replaceState.Expire(EffectInstanceId.Parse("stance-new"));
    Require(expired.Kind == EffectMutationKind.Expire && replaceState.Effects.Count == 0,
        "explicit effect expiry was not caller-driven");
}

static void ExerciseManagedInventory()
{
    const ulong Owner = 1;
    const ulong SecondOwner = 2;
    const ulong RifleEntity = 10;
    const ulong ShieldEntity = 11;

    CapacityMetricId mass = CapacityMetricId.Parse("mass");
    ItemDefinition ammunition = new(
        ItemDefinitionId.Parse("ammunition"),
        ItemKind.Fungible,
        maximumQuantity: 100,
        capacityCosts: [new ItemCapacityCost(mass, 1)]);
    ItemDefinition rifle = new(
        ItemDefinitionId.Parse("rifle"),
        ItemKind.Unique,
        maximumQuantity: 1,
        classifications: [ItemClassificationId.Parse("weapon")],
        capacityCosts: [new ItemCapacityCost(mass, 8)],
        equipment: new ItemEquipmentPolicy(
            requiredSlots: 2,
            EquipmentExclusivityId.Parse("weapons")),
        sourceDefinitions: [SourceDefinitionId.Parse("precision")]);
    ItemDefinition shield = new(
        ItemDefinitionId.Parse("shield"),
        ItemKind.Unique,
        maximumQuantity: 1,
        classifications: [ItemClassificationId.Parse("shield")],
        capacityCosts: [new ItemCapacityCost(mass, 6)],
        equipment: new ItemEquipmentPolicy(
            requiredSlots: 1,
            EquipmentExclusivityId.Parse("weapons")));
    EquipmentSlotDefinition leftHand = new(
        EquipmentSlotId.Parse("hand-left"),
        [ItemClassificationId.Parse("weapon")]);
    EquipmentSlotDefinition rightHand = new(
        EquipmentSlotId.Parse("hand-right"),
        [ItemClassificationId.Parse("weapon")]);
    EquipmentSlotDefinition shieldHand = new(
        EquipmentSlotId.Parse("shield-hand"),
        [ItemClassificationId.Parse("shield")]);

    var world = new InventoryStore();
    world.RegisterInventory(new InventoryState(
        new EntityId(Owner),
        [new InventoryCapacityLimit(mass, 18)]));
    world.RegisterInventory(new InventoryState(
        new EntityId(SecondOwner),
        [new InventoryCapacityLimit(mass, 20)]));
    world.RegisterEquipment(new EquipmentState(new EntityId(Owner)));
    world.RegisterEquipment(new EquipmentState(new EntityId(SecondOwner)));

    InventoryMutationReceipt granted = world.Grant(new EntityId(Owner), ammunition, 5);
    Require(granted.AfterQuantity == 5, "managed fungible grant was not applied");
    Require(world.View(new EntityId(Owner)).Stacks.Single().Quantity == 5,
        "managed stacks were not exposed canonically");

    ItemMaterializationReceipt materialized = world.MaterializeUnique(
        new ItemState(new EntityId(RifleEntity), rifle),
        new EntityId(Owner));
    Require(materialized.CapacityAfter.Single().Used == 13,
        "managed unique item capacity was not included");

    EquipmentMutationReceipt equipped = world.Equip(new EntityId(Owner),
        new EntityId(RifleEntity),
        [leftHand, rightHand]);
    Require(equipped.SourceActivations.Count == 1
        && equipped.SourceActivations[0].Identity.Item == new EntityId(RifleEntity),
        "equipped item source was not activated once per item");

    ulong beforeRejectedCapacity = world.Revision;
    ExpectMechanicsError(
        () => world.MaterializeUnique(
            new ItemState(new EntityId(ShieldEntity), shield),
            new EntityId(Owner)),
        "managed capacity rejection was not atomic");
    Require(world.Revision == beforeRejectedCapacity
        && !world.TryGetItem(new EntityId(ShieldEntity), out _),
        "rejected materialization changed managed world state");

    ulong beforeEquippedTransfer = world.Revision;
    ExpectMechanicsError(
        () => world.TransferUnique(new EntityId(RifleEntity),
            new EntityId(Owner),
            new EntityId(SecondOwner)),
        "equipped unique item transfer was not blocked");
    Require(world.Revision == beforeEquippedTransfer
        && world.TryGetContainer(new EntityId(RifleEntity), out EntityId owner)
        && owner == new EntityId(Owner),
        "rejected equipped transfer changed containment");

    InventoryEdit transfer = world.Prepare();
    transfer.Unequip(new EntityId(Owner), new EntityId(RifleEntity));
    ItemTransferReceipt moved = transfer.TransferUnique(
        new EntityId(RifleEntity),
        new EntityId(Owner),
        new EntityId(SecondOwner));
    transfer.Publish();
    Require(moved.ToCapacityAfter.Single().Used == 8
        && world.TryGetContainer(new EntityId(RifleEntity), out EntityId newOwner)
        && newOwner == new EntityId(SecondOwner),
        "detached unequip and transfer did not publish together");
    ExpectInvalidOperation(
        () => transfer.View(new EntityId(SecondOwner)),
        "published inventory edit retained its detached state");

    InventoryEdit rejectedEdit = world.Prepare();
    rejectedEdit.Grant(new EntityId(Owner), ammunition, 1);
    ExpectMechanicsError(
        () => rejectedEdit.Consume(new EntityId(Owner), ammunition, 10),
        "rejected edit operation was accepted");
    ExpectInvalidOperation(
        rejectedEdit.Publish,
        "failed inventory edit published staged changes");
    Require(world.View(new EntityId(Owner)).Stacks.Single().Quantity == 5,
        "failed inventory edit changed the live owner");

    InventoryEdit staleEdit = world.Prepare();
    staleEdit.Grant(new EntityId(Owner), ammunition, 1);
    world.Grant(new EntityId(Owner), ammunition, 1);
    ExpectMechanicsError(staleEdit.Publish, "stale inventory edit was published");
    ExpectInvalidOperation(staleEdit.Publish, "stale inventory edit was retried");
    Require(world.View(new EntityId(Owner)).Stacks.Single().Quantity == 6,
        "stale inventory edit overwrote current state");

    InventoryEdit discardedEdit = world.Prepare();
    discardedEdit.Dispose();
    ExpectInvalidOperation(
        () => discardedEdit.Grant(new EntityId(Owner), ammunition, 1),
        "disposed inventory edit remained usable");

    InventoryEdit cancelledEdit = world.Prepare();
    cancelledEdit.Cancel();
    ExpectInvalidOperation(
        () => cancelledEdit.View(new EntityId(Owner)),
        "cancelled inventory edit retained its detached state");

    world.Equip(new EntityId(SecondOwner),
        new EntityId(RifleEntity),
        [leftHand, rightHand]);

    ItemMaterializationReceipt secondShield = world.MaterializeUnique(
        new ItemState(new EntityId(ShieldEntity), shield),
        new EntityId(SecondOwner));
    Require(secondShield.CapacityAfter.Single().Used == 14,
        "second-owner capacity was not maintained");
    ExpectMechanicsError(
        () => world.Equip(new EntityId(SecondOwner),
            new EntityId(ShieldEntity),
            [shieldHand]),
        "exclusivity or containment validation was not enforced");
    Require(world.TryGetEquipment(new EntityId(SecondOwner), out EquipmentState? equipment)
        && equipment is not null
        && equipment.Assignments.Count == 2
        && equipment.Assignments.All(assignment => assignment.Item == new EntityId(RifleEntity)),
        "rejected equipment changed state");

    ItemDestroyReceipt destroyed = world.DestroyUnique(new EntityId(ShieldEntity));
    Require(destroyed.FormerOwner == new EntityId(SecondOwner)
        && !world.TryGetItem(new EntityId(ShieldEntity), out _),
        "explicit unique destruction did not remove the item");

    var boundedWorld = new InventoryStore();
    EntityId boundedOwner = new(100);
    boundedWorld.RegisterInventory(new InventoryState(boundedOwner));
    for (int index = 0; index < ManagedInventoryLimits.MaximumStacksPerInventory; index++)
    {
        boundedWorld.Grant(
            boundedOwner,
            new ItemDefinition(
                ItemDefinitionId.Parse($"stack-{index}"),
                ItemKind.Fungible,
                maximumQuantity: 1),
            quantity: 1);
    }

    ulong beforeRejectedStack = boundedWorld.Revision;
    ExpectMechanicsError(
        () => boundedWorld.Grant(
            boundedOwner,
            new ItemDefinition(
                ItemDefinitionId.Parse("stack-overflow"),
                ItemKind.Fungible,
                maximumQuantity: 1),
            quantity: 1),
        "managed inventory stack limit was not enforced");
    Require(boundedWorld.Revision == beforeRejectedStack
        && boundedWorld.View(boundedOwner).Stacks.Count == ManagedInventoryLimits.MaximumStacksPerInventory,
        "rejected stack insertion changed managed inventory state");
}

static void ExpectMechanicsError(Action action, string message)
{
    try
    {
        action();
    }
    catch (Exception exception) when (
        exception is MechanicsException or ArgumentException or OverflowException)
    {
        return;
    }

    throw new InvalidOperationException(message);
}

static void ExpectInvalidOperation(Action action, string message)
{
    try
    {
        action();
    }
    catch (InvalidOperationException)
    {
        return;
    }

    throw new InvalidOperationException(message);
}

static void Require(bool condition, string message)
{
    if (!condition)
    {
        throw new InvalidOperationException(message);
    }
}
