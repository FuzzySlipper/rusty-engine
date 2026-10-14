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
    /// Publishes one snapshot in the caller's order. Every supplied entity must be active with a
    /// Transform. The Engine checks the hierarchy (parents present, no cycles; a repeated entity
    /// takes its last entry) against the whole snapshot and refuses it unchanged otherwise.
    /// </summary>
    public EntityGraphicsProjectionReceipt Publish(ReadOnlyMemory<EntityGraphicsProjectionEntry> entries)
    {
        ReadOnlySpan<EntityGraphicsProjectionEntry> span = entries.Span;
        var facts = new AppearanceFact[span.Length];
        for (int index = 0; index < span.Length; index++)
        {
            EntityGraphicsProjectionEntry entry = span[index];
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
}
