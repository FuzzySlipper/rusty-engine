using System.Numerics;

namespace Rusty.Engine;

public readonly partial record struct ImplicitGenerateRequest
{
    /// <summary>Generates with whole-triangle centroid material assignment.</summary>
    public ImplicitGenerateRequest(ImplicitField Field, ImplicitNode Source,
        Vector3 Minimum, Vector3 Maximum, float CellSize, float CreaseAngleDegrees,
        float UvScale, Material DefaultMaterial, ReadOnlyMemory<ImplicitMaterialRegion> Regions)
        : this(Field, Source, Minimum, Maximum, CellSize, CreaseAngleDegrees,
            UvScale, DefaultMaterial, Regions, ImplicitMaterialBoundaryMode.Centroid, 0.0f) { }

    /// <summary>Generates with the requested material-boundary assignment mode.</summary>
    public ImplicitGenerateRequest(ImplicitField Field, ImplicitNode Source,
        Vector3 Minimum, Vector3 Maximum, float CellSize, float CreaseAngleDegrees,
        float UvScale, Material DefaultMaterial, ReadOnlyMemory<ImplicitMaterialRegion> Regions,
        ImplicitMaterialBoundaryMode MaterialBoundaryMode)
        : this(Field, Source, Minimum, Maximum, CellSize, CreaseAngleDegrees,
            UvScale, DefaultMaterial, Regions, MaterialBoundaryMode, 0.0f) { }
    /// <summary>Uses the default extraction budgets of 262,144 vertices and triangles.</summary>
    public ImplicitGenerateRequest(ImplicitField Field, ImplicitNode Source,
        Vector3 Minimum, Vector3 Maximum, float CellSize, float CreaseAngleDegrees,
        float UvScale, Material DefaultMaterial, ReadOnlyMemory<ImplicitMaterialRegion> Regions,
        ImplicitMaterialBoundaryMode MaterialBoundaryMode, float MaterialSampleSpacing)
        : this(Field, Source, Minimum, Maximum, CellSize, CreaseAngleDegrees,
            UvScale, DefaultMaterial, Regions, MaterialBoundaryMode, MaterialSampleSpacing, 0, 0) { }
}
