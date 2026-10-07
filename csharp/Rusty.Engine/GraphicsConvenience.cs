using System;
using System.Numerics;

namespace Rusty.Engine;

public readonly partial record struct MaterialRequest
{
    /// <summary>An opaque dielectric material.</summary>
    public MaterialRequest(
        Color Color,
        RenderResourceReference Texture,
        float Roughness,
        Color TextureTint,
        Vector3 EmissionColor,
        float EmissionIntensity,
        bool DoubleSided)
        : this(
            Color,
            Texture,
            Roughness,
            TextureTint,
            EmissionColor,
            EmissionIntensity,
            DoubleSided,
            MaterialAlphaMode.Opaque,
            0.5f,
            0,
            default,
            1,
            0,
            default,
            default,
            default,
            0,
            false,
            default,
            default,
            default,
            0f,
            false,
            0f,
            0f)
    {
    }

    /// <summary>A dielectric material (metalness 0).</summary>
    public MaterialRequest(
        Color Color,
        RenderResourceReference Texture,
        float Roughness,
        Color TextureTint,
        Vector3 EmissionColor,
        float EmissionIntensity,
        bool DoubleSided,
        MaterialAlphaMode AlphaMode,
        float AlphaCutoff)
        : this(
            Color,
            Texture,
            Roughness,
            TextureTint,
            EmissionColor,
            EmissionIntensity,
            DoubleSided,
            AlphaMode,
            AlphaCutoff,
            0,
            default,
            1,
            0,
            default,
            default,
            default,
            0,
            false,
            default,
            default,
            default,
            0f,
            false,
            0f,
            0f)
    {
    }

    /// <summary>A material without a normal map.</summary>
    public MaterialRequest(
        Color Color,
        RenderResourceReference Texture,
        float Roughness,
        Color TextureTint,
        Vector3 EmissionColor,
        float EmissionIntensity,
        bool DoubleSided,
        MaterialAlphaMode AlphaMode,
        float AlphaCutoff,
        float Metalness)
        : this(
            Color,
            Texture,
            Roughness,
            TextureTint,
            EmissionColor,
            EmissionIntensity,
            DoubleSided,
            AlphaMode,
            AlphaCutoff,
            Metalness,
            default,
            1,
            0,
            default,
            default,
            default,
            0,
            false,
            default,
            default,
            default,
            0f,
            false,
            0f,
            0f)
    {
    }

    /// <summary>A material read through the mesh uv (no triplanar planes).</summary>
    public MaterialRequest(
        Color Color,
        RenderResourceReference Texture,
        float Roughness,
        Color TextureTint,
        Vector3 EmissionColor,
        float EmissionIntensity,
        bool DoubleSided,
        MaterialAlphaMode AlphaMode,
        float AlphaCutoff,
        float Metalness,
        RenderResourceReference NormalMap,
        float NormalScale)
        : this(
            Color,
            Texture,
            Roughness,
            TextureTint,
            EmissionColor,
            EmissionIntensity,
            DoubleSided,
            AlphaMode,
            AlphaCutoff,
            Metalness,
            NormalMap,
            NormalScale,
            0,
            default,
            default,
            default,
            0,
            false,
            default,
            default,
            default,
            0f,
            false,
            0f,
            0f)
    {
    }

    /// <summary>A material shaded by the standard shader.</summary>
    public MaterialRequest(
        Color Color,
        RenderResourceReference Texture,
        float Roughness,
        Color TextureTint,
        Vector3 EmissionColor,
        float EmissionIntensity,
        bool DoubleSided,
        MaterialAlphaMode AlphaMode,
        float AlphaCutoff,
        float Metalness,
        RenderResourceReference NormalMap,
        float NormalScale,
        float TriplanarSharpness)
        : this(
            Color,
            Texture,
            Roughness,
            TextureTint,
            EmissionColor,
            EmissionIntensity,
            DoubleSided,
            AlphaMode,
            AlphaCutoff,
            Metalness,
            NormalMap,
            NormalScale,
            TriplanarSharpness,
            default,
            default,
            default,
            0,
            false,
            default,
            default,
            default,
            0f,
            false,
            0f,
            0f)
    {
    }

    /// <summary>A material repeating its texture once per uv unit, without stochastic tiling.</summary>
    public MaterialRequest(
        Color Color,
        RenderResourceReference Texture,
        float Roughness,
        Color TextureTint,
        Vector3 EmissionColor,
        float EmissionIntensity,
        bool DoubleSided,
        MaterialAlphaMode AlphaMode,
        float AlphaCutoff,
        float Metalness,
        RenderResourceReference NormalMap,
        float NormalScale,
        float TriplanarSharpness,
        MaterialShader Shader)
        : this(
            Color,
            Texture,
            Roughness,
            TextureTint,
            EmissionColor,
            EmissionIntensity,
            DoubleSided,
            AlphaMode,
            AlphaCutoff,
            Metalness,
            NormalMap,
            NormalScale,
            TriplanarSharpness,
            Shader,
            default,
            default,
            0,
            false,
            default,
            default,
            default,
            0f,
            false,
            0f,
            0f)
    {
    }
}

