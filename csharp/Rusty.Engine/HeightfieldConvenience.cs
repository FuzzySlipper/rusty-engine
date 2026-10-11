using System;
using System.Numerics;

namespace Rusty.Engine;

public readonly partial record struct HeightfieldCreateRequest
{
    /// <summary>
    /// A heightfield whose point (0, 0, 0) stands at <paramref name="Origin"/> in
    /// <paramref name="Layer"/>, one of its units spanning <paramref name="Scale"/> layer units,
    /// centred on <paramref name="Focus"/>, casting shadows. Use <c>with</c> to anchor it to a
    /// spatial session's world point instead, so it follows world-origin rebases.
    /// </summary>
    public HeightfieldCreateRequest(
        uint TileCells,
        ReadOnlyMemory<HeightfieldRing> Rings,
        ReadOnlyMemory<HeightfieldMaterial> Materials,
        RenderLayer Layer,
        Vector3 Origin,
        float Scale,
        HeightfieldFocus Focus)
        : this(TileCells, Rings, Materials, Layer, default, default, Origin, Scale, Focus, ShadowCasting.Cast)
    {
    }
}

public readonly partial record struct HeightfieldFocus
{
    /// <summary>Centre the rings on where <paramref name="camera"/> stands, measured as each call finishes.</summary>
    public static HeightfieldFocus Following(Camera camera) => new(camera, 0, 0);

    /// <summary>Centre the rings on a point of the heightfield, in its own units.</summary>
    public static HeightfieldFocus At(double x, double z) => new(default, x, z);
}

public readonly partial record struct HeightfieldTileRequest
{
    /// <summary>Height samples along the tile's side: its vertices and one more each side.</summary>
    public int HeightSide => (int)TileCells + 3;

    /// <summary>Colours, weights and material indices along the tile's side: its vertices.</summary>
    public int Side => (int)TileCells + 1;

    /// <summary>The heightfield's own x of the tile's vertex column <paramref name="i"/> (−1 to <c>TileCells + 1</c>).</summary>
    public double SampleX(int i) => OriginX + (i * CellSize);

    /// <summary>The heightfield's own z of the tile's vertex row <paramref name="j"/> (−1 to <c>TileCells + 1</c>).</summary>
    public double SampleZ(int j) => OriginZ + (j * CellSize);
}
