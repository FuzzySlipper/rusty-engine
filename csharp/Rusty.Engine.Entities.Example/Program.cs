using System.Buffers;
using System.Numerics;
using Rusty.Engine;
using Rusty.Engine.Entities;
using Rusty.Engine.Mechanics;
using Rusty.Engine.Persistence;
using Rusty.Engine.Resolution;
using Rusty.Engine.StateMachine;
using MechanicsStackingPolicy = Rusty.Engine.Mechanics.MechanicsStackingPolicy;
using ResolutionCommitStatus = Rusty.Engine.Resolution.ResolutionCommitStatus;
using ResolutionMode = Rusty.Engine.Resolution.ResolutionMode;
using StateMachineDefinition = Rusty.Engine.StateMachine.StateMachineDefinition;
using StateMachineInstance = Rusty.Engine.StateMachine.StateMachineInstance;
using StateMachineTransition = Rusty.Engine.StateMachine.StateMachineTransition;
using StateMachineTransitionReceipt = Rusty.Engine.StateMachine.StateMachineTransitionReceipt;
using StateMachineTransitionRequest = Rusty.Engine.StateMachine.StateMachineTransitionRequest;

const uint HealthLocalComponentId = 1;
const uint ArmorLocalComponentId = 2;
const int InitialHealth = 10;
const int InitialArmor = 3;

var health = ComponentType<Health>.Create(
    ProductComponentKeys.Create(HealthLocalComponentId),
    validator: ValidateHealth);
var armor = ComponentType<Armor>.Create(ProductComponentKeys.Create(ArmorLocalComponentId));
using var world = new EntityWorld([EngineComponentTypes.Transform, EngineComponentTypes.CharacterMotion, health, armor]);

EntityId actor = world.Create();
EntityId pack = world.Create();
EntityId pouch = world.Create(EntityLifecycle.Disabled);
ContainmentReceipt contained = world.SetContainment(pouch, pack, world.Revision);
Require(contained.Changed && world.TryGetContainedIn(pouch, out EntityId container) && container == pack,
    "canonical containment did not preserve its parent");
Require(world.ContainedEntities(pack).SequenceEqual([pouch]), "reverse containment was not deterministic");
Throws(() => world.SetContainment(pack, pouch), "containment cycle was not rejected");
EntityWorldSnapshot relationshipSnapshot = world.Snapshot();
world.ClearContainment(pouch);
world.Restore(relationshipSnapshot);
Require(world.TryGetContainedIn(pouch, out container) && container == pack, "snapshot restore lost containment");
world.Set(actor, health, new Health(InitialHealth));
Throws(() => world.Set(actor, health, new Health(-1)), "typed component validator did not reject invalid state");
world.Set(actor, armor, new Armor(InitialArmor));
ComponentRevision healthRevision = world.GetComponentRevision(actor, health);

EntityWorldSnapshot snapshot = world.Snapshot();
EntityBatchReceipt receipt = world.Commit(new EntityBatch()
    .Mutate(staged => staged.Set(actor, health, new Health(6), healthRevision))
    .Mutate(staged => staged.SetLifecycle(actor, EntityLifecycle.Disabled)), expectedRevision: snapshot.Revision);

Require(receipt.RevisionAfter == snapshot.Revision + 1, "a successful batch must advance the world revision exactly once");
Require(world.Query(health).Count == 0, "disabled entities are omitted from normal queries");
Require(world.Query(health, includeDisabled: true).Single().Value.Current == 6, "typed batch mutation did not commit");
Require(world.Query(health, armor, includeDisabled: true).Single().Second.Current == InitialArmor, "two-component query did not join typed columns");

Throws(
    () => world.Commit(new EntityBatch()
        .Mutate(staged => staged.Set(actor, health, new Health(4)))
        .Mutate(staged => staged.Set(new EntityId(999), health, new Health(1))), expectedRevision: receipt.RevisionAfter),
    "a rejected batch must report its invalid staged mutation");
Require(world.Get(actor, health).Current == 6 && world.Revision == receipt.RevisionAfter, "a rejected batch changed live state");

world.Restore(snapshot, expectedRevision: receipt.RevisionAfter);
Require(world.Get(actor, health).Current == InitialHealth, "in-memory snapshot restore did not recover the typed value");
Throws(() => world.Set(actor, health, new Health(9), healthRevision), "snapshot restore must invalidate old component guards");
Require(world.Diagnostics().Components.Single(component => component.Key == health.Key).ValueCount == 1, "diagnostics lost the component table");

ExerciseManagedRestorePlan(world, actor, pack, health, armor);
ExerciseEntityPersistence(world, actor, health);
ExerciseManagedMechanics();
ExerciseManagedStateMachine();
ExerciseManagedResolution();

ExerciseSpatialEntityProjection();
ExerciseCharacterEntityComposition();
ExerciseAppearanceEntityComposition();
ExerciseWorldOriginEntityComposition();
ExerciseMotionEntityComposition();
ExerciseKinematicEntityComposition();
ExerciseDynamicsEntityComposition();

static void Require(bool condition, string message)
{
    if (!condition)
    {
        throw new InvalidOperationException(message);
    }
}

static void Throws(Action action, string message)
{
    try
    {
        action();
    }
    catch (Exception exception) when (exception is InvalidOperationException or ArgumentException)
    {
        return;
    }
    throw new InvalidOperationException(message);
}

static void ValidateHealth(in Health health)
{
    if (health.Current < 0)
    {
        throw new ArgumentOutOfRangeException(nameof(health), "Health cannot be negative.");
    }
}

static void ExerciseEntityPersistence(
    EntityWorld world,
    EntityId actor,
    ComponentType<Health> health)
{
    var persistence = new InMemoryPersistenceService();
    using var store = new EntityWorldProductStateStore<EntityCheckpoint>(
        world,
        current => new EntityCheckpoint(current.Get(actor, health).Current),
        (target, state) => target.Set(actor, health, new Health(state.Health)),
        new PersistenceEngineContext(persistence),
        "entities-example",
        new EntityCheckpointCodec());

    store.Save("checkpoint");
    world.Set(actor, health, new Health(4));
    ProductStateLoad<EntityCheckpoint> loaded = store.LoadAndRestore("checkpoint");
    Require(loaded.Present && loaded.State is EntityCheckpoint state && state.Health == InitialHealth,
        "product-owned EntityWorld persistence did not restore the selected typed state");
    Require(world.Get(actor, health).Current == InitialHealth,
        "EntityWorld persistence restore did not publish through the product callback");
}

