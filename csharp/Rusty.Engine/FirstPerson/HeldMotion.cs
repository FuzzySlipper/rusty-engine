using System.Numerics;
using System.Text.Json.Serialization;

namespace Rusty.Engine.FirstPerson;

/// <summary>The phase of an action a motion key is timed through.</summary>
[JsonConverter(typeof(JsonStringEnumConverter<MotionPhase>))]
public enum MotionPhase { Windup, Commit, Recovery }

/// <summary>An action's phases in seconds: the windup, the commit (whose start is the strike) and the recovery.</summary>
public readonly record struct PhaseTiming(float Windup, float Commit, float Recovery)
{
    /// <summary>The whole action, windup to the end of recovery.</summary>
    public float Duration => Windup + Commit + Recovery;

    /// <summary>Seconds from the action's start to <paramref name="at"/> (0 to 1) through <paramref name="phase"/>.</summary>
    public float Time(MotionPhase phase, float at) => phase switch
    {
        MotionPhase.Windup => Windup * at,
        MotionPhase.Commit => Windup + Commit * at,
        _ => Windup + Commit + Recovery * at
    };
}

/// <summary>
/// A pose of a held look, from where its stance seats it: its grip moved by <see cref="Position"/> (camera space metres)
/// and the look turned further by <see cref="Rotation"/> (degrees about camera X, Y, Z) about that grip. The rest pose is
/// no change: the look as its stance holds it.
/// </summary>
public sealed record HeldPose(Vector3 Position, Vector3 Rotation)
{
    public Vector3 Position { get; set; } = Position;
    public Vector3 Rotation { get; set; } = Rotation;

    /// <summary>No change: the look where its stance seats it.</summary>
    public static HeldPose Rest => new(Vector3.Zero, Vector3.Zero);
}

/// <summary>
/// A key: the look reaches <see cref="Pose"/> at <see cref="At"/> (0 to 1) through <see cref="Phase"/>, easing in by
/// <see cref="Ease"/>, with the holding hand closed to <see cref="Grip"/> (-1 open, 0 its grip, 1 squeezed). When
/// given, the body leans the shoulders by <see cref="Shoulder"/> (camera space metres, the main hand's; mirrored for the
/// off hand), and the holding arm's elbow bends toward <see cref="Elbow"/> (camera space, the main hand's; without it the
/// arms' pole). For a look held in both hands, the off hand on it closes to <see cref="OffGrip"/> and its elbow bends
/// toward <see cref="OffElbow"/> (camera space as is: it holds the main hand's look, so nothing is mirrored).
/// </summary>
public sealed record MotionKey(string Pose, MotionPhase Phase, float At, TweenEasingKind Ease, float Grip = 0, Vector3? Shoulder = null,
    Vector3? Elbow = null, float OffGrip = 0, Vector3? OffElbow = null)
{
    /// <summary>Seconds from the action's start at <paramref name="timing"/>.</summary>
    public float Time(PhaseTiming timing) => timing.Time(Phase, At);
}

