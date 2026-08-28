using Rusty.Engine;
using Rusty.Engine.Entities;
using Rusty.Engine.Persistence;

const uint HealthLocalComponentId = 1;
const uint ArmorLocalComponentId = 2;
var health = ComponentType<Health>.Create(ProductComponentKeys.Create(HealthLocalComponentId), validator: ValidateHealth);
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
world.Set(actor, health, new Health(10));
Throws(() => world.Set(actor, health, new Health(-1)), "typed component validator did not reject invalid state");
world.Set(actor, armor, new Armor(3));
ComponentRevision healthRevision = world.GetComponentRevision(actor, health);

EntityWorldSnapshot snapshot = world.Snapshot();
EntityBatchReceipt receipt = world.Commit(new EntityBatch()
    .Mutate(staged => staged.Set(actor, health, new Health(6), healthRevision))
    .Mutate(staged => staged.SetLifecycle(actor, EntityLifecycle.Disabled)), expectedRevision: snapshot.Revision);

Require(receipt.RevisionAfter == snapshot.Revision + 1, "a successful batch must advance the world revision exactly once");
Require(world.Query(health).Count == 0, "disabled entities are omitted from normal queries");
Require(world.Query(health, includeDisabled: true).Single().Value.Current == 6, "typed batch mutation did not commit");
Require(world.Query(health, armor, includeDisabled: true).Single().Second.Current == 3, "two-component query did not join typed columns");

Throws(
    () => world.Commit(new EntityBatch()
        .Mutate(staged => staged.Set(actor, health, new Health(4)))
        .Mutate(staged => staged.Set(new EntityId(999), health, new Health(1))), expectedRevision: receipt.RevisionAfter),
    "a rejected batch must report its invalid staged mutation");
Require(world.Get(actor, health).Current == 6 && world.Revision == receipt.RevisionAfter, "a rejected batch changed live state");

world.Restore(snapshot, expectedRevision: receipt.RevisionAfter);
Require(world.Get(actor, health).Current == 10, "in-memory snapshot restore did not recover the typed value");
Throws(() => world.Set(actor, health, new Health(9), healthRevision), "snapshot restore must invalidate old component guards");
Require(world.Diagnostics().Components.Single(component => component.Key == health.Key).ValueCount == 1, "diagnostics lost the component table");
ExerciseManagedRestorePlan(world, actor, pack, health, armor);

ExerciseMechanicsLeaseRebind();
ExerciseMechanicsUniqueItemTransfer();
ExerciseMechanicsUniqueItemLifecycle();
ExerciseMechanicsWorldRestore();
ExerciseMechanicsWorldImport();
ExerciseMechanicsWorldPersistenceComposition();
ExerciseContinuousMechanicsSibling();
ExerciseContinuousMechanicsComposition();

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

static void ExerciseMechanicsLeaseRebind()
{
    using var world = new EntityWorld();
    EntityId actor = world.Create();
    var service = new MechanicsAdapterFake();
    using (var mechanics = new MechanicsEntityWorld(world, service, service.Catalog))
    {
        mechanics.Bind(actor, "actor");
        mechanics.SetInitialStat(actor, "strength", 10);
        MechanicsEntityReceipt receipt = mechanics.Commit(actor);
        Require(receipt.StatsSlot.Present && !receipt.InventoryRevision.Present, "fixed mechanics family slots were not preserved");
        Require(receipt.Lifecycle.Stamp == MechanicsAdapterFake.InitialLifecycleStamp, "initial lifecycle stamp was not retained");
    }
    Require(service.ReleasedLeases == 1, "disposing an adapter must release its committed native lease");
    using (var mechanics = new MechanicsEntityWorld(world, service, service.Catalog))
    {
        mechanics.Rebind(actor, MechanicsAdapterFake.InitialLifecycleStamp, world.GetEntityRevision(actor));
        mechanics.SetLifecycle(actor, EntityLifecycle.Tombstoned, world.GetEntityRevision(actor));
    }
    Require(service.Rebinds == 1 && service.ReleasedLeases == 2, "released canonical mechanics state was not rebindable");
    Require(world.GetLifecycle(actor) == EntityLifecycle.Tombstoned, "explicit adapter lifecycle transition did not preserve product ownership");
}

static void ExerciseMechanicsUniqueItemTransfer()
{
    using var world = new EntityWorld();
    EntityId source = world.Create();
    EntityId destination = world.Create();
    EntityId item = world.Create();
    world.SetContainment(item, source, world.Revision);
    var service = new MechanicsAdapterFake();
    using var mechanics = new MechanicsEntityWorld(world, service, service.Catalog);
    mechanics.Bind(source, "source");
    mechanics.Bind(destination, "destination");
    mechanics.Bind(item, "item");
    mechanics.Commit(source);
    mechanics.Commit(destination);
    mechanics.Commit(item);

    ulong managedRevision = world.Revision;
    var sourceIdentity = new MechanicsSourceIdentity(
        MechanicsActiveEffectProvenanceKind.Request, 0, "", 0, "", 0, "", 0, 0, "", "example", "transfer");
    MechanicsUniqueItemTransferLeaseReceipt receipt = mechanics.TransferUniqueItem(
        item,
        source,
        destination,
        new MechanicsUniqueItemTransferOperation(
            "transfer-item",
            sourceIdentity,
            7,
            MechanicsRevisionGuard.Unchecked,
            default,
            MechanicsRevisionGuard.Unchecked,
            default),
        managedRevision);
    Require(receipt.ItemEntityId == item.Value && receipt.FromOwnerEntityId == source.Value && receipt.ToOwnerEntityId == destination.Value,
        "native unique-item receipt did not retain canonical identities");
    Require(service.UniqueTransfers == 1 && service.LastUniqueTransfer == (item.Value, source.Value, destination.Value),
        "managed adapter did not invoke the canonical native unique-item transfer");
    Require(world.TryGetContainedIn(item, out EntityId managedOwner) && managedOwner == destination,
        "successful native unique-item transfer did not mirror managed containment");
    Throws(
        () => mechanics.TransferUniqueItem(
            item,
            source,
            destination,
            new MechanicsUniqueItemTransferOperation("reject", sourceIdentity, 8, MechanicsRevisionGuard.Unchecked, default, MechanicsRevisionGuard.Unchecked, default),
            world.Revision),
        "managed source-containment preflight did not reject before native invocation");
    Require(service.UniqueTransfers == 1 && world.TryGetContainedIn(item, out managedOwner) && managedOwner == destination,
        "rejected managed preflight changed native or managed containment");
}

static void ExerciseMechanicsUniqueItemLifecycle()
{
    using var world = new EntityWorld();
    EntityId container = world.Create();
    EntityId item = world.Create();
    EntityId rejected = world.Create();
    var service = new MechanicsAdapterFake();
    using var mechanics = new MechanicsEntityWorld(world, service, service.Catalog);
    mechanics.Bind(container, "container");
    mechanics.Commit(container);

    ulong managedRevision = world.Revision;
    MechanicsUniqueItemMaterializationLeaseReceipt materialized = mechanics.MaterializeUniqueItem(
        item, "item", container, "unique-blade", 41, managedRevision);
    Require(materialized.ItemEntityId == item.Value && materialized.ContainerEntityId == container.Value
        && materialized.Lifecycle.Lifecycle == MechanicsEntityLifecycle.Active,
        "unique materialization did not retain canonical item/container/lifecycle facts");
    Require(world.TryGetContainedIn(item, out EntityId owner) && owner == container,
        "materialized item was not visible through canonical managed containment");
    Require(service.Materializations == 1 && service.ActiveLeases == 2,
        "materialization did not admit exactly one owned native binding");

    int releasedBeforeFailure = service.ReleasedLeases;
    Throws(
        () => mechanics.MaterializeUniqueItem(rejected, "rejected", container, "reject", 42, world.Revision),
        "rejected materialization did not propagate its owner failure");
    Require(!world.TryGetContainedIn(rejected, out _) && service.Materializations == 1
        && service.ReleasedLeases == releasedBeforeFailure + 1 && service.ActiveLeases == 2,
        "rejected materialization changed canonical containment or left an uncommitted native binding");

    var source = new MechanicsSourceIdentity(
        MechanicsActiveEffectProvenanceKind.Request, 0, "", 0, "", 0, "", 0, 0, "", "example", "destroy");
    MechanicsUniqueItemDestroyLeaseReceipt destroyed = mechanics.DestroyUniqueItem(
        item, new MechanicsUniqueItemDestroyOperation("destroy-item", source), 43, world.Revision);
    Require(destroyed.ItemEntityId == item.Value && destroyed.HasFormerOwner
        && destroyed.FormerOwnerEntityId == container.Value
        && destroyed.Lifecycle.Lifecycle == MechanicsEntityLifecycle.Tombstoned,
        "unique destruction did not retain exact former-owner/lifecycle facts");
    Require(world.GetLifecycle(item) == EntityLifecycle.Tombstoned && service.UniqueDestroys == 1
        && service.ActiveLeases == 1,
        "unique destruction did not tombstone the canonical entity and release its native binding");
    Throws(() => mechanics.Rebind(item, destroyed.Lifecycle.Stamp),
        "destroyed item retained a dangling Mechanics binding");
}

