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
Console.WriteLine("first-person arms exercises passed");

static void ThumbShapesReadBack(DigitFrames frames)
{
    foreach (float swing in new[] { -60f, -30f, 30f, 60f })
    {
        foreach (float curl in new[] { -20f, 0f, 20f })
        {
            HandShape shape = new([[10, 5, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0]], [[curl, swing], [-curl, swing / 2], [curl / 2, -swing / 3]]);
            FakeRig fake = new();
            using FirstPersonArms arms = fake.Arms(Rig(frames));
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

static ArmRig Rig(DigitFrames frames)
{
    ArmJoints Arm(string s) => new($"{s}_upper", $"{s}_lower", $"{s}_hand",
        [.. new[] { "index", "middle", "ring", "little" }.Select(f => new[] { $"{s}_{f}1", $"{s}_{f}2", $"{s}_{f}3" })],
        [$"{s}_thumb1", $"{s}_thumb2", $"{s}_thumb3"]);
    return new("arms.glb", FakeRig.Clip, Vector3.Zero, Arm("r"), Arm("l"), frames, Vector3.Zero, new(HandShape.None));
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

    // The clip pose with the last pose's overrides, parents first (each joint's parent comes before it).
    private JointPose[] Draw()
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
            foreach (JointOverride joint in Pose?.Overrides.ToArray() ?? [])
            {
                if (joint.Joint != i) continue;
                drawn = joint.Space == PoseSpace.Local ? parentTurn * (local = local * joint.Rotation) : joint.Rotation;
                local = Quaternion.Inverse(parentTurn) * drawn;
            }
            model[i] = new(parentAt + Vector3.Transform(offset, parentTurn), Quaternion.Normalize(drawn), Vector3.One);
        }
        return [.. model.Select(t => new JointPose(t, t))];
    }

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
