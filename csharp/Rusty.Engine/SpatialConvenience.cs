using System.Numerics;

namespace Rusty.Engine;

public readonly partial record struct SpatialContentArtifactInstance
{
    /// <summary>An unrotated placement.</summary>
    public SpatialContentArtifactInstance(ulong id, ContentReference content, long columnOffset, long levelOffset, long rowOffset)
        : this(id, content, columnOffset, levelOffset, rowOffset, 0, Vector3.Zero)
    {
    }

    /// <summary>A placement with an integer quarter-turn rotation.</summary>
    public SpatialContentArtifactInstance(ulong id, ContentReference content, long columnOffset, long levelOffset, long rowOffset, uint quarterTurns)
        : this(id, content, columnOffset, levelOffset, rowOffset, quarterTurns, Vector3.Zero)
    {
    }

    /// <summary>
    /// A placement with a continuous world-unit translation applied after the
    /// integer offsets and quarter-turn rotation.
    /// </summary>
    public SpatialContentArtifactInstance(
        ulong id,
        ContentReference content,
        long columnOffset,
        long levelOffset,
        long rowOffset,
        Vector3 translation)
        : this(id, content, columnOffset, levelOffset, rowOffset, 0, translation)
    {
    }
}

public readonly partial record struct CollisionNavigationConfig
{
    /// <summary>A configuration without jump edges.</summary>
    public CollisionNavigationConfig(
        ulong gridId,
        double cellSize,
        uint chunkSize,
        uint maximumCells,
        CharacterControllerConfig character,
        double maximumDrop,
        uint verticalSearchCells,
        uint supportsPerColumn,
        bool diagonalNeighbors,
        double snapAbove,
        double snapBelow,
        double snapAcross)
        : this(
            gridId,
            cellSize,
            chunkSize,
            maximumCells,
            character,
            maximumDrop,
            verticalSearchCells,
            supportsPerColumn,
            diagonalNeighbors,
            snapAbove,
            snapBelow,
            snapAcross,
            false,
            0,
            0)
    {
    }
}

public readonly partial record struct NavigationStepRequest
{
    /// <summary>A request of the grid published last.</summary>
    public NavigationStepRequest(SpatialSession session, Vector3 from, Vector3 target, float maxStepUnits, uint maxVisited)
        : this(session, from, target, maxStepUnits, maxVisited, 0)
    {
    }
}

public readonly partial record struct NavigationPathRequest
{
    /// <summary>A request of the grid published last.</summary>
    public NavigationPathRequest(SpatialSession session, PlanarNavCell start, PlanarNavCell goal, uint maxVisited)
        : this(session, start, goal, maxVisited, 0)
    {
    }
}

public readonly partial record struct NavigationWeightedPathRequest
{
    /// <summary>A request of the grid published last.</summary>
    public NavigationWeightedPathRequest(SpatialSession session, PlanarNavCell start, PlanarNavCell goal, uint maxVisited)
        : this(session, start, goal, maxVisited, 0)
    {
    }
}

public readonly partial record struct NavigationVolumetricPathRequest
{
    /// <summary>A request of the grid published last.</summary>
    public NavigationVolumetricPathRequest(SpatialSession session, PlanarNavCell start, PlanarNavCell goal, uint maxVisited, NavigationVolumetricConfig config)
        : this(session, start, goal, maxVisited, config, 0)
    {
    }
}

public readonly partial record struct NavigationVolumetricWeightedPathRequest
{
    /// <summary>A request of the grid published last.</summary>
    public NavigationVolumetricWeightedPathRequest(SpatialSession session, PlanarNavCell start, PlanarNavCell goal, uint maxVisited, NavigationVolumetricConfig config)
        : this(session, start, goal, maxVisited, config, 0)
    {
    }
}

public readonly partial record struct NavigationProjectionReadRequest
{
    /// <summary>A request of the grid published last.</summary>
    public NavigationProjectionReadRequest(SpatialSession session)
        : this(session, 0)
    {
    }
}

public readonly partial record struct NavigationTraversalReplaceRequest
{
    /// <summary>A request of the grid published last.</summary>
    public NavigationTraversalReplaceRequest(SpatialSession session, ReadOnlyMemory<NavigationTraversalCell> cells)
        : this(session, cells, 0)
    {
    }
}

public readonly partial record struct NavigationTraversalClearRequest
{
    /// <summary>A request of the grid published last.</summary>
    public NavigationTraversalClearRequest(SpatialSession session)
        : this(session, 0)
    {
    }
}

public readonly partial record struct NavigationVolumetricTraversalReplaceRequest
{
    /// <summary>A request of the grid published last.</summary>
    public NavigationVolumetricTraversalReplaceRequest(SpatialSession session, ReadOnlyMemory<NavigationVolumetricTraversalCell> cells)
        : this(session, cells, 0)
    {
    }
}

