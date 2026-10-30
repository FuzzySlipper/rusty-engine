using System.Numerics;
using System.Reflection;
using Rusty.Engine;
using Rusty.Engine.FirstPerson;

// First-person arms over a fake animation service that draws the clip pose with the arms' overrides by forward
// kinematics (no IK: the hands stay where the clip holds them). The hand's thumb joints are turned off the palm's
// frame, as the generated arms' are, so a thumb turned about its own axes would read far from what was asked.
foreach (DigitFrames frames in new[] { DigitFrames.Native, DigitFrames.Measured })
{
    ThumbShapesReadBack(frames);
}
TwistJointsShareTheHandsRoll();
TwistJointsLieOnTheArm();
WristsBendWithinTheirRange();
ElbowsSwingToKeepTheWrist();
ClampedHandsKeepTheirPalmOnTheGrip();
SolvedGripsSeatTheLookInTheHand();
Console.WriteLine("first-person arms exercises passed");

static void ThumbShapesReadBack(DigitFrames frames)
{
    foreach (float swing in new[] { -60f, -30f, 30f, 60f })
    {
        foreach (float curl in new[] { -20f, 0f, 20f })
        {
            HandShape shape = new([[10, 5, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0]], [[curl, swing], [-curl, swing / 2], [curl / 2, -swing / 3]]);
            FakeRig fake = new();
            using FirstPersonArms arms = fake.Arms(Rig(frames, [], []));
            HandAim aim = new(new(0.2f, -0.2f, -0.5f), Quaternion.Identity, null, 0, new(0.5f, -1, 0), Shape: shape);
            // The first reach measures the clip and poses the hands; the second reads the posed hands.
            arms.Reach(aim, aim);
            arms.Reach(aim, aim);
            foreach (bool offHand in new[] { false, true })
            {
                (HandShape clip, HandShape drawn) = arms.Knuckles(offHand)!.Value;
                for (int i = 0; i < 3; i++)
                {
                    for (int a = 0; a < 2; a++)
                    {
                        float asked = clip.Thumb[i][a] + shape.Thumb[i][a];
                        Require(MathF.Abs(Wrap(drawn.Thumb[i][a] - asked)) < 0.5f,
                            $"{frames} {(offHand ? "off" : "main")} thumb [{curl}, {swing}]: joint {i} reads {HandKnuckles.Json(drawn)}, asked {asked} at [{i}][{a}] over {HandKnuckles.Json(clip)}");
                    }
                }
                for (int j = 0; j < 3; j++)
                {
                    Require(MathF.Abs(Wrap(drawn.Fingers[0][j] - clip.Fingers[0][j] - shape.Fingers[0][j])) < 0.5f,
                        $"{frames} index finger reads {HandKnuckles.Json(drawn)} over {HandKnuckles.Json(clip)}");
                }
            }
        }
    }
}

static void TwistJointsShareTheHandsRoll()
{
    FakeRig fake = new();
    using FirstPersonArms arms = fake.Arms(Rig(DigitFrames.Measured, [new("r_forearm2", 0.6f)], [new("l_twist", 0.5f)]));
    HandAim aim = new(new(0.2f, -0.2f, -0.5f), Quaternion.CreateFromAxisAngle(Vector3.UnitZ, MathF.PI / 2), null, 0, new(0.5f, -1, 0));
    arms.Reach(aim, aim);
    JointTwist[] twists = fake.Pose!.Value.Twists.ToArray();
    JointTwist[] expected = [new(fake.Index("r_forearm2"), fake.Index("r_lower"), fake.Index("r_hand"), 0.6f),
        new(fake.Index("l_twist"), fake.Index("l_lower"), fake.Index("l_hand"), 0.5f)];
    Require(twists.SequenceEqual(expected), $"twists sent: {string.Join(", ", twists)}");
}

static void TwistJointsLieOnTheArm()
{
    foreach (string joint in new[] { "root", "r_hand", "r_index1", "l_upper" })
    {
        FakeRig fake = new();
        try
        {
            using FirstPersonArms arms = fake.Arms(Rig(DigitFrames.Measured, [new(joint, 0.5f)], []));
            throw new InvalidOperationException($"a twist joint '{joint}' off the right forearm was taken");
        }
        catch (InvalidOperationException error) when (error.Message.StartsWith("main.twist[0]: ", StringComparison.Ordinal))
        {
        }
    }
}