static void ExerciseManagedMechanics()
{
    const long BaseStrength = 10;
    const long StrengthBonus = 5;
    const double BaseSpeed = 2.0;
    const double SpeedBonus = 0.5;
    const long MaximumHealth = 100;
    const long StartingHealth = 80;
    const long HealthSpend = 15;
    const double MaximumStamina = 100.0;
    const double StartingStamina = 60.0;
    const double StaminaSpend = 12.5;

    StatId strengthId = StatId.Parse("strength");
    ExactStatDefinition strength = new(strengthId, new ExactValue(0), new ExactValue(MaximumHealth));
    ExactSource strengthSource = new(
        new RequestSourceIdentity(OperationId.Parse("example"), SourceInstanceId.Parse("strength-bonus")),
        SourceDefinitionId.Parse("strength-bonus"),
        priority: 0,
        [
            new ExactStatContributionDefinition(
                strengthId,
                StackingGroupId.Parse("strength-additions"),
                MechanicsStackingPolicy.Sum,
                new ExactStatContribution.Add(new ExactValue(StrengthBonus))),
        ]);
    ExactStatEvaluation strengthResult = ExactStatEvaluator.Evaluate(
        strength,
        new ExactValue(BaseStrength),
        [strengthSource]);
    Require(strengthResult.Value.Raw == BaseStrength + StrengthBonus,
        "direct managed exact-stat evaluation did not apply its typed source");

    StatId speedId = StatId.Parse("movement-speed");
    ContinuousStatDefinition speed = new(
        speedId,
        new ContinuousValue(0.0),
        new ContinuousValue(10.0));
    ContinuousSource speedSource = new(
        new RequestSourceIdentity(OperationId.Parse("example"), SourceInstanceId.Parse("speed-bonus")),
        SourceDefinitionId.Parse("speed-bonus"),
        priority: 0,
        [
            new ContinuousStatContributionDefinition(
                speedId,
                StackingGroupId.Parse("speed-additions"),
                MechanicsStackingPolicy.Sum,
                new ContinuousStatContribution.Add(new ContinuousValue(SpeedBonus))),
        ]);
    ContinuousStatEvaluation speedResult = ContinuousStatEvaluator.Evaluate(
        speed,
        new ContinuousValue(BaseSpeed),
        [speedSource]);
    Require(Math.Abs(speedResult.Value.Value - (BaseSpeed + SpeedBonus)) < 0.0001,
        "direct managed continuous-stat evaluation did not apply its typed source");

    ExactTrack healthTrack = new(
        new ExactTrackDefinition(
            TrackId.Parse("health"),
            ExactValue.Zero,
            new ExactTrackMaximum.Fixed(new ExactValue(MaximumHealth))),
        new ExactValue(StartingHealth));
    ExactTrackMutationReceipt healthSpend = healthTrack.Spend(new ExactValue(HealthSpend));
    Require(healthSpend.After.Raw == StartingHealth - HealthSpend,
        "direct managed exact-track mutation did not update product-owned state");

    ContinuousTrack staminaTrack = new(
        new ContinuousTrackDefinition(
            TrackId.Parse("stamina"),
            ContinuousValue.Zero,
            new ContinuousTrackMaximum.Fixed(new ContinuousValue(MaximumStamina))),
        new ContinuousValue(StartingStamina));
    ContinuousTrackMutationReceipt staminaSpend = staminaTrack.Spend(new ContinuousValue(StaminaSpend));
    Require(Math.Abs(staminaSpend.After.Value - (StartingStamina - StaminaSpend)) < 0.0001,
        "direct managed continuous-track mutation did not update product-owned state");

    var inventory = new InventoryWorld();
    EntityId hero = new(1);
    EntityId chest = new(2);
    EntityId swordEntity = new(3);
    CapacityMetricId weight = CapacityMetricId.Parse("weight");
    ItemClassificationId weapon = ItemClassificationId.Parse("weapon");
    SourceDefinitionId swordSource = SourceDefinitionId.Parse("sword-source");
    ItemDefinition potion = new(
        ItemDefinitionId.Parse("potion"),
        ItemKind.Fungible,
        maximumQuantity: 20,
        capacityCosts: [new ItemCapacityCost(weight, 1)]);
    ItemDefinition sword = new(
        ItemDefinitionId.Parse("sword"),
        ItemKind.Unique,
        maximumQuantity: 1,
        classifications: [weapon],
        capacityCosts: [new ItemCapacityCost(weight, 3)],
        equipment: new ItemEquipmentPolicy(1),
        sourceDefinitions: [swordSource]);

    inventory.RegisterInventory(new InventoryState(
        hero,
        [new InventoryCapacityLimit(weight, 20)]));
    inventory.RegisterEquipment(new EquipmentState(hero));
    inventory.RegisterInventory(new InventoryState(
        chest,
        [new InventoryCapacityLimit(weight, 20)]));

    InventoryMutationReceipt potions = inventory.Grant(hero, potion, 3);
    Require(potions.AfterQuantity == 3, "managed inventory did not grant the requested stack");
    inventory.MaterializeUnique(new ItemState(swordEntity, sword), hero);
    EquipmentSlotDefinition mainHand = new(
        EquipmentSlotId.Parse("main-hand"),
        [weapon]);
    EquipmentMutationReceipt equipped = EquipmentService.Equip(inventory, hero, swordEntity, [mainHand]);
    Require(equipped.SourceActivations.Count == 1
        && inventory.View(hero).UniqueItems.Single().Entity == swordEntity,
        "managed equipment did not publish the equipped item and its source activation");

    Throws(
        () => inventory.TransferUnique(swordEntity, hero, chest),
        "managed inventory allowed an equipped unique item to transfer");
    EquipmentService.Unequip(inventory, hero, swordEntity);
    ItemTransferReceipt transferred = inventory.TransferUnique(swordEntity, hero, chest);
    Require(transferred.ToOwner == chest
        && inventory.View(chest).UniqueItems.Single().Entity == swordEntity,
        "managed inventory did not transfer the unequipped unique item");

    InventoryWorldCandidate candidate = inventory.Prepare(inventory.Revision);
    InventoryMutationReceipt chestPotions = candidate.Grant(chest, potion, 2);
    candidate.Publish();
    Require(chestPotions.AfterQuantity == 2 && inventory.View(chest).Stacks.Single().Quantity == 2,
        "managed inventory candidate did not publish one atomic product mutation");
}

static void ExerciseManagedStateMachine()
{
    const ulong MachineId = 1;
    const ulong IdleState = 0;
    const ulong ActiveState = 1;

    var definition = new StateMachineDefinition(
        MachineId,
        [IdleState, ActiveState],
        [new StateMachineTransition(IdleState, ActiveState), new StateMachineTransition(ActiveState, IdleState)]);
    StateMachineInstance instance = definition.CreateInstance(IdleState);
    StateMachineTransitionReceipt transition = definition.Transition(
        instance,
        new StateMachineTransitionRequest(IdleState, ActiveState, ExpectedRevision: 0));
    Require(transition.Instance.Current == ActiveState && transition.Revision == 1,
        "direct managed state-machine transition did not return the updated caller-owned value");
    Throws(
        () => definition.Transition(instance, new StateMachineTransitionRequest(ActiveState, IdleState)),
        "managed state-machine accepted a transition from a mismatched current state");
}

static void ExerciseManagedResolution()
{
    const ulong ResolutionId = 1;
    const ulong CorrelationId = 42;
    var session = new StructuralResolutionSession(
        ResolutionId,
        CorrelationId,
        ResolutionMode.Apply);
    session.Root.Record(work: 1, effects: 1, events: 1);
    session.Root.Complete();

    var transaction = new RecordingResolutionTransaction();
    ResolutionReceipt receipt = session.Finalize(transaction);
    Require(receipt.Commit == ResolutionCommitStatus.Applied
        && transaction.Staged
        && transaction.Committed
        && !transaction.Aborted,
        "direct managed resolution did not finalize its product transaction");
}

