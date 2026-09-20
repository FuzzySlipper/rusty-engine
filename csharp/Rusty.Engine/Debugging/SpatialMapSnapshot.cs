using System.Buffers;
using System.Globalization;
using System.Numerics;
using System.Text;
using System.Text.Json;

namespace Rusty.Engine.Debugging;

/// <summary>
/// Product-supplied observation facts that give an omniscient spatial-map readout useful
/// context without making the map query depend on product state.
/// </summary>
public readonly record struct SpatialMapObservation(
    string Stamp,
    Vector3 PlayerPosition,
    Vector3 PlayerFacing);

/// <summary>
/// A product-owned annotation for a selected entity. The stable ID is used only for a
/// deterministic debug glyph and remains the authoritative legend/JSON identity.
/// </summary>
public readonly record struct SpatialMapAnnotation(
    string StableId,
    string Label,
    string Relation,
    string State,
    Vector3 WorldPosition);

/// <summary>Copied geometry of a spatial-map query. It deliberately has no session or collider input.</summary>
public readonly record struct SpatialMapGeometry(
    Vector3 Origin,
    double CellSize,
    uint Columns,
    uint Rows,
    double CollisionMinY,
    double CollisionMaxY,
    double NavigationMinY,
    double NavigationMaxY);

/// <summary>A retained annotation and its map cell after the bounded snapshot capture.</summary>
public readonly record struct SpatialMapPlacedAnnotation(
    SpatialMapAnnotation Annotation,
    uint Column,
    uint Row,
    char Glyph);

/// <summary>Counts annotations excluded from a captured spatial-map snapshot.</summary>
public readonly record struct SpatialMapAnnotationSummary(
    int InBounds,
    int HorizontalOutOfView,
    int VerticalOutOfView,
    int Truncated,
    int MaximumRetained)
{
    public int OutOfView => HorizontalOutOfView + VerticalOutOfView;
}

/// <summary>
/// An immutable, omniscient formatting snapshot of one coherent <see cref="ISpatialService.ReadMap"/>
/// result. It is a debugger convenience over the named Engine query: it neither owns a spatial
/// world nor derives collision or navigation facts in managed code.
/// </summary>
public sealed class SpatialMapSnapshot
{
    private const string EntityGlyphAlphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

    private readonly SpatialMapCell[] _cells;
    private readonly SpatialMapPlacedAnnotation[] _annotations;

    private SpatialMapSnapshot(
        SpatialMapGeometry geometry,
        SpatialMapObservation observation,
        SpatialMapLeaseReceipt receipt,
        SpatialMapCell[] cells,
        SpatialMapPlacedAnnotation[] annotations,
        SpatialMapAnnotationSummary annotationSummary)
    {
        Geometry = geometry;
        Observation = observation;
        ProjectionIdentity = receipt.ProjectionIdentity;
        SourceRevision = receipt.SourceRevision;
        CollisionRevision = receipt.CollisionRevision;
        NavigationRevision = receipt.NavigationRevision;
        NavigationPresent = receipt.NavigationPresent;
        _cells = cells;
        _annotations = annotations;
        AnnotationSummary = annotationSummary;
    }

    /// <summary>Query geometry whose origin is the minimum X/Z corner; columns advance +X and rows +Z.</summary>
    public SpatialMapGeometry Geometry { get; }

    /// <summary>Product-supplied observation context captured with this readout.</summary>
    public SpatialMapObservation Observation { get; }

    public ulong ProjectionIdentity { get; }
    public ulong SourceRevision { get; }
    public ulong CollisionRevision { get; }
    public ulong NavigationRevision { get; }
    public bool NavigationPresent { get; }

    /// <summary>Copied cells in row-major order: index = row * columns + column.</summary>
    public ReadOnlyMemory<SpatialMapCell> Cells => _cells;

    /// <summary>Bounded, copied product annotations which intersect the requested horizontal and vertical view.</summary>
    public ReadOnlyMemory<SpatialMapPlacedAnnotation> Annotations => _annotations;

    public SpatialMapAnnotationSummary AnnotationSummary { get; }

