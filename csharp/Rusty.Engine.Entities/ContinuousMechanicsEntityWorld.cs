using Rusty.Engine;

namespace Rusty.Engine.Entities;

/// <summary>
/// A thin continuous-Mechanics sibling for one admitted <see cref="MechanicsEntityWorld"/>.
/// It borrows that world's canonical exact entity lease for every call; it never creates a
/// second binding, lifecycle projection, or native-world mirror.
/// </summary>
public sealed class ContinuousMechanicsEntityWorld
{
    private readonly MechanicsEntityWorld _mechanicsWorld;
    private readonly IContinuousMechanicsService _continuous;
    private readonly ContinuousMechanicsCatalog _catalog;

    public ContinuousMechanicsEntityWorld(
        MechanicsEntityWorld mechanicsWorld,
        IContinuousMechanicsService continuous,
        ContinuousMechanicsCatalog catalog)
    {
        _mechanicsWorld = mechanicsWorld ?? throw new ArgumentNullException(nameof(mechanicsWorld));
        _continuous = continuous ?? throw new ArgumentNullException(nameof(continuous));
        _catalog = catalog ?? throw new ArgumentNullException(nameof(catalog));
    }

    /// <summary>
    /// Attaches all four continuous component families to an already committed exact entity.
    /// Presence flags intentionally distinguish absent components from present-but-empty ones.
    /// </summary>
    public void Initialize(EntityId entity, ContinuousMechanicsInitialComponents initial)
    {
        ArgumentNullException.ThrowIfNull(initial);
        _continuous.SetInitialComponents(new ContinuousMechanicsInitialComponentsRequest(
            _catalog,
            Native(entity),
            initial.HasStats,
            initial.Stats,
            initial.HasTracks,
            initial.Tracks,
            initial.HasIntrinsicSources,
            initial.IntrinsicSources,
            initial.HasActiveEffects,
            initial.ActiveEffects));
    }

    /// <summary>
    /// Exports the exact and continuous facts as one copied, correlated pair. A product can
    /// encode the pair in its own state format without retaining a native lease or creating a
    /// second entity projection.
    /// </summary>
    public ContinuousMechanicsEntityWorldExport Export()
    {
        MechanicsWorldExportLeaseReceipt exact = _mechanicsWorld.Export();
        ContinuousMechanicsWorldExportLeaseReceipt continuous = _continuous.ExportWorld(
            new ContinuousMechanicsWorldExportRequest(_mechanicsWorld.Catalog, _catalog));
        ValidateExportCorrelation(exact, continuous, _continuous.ReadCatalog(_catalog));
        return new ContinuousMechanicsEntityWorldExport(exact, continuous);
    }

    /// <summary>
    /// Stages a handle-free continuous image into the same detached exact import candidate as
    /// its correlated Mechanics request. Nothing becomes live until the returned wrapper is
    /// explicitly published.
    /// </summary>
    public ContinuousMechanicsEntityWorldImportCandidate PrepareImport(
        EntityWorldRestorePlan plan,
        MechanicsWorldImportRequest mechanics,
        ContinuousMechanicsWorldImportImage continuous,
        ulong? expectedManagedRevision = null)
    {
        ArgumentNullException.ThrowIfNull(plan);
        ArgumentNullException.ThrowIfNull(continuous);
        ContinuousMechanicsWorldImportLeaseReceipt? staged = null;
        MechanicsEntityWorldImportCandidate exact = _mechanicsWorld.PrepareImport(
            plan,
            mechanics,
            expectedManagedRevision,
            (import, admitted) =>
            {
                ValidateImage(admitted, continuous);
                ContinuousMechanicsCatalogLeaseReceipt catalog = _continuous.ReadCatalog(_catalog);
                ValidateCurrentCatalog(catalog, continuous);
                var request = new ContinuousMechanicsWorldImportStageRequest(
                    import,
                    _mechanicsWorld.Catalog,
                    continuous.MechanicsStateRevision,
                    _catalog,
                    continuous.ContinuousCatalogVersion,
                    continuous.ContinuousCatalogFingerprint,
                    continuous.ComponentPresence,
                    continuous.Stats,
                    continuous.Tracks,
                    continuous.IntrinsicSources,
                    continuous.ActiveEffects);
                ContinuousMechanicsWorldImportLeaseReceipt receipt = _continuous.StageWorldImport(request);
                ValidateStageReceipt(admitted, continuous, catalog, receipt);
                staged = receipt;
            });
        return new ContinuousMechanicsEntityWorldImportCandidate(
            exact,
            staged ?? throw new InvalidOperationException("Continuous Mechanics staging did not produce a receipt."));
    }