static void ExerciseWorldOriginEntityComposition()
{
    const uint GlobalPositionLocalComponentId = 40;
    var globalPositions = ComponentType<WorldOriginGlobalPosition>.Create(
        ProductComponentKeys.Create(GlobalPositionLocalComponentId));
    using var world = new EntityWorld([EngineComponentTypes.Transform, globalPositions]);
    EntityId entity = world.Create();
    world.Set(entity, EngineComponentTypes.Transform, new Transform(
        new Vector3(100.0f, 2.0f, -3.0f), Quaternion.Identity, new Vector3(2.0f, 3.0f, 4.0f)));
    world.Set(entity, globalPositions, new WorldOriginGlobalPosition(100, 2, -3, 0.0, 0.0, 0.0));
    var service = new WorldOriginServiceFake();
    var adapter = new WorldOriginEntityWorld(world, service, service.Session, globalPositions);

    using WorldOriginEntityWorldPrepared prepared = adapter.Prepare(100, 0, 0, maximumEntities: 1);
    Require(prepared.Receipt.Native.AffectedEntityCount == 1
        && prepared.Receipt.Affected.Span[0].EntityId == entity.Value,
        "world-origin prepare did not retain one deterministic root fact");
    WorldOriginEntityWorldCommitReceipt committed = prepared.Commit();
    Require(committed.Native.OriginAfterCellX == 100
        && committed.Managed.MutationCount == 1
        && world.Get(entity, EngineComponentTypes.Transform).Translation.X == 0.0f,
        "world-origin commit did not pair the native receipt with one managed transform batch");

    using WorldOriginEntityWorldPrepared stale = adapter.Prepare(200, 0, 0, maximumEntities: 1);
    world.Set(entity, EngineComponentTypes.Transform, new Transform(
        new Vector3(1.0f, 2.0f, -3.0f), Quaternion.Identity, new Vector3(2.0f, 3.0f, 4.0f)));
    Throws(() => stale.Commit(), "world-origin candidate did not reject stale managed transform state");
    Require(service.CommitCount == 1,
        "stale managed world state crossed into the native world-origin commit");
}

static void ExerciseMotionEntityComposition()
{
    using var world = new EntityWorld([EngineComponentTypes.Transform, EngineComponentTypes.SpatialCollider]);
    EntityId mover = world.Create();
    EntityId wall = world.Create();
    world.Set(mover, EngineComponentTypes.Transform, new Transform(
        Vector3.Zero, Quaternion.Identity, Vector3.One));
    world.Set(mover, EngineComponentTypes.SpatialCollider, new SpatialCollider(
        new Vector3(-0.5f), new Vector3(0.5f), 0, 0, true, false, false));
    world.Set(wall, EngineComponentTypes.Transform, new Transform(
        new Vector3(2.0f, 0.0f, 0.0f), Quaternion.Identity, Vector3.One));
    world.Set(wall, EngineComponentTypes.SpatialCollider, new SpatialCollider(
        new Vector3(-0.5f), new Vector3(0.5f), 0, 0, true, true, false));
    var service = new MotionServiceFake();
    var adapter = new MotionEntityWorld(world, service, EngineComponentTypes.SpatialCollider);

    MotionEntityWorldReceipt moved = adapter.Resolve(mover, new Vector3(1.0f, 0.0f, 0.0f), maximumEntities: 2);
    Require(moved.Resolution.Outcome == MotionOutcome.Moved
        && moved.Managed.MutationCount == 1
        && world.Get(mover, EngineComponentTypes.Transform).Translation.X == 1.0f,
        "motion adapter did not apply the pure candidate transform in one managed batch");

    world.Set(mover, EngineComponentTypes.Transform, new Transform(
        Vector3.Zero, Quaternion.Identity, Vector3.One));
    Throws(() => adapter.Resolve(
        mover,
        new Vector3(1.0f, 0.0f, 0.0f),
        maximumEntities: 2,
        expectedGuard: moved.Guard),
        "motion adapter did not reject stale managed projection evidence");
    Require(service.ResolveCount == 1,
        "stale managed motion state reached the pure generated service");
}

static void ExerciseKinematicEntityComposition()
{
    using var world = new EntityWorld([
        EngineComponentTypes.Transform,
        EngineComponentTypes.Kinematic,
        EngineComponentTypes.SpatialCollider]);
    EntityId mover = world.Create();
    EntityId selectedPeer = world.Create();
    EntityId blocker = world.Create();
    world.Set(mover, EngineComponentTypes.Transform, new Transform(Vector3.Zero, Quaternion.Identity, Vector3.One));
    world.Set(mover, EngineComponentTypes.Kinematic, new Kinematic(new Vector3(0.4f), new Vector3(2.0f, 0.0f, 2.0f)));
    world.Set(mover, EngineComponentTypes.SpatialCollider, new SpatialCollider(new Vector3(-0.4f), new Vector3(0.4f), 0, 0, true, false, false));
    world.Set(selectedPeer, EngineComponentTypes.Transform, new Transform(new Vector3(1.0f, 0.0f, 0.0f), Quaternion.Identity, Vector3.One));
    world.Set(selectedPeer, EngineComponentTypes.Kinematic, new Kinematic(new Vector3(0.4f), Vector3.Zero));
    world.Set(selectedPeer, EngineComponentTypes.SpatialCollider, new SpatialCollider(new Vector3(-0.4f), new Vector3(0.4f), 0, 0, true, false, false));
    world.Set(blocker, EngineComponentTypes.Transform, new Transform(new Vector3(2.0f, 0.0f, 1.0f), Quaternion.Identity, Vector3.One));
    world.Set(blocker, EngineComponentTypes.Kinematic, new Kinematic(new Vector3(0.4f), Vector3.Zero));
    world.Set(blocker, EngineComponentTypes.SpatialCollider, new SpatialCollider(new Vector3(-0.4f), new Vector3(0.4f), 0, 0, true, false, false));
    var service = new KinematicServiceFake();
    var adapter = new KinematicEntityWorld(world, service, EngineComponentTypes.SpatialCollider);

    ulong before = world.Revision;
    KinematicEntityWorldPrepared prepared = adapter.Prepare(
        service.Session,
        deltaSeconds: 1.0f,
        maximumEntities: 3,
        selection: new EntityId[] { mover, selectedPeer });
    Require(prepared.Motion.BodiesConsidered == 2
        && prepared.Motion.Candidates.Span.Length == 1
        && prepared.Motion.Facts.Span.Length == 2
        && prepared.Motion.Facts.Span[0].Kind == KinematicMotionFactKind.Blocked
        && prepared.Motion.Facts.Span[0].EntityId == mover.Value
        && prepared.Motion.Facts.Span[1].Kind == KinematicMotionFactKind.Moved
        && prepared.Motion.Facts.Span[1].EntityId == mover.Value,
        "Kinematic prepare did not preserve deterministic selected blocked and moved facts");
    KinematicEntityWorldReceipt applied = prepared.Apply();
    Require(applied.Managed.RevisionBefore == before
        && applied.Managed.RevisionAfter == before + 1
        && world.Get(mover, EngineComponentTypes.Transform).Translation == new Vector3(2.0f, 0.0f, 0.0f)
        && world.Get(mover, EngineComponentTypes.Kinematic).Velocity == new Vector3(2.0f, 0.0f, 0.0f)
        && world.Get(blocker, EngineComponentTypes.Transform).Translation == new Vector3(2.0f, 0.0f, 1.0f)
        && world.Get(blocker, EngineComponentTypes.Kinematic).Velocity == Vector3.Zero,
        "Kinematic apply did not publish exactly one managed mover batch while retaining the blocker");

    int callsBeforeStale = service.RunCount;
    world.Set(selectedPeer, EngineComponentTypes.Kinematic, new Kinematic(new Vector3(0.4f), new Vector3(1.0f, 0.0f, 0.0f)));
    Throws(
        () => adapter.Prepare(service.Session, 1.0f, 3, new EntityId[] { mover }, applied.Guard),
        "Kinematic stale managed guard was not rejected before native crossing");
    Require(service.RunCount == callsBeforeStale,
        "Kinematic stale managed guard reached the generated service");

    ulong noOpBefore = world.Revision;
    KinematicEntityWorldReceipt noOp = adapter.Prepare(
        service.Session,
        1.0f,
        3,
        ReadOnlyMemory<EntityId>.Empty).Apply();
    Require(noOp.Managed.RevisionBefore == noOpBefore && noOp.Managed.RevisionAfter == noOpBefore,
        "Kinematic empty selected phase changed the managed world revision");
}

