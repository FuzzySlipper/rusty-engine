using System;
using System.Numerics;

namespace Rusty.Engine;

public readonly partial record struct MaterialRequest
{
    /// <summary>An opaque dielectric material.</summary>
    public MaterialRequest(
        Color color,
        RenderResourceReference texture,
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
            0.5f,
            0,
            default,
            1,
            0,
            default,
            default,
            default,
            0)
    {
    }

    /// <summary>A dielectric material (metalness 0).</summary>
    public MaterialRequest(
        Color color,
        RenderResourceReference texture,
        float roughness,
        Color textureTint,
        Vector3 emissionColor,
        float emissionIntensity,
        bool doubleSided,
        MaterialAlphaMode alphaMode,
        float alphaCutoff)
        : this(
            color,
            texture,
            roughness,
            textureTint,
            emissionColor,
            emissionIntensity,
            doubleSided,
            alphaMode,
            alphaCutoff,
            0,
            default,
            1,
            0,
            default,
            default,
            default,
            0)
    {
    }

    /// <summary>A material without a normal map.</summary>
    public MaterialRequest(
        Color color,
        RenderResourceReference texture,
        float roughness,
        Color textureTint,
        Vector3 emissionColor,
        float emissionIntensity,
        bool doubleSided,
        MaterialAlphaMode alphaMode,
        float alphaCutoff,
        float metalness)
        : this(
            color,
            texture,
            roughness,
            textureTint,
            emissionColor,
            emissionIntensity,
            doubleSided,
            alphaMode,
            alphaCutoff,
            metalness,
            default,
            1,
            0,
            default,
            default,
            default,
            0)
    {
    }

    /// <summary>A material read through the mesh uv (no triplanar planes).</summary>
    public MaterialRequest(
        Color color,
        RenderResourceReference texture,
        float roughness,
        Color textureTint,
        Vector3 emissionColor,
        float emissionIntensity,
        bool doubleSided,
        MaterialAlphaMode alphaMode,
        float alphaCutoff,
        float metalness,
        RenderResourceReference normalMap,
        float normalScale)
        : this(
            color,
            texture,
            roughness,
            textureTint,
            emissionColor,
            emissionIntensity,
            doubleSided,
            alphaMode,
            alphaCutoff,
            metalness,
            normalMap,
            normalScale,
            0,
            default,
            default,
            default,
            0)
    {
    }

    /// <summary>A material shaded by the standard shader.</summary>
    public MaterialRequest(
        Color color,
        RenderResourceReference texture,
        float roughness,
        Color textureTint,
        Vector3 emissionColor,
        float emissionIntensity,
        bool doubleSided,
        MaterialAlphaMode alphaMode,
        float alphaCutoff,
        float metalness,
        RenderResourceReference normalMap,
        float normalScale,
        float triplanarSharpness)
        : this(
            color,
            texture,
            roughness,
            textureTint,
            emissionColor,
            emissionIntensity,
            doubleSided,
            alphaMode,
            alphaCutoff,
            metalness,
            normalMap,
            normalScale,
            triplanarSharpness,
            default,
            default,
            default,
            0)
    {
    }

    /// <summary>A material repeating its texture once per uv unit, without stochastic tiling.</summary>
    public MaterialRequest(
        Color color,
        RenderResourceReference texture,
        float roughness,
        Color textureTint,
        Vector3 emissionColor,
        float emissionIntensity,
        bool doubleSided,
        MaterialAlphaMode alphaMode,
        float alphaCutoff,
        float metalness,
        RenderResourceReference normalMap,
        float normalScale,
        float triplanarSharpness,
        MaterialShader shader)
        : this(
            color,
            texture,
            roughness,
            textureTint,
            emissionColor,
            emissionIntensity,
            doubleSided,
            alphaMode,
            alphaCutoff,
            metalness,
            normalMap,
            normalScale,
            triplanarSharpness,
            shader,
            default,
            default,
            0)
    {
    }
}

public readonly partial record struct MaterialShader
{
    /// <summary>A product shader reading one parameter vector; the rest are zero.</summary>
    public MaterialShader(RenderResourceReference shader, Vector4 parameter0)
        : this(shader, parameter0, default, default, default) { }

    /// <summary>A product shader without textures of its own.</summary>
    public MaterialShader(
        RenderResourceReference shader,
        Vector4 parameter0,
        Vector4 parameter1,
        Vector4 parameter2,
        Vector4 parameter3)
        : this(shader, parameter0, parameter1, parameter2, parameter3, default, default) { }
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
        RenderResource texture,
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

public readonly partial record struct AuthoredMaterialAppearanceRequest
{
    /// <summary>An authored material appearance without a normal map.</summary>
    public AuthoredMaterialAppearanceRequest(AuthoredCatalog catalog, string materialId, RenderResourceReference texture)
        : this(catalog, materialId, texture, default, 1, 0, default) { }

    /// <summary>An authored material appearance read through its tile coordinates (no triplanar planes).</summary>
    public AuthoredMaterialAppearanceRequest(
        AuthoredCatalog catalog,
        string materialId,
        RenderResourceReference texture,
        RenderResourceReference normalMap,
        float normalScale)
        : this(catalog, materialId, texture, normalMap, normalScale, 0, default) { }

    /// <summary>An authored material appearance shaded by the standard shader.</summary>
    public AuthoredMaterialAppearanceRequest(
        AuthoredCatalog catalog,
        string materialId,
        RenderResourceReference texture,
        RenderResourceReference normalMap,
        float normalScale,
        float triplanarSharpness)
        : this(catalog, materialId, texture, normalMap, normalScale, triplanarSharpness, default) { }
}

public readonly partial record struct LightDescriptor
{
    /// <summary>A light whose requested shadow takes the Engine's default resolution, priority 0 and the standard filter.</summary>
    public LightDescriptor(
        LightKind kind,
        Vector3 color,
        float intensity,
        bool enabled,
        Vector3 position,
        Vector3 direction,
        bool hasRange,
        float range,
        float decay,
        float outerAngleRadians,
        float penumbra,
        LightShadowIntent shadowIntent)
        : this(
            kind,
            color,
            intensity,
            enabled,
            position,
            direction,
            hasRange,
            range,
            decay,
            outerAngleRadians,
            penumbra,
            shadowIntent,
            0,
            0,
            false)
    {
    }
}
