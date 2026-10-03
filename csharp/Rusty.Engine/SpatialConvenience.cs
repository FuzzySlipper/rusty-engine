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