static void ExerciseDynamicsEntityComposition()
{
    using var entities = new EntityWorld([
        EngineComponentTypes.Transform,
        EngineComponentTypes.DynamicsMotion]);
    EntityId entity = entities.Create();
    entities.Set(entity, EngineComponentTypes.Transform, new Transform(Vector3.Zero, Quaternion.Identity, new Vector3(2.0f, 3.0f, 4.0f)));
    entities.Set(entity, EngineComponentTypes.DynamicsMotion, new DynamicsMotion(Vector3.Zero, Vector3.Zero, false));
    var service = new DynamicsServiceFake();
    using var dynamicsWorld = new DynamicsWorld(new DynamicsWorldHandle(10), static () => { });
    using var body = new DynamicsBody(new DynamicsBodyHandle(20), static () => { });
    var adapter = new DynamicsEntityWorld(entities, service, dynamicsWorld);

    ulong before = entities.Revision;
    DynamicsEntityWorldReceipt receipt = adapter.Step(
        stepSeconds: 1.0f / 60.0f,
        steps: 1,
        bindings: new[] { new DynamicsEntityBinding(entity, body) },
        actions: new[] { new DynamicsEntityAction(entity, new Vector3(3.0f, 0.0f, 0.0f), Vector3.Zero, Vector3.Zero, Vector3.Zero, true) },
        maximumBodies: 1,
        maximumActions: 1);
    Require(receipt.Native.Bodies.Length == 1
        && receipt.Native.Bodies.Span[0].Body.Value == body.Handle.Value
        && receipt.Managed.RevisionBefore == before
        && receipt.Managed.RevisionAfter == before + 1
        && entities.Get(entity, EngineComponentTypes.Transform).Translation == new Vector3(3.0f, 0.0f, 0.0f)
        && entities.Get(entity, EngineComponentTypes.Transform).Scale == new Vector3(2.0f, 3.0f, 4.0f)
        && entities.Get(entity, EngineComponentTypes.DynamicsMotion).LinearVelocity == new Vector3(3.0f, 0.0f, 0.0f),
        "Dynamics adapter did not publish the one correlated native body readout in one managed batch");

    int callsBeforeStale = service.StepAndReadCalls;
    entities.Set(entity, EngineComponentTypes.DynamicsMotion, new DynamicsMotion(Vector3.One, Vector3.Zero, false));
    Throws(
        () => adapter.Step(1.0f / 60.0f, 1,
            new[] { new DynamicsEntityBinding(entity, body) },
            Array.Empty<DynamicsEntityAction>(),
            maximumBodies: 1,
            maximumActions: 0,
            expectedGuard: receipt.Guard),
        "Dynamics adapter did not reject stale managed state before its native crossing");
    Require(service.StepAndReadCalls == callsBeforeStale,
        "Dynamics stale managed state reached the generated step/read crossing");
}

static void ExerciseSpatialEntityProjection()
{
    using var world = new EntityWorld([EngineComponentTypes.Transform, EngineComponentTypes.SpatialCollider]);
    var spatial = new SpatialServiceFake();
    using var session = new SpatialSession(new SpatialSessionHandle(1), () => { });
    var adapter = new SpatialEntityWorld(world, spatial, session, EngineComponentTypes.SpatialCollider);
    EntityId actor = world.Create();
    world.Set(actor, EngineComponentTypes.Transform, new Transform(
        new Vector3(10f, 2f, -3f),
        Quaternion.Identity,
        new Vector3(2f, 1f, 1f)));
    world.Set(actor, EngineComponentTypes.SpatialCollider,
        new SpatialCollider(
            new Vector3(-1f, -1f, -1f),
            new Vector3(1f, 1f, 1f),
            CollisionGroup: 1,
            CollisionMask: 1,
            Enabled: true,
            StaticCollider: false,
            Trigger: true));

    SpatialEntityWorldReconcileReceipt receipt = adapter.ReconcileTriggers(
        tick: 7,
        cause: SpatialTriggerCause.Movement,
        maximumEntities: 4,
        maximumFactReadback: 1);
    Require(receipt.Entities.Length == 1, "spatial projection did not produce one active entity");
    SpatialEntityCollider projected = receipt.Entities.Span[0];
    Require(spatial.ReconcileCalls == 1
        && projected.Entity == actor.Value
        && projected.Min == new Vector3(8f, 1f, -4f)
        && projected.Max == new Vector3(12f, 3f, -2f),
        "spatial projection did not inject the canonical entity identity and transformed bounds");
    Require(receipt.Trigger.Tick == 7
        && receipt.Facts.Length == 1
        && receipt.Facts.Span[0].Present
        && receipt.Facts.Span[0].Subject == actor.Value
        && !receipt.FactsTruncated,
        "spatial reconciliation did not copy its bounded generated readback");

    world.Set(actor, EngineComponentTypes.Transform, new Transform(
        new Vector3(11f, 2f, -3f),
        Quaternion.Identity,
        new Vector3(2f, 1f, 1f)));
    Throws(
        () => adapter.ReconcileTriggers(8, SpatialTriggerCause.Movement, 4, 1, receipt.Guard),
        "stale spatial world guard was accepted");
    SpatialEntityWorldGuard staleComponentGuard = receipt.Guard with { WorldRevision = world.Revision };
    Throws(
        () => adapter.ReconcileTriggers(8, SpatialTriggerCause.Movement, 4, 1, staleComponentGuard),
        "stale spatial component guard was accepted");
    Require(spatial.ReconcileCalls == 1, "stale spatial projection crossed into the generated service");
}

