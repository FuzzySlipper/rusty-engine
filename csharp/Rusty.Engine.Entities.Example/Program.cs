using System.Buffers;
using System.Numerics;
using System.Text.Json;
using System.Text.Json.Serialization;
using System.Text.Json.Serialization.Metadata;
using Rusty.Engine;
using Rusty.Engine.Entities;
using Rusty.Engine.Mechanics;
using Rusty.Engine.Persistence;
using Rusty.Engine.StateMachine;
using MechanicsStackingPolicy = Rusty.Engine.Mechanics.MechanicsStackingPolicy;
using StateMachineDefinition = Rusty.Engine.StateMachine.StateMachineDefinition;
using StateMachineInstance = Rusty.Engine.StateMachine.StateMachineInstance;
using StateMachineTransition = Rusty.Engine.StateMachine.StateMachineTransition;
using StateMachineTransitionReceipt = Rusty.Engine.StateMachine.StateMachineTransitionReceipt;
using StateMachineTransitionRequest = Rusty.Engine.StateMachine.StateMachineTransitionRequest;

// This executable is a deliberately broad managed-helper proof harness. It is not a recommended
// product architecture or a template for assembling unrelated gameplay domains in one program.
EntityAdapterSafetyExercise.Run();

const uint HealthLocalComponentId = 1;
const uint ArmorLocalComponentId = 2;
const int InitialHealth = 10;
const int InitialArmor = 3;

var health = ComponentType<Health>.Create(
    ProductComponentKeys.Create(HealthLocalComponentId),
    validator: ValidateHealth);
var armor = ComponentType<Armor>.Create(ProductComponentKeys.Create(ArmorLocalComponentId));
using var world = new EntityStore([EngineComponentTypes.Transform, EngineComponentTypes.CharacterMotion, health, armor]);

EntityId actor = world.Create();
EntityId pack = world.Create();
EntityId pouch = world.Create(EntityLifecycle.Disabled);
ContainmentReceipt contained = world.SetContainment(pouch, pack, world.Revision);
Require(contained.Changed && world.TryGetContainedIn(pouch, out EntityId container) && container == pack,
    "canonical containment did not preserve its parent");
Require(world.ContainedEntities(pack).SequenceEqual([pouch]), "reverse containment was not deterministic");
Throws(() => world.SetContainment(pack, pouch), "containment cycle was not rejected");
world.ClearContainment(pouch);
world.SetContainment(pouch, pack);
Require(world.TryGetContainedIn(pouch, out container) && container == pack, "explicit reparenting lost containment");
world.Set(actor, health, new Health(InitialHealth));
Throws(() => world.Set(actor, health, new Health(-1)), "typed component validator did not reject invalid state");
world.Set(actor, armor, new Armor(InitialArmor));
ComponentRevision healthRevision = world.GetComponentRevision(actor, health);

ulong revisionBeforeEdit = world.Revision;
EntityBatchReceipt receipt = world.Commit(new EntityBatch()
    .Set(actor, health, new Health(6), healthRevision), expectedRevision: revisionBeforeEdit);
Require(receipt.RevisionAfter == revisionBeforeEdit + 1, "a prepared edit should publish one structural version");
world.SetLifecycle(actor, EntityLifecycle.Disabled);
Require(world.Query(health).Count == 0, "disabled entities are omitted from normal queries");
Require(world.Query(health, includeDisabled: true).Single().Value.Current == 6, "typed replacement did not commit");
Require(world.Query(health, armor, includeDisabled: true).Single().Second.Current == InitialArmor, "two-component query did not join typed columns");
ulong beforeRejectedEdit = world.Revision;
Throws(
    () => world.Commit(new EntityBatch()
        .Set(actor, health, new Health(4))
        .Set(new EntityId(999), health, new Health(1))),
    "a rejected edit must report its invalid value replacement");
Require(world.Get(actor, health).Current == 6 && world.Revision == beforeRejectedEdit, "a rejected edit changed live state");
world.SetLifecycle(actor, EntityLifecycle.Active);
world.Set(actor, health, new Health(InitialHealth));
Throws(() => world.Set(actor, health, new Health(9), healthRevision), "an old component guard accepted an explicit replacement");
Require(world.Diagnostics().Components.Single(component => component.Key == health.Key).ValueCount == 1, "diagnostics lost the component table");

