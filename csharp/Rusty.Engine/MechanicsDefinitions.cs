namespace Rusty.Engine;

/// <summary>
/// Typed construction helpers for the exact Mechanics catalog and initial
/// component records generated from the Engine ABI.
/// </summary>
public static class MechanicsDefinitions
{
    public static MechanicsDamageResponseDefinitionRequest Prevent(
        MechanicsCatalog catalog, string source, bool exactSelector, string damageKind,
        string stackingGroup, MechanicsStackingPolicy stacking) =>
        new(catalog, source, MechanicsDamageResponseKind.Prevent, exactSelector, damageKind,
            0, 0, 0, stackingGroup, stacking, string.Empty);

    public static MechanicsDamageResponseDefinitionRequest FlatReduction(
        MechanicsCatalog catalog, string source, bool exactSelector, string damageKind,
        long amount, string stackingGroup, MechanicsStackingPolicy stacking) =>
        new(catalog, source, MechanicsDamageResponseKind.FlatReduction, exactSelector, damageKind,
            amount, 0, 0, stackingGroup, stacking, string.Empty);

    public static MechanicsDamageResponseDefinitionRequest Scale(
        MechanicsCatalog catalog, string source, bool exactSelector, string damageKind,
        uint numerator, uint denominator, string stackingGroup, MechanicsStackingPolicy stacking) =>
        new(catalog, source, MechanicsDamageResponseKind.Scale, exactSelector, damageKind,
            0, numerator, denominator, stackingGroup, stacking, string.Empty);

    public static MechanicsDamageResponseDefinitionRequest Absorb(
        MechanicsCatalog catalog, string source, bool exactSelector, string damageKind, string track) =>
        new(catalog, source, MechanicsDamageResponseKind.Absorb, exactSelector, damageKind,
            0, 0, 0, string.Empty, MechanicsStackingPolicy.Sum, track);

    public static MechanicsEffectDefinitionRequest IndependentEffect(
        MechanicsCatalog catalog, string id, string stackingGroup, ushort maximumInstances,
        ushort maximumStacks, ReadOnlyMemory<MechanicsText> sources) =>
        new(catalog, id, stackingGroup, MechanicsEffectStackingKind.IndependentByProvenance,
            maximumInstances, maximumStacks, sources);

    public static MechanicsEffectDefinitionRequest RefreshEffect(
        MechanicsCatalog catalog, string id, string stackingGroup, ushort maximumStacks,
        ReadOnlyMemory<MechanicsText> sources) =>
        new(catalog, id, stackingGroup, MechanicsEffectStackingKind.Refresh, 0, maximumStacks, sources);

    public static MechanicsEffectDefinitionRequest ReplaceEffect(
        MechanicsCatalog catalog, string id, string stackingGroup, ushort maximumStacks,
        ReadOnlyMemory<MechanicsText> sources) =>
        new(catalog, id, stackingGroup, MechanicsEffectStackingKind.Replace, 0, maximumStacks, sources);

    public static MechanicsInitialActiveEffect IntrinsicEffect(
        string instance, string definition, ulong entityId, string sourceInstance, ushort stacks) =>
        new(instance, definition, MechanicsActiveEffectProvenanceKind.Intrinsic, entityId,
            string.Empty, 0, string.Empty, 0, string.Empty, sourceInstance, stacks);

    public static MechanicsInitialActiveEffect EffectEffect(
        string instance, string definition, ulong entityId, string effectInstance, ushort stack,
        string source, ushort stacks) =>
        new(instance, definition, MechanicsActiveEffectProvenanceKind.Effect, entityId,
            effectInstance, stack, source, 0, string.Empty, string.Empty, stacks);

    public static MechanicsInitialActiveEffect EquippedItemEffect(
        string instance, string definition, ulong ownerEntityId, ulong itemEntityId, string source,
        ushort stacks) =>
        new(instance, definition, MechanicsActiveEffectProvenanceKind.EquippedItem, ownerEntityId,
            string.Empty, 0, source, itemEntityId, string.Empty, string.Empty, stacks);

    public static MechanicsInitialActiveEffect RequestEffect(
        string instance, string definition, string operation, string sourceInstance, ushort stacks) =>
        new(instance, definition, MechanicsActiveEffectProvenanceKind.Request, 0,
            string.Empty, 0, string.Empty, 0, operation, sourceInstance, stacks);
}
