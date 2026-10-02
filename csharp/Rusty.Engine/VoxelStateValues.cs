namespace Rusty.Engine;

/// <summary>Compact cell orientation and product-defined variant/stage.</summary>
public static class VoxelCellState
{
    public const uint MaximumVariant = 8191;
    public static uint Encode(uint quarterTurns, uint variant)
    {
        if (quarterTurns > 3) throw new System.ArgumentOutOfRangeException(nameof(quarterTurns));
        if (variant > MaximumVariant) throw new System.ArgumentOutOfRangeException(nameof(variant));
        return (variant << 2) | quarterTurns;
    }
    public static uint QuarterTurns(uint state) => state & 3;
    public static uint Variant(uint state) => state >> 2;
}

public readonly partial record struct VoxelEdit
{
    public VoxelEdit(VoxelEditKind Kind, VoxelAddress Address, uint MaterialSlot)
        : this(0, Kind, Address, MaterialSlot) { }
}
public readonly partial record struct VoxelResidencyTransaction
{
    public VoxelResidencyTransaction(SpatialSession Session,
        System.ReadOnlyMemory<VoxelResidencyOperation> Operations,
        System.ReadOnlyMemory<uint> MaterialSlots)
        : this(default, Session, Operations, MaterialSlots, default) { }

    /// <summary>Admits chunks with cell states and no densities.</summary>
    public VoxelResidencyTransaction(System.ReadOnlyMemory<uint> States, SpatialSession Session,
        System.ReadOnlyMemory<VoxelResidencyOperation> Operations,
        System.ReadOnlyMemory<uint> MaterialSlots)
        : this(States, Session, Operations, MaterialSlots, default) { }
}
public readonly partial record struct VoxelResidencyOperation
{
    /// <summary>An operation whose chunk carries no densities.</summary>
    public VoxelResidencyOperation(VoxelResidencyOperationKind Kind, VoxelChunkIdentity Chunk,
        uint MaterialOffset, uint MaterialCount)
        : this(Kind, Chunk, MaterialOffset, MaterialCount, 0, 0) { }
}
public readonly partial record struct VoxelSceneFaceMaterialBinding
{
    public VoxelSceneFaceMaterialBinding(uint MaterialSlot, SpatialFace Face, Material Material)
        : this(0, MaterialSlot, Face, Material) { }
}
