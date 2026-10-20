using System.Numerics;

namespace Rusty.Engine.FirstPerson;

/// <summary>
/// First-person arms drawn by one rig profile (<see cref="ArmRig"/>): its model on the viewmodel layer, placed so its
/// right upper arm sits at the arms' shoulder, whose hands reach their weapons' grips by the Engine's two-bone IK each
/// update over the rig's clip, each hand turned with its weapon's look by the hold and its digits shaped and closed as
/// the look and motion say. Only the weapons move by authored motion; the arms follow, elbows bending toward their
/// poles, the left mirroring the right. Nothing names a rig's frames: each hand's frame and digit axes are measured from
/// the drawn model once, so any rig with two arms and five digits a hand can hold.
/// </summary>
/// <remarks>
/// Each update: set <see cref="Lean"/>, call <see cref="Aim"/> with both hands' aims, publish <see cref="Fact"/> in the
/// snapshot, then call <see cref="Reach"/> with the same aims once that snapshot is published.
/// </remarks>
public sealed class FirstPersonArms : IDisposable
{
    // A reach never quite straightens the arm, and the body leans at most this far (metres).
    private const float Slack = .01f, MostReaching = .3f;

    private readonly IEngineContext engine;
    private readonly HeldArms arms;
    private readonly ArmRig rig;
    private readonly RenderResource model;
    private readonly Appearance appearance;
    private readonly Side main, off;
    private readonly HandRig mainRig, offRig;
    private AnimationInstance? instance;
    // Where each upper arm was last drawn (model space); the right one places the model so it sits at the shoulder.
    private Vector3? upperArm, offUpperArm;
    private Vector3 reaching;
    private JointPose[]? drawnJoints;

    /// <summary>
    /// Opens the rig's model and names its joints. Throws <see cref="InvalidOperationException"/> naming the field when
    /// the model lacks the rig's clip or a joint it names (and the Engine's own error when the model does not open).
    /// </summary>
    /// <param name="objectId">The published object the arms are drawn on; an object keeps one animation instance, so a new rig takes a new object.</param>
    public FirstPersonArms(IEngineContext engine, HeldArms arms, ArmRig rig, ulong objectId)
    {
        this.engine = engine;
        this.arms = arms;
        this.rig = rig;
        Object = objectId;
        using ContentReference content = engine.Content.OpenReference(new(rig.Model));
        model = engine.Animation.OpenAnimatedMeshFromContent(new(content));
        try
        {
            AnimationJointInfo[] joints = engine.Animation.ReadJoints(model).ToArray();
            string[] clips = [.. engine.Animation.ReadClips(model).ToArray().Select(c => c.Id)];
            if (!clips.Contains(rig.Clip))
                throw new InvalidOperationException($"clip: the arms model '{rig.Model}' has no clip '{rig.Clip}'. Its clips: {string.Join(", ", clips)}.");
            uint Joint(string name, string field)
            {
                int at = Array.FindIndex(joints, j => j.Id == name);
                return at >= 0 ? (uint)at : throw new InvalidOperationException($"{field}: the arms model '{rig.Model}' has no joint '{name}'.");
            }
            Side Arm(ArmJoints chain, string field, bool left) => new(left,
                Joint(chain.Upper, $"{field}.upper"), Joint(chain.Lower, $"{field}.lower"), Joint(chain.Hand, $"{field}.hand"),
                [.. chain.Fingers.Select((f, i) => f.Select(n => Joint(n, $"{field}.fingers[{i}]")).ToArray())],
                [.. chain.Thumb.Select(n => Joint(n, $"{field}.thumb"))]);
            main = Arm(rig.Main, "main", false);
            off = Arm(rig.Off, "off", true);
            // Each hand's joints named once for the Engine's hand landmarks.
            HandRig Hand(ArmJoints chain, HandSide side) => HandRig.FromJoints(joints, objectId, side, chain.Hand, chain.Thumb,
                chain.Fingers[0], chain.Fingers[1], chain.Fingers[2], chain.Fingers[3]);
            mainRig = Hand(rig.Main, HandSide.Right);
            offRig = Hand(rig.Off, HandSide.Left);
            appearance = engine.Animation.CreateAnimatedMeshAppearance(new(model));
        }
        catch { model.Dispose(); throw; }
    }

    /// <summary>The published object these arms are drawn on.</summary>
    public ulong Object { get; }

