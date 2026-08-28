using Rusty.Engine;

namespace Rusty.Engine.Entities;

/// <summary>
/// The explicit bridge from product-owned <see cref="EntityWorld"/> identities to one admitted
/// Mechanics catalog. It deliberately covers only the typed Mechanics family and does not turn
/// the managed world into a general native ECS mirror.
/// </summary>
public sealed class MechanicsEntityWorld : IDisposable
{
    private readonly EntityWorld _entities;
    private readonly IMechanicsService _mechanics;
    private readonly MechanicsCatalog _catalog;
    private Dictionary<EntityId, Binding> _bindings = [];
    private bool _disposed;

    public MechanicsEntityWorld(EntityWorld entities, IMechanicsService mechanics, MechanicsCatalog catalog)
    {
        _entities = entities ?? throw new ArgumentNullException(nameof(entities));
        _mechanics = mechanics ?? throw new ArgumentNullException(nameof(mechanics));
        _catalog = catalog ?? throw new ArgumentNullException(nameof(catalog));
    }

    /// <summary>
    /// Exports the admitted Mechanics world through its typed Engine service. The returned receipt
    /// is already copied by the generated binding and can therefore be handed to product-owned
    /// persistence code without retaining a native lease.
    /// </summary>
    public MechanicsWorldExportLeaseReceipt Export()
    {
        ThrowIfDisposed();
        RequireNoUncommittedBindings();
        return _mechanics.ExportWorld(_catalog);
    }

    /// <summary>
    /// Prepares one paired managed/native import from product-decoded semantic state. The caller
    /// supplies the generated Mechanics request, but this adapter admits only its own catalog and
    /// only when its canonical entity/lifecycle/containment facts exactly match the managed plan.
    /// Neither side becomes visible until the returned candidate is published.
    /// </summary>
    public MechanicsEntityWorldImportCandidate PrepareImport(
        EntityWorldRestorePlan plan,
        MechanicsWorldImportRequest request,
        ulong? expectedManagedRevision = null)
        => PrepareImport(plan, request, expectedManagedRevision, null);

    /// <summary>
    /// Internal extension point for a named sibling service that must stage facts on the one
    /// detached exact import candidate. The callback runs after exact admission and native
    /// candidate creation, but before either managed preparation or exact receipt validation.
    /// It must not publish or retain the supplied import handle.
    /// </summary>
    internal MechanicsEntityWorldImportCandidate PrepareImport(
        EntityWorldRestorePlan plan,
        MechanicsWorldImportRequest request,
        ulong? expectedManagedRevision,
        Action<MechanicsWorldImport, MechanicsWorldImportRequest>? stageBeforeManagedPrepare)
    {
        ThrowIfDisposed();
        ArgumentNullException.ThrowIfNull(plan);
        RequireNoUncommittedBindings();

        MechanicsWorldImportRequest admitted = AdmitImportRequest(plan, request);
        MechanicsWorldImport native = _mechanics.PrepareWorldImport(admitted);
        try
        {
            stageBeforeManagedPrepare?.Invoke(native, admitted);
            EntityWorldRestoreCandidate managed = _entities.PrepareRestore(plan, expectedManagedRevision);
            MechanicsWorldImportLeaseReceipt receipt = _mechanics.ReadWorldImport(native);
            ValidateImportReceipt(admitted, receipt);
            return new MechanicsEntityWorldImportCandidate(this, native, managed, receipt);
        }
        catch
        {
            native.Dispose();
            throw;
        }
    }

    /// <summary>
    /// Captures a retained, process-local typed checkpoint. Product persistence deliberately owns
    /// any schema, encoding, migration, and durable storage outside this Engine mechanism.
    /// </summary>
    public MechanicsEntityWorldSnapshot Capture()
    {
        ThrowIfDisposed();
        RequireNoUncommittedBindings();
        MechanicsWorldSnapshot native = _mechanics.CaptureWorldSnapshot(_catalog);
        try
        {
            MechanicsWorldSnapshotLeaseReceipt metadata = _mechanics.ReadWorldSnapshot(native);
            Dictionary<EntityId, MechanicsBindingSnapshot> bindings = _bindings.ToDictionary(
                entry => entry.Key,
                entry => new MechanicsBindingSnapshot(
                    entry.Value.Native.Handle.Value,
                    entry.Value.IsCommitted,
                    entry.Value.LifecycleStamp));
            return new MechanicsEntityWorldSnapshot(_entities.Snapshot(), native, metadata.StateRevision, bindings);
        }
        catch
        {
            native.Dispose();
            throw;
        }
    }

