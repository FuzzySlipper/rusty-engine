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

/// <summary>Named limits for one immutable inline mesh admission.</summary>
public static class GraphicsMeshLimits
{
    /// <summary>Maximum vertices admitted in one mesh resource.</summary>
    public const int MaximumVertices = 262_144;

    /// <summary>Maximum triangle indices admitted in one mesh resource.</summary>
    public const int MaximumIndices = 786_432;

    /// <summary>Maximum copied attribute/index bytes and encoded definition bytes for one inline mesh resource.</summary>
    public const int MaximumInlineBytes = 16 * 1024 * 1024;
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
