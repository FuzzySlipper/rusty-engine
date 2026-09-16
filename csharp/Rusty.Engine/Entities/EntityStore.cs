using System.Diagnostics.CodeAnalysis;

namespace Rusty.Engine.Entities;

/// <summary>
/// Engine-maintained managed storage for product-owned typed entity facts.
///
/// It is intentionally independent of the host update pipeline and is not a projection of Rust
/// entity-state. Rust mechanisms remain reachable through their generated services; this store
/// avoids a native crossing for every ordinary product component read or write.
/// </summary>
public sealed class EntityStore : IDisposable
{
    private const int MaximumDiagnosticSample = 64;
    private StoreState _state;
    private bool _isDisposed;
    private bool _staging;
    private readonly HashSet<Type> _editedFamilies = [];

    public EntityStore(IEnumerable<ComponentType>? componentTypes = null)
    {
        _state = new StoreState();
        if (componentTypes is null)
        {
            return;
        }

        foreach (ComponentType componentType in componentTypes)
        {
            RegisterUntyped(componentType);
        }
    }

    private EntityStore(StoreState state, bool staging)
    {
        _state = state;
        _staging = staging;
    }

    /// <summary>Explicit structural/replacement version; does not track fields inside attached objects.</summary>
    public ulong Revision
    {
        get
        {
            ThrowIfDisposed();
            return _state.Revision;
        }
    }

    /// <summary>The next identity cursor admitted by this store.</summary>
    public ulong NextEntityValue
    {
        get
        {
            ThrowIfDisposed();
            return _state.NextEntityValue;
        }
    }

    public void Register<T>(ComponentType<T> componentType) where T : notnull
    {
        ThrowIfDisposed();
        if (_staging)
        {
            throw new InvalidOperationException("Component registration must complete before a batch is staged.");
        }
        ArgumentNullException.ThrowIfNull(componentType);
        RegisterUntyped(componentType);
        Mutated();
    }

    public EntityId Create(EntityLifecycle lifecycle = EntityLifecycle.Active)
    {
        ThrowIfDisposed();
        EntityLifecycleValidation.EnsureDefined(lifecycle, nameof(lifecycle));
        if (lifecycle == EntityLifecycle.Tombstoned)
        {
            throw new ArgumentOutOfRangeException(nameof(lifecycle), "New entities must be alive.");
        }
        if (_state.NextEntityValue == ulong.MaxValue)
        {
            throw new InvalidOperationException("Entity identity space is exhausted.");
        }

        EntityId entity = new(_state.NextEntityValue++);
        _state.Entities.Add(entity.Value, new EntityRecord(lifecycle, 1));
        Mutated();
        return entity;
    }

    public EntityRevision GetEntityRevision(EntityId entity)
    {
        ThrowIfDisposed();
        return new EntityRevision(entity, RequireEntity(entity).Revision);
    }

    public ComponentRevision GetComponentRevision<T>(EntityId entity, ComponentType<T> componentType) where T : notnull
    {
        ThrowIfDisposed();
        RequireEntity(entity);
        return new ComponentRevision(entity, componentType.Key, GetTable(componentType).RevisionFor(entity));
    }

    public EntityLifecycle GetLifecycle(EntityId entity)
    {
        ThrowIfDisposed();
        return RequireEntity(entity).Lifecycle;
    }

    public bool IsAlive(EntityId entity)
    {
        ThrowIfDisposed();
        return _state.Entities.TryGetValue(entity.Value, out EntityRecord? record)
            && record.Lifecycle != EntityLifecycle.Tombstoned;
    }