    /// <summary>
    /// Restores one paired managed/native in-process checkpoint. Both candidates are prepared and
    /// fully validated before native publication; managed publication is then an assignment only.
    /// Every component and lifecycle guard is explicitly remapped, including absent component
    /// slots, so neither snapshot-era nor pre-restore guards can pass after this method returns.
    /// </summary>
    public MechanicsWorldRestoreLeaseReceipt Restore(
        MechanicsEntityWorldSnapshot snapshot,
        ulong? expectedNativeStateRevision = null,
        ulong? expectedManagedRevision = null)
    {
        ThrowIfDisposed();
        ArgumentNullException.ThrowIfNull(snapshot);
        RequireNoUncommittedBindings();
        RequireMatchingBindingTopology(snapshot);
        ulong managedRevision = expectedManagedRevision ?? _entities.Revision;
        ulong nativeRevision = expectedNativeStateRevision ?? snapshot.NativeStateRevision;

        using MechanicsWorldRestore native = _mechanics.PrepareWorldRestore(
            new MechanicsWorldRestoreRequest(_catalog, snapshot.Mechanics, nativeRevision));
        MechanicsWorldRestoreLeaseReceipt receipt = _mechanics.ReadWorldRestore(native);
        Dictionary<EntityId, ulong> lifecycleStamps = ValidateRestoreReceipt(receipt);
        EntityWorldRestoreCandidate managed = _entities.PrepareRestore(snapshot.Entities, managedRevision);

        // Native publication has no remaining fallible work after prepare/read validation. The
        // managed candidate is already validated and publishes through one field assignment.
        _mechanics.PublishWorldRestore(native);
        managed.Publish();
        foreach ((EntityId entity, Binding binding) in _bindings)
        {
            binding.LifecycleStamp = lifecycleStamps[entity];
        }
        return receipt;
    }

    /// <summary>Creates the native lease for an already-active canonical product entity.</summary>
    public void Bind(EntityId entity, string identity)
    {
        ThrowIfDisposed();
        RequireActive(entity);
        ArgumentException.ThrowIfNullOrWhiteSpace(identity);
        if (_bindings.ContainsKey(entity))
        {
            throw new InvalidOperationException($"Entity {entity.Value} is already bound to this Mechanics world.");
        }

        MechanicsEntity native = _mechanics.BindEntity(new MechanicsEntityBindRequest(_catalog, entity.Value, identity));
        _bindings.Add(entity, new Binding(native));
    }

    /// <summary>
    /// Reacquires an owned native lease for a live canonical entity after a previous adapter
    /// released its lease. The product carries the latest lifecycle stamp from its prior receipt.
    /// </summary>
    public void Rebind(EntityId entity, ulong expectedLifecycleStamp, EntityRevision? expectedRevision = null)
    {
        ThrowIfDisposed();
        if (_bindings.ContainsKey(entity))
        {
            throw new InvalidOperationException($"Entity {entity.Value} is already bound to this Mechanics world.");
        }
        EntityRevision observed = _entities.GetEntityRevision(entity);
        if (expectedRevision is EntityRevision expected && observed != expected)
        {
            throw new InvalidOperationException($"Entity {entity.Value} has a stale lifecycle revision.");
        }
        if (_entities.GetLifecycle(entity) == EntityLifecycle.Tombstoned)
        {
            throw new InvalidOperationException($"Entity {entity.Value} has already been tombstoned.");
        }

        MechanicsEntity native = _mechanics.RebindEntity(new MechanicsEntityRebindRequest(
            _catalog,
            entity.Value,
            MechanicsLifecycleGuard.Exact,
            expectedLifecycleStamp));
        _bindings.Add(entity, new Binding(native)
        {
            IsCommitted = true,
            LifecycleStamp = expectedLifecycleStamp,
        });
    }

    /// <summary>Stages one typed Stats value before <see cref="Commit"/>.</summary>
    public void SetInitialStat(EntityId entity, string stat, long @base)
    {
        ThrowIfDisposed();
        ArgumentException.ThrowIfNullOrWhiteSpace(stat);
        _mechanics.SetInitialStat(new MechanicsInitialStatRequest(RequireUncommitted(entity).Native, stat, @base));
    }

    /// <summary>Stages one typed Tracks value before <see cref="Commit"/>.</summary>
    public void SetInitialTrack(EntityId entity, string track, long current)
    {
        ThrowIfDisposed();
        ArgumentException.ThrowIfNullOrWhiteSpace(track);
        _mechanics.SetInitialTrack(new MechanicsInitialTrackRequest(RequireUncommitted(entity).Native, track, current));
    }

