namespace Rusty.Engine;

/// <summary>Names a live Graphics mesh for one synchronous Spatial collision copy.</summary>
public readonly partial record struct MeshResourceReference
{
    public MeshResourceReference(MeshResource resource)
        : this(resource.Handle.Value)
    {
    }
}

public readonly partial record struct StaticMeshAsset
{
    public StaticMeshAsset(
        ulong id,
        uint firstVertex,
        uint vertexCount,
        uint firstTriangle,
        uint triangleCount)
        : this(id, default, firstVertex, vertexCount, firstTriangle, triangleCount)
    {
    }
}