    public void SetLifecycle(EntityId entity, EntityLifecycle lifecycle, EntityRevision? expectedRevision = null)
    {
        ThrowIfDisposed();
        EntityLifecycleValidation.EnsureDefined(lifecycle, nameof(lifecycle));
        EntityRecord record = RequireEntity(entity);
        EnsureEntityRevision(entity, record, expectedRevision);
        if (record.Lifecycle == EntityLifecycle.Tombstoned)
        {
            throw new InvalidOperationException($"Entity {entity.Value} has been tombstoned.");
        }
        if (lifecycle == EntityLifecycle.Tombstoned)
        {
            Destroy(entity, expectedRevision);
            return;
        }
        if (record.Lifecycle == lifecycle)
        {
            return;
        }

        record.Lifecycle = lifecycle;
        record.Revision++;
        Mutated();
    }

    public void Destroy(EntityId entity, EntityRevision? expectedRevision = null)
    {
        ThrowIfDisposed();
        EntityRecord record = RequireEntity(entity);
        EnsureEntityRevision(entity, record, expectedRevision);
        if (record.Lifecycle == EntityLifecycle.Tombstoned)
        {
            throw new InvalidOperationException($"Entity {entity.Value} has already been tombstoned.");
        }

        foreach (ComponentTable table in _state.Tables.Values)
        {
            table.Forget(entity);
        }
        RemoveContainmentForDestroy(entity);
        _state.Entities.Remove(entity.Value);
        Mutated();
    }

    /// <summary>
    /// Places one live entity in one live container. Reparenting is atomic and a relation that
    /// already has the requested container is an idempotent no-op.
    /// </summary>
    public ContainmentReceipt SetContainment(EntityId child, EntityId container, ulong? expectedRevision = null)
    {
        ThrowIfDisposed();
        EnsureStoreRevision(expectedRevision);
        RequireAlive(child);
        RequireAlive(container);
        if (child == container)
        {
            throw new InvalidOperationException($"Entity {child.Value} cannot contain itself.");
        }
        for (EntityId ancestor = container; _state.Containment.TryGetValue(ancestor.Value, out ulong next); ancestor = new EntityId(next))
        {
            if (next == child.Value)
            {
                throw new InvalidOperationException($"Containing entity {child.Value} in {container.Value} would create a cycle.");
            }
        }

        ulong revisionBefore = _state.Revision;
        if (_state.Containment.TryGetValue(child.Value, out ulong existing) && existing == container.Value)
        {
            return new ContainmentReceipt(revisionBefore, revisionBefore, child, container, false);
        }

        if (existing != 0)
        {
            RemoveReverse(existing, child.Value);
            TouchEntity(new EntityId(existing));
        }
        _state.Containment[child.Value] = container.Value;
        GetContainedChildren(container.Value).Add(child.Value);
        TouchEntity(child);
        TouchEntity(container);
        Mutated();
        return new ContainmentReceipt(revisionBefore, _state.Revision, child, container, true);
    }

    /// <summary>Clears one live entity's container, if present.</summary>
    public ContainmentReceipt ClearContainment(EntityId child, ulong? expectedRevision = null)
    {
        ThrowIfDisposed();
        EnsureStoreRevision(expectedRevision);
        RequireAlive(child);
        ulong revisionBefore = _state.Revision;
        if (!_state.Containment.Remove(child.Value, out ulong container))
        {
            return new ContainmentReceipt(revisionBefore, revisionBefore, child, null, false);
        }
        RemoveReverse(container, child.Value);
        TouchEntity(child);
        TouchEntity(new EntityId(container));
        Mutated();
        return new ContainmentReceipt(revisionBefore, _state.Revision, child, null, true);
    }

    public bool TryGetContainedIn(EntityId child, out EntityId container)
    {
        ThrowIfDisposed();
        RequireEntity(child);
        if (_state.Containment.TryGetValue(child.Value, out ulong value))
        {
            container = new EntityId(value);
            return true;
        }
        container = default;
        return false;
    }

    /// <summary>Returns direct children in stable entity-id order.</summary>
    public IReadOnlyList<EntityId> ContainedEntities(EntityId container)
    {
        ThrowIfDisposed();
        RequireEntity(container);
        return _state.ContainedChildren.TryGetValue(container.Value, out SortedSet<ulong>? children)
            ? children.Select(value => new EntityId(value)).ToArray()
            : [];
    }