    /// <summary>Reads copied continuous component facts for the exact bound entity.</summary>
    public ContinuousMechanicsComponentLeaseReceipt Read(EntityId entity)
        => _continuous.ReadComponents(new ContinuousMechanicsComponentReadRequest(_catalog, Native(entity)));

    /// <summary>Evaluates a continuous stat while preserving its binary64 result bits.</summary>
    public ContinuousMechanicsStatEvaluationLeaseReceipt EvaluateStat(EntityId entity, string stat)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(stat);
        return _continuous.EvaluateStat(new ContinuousMechanicsStatEvaluateRequest(_catalog, Native(entity), stat));
    }

    public ContinuousMechanicsStatMutationLeaseReceipt SetStatBase(
        EntityId entity,
        string operation,
        string stat,
        ulong baseBits,
        ContinuousMechanicsRevisionGuard revisionGuard = ContinuousMechanicsRevisionGuard.Unchecked,
        ulong expectedRevision = 0)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(operation);
        ArgumentException.ThrowIfNullOrWhiteSpace(stat);
        return _continuous.SetStatBase(new ContinuousMechanicsStatBaseMutationRequest(
            _catalog, Native(entity), operation, stat, baseBits, revisionGuard, expectedRevision));
    }

    public ContinuousMechanicsTrackLeaseReceipt ReadTrack(EntityId entity, string track)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(track);
        return _continuous.ReadTrack(new ContinuousMechanicsTrackReadRequest(_catalog, Native(entity), track));
    }

    public ContinuousMechanicsTrackLeaseReceipt SetTrack(
        EntityId entity,
        string operation,
        string track,
        ulong valueBits,
        ContinuousMechanicsTrackSetPolicy policy = ContinuousMechanicsTrackSetPolicy.RejectOutOfBounds,
        ContinuousMechanicsRevisionGuard revisionGuard = ContinuousMechanicsRevisionGuard.Unchecked,
        ulong expectedRevision = 0)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(operation);
        ArgumentException.ThrowIfNullOrWhiteSpace(track);
        return _continuous.SetTrack(new ContinuousMechanicsTrackSetRequest(
            _catalog, Native(entity), operation, track, valueBits, policy, revisionGuard, expectedRevision));
    }

    public ContinuousMechanicsTrackLeaseReceipt SpendTrack(
        EntityId entity,
        string operation,
        string track,
        ulong amountBits,
        ContinuousMechanicsRevisionGuard revisionGuard = ContinuousMechanicsRevisionGuard.Unchecked,
        ulong expectedRevision = 0)
        => AdjustTrack(entity, operation, track, amountBits, revisionGuard, expectedRevision, _continuous.SpendTrack);

    public ContinuousMechanicsTrackLeaseReceipt RestoreTrack(
        EntityId entity,
        string operation,
        string track,
        ulong amountBits,
        ContinuousMechanicsRevisionGuard revisionGuard = ContinuousMechanicsRevisionGuard.Unchecked,
        ulong expectedRevision = 0)
        => AdjustTrack(entity, operation, track, amountBits, revisionGuard, expectedRevision, _continuous.RestoreTrack);

    public ContinuousMechanicsEffectLeaseReceipt ApplyEffect(
        EntityId entity,
        string operation,
        string instance,
        string definition,
        ContinuousMechanicsRevisionGuard revisionGuard = ContinuousMechanicsRevisionGuard.Unchecked,
        ulong expectedRevision = 0)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(operation);
        ArgumentException.ThrowIfNullOrWhiteSpace(instance);
        ArgumentException.ThrowIfNullOrWhiteSpace(definition);
        return _continuous.ApplyEffect(new ContinuousMechanicsEffectApplyRequest(
            _catalog, Native(entity), operation, instance, definition, revisionGuard, expectedRevision));
    }

    public ContinuousMechanicsEffectLeaseReceipt RemoveEffect(
        EntityId entity,
        string operation,
        string instance,
        ContinuousMechanicsRevisionGuard revisionGuard = ContinuousMechanicsRevisionGuard.Unchecked,
        ulong expectedRevision = 0)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(operation);
        ArgumentException.ThrowIfNullOrWhiteSpace(instance);
        return _continuous.RemoveEffect(new ContinuousMechanicsEffectRemoveRequest(
            _catalog, Native(entity), operation, instance, revisionGuard, expectedRevision));
    }

    private ContinuousMechanicsTrackLeaseReceipt AdjustTrack(
        EntityId entity,
        string operation,
        string track,
        ulong amountBits,
        ContinuousMechanicsRevisionGuard revisionGuard,
        ulong expectedRevision,
        Func<ContinuousMechanicsTrackAdjustmentRequest, ContinuousMechanicsTrackLeaseReceipt> adjust)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(operation);
        ArgumentException.ThrowIfNullOrWhiteSpace(track);
        return adjust(new ContinuousMechanicsTrackAdjustmentRequest(
            _catalog, Native(entity), operation, track, amountBits, revisionGuard, expectedRevision));
    }

    private MechanicsEntity Native(EntityId entity) => _mechanicsWorld.RequireCommittedNativeEntity(entity);

    private void ValidateExportCorrelation(
        MechanicsWorldExportLeaseReceipt exact,
        ContinuousMechanicsWorldExportLeaseReceipt continuous,
        ContinuousMechanicsCatalogLeaseReceipt catalog)
    {
        if (exact.CatalogId != _mechanicsWorld.Catalog.Handle.Value
            || continuous.MechanicsCatalogId != exact.CatalogId
            || continuous.MechanicsStateRevision != exact.StateRevision
            || continuous.ContinuousCatalogId != _catalog.Handle.Value
            || catalog.CatalogId != _catalog.Handle.Value
            || continuous.ContinuousCatalogVersion != catalog.Version
            || continuous.ContinuousCatalogFingerprint != catalog.Fingerprint)
        {
            throw new InvalidOperationException("Continuous Mechanics export did not correlate with the exact Mechanics world.");
        }
    }

    private static void ValidateImage(
        MechanicsWorldImportRequest mechanics,
        ContinuousMechanicsWorldImportImage image)
    {
        if (image.MechanicsStateRevision != mechanics.StateRevision
            || string.IsNullOrWhiteSpace(image.ContinuousCatalogVersion)
            || string.IsNullOrWhiteSpace(image.ContinuousCatalogFingerprint))
        {
            throw new InvalidOperationException("Continuous import metadata does not correlate with its exact Mechanics import.");
        }

        ulong[] entities = mechanics.Entities.Span.ToArray().Select(row => row.EntityId).ToArray();
        var entitySet = new HashSet<ulong>(entities);
        if (entitySet.Count != entities.Length)
        {
            throw new InvalidOperationException("Exact Mechanics import must have distinct entities before continuous staging.");
        }

        ContinuousMechanicsWorldComponentPresenceRow[] presenceRows = image.ComponentPresence.ToArray();
        ContinuousMechanicsComponentKind[] components = Enum.GetValues<ContinuousMechanicsComponentKind>();
        var presence = new Dictionary<(ulong Entity, ContinuousMechanicsComponentKind Component), ContinuousMechanicsWorldComponentPresenceRow>();
        foreach (ContinuousMechanicsWorldComponentPresenceRow row in presenceRows)
        {
            if (!entitySet.Contains(row.EntityId)
                || !components.Contains(row.Component)
                || !presence.TryAdd((row.EntityId, row.Component), row))
            {
                throw new InvalidOperationException("Continuous import presence must contain one valid row per exact entity and component family.");
            }
        }
        if (presence.Count != entitySet.Count * components.Length
            || entitySet.Any(entity => components.Any(component => !presence.ContainsKey((entity, component)))))
        {
            throw new InvalidOperationException("Continuous import presence must cover every exact entity and component family.");
        }

        ValidateStats(image.Stats.Span, entitySet, presence);
        ValidateTracks(image.Tracks.Span, entitySet, presence);
        ValidateIntrinsicSources(image.IntrinsicSources.Span, entitySet, presence);
        ValidateActiveEffects(image.ActiveEffects.Span, entitySet, presence);
    }

    private void ValidateCurrentCatalog(
        ContinuousMechanicsCatalogLeaseReceipt catalog,
        ContinuousMechanicsWorldImportImage image)
    {
        if (catalog.CatalogId != _catalog.Handle.Value
            || catalog.Version != image.ContinuousCatalogVersion
            || catalog.Fingerprint != image.ContinuousCatalogFingerprint)
        {
            throw new InvalidOperationException("Continuous import image does not match this admitted continuous catalog.");
        }
    }

    private void ValidateStageReceipt(
        MechanicsWorldImportRequest mechanics,
        ContinuousMechanicsWorldImportImage image,
        ContinuousMechanicsCatalogLeaseReceipt catalog,
        ContinuousMechanicsWorldImportLeaseReceipt receipt)
    {
        if (receipt.MechanicsCatalogId != mechanics.Catalog.Handle.Value
            || receipt.MechanicsStateRevisionAfter <= receipt.MechanicsStateRevisionBefore
            || receipt.MechanicsStateRevisionAfter <= image.MechanicsStateRevision
            || receipt.ContinuousCatalogId != catalog.CatalogId
            || receipt.ContinuousCatalogVersion != catalog.Version
            || receipt.ContinuousCatalogFingerprint != catalog.Fingerprint)
        {
            throw new InvalidOperationException("Continuous import staging returned invalid catalog or state revision evidence.");
        }

        var expected = image.ComponentPresence.Span.ToArray()
            .ToDictionary(row => (row.EntityId, row.Component));
        var observed = new HashSet<(ulong Entity, ContinuousMechanicsComponentKind Component)>();
        foreach (ContinuousMechanicsRevisionRemapRow row in receipt.Revisions.Span)
        {
            if (!observed.Add((row.EntityId, row.Component))
                || !expected.TryGetValue((row.EntityId, row.Component), out ContinuousMechanicsWorldComponentPresenceRow saved)
                || row.Present != saved.Present
                || row.SnapshotRevision != saved.Revision
                || row.RestoredRevision <= row.SnapshotRevision
                || row.RestoredRevision <= row.CurrentRevision)
            {
                throw new InvalidOperationException("Continuous import staging did not produce one fresh remap per saved component family.");
            }
        }
        if (observed.Count != expected.Count)
        {
            throw new InvalidOperationException("Continuous import staging did not remap every saved component family.");
        }
    }

    private static void ValidateStats(
        ReadOnlySpan<ContinuousMechanicsWorldStatRow> rows,
        HashSet<ulong> entities,
        Dictionary<(ulong Entity, ContinuousMechanicsComponentKind Component), ContinuousMechanicsWorldComponentPresenceRow> presence)
    {
        var seen = new HashSet<(ulong Entity, string Key)>();
        foreach (ContinuousMechanicsWorldStatRow row in rows)
        {
            if (!entities.Contains(row.EntityId) || string.IsNullOrWhiteSpace(row.Stat)
                || !presence[(row.EntityId, ContinuousMechanicsComponentKind.Stats)].Present
                || !seen.Add((row.EntityId, row.Stat)) || !IsFiniteNormalizedBits(row.BaseBits))
            {
                throw new InvalidOperationException("Continuous Stats rows are malformed or contradict component presence.");
            }
        }
    }

    private static void ValidateTracks(
        ReadOnlySpan<ContinuousMechanicsWorldTrackRow> rows,
        HashSet<ulong> entities,
        Dictionary<(ulong Entity, ContinuousMechanicsComponentKind Component), ContinuousMechanicsWorldComponentPresenceRow> presence)
    {
        var seen = new HashSet<(ulong Entity, string Key)>();
        foreach (ContinuousMechanicsWorldTrackRow row in rows)
        {
            if (!entities.Contains(row.EntityId) || string.IsNullOrWhiteSpace(row.Track)
                || !presence[(row.EntityId, ContinuousMechanicsComponentKind.Tracks)].Present
                || !seen.Add((row.EntityId, row.Track)) || !IsFiniteNormalizedBits(row.CurrentBits))
            {
                throw new InvalidOperationException("Continuous Tracks rows are malformed or contradict component presence.");
            }
        }
    }

    private static void ValidateIntrinsicSources(
        ReadOnlySpan<ContinuousMechanicsWorldIntrinsicSourceRow> rows,
        HashSet<ulong> entities,
        Dictionary<(ulong Entity, ContinuousMechanicsComponentKind Component), ContinuousMechanicsWorldComponentPresenceRow> presence)
    {
        var seen = new HashSet<(ulong Entity, string Instance)>();
        foreach (ContinuousMechanicsWorldIntrinsicSourceRow row in rows)
        {
            if (!entities.Contains(row.EntityId) || string.IsNullOrWhiteSpace(row.Instance) || string.IsNullOrWhiteSpace(row.Definition)
                || !presence[(row.EntityId, ContinuousMechanicsComponentKind.IntrinsicSources)].Present
                || !seen.Add((row.EntityId, row.Instance)))
            {
                throw new InvalidOperationException("Continuous IntrinsicSources rows are malformed or contradict component presence.");
            }
        }
    }

    private static void ValidateActiveEffects(
        ReadOnlySpan<ContinuousMechanicsWorldActiveEffectRow> rows,
        HashSet<ulong> entities,
        Dictionary<(ulong Entity, ContinuousMechanicsComponentKind Component), ContinuousMechanicsWorldComponentPresenceRow> presence)
    {
        var seen = new HashSet<(ulong Entity, string Instance)>();
        foreach (ContinuousMechanicsWorldActiveEffectRow row in rows)
        {
            if (!entities.Contains(row.EntityId) || string.IsNullOrWhiteSpace(row.Instance) || string.IsNullOrWhiteSpace(row.Definition)
                || !presence[(row.EntityId, ContinuousMechanicsComponentKind.ActiveEffects)].Present
                || !seen.Add((row.EntityId, row.Instance)))
            {
                throw new InvalidOperationException("Continuous ActiveEffects rows are malformed or contradict component presence.");
            }
        }
    }

    private static bool IsFiniteNormalizedBits(ulong bits)
    {
        if (bits == 0x8000_0000_0000_0000)
        {
            return false;
        }
        double value = BitConverter.Int64BitsToDouble(unchecked((long)bits));
        return !double.IsNaN(value) && !double.IsInfinity(value);
    }
}

/// <summary>
/// Product-supplied initial facts for the four continuous Mechanics component families.
/// Numeric values remain their admitted binary64 bit patterns end-to-end.
/// </summary>
public sealed record ContinuousMechanicsInitialComponents(
    bool HasStats,
    ReadOnlyMemory<ContinuousMechanicsInitialStatRow> Stats,
    bool HasTracks,
    ReadOnlyMemory<ContinuousMechanicsInitialTrackRow> Tracks,
    bool HasIntrinsicSources,
    ReadOnlyMemory<ContinuousMechanicsInitialIntrinsicSourceRow> IntrinsicSources,
    bool HasActiveEffects,
    ReadOnlyMemory<ContinuousMechanicsInitialActiveEffectRow> ActiveEffects);