    /// <summary>
    /// Performs one coherent Engine map read and copies its receipt before formatting it. The
    /// request's session and collider memory are call-only inputs and are never retained.
    /// </summary>
    public static SpatialMapSnapshot Capture(
        ISpatialService spatial,
        SpatialMapRequest request,
        SpatialMapObservation observation,
        ReadOnlySpan<SpatialMapAnnotation> annotations,
        int maximumAnnotations)
    {
        ArgumentNullException.ThrowIfNull(spatial);
        ValidateRequest(request);
        ValidateObservation(observation);
        if (maximumAnnotations < 0)
        {
            throw new ArgumentOutOfRangeException(nameof(maximumAnnotations));
        }

        SpatialMapLeaseReceipt receipt = spatial.ReadMap(request);
        int expectedCellCount = checked((int)((ulong)request.Columns * request.Rows));
        if (receipt.Cells.Length != expectedCellCount)
        {
            throw new InvalidOperationException(
                $"Spatial map returned {receipt.Cells.Length} cells for {request.Columns} by {request.Rows} geometry.");
        }

        // The receipt is a lease-shaped generated value. Retain a managed copy so either
        // formatter is valid after the generated service has released its native scratch data.
        SpatialMapCell[] copiedCells = receipt.Cells.ToArray();
        SpatialMapGeometry geometry = new(
            request.Origin,
            request.CellSize,
            request.Columns,
            request.Rows,
            request.CollisionMinY,
            request.CollisionMaxY,
            request.NavigationMinY,
            request.NavigationMaxY);
        (SpatialMapPlacedAnnotation[] retained, SpatialMapAnnotationSummary summary) =
            CaptureAnnotations(geometry, annotations, maximumAnnotations);
        return new SpatialMapSnapshot(geometry, observation, receipt, copiedCells, retained, summary);
    }

    /// <summary>
    /// Renders collision, navigation, and entity facts as separate world-aligned layers. The
    /// view is omniscient rather than a first-person projection: right is +X and down is +Z.
    /// </summary>
    public string ToAscii()
    {
        var output = new StringBuilder();
        output.Append("spatial-map perspective=omniscient axes=+X-right,+Z-down origin=");
        AppendVector(output, Geometry.Origin);
        output.Append(" cellSize=").Append(Format(Geometry.CellSize));
        output.Append(" columns=").Append(Geometry.Columns).Append(" rows=").Append(Geometry.Rows).AppendLine();
        output.Append("observation=").Append(Observation.Stamp).Append(" player=");
        AppendVector(output, Observation.PlayerPosition);
        output.Append(" facing=");
        AppendVector(output, Observation.PlayerFacing);
        output.AppendLine();
        output.Append("collisionY=[").Append(Format(Geometry.CollisionMinY)).Append(',')
            .Append(Format(Geometry.CollisionMaxY)).Append("] navigationY=[")
            .Append(Format(Geometry.NavigationMinY)).Append(',').Append(Format(Geometry.NavigationMaxY))
            .Append("] navigationPresent=").Append(NavigationPresent).AppendLine();
        output.Append("projection=").Append(ProjectionIdentity).Append(" sourceRevision=").Append(SourceRevision)
            .Append(" collisionRevision=").Append(CollisionRevision).Append(" navigationRevision=")
            .Append(NavigationRevision).AppendLine();
        output.AppendLine("collision (#=occupied, blank=no hit in retained sources; not walkability):");
        AppendLayer(output, CollisionGlyph);
        output.AppendLine("navigation (.=allowed, x=disallowed, ?=unknown/no samples):");
        if (NavigationPresent) AppendLayer(output, NavigationGlyph);
        else output.AppendLine("unavailable: no navigation projection; all cells unknown");
        output.AppendLine("map (entities over collision; @=player, *=multiple, #=occupied, blank=no hit):");
        AppendLayer(output, EntityGlyph);
        output.Append("annotations inBounds=").Append(AnnotationSummary.InBounds)
            .Append(" horizontalOutOfView=").Append(AnnotationSummary.HorizontalOutOfView)
            .Append(" verticalOutOfView=").Append(AnnotationSummary.VerticalOutOfView)
            .Append(" truncated=").Append(AnnotationSummary.Truncated)
            .Append(" maximumRetained=").Append(AnnotationSummary.MaximumRetained).AppendLine();
        output.AppendLine("legend:");
        if (PlayerCell(out uint playerColumn, out uint playerRow))
        {
            output.Append("@ player cell=").Append(playerColumn).Append(',').Append(playerRow).AppendLine();
        }
        else
        {
            output.AppendLine("@ player outside horizontal map extent");
        }
        foreach (SpatialMapPlacedAnnotation annotation in _annotations)
        {
            output.Append(annotation.Glyph).Append(" id=").Append(annotation.Annotation.StableId)
                .Append(" label=").Append(annotation.Annotation.Label)
                .Append(" relation=").Append(annotation.Annotation.Relation)
                .Append(" state=").Append(annotation.Annotation.State)
                .Append(" cell=").Append(annotation.Column).Append(',').Append(annotation.Row)
                .Append(" world=");
            AppendVector(output, annotation.Annotation.WorldPosition);
            output.AppendLine();
        }
        return output.ToString();
    }

