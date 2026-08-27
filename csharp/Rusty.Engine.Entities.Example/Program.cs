using Rusty.Engine;
using Rusty.Engine.Entities;

const uint HealthLocalComponentId = 1;
const uint ArmorLocalComponentId = 2;
var health = ComponentType<Health>.Create(ProductComponentKeys.Create(HealthLocalComponentId), validator: ValidateHealth);
var armor = ComponentType<Armor>.Create(ProductComponentKeys.Create(ArmorLocalComponentId));
using var world = new EntityWorld([EngineComponentTypes.Transform, EngineComponentTypes.CharacterMotion, health, armor]);

EntityId actor = world.Create();
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

readonly record struct Health(int Current);

readonly record struct Armor(int Current);

sealed class MechanicsAdapterFake : IMechanicsService
{
    public const ulong InitialLifecycleStamp = 11;
    private ulong _nextHandle = 1;
    public int ReleasedLeases { get; private set; }
    public int Rebinds { get; private set; }
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
    public MechanicsEntity BindEntity(MechanicsEntityBindRequest arg0) => Lease();
    public MechanicsEntity RebindEntity(MechanicsEntityRebindRequest arg0)
    {
        if (arg0.Guard != MechanicsLifecycleGuard.Exact || arg0.ExpectedStamp != InitialLifecycleStamp)
        {
            throw new InvalidOperationException("rebind must retain the exact lifecycle stamp");
        }
        Rebinds++;
        return Lease();
    }
    public void SetInitialStat(MechanicsInitialStatRequest arg0) { }
    public void SetInitialTrack(MechanicsInitialTrackRequest arg0) => throw new NotSupportedException();
    public void BindIntrinsicSource(MechanicsIntrinsicSourceRequest arg0) => throw new NotSupportedException();
    public void SetInitialComponents(MechanicsInitialComponentsRequest arg0) => throw new NotSupportedException();
    public MechanicsEntityReceipt CommitEntity(MechanicsEntity arg0)
    {
        MechanicsComponentRevision Slot(MechanicsRevisionComponent component, bool present)
            => new(1, 1, component, present);
        return new MechanicsEntityReceipt(
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
    public MechanicsStatEvaluationReceipt EvaluateStat(MechanicsStatOperationRequest arg0) => throw new NotSupportedException();
    public MechanicsTrackReadReceipt ReadTrack(MechanicsTrackReadRequest arg0) => throw new NotSupportedException();
    public MechanicsStatMutationReceipt SetStatBase(MechanicsStatBaseMutationRequest arg0) => throw new NotSupportedException();
    public MechanicsTrackSetReceipt SetTrack(MechanicsTrackSetRequest arg0) => throw new NotSupportedException();
    public MechanicsTrackMutationReceipt SpendTrack(MechanicsTrackMutationRequest arg0) => throw new NotSupportedException();
    public MechanicsTrackMutationReceipt RestoreTrack(MechanicsTrackMutationRequest arg0) => throw new NotSupportedException();
    public MechanicsTrackReconciliationReceipt ReconcileTrack(MechanicsTrackReconciliationRequest arg0) => throw new NotSupportedException();

    private MechanicsEntity Lease()
    {
        MechanicsEntityHandle handle = new(_nextHandle++);
        return new MechanicsEntity(handle, () => ReleasedLeases++);
    }
}