/// <summary>
/// One family of held motion: named poses (one of them <see cref="Rest"/>), keyed tracks by action id (with
/// <see cref="DefaultTrack"/> for the rest), the loops it sways by between and under actions, and how a look of it is
/// drawn and put away.
/// </summary>
public sealed record HeldMotion(Dictionary<string, HeldPose> Poses, Dictionary<string, MotionKey[]> Tracks, HeldLoops? Loops = null,
    HeldTransition? Equip = null, HeldTransition? Unequip = null)
{
    /// <summary>Every motion has a pose of this name: where the look rests between actions.</summary>
    public const string Rest = "rest";
    /// <summary>A motion's track for actions with no track of their own.</summary>
    public const string DefaultTrack = "default";

    /// <summary>An action's track: its own, else the default, else none.</summary>
    public MotionKey[]? Track(string action) => Tracks.GetValueOrDefault(action) ?? Tracks.GetValueOrDefault(DefaultTrack);

    /// <summary>
    /// What is wrong with this motion, if anything: a rest pose that is not no change, keys naming poses it lacks or out
    /// of range, loops and transitions that cannot play. Track timing against an action's phases is
    /// <see cref="OrderProblem"/>.
    /// </summary>
    public IEnumerable<HeldProblem> Problems()
    {
        if (!Poses.TryGetValue(Rest, out HeldPose? rest) || rest.Position != Vector3.Zero || rest.Rotation != Vector3.Zero)
            yield return new($"poses.{Rest}", "is required and must be no change ([0, 0, 0] both): a look rests where its stance seats it.");
        if (Tracks.Count == 0) yield return new("tracks", "needs at least one track.");
        foreach (var (track, keys) in Tracks)
            for (int i = 0; i < keys.Length; i++)
            {
                MotionKey key = keys[i];
                string at = $"tracks.{track}[{i}]";
                if (!Poses.ContainsKey(key.Pose)) yield return new($"{at}.pose", $"unknown pose '{key.Pose}'.");
                if (key.At is < 0 or > 1) yield return new($"{at}.at", "must lie from 0 to 1.");
                if (key.Grip is < -1 or > 1) yield return new($"{at}.grip", "must lie from -1 to 1.");
                if (key.OffGrip is < -1 or > 1) yield return new($"{at}.offGrip", "must lie from -1 to 1.");
                if (!HeldEases.IsNamed(key.Ease)) yield return new($"{at}.ease", HeldEases.NamedProblem);
            }
        if (Loops is { } loops)
            foreach (var (name, loop) in loops.Named)
                foreach (HeldProblem problem in loop.Problems()) yield return problem.Under($"loops.{name}");
        foreach (var (name, transition) in new[] { ("equip", Equip), ("unequip", Unequip) })
            if (transition is not null)
                foreach (HeldProblem problem in transition.Problems(Poses)) yield return problem.Under(name);
    }

    /// <summary>Where an action's keys fall out of time order through its phases, or null when they are in order.</summary>
    public string? OrderProblem(string action, PhaseTiming timing)
    {
        if (Track(action) is not { } keys) return null;
        for (int i = 1; i < keys.Length; i++)
        {
            float before = keys[i - 1].Time(timing), now = keys[i].Time(timing);
            if (now <= before)
                return FormattableString.Invariant(
                    $"keys for '{action}' must fall in time order through its phases: key {i} ({keys[i].Pose}, {now:0.###} s) follows key {i - 1} ({before:0.###} s).");
        }
        return null;
    }
}

/// <summary>A motion's sway loops: standing (<see cref="Idle"/>), walking and sprinting, each optional.</summary>
public sealed record HeldLoops(HeldLoop? Idle = null, HeldLoop? Walk = null, HeldLoop? Sprint = null)
{
    /// <summary>The loops given, by their field name.</summary>
    [JsonIgnore]
    public IEnumerable<(string Name, HeldLoop Loop)> Named =>
        new[] { ("idle", Idle), ("walk", Walk), ("sprint", Sprint) }.Where(p => p.Item2 is not null).Select(p => (p.Item1, p.Item2!));
}

/// <summary>
/// A sway loop: offsets from rest (a grip move, camera metres, and a turn about it, degrees about camera X, Y, Z) at
/// moments around one cycle, drawn as a smooth closed curve through them and scaled by <see cref="Scale"/>. Its
/// <see cref="Cycle"/> is seconds for standing and metres walked for walking and sprinting, so the bob keeps step.
/// </summary>
public sealed record HeldLoop(float Cycle, LoopKey[] Keys)
{
    public float Cycle { get; set; } = Cycle;
    public float Scale { get; set; } = 1;