    /// <summary>Stages one typed IntrinsicSources binding before <see cref="Commit"/>.</summary>
    public void BindIntrinsicSource(EntityId entity, string instance, string definition)
    {
        ThrowIfDisposed();
        ArgumentException.ThrowIfNullOrWhiteSpace(instance);
        ArgumentException.ThrowIfNullOrWhiteSpace(definition);
        _mechanics.BindIntrinsicSource(new MechanicsIntrinsicSourceRequest(RequireUncommitted(entity).Native, instance, definition));
    }

    /// <summary>
    /// Stages the canonical managed child-to-owner relation in the same native candidate that will
    /// admit the owner. This is the ordering required for initial non-empty Equipment validation.
    /// </summary>
    public void StageInitialContainment(EntityId owner, EntityId child)
    {
        ThrowIfDisposed();
        Binding ownerBinding = RequireUncommitted(owner);
        Binding childBinding = RequireBinding(child);
        if (!childBinding.IsCommitted)
        {
            throw new InvalidOperationException($"Contained entity {child.Value} must commit before owner {owner.Value}.");
        }
        if (!_entities.TryGetContainedIn(child, out EntityId container) || container != owner)
        {
            throw new InvalidOperationException($"Managed containment does not place entity {child.Value} in owner {owner.Value}.");
        }

        MechanicsContainmentReceipt observed = _mechanics.ReadContainment(
            new MechanicsContainmentReadRequest(childBinding.Native));
        if (observed.ChildEntityId != child.Value)
        {
            throw new InvalidOperationException("Mechanics containment readback returned the wrong canonical entity.");
        }
        _mechanics.StageInitialContainment(new MechanicsInitialContainmentRequest(
            ownerBinding.Native,
            child.Value,
            observed.StateRevision));
    }

    /// <summary>
    /// Atomically admits the currently staged typed Mechanics facts. The native bridge validates a
    /// complete candidate before making it visible; this method never uses <see cref="EntityBatch"/>.
    /// </summary>
    public MechanicsEntityReceipt Commit(EntityId entity)
    {
        ThrowIfDisposed();
        RequireActive(entity);
        Binding binding = RequireBinding(entity);
        if (binding.IsCommitted)
        {
            throw new InvalidOperationException($"Entity {entity.Value} has already committed its Mechanics projection.");
        }

        MechanicsEntityReceipt receipt = _mechanics.CommitEntity(binding.Native);
        binding.IsCommitted = true;
        binding.LifecycleStamp = receipt.Lifecycle.Stamp;
        return receipt;
    }

    /// <summary>
    /// Mirrors one explicit managed lifecycle change. The adapter preflights the canonical managed
    /// revision (or captures it as an exact guard), sends the matching native lifecycle stamp, then
    /// applies the deterministic managed transition. Product <see cref="EntityWorld"/> remains canonical.
    /// </summary>
    public MechanicsLifecycleReceipt SetLifecycle(
        EntityId entity,
        EntityLifecycle lifecycle,
        EntityRevision? expectedRevision = null)
    {
        ThrowIfDisposed();
        Binding binding = RequireBinding(entity);
        if (!binding.IsCommitted)
        {
            throw new InvalidOperationException($"Entity {entity.Value} must commit before its Mechanics lifecycle can change.");
        }

        EntityRevision observed = _entities.GetEntityRevision(entity);
        if (expectedRevision is EntityRevision expected && observed != expected)
        {
            throw new InvalidOperationException($"Entity {entity.Value} has a stale lifecycle revision.");
        }
        EntityLifecycle currentLifecycle = _entities.GetLifecycle(entity);
        if (currentLifecycle == EntityLifecycle.Tombstoned)
        {
            throw new InvalidOperationException($"Entity {entity.Value} has already been tombstoned.");
        }
        if (currentLifecycle == lifecycle)
        {
            throw new InvalidOperationException($"Entity {entity.Value} is already {lifecycle}.");
        }

        EntityRevision guardRevision = expectedRevision ?? observed;
        MechanicsLifecycleReceipt receipt = _mechanics.SetEntityLifecycle(new MechanicsLifecycleRequest(
            binding.Native,
            ToNative(lifecycle),
            MechanicsLifecycleGuard.Exact,
            binding.LifecycleStamp));

        // EntityWorld's transition has no remaining fallible work after the revision/lifecycle
        // preflight above. The adapter is intentionally synchronous: callers do not concurrently
        // mutate one EntityWorld while coordinating its native Mechanics mirror.
        _entities.SetLifecycle(entity, lifecycle, guardRevision);
        binding.LifecycleStamp = receipt.Stamp;
        if (lifecycle == EntityLifecycle.Tombstoned)
        {
            binding.Native.Dispose();
            _bindings.Remove(entity);
        }
        return receipt;
    }

