using System.Globalization;
using System.Numerics;
using Rusty.Engine;
using Rusty.Engine.Debugging;

namespace CsharpSkeletonPoses;

/// <summary>
/// Pose controls on the joint-attachment fixture's character: its right arm
/// reaches a target circling in front of it with a fixed pole (two-bone IK),
/// one weight curls its right index finger over the idle clip, and the
/// reported joints are checked against the pose drawn for the previous call.
/// </summary>
public sealed class Product : IEngineProduct, IDebugCommandModuleSource, IDebugCommandModule
{
    private const ulong BodyId = 9784;
    private const string Clip = "idle";
    private const float CircleRadius = 0.2f;
    private const float CircleSecondsPerTurn = 4f;
    /// <summary>The circle's centre and the pole, from the rest shoulder in world metres.</summary>
    private static readonly Vector3 CircleOffset = new(-0.25f, -0.3f, 0.35f);
    private static readonly Vector3 PoleOffset = new(-0.6f, -0.6f, -0.6f);
    /// <summary>An elbow that moves further than this in one step has flipped.</summary>
    private const float FlipStepMetres = 0.1f;
    private static readonly Quaternion FingerCurl = Quaternion.CreateFromAxisAngle(Vector3.UnitZ, MathF.PI / 2);
    private static readonly string[] Finger = ["RightHandIndex1", "RightHandIndex2", "RightHandIndex3"];

    private readonly IEngineContext engine;
    private readonly Camera camera;
    private readonly RenderResource bodyResource;
    private readonly Appearance body;
    private readonly AnimationInstance instance;
    private readonly uint arm, forearm, hand;
    private readonly uint[] finger;
    private readonly uint joints;
    private float curl;
    private bool reaching = true;
    private Vector3? shoulder;
    private Vector3? drawnTarget;
    private Vector3? previousElbow;
    private float handError = float.NaN, worstHandError, worstPoleSide = 1;
    private int reports, flips;
    private double reportedSeconds;
    private float fingerAngle;

    public Product(ProductCreateContext context)
    {
        engine = context.Engine;
        CameraQueries.TryLookAtPose(new(2.5f, 2.6f, 3.5f), new(-0.3f, 1.8f, 0.3f), 0, out CameraPose cameraPose);
        camera = engine.CameraView.CreateCamera(new(cameraPose, CameraBasisMode.Derived, default,
            new(CameraProjectionKind.Perspective, 50, 0, .05f, 100), CameraViewports.Full));
        engine.CameraView.SetActiveCamera(camera);
        using var source = engine.Content.OpenReference(new("body.glb"));
        bodyResource = engine.Animation.OpenAnimatedMeshFromContent(new(source));
        body = engine.Animation.CreateAnimatedMeshAppearance(new(bodyResource));
        engine.Graphics.PublishSnapshot([new(BodyId, false, 0, new(Vector3.Zero, Quaternion.Identity, Vector3.One), body, true, RenderLayer.Scene)]);
        instance = engine.Animation.CreateInstance(new(body, BodyId));
        engine.Animation.SetPlayback(new(instance, AnimationPlaybackKind.Play, Clip, AnimationLoopMode.Repeat, 1, 1, true, 0, false, 0));
        ReadOnlyMemory<AnimationJointInfo> rig = engine.Animation.ReadJoints(bodyResource);
        joints = (uint)rig.Length;
        arm = AnimationJoints.IndexOf(rig.Span, "RightArm");
        forearm = AnimationJoints.IndexOf(rig.Span, "RightForeArm");
        hand = AnimationJoints.IndexOf(rig.Span, "RightHand");
        finger = [.. Finger.Select(id => AnimationJoints.IndexOf(rig.Span, id))];
        engine.Animation.SetPose(AnimationPoseRequest.ReportOnly(instance));
    }

