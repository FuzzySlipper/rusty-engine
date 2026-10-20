using System.Numerics;

namespace Rusty.Engine.FirstPerson;

/// <summary>
/// How held weapons sway: how quickly the sway eases from standing to walking to sprinting (<see cref="BlendSeconds"/>),
/// how much of it stays under a hand's own action (<see cref="Acting"/>, 0 to 1), and how far a single update's move may
/// be and still count as walking (a teleport or a level change is not a stride).
/// </summary>
public sealed record SwayTuning(float BlendSeconds, float Acting, float LongestStride)
{
    public float BlendSeconds { get; set; } = BlendSeconds;
    public float Acting { get; set; } = Acting;
    public float LongestStride { get; set; } = LongestStride;

    /// <summary>What is wrong with this tuning, if anything.</summary>
    public IEnumerable<HeldProblem> Problems()
    {
        if (BlendSeconds <= 0) yield return new("blendSeconds", "must be positive.");
        if (Acting is < 0 or > 1) yield return new("acting", "must lie from 0 to 1.");
        if (LongestStride <= 0) yield return new("longestStride", "must be positive.");
    }
}

/// <summary>
/// The held weapons' sway over the walker's movement: a motion's standing loop on time, its walking and sprinting loops
/// on distance walked (so the bob keeps step at any speed), blended by speed and eased between, faded in the air, and
/// turned down under each hand's own action. Each hand's sway is a pose from rest, which the product publishes as the
/// look's rest placement, so actions, punches and the arms' reach compose over it. Advance it on host seconds: it is
/// presentation, and the walker still breathes while the world holds.
/// </summary>
public sealed class HeldSway(SwayTuning tuning)
{
    private double standing;
    private float walked;
    private Vector3? lastFeet;
    private (float Idle, float Walk, float Sprint) weight = (1, 0, 0);
    private (float Main, float Off) acting;

    /// <summary>
    /// Advances by an update's host seconds with the walker's feet and horizontal speed, whether it is on the ground, its
    /// walking and sprinting speeds, and whether each hand's action is playing.
    /// </summary>
    public void Advance(double hostSeconds, Vector3 feet, float speed, bool grounded, float walkSpeed, float sprintSpeed, bool mainActing, bool offActing)
    {
        standing += hostSeconds;
        Vector3 flat = feet with { Y = 0 };
        if (lastFeet is { } last && grounded)
        {
            float stride = Vector3.Distance(flat, last);
            if (stride <= tuning.LongestStride) walked += stride;
        }
        lastFeet = flat;
        float moving = grounded ? Math.Clamp(speed / MathF.Max(.01f, walkSpeed), 0, 1) : 0;
        float sprinting = grounded ? Math.Clamp((speed - walkSpeed) / MathF.Max(.01f, sprintSpeed - walkSpeed), 0, 1) : 0;
        float ease = 1 - MathF.Exp(-(float)hostSeconds / tuning.BlendSeconds);
        weight = (Toward(weight.Idle, 1 - moving), Toward(weight.Walk, moving * (1 - sprinting)), Toward(weight.Sprint, moving * sprinting));
        acting = (Toward(acting.Main, mainActing ? 1 : 0), Toward(acting.Off, offActing ? 1 : 0));
        float Toward(float from, float to) => from + (to - from) * ease;
    }

    /// <summary>A hand's sway now, as a pose from rest, from its look's loops (none: no sway).</summary>
    public HeldPose Pose(HeldLoops? loops, bool offHand)
    {
        (Vector3 move, Vector3 turn) = (Vector3.Zero, Vector3.Zero);
        void Add(HeldLoop? loop, float amount, double along)
        {
            if (loop is null || amount <= 1e-3f) return;
            (Vector3 m, Vector3 t) = loop.At((float)(along / loop.Cycle));
            (move, turn) = (move + m * amount, turn + t * amount);
        }
        Add(loops?.Idle, weight.Idle, standing);
        Add(loops?.Walk, weight.Walk, walked);
        Add(loops?.Sprint, weight.Sprint, walked);
        float keep = 1 - (offHand ? acting.Off : acting.Main) * (1 - tuning.Acting);
        return new HeldPose(move * keep, turn * keep);
    }

    /// <summary>A level change or a restart: the next move is not a stride.</summary>
    public void Moved() => lastFeet = null;
}

/// <summary>
/// How a first-person view bobs: a gait for walking and one for sprinting, blended by speed and eased in and out over
/// <see cref="BlendSeconds"/>, and a dip on landing.
/// </summary>
public sealed record HeadBobTuning(BobGait Walk, BobGait Sprint, float BlendSeconds, BobLanding Landing)
{
    public float BlendSeconds { get; set; } = BlendSeconds;

    /// <summary>What is wrong with this tuning, if anything.</summary>
    public IEnumerable<HeldProblem> Problems()
    {
        foreach (var (name, gait) in new[] { ("walk", Walk), ("sprint", Sprint) })
        {
            if (gait.StepMetres <= 0) yield return new($"{name}.stepMetres", "must be positive.");
            if (gait.Rise < 0) yield return new($"{name}.rise", "must not be negative.");
            if (gait.Sway < 0) yield return new($"{name}.sway", "must not be negative.");
            if (gait.RollDegrees < 0) yield return new($"{name}.rollDegrees", "must not be negative.");
        }
        if (BlendSeconds <= 0) yield return new("blendSeconds", "must be positive.");
        if (Landing.MetresPerSpeed < 0) yield return new("landing.metresPerSpeed", "must not be negative.");
        if (Landing.Softest < 0) yield return new("landing.softest", "must not be negative.");
        if (Landing.Deepest < 0) yield return new("landing.deepest", "must not be negative.");
        if (Landing.Seconds <= 0) yield return new("landing.seconds", "must be positive.");
        if (Landing.Down is < .01f or > .99f) yield return new("landing.down", "must lie from 0.01 to 0.99.");
    }
}