    /// <summary>Attaches one instance/value under the explicit generic family T.</summary>
    public void Add<T>(EntityId entity, T value) where T : notnull
    {
        ThrowIfDisposed();
        RequireAlive(entity);
        ArgumentNullException.ThrowIfNull(value);
        ComponentTable<T> table = GetOrCreateTable<T>();
        if (table.Contains(entity))
        {
            throw new InvalidOperationException($"Entity {entity.Value} already has component {typeof(T).Name}.");
        }
        Set(entity, (ComponentType<T>)table.Descriptor, value);
    }

    /// <summary>Replaces an attached value. Replacing a class with the same instance is a no-op.</summary>
    public void Replace<T>(EntityId entity, T value) where T : notnull
    {
        ThrowIfDisposed();
        RequireAlive(entity);
        ArgumentNullException.ThrowIfNull(value);
        ComponentTable<T>? table = FindTable<T>();
        if (table is null || !table.Contains(entity))
        {
            throw new InvalidOperationException($"Entity {entity.Value} does not have component {typeof(T).Name}.");
        }
        Set(entity, (ComponentType<T>)table.Descriptor, value);
    }

    /// <summary>Explicit value-fact insertion/replacement; class callers use Add or Replace.</summary>
    public void Set<T>(EntityId entity, T value) where T : struct
    {
        ThrowIfDisposed();
        RequireAlive(entity);
        ComponentTable<T> table = GetOrCreateTable<T>();
        Set(entity, (ComponentType<T>)table.Descriptor, value);
    }

    public bool Has<T>(EntityId entity) where T : notnull
    {
        ThrowIfDisposed();
        return _state.Entities.ContainsKey(entity.Value) && FindTable<T>()?.Contains(entity) == true;
    }

    public bool TryGet<T>(EntityId entity, [NotNullWhen(true)] out T? value) where T : notnull
    {
        ThrowIfDisposed();
        if (_state.Entities.ContainsKey(entity.Value) && FindTable<T>() is ComponentTable<T> table)
        {
            return table.TryGet(entity, out value);
        }
        value = default;
        return false;
    }

    /// <summary>Returns the attached class instance, or an ordinary C# copy of a value component.</summary>
    public T Get<T>(EntityId entity) where T : notnull
    {
        ThrowIfDisposed();
        RequireAlive(entity);
        return TryGet<T>(entity, out T? value) ? value
            : throw new InvalidOperationException($"Entity {entity.Value} does not have component {typeof(T).Name}.");
    }

    public bool Remove<T>(EntityId entity) where T : notnull
    {
        ThrowIfDisposed();
        if (!_state.Entities.ContainsKey(entity.Value) || FindTable<T>() is not ComponentTable<T> table)
        {
            return false;
        }
        return Remove(entity, (ComponentType<T>)table.Descriptor);
    }

    /// <summary>Captures ordered membership now; returned class objects remain live references.</summary>
    public IReadOnlyList<EntityComponent<T>> Query<T>(bool includeDisabled = false) where T : notnull
    {
        ThrowIfDisposed();
        return FindTable<T>() is ComponentTable<T> table
            ? Query((ComponentType<T>)table.Descriptor, includeDisabled) : [];
    }

    public IReadOnlyList<EntityComponents<TFirst, TSecond>> Query<TFirst, TSecond>(bool includeDisabled = false)
        where TFirst : notnull where TSecond : notnull
    {
        ThrowIfDisposed();
        return FindTable<TFirst>() is ComponentTable<TFirst> first && FindTable<TSecond>() is ComponentTable<TSecond> second
            ? Query((ComponentType<TFirst>)first.Descriptor, (ComponentType<TSecond>)second.Descriptor, includeDisabled) : [];
    }

    public bool Has<T>(EntityId entity, ComponentType<T> componentType) where T : notnull
    {
        ThrowIfDisposed();
        return _state.Entities.ContainsKey(entity.Value) && GetTable(componentType).Contains(entity);
    }

