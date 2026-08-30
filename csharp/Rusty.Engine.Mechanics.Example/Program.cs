using Rusty.Engine.Entities;
using Rusty.Engine.Mechanics;

const long BaseStrength = 40;
const long EquipmentBonus = 4;
const long BlessingBonus = 6;
const long ResourceStart = 30;
const long ResourceSpend = 10;
const double ContinuousBase = 0.5;
const double ContinuousBonus = 0.25;

ExerciseTypedValues();
ExerciseExactStatEvaluation();
ExerciseExactStatTrackSourceChanges();
ExerciseContinuousStatEvaluation();
ExerciseExactTrackAtomicity();
ExerciseContinuousTrackAtomicity();
ExerciseEffectPolicies();
ExerciseManagedInventory();

static void ExerciseTypedValues()
{
    StatId strength = StatId.Parse("strength");
    Require(!StatId.TryParse("Strength", out _), "invalid typed identity was admitted");
    Require(strength.Value == "strength", "typed identity lost its value");

    ExactRatio ratio = new(2, 4);
    Require(ratio.Numerator == 1 && ratio.Denominator == 2, "ratio was not normalized");
    Require(ratio.Apply(new ExactValue(11)).Raw == 5, "exact ratio did not round toward zero once");
    ExactRatio invalidDefaultRatio = default;
    ExpectMechanicsError(
        () => ExactRatioProduct.One.Include(invalidDefaultRatio),
        "default exact ratio was accepted during composition");
    ExpectMechanicsError(
        () => invalidDefaultRatio.Apply(new ExactValue(1)),
        "default exact ratio was accepted during application");

    ContinuousValue negativeZero = ContinuousValue.FromBits(0x8000_0000_0000_0000);
    Require(negativeZero.Bits == 0, "continuous negative zero was not normalized");
    ExpectMechanicsError(
        () => ContinuousValue.FromBits(0x7ff0_0000_0000_0000),
        "non-finite continuous value was admitted");
}

static void ExerciseExactStatEvaluation()
{
    StatId strength = StatId.Parse("strength");
    StackingGroupId additions = StackingGroupId.Parse("strength-additions");
    StackingGroupId scale = StackingGroupId.Parse("strength-scale");
    ExactStatDefinition definition = new(strength, new ExactValue(0), new ExactValue(100));
    ExactSource equipment = new(
        new EquippedItemSourceIdentity(new EntityId(1), new EntityId(2), SourceDefinitionId.Parse("equipment")),
        SourceDefinitionId.Parse("equipment"),
        priority: 10,
        [new ExactStatContributionDefinition(
            strength,
            additions,
            MechanicsStackingPolicy.Sum,
            new ExactStatContribution.Add(new ExactValue(EquipmentBonus)))]);
    ExactSource blessing = new(
        new IntrinsicSourceIdentity(new EntityId(1), SourceInstanceId.Parse("blessing")),
        SourceDefinitionId.Parse("blessing"),
        priority: 20,
        [
            new ExactStatContributionDefinition(
                strength,
                additions,
                MechanicsStackingPolicy.Sum,
                new ExactStatContribution.Add(new ExactValue(BlessingBonus))),
            new ExactStatContributionDefinition(
                strength,
                scale,
                MechanicsStackingPolicy.Highest,
                new ExactStatContribution.Scale(new ExactRatio(3, 2))),
        ]);

    ExactStatEvaluation evaluation = ExactStatEvaluator.Evaluate(
        definition,
        new ExactValue(BaseStrength),
        [blessing, equipment]);
    Require(evaluation.Value.Raw == 75, "exact stat modifiers did not combine deterministically");
    Require(evaluation.Decisions[0].SourceDefinition.Value == "equipment",
        "source order did not use priority before identity");
    Require(evaluation.Decisions.All(decision => decision.Outcome == MechanicsDecisionOutcome.Applied),
        "applicable exact contributions were unexpectedly suppressed");

    ExpectMechanicsError(
        () => ExactStatEvaluator.Evaluate(definition, new ExactValue(BaseStrength), [equipment, equipment]),
        "duplicate source identity was silently accepted");
}

static void ExerciseContinuousStatEvaluation()
{
    StatId accuracy = StatId.Parse("accuracy");
    ContinuousStatDefinition definition = new(
        accuracy,
        new ContinuousValue(0.0),
        new ContinuousValue(2.0));
    ContinuousSource source = new(
        new RequestSourceIdentity(OperationId.Parse("aim"), SourceInstanceId.Parse("focus")),
        SourceDefinitionId.Parse("focus"),
        priority: 1,
        [new ContinuousStatContributionDefinition(
            accuracy,
            StackingGroupId.Parse("accuracy-additions"),
            MechanicsStackingPolicy.Sum,
            new ContinuousStatContribution.Add(new ContinuousValue(ContinuousBonus)))]);

    ContinuousStatEvaluation evaluation = ContinuousStatEvaluator.Evaluate(
        definition,
        new ContinuousValue(ContinuousBase),
        [source]);
    Require(evaluation.Value.Value == 0.75, "continuous stat addition was not retained");
}