// The right hand's canonical frame with its wrist neutral on the fake's arm along -Z with its elbow's pole below:
// fingers along -Z, the thumb up (toward the inside of the elbow) and the back of the hand outward (+X).
static Quaternion Neutral() => Quaternion.CreateFromAxisAngle(Vector3.UnitZ, -MathF.PI / 2) * Quaternion.CreateFromAxisAngle(Vector3.UnitX, -MathF.PI / 2);

static float Radians(float degrees) => degrees * MathF.PI / 180;

// The turn from a fake hand's canonical frame to its (unturned) joint's, as the arms measure it from its knuckles.
static Quaternion FakeFrame(bool left)
{
    float side = left ? -1 : 1;
    Vector3 Knuckle(float x) => new Vector3(x * side, 0, -0.69f) - new Vector3(0.2f * side, 0, -0.6f);
    return HandFrame.Measure(Knuckle(0.17f), Knuckle(0.19f), Knuckle(0.23f), left);
}

static (WristBend Main, WristBend Off, JointPose[] Drawn, FakeRig Fake) Reach(Quaternion look, float z, Func<WristRange, WristRange>? range = null,
    Vector3 palm = default)
{
    // Each hand at its clip's wrist (z -0.6, so the arm reaches straight down -Z) or nearer, its elbow's pole below.
    FakeRig fake = new();
    ArmRig rig = Rig(DigitFrames.Measured, [], []);
    rig.Palm = palm;
    if (range is not null) rig.Wrist = range(rig.Wrist);
    using FirstPersonArms arms = fake.Arms(rig);
    HandAim right = new(new(0.2f, -0.2f, z), look, Vector3.Zero, 0, new(0.2f, -1, z / 2));
    HandAim left = right with { Grip = new(-0.2f, -0.2f, z), Look = HeldFrames.Mirror(look), Pole = new(-0.2f, -1, z / 2) };
    arms.Reach(right, left);
    arms.Reach(right, left);
    return (arms.WristClamp.Main, arms.WristClamp.Off, fake.Drawn(), fake);
}

// A drawn hand read off the fake's drawn joints (model space): its bend from the forearm (flexion, ulnar deviation),
// its roll from the inside of the elbow, where its palm sits, and its canonical frame, in the right hand's terms.
static (float Flexion, float Ulnar, float Roll, Vector3 Palm, Quaternion Canonical) Measured(JointPose[] joints, FakeRig fake, bool left, Vector3 palm)
{
    string s = left ? "l" : "r";
    float side = left ? -1 : 1;
    Vector3 a = joints[fake.Index($"{s}_upper")].Model.Translation, b = joints[fake.Index($"{s}_lower")].Model.Translation;
    Transform hand = joints[fake.Index($"{s}_hand")].Model;
    Quaternion canonical = Quaternion.Normalize(hand.Rotation * Quaternion.Inverse(FakeFrame(left)));
    Vector3 forearm = Vector3.Normalize(hand.Translation - b), f = Vector3.Transform(forearm, Quaternion.Inverse(canonical));
    Vector3 along = Vector3.Normalize(hand.Translation - a);
    Vector3 Across(Vector3 v, Vector3 axis) => v - axis * Vector3.Dot(v, axis);
    Vector3 inside = Across(-Across(b - a, along), forearm), thumb = Across(Vector3.Transform(new Vector3(-side, 0, 0), canonical), forearm);
    float roll = MathF.Atan2(Vector3.Dot(Vector3.Cross(inside, thumb), forearm) * side, Vector3.Dot(inside, thumb));
    return (MathF.Atan2(f.Z, f.Y) * 180 / MathF.PI, -MathF.Asin(Math.Clamp(f.X * side, -1, 1)) * 180 / MathF.PI, roll * 180 / MathF.PI,
        hand.Translation + Vector3.Transform(left ? HeldFrames.Mirror(palm) : palm, canonical), left ? HeldFrames.Mirror(canonical) : canonical);
}

