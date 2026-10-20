using System.Numerics;
using System.Text.Json.Serialization;

namespace Rusty.Engine.FirstPerson;

/// <summary>
/// Points on a held look (look space, as its grips) that a correct hold puts parts of the hand on, each optional: the
/// trigger (the main index finger's pad), the back strap's top under the hammer (the main hand's web), the butt (the main
/// little finger) and the fore-end (the off palm, on a look held in both hands). <see cref="GripLandmarks"/> measures the
/// drawn hands against them.
/// </summary>
public sealed record HeldLandmarks(Vector3? Trigger = null, Vector3? BackstrapTop = null, Vector3? Butt = null, Vector3? ForeEnd = null)
{
    public Vector3? Trigger { get; set; } = Trigger;
    public Vector3? BackstrapTop { get; set; } = BackstrapTop;
    public Vector3? Butt { get; set; } = Butt;
    public Vector3? ForeEnd { get; set; } = ForeEnd;

    /// <summary>The landmarks given, by their field name.</summary>
    [JsonIgnore]
    public IEnumerable<(string Field, Vector3 Point)> Named =>
        new[] { ("trigger", Trigger), ("backstrapTop", BackstrapTop), ("butt", Butt), ("foreEnd", ForeEnd) }
            .Where(p => p.Item2 is not null).Select(p => (p.Item1, p.Item2!.Value));

    /// <summary>What is wrong with these landmarks, if anything; <paramref name="offGrip"/> says whether the look has an off grip.</summary>
    public IEnumerable<HeldProblem> Problems(bool offGrip)
    {
        if (ForeEnd is not null && !offGrip) yield return new("foreEnd", "is where the off hand holds: the look needs an off grip.");
    }

    /// <summary>Takes another set's points in place.</summary>
    public void Take(HeldLandmarks fresh) => (Trigger, BackstrapTop, Butt, ForeEnd) = (fresh.Trigger, fresh.BackstrapTop, fresh.Butt, fresh.ForeEnd);
}

/// <summary>
/// Where the grip check finds a hand's own points and how near a look's landmarks they must come: a pad past a digit's
/// last joint by <see cref="Pad"/> of the bone before it; the web <see cref="Web"/> of the way from the thumb's base to
/// the index finger's; the palm's centre <see cref="Palm"/> of the way from the wrist to the middle finger's base and
/// <see cref="PalmDepth"/> metres out of the palm; then the tolerances: metres from the trigger, back strap, butt and
/// fore-end, and degrees between a palm's normal and what it should face.
/// </summary>
public sealed record HandLandmarkTuning
{
    public required float Pad { get; set; }
    public required float Web { get; set; }
    public required float Palm { get; set; }
    public required float PalmDepth { get; set; }
    public required float Trigger { get; set; }
    public required float Backstrap { get; set; }
    public required float Butt { get; set; }
    public required float ForeEnd { get; set; }
    public required float Facing { get; set; }

    /// <summary>What is wrong with this tuning, if anything.</summary>
    public IEnumerable<HeldProblem> Problems()
    {
        foreach (var (name, value) in new[] { ("pad", Pad), ("web", Web), ("palm", Palm), ("trigger", Trigger), ("backstrap", Backstrap),
            ("butt", Butt), ("foreEnd", ForeEnd), ("facing", Facing) })
            if (value <= 0) yield return new(name, "must be positive.");
    }

    /// <summary>Takes another tuning's values in place.</summary>
    public void Take(HandLandmarkTuning fresh) =>
        (Pad, Web, Palm, PalmDepth, Trigger, Backstrap, Butt, ForeEnd, Facing) =
        (fresh.Pad, fresh.Web, fresh.Palm, fresh.PalmDepth, fresh.Trigger, fresh.Backstrap, fresh.Butt, fresh.ForeEnd, fresh.Facing);
}

/// <summary>
/// The grip check: which of the drawn hands' own points must meet which of a held look's landmarks, published as Engine
/// landmark pairs in an overlay set. The Engine measures them on the poses it draws (<c>engine.inspect.landmarks</c>
/// with the set's name, or <see cref="Read"/>), green within tolerance and red outside while inspection shows the set.
/// </summary>
public sealed class GripLandmarks(IEngineContext engine, string set)
{
    // What the published pairs were made from; they are published again only when it changes.
    private string published = "";

    /// <summary>The overlay set the pairs are published in.</summary>
    public string Set => set;

    /// <summary>
    /// Publishes the pairs for a look in the main hand (drawn on object <paramref name="held"/>, its model fitted into look
    /// space by <paramref name="fit"/>, held at <paramref name="mainGrip"/>, look space) against the drawn arms' main hand
    /// and, when <paramref name="offHolds"/>, the off hand on its fore-end; or removes them when it has no landmarks.
    /// </summary>
    public void Publish(HeldLandmarks? marks, Vector3 mainGrip, ulong held, Transform fit, FirstPersonArms arms, bool offHolds, HandLandmarkTuning tuning)
    {
        string now = marks is null ? "" : string.Join("|", held, arms.Object, offHolds, mainGrip, fit.Translation, fit.Rotation, fit.Scale, tuning,
            string.Join(";", marks.Named.Select(m => $"{m.Field}:{m.Point}")));
        if (now == published) return;
        published = now;
        engine.Graphics.PublishOverlaySet(new(set, Array.Empty<OverlayItem>(), marks is null ? [] : Pairs(marks, mainGrip, held, fit, arms, offHolds, tuning)));
    }

