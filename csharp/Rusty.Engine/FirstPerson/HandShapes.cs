using System.Numerics;
using System.Text.Json.Serialization;

namespace Rusty.Engine.FirstPerson;

/// <summary>
/// Degrees for one hand's digits: each finger's three joints' curl (the index, middle, ring and little fingers, base to
/// tip; positive toward the palm) and each thumb joint's [curl, swing]. As a look's shape it is added over the arms'
/// canonical holding pose (<see cref="ArmDigits.Pose"/>, where each rig's <see cref="ArmRig.Grip"/> brings its clip)
/// through each rig's digit frames, so one shape means the same hand on every rig. The same terms give the holding pose
/// itself and a drawn hand's knuckles (<see cref="HandKnuckles"/>).
/// </summary>
public sealed record HandShape(float[][] Fingers, float[][] Thumb)
{
    /// <summary>A shape that changes nothing.</summary>
    public static HandShape None => new([[0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0]], [[0, 0], [0, 0], [0, 0]]);

    /// <summary>What is wrong with this shape, if anything: four fingers of three joints and three thumb joints of two angles.</summary>
    public IEnumerable<HeldProblem> Problems()
    {
        if (Fingers.Length != 4 || Fingers.Any(f => f.Length != 3))
            yield return new("fingers", "gives the index, middle, ring and little fingers' three joints' curl each, in degrees.");
        if (Thumb.Length != 3 || Thumb.Any(j => j.Length != 2))
            yield return new("thumb", "gives three thumb joints' [curl, swing], in degrees.");
    }

    /// <summary>Takes another shape's angles in place (live controls hold these arrays).</summary>
    public void Take(HandShape fresh)
    {
        for (int i = 0; i < 4; i++) fresh.Fingers[i].CopyTo(Fingers[i], 0);
        for (int i = 0; i < 3; i++) fresh.Thumb[i].CopyTo(Thumb[i], 0);
    }
}

/// <summary>
/// How each hand's digits sit on a look, or a rig's grip toward the holding pose: the main hand, and the off hand when
/// it differs (the off grip of a look held in both hands, or a rig whose hands' clips differ). A look in the off hand
/// alone takes its main shape, mirrored as the hand is.
/// </summary>
public sealed record HeldShape(HandShape Main, HandShape? Off = null)
{
    /// <summary>What is wrong with this shape, if anything; <paramref name="offAllowed"/> says whether an off shape may be given.</summary>
    public IEnumerable<HeldProblem> Problems(bool offAllowed)
    {
        foreach (HeldProblem problem in Main.Problems()) yield return problem.Under("main");
        if (Off is { } off)
        {
            if (!offAllowed) yield return new("off", "is only given for a look with an off grip.");
            foreach (HeldProblem problem in off.Problems()) yield return problem.Under("off");
        }
    }

    /// <summary>Takes another shape's angles in place (an off shape added or removed is not taken).</summary>
    public void Take(HeldShape fresh)
    {
        Main.Take(fresh.Main);
        if (Off is { } off && fresh.Off is { } next) off.Take(next);
    }
}

/// <summary>
/// Degrees added to each finger joint's curl (base to tip, the same for every finger; positive toward the palm) and each
/// thumb joint's [curl, swing]: how far a hand closes or opens from its grip.
/// </summary>
public sealed record DigitBend(float[] Fingers, float[][] Thumb)
{
    /// <summary>What is wrong with this bend, if anything.</summary>
    public IEnumerable<HeldProblem> Problems()
    {
        if (Fingers.Length != 3) yield return new("fingers", "gives three finger joints' curl, in degrees.");
        if (Thumb.Length != 3 || Thumb.Any(j => j.Length != 2)) yield return new("thumb", "gives three thumb joints' [curl, swing], in degrees.");
    }

