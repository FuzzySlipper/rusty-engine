namespace Rusty.Engine.Entities;

public sealed record EntityWorldDiagnostics(
    ulong Revision,
    ulong NextEntityValue,
    int EntityCount,
    int ActiveCount,
    int DisabledCount,
    int TombstonedCount,
    IReadOnlyList<ComponentTypeDiagnostics> Components);

public sealed record ComponentTypeDiagnostics(
    ComponentTypeKey Key,
    int ValueCount,
    IReadOnlyList<EntityId> EntitySample);