static void WristsBendWithinTheirRange()
{
    // A hand asked 100 degrees back (extension), and one rolled 120 degrees palm up (supination), with the elbow held
    // to its pole: each is drawn at the range's edge, as the Engine's solve reaches.
    foreach (var (name, look) in new[] {
        ("extension", Neutral() * Quaternion.CreateFromAxisAngle(Vector3.UnitX, Radians(100))),
        ("roll", Quaternion.CreateFromAxisAngle(-Vector3.UnitZ, Radians(120)) * Neutral()) })
    {
        var (main, off, drawn, fake) = Reach(look, -0.6f, r => r with { Elbow = 0 });
        bool extension = name == "extension";
        foreach (bool offHand in new[] { false, true })
        {
            WristBend bend = offHand ? off : main;
            var hand = Measured(drawn, fake, offHand, Vector3.Zero);
            string at = $"{name} {(offHand ? "off" : "main")}";
            Require(extension ? MathF.Abs(hand.Flexion + 60) < 0.05f : MathF.Abs(hand.Roll - 90) < 0.05f, $"{at}: drawn at {hand}");
            Require((extension ? MathF.Abs(bend.Flexion + 100) : MathF.Abs(bend.Roll - 120)) < 1.5f && MathF.Abs(bend.Clamped - (extension ? 40 : 30)) < 1.5f,
                $"{at}: reported {bend}");
        }
    }
}

static void ClampedHandsKeepTheirPalmOnTheGrip()
{
    // A palm well off the hand joint, held 100 degrees back, at full reach and with the elbow bent, the elbow held or
    // free: the drawn hand is within the range of the forearm the solve drew, its palm on the grip, and the reported
    // correction is the one drawn.
    Vector3 palm = new(0.01f, 0.08f, -0.03f);
    Quaternion look = Neutral() * Quaternion.CreateFromAxisAngle(Vector3.UnitX, Radians(100));
    foreach (float z in new[] { -0.55f, -0.35f })
        foreach (float elbow in new[] { 0f, 30f })
        {
            WristRange range = new() { Elbow = elbow };
            var (main, off, drawn, fake) = Reach(look, z, r => range, palm);
            foreach (bool offHand in new[] { false, true })
            {
                WristBend bend = offHand ? off : main;
                var hand = Measured(drawn, fake, offHand, palm);
                string at = $"z {z} elbow {elbow} {(offHand ? "off" : "main")}";
                const float Slop = 0.05f;
                Require(hand.Flexion >= -range.Extension - Slop && hand.Flexion <= range.Flexion + Slop
                    && hand.Ulnar >= -range.Radial - Slop && hand.Ulnar <= range.Ulnar + Slop && MathF.Abs(hand.Roll) <= range.Roll + Slop,
                    $"{at}: drawn at {hand}");
                // Model space is camera space less the shoulder's placement (0, -0.2, 0).
                Vector3 grip = new((offHand ? -1 : 1) * 0.2f, 0, z);
                Require(Vector3.Distance(hand.Palm, grip) < 1e-4f, $"{at}: palm at {hand.Palm}, grip {grip}");
                float turned = 2 * MathF.Acos(MathF.Min(1, MathF.Abs(Quaternion.Dot(hand.Canonical, look)))) * 180 / MathF.PI;
                Require(bend.Clamped > 10 && MathF.Abs(bend.Clamped - turned) < Slop, $"{at}: reported {bend}, drawn {turned} degrees from the ask");
            }
        }
}

static void ElbowsSwingToKeepTheWrist()
{
    var (rolledMain, rolledOff, _, _) = Reach(Quaternion.CreateFromAxisAngle(-Vector3.UnitZ, Radians(110)) * Neutral(), -0.6f);
    Require(rolledMain.Clamped < .5f && rolledOff.Clamped < .5f && MathF.Abs(rolledMain.Elbow) >= 20 && MathF.Abs(rolledOff.Elbow) >= 20,
        $"a straight arm rolled 110 degrees: {rolledMain}, {rolledOff}");

    // A hand nearer the shoulder, so the elbow bends and the forearm rises to it, turned outward past the range
    // (extension): the elbow swings in to lay the forearm under it and less is clamped than with the elbow held.
    Quaternion look = Quaternion.CreateFromAxisAngle(Vector3.UnitY, Radians(-70)) * Neutral();
    WristBend held = Reach(look, -0.45f, r => r with { Elbow = 0 }).Main, swung = Reach(look, -0.45f).Main;
    Require(held.Elbow == 0 && swung.Elbow != 0 && MathF.Abs(swung.Elbow) <= 30, $"elbow swung {held.Elbow} held, {swung.Elbow} free");
    Require(held.Clamped > 1 && swung.Clamped < held.Clamped - 1, $"clamped {held.Clamped} degrees with the elbow held, {swung.Clamped} with it swinging");
}

