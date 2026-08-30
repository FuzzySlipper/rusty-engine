using System.Globalization;
using System.Text;
using Rusty.Engine.Entities;

namespace Rusty.Engine.Debugging;

/// <summary>Restricts the entity rows returned by <see cref="EntityWorldDebugModule"/>.</summary>
public enum EntityWorldDebugSelector
{
    All,
    Active,
    Disabled,
    Tombstoned,
}

/// <summary>Renders one explicitly opted-in component value for live debug output.</summary>
public delegate string EntityWorldDebugProjection<T>(in T value) where T : struct;

/// <summary>
/// Read-only live inspection for explicitly product-registered <see cref="EntityWorld"/>
/// instances. It is a normal generated debug-command module: products create one instance,
/// register their worlds and typed projections, then register that instance with their catalog.
/// </summary>
public sealed class EntityWorldDebugModule : IDebugCommandModule
{
    public const int MaximumPageSize = 64;
    public const int MaximumResultLength = 4096;
    public const int MaximumWorldNameLength = 64;

    private readonly SortedDictionary<string, EntityWorld> _worlds = new(StringComparer.Ordinal);
    private readonly SortedDictionary<ComponentTypeKey, Projection> _projections = [];

    public void RegisterWorld(string name, EntityWorld world)
    {
        if (string.IsNullOrWhiteSpace(name) || name.Length > MaximumWorldNameLength || name.Any(char.IsWhiteSpace))
        {
            throw new ArgumentException($"A world name must be a non-empty, whitespace-free token of at most {MaximumWorldNameLength} characters.", nameof(name));
        }
        ArgumentNullException.ThrowIfNull(world);
        if (!_worlds.TryAdd(name, world))
        {
            throw new InvalidOperationException($"A debug world named '{name}' is already registered.");
        }
    }

    public void RegisterProjection<T>(ComponentType<T> componentType, EntityWorldDebugProjection<T> projection)
        where T : struct
    {
        ArgumentNullException.ThrowIfNull(componentType);
        ArgumentNullException.ThrowIfNull(projection);
        if (!_projections.TryAdd(componentType.Key, new Projection<T>(componentType, projection)))
        {
            throw new InvalidOperationException($"A debug projection for component {componentType.Key.Value} is already registered.");
        }
    }

    [DebugCommand("entity.worlds", Description = "Lists product-registered EntityWorld names.")]
    public DebugCommandResult ListWorlds()
    {
        var output = new DebugOutput();
        output.Append($"worlds={_worlds.Count}");
        foreach (string name in _worlds.Keys)
        {
            output.Append($"name={name}");
        }
        return DebugCommandResult.Success(output.ToString());
    }

    [DebugCommand("entity.summary", Description = "Shows one registered EntityWorld summary.")]
    public DebugCommandResult Summary(string world)
        => WithWorld(world, (name, snapshot) =>
        {
            int active = snapshot.Entities.Count(entity => entity.Lifecycle == EntityLifecycle.Active);
            int disabled = snapshot.Entities.Count(entity => entity.Lifecycle == EntityLifecycle.Disabled);
            int tombstoned = snapshot.Entities.Count(entity => entity.Lifecycle == EntityLifecycle.Tombstoned);
            return DebugCommandResult.Success($"world={name};revision={snapshot.Revision};next={snapshot.NextEntityValue};entities={snapshot.Entities.Count};active={active};disabled={disabled};tombstoned={tombstoned}");
        });

    [DebugCommand("entity.list", Description = "Lists a bounded, lifecycle-selected EntityWorld page after an entity-id cursor.")]
    public DebugCommandResult ListEntities(string world, EntityWorldDebugSelector selector, ulong cursor, int limit)
    {
        if (!Enum.IsDefined(selector))
        {
            return Invalid("The entity selector is not supported.");
        }
        return WithPage(world, limit, (name, snapshot, pageLimit) =>
        {
            EntityWorldDebugEntitySnapshot[] rows = snapshot.Entities
                .Where(entity => entity.Entity.Value > cursor && Matches(selector, entity.Lifecycle))
                .Take(pageLimit)
                .ToArray();
            var output = new DebugOutput();
            output.Append($"world={name}");
            output.Append($"selector={selector}");
            output.Append($"cursor={cursor}");
            output.Append($"count={rows.Length}");
            foreach (EntityWorldDebugEntitySnapshot entity in rows)
            {
                output.Append(EntitySummary(entity));
            }
            return DebugCommandResult.Success(output.ToString());
        });
    }

    [DebugCommand("entity.get", Description = "Shows one EntityWorld entity, containment, component keys, and revisions.")]
    public DebugCommandResult GetEntity(string world, ulong entity)
        => WithWorld(world, (name, snapshot) =>
        {
            EntityWorldDebugEntitySnapshot? row = snapshot.Entities.FirstOrDefault(candidate => candidate.Entity.Value == entity);
            if (row is null)
            {
                return Invalid($"Unknown entity {entity} in world '{name}'.");
            }
            var output = new DebugOutput();
            output.Append($"world={name}");
            output.Append(EntitySummary(row));
            output.Append($"containedIn={row.Container?.Value.ToString(CultureInfo.InvariantCulture) ?? "none"}");
            output.Append($"children={string.Join(',', row.Children.Select(child => child.Value.ToString(CultureInfo.InvariantCulture)))}");
            foreach (EntityWorldDebugComponentPresence component in row.Components)
            {
                output.Append($"component={component.Key.Value}:revision={component.Revision}");
            }
            return DebugCommandResult.Success(output.ToString());
        });

