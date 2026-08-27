using Rusty.Engine;
using Rusty.Engine.Entities;

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

ExerciseMechanicsLeaseRebind();
ExerciseMechanicsUniqueItemTransfer();
ExerciseMechanicsUniqueItemLifecycle();

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

readonly record struct Health(int Current);

readonly record struct Armor(int Current);

sealed class MechanicsAdapterFake : IMechanicsService
{
    public const ulong InitialLifecycleStamp = 11;
    private ulong _nextHandle = 1;
    private readonly Dictionary<ulong, ulong> _entityIds = [];
    private readonly HashSet<ulong> _materialized = [];
    public int ReleasedLeases { get; private set; }
    public int Rebinds { get; private set; }
    public int UniqueTransfers { get; private set; }
    public int Materializations { get; private set; }
    public int UniqueDestroys { get; private set; }
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
        if (arg0.Guard != MechanicsLifecycleGuard.Exact || arg0.ExpectedStamp != InitialLifecycleStamp)
        {
            throw new InvalidOperationException("lifecycle transition must retain the exact native stamp");
        }
        return new MechanicsLifecycleReceipt(1, arg0.Lifecycle, InitialLifecycleStamp + 1);
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