static void ExerciseMechanicsWorldRestore()
{
    var health = ComponentType<Health>.Create(ProductComponentKeys.Create(31));
    using var world = new EntityWorld([health]);
    EntityId actor = world.Create();
    world.Set(actor, health, new Health(10));
    ComponentRevision before = world.GetComponentRevision(actor, health);
    var service = new MechanicsAdapterFake();
    using var mechanics = new MechanicsEntityWorld(world, service, service.Catalog);
    mechanics.Bind(actor, "restore-actor");
    mechanics.Commit(actor);
    using MechanicsEntityWorldSnapshot snapshot = mechanics.Capture();
    mechanics.SetLifecycle(actor, EntityLifecycle.Disabled, world.GetEntityRevision(actor));
    EntityId currentOnly = world.Create();
    world.Set(actor, health, new Health(3));
    MechanicsWorldRestoreLeaseReceipt receipt = mechanics.Restore(snapshot, expectedManagedRevision: world.Revision);
    Require(world.Get(actor, health).Current == 10 && world.GetLifecycle(actor) == EntityLifecycle.Active,
        "paired Mechanics restore did not publish active snapshot state");
    Require(receipt.Revisions.Length == 7 && receipt.Revisions.ToArray().All(row => row.RestoredRevision > row.CurrentRevision),
        "paired Mechanics restore did not remap all seven Mechanics guard families");
    Require(service.RestorePublishes == 1, "paired Mechanics restore did not publish its prepared native candidate");
    Throws(() => world.Set(actor, health, new Health(9), before), "paired restore retained a stale managed component guard");
    Require(world.Create().Value != currentOnly.Value, "managed restore reused a current-only entity identity");
}

static void ExerciseMechanicsWorldImport()
{
    using var world = new EntityWorld();
    EntityId retired = world.Create();
    var service = new MechanicsAdapterFake();
    using var mechanics = new MechanicsEntityWorld(world, service, service.Catalog);
    mechanics.Bind(retired, "retired");
    mechanics.Commit(retired);

    EntityWorldRestorePlan plan = new(world.Revision, 4);
    plan.AddEntity(new EntityWorldEntityState(new EntityId(2), EntityLifecycle.Disabled, 1));
    plan.AddEntity(new EntityWorldEntityState(new EntityId(3), EntityLifecycle.Active, 1));
    plan.AddContainment(new EntityWorldContainmentState(new EntityId(3), new EntityId(2)));
    MechanicsWorldImportRequest request = ImportRequest(service.Catalog);

    MechanicsWorldImportRequest malformed = request with { Entities = request.Entities[..1] };
    Throws(() => mechanics.PrepareImport(plan, malformed, world.Revision),
        "malformed managed/native import correlation was accepted");
    Require(world.IsAlive(retired) && service.ImportPrepares == 0 && service.ActiveLeases == 1,
        "malformed import correlation changed either live world");

    service.ReturnMalformedImportRevisions = true;
    Throws(() => mechanics.PrepareImport(plan, request, world.Revision),
        "malformed native import revision receipt was accepted");
    service.ReturnMalformedImportRevisions = false;
    Require(world.IsAlive(retired) && service.ImportPrepares == 1 && service.ImportDisposals == 1
        && service.ActiveLeases == 1,
        "malformed native import receipt changed either live world");

    using (MechanicsEntityWorldImportCandidate cancelled = mechanics.PrepareImport(plan, request, world.Revision))
    {
        cancelled.Dispose();
        cancelled.Dispose();
    }
    Require(world.IsAlive(retired) && service.ImportPrepares == 2 && service.ImportPublishes == 0
        && service.ImportDisposals == 2 && service.ActiveLeases == 1,
        "cancelling a prepared import released more than its native preparation");

    using MechanicsEntityWorldImportCandidate candidate = mechanics.PrepareImport(plan, request, world.Revision);
    Require(service.ImportPrepares == 3 && service.ImportPublishes == 0 && service.ActiveLeases == 1,
        "preparing an import changed live bindings or published native state");
    candidate.Publish();
    candidate.Publish();
    candidate.Dispose();
    candidate.Dispose();
    Require(service.ImportPublishes == 1 && service.ImportClaims == 2 && service.ImportDisposals == 3,
        "import candidate was not idempotent or did not retire its native handle");
    Require(!world.IsAlive(retired) && world.GetLifecycle(new EntityId(2)) == EntityLifecycle.Disabled
        && world.TryGetContainedIn(new EntityId(3), out EntityId owner) && owner.Value == 2,
        "import did not replace managed membership, lifecycle, and containment together");
    Require(service.ObservedPresentEmptyStats && service.ObservedAbsentStats,
        "import request did not retain distinct present-empty and absent component facts");
    mechanics.SetLifecycle(new EntityId(3), EntityLifecycle.Disabled, world.GetEntityRevision(new EntityId(3)));
    Require(service.ActiveLeases == 2, "fresh native import bindings were not usable by the adapter");
    Require(mechanics.Export().Entities.Length == 2, "typed Mechanics export did not return copied native facts");
}

static void ExerciseMechanicsWorldPersistenceComposition()
{
    using var world = new EntityWorld();
    EntityId retired = world.Create();
    var mechanicsService = new MechanicsAdapterFake();
    var persistence = new InMemoryPersistenceService();
    using var mechanics = new MechanicsEntityWorld(world, mechanicsService, mechanicsService.Catalog);
    mechanics.Bind(retired, "retired");
    mechanics.Commit(retired);

    EntityWorldRestorePlan plan = new(world.Revision, 4);
    plan.AddEntity(new EntityWorldEntityState(new EntityId(2), EntityLifecycle.Disabled, 1));
    plan.AddEntity(new EntityWorldEntityState(new EntityId(3), EntityLifecycle.Active, 1));
    plan.AddContainment(new EntityWorldContainmentState(new EntityId(3), new EntityId(2)));
    MechanicsWorldImportRequest request = ImportRequest(mechanicsService.Catalog);
    var state = new MechanicsCheckpoint(plan, request);
    var codec = new InMemoryMechanicsCheckpointCodec();
    int captures = 0;
    int mappings = 0;
    bool emitMalformedPlan = false;

    using var store = new MechanicsEntityWorldProductStateStore<MechanicsCheckpoint>(
        mechanics,
        new PersistenceEngineContext(persistence),
        "mechanics-example",
        codec,
        export =>
        {
            captures++;
            Require(export.Entities.Length == 1 && export.Entities.Span[0].EntityId == retired.Value,
                "product capture did not receive the copied typed Mechanics export");
            return state;
        },
        checkpoint =>
        {
            mappings++;
            return emitMalformedPlan
                ? new MechanicsProductStateRestorePlan(
                    checkpoint.Plan,
                    checkpoint.Request with { Entities = checkpoint.Request.Entities[..1] })
                : new MechanicsProductStateRestorePlan(checkpoint.Plan, checkpoint.Request);
        });

    store.Save("checkpoint");
    Require(captures == 1, "product capture was not called exactly once during save");

    using (MechanicsEntityWorldProductStateLoad<MechanicsCheckpoint> absent = store.LoadPrepared("missing", world.Revision))
    {
        Require(!absent.Present && absent.PersistenceRevision == 0 && absent.PreparedImport is null,
            "an absent persistence load did not remain an honest no-op candidate");
        absent.Publish();
    }

    emitMalformedPlan = true;
    Throws(() => store.LoadPrepared("checkpoint", world.Revision),
        "a malformed product restore mapping was accepted");
    Require(world.IsAlive(retired) && mechanicsService.ImportPrepares == 0,
        "a rejected product restore mapping changed a live world");

    emitMalformedPlan = false;
    using (MechanicsEntityWorldProductStateLoad<MechanicsCheckpoint> cancelled = store.LoadPrepared("checkpoint", world.Revision))
    {
        Require(cancelled.Present && cancelled.PreparedImport is not null && mechanicsService.ImportPrepares == 1,
            "a present persistence load did not prepare its paired Engine candidate");
        cancelled.Dispose();
    }
    Require(world.IsAlive(retired) && mechanicsService.ImportPublishes == 0 && mechanicsService.ImportDisposals == 1,
        "cancelling a product persistence candidate changed either live world");

    using MechanicsEntityWorldProductStateLoad<MechanicsCheckpoint> loaded = store.LoadPrepared("checkpoint", world.Revision);
    Require(loaded.Present && loaded.PersistenceRevision == 1 && loaded.PreparedImport is not null
        && mappings == 3 && mechanicsService.ImportPrepares == 2 && mechanicsService.ImportPublishes == 0,
        "load preparation did not finish product mapping and Engine validation before publication");
    loaded.Publish();
    loaded.Publish();
    Require(mappings == 3 && mechanicsService.ImportPublishes == 1 && !world.IsAlive(retired)
        && world.GetLifecycle(new EntityId(2)) == EntityLifecycle.Disabled,
        "paired persistence publication invoked product callbacks or failed to replace live state");
}

