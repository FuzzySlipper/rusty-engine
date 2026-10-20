using System.Numerics;

namespace Rusty.Engine.FirstPerson;

/// <summary>
/// Where a pose puts a held look's drawn model (camera space): the product's placement, so a timeline can turn poses
/// into the Engine tween channels' offsets from rest.
/// </summary>
public delegate (Vector3 At, Quaternion Turn) HeldPlacement(HeldPose pose);

/// <summary>
/// Held motion as Engine tween timelines over a look's drawn model. Values are offsets from rest, as the tween
/// channels apply them: a translation in camera space and a rotation in the model's own frame. The product publishes
/// the model at rest and starts the timeline on its object.
/// </summary>
public static class HeldTimelines
{
    /// <summary>
    /// An action's timeline: from <paramref name="from"/> (the offset shown when it starts, so it carries on without a
    /// jump; rest by default) through each key's pose by its curve, back to rest by the end of recovery on
    /// <paramref name="settleEase"/>. The phases are scaled by <paramref name="scale"/> (the user's action time) and
    /// played <paramref name="speed"/> times as fast (a viewer's slow motion).
    /// </summary>
    public static TweenSegment[] Action(HeldMotion motion, IReadOnlyList<MotionKey> keys, PhaseTiming timing, HeldPlacement place,
        TweenEasingKind settleEase, float scale = 1, float speed = 1, (Vector3 Move, Quaternion Turn)? from = null)
    {
        Func<HeldPose, (Vector3, Quaternion)> offset = Offsets(motion, place);
        List<TweenSegment> segments = [];
        (Vector3 Move, Quaternion Turn) last = from ?? (Vector3.Zero, Quaternion.Identity);
        float start = 0;
        void Ease((Vector3 Move, Quaternion Turn) to, float time, TweenEasingKind ease)
        {
            if (time - start <= 0) return;
            float at = start / speed, seconds = (time - start) / speed;
            segments.Add(TweenSegment.Move(last.Move, to.Move, seconds, ease).At(at));
            segments.Add(TweenSegment.Rotate(last.Turn, to.Turn, seconds, ease).At(at));
            last = to;
            start = time;
        }
        foreach (MotionKey key in keys) Ease(offset(motion.Poses[key.Pose]), key.Time(timing) * scale, key.Ease);
        // Every timeline ends at rest, where the model is published, when the action ends.
        Ease((Vector3.Zero, Quaternion.Identity), timing.Duration * scale, settleEase);
        return [.. segments];
    }

    /// <summary>
    /// A look being drawn or put away: drawing starts at its first key's pose and ends at rest; putting away starts from
    /// <paramref name="from"/> (the offset shown) and ends at its last key's pose, held there <paramref name="hold"/>
    /// seconds past its end so the look never shows at rest before the product swaps it.
    /// </summary>
    public static TweenSegment[] Transition(HeldMotion motion, HeldTransition transition, bool drawing, HeldPlacement place,
        (Vector3 Move, Quaternion Turn) from = default, float hold = 0)
    {
        Func<HeldPose, (Vector3, Quaternion)> offset = Offsets(motion, place);
        List<TweenSegment> segments = [];
        (Vector3 Move, Quaternion Turn) last = drawing ? offset(motion.Poses[transition.Keys[0].Pose])
            : from.Turn == default ? (Vector3.Zero, Quaternion.Identity) : from;
        float start = 0;
        void Ease((Vector3 Move, Quaternion Turn) to, float time, TweenEasingKind ease)
        {
            if (time - start <= 0) { last = to; return; }
            segments.Add(TweenSegment.Move(last.Move, to.Move, time - start, ease).At(start));
            segments.Add(TweenSegment.Rotate(last.Turn, to.Turn, time - start, ease).At(start));
            (last, start) = (to, time);
        }
        foreach (TransitionKey key in transition.Keys) Ease(offset(motion.Poses[key.Pose]), key.At * transition.Seconds, key.Ease);
        if (drawing) Ease((Vector3.Zero, Quaternion.Identity), transition.Seconds, transition.Keys[^1].Ease);
        else Ease(last, transition.Seconds + hold, TweenEasingKind.Linear);
        // A timeline whose keys all sit at its start still holds its pose for its length.
        if (segments.Count == 0)
        {
            segments.Add(TweenSegment.Move(last.Move, last.Move, transition.Seconds, TweenEasingKind.Linear));
            segments.Add(TweenSegment.Rotate(last.Turn, last.Turn, transition.Seconds, TweenEasingKind.Linear));
        }
        return [.. segments];
    }

    /// <summary>The settle back to rest from <paramref name="from"/>, the offset a cut-short action left the model at.</summary>
    public static TweenSegment[] Settle((Vector3 Move, Quaternion Turn) from, float seconds, TweenEasingKind ease) =>
    [
        TweenSegment.Move(from.Move, Vector3.Zero, seconds, ease),
        TweenSegment.Rotate(from.Turn, Quaternion.Identity, seconds, ease)
    ];

    /// <summary>
    /// What a hand's keys say at <paramref name="time"/> seconds into an action (at its authored timing, before any
    /// scale): each value read from a key by <paramref name="read"/>, eased from the previous key's by the key's curve as
    /// the Engine eases the timeline, and back to <paramref name="idle"/> by the end of recovery on
    /// <paramref name="settleEase"/>; idle once the action is over.
    /// </summary>
    public static HandKeying Keyed(IEngineContext engine, IReadOnlyList<MotionKey> keys, PhaseTiming timing, float time,
        Func<MotionKey, HandKeying> read, HandKeying idle, TweenEasingKind settleEase)
    {
        HandKeying from = idle;
        float start = 0;
        foreach ((float at, HandKeying to, TweenEasingKind curve) in keys.Select(k => (k.Time(timing), read(k), k.Ease))
            .Append((timing.Duration, idle, settleEase)))
        {
            if (time <= at)
            {
                float t = at - start <= 0 ? 1 : engine.Tween.Evaluate(curve, Math.Clamp((time - start) / (at - start), 0, 1));
                return new(from.Closure + (to.Closure - from.Closure) * t, Vector3.Lerp(from.Lean, to.Lean, t), Vector3.Lerp(from.Elbow, to.Elbow, t));
            }
            (from, start) = (to, at);
        }
        return idle;
    }

    // A pose as the tween channels' offset from rest: the drawn model's move, and its turn in its own frame.
    private static Func<HeldPose, (Vector3, Quaternion)> Offsets(HeldMotion motion, HeldPlacement place)
    {
        (Vector3 restAt, Quaternion restTurn) = place(motion.Poses[HeldMotion.Rest]);
        return pose =>
        {
            (Vector3 at, Quaternion turn) = place(pose);
            return (at - restAt, Quaternion.Normalize(Quaternion.Inverse(restTurn) * turn));
        };
    }
}

/// <summary>What a hand's motion keys say at a moment: how closed it is (-1 open, 0 its grip, 1 squeezed), how far the body leans (camera space) and where its elbow bends toward (camera space).</summary>
public readonly record struct HandKeying(float Closure, Vector3 Lean, Vector3 Elbow);
