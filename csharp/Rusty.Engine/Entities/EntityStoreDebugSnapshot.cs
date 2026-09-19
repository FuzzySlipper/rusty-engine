namespace Rusty.Engine.Entities;

/// <summary>
/// Purpose-narrow immutable metadata captured for an EntityStore debug read. Component values
/// remain typed and can only be rendered by an explicitly registered projection.
/// </summary>
internal sealed record EntityStoreDebugSnapshot(
    ulong Revision,
    ulong NextEntityValue,
    IReadOnlyList<EntityStoreDebugEntitySnapshot> Entities,
    IReadOnlyList<EntityStoreDebugComponentFamily> ComponentFamilies);

internal sealed record EntityStoreDebugComponentFamily(ComponentTypeKey Key, ComponentType Descriptor);

internal sealed record EntityStoreDebugEntitySnapshot(
    EntityId Entity,
    EntityLifecycle Lifecycle,
    ulong Revision,
    EntityTypeId TypeId,
    EntityId? Container,
    IReadOnlyList<EntityId> Children,
    IReadOnlyList<EntityStoreDebugComponentPresence> Components);

internal sealed record EntityStoreDebugComponentPresence(
    ComponentTypeKey Key,
    ComponentType Descriptor,
    ulong Revision);