    /// <summary>The profile drawing these arms.</summary>
    public ArmRig Rig => rig;

    /// <summary>Each hand's measured turn from its canonical frame to its joint's (degrees about X, Y, Z), once measured; null before.</summary>
    public (Vector3 Main, Vector3 Off)? HandFrames => main.Measured ? (HeldFrames.Degrees(main.Frame), HeldFrames.Degrees(off.Frame)) : null;

    /// <summary>
    /// A hand's knuckle angles (<see cref="HandKnuckles"/>): on the rig's clip alone, as first drawn, and as last drawn
    /// (with the rig's grip, the look's shape and the closure); null before the hands are measured.
    /// </summary>
    public (HandShape Clip, HandShape Drawn)? Knuckles(bool offHand) =>
        (offHand ? off : main) is { Clip: { } clip } side && drawnJoints is { } joints ? (clip, side.Knuckles(joints)) : null;

    /// <summary>A hand's rig joints for the Engine's hand landmarks: the main (right) hand or the off (left).</summary>
    public HandRig Hand(bool offHand) => offHand ? offRig : mainRig;

    /// <summary>How far the motion leans the body: the shoulders' move from where the arms place them (camera space).</summary>
    public Vector3 Lean { get; set; }

    /// <summary>How far the body leans in on its own so a hand reaches a grip past its arm's length (camera space).</summary>
    public Vector3 Reaching => reaching;

    /// <summary>
    /// How far each wrist was drawn from its target, as the Engine's solve reports it (metres): more than a centimetre or
    /// so is a grip out of the arm's reach.
    /// </summary>
    public (float Main, float Off) Shortfall { get; private set; }

    /// <summary>Each arm's length from the upper arm joint through the elbow to the hand joint as last drawn (metres), once measured.</summary>
    public (float Main, float Off) ArmLength { get; private set; }

    /// <summary>
    /// Before the arms are published: leans the body in, beyond the motion's own <see cref="Lean"/>, when a grip lies
    /// further from its shoulder than the arm reaches, so both hands stay on their grips whatever the rig's arm length.
    /// The arm furthest short sets the lean; the other arm's IK takes up the move.
    /// </summary>
    public void Aim(HandAim mainHand, HandAim offHand)
    {
        reaching = Vector3.Zero;
        if (upperArm is not { } right || offUpperArm is not { } left || ArmLength.Main <= 0) return;
        Transform placed = Placement();
        Vector3 Shoulder(Vector3 joint) => placed.Translation + Vector3.Transform(joint * rig.Scale, placed.Rotation);
        Vector3 best = Vector3.Zero;
        foreach (var (shoulder, hand, length, leftArm) in new[] { (Shoulder(right), mainHand, ArmLength.Main, false), (Shoulder(left), offHand, ArmLength.Off, true) })
        {
            Vector3 toward = Wrist(hand, leftArm).Wrist - shoulder;
            float beyond = toward.Length() - (length - Slack);
            if (beyond > best.Length()) best = Vector3.Normalize(toward) * beyond;
        }
        reaching = best.Length() > MostReaching ? Vector3.Normalize(best) * MostReaching : best;
    }

    /// <summary>The arms' fact for the snapshot: the model placed so its right upper arm sits at the shoulder, facing -Z, on the viewmodel layer.</summary>
    public AppearanceFact Fact(bool shown) => new(Object, false, 0, Placement(), appearance, shown, RenderLayer.Viewmodel);