    /// <summary>The loop's offset at a phase (0 to 1, wrapping), through its keys on a closed Catmull-Rom curve.</summary>
    public (Vector3 Move, Vector3 Turn) At(float phase)
    {
        phase -= MathF.Floor(phase);
        int n = Keys.Length;
        int i = Array.FindLastIndex(Keys, k => k.At <= phase);
        if (i < 0) i = n - 1;
        LoopKey a = Keys[i], b = Keys[(i + 1) % n];
        float span = (b.At - a.At + 1) % 1, into = (phase - a.At + 1) % 1;
        float t = span <= 0 ? 0 : into / span;
        LoopKey before = Keys[(i - 1 + n) % n], after = Keys[(i + 2) % n];
        Vector3 Curve(Func<LoopKey, Vector3> value) => CatmullRom(value(before), value(a), value(b), value(after), t);
        return (Curve(k => k.Position) * Scale, Curve(k => k.Rotation) * Scale);
    }

    internal IEnumerable<HeldProblem> Problems()
    {
        if (Cycle <= 0) yield return new("cycle", "must be positive.");
        if (Scale < 0) yield return new("scale", "must not be negative.");
        if (Keys.Length < 2) yield return new("keys", "needs at least two keys around the cycle.");
        for (int i = 0; i < Keys.Length; i++)
        {
            if (Keys[i].At is < 0 or > 1) yield return new($"keys[{i}].at", "must lie from 0 to 1.");
            else if (i > 0 && Keys[i].At <= Keys[i - 1].At) yield return new($"keys[{i}].at", "keys go around the cycle in order.");
        }
    }

    private static Vector3 CatmullRom(Vector3 p0, Vector3 p1, Vector3 p2, Vector3 p3, float t) =>
        0.5f * (2 * p1 + (p2 - p0) * t + (2 * p0 - 5 * p1 + 4 * p2 - p3) * t * t + (3 * p1 - p0 - 3 * p2 + p3) * t * t * t);
}

/// <summary>A sway loop's key: its moment around the cycle (0 to 1) and its offset from rest.</summary>
public sealed record LoopKey(float At, Vector3 Position, Vector3 Rotation);

/// <summary>
/// How a look is drawn or put away: keys through the motion's poses over <see cref="Seconds"/>. Drawing ends at rest;
/// putting away ends at its last key, out of the way.
/// </summary>
public sealed record HeldTransition(float Seconds, TransitionKey[] Keys)
{
    public float Seconds { get; set; } = Seconds;

    internal IEnumerable<HeldProblem> Problems(Dictionary<string, HeldPose> poses)
    {
        if (Seconds <= 0) yield return new("seconds", "must be positive.");
        if (Keys.Length < 1) yield return new("keys", "needs at least one key.");
        for (int i = 0; i < Keys.Length; i++)
        {
            TransitionKey key = Keys[i];
            if (!poses.ContainsKey(key.Pose)) yield return new($"keys[{i}].pose", $"unknown pose '{key.Pose}'.");
            if (key.At is < 0 or > 1) yield return new($"keys[{i}].at", "must lie from 0 to 1.");
            else if (i > 0 && key.At <= Keys[i - 1].At) yield return new($"keys[{i}].at", "keys fall in time order.");
            if (!HeldEases.IsNamed(key.Ease)) yield return new($"keys[{i}].ease", HeldEases.NamedProblem);
        }
    }
}

/// <summary>A transition key: the look reaches <see cref="Pose"/> at <see cref="At"/> (0 to 1) of the transition, easing in by <see cref="Ease"/>.</summary>
public sealed record TransitionKey(string Pose, float At, TweenEasingKind Ease);

/// <summary>The curves held motion keys may use: every named one (those needing parameters, bezier, steps and spring, are left out).</summary>
public static class HeldEases
{
    public static readonly TweenEasingKind[] Named = [.. Enum.GetValues<TweenEasingKind>()
        .Where(e => e is not (TweenEasingKind.CubicBezier or TweenEasingKind.Steps or TweenEasingKind.Spring))];

    public static bool IsNamed(TweenEasingKind ease) => Array.IndexOf(Named, ease) >= 0;

    internal const string NamedProblem =
        "must be a named curve (Linear, or Quad, Cubic, Quart, Quint, Sine, Expo, Circ, Back, Elastic or Bounce with In, Out or InOut).";
}
