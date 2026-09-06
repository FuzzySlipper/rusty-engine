using System.Numerics;

namespace Rusty.Engine;

public readonly partial record struct ImplicitGenerateRequest
{
    /// <summary>Generates with whole-triangle centroid material assignment.</summary>
    public ImplicitGenerateRequest(ImplicitField Field, ImplicitNode Source,
        Vector3 Minimum, Vector3 Maximum, float CellSize, float CreaseAngleDegrees,
        float UvScale, Material DefaultMaterial, ReadOnlyMemory<ImplicitMaterialRegion> Regions)
        : this(Field, Source, Minimum, Maximum, CellSize, CreaseAngleDegrees,
            UvScale, DefaultMaterial, Regions, ImplicitMaterialBoundaryMode.Centroid) { }
}