    public bool TryGet<T>(EntityId entity, ComponentType<T> componentType, [MaybeNullWhen(false)] out T value) where T : notnull
    {
        ThrowIfDisposed();
        if (!_state.Entities.ContainsKey(entity.Value))
        {
            value = default!;
            return false;
        }
        return GetTable(componentType).TryGet(entity, out value);
    }

    public T Get<T>(EntityId entity, ComponentType<T> componentType) where T : notnull
    {
        ThrowIfDisposed();
        RequireEntity(entity);
        return GetTable(componentType).TryGet(entity, out T? value)
            ? value
            : throw new InvalidOperationException($"Entity {entity.Value} does not have component {componentType.Key.Value}.");
    }

    public void Set<T>(EntityId entity, ComponentType<T> componentType, T value, ComponentRevision? expectedRevision = null)
        where T : notnull
    {
        ThrowIfDisposed();
        RequireAlive(entity);
        ComponentTable<T> table = GetTable(componentType);
        EnsureComponentRevision(entity, componentType, table, expectedRevision);
        table = WritableTable(table);
        if (!table.Set(entity, value))
        {
            return;
        }
        TouchEntity(entity);
        Mutated();
    }

    public bool Remove<T>(EntityId entity, ComponentType<T> componentType, ComponentRevision? expectedRevision = null)
        where T : notnull
    {
        ThrowIfDisposed();
        RequireAlive(entity);
        ComponentTable<T> table = GetTable(componentType);
        EnsureComponentRevision(entity, componentType, table, expectedRevision);
        if (!table.Remove(entity))
        {
            return false;
        }
        TouchEntity(entity);
        Mutated();
        return true;
    }

    public IReadOnlyList<EntityComponent<T>> Query<T>(ComponentType<T> componentType, bool includeDisabled = false)
        where T : notnull
    {
        ThrowIfDisposed();
        List<EntityComponent<T>> result = [];
        foreach ((EntityId entity, T value) in GetTable(componentType).Values())
        {
            if (_state.Entities.TryGetValue(entity.Value, out EntityRecord? record)
                && (record.Lifecycle == EntityLifecycle.Active || includeDisabled && record.Lifecycle == EntityLifecycle.Disabled))
            {
                result.Add(new EntityComponent<T>(entity, value));
            }
        }
        return result;
    }

    /// <summary>Deterministically joins two typed component columns without product-side table scans.</summary>
    public IReadOnlyList<EntityComponents<TFirst, TSecond>> Query<TFirst, TSecond>(
        ComponentType<TFirst> first,
        ComponentType<TSecond> second,
        bool includeDisabled = false)
        where TFirst : notnull
        where TSecond : notnull
    {
        ThrowIfDisposed();
        List<EntityComponents<TFirst, TSecond>> result = [];
        ComponentTable<TSecond> secondTable = GetTable(second);
        foreach ((EntityId entity, TFirst firstValue) in GetTable(first).Values())
        {
            if (_state.Entities.TryGetValue(entity.Value, out EntityRecord? record)
                && (record.Lifecycle == EntityLifecycle.Active || includeDisabled && record.Lifecycle == EntityLifecycle.Disabled)
                && secondTable.TryGet(entity, out TSecond? secondValue))
            {
                result.Add(new EntityComponents<TFirst, TSecond>(entity, firstValue, secondValue));
            }
        }
        return result;
    }

    public EntityBatchReceipt Commit(EntityBatch batch, ulong? expectedRevision = null)
    {
        EntityEdit prepared = PrepareBatch(batch, expectedRevision);
        prepared.Publish();
        return prepared.Receipt;
    }