    [DebugCommand("entity.children", Description = "Lists a bounded page of direct contained EntityWorld children after an entity-id cursor.")]
    public DebugCommandResult ListChildren(string world, ulong entity, ulong cursor, int limit)
        => WithPage(world, limit, (name, snapshot, pageLimit) =>
        {
            EntityWorldDebugEntitySnapshot? row = snapshot.Entities.FirstOrDefault(candidate => candidate.Entity.Value == entity);
            if (row is null)
            {
                return Invalid($"Unknown entity {entity} in world '{name}'.");
            }
            EntityId[] children = row.Children.Where(child => child.Value > cursor).Take(pageLimit).ToArray();
            var output = new DebugOutput();
            output.Append($"world={name}");
            output.Append($"entity={entity}");
            output.Append($"cursor={cursor}");
            output.Append($"count={children.Length}");
            foreach (EntityId child in children)
            {
                output.Append($"child={child.Value}");
            }
            return DebugCommandResult.Success(output.ToString());
        });

    [DebugCommand("entity.component", Description = "Shows one explicit typed EntityWorld component projection.")]
    public DebugCommandResult GetComponent(string world, ulong entity, uint componentKey)
        => WithWorld(world, (name, snapshot) =>
        {
            EntityWorldDebugEntitySnapshot? row = snapshot.Entities.FirstOrDefault(candidate => candidate.Entity.Value == entity);
            if (row is null)
            {
                return Invalid($"Unknown entity {entity} in world '{name}'.");
            }
            if (!snapshot.ComponentFamilies.Any(family => family.Key.Value == componentKey))
            {
                return Invalid($"Unknown component {componentKey} in world '{name}'.");
            }
            EntityWorldDebugComponentPresence? component = row.Components.FirstOrDefault(candidate => candidate.Key.Value == componentKey);
            if (component is null)
            {
                return DebugCommandResult.Success($"world={name};entity={entity};component={componentKey};present=false");
            }
            if (!_projections.TryGetValue(component.Key, out Projection? projection)
                || !ReferenceEquals(component.Descriptor, projection.Descriptor))
            {
                return DebugCommandResult.Success($"world={name};entity={entity};component={componentKey};present=true;revision={component.Revision};value=projection-unavailable");
            }
            return projection.Project(_worlds[name], row.Entity, name, component);
        });

    private DebugCommandResult WithWorld(string name, Func<string, EntityWorldDebugSnapshot, DebugCommandResult> query)
    {
        if (!_worlds.TryGetValue(name, out EntityWorld? world))
        {
            return Invalid($"Unknown debug world '{name}'.");
        }
        try
        {
            return query(name, world.CaptureDebugSnapshot());
        }
        catch (ObjectDisposedException)
        {
            return DebugCommandResult.Failure(DebugCommandStatus.Failed, $"Debug world '{name}' has been disposed.");
        }
    }

    private DebugCommandResult WithPage(string world, int limit, Func<string, EntityWorldDebugSnapshot, int, DebugCommandResult> query)
    {
        if (limit is <= 0 or > MaximumPageSize)
        {
            return Invalid($"Page limit must be between 1 and {MaximumPageSize}.");
        }
        return WithWorld(world, (name, snapshot) => query(name, snapshot, limit));
    }

    private static bool Matches(EntityWorldDebugSelector selector, EntityLifecycle lifecycle)
        => selector switch
        {
            EntityWorldDebugSelector.All => true,
            EntityWorldDebugSelector.Active => lifecycle == EntityLifecycle.Active,
            EntityWorldDebugSelector.Disabled => lifecycle == EntityLifecycle.Disabled,
            EntityWorldDebugSelector.Tombstoned => lifecycle == EntityLifecycle.Tombstoned,
            _ => false,
        };

    private static string EntitySummary(EntityWorldDebugEntitySnapshot entity)
        => $"entity={entity.Entity.Value}:lifecycle={entity.Lifecycle}:revision={entity.Revision}";

    private static DebugCommandResult Invalid(string message)
        => DebugCommandResult.Failure(DebugCommandStatus.InvalidArguments, BoundedMessage(message));

    private static string BoundedMessage(string message)
        => message.Length <= MaximumResultLength
            ? message
            : string.Concat(message.AsSpan(0, MaximumResultLength - 3), "...");

    private abstract class Projection(ComponentType descriptor)
    {
        internal ComponentType Descriptor { get; } = descriptor;
        internal abstract DebugCommandResult Project(EntityWorld world, EntityId entity, string worldName, EntityWorldDebugComponentPresence component);
    }

    private sealed class Projection<T>(ComponentType<T> descriptor, EntityWorldDebugProjection<T> formatter) : Projection(descriptor)
        where T : struct
    {
        internal override DebugCommandResult Project(EntityWorld world, EntityId entity, string worldName, EntityWorldDebugComponentPresence component)
        {
            try
            {
                if (!world.TryGet(entity, descriptor, out T value))
                {
                    return DebugCommandResult.Success($"world={worldName};entity={entity.Value};component={component.Key.Value};present=false");
                }
                var output = new DebugOutput();
                output.Append($"world={worldName}");
                output.Append($"entity={entity.Value}");
                output.Append($"component={component.Key.Value}");
                output.Append("present=true");
                output.Append($"revision={component.Revision}");
                output.Append($"value={formatter(in value)}");
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
