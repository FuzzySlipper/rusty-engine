using System.Numerics;

namespace Rusty.Engine;

public readonly partial record struct SpatialContentArtifactInstance
{
    /// <summary>An unrotated placement.</summary>
    public SpatialContentArtifactInstance(ulong id, ContentReference content, long columnOffset, long levelOffset, long rowOffset)
        : this(id, content, columnOffset, levelOffset, rowOffset, 0)
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