static void ExerciseContinuousMechanicsSibling()
{
    const ulong SubnormalBits = 0x0000_0000_0000_0001;
    const ulong NegativeZeroBits = 0x8000_0000_0000_0000;
    using var world = new EntityWorld();
    EntityId actor = world.Create();
    var exactService = new MechanicsAdapterFake();
    var continuousService = new ContinuousMechanicsAdapterFake();
    using var mechanics = new MechanicsEntityWorld(world, exactService, exactService.Catalog);
    using ContinuousMechanicsCatalog catalog = continuousService.CreateCatalog(new ContinuousMechanicsCatalogCreateRequest(
        "example", ReadOnlyMemory<ContinuousMechanicsCatalogStatRow>.Empty,
        ReadOnlyMemory<ContinuousMechanicsCatalogTrackRow>.Empty,
        ReadOnlyMemory<ContinuousMechanicsCatalogSourceRow>.Empty,
        ReadOnlyMemory<ContinuousMechanicsCatalogContributionRow>.Empty,
        ReadOnlyMemory<ContinuousMechanicsCatalogEffectRow>.Empty,
        ReadOnlyMemory<ContinuousMechanicsCatalogEffectSourceRow>.Empty));
    var continuous = new ContinuousMechanicsEntityWorld(mechanics, continuousService, catalog);

    mechanics.Bind(actor, "continuous-actor");
    mechanics.Commit(actor);
    continuous.Initialize(actor, new ContinuousMechanicsInitialComponents(
        HasStats: true,
        Stats: new[] { new ContinuousMechanicsInitialStatRow("strength", SubnormalBits) },
        HasTracks: true,
        Tracks: new[] { new ContinuousMechanicsInitialTrackRow("health", SubnormalBits) },
        HasIntrinsicSources: true,
        IntrinsicSources: new[] { new ContinuousMechanicsInitialIntrinsicSourceRow("body", "body-source") },
        HasActiveEffects: true,
        ActiveEffects: new[] { new ContinuousMechanicsInitialActiveEffectRow("blessing", "blessing-effect") }));

    ContinuousMechanicsComponentLeaseReceipt components = continuous.Read(actor);
    Require(components.Components.Length == 4 && components.Components.Span.ToArray().All(row => row.Present)
        && components.Stats.Length == 1 && components.Tracks.Length == 1
        && components.IntrinsicSources.Length == 1 && components.ActiveEffects.Length == 1,
        "continuous sibling did not retain all four component families");
    ContinuousMechanicsStatEvaluationLeaseReceipt initial = continuous.EvaluateStat(actor, "strength");
    Require(initial.ValueBits == SubnormalBits && continuousService.LastEntityHandle == exactService.LastBoundHandle,
        "continuous evaluation did not use the exact existing native entity lease bit-for-bit");

    ContinuousMechanicsStatMutationLeaseReceipt mutation = continuous.SetStatBase(
        actor, "normalize-zero", "strength", NegativeZeroBits);
    Require(mutation.AfterBits == 0 && continuous.EvaluateStat(actor, "strength").ValueBits == 0,
        "continuous mutation did not preserve Engine binary64 normalization through the same lease");

    ContinuousMechanicsEntityWorldExport pairedExport = continuous.Export();
    Require(pairedExport.Exact.CatalogId == pairedExport.Continuous.MechanicsCatalogId
        && pairedExport.Exact.StateRevision == pairedExport.Continuous.MechanicsStateRevision
        && pairedExport.Continuous.ComponentPresence.Length == 4 && continuousService.WorldExports == 1,
        "paired continuous export did not retain copied exact catalog/state correlation");

    mechanics.SetLifecycle(actor, EntityLifecycle.Tombstoned, world.GetEntityRevision(actor));
    int evaluationsBeforeFence = continuousService.Evaluations;
    Throws(() => continuous.EvaluateStat(actor, "strength"),
        "continuous sibling bypassed the exact Mechanics lifecycle fence");
    Require(continuousService.Evaluations == evaluationsBeforeFence,
        "continuous service was called after the exact binding had been retired");

    EntityWorldRestorePlan plan = new(world.Revision, 4);
    plan.AddEntity(new EntityWorldEntityState(new EntityId(2), EntityLifecycle.Disabled, 1));
    plan.AddEntity(new EntityWorldEntityState(new EntityId(3), EntityLifecycle.Active, 1));
    plan.AddContainment(new EntityWorldContainmentState(new EntityId(3), new EntityId(2)));
    MechanicsWorldImportRequest request = ImportRequest(exactService.Catalog);
    ContinuousMechanicsWorldImportImage image = ContinuousImportImage();

    Throws(() => continuous.PrepareImport(plan, request, new ContinuousMechanicsWorldImportImage(
            image.MechanicsStateRevision + 1,
            image.ContinuousCatalogVersion,
            image.ContinuousCatalogFingerprint,
            image.ComponentPresence,
            image.Stats,
            image.Tracks,
            image.IntrinsicSources,
            image.ActiveEffects), world.Revision),
        "mismatched exact/continuous state revisions were accepted");
    Require(world.GetLifecycle(actor) == EntityLifecycle.Tombstoned && exactService.ImportPublishes == 0
        && exactService.ImportDisposals == 1 && continuousService.WorldImportStages == 0,
        "rejected continuous correlation changed a live world or reached staging");

    using (ContinuousMechanicsEntityWorldImportCandidate cancelled = continuous.PrepareImport(plan, request, image, world.Revision))
    {
        Require(cancelled.ExactReceipt.Entities.Length == 2 && cancelled.ContinuousReceipt.Revisions.Length == 8,
            "paired continuous preparation did not produce complete copied remap evidence");
        cancelled.Dispose();
    }
    Require(exactService.ImportPublishes == 0 && exactService.ImportDisposals == 2 && continuousService.WorldImportStages == 1,
        "cancelling a paired continuous import did not retire exactly its exact candidate");

    using ContinuousMechanicsEntityWorldImportCandidate prepared = continuous.PrepareImport(plan, request, image, world.Revision);
    prepared.Publish();
    prepared.Publish();
    Require(exactService.ImportPublishes == 1 && exactService.ImportClaims == 2
        && prepared.ContinuousReceipt.Revisions.Span.ToArray().All(row => row.RestoredRevision > row.CurrentRevision),
        "paired continuous import did not publish once with fresh continuous remaps and exact bindings");

    var persistence = new InMemoryPersistenceService();
    var checkpoint = new ContinuousCheckpoint(plan, request, image);
    var codec = new InMemoryContinuousCheckpointCodec();
    var migration = new InMemoryContinuousCheckpointMigration();
    int captures = 0;
    int mappings = 0;
    using var store = new ContinuousMechanicsEntityWorldProductStateStore<ContinuousCheckpoint>(
        continuous,
        new PersistenceEngineContext(persistence),
        "continuous-example",
        codec,
        export =>
        {
            captures++;
            Require(export.Exact.CatalogId == export.Continuous.MechanicsCatalogId
                && export.Exact.StateRevision == export.Continuous.MechanicsStateRevision,
                "product capture did not receive the correlated paired copied export");
            return checkpoint;
        },
        state =>
        {
            mappings++;
            return new ContinuousMechanicsEntityWorldProductStateRestorePlan(state.Plan, state.Request, state.Image);
        },
        new[] { migration });
    store.Save("checkpoint");
    persistence.Seed("continuous-example", "checkpoint", 1, new byte[] { 0 });
    using (ContinuousMechanicsEntityWorldProductStateLoad<ContinuousCheckpoint> cancelled = store.LoadPrepared("checkpoint", world.Revision))
    {
        Require(cancelled.Present && cancelled.PreparedExactImport is not null && cancelled.PreparedContinuousImport is not null,
            "continuous product load did not complete mapping and paired Engine preparation before publication");
        cancelled.Dispose();
    }
    using ContinuousMechanicsEntityWorldProductStateLoad<ContinuousCheckpoint> loaded = store.LoadPrepared("checkpoint", world.Revision);
    loaded.Publish();
    loaded.Publish();
    Require(captures == 1 && mappings == 2 && migration.Calls == 2 && continuousService.WorldImportStages == 4,
        "continuous product persistence did not retain explicit, callback-free paired publication");
}

static MechanicsWorldImportRequest ImportRequest(MechanicsCatalog catalog)
    => new(
        catalog,
        40,
        "example",
        "example",
        new MechanicsWorldEntityRow[]
        {
            new MechanicsWorldEntityRow(2, "pack", MechanicsEntityLifecycle.Disabled, 101),
            new MechanicsWorldEntityRow(3, "item", MechanicsEntityLifecycle.Active, 102),
        },
        new MechanicsWorldContainmentRow[] { new(3, 2) },
        ImportPresence(2, statsPresent: true)
            .Concat(ImportPresence(3, statsPresent: false))
            .ToArray(),
        ReadOnlyMemory<MechanicsWorldStatRow>.Empty,
        ReadOnlyMemory<MechanicsWorldTrackRow>.Empty,
        ReadOnlyMemory<MechanicsWorldIntrinsicSourceRow>.Empty,
        ReadOnlyMemory<MechanicsWorldActiveEffectRow>.Empty,
        ReadOnlyMemory<MechanicsWorldInventoryStackRow>.Empty,
        ReadOnlyMemory<MechanicsWorldInventoryCapacityLimitRow>.Empty,
        ReadOnlyMemory<MechanicsWorldItemRow>.Empty,
        ReadOnlyMemory<MechanicsWorldEquipmentAssignmentRow>.Empty);

static IEnumerable<MechanicsWorldComponentPresenceRow> ImportPresence(ulong entity, bool statsPresent)
    => Enum.GetValues<MechanicsRevisionComponent>()
        .Select(component => new MechanicsWorldComponentPresenceRow(
            entity,
            component,
            component == MechanicsRevisionComponent.Stats && statsPresent,
            (ulong)component + 1));

static ContinuousMechanicsWorldImportImage ContinuousImportImage()
{
    ReadOnlyMemory<ContinuousMechanicsWorldComponentPresenceRow> presence = new[]
    {
        new ContinuousMechanicsWorldComponentPresenceRow(2, ContinuousMechanicsComponentKind.Stats, true, 1),
        new ContinuousMechanicsWorldComponentPresenceRow(2, ContinuousMechanicsComponentKind.Tracks, true, 2),
        new ContinuousMechanicsWorldComponentPresenceRow(2, ContinuousMechanicsComponentKind.IntrinsicSources, true, 3),
        new ContinuousMechanicsWorldComponentPresenceRow(2, ContinuousMechanicsComponentKind.ActiveEffects, true, 4),
        new ContinuousMechanicsWorldComponentPresenceRow(3, ContinuousMechanicsComponentKind.Stats, false, 5),
        new ContinuousMechanicsWorldComponentPresenceRow(3, ContinuousMechanicsComponentKind.Tracks, false, 6),
        new ContinuousMechanicsWorldComponentPresenceRow(3, ContinuousMechanicsComponentKind.IntrinsicSources, false, 7),
        new ContinuousMechanicsWorldComponentPresenceRow(3, ContinuousMechanicsComponentKind.ActiveEffects, false, 8),
    };
    return new ContinuousMechanicsWorldImportImage(
        40,
        "example",
        "example",
        presence,
        new[] { new ContinuousMechanicsWorldStatRow(2, "strength", 0x0000_0000_0000_0001) },
        new[] { new ContinuousMechanicsWorldTrackRow(2, "health", 0) },
        new[] { new ContinuousMechanicsWorldIntrinsicSourceRow(2, "body", "body-source") },
        new[] { new ContinuousMechanicsWorldActiveEffectRow(2, "blessing", "blessing-effect") });
}

