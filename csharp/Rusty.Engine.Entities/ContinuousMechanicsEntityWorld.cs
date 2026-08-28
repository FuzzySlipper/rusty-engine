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
