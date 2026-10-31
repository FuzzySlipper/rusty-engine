namespace Rusty.Engine.FirstPerson;

/// <summary>
/// An arms clip an action plays when the arms are clip-led (<see cref="FirstPersonArms.Lead"/>), fitted to the action's
/// timing: <see cref="Marks"/> say where each phase lies in the clip, and each phase is played over the seconds the
/// product's <see cref="PhaseTiming"/> gives it, so retuning an action's speed needs no new export. A clip started while
/// another plays, or this one again, fades in over <see cref="Fade"/> seconds.
/// </summary>
public sealed record ArmClip(string Clip, ClipMarks Marks)
{
    public ClipMarks Marks { get; set; } = Marks;
    /// <summary>Seconds a clip fades in over the one before it, and the arms between clip-led and weapon-led.</summary>
    public float Fade { get; set; } = .15f;

    /// <summary>The clip at <paramref name="seconds"/> into an action at <paramref name="timing"/>: the clip's end once the action is over.</summary>
    public ArmClipFrame At(PhaseTiming timing, float seconds)
    {
        (MotionPhase phase, float from, float to, float length, float into) =
            seconds < timing.Windup ? (MotionPhase.Windup, Marks.Start, Marks.Strike, timing.Windup, seconds)
            : seconds < timing.Windup + timing.Commit ? (MotionPhase.Commit, Marks.Strike, Marks.Recover, timing.Commit, seconds - timing.Windup)
            : (MotionPhase.Recovery, Marks.Recover, Marks.End, timing.Recovery, seconds - timing.Windup - timing.Commit);
        float at = length <= 0 ? 1 : Math.Clamp(into / length, 0, 1);
        return new(Clip, from + (to - from) * at, phase, Fade);
    }

    /// <summary>What is wrong with this clip, if anything: no clip named, marks out of order or outside the clip, a negative fade.</summary>
    public IEnumerable<HeldProblem> Problems()
    {
        if (Clip.Length == 0) yield return new("clip", "must name a clip.");
        if (!(0 <= Marks.Start && Marks.Start <= Marks.Strike && Marks.Strike <= Marks.Recover && Marks.Recover <= Marks.End && Marks.End <= 1))
            yield return new("marks", "must lie in order (start, strike, recover, end) from 0 to 1.");
        if (Fade < 0) yield return new("fade", "must not be negative.");
    }
}

/// <summary>
/// Where an action's phases lie in its arms clip, as fractions of the clip: the windup plays from <see cref="Start"/>
/// to <see cref="Strike"/>, the commit to <see cref="Recover"/> and the recovery to <see cref="End"/>.
/// </summary>
public readonly record struct ClipMarks(float Start, float Strike, float Recover, float End);

/// <summary>
/// An arms clip at a moment: the clip, how far through it (a fraction), the action's phase there when an action plays it,
/// and how many seconds it fades in over what played before. <see cref="ArmClip.At"/> fits an action's; a product
/// playing a clip of its own (an idle loop, say) gives the time itself.
/// </summary>
public readonly record struct ArmClipFrame(string Clip, float Time, MotionPhase? Phase = null, float Fade = .15f);

/// <summary>
/// What clip-led arms are sampling: the clip and time (<see cref="Playing"/>), the clip fading out under it and the time
/// it holds at (<see cref="From"/>, <see cref="FromTime"/>), how far the playing clip has faded in over it
/// (<see cref="Blend"/>, 1 once there is nothing under it) and how far the arms are clip-led (<see cref="Led"/>, 0
/// weapon-led, 1 clip-led).
/// </summary>
public readonly record struct ArmClipReading(ArmClipFrame? Playing, string? From, float FromTime, float Blend, float Led);

// The clips clip-led arms play and their fades: the playing clip over the one it replaces, and how far the arms are led
// by clips rather than by their weapons. A clip whose time goes back has started again (an action played twice, a loop
// come round), so it fades in over where it was. A clip started inside another's fade fades in over whichever of the two showed
// more, and the other drops out.
internal sealed class ClipLead
{
    private ArmClipFrame? playing;
    private (string Clip, float Time)? from;
    private float blend = 1, led;

    internal ArmClipFrame? Playing => playing;
    internal (string Clip, float Time)? From => from;
    internal float Blend => blend;
    internal float Led => led;

    internal ArmClipReading Reading => new(playing, from?.Clip, from?.Time ?? 0, blend, led);

    internal void Advance(ArmClipFrame? frame, float delta)
    {
        float fade = frame?.Fade ?? playing?.Fade ?? 0;
        float Step(float value, float toward) => fade <= 0 ? toward : toward > value ? MathF.Min(toward, value + delta / fade) : MathF.Max(toward, value - delta / fade);
        if (frame is { } next)
        {
            if (playing is { } now && (now.Clip != next.Clip || next.Time < now.Time) && led > 0)
            {
                from = blend >= .5f || from is null ? (now.Clip, now.Time) : from;
                blend = 0;
            }
            else if (led <= 0) (from, blend) = (null, 1);
            playing = next;
            blend = Step(blend, 1);
            if (blend >= 1) from = null;
            led = Step(led, 1);
        }
        else
        {
            // Back to weapon-led: the last clip holds where it was while the arms fade back.
            led = Step(led, 0);
            if (led <= 0) (playing, from, blend) = (null, null, 1);
        }
    }
}