/// <summary>
/// A gait's bob: the ground one step covers (metres), how far the eye drops at each footfall (<see cref="Rise"/>, metres
/// below mid-stance), how far it sways over the planted foot (<see cref="Sway"/>, metres sideways) and how far the view
/// rolls with it (degrees).
/// </summary>
public sealed record BobGait(float StepMetres, float Rise, float Sway, float RollDegrees)
{
    public float StepMetres { get; set; } = StepMetres;
    public float Rise { get; set; } = Rise;
    public float Sway { get; set; } = Sway;
    public float RollDegrees { get; set; } = RollDegrees;
}

/// <summary>
/// The dip on landing: <see cref="MetresPerSpeed"/> of drop for each m/s of fall past <see cref="Softest"/> (a hop does
/// not dip), at most <see cref="Deepest"/>, down over the first <see cref="Down"/> share of <see cref="Seconds"/> and back up.
/// </summary>
public sealed record BobLanding(float MetresPerSpeed, float Softest, float Deepest, float Seconds, float Down)
{
    public float MetresPerSpeed { get; set; } = MetresPerSpeed;
    public float Softest { get; set; } = Softest;
    public float Deepest { get; set; } = Deepest;
    public float Seconds { get; set; } = Seconds;
    public float Down { get; set; } = Down;
}

/// <summary>
/// A first-person head bob: an offset and roll for the camera only (aim stays on the steady eye). The step keeps pace
/// with the ground walked, so it stays in step at any speed; its size eases with speed and fades in the air; a landing
/// dips the view by how fast the body fell. Advance it on host seconds, as <see cref="HeldSway"/>.
/// </summary>
public sealed class HeadBob(HeadBobTuning tuning)
{
    private float phase;
    private (float Walk, float Sprint) weight;
    private double sinceLanding = double.MaxValue;
    private float landingDepth;

    /// <summary>
    /// Advances by an update's host seconds with the ground walked since the last, the walker's horizontal speed, whether
    /// it is on the ground, its walking and sprinting speeds, and the fall speed of a landing in this update.
    /// </summary>
    public void Advance(double hostSeconds, float walked, float speed, bool grounded, float walkSpeed, float sprintSpeed, float? landedFalling)
    {
        float moving = grounded ? Math.Clamp(speed / MathF.Max(.01f, walkSpeed), 0, 1) : 0;
        float sprinting = grounded ? Math.Clamp((speed - walkSpeed) / MathF.Max(.01f, sprintSpeed - walkSpeed), 0, 1) : 0;
        float ease = 1 - MathF.Exp(-(float)hostSeconds / tuning.BlendSeconds);
        weight = (weight.Walk + (moving * (1 - sprinting) - weight.Walk) * ease, weight.Sprint + (moving * sprinting - weight.Sprint) * ease);
        // The gait's step even before its bob grows.
        float step = Blend(g => g.StepMetres, (1 - sprinting, sprinting));
        if (grounded) phase = (phase + walked / step * MathF.PI) % (2 * MathF.PI);
        sinceLanding += hostSeconds;
        if (landedFalling is { } falling)
        {
            BobLanding landing = tuning.Landing;
            float depth = Math.Min(landing.Deepest, MathF.Max(0, falling - landing.Softest) * landing.MetresPerSpeed);
            if (depth > 0) (landingDepth, sinceLanding) = (depth, 0);
            phase = 0; // a landing is a footfall: the next step starts here
        }
    }

    /// <summary>The bob now: the eye's offset along the view's own right and up (metres) and its roll (degrees, positive to the right).</summary>
    public (float Right, float Up, float RollDegrees) Now()
    {
        float side = MathF.Sin(phase);
        float up = (MathF.Abs(side) - 1) * Blend(g => g.Rise, weight);
        BobLanding landing = tuning.Landing;
        float t = (float)(sinceLanding / landing.Seconds);
        if (t < 1)
            up -= landingDepth * (t < landing.Down
                ? MathF.Sin(t / landing.Down * MathF.PI / 2)
                : 0.5f + 0.5f * MathF.Cos((t - landing.Down) / (1 - landing.Down) * MathF.PI));
        return (side * Blend(g => g.Sway, weight), up, side * Blend(g => g.RollDegrees, weight));
    }

    /// <summary>
    /// The camera's basis at a look direction with the bob's roll, and the eye moved by its offset: an explicit camera
    /// basis (forward, right, up) and position for a <see cref="CameraBasisMode.Explicit"/> camera.
    /// </summary>
    public (Vector3 Eye, CameraBasis Basis) Apply(Vector3 eye, Vector3 forward)
    {
        Vector3 right = Vector3.Normalize(Vector3.Cross(forward, Vector3.UnitY));
        Vector3 up = Vector3.Cross(right, forward);
        (float side, float rise, float rollDegrees) = Now();
        Quaternion roll = Quaternion.CreateFromAxisAngle(forward, -rollDegrees * MathF.PI / 180);
        return (eye + right * side + up * rise, new CameraBasis(forward, Vector3.Transform(right, roll), Vector3.Transform(up, roll)));
    }

    /// <summary>A level change or a restart: the view stands still.</summary>
    public void Reset() => (phase, weight, sinceLanding) = (0, (0, 0), double.MaxValue);

    private float Blend(Func<BobGait, float> of, (float Walk, float Sprint) by) => of(tuning.Walk) * by.Walk + of(tuning.Sprint) * by.Sprint;
}
