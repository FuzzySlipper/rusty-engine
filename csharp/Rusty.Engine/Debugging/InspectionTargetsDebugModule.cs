using System.Numerics;
using System.Text.Json;

namespace Rusty.Engine.Debugging;

/// <summary>
/// One named point a product offers inspection: a grip, a probe, a reach target, a muzzle. It is
/// placed like an overlay item, so one on a joint or in camera space follows what it is on.
/// </summary>
/// <remarks>A name is one word (no spaces), as commands name it.</remarks>
public sealed record InspectionTarget(string Name, OverlayPlace Place, Vector3 Point)
{
    /// <summary>The frame framings look along: front from its +Z, side from +X, top from +Y.</summary>
    public Quaternion Rotation { get; init; } = Quaternion.Identity;
    /// <summary>How far around the point a framing shows, in metres.</summary>
    public float Radius { get; init; } = 0.1f;
    /// <summary>The framing a pane offers first (front, back, left, right, side, top, underside, close).</summary>
    public string? Framing { get; init; }
    public Color Color { get; init; } = new(1, 0.85f, 0.2f, 1);
}

/// <summary>
/// Serves inspection targets from the product's own explicit list: <c>inspect.targets</c> answers
/// the <c>inspect.targets.v1</c> list and publishes the targets as the overlay set
/// <see cref="OverlaySet"/>, a marker, axes and a label each, which
/// <c>engine.inspect.overlay show set inspect.targets</c> draws. The Engine frames inspection
/// views on them (<c>engine.inspect.view frame ID FRAMING target NAME</c>); the product never
/// positions an inspection camera. Register one instance with the debug catalog.
/// </summary>
public sealed class InspectionTargetsDebugModule(IGraphicsService graphics, Func<IReadOnlyList<InspectionTarget>> targets) : IDebugCommandModule
{
    public const string Contract = "inspect.targets.v1";
    public const string OverlaySet = "inspect.targets";
    /// <summary>A marker's size and its axes' length, as a share of the target's radius.</summary>
    private const float MarkerShare = 0.5f;
    private readonly IGraphicsService _graphics = graphics ?? throw new ArgumentNullException(nameof(graphics));
    private readonly Func<IReadOnlyList<InspectionTarget>> _targets = targets ?? throw new ArgumentNullException(nameof(targets));

    [DebugCommand("inspect.targets", Description = "The inspect.targets.v1 list of the product's inspection targets (name, place, point, frame, radius, framing); publishes them as the overlay set inspect.targets.")]
    public DebugCommandResult List()
    {
        IReadOnlyList<InspectionTarget> list = _targets();
        List<OverlayItem> items = new(list.Count * 3);
        foreach (InspectionTarget target in list)
        {
            float size = target.Radius * MarkerShare;
            items.Add(OverlayItem.Marker(target.Place, target.Point, size, target.Color, onTop: true));
            items.Add(OverlayItem.Axes(target.Place, target.Point, target.Rotation, size, onTop: true));
            items.Add(OverlayItem.Label(target.Place, target.Point + new Vector3(0, size, 0), target.Name, target.Color, onTop: true));
        }
        _graphics.PublishOverlaySet(new(OverlaySet, [.. items]));
        return DebugCommandResult.Success(Json(list));
    }

    private static string Json(IReadOnlyList<InspectionTarget> list)
    {
        using MemoryStream stream = new();
        using (Utf8JsonWriter writer = new(stream))
        {
            writer.WriteStartObject();
            writer.WriteString("contract", Contract);
            writer.WritePropertyName("targets");
            writer.WriteStartArray();
            foreach (InspectionTarget target in list)
            {
                writer.WriteStartObject();
                writer.WriteString("name", target.Name);
                writer.WritePropertyName("space");
                writer.WriteStartObject();
                writer.WriteString("kind", target.Place.Space switch
                {
                    OverlaySpace.World => "world",
                    OverlaySpace.Model => "model",
                    OverlaySpace.Joint => "joint",
                    _ => "camera",
                });
                if (target.Place.Space is OverlaySpace.Model or OverlaySpace.Joint) writer.WriteNumber("entity", target.Place.Entity);
                if (target.Place.Space is OverlaySpace.Joint) writer.WriteNumber("joint", target.Place.Joint);
                writer.WriteEndObject();
                Numbers(writer, "point", [target.Point.X, target.Point.Y, target.Point.Z]);
                Numbers(writer, "rotation", [target.Rotation.X, target.Rotation.Y, target.Rotation.Z, target.Rotation.W]);
                writer.WriteNumber("radius", target.Radius);
                if (target.Framing is { } framing) writer.WriteString("framing", framing);
                writer.WriteEndObject();
            }
            writer.WriteEndArray();
            writer.WriteEndObject();
        }
        return System.Text.Encoding.UTF8.GetString(stream.ToArray());
    }

    private static void Numbers(Utf8JsonWriter writer, string name, ReadOnlySpan<float> values)
    {
        writer.WritePropertyName(name);
        writer.WriteStartArray();
        foreach (float value in values) writer.WriteNumberValue(value);
        writer.WriteEndArray();
    }
}