static void ExerciseExactStatTrackSourceChanges()
{
    StatId maximum = StatId.Parse("health-maximum");
    ExactStatDefinition stat = new(maximum, new ExactValue(0), new ExactValue(1_000));
    ExactTrackDefinition track = new(
        TrackId.Parse("health"),
        new ExactValue(0),
        new ExactTrackMaximum.FromStat(maximum));
    ExactStatTrackState state = new(
        stat,
        new ExactValue(100),
        [],
        track,
        new ExactValue(60));

    ExactStatTrackChangeCandidate preserve = state.PrepareSourceChange(
        new ExactValue(120),
        [],
        ExactStatTrackCurrentPolicy.PreserveCurrent,
        expectedRevision: 0);
    Require(state.Revision == 0 && state.Read().TrackBounds.Maximum.Raw == 100,
        "stat-track preview mutated live state");
    Require(preserve.Preview.After.Stat.Base.Raw == 120
        && preserve.Preview.After.TrackBounds.Maximum.Raw == 120
        && preserve.Preview.After.TrackCurrent.Raw == 60,
        "stat-track preserve preview was incorrect");
    ExactStatTrackChangeReceipt preserved = preserve.Publish();
    Require(preserved.Before.Revision == 0 && preserved.After.Revision == 1
        && preserved.After.TrackCurrent.Raw == 60,
        "stat-track preserve publish receipt was incorrect");

    ExactStatTrackChangeCandidate stale = state.PrepareSourceChange(
        new ExactValue(130),
        [],
        ExactStatTrackCurrentPolicy.PreserveCurrent,
        expectedRevision: 1);
    ExactStatTrackChangeReceipt increased = state.ApplySourceChange(
        new ExactValue(150),
        [],
        ExactStatTrackCurrentPolicy.PreserveDistanceFromMaximum,
        expectedRevision: 1);
    Require(increased.After.TrackCurrent.Raw == 90
        && increased.After.TrackBounds.Maximum.Raw == 150,
        "stat-track maximum expansion did not preserve distance from maximum");

    ExactStatTrackSnapshot beforeStale = state.Read();
    ExpectMechanicsError(() => stale.Publish(), "stale stat-track candidate was published");
    Require(state.Read() == beforeStale, "stale stat-track candidate partially mutated state");

    ExactSource advancement = new(
        new IntrinsicSourceIdentity(new EntityId(1), SourceInstanceId.Parse("level-advance")),
        SourceDefinitionId.Parse("level-advance"),
        priority: 0,
        [new ExactStatContributionDefinition(
            maximum,
            StackingGroupId.Parse("health-maximum-advancement"),
            MechanicsStackingPolicy.Sum,
            new ExactStatContribution.Add(new ExactValue(10)))]);
    ExactStatTrackChangeReceipt sourceChanged = state.ApplySourceChange(
        new ExactValue(150),
        [advancement],
        ExactStatTrackCurrentPolicy.PreserveDistanceFromMaximum,
        expectedRevision: 2);
    Require(sourceChanged.After.Stat.Value.Raw == 160
        && sourceChanged.After.TrackCurrent.Raw == 100
        && sourceChanged.After.Stat.Decisions.Single().Outcome == MechanicsDecisionOutcome.Applied,
        "stat source replacement did not reconcile the dependent track");

    ExactStatTrackSnapshot beforeRejected = state.Read();
    ExpectMechanicsError(
        () => state.ApplySourceChange(
            new ExactValue(50),
            [],
            ExactStatTrackCurrentPolicy.PreserveCurrent,
            expectedRevision: state.Revision),
        "stranding stat-track source change was accepted");
    Require(state.Read() == beforeRejected, "rejected stat-track source change partially mutated state");
}