    /// <summary>
    /// Transfers one unique item through the Engine's atomic Mechanics service, then mirrors the
    /// successful canonical containment relation in the product world. The managed revision guard
    /// makes the synchronous cross-boundary ordering explicit to the product.
    /// </summary>
    public MechanicsUniqueItemTransferLeaseReceipt TransferUniqueItem(
        EntityId item,
        EntityId fromOwner,
        EntityId toOwner,
        MechanicsUniqueItemTransferOperation operation,
        ulong? expectedManagedRevision = null)
    {
        ThrowIfDisposed();
        Binding itemBinding = RequireCommitted(item);
        Binding fromBinding = RequireCommitted(fromOwner);
        Binding toBinding = RequireCommitted(toOwner);
        ulong observedManagedRevision = _entities.Revision;
        if (expectedManagedRevision is ulong expected && observedManagedRevision != expected)
        {
            throw new InvalidOperationException("The managed containment revision is stale.");
        }
        PreflightUniqueItemTransfer(item, fromOwner, toOwner);

        // All managed failure conditions for SetContainment are preflighted above. Under this
        // adapter's synchronous EntityWorld contract, the exact revision guard leaves no fallible
        // managed work after native EquipmentService commits its already-atomic transfer.
        MechanicsUniqueItemTransferLeaseReceipt receipt = _mechanics.TransferUniqueItem(
            new MechanicsUniqueItemTransferRequest(
                itemBinding.Native,
                fromBinding.Native,
                toBinding.Native,
                operation.Operation,
                operation.Source.Kind,
                operation.Source.IntrinsicEntityId,
                operation.Source.IntrinsicInstance,
                operation.Source.EffectEntityId,
                operation.Source.EffectInstance,
                operation.Source.EffectStack,
                operation.Source.EffectSource,
                operation.Source.EquippedOwnerEntityId,
                operation.Source.EquippedItemEntityId,
                operation.Source.EquippedSource,
                operation.Source.RequestOperation,
                operation.Source.RequestInstance,
                operation.ExpectedRelationshipRevision,
                operation.FromRevisionGuard,
                operation.ExpectedFromRevision,
                operation.ToRevisionGuard,
                operation.ExpectedToRevision));
        _entities.SetContainment(item, toOwner, observedManagedRevision);
        return receipt;
    }

    /// <summary>
    /// Materializes a unique item using a product-created, already-active canonical identity.
    /// The native binding is intentionally temporary until the owner accepts its candidate; a
    /// rejected materialization releases only that uncommitted binding and never creates or rolls
    /// back a product entity.
    /// </summary>
    public MechanicsUniqueItemMaterializationLeaseReceipt MaterializeUniqueItem(
        EntityId item,
        string identity,
        EntityId container,
        string definition,
        ulong expectedNativeStateRevision,
        ulong? expectedManagedRevision = null)
    {
        ThrowIfDisposed();
        ArgumentException.ThrowIfNullOrWhiteSpace(identity);
        ArgumentException.ThrowIfNullOrWhiteSpace(definition);
        RequireActive(item);
        if (_bindings.ContainsKey(item))
        {
            throw new InvalidOperationException($"Entity {item.Value} is already bound to this Mechanics world.");
        }
        Binding containerBinding = RequireCommitted(container);
        ulong observedManagedRevision = _entities.Revision;
        if (expectedManagedRevision is ulong expected && observedManagedRevision != expected)
        {
            throw new InvalidOperationException("The managed materialization revision is stale.");
        }
        if (item == container || _entities.TryGetContainedIn(item, out _))
        {
            throw new InvalidOperationException($"Materialized item {item.Value} must be a distinct uncontained canonical entity.");
        }

        MechanicsEntity native = _mechanics.BindEntity(new MechanicsEntityBindRequest(_catalog, item.Value, identity));
        try
        {
            MechanicsUniqueItemMaterializationLeaseReceipt receipt = _mechanics.MaterializeUniqueItem(
                new MechanicsUniqueItemMaterializationRequest(
                    native,
                    containerBinding.Native,
                    definition,
                    expectedNativeStateRevision));

            // The native owner has admitted the same caller-owned identity. All managed failure
            // conditions for this exact synchronous mirror were preflighted before that commit.
            _entities.SetContainment(item, container, observedManagedRevision);
            _bindings.Add(item, new Binding(native)
            {
                IsCommitted = true,
                LifecycleStamp = receipt.Lifecycle.Stamp,
            });
            return receipt;
        }
        catch
        {
            native.Dispose();
            throw;
        }
    }

