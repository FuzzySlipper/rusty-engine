using System;
using System.Numerics;

namespace Rusty.Engine;

public readonly partial record struct TweenEasing
{
    public static TweenEasing Linear => new(TweenEasingKind.Linear, 0, 0, 0, 0);

    /// <summary>CSS <c>cubic-bezier(x1, y1, x2, y2)</c>; <paramref name="x1"/> and <paramref name="x2"/> lie in [0, 1].</summary>
    public static TweenEasing CubicBezier(float x1, float y1, float x2, float y2) =>
        new(TweenEasingKind.CubicBezier, x1, y1, x2, y2);

    /// <summary>Holds each of <paramref name="count"/> levels, reaching the end at the end.</summary>
    public static TweenEasing Steps(int count) => new(TweenEasingKind.Steps, count, 0, 0, 0);

    /// <summary>A unit-mass spring released toward the target; its whole settle time is stretched over the segment. Damping below <c>2√stiffness</c> overshoots.</summary>
    public static TweenEasing Spring(float stiffness, float damping) =>
        new(TweenEasingKind.Spring, stiffness, damping, 0, 0);

    public static implicit operator TweenEasing(TweenEasingKind kind) => new(kind, 0, 0, 0, 0);
}

public readonly partial record struct TweenSegment
{
    private static readonly Vector4 NoValue = Vector4.Zero;

    /// <summary>A base tween segment of one channel, starting at the timeline's start.</summary>
    public TweenSegment(TweenChannel channel, Vector4 from, Vector4 to, float durationSeconds, TweenEasing easing)
        : this(0, durationSeconds, channel, TweenLayer.Base, TweenShape.Tween, easing, from, to, Vector3.Zero, 0, 0, NoValue, NoValue)
    {
    }

    /// <summary>The channel's value that changes nothing: no translation, no rotation, unit scale, white.</summary>
    public static Vector4 Identity(TweenChannel channel) => channel switch
    {
        TweenChannel.Translation => Vector4.Zero,
        TweenChannel.Rotation => new(0, 0, 0, 1),
        _ => Vector4.One,
    };

    /// <summary>An offset from the published translation, e.g. from the previous cell to zero.</summary>
    public static TweenSegment Move(Vector3 from, Vector3 to, float durationSeconds, TweenEasing easing) =>
        new(TweenChannel.Translation, new Vector4(from, 0), new Vector4(to, 0), durationSeconds, easing);

    /// <summary>A move that also rises by <paramref name="arc"/> at its middle on a parabola in linear time.</summary>
    public static TweenSegment Hop(Vector3 from, Vector3 to, Vector3 arc, float durationSeconds, TweenEasing easing) =>
        Move(from, to, durationSeconds, easing) with { Arc = arc };

    public static TweenSegment Rotate(Quaternion from, Quaternion to, float durationSeconds, TweenEasing easing) =>
        new(TweenChannel.Rotation, new Vector4(from.X, from.Y, from.Z, from.W), new Vector4(to.X, to.Y, to.Z, to.W), durationSeconds, easing);

    /// <summary>A multiplier of the published scale, e.g. a squash from (1.3, 0.7, 1.3) to one.</summary>
    public static TweenSegment Scale(Vector3 from, Vector3 to, float durationSeconds, TweenEasing easing) =>
        new(TweenChannel.Scale, new Vector4(from, 1), new Vector4(to, 1), durationSeconds, easing);

    /// <summary>An RGBA multiplier of a sprite's tint or a primitive's colour.</summary>
    public static TweenSegment Tint(Vector4 from, Vector4 to, float durationSeconds, TweenEasing easing) =>
        new(TweenChannel.Tint, from, to, durationSeconds, easing);

    /// <summary>Swings from the channel's identity toward <paramref name="peak"/> and back <paramref name="frequency"/> times, shrinking by <paramref name="decay"/>. Additive, so it layers over a base track.</summary>
    public static TweenSegment Punch(TweenChannel channel, Vector4 peak, float durationSeconds, float frequency, TweenEasing decay) =>
        new TweenSegment(channel, Identity(channel), peak, durationSeconds, decay) with
        {
            Layer = TweenLayer.Additive,
            Shape = TweenShape.Punch,
            Frequency = frequency,
        };

    /// <summary>Smooth seeded noise reaching up to <paramref name="amplitude"/> from the channel's identity, <paramref name="frequency"/> changes per second, fading by <paramref name="fade"/>. Additive.</summary>
    public static TweenSegment Shake(TweenChannel channel, Vector4 amplitude, float durationSeconds, float frequency, uint seed, TweenEasing fade)
    {
        Vector4 identity = Identity(channel);
        Vector4 to = channel == TweenChannel.Rotation ? amplitude : identity + amplitude;
        return new TweenSegment(channel, identity, to, durationSeconds, fade) with
        {
            Layer = TweenLayer.Additive,
            Shape = TweenShape.Shake,
            Frequency = frequency,
            Seed = seed,
        };
    }

    /// <summary>Translation spans through <paramref name="points"/> (at least two), sharing <paramref name="durationSeconds"/> by length, each eased by <paramref name="easing"/>.</summary>
    public static TweenSegment[] Path(ReadOnlySpan<Vector3> points, float durationSeconds, TweenEasing easing)
    {
        if (points.Length < 2)
        {
            throw new ArgumentException("a path needs at least two points", nameof(points));
        }
        float length = 0;
        for (int index = 1; index < points.Length; index++)
        {
            length += Vector3.Distance(points[index - 1], points[index]);
        }
        var spans = new TweenSegment[points.Length - 1];
        float start = 0;
        for (int index = 0; index < spans.Length; index++)
        {
            float share = length > 0
                ? Vector3.Distance(points[index], points[index + 1]) / length
                : 1f / spans.Length;
            float seconds = durationSeconds * share;
            spans[index] = Move(points[index], points[index + 1], seconds, easing) with
            {
                StartSeconds = start,
                Shape = TweenShape.Spline,
                Before = new Vector4(points[Math.Max(index - 1, 0)], 0),
                After = new Vector4(points[Math.Min(index + 2, points.Length - 1)], 0),
            };
            start += seconds;
        }
        return spans;
    }

    /// <summary>This segment starting <paramref name="startSeconds"/> into the timeline.</summary>
    public TweenSegment At(float startSeconds) => this with { StartSeconds = startSeconds };

    /// <summary>This segment layered over its channel's base track.</summary>
    public TweenSegment Additive() => this with { Layer = TweenLayer.Additive };
}

