using System;
using System.Numerics;

namespace Rusty.Engine;

public readonly partial record struct VoxelSceneScatterRequest
{
    /// <summary>
    /// Grow <paramref name="Appearance"/> drawn with <paramref name="Material"/> on the named
    /// material slots (all slots when empty), <paramref name="Density"/> copies per square metre of
    /// ground within <paramref name="Radius"/> metres of the camera: upright, full size and
    /// untinted on ground up to 35 degrees steep, shrinking away over the last quarter of the
    /// radius, casting no shadows, at most 65,536 copies. Use <c>with</c> to vary the rest.
    /// </summary>
    public VoxelSceneScatterRequest(
        VoxelScenePresentation Presentation,
        uint Scatter,
        Appearance Appearance,
        Material Material,
        ReadOnlyMemory<uint> Slots,
        float Density,
        float Radius)
        : this(
            Presentation,
            Scatter,
            Appearance,
            Material,
            Slots,
            Density,
            Radius,
            Radius * 0.25f,
            1f,
            1f,
            Vector3.One,
            Vector3.One,
            35f,
            0f,
            false,
            65_536,
            0)
    {
    }
}
