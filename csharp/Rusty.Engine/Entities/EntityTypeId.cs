namespace Rusty.Engine.Entities;

/// <summary>
/// Kind/origin metadata for one entity, stored on the canonical entity record.
/// </summary>
/// <remarks>
/// <para>
/// TypeId describes what kind of thing an entity is or where it came from, independently of
/// the unique runtime <see cref="EntityId"/> allocated by its store and of any product-owned
/// durable identity. It is useful for debugging and may name a runtime-created kind without
/// resolving an authored definition.
/// </para>
/// <para>
/// The value is an ordinary free-form string fixed at creation: there is no registry, no naming
/// grammar, and no validation. <see cref="Unspecified"/> (the default) means the creator did not
/// name a kind; null and empty values normalize to it. It is metadata, not a component: it is
/// read through <see cref="EntityStore.GetTypeId"/> rather than component access.
/// </para>
/// </remarks>
public readonly record struct EntityTypeId
{
    private readonly string? _value;

    public EntityTypeId(string? value) => _value = string.IsNullOrEmpty(value) ? null : value;

    /// <summary>The named kind, or <see cref="string.Empty"/> when unspecified. Never null.</summary>
    public string Value => _value ?? string.Empty;

    /// <summary>No kind was named. This is the default for <see cref="EntityStore.Create(EntityLifecycle)"/>.</summary>
    public static EntityTypeId Unspecified => default;

    /// <summary>Whether a kind was named. Empty and null values count as unspecified.</summary>
    public bool IsSpecified => _value is not null;

    public override string ToString() => Value;
}
