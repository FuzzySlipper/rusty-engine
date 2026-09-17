using System.Globalization;
using System.Text;
using Rusty.Engine.Entities;

namespace Rusty.Engine.Debugging;

/// <summary>Restricts the entity rows returned by <see cref="EntityStoreDebugModule"/>.</summary>
public enum EntityStoreDebugSelector
{
    All,
    Active,
    Disabled,
    Tombstoned,
}

/// <summary>Renders one explicitly opted-in component value for live debug output.</summary>
public delegate string EntityStoreDebugProjection<T>(in T value) where T : notnull;

/// <summary>
/// Read-only live inspection for explicitly product-registered <see cref="EntityStore"/>
/// instances. It is a normal generated debug-command module: products create one instance,
/// register their stores and typed projections, then register that instance with their catalog.
/// </summary>
public sealed partial class EntityStoreDebugModule : IDebugCommandModule
{
    public const int MaximumPageSize = 64;
    public const int MaximumResultLength = 4096;
    public const int MaximumStoreNameLength = 64;

    private readonly SortedDictionary<string, EntityStore> _stores = new(StringComparer.Ordinal);
    private readonly SortedDictionary<ComponentTypeKey, Projection> _projections = [];
    private readonly Dictionary<Type, Projection> _typedProjections = [];

    /// <summary>Registers one live store under a stable product-selected name.</summary>
    public void RegisterStore(string name, EntityStore store)
    {
        ValidateStoreName(name);
        ValidateStore(store);
        if (!_stores.TryAdd(name, store))
        {
            throw new InvalidOperationException($"A debug store named '{name}' is already registered.");
        }
    }

    /// <summary>
    /// Replaces an existing registration after a product atomically installs a
    /// new authoritative store generation. The module does not dispose either
    /// store; their lifetimes remain product-owned.
    /// </summary>
    public void ReplaceStore(string name, EntityStore store)
    {
        ValidateStoreName(name);
        ValidateStore(store);
        if (!_stores.ContainsKey(name))
        {
            throw new InvalidOperationException($"A debug store named '{name}' is not registered.");
        }
        _stores[name] = store;
    }

    /// <summary>Removes an existing store registration without disposing the product-owned store.</summary>
    public void UnregisterStore(string name)
    {
        ValidateStoreName(name);
        if (!_stores.Remove(name))
        {
            throw new InvalidOperationException($"A debug store named '{name}' is not registered.");
        }
    }

    public void RegisterProjection<T>(ComponentType<T> componentType, EntityStoreDebugProjection<T> projection)
        where T : notnull
    {
        ArgumentNullException.ThrowIfNull(componentType);
        ArgumentNullException.ThrowIfNull(projection);
        if (!_projections.TryAdd(componentType.Key, new Projection<T>(componentType, projection)))
        {
            throw new InvalidOperationException($"A debug projection for component {componentType.Key.Value} is already registered.");
        }
    }

    /// <summary>Opts one component type into inspection without assigning a numeric descriptor.
    /// The formatter reads the currently attached value on every command, including in a replacement store.</summary>
    public void RegisterProjection<T>(EntityStoreDebugProjection<T> projection) where T : notnull
    {
        ArgumentNullException.ThrowIfNull(projection);
        if (!_typedProjections.TryAdd(typeof(T), new Projection<T>(null, projection)))
            throw new InvalidOperationException($"A debug projection for {typeof(T).Name} is already registered.");
    }

    [DebugCommand("entity.stores", Description = "Lists product-registered EntityStore names.")]
    public DebugCommandResult ListStores()
    {
        var output = new DebugOutput();
        output.Append($"stores={_stores.Count}");
        foreach (string name in _stores.Keys)
        {
            output.Append($"name={name}");
        }
        return DebugCommandResult.Success(output.ToString());
    }

    [DebugCommand("entity.summary", Description = "Shows one registered EntityStore summary.")]
    public DebugCommandResult Summary(string store)
        => WithStore(store, (name, snapshot) =>
        {
            int active = snapshot.Entities.Count(entity => entity.Lifecycle == EntityLifecycle.Active);
            int disabled = snapshot.Entities.Count(entity => entity.Lifecycle == EntityLifecycle.Disabled);
            int tombstoned = snapshot.Entities.Count(entity => entity.Lifecycle == EntityLifecycle.Tombstoned);
            return DebugCommandResult.Success($"store={name};revision={snapshot.Revision};next={snapshot.NextEntityValue};entities={snapshot.Entities.Count};active={active};disabled={disabled};tombstoned={tombstoned}");
        });