    /// <summary>
    /// Validates and stages one batch without changing the live store. This is
    /// a scoped set of value replacements. Callers coordinating another owner
    /// must manage its commit order; this edit cannot roll that owner back.
    /// </summary>
    public EntityEdit PrepareBatch(EntityBatch batch, ulong? expectedRevision = null)
    {
        ThrowIfDisposed();
        ArgumentNullException.ThrowIfNull(batch);
        if (expectedRevision is ulong expected && expected != _state.Revision)
        {
            throw new InvalidOperationException($"Store revision is stale: expected {expected}, actual {_state.Revision}.");
        }

        ulong revisionBefore = _state.Revision;
        StoreState stagedState = _state.ForkForEdit();
        var staged = new EntityStore(stagedState, staging: true);
        foreach (Action<EntityStore> mutation in batch.Mutations)
        {
            mutation(staged);
        }

        if (batch.Mutations.Count != 0)
        {
            staged._state.Revision = checked(revisionBefore + 1);
        }
        return new EntityEdit(
            this,
            staged._state,
            revisionBefore,
            new EntityBatchReceipt(revisionBefore, staged._state.Revision));
    }

    public EntityStoreDiagnostics Diagnostics(int maxEntitySample = MaximumDiagnosticSample)
    {
        ThrowIfDisposed();
        if (maxEntitySample is < 0 or > MaximumDiagnosticSample)
        {
            throw new ArgumentOutOfRangeException(nameof(maxEntitySample));
        }

        int active = 0;
        int disabled = 0;
        int tombstoned = 0;
        foreach (EntityRecord record in _state.Entities.Values)
        {
            switch (record.Lifecycle)
            {
                case EntityLifecycle.Active: active++; break;
                case EntityLifecycle.Disabled: disabled++; break;
                case EntityLifecycle.Tombstoned: tombstoned++; break;
            }
        }
        IReadOnlyList<ComponentTypeDiagnostics> components = _state.Tables.Values
            .Select(table => table.Diagnostics(maxEntitySample))
            .ToArray();
        return new EntityStoreDiagnostics(_state.Revision, _state.NextEntityValue, _state.Entities.Count, active, disabled, tombstoned, components);
    }

    /// <summary>
    /// Captures the identity, relation, revision, and component-presence facts needed by the
    /// Engine-managed live debug module. This deliberately excludes component values and is
    /// internal so it cannot become a general untyped EntityStore access surface.
    /// </summary>
    internal EntityStoreDebugSnapshot CaptureDebugSnapshot()
    {
        ThrowIfDisposed();
        var entities = new List<EntityStoreDebugEntitySnapshot>(_state.Entities.Count);
        foreach ((ulong value, EntityRecord record) in _state.Entities)
        {
            EntityId entity = new(value);
            var components = new List<EntityStoreDebugComponentPresence>();
            foreach (ComponentTable table in _state.Tables.Values)
            {
                if (table.Contains(entity))
                {
                    components.Add(new EntityStoreDebugComponentPresence(
                        table.Descriptor.Key,
                        table.Descriptor,
                        table.RevisionFor(entity)));
                }
            }

            EntityId? container = _state.Containment.TryGetValue(value, out ulong containerValue)
                ? new EntityId(containerValue)
                : null;
            IReadOnlyList<EntityId> children = _state.ContainedChildren.TryGetValue(value, out SortedSet<ulong>? contained)
                ? contained.Select(child => new EntityId(child)).ToArray()
                : [];
            entities.Add(new EntityStoreDebugEntitySnapshot(entity, record.Lifecycle, record.Revision, container, children, components.ToArray()));
        }

        EntityStoreDebugComponentFamily[] componentFamilies = _state.Tables.Values
            .Select(table => new EntityStoreDebugComponentFamily(table.Descriptor.Key, table.Descriptor))
            .ToArray();
        return new EntityStoreDebugSnapshot(_state.Revision, _state.NextEntityValue, entities.ToArray(), componentFamilies);
    }

    internal void ValidateDebugRegistration() => ThrowIfDisposed();

    public void Dispose()
    {
        if (_staging)
        {
            throw new InvalidOperationException("A batch cannot dispose its staging store.");
        }
        if (_isDisposed)
        {
            return;
        }
        _state.Tables.Clear();
        _state.Families.Clear();
        _state.Entities.Clear();
        _state.Containment.Clear();
        _state.ContainedChildren.Clear();
        _isDisposed = true;
    }

