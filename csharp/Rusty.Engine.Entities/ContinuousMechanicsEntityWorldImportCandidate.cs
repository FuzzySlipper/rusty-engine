using Rusty.Engine;

namespace Rusty.Engine.Entities;

/// <summary>
/// The paired continuous/exact import result. It owns no second native candidate: disposal and
/// publication delegate to the one exact Mechanics candidate created for the restore.
/// </summary>
public sealed class ContinuousMechanicsEntityWorldImportCandidate : IDisposable
{
    private MechanicsEntityWorldImportCandidate? _exact;

    internal ContinuousMechanicsEntityWorldImportCandidate(
        MechanicsEntityWorldImportCandidate exact,
        ContinuousMechanicsWorldImportLeaseReceipt continuousReceipt)
    {
        _exact = exact ?? throw new ArgumentNullException(nameof(exact));
        ContinuousReceipt = continuousReceipt;
    }

    /// <summary>Copied evidence for the exact half of the paired prepared import.</summary>
    public MechanicsWorldImportLeaseReceipt ExactReceipt
        => _exact?.Receipt ?? throw new ObjectDisposedException(nameof(ContinuousMechanicsEntityWorldImportCandidate));

    /// <summary>Copied evidence for the staged continuous half of the paired prepared import.</summary>
    public ContinuousMechanicsWorldImportLeaseReceipt ContinuousReceipt { get; }

    /// <summary>Explicit, idempotent publication of the one underlying exact candidate.</summary>
    public void Publish()
    {
        MechanicsEntityWorldImportCandidate exact = _exact
            ?? throw new ObjectDisposedException(nameof(ContinuousMechanicsEntityWorldImportCandidate));
        exact.Publish();
    }

    /// <summary>Cancels the one underlying exact candidate when it has not been published.</summary>
    public void Dispose()
    {
        MechanicsEntityWorldImportCandidate? exact = Interlocked.Exchange(ref _exact, null);
        exact?.Dispose();
    }
}

/// <summary>
/// Copied paired export evidence. Products choose how to encode this value; Engine retains no
/// archive schema and no native export lease after this object is returned.
/// </summary>
public readonly record struct ContinuousMechanicsEntityWorldExport(
    MechanicsWorldExportLeaseReceipt Exact,
    ContinuousMechanicsWorldExportLeaseReceipt Continuous);

/// <summary>
/// Product-decoded, handle-free continuous Mechanics import facts. Presence rows preserve the
/// intentional distinction between absent and present-but-empty component families.
/// </summary>
public sealed class ContinuousMechanicsWorldImportImage
{
    public ContinuousMechanicsWorldImportImage(
        ulong mechanicsStateRevision,
        string continuousCatalogVersion,
        string continuousCatalogFingerprint,
        ReadOnlyMemory<ContinuousMechanicsWorldComponentPresenceRow> componentPresence,
        ReadOnlyMemory<ContinuousMechanicsWorldStatRow> stats,
        ReadOnlyMemory<ContinuousMechanicsWorldTrackRow> tracks,
        ReadOnlyMemory<ContinuousMechanicsWorldIntrinsicSourceRow> intrinsicSources,
        ReadOnlyMemory<ContinuousMechanicsWorldActiveEffectRow> activeEffects)
    {
        MechanicsStateRevision = mechanicsStateRevision;
        ContinuousCatalogVersion = continuousCatalogVersion ?? throw new ArgumentNullException(nameof(continuousCatalogVersion));
        ContinuousCatalogFingerprint = continuousCatalogFingerprint ?? throw new ArgumentNullException(nameof(continuousCatalogFingerprint));
        ComponentPresence = componentPresence;
        Stats = stats;
        Tracks = tracks;
        IntrinsicSources = intrinsicSources;
        ActiveEffects = activeEffects;
    }

    public ulong MechanicsStateRevision { get; }
    public string ContinuousCatalogVersion { get; }
    public string ContinuousCatalogFingerprint { get; }
    public ReadOnlyMemory<ContinuousMechanicsWorldComponentPresenceRow> ComponentPresence { get; }
    public ReadOnlyMemory<ContinuousMechanicsWorldStatRow> Stats { get; }
    public ReadOnlyMemory<ContinuousMechanicsWorldTrackRow> Tracks { get; }
    public ReadOnlyMemory<ContinuousMechanicsWorldIntrinsicSourceRow> IntrinsicSources { get; }
    public ReadOnlyMemory<ContinuousMechanicsWorldActiveEffectRow> ActiveEffects { get; }
}
