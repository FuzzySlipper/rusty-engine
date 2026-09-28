using System.Numerics;
using System.Text;
using System.Text.Json;

namespace Rusty.Engine.Debugging;

/// <summary>On-demand collision occupancy from stacked Engine map queries.
/// This formats retained collision facts; it does not reconstruct a second world.</summary>
public static class SpatialGridSnapshot
{
    public const int MaximumCells = 8192;

    public static string Capture(ISpatialService spatial, SpatialMapRequest footprint, uint layers,
        SpatialMapObservation observation)
    {
        if (layers == 0 || footprint.Columns == 0 || footprint.Rows == 0 ||
            footprint.Columns > 31 || footprint.Rows > 31 || layers > 31 ||
            (ulong)layers * footprint.Columns * footprint.Rows > MaximumCells ||
            !double.IsFinite(footprint.CellSize) || footprint.CellSize <= 0)
            throw new ArgumentOutOfRangeException(nameof(layers), "Grid requires positive dimensions <=31, <=8192 cells and positive finite cell size.");
        using MemoryStream stream = new();
        using (Utf8JsonWriter w = new(stream))
        {
            w.WriteStartObject();
            w.WriteString("kind", "collision-grid");
            w.WriteString("source", "retained Engine static collision and supplied dynamic collider bounds");
            w.WriteString("meaning", "# occupied static; D dynamic; B both; . no collision in queried sources. Free is not walkable; unsampled/outside cells are unknown. Not visual voxels or a movement clearance certificate.");
            w.WriteString("axes", "right-handed world XYZ; layers +Y, rows +Z, characters +X");
            w.WriteString("cellWorldBounds", "min=origin+[x,y,z]*cellSize; max=min+cellSize");
            Point(w, "origin", footprint.Origin); Point(w, "player", observation.PlayerPosition);
            Point(w, "facing", observation.PlayerFacing);
            w.WriteString("stamp", observation.Stamp);
            w.WriteNumber("cellSize", footprint.CellSize);
            w.WriteNumber("columns", footprint.Columns); w.WriteNumber("rows", footprint.Rows); w.WriteNumber("layers", layers);
            w.WritePropertyName("slices"); w.WriteStartArray();
            for (uint y = 0; y < layers; y++)
            {
                double low = footprint.Origin.Y + y * footprint.CellSize;
                var receipt = spatial.ReadMap(new SpatialMapRequest(footprint.Session, footprint.Origin,
                    footprint.CellSize, footprint.Columns, footprint.Rows, low, low + footprint.CellSize,
                    low, low + footprint.CellSize, footprint.Entities));
                w.WriteStartObject(); w.WriteNumber("y", y); w.WriteNumber("minY", low);
                w.WritePropertyName("rows"); w.WriteStartArray();
                for (uint z = 0; z < footprint.Rows; z++)
                {
                    var row = new StringBuilder((int)footprint.Columns);
                    for (uint x = 0; x < footprint.Columns; x++)
                    {
                        var c = receipt.Cells.Span[(int)(z * footprint.Columns + x)];
                        row.Append(c.StaticCollision ? c.DynamicCollisionCount > 0 ? 'B' : '#' : c.DynamicCollisionCount > 0 ? 'D' : '.');
                    }
                    w.WriteStringValue(row.ToString());
                }
                w.WriteEndArray(); w.WriteEndObject();
            }
            w.WriteEndArray(); w.WriteEndObject();
        }
        return Encoding.UTF8.GetString(stream.ToArray());
    }

    private static void Point(Utf8JsonWriter w, string name, Vector3 p)
    {
        w.WritePropertyName(name); w.WriteStartArray(); w.WriteNumberValue(p.X); w.WriteNumberValue(p.Y); w.WriteNumberValue(p.Z); w.WriteEndArray();
    }
}