static void ExerciseContinuousMechanicsComposition()
{
    using var world = new EntityWorld([]);
    EntityId first = world.Create();
    EntityId second = world.Create();
    var exactService = new MechanicsAdapterFake();
    var continuousService = new ContinuousMechanicsAdapterFake();
    using var mechanics = new MechanicsEntityWorld(world, exactService, exactService.Catalog);
    using ContinuousMechanicsCatalog firstCatalog = continuousService.CreateCatalog(new ContinuousMechanicsCatalogCreateRequest(
        "first", ReadOnlyMemory<ContinuousMechanicsCatalogStatRow>.Empty,
        ReadOnlyMemory<ContinuousMechanicsCatalogTrackRow>.Empty,
        ReadOnlyMemory<ContinuousMechanicsCatalogSourceRow>.Empty,
        ReadOnlyMemory<ContinuousMechanicsCatalogContributionRow>.Empty,
        ReadOnlyMemory<ContinuousMechanicsCatalogEffectRow>.Empty,
        ReadOnlyMemory<ContinuousMechanicsCatalogEffectSourceRow>.Empty));
    using var secondCatalog = new ContinuousMechanicsCatalog(new ContinuousMechanicsCatalogHandle(2), static () => { });
    var firstContinuous = new ContinuousMechanicsEntityWorld(mechanics, continuousService, firstCatalog);
    var secondContinuous = new ContinuousMechanicsEntityWorld(mechanics, continuousService, secondCatalog);
    var composition = new ContinuousMechanicsEntityWorldComposition(mechanics, [firstContinuous, secondContinuous]);
    var duplicateCatalogAdapter = new ContinuousMechanicsEntityWorld(mechanics, continuousService, firstCatalog);
    Throws(() => new ContinuousMechanicsEntityWorldComposition(mechanics, [firstContinuous, duplicateCatalogAdapter]),
        "composition accepted a duplicate continuous catalog");
    var differentServiceAdapter = new ContinuousMechanicsEntityWorld(mechanics, new ContinuousMechanicsAdapterFake(), secondCatalog);
    Throws(() => new ContinuousMechanicsEntityWorldComposition(mechanics, [firstContinuous, differentServiceAdapter]),
        "composition accepted adapters from different continuous services");
    using var foreignEntities = new EntityWorld([]);
    using var foreignMechanics = new MechanicsEntityWorld(foreignEntities, new MechanicsAdapterFake(), exactService.Catalog);
    var foreignAdapter = new ContinuousMechanicsEntityWorld(foreignMechanics, continuousService, secondCatalog);
    Throws(() => new ContinuousMechanicsEntityWorldComposition(mechanics, [firstContinuous, foreignAdapter]),
        "composition accepted an adapter borrowing a different exact world");

    mechanics.Bind(first, "first");
    mechanics.Commit(first);
    mechanics.Bind(second, "second");
    mechanics.Commit(second);
    firstContinuous.Initialize(first, EmptyContinuousComponents());
    secondContinuous.Initialize(second, EmptyContinuousComponents());

    ContinuousMechanicsEntityWorldCompositionExport exported = composition.Export();
    Require(exported.Continuous.Count == 2 && exported.Continuous.All(receipt => receipt.ComponentPresence.Length == 4)
        && exported.Continuous[0].ComponentPresence.Span.ToArray().All(row => row.EntityId == first.Value)
        && exported.Continuous[1].ComponentPresence.Span.ToArray().All(row => row.EntityId == second.Value)
        && exported.Continuous.All(receipt => receipt.MechanicsCatalogId == exported.Exact.CatalogId
            && receipt.MechanicsStateRevision == exported.Exact.StateRevision)
        && continuousService.WorldExports == 2,
        "composition did not export one exact receipt with ordered scoped continuous receipts");

    EntityWorldRestorePlan plan = new(world.Revision, 3);
    plan.AddEntity(new EntityWorldEntityState(first, EntityLifecycle.Active, 1));
    plan.AddEntity(new EntityWorldEntityState(second, EntityLifecycle.Active, 1));
    MechanicsWorldImportRequest request = MultiImportRequest(exactService.Catalog, first.Value, second.Value);
    ContinuousMechanicsWorldImportImage firstImage = ContinuousSubsetImage(first.Value, statsPresent: true);
    ContinuousMechanicsWorldImportImage secondImage = ContinuousSubsetImage(second.Value, statsPresent: false);
    var firstPart = new ContinuousMechanicsEntityWorldImportPart(firstContinuous, firstImage);
    var secondPart = new ContinuousMechanicsEntityWorldImportPart(secondContinuous, secondImage);

    int preparesBeforeRejects = exactService.ImportPrepares;
    Throws(() => composition.PrepareImport(plan, request, [firstPart, firstPart], world.Revision),
        "duplicate continuous catalog was accepted");
    Throws(() => composition.PrepareImport(plan, request,
            [firstPart, new ContinuousMechanicsEntityWorldImportPart(secondContinuous, firstImage)], world.Revision),
        "overlapping continuous entity subsets were accepted");
    Throws(() => composition.PrepareImport(plan, request,
            [new ContinuousMechanicsEntityWorldImportPart(firstContinuous, WithContinuousRevision(firstImage, 41)), secondPart], world.Revision),
        "mismatched exact and continuous image revision was accepted");
    Require(exactService.ImportPrepares == preparesBeforeRejects && exactService.ImportPublishes == 0,
        "composition rejection prepared or published an exact candidate");

    using (ContinuousMechanicsEntityWorldCompositionImportCandidate cancelled = composition.PrepareImport(
        plan, request, [firstPart, secondPart], world.Revision))
    {
        Require(cancelled.ExactReceipt.Entities.Length == 2 && cancelled.ContinuousReceipts.Count == 2
            && cancelled.ContinuousReceipts.All(receipt => receipt.Revisions.Length == 4),
            "composition preparation did not preserve two scoped continuous receipts");
    }
    Require(exactService.ImportPublishes == 0 && exactService.ImportDisposals == 1,
        "composition cancellation did not retire exactly the one exact candidate");

    using (ContinuousMechanicsEntityWorldCompositionImportCandidate prepared = composition.PrepareImport(
        plan, request, [firstPart, secondPart], world.Revision))
    {
        prepared.Publish();
        prepared.Publish();
        Require(exactService.ImportPublishes == 1 && exactService.ImportClaims == 2
            && prepared.ContinuousReceipts.All(receipt => receipt.Revisions.Span.ToArray()
                .All(row => row.RestoredRevision > row.CurrentRevision)),
            "composition publication was not idempotent with four fresh remaps per catalog");
    }

    var persistence = new InMemoryPersistenceService();
    var checkpoint = new MultiContinuousCheckpoint(plan, request, [firstImage, secondImage]);
    var codec = new InMemoryMultiContinuousCheckpointCodec();
    var migration = new InMemoryContinuousCheckpointMigration();
    int captures = 0;
    int mappings = 0;
    using var store = new ContinuousMechanicsEntityWorldCompositionProductStateStore<MultiContinuousCheckpoint>(
        composition,
        new PersistenceEngineContext(persistence),
        "multi-continuous-example",
        codec,
        export =>
        {
            captures++;
            Require(export.Continuous.Count == 2 && export.Continuous.All(receipt => receipt.MechanicsStateRevision == export.Exact.StateRevision),
                "product capture did not receive the ordered correlated composition export");
            return checkpoint;
        },
        state =>
        {
            mappings++;
            if (state.Images.Count != 2)
            {
                throw new InvalidOperationException("product checkpoint did not contain its ordered continuous images");
            }
            return new ContinuousMechanicsEntityWorldCompositionProductStateRestorePlan(state.Plan, state.Request,
            [
                new ContinuousMechanicsEntityWorldImportPart(firstContinuous, state.Images[0]),
                new ContinuousMechanicsEntityWorldImportPart(secondContinuous, state.Images[1]),
            ]);
        },
        new[] { migration });
    store.Save("checkpoint");
    persistence.Seed("multi-continuous-example", "checkpoint", 1, new byte[] { 0 });
    using (ContinuousMechanicsEntityWorldCompositionProductStateLoad<MultiContinuousCheckpoint> cancelled = store.LoadPrepared("checkpoint", world.Revision))
    {
        Require(cancelled.Present && cancelled.PreparedExactImport is not null
            && cancelled.PreparedContinuousImports?.Count == 2,
            "composition product load did not map ordered images before publication");
    }
    using (ContinuousMechanicsEntityWorldCompositionProductStateLoad<MultiContinuousCheckpoint> loaded = store.LoadPrepared("checkpoint", world.Revision))
    {
        loaded.Publish();
        loaded.Publish();
    }
    Require(captures == 1 && mappings == 2 && migration.Calls == 2,
        "composition product persistence did not retain product-owned capture and restore mapping");
}

static ContinuousMechanicsInitialComponents EmptyContinuousComponents()
    => new(false, ReadOnlyMemory<ContinuousMechanicsInitialStatRow>.Empty,
        false, ReadOnlyMemory<ContinuousMechanicsInitialTrackRow>.Empty,
        false, ReadOnlyMemory<ContinuousMechanicsInitialIntrinsicSourceRow>.Empty,
        false, ReadOnlyMemory<ContinuousMechanicsInitialActiveEffectRow>.Empty);