    /// <summary>The pairs as the Engine last measured them on the drawn hands.</summary>
    public LandmarkReadResult Read() => engine.Graphics.ReadLandmarks(set);

    private static LandmarkPair[] Pairs(HeldLandmarks marks, Vector3 mainGrip, ulong held, Transform fit, FirstPersonArms arms, bool offHolds, HandLandmarkTuning tuning)
    {
        // Look space into the drawn model's own: the fit's move, turn and scale undone. Offsets read along look space.
        Quaternion unturn = Quaternion.Inverse(fit.Rotation);
        Vector3 Model(Vector3 point) => Vector3.Transform(point - fit.Translation, unturn) / fit.Scale;
        Vector3 Direction(Vector3 along) => Vector3.Normalize(Vector3.Transform(along, unturn) / fit.Scale);
        OverlayPlace place = OverlayPlace.Model(held);
        Landmark At(Vector3 point, Vector3 axis = default) => Landmark.At(place, Model(point), axis, unturn);
        HandRig main = arms.Hand(false);
        List<LandmarkPair> pairs = [];
        if (marks.Trigger is { } trigger)
            pairs.Add(LandmarkPair.Distance("trigger", main.Pad(Digit.Index, tuning.Pad), At(trigger), tuning.Trigger));
        if (marks.BackstrapTop is { } top)
            pairs.Add(LandmarkPair.Distance("backstrap", main.Web(tuning.Web), At(top), tuning.Backstrap));
        if (marks.Butt is { } butt)
            pairs.Add(LandmarkPair.Distance("butt", main.Joint(Digit.Pinky, 1, default), At(butt), tuning.Butt));
        // The handle runs through the main grip from the back strap's top toward the butt (down the look without a
        // butt); the palm wraps it, facing the nearest point of that line.
        if (marks.BackstrapTop is { } from)
        {
            Vector3 along = marks.Butt is { } toward ? toward - from : -Vector3.UnitY;
            if (along.LengthSquared() > 1e-10f)
                pairs.Add(LandmarkPair.Facing("main-facing", main.Palm(tuning.Palm, tuning.PalmDepth), At(mainGrip, Direction(along)), LandmarkAim.Line, tuning.Facing, 0));
        }
        if (offHolds && marks.ForeEnd is { } foreEnd)
        {
            Landmark palm = arms.Hand(true).Palm(tuning.Palm, tuning.PalmDepth);
            pairs.Add(LandmarkPair.Distance("fore-end", palm, At(foreEnd), tuning.ForeEnd));
            pairs.Add(LandmarkPair.Facing("off-facing", palm, At(foreEnd), LandmarkAim.Point, tuning.Facing, 0));
        }
        return [.. pairs];
    }

    // What each pair's ends are, in a report's words: the hand's point and the look's landmark.
    private static readonly Dictionary<string, (string Hand, string Landmark)> Words = new(StringComparer.Ordinal)
    {
        ["trigger"] = ("main index pad", "trigger"),
        ["backstrap"] = ("main web", "back strap's top"),
        ["butt"] = ("main little finger", "butt"),
        ["main-facing"] = ("main", "the handle"),
        ["fore-end"] = ("off palm", "fore-end"),
        ["off-facing"] = ("off", "the fore-end"),
    };

    /// <summary>
    /// A report line for one pair as the Engine measured it: within or off, the distance (or a palm's facing angle) and
    /// its tolerance, and where the hand's point lies from the landmark along the look's own axes.
    /// </summary>
    public static string Line(LandmarkMeasure pair)
    {
        (string hand, string landmark) = Words.TryGetValue(pair.Name, out var words) ? words : (pair.Name, "its landmark");
        if (!pair.Drawn) return $"--  {hand}: not drawn";
        string mark = pair.Within ? "ok " : "OFF";
        if (pair.Aim != LandmarkAim.None)
            return FormattableString.Invariant($"{mark} {hand} palm faces {landmark} {(pair.HasAngle ? $"{pair.Angle:0}°" : "(no angle)")} off (within {pair.AngleTolerance:0})");
        static string Part(float v, string plus, string minus) => FormattableString.Invariant($"{MathF.Abs(v) * 1000:0} {(v >= 0 ? plus : minus)}");
        Vector3 d = pair.Offset;
        return FormattableString.Invariant($"{mark} {hand} {pair.Distance * 1000:0.0} mm from the {landmark} (within {pair.Tolerance * 1000:0}): ") +
            $"{Part(d.Y, "above", "below")}, {Part(d.Z, "behind", "ahead")}, {Part(d.X, "right", "left")}";
    }
}
