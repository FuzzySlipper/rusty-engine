using Rusty.Engine;

namespace Rusty.Engine.Entities;

/// <summary>
/// A caller-owned Appearance handle attached to one managed entity for a single snapshot. It is
/// intentionally not an EntityWorld component: handle lifetime remains with the caller.
/// </summary>
public readonly record struct AppearanceEntityWorldEntry(
    EntityId Entity,
    Appearance Appearance,
    bool Visible,
    RenderLayer Layer);

/// <summary>Exact managed and caller-supplied facts captured for one Appearance snapshot.</summary>
public readonly record struct AppearanceEntityWorldEntryGuard(
    EntityId Entity,
    ComponentRevision TransformRevision,
    AppearanceHandle Appearance,
    bool Visible,
    RenderLayer Layer);

/// <summary>Copied evidence used to reject a stale Appearance projection before publishing.</summary>
public readonly record struct AppearanceEntityWorldGuard(
    ulong WorldRevision,
    ReadOnlyMemory<AppearanceEntityWorldEntryGuard> Entries);

/// <summary>A copied deterministic snapshot published through the generated Appearance family.</summary>
public readonly record struct AppearanceEntityWorldReceipt(
    AppearanceEntityWorldGuard Guard,
    ReadOnlyMemory<AppearanceFact> Facts);

/// <summary>
/// Projects active managed Transform values and caller-owned Appearance handles into one
/// generated snapshot. It retains neither an Appearance component nor a handle ownership mirror.
/// </summary>
public sealed class AppearanceEntityWorld
{
    private readonly EntityWorld _entities;
    private readonly IAppearanceService _appearance;

    public AppearanceEntityWorld(EntityWorld entities, IAppearanceService appearance)
    {
        _entities = entities ?? throw new ArgumentNullException(nameof(entities));
        _appearance = appearance ?? throw new ArgumentNullException(nameof(appearance));
    }

    /// <summary>
    /// Publishes one bounded snapshot in ascending managed entity order. Every supplied entity
    /// must currently be active with Transform; duplicate entity bindings are rejected before
    /// the single generated family crossing.
    /// </summary>
    public AppearanceEntityWorldReceipt Publish(
        ReadOnlyMemory<AppearanceEntityWorldEntry> entries,
        int maximumEntities,
        AppearanceEntityWorldGuard? expectedGuard = null)
    {
        if (maximumEntities < 0)
        {
            throw new ArgumentOutOfRangeException(nameof(maximumEntities));
        }
        if (entries.Length > maximumEntities)
        {
            throw new InvalidOperationException(
                $"Appearance snapshot has {entries.Length} entities, exceeding its explicit batch bound {maximumEntities}.");
        }

        AppearanceEntityWorldEntry[] ordered = OrderEntries(entries.Span);
        AppearanceEntityWorldGuard guard = CaptureGuard(ordered);
        if (expectedGuard is AppearanceEntityWorldGuard expected)
        {
            ValidateGuard(expected, guard);
        }

        AppearanceFact[] facts = ProjectFacts(ordered);
        ValidateGuard(guard, CaptureGuard(ordered));
        _appearance.PublishSnapshot(facts);
        return new AppearanceEntityWorldReceipt(guard, facts);
    }

    private AppearanceEntityWorldGuard CaptureGuard(ReadOnlySpan<AppearanceEntityWorldEntry> entries)
    {
        var activeTransforms = new Dictionary<EntityId, Transform>();
        foreach (EntityComponent<Transform> row in _entities.Query(EngineComponentTypes.Transform))
        {
            activeTransforms.Add(row.Entity, row.Value);
        }

        var guards = new AppearanceEntityWorldEntryGuard[entries.Length];
        for (int index = 0; index < entries.Length; index++)
        {
            AppearanceEntityWorldEntry entry = entries[index];
            if (entry.Appearance is null)
            {
                throw new ArgumentNullException(nameof(entries), $"Appearance entity {entry.Entity.Value} has no caller-owned handle.");
            }
            if (!activeTransforms.ContainsKey(entry.Entity))
            {
                throw new InvalidOperationException(
                    $"Appearance entity {entry.Entity.Value} must be active with a Transform component.");
            }
            guards[index] = new AppearanceEntityWorldEntryGuard(
                entry.Entity,
                _entities.GetComponentRevision(entry.Entity, EngineComponentTypes.Transform),
                entry.Appearance.Handle,
                entry.Visible,
                entry.Layer);
        }

        return new AppearanceEntityWorldGuard(_entities.Revision, guards);
    }

    private AppearanceFact[] ProjectFacts(ReadOnlySpan<AppearanceEntityWorldEntry> entries)
    {
        var facts = new AppearanceFact[entries.Length];
        for (int index = 0; index < entries.Length; index++)
        {
            AppearanceEntityWorldEntry entry = entries[index];
            facts[index] = new AppearanceFact(
                entry.Entity.Value,
                _entities.Get(entry.Entity, EngineComponentTypes.Transform),
                entry.Appearance,
                entry.Visible,
                entry.Layer);
        }
        return facts;
    }

    private static AppearanceEntityWorldEntry[] OrderEntries(ReadOnlySpan<AppearanceEntityWorldEntry> entries)
    {
        AppearanceEntityWorldEntry[] ordered = entries.ToArray();
        Array.Sort(ordered, static (left, right) => left.Entity.CompareTo(right.Entity));
        for (int index = 1; index < ordered.Length; index++)
        {
            if (ordered[index - 1].Entity == ordered[index].Entity)
            {
                throw new ArgumentException(
                    $"Appearance snapshot contains duplicate entity {ordered[index].Entity.Value}.",
                    nameof(entries));
            }
        }
        return ordered;
    }

    private static void ValidateGuard(AppearanceEntityWorldGuard expected, AppearanceEntityWorldGuard observed)
    {
        if (expected.WorldRevision != observed.WorldRevision)
        {
            throw new InvalidOperationException(
                $"Appearance managed world revision is stale: expected {expected.WorldRevision}, actual {observed.WorldRevision}.");
        }
        ReadOnlySpan<AppearanceEntityWorldEntryGuard> expectedEntries = expected.Entries.Span;
        ReadOnlySpan<AppearanceEntityWorldEntryGuard> observedEntries = observed.Entries.Span;
        if (!expectedEntries.SequenceEqual(observedEntries))
        {
            throw new InvalidOperationException("Appearance managed projection is stale.");
        }
    }
}