    private void RegisterUntyped(ComponentType componentType)
    {
        ArgumentNullException.ThrowIfNull(componentType);
        _state.Families.TryGetValue(componentType.Family, out ComponentTable? existing);
        if (existing is not null && !existing.Descriptor.IsAutomatic)
        {
            throw new InvalidOperationException($"Component family {componentType.Family.Name} is already registered in this store.");
        }
        _state.Tables.TryGetValue(componentType.Key, out ComponentTable? occupied);
        if (occupied is not null && !occupied.Descriptor.IsAutomatic)
        {
            throw new InvalidOperationException($"Component key {componentType.Key.Value} is already registered in this store.");
        }

        ComponentTypeKey oldKey = existing?.Descriptor.Key ?? default;
        // Validate any attached values before changing indexes or moving an automatic key.
        ComponentTable table = existing ?? componentType.CreateTable();
        existing?.BindDescriptor(componentType);
        if (occupied is not null && !ReferenceEquals(occupied, existing))
        {
            ComponentTypeKey relocatedKey = FindAutomaticKey();
            occupied.RelocateAutomaticDescriptor(relocatedKey);
            _state.Tables.Remove(componentType.Key);
            _state.Tables.Add(relocatedKey, occupied);
        }
        if (existing is not null)
        {
            _state.Tables.Remove(oldKey);
            _state.Tables.Add(componentType.Key, table);
        }
        else
        {
            _state.AddTable(table);
        }
    }

    private ComponentTypeKey FindAutomaticKey()
    {
        uint key = uint.MaxValue;
        while (_state.Tables.ContainsKey(new ComponentTypeKey(key)))
        {
            key = checked(key - 1);
        }
        return new ComponentTypeKey(key);
    }

    private ComponentTable<T> GetTable<T>(ComponentType<T> componentType) where T : notnull
    {
        ArgumentNullException.ThrowIfNull(componentType);
        if (!_state.Tables.TryGetValue(componentType.Key, out ComponentTable? table)
            || !ReferenceEquals(table.Descriptor, componentType))
        {
            throw new InvalidOperationException($"Component key {componentType.Key.Value} is not the registered descriptor in this store.");
        }
        return (ComponentTable<T>)table;
    }

    private ComponentTable<T>? FindTable<T>() where T : notnull
        => _state.Families.TryGetValue(typeof(T), out ComponentTable? table) ? (ComponentTable<T>)table : null;

    private ComponentTable<T> GetOrCreateTable<T>() where T : notnull
    {
        if (FindTable<T>() is ComponentTable<T> existing)
        {
            return existing;
        }
        if (_staging)
        {
            throw new InvalidOperationException("Register new component families before preparing a legacy edit.");
        }
        // These keys support legacy diagnostics only. Ordinary callers never allocate keys.
        var table = new ComponentTable<T>(ComponentType<T>.CreateAutomatic(FindAutomaticKey()));
        _state.AddTable(table);
        return table;
    }

    private EntityRecord RequireEntity(EntityId entity) => _state.Entities.TryGetValue(entity.Value, out EntityRecord? record)
        ? record
        : throw new InvalidOperationException($"Unknown entity {entity.Value}.");

    private void RequireAlive(EntityId entity)
    {
        if (RequireEntity(entity).Lifecycle == EntityLifecycle.Tombstoned)
        {
            throw new InvalidOperationException($"Entity {entity.Value} has been tombstoned.");
        }
    }

    private void EnsureStoreRevision(ulong? expectedRevision)
    {
        if (expectedRevision is ulong expected && expected != _state.Revision)
        {
            throw new InvalidOperationException($"Store revision is stale: expected {expected}, actual {_state.Revision}.");
        }
    }

    private SortedSet<ulong> GetContainedChildren(ulong container)
    {
        if (!_state.ContainedChildren.TryGetValue(container, out SortedSet<ulong>? children))
        {
            children = [];
            _state.ContainedChildren.Add(container, children);
        }
        return children;
    }