static void ExerciseCharacterEntityComposition()
{
    using var world = new EntityWorld([EngineComponentTypes.Transform, EngineComponentTypes.CharacterMotion]);
    _ = world.Create();
    EntityId actor = world.Create();
    Quaternion actorRotation = Quaternion.CreateFromAxisAngle(Vector3.UnitY, MathF.PI / 2.0f);
    Vector3 actorScale = new(2.0f, 3.0f, 4.0f);
    world.Set(actor, EngineComponentTypes.Transform, new Transform(Vector3.Zero, actorRotation, actorScale));
    world.Set(actor, EngineComponentTypes.CharacterMotion, new CharacterMotion(
        Vector3.Zero,
        Vector3.Zero,
        false,
        CharacterStance.Standing,
        0,
        0,
        0,
        false,
        0,
        Vector3.Zero,
        Vector3.Zero,
        Quaternion.Identity,
        Vector3.Zero,
        0,
        0,
        0,
        0));
    var spatial = new SpatialServiceFake();
    var adapter = new CharacterEntityWorld(world, spatial);
    var command = new CharacterControllerCommand(
        Vector2.Zero, 0, false, false, false, Vector3.Zero, Vector3.Zero, 1.0f / 60.0f, 7);

    ulong before = world.Revision;
    CharacterEntityWorldReceipt receipt = adapter.Step(
        actor,
        spatial.Session,
        default,
        default,
        command);
    Require(receipt.Entity == actor
        && receipt.Native.Entity == 1
        && receipt.Managed.RevisionBefore == before
        && receipt.Managed.RevisionAfter == before + 1
        && world.Get(actor, EngineComponentTypes.Transform).Translation == Vector3.UnitX
        && world.Get(actor, EngineComponentTypes.Transform).Rotation == actorRotation
        && world.Get(actor, EngineComponentTypes.Transform).Scale == actorScale
        && world.Get(actor, EngineComponentTypes.CharacterMotion).LastCommandSequence == command.Sequence,
        "character adapter did not preserve a non-native managed identity and transform shape while publishing its returned state");

    int callsBeforeStale = spatial.CharacterStepCalls;
    CharacterMotion stale = world.Get(actor, EngineComponentTypes.CharacterMotion) with { LastCommandSequence = 8 };
    world.Set(actor, EngineComponentTypes.CharacterMotion, stale);
    Throws(
        () => adapter.Step(actor, spatial.Session, default, default, command, receipt.Guard),
        "character adapter accepted a stale managed projection");
    Require(spatial.CharacterStepCalls == callsBeforeStale,
        "stale character managed state reached the generated service");
}

static void ExerciseAppearanceEntityComposition()
{
    using var world = new EntityWorld([EngineComponentTypes.Transform]);
    EntityId first = world.Create();
    EntityId second = world.Create();
    world.Set(first, EngineComponentTypes.Transform, new Transform(Vector3.Zero, Quaternion.Identity, Vector3.One));
    world.Set(second, EngineComponentTypes.Transform, new Transform(new Vector3(2, 0, 0), Quaternion.Identity, Vector3.One));
    var appearance = new AppearanceServiceFake();
    using var firstHandle = new Appearance(new AppearanceHandle(10), () => { });
    using var secondHandle = new Appearance(new AppearanceHandle(20), () => { });
    var adapter = new AppearanceEntityWorld(world, appearance);
    AppearanceEntityWorldEntry[] entries =
    [
        new(second, secondHandle, true, RenderLayer.Debug),
        new(first, firstHandle, true, RenderLayer.Scene),
    ];

    AppearanceEntityWorldReceipt receipt = adapter.Publish(entries, maximumEntities: 2);
    Require(appearance.PublishCalls == 1
        && receipt.Facts.Span.Length == 2
        && appearance.LastSnapshot.Span[0].ObjectId == first.Value
        && appearance.LastSnapshot.Span[0].Appearance == firstHandle
        && appearance.LastSnapshot.Span[1].ObjectId == second.Value
        && appearance.LastSnapshot.Span[1].Appearance == secondHandle,
        "appearance adapter did not publish caller-owned handles in deterministic managed entity order");

    world.Set(first, EngineComponentTypes.Transform, new Transform(Vector3.UnitY, Quaternion.Identity, Vector3.One));
    Throws(
        () => adapter.Publish(entries, maximumEntities: 2, receipt.Guard),
        "appearance adapter accepted a stale managed transform projection");
    Require(appearance.PublishCalls == 1,
        "stale appearance managed state reached the generated service");
}

static void ExerciseManagedRestorePlan(
    EntityWorld world,
    EntityId actor,
    EntityId pack,
    ComponentType<Health> health,
    ComponentType<Armor> armor)
{
    ulong revisionBefore = world.Revision;
    ComponentRevision absentHealthRevision = world.GetComponentRevision(pack, health);
    EntityWorldRestorePlan plan = new(world.Revision, world.NextEntityValue);
    foreach (EntityWorldEntityState entity in world.CaptureEntities())
    {
        plan.AddEntity(entity);
    }
    foreach (EntityWorldContainmentState relation in world.CaptureContainment())
    {
        plan.AddContainment(relation);
    }
    plan.AddComponentFamily(EngineComponentTypes.Transform, world.CaptureComponentFamily(EngineComponentTypes.Transform));
    plan.AddComponentFamily(EngineComponentTypes.CharacterMotion, world.CaptureComponentFamily(EngineComponentTypes.CharacterMotion));
    plan.AddComponentFamily(health, world.CaptureComponentFamily(health));
    plan.AddComponentFamily(armor, world.CaptureComponentFamily(armor));

    EntityWorldRestoreCandidate candidate = world.PrepareRestore(plan, revisionBefore);
    candidate.Publish();
    candidate.Publish();
    Require(world.Revision == revisionBefore + 1, "managed restore candidate did not publish exactly once");
    Throws(
        () => world.Set(pack, health, new Health(3), absentHealthRevision),
        "absent component revision was not rebased during managed restore");

    EntityWorldRestorePlan invalid = new(world.Revision, world.NextEntityValue);
    foreach (EntityWorldEntityState entity in world.CaptureEntities())
    {
        invalid.AddEntity(entity);
    }
    foreach (EntityWorldContainmentState relation in world.CaptureContainment())
    {
        invalid.AddContainment(relation);
    }
    invalid.AddComponentFamily(EngineComponentTypes.Transform, world.CaptureComponentFamily(EngineComponentTypes.Transform));
    invalid.AddComponentFamily(EngineComponentTypes.CharacterMotion, world.CaptureComponentFamily(EngineComponentTypes.CharacterMotion));
    invalid.AddComponentFamily(
        health,
        world.CaptureComponentFamily(health)
            .Select(slot => slot.Entity == actor ? slot with { Present = true, Value = new Health(-1) } : slot)
            .ToArray());
    invalid.AddComponentFamily(armor, world.CaptureComponentFamily(armor));
    Throws(
        () => world.PrepareRestore(invalid, world.Revision),
        "invalid managed restore input was accepted");
    Require(world.Get(actor, health).Current == 10, "rejected managed restore input changed live state");
}


sealed class SpatialServiceFake : ISpatialService
{
    public int ReconcileCalls { get; private set; }
    public int CharacterStepCalls { get; private set; }
    public SpatialSession Session { get; } = new(new SpatialSessionHandle(2), () => { });
    private SpatialEntityCollider[] _entities = [];

    public SpatialSession CreateSession(SpatialSessionConfig arg0) => throw new NotSupportedException();
    public CollisionReplaceReceipt ReplaceCollision(CollisionReplaceRequest arg0) => throw new NotSupportedException();
    public NavigationReplaceReceipt ReplaceNavigation(NavigationReplaceRequest arg0) => throw new NotSupportedException();
    public NavigationReplaceReceipt ReplaceVoxelNavigation(NavigationVoxelReplaceRequest arg0) => throw new NotSupportedException();
    public NavigationTraversalReplaceReceipt ReplaceNavigationTraversal(NavigationTraversalReplaceRequest arg0) => throw new NotSupportedException();
    public NavigationTraversalReplaceReceipt ClearNavigationTraversal(NavigationTraversalClearRequest arg0) => throw new NotSupportedException();
    public NavigationVolumetricTraversalReplaceReceipt ReplaceVolumetricNavigationTraversal(NavigationVolumetricTraversalReplaceRequest arg0) => throw new NotSupportedException();
    public NavigationVolumetricTraversalReplaceReceipt ClearVolumetricNavigationTraversal(NavigationVolumetricTraversalClearRequest arg0) => throw new NotSupportedException();
    public NavigationProjectionReadout ReadNavigationProjection(NavigationProjectionReadRequest arg0) => throw new NotSupportedException();
    public NavigationPathReadout RequestNavigationPath(NavigationPathRequest arg0) => throw new NotSupportedException();
    public NavigationWeightedPathReadout RequestWeightedNavigationPath(NavigationWeightedPathRequest arg0) => throw new NotSupportedException();
    public NavigationPathCellAtReceipt ReadNavigationPathCellAt(NavigationPathCellAtRequest arg0) => throw new NotSupportedException();
    public NavigationPathReadout RequestVolumetricNavigationPath(NavigationVolumetricPathRequest arg0) => throw new NotSupportedException();
    public NavigationVolumetricWeightedPathReadout RequestWeightedVolumetricNavigationPath(NavigationVolumetricWeightedPathRequest arg0) => throw new NotSupportedException();
    public void ClearNavigation(NavigationClearRequest arg0) => throw new NotSupportedException();
    public CharacterControllerConfig DefaultCharacterControllerConfig() => default;

