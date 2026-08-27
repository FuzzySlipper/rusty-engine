namespace Rusty.Engine;

/// <summary>
/// One complete copied view of an admitted Mechanics catalog. Each collection remains a distinct
/// typed family so products can join only the relationships relevant to their own logic.
/// </summary>
public sealed class MechanicsCatalogInspection
{
    private MechanicsCatalogInspection(
        MechanicsCatalogIdentityRow identity,
        ReadOnlyMemory<MechanicsStatCatalogRow> stats,
        ReadOnlyMemory<MechanicsTrackCatalogRow> tracks,
        ReadOnlyMemory<MechanicsSourceCatalogRow> sources,
        ReadOnlyMemory<MechanicsStatContributionCatalogRow> statContributions,
        ReadOnlyMemory<MechanicsDamageKindCatalogRow> damageKinds,
        ReadOnlyMemory<MechanicsDamageResponseCatalogRow> damageResponses,
        ReadOnlyMemory<MechanicsEffectCatalogRow> effects,
        ReadOnlyMemory<MechanicsEffectSourceCatalogRow> effectSources,
        ReadOnlyMemory<MechanicsCapacityMetricCatalogRow> capacityMetrics,
        ReadOnlyMemory<MechanicsItemCatalogRow> items,
        ReadOnlyMemory<MechanicsItemClassificationCatalogRow> itemClassifications,
        ReadOnlyMemory<MechanicsItemCapacityCostCatalogRow> itemCapacityCosts,
        ReadOnlyMemory<MechanicsItemEquipmentPolicyCatalogRow> itemEquipmentPolicies,
        ReadOnlyMemory<MechanicsItemSourceCatalogRow> itemSources,
        ReadOnlyMemory<MechanicsEquipmentSlotCatalogRow> equipmentSlots,
        ReadOnlyMemory<MechanicsSlotClassificationCatalogRow> slotClassifications)
    {
        Identity = identity;
        Stats = stats;
        Tracks = tracks;
        Sources = sources;
        StatContributions = statContributions;
        DamageKinds = damageKinds;
        DamageResponses = damageResponses;
        Effects = effects;
        EffectSources = effectSources;
        CapacityMetrics = capacityMetrics;
        Items = items;
        ItemClassifications = itemClassifications;
        ItemCapacityCosts = itemCapacityCosts;
        ItemEquipmentPolicies = itemEquipmentPolicies;
        ItemSources = itemSources;
        EquipmentSlots = equipmentSlots;
        SlotClassifications = slotClassifications;
    }

    public MechanicsCatalogIdentityRow Identity { get; }
    public ReadOnlyMemory<MechanicsStatCatalogRow> Stats { get; }
    public ReadOnlyMemory<MechanicsTrackCatalogRow> Tracks { get; }
    public ReadOnlyMemory<MechanicsSourceCatalogRow> Sources { get; }
    public ReadOnlyMemory<MechanicsStatContributionCatalogRow> StatContributions { get; }
    public ReadOnlyMemory<MechanicsDamageKindCatalogRow> DamageKinds { get; }
    public ReadOnlyMemory<MechanicsDamageResponseCatalogRow> DamageResponses { get; }
    public ReadOnlyMemory<MechanicsEffectCatalogRow> Effects { get; }
    public ReadOnlyMemory<MechanicsEffectSourceCatalogRow> EffectSources { get; }
    public ReadOnlyMemory<MechanicsCapacityMetricCatalogRow> CapacityMetrics { get; }
    public ReadOnlyMemory<MechanicsItemCatalogRow> Items { get; }
    public ReadOnlyMemory<MechanicsItemClassificationCatalogRow> ItemClassifications { get; }
    public ReadOnlyMemory<MechanicsItemCapacityCostCatalogRow> ItemCapacityCosts { get; }
    public ReadOnlyMemory<MechanicsItemEquipmentPolicyCatalogRow> ItemEquipmentPolicies { get; }
    public ReadOnlyMemory<MechanicsItemSourceCatalogRow> ItemSources { get; }
    public ReadOnlyMemory<MechanicsEquipmentSlotCatalogRow> EquipmentSlots { get; }
    public ReadOnlyMemory<MechanicsSlotClassificationCatalogRow> SlotClassifications { get; }