    private void RemoveReverse(ulong container, ulong child)
    {
        if (_state.ContainedChildren.TryGetValue(container, out SortedSet<ulong>? children))
        {
            children.Remove(child);
            if (children.Count == 0)
            {
                _state.ContainedChildren.Remove(container);
            }
        }
    }

    private void RemoveContainmentForDestroy(EntityId entity)
    {
        if (_state.Containment.Remove(entity.Value, out ulong container))
        {
            RemoveReverse(container, entity.Value);
            TouchEntity(new EntityId(container));
        }
        if (_state.ContainedChildren.Remove(entity.Value, out SortedSet<ulong>? children))
        {
            foreach (ulong child in children)
            {
                _state.Containment.Remove(child);
                TouchEntity(new EntityId(child));
            }
        }
    }

    private static void EnsureEntityRevision(EntityId entity, EntityRecord record, EntityRevision? expected)
    {
        if (expected is EntityRevision guard && (guard.Entity != entity || guard.Revision != record.Revision))
        {
            throw new InvalidOperationException($"Entity revision is stale for entity {entity.Value}.");
        }
    }

    private static void EnsureComponentRevision<T>(EntityId entity, ComponentType<T> componentType, ComponentTable<T> table, ComponentRevision? expected)
        where T : notnull
    {
        if (expected is ComponentRevision guard
            && (guard.Entity != entity || guard.Component != componentType.Key || guard.Revision != table.RevisionFor(entity)))
        {
            throw new InvalidOperationException($"Component revision is stale for entity {entity.Value}, component {componentType.Key.Value}.");
        }
    }

    private void TouchEntity(EntityId entity)
    {
        EntityRecord record = RequireEntity(entity);
        if (_staging)
        {
            record = record.Clone();
            _state.Entities[entity.Value] = record;
        }
        record.Revision++;
    }

    private ComponentTable<T> WritableTable<T>(ComponentTable<T> table) where T : notnull
    {
        if (!_staging || !_editedFamilies.Add(typeof(T))) return table;
        var copy = (ComponentTable<T>)table.CopySlots();
        _state.Tables[copy.Descriptor.Key] = copy;
        _state.Families[typeof(T)] = copy;
        return copy;
    }

    private void Mutated()
    {
        if (!_staging)
        {
            _state.Revision = checked(_state.Revision + 1);
        }
    }

    internal void PublishPreparedBatch(StoreState state, ulong preparedRevision)
    {
        ThrowIfDisposed();
        EnsureStoreRevision(preparedRevision);
        _state = state;
    }

    internal sealed class StoreState
    {
        internal ulong Revision;
        internal ulong NextEntityValue = 1;
        internal SortedDictionary<ulong, EntityRecord> Entities { get; } = [];
        internal SortedDictionary<ComponentTypeKey, ComponentTable> Tables { get; } = [];
        internal Dictionary<Type, ComponentTable> Families { get; } = [];
        internal SortedDictionary<ulong, ulong> Containment { get; private init; } = [];
        internal SortedDictionary<ulong, SortedSet<ulong>> ContainedChildren { get; private init; } = [];

        internal void AddTable(ComponentTable table)
        {
            Tables.Add(table.Descriptor.Key, table);
            Families.Add(table.Descriptor.Family, table);
        }

        // Only Create and typed Set can run on an edit's private store. They do not
        // change relations. Entity records copy on write and value families on first write;
        // unrelated classes remain the exact same attachments, never graph copies.
        internal StoreState ForkForEdit()
        {
            var result = new StoreState
            {
                Revision = Revision,
                NextEntityValue = NextEntityValue,
                Containment = Containment,
                ContainedChildren = ContainedChildren,
            };
            foreach ((ulong id, EntityRecord entity) in Entities)
                result.Entities.Add(id, entity);
            foreach (ComponentTable table in Tables.Values)
                result.AddTable(table);
            return result;
        }
    }