static void ExerciseExactTrackAtomicity()
{
    ExactTrackDefinition definition = new(
        TrackId.Parse("resource"),
        new ExactValue(0),
        new ExactTrackMaximum.Fixed(new ExactValue(100)));
    ExactTrack track = new(definition, new ExactValue(ResourceStart));
    ExactTrackMutationReceipt spent = track.Spend(new ExactValue(ResourceSpend));
    Require(spent.After.Raw == 20 && spent.AppliedAmount.Raw == ResourceSpend,
        "exact track spend was not applied");

    ExactValue beforeRejectedSpend = track.Current;
    ExpectMechanicsError(
        () => track.Spend(new ExactValue(100)),
        "overspend was accepted");
    Require(track.Current == beforeRejectedSpend, "rejected spend changed track state");

    ExactTrackMutationReceipt restored = track.Restore(new ExactValue(100));
    Require(restored.After.Raw == 100 && restored.AppliedAmount.Raw == 80,
        "restore did not clamp to the maximum");
    ExactTrackReconciliationReceipt clamped = track.Reconcile(
        definition.ResolveBounds(new ExactValue(60)),
        ExactTrackReconciliationPolicy.ClampToMaximum);
    Require(clamped.Before.Raw == 100 && track.Current.Raw == 60,
        "track reconciliation did not clamp atomically");

    ExactTrackBounds beforeRejectedReconcile = track.Bounds;
    ExpectMechanicsError(
        () => track.Reconcile(
            definition.ResolveBounds(new ExactValue(40)),
            ExactTrackReconciliationPolicy.PreserveCurrent),
        "no-stranding preserve policy was ignored");
    Require(track.Current.Raw == 60 && track.Bounds == beforeRejectedReconcile,
        "rejected reconciliation changed track state");

    ExactTrackBounds beforeExpandedReconcile = track.Bounds;
    ExactValue beforeExpandedValue = track.Current;
    ExactValue expandedMaximum = new(80);
    ExpectMechanicsError(
        () => track.Reconcile(
            definition.ResolveBounds(expandedMaximum),
            ExactTrackReconciliationPolicy.ClampToMaximum),
        "exact track reconciliation expanded its maximum");
    Require(track.Current == beforeExpandedValue && track.Bounds == beforeExpandedReconcile,
        "rejected exact maximum expansion changed track state");
}

static void ExerciseContinuousTrackAtomicity()
{
    ContinuousTrackDefinition definition = new(
        TrackId.Parse("continuous-resource"),
        new ContinuousValue(0.0),
        new ContinuousTrackMaximum.Fixed(new ContinuousValue(1.0)));
    ContinuousTrack track = new(definition, new ContinuousValue(0.5));
    ContinuousTrackBounds beforeExpandedReconcile = track.Bounds;
    ContinuousValue beforeExpandedValue = track.Current;
    ContinuousValue expandedMaximum = new(1.5);
    ExpectMechanicsError(
        () => track.Reconcile(
            definition.ResolveBounds(expandedMaximum),
            ContinuousTrackReconciliationPolicy.ClampToMaximum),
        "continuous track reconciliation expanded its maximum");
    Require(track.Current == beforeExpandedValue && track.Bounds == beforeExpandedReconcile,
        "rejected continuous maximum expansion changed track state");
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
    EffectState independentState = new(new EntityId(1));
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
    EffectState refreshState = new();
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
    EffectState replaceState = new();
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

    var world = new InventoryWorld();
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

    EquipmentMutationReceipt equipped = EquipmentService.Equip(
        world,
        new EntityId(Owner),
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
        () => ItemService.TransferUnique(
            world,
            new EntityId(RifleEntity),
            new EntityId(Owner),
            new EntityId(SecondOwner)),
        "equipped unique item transfer was not blocked");
    Require(world.Revision == beforeEquippedTransfer
        && world.TryGetContainer(new EntityId(RifleEntity), out EntityId owner)
        && owner == new EntityId(Owner),
        "rejected equipped transfer changed containment");

    InventoryWorldCandidate transfer = world.Prepare();
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

    EquipmentService.Equip(
        world,
        new EntityId(SecondOwner),
        new EntityId(RifleEntity),
        [leftHand, rightHand]);

    ItemMaterializationReceipt secondShield = world.MaterializeUnique(
        new ItemState(new EntityId(ShieldEntity), shield),
        new EntityId(SecondOwner));
    Require(secondShield.CapacityAfter.Single().Used == 14,
        "second-owner capacity was not maintained");
    ExpectMechanicsError(
        () => EquipmentService.Equip(
            world,
            new EntityId(SecondOwner),
            new EntityId(ShieldEntity),
            [shieldHand]),
        "exclusivity or containment validation was not enforced");
    Require(world.TryGetEquipment(new EntityId(SecondOwner), out EquipmentState? equipment)
        && equipment is not null
        && equipment.Assignments.Count == 2
        && equipment.Assignments.All(assignment => assignment.Item == new EntityId(RifleEntity)),
        "rejected equipment changed state");

    ItemDestroyReceipt destroyed = ItemService.DestroyUnique(world, new EntityId(ShieldEntity));
    Require(destroyed.FormerOwner == new EntityId(SecondOwner)
        && !world.TryGetItem(new EntityId(ShieldEntity), out _),
        "explicit unique destruction did not remove the item");

    var boundedWorld = new InventoryWorld();
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
        exception is MechanicsException or MechanicsArithmeticException or ArgumentException)
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
