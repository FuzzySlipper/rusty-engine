using System.Numerics;

namespace Rusty.Engine.FirstPerson;

/// <summary>
/// The first-person helpers' frames and degree convention. Held things are placed in camera space: metres right (+X),
/// up (+Y) and back (+Z), the view looking down -Z. An authored turn is degrees about X, Y and Z, applied as
/// <see cref="Quaternion.CreateFromYawPitchRoll"/> does (yaw about Y, then pitch about X, then roll about Z). The off
/// (left) hand holds what the main (right) hand would, seen in the mirror across the view's centre line.
/// </summary>
public static class HeldFrames
{
    /// <summary>Degrees about X, Y and Z as a turn.</summary>
    public static Quaternion Turn(Vector3 degrees)
    {
        Vector3 d = degrees * (MathF.PI / 180);
        return Quaternion.CreateFromYawPitchRoll(d.Y, d.X, d.Z);
    }

    /// <summary>Degrees about X, Y and Z that <see cref="Turn"/> turns back into <paramref name="turn"/>.</summary>
    public static Vector3 Degrees(Quaternion turn)
    {
        // The rotation's columns are the images of the axes; for R = Ry Rx Rz, R12 = -sin x, R02 / R22 = tan y and
        // R10 / R11 = tan z.
        Vector3 cx = Vector3.Transform(Vector3.UnitX, turn), cy = Vector3.Transform(Vector3.UnitY, turn), cz = Vector3.Transform(Vector3.UnitZ, turn);
        return new Vector3(MathF.Asin(Math.Clamp(-cz.Y, -1, 1)), MathF.Atan2(cz.X, cz.Z), MathF.Atan2(cx.Y, cy.Y)) * (180 / MathF.PI);
    }

    /// <summary>The rotation taking <paramref name="tip"/> to <paramref name="toward"/>, and <paramref name="up"/> as near <paramref name="upToward"/> as it can.</summary>
    public static Quaternion Between(Vector3 tip, Vector3 up, Vector3 toward, Vector3 upToward)
    {
        (Vector3 a, Vector3 b, Vector3 c) = Basis(tip, up);
        (Vector3 x, Vector3 y, Vector3 z) = Basis(toward, upToward);
        // Each axis goes where its share of the source basis says, rebuilt on the target basis.
        Vector3 Image(Vector3 v) => x * Vector3.Dot(v, a) + y * Vector3.Dot(v, b) + z * Vector3.Dot(v, c);
        Vector3 ix = Image(Vector3.UnitX), iy = Image(Vector3.UnitY), iz = Image(Vector3.UnitZ);
        // System.Numerics matrices carry each basis vector's image as a row.
        return Quaternion.Normalize(Quaternion.CreateFromRotationMatrix(new(ix.X, ix.Y, ix.Z, 0, iy.X, iy.Y, iy.Z, 0, iz.X, iz.Y, iz.Z, 0, 0, 0, 0, 1)));
    }

    /// <summary>A camera-space point seen in the mirror across the view's centre line (X negated).</summary>
    public static Vector3 Mirror(Vector3 point) => point with { X = -point.X };

    /// <summary>
    /// A turn seen in the same mirror: conjugating by the reflection keeps rotations about X and flips those about Y and
    /// Z. It preserves composition, so an offset between mirrored turns is the mirrored offset.
    /// </summary>
    public static Quaternion Mirror(Quaternion turn) => new(turn.X, -turn.Y, -turn.Z, turn.W);

    private static (Vector3, Vector3, Vector3) Basis(Vector3 along, Vector3 upHint)
    {
        Vector3 a = Vector3.Normalize(along);
        Vector3 b = Vector3.Normalize(upHint - a * Vector3.Dot(upHint, a));
        return (a, b, Vector3.Cross(a, b));
    }
}

/// <summary>
/// Where a kind of held thing sits at rest in the main hand (camera space): where its grip sits, a point its length
/// heads toward, and the way its up side faces. Every look seated on one stance sits alike.
/// </summary>
public sealed record HeldStance(Vector3 Grip, Vector3 Toward, Vector3 Up)
{
    public Vector3 Grip { get; set; } = Grip;
    public Vector3 Toward { get; set; } = Toward;
    public Vector3 Up { get; set; } = Up;

    /// <summary>The turn that lays a look's length (-Z) toward <see cref="Toward"/> with its up side (+Y) facing <see cref="Up"/>.</summary>
    public Quaternion Turn() => HeldFrames.Between(-Vector3.UnitZ, Vector3.UnitY, Vector3.Normalize(Toward - Grip), Up);

    /// <summary>
    /// Where a look is seated on this stance after its own nudge: its grip's place and its turn, the nudge's move added
    /// to the stance's grip and its turn (degrees) applied after the stance's.
    /// </summary>
    public (Vector3 Grip, Quaternion Turn) Seat(Vector3 nudgeGrip, Vector3 nudgeTurn) =>
        (Grip + nudgeGrip, Quaternion.Normalize(HeldFrames.Turn(nudgeTurn) * Turn()));

    /// <summary>What is wrong with this stance, if anything: a toward point on the grip, or an up side along the length.</summary>
    public IEnumerable<HeldProblem> Problems()
    {
        Vector3 along = Toward - Grip;
        if (along.Length() <= 0.05f) yield return new("toward", "must lie away from the grip.");
        else if (Vector3.Cross(Vector3.Normalize(along), Up).Length() <= 0.1f) yield return new("up", "must not lie along the item's length.");
    }
}

/// <summary>
/// Something wrong with first-person content, at a field relative to the value checked (for example
/// <c>tracks.slash[2].pose</c>); the product names its file and reports it its own way.
/// </summary>
public readonly record struct HeldProblem(string Field, string Message)
{
    /// <summary>The same problem one level down, under <paramref name="parent"/>.</summary>
    public HeldProblem Under(string parent) => this with { Field = Field.Length == 0 ? parent : Field.StartsWith('[') ? parent + Field : $"{parent}.{Field}" };
}