    public CharacterStepReceipt ProposeCharacterStep(CharacterStepRequest request)
    {
        CharacterStepCalls++;
        Transform before = new(request.Position, Quaternion.Identity, Vector3.One);
        Transform after = before with { Translation = before.Translation + Vector3.UnitX };
        CharacterMotion motion = request.Motion with { LastCommandSequence = request.Command.Sequence };
        return new CharacterStepReceipt(
            1,
            0,
            1,
            1,
            request.Command.Sequence,
            before,
            after,
            motion,
            Vector3.UnitX,
            Vector3.UnitX,
            default,
            default,
            default,
            default,
            default,
            default,
            0,
            0,
            0,
            0,
            0,
            0);
    }
    public CharacterControllerReadout ReadCharacterController(CharacterControllerReadRequest arg0) => throw new NotSupportedException();
    public CharacterContactAtReceipt ReadCharacterContactAt(CharacterContactAtRequest arg0) => throw new NotSupportedException();
    public CharacterDynamicImpulseAtReceipt ReadCharacterDynamicImpulseAt(CharacterDynamicImpulseAtRequest arg0) => throw new NotSupportedException();
    public NavigationStepReceipt ProposeNavigationStep(NavigationStepRequest arg0) => throw new NotSupportedException();
    public SpatialProjectionReadout ReadProjection(SpatialProjectionReadRequest arg0) => throw new NotSupportedException();
    public SpatialQueryReceipt ContainsPoint(SpatialContainsPointRequest arg0) => throw new NotSupportedException();
    public SpatialHit CastRay(SpatialRaycastRequest arg0) => throw new NotSupportedException();
    public SpatialHit CastSegment(SpatialSegmentCastRequest arg0) => throw new NotSupportedException();
    public SpatialQueryReceipt OverlapAabb(SpatialAabbQueryRequest arg0) => throw new NotSupportedException();
    public SpatialQueryReceipt SweepAabb(SpatialAabbQueryRequest arg0) => throw new NotSupportedException();
    public SpatialHit CastCapsule(SpatialCapsuleQueryRequest arg0) => throw new NotSupportedException();
    public SpatialHit OverlapCapsule(SpatialCapsuleQueryRequest arg0) => throw new NotSupportedException();
    public SpatialHit PickVoxel(SpatialPickRequest arg0) => throw new NotSupportedException();
    public void RegisterTrigger(SpatialTriggerRegisterRequest arg0) => throw new NotSupportedException();

    public SpatialTriggerReceipt ReconcileTriggers(SpatialTriggerReconcileRequest request)
    {
        ReconcileCalls++;
        _entities = request.Entities.ToArray();
        return new SpatialTriggerReceipt(
            request.Tick,
            request.Cause,
            (ulong)ReconcileCalls,
            _entities.Length == 0 ? 0u : 1u,
            0,
            0,
            0);
    }

    public SpatialTriggerReadReceipt ReadTrigger(SpatialTriggerReadRequest arg0) => throw new NotSupportedException();
    public SpatialTriggerOverlapAtReceipt ReadTriggerOverlapAt(SpatialTriggerOverlapAtRequest arg0) => throw new NotSupportedException();

    public SpatialTriggerFactAtReceipt ReadTriggerFactAt(SpatialTriggerFactAtRequest request)
        => request.Index == 0 && _entities.Length != 0
            ? new SpatialTriggerFactAtReceipt(true, true, _entities[0].Entity, _entities[0].Entity, 7, SpatialTriggerCause.Movement)
            : default;
}

sealed class AppearanceServiceFake : IAppearanceService
{
    private AppearanceFact[] _lastSnapshot = [];

    public int PublishCalls { get; private set; }

    public ReadOnlyMemory<AppearanceFact> LastSnapshot => _lastSnapshot;

    public RenderResourceInfo OpenResource(RenderResourceRequest arg0) => throw new NotSupportedException();
    public Material CreateMaterial(MaterialRequest arg0) => throw new NotSupportedException();
    public void UpdateMaterial(MaterialUpdateRequest arg0) => throw new NotSupportedException();
    public Material ReplaceMaterial(MaterialUpdateRequest arg0) => throw new NotSupportedException();
    public Appearance CreatePrimitive(PrimitiveAppearanceRequest arg0) => throw new NotSupportedException();
    public Appearance ReplacePrimitive(PrimitiveAppearanceReplaceRequest arg0) => throw new NotSupportedException();
    public Appearance CreateStaticMesh(StaticMeshAppearanceRequest arg0) => throw new NotSupportedException();
    public Appearance CreateStaticMeshFromContent(StaticMeshContentAppearanceRequest arg0) => throw new NotSupportedException();
    public Appearance ReplaceStaticMesh(Appearance arg0, StaticMeshAppearanceRequest arg1) => throw new NotSupportedException();
    public Appearance ReplaceStaticMeshFromContent(Appearance arg0, StaticMeshContentAppearanceRequest arg1) => throw new NotSupportedException();
    public void UpdateStaticMeshMaterials(StaticMeshMaterialUpdateRequest arg0) => throw new NotSupportedException();
    public Appearance CreateSprite(SpriteAppearanceRequest arg0) => throw new NotSupportedException();
    public Appearance ReplaceSprite(SpriteAppearanceReplaceRequest arg0) => throw new NotSupportedException();
    public SpriteAtlas CreateSpriteAtlas(SpriteAtlasCreateRequest arg0) => throw new NotSupportedException();
    public Appearance CreateSpriteFromAtlas(SpriteFromAtlasRequest arg0) => throw new NotSupportedException();
    public Appearance ReplaceSpriteFromAtlas(SpriteFromAtlasReplaceRequest arg0) => throw new NotSupportedException();
    public void SetSpriteFrame(SpriteFrameUpdateRequest arg0) => throw new NotSupportedException();
    public SpriteReadout ReadSprite(Appearance arg0) => throw new NotSupportedException();

    public void PublishSnapshot(ReadOnlySpan<AppearanceFact> values)
    {
        PublishCalls++;
        _lastSnapshot = values.ToArray();
    }

    public Light CreateLight(LightRequest arg0) => throw new NotSupportedException();
    public void UpdateLight(LightUpdateRequest arg0) => throw new NotSupportedException();
    public Light ReplaceLight(LightUpdateRequest arg0) => throw new NotSupportedException();
    public LightReadout ReadLight(Light arg0) => throw new NotSupportedException();
    public PresentationReadout ReadPresentation() => throw new NotSupportedException();
}