    /// <summary>
    /// Destroys one committed unique item through the Engine owner, then tombstones the canonical
    /// managed entity and releases its native lease. The native callback publishes the terminal
    /// lifecycle record, so this deliberately does not route through <see cref="SetLifecycle"/>.
    /// </summary>
    public MechanicsUniqueItemDestroyLeaseReceipt DestroyUniqueItem(
        EntityId item,
        MechanicsUniqueItemDestroyOperation operation,
        ulong expectedNativeStateRevision,
        ulong? expectedManagedRevision = null)
    {
        ThrowIfDisposed();
        Binding binding = RequireCommitted(item);
        ulong observedManagedRevision = _entities.Revision;
        if (expectedManagedRevision is ulong expected && observedManagedRevision != expected)
        {
            throw new InvalidOperationException("The managed destruction revision is stale.");
        }
        EntityRevision entityRevision = _entities.GetEntityRevision(item);

        MechanicsUniqueItemDestroyLeaseReceipt receipt = _mechanics.DestroyUniqueItem(
            new MechanicsUniqueItemDestroyRequest(
                binding.Native,
                operation.Operation,
                operation.Source.Kind,
                operation.Source.IntrinsicEntityId,
                operation.Source.IntrinsicInstance,
                operation.Source.EffectEntityId,
                operation.Source.EffectInstance,
                operation.Source.EffectStack,
                operation.Source.EffectSource,
                operation.Source.EquippedOwnerEntityId,
                operation.Source.EquippedItemEntityId,
                operation.Source.EquippedSource,
                operation.Source.RequestOperation,
                operation.Source.RequestInstance,
                expectedNativeStateRevision));

        // EntityWorld's direct destroy has no remaining failure after the exact revision and
        // liveness preflight above. It mirrors the owner result while avoiding a second native
        // lifecycle mutation.
        _entities.Destroy(item, entityRevision);
        binding.Native.Dispose();
        _bindings.Remove(item);
        return receipt;
    }

    /// <summary>
    /// Returns the sole native entity lease for a committed, live canonical entity.
    /// Sibling Mechanics service adapters may borrow this exact lease for a named Engine
    /// capability, but never own, dispose, rebind, or mirror it.
    /// </summary>
    internal MechanicsEntity RequireCommittedNativeEntity(EntityId entity)
    {
        ThrowIfDisposed();
        return RequireCommitted(entity).Native;
    }

    /// <summary>Catalog identity for named sibling services sharing this exact world.</summary>
    internal MechanicsCatalog Catalog => _catalog;

    public void Dispose()
    {
        if (_disposed)
        {
            return;
        }
        _disposed = true;
        foreach (Binding binding in _bindings.Values)
        {
            binding.Native.Dispose();
        }
        _bindings.Clear();
    }

    internal void PublishPreparedImport(
        MechanicsWorldImport native,
        EntityWorldRestoreCandidate managed,
        MechanicsWorldImportLeaseReceipt receipt)
    {
        ThrowIfDisposed();

        // ReadWorldImport proved this complete exact membership before either side was visible.
        // Claim into an isolated map first, so the adapter never exposes a partly rebuilt map.
        var replacement = new Dictionary<EntityId, Binding>(receipt.Entities.Length);
        try
        {
            _mechanics.PublishWorldImport(native);
            foreach (MechanicsWorldImportEntityRow row in receipt.Entities.Span)
            {
                MechanicsEntity entity = _mechanics.ClaimWorldImportEntity(
                    new MechanicsWorldImportEntityClaimRequest(native, row.EntityId));
                replacement.Add(new EntityId(row.EntityId), new Binding(entity)
                {
                    IsCommitted = true,
                    LifecycleStamp = row.LifecycleStamp,
                });
            }

            // The managed candidate has no remaining validation or product callback work.
            managed.Publish();
            Dictionary<EntityId, Binding> retired = _bindings;
            _bindings = replacement;
            replacement = [];
            foreach (Binding binding in retired.Values)
            {
                binding.Native.Dispose();
            }
        }
        finally
        {
            // On a failed claim, release only newly claimed wrappers. The prepared import itself
            // is owned by the candidate and is always released by its finally path.
            foreach (Binding binding in replacement.Values)
            {
                binding.Native.Dispose();
            }
        }
    }