    /// <summary>Takes another bend's angles in place.</summary>
    public void Take(DigitBend fresh)
    {
        fresh.Fingers.CopyTo(Fingers, 0);
        for (int i = 0; i < 3; i++) fresh.Thumb[i].CopyTo(Thumb[i], 0);
    }
}

/// <summary>
/// A hand's digits at full closure (<see cref="Close"/>) and fully open (<see cref="Open"/>), and the canonical holding
/// pose (<see cref="Pose"/>): every hand's knuckle angles at its grip, before a look's shape.
/// </summary>
public sealed record ArmDigits(DigitBend Close, DigitBend Open, HandShape Pose)
{
    /// <summary>What is wrong with these digits, if anything.</summary>
    public IEnumerable<HeldProblem> Problems() =>
        Close.Problems().Select(p => p.Under("close")).Concat(Open.Problems().Select(p => p.Under("open"))).Concat(Pose.Problems().Select(p => p.Under("pose")));

    /// <summary>Takes other digits' angles in place.</summary>
    public void Take(ArmDigits fresh)
    {
        Close.Take(fresh.Close);
        Open.Take(fresh.Open);
        Pose.Take(fresh.Pose);
    }
}

/// <summary>
/// The hand's canonical frame, which every rig's hands are measured into so holds and shapes suit any arms model: +Y
/// along the fingers (toward the middle knuckle), +Z out of the back of the hand, and +X toward the little finger on the
/// right hand (toward the thumb on the left).
/// </summary>
public static class HandFrame
{
    /// <summary>
    /// The turn from a hand joint's own frame to the hand's canonical frame, from where the index, middle and little
    /// knuckles sit in the hand joint's frame.
    /// </summary>
    public static Quaternion Measure(Vector3 index, Vector3 middle, Vector3 little, bool left)
    {
        Vector3 y = Vector3.Normalize(middle);
        Vector3 across = left ? index - little : little - index;
        Vector3 x = Vector3.Normalize(across - y * Vector3.Dot(across, y));
        Vector3 z = Vector3.Cross(x, y);
        // The canonical axes as columns in the joint's frame; its inverse takes the canonical frame to the joint's.
        Quaternion canonical = Quaternion.CreateFromRotationMatrix(new(x.X, x.Y, x.Z, 0, y.X, y.Y, y.Z, 0, z.X, z.Y, z.Z, 0, 0, 0, 0, 1));
        return Quaternion.Normalize(Quaternion.Inverse(canonical));
    }
}

/// <summary>
/// A drawn hand's knuckle angles, read from its joints in the hand's canonical frame, in a <see cref="HandShape"/>'s
/// terms: each finger joint's curl (the bend from the previous bone, or from +Y at the knuckle, positive toward the
/// palm) and each thumb joint's [curl, swing] (its bone's rise out of the palm toward it, and its turn about the palm's
/// normal from +Y toward the thumb side, each from the previous bone's). A rig's grip is the holding pose less its
/// clip's knuckles (<see cref="Difference"/>).
/// </summary>
public static class HandKnuckles
{
    private const float Degrees = 180 / MathF.PI;

    /// <param name="joints">The drawn joints (model space).</param>
    /// <param name="canonical">The hand's canonical frame in model space.</param>
    /// <param name="fingers">The index, middle, ring and little fingers' joints, base to tip.</param>
    /// <param name="thumb">The thumb's joints, base to tip.</param>
    /// <param name="left">The left hand, whose swing is mirrored.</param>
    public static HandShape Read(ReadOnlySpan<JointPose> joints, Quaternion canonical, uint[][] fingers, uint[] thumb, bool left)
    {
        Quaternion unturn = Quaternion.Inverse(canonical);
        float side = left ? -1 : 1;
        float[][] curls = new float[fingers.Length][];
        for (int f = 0; f < fingers.Length; f++) curls[f] = Curls(Bones(joints, fingers[f], unturn));
        return new(curls, Thumb(Bones(joints, thumb, unturn), side));
    }