    /// <summary>
    /// Once the arms are in the published snapshot, reaches each hand for its grip with the hand turned with its look,
    /// its digits shaped as the look says and closed as given. The off hand's grip and turn are already mirrored.
    /// </summary>
    public void Reach(HandAim mainHand, HandAim offHand)
    {
        if (instance is null)
        {
            instance = engine.Animation.CreateInstance(new(appearance, Object));
            engine.Animation.SetPlayback(new(instance, AnimationPlaybackKind.Play, rig.Clip, AnimationLoopMode.Repeat, 1, 1, true, 0, true, 0));
        }
        if (engine.Animation.ReadJointPose(instance) is { Reported: true } drawn)
        {
            JointPose[] joints = drawn.Joints.ToArray();
            drawnJoints = joints;
            upperArm = joints[main.Upper].Model.Translation;
            offUpperArm = joints[off.Upper].Model.Translation;
            if (!main.Measured) main.Measure(joints, rig.Digits);
            if (!off.Measured) off.Measure(joints, rig.Digits);
            ArmLength = (main.Length(joints) * rig.Scale, off.Length(joints) * rig.Scale);
            if (drawn.IkChains.Length == 2) Shortfall = (drawn.IkChains.Span[0].Shortfall * rig.Scale, drawn.IkChains.Span[1].Shortfall * rig.Scale);
        }
        Transform placed = Placement();
        Quaternion unturn = Quaternion.Inverse(placed.Rotation);
        // Camera space into the model's own space, where the IK targets and hand turns are given.
        Vector3 Into(Vector3 point) => Vector3.Transform(point - placed.Translation, unturn) / rig.Scale;
        List<TwoBoneIk> chains = [];
        List<JointOverride> overrides = [];
        foreach (var (side, hand) in new[] { (main, mainHand), (off, offHand) })
        {
            (Vector3 wrist, Quaternion turn) = Wrist(hand, side.Left);
            chains.Add(new(side.Upper, side.Lower, side.Hand, PoseSpace.Model, Into(wrist), Into(hand.Pole), 1));
            if (!side.Measured) continue;
            overrides.Add(JointOverride.Orient(side.Hand, PoseSpace.Model, Quaternion.Normalize(unturn * turn * side.Frame)));
            overrides.AddRange(side.Close(rig, arms.Hands, hand.Shape, hand.Closure).Select(t => JointOverride.AddLocalRotation(t.Joint, t.Turn)));
        }
        engine.Animation.SetPose(new(instance, chains.ToArray(), overrides.ToArray(), true));
    }

    /// <summary>
    /// Where a hand's wrist goes to hold its grip (camera space), and how the hand's canonical frame turns: with the look,
    /// by the look's own hold or else the arms', with the palm, not the wrist, on the grip. The left hand holds as the
    /// right does, mirrored.
    /// </summary>
    public (Vector3 Wrist, Quaternion Turn) Wrist(HandAim hand, bool left)
    {
        Quaternion hold = HeldFrames.Turn(hand.Hold ?? arms.Hold);
        Vector3 palm = rig.Palm;
        // The left hand's frame mirrors the right's, so its palm offset is the right's mirrored; its hold is the right
        // hand's mirrored too when it holds its own (mirrored) look, but given as is on a look held in both hands.
        if (left) palm = HeldFrames.Mirror(palm);
        if (left && hand.HoldMirrored) hold = HeldFrames.Mirror(hold);
        Quaternion turn = Quaternion.Normalize(hand.Look * hold);
        return (hand.Grip - Vector3.Transform(palm, turn), turn);
    }

    public void Dispose()
    {
        instance?.Dispose();
        appearance.Dispose();
        model.Dispose();
    }

    // The model turned and scaled into camera space, its right upper arm (as last drawn) at the arms' shoulder.
    private Transform Placement()
    {
        Quaternion turn = HeldFrames.Turn(rig.Turn);
        Vector3 at = arms.Shoulder + Lean + reaching - Vector3.Transform((upperArm ?? Vector3.Zero) * rig.Scale, turn);
        return new(at, turn, new Vector3(rig.Scale));
    }

    // One arm: its IK chain, its digit joints (index, middle, ring and little fingers, then the thumb), and what is
    // measured from the drawn model: the turn from the hand's canonical frame to its joint's, and each digit joint's
    // curl and swing axes in its own frame.
    private sealed class Side(bool left, uint upper, uint lower, uint hand, uint[][] fingers, uint[] thumb)
    {
        private readonly Dictionary<uint, (Vector3 Curl, Vector3 Swing)> axes = [];

        internal bool Left => left;
        internal uint Upper => upper;
        internal uint Lower => lower;
        internal uint Hand => hand;
        internal bool Measured { get; private set; }
        internal Quaternion Frame { get; private set; } = Quaternion.Identity;
        internal HandShape? Clip { get; private set; }

        internal float Length(JointPose[] joints) =>
            Vector3.Distance(joints[upper].Model.Translation, joints[lower].Model.Translation)
            + Vector3.Distance(joints[lower].Model.Translation, joints[hand].Model.Translation);

