namespace Rusty.Engine.Entities;

/// <summary>A stable, product-declared identity for one typed component column.</summary>
public readonly record struct ComponentTypeKey : IComparable<ComponentTypeKey>
{
    internal ComponentTypeKey(uint value) : this()
    {
        if (value == 0)
        {
            throw new ArgumentOutOfRangeException(nameof(value), "Component type keys start at one.");
        }

        Value = value;
    }

    public uint Value { get; }

    public int CompareTo(ComponentTypeKey other) => Value.CompareTo(other.Value);

    internal bool IsProduct => Value >= ProductComponentKeys.FirstProductValue;

    internal bool IsEngine => Value is > 0 and <= EngineComponentKeys.LastEngineValue;
}

/// <summary>Copies a component value when an in-memory world snapshot is captured or restored.</summary>
public delegate T ComponentSnapshotCodec<T>(in T value) where T : struct;

/// <summary>Rejects one component value before it reaches live world state.</summary>
public delegate void ComponentValidator<T>(in T value) where T : struct;

/// <summary>Creates product component keys outside the Engine-reserved key range.</summary>
public static class ProductComponentKeys
{
    public const uint FirstProductValue = 1024;

    public static ComponentTypeKey Create(uint localId)
    {
        if (localId == 0 || localId > uint.MaxValue - FirstProductValue + 1)
        {
            throw new ArgumentOutOfRangeException(nameof(localId), "Product component local IDs must fit the product key range.");
        }
        return new ComponentTypeKey(checked(FirstProductValue - 1 + localId));
    }
}

internal static class EngineComponentKeys
{
    internal const uint LastEngineValue = ProductComponentKeys.FirstProductValue - 1;

    internal static ComponentTypeKey Create(uint localId)
    {
        if (localId == 0 || localId > LastEngineValue)
        {
            throw new ArgumentOutOfRangeException(nameof(localId));
        }
        return new ComponentTypeKey(localId);
    }
}

/// <summary>Non-generic identity used only by the Engine-maintained world storage.</summary>
public abstract class ComponentType
{
    internal ComponentType(ComponentTypeKey key)
    {
        Key = key;
    }

    public ComponentTypeKey Key { get; }

    internal abstract EntityWorld.ComponentTable CreateTable();
}

/// <summary>
/// A compile-time-safe descriptor for one component value type.
///
/// Product code keeps descriptors as ordinary static values and registers them with an
/// <see cref="EntityWorld"/>. There is no string lookup, reflection registration, or global
/// component registry.
/// </summary>
public sealed class ComponentType<T> : ComponentType where T : struct
{
    private ComponentType(ComponentTypeKey key, ComponentSnapshotCodec<T>? snapshotCodec, ComponentValidator<T>? validator)
        : base(key)
    {
        SnapshotCodec = snapshotCodec;
        Validator = validator;
    }

    public ComponentSnapshotCodec<T>? SnapshotCodec { get; }

    public ComponentValidator<T>? Validator { get; }

    public static ComponentType<T> Create(
        ComponentTypeKey key,
        ComponentSnapshotCodec<T>? snapshotCodec = null,
        ComponentValidator<T>? validator = null)
    {
        if (!key.IsProduct)
        {
            throw new ArgumentOutOfRangeException(nameof(key), "Product descriptors must use ProductComponentKeys.Create.");
        }
        return new(key, snapshotCodec, validator);
    }

    internal static ComponentType<T> CreateEngine(
        ComponentTypeKey key,
        ComponentSnapshotCodec<T>? snapshotCodec = null,
        ComponentValidator<T>? validator = null)
    {
        if (!key.IsEngine)
        {
            throw new ArgumentOutOfRangeException(nameof(key));
        }
        return new(key, snapshotCodec, validator);
    }

    internal void Validate(in T value) => Validator?.Invoke(in value);

    internal override EntityWorld.ComponentTable CreateTable() => new EntityWorld.ComponentTable<T>(this);
}