    /// <summary>Writes compact, reflection-free JSON suitable for NativeAOT debug responses.</summary>
    public string ToJson()
    {
        var destination = new ArrayBufferWriter<byte>();
        using (var writer = new Utf8JsonWriter(destination, new JsonWriterOptions { Indented = false }))
        {
            writer.WriteStartObject();
            writer.WriteString("kind", "spatial-map-snapshot");
            writer.WriteString("perspective", "omniscient");
            writer.WriteString("axes", "+X-right,+Z-down");
            writer.WritePropertyName("geometry");
            writer.WriteStartObject();
            writer.WritePropertyName("origin");
            WriteVector(writer, Geometry.Origin);
            writer.WriteNumber("cellSize", Geometry.CellSize);
            writer.WriteNumber("columns", Geometry.Columns);
            writer.WriteNumber("rows", Geometry.Rows);
            writer.WritePropertyName("collisionY");
            WriteInterval(writer, Geometry.CollisionMinY, Geometry.CollisionMaxY);
            writer.WritePropertyName("navigationY");
            WriteInterval(writer, Geometry.NavigationMinY, Geometry.NavigationMaxY);
            writer.WriteEndObject();
            writer.WritePropertyName("observation");
            writer.WriteStartObject();
            writer.WriteString("stamp", Observation.Stamp);
            writer.WritePropertyName("playerPosition");
            WriteVector(writer, Observation.PlayerPosition);
            writer.WritePropertyName("playerFacing");
            WriteVector(writer, Observation.PlayerFacing);
            writer.WriteEndObject();
            writer.WritePropertyName("revisions");
            writer.WriteStartObject();
            writer.WriteNumber("projection", ProjectionIdentity);
            writer.WriteNumber("source", SourceRevision);
            writer.WriteNumber("collision", CollisionRevision);
            writer.WriteNumber("navigation", NavigationRevision);
            writer.WriteEndObject();
            writer.WriteBoolean("navigationPresent", NavigationPresent);
            writer.WritePropertyName("cellFields");
            writer.WriteStartArray();
            writer.WriteStringValue("staticCollision");
            writer.WriteStringValue("dynamicCollisionCount");
            writer.WriteStringValue("firstDynamicEntity");
            writer.WriteStringValue("navigationSamples");
            writer.WriteStringValue("navigationAllowedSamples");
            writer.WriteStringValue("minimumSupportY");
            writer.WriteStringValue("maximumSupportY");
            writer.WriteEndArray();
            writer.WritePropertyName("cells");
            writer.WriteStartArray();
            foreach (SpatialMapCell cell in _cells)
            {
                writer.WriteStartArray();
                writer.WriteBooleanValue(cell.StaticCollision);
                writer.WriteNumberValue(cell.DynamicCollisionCount);
                writer.WriteNumberValue(cell.FirstDynamicEntity);
                writer.WriteNumberValue(cell.NavigationSamples);
                writer.WriteNumberValue(cell.NavigationAllowedSamples);
                WriteFiniteNumber(writer, cell.MinimumSupportY);
                WriteFiniteNumber(writer, cell.MaximumSupportY);
                writer.WriteEndArray();
            }
            writer.WriteEndArray();
            writer.WritePropertyName("annotationSummary");
            writer.WriteStartObject();
            writer.WriteNumber("inBounds", AnnotationSummary.InBounds);
            writer.WriteNumber("horizontalOutOfView", AnnotationSummary.HorizontalOutOfView);
            writer.WriteNumber("verticalOutOfView", AnnotationSummary.VerticalOutOfView);
            writer.WriteNumber("outOfView", AnnotationSummary.OutOfView);
            writer.WriteNumber("truncated", AnnotationSummary.Truncated);
            writer.WriteNumber("maximumRetained", AnnotationSummary.MaximumRetained);
            writer.WriteEndObject();
            writer.WritePropertyName("annotations");
            writer.WriteStartArray();
            foreach (SpatialMapPlacedAnnotation annotation in _annotations)
            {
                writer.WriteStartObject();
                writer.WriteString("id", annotation.Annotation.StableId);
                writer.WriteString("label", annotation.Annotation.Label);
                writer.WriteString("relation", annotation.Annotation.Relation);
                writer.WriteString("state", annotation.Annotation.State);
                writer.WriteString("glyph", annotation.Glyph.ToString());
                writer.WriteNumber("column", annotation.Column);
                writer.WriteNumber("row", annotation.Row);
                writer.WritePropertyName("worldPosition");
                WriteVector(writer, annotation.Annotation.WorldPosition);
                writer.WriteEndObject();
            }
            writer.WriteEndArray();
            writer.WriteEndObject();
        }
        return Encoding.UTF8.GetString(destination.WrittenSpan);
    }

