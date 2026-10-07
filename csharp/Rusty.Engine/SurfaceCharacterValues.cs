using System.Numerics;

namespace Rusty.Engine;

public readonly partial record struct SurfaceCharacter
{
    /// <summary>Sharp placement with smooth shading and no roughness: how a
    /// reconstructed material without its own surface is drawn.</summary>
    public static SurfaceCharacter Default => new(VertexPlacement.Sharp, 180.0f, 0.0f);
}

public readonly partial record struct VoxelDensityEdit
{
    /// <summary>Replaces the densities of <paramref name="size"/> voxels from
    /// <paramref name="min"/> (x-fastest) with a range of the transaction's
    /// densities and, when <paramref name="materialCount"/> is not zero, the
    /// matching range of its materials.</summary>
    public static VoxelDensityEdit Region(VoxelAddress min, (uint X, uint Y, uint Z) size,
        uint densityOffset, uint densityCount, uint materialOffset = 0, uint materialCount = 0) => new(
        VoxelDensityEditKind.Region, min, size.X, size.Y, size.Z, densityOffset, densityCount,
        materialOffset, materialCount, VoxelDensityShape.Sphere, VoxelDensityOperation.Add,
        Vector3.Zero, 0.0f, Vector3.Zero, Vector3.Zero, 0.0f, 0);

    /// <summary>Blends a sphere in the scene's local frame into the densities.</summary>
    public static VoxelDensityEdit Sphere(Vector3 center, float radius, VoxelDensityOperation operation,
        uint materialSlot, float strength = 1.0f) => new(
        VoxelDensityEditKind.Brush, default, 0, 0, 0, 0, 0, 0, 0, VoxelDensityShape.Sphere, operation,
        center, radius, Vector3.Zero, Vector3.Zero, strength, materialSlot);

    /// <summary>Blends an axis-aligned box in the scene's local frame into the densities.</summary>
    public static VoxelDensityEdit Box(Vector3 min, Vector3 max, VoxelDensityOperation operation,
        uint materialSlot, float strength = 1.0f) => new(
        VoxelDensityEditKind.Brush, default, 0, 0, 0, 0, 0, 0, 0, VoxelDensityShape.Box, operation,
        Vector3.Zero, 0.0f, min, max, strength, materialSlot);
}

public readonly partial record struct VoxelDensityTransaction
{
    /// <summary>Brush edits only.</summary>
    public VoxelDensityTransaction(SpatialSession Session, ReadOnlyMemory<VoxelDensityEdit> Edits)
        : this(Session, Edits, default, default) { }
}

public readonly partial record struct VoxelTerrainLayerRequest
{
    /// <summary>One to sixteen slots, drawn as layers 0 to 15 in order.</summary>
    public VoxelTerrainLayerRequest(SpatialSession Session, ReadOnlyMemory<uint> Slots, uint TransitionCells)
        : this(Session, Slots, TransitionCells, default) { }
}

public readonly partial record struct SampledVolumeCreateRequest
{
    /// <summary>Holds at most 8,000,000 samples.</summary>
    public SampledVolumeCreateRequest(Vector3 Origin, float Spacing, uint Width, uint Height,
        uint Depth, float InitialValue)
        : this(Origin, Spacing, Width, Height, Depth, InitialValue, 0) { }
}

public readonly partial record struct SampledVolumeMaterial
{
    /// <summary>Uses the request's texture mapping.</summary>
    public SampledVolumeMaterial(uint Index, Material Material, SurfaceCharacter Character)
        : this(Index, Material, Character, default) { }
}

public readonly partial record struct SampledVolumeGenerateRequest
{
    /// <summary>Meshes the whole volume with the default limits and every
    /// sample in the default material.</summary>
    public SampledVolumeGenerateRequest(SampledVolume Volume, ImplicitField Field, float Isovalue,
        float CreaseAngleDegrees, float UvScale, Material DefaultMaterial,
        ReadOnlyMemory<ImplicitMaterialRegion> Regions, ImplicitMaterialBoundaryMode MaterialBoundaryMode,
        float MaterialSampleSpacing)
        : this(Volume, Field, Isovalue, CreaseAngleDegrees, UvScale, DefaultMaterial, Regions,
            MaterialBoundaryMode, MaterialSampleSpacing, default, default, 0, 0, 0, 0, 0, 0, 0, 0) { }

    /// <summary>Meshes the whole volume with per-sample materials and the
    /// default limits.</summary>
    public SampledVolumeGenerateRequest(SampledVolume Volume, ImplicitField Field, float Isovalue,
        float CreaseAngleDegrees, float UvScale, Material DefaultMaterial,
        ReadOnlyMemory<SampledVolumeMaterial> Materials)
        : this(Volume, Field, Isovalue, CreaseAngleDegrees, UvScale, DefaultMaterial, default,
            ImplicitMaterialBoundaryMode.Centroid, 0.0f, default, Materials, 0, 0, 0, 0, 0, 0, 0, 0) { }

    /// <summary>The same request for one block of <paramref name="blockSamples"/>
    /// owned samples per axis.</summary>
    public SampledVolumeGenerateRequest ForBlock(uint blockSamples, SampledVolumeBlock block) =>
        this with { BlockSamples = blockSamples, BlockX = block.X, BlockY = block.Y, BlockZ = block.Z };
}