    private MechanicsWorldImportRequest AdmitImportRequest(
        EntityWorldRestorePlan plan,
        MechanicsWorldImportRequest request)
    {
        ArgumentNullException.ThrowIfNull(request.Catalog);
        if (request.Catalog.Handle != _catalog.Handle)
        {
            throw new InvalidOperationException("Mechanics import request must use this adapter's admitted catalog.");
        }

        EntityWorldEntityState[] managedEntities = plan.Entities.ToArray();
        MechanicsWorldEntityRow[] nativeEntities = request.Entities.ToArray();
        var managedById = new Dictionary<ulong, EntityWorldEntityState>(managedEntities.Length);
        foreach (EntityWorldEntityState entity in managedEntities)
        {
            if (entity.Id.Value == 0 || !managedById.TryAdd(entity.Id.Value, entity))
            {
                throw new InvalidOperationException("Managed import plan has a duplicate or zero canonical entity identity.");
            }
        }

        var nativeById = new Dictionary<ulong, MechanicsWorldEntityRow>(nativeEntities.Length);
        foreach (MechanicsWorldEntityRow entity in nativeEntities)
        {
            if (entity.EntityId == 0 || !nativeById.TryAdd(entity.EntityId, entity)
                || !managedById.TryGetValue(entity.EntityId, out EntityWorldEntityState managed)
                || entity.Lifecycle != ToNative(managed.Lifecycle))
            {
                throw new InvalidOperationException("Mechanics import entity facts do not exactly correlate with the managed plan.");
            }
        }
        if (nativeById.Count != managedById.Count)
        {
            throw new InvalidOperationException("Mechanics import membership does not exactly correlate with the managed plan.");
        }

        MechanicsWorldComponentPresenceRow[] presenceRows = request.ComponentPresence.ToArray();
        MechanicsRevisionComponent[] components = Enum.GetValues<MechanicsRevisionComponent>();
        var presence = new Dictionary<(ulong Entity, MechanicsRevisionComponent Component), MechanicsWorldComponentPresenceRow>(
            presenceRows.Length);
        foreach (MechanicsWorldComponentPresenceRow row in presenceRows)
        {
            if (!managedById.ContainsKey(row.EntityId)
                || !components.Contains(row.Component)
                || !presence.TryAdd((row.EntityId, row.Component), row))
            {
                throw new InvalidOperationException("Mechanics import component presence must contain one exact row per entity and component family.");
            }
        }
        if (presence.Count != managedById.Count * components.Length
            || managedById.Keys.Any(entity => components.Any(component => !presence.ContainsKey((entity, component)))))
        {
            throw new InvalidOperationException("Mechanics import component presence must cover every requested entity and component family exactly once.");
        }

        var managedContainment = new HashSet<(ulong Child, ulong Container)>(
            plan.Containment.Select(row => (row.Child.Value, row.Container.Value)));
        var nativeContainment = new HashSet<(ulong Child, ulong Container)>(
            request.Containment.Span.ToArray().Select(row => (row.ChildEntityId, row.ContainerEntityId)));
        if (managedContainment.Count != plan.Containment.Count
            || nativeContainment.Count != request.Containment.Length
            || !managedContainment.SetEquals(nativeContainment))
        {
            throw new InvalidOperationException("Mechanics import containment does not exactly correlate with the managed plan.");
        }
        if (nativeContainment.Any(edge => !managedById.ContainsKey(edge.Child) || !managedById.ContainsKey(edge.Container)))
        {
            throw new InvalidOperationException("Mechanics import containment references an entity outside the managed plan.");
        }

        return request with { Catalog = _catalog };
    }