    /// <summary>The shape that takes one hand's angles to another's (<paramref name="to"/> less <paramref name="from"/>).</summary>
    public static HandShape Difference(HandShape to, HandShape from) =>
        new([.. to.Fingers.Zip(from.Fingers, (t, f) => t.Zip(f, (a, b) => Wrap(a - b)).ToArray())],
            [.. to.Thumb.Zip(from.Thumb, (t, f) => t.Zip(f, (a, b) => Wrap(a - b)).ToArray())]);

    /// <summary>One hand's angles as a shape's JSON (whole degrees), for pasting into content.</summary>
    public static string Json(HandShape hand) =>
        FormattableString.Invariant($"{{\"fingers\": [{string.Join(", ", hand.Fingers.Select(f => $"[{string.Join(", ", f.Select(Round))}]"))}], \"thumb\": [{string.Join(", ", hand.Thumb.Select(j => $"[{Round(j[0])}, {Round(j[1])}]"))}]}}");

    // Each joint's bone in the canonical frame: toward the next joint, and for the last (no tip joint is named) along
    // the axis the previous bone runs in its own joint's frame, turned as the last joint is.
    private static Vector3[] Bones(ReadOnlySpan<JointPose> joints, uint[] chain, Quaternion unturn)
    {
        Vector3 a0 = joints[(int)chain[0]].Model.Translation, a1 = joints[(int)chain[1]].Model.Translation, a2 = joints[(int)chain[2]].Model.Translation;
        Quaternion t1 = joints[(int)chain[1]].Model.Rotation, t2 = joints[(int)chain[2]].Model.Rotation;
        Vector3 along = Vector3.Normalize(Vector3.Transform(a2 - a1, Quaternion.Inverse(t1)));
        Vector3[] bones = [a1 - a0, a2 - a1, Vector3.Transform(along, t2)];
        return [.. bones.Select(b => Vector3.Normalize(Vector3.Transform(b, unturn)))];
    }

    // A finger curls about the knuckle line (X): from +Y toward the palm (-Z).
    private static float[] Curls(Vector3[] bones)
    {
        float before = 0;
        float[] curls = new float[3];
        for (int i = 0; i < 3; i++)
        {
            float pitch = MathF.Atan2(-bones[i].Z, bones[i].Y) * Degrees;
            curls[i] = Wrap(pitch - before);
            before = pitch;
        }
        return curls;
    }

    // A thumb rises out of the palm's plane toward the palm (-Z) and swings in it from +Y toward the thumb (-X on the right).
    private static float[][] Thumb(Vector3[] bones, float side)
    {
        (float Rise, float Swing) before = (0, 0);
        float[][] joints = new float[bones.Length][];
        for (int i = 0; i < bones.Length; i++)
        {
            Vector3 bone = bones[i];
            (float Rise, float Swing) now = (MathF.Asin(Math.Clamp(-bone.Z, -1, 1)) * Degrees, MathF.Atan2(-bone.X, bone.Y) * Degrees * side);
            joints[i] = [Wrap(now.Rise - before.Rise), Wrap(now.Swing - before.Swing)];
            before = now;
        }
        return joints;
    }

    private static float Wrap(float degrees) => degrees > 180 ? degrees - 360 : degrees <= -180 ? degrees + 360 : degrees;

    private static float Round(float degrees) => MathF.Round(degrees) is var r && r == 0 ? 0 : r;
}

/// <summary>
/// How a rig's fingers curl. <c>Native</c>: about each finger joint's own X. <c>Measured</c>: about the knuckle line
/// measured from the drawn hand, for rigs authored any other way. A thumb is aimed on either: each bone goes where its
/// [curl, swing] reads (<see cref="HandKnuckles"/>), whatever its joints' frames.
/// </summary>
[JsonConverter(typeof(JsonStringEnumConverter<DigitFrames>))]
public enum DigitFrames { Native, Measured }
