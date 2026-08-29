using Rusty.Engine.Entities;

namespace Rusty.Engine.Mechanics;

/// <summary>How active effects sharing a stacking group interact.</summary>
public enum EffectStackingPolicy
{
    IndependentByProvenance,
    Refresh,
    Replace,
}

/// <summary>Kind of an explicit active-effect mutation.</summary>
public enum EffectMutationKind
{
    Apply,
    Refresh,
    Replace,
    Remove,
    Expire,
}

/// <summary>
/// Product-authored effect metadata. Duration, timing, names, and
/// consequences stay outside this reusable stacking mechanism.
/// </summary>
public sealed class EffectDefinition
{
    public const ushort MaximumSupportedStacks = 32;

    public EffectDefinition(
        EffectDefinitionId id,
        StackingGroupId stackingGroup,
        EffectStackingPolicy stacking,
        ushort maximumInstances,
        ushort maximumStacks,
        IEnumerable<SourceDefinitionId>? sourceDefinitions = null)
    {
        Id = id ?? throw new ArgumentNullException(nameof(id));
        StackingGroup = stackingGroup ?? throw new ArgumentNullException(nameof(stackingGroup));
        if (maximumStacks == 0 || maximumStacks > MaximumSupportedStacks)
        {
            throw new ArgumentOutOfRangeException(
                nameof(maximumStacks),
                $"Effect stack limits must be between one and {MaximumSupportedStacks}.");
        }

        if (stacking == EffectStackingPolicy.IndependentByProvenance && maximumInstances == 0)
        {
            throw new ArgumentOutOfRangeException(
                nameof(maximumInstances),
                "Independent effects need at least one allowed instance.");
        }

        SourceDefinitions = CopyAndSortSources(sourceDefinitions);
        Stacking = stacking;
        MaximumInstances = maximumInstances;
        MaximumStacks = maximumStacks;
    }

    public EffectDefinitionId Id { get; }

    public StackingGroupId StackingGroup { get; }

    public EffectStackingPolicy Stacking { get; }

    public ushort MaximumInstances { get; }

    public ushort MaximumStacks { get; }

    public IReadOnlyList<SourceDefinitionId> SourceDefinitions { get; }

    private static IReadOnlyList<SourceDefinitionId> CopyAndSortSources(
        IEnumerable<SourceDefinitionId>? sourceDefinitions)
    {
        if (sourceDefinitions is null)
        {
            return Array.Empty<SourceDefinitionId>();
        }

        SourceDefinitionId[] sources = sourceDefinitions
            .Select(source => source ?? throw new ArgumentException("Effect source IDs cannot be null."))
            .OrderBy(source => source.Value, StringComparer.Ordinal)
            .ToArray();
        if (sources.Distinct().Count() != sources.Length)
        {
            throw new ArgumentException("An effect cannot activate one source definition twice.");
        }

        return Array.AsReadOnly(sources);
    }
}

/// <summary>One validated active effect instance.</summary>
public sealed record ActiveEffect
{
    public ActiveEffect(
        EffectInstanceId instance,
        EffectDefinition definition,
        MechanicsSourceIdentity provenance,
        ushort stacks)
    {
        Instance = instance ?? throw new ArgumentNullException(nameof(instance));
        Definition = definition ?? throw new ArgumentNullException(nameof(definition));
        Provenance = provenance ?? throw new ArgumentNullException(nameof(provenance));
        if (stacks == 0 || stacks > definition.MaximumStacks)
        {
            throw new ArgumentOutOfRangeException(
                nameof(stacks),
                $"Active effect stacks must be between one and {definition.MaximumStacks}.");
        }

        Stacks = stacks;
    }

    public EffectInstanceId Instance { get; }

    public EffectDefinition Definition { get; }

    public EffectDefinitionId DefinitionId => Definition.Id;

    public MechanicsSourceIdentity Provenance { get; }

    public ushort Stacks { get; }
}

/// <summary>Result of one explicit effect mutation.</summary>
public sealed class EffectMutationReceipt
{
    internal EffectMutationReceipt(
        EffectMutationKind kind,
        IReadOnlyList<ActiveEffect> removed,
        ActiveEffect? current,
        IReadOnlyList<EffectSourceActivation> activatedSources)
    {
        Kind = kind;
        Removed = removed;
        Current = current;
        ActivatedSources = activatedSources;
    }

