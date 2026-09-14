using System.Numerics;

namespace Rusty.Engine;

/// <summary>Texture coordinates in an implicit field's extraction space.</summary>
public readonly partial record struct ImplicitTextureMapping
{
    /// <summary>Uses the established dominant-normal chart with independent U/V transforms.</summary>
    public static ImplicitTextureMapping MajorAxis(Vector2 scale, Vector2 offset) => new(
        true,
        ImplicitTextureProjection.MajorAxis,
        Vector3.Zero,
        Vector3.Zero,
        scale,
        offset);

    /// <summary>Projects every surface point onto the supplied orthonormal U/V extraction axes.</summary>
    public static ImplicitTextureMapping Basis(
        Vector3 uAxis,
        Vector3 vAxis,
        Vector2 scale,
        Vector2 offset) => new(
        true,
        ImplicitTextureProjection.Basis,
        uAxis,
        vAxis,
        scale,
        offset);
}