static void SolvedGripsSeatTheLookInTheHand()
{
    // A solve's look placement in the hand's canonical frame holds wherever the look is turned.
    SolvedGrip solved = new(HandShape.None, new(new(new(0.0002f, 0.0743f, -0.0294f), [0.43225f, -0.42867f, 0.55961f, -0.56236f])));
    FakeRig fake = new();
    using FirstPersonArms arms = fake.Arms(Rig(DigitFrames.Measured, [], []));
    Quaternion look = Quaternion.CreateFromYawPitchRoll(0.3f, -0.5f, 0.2f);
    Vector3 grip = new(0.15f, -0.25f, -0.5f);
    (Vector3 wrist, Quaternion turn) = arms.Wrist(new(grip, look, new(40, 0, 0), 0, new(0.5f, -1, 0), Solved: solved), false);
    Vector3 position = Vector3.Transform(grip - wrist, Quaternion.Inverse(turn));
    Quaternion placed = Quaternion.Normalize(Quaternion.Inverse(turn) * look);
    Require(Vector3.Distance(position, solved.Palm) < 1e-5f && MathF.Abs(Quaternion.Dot(placed, solved.Object.InCanonical.Turn)) > 0.99999f,
        $"the look sits at {position}, {placed} in the hand; solved {solved.Palm}, {solved.Object.InCanonical.Turn}");
}

static ArmRig Rig(DigitFrames frames, ArmTwist[] mainTwist, ArmTwist[] offTwist)
{
    ArmJoints Arm(string s, ArmTwist[] twist) => new($"{s}_upper", $"{s}_lower", $"{s}_hand",
        [.. new[] { "index", "middle", "ring", "little" }.Select(f => new[] { $"{s}_{f}1", $"{s}_{f}2", $"{s}_{f}3" })],
        [$"{s}_thumb1", $"{s}_thumb2", $"{s}_thumb3"]) { Twist = twist };
    return new("arms.glb", FakeRig.Clip, Vector3.Zero, Arm("r", mainTwist), Arm("l", offTwist), frames, Vector3.Zero, new(HandShape.None));
}

static float Wrap(float degrees) => degrees > 180 ? degrees - 360 : degrees <= -180 ? degrees + 360 : degrees;

static void Require(bool condition, string message)
{
    if (!condition)
    {
        throw new InvalidOperationException(message);
    }
}

/// <summary>
/// A two-armed rig facing -Z with the arms along -Z, palms down. The right forearm has a second segment between the
/// lower arm and the hand (as a CAT rig's), the left a twist joint beside the hand (as UE's). Each thumb's joints are
/// turned well off the hand's frame.
/// </summary>
sealed class FakeRig
{
    public const string Clip = "hold";

    private readonly List<(string Id, int Parent, Vector3 At, Quaternion Turn)> joints = [];