        // The knuckles are fixed to the hand, so where they sit in its frame gives the canonical frame whatever the
        // hand's turn; each digit's axes are taken in the joint's own frame, so they hold however the digit has turned.
        internal void Measure(JointPose[] joints, DigitFrames frames)
        {
            Transform wrist = joints[hand].Model;
            Quaternion unturn = Quaternion.Inverse(wrist.Rotation);
            Vector3 Local(uint joint) => Vector3.Transform(joints[joint].Model.Translation - wrist.Translation, unturn);
            Frame = HandFrame.Measure(Local(fingers[0][0]), Local(fingers[1][0]), Local(fingers[3][0]), left);
            Quaternion canonical = Quaternion.Normalize(wrist.Rotation * Quaternion.Inverse(Frame));
            Vector3 x = Vector3.Transform(Vector3.UnitX, canonical), z = Vector3.Transform(Vector3.UnitZ, canonical);
            foreach (uint[] finger in fingers)
                foreach (uint joint in finger)
                    axes[joint] = frames == DigitFrames.Native ? (Vector3.UnitX, Vector3.UnitZ)
                        : (Vector3.Transform(x, Quaternion.Inverse(joints[joint].Model.Rotation)), Vector3.Zero);
            for (int i = 0; i < 3; i++)
            {
                if (frames == DigitFrames.Native) { axes[thumb[i]] = (Vector3.UnitX, Vector3.UnitZ); continue; }
                // A thumb curls across the palm: about the axis square to its own length and the palm's normal (-Z),
                // signed so a positive turn of a negative curl angle brings it toward the palm, as a finger's does.
                Quaternion own = joints[thumb[i]].Model.Rotation;
                Vector3 along = Vector3.Normalize(joints[i < 2 ? thumb[i + 1] : thumb[i]].Model.Translation - joints[i < 2 ? thumb[i] : thumb[i - 1]].Model.Translation);
                Vector3 curl = Vector3.Normalize(Vector3.Cross(-z, along));
                axes[thumb[i]] = (Vector3.Transform(curl, Quaternion.Inverse(own)), Vector3.Transform(z, Quaternion.Inverse(own)));
            }
            Clip = Knuckles(joints);
            Measured = true;
        }

        internal HandShape Knuckles(JointPose[] joints) =>
            HandKnuckles.Read(joints, Quaternion.Normalize(joints[hand].Model.Rotation * Quaternion.Inverse(Frame)), fingers, thumb, left);

        // Each digit joint's turn over the clip: the rig's grip (to the canonical holding pose), the look's shape, then
        // the closure toward the arms' close at 1 or open at -1. Opening blends the shape toward the open bend, so a
        // shaped finger never bends back past it; squeezing adds to the shape. Curls are negative turns about the curl
        // axis (toward the palm); a thumb swings about its swing axis, mirrored on the left.
        internal IEnumerable<(uint Joint, Quaternion Turn)> Close(ArmRig rig, ArmDigits digits, HandShape? shape, float closure)
        {
            DigitBend bend = closure > 0 ? digits.Close : digits.Open;
            HandShape grip = left ? rig.Grip.Off ?? rig.Grip.Main : rig.Grip.Main;
            float weight = MathF.Abs(closure), side = left ? -1 : 1;
            float Bend(float shaped, float target) => closure < 0 ? shaped * (1 - weight) + target * weight : shaped + target * weight;
            const float Radians = MathF.PI / 180;
            for (int f = 0; f < fingers.Length; f++)
                for (int i = 0; i < 3; i++)
                {
                    float curl = grip.Fingers[f][i] + Bend(shape?.Fingers[f][i] ?? 0, bend.Fingers[i]);
                    if (curl != 0) yield return (fingers[f][i], Quaternion.CreateFromAxisAngle(axes[fingers[f][i]].Curl, -curl * Radians));
                }
            for (int i = 0; i < 3; i++)
            {
                float curl = grip.Thumb[i][0] + Bend(shape?.Thumb[i][0] ?? 0, bend.Thumb[i][0]),
                    swing = grip.Thumb[i][1] + Bend(shape?.Thumb[i][1] ?? 0, bend.Thumb[i][1]);
                if (curl == 0 && swing == 0) continue;
                (Vector3 curlAxis, Vector3 swingAxis) = axes[thumb[i]];
                yield return (thumb[i], Quaternion.Normalize(
                    Quaternion.CreateFromAxisAngle(swingAxis, swing * side * Radians) * Quaternion.CreateFromAxisAngle(curlAxis, -curl * Radians)));
            }
        }
    }
}