    public EffectMutationKind Kind { get; }

    public IReadOnlyList<ActiveEffect> Removed { get; }

    public ActiveEffect? Current { get; }

    public IReadOnlyList<EffectSourceActivation> ActivatedSources { get; }
}

/// <summary>
/// Mutable product-owned collection of active effects. It performs stacking
/// and provenance checks only; callers decide when an effect expires or what
/// an effect means. Every mutation validates a complete candidate before
/// publishing it.
/// </summary>
public sealed class EffectState
{
    public const int MaximumActiveEffects = 64;

    private readonly List<ActiveEffect> _effects = [];

    public EffectState(EntityId? owner = null)
    {
        Owner = owner;
    }

    public EntityId? Owner { get; }

    public IReadOnlyList<ActiveEffect> Effects =>
        Array.AsReadOnly(_effects.ToArray());

    public EffectMutationReceipt Apply(
        EffectDefinition definition,
        EffectInstanceId instance,
        MechanicsSourceIdentity provenance,
        ushort stacks)
    {
        ArgumentNullException.ThrowIfNull(definition);
        ArgumentNullException.ThrowIfNull(instance);
        ArgumentNullException.ThrowIfNull(provenance);
        ActiveEffect current = new(instance, definition, provenance, stacks);
        if (_effects.Any(effect => effect.Instance == instance))
        {
            throw new MechanicsException($"Effect instance {instance} is already active.");
        }

        ActiveEffect[] matching = _effects
            .Where(effect => effect.Definition.StackingGroup == definition.StackingGroup)
            .ToArray();
        switch (definition.Stacking)
        {
            case EffectStackingPolicy.IndependentByProvenance:
                if (matching.Any(effect => effect.Provenance == provenance))
                {
                    throw new MechanicsException(
                        $"Effect group {definition.StackingGroup} already has this provenance.");
                }

                if (matching.Length >= definition.MaximumInstances)
                {
                    throw new MechanicsException(
                        $"Effect group {definition.StackingGroup} has reached its instance limit.");
                }
                break;
            case EffectStackingPolicy.Refresh:
            case EffectStackingPolicy.Replace:
                if (matching.Length != 0)
                {
                    throw new MechanicsException(
                        $"Effect group {definition.StackingGroup} requires {definition.Stacking.ToString().ToLowerInvariant()} instead of apply.");
                }
                break;
            default:
                throw new MechanicsException("Unknown effect stacking policy.");
        }

        return Publish(
            EffectMutationKind.Apply,
            [.. _effects, current],
            [],
            current);
    }

    public EffectMutationReceipt Refresh(
        EffectInstanceId instance,
        MechanicsSourceIdentity provenance,
        ushort stacks)
    {
        ArgumentNullException.ThrowIfNull(instance);
        ArgumentNullException.ThrowIfNull(provenance);
        int index = FindIndex(instance);
        ActiveEffect previous = _effects[index];
        if (previous.Definition.Stacking != EffectStackingPolicy.Refresh)
        {
            throw new MechanicsException($"Effect {instance} does not use refresh stacking.");
        }

        ActiveEffect current = new(instance, previous.Definition, provenance, stacks);
        List<ActiveEffect> candidate = [.. _effects];
        candidate[index] = current;
        return Publish(EffectMutationKind.Refresh, candidate, [previous], current);
    }

    public EffectMutationReceipt Replace(
        EffectDefinition definition,
        EffectInstanceId instance,
        MechanicsSourceIdentity provenance,
        ushort stacks)
    {
        ArgumentNullException.ThrowIfNull(definition);
        ArgumentNullException.ThrowIfNull(instance);
        ArgumentNullException.ThrowIfNull(provenance);
        if (definition.Stacking != EffectStackingPolicy.Replace)
        {
            throw new MechanicsException($"Effect {definition.Id} does not use replace stacking.");
        }

        ActiveEffect current = new(instance, definition, provenance, stacks);
        List<ActiveEffect> candidate = [];
        List<ActiveEffect> removed = [];
        foreach (ActiveEffect effect in _effects)
        {
            if (effect.Definition.StackingGroup == definition.StackingGroup)
            {
                removed.Add(effect);
            }
            else
            {
                candidate.Add(effect);
            }
        }

        if (candidate.Any(effect => effect.Instance == instance))
        {
            throw new MechanicsException($"Effect instance {instance} is already active.");
        }

        candidate.Add(current);
        return Publish(EffectMutationKind.Replace, candidate, removed, current);
    }

