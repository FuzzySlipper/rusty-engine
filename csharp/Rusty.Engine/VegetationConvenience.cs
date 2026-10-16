using System;
using System.Numerics;

namespace Rusty.Engine;

public readonly partial record struct VegetationFieldCreateRequest
{
    /// <summary>
    /// A field over <paramref name="Session"/>'s world (<c>default</c>: the scene's own space) in
    /// square tiles of <paramref name="TileSize"/> metres sampled <paramref name="TileCells"/> cells a
    /// side, centred on <paramref name="Focus"/>, placing at most 64 tile layers a call.
    /// </summary>
    public VegetationFieldCreateRequest(OptionalSpatialSession Session, double TileSize, uint TileCells, VegetationFocus Focus)
        : this(Session, TileSize, TileCells, 64, Focus)
    {
    }
}

public readonly partial record struct VegetationFocus
{
    /// <summary>Centre the tiles on where <paramref name="camera"/> stands, measured as each call finishes.</summary>
    public static VegetationFocus Following(Camera camera) => new(camera, 0, 0, 0);

    /// <summary>Centre the tiles on a world point.</summary>
    public static VegetationFocus At(double x, double y, double z) => new(default, x, y, z);
}

public readonly partial record struct VegetationLayerRequest
{
    /// <summary>
    /// Grow <paramref name="Appearance"/> drawn with <paramref name="Material"/>,
    /// <paramref name="Density"/> copies per square metre on ground class 0, on tiles within
    /// <paramref name="Reach"/> metres of the focus: upright, full size and untinted on ground up
    /// to 35 degrees steep, shrinking away over the last quarter of the reach, however near,
    /// casting no shadows, at most 65,536 copies. Use <c>with</c> to vary the rest; a voxel scene
    /// scatter with the same key, seed and density grows the same spots, so its fade-out band can
    /// be this layer's <c>Near</c> and <c>NearFade</c>.
    /// </summary>
    public VegetationLayerRequest(VegetationField Field, uint Layer, Appearance Appearance, Material Material, float Density, float Reach)
        : this(Field, Layer, Appearance, Material, Density, 1, 35f, 0f, 1f, 1f, Vector3.One, Vector3.One, 0, Reach, Reach * 0.25f,
            0f, 0f, 65_536, false)
    {
    }

    /// <summary>The bit for ground class <paramref name="ground"/> (0 to 63), to combine into <c>Grounds</c>.</summary>
    public static ulong Ground(uint ground)
    {
        ArgumentOutOfRangeException.ThrowIfGreaterThan(ground, 63u);
        return 1UL << (int)ground;
    }
}

public readonly partial record struct VegetationTileRequest
{
    /// <summary>Height samples along the tile's side: its vertices and one more each side.</summary>
    public int HeightSide => (int)TileCells + 3;

    /// <summary>Ground classes and cover weights along the tile's side: its vertices.</summary>
    public int Side => (int)TileCells + 1;

    /// <summary>The world x of the tile's vertex column <paramref name="i"/> (−1 to <c>TileCells + 1</c>).</summary>
    public double SampleX(int i) => OriginX + (i * CellSize);

    /// <summary>The world z of the tile's vertex row <paramref name="j"/> (−1 to <c>TileCells + 1</c>).</summary>
    public double SampleZ(int j) => OriginZ + (j * CellSize);
}