sealed class PersistenceEngineContext(IPersistenceService persistence) : IEngineContext
{
    public ILookService Look => throw new NotSupportedException();
    public IDynamicsService Dynamics => throw new NotSupportedException();
    public IMotionService Motion => throw new NotSupportedException();
    public IKinematicService Kinematic => throw new NotSupportedException();
    public ISpatialService Spatial => throw new NotSupportedException();
    public IPerceptionService Perception => throw new NotSupportedException();
    public IWorldOriginService WorldOrigin => throw new NotSupportedException();
    public IVoxelService Voxel => throw new NotSupportedException();
    public IVoxelContentService VoxelContent => throw new NotSupportedException();
    public IContentService Content => throw new NotSupportedException();
    public IAuthoredContentService AuthoredContent => throw new NotSupportedException();
    public IAppearanceService Appearance => throw new NotSupportedException();
    public IPresentationService Presentation => throw new NotSupportedException();
    public IAnimationService Animation => throw new NotSupportedException();
    public IAudioService Audio => throw new NotSupportedException();
    public ICameraViewService CameraView => throw new NotSupportedException();
    public IRandomService Random => throw new NotSupportedException();
    public IVoxelScenePresentationService VoxelScenePresentation => throw new NotSupportedException();
    public IPersistenceService Persistence { get; } = persistence;
    public IContentStoreService ContentStore => throw new NotSupportedException();
    public IUiService Ui => throw new NotSupportedException();
}

sealed class InMemoryPersistenceService : IPersistenceService
{
    private sealed record Saved(uint SchemaVersion, ulong Revision, byte[] Payload);

    private readonly Dictionary<ulong, string> _scopes = [];
    private readonly Dictionary<ulong, Saved> _blobs = [];
    private readonly Dictionary<(string Scope, string Key), Saved> _saved = [];
    private ulong _nextHandle = 1;

    public PersistenceStore OpenStore(PersistenceOpenRequest request)
    {
        ulong handle = _nextHandle++;
        _scopes.Add(handle, request.Scope);
        return new PersistenceStore(new PersistenceStoreHandle(handle), () => _scopes.Remove(handle));
    }

    public PersistenceSaveReceipt Save(PersistenceSaveRequest request)
    {
        string scope = _scopes[request.Store.Handle.Value];
        var key = (scope, request.Key);
        _saved.TryGetValue(key, out Saved? previous);
        ulong revision = (previous?.Revision ?? 0) + 1;
        _saved[key] = new Saved(request.SchemaVersion, revision, request.Payload.ToArray());
        return new PersistenceSaveReceipt(revision, request.SchemaVersion);
    }

    public PersistenceBlob Load(PersistenceLoadRequest request)
    {
        string scope = _scopes[request.Store.Handle.Value];
        _saved.TryGetValue((scope, request.Key), out Saved? saved);
        ulong handle = _nextHandle++;
        _blobs.Add(handle, saved ?? new Saved(0, 0, []));
        return new PersistenceBlob(new PersistenceBlobHandle(handle), () => _blobs.Remove(handle));
    }

    public PersistenceBlobInfo DescribeBlob(PersistenceBlob blob)
    {
        Saved saved = _blobs[blob.Handle.Value];
        return new PersistenceBlobInfo(saved.Revision != 0, saved.SchemaVersion, saved.Revision, (nuint)saved.Payload.Length);
    }

    public void CopyBlob(PersistenceCopyBlobRequest request)
        => _blobs[request.Blob.Handle.Value].Payload.CopyTo(request.Destination.Span);

    public ReadOnlyMemory<byte> ReadBlobBytes(PersistenceBlob blob)
        => _blobs[blob.Handle.Value].Payload;

    public void Seed(string scope, string key, uint schemaVersion, byte[] payload)
        => _saved[(scope, key)] = new Saved(schemaVersion, 1, payload);
}


sealed class WorldOriginServiceFake : IWorldOriginService
{
    private const ulong InitialRevision = 0;
    private readonly Dictionary<ulong, Prepared> _prepared = [];
    private ulong _nextPrepared = 1;

    public SpatialSession Session { get; } = new(new SpatialSessionHandle(1), () => { });
    public int CommitCount { get; private set; }

    public WorldOriginPrepared Prepare(WorldOriginPrepareRequest request)
    {
        ulong handle = _nextPrepared++;
        var facts = new WorldOriginAffectedAtReceipt[request.Entities.Length];
        ReadOnlySpan<WorldOriginEntityRow> rows = request.Entities.Span;
        for (int index = 0; index < rows.Length; index++)
        {
            WorldOriginEntityRow row = rows[index];
            Transform local = row.LocalTransform with
            {
                Translation = new Vector3(
                    checked((float)(row.GlobalPosition.CellX - request.TargetCellX)) + (float)row.GlobalPosition.OffsetX,
                    checked((float)(row.GlobalPosition.CellY - request.TargetCellY)) + (float)row.GlobalPosition.OffsetY,
                    checked((float)(row.GlobalPosition.CellZ - request.TargetCellZ)) + (float)row.GlobalPosition.OffsetZ),
            };
            facts[index] = new WorldOriginAffectedAtReceipt(true, row.EntityId, local);
        }
        _prepared.Add(handle, new Prepared(request, facts));
        return new WorldOriginPrepared(new WorldOriginPreparedHandle(handle), () => _prepared.Remove(handle));
    }

    public WorldOriginReadout Read(WorldOriginReadRequest request)
        => new(0, 0, 0, InitialRevision, 16_384.0f, 0, 0);

    public WorldOriginPreparedReadout ReadPrepared(WorldOriginPreparedReadRequest request)
    {
        Prepared prepared = Require(request.Prepared);
        return new WorldOriginPreparedReadout(
            true,
            prepared.Request.TargetCellX,
            prepared.Request.TargetCellY,
            prepared.Request.TargetCellZ,
            InitialRevision + 1,
            0,
            0,
            checked((uint)prepared.Facts.Length),
            16_384.0f);
    }

    public WorldOriginAffectedAtReceipt ReadAffectedAt(WorldOriginAffectedAtRequest request)
    {
        Prepared prepared = Require(request.Prepared);
        return request.Index < prepared.Facts.Length
            ? prepared.Facts[request.Index]
            : default;
    }

    public WorldOriginCommitReceipt Commit(WorldOriginCommitRequest request)
    {
        Prepared prepared = Require(request.Prepared);
        CommitCount++;
        return new WorldOriginCommitReceipt(
            InitialRevision,
            InitialRevision + 1,
            0,
            0,
            0,
            prepared.Request.TargetCellX,
            prepared.Request.TargetCellY,
            prepared.Request.TargetCellZ,
            0,
            0,
            checked((uint)prepared.Facts.Length),
            16_384.0f);
    }

    private Prepared Require(WorldOriginPrepared prepared)
        => _prepared.TryGetValue(prepared.Handle.Value, out Prepared? value)
            ? value
            : throw new InvalidOperationException("world-origin prepared handle was unavailable");

    private sealed record Prepared(WorldOriginPrepareRequest Request, WorldOriginAffectedAtReceipt[] Facts);
}

sealed class MotionServiceFake : IMotionService
{
    public int ResolveCount { get; private set; }