    /// <summary>Removes one active effect at the caller's chosen time.</summary>
    public EffectMutationReceipt Remove(EffectInstanceId instance) =>
        RemoveCore(instance, EffectMutationKind.Remove);

    /// <summary>Records an explicit caller-driven expiry; it owns no timer.</summary>
    public EffectMutationReceipt Expire(EffectInstanceId instance) =>
        RemoveCore(instance, EffectMutationKind.Expire);

    private EffectMutationReceipt RemoveCore(EffectInstanceId instance, EffectMutationKind kind)
    {
        ArgumentNullException.ThrowIfNull(instance);
        int index = FindIndex(instance);
        ActiveEffect removed = _effects[index];
        List<ActiveEffect> candidate = [.. _effects];
        candidate.RemoveAt(index);
        return Publish(kind, candidate, [removed], null);
    }

    private EffectMutationReceipt Publish(
        EffectMutationKind kind,
        IEnumerable<ActiveEffect> candidate,
        IEnumerable<ActiveEffect> removed,
        ActiveEffect? current)
    {
        ActiveEffect[] validated = candidate
            .OrderBy(effect => effect.Instance.Value, StringComparer.Ordinal)
            .ToArray();
        ValidateCollection(validated);
        EffectSourceActivation[] activations = current is null
            ? []
            : ActivateSources(current);

        _effects.Clear();
        _effects.AddRange(validated);
        ActiveEffect[] removedArray = removed
            .OrderBy(effect => effect.Instance.Value, StringComparer.Ordinal)
            .ToArray();
        return new EffectMutationReceipt(
            kind,
            Array.AsReadOnly(removedArray),
            current,
            Array.AsReadOnly(activations));
    }

    private EffectSourceActivation[] ActivateSources(ActiveEffect effect)
    {
        List<EffectSourceActivation> activations = [];
        for (ushort stack = 1; stack <= effect.Stacks; stack++)
        {
            foreach (SourceDefinitionId source in effect.Definition.SourceDefinitions)
            {
                activations.Add(new EffectSourceActivation(
                    new EffectSourceIdentity(Owner, effect.Instance, stack, source),
                    source));
            }
        }

        return activations.ToArray();
    }

    private void ValidateCollection(IReadOnlyList<ActiveEffect> effects)
    {
        if (effects.Count > MaximumActiveEffects)
        {
            throw new MechanicsException(
                $"An effect state cannot contain more than {MaximumActiveEffects} active effects.");
        }

        if (effects.Select(effect => effect.Instance).Distinct().Count() != effects.Count)
        {
            throw new MechanicsException("Active effect instances must have unique identities.");
        }

        foreach (IGrouping<StackingGroupId, ActiveEffect> group in effects.GroupBy(
            effect => effect.Definition.StackingGroup))
        {
            ActiveEffect[] members = group.ToArray();
            EffectStackingPolicy policy = members[0].Definition.Stacking;
            if (members.Any(effect => effect.Definition.Stacking != policy))
            {
                throw new MechanicsException(
                    $"Effect group {group.Key} uses more than one stacking policy.");
            }

            if (policy == EffectStackingPolicy.IndependentByProvenance)
            {
                if (members.Length > members[0].Definition.MaximumInstances
                    || members.Select(effect => effect.Provenance).Distinct().Count() != members.Length)
                {
                    throw new MechanicsException(
                        $"Effect group {group.Key} violates independent stacking limits.");
                }
            }
            else if (members.Length > 1)
            {
                throw new MechanicsException(
                    $"Effect group {group.Key} allows only one active effect.");
            }
        }
    }

    private int FindIndex(EffectInstanceId instance)
    {
        int index = _effects.FindIndex(effect => effect.Instance == instance);
        return index >= 0
            ? index
            : throw new MechanicsException($"Effect instance {instance} is not active.");
    }
}
