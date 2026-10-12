using System.Numerics;
using Rusty.Engine;

namespace CsharpWaterField;

/// <summary>
/// The fixture's ground: a valley falling gently along x, a river channel down its middle with
/// banks, a floodplain sloping back to the channel between rising sides, and a closed hollow in
/// the floodplain.
/// Heights are per 2 m cell, from the field's corner at the scene origin.
/// </summary>
internal sealed class Valley
{
    public const uint Width = 64, Length = 40;
    public const float Cell = 2;
    public const uint ChannelFirst = 18, ChannelCells = 4;
    private const float ValleySlope = .002f, ChannelDepth = 1.5f, PlainCrossSlope = .01f;
    /// <summary>The valley's sides: the last cells on each side rise this much a cell.</summary>
    private const uint SideCells = 6;
    private const float SideRise = .8f;
    public static readonly Vector2 HollowCentre = new(40.5f, 8.5f);
    public const float HollowRadius = 5;
    private const float HollowDepth = 1.2f;
    public const uint DamColumn = 30;
    private const float DamHeight = 4;

    private readonly float[] bed = new float[Width * Length];
    private readonly float[] natural;

    public Valley()
    {
        for (uint z = 0; z < Length; z++)
        for (uint x = 0; x < Width; x++)
            bed[Index(x, z)] = NaturalHeight(x, z);
        natural = (float[])bed.Clone();
    }

    public ReadOnlyMemory<float> Bed => bed;

    public static uint Index(uint x, uint z) => z * Width + x;

    public static bool InChannel(uint z) => z >= ChannelFirst && z < ChannelFirst + ChannelCells;

    public static bool InHollow(uint x, uint z) => Vector2.Distance(new(x, z), HollowCentre) < HollowRadius;

    /// <summary>The scene point over cell (x, z)'s centre.</summary>
    public static Vector3 Over(float x, float z) => new((x + .5f) * Cell, 0, (z + .5f) * Cell);

    private static float NaturalHeight(uint x, uint z)
    {
        float valley = ValleySlope * (Width - x) * Cell;
        if (InChannel(z)) return valley;
        uint fromBank = z < ChannelFirst ? ChannelFirst - z : z + 1 - (ChannelFirst + ChannelCells);
        uint side = z < SideCells ? SideCells - z : z + SideCells >= Length ? z + SideCells + 1 - Length : 0;
        float plain = valley + ChannelDepth + PlainCrossSlope * fromBank * Cell + SideRise * side;
        float hollow = Vector2.Distance(new(x, z), HollowCentre) / HollowRadius;
        return hollow < 1 ? plain - HollowDepth * (1 - hollow * hollow) : plain;
    }

    /// <summary>A wall across the valley at the dam column; the river backs up behind it.</summary>
    public WaterFieldCellValue[] Dam(bool built)
    {
        var cells = new WaterFieldCellValue[Length];
        for (uint z = 0; z < Length; z++)
        {
            float height = natural[Index(DamColumn, z)] + (built ? DamHeight : 0);
            bed[Index(DamColumn, z)] = height;
            cells[z] = new(DamColumn, z, height);
        }
        return cells;
    }

    public MeshResourceCreateRequest Mesh(Material material)
    {
        var positions = new Vector3[Width * Length];
        var normals = new Vector3[positions.Length];
        var uvs = new Vector2[positions.Length];
        for (uint z = 0; z < Length; z++)
        for (uint x = 0; x < Width; x++)
        {
            uint i = Index(x, z);
            positions[i] = Over(x, z) with { Y = bed[i] };
            int ix = (int)x, iz = (int)z;
            float dx = Height(ix + 1, iz) - Height(ix - 1, iz), dz = Height(ix, iz + 1) - Height(ix, iz - 1);
            normals[i] = Vector3.Normalize(new(-dx, 2 * Cell, -dz));
            uvs[i] = new(positions[i].X, positions[i].Z);
        }
        var indices = new List<uint>();
        for (uint z = 0; z + 1 < Length; z++)
        for (uint x = 0; x + 1 < Width; x++)
        {
            uint a = Index(x, z), b = Index(x + 1, z), c = Index(x, z + 1), d = Index(x + 1, z + 1);
            indices.AddRange([a, c, b, b, c, d]);
        }
        return new(positions, normals, uvs, indices.ToArray(), new MeshGroup[] { new(0, 0, (uint)indices.Count) }, new MeshMaterialBinding[] { new(0, material) });
    }

    private float Height(int x, int z) => bed[Index((uint)Math.Clamp(x, 0, (int)Width - 1), (uint)Math.Clamp(z, 0, (int)Length - 1))];
}