    [DebugCommand("entity.list", Description = "Lists a bounded, lifecycle-selected EntityStore page after an entity-id cursor.")]
    public DebugCommandResult ListEntities(string store, EntityStoreDebugSelector selector, ulong cursor, int limit)
    {
        if (!Enum.IsDefined(selector))
        {
            return Invalid("The entity selector is not supported.");
        }
        return WithPage(store, limit, (name, snapshot, pageLimit) =>
        {
            EntityStoreDebugEntitySnapshot[] rows = snapshot.Entities
                .Where(entity => entity.Entity.Value > cursor && Matches(selector, entity.Lifecycle))
                .Take(pageLimit)
                .ToArray();
            var output = new DebugOutput();
            output.Append($"store={name}");
            output.Append($"selector={selector}");
            output.Append($"cursor={cursor}");
            output.Append($"count={rows.Length}");
            foreach (EntityStoreDebugEntitySnapshot entity in rows)
            {
                output.Append(EntitySummary(entity));
            }
            return DebugCommandResult.Success(output.ToString());
        });
    }

    [DebugCommand("entity.get", Description = "Shows one EntityStore entity, containment, component keys, and revisions.")]
    public DebugCommandResult GetEntity(string store, ulong entity)
        => WithStore(store, (name, snapshot) =>
        {
            EntityStoreDebugEntitySnapshot? row = snapshot.Entities.FirstOrDefault(candidate => candidate.Entity.Value == entity);
            if (row is null)
            {
                return Invalid($"Unknown entity {entity} in store '{name}'.");
            }
            var output = new DebugOutput();
            output.Append($"store={name}");
            output.Append(EntitySummary(row));
            output.Append($"containedIn={row.Container?.Value.ToString(CultureInfo.InvariantCulture) ?? "none"}");
            output.Append($"children={string.Join(',', row.Children.Select(child => child.Value.ToString(CultureInfo.InvariantCulture)))}");
            foreach (EntityStoreDebugComponentPresence component in row.Components)
            {
                output.Append($"component={component.Key.Value}:revision={component.Revision}:type={component.Descriptor.Family.Name}");
            }
            return DebugCommandResult.Success(output.ToString());
        });

    [DebugCommand("entity.children", Description = "Lists a bounded page of direct contained EntityStore children after an entity-id cursor.")]
    public DebugCommandResult ListChildren(string store, ulong entity, ulong cursor, int limit)
        => WithPage(store, limit, (name, snapshot, pageLimit) =>
        {
            EntityStoreDebugEntitySnapshot? row = snapshot.Entities.FirstOrDefault(candidate => candidate.Entity.Value == entity);
            if (row is null)
            {
                return Invalid($"Unknown entity {entity} in store '{name}'.");
            }
            EntityId[] children = row.Children.Where(child => child.Value > cursor).Take(pageLimit).ToArray();
            var output = new DebugOutput();
            output.Append($"store={name}");
            output.Append($"entity={entity}");
            output.Append($"cursor={cursor}");
            output.Append($"count={children.Length}");
            foreach (EntityId child in children)
            {
                output.Append($"child={child.Value}");
            }
            return DebugCommandResult.Success(output.ToString());
        });

    [DebugCommand("entity.component", Description = "Shows one explicit typed EntityStore component projection.")]
    public DebugCommandResult GetComponent(string store, ulong entity, uint componentKey)
        => WithStore(store, (name, snapshot) =>
        {
            EntityStoreDebugEntitySnapshot? row = snapshot.Entities.FirstOrDefault(candidate => candidate.Entity.Value == entity);
            if (row is null)
            {
                return Invalid($"Unknown entity {entity} in store '{name}'.");
            }
            if (!snapshot.ComponentFamilies.Any(family => family.Key.Value == componentKey))
            {
                return Invalid($"Unknown component {componentKey} in store '{name}'.");
            }
            EntityStoreDebugComponentPresence? component = row.Components.FirstOrDefault(candidate => candidate.Key.Value == componentKey);
            if (component is null)
            {
                return DebugCommandResult.Success($"store={name};entity={entity};component={componentKey};present=false");
            }
            if (!_projections.TryGetValue(component.Key, out Projection? projection)
                || !ReferenceEquals(component.Descriptor, projection.Descriptor))
            {
                _typedProjections.TryGetValue(component.Descriptor.Family, out projection);
            }
            return projection is null
                ? DebugCommandResult.Success($"store={name};entity={entity};component={componentKey};present=true;revision={component.Revision};value=projection-unavailable")
                : projection.Project(_stores[name], row.Entity, name, component);
        });

