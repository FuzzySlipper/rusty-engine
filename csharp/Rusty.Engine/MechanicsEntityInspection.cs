namespace Rusty.Engine;

/// <summary>
/// A copied, bounded inspection of the seven durable Mechanics component families for one
/// committed canonical entity. Each family stays typed; inventory intentionally exposes its
/// stacks and capacity limits as distinct collections.
/// </summary>
public sealed class MechanicsEntityInspection
{
    private MechanicsEntityInspection(
        MechanicsComponentInspection<MechanicsStatComponentRow> stats,
        MechanicsComponentInspection<MechanicsTrackComponentRow> tracks,
        MechanicsComponentInspection<MechanicsIntrinsicSourceComponentRow> intrinsicSources,
        MechanicsComponentInspection<MechanicsActiveEffectComponentRow> activeEffects,
        MechanicsInventoryInspection inventory,
        MechanicsComponentInspection<MechanicsItemComponentRow> item,
        MechanicsComponentInspection<MechanicsEquipmentAssignmentComponentRow> equipment)
    {
        Metadata = stats.Metadata;
        Stats = stats;
        Tracks = tracks;
        IntrinsicSources = intrinsicSources;
        ActiveEffects = activeEffects;
        Inventory = inventory;
        Item = item;
        Equipment = equipment;
    }

    public MechanicsComponentReadMetadata Metadata { get; }
    public MechanicsComponentInspection<MechanicsStatComponentRow> Stats { get; }
    public MechanicsComponentInspection<MechanicsTrackComponentRow> Tracks { get; }
    public MechanicsComponentInspection<MechanicsIntrinsicSourceComponentRow> IntrinsicSources { get; }
    public MechanicsComponentInspection<MechanicsActiveEffectComponentRow> ActiveEffects { get; }
    public MechanicsInventoryInspection Inventory { get; }
    public MechanicsComponentInspection<MechanicsItemComponentRow> Item { get; }
    public MechanicsComponentInspection<MechanicsEquipmentAssignmentComponentRow> Equipment { get; }

    /// <summary>Copies and immediately releases every generated component lease.</summary>
    public static MechanicsEntityInspection Read(IMechanicsService mechanics, MechanicsEntity entity)
    {
        ArgumentNullException.ThrowIfNull(mechanics);
        ArgumentNullException.ThrowIfNull(entity);

        MechanicsStatComponentLeaseReceipt statsLease = mechanics.ReadStatComponent(entity);
        MechanicsTrackComponentLeaseReceipt tracksLease = mechanics.ReadTrackComponent(entity);
        MechanicsIntrinsicSourceComponentLeaseReceipt intrinsicLease = mechanics.ReadIntrinsicSourceComponent(entity);
        MechanicsActiveEffectComponentLeaseReceipt effectsLease = mechanics.ReadActiveEffectComponent(entity);
        MechanicsInventoryStackComponentLeaseReceipt stacksLease = mechanics.ReadInventoryStackComponent(entity);
        MechanicsInventoryCapacityLimitComponentLeaseReceipt limitsLease = mechanics.ReadInventoryCapacityLimitComponent(entity);
        MechanicsItemComponentLeaseReceipt itemLease = mechanics.ReadItemComponent(entity);
        MechanicsEquipmentAssignmentComponentLeaseReceipt equipmentLease = mechanics.ReadEquipmentAssignmentComponent(entity);

        var stats = new MechanicsComponentInspection<MechanicsStatComponentRow>(statsLease.Metadata, statsLease.Entries);
        var tracks = new MechanicsComponentInspection<MechanicsTrackComponentRow>(tracksLease.Metadata, tracksLease.Entries);
        var intrinsicSources = new MechanicsComponentInspection<MechanicsIntrinsicSourceComponentRow>(intrinsicLease.Metadata, intrinsicLease.Entries);
        var activeEffects = new MechanicsComponentInspection<MechanicsActiveEffectComponentRow>(effectsLease.Metadata, effectsLease.Entries);
        var inventory = new MechanicsInventoryInspection(stacksLease.Metadata, stacksLease.Entries, limitsLease.Entries);
        var item = new MechanicsComponentInspection<MechanicsItemComponentRow>(itemLease.Metadata, itemLease.Entries);
        var equipment = new MechanicsComponentInspection<MechanicsEquipmentAssignmentComponentRow>(equipmentLease.Metadata, equipmentLease.Entries);

        Require(stats.Metadata, MechanicsRevisionComponent.Stats, stats.Metadata);
        Require(tracks.Metadata, MechanicsRevisionComponent.Tracks, stats.Metadata);
        Require(intrinsicSources.Metadata, MechanicsRevisionComponent.IntrinsicSources, stats.Metadata);
        Require(activeEffects.Metadata, MechanicsRevisionComponent.ActiveEffects, stats.Metadata);
        Require(inventory.Metadata, MechanicsRevisionComponent.Inventory, stats.Metadata);
        Require(limitsLease.Metadata, MechanicsRevisionComponent.Inventory, inventory.Metadata);
        if (limitsLease.Metadata != inventory.Metadata)
        {
            throw new InvalidOperationException("Mechanics inventory inspection mixed component revisions or presence.");
        }
        Require(item.Metadata, MechanicsRevisionComponent.Item, stats.Metadata);
        Require(equipment.Metadata, MechanicsRevisionComponent.Equipment, stats.Metadata);
        if ((item.Metadata.Present && item.Entries.Length != 1) || (!item.Metadata.Present && item.Entries.Length != 0))
        {
            throw new InvalidOperationException("Mechanics item inspection did not preserve component presence.");
        }

        return new MechanicsEntityInspection(stats, tracks, intrinsicSources, activeEffects, inventory, item, equipment);
    }

    private static void Require(
        MechanicsComponentReadMetadata actual,
        MechanicsRevisionComponent expectedComponent,
        MechanicsComponentReadMetadata expectedIdentity)
    {
        if (actual.Component != expectedComponent || actual.EntityId != expectedIdentity.EntityId ||
            actual.CatalogId != expectedIdentity.CatalogId || actual.CatalogVersion != expectedIdentity.CatalogVersion ||
            actual.CatalogFingerprint != expectedIdentity.CatalogFingerprint)
        {
            throw new InvalidOperationException("Mechanics component inspection mixed entity or catalog identities.");
        }
    }
}

public readonly record struct MechanicsComponentInspection<T>(
    MechanicsComponentReadMetadata Metadata,
    ReadOnlyMemory<T> Entries);

public readonly record struct MechanicsInventoryInspection(
    MechanicsComponentReadMetadata Metadata,
    ReadOnlyMemory<MechanicsInventoryStackComponentRow> Stacks,
    ReadOnlyMemory<MechanicsInventoryCapacityLimitComponentRow> CapacityLimits);
