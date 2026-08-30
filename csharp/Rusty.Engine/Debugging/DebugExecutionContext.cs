namespace Rusty.Engine.Debugging;

/// <summary>
/// Retains the product lifecycle facts most recently delivered by the generated
/// NativeAOT bootstrap. Product-owned debug modules can read this context without
/// retaining a borrowed <see cref="ProductUpdate"/> or inferring host state.
/// Host-only transitions without a completed product callback, including a host
/// <c>ReportFault</c>, are intentionally not fabricated here.
/// </summary>
public class DebugExecutionContext
{
    private readonly object _gate = new();
    private ProductLifecycleState _lifecycleState = ProductLifecycleState.Created;
    private ProductUpdateFacts? _latestUpdateFacts;

    /// <summary>
    /// Gets one coherent, retained view of the latest successfully completed
    /// lifecycle callback and update delivery.
    /// </summary>
    public DebugExecutionSnapshot Snapshot
    {
        get
        {
            lock (_gate)
            {
                return new DebugExecutionSnapshot(_lifecycleState, _latestUpdateFacts);
            }
        }
    }

    /// <summary>
    /// Records a successfully completed non-update lifecycle callback. Generated
    /// product bootstrap code owns these transitions.
    /// </summary>
    protected void RecordLifecycleState(ProductLifecycleState lifecycleState, bool clearLatestUpdate)
    {
        lock (_gate)
        {
            _lifecycleState = lifecycleState;
            if (clearLatestUpdate)
            {
                _latestUpdateFacts = null;
            }
        }
    }

    /// <summary>
    /// Copies facts only after the product has successfully consumed its borrowed
    /// update. Generated product bootstrap code owns this operation.
    /// </summary>
    protected void RecordUpdate(ProductUpdateFacts facts)
    {
        lock (_gate)
        {
            _lifecycleState = facts.LifecycleState;
            _latestUpdateFacts = facts;
        }
    }
}

/// <summary>
/// Immutable retained debug facts from one product execution context. Lifecycle
/// state is current; update facts, generation, and control revision are absent
/// until a product update has completed successfully.
/// </summary>
public readonly record struct DebugExecutionSnapshot(
    ProductLifecycleState LifecycleState,
    ProductUpdateFacts? LatestUpdateFacts)
{
    public bool HasObservedUpdate => LatestUpdateFacts.HasValue;

    public ulong? Generation => LatestUpdateFacts?.Generation;

    public ulong? ControlRevision => LatestUpdateFacts?.ControlRevision;
}