ClassComponentExercise.Run();
ActorExercise.Run();
MechanicsPersistenceExercise.Run();
ExerciseJsonPersistence();
ExercisePreparedValueEdits();
ExerciseEntityPersistence(world, actor, health);
ExerciseManagedMechanics();
ExerciseManagedStateMachine();

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
    catch (Exception exception) when (exception is InvalidOperationException or ArgumentException or ObjectDisposedException)
    {
        return;
    }
    throw new InvalidOperationException(message);
}

static bool IsOrWrapsProbe(Exception? error)
{
    for (Exception? current = error; current is not null; current = current.InnerException)
    {
        if (current is ResolverProbeException)
        {
            return true;
        }
    }
    return false;
}

static void ExercisePreparedValueEdits()
{
    ComponentType<ReferenceComponent> references = ComponentType<ReferenceComponent>.Create(ProductComponentKeys.Create(90));
    ComponentType<int> values = ComponentType<int>.Create(ProductComponentKeys.Create(91));
    using var store = new EntityStore([references, values]);
    EntityId entity = store.Create();
    int[] source = [1];
    store.Set(entity, references, new ReferenceComponent(source));
    source[0] = 99;
    Require(store.Get(entity, references).Values[0] == 99, "ordinary values must retain C# nested reference semantics");
    store.Set(entity, values, 1);

    using EntityEdit stale = store.PrepareBatch(new EntityBatch().Set(entity, values, 2));
    store.Register(ComponentType<long>.Create(ProductComponentKeys.Create(93)));
    Throws(() => stale.Publish(), "stale edit overwrote a later registration");
    try
    {
        stale.Publish();
        throw new Exception("failed edit was reusable");
    }
    catch (InvalidOperationException error)
    {
        Require(error.Message.Contains("failed or been disposed", StringComparison.Ordinal), "failed edit did not become terminal");
    }
    Require(store.Get(entity, values) == 1, "stale edit discarded live state");

    using EntityEdit canceled = store.PrepareBatch(new EntityBatch().Set(entity, values, 3));
    canceled.Dispose();
    Throws(() => canceled.Publish(), "disposed edit was reusable");
    using EntityEdit published = store.PrepareBatch(new EntityBatch().Set(entity, values, 4));
    published.Publish();
    ulong afterPublish = store.Revision;
    published.Publish();
    Require(store.Revision == afterPublish && store.Get(entity, values) == 4, "successful publication was not idempotent");
    Require(ReferenceEquals(store.Get(entity, references).Values, source), "prepared value edit copied an unrelated family");
    Throws(() => store.Create((EntityLifecycle)99), "create admitted an undeclared lifecycle");
    store.Dispose();
    Throws(() => store.Diagnostics(), "disposed store allowed diagnostics");
}

static void ValidateHealth(in Health health)
{
    if (health.Current < 0)
    {
        throw new ArgumentOutOfRangeException(nameof(health), "Health cannot be negative.");
    }
}

static void ExerciseEntityPersistence(
    EntityStore world,
    EntityId actor,
    ComponentType<Health> health)
{
    var persistence = new InMemoryPersistenceService();
    using var store = new ProductStateStore<EntityCheckpoint>(
        new PersistenceEngineContext(persistence), "entities-example", new EntityCheckpointCodec());
    PersistenceSaveReceipt saved = store.Save("checkpoint", new EntityCheckpoint(world.Get(actor, health).Current));
    Require(saved.Outcome == PersistenceSaveOutcome.Saved,
        "the ordinary save did not report success");
    world.Set(actor, health, new Health(4));
    ProductStateLoad<EntityCheckpoint> loaded = store.Load("checkpoint");
    Require(loaded.Present && loaded.State.Health == InitialHealth && loaded.Revision == saved.Revision,
        "product-owned persistence did not decode the selected value with its revision");
    // This product owns the adoption. For a multi-object save, build and validate
    // a replacement graph before swapping its owner; the byte store never rolls back gameplay.
    world.Set(actor, health, new Health(loaded.State.Health));
    Require(world.Get(actor, health).Current == InitialHealth,
        "explicit product adoption did not apply the loaded value");
    // Guards stay optional with real storage meaning: the store forwards them untouched.
    // Enforcement itself lives in the Rust service (covered by its own tests); the fake
    // records rather than enforces, so this pins forwarding, not conflict behavior.
    store.Save("checkpoint", new EntityCheckpoint(InitialHealth), PersistenceRevisionGuard.Exact, loaded.Revision);
    Require(persistence.LastRevisionGuard == PersistenceRevisionGuard.Exact
        && persistence.LastExpectedRevision == loaded.Revision,
        "the revision guard was not forwarded to storage");
    // Absent keys report absent, and malformed bytes fail in the codec rather than partially succeeding.
    ProductStateLoad<EntityCheckpoint> missing = store.Load("never-saved");
    Require(!missing.Present && missing.Revision == 0,
        "a missing save did not report absent");
    persistence.Seed("entities-example", "corrupt", [0xFF, 0xFF]);
    Exception? malformed = null;
    try
    {
        store.Load("corrupt");
    }
    catch (InvalidOperationException error) when (error.Message.Contains("unexpected length", StringComparison.Ordinal))
    {
        malformed = error;
    }
    Require(malformed is not null, "malformed current bytes did not fail in the codec");
}