    /// <summary>Copies every exact typed catalog family through the generated lease APIs.</summary>
    public static MechanicsCatalogInspection Read(IMechanicsService mechanics, MechanicsCatalog catalog)
    {
        ArgumentNullException.ThrowIfNull(mechanics);
        ArgumentNullException.ThrowIfNull(catalog);

        MechanicsCatalogIdentityLeaseReceipt identity = mechanics.ReadCatalogIdentity(catalog);
        if (identity.CatalogId != catalog.Handle.Value || identity.Entries.Length != 1)
        {
            throw new InvalidOperationException("Mechanics catalog identity inspection was not exact.");
        }

        MechanicsStatCatalogLeaseReceipt stats = mechanics.ReadCatalogStats(catalog);
        MechanicsTrackCatalogLeaseReceipt tracks = mechanics.ReadCatalogTracks(catalog);
        MechanicsSourceCatalogLeaseReceipt sources = mechanics.ReadCatalogSources(catalog);
        MechanicsStatContributionCatalogLeaseReceipt statContributions = mechanics.ReadCatalogStatContributions(catalog);
        MechanicsDamageKindCatalogLeaseReceipt damageKinds = mechanics.ReadCatalogDamageKinds(catalog);
        MechanicsDamageResponseCatalogLeaseReceipt damageResponses = mechanics.ReadCatalogDamageResponses(catalog);
        MechanicsEffectCatalogLeaseReceipt effects = mechanics.ReadCatalogEffects(catalog);
        MechanicsEffectSourceCatalogLeaseReceipt effectSources = mechanics.ReadCatalogEffectSources(catalog);
        MechanicsCapacityMetricCatalogLeaseReceipt capacityMetrics = mechanics.ReadCatalogCapacityMetrics(catalog);
        MechanicsItemCatalogLeaseReceipt items = mechanics.ReadCatalogItems(catalog);
        MechanicsItemClassificationCatalogLeaseReceipt itemClassifications = mechanics.ReadCatalogItemClassifications(catalog);
        MechanicsItemCapacityCostCatalogLeaseReceipt itemCapacityCosts = mechanics.ReadCatalogItemCapacityCosts(catalog);
        MechanicsItemEquipmentPolicyCatalogLeaseReceipt itemEquipmentPolicies = mechanics.ReadCatalogItemEquipmentPolicies(catalog);
        MechanicsItemSourceCatalogLeaseReceipt itemSources = mechanics.ReadCatalogItemSources(catalog);
        MechanicsEquipmentSlotCatalogLeaseReceipt equipmentSlots = mechanics.ReadCatalogEquipmentSlots(catalog);
        MechanicsSlotClassificationCatalogLeaseReceipt slotClassifications = mechanics.ReadCatalogSlotClassifications(catalog);

        RequireCatalog(catalog, stats.CatalogId, tracks.CatalogId, sources.CatalogId, statContributions.CatalogId,
            damageKinds.CatalogId, damageResponses.CatalogId, effects.CatalogId, effectSources.CatalogId,
            capacityMetrics.CatalogId, items.CatalogId, itemClassifications.CatalogId, itemCapacityCosts.CatalogId,
            itemEquipmentPolicies.CatalogId, itemSources.CatalogId, equipmentSlots.CatalogId, slotClassifications.CatalogId);

        return new MechanicsCatalogInspection(identity.Entries.Span[0], stats.Entries, tracks.Entries, sources.Entries,
            statContributions.Entries, damageKinds.Entries, damageResponses.Entries, effects.Entries,
            effectSources.Entries, capacityMetrics.Entries, items.Entries, itemClassifications.Entries,
            itemCapacityCosts.Entries, itemEquipmentPolicies.Entries, itemSources.Entries, equipmentSlots.Entries,
            slotClassifications.Entries);
    }

    private static void RequireCatalog(MechanicsCatalog catalog, params ulong[] inspectedCatalogIds)
    {
        foreach (ulong inspectedCatalogId in inspectedCatalogIds)
        {
            if (inspectedCatalogId != catalog.Handle.Value)
            {
                throw new InvalidOperationException("Mechanics catalog inspection mixed catalog identities.");
            }
        }
    }
}
