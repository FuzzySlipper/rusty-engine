using System;
using System.Numerics;

namespace Rusty.Engine.VegetationTiles;

/// <summary>
/// One atlas frame made ready to grow in a vegetation layer: a billboard quad of the frame's size
/// that stands upright and turns about the vertical to face the camera, and a material made from
/// a sprite material. Pass <see cref="Look"/> as a layer's appearance, or a level's with
/// <see cref="VegetationLod.Mesh"/>, with a <c>default</c> material; every copy then takes the
/// layer's fade, thinning, handover and budgets as a mesh copy does. Keep this while the layer
/// grows it.
/// </summary>
public sealed class VegetationSprite : IDisposable
{
    private const float SpriteRoughness = .82f;
    private readonly Material paint;
    private readonly MeshResource mesh;

    /// <param name="engine">The product's Engine context.</param>
    /// <param name="texture">The atlas's image.</param>
    /// <param name="uvMin">The frame's top left in the image, 0 to 1.</param>
    /// <param name="uvMax">The frame's bottom right.</param>
    /// <param name="size">The frame's width and height in metres.</param>
    /// <param name="pivot">Where a copy's origin stands in the frame, from its bottom left, 0 to 1.</param>
    /// <param name="material">
    /// The lighting, alpha mode, emission and wind a sprite takes. A <see cref="SpriteAlphaMode.Blend"/>
    /// sprite draws through no per-copy selection and writes no depth. Additive blending, a tint mask
    /// and softness are a blended sprite's own and are refused.
    /// </param>
    public VegetationSprite(IEngineContext engine, RenderResourceReference texture, Vector2 uvMin, Vector2 uvMax,
        Vector2 size, Vector2 pivot, SpriteMaterialDescriptor material)
    {
        ArgumentNullException.ThrowIfNull(engine);
        if (material.Blend != SpriteBlendMode.Alpha || material.SoftnessMetres > 0 || material.TintMask.Value != 0)
            throw new ArgumentException("A vegetation sprite has no additive blending, softness or tint mask.", nameof(material));
        (BillboardLighting lighting, RenderResourceReference map) = material.Lighting switch
        {
            SpriteLightingMode.Unlit => (BillboardLighting.Flat, default),
            SpriteLightingMode.Synthetic => (BillboardLighting.Dome, default),
            SpriteLightingMode.AuthoredNormal => (BillboardLighting.NormalMap, material.NormalTexture),
            SpriteLightingMode.AuthoredDepth => (BillboardLighting.Height, material.DepthTexture),
            SpriteLightingMode.DerivedGradient => (BillboardLighting.ColorHeight, default),
            _ => throw new ArgumentOutOfRangeException(nameof(material)),
        };

        Color white = new(1, 1, 1, 1);
        paint = engine.Graphics.CreateMaterial(new MaterialRequest(white, texture, SpriteRoughness, white,
            material.EmissionColor, material.EmissionIntensity, true) with
        {
            AlphaMode = material.AlphaMode switch
            {
                SpriteAlphaMode.Mask => MaterialAlphaMode.Mask,
                SpriteAlphaMode.Blend => MaterialAlphaMode.Blend,
                _ => MaterialAlphaMode.Opaque,
            },
            AlphaCutoff = material.AlphaCutoff,
            Unlit = material.Lighting == SpriteLightingMode.Unlit,
            NormalMap = map,
            EmissionMap = material.EmissionMap,
            WindBend = material.WindBend,
            Billboard = new(lighting, material.NormalStrength, material.NormalBias, size, uvMin, uvMax, material.WindWholeTexels),
        });
        float left = -pivot.X * size.X, right = (1 - pivot.X) * size.X, bottom = -pivot.Y * size.Y, top = (1 - pivot.Y) * size.Y;
        Vector3[] positions = [new(left, bottom, 0), new(right, bottom, 0), new(right, top, 0), new(left, top, 0)];
        Vector2[] uvs = [new(uvMin.X, uvMax.Y), new(uvMax.X, uvMax.Y), new(uvMax.X, uvMin.Y), new(uvMin.X, uvMin.Y)];
        Vector3[] normals = [Vector3.UnitZ, Vector3.UnitZ, Vector3.UnitZ, Vector3.UnitZ];
        mesh = engine.Graphics.CreateMeshResource(new MeshResourceCreateRequest(positions, normals, uvs,
            ReadOnlyMemory<Color>.Empty, new uint[] { 0, 1, 2, 0, 2, 3 }, new MeshGroup[] { new(0, 0, 6) },
            new MeshMaterialBinding[] { new(0, paint) }, ReadOnlyMemory<Vector4>.Empty, ReadOnlyMemory<uint>.Empty, true));
        Look = engine.Graphics.CreateMeshAppearance(mesh);
    }

    /// <summary>The appearance a vegetation layer or level grows.</summary>
    public Appearance Look { get; }

    public void Dispose()
    {
        Look.Dispose();
        mesh.Dispose();
        paint.Dispose();
    }
}