    public MotionResolveReceipt Resolve(MotionResolveRequest request)
    {
        ResolveCount++;
        MotionEntityRow mover = request.Entities.Span
            .ToArray()
            .Single(row => row.EntityId == request.TargetEntityId);
        Transform candidate = mover.Transform with
        {
            Translation = mover.Transform.Translation + request.Delta,
        };
        return new MotionResolveReceipt(
            MotionOutcome.Moved,
            false,
            false,
            false,
            false,
            0,
            mover.Transform.Translation,
            candidate.Translation,
            candidate);
    }
}

sealed class DynamicsServiceFake : IDynamicsService
{
    public int StepAndReadCalls { get; private set; }

    public DynamicsWorld CreateWorld(DynamicsWorldConfig request) => throw new NotSupportedException();
    public DynamicsBody CreateBody(DynamicsCreateBodyRequest request) => throw new NotSupportedException();
    public DynamicsBody CreateSphereBody(DynamicsCreateSphereBodyRequest request) => throw new NotSupportedException();
    public DynamicsBody CreateCuboidBody(DynamicsCreateCuboidBodyRequest request) => throw new NotSupportedException();
    public DynamicsBody CreateSphereBodyWithProperties(DynamicsCreateSphereBodyPropertiesRequest request) => throw new NotSupportedException();
    public DynamicsBody CreateCapsuleBody(DynamicsCreateCapsuleBodyRequest request) => throw new NotSupportedException();
    public void BindWorldCollision(DynamicsWorldCollisionBindingRequest request) => throw new NotSupportedException();
    public DynamicsRebaseWorldOriginReceipt RebaseWorldOrigin(DynamicsRebaseWorldOriginRequest request) => throw new NotSupportedException();
    public DynamicsStepReceipt Step(DynamicsStepRequest request) => throw new NotSupportedException();
    public DynamicsReadout Read(DynamicsReadRequest request) => throw new NotSupportedException();
    public void Reset(DynamicsResetRequest request) => throw new NotSupportedException();
    public void UpdateBody(DynamicsUpdateBodyRequest request) => throw new NotSupportedException();
    public DynamicsWorldReadout ReadWorld(DynamicsWorldReadRequest request) => throw new NotSupportedException();
    public DynamicsBodyAtReceipt ReadBodyAt(DynamicsBodyAtRequest request) => throw new NotSupportedException();
    public DynamicsContactAtReceipt ReadContactAt(DynamicsContactAtRequest request) => throw new NotSupportedException();
    public DynamicsBody ReplaceBody(DynamicsReplaceBodyRequest request) => throw new NotSupportedException();
    public DynamicsBody ReplaceCuboidBody(DynamicsReplaceCuboidBodyRequest request) => throw new NotSupportedException();
    public DynamicsBody ReplaceSphereBody(DynamicsReplaceSphereBodyRequest request) => throw new NotSupportedException();
    public DynamicsBody ReplaceCapsuleBody(DynamicsReplaceCapsuleBodyRequest request) => throw new NotSupportedException();

    public DynamicsStepAndReadLeaseReceipt StepAndRead(DynamicsStepAndReadRequest request)
    {
        StepAndReadCalls++;
        if (request.World.Handle.Value != 10 || request.Bodies.Length != 1 || request.Actions.Length > 1)
        {
            throw new InvalidOperationException("Dynamics adapter did not preserve its bounded typed request.");
        }
        DynamicsBody body = request.Bodies.Span[0];
        Vector3 translation = request.Actions.Length == 0
            ? Vector3.Zero
            : request.Actions.Span[0].Force;
        var readout = new DynamicsReadout(
            new Transform(translation, Quaternion.Identity, Vector3.One),
            translation,
            Vector3.Zero,
            false,
            default,
            0,
            default);
        return new DynamicsStepAndReadLeaseReceipt(
            new[] { new DynamicsStepAndReadBody(new DynamicsBodyReference(body.Handle.Value), readout) },
            4,
            1,
            0);
    }
}

sealed class KinematicServiceFake : IKinematicService
{
    public SpatialSession Session { get; } = new(new SpatialSessionHandle(1), () => { });
    public int RunCount { get; private set; }

    public IntegrationResult Integrate(KinematicIntegrationRequest request) => throw new NotSupportedException();

    public IntegrationResult IntegrateSpatial(KinematicSpatialIntegrationRequest request) => throw new NotSupportedException();

    public KinematicMotionLeaseReceipt RunMotion(KinematicMotionRequest request)
    {
        RunCount++;
        if (!request.SelectionPresent)
        {
            throw new InvalidOperationException("example Kinematic test requires explicit selection");
        }
        if (request.SelectedEntityIds.Length == 0)
        {
            return new KinematicMotionLeaseReceipt(
                ReadOnlyMemory<KinematicMotionCandidate>.Empty,
                ReadOnlyMemory<KinematicMotionFact>.Empty,
                0,
                0,
                0,
                0,
                0);
        }
        KinematicMotionEntityRow[] rows = request.Rows.ToArray();
        if (rows.Length != 3 || !request.SelectedEntityIds.Span.SequenceEqual(new ulong[] { 1, 2 }))
        {
            throw new InvalidOperationException("Kinematic adapter did not project deterministic full rows and selection ids");
        }
        KinematicMotionEntityRow mover = rows.Single(row => row.EntityId == 1);
        KinematicMotionEntityRow blocker = rows.Single(row => row.EntityId == 3);
        if (!mover.CollisionEnabled || !blocker.CollisionEnabled)
        {
            throw new InvalidOperationException("Kinematic adapter did not retain active collider facts for dynamic blockers");
        }
        Transform after = mover.Transform with { Translation = new Vector3(2.0f, 0.0f, 0.0f) };
        var candidate = new KinematicMotionCandidate(
            mover.EntityId,
            mover.Transform,
            after,
            mover.Velocity,
            new Vector3(2.0f, 0.0f, 0.0f));
        var facts = new[]
        {
            new KinematicMotionFact(mover.EntityId, KinematicMotionFactKind.Blocked, KinematicMotionAxis.Z, Vector3.Zero, Vector3.Zero, 2.0f),
            new KinematicMotionFact(mover.EntityId, KinematicMotionFactKind.Moved, KinematicMotionAxis.X, mover.Transform.Translation, after.Translation, 0.0f),
        };
        return new KinematicMotionLeaseReceipt(new[] { candidate }, facts, 2, 1, 1, 0, 1);
    }
}

readonly record struct Health(int Current);
readonly record struct Armor(int Current);
readonly record struct EntityCheckpoint(int Health);

sealed class EntityCheckpointCodec : IProductStateCodec<EntityCheckpoint>
{
    private const int PayloadLength = 1;

    public uint SchemaVersion => 1;

    public void Encode(in EntityCheckpoint state, IBufferWriter<byte> destination)
    {
        destination.GetSpan(PayloadLength)[0] = checked((byte)state.Health);
        destination.Advance(PayloadLength);
    }

    public EntityCheckpoint Decode(ReadOnlySpan<byte> payload)
        => payload.Length == PayloadLength
            ? new EntityCheckpoint(payload[0])
            : throw new InvalidOperationException("entity checkpoint payload had an unexpected length");
}

sealed class RecordingResolutionTransaction : IResolutionTransaction
{
    public bool Staged { get; private set; }
    public bool Committed { get; private set; }
    public bool Aborted { get; private set; }

    public void Stage() => Staged = true;
    public void Commit() => Committed = true;
    public void Abort() => Aborted = true;
}