    private static void ValidateImportReceipt(
        MechanicsWorldImportRequest request,
        MechanicsWorldImportLeaseReceipt receipt)
    {
        if (receipt.CatalogId != request.Catalog.Handle.Value
            || receipt.StateRevisionAfter <= receipt.StateRevisionBefore
            || receipt.StateRevisionAfter <= request.StateRevision)
        {
            throw new InvalidOperationException("Prepared Mechanics import returned an invalid catalog or state revision receipt.");
        }

        Dictionary<ulong, MechanicsWorldEntityRow> requested = request.Entities.Span.ToArray()
            .ToDictionary(row => row.EntityId);
        var observed = new HashSet<ulong>();
        foreach (MechanicsWorldImportEntityRow row in receipt.Entities.Span)
        {
            if (!observed.Add(row.EntityId)
                || !requested.TryGetValue(row.EntityId, out MechanicsWorldEntityRow expected)
                || row.Identity != expected.Identity
                || row.Lifecycle != expected.Lifecycle
                || row.LifecycleStamp == 0)
            {
                throw new InvalidOperationException("Prepared Mechanics import receipt did not preserve exact entity/lifecycle facts.");
            }
        }
        if (observed.Count != requested.Count)
        {
            throw new InvalidOperationException("Prepared Mechanics import receipt did not preserve exact entity membership.");
        }

        var lifecycleRows = new Dictionary<ulong, MechanicsLifecycleReceipt>();
        foreach (MechanicsLifecycleReceipt lifecycle in receipt.Lifecycles.Span)
        {
            if (!lifecycleRows.TryAdd(lifecycle.EntityId, lifecycle)
                || !requested.TryGetValue(lifecycle.EntityId, out MechanicsWorldEntityRow expected)
                || lifecycle.Lifecycle != expected.Lifecycle
                || lifecycle.Stamp == 0)
            {
                throw new InvalidOperationException("Prepared Mechanics import receipt did not preserve exact lifecycle rows.");
            }
        }
        if (lifecycleRows.Count != requested.Count
            || receipt.Entities.Span.ToArray().Any(row => lifecycleRows[row.EntityId].Stamp != row.LifecycleStamp))
        {
            throw new InvalidOperationException("Prepared Mechanics import receipt lifecycle stamps do not exactly correlate with entity rows.");
        }

        MechanicsWorldComponentPresenceRow[] presenceRows = request.ComponentPresence.ToArray();
        var expectedRevisions = presenceRows.ToDictionary(row => (row.EntityId, row.Component));
        var observedRevisions = new HashSet<(ulong Entity, MechanicsRevisionComponent Component)>();
        foreach (MechanicsRevisionRemapRow revision in receipt.Revisions.Span)
        {
            if (!observedRevisions.Add((revision.EntityId, revision.Component))
                || !expectedRevisions.TryGetValue((revision.EntityId, revision.Component), out MechanicsWorldComponentPresenceRow expected)
                || revision.Present != expected.Present
                || revision.SnapshotRevision != expected.Revision
                || revision.RestoredRevision <= revision.SnapshotRevision
                || revision.RestoredRevision <= revision.CurrentRevision)
            {
                throw new InvalidOperationException("Prepared Mechanics import receipt did not preserve exact component revision facts.");
            }
        }
        if (observedRevisions.Count != expectedRevisions.Count)
        {
            throw new InvalidOperationException("Prepared Mechanics import receipt did not remap every component family exactly once.");
        }
    }

    private Binding RequireBinding(EntityId entity)
        => _bindings.TryGetValue(entity, out Binding? binding)
            ? binding
            : throw new InvalidOperationException($"Entity {entity.Value} is not bound to this Mechanics world.");

    private void RequireNoUncommittedBindings()
    {
        if (_bindings.Any(entry => !entry.Value.IsCommitted))
        {
            throw new InvalidOperationException("Mechanics world capture and restore require every bound entity to be committed.");
        }
    }

    private void RequireMatchingBindingTopology(MechanicsEntityWorldSnapshot snapshot)
    {
        if (_bindings.Count != snapshot.Bindings.Count
            || _bindings.Any(entry => !snapshot.Bindings.TryGetValue(entry.Key, out MechanicsBindingSnapshot captured)
                || captured.NativeHandle != entry.Value.Native.Handle.Value
                || captured.Committed != entry.Value.IsCommitted))
        {
            throw new InvalidOperationException("Mechanics restore requires the same local canonical binding topology as its snapshot.");
        }
    }