public readonly partial record struct TweenStartRequest
{
    /// <summary>Plays <paramref name="segments"/> once on world time, replacing the object's tweens.</summary>
    public TweenStartRequest(ulong objectId, ReadOnlyMemory<TweenSegment> segments)
        : this(objectId, segments, ReadOnlyMemory<TweenMarker>.Empty, 1, false, false, TweenClock.World, TweenStart.Replace, 0)
    {
    }
}

public readonly partial record struct TweenControlRequest
{
    /// <summary>Pauses, resumes, completes or cancels <paramref name="tween"/>.</summary>
    public TweenControlRequest(TweenHandle tween, TweenControl control)
        : this(tween, control, 0)
    {
    }

    /// <summary>Moves <paramref name="tween"/> to <paramref name="elapsedSeconds"/> after its start and shows that pose at the end of the call; a paused tween stays paused there.</summary>
    public static TweenControlRequest Seek(TweenHandle tween, double elapsedSeconds) =>
        new(tween, TweenControl.Seek, elapsedSeconds);
}

public readonly partial record struct TweenSampleRequest
{
    /// <summary>The timeline <paramref name="timeline"/> would play, <paramref name="elapsedSeconds"/> after its start. Its object, clock, start mode and markers do not matter.</summary>
    public TweenSampleRequest(TweenStartRequest timeline, double elapsedSeconds)
        : this(timeline.Segments, timeline.Iterations, timeline.Forever, timeline.Yoyo, elapsedSeconds)
    {
    }
}

public static class TweenServiceConvenience
{
    /// <summary>The Engine's <paramref name="easing"/> curve at linear <paramref name="progress"/> in [0, 1], as tweens play it.</summary>
    public static float Evaluate(this ITweenService tween, TweenEasing easing, float progress) =>
        tween.EvaluateEasing(new TweenEasingSampleRequest(easing, progress)).Value;

