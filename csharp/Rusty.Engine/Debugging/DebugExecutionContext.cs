namespace Rusty.Engine.Debugging;

/// <summary>
/// Retains committed Rust-owned lifecycle facts and the most recently delivered
/// product update. Product-owned debug modules can read this context without
/// retaining borrowed callback data or inferring host state.
/// </summary>
public class DebugExecutionContext
{
    private readonly object _gate = new();
    private ProductLifecycleState _lifecycleState = ProductLifecycleState.Created;
    private ProductRuntimeBinding? _runtimeBinding;
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
                return new DebugExecutionSnapshot(_lifecycleState, _runtimeBinding, _latestUpdateFacts);
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
    /// Copies one authoritative host snapshot after Rust has committed the
    /// lifecycle or control transition.
    /// </summary>
    protected void RecordCommittedRuntime(ProductRuntimeFacts facts)
    {
        lock (_gate)
        {
            bool generationChanged = _runtimeBinding is { } previous
                && previous.Generation != facts.Generation;
            _lifecycleState = facts.LifecycleState;
            _runtimeBinding = new ProductRuntimeBinding(facts.InstanceId, facts.Generation, facts.ControlRevision);
            if (generationChanged)
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
/// state and runtime binding come from the latest committed Rust observation;
/// update facts remain absent until a product update completes successfully.
/// </summary>
public readonly record struct DebugExecutionSnapshot(
    ProductLifecycleState LifecycleState,
    ProductRuntimeBinding? RuntimeBinding,
    ProductUpdateFacts? LatestUpdateFacts)
{
    public bool HasObservedUpdate => LatestUpdateFacts.HasValue;

    public ulong? Generation => RuntimeBinding?.Generation ?? LatestUpdateFacts?.Generation;

    public ulong? ControlRevision => RuntimeBinding?.ControlRevision ?? LatestUpdateFacts?.ControlRevision;
}