    public FakeRig()
    {
        Add("root", -1, Vector3.Zero, Quaternion.Identity);
        foreach (float side in new[] { 1f, -1f })
        {
            string s = side > 0 ? "r" : "l";
            Quaternion Mirror(Quaternion q) => side > 0 ? q : new(q.X, -q.Y, -q.Z, q.W);
            Vector3 At(float x, float y, float z) => new(x * side, y, z);
            int upper = Add($"{s}_upper", 0, At(0.2f, 0, 0), Quaternion.Identity);
            int lower = Add($"{s}_lower", upper, At(0.2f, 0, -0.3f), Quaternion.Identity);
            int carrier = lower;
            if (side > 0) carrier = Add("r_forearm2", lower, At(0.2f, 0, -0.45f), Quaternion.Identity);
            else Add("l_twist", lower, At(0.2f, 0, -0.45f), Quaternion.Identity);
            int hand = Add($"{s}_hand", carrier, At(0.2f, 0, -0.6f), Quaternion.Identity);
            string[] fingers = ["index", "middle", "ring", "little"];
            for (int f = 0; f < 4; f++)
            {
                int parent = hand;
                for (int j = 0; j < 3; j++)
                    parent = Add($"{s}_{fingers[f]}{j + 1}", parent, At(0.17f + 0.02f * f, 0, -0.69f - 0.03f * j), Quaternion.Identity);
            }
            Vector3[] thumb = [At(0.165f, -0.01f, -0.62f), At(0.14f, -0.02f, -0.65f), At(0.13f, -0.025f, -0.68f)];
            Quaternion[] turns = [Quaternion.CreateFromYawPitchRoll(0.7f, 0.4f, 1.1f), Quaternion.CreateFromYawPitchRoll(-0.5f, 0.9f, 0.3f),
                Quaternion.CreateFromYawPitchRoll(1.3f, -0.2f, -0.8f)];
            int at = hand;
            for (int j = 0; j < 3; j++) at = Add($"{s}_thumb{j + 1}", at, thumb[j], Mirror(turns[j]));
        }
    }

    public AnimationPoseRequest? Pose { get; private set; }

    /// <summary>The joints as drawn now.</summary>
    public JointPose[] Drawn() => Draw();

    public uint Index(string id) => (uint)joints.FindIndex(j => j.Id == id);

    public FirstPersonArms Arms(ArmRig rig)
    {
        Animation animation = DispatchProxy.Create<IAnimationService, Animation>() as Animation ?? throw new InvalidOperationException();
        animation.Rig = this;
        Engine engine = DispatchProxy.Create<IEngineContext, Engine>() as Engine ?? throw new InvalidOperationException();
        engine.Animation = (IAnimationService)animation;
        HeldArms held = new(new(0.2f, -0.2f, 0), new(0.5f, -1, 0), Vector3.Zero,
            new(new([0, 0, 0], [[0, 0], [0, 0], [0, 0]]), new([0, 0, 0], [[0, 0], [0, 0], [0, 0]]), HandShape.None));
        return new FirstPersonArms((IEngineContext)engine, held, rig, 1, new ContentReference(new(1), () => { }));
    }

    private int Add(string id, int parent, Vector3 at, Quaternion turn)
    {
        joints.Add((id, parent, at, turn));
        return joints.Count - 1;
    }

    // The clip pose with the last pose's two-bone IK, then its overrides, parents first (each joint's parent comes before
    // it). Each chain is solved as the Engine solves it: the root turned so the mid joint lies on the bend the pole
    // selects, then the mid turned so the end reaches the target, each turn about the joint's origin.
    private JointPose[] Draw()
    {
        Dictionary<int, Quaternion> turned = [];
        foreach (TwoBoneIk chain in Pose?.TwoBoneIk.ToArray() ?? [])
        {
            const float Margin = 1e-4f;
            Transform[] posed = Forward(turned, false);
            Vector3 a = posed[chain.Root].Translation, b = posed[chain.Mid].Translation, c = posed[chain.End].Translation;
            float upper = Vector3.Distance(a, b), lower = Vector3.Distance(b, c), reach = Vector3.Distance(a, chain.Target);
            Vector3 along = (chain.Target - a) / reach;
            float span = upper + lower, distance = Math.Clamp(reach, MathF.Abs(upper - lower) + span * Margin, span * (1 - Margin));
            Vector3 OffAxis(Vector3 point) => point - a - along * Vector3.Dot(point - a, along);
            Vector3 bend = Vector3.Normalize(OffAxis(chain.Pole).Length() > 1e-6f ? OffAxis(chain.Pole) : OffAxis(b));
            float cos = Math.Clamp((upper * upper + distance * distance - lower * lower) / (2 * upper * distance), -1, 1);
            Vector3 mid = a + (along * cos + bend * MathF.Sqrt(1 - cos * cos)) * upper, end = a + along * distance;
            turned[(int)chain.Root] = Arc((b - a) / upper, (mid - a) / upper) * turned.GetValueOrDefault((int)chain.Root, Quaternion.Identity);
            posed = Forward(turned, false);
            b = posed[chain.Mid].Translation;
            c = posed[chain.End].Translation;
            turned[(int)chain.Mid] = Arc(Vector3.Normalize(c - b), Vector3.Normalize(end - b)) * turned.GetValueOrDefault((int)chain.Mid, Quaternion.Identity);
        }
        return [.. Forward(turned, true).Select(t => new JointPose(t, t))];
    }

