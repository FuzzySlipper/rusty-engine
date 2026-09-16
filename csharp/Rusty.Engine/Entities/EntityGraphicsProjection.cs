using Rusty.Engine;

namespace Rusty.Engine.Entities;

/// <summary>
/// A caller-owned Appearance handle attached to one managed entity for a single snapshot. It is
/// intentionally not an EntityStore component: handle lifetime remains with the caller.
/// </summary>
public readonly record struct EntityGraphicsProjectionEntry(
    EntityId Entity,
    Appearance Appearance,
    bool Visible,
    RenderLayer Layer,
    EntityId? Parent = null);

/// <summary>Exact managed and caller-supplied facts captured for one Appearance snapshot.</summary>
public readonly record struct EntityGraphicsProjectionEntryGuard(
    EntityId Entity,
    ComponentRevision TransformRevision,
    AppearanceHandle Appearance,
    bool Visible,
    RenderLayer Layer,
    EntityId? Parent);

/// <summary>Copied evidence used to reject a stale Appearance projection before publishing.</summary>
public readonly record struct EntityGraphicsProjectionGuard(
    ulong StoreRevision,
    ReadOnlyMemory<EntityGraphicsProjectionEntryGuard> Entries);

/// <summary>A copied deterministic snapshot published through the generated Graphics family.</summary>
public readonly record struct EntityGraphicsProjectionReceipt(
    EntityGraphicsProjectionGuard Guard,
    ReadOnlyMemory<AppearanceFact> Facts);

/// <summary>
/// Projects active managed Transform values and caller-owned Appearance handles into one
/// generated Graphics snapshot. It retains neither an Appearance component nor a handle ownership mirror.
/// </summary>
public sealed class EntityGraphicsProjection
{
    private readonly EntityStore _entities;
    private readonly IGraphicsService _graphics;

    public EntityGraphicsProjection(EntityStore entities, IGraphicsService graphics)
    {
        _entities = entities ?? throw new ArgumentNullException(nameof(entities));
        _graphics = graphics ?? throw new ArgumentNullException(nameof(graphics));
    }

    /// <summary>
    /// Publishes one bounded snapshot in ascending managed entity order. Every supplied entity
    /// must currently be active with Transform; duplicate entity bindings are rejected before
    /// the single generated family crossing.
    /// </summary>
    public EntityGraphicsProjectionReceipt Publish(
        ReadOnlyMemory<EntityGraphicsProjectionEntry> entries,
        int maximumEntities,
        EntityGraphicsProjectionGuard? expectedGuard = null)
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

        EntityGraphicsProjectionEntry[] ordered = OrderEntries(entries.Span);
        EntityGraphicsProjectionGuard guard = CaptureGuard(ordered);
        if (expectedGuard is EntityGraphicsProjectionGuard expected)
        {
            ValidateGuard(expected, guard);
        }

        AppearanceFact[] facts = ProjectFacts(ordered);
        ValidateGuard(guard, CaptureGuard(ordered));
        _graphics.PublishSnapshot(facts);
        return new EntityGraphicsProjectionReceipt(guard, facts);
    }

    private EntityGraphicsProjectionGuard CaptureGuard(ReadOnlySpan<EntityGraphicsProjectionEntry> entries)
    {
        var activeTransforms = new Dictionary<EntityId, Transform>();
        foreach (EntityComponent<Transform> row in _entities.Query(EngineComponentTypes.Transform))
        {
            activeTransforms.Add(row.Entity, row.Value);
        }

        var guards = new EntityGraphicsProjectionEntryGuard[entries.Length];
        for (int index = 0; index < entries.Length; index++)
        {
            EntityGraphicsProjectionEntry entry = entries[index];
            if (entry.Appearance is null)
            {
                throw new ArgumentNullException(nameof(entries), $"Appearance entity {entry.Entity.Value} has no caller-owned handle.");
            }
            if (!activeTransforms.ContainsKey(entry.Entity))
            {
                throw new InvalidOperationException(
                    $"Appearance entity {entry.Entity.Value} must be active with a Transform component.");
            }
            guards[index] = new EntityGraphicsProjectionEntryGuard(
                entry.Entity,
                _entities.GetComponentRevision(entry.Entity, EngineComponentTypes.Transform),
                entry.Appearance.Handle,
                entry.Visible,
                entry.Layer,
                entry.Parent);
        }

        return new EntityGraphicsProjectionGuard(_entities.Revision, guards);
    }

    private AppearanceFact[] ProjectFacts(ReadOnlySpan<EntityGraphicsProjectionEntry> entries)
    {
        var facts = new AppearanceFact[entries.Length];
        for (int index = 0; index < entries.Length; index++)
        {
            EntityGraphicsProjectionEntry entry = entries[index];
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

    private static EntityGraphicsProjectionEntry[] OrderEntries(ReadOnlySpan<EntityGraphicsProjectionEntry> entries)
    {
        EntityGraphicsProjectionEntry[] ordered = entries.ToArray();
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
        foreach (EntityGraphicsProjectionEntry entry in ordered)
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
        ReadOnlySpan<EntityGraphicsProjectionEntry> entries,
        IReadOnlyDictionary<EntityId, int> positions,
        IDictionary<EntityId, int> depths,
        ISet<EntityId> visiting)
    {
        if (depths.TryGetValue(entity, out int known)) return known;
        if (!visiting.Add(entity))
        {
            throw new ArgumentException($"Appearance snapshot has a parent cycle at entity {entity.Value}.", nameof(entries));
        }
        EntityGraphicsProjectionEntry entry = entries[positions[entity]];
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

    private static void ValidateGuard(EntityGraphicsProjectionGuard expected, EntityGraphicsProjectionGuard observed)
    {
        if (expected.StoreRevision != observed.StoreRevision)
        {
            throw new InvalidOperationException(
                $"Appearance managed store revision is stale: expected {expected.StoreRevision}, actual {observed.StoreRevision}.");
        }
        ReadOnlySpan<EntityGraphicsProjectionEntryGuard> expectedEntries = expected.Entries.Span;
        ReadOnlySpan<EntityGraphicsProjectionEntryGuard> observedEntries = observed.Entries.Span;
        if (!expectedEntries.SequenceEqual(observedEntries))
        {
            throw new InvalidOperationException("Appearance managed projection is stale.");
        }
    }
}