    /// <summary>The offset <paramref name="timeline"/> shows <paramref name="elapsedSeconds"/> after its start, as a tween playing it would show it.</summary>
    public static TweenSample Sample(this ITweenService tween, TweenStartRequest timeline, double elapsedSeconds) =>
        tween.Sample(new TweenSampleRequest(timeline, elapsedSeconds));
}

/// <summary>Common tween timelines. Each returns an ordinary request to adjust with <c>with</c> and pass to <see cref="ITweenService.Start"/>.</summary>
public static class Tweens
{
    private const float DefaultLandingSeconds = 0.18f;

    /// <summary>A hop from <paramref name="offset"/> (the previous position minus the newly published one) with a peak <paramref name="height"/> up, then a landing squash of <paramref name="squash"/> settling with an overshoot. Starts from the pose shown, so a hop replacing one mid-flight continues smoothly.</summary>
    public static TweenStartRequest HopFrom(ulong objectId, Vector3 offset, float height, float seconds, float squash)
    {
        Vector3 squashed = new(1 + squash, 1 - squash, 1 + squash);
        TweenSegment[] segments =
        [
            TweenSegment.Hop(offset, Vector3.Zero, Vector3.UnitY * height, seconds, TweenEasing.Linear),
            // Unit scale in flight (easing out of any squash shown when it
            // replaces a hop), so the landing squash shows only on landing.
            TweenSegment.Scale(Vector3.One, Vector3.One, seconds, TweenEasingKind.QuadOut),
            TweenSegment.Scale(squashed, Vector3.One, DefaultLandingSeconds, TweenEasingKind.BackOut).At(seconds),
        ];
        return new TweenStartRequest(objectId, segments) with { Start = TweenStart.FromPresented };
    }

    /// <summary>A gentle scale pulse to <c>1 + amount</c> and back every <paramref name="periodSeconds"/>, forever, layered over the object's other tweens.</summary>
    public static TweenStartRequest Breathe(ulong objectId, float amount, float periodSeconds)
    {
        TweenSegment[] segments =
        [
            TweenSegment.Scale(Vector3.One, new Vector3(1 + amount), periodSeconds / 2, TweenEasingKind.SineInOut),
        ];
        return new TweenStartRequest(objectId, segments) with
        {
            Forever = true,
            Yoyo = true,
            Start = TweenStart.Layer,
        };
    }

    /// <summary>A scale punch toward <c>1 + amount</c>, layered.</summary>
    public static TweenStartRequest PunchScale(ulong objectId, float amount, float seconds)
    {
        TweenSegment[] segments =
        [
            TweenSegment.Punch(TweenChannel.Scale, new Vector4(new Vector3(1 + amount), 1), seconds, 2, TweenEasingKind.QuadOut),
        ];
        return new TweenStartRequest(objectId, segments) with { Start = TweenStart.Layer };
    }

    /// <summary>A positional shake of up to <paramref name="amplitude"/>, layered.</summary>
    public static TweenStartRequest Shake(ulong objectId, Vector3 amplitude, float seconds, float frequency, uint seed)
    {
        TweenSegment[] segments =
        [
            TweenSegment.Shake(TweenChannel.Translation, new Vector4(amplitude, 0), seconds, frequency, seed, TweenEasingKind.QuadOut),
        ];
        return new TweenStartRequest(objectId, segments) with { Start = TweenStart.Layer };
    }

    /// <summary>Starts at <paramref name="color"/> times the object's colour and returns to it, layered: a hit flash.</summary>
    public static TweenStartRequest Flash(ulong objectId, Vector4 color, float seconds)
    {
        TweenSegment[] segments = [TweenSegment.Tint(color, Vector4.One, seconds, TweenEasingKind.QuadOut)];
        return new TweenStartRequest(objectId, segments) with { Start = TweenStart.Layer };
    }

    /// <summary>Fades the object in from transparent, layered.</summary>
    public static TweenStartRequest FadeIn(ulong objectId, float seconds)
    {
        TweenSegment[] segments = [TweenSegment.Tint(new Vector4(1, 1, 1, 0), Vector4.One, seconds, TweenEasingKind.QuadOut)];
        return new TweenStartRequest(objectId, segments) with { Start = TweenStart.Layer };
    }
}
