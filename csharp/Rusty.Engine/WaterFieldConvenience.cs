using System;
using System.Numerics;

namespace Rusty.Engine;

public readonly partial record struct WaterFieldCreateRequest
{
    /// <summary>
    /// A dry field of <paramref name="Width"/> by <paramref name="Length"/> cells over
    /// <paramref name="Bed"/> (heights from its corner, x fastest), its corner at
    /// <paramref name="Origin"/> in the scene, with the default policy. Use <c>with</c> to start
    /// it wet, anchor it to a spatial session's world point so it follows world-origin rebases,
    /// or choose a policy.
    /// </summary>
    public WaterFieldCreateRequest(uint Width, uint Length, float CellSize, ReadOnlyMemory<float> Bed, Vector3 Origin)
        : this(Width, Length, CellSize, Bed, default, default, default, Origin, default)
    {
    }
}

public readonly partial record struct WaterFieldBoundaryRequest
{
    /// <summary>A reservoir held at <paramref name="surface"/> (field heights) beyond the edge cells.</summary>
    public static WaterFieldBoundaryRequest Stage(WaterField field, WaterFieldSide side, float surface, uint first = 0, uint count = 0) =>
        new(field, side, first, count, WaterFieldBoundaryKind.Stage, surface);

    /// <summary><paramref name="discharge"/> cubic metres a second entering, spread over the edge cells.</summary>
    public static WaterFieldBoundaryRequest Inflow(WaterField field, WaterFieldSide side, float discharge, uint first = 0, uint count = 0) =>
        new(field, side, first, count, WaterFieldBoundaryKind.Inflow, discharge);

    /// <summary>Water leaves as if the ground carried on past the edge cells.</summary>
    public static WaterFieldBoundaryRequest Open(WaterField field, WaterFieldSide side, uint first = 0, uint count = 0) =>
        new(field, side, first, count, WaterFieldBoundaryKind.Open, 0);

    /// <summary>Nothing crosses the edge cells.</summary>
    public static WaterFieldBoundaryRequest Wall(WaterField field, WaterFieldSide side, uint first = 0, uint count = 0) =>
        new(field, side, first, count, WaterFieldBoundaryKind.Wall, 0);
}

public readonly partial record struct WaterFieldSample
{
    /// <summary>The depth-averaged flow, x and z.</summary>
    public Vector2 Velocity => new(VelocityX, VelocityZ);
}

public static class WaterFieldConvenience
{
    /// <summary>The water at one scene point (its x and z), from the field's latest snapshot.</summary>
    public static WaterFieldSample Sample(this IWaterFieldService water, WaterField field, Vector3 point)
    {
        ReadOnlyMemory<WaterFieldPoint> points = new[] { new WaterFieldPoint(point.X, point.Z) };
        return water.Sample(new WaterFieldSampleRequest(field, points)).Span[0];
    }

    /// <summary>Draws the field's wet cells with <paramref name="material"/> in the scene.</summary>
    public static WaterFieldReadout Present(this IWaterFieldService water, WaterField field, Material material) =>
        water.Present(new WaterFieldPresentRequest(field, material, RenderLayer.Scene, 0));
}
