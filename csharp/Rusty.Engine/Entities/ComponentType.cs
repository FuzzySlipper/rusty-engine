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

/// <summary>Rejects one component value before it reaches live store state.</summary>
public delegate void ComponentValidator<T>(in T value) where T : notnull;

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

/// <summary>Non-generic identity used only by the Engine-maintained store storage.</summary>
public abstract class ComponentType
{
    internal ComponentType(ComponentTypeKey key)
    {
        Key = key;
    }

    public ComponentTypeKey Key { get; }

    internal abstract Type Family { get; }

    internal bool IsAutomatic { get; init; }

    internal abstract EntityStore.ComponentTable CreateTable();
}

/// <summary>
/// Optional descriptor for a typed family that needs explicit validation or debug identity. Ordinary Add/Get/Query require no descriptor. A descriptor and
/// generic access share the same store-local family; there is no global component registry.
/// </summary>
public sealed class ComponentType<T> : ComponentType where T : notnull
{
    private ComponentType(ComponentTypeKey key, ComponentValidator<T>? validator)
        : base(key)
    {
        Validator = validator;
    }

    public ComponentValidator<T>? Validator { get; }

    public static ComponentType<T> Create(
        ComponentTypeKey key,
        ComponentValidator<T>? validator = null)
    {
        if (!key.IsProduct)
        {
            throw new ArgumentOutOfRangeException(nameof(key), "Product descriptors must use ProductComponentKeys.Create.");
        }
        return new(key, validator);
    }

    internal static ComponentType<T> CreateEngine(
        ComponentTypeKey key,
        ComponentValidator<T>? validator = null)
    {
        if (!key.IsEngine)
        {
            throw new ArgumentOutOfRangeException(nameof(key));
        }
        return new(key, validator);
    }

    internal void Validate(in T value) => Validator?.Invoke(in value);

    internal override EntityStore.ComponentTable CreateTable() => new EntityStore.ComponentTable<T>(this);

    internal override Type Family => typeof(T);

    internal static ComponentType<T> CreateAutomatic(ComponentTypeKey key)
        => new(key, null) { IsAutomatic = true };
}