    private DebugCommandResult WithStore(string name, Func<string, EntityStoreDebugSnapshot, DebugCommandResult> query)
    {
        if (!_stores.TryGetValue(name, out EntityStore? store))
        {
            return Invalid($"Unknown debug store '{name}'.");
        }
        try
        {
            return query(name, store.CaptureDebugSnapshot());
        }
        catch (ObjectDisposedException)
        {
            return DebugCommandResult.Failure(DebugCommandStatus.Failed, $"Debug store '{name}' has been disposed.");
        }
    }

    private DebugCommandResult WithPage(string store, int limit, Func<string, EntityStoreDebugSnapshot, int, DebugCommandResult> query)
    {
        if (limit is <= 0 or > MaximumPageSize)
        {
            return Invalid($"Page limit must be between 1 and {MaximumPageSize}.");
        }
        return WithStore(store, (name, snapshot) => query(name, snapshot, limit));
    }

    private static void ValidateStoreName(string name)
    {
        if (string.IsNullOrWhiteSpace(name) || name.Length > MaximumStoreNameLength || name.Any(char.IsWhiteSpace))
        {
            throw new ArgumentException($"A store name must be a non-empty, whitespace-free token of at most {MaximumStoreNameLength} characters.", nameof(name));
        }
    }

    private static void ValidateStore(EntityStore store)
    {
        ArgumentNullException.ThrowIfNull(store);
        store.ValidateDebugRegistration();
    }

    private static bool Matches(EntityStoreDebugSelector selector, EntityLifecycle lifecycle)
        => selector switch
        {
            EntityStoreDebugSelector.All => true,
            EntityStoreDebugSelector.Active => lifecycle == EntityLifecycle.Active,
            EntityStoreDebugSelector.Disabled => lifecycle == EntityLifecycle.Disabled,
            EntityStoreDebugSelector.Tombstoned => lifecycle == EntityLifecycle.Tombstoned,
            _ => false,
        };

    private static string EntitySummary(EntityStoreDebugEntitySnapshot entity)
        => $"entity={entity.Entity.Value}:lifecycle={entity.Lifecycle}:revision={entity.Revision}";

    private static DebugCommandResult Invalid(string message)
        => DebugCommandResult.Failure(DebugCommandStatus.InvalidArguments, BoundedMessage(message));

    private static string BoundedMessage(string message)
        => message.Length <= MaximumResultLength
            ? message
            : string.Concat(message.AsSpan(0, MaximumResultLength - 3), "...");

    private abstract class Projection(ComponentType? descriptor)
    {
        internal ComponentType? Descriptor { get; } = descriptor;
        internal abstract DebugCommandResult Project(EntityStore store, EntityId entity, string storeName, EntityStoreDebugComponentPresence component);
    }

    private sealed class Projection<T>(ComponentType<T>? descriptor, EntityStoreDebugProjection<T> formatter) : Projection(descriptor)
        where T : notnull
    {
        internal override DebugCommandResult Project(EntityStore store, EntityId entity, string storeName, EntityStoreDebugComponentPresence component)
        {
            try
            {
                T? value;
                bool present = descriptor is null
                    ? store.TryGet<T>(entity, out value)
                    : store.TryGet(entity, descriptor, out value);
                if (!present)
                {
                    return DebugCommandResult.Success($"store={storeName};entity={entity.Value};component={component.Key.Value};present=false");
                }
                var output = new DebugOutput();
                output.Append($"store={storeName}");
                output.Append($"entity={entity.Value}");
                output.Append($"component={component.Key.Value}");
                output.Append("present=true");
                output.Append($"revision={component.Revision}");
                output.Append($"value={formatter(in value!)}");
                return DebugCommandResult.Success(output.ToString());
            }
            catch (Exception)
            {
                return DebugCommandResult.Failure(DebugCommandStatus.Failed, $"Debug projection for component {component.Key.Value} failed.");
            }
        }
    }

    private sealed class DebugOutput
    {
        private readonly StringBuilder _builder = new();
        private bool _truncated;

        internal void Append(string value)
        {
            if (_truncated)
            {
                return;
            }
            int available = MaximumResultLength - _builder.Length;
            if (_builder.Length != 0)
            {
                if (available <= 1)
                {
                    _truncated = true;
                    return;
                }
                _builder.Append(';');
                available--;
            }
            if (value.Length <= available)
            {
                _builder.Append(value);
                return;
            }
            const string suffix = "...";
            _builder.Append(value.AsSpan(0, Math.Max(0, available - suffix.Length)));
            _builder.Append(suffix.AsSpan(0, Math.Min(suffix.Length, available)));
            _truncated = true;
        }

        public override string ToString() => _builder.ToString();
    }
}
