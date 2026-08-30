namespace Rusty.Engine.Entities;

/// <summary>
/// Purpose-narrow immutable metadata captured for an EntityWorld debug read. Component values
/// remain typed and can only be rendered by an explicitly registered projection.
/// </summary>
internal sealed record EntityWorldDebugSnapshot(
    ulong Revision,
    ulong NextEntityValue,
    IReadOnlyList<EntityWorldDebugEntitySnapshot> Entities,
    IReadOnlyList<EntityWorldDebugComponentFamily> ComponentFamilies);

internal sealed record EntityWorldDebugComponentFamily(ComponentTypeKey Key, ComponentType Descriptor);

internal sealed record EntityWorldDebugEntitySnapshot(
    EntityId Entity,
    EntityLifecycle Lifecycle,
    ulong Revision,
    EntityId? Container,
    IReadOnlyList<EntityId> Children,
    IReadOnlyList<EntityWorldDebugComponentPresence> Components);

internal sealed record EntityWorldDebugComponentPresence(
    ComponentTypeKey Key,
    ComponentType Descriptor,
    ulong Revision);
