using System.Numerics;

namespace Rusty.Engine.FirstPerson;

/// <summary>
/// A damped punch felt in a held weapon: it swings toward <see cref="Move"/> (camera space metres, the main hand's;
/// mirrored for the off hand) and turns toward <see cref="Turn"/> (degrees about camera X, Y, Z) and back
/// <see cref="Frequency"/> times over <see cref="Seconds"/>, the swing dying away by <see cref="Decay"/>.
/// </summary>
public sealed record HeldPunch(Vector3 Move, Vector3 Turn, float Seconds, float Frequency, TweenEasingKind Decay)
{
    public Vector3 Move { get; set; } = Move;
    public Vector3 Turn { get; set; } = Turn;
    public float Seconds { get; set; } = Seconds;
    public float Frequency { get; set; } = Frequency;
    public TweenEasingKind Decay { get; set; } = Decay;

    /// <summary>What is wrong with this punch, if anything.</summary>
    public IEnumerable<HeldProblem> Problems()
    {
        if (Seconds <= 0) yield return new("seconds", "must be positive.");
        if (Frequency <= 0) yield return new("frequency", "must be positive.");
        if (!HeldEases.IsNamed(Decay)) yield return new("decay", HeldEases.NamedProblem);
    }
}

/// <summary>
/// One hand's punches layered over its motion: each an Engine tween started as a layer on the hand's object, so it adds
/// to a keyed swing instead of replacing it, and kept so where the weapon is drawn can be told (the arms reach for the
/// weapon as drawn, layers and all). A motion started over them with a replacing start ends them, as the Engine ends an
/// object's tweens; the product calls <see cref="Ended"/> then.
/// </summary>
public sealed class HeldLayers(IEngineContext engine)
{
    private readonly List<(TweenHandle Handle, TweenStartRequest Request)> playing = [];

    /// <summary>
    /// Starts a punch on a hand's object, its turn given in camera space and applied in the drawn model's own frame
    /// (<paramref name="model"/>, its turn now), as the Engine applies a rotation channel.
    /// </summary>
    public void Start(ulong objectId, HeldPunch punch, Quaternion model, bool offHand, TweenClock clock)
    {
        Vector3 move = punch.Move;
        Quaternion turn = HeldFrames.Turn(punch.Turn);
        if (offHand) (move, turn) = (HeldFrames.Mirror(move), HeldFrames.Mirror(turn));
        Quaternion local = Quaternion.Normalize(Quaternion.Inverse(model) * turn * model);
        TweenSegment[] segments =
        [
            TweenSegment.Punch(TweenChannel.Translation, new Vector4(move, 0), punch.Seconds, punch.Frequency, punch.Decay),
            TweenSegment.Punch(TweenChannel.Rotation, new Vector4(local.X, local.Y, local.Z, local.W), punch.Seconds, punch.Frequency, punch.Decay)
        ];
        TweenStartRequest request = new(objectId, segments) { Clock = clock, Start = TweenStart.Layer };
        playing.Add((engine.Tween.Start(request).Tween, request));
    }

    /// <summary>The offset the running punches add now: translations summed, turns composed in the model's frame.</summary>
    public (Vector3 Move, Quaternion Turn) Offset()
    {
        playing.RemoveAll(p => engine.Tween.Read(p.Handle).State == TweenState.Ended);
        (Vector3 move, Quaternion turn) = (Vector3.Zero, Quaternion.Identity);
        foreach (var (handle, request) in playing)
        {
            TweenSample sample = engine.Tween.Sample(request, engine.Tween.Read(handle).ElapsedSeconds);
            (move, turn) = (move + sample.Translation, Quaternion.Normalize(turn * sample.Rotation));
        }
        return (move, turn);
    }

    /// <summary>How many punches have run since the hand last started a motion or the offset was read (they end on their own).</summary>
    public int Count => playing.Count;

    /// <summary>Forgets the punches: a replacing start has ended them.</summary>
    public void Ended() => playing.Clear();

    /// <summary>Ends the punches.</summary>
    public void Cancel()
    {
        foreach (var (handle, _) in playing) engine.Tween.Control(new TweenControlRequest(handle, TweenControl.Cancel));
        playing.Clear();
    }
}