    private Dictionary<EntityId, ulong> ValidateRestoreReceipt(MechanicsWorldRestoreLeaseReceipt receipt)
    {
        if (receipt.StateRevisionAfter <= receipt.StateRevisionBefore)
        {
            throw new InvalidOperationException("Prepared Mechanics restore did not advance its state revision.");
        }
        const int MechanicsFamilyCount = 7;
        MechanicsLifecycleReceipt[] lifecycleRows = receipt.Lifecycles.ToArray();
        MechanicsRevisionRemapRow[] revisionRows = receipt.Revisions.ToArray();
        var lifecycleStamps = new Dictionary<EntityId, ulong>();
        foreach ((EntityId entity, Binding binding) in _bindings)
        {
            if (!binding.IsCommitted)
            {
                throw new InvalidOperationException("Prepared Mechanics restore observed an uncommitted binding.");
            }
            MechanicsLifecycleReceipt[] lifecycles = lifecycleRows
                .Where(row => row.EntityId == entity.Value)
                .ToArray();
            if (lifecycles.Length != 1 || lifecycles[0].Stamp <= binding.LifecycleStamp)
            {
                throw new InvalidOperationException($"Prepared Mechanics restore did not remap lifecycle guard for entity {entity.Value}.");
            }
            lifecycleStamps.Add(entity, lifecycles[0].Stamp);
            MechanicsRevisionRemapRow[] revisions = revisionRows
                .Where(row => row.EntityId == entity.Value)
                .ToArray();
            if (revisions.Length != MechanicsFamilyCount
                || revisions.Select(row => row.Component).Distinct().Count() != MechanicsFamilyCount
                || revisions.Any(row => row.RestoredRevision <= row.SnapshotRevision
                    || row.RestoredRevision <= row.CurrentRevision))
            {
                throw new InvalidOperationException($"Prepared Mechanics restore did not remap all component guards for entity {entity.Value}.");
            }
        }
        return lifecycleStamps;
    }

    private Binding RequireUncommitted(EntityId entity)
    {
        Binding binding = RequireBinding(entity);
        if (binding.IsCommitted)
        {
            throw new InvalidOperationException($"Entity {entity.Value} has already committed its Mechanics projection.");
        }
        return binding;
    }

    private Binding RequireCommitted(EntityId entity)
    {
        Binding binding = RequireBinding(entity);
        if (!binding.IsCommitted)
        {
            throw new InvalidOperationException($"Entity {entity.Value} must commit before its Mechanics state can be transferred.");
        }
        if (!_entities.IsAlive(entity))
        {
            throw new InvalidOperationException($"Entity {entity.Value} must be live before its Mechanics state can be transferred.");
        }
        return binding;
    }

    private void PreflightUniqueItemTransfer(EntityId item, EntityId fromOwner, EntityId toOwner)
    {
        if (item == fromOwner || item == toOwner || fromOwner == toOwner)
        {
            throw new InvalidOperationException("A unique-item transfer requires three distinct canonical entities.");
        }
        if (!_entities.TryGetContainedIn(item, out EntityId managedOwner) || managedOwner != fromOwner)
        {
            throw new InvalidOperationException($"Managed containment does not place item {item.Value} in source owner {fromOwner.Value}.");
        }
        for (EntityId ancestor = toOwner;
             _entities.TryGetContainedIn(ancestor, out EntityId next);
             ancestor = next)
        {
            if (next == item)
            {
                throw new InvalidOperationException($"Transferring item {item.Value} into owner {toOwner.Value} would create a containment cycle.");
            }
        }
    }

    private void RequireActive(EntityId entity)
    {
        if (_entities.GetLifecycle(entity) != EntityLifecycle.Active)
        {
            throw new InvalidOperationException($"Entity {entity.Value} must be active before it can enter Mechanics.");
        }
    }

    private void ThrowIfDisposed()
    {
        if (_disposed)
        {
            throw new ObjectDisposedException(nameof(MechanicsEntityWorld));
        }
    }

    private static MechanicsEntityLifecycle ToNative(EntityLifecycle lifecycle) => lifecycle switch
    {
        EntityLifecycle.Active => MechanicsEntityLifecycle.Active,
        EntityLifecycle.Disabled => MechanicsEntityLifecycle.Disabled,
        EntityLifecycle.Tombstoned => MechanicsEntityLifecycle.Tombstoned,
        _ => throw new ArgumentOutOfRangeException(nameof(lifecycle)),
    };

    private sealed class Binding(MechanicsEntity native)
    {
        public MechanicsEntity Native { get; } = native;
        public bool IsCommitted { get; set; }
        public ulong LifecycleStamp { get; set; }
    }
}

/// <summary>
/// Product-supplied facts for one Engine-owned unique-item transfer. Canonical entity bindings are
/// deliberately selected by <see cref="MechanicsEntityWorld"/> rather than supplied by a product.
/// </summary>
public readonly record struct MechanicsUniqueItemTransferOperation(
    string Operation,
    MechanicsSourceIdentity Source,
    ulong ExpectedRelationshipRevision,
    MechanicsRevisionGuard FromRevisionGuard,
    MechanicsComponentRevision ExpectedFromRevision,
    MechanicsRevisionGuard ToRevisionGuard,
    MechanicsComponentRevision ExpectedToRevision);

/// <summary>Product-supplied operation/source facts for an exact unique-item destruction.</summary>
public readonly record struct MechanicsUniqueItemDestroyOperation(
    string Operation,
    MechanicsSourceIdentity Source);