static void ExerciseJsonPersistence()
{
    var log = new QuestLog
    {
        Title = "Harbor Lights",
        Objectives =
        [
            new QuestObjective { Name = "Light the beacon", Done = true },
            new QuestObjective { Name = "Return to the harbor", Done = false },
        ],
        Reputation = new Dictionary<string, int> { ["harbor"] = 3, ["beacon"] = 1 },
    };
    // The payload stays focused on product state, without storage compatibility
    // metadata or a migration branch.
    string json = JsonSerializer.Serialize(log, QuestLogJsonContext.Default.QuestLog);
    Require(!json.Contains("schema", StringComparison.OrdinalIgnoreCase)
        && !json.Contains("migrat", StringComparison.OrdinalIgnoreCase)
        && !json.Contains("fingerprint", StringComparison.OrdinalIgnoreCase),
        "the example save shape smuggled in versioning vocabulary");

    var persistence = new InMemoryPersistenceService();
    using var store = new ProductStateStore<QuestLog>(
        new PersistenceEngineContext(persistence), "json-example",
        new JsonProductStateCodec<QuestLog>(QuestLogJsonContext.Default.QuestLog));
    PersistenceSaveReceipt saved = store.Save("quest", log);
    Require(saved.Outcome == PersistenceSaveOutcome.Saved,
        "the JSON save did not report success");
    ProductStateLoad<QuestLog> loaded = store.Load("quest");
    QuestLog? quest = loaded.State;
    Require(loaded.Present && loaded.Revision == saved.Revision && quest is not null
        && quest.Title == "Harbor Lights"
        && quest.Objectives.Count == 2
        && quest.Objectives[0].Name == "Light the beacon" && quest.Objectives[0].Done
        && !quest.Objectives[1].Done
        && quest.Reputation["harbor"] == 3 && quest.Reputation["beacon"] == 1,
        "the JSON roundtrip did not preserve nested data and collections");

    ProductStateLoad<QuestLog> missing = store.Load("never-saved");
    Require(!missing.Present && missing.Revision == 0, "a missing JSON save did not report absent");
    persistence.Seed("json-example", "corrupt", "{not json"u8.ToArray());
    JsonException? malformed = null;
    try
    {
        store.Load("corrupt");
    }
    catch (JsonException error)
    {
        malformed = error;
    }
    Require(malformed is not null, "invalid JSON did not fail with an understandable error");
    persistence.Seed("json-example", "nulldoc", "null"u8.ToArray());
    InvalidOperationException? nullDoc = null;
    try
    {
        store.Load("nulldoc");
    }
    catch (InvalidOperationException error) when (error.Message.Contains("decoded to null", StringComparison.Ordinal))
    {
        nullDoc = error;
    }
    Require(nullDoc is not null, "a JSON null document did not fail the load");

    // The options overload is CoreCLR convenience over reflection-based metadata: prove it
    // resolves the right type and honors its options through public behavior. Trailing commas
    // decode only because AllowTrailingCommas flows through; default options would throw.
    using var trailingStore = new ProductStateStore<QuestLog>(
        new PersistenceEngineContext(persistence), "json-example",
        new JsonProductStateCodec<QuestLog>(new JsonSerializerOptions { AllowTrailingCommas = true }));
    persistence.Seed("json-example", "trailing", """{"Title":"Seeded","Objectives":[],"Reputation":{},}"""u8.ToArray());
    ProductStateLoad<QuestLog> trailing = trailingStore.Load("trailing");
    Require(trailing.Present && trailing.State is not null && trailing.State.Title == "Seeded",
        "the options-based codec did not honor its serialization options");

    // The caller's configured chain is consulted before any reflection fallback: a resolver
    // that explodes on consultation must surface its own failure, not a metadata error.
    var probeOptions = new JsonSerializerOptions();
    var probe = new ExplodingResolver();
    probeOptions.TypeInfoResolverChain.Add(probe);
    Exception? probeFailure = null;
    try
    {
        _ = new JsonProductStateCodec<QuestLog>(probeOptions);
    }
    catch (Exception error)
    {
        probeFailure = error;
    }
    Require(probe.Consulted && IsOrWrapsProbe(probeFailure),
        "the configured resolver was not consulted first");

    // Null options mean the shared defaults; reflection still resolves the shape.
    using var defaultStore = new ProductStateStore<QuestLog>(
        new PersistenceEngineContext(persistence), "json-example",
        new JsonProductStateCodec<QuestLog>((JsonSerializerOptions?)null));
    persistence.Seed("json-example", "defaulted", """{"Title":"Defaulted","Objectives":[],"Reputation":{}}"""u8.ToArray());
    ProductStateLoad<QuestLog> defaulted = defaultStore.Load("defaulted");
    Require(defaulted.Present && defaulted.State is not null && defaulted.State.Title == "Defaulted",
        "null options did not resolve shared-default metadata");
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

    var strength = new Stat(BaseStrength, minimum: 0, maximum: MaximumHealth);
    strength.AddModifier(StrengthBonus);
    Require(strength.ValueInt64 == BaseStrength + StrengthBonus,
        "ordinary strength modifier did not apply");
    var speed = new Stat(BaseSpeed, minimum: 0, maximum: 10);
    speed.AddModifier(SpeedBonus);
    Require(speed.Value == BaseSpeed + SpeedBonus,
        "ordinary stat did not retain fractional speed");

    var healthTrack = new Track(MaximumHealth, current: StartingHealth);
    double healthSpent = healthTrack.Spend(HealthSpend);
    Require(healthTrack.Value == StartingHealth - HealthSpend && healthSpent == HealthSpend,
        "direct managed track mutation did not update product-owned state");

    var staminaTrack = new Track(MaximumStamina, current: StartingStamina);
    double staminaSpent = staminaTrack.Spend(StaminaSpend);
    Require(Math.Abs(staminaTrack.Value - (StartingStamina - StaminaSpend)) < 0.0001
        && Math.Abs(staminaSpent - StaminaSpend) < 0.0001,
        "direct managed track mutation did not update product-owned state");

    var inventory = new InventoryStore();
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
    EquipmentMutationReceipt equipped = inventory.Equip(hero, swordEntity, [mainHand]);
    Require(equipped.SourceActivations.Count == 1
        && inventory.View(hero).UniqueItems.Single().Entity == swordEntity,
        "managed equipment did not publish the equipped item and its source activation");

    Throws(
        () => inventory.TransferUnique(swordEntity, hero, chest),
        "managed inventory allowed an equipped unique item to transfer");
    inventory.Unequip(hero, swordEntity);
    ItemTransferReceipt transferred = inventory.TransferUnique(swordEntity, hero, chest);
    Require(transferred.ToOwner == chest
        && inventory.View(chest).UniqueItems.Single().Entity == swordEntity,
        "managed inventory did not transfer the unequipped unique item");

    InventoryEdit candidate = inventory.Prepare(inventory.Revision);
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

static void ExerciseWorldOriginEntityComposition()
{
    const uint GlobalPositionLocalComponentId = 40;
    var globalPositions = ComponentType<WorldOriginGlobalPosition>.Create(
        ProductComponentKeys.Create(GlobalPositionLocalComponentId));
    using var world = new EntityStore([EngineComponentTypes.Transform, globalPositions]);
    EntityId entity = world.Create();
    world.Set(entity, EngineComponentTypes.Transform, new Transform(
        new Vector3(100.0f, 2.0f, -3.0f), Quaternion.Identity, new Vector3(2.0f, 3.0f, 4.0f)));
    world.Set(entity, globalPositions, new WorldOriginGlobalPosition(100, 2, -3, 0.0, 0.0, 0.0));
    var service = new WorldOriginServiceFake();
    var adapter = new EntityOriginRebaser(world, service, service.Session, globalPositions);

    using EntityOriginRebaserPrepared prepared = adapter.Prepare(100, 0, 0, maximumEntities: 1);
    Require(prepared.Receipt.Native.AffectedEntityCount == 1
        && prepared.Receipt.Affected.Span[0].EntityId == entity.Value,
        "world-origin prepare did not retain one deterministic root fact");
    EntityOriginRebaserCommitReceipt committed = prepared.Commit();
    Require(committed.Native.OriginAfterCellX == 100
        && world.Get(entity, EngineComponentTypes.Transform).Translation.X == 0.0f,
        "world-origin commit did not pair the native receipt with one managed transform batch");

    using EntityOriginRebaserPrepared stale = adapter.Prepare(200, 0, 0, maximumEntities: 1);
    world.Set(entity, EngineComponentTypes.Transform, new Transform(
        new Vector3(1.0f, 2.0f, -3.0f), Quaternion.Identity, new Vector3(2.0f, 3.0f, 4.0f)));
    Throws(() => stale.Commit(), "world-origin candidate did not reject stale managed transform state");
    Require(service.CommitCount == 1,
        "stale managed world state crossed into the native world-origin commit");
}

static void ExerciseMotionEntityComposition()
{
    using var world = new EntityStore([EngineComponentTypes.Transform, EngineComponentTypes.SpatialCollider]);
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
    var adapter = new EntityMotionResolver(world, service, EngineComponentTypes.SpatialCollider);

    EntityMotionResolverReceipt moved = adapter.Resolve(mover, new Vector3(1.0f, 0.0f, 0.0f), maximumEntities: 2);
    Require(moved.Resolution.Outcome == MotionOutcome.Moved
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
    using var world = new EntityStore([
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
    var adapter = new EntityKinematicMotion(world, service, EngineComponentTypes.SpatialCollider);

    ulong before = world.Revision;
    EntityKinematicMotionPrepared prepared = adapter.Prepare(
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
    EntityKinematicMotionReceipt applied = prepared.Apply();
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
    EntityKinematicMotionReceipt noOp = adapter.Prepare(
        service.Session,
        1.0f,
        3,
        ReadOnlyMemory<EntityId>.Empty).Apply();
    Require(noOp.Managed.RevisionBefore == noOpBefore && noOp.Managed.RevisionAfter == noOpBefore,
        "Kinematic empty selected phase changed the managed world revision");
}

static void ExerciseDynamicsEntityComposition()
{
    using var entities = new EntityStore([
        EngineComponentTypes.Transform,
        EngineComponentTypes.DynamicsMotion]);
    EntityId entity = entities.Create();
    entities.Set(entity, EngineComponentTypes.Transform, new Transform(Vector3.Zero, Quaternion.Identity, new Vector3(2.0f, 3.0f, 4.0f)));
    entities.Set(entity, EngineComponentTypes.DynamicsMotion, new DynamicsMotion(Vector3.Zero, Vector3.Zero, false));
    var service = new DynamicsServiceFake();
    using var dynamicsWorld = new DynamicsWorld(new DynamicsWorldHandle(10), static () => { });
    using var body = new DynamicsBody(new DynamicsBodyHandle(20), static () => { });
    var adapter = new EntityDynamicsAdapter(entities, service, dynamicsWorld);

    ulong before = entities.Revision;
    EntityDynamicsAdapterReceipt receipt = adapter.Step(
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
    using var world = new EntityStore([EngineComponentTypes.Transform, EngineComponentTypes.SpatialCollider]);
    var spatial = new SpatialServiceFake();
    using var session = new SpatialSession(new SpatialSessionHandle(1), () => { });
    var adapter = new EntityTriggerProjection(world, spatial, session, EngineComponentTypes.SpatialCollider);
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

    EntityTriggerProjectionReconcileReceipt receipt = adapter.ReconcileTriggers(
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
    EntityTriggerProjectionGuard staleComponentGuard = receipt.Guard with { StoreRevision = world.Revision };
    Throws(
        () => adapter.ReconcileTriggers(8, SpatialTriggerCause.Movement, 4, 1, staleComponentGuard),
        "stale spatial component guard was accepted");
    Require(spatial.ReconcileCalls == 1, "stale spatial projection crossed into the generated service");
}

static void ExerciseCharacterEntityComposition()
{
    using var world = new EntityStore([EngineComponentTypes.Transform, EngineComponentTypes.CharacterMotion]);
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
    var adapter = new EntityCharacterController(world, spatial);
    var command = new CharacterControllerCommand(
        Vector2.Zero, 0, false, false, false, Vector3.Zero, Vector3.Zero, 1.0f / 60.0f, 7);

    ulong before = world.Revision;
    EntityCharacterControllerReceipt receipt = adapter.Step(
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
    using var world = new EntityStore([EngineComponentTypes.Transform]);
    EntityId first = world.Create();
    EntityId second = world.Create();
    world.Set(first, EngineComponentTypes.Transform, new Transform(Vector3.Zero, Quaternion.Identity, Vector3.One));
    world.Set(second, EngineComponentTypes.Transform, new Transform(new Vector3(2, 0, 0), Quaternion.Identity, Vector3.One));
    var graphics = new GraphicsServiceFake();
    using var firstHandle = new Appearance(new AppearanceHandle(10), () => { });
    using var secondHandle = new Appearance(new AppearanceHandle(20), () => { });
    var adapter = new EntityGraphicsProjection(world, graphics);
    EntityGraphicsProjectionEntry[] entries =
    [
        new(second, secondHandle, true, RenderLayer.Debug, first),
        new(first, firstHandle, true, RenderLayer.Scene),
    ];

    EntityGraphicsProjectionReceipt receipt = adapter.Publish(entries, maximumEntities: 2);
    Require(graphics.PublishCalls == 1
        && receipt.Facts.Span.Length == 2
        && graphics.LastSnapshot.Span[0].ObjectId == first.Value
        && graphics.LastSnapshot.Span[0].Appearance == firstHandle
        && graphics.LastSnapshot.Span[1].ObjectId == second.Value
        && graphics.LastSnapshot.Span[1].Appearance == secondHandle
        && graphics.LastSnapshot.Span[1].HasParentObject
        && graphics.LastSnapshot.Span[1].ParentObjectId == first.Value,
        "graphics adapter did not publish caller-owned handles and hierarchy in parent-before-child order");

    Throws(
        () => adapter.Publish(
            new EntityGraphicsProjectionEntry[] { new(second, secondHandle, true, RenderLayer.Debug, new EntityId(999)) },
            maximumEntities: 1),
        "graphics adapter accepted a parent that was absent from its complete snapshot");

    world.Set(first, EngineComponentTypes.Transform, new Transform(Vector3.UnitY, Quaternion.Identity, Vector3.One));
    Throws(
        () => adapter.Publish(entries, maximumEntities: 2, receipt.Guard),
        "appearance adapter accepted a stale managed transform projection");
    Require(graphics.PublishCalls == 1,
        "stale appearance managed state reached the generated service");
}

sealed class SpatialServiceFake : ISpatialService
{
    public int ReconcileCalls { get; private set; }
    public int CharacterStepCalls { get; private set; }
    public SpatialSession Session { get; } = new(new SpatialSessionHandle(2), () => { });
    private SpatialEntityCollider[] _entities = [];

    public SpatialSession CreateSession(SpatialSessionConfig arg0) => throw new NotSupportedException();
    public CollisionReplaceReceipt ReplaceCollision(CollisionReplaceRequest arg0) => throw new NotSupportedException();
    public SpatialContentArtifactReplaceReceipt ReplaceContentArtifact(SpatialContentArtifactReplaceRequest arg0) => throw new NotSupportedException();
    public SpatialContentArtifactReadout ReadContentArtifact(SpatialContentArtifactReadRequest arg0) => throw new NotSupportedException();
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
    public void ValidateCharacterControllerConfig(CharacterControllerConfig arg0) => throw new NotSupportedException();
    public void ValidateCharacterControllerCommand(CharacterControllerValidationRequest arg0) => throw new NotSupportedException();
    public CharacterContinuationCheckpoint CaptureCharacterContinuation(CharacterContinuationCaptureRequest arg0) => throw new NotSupportedException();
    public CharacterContinuationRestoreReceipt RestoreCharacterContinuation(CharacterContinuationRestoreRequest arg0) => throw new NotSupportedException();

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
    public NavigationStepReceipt EvaluateNavigationStep(NavigationStepRequest arg0) => throw new NotSupportedException();
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

    public SpatialTriggerLifecycleReceipt SetTriggerActive(SpatialTriggerSetActiveRequest request) =>
        throw new NotSupportedException();

    public SpatialTriggerRestoreReceipt RestoreTriggers(SpatialTriggerRestoreRequest request) =>
        throw new NotSupportedException();

    public SpatialTriggerReadReceipt ReadTrigger(SpatialTriggerReadRequest arg0) => throw new NotSupportedException();
    public SpatialTriggerOverlapAtReceipt ReadTriggerOverlapAt(SpatialTriggerOverlapAtRequest arg0) => throw new NotSupportedException();
    public SpatialTriggerOverlapPageLeaseReceipt ReadTriggerOverlapPage(SpatialTriggerOverlapPageRequest request)
    {
        if (request.PageSize == 0 || request.Cursor != 0)
        {
            throw new ArgumentOutOfRangeException(nameof(request));
        }

        ulong revision = (ulong)ReconcileCalls;
        if (request.ExpectedRevision != 0 && request.ExpectedRevision != revision)
        {
            throw new InvalidOperationException("trigger overlap continuation revision is stale");
        }

        return new SpatialTriggerOverlapPageLeaseReceipt(
            ReadOnlyMemory<SpatialTriggerOverlapSubject>.Empty,
            request.Trigger,
            revision,
            0,
            false,
            0);
    }

    public SpatialTriggerFactAtReceipt ReadTriggerFactAt(SpatialTriggerFactAtRequest request)
        => request.Index == 0 && _entities.Length != 0
            ? new SpatialTriggerFactAtReceipt(true, true, _entities[0].Entity, _entities[0].Entity, 7, SpatialTriggerCause.Movement)
            : default;
}

sealed class GraphicsServiceFake : IGraphicsService
{
    private AppearanceFact[] _lastSnapshot = [];

    public int PublishCalls { get; private set; }

    public ReadOnlyMemory<AppearanceFact> LastSnapshot => _lastSnapshot;

    public RenderResourceInfo OpenResource(RenderResourceRequest arg0) => throw new NotSupportedException();
    public RenderResourceInfo OpenResourceFromContent(RenderResourceContentRequest arg0) => throw new NotSupportedException();
    public Appearance CreateStaticMeshFromContentReference(StaticMeshContentReferenceRequest arg0) => throw new NotSupportedException();
    public Material CreateMaterial(MaterialRequest arg0) => throw new NotSupportedException();
    public Material CreateAuthoredMaterial(AuthoredMaterialAppearanceRequest arg0) => throw new NotSupportedException();
    public void UpdateMaterial(MaterialUpdateRequest arg0) => throw new NotSupportedException();
    public Material ReplaceMaterial(MaterialUpdateRequest arg0) => throw new NotSupportedException();
    public Appearance CreatePrimitive(PrimitiveAppearanceRequest arg0) => throw new NotSupportedException();
    public Appearance ReplacePrimitive(PrimitiveAppearanceReplaceRequest arg0) => throw new NotSupportedException();
    public MeshPartition PartitionMesh(MeshPartitionRequest request) => throw new NotSupportedException();
    public MeshPartitionReadout ReadMeshPartition(MeshPartition partition) => throw new NotSupportedException();
    public MeshResource TakeMeshPartitionPart(MeshPartitionPartRequest request) => throw new NotSupportedException();
    public MeshResource CreateMeshResource(MeshResourceCreateRequest arg0) => throw new NotSupportedException();
    public Appearance CreateMeshAppearance(MeshResource arg0) => throw new NotSupportedException();
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
    public void SetSpriteViewport(SpriteViewportUpdateRequest arg0) => throw new NotSupportedException();
    public SpriteReadout ReadSprite(Appearance arg0) => throw new NotSupportedException();
    public SpritePlayback CreateSpritePlayback(SpritePlaybackCreateRequest arg0) => throw new NotSupportedException();
    public SpritePlaybackReadout ControlSpritePlayback(SpritePlaybackControlRequest arg0) => throw new NotSupportedException();
    public SpritePlaybackReadout SelectSpritePlaybackFrame(SpritePlaybackFrameSelectionRequest arg0) => throw new NotSupportedException();
    public SpritePlaybackAdvanceLeaseReceipt AdvanceSpritePlayback(SpritePlaybackAdvanceRequest arg0) => throw new NotSupportedException();
    public SpritePlaybackSample SampleSpritePlayback(SpritePlaybackSampleRequest arg0) => throw new NotSupportedException();
    public SpritePlaybackReadout ReadSpritePlayback(SpritePlayback arg0) => throw new NotSupportedException();

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
    public IDiagnosticsService Diagnostics => throw new NotSupportedException();
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
    public IGraphicsService Graphics => throw new NotSupportedException();
    public IImplicitSurfacesService ImplicitSurfaces => throw new NotSupportedException();
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
    private sealed record Saved(ulong Revision, byte[] Payload);

    private readonly Dictionary<ulong, string> _scopes = [];
    private readonly Dictionary<ulong, Saved> _blobs = [];
    private readonly Dictionary<(string Scope, string Key), Saved> _saved = [];
    private ulong _nextHandle = 1;

    public PersistenceRevisionGuard LastRevisionGuard { get; private set; } = PersistenceRevisionGuard.Any;
    public ulong LastExpectedRevision { get; private set; }

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
        _saved[key] = new Saved(revision, request.Payload.ToArray());
        LastRevisionGuard = request.RevisionGuard;
        LastExpectedRevision = request.ExpectedRevision;
        return new PersistenceSaveReceipt(revision);
    }

    public PersistenceBlob Load(PersistenceLoadRequest request)
    {
        string scope = _scopes[request.Store.Handle.Value];
        _saved.TryGetValue((scope, request.Key), out Saved? saved);
        ulong handle = _nextHandle++;
        _blobs.Add(handle, saved ?? new Saved(0, []));
        return new PersistenceBlob(new PersistenceBlobHandle(handle), () => _blobs.Remove(handle));
    }

    public PersistenceBlobInfo DescribeBlob(PersistenceBlob blob)
    {
        Saved saved = _blobs[blob.Handle.Value];
        return new PersistenceBlobInfo(saved.Revision != 0, saved.Revision, (nuint)saved.Payload.Length);
    }

    public void CopyBlob(PersistenceCopyBlobRequest request)
        => _blobs[request.Blob.Handle.Value].Payload.CopyTo(request.Destination.Span);

    public ReadOnlyMemory<byte> ReadBlobBytes(PersistenceBlob blob)
        => _blobs[blob.Handle.Value].Payload;

    public void Seed(string scope, string key, byte[] payload)
        => _saved[(scope, key)] = new Saved(1, payload);
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
        MotionSpatialEntity mover = request.Entities.Span
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
readonly record struct ReferenceComponent(int[] Values);
readonly record struct EntityCheckpoint(int Health);

sealed class QuestObjective
{
    public string Name { get; set; } = string.Empty;
    public bool Done { get; set; }
}

sealed class QuestLog
{
    public string Title { get; set; } = string.Empty;
    public List<QuestObjective> Objectives { get; set; } = [];
    public Dictionary<string, int> Reputation { get; set; } = new();
}

[JsonSerializable(typeof(QuestLog))]
internal partial class QuestLogJsonContext : JsonSerializerContext
{
}

/// <summary>Test-only failure proving resolver consultation order.</summary>
sealed class ResolverProbeException : Exception;

/// <summary>Test-only resolver exploding when consulted, instead of covering any type.</summary>
sealed class ExplodingResolver : IJsonTypeInfoResolver
{
    public bool Consulted { get; private set; }

    public JsonTypeInfo? GetTypeInfo(Type type, JsonSerializerOptions options)
    {
        Consulted = true;
        throw new ResolverProbeException();
    }
}

sealed class EntityCheckpointCodec : IProductStateCodec<EntityCheckpoint>
{
    private const int PayloadLength = 1;

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