    private static (SpatialMapPlacedAnnotation[] Retained, SpatialMapAnnotationSummary Summary) CaptureAnnotations(
        SpatialMapGeometry geometry,
        ReadOnlySpan<SpatialMapAnnotation> annotations,
        int maximumAnnotations)
    {
        var inBounds = new List<SpatialMapPlacedAnnotation>();
        int horizontalOutOfView = 0;
        int verticalOutOfView = 0;
        for (int index = 0; index < annotations.Length; index++)
        {
            SpatialMapAnnotation annotation = annotations[index];
            ValidateAnnotation(annotation, index);
            if (!TryGetCell(geometry, annotation.WorldPosition, out uint column, out uint row))
            {
                horizontalOutOfView++;
                continue;
            }
            if (!InVerticalView(geometry, annotation.WorldPosition.Y))
            {
                verticalOutOfView++;
                continue;
            }
            inBounds.Add(new SpatialMapPlacedAnnotation(annotation, column, row, GlyphFor(annotation.StableId)));
        }

        inBounds.Sort(static (left, right) => StringComparer.Ordinal.Compare(left.Annotation.StableId, right.Annotation.StableId));
        for (int index = 1; index < inBounds.Count; index++)
        {
            if (StringComparer.Ordinal.Equals(inBounds[index - 1].Annotation.StableId, inBounds[index].Annotation.StableId))
            {
                throw new ArgumentException($"Spatial map annotations contain duplicate stable ID '{inBounds[index].Annotation.StableId}'.", nameof(annotations));
            }
        }

        int retainedCount = Math.Min(inBounds.Count, maximumAnnotations);
        SpatialMapPlacedAnnotation[] retained = inBounds.Take(retainedCount).ToArray();
        return (retained, new SpatialMapAnnotationSummary(
            inBounds.Count,
            horizontalOutOfView,
            verticalOutOfView,
            inBounds.Count - retainedCount,
            maximumAnnotations));
    }

    private void AppendLayer(StringBuilder output, Func<uint, uint, char> glyph)
    {
        for (uint row = 0; row < Geometry.Rows; row++)
        {
            for (uint column = 0; column < Geometry.Columns; column++)
            {
                output.Append(glyph(column, row));
            }
            output.AppendLine();
        }
    }

    private char CollisionGlyph(uint column, uint row)
    {
        SpatialMapCell cell = CellAt(column, row);
        return cell.StaticCollision || cell.DynamicCollisionCount > 0 ? '#' : ' ';
    }

    private char NavigationGlyph(uint column, uint row)
    {
        SpatialMapCell cell = CellAt(column, row);
        if (cell.NavigationSamples == 0) return '?';
        return cell.NavigationAllowedSamples > 0 ? '.' : 'x';
    }

    private char EntityGlyph(uint column, uint row)
    {
        bool player = PlayerCell(out uint playerColumn, out uint playerRow) && playerColumn == column && playerRow == row;
        SpatialMapPlacedAnnotation? first = null;
        foreach (SpatialMapPlacedAnnotation annotation in _annotations)
        {
            if (annotation.Column != column || annotation.Row != row) continue;
            if (player || first is not null) return '*';
            first = annotation;
        }
        return player ? '@' : first?.Glyph ?? CollisionGlyph(column, row);
    }

    private SpatialMapCell CellAt(uint column, uint row) => _cells[checked((int)((ulong)row * Geometry.Columns + column))];