static MechanicsWorldImportRequest MultiImportRequest(MechanicsCatalog catalog, ulong first, ulong second)
    => new(catalog, 40, "example", "example",
        new[]
        {
            new MechanicsWorldEntityRow(first, "first", MechanicsEntityLifecycle.Active, 101),
            new MechanicsWorldEntityRow(second, "second", MechanicsEntityLifecycle.Active, 102),
        },
        ReadOnlyMemory<MechanicsWorldContainmentRow>.Empty,
        Enumerable.Range(0, 2).SelectMany(index => ImportPresence(index == 0 ? first : second, index == 0)).ToArray(),
        ReadOnlyMemory<MechanicsWorldStatRow>.Empty,
        ReadOnlyMemory<MechanicsWorldTrackRow>.Empty,
        ReadOnlyMemory<MechanicsWorldIntrinsicSourceRow>.Empty,
        ReadOnlyMemory<MechanicsWorldActiveEffectRow>.Empty,
        ReadOnlyMemory<MechanicsWorldInventoryStackRow>.Empty,
        ReadOnlyMemory<MechanicsWorldInventoryCapacityLimitRow>.Empty,
        ReadOnlyMemory<MechanicsWorldItemRow>.Empty,
        ReadOnlyMemory<MechanicsWorldEquipmentAssignmentRow>.Empty);

static ContinuousMechanicsWorldImportImage ContinuousSubsetImage(ulong entity, bool statsPresent)
    => new(40, "example", "example",
        new[]
        {
            new ContinuousMechanicsWorldComponentPresenceRow(entity, ContinuousMechanicsComponentKind.Stats, statsPresent, 1),
            new ContinuousMechanicsWorldComponentPresenceRow(entity, ContinuousMechanicsComponentKind.Tracks, false, 2),
            new ContinuousMechanicsWorldComponentPresenceRow(entity, ContinuousMechanicsComponentKind.IntrinsicSources, false, 3),
            new ContinuousMechanicsWorldComponentPresenceRow(entity, ContinuousMechanicsComponentKind.ActiveEffects, false, 4),
        },
        statsPresent ? new[] { new ContinuousMechanicsWorldStatRow(entity, "strength", 1) } : ReadOnlyMemory<ContinuousMechanicsWorldStatRow>.Empty,
        ReadOnlyMemory<ContinuousMechanicsWorldTrackRow>.Empty,
        ReadOnlyMemory<ContinuousMechanicsWorldIntrinsicSourceRow>.Empty,
        ReadOnlyMemory<ContinuousMechanicsWorldActiveEffectRow>.Empty);

static ContinuousMechanicsWorldImportImage WithContinuousRevision(ContinuousMechanicsWorldImportImage image, ulong revision)
    => new(revision, image.ContinuousCatalogVersion, image.ContinuousCatalogFingerprint,
        image.ComponentPresence, image.Stats, image.Tracks, image.IntrinsicSources, image.ActiveEffects);

readonly record struct Health(int Current);

readonly record struct Armor(int Current);

readonly record struct MechanicsCheckpoint(
    EntityWorldRestorePlan Plan,
    MechanicsWorldImportRequest Request);

readonly record struct ContinuousCheckpoint(
    EntityWorldRestorePlan Plan,
    MechanicsWorldImportRequest Request,
    ContinuousMechanicsWorldImportImage Image);

readonly record struct MultiContinuousCheckpoint(
    EntityWorldRestorePlan Plan,
    MechanicsWorldImportRequest Request,
    IReadOnlyList<ContinuousMechanicsWorldImportImage> Images);

// This in-memory codec exists only to exercise the persistence composition. Products supply
// their own durable archive bytes, schema, and migrations; the Engine does not select them.
sealed class InMemoryMechanicsCheckpointCodec : IProductStateCodec<MechanicsCheckpoint>
{
    private MechanicsCheckpoint? _saved;

    public uint SchemaVersion => 1;

    public void Encode(in MechanicsCheckpoint state, System.Buffers.IBufferWriter<byte> destination)
    {
        _saved = state;
        destination.GetSpan(1)[0] = 1;
        destination.Advance(1);
    }

    public MechanicsCheckpoint Decode(ReadOnlySpan<byte> payload)
        => payload.Length == 1 && payload[0] == 1 && _saved is MechanicsCheckpoint saved
            ? saved
            : throw new InvalidOperationException("example checkpoint bytes were not available");
}

sealed class InMemoryContinuousCheckpointCodec : IProductStateCodec<ContinuousCheckpoint>
{
    private ContinuousCheckpoint? _saved;

    public uint SchemaVersion => 2;

    public void Encode(in ContinuousCheckpoint state, System.Buffers.IBufferWriter<byte> destination)
    {
        _saved = state;
        destination.GetSpan(1)[0] = 1;
        destination.Advance(1);
    }

    public ContinuousCheckpoint Decode(ReadOnlySpan<byte> payload)
        => payload.Length == 1 && payload[0] == 1 && _saved is ContinuousCheckpoint saved
            ? saved
            : throw new InvalidOperationException("example continuous checkpoint bytes were not available");
}

sealed class InMemoryMultiContinuousCheckpointCodec : IProductStateCodec<MultiContinuousCheckpoint>
{
    private MultiContinuousCheckpoint? _saved;
    public uint SchemaVersion => 2;

    public void Encode(in MultiContinuousCheckpoint state, System.Buffers.IBufferWriter<byte> destination)
    {
        _saved = state;
        destination.GetSpan(1)[0] = 1;
        destination.Advance(1);
    }

    public MultiContinuousCheckpoint Decode(ReadOnlySpan<byte> payload)
        => payload.Length == 1 && payload[0] == 1 && _saved is MultiContinuousCheckpoint saved
            ? saved
            : throw new InvalidOperationException("example multi-continuous checkpoint bytes were not available");
}

sealed class InMemoryContinuousCheckpointMigration : IProductStateMigration
{
    public uint FromSchemaVersion => 1;
    public uint ToSchemaVersion => 2;
    public int Calls { get; private set; }

    public byte[] Migrate(ReadOnlySpan<byte> payload)
    {
        if (!payload.SequenceEqual(new byte[] { 0 }))
        {
            throw new InvalidOperationException("continuous product migration received unexpected bytes");
        }
        Calls++;
        return new byte[] { 1 };
    }
}

sealed class PersistenceEngineContext(IPersistenceService persistence) : IEngineContext
{
    public ILookService Look => throw new NotSupportedException();
    public IDynamicsService Dynamics => throw new NotSupportedException();
    public ISpatialService Spatial => throw new NotSupportedException();
    public IVoxelService Voxel => throw new NotSupportedException();
    public IVoxelContentService VoxelContent => throw new NotSupportedException();
    public IContentService Content => throw new NotSupportedException();
    public IAppearanceService Appearance => throw new NotSupportedException();
    public IAnimationService Animation => throw new NotSupportedException();
    public IAudioService Audio => throw new NotSupportedException();
    public ICameraViewService CameraView => throw new NotSupportedException();
    public IRandomService Random => throw new NotSupportedException();
    public IMechanicsService Mechanics => throw new NotSupportedException();
    public IContinuousMechanicsService ContinuousMechanics => throw new NotSupportedException();
    public IPersistenceService Persistence { get; } = persistence;
    public IContentStoreService ContentStore => throw new NotSupportedException();
    public IRulesService Rules => throw new NotSupportedException();
    public IStandardExactService StandardExact => throw new NotSupportedException();
    public IStandardContinuousService StandardContinuous => throw new NotSupportedException();
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

sealed class ContinuousMechanicsAdapterFake : IContinuousMechanicsService
{
    private readonly Dictionary<(ulong Entity, string Stat), ulong> _stats = [];
    private readonly Dictionary<ulong, (ulong Entity, ContinuousMechanicsInitialComponentsRequest Initial)> _initials = [];

    public ContinuousMechanicsCatalog Catalog { get; } = new(new ContinuousMechanicsCatalogHandle(1), static () => { });
    public ulong LastEntityHandle { get; private set; }
    public int Evaluations { get; private set; }
    public int WorldExports { get; private set; }
    public int WorldImportStages { get; private set; }

    public ContinuousMechanicsCatalog CreateCatalog(ContinuousMechanicsCatalogCreateRequest arg0) => Catalog;

    public ContinuousMechanicsCatalogLeaseReceipt ReadCatalog(ContinuousMechanicsCatalog arg0)
        => new(
            ReadOnlyMemory<ContinuousMechanicsCatalogStatRow>.Empty,
            ReadOnlyMemory<ContinuousMechanicsCatalogTrackRow>.Empty,
            ReadOnlyMemory<ContinuousMechanicsCatalogSourceRow>.Empty,
            ReadOnlyMemory<ContinuousMechanicsCatalogContributionRow>.Empty,
            ReadOnlyMemory<ContinuousMechanicsCatalogEffectRow>.Empty,
            ReadOnlyMemory<ContinuousMechanicsCatalogEffectSourceRow>.Empty,
            arg0.Handle.Value,
            "example",
            "example");

    public void SetInitialComponents(ContinuousMechanicsInitialComponentsRequest arg0)
    {
        LastEntityHandle = arg0.Entity.Handle.Value;
        _initials[arg0.Catalog.Handle.Value] = (LastEntityHandle, arg0);
        foreach (ContinuousMechanicsInitialStatRow stat in arg0.Stats.Span)
        {
            _stats[(LastEntityHandle, stat.Stat)] = Normalize(stat.BaseBits);
        }
    }