    public ProductUpdateResult Update(ProductUpdate update)
    {
        // The pose drawn for the previous call, with the target it was given.
        AnimationJointPoseResult drawn = engine.Animation.ReadJointPose(instance);
        if (drawn.Reported)
        {
            Observe(drawn);
        }
        if (shoulder is not Vector3 rest)
        {
            return ProductUpdateResult.None;
        }
        double seconds = update.Facts.SimulationStep * update.Facts.FixedDeltaSeconds;
        float turn = (float)(seconds / CircleSecondsPerTurn * Math.Tau);
        Vector3 target = rest + CircleOffset + CircleRadius * new Vector3(MathF.Cos(turn), MathF.Sin(turn), 0);
        TwoBoneIk[] ik = reaching ? [new(arm, forearm, hand, PoseSpace.World, target, rest + PoleOffset, 1)] : [];
        JointOverride[] overrides = [.. finger.Select(joint => JointOverride.AddLocalRotation(joint, FingerCurl, curl))];
        engine.Animation.SetPose(new(instance, ik, overrides, true));
        drawnTarget = reaching ? target : null;
        return ProductUpdateResult.None;
    }

    private void Observe(AnimationJointPoseResult drawn)
    {
        ReadOnlySpan<JointPose> pose = drawn.Joints.Span;
        reports++;
        reportedSeconds = drawn.Seconds;
        shoulder ??= pose[(int)arm].World.Translation;
        Vector3 handAt = pose[(int)hand].World.Translation;
        Vector3 elbow = pose[(int)forearm].World.Translation;
        if (drawnTarget is Vector3 target && shoulder is Vector3 rest)
        {
            handError = Vector3.Distance(handAt, target);
            worstHandError = MathF.Max(worstHandError, handError);
            Vector3 root = pose[(int)arm].World.Translation;
            Vector3 axis = Vector3.Normalize(target - root);
            Vector3 OffAxis(Vector3 point)
            {
                Vector3 offset = point - root;
                return Vector3.Normalize(offset - axis * Vector3.Dot(offset, axis));
            }
            worstPoleSide = MathF.Min(worstPoleSide, Vector3.Dot(OffAxis(elbow), OffAxis(rest + PoleOffset)));
            if (previousElbow is Vector3 before && Vector3.Distance(before, elbow) > FlipStepMetres)
            {
                flips++;
            }
        }
        previousElbow = elbow;
        // The first finger joint's turn from the hand, in the hand's frame.
        Quaternion relative = Quaternion.Inverse(pose[(int)hand].Model.Rotation) * pose[(int)finger[0]].Model.Rotation;
        fingerAngle = 2 * MathF.Acos(Math.Clamp(MathF.Abs(relative.W), 0, 1)) * 180 / MathF.PI;
    }

    [DebugCommand("pose.curl")]
    public string Curl(float weight)
    {
        curl = Math.Clamp(weight, 0, 1);
        return Inspect();
    }

    [DebugCommand("pose.reach")]
    public string Reach(bool enabled)
    {
        reaching = enabled;
        previousElbow = null;
        return Inspect();
    }

    [DebugCommand("pose.reset")]
    public string Reset()
    {
        worstHandError = 0;
        worstPoleSide = 1;
        flips = 0;
        return Inspect();
    }

    [DebugCommand("pose.inspect")]
    public string Inspect() => string.Create(CultureInfo.InvariantCulture,
        $"joints={joints}; reports={reports}; seconds={reportedSeconds:F3}; reaching={reaching}; handError={handError:F4}; worstHandError={worstHandError:F4}; worstPoleSide={worstPoleSide:F3}; flips={flips}; curl={curl:F2}; fingerAngle={fingerAngle:F1}");

    public void RegisterDebugCommands(IDebugCommandModuleRegistrar registrar) => registrar.Register(this);
    public void Start() { }
    public void Pause() { }
    public void Resume() { }
    public void Restart() { }
    public void Shutdown() { }

    public void Dispose()
    {
        instance.Dispose();
        engine.Graphics.PublishSnapshot(ReadOnlySpan<AppearanceFact>.Empty);
        body.Dispose();
        bodyResource.Dispose();
        camera.Dispose();
    }
}