    // The clip pose with each joint in `turned` turned about its own origin (model space) and, when asked, the pose's
    // overrides.
    private Transform[] Forward(Dictionary<int, Quaternion> turned, bool overridden)
    {
        Transform[] model = new Transform[joints.Count];
        for (int i = 0; i < joints.Count; i++)
        {
            (string _, int parent, Vector3 at, Quaternion turn) = joints[i];
            Quaternion parentTurn = parent < 0 ? Quaternion.Identity : model[parent].Rotation;
            Quaternion clipParentTurn = parent < 0 ? Quaternion.Identity : joints[parent].Turn;
            Vector3 parentAt = parent < 0 ? Vector3.Zero : model[parent].Translation;
            Vector3 clipParentAt = parent < 0 ? Vector3.Zero : joints[parent].At;
            // The clip's local transform, under the parent as drawn.
            Quaternion local = Quaternion.Inverse(clipParentTurn) * turn;
            Vector3 offset = Vector3.Transform(at - clipParentAt, Quaternion.Inverse(clipParentTurn));
            Quaternion drawn = parentTurn * local;
            if (turned.TryGetValue(i, out Quaternion extra)) drawn = extra * drawn;
            local = Quaternion.Inverse(parentTurn) * drawn;
            foreach (JointOverride joint in overridden ? Pose?.Overrides.ToArray() ?? [] : [])
            {
                if (joint.Joint != i) continue;
                drawn = joint.Space == PoseSpace.Local ? parentTurn * (local = local * joint.Rotation) : joint.Rotation;
                local = Quaternion.Inverse(parentTurn) * drawn;
            }
            model[i] = new(parentAt + Vector3.Transform(offset, parentTurn), Quaternion.Normalize(drawn), Vector3.One);
        }
        return model;
    }

    private static Quaternion Arc(Vector3 from, Vector3 to) => Quaternion.Normalize(new Quaternion(Vector3.Cross(from, to), 1 + Vector3.Dot(from, to)));

    public class Animation : DispatchProxy
    {
        internal FakeRig Rig = null!;

        protected override object? Invoke(MethodInfo? method, object?[]? args) => method!.Name switch
        {
            nameof(IAnimationService.OpenAnimatedMeshFromContent) => new RenderResource(new(1), () => { }),
            nameof(IAnimationService.ReadJoints) => new ReadOnlyMemory<AnimationJointInfo>(
                [.. Rig.joints.Select(j => new AnimationJointInfo(j.Id, j.Parent < 0 ? uint.MaxValue : (uint)j.Parent))]),
            nameof(IAnimationService.ReadClips) => new ReadOnlyMemory<AnimationClipInfo>([new(Clip, Clip, 1, true, 0)]),
            nameof(IAnimationService.CreateAnimatedMeshAppearance) => new Appearance(new(1), () => { }),
            nameof(IAnimationService.CreateInstance) => new AnimationInstance(new(1), () => { }),
            nameof(IAnimationService.SetPlayback) => null,
            nameof(IAnimationService.SetPose) => Rig.Pose = (AnimationPoseRequest)args![0]!,
            nameof(IAnimationService.ReadJointPose) => new AnimationJointPoseResult(Rig.Draw(), ReadOnlyMemory<IkChainReach>.Empty,
                new(Vector3.Zero, Quaternion.Identity, Vector3.One), 0, true),
            _ => throw new NotSupportedException(method.Name),
        };
    }

    public class Engine : DispatchProxy
    {
        internal IAnimationService Animation = null!;

        protected override object? Invoke(MethodInfo? method, object?[]? args) =>
            method!.Name == "get_Animation" ? Animation : throw new NotSupportedException(method.Name);
    }
}
