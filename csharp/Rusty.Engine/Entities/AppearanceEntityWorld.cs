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
    RenderLayer Layer,
    EntityId? Parent = null);

/// <summary>Exact managed and caller-supplied facts captured for one Appearance snapshot.</summary>
public readonly record struct AppearanceEntityWorldEntryGuard(
    EntityId Entity,
    ComponentRevision TransformRevision,
    AppearanceHandle Appearance,
    bool Visible,
    RenderLayer Layer,
    EntityId? Parent);

/// <summary>Copied evidence used to reject a stale Appearance projection before publishing.</summary>
public readonly record struct AppearanceEntityWorldGuard(
    ulong WorldRevision,
    ReadOnlyMemory<AppearanceEntityWorldEntryGuard> Entries);

/// <summary>A copied deterministic snapshot published through the generated Graphics family.</summary>
public readonly record struct AppearanceEntityWorldReceipt(
    AppearanceEntityWorldGuard Guard,
    ReadOnlyMemory<AppearanceFact> Facts);

/// <summary>
/// Projects active managed Transform values and caller-owned Appearance handles into one
/// generated Graphics snapshot. It retains neither an Appearance component nor a handle ownership mirror.
/// </summary>
public sealed class AppearanceEntityWorld
{
    private readonly EntityWorld _entities;
    private readonly IGraphicsService _graphics;

    public AppearanceEntityWorld(EntityWorld entities, IGraphicsService graphics)
    {
        _entities = entities ?? throw new ArgumentNullException(nameof(entities));
        _graphics = graphics ?? throw new ArgumentNullException(nameof(graphics));
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
        _graphics.PublishSnapshot(facts);
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
                entry.Layer,
                entry.Parent);
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
                entry.Parent is not null,
                entry.Parent?.Value ?? 0,
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
        var positions = new Dictionary<EntityId, int>(ordered.Length);
        for (int index = 0; index < ordered.Length; index++)
        {
            positions.Add(ordered[index].Entity, index);
        }
        var depths = new Dictionary<EntityId, int>(ordered.Length);
        var visiting = new HashSet<EntityId>();
        foreach (AppearanceEntityWorldEntry entry in ordered)
        {
            GetDepth(entry.Entity, ordered, positions, depths, visiting);
        }
        Array.Sort(ordered, (left, right) =>
        {
            int depth = depths[left.Entity].CompareTo(depths[right.Entity]);
            return depth != 0 ? depth : left.Entity.CompareTo(right.Entity);
        });
        return ordered;
    }

    private static int GetDepth(
        EntityId entity,
        ReadOnlySpan<AppearanceEntityWorldEntry> entries,
        IReadOnlyDictionary<EntityId, int> positions,
        IDictionary<EntityId, int> depths,
        ISet<EntityId> visiting)
    {
        if (depths.TryGetValue(entity, out int known)) return known;
        if (!visiting.Add(entity))
        {
            throw new ArgumentException($"Appearance snapshot has a parent cycle at entity {entity.Value}.", nameof(entries));
        }
        AppearanceEntityWorldEntry entry = entries[positions[entity]];
        int depth = 0;
        if (entry.Parent is EntityId parent)
        {
            if (!positions.ContainsKey(parent))
            {
                throw new ArgumentException(
                    $"Appearance entity {entity.Value} names parent {parent.Value}, which is not in this snapshot.",
                    nameof(entries));
            }
            depth = GetDepth(parent, entries, positions, depths, visiting) + 1;
        }
        visiting.Remove(entity);
        depths.Add(entity, depth);
        return depth;
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