public readonly partial record struct NavigationVolumetricTraversalClearRequest
{
    /// <summary>A request of the grid published last.</summary>
    public NavigationVolumetricTraversalClearRequest(SpatialSession session)
        : this(session, 0)
    {
    }
}

public readonly partial record struct CollisionNavigationColumnRequest
{
    /// <summary>A request of the grid published last.</summary>
    public CollisionNavigationColumnRequest(SpatialSession session, long x, long z)
        : this(session, x, z, 0)
    {
    }
}

public readonly partial record struct CollisionNavigationEdgeRequest
{
    /// <summary>A request of the grid published last.</summary>
    public CollisionNavigationEdgeRequest(SpatialSession session, PlanarNavCell from, PlanarNavCell to)
        : this(session, from, to, 0)
    {
    }
}

public readonly partial record struct NavigationClearRequest
{
    /// <summary>A request clearing every grid.</summary>
    public NavigationClearRequest(SpatialSession session)
        : this(session, 0)
    {
    }
}

public readonly partial record struct NavigationPathResult
{
    /// <summary>A result without per-edge kinds, as before jump edges.</summary>
    public NavigationPathResult(
        ReadOnlyMemory<PlanarNavCell> path,
        NavigationPathOutcome outcome,
        NavigationProjectionKind kind,
        uint visited,
        ulong navigationRevision,
        ulong projectionHash,
        ulong pathHash)
        : this(path, ReadOnlyMemory<NavigationPathEdge>.Empty, outcome, kind, visited, navigationRevision, projectionHash, pathHash)
    {
    }
}

public readonly partial record struct NavigationWeightedPathResult
{
    /// <summary>A result without per-edge kinds, as before jump edges.</summary>
    public NavigationWeightedPathResult(
        ReadOnlyMemory<PlanarNavCell> path,
        NavigationPathOutcome outcome,
        NavigationProjectionKind kind,
        uint visited,
        ulong totalTraversalCost,
        ulong navigationRevision,
        ulong projectionHash,
        ulong traversalOverlayHash,
        ulong pathHash)
        : this(path, ReadOnlyMemory<NavigationPathEdge>.Empty, outcome, kind, visited, totalTraversalCost, navigationRevision, projectionHash, traversalOverlayHash, pathHash)
    {
    }
}

public readonly partial record struct NavigationStepResult
{
    /// <summary>A result without per-edge kinds, as before jump edges.</summary>
    public NavigationStepResult(
        ReadOnlyMemory<PlanarNavCell> path,
        NavigationPathOutcome outcome,
        Vector3 nextWaypoint,
        PlanarNavCell nextPathCell,
        uint reached,
        uint visited,
        ulong navigationRevision,
        ulong projectionHash,
        ulong pathHash,
        bool nearestPresent,
        PlanarNavCell nearestCell,
        Vector3 nearest)
        : this(path, ReadOnlyMemory<NavigationPathEdge>.Empty, outcome, nextWaypoint, nextPathCell, NavigationEdgeKind.Walk, 0f, reached, visited, navigationRevision, projectionHash, pathHash, nearestPresent, nearestCell, nearest)
    {
    }
}

public readonly partial record struct WorldOriginPrepareRequest
{
    /// <summary>A rebase that refuses when any row falls outside the local envelope.</summary>
    public WorldOriginPrepareRequest(
        SpatialSession session,
        long targetCellX,
        long targetCellY,
        long targetCellZ,
        ReadOnlyMemory<WorldOriginEntityRow> entities)
        : this(session, targetCellX, targetCellY, targetCellZ, entities, false)
    {
    }
}

public readonly partial record struct CollisionResidencyRequest
{
    /// <summary>A batch applied before the call returns.</summary>
    public CollisionResidencyRequest(SpatialSession Session, System.ReadOnlyMemory<StaticMeshAsset> Assets,
        System.ReadOnlyMemory<Vector3> Vertices, System.ReadOnlyMemory<Triangle> Triangles,
        System.ReadOnlyMemory<StaticMeshInstance> Instances, System.ReadOnlyMemory<ulong> RemovedAssets,
        System.ReadOnlyMemory<ulong> RemovedInstances)
        : this(Session, Assets, Vertices, Triangles, Instances, RemovedAssets, RemovedInstances, false)
    {
    }
}

public readonly partial record struct SpatialContentArtifactResidencyRequest
{
    /// <summary>A change applied before the call returns.</summary>
    public SpatialContentArtifactResidencyRequest(SpatialSession Session,
        System.ReadOnlyMemory<SpatialContentArtifactInstance> Admitted, System.ReadOnlyMemory<ulong> Removed,
        ulong NavigationGridId, uint NavigationChunkSize, uint NavigationMaxStepCells)
        : this(Session, Admitted, Removed, NavigationGridId, NavigationChunkSize, NavigationMaxStepCells, false)
    {
    }
}