    public ContinuousMechanicsComponentLeaseReceipt ReadComponents(ContinuousMechanicsComponentReadRequest arg0)
    {
        LastEntityHandle = arg0.Entity.Handle.Value;
        if (!_initials.TryGetValue(arg0.Catalog.Handle.Value, out var scoped))
        {
            throw new InvalidOperationException("continuous components were not initialized");
        }
        ContinuousMechanicsInitialComponentsRequest initial = scoped.Initial;
        return new ContinuousMechanicsComponentLeaseReceipt(
            new[]
            {
                new ContinuousMechanicsComponentPresenceRow(ContinuousMechanicsComponentKind.Stats, initial.HasStats, 1),
                new ContinuousMechanicsComponentPresenceRow(ContinuousMechanicsComponentKind.Tracks, initial.HasTracks, 1),
                new ContinuousMechanicsComponentPresenceRow(ContinuousMechanicsComponentKind.IntrinsicSources, initial.HasIntrinsicSources, 1),
                new ContinuousMechanicsComponentPresenceRow(ContinuousMechanicsComponentKind.ActiveEffects, initial.HasActiveEffects, 1),
            },
            initial.Stats,
            initial.Tracks,
            initial.IntrinsicSources,
            initial.ActiveEffects,
            arg0.Catalog.Handle.Value,
            "example",
            "example",
            LastEntityHandle);
    }

    public ContinuousMechanicsWorldExportLeaseReceipt ExportWorld(ContinuousMechanicsWorldExportRequest arg0)
    {
        WorldExports++;
        if (!_initials.TryGetValue(arg0.ContinuousCatalog.Handle.Value, out var scoped))
        {
            throw new InvalidOperationException("continuous components were not initialized");
        }
        ContinuousMechanicsInitialComponentsRequest initial = scoped.Initial;
        ulong entity = scoped.Entity;
        return new ContinuousMechanicsWorldExportLeaseReceipt(
            new[]
            {
                new ContinuousMechanicsWorldComponentPresenceRow(entity, ContinuousMechanicsComponentKind.Stats, initial.HasStats, 1),
                new ContinuousMechanicsWorldComponentPresenceRow(entity, ContinuousMechanicsComponentKind.Tracks, initial.HasTracks, 1),
                new ContinuousMechanicsWorldComponentPresenceRow(entity, ContinuousMechanicsComponentKind.IntrinsicSources, initial.HasIntrinsicSources, 1),
                new ContinuousMechanicsWorldComponentPresenceRow(entity, ContinuousMechanicsComponentKind.ActiveEffects, initial.HasActiveEffects, 1),
            },
            initial.Stats.Span.ToArray().Select(row => new ContinuousMechanicsWorldStatRow(entity, row.Stat, row.BaseBits)).ToArray(),
            initial.Tracks.Span.ToArray().Select(row => new ContinuousMechanicsWorldTrackRow(entity, row.Track, row.CurrentBits)).ToArray(),
            initial.IntrinsicSources.Span.ToArray().Select(row => new ContinuousMechanicsWorldIntrinsicSourceRow(entity, row.Instance, row.Definition)).ToArray(),
            initial.ActiveEffects.Span.ToArray().Select(row => new ContinuousMechanicsWorldActiveEffectRow(entity, row.Instance, row.Definition)).ToArray(),
            arg0.MechanicsCatalog.Handle.Value,
            50,
            arg0.ContinuousCatalog.Handle.Value,
            "example",
            "example");
    }

    public ContinuousMechanicsWorldImportLeaseReceipt StageWorldImport(ContinuousMechanicsWorldImportStageRequest arg0)
    {
        WorldImportStages++;
        ContinuousMechanicsRevisionRemapRow[] remaps = arg0.ComponentPresence.Span.ToArray()
            .Select(row => new ContinuousMechanicsRevisionRemapRow(
                row.EntityId, row.Component, row.Present, row.Revision, row.Revision + 1, row.Revision + 2))
            .ToArray();
        return new ContinuousMechanicsWorldImportLeaseReceipt(
            remaps,
            arg0.MechanicsCatalog.Handle.Value,
            arg0.MechanicsStateRevision,
            arg0.MechanicsStateRevision + 1,
            arg0.ContinuousCatalog.Handle.Value,
            arg0.ContinuousCatalogVersion,
            arg0.ContinuousCatalogFingerprint);
    }

    public ContinuousMechanicsStatEvaluationLeaseReceipt EvaluateStat(ContinuousMechanicsStatEvaluateRequest arg0)
    {
        LastEntityHandle = arg0.Entity.Handle.Value;
        Evaluations++;
        ulong value = _stats[(LastEntityHandle, arg0.Stat)];
        return new ContinuousMechanicsStatEvaluationLeaseReceipt(
            ReadOnlyMemory<ContinuousMechanicsStatDecisionRow>.Empty,
            arg0.Catalog.Handle.Value,
            "example",
            "example",
            LastEntityHandle,
            arg0.Stat,
            value,
            value,
            value,
            0,
            ulong.MaxValue,
            value,
            default);
    }

    public ContinuousMechanicsStatMutationLeaseReceipt SetStatBase(ContinuousMechanicsStatBaseMutationRequest arg0)
    {
        LastEntityHandle = arg0.Entity.Handle.Value;
        ulong before = _stats[(LastEntityHandle, arg0.Stat)];
        ulong after = Normalize(arg0.BaseBits);
        _stats[(LastEntityHandle, arg0.Stat)] = after;
        return new ContinuousMechanicsStatMutationLeaseReceipt(
            arg0.Catalog.Handle.Value,
            "example",
            "example",
            arg0.Operation,
            LastEntityHandle,
            arg0.Stat,
            before,
            after,
            0,
            ulong.MaxValue,
            1,
            2);
    }

    public ContinuousMechanicsTrackLeaseReceipt ReadTrack(ContinuousMechanicsTrackReadRequest arg0) => throw new NotSupportedException();
    public ContinuousMechanicsTrackLeaseReceipt SetTrack(ContinuousMechanicsTrackSetRequest arg0) => throw new NotSupportedException();
    public ContinuousMechanicsTrackLeaseReceipt SpendTrack(ContinuousMechanicsTrackAdjustmentRequest arg0) => throw new NotSupportedException();
    public ContinuousMechanicsTrackLeaseReceipt RestoreTrack(ContinuousMechanicsTrackAdjustmentRequest arg0) => throw new NotSupportedException();
    public ContinuousMechanicsEffectLeaseReceipt ApplyEffect(ContinuousMechanicsEffectApplyRequest arg0) => throw new NotSupportedException();
    public ContinuousMechanicsEffectLeaseReceipt RemoveEffect(ContinuousMechanicsEffectRemoveRequest arg0) => throw new NotSupportedException();

    private static ulong Normalize(ulong bits) => bits == 0x8000_0000_0000_0000 ? 0 : bits;
}

sealed class MechanicsAdapterFake : IMechanicsService
{
    public const ulong InitialLifecycleStamp = 11;
    private ulong _nextHandle = 1;
    private readonly Dictionary<ulong, ulong> _entityIds = [];
    private readonly HashSet<ulong> _materialized = [];
    private MechanicsWorldImportRequest? _preparedImport;
    private bool _importPublished;
    private readonly HashSet<ulong> _claimedImportEntities = [];
    public int ReleasedLeases { get; private set; }
    public int Rebinds { get; private set; }
    public int UniqueTransfers { get; private set; }
    public int Materializations { get; private set; }
    public int UniqueDestroys { get; private set; }
    public int RestorePublishes { get; private set; }
    public int ImportPrepares { get; private set; }
    public int ImportPublishes { get; private set; }
    public int ImportClaims { get; private set; }
    public int ImportDisposals { get; private set; }
    public ulong LastBoundHandle { get; private set; }
    public bool ObservedPresentEmptyStats { get; private set; }
    public bool ObservedAbsentStats { get; private set; }
    public bool ReturnMalformedImportRevisions { get; set; }
    public int ActiveLeases => _entityIds.Count;
    public (ulong Item, ulong FromOwner, ulong ToOwner) LastUniqueTransfer { get; private set; }
    public MechanicsCatalog Catalog { get; } = new(new MechanicsCatalogHandle(1), static () => { });

