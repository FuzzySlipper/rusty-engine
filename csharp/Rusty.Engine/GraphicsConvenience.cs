using System;
using System.Numerics;

namespace Rusty.Engine;

public readonly partial record struct MaterialRequest
{
    public MaterialRequest(
        Color color,
        RenderResourceHandle texture,
        float roughness,
        Color textureTint,
        Vector3 emissionColor,
        float emissionIntensity,
        bool doubleSided)
        : this(
            color,
            texture,
            roughness,
            textureTint,
            emissionColor,
            emissionIntensity,
            doubleSided,
            MaterialAlphaMode.Opaque,
            0.5f)
    {
    }
}

public readonly partial record struct MeshResourceCreateRequest
{
    public MeshResourceCreateRequest(
        ReadOnlyMemory<Vector3> positions,
        ReadOnlyMemory<Vector3> normals,
        ReadOnlyMemory<Vector2> uvs,
        ReadOnlyMemory<uint> indices,
        ReadOnlyMemory<MeshGroup> groups,
        ReadOnlyMemory<MeshMaterialBinding> bindings)
        : this(positions, normals, uvs, ReadOnlyMemory<Color>.Empty, indices, groups, bindings)
    {
    }
}

public readonly partial record struct SpriteAppearanceRequest
{
    public SpriteAppearanceRequest(
        RenderResourceHandle texture,
        Vector2 uvMin,
        Vector2 uvMax,
        Vector2 pivot,
        Vector2 size,
        BillboardMode billboard,
        SpriteSizeMode sizeMode,
        int renderOrder,
        SpriteDepthPolicy depth,
        Color tint)
        : this(texture, uvMin, uvMax, pivot, size, billboard, sizeMode, renderOrder, depth, tint, GraphicsDefaults.SpriteMaterial)
    {
    }
}

public readonly partial record struct SpriteFromAtlasRequest
{
    public SpriteFromAtlasRequest(
        SpriteAtlas atlas,
        uint frameId,
        Vector2 pivot,
        Vector2 size,
        BillboardMode billboard,
        SpriteSizeMode sizeMode,
        int renderOrder,
        SpriteDepthPolicy depth,
        Color tint)
        : this(atlas, frameId, pivot, size, billboard, sizeMode, renderOrder, depth, tint, GraphicsDefaults.SpriteMaterial)
    {
    }
}

/// <summary>Managed span count representation for generated mesh inputs; not allocation guarantees.</summary>
public static class GraphicsMeshLimits
{
    /// <summary>Managed span length representation. Meshes have no separate vertex policy cap.</summary>
    public const int MaximumVertices = int.MaxValue;

    /// <summary>Managed span length representation. Meshes have no separate index policy cap.</summary>
    public const int MaximumIndices = int.MaxValue;
}

internal static class GraphicsDefaults
{
    internal static readonly SpriteMaterialDescriptor SpriteMaterial = new(
        SpriteLightingMode.Unlit,
        default,
        default,
        1.0f,
        0.0f,
        SpriteAlphaMode.Blend,
        0.5f,
        SpriteShadowPolicy.None);
}
