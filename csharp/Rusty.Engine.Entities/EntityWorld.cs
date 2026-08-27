namespace Rusty.Engine.Entities;

/// <summary>
/// Engine-maintained managed storage for product-owned typed entity facts.
///
/// It is intentionally independent of the host update pipeline and is not a projection of Rust
/// entity-state. Rust mechanisms remain reachable through their generated services; this world
/// avoids a native crossing for every ordinary product component read or write.
/// </summary>
public sealed class EntityWorld : IDisposable
{
    private const int MaximumDiagnosticSample = 64;
    private WorldState _state;
    private bool _isDisposed;
    private bool _staging;

    public EntityWorld(IEnumerable<ComponentType>? componentTypes = null)
    {
        _state = new WorldState();
        if (componentTypes is null)
        {
            return;
        }

        foreach (ComponentType componentType in componentTypes)
        {
            RegisterUntyped(componentType);
        }
    }

    private EntityWorld(WorldState state, bool staging)
    {
        _state = state;
        _staging = staging;
    }

    public ulong Revision
    {
        get
        {
            ThrowIfDisposed();
            return _state.Revision;
        }
    }

    public void Register<T>(ComponentType<T> componentType) where T : struct
    {
        ThrowIfDisposed();
        if (_staging)
        {
            throw new InvalidOperationException("Component registration must complete before a batch is staged.");
        }
        ArgumentNullException.ThrowIfNull(componentType);
        RegisterUntyped(componentType);
    }