    public MechanicsCatalog CreateCatalog(MechanicsCatalogCreateRequest arg0) => Catalog;
    public void DefineStat(MechanicsStatDefinitionRequest arg0) => throw new NotSupportedException();
    public void DefineTrack(MechanicsTrackDefinitionRequest arg0) => throw new NotSupportedException();
    public void DefineContribution(MechanicsContributionDefinitionRequest arg0) => throw new NotSupportedException();
    public void DefineSource(MechanicsSourceDefinitionRequest arg0) => throw new NotSupportedException();
    public void DefineDamageKind(MechanicsDamageKindDefinitionRequest arg0) => throw new NotSupportedException();
    public void DefineDamageResponse(MechanicsDamageResponseDefinitionRequest arg0) => throw new NotSupportedException();
    public void DefineEffect(MechanicsEffectDefinitionRequest arg0) => throw new NotSupportedException();
    public void DefineCapacityMetric(MechanicsCapacityMetricDefinitionRequest arg0) => throw new NotSupportedException();
    public void DefineItem(MechanicsItemDefinitionRequest arg0) => throw new NotSupportedException();
    public void DefineEquipmentSlot(MechanicsEquipmentSlotDefinitionRequest arg0) => throw new NotSupportedException();
    public void AdmitCatalog(MechanicsCatalog arg0) => throw new NotSupportedException();
    public MechanicsCatalogIdentityLeaseReceipt ReadCatalogIdentity(MechanicsCatalog arg0) => throw new NotSupportedException();
    public MechanicsStatCatalogLeaseReceipt ReadCatalogStats(MechanicsCatalog arg0) => throw new NotSupportedException();
    public MechanicsTrackCatalogLeaseReceipt ReadCatalogTracks(MechanicsCatalog arg0) => throw new NotSupportedException();
    public MechanicsSourceCatalogLeaseReceipt ReadCatalogSources(MechanicsCatalog arg0) => throw new NotSupportedException();
    public MechanicsStatContributionCatalogLeaseReceipt ReadCatalogStatContributions(MechanicsCatalog arg0) => throw new NotSupportedException();
    public MechanicsDamageKindCatalogLeaseReceipt ReadCatalogDamageKinds(MechanicsCatalog arg0) => throw new NotSupportedException();
    public MechanicsDamageResponseCatalogLeaseReceipt ReadCatalogDamageResponses(MechanicsCatalog arg0) => throw new NotSupportedException();
    public MechanicsEffectCatalogLeaseReceipt ReadCatalogEffects(MechanicsCatalog arg0) => throw new NotSupportedException();
    public MechanicsEffectSourceCatalogLeaseReceipt ReadCatalogEffectSources(MechanicsCatalog arg0) => throw new NotSupportedException();
    public MechanicsCapacityMetricCatalogLeaseReceipt ReadCatalogCapacityMetrics(MechanicsCatalog arg0) => throw new NotSupportedException();
    public MechanicsItemCatalogLeaseReceipt ReadCatalogItems(MechanicsCatalog arg0) => throw new NotSupportedException();
    public MechanicsItemClassificationCatalogLeaseReceipt ReadCatalogItemClassifications(MechanicsCatalog arg0) => throw new NotSupportedException();
    public MechanicsItemCapacityCostCatalogLeaseReceipt ReadCatalogItemCapacityCosts(MechanicsCatalog arg0) => throw new NotSupportedException();
    public MechanicsItemEquipmentPolicyCatalogLeaseReceipt ReadCatalogItemEquipmentPolicies(MechanicsCatalog arg0) => throw new NotSupportedException();
    public MechanicsItemSourceCatalogLeaseReceipt ReadCatalogItemSources(MechanicsCatalog arg0) => throw new NotSupportedException();
    public MechanicsEquipmentSlotCatalogLeaseReceipt ReadCatalogEquipmentSlots(MechanicsCatalog arg0) => throw new NotSupportedException();
    public MechanicsSlotClassificationCatalogLeaseReceipt ReadCatalogSlotClassifications(MechanicsCatalog arg0) => throw new NotSupportedException();
    public MechanicsStatComponentLeaseReceipt ReadStatComponent(MechanicsEntity arg0) => throw new NotSupportedException();
    public MechanicsTrackComponentLeaseReceipt ReadTrackComponent(MechanicsEntity arg0) => throw new NotSupportedException();
    public MechanicsIntrinsicSourceComponentLeaseReceipt ReadIntrinsicSourceComponent(MechanicsEntity arg0) => throw new NotSupportedException();
    public MechanicsActiveEffectComponentLeaseReceipt ReadActiveEffectComponent(MechanicsEntity arg0) => throw new NotSupportedException();
    public MechanicsInventoryStackComponentLeaseReceipt ReadInventoryStackComponent(MechanicsEntity arg0) => throw new NotSupportedException();
    public MechanicsInventoryCapacityLimitComponentLeaseReceipt ReadInventoryCapacityLimitComponent(MechanicsEntity arg0) => throw new NotSupportedException();
    public MechanicsItemComponentLeaseReceipt ReadItemComponent(MechanicsEntity arg0) => throw new NotSupportedException();
    public MechanicsEquipmentAssignmentComponentLeaseReceipt ReadEquipmentAssignmentComponent(MechanicsEntity arg0) => throw new NotSupportedException();
    public MechanicsWorldSnapshot CaptureWorldSnapshot(MechanicsCatalog arg0)
        => new(new MechanicsWorldSnapshotHandle(1), static () => { });
    public MechanicsWorldSnapshotLeaseReceipt ReadWorldSnapshot(MechanicsWorldSnapshot arg0)
        => new(2);
    public MechanicsWorldRestore PrepareWorldRestore(MechanicsWorldRestoreRequest arg0)
        => new(new MechanicsWorldRestoreHandle(1), static () => { });
    public MechanicsWorldRestoreLeaseReceipt ReadWorldRestore(MechanicsWorldRestore arg0)
    {
        MechanicsRevisionRemapRow[] revisions = _entityIds.Values.Distinct()
            .SelectMany(entity => Enum.GetValues<MechanicsRevisionComponent>()
                .Select(component => new MechanicsRevisionRemapRow(entity, component, component is MechanicsRevisionComponent.Stats or MechanicsRevisionComponent.Tracks,
                    1, 2, 3)))
            .ToArray();
        MechanicsLifecycleReceipt[] lifecycles = _entityIds.Values.Distinct()
            .Select(entity => new MechanicsLifecycleReceipt(entity, MechanicsEntityLifecycle.Active, 20))
            .ToArray();
        return new MechanicsWorldRestoreLeaseReceipt(revisions, lifecycles, 2, 3);
    }
    public void PublishWorldRestore(MechanicsWorldRestore arg0) => RestorePublishes++;
    public MechanicsWorldExportLeaseReceipt ExportWorld(MechanicsCatalog arg0)
    {
        MechanicsWorldEntityRow[] entities = _entityIds.Values.Distinct()
            .Order()
            .Select(id => new MechanicsWorldEntityRow(id, $"entity-{id}", MechanicsEntityLifecycle.Active, 1))
            .ToArray();
        return new MechanicsWorldExportLeaseReceipt(
            entities,
            ReadOnlyMemory<MechanicsWorldContainmentRow>.Empty,
            ReadOnlyMemory<MechanicsWorldComponentPresenceRow>.Empty,
            ReadOnlyMemory<MechanicsWorldStatRow>.Empty,
            ReadOnlyMemory<MechanicsWorldTrackRow>.Empty,
            ReadOnlyMemory<MechanicsWorldIntrinsicSourceRow>.Empty,
            ReadOnlyMemory<MechanicsWorldActiveEffectRow>.Empty,
            ReadOnlyMemory<MechanicsWorldInventoryStackRow>.Empty,
            ReadOnlyMemory<MechanicsWorldInventoryCapacityLimitRow>.Empty,
            ReadOnlyMemory<MechanicsWorldItemRow>.Empty,
            ReadOnlyMemory<MechanicsWorldEquipmentAssignmentRow>.Empty,
            arg0.Handle.Value,
            50,
            "example",
            "example");
    }
    public MechanicsWorldImport PrepareWorldImport(MechanicsWorldImportRequest arg0)
    {
        if (arg0.Catalog.Handle != Catalog.Handle)
        {
            throw new InvalidOperationException("import must use the admitted catalog");
        }
        _preparedImport = arg0;
        _importPublished = false;
        _claimedImportEntities.Clear();
        ImportPrepares++;
        ObservedPresentEmptyStats = arg0.ComponentPresence.Span.ToArray().Any(row => row.EntityId == 2
            && row.Component == MechanicsRevisionComponent.Stats && row.Present && arg0.Stats.IsEmpty);
        ObservedAbsentStats = arg0.ComponentPresence.Span.ToArray().Any(row => row.EntityId == 3
            && row.Component == MechanicsRevisionComponent.Stats && !row.Present);
        return new MechanicsWorldImport(new MechanicsWorldImportHandle(77), () =>
        {
            ImportDisposals++;
            _preparedImport = null;
        });
    }
    public MechanicsWorldImportLeaseReceipt ReadWorldImport(MechanicsWorldImport arg0)
    {
        MechanicsWorldImportRequest request = _preparedImport
            ?? throw new InvalidOperationException("import was not prepared");
        MechanicsWorldImportEntityRow[] entities = request.Entities.Span.ToArray()
            .Select(row => new MechanicsWorldImportEntityRow(row.EntityId, row.Identity, row.Lifecycle, row.LifecycleStamp))
            .ToArray();
        MechanicsLifecycleReceipt[] lifecycles = entities
            .Select(row => new MechanicsLifecycleReceipt(row.EntityId, row.Lifecycle, row.LifecycleStamp))
            .ToArray();
        MechanicsRevisionRemapRow[] revisions = request.ComponentPresence.Span.ToArray()
            .Select(row => new MechanicsRevisionRemapRow(
                row.EntityId,
                row.Component,
                row.Present,
                row.Revision,
                row.Revision + 1,
                row.Revision + 2))
            .ToArray();
        return new MechanicsWorldImportLeaseReceipt(
            entities,
            ReturnMalformedImportRevisions ? revisions[..^1] : revisions,
            lifecycles,
            Catalog.Handle.Value,
            request.StateRevision,
            request.StateRevision + 1);
    }
    public void PublishWorldImport(MechanicsWorldImport arg0)
    {
        if (_preparedImport is null || _importPublished)
        {
            throw new InvalidOperationException("import must publish exactly once after preparation");
        }
        _importPublished = true;
        ImportPublishes++;
    }
    public MechanicsEntity ClaimWorldImportEntity(MechanicsWorldImportEntityClaimRequest arg0)
    {
        if (!_importPublished || _preparedImport is not MechanicsWorldImportRequest request
            || !request.Entities.Span.ToArray().Any(row => row.EntityId == arg0.EntityId)
            || !_claimedImportEntities.Add(arg0.EntityId))
        {
            throw new InvalidOperationException("import entity claims must be fresh and exact");
        }
        ImportClaims++;
        return Lease(arg0.EntityId);
    }
    public MechanicsEntity BindEntity(MechanicsEntityBindRequest arg0) => Lease(arg0.EntityId);
    public MechanicsEntity RebindEntity(MechanicsEntityRebindRequest arg0)
    {
        if (arg0.Guard != MechanicsLifecycleGuard.Exact || arg0.ExpectedStamp != InitialLifecycleStamp)
        {
            throw new InvalidOperationException("rebind must retain the exact lifecycle stamp");
        }
        Rebinds++;
        return Lease(arg0.EntityId);
    }
    public void SetInitialStat(MechanicsInitialStatRequest arg0) { }
    public void SetInitialTrack(MechanicsInitialTrackRequest arg0) => throw new NotSupportedException();
    public void BindIntrinsicSource(MechanicsIntrinsicSourceRequest arg0) => throw new NotSupportedException();
    public void SetInitialComponents(MechanicsInitialComponentsRequest arg0) => throw new NotSupportedException();
    public void StageInitialContainment(MechanicsInitialContainmentRequest arg0) => throw new NotSupportedException();
    public MechanicsContainmentReceipt ReadContainment(MechanicsContainmentReadRequest arg0) => throw new NotSupportedException();
    public MechanicsEntityReceipt CommitEntity(MechanicsEntity arg0)
    {
        MechanicsComponentRevision Slot(MechanicsRevisionComponent component, bool present)
            => new(1, 1, component, present);
        return new MechanicsEntityReceipt(
            0,
            1,
            new MechanicsStatsRevision(1, 1, MechanicsRevisionComponent.Stats),
            new MechanicsTracksRevision(1, 1, MechanicsRevisionComponent.Tracks),
            new MechanicsLifecycleReceipt(1, MechanicsEntityLifecycle.Active, InitialLifecycleStamp),
            Slot(MechanicsRevisionComponent.Stats, true),
            Slot(MechanicsRevisionComponent.Tracks, true),
            Slot(MechanicsRevisionComponent.IntrinsicSources, false),
            Slot(MechanicsRevisionComponent.ActiveEffects, false),
            Slot(MechanicsRevisionComponent.Inventory, false),
            Slot(MechanicsRevisionComponent.Item, false),
            Slot(MechanicsRevisionComponent.Equipment, false));
    }
    public MechanicsLifecycleReceipt SetEntityLifecycle(MechanicsLifecycleRequest arg0)
    {
        if (arg0.Guard != MechanicsLifecycleGuard.Exact
            || (arg0.ExpectedStamp != InitialLifecycleStamp && arg0.ExpectedStamp != 102))
        {
            throw new InvalidOperationException("lifecycle transition must retain the exact native stamp");
        }
        return new MechanicsLifecycleReceipt(EntityId(arg0.Entity), arg0.Lifecycle, arg0.ExpectedStamp + 1);
    }
    public MechanicsStatReadReceipt ReadStat(MechanicsStatReadRequest arg0) => throw new NotSupportedException();
    public MechanicsStatEvaluationLeaseReceipt EvaluateStat(MechanicsStatOperationRequest arg0) => throw new NotSupportedException();
    public MechanicsTrackReadLeaseReceipt ReadTrack(MechanicsTrackReadRequest arg0) => throw new NotSupportedException();
    public MechanicsInventoryViewLeaseReceipt ReadInventoryView(MechanicsEntity arg0) => throw new NotSupportedException();
    public MechanicsInventoryMutationLeaseReceipt GrantInventory(MechanicsInventoryMutationRequest arg0) => throw new NotSupportedException();
    public MechanicsInventoryMutationLeaseReceipt ConsumeInventory(MechanicsInventoryMutationRequest arg0) => throw new NotSupportedException();
    public MechanicsInventoryTransferLeaseReceipt TransferInventory(MechanicsInventoryTransferRequest arg0) => throw new NotSupportedException();
    public MechanicsUniqueItemTransferLeaseReceipt TransferUniqueItem(MechanicsUniqueItemTransferRequest arg0)
    {
        ulong item = EntityId(arg0.Item);
        ulong fromOwner = EntityId(arg0.FromOwner);
        ulong toOwner = EntityId(arg0.ToOwner);
        UniqueTransfers++;
        LastUniqueTransfer = (item, fromOwner, toOwner);
        return new MechanicsUniqueItemTransferLeaseReceipt(
            ReadOnlyMemory<MechanicsInventoryViewCapacityUsageRow>.Empty,
            ReadOnlyMemory<MechanicsInventoryViewCapacityUsageRow>.Empty,
            ReadOnlyMemory<MechanicsInventoryViewCapacityUsageRow>.Empty,
            ReadOnlyMemory<MechanicsInventoryViewCapacityUsageRow>.Empty,
            Catalog.Handle.Value,
            "example",
            "example",
            arg0.Operation,
            new MechanicsSourceIdentity(arg0.SourceKind, arg0.SourceIntrinsicEntityId, arg0.SourceIntrinsicInstance, arg0.SourceEffectEntityId, arg0.SourceEffectInstance, arg0.SourceEffectStack, arg0.SourceEffectSource, arg0.SourceEquippedOwnerEntityId, arg0.SourceEquippedItemEntityId, arg0.SourceEquippedSource, arg0.SourceRequestOperation, arg0.SourceRequestInstance),
            item,
            fromOwner,
            toOwner,
            arg0.ExpectedRelationshipRevision,
            arg0.ExpectedRelationshipRevision + 1,
            default,
            default,
            default);
    }
    public MechanicsUniqueItemMaterializationLeaseReceipt MaterializeUniqueItem(MechanicsUniqueItemMaterializationRequest arg0)
    {
        if (arg0.Definition == "reject")
        {
            throw new InvalidOperationException("owner rejected item definition");
        }
        ulong item = EntityId(arg0.Item);
        ulong container = EntityId(arg0.Container);
        _materialized.Add(item);
        Materializations++;
        return new MechanicsUniqueItemMaterializationLeaseReceipt(
            Catalog.Handle.Value, "example", "example", item, arg0.Definition, container,
            arg0.ExpectedStateRevision, arg0.ExpectedStateRevision + 1, arg0.ExpectedStateRevision + 2,
            arg0.ExpectedStateRevision + 3, 0, 1, false, 0, true, container,
            new MechanicsLifecycleReceipt(item, MechanicsEntityLifecycle.Active, 17));
    }
    public MechanicsUniqueItemDestroyLeaseReceipt DestroyUniqueItem(MechanicsUniqueItemDestroyRequest arg0)
    {
        ulong item = EntityId(arg0.Item);
        if (!_materialized.Remove(item))
        {
            throw new InvalidOperationException("owner does not have that materialized item");
        }
        UniqueDestroys++;
        return new MechanicsUniqueItemDestroyLeaseReceipt(
            Catalog.Handle.Value, "example", "example", arg0.Operation,
            new MechanicsSourceIdentity(arg0.SourceKind, arg0.SourceIntrinsicEntityId, arg0.SourceIntrinsicInstance,
                arg0.SourceEffectEntityId, arg0.SourceEffectInstance, arg0.SourceEffectStack, arg0.SourceEffectSource,
                arg0.SourceEquippedOwnerEntityId, arg0.SourceEquippedItemEntityId, arg0.SourceEquippedSource,
                arg0.SourceRequestOperation, arg0.SourceRequestInstance),
            item, true, 1, arg0.ExpectedStateRevision, arg0.ExpectedStateRevision + 1,
            new MechanicsLifecycleReceipt(item, MechanicsEntityLifecycle.Tombstoned, 18));
    }
    public MechanicsEquipmentMutationLeaseReceipt EquipEquipment(MechanicsEquipmentEquipRequest arg0) => throw new NotSupportedException();
    public MechanicsEquipmentMutationLeaseReceipt UnequipEquipment(MechanicsEquipmentUnequipRequest arg0) => throw new NotSupportedException();
    public MechanicsEquipmentMutationLeaseReceipt SwapEquipment(MechanicsEquipmentSwapRequest arg0) => throw new NotSupportedException();
    public MechanicsStatMutationLeaseReceipt SetStatBase(MechanicsStatBaseMutationRequest arg0) => throw new NotSupportedException();
    public MechanicsTrackSetLeaseReceipt SetTrack(MechanicsTrackSetRequest arg0) => throw new NotSupportedException();
    public MechanicsTrackMutationLeaseReceipt SpendTrack(MechanicsTrackMutationRequest arg0) => throw new NotSupportedException();
    public MechanicsTrackMutationLeaseReceipt RestoreTrack(MechanicsTrackMutationRequest arg0) => throw new NotSupportedException();
    public MechanicsTrackReconciliationLeaseReceipt ReconcileTrack(MechanicsTrackReconciliationRequest arg0) => throw new NotSupportedException();
    public MechanicsEffectOperationLeaseReceipt ApplyEffect(MechanicsEffectMutationRequest arg0) => throw new NotSupportedException();
    public MechanicsEffectOperationLeaseReceipt RefreshEffect(MechanicsEffectRefreshRequest arg0) => throw new NotSupportedException();
    public MechanicsEffectOperationLeaseReceipt ReplaceEffect(MechanicsEffectMutationRequest arg0) => throw new NotSupportedException();
    public MechanicsEffectOperationLeaseReceipt RemoveEffect(MechanicsEffectRemovalRequest arg0) => throw new NotSupportedException();
    public MechanicsEffectOperationLeaseReceipt ExpireEffect(MechanicsEffectRemovalRequest arg0) => throw new NotSupportedException();
    public MechanicsDamageLeaseReceipt PreviewDamage(MechanicsDamageRequest arg0) => throw new NotSupportedException();
    public MechanicsDamageLeaseReceipt ApplyDamage(MechanicsDamageRequest arg0) => throw new NotSupportedException();

    private MechanicsEntity Lease(ulong entityId = 0)
    {
        MechanicsEntityHandle handle = new(_nextHandle++);
        LastBoundHandle = handle.Value;
        _entityIds.Add(handle.Value, entityId);
        return new MechanicsEntity(handle, () =>
        {
            _entityIds.Remove(handle.Value);
            ReleasedLeases++;
        });
    }

    private ulong EntityId(MechanicsEntity entity)
        => _entityIds.TryGetValue(entity.Handle.Value, out ulong entityId)
            ? entityId
            : throw new InvalidOperationException("transfer must use a bound Mechanics entity");
}
