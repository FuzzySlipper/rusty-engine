using System.Globalization;
using System.Numerics;
using System.Text.Json;
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
    /// <summary>A held model on the viewmodel layer, in the camera's space: an assembly inspection views look at from outside.</summary>
    private const ulong HeldId = 9785;
    /// <summary>The 0.8 m rod laid forward, a quarter of its length, low and right of the view.</summary>
    private static readonly Transform HeldPlacement = new(new(0.2f, -0.2f, -0.4f), Quaternion.CreateFromYawPitchRoll(0.3f, -1.35f, 0), new(0.3f, 0.3f, 0.3f));
    private const string Clip = "idle";
    private const float CircleRadius = 0.2f;
    private const float CircleSecondsPerTurn = 4f;
    /// <summary>The circle's centre and the pole, from the rest shoulder in world metres.</summary>
    private static readonly Vector3 CircleOffset = new(-0.25f, -0.3f, 0.35f);
    private static readonly Vector3 PoleOffset = new(-0.6f, -0.6f, -0.6f);
    /// <summary>Where <c>pose.far</c> moves the circle: beyond the arm's reach.</summary>
    private static readonly Vector3 FarOffset = new(-1.5f, -0.3f, 0.35f);
    /// <summary>An elbow that moves further than this in one step has flipped.</summary>
    private const float FlipStepMetres = 0.1f;
    private static readonly Quaternion FingerCurl = Quaternion.CreateFromAxisAngle(Vector3.UnitZ, MathF.PI / 2);
    private static readonly string[] Finger = ["RightHandIndex1", "RightHandIndex2", "RightHandIndex3"];
    /// <summary>The overlay set the fixture publishes: one item in each overlay space.</summary>
    private const string GuideSet = "fixture.guide";
    private static readonly Color GuideColor = new(1, 0.2f, 1, 1);
    private static readonly Color HandColor = new(0.2f, 1, 1, 1);
    /// <summary>The landmark pairs the fixture publishes: the hand on its reach circle, the index pad on the held tip, and the finger aimed at the circle.</summary>
    private const string GripSet = "fixture.grip";
    /// <summary>How far from the circle's centre the reaching hand may be: the circle's radius and a margin.</summary>
    private const float ReachTolerance = CircleRadius + 0.05f;
    /// <summary>How near the held tip the index pad must be; it is never there, so the pair draws red.</summary>
    private const float TriggerTolerance = 0.02f;
    private const float AimTolerance = 60f;
    private static readonly Vector3 HeldTip = new(0, 0.8f, 0);

    private readonly IEngineContext engine;
    private readonly Camera camera;
    private readonly RenderResource bodyResource;
    private readonly Appearance body;
    private readonly RenderResource heldResource;
    private readonly Appearance held;
    private readonly AnimationInstance instance;
    private readonly uint arm, forearm, hand;
    private readonly uint[] finger;
    private readonly HandRig rightHand;
    private readonly uint joints;
    private float curl;
    private bool reaching = true;
    private bool far;
    private Vector3? shoulder;
    private Vector3? drawnTarget;
    private Vector3? previousElbow;
    private float handError = float.NaN, worstHandError, worstPoleSide = 1;
    private int reports, flips;
    private double reportedSeconds;
    private float fingerAngle;
    private float chainShortfall = float.NaN, worstShortfallMismatch;
    private bool chainStraightened;

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
        using var heldSource = engine.Content.OpenReference(new("weapon.glb"));
        heldResource = engine.Animation.OpenAnimatedMeshFromContent(new(heldSource));
        held = engine.Animation.CreateAnimatedMeshAppearance(new(heldResource));
        engine.Graphics.PublishSnapshot([
            new(BodyId, false, 0, new(Vector3.Zero, Quaternion.Identity, Vector3.One), body, true, RenderLayer.Scene),
            new(HeldId, false, 0, HeldPlacement, held, true, RenderLayer.Viewmodel),
        ]);
        instance = engine.Animation.CreateInstance(new(body, BodyId));
        engine.Animation.SetPlayback(new(instance, AnimationPlaybackKind.Play, Clip, AnimationLoopMode.Repeat, 1, 1, true, 0, false, 0));
        ReadOnlyMemory<AnimationJointInfo> rig = engine.Animation.ReadJoints(bodyResource);
        joints = (uint)rig.Length;
        arm = AnimationJoints.IndexOf(rig.Span, "RightArm");
        forearm = AnimationJoints.IndexOf(rig.Span, "RightForeArm");
        hand = AnimationJoints.IndexOf(rig.Span, "RightHand");
        finger = [.. Finger.Select(id => AnimationJoints.IndexOf(rig.Span, id))];
        // The character's hand has a thumb and an index finger only, so it has no palm landmark.
        rightHand = HandRig.FromJoints(rig.Span, BodyId, HandSide.Right, "RightHand",
            ["RightHandThumb1", "RightHandThumb2"], Finger, [], [], []);
        // Drawn only while inspection shows it: engine.inspect.overlay show.
        engine.Graphics.PublishOverlaySet(new(GuideSet, [
            OverlayItem.Segment(OverlayPlace.World, new(-1, 0, 0), new(1, 0, 0), GuideColor),
            OverlayItem.WireBox(OverlayPlace.Model(BodyId), new(0, 1.9f, 0), new(1.8f, 1.9f, 0.55f), Quaternion.Identity, GuideColor),
            OverlayItem.Axes(OverlayPlace.AtJoint(BodyId, hand), Vector3.Zero, Quaternion.Identity, 0.15f, onTop: true),
            OverlayItem.Marker(OverlayPlace.AtJoint(BodyId, hand), Vector3.Zero, 0.08f, HandColor, onTop: true),
            OverlayItem.Label(OverlayPlace.AtJoint(BodyId, hand), new(0, 0.12f, 0), "right hand", HandColor, onTop: true),
            OverlayItem.Marker(OverlayPlace.Camera, new(0, 0, -1), 0.02f, GuideColor, onTop: true),
            OverlayItem.Axes(OverlayPlace.Model(HeldId), Vector3.Zero, Quaternion.Identity, 0.05f, onTop: true),
            OverlayItem.Label(OverlayPlace.Model(HeldId), new(0, 0.04f, 0), "held", HandColor, onTop: true),
        ]));
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
        Vector3 target = rest + (far ? FarOffset : CircleOffset) + CircleRadius * new Vector3(MathF.Cos(turn), MathF.Sin(turn), 0);
        TwoBoneIk[] ik = reaching ? [new(arm, forearm, hand, PoseSpace.World, target, rest + PoleOffset, 1)] : [];
        JointOverride[] overrides = [.. finger.Select(joint => JointOverride.AddLocalRotation(joint, FingerCurl, curl))];
        engine.Animation.SetPose(new(instance, ik, overrides));
        drawnTarget = reaching ? target : null;
        return ProductUpdateResult.None;
    }

    private void Observe(AnimationJointPoseResult drawn)
    {
        ReadOnlySpan<JointPose> pose = drawn.Joints.Span;
        reports++;
        reportedSeconds = drawn.Seconds;
        if (shoulder is null)
        {
            shoulder = pose[(int)arm].World.Translation;
            PublishGrip();
        }
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
        if (drawn.IkChains.Length > 0)
        {
            // The Engine's shortfall against the one computed from the reported joints.
            IkChainReach chain = drawn.IkChains.Span[0];
            chainShortfall = chain.Shortfall;
            chainStraightened = chain.Straightened;
            worstShortfallMismatch = MathF.Max(worstShortfallMismatch,
                MathF.Abs(chain.Shortfall - Vector3.Distance(handAt, chain.TargetWorld)));
        }
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

    [DebugCommand("pose.far")]
    public string Far(bool enabled)
    {
        far = enabled;
        previousElbow = null;
        PublishGrip();
        return Inspect();
    }

    /// <summary>
    /// <c>fixture.grip</c> read from C# (<see cref="LandmarkReadConvenience.ReadLandmarks"/>), with
    /// the field names <c>engine.inspect.landmarks fixture.grip</c> answers.
    /// </summary>
    [DebugCommand("pose.landmarks")]
    public string Landmarks()
    {
        LandmarkReadResult read = engine.Graphics.ReadLandmarks(GripSet);
        using MemoryStream stream = new();
        using (Utf8JsonWriter json = new(stream))
        {
            json.WriteStartObject();
            json.WriteBoolean("measured", read.Measured);
            json.WriteNumber("step", read.Step);
            json.WriteNumber("revision", read.Revision);
            json.WriteStartArray("pairs");
            foreach (LandmarkMeasure pair in read.Pairs.Span)
            {
                json.WriteStartObject();
                json.WriteString("name", pair.Name);
                json.WriteBoolean("drawn", pair.Drawn);
                json.WriteBoolean("within", pair.Within);
                json.WriteNumber("distance", pair.Distance);
                if (pair.HasAngle)
                {
                    json.WriteNumber("angle", pair.Angle);
                }
                foreach ((string name, Vector3 value) in new[] { ("from", pair.From), ("to", pair.To), ("offset", pair.Offset) })
                {
                    json.WriteStartArray(name);
                    json.WriteNumberValue(value.X);
                    json.WriteNumberValue(value.Y);
                    json.WriteNumberValue(value.Z);
                    json.WriteEndArray();
                }
                json.WriteEndObject();
            }
            json.WriteEndArray();
            json.WriteEndObject();
        }
        return System.Text.Encoding.UTF8.GetString(stream.ToArray());
    }

    /// <summary>
    /// The landmark pairs, measured by the Engine as drawn (<c>engine.inspect.landmarks fixture.grip</c>):
    /// the wrist within reach of the circle's centre (outside it once <c>pose.far</c> moves the
    /// circle), the index pad on the held model's tip, which it never reaches, and the index
    /// finger's last bone aimed at the circle's centre.
    /// </summary>
    private void PublishGrip()
    {
        if (shoulder is not Vector3 rest)
        {
            return;
        }
        Landmark centre = Landmark.At(OverlayPlace.World, rest + (far ? FarOffset : CircleOffset));
        engine.Graphics.PublishOverlaySet(new(GripSet, [], [
            LandmarkPair.Distance("reach", rightHand.AtWrist(), centre, ReachTolerance),
            LandmarkPair.Distance("trigger", rightHand.Pad(Digit.Index), Landmark.At(OverlayPlace.Model(HeldId), HeldTip), TriggerTolerance),
            LandmarkPair.Facing("pointing", rightHand.Pad(Digit.Index), centre, LandmarkAim.Point, AimTolerance),
        ]));
    }

    [DebugCommand("pose.reset")]
    public string Reset()
    {
        worstHandError = 0;
        worstPoleSide = 1;
        flips = 0;
        worstShortfallMismatch = 0;
        return Inspect();
    }

    [DebugCommand("pose.inspect")]
    public string Inspect() => string.Create(CultureInfo.InvariantCulture,
        $"joints={joints}; reports={reports}; seconds={reportedSeconds:F3}; reaching={reaching}; handError={handError:F4}; worstHandError={worstHandError:F4}; worstPoleSide={worstPoleSide:F3}; flips={flips}; curl={curl:F2}; fingerAngle={fingerAngle:F1}; far={far}; shortfall={chainShortfall:F4}; straightened={chainStraightened}; worstShortfallMismatch={worstShortfallMismatch:F5}");

    public void RegisterDebugCommands(IDebugCommandModuleRegistrar registrar)
    {
        registrar.Register(this);
        registrar.Register(new InspectionTargetsDebugModule(engine.Graphics, Targets));
    }

    /// <summary>What inspection views frame: the right hand's grip, the held model's tip and the space before the camera.</summary>
    private IReadOnlyList<InspectionTarget> Targets() =>
    [
        new("right-hand", OverlayPlace.AtJoint(BodyId, hand), Vector3.Zero) { Radius = 0.15f, Framing = "underside" },
        new("held-tip", OverlayPlace.Model(HeldId), HeldTip) { Radius = 0.08f, Framing = "side" },
        new("view-centre", OverlayPlace.Camera, new(0, 0, -1)) { Radius = 0.2f },
    ];
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
        held.Dispose();
        heldResource.Dispose();
        camera.Dispose();
    }
}