    public EntityId Create(EntityLifecycle lifecycle = EntityLifecycle.Active)
    {
        ThrowIfDisposed();
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

    public ComponentRevision GetComponentRevision<T>(EntityId entity, ComponentType<T> componentType) where T : struct
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
            table.Remove(entity);
        }
        RemoveContainmentForDestroy(entity);
        record.Lifecycle = EntityLifecycle.Tombstoned;
        record.Revision++;
        Mutated();
    }

    /// <summary>
    /// Places one live entity in one live container. Reparenting is atomic and a relation that
    /// already has the requested container is an idempotent no-op.
    /// </summary>
    public ContainmentReceipt SetContainment(EntityId child, EntityId container, ulong? expectedRevision = null)
    {
        ThrowIfDisposed();
        EnsureWorldRevision(expectedRevision);
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
        EnsureWorldRevision(expectedRevision);
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

    public bool Has<T>(EntityId entity, ComponentType<T> componentType) where T : struct
    {
        ThrowIfDisposed();
        return _state.Entities.ContainsKey(entity.Value) && GetTable(componentType).Contains(entity);
    }

    public bool TryGet<T>(EntityId entity, ComponentType<T> componentType, out T value) where T : struct
    {
        ThrowIfDisposed();
        if (!_state.Entities.ContainsKey(entity.Value))
        {
            value = default;
            return false;
        }
        return GetTable(componentType).TryGet(entity, out value);
    }

    public T Get<T>(EntityId entity, ComponentType<T> componentType) where T : struct
    {
        ThrowIfDisposed();
        RequireEntity(entity);
        return GetTable(componentType).TryGet(entity, out T value)
            ? value
            : throw new InvalidOperationException($"Entity {entity.Value} does not have component {componentType.Key.Value}.");
    }

    public void Set<T>(EntityId entity, ComponentType<T> componentType, T value, ComponentRevision? expectedRevision = null)
        where T : struct
    {
        ThrowIfDisposed();
        RequireAlive(entity);
        ComponentTable<T> table = GetTable(componentType);
        EnsureComponentRevision(entity, componentType, table, expectedRevision);
        table.Set(entity, value);
        TouchEntity(entity);
        Mutated();
    }

    public bool Remove<T>(EntityId entity, ComponentType<T> componentType, ComponentRevision? expectedRevision = null)
        where T : struct
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
        where T : struct
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
        where TFirst : struct
        where TSecond : struct
    {
        ThrowIfDisposed();
        List<EntityComponents<TFirst, TSecond>> result = [];
        ComponentTable<TSecond> secondTable = GetTable(second);
        foreach ((EntityId entity, TFirst firstValue) in GetTable(first).Values())
        {
            if (_state.Entities.TryGetValue(entity.Value, out EntityRecord? record)
                && (record.Lifecycle == EntityLifecycle.Active || includeDisabled && record.Lifecycle == EntityLifecycle.Disabled)
                && secondTable.TryGet(entity, out TSecond secondValue))
            {
                result.Add(new EntityComponents<TFirst, TSecond>(entity, firstValue, secondValue));
            }
        }
        return result;
    }

    public EntityBatchReceipt Commit(EntityBatch batch, ulong? expectedRevision = null)
    {
        ThrowIfDisposed();
        ArgumentNullException.ThrowIfNull(batch);
        if (expectedRevision is ulong expected && expected != _state.Revision)
        {
            throw new InvalidOperationException($"World revision is stale: expected {expected}, actual {_state.Revision}.");
        }

        ulong revisionBefore = _state.Revision;
        WorldState stagedState = _state.Clone(forSnapshot: false);
        var staged = new EntityWorld(stagedState, staging: true);
        foreach (Action<EntityWorld> mutation in batch.Mutations)
        {
            mutation(staged);
        }

        if (batch.Mutations.Count != 0)
        {
            staged._state.Revision = checked(revisionBefore + 1);
            _state = staged._state;
        }
        return new EntityBatchReceipt(revisionBefore, _state.Revision, batch.Mutations.Count);
    }

    public EntityWorldSnapshot Snapshot()
    {
        ThrowIfDisposed();
        return new EntityWorldSnapshot(_state.Clone(forSnapshot: true));
    }

    public void Restore(EntityWorldSnapshot snapshot, ulong? expectedRevision = null)
    {
        ThrowIfDisposed();
        ArgumentNullException.ThrowIfNull(snapshot);
        if (expectedRevision is ulong expected && expected != _state.Revision)
        {
            throw new InvalidOperationException($"World revision is stale: expected {expected}, actual {_state.Revision}.");
        }
        EnsureSameRegistrations(snapshot.State);
        ulong revisionBefore = _state.Revision;
        WorldState restored = snapshot.State.Clone(forSnapshot: true);
        restored.ValidateComponents();
        restored.ValidateContainment();
        restored.InvalidateRevisions();
        _state = restored;
        if (!_staging)
        {
            _state.Revision = checked(revisionBefore + 1);
        }
    }

    public EntityWorldDiagnostics Diagnostics(int maxEntitySample = MaximumDiagnosticSample)
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
        return new EntityWorldDiagnostics(_state.Revision, _state.NextEntityValue, false, _state.Entities.Count, active, disabled, tombstoned, components);
    }

    public void Dispose()
    {
        if (_staging)
        {
            throw new InvalidOperationException("A batch cannot dispose its staging world.");
        }
        if (_isDisposed)
        {
            return;
        }
        _state.Tables.Clear();
        _state.Entities.Clear();
        _state.Containment.Clear();
        _state.ContainedChildren.Clear();
        _isDisposed = true;
    }

    private void RegisterUntyped(ComponentType componentType)
    {
        ArgumentNullException.ThrowIfNull(componentType);
        if (_state.Tables.ContainsKey(componentType.Key))
        {
            throw new InvalidOperationException($"Component key {componentType.Key.Value} is already registered in this world.");
        }
        _state.Tables.Add(componentType.Key, componentType.CreateTable());
    }

    private ComponentTable<T> GetTable<T>(ComponentType<T> componentType) where T : struct
    {
        ArgumentNullException.ThrowIfNull(componentType);
        if (!_state.Tables.TryGetValue(componentType.Key, out ComponentTable? table))
        {
            throw new InvalidOperationException($"Component key {componentType.Key.Value} is not registered in this world.");
        }
        return table as ComponentTable<T>
            ?? throw new InvalidOperationException($"Component key {componentType.Key.Value} is registered with a different component type.");
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

    private void EnsureWorldRevision(ulong? expectedRevision)
    {
        if (expectedRevision is ulong expected && expected != _state.Revision)
        {
            throw new InvalidOperationException($"World revision is stale: expected {expected}, actual {_state.Revision}.");
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
        where T : struct
    {
        if (expected is ComponentRevision guard
            && (guard.Entity != entity || guard.Component != componentType.Key || guard.Revision != table.RevisionFor(entity)))
        {
            throw new InvalidOperationException($"Component revision is stale for entity {entity.Value}, component {componentType.Key.Value}.");
        }
    }

    private void TouchEntity(EntityId entity) => RequireEntity(entity).Revision++;

    private void Mutated()
    {
        if (!_staging)
        {
            _state.Revision = checked(_state.Revision + 1);
        }
    }

    private void EnsureSameRegistrations(WorldState snapshot)
    {
        if (_state.Tables.Count != snapshot.Tables.Count || _state.Tables.Keys.Except(snapshot.Tables.Keys).Any())
        {
            throw new InvalidOperationException("Snapshot component registrations do not match this world.");
        }
        foreach ((ComponentTypeKey key, ComponentTable table) in _state.Tables)
        {
            if (snapshot.Tables[key].Descriptor != table.Descriptor)
            {
                throw new InvalidOperationException($"Snapshot component descriptor for key {key.Value} does not match this world.");
            }
        }
    }

    internal sealed class WorldState
    {
        internal ulong Revision;
        internal ulong NextEntityValue = 1;
        internal SortedDictionary<ulong, EntityRecord> Entities { get; } = [];
        internal SortedDictionary<ComponentTypeKey, ComponentTable> Tables { get; } = [];
        internal SortedDictionary<ulong, ulong> Containment { get; } = [];
        internal SortedDictionary<ulong, SortedSet<ulong>> ContainedChildren { get; } = [];

        internal WorldState Clone(bool forSnapshot)
        {
            var result = new WorldState { Revision = Revision, NextEntityValue = NextEntityValue };
            foreach ((ulong id, EntityRecord entity) in Entities)
            {
                result.Entities.Add(id, entity.Clone());
            }
            foreach ((ComponentTypeKey key, ComponentTable table) in Tables)
            {
                result.Tables.Add(key, table.Clone(forSnapshot));
            }
            foreach ((ulong child, ulong container) in Containment)
            {
                result.Containment.Add(child, container);
            }
            foreach ((ulong container, SortedSet<ulong> children) in ContainedChildren)
            {
                result.ContainedChildren.Add(container, [.. children]);
            }
            return result;
        }

        internal void InvalidateRevisions()
        {
            foreach (EntityRecord entity in Entities.Values)
            {
                entity.Revision = checked(entity.Revision + 1);
            }
            foreach (ComponentTable table in Tables.Values)
            {
                table.InvalidateRevisions();
            }
        }

        internal void ValidateComponents()
        {
            foreach (ComponentTable table in Tables.Values)
            {
                table.ValidateValues();
            }
        }


        internal void ValidateContainment()
        {
            foreach ((ulong child, ulong container) in Containment)
            {
                if (child == container || !IsAlive(child) || !IsAlive(container)
                    || !ContainedChildren.TryGetValue(container, out SortedSet<ulong>? children)
                    || !children.Contains(child))
                {
                    throw new InvalidOperationException("Snapshot containment is inconsistent.");
                }
                var visited = new HashSet<ulong> { child };
                for (ulong current = container; Containment.TryGetValue(current, out ulong next); current = next)
                {
                    if (!visited.Add(current) || next == child)
                    {
                        throw new InvalidOperationException("Snapshot containment contains a cycle.");
                    }
                }
            }
            foreach ((ulong container, SortedSet<ulong> children) in ContainedChildren)
            {
                if (!IsAlive(container) || children.Any(child => !IsAlive(child)
                    || !Containment.TryGetValue(child, out ulong owner) || owner != container))
                {
                    throw new InvalidOperationException("Snapshot containment reverse index is inconsistent.");
                }
            }
        }

        private bool IsAlive(ulong entity) => Entities.TryGetValue(entity, out EntityRecord? record)
            && record.Lifecycle != EntityLifecycle.Tombstoned;
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
        internal ComponentType Descriptor { get; }
        internal abstract ComponentTable Clone(bool forSnapshot);
        internal abstract bool Remove(EntityId entity);
        internal abstract void InvalidateRevisions();
        internal abstract void ValidateValues();
        internal abstract ComponentTypeDiagnostics Diagnostics(int maxEntitySample);
    }

    internal sealed class ComponentTable<T> : ComponentTable where T : struct
    {
        private readonly SortedDictionary<ulong, T> _values = [];
        private readonly SortedDictionary<ulong, ulong> _revisions = [];

        public ComponentTable(ComponentType<T> descriptor) : base(descriptor) { }

        private ComponentTable(ComponentTable<T> source, bool forSnapshot) : base(source.TypedDescriptor)
        {
            foreach ((ulong entity, T value) in source._values)
            {
                T copied = forSnapshot && TypedDescriptor.SnapshotCodec is ComponentSnapshotCodec<T> codec ? codec(in value) : value;
                TypedDescriptor.Validate(in copied);
                _values.Add(entity, copied);
            }
            foreach ((ulong entity, ulong revision) in source._revisions)
            {
                _revisions.Add(entity, revision);
            }
        }

        private ComponentType<T> TypedDescriptor => (ComponentType<T>)Descriptor;

        internal override ComponentTable Clone(bool forSnapshot) => new ComponentTable<T>(this, forSnapshot);

        internal bool Contains(EntityId entity) => _values.ContainsKey(entity.Value);

        internal bool TryGet(EntityId entity, out T value) => _values.TryGetValue(entity.Value, out value);

        internal void Set(EntityId entity, T value)
        {
            TypedDescriptor.Validate(in value);
            _values[entity.Value] = value;
            BumpRevision(entity);
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

        internal ulong RevisionFor(EntityId entity) => _revisions.GetValueOrDefault(entity.Value);

        internal override void InvalidateRevisions()
        {
            foreach (ulong entity in _revisions.Keys.ToArray())
            {
                _revisions[entity] = checked(_revisions[entity] + 1);
            }
        }

        internal override void ValidateValues()
        {
            foreach (T value in _values.Values)
            {
                TypedDescriptor.Validate(in value);
            }
        }

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
