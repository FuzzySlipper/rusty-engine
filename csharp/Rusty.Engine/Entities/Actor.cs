using System.Diagnostics.CodeAnalysis;

namespace Rusty.Engine.Entities;

/// <summary>
/// An optional discoverable facade over one existing <see cref="EntityStore"/> entity.
/// </summary>
/// <remarks>
/// <para>
/// Actor wraps an entity; it never owns one. Constructing an Actor attaches nothing, creates no
/// components, and starts no lifecycle: wrapping and explicit construction are distinct operations.
/// Every member reads the authoritative store live on each call, so a named downstream property
/// such as <c>actor.Get&lt;StatsComponent&gt;()</c> returns the same attached class instance the
/// store holds — never a copy, snapshot, or mirrored state graph. There is deliberately no
/// component cache to rebind and no registration or reflection anywhere on this path.
/// </para>
/// <para>
/// Downstream convenience lives one layer out: a product or Kit type holds an Actor and exposes
/// named properties over it (composition), rather than inheriting from it. Actor is sealed so that
/// pattern stays compositional instead of growing a template or inheritance chain.
/// </para>
/// <para>
/// Ownership: the facade holds references to the store and the entity id. Releasing the facade
/// (dropping the last reference, letting it be collected) does not destroy, detach, or otherwise
/// affect the entity — there is nothing to dispose and no cascade. Entity lifetime is governed
/// only by <see cref="EntityStore"/> (Destroy, lifecycle, store disposal).
/// </para>
/// <para>
/// Failure semantics are the store's ordinary semantics, unchanged: reading an unattached
/// component throws <see cref="InvalidOperationException"/>, as does any access after the entity
/// is destroyed or the store is disposed. New failure modes, revisions, permissions, and
/// transactions are intentionally absent.
/// </para>
/// </remarks>
public sealed class Actor
{
    /// <summary>
    /// Wraps an existing entity. The entity must already be known to the store; wrapping an
    /// unknown or destroyed entity throws <see cref="InvalidOperationException"/>.
    /// </summary>
    public Actor(EntityStore store, EntityId entity)
    {
        ArgumentNullException.ThrowIfNull(store);
        // Validate without touching components: wrapping must not create anything.
        store.GetLifecycle(entity);
        Store = store;
        Entity = entity;
    }

    /// <summary>The authoritative store this facade reads.</summary>
    public EntityStore Store { get; }

    /// <summary>The wrapped entity identity.</summary>
    public EntityId Entity { get; }

    /// <summary>The wrapped entity's kind/origin metadata.</summary>
    public EntityTypeId TypeId => Store.GetTypeId(Entity);

    /// <summary>The wrapped entity's current lifecycle.</summary>
    public EntityLifecycle Lifecycle => Store.GetLifecycle(Entity);

    /// <summary>Whether the wrapped entity is still known and not tombstoned.</summary>
    public bool IsAlive => Store.IsAlive(Entity);

    /// <summary>Whether the wrapped entity currently carries the component family.</summary>
    public bool Has<T>() where T : notnull => Store.Has<T>(Entity);

    /// <summary>
    /// Returns the attached component: the live class instance, or an ordinary C# copy of a
    /// value component. Throws <see cref="InvalidOperationException"/> when absent.
    /// </summary>
    public T Get<T>() where T : notnull => Store.Get<T>(Entity);

    /// <summary>Reads the attached component when present, following store membership semantics.</summary>
    public bool TryGet<T>([NotNullWhen(true)] out T? value) where T : notnull => Store.TryGet(Entity, out value);

    /// <summary>Attaches one instance/value. This is explicit composition, not wrapping.</summary>
    public void Add<T>(T value) where T : notnull => Store.Add(Entity, value);

    /// <summary>Replaces the attached value. Reads through this facade observe the replacement.</summary>
    public void Replace<T>(T value) where T : notnull => Store.Replace(Entity, value);

    /// <summary>Explicit value-fact insertion/replacement.</summary>
    public void Set<T>(T value) where T : struct => Store.Set(Entity, value);

    /// <summary>Detaches the component family; absent slots report false.</summary>
    public bool Remove<T>() where T : notnull => Store.Remove<T>(Entity);
}