    internal sealed class EntityRecord(EntityLifecycle lifecycle, ulong revision)
    {
        internal EntityLifecycle Lifecycle { get; set; } = lifecycle;
        internal ulong Revision { get; set; } = revision;
        internal EntityRecord Clone() => new(Lifecycle, Revision);
    }

    internal abstract class ComponentTable
    {
        protected ComponentTable(ComponentType descriptor) => Descriptor = descriptor;
        internal ComponentType Descriptor { get; private protected set; }
        internal abstract ComponentTable CopySlots();
        internal abstract void BindDescriptor(ComponentType descriptor);
        internal abstract void RelocateAutomaticDescriptor(ComponentTypeKey key);
        internal abstract void Forget(EntityId entity);
        internal abstract bool Contains(EntityId entity);
        internal abstract ulong RevisionFor(EntityId entity);
        internal abstract bool Remove(EntityId entity);
        internal abstract ComponentTypeDiagnostics Diagnostics(int maxEntitySample);
    }

    internal sealed class ComponentTable<T> : ComponentTable where T : notnull
    {
        private readonly SortedDictionary<ulong, T> _values = [];
        private readonly SortedDictionary<ulong, ulong> _revisions = [];

        public ComponentTable(ComponentType<T> descriptor) : base(descriptor) { }

        private ComponentTable(ComponentTable<T> source) : base(source.TypedDescriptor)
        {
            foreach ((ulong entity, T value) in source._values)
            {
                _values.Add(entity, value);
            }
            foreach ((ulong entity, ulong revision) in source._revisions)
            {
                _revisions.Add(entity, revision);
            }
        }

        private ComponentType<T> TypedDescriptor => (ComponentType<T>)Descriptor;

        internal override ComponentTable CopySlots() => new ComponentTable<T>(this);

        internal override void RelocateAutomaticDescriptor(ComponentTypeKey key)
            => Descriptor = ComponentType<T>.CreateAutomatic(key);

        internal override void BindDescriptor(ComponentType descriptor)
        {
            var typed = (ComponentType<T>)descriptor;
            foreach (T value in _values.Values)
            {
                typed.Validate(in value);
            }
            Descriptor = descriptor;
        }

        internal override void Forget(EntityId entity)
        {
            _values.Remove(entity.Value);
            _revisions.Remove(entity.Value);
        }

        internal override bool Contains(EntityId entity) => _values.ContainsKey(entity.Value);

        internal bool TryGet(EntityId entity, [MaybeNullWhen(false)] out T value)
            => _values.TryGetValue(entity.Value, out value);

        internal bool Set(EntityId entity, T value)
        {
            ArgumentNullException.ThrowIfNull(value);
            if (!typeof(T).IsValueType && _values.TryGetValue(entity.Value, out T? current)
                && ReferenceEquals(current, value))
            {
                return false;
            }
            TypedDescriptor.Validate(in value);
            _values[entity.Value] = value;
            BumpRevision(entity);
            return true;
        }

        internal override bool Remove(EntityId entity)
        {
            if (!_values.Remove(entity.Value))
            {
                return false;
            }
            BumpRevision(entity);
            return true;
        }

        internal override ulong RevisionFor(EntityId entity) => _revisions.GetValueOrDefault(entity.Value);

        internal IEnumerable<(EntityId Entity, T Value)> Values()
        {
            foreach ((ulong entity, T value) in _values)
            {
                yield return (new EntityId(entity), value);
            }
        }

        internal override ComponentTypeDiagnostics Diagnostics(int maxEntitySample)
            => new(Descriptor.Key, _values.Count, _values.Keys.Take(maxEntitySample).Select(value => new EntityId(value)).ToArray());

        private void BumpRevision(EntityId entity) => _revisions[entity.Value] = checked(RevisionFor(entity) + 1);
    }

    private void ThrowIfDisposed()
    {
        ObjectDisposedException.ThrowIf(_isDisposed, this);
    }
}