public readonly partial record struct MaterialShader
{
    /// <summary>A product shader reading one parameter vector; the rest are zero.</summary>
    public MaterialShader(RenderResourceReference Shader, Vector4 Parameter0)
        : this(Shader, Parameter0, default, default, default) { }

    /// <summary>A product shader without textures of its own.</summary>
    public MaterialShader(
        RenderResourceReference Shader,
        Vector4 Parameter0,
        Vector4 Parameter1,
        Vector4 Parameter2,
        Vector4 Parameter3)
        : this(Shader, Parameter0, Parameter1, Parameter2, Parameter3, default, default) { }
}

public readonly partial record struct MeshResourceCreateRequest
{
    public MeshResourceCreateRequest(
        ReadOnlyMemory<Vector3> Positions,
        ReadOnlyMemory<Vector3> Normals,
        ReadOnlyMemory<Vector2> Uvs,
        ReadOnlyMemory<uint> Indices,
        ReadOnlyMemory<MeshGroup> Groups,
        ReadOnlyMemory<MeshMaterialBinding> Bindings)
        : this(Positions, Normals, Uvs, ReadOnlyMemory<Color>.Empty, Indices, Groups, Bindings)
    {
    }
}

public readonly partial record struct SpriteAppearanceRequest
{
    public SpriteAppearanceRequest(
        RenderResource Texture,
        Vector2 UvMin,
        Vector2 UvMax,
        Vector2 Pivot,
        Vector2 Size,
        BillboardMode Billboard,
        SpriteSizeMode SizeMode,
        int RenderOrder,
        SpriteDepthPolicy Depth,
        Color Tint)
        : this(Texture, UvMin, UvMax, Pivot, Size, Billboard, SizeMode, RenderOrder, Depth, Tint, GraphicsDefaults.SpriteMaterial)
    {
    }
}

public readonly partial record struct SpriteFromAtlasRequest
{
    public SpriteFromAtlasRequest(
        SpriteAtlas Atlas,
        uint FrameId,
        Vector2 Pivot,
        Vector2 Size,
        BillboardMode Billboard,
        SpriteSizeMode SizeMode,
        int RenderOrder,
        SpriteDepthPolicy Depth,
        Color Tint)
        : this(Atlas, FrameId, Pivot, Size, Billboard, SizeMode, RenderOrder, Depth, Tint, GraphicsDefaults.SpriteMaterial)
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
        SpriteShadowPolicy.None,
        SpriteBlendMode.Alpha,
        0f);
}

public readonly partial record struct SpriteMaterialDescriptor
{
    /// <summary>A sprite material blended by alpha (<see cref="SpriteBlendMode.Alpha"/>) with a hard depth edge (<see cref="SoftnessMetres"/> 0).</summary>
    public SpriteMaterialDescriptor(
        SpriteLightingMode Lighting,
        RenderResourceReference NormalTexture,
        RenderResourceReference DepthTexture,
        float NormalStrength,
        float NormalBias,
        SpriteAlphaMode AlphaMode,
        float AlphaCutoff,
        SpriteShadowPolicy Shadow)
        : this(Lighting, NormalTexture, DepthTexture, NormalStrength, NormalBias, AlphaMode, AlphaCutoff, Shadow, SpriteBlendMode.Alpha, 0f)
    {
    }
}

