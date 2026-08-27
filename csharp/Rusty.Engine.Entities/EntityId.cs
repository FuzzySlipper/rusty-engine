namespace Rusty.Engine.Entities;

/// <summary>Identity allocated deterministically by one <see cref="EntityWorld"/>.</summary>
public readonly record struct EntityId(ulong Value) : IComparable<EntityId>
{
    public int CompareTo(EntityId other) => Value.CompareTo(other.Value);
}

public enum EntityLifecycle
{
    Active,
    Disabled,
    Tombstoned,
}

public readonly record struct EntityRevision(EntityId Entity, ulong Revision);

public readonly record struct ComponentRevision(EntityId Entity, ComponentTypeKey Component, ulong Revision);

public readonly record struct EntityComponent<T>(EntityId Entity, T Value) where T : struct;

public readonly record struct EntityComponents<TFirst, TSecond>(EntityId Entity, TFirst First, TSecond Second)
    where TFirst : struct
    where TSecond : struct;
