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

/// <summary>The facts one Appearance snapshot published.</summary>
public readonly record struct EntityGraphicsProjectionReceipt(ReadOnlyMemory<AppearanceFact> Facts);

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
    /// Publishes one snapshot with parents before children, then ascending entity order. Every
    /// supplied entity must be active with a Transform.
    /// </summary>
    public EntityGraphicsProjectionReceipt Publish(ReadOnlyMemory<EntityGraphicsProjectionEntry> entries)
    {
        EntityGraphicsProjectionEntry[] ordered = OrderEntries(entries.Span);
        var facts = new AppearanceFact[ordered.Length];
        for (int index = 0; index < ordered.Length; index++)
        {
            EntityGraphicsProjectionEntry entry = ordered[index];
            if (_entities.GetLifecycle(entry.Entity) != EntityLifecycle.Active)
            {
                throw new InvalidOperationException($"Appearance entity {entry.Entity.Value} must be active.");
            }
            facts[index] = new AppearanceFact(
                entry.Entity.Value,
                entry.Parent is not null,
                entry.Parent?.Value ?? 0,
                _entities.Get(entry.Entity, EngineComponentTypes.Transform),
                entry.Appearance ?? throw new ArgumentNullException(
                    nameof(entries), $"Appearance entity {entry.Entity.Value} has no caller-owned handle."),
                entry.Visible,
                entry.Layer);
        }
        _graphics.PublishSnapshot(facts);
        return new EntityGraphicsProjectionReceipt(facts);
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
}