public readonly partial record struct AuthoredMaterialAppearanceRequest
{
    /// <summary>An authored material appearance without a normal map.</summary>
    public AuthoredMaterialAppearanceRequest(AuthoredCatalog Catalog, string MaterialId, RenderResourceReference Texture)
        : this(Catalog, MaterialId, Texture, default, 1, 0, default) { }

    /// <summary>An authored material appearance read through its tile coordinates (no triplanar planes).</summary>
    public AuthoredMaterialAppearanceRequest(
        AuthoredCatalog Catalog,
        string MaterialId,
        RenderResourceReference Texture,
        RenderResourceReference NormalMap,
        float NormalScale)
        : this(Catalog, MaterialId, Texture, NormalMap, NormalScale, 0, default) { }

    /// <summary>An authored material appearance shaded by the standard shader.</summary>
    public AuthoredMaterialAppearanceRequest(
        AuthoredCatalog Catalog,
        string MaterialId,
        RenderResourceReference Texture,
        RenderResourceReference NormalMap,
        float NormalScale,
        float TriplanarSharpness)
        : this(Catalog, MaterialId, Texture, NormalMap, NormalScale, TriplanarSharpness, default) { }
}

public readonly partial record struct LightDescriptor
{
    /// <summary>A light whose requested shadow takes the Engine's default resolution, priority 0 and the standard filter.</summary>
    public LightDescriptor(
        LightKind Kind,
        Vector3 Color,
        float Intensity,
        bool Enabled,
        Vector3 Position,
        Vector3 Direction,
        bool HasRange,
        float Range,
        float Decay,
        float OuterAngleRadians,
        float Penumbra,
        LightShadowIntent ShadowIntent)
        : this(
            Kind,
            Color,
            Intensity,
            Enabled,
            Position,
            Direction,
            HasRange,
            Range,
            Decay,
            OuterAngleRadians,
            Penumbra,
            ShadowIntent,
            0,
            0,
            false,
            Vector3.Zero)
    {
    }

    /// <summary>A light of any kind but hemisphere, with its shadow settings.</summary>
    public LightDescriptor(
        LightKind Kind,
        Vector3 Color,
        float Intensity,
        bool Enabled,
        Vector3 Position,
        Vector3 Direction,
        bool HasRange,
        float Range,
        float Decay,
        float OuterAngleRadians,
        float Penumbra,
        LightShadowIntent ShadowIntent,
        uint ShadowResolution,
        int ShadowPriority,
        bool ShadowSoft)
        : this(
            Kind,
            Color,
            Intensity,
            Enabled,
            Position,
            Direction,
            HasRange,
            Range,
            Decay,
            OuterAngleRadians,
            Penumbra,
            ShadowIntent,
            ShadowResolution,
            ShadowPriority,
            ShadowSoft,
            Vector3.Zero)
    {
    }

    /// <summary>
    /// Sky light from above and ground light from below, blended by how far a
    /// surface faces up. It casts no shadow. The Engine's neutral rig lights
    /// the world with one; a product that owns its lights makes its own.
    /// </summary>
    public static LightDescriptor Hemisphere(Vector3 skyColor, Vector3 groundColor, float intensity, bool enabled = true) =>
        new(
            LightKind.Hemisphere,
            skyColor,
            intensity,
            enabled,
            Vector3.Zero,
            Vector3.UnitY,
            false,
            0,
            0,
            0,
            0,
            LightShadowIntent.Disabled,
            0,
            0,
            false,
            groundColor);
}

public readonly partial record struct MeshMaterialFactors
{
    /// <summary>Per-slot factors that keep the material's own texture tint.</summary>
    public MeshMaterialFactors(
        uint MaterialSlot,
        bool OverrideBaseColor,
        Color BaseColor,
        bool OverrideEmission,
        Vector3 EmissiveFactor,
        float EmissiveStrength)
        : this(MaterialSlot, OverrideBaseColor, BaseColor, OverrideEmission, EmissiveFactor, EmissiveStrength, false, default)
    {
    }

    /// <summary>Factors that only tint one slot: the colour is multiplied by <paramref name="tint"/>.</summary>
    public static MeshMaterialFactors Tint(uint materialSlot, Color tint) =>
        new(materialSlot, false, default, false, default, 0, true, tint);
}

public readonly partial record struct AppearanceFact
{
    /// <summary>An appearance fact whose parts cast shadows.</summary>
    public AppearanceFact(
        ulong ObjectId,
        bool HasParentObject,
        ulong ParentObjectId,
        Transform Transform,
        Appearance Appearance,
        bool Visible,
        RenderLayer Layer)
        : this(ObjectId, HasParentObject, ParentObjectId, Transform, Appearance, Visible, Layer, ShadowCasting.Cast)
    {
    }
}