    private bool PlayerCell(out uint column, out uint row)
        => TryGetCell(Geometry, Observation.PlayerPosition, out column, out row);

    private static bool TryGetCell(SpatialMapGeometry geometry, Vector3 position, out uint column, out uint row)
    {
        column = 0;
        row = 0;
        if (!float.IsFinite(position.X) || !float.IsFinite(position.Z)) return false;
        double x = (position.X - geometry.Origin.X) / geometry.CellSize;
        double z = (position.Z - geometry.Origin.Z) / geometry.CellSize;
        if (x < 0 || z < 0 || x >= geometry.Columns || z >= geometry.Rows) return false;
        column = (uint)Math.Floor(x);
        row = (uint)Math.Floor(z);
        return true;
    }

    private static bool InVerticalView(SpatialMapGeometry geometry, float y)
        => float.IsFinite(y)
            && (y >= geometry.CollisionMinY && y <= geometry.CollisionMaxY
                || y >= geometry.NavigationMinY && y <= geometry.NavigationMaxY);

    private static char GlyphFor(string stableId)
    {
        uint hash = 2166136261;
        foreach (char character in stableId)
        {
            hash ^= character;
            hash *= 16777619;
        }
        return EntityGlyphAlphabet[(int)(hash % (uint)EntityGlyphAlphabet.Length)];
    }

    private static void ValidateRequest(SpatialMapRequest request)
    {
        if (!float.IsFinite(request.Origin.X) || !float.IsFinite(request.Origin.Y) || !float.IsFinite(request.Origin.Z)
            || !double.IsFinite(request.CellSize) || request.CellSize <= 0
            || request.Columns == 0 || request.Rows == 0 || (ulong)request.Columns * request.Rows > 1024
            || !double.IsFinite(request.CollisionMinY) || !double.IsFinite(request.CollisionMaxY)
            || !double.IsFinite(request.NavigationMinY) || !double.IsFinite(request.NavigationMaxY)
            || request.CollisionMinY >= request.CollisionMaxY || request.NavigationMinY > request.NavigationMaxY)
        {
            throw new ArgumentException("Spatial map geometry must be finite, ordered, and contain 1..1024 cells.", nameof(request));
        }
    }

    private static void ValidateObservation(SpatialMapObservation observation)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(observation.Stamp);
        if (!float.IsFinite(observation.PlayerPosition.X) || !float.IsFinite(observation.PlayerPosition.Y) || !float.IsFinite(observation.PlayerPosition.Z)
            || !float.IsFinite(observation.PlayerFacing.X) || !float.IsFinite(observation.PlayerFacing.Y) || !float.IsFinite(observation.PlayerFacing.Z))
        {
            throw new ArgumentException("Spatial map observation vectors must be finite.", nameof(observation));
        }
    }

    private static void ValidateAnnotation(SpatialMapAnnotation annotation, int index)
    {
        if (string.IsNullOrWhiteSpace(annotation.StableId))
        {
            throw new ArgumentException($"Spatial map annotation {index} has no stable ID.", nameof(annotation));
        }
        ArgumentNullException.ThrowIfNull(annotation.Label);
        ArgumentNullException.ThrowIfNull(annotation.Relation);
        ArgumentNullException.ThrowIfNull(annotation.State);
    }

    private static void AppendVector(StringBuilder output, Vector3 value)
        => output.Append('(').Append(Format(value.X)).Append(',').Append(Format(value.Y)).Append(',').Append(Format(value.Z)).Append(')');

    private static string Format(double value) => value.ToString("G17", CultureInfo.InvariantCulture);
    private static string Format(float value) => value.ToString("G9", CultureInfo.InvariantCulture);

    private static void WriteInterval(Utf8JsonWriter writer, double minimum, double maximum)
    {
        writer.WriteStartArray();
        WriteFiniteNumber(writer, minimum);
        WriteFiniteNumber(writer, maximum);
        writer.WriteEndArray();
    }

    private static void WriteVector(Utf8JsonWriter writer, Vector3 value)
    {
        writer.WriteStartArray();
        WriteFiniteNumber(writer, value.X);
        WriteFiniteNumber(writer, value.Y);
        WriteFiniteNumber(writer, value.Z);
        writer.WriteEndArray();
    }

    private static void WriteFiniteNumber(Utf8JsonWriter writer, double value)
    {
        if (double.IsFinite(value)) writer.WriteNumberValue(value);
        else writer.WriteNullValue();
    }
}
