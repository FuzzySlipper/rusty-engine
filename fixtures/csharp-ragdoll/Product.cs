using System.Globalization;
using System.Numerics;
using Rusty.Engine;
using Rusty.Engine.Debugging;

namespace CsharpRagdoll;

/// <summary>
/// The joint-attachment fixture's character, held in its idle pose on a
/// floor, falls as a ragdoll when it is hit (<c>ragdoll.fall</c>), rests,
/// and gets up again (<c>ragdoll.getup</c>). <c>ragdoll.inspect</c> reports
/// rest, floor clearance, the hinges' least flexion and the pose a fixed
/// number of steps after the hit, so two falls can be compared.
/// </summary>
public sealed class Product : IEngineProduct, IDebugCommandModuleSource, IDebugCommandModule
{
    private const ulong BodyId = 9785, FloorId = 9786;
    private const string Idle = "idle";
    private const float IdleSample = 0.3f;
    private const float FloorHalfSize = 10;
    private const float FloorThickness = 0.2f;
    /// <summary>Steps after the hit at which a fall's pose is recorded.</summary>
    private const int RecordedTick = 480;
    private const float GetUpSeconds = 1;
    private const uint FallGroup = 1;
    /// <summary>The hit: on the chest, from the front.</summary>
    private const int HitBone = 1;
    private static readonly Vector3 HitOffset = new(0.05f, 0.4f, 0.1f);
    private static readonly Vector3 HitImpulse = new(0, 0, -110);

    private sealed record Bone(string Joint, string? End, float Length, DynamicsRagdollShape Shape, float Radius, float HalfDepth, float Mass);
    private sealed record Link(int Parent, int Child, Vector3 Axis, DynamicsJointLimits Limits);

    // The rig's bones run along each joint's +Y. Knees bend about the
    // shins' X; the left elbow about its X and the right about its Z.
    private static readonly Bone[] Bones =
    [
        new("Hips", "Spine", 0, DynamicsRagdollShape.Box, 0.26f, 0.16f, 12),
        new("Spine", "Neck", 0, DynamicsRagdollShape.Box, 0.3f, 0.18f, 24),
        new("Head", null, 0.5f, DynamicsRagdollShape.Capsule, 0.24f, 0, 6),
        new("LeftArm", "LeftForeArm", 0, DynamicsRagdollShape.Capsule, 0.09f, 0, 3),
        new("LeftForeArm", "LeftHand", 0, DynamicsRagdollShape.Capsule, 0.08f, 0, 2),
        new("RightArm", "RightForeArm", 0, DynamicsRagdollShape.Capsule, 0.09f, 0, 3),
        new("RightForeArm", "RightHand", 0, DynamicsRagdollShape.Capsule, 0.08f, 0, 2),
        new("LeftUpLeg", "LeftLeg", 0, DynamicsRagdollShape.Capsule, 0.11f, 0, 8),
        new("LeftLeg", "LeftFoot", 0, DynamicsRagdollShape.Capsule, 0.09f, 0, 4),
        new("RightUpLeg", "RightLeg", 0, DynamicsRagdollShape.Capsule, 0.11f, 0, 8),
        new("RightLeg", "RightFoot", 0, DynamicsRagdollShape.Capsule, 0.09f, 0, 4),
    ];
    private const float JointDamping = 2;
    private static readonly DynamicsJointLimits Elbow = DynamicsJointLimits.Hinge(0, 2.5f, JointDamping);
    private static readonly DynamicsJointLimits Knee = DynamicsJointLimits.Hinge(0, 2.4f, JointDamping);
    private static readonly Link[] Links =
    [
        new(0, 1, Vector3.UnitY, DynamicsJointLimits.Cone(0.5f, -0.3f, 0.3f, JointDamping)),
        new(1, 2, Vector3.UnitY, DynamicsJointLimits.Cone(0.6f, -0.6f, 0.6f, JointDamping)),
        new(1, 3, Vector3.UnitY, DynamicsJointLimits.Cone(1.4f, -0.8f, 0.8f, JointDamping)),
        new(3, 4, Vector3.UnitX, Elbow),
        new(1, 5, Vector3.UnitY, DynamicsJointLimits.Cone(1.4f, -0.8f, 0.8f, JointDamping)),
        new(5, 6, Vector3.UnitZ, Elbow),
        new(0, 7, Vector3.UnitY, DynamicsJointLimits.Cone(1.2f, -0.4f, 0.4f, JointDamping)),
        new(7, 8, Vector3.UnitX, Knee),
        new(0, 9, Vector3.UnitY, DynamicsJointLimits.Cone(1.2f, -0.4f, 0.4f, JointDamping)),
        new(9, 10, Vector3.UnitX, Knee),
    ];

    private readonly IEngineContext engine;
    private readonly Camera camera;
    private readonly RenderResource bodyResource;
    private readonly Appearance body, floor;
    private readonly AnimationInstance instance;
    private readonly SpatialSession spatial;
    private DynamicsWorld world;
    private readonly DynamicsRagdollBone[] bones;
    private readonly DynamicsRagdollLink[] links;
    private readonly float[] lengths;
    private DynamicsRagdoll? ragdoll;
    private bool falling;
    private float blend;
    private bool gettingUp;
    private int ticks, falls;
    private int? restedAt;
    private float lowest = float.MaxValue, leastFlexion = float.MaxValue, lowestNow;
    private string lowestBone = "-";
    private readonly List<string> recorded = [];

    public Product(ProductCreateContext context)
    {
        engine = context.Engine;
        CameraQueries.TryLookAtPose(new(5, 3.5f, 6), new(0, 0.8f, 0), 0, out CameraPose cameraPose);
        camera = engine.CameraView.CreateCamera(new(cameraPose, CameraBasisMode.Derived, default,
            new(CameraProjectionKind.Perspective, 50, 0, .05f, 100), CameraViewports.Full));
        engine.CameraView.SetActiveCamera(camera);
        using var source = engine.Content.OpenReference(new("body.glb"));
        bodyResource = engine.Animation.OpenAnimatedMeshFromContent(new(source));
        body = engine.Animation.CreateAnimatedMeshAppearance(new(bodyResource));
        floor = engine.Graphics.CreatePrimitive(new(PrimitiveGeometry.Cube, false, new Color(.3f, .32f, .35f, 1)));
        engine.Graphics.PublishSnapshot([
            new(BodyId, false, 0, new(Vector3.Zero, Quaternion.Identity, Vector3.One), body, true, RenderLayer.Scene),
            new(FloorId, false, 0, new(new Vector3(0, -FloorThickness / 2, 0), Quaternion.Identity,
                new Vector3(2 * FloorHalfSize, FloorThickness, 2 * FloorHalfSize)), floor, true, RenderLayer.Scene),
        ]);
        instance = engine.Animation.CreateInstance(new(body, BodyId));
        engine.Animation.SetPlayback(new(instance, AnimationPlaybackKind.Sample, Idle, AnimationLoopMode.Repeat, 1, 1, true, 0, false, IdleSample));
        engine.Animation.SetPose(AnimationPoseRequest.ReportOnly(instance));

        spatial = engine.Spatial.CreateSession(new SpatialSessionConfig(1.0, 16, VoxelSurfaceMode.GreedyCubes));
        engine.Spatial.ReplaceCollision(new CollisionReplaceRequest(
            spatial,
            new[] { new StaticMeshAsset(1, 0, 4, 0, 2) },
            new[]
            {
                new Vector3(-FloorHalfSize, 0, -FloorHalfSize),
                new Vector3(FloorHalfSize, 0, -FloorHalfSize),
                new Vector3(FloorHalfSize, 0, FloorHalfSize),
                new Vector3(-FloorHalfSize, 0, FloorHalfSize),
            },
            new[] { new Triangle(0, 2, 1), new Triangle(0, 3, 2) },
            new[] { new StaticMeshInstance(1, 1, new Transform(Vector3.Zero, Quaternion.Identity, Vector3.One)) }));
        world = CreateWorld();

        ReadOnlyMemory<AnimationJointInfo> rig = engine.Animation.ReadJoints(bodyResource);
        uint Joint(string? id) => id is null ? DynamicsRagdollBone.NoEndJoint : AnimationJoints.IndexOf(rig.Span, id);
        bones = [.. Bones.Select(bone => new DynamicsRagdollBone(Joint(bone.Joint), Joint(bone.End), bone.Length, bone.Shape, bone.Radius, bone.HalfDepth, bone.Mass))];
        links = [.. Links.Select(link => new DynamicsRagdollLink((uint)link.Parent, (uint)link.Child, link.Axis, link.Limits))];
        lengths = new float[Bones.Length];
    }

    /// <summary>A world holding only the floor, so every fall starts from the same solver state.</summary>
    private DynamicsWorld CreateWorld()
    {
        DynamicsWorld created = engine.Dynamics.CreateWorld(new DynamicsWorldConfig(new Vector3(0, -9.81f, 0)));
        engine.Dynamics.BindWorldCollision(new DynamicsWorldCollisionBindingRequest(created, spatial));
        return created;
    }

    public ProductUpdateResult Update(ProductUpdate update)
    {
        ProductUpdateFacts facts = update.Facts;
        if (falling && engine.Animation.ReadJointPose(instance).Reported)
        {
            Spawn();
        }
        if (ragdoll is null || facts.AdmittedStepCount == 0)
        {
            return ProductUpdateResult.None;
        }
        engine.Dynamics.Step(new(world, (float)facts.FixedDeltaSeconds, facts.AdmittedStepCount, ReadOnlyMemory<DynamicsAction>.Empty));
        ticks += (int)facts.AdmittedStepCount;
        Observe();
        if (gettingUp)
        {
            blend = MathF.Max(0, blend - (float)(facts.FixedDeltaSeconds * facts.AdmittedStepCount) / GetUpSeconds);
            engine.Dynamics.SetRagdollBlend(new(ragdoll, blend));
            if (blend == 0)
            {
                ragdoll.Dispose();
                ragdoll = null;
                gettingUp = false;
            }
        }
        return ProductUpdateResult.None;
    }

    private void Spawn()
    {
        falling = false;
        ragdoll = engine.Dynamics.CreateRagdoll(new(world, instance, bones, links, FallGroup, uint.MaxValue, 0.8f, 0, 0.05f, 0.3f, 1));
        blend = 1;
        ticks = 0;
        restedAt = null;
        lowest = float.MaxValue;
        leastFlexion = float.MaxValue;
        falls++;
        // Each bone's length, from the pose it spawned in.
        ReadOnlySpan<AnimationJointInfo> rig = engine.Animation.ReadJoints(bodyResource).Span;
        ReadOnlySpan<JointPose> pose = engine.Animation.ReadJointPose(instance).Joints.Span;
        for (int index = 0; index < Bones.Length; index++)
        {
            Bone bone = Bones[index];
            lengths[index] = bone.End is null
                ? bone.Length
                : Vector3.Distance(
                    pose[(int)AnimationJoints.IndexOf(rig, bone.Joint)].World.Translation,
                    pose[(int)AnimationJoints.IndexOf(rig, bone.End)].World.Translation);
        }
        DynamicsRagdollResult spawned = engine.Dynamics.ReadRagdoll(ragdoll);
        Transform chest = spawned.Bones.Span[HitBone];
        engine.Dynamics.ApplyRagdollImpulse(new(ragdoll, HitBone, chest.Translation + Vector3.Transform(HitOffset, chest.Rotation), HitImpulse));
    }

    private void Observe()
    {
        DynamicsRagdollResult read = engine.Dynamics.ReadRagdoll(ragdoll!);
        ReadOnlySpan<Transform> placed = read.Bones.Span;
        if (read.Resting)
        {
            restedAt ??= ticks;
        }
        lowestNow = float.MaxValue;
        for (int index = 0; index < placed.Length; index++)
        {
            float low = Lowest(index, placed);
            lowestNow = MathF.Min(lowestNow, low);
            if (low < lowest)
            {
                lowest = low;
                lowestBone = string.Create(CultureInfo.InvariantCulture, $"{Bones[index].Joint}@{ticks}");
            }
        }
        foreach (Link link in Links.Where(link => link.Limits.Kind == DynamicsJointKind.Hinge))
        {
            Vector3 parent = Vector3.Normalize(Vector3.Transform(Vector3.UnitY, placed[link.Parent].Rotation));
            Vector3 child = Vector3.Normalize(Vector3.Transform(Vector3.UnitY, placed[link.Child].Rotation));
            Vector3 axis = Vector3.Normalize(Vector3.Transform(link.Axis, placed[link.Child].Rotation));
            float flexion = MathF.Atan2(Vector3.Dot(Vector3.Cross(parent, child), axis), Vector3.Dot(parent, child));
            leastFlexion = MathF.Min(leastFlexion, flexion);
        }
        if (ticks >= RecordedTick && recorded.Count < falls)
        {
            Vector3 sum = Vector3.Zero;
            foreach (Transform bone in placed)
            {
                sum += bone.Translation;
            }
            recorded.Add(string.Create(CultureInfo.InvariantCulture, $"{sum.X:F5},{sum.Y:F5},{sum.Z:F5}"));
        }
    }

    /// <summary>The lowest point of a bone's shape, from its joint placement.</summary>
    private float Lowest(int index, ReadOnlySpan<Transform> placed)
    {
        Bone bone = Bones[index];
        Transform joint = placed[index];
        float length = lengths[index];
        Vector3 start = joint.Translation;
        Vector3 end = start + Vector3.Transform(Vector3.UnitY, joint.Rotation) * length;
        if (bone.End is not null)
        {
            // The body runs to where its end joint is drawn, which can be off
            // the joint's own +Y.
            ReadOnlySpan<AnimationJointInfo> rig = engine.Animation.ReadJoints(bodyResource).Span;
            end = engine.Animation.ReadJointPose(instance).Joints.Span[(int)AnimationJoints.IndexOf(rig, bone.End)].World.Translation;
        }
        Vector3 along = Vector3.Normalize(end - start);
        if (bone.Shape == DynamicsRagdollShape.Capsule)
        {
            // The capsule's core ends a radius in from each end of the bone.
            float inset = MathF.Min(bone.Radius, length / 2);
            return MathF.Min((start + along * inset).Y, (end - along * inset).Y) - bone.Radius;
        }
        Vector3 x = Vector3.Transform(Vector3.UnitX, joint.Rotation) * bone.Radius;
        Vector3 z = Vector3.Transform(Vector3.UnitZ, joint.Rotation) * bone.HalfDepth;
        float least = float.MaxValue;
        foreach (Vector3 point in new[] { start, end })
        {
            foreach (float sx in new[] { -1f, 1f })
            {
                foreach (float sz in new[] { -1f, 1f })
                {
                    least = MathF.Min(least, (point + sx * x + sz * z).Y);
                }
            }
        }
        return least;
    }

    [DebugCommand("ragdoll.fall")]
    public string Fall()
    {
        ragdoll?.Dispose();
        ragdoll = null;
        world.Dispose();
        world = CreateWorld();
        gettingUp = false;
        engine.Animation.SetPlayback(new(instance, AnimationPlaybackKind.Sample, Idle, AnimationLoopMode.Repeat, 1, 1, true, 0, false, IdleSample));
        falling = true;
        return Inspect();
    }

    [DebugCommand("ragdoll.getup")]
    public string GetUp()
    {
        engine.Animation.SetPlayback(new(instance, AnimationPlaybackKind.Play, Idle, AnimationLoopMode.Repeat, 1, 1, true, 0, false, 0));
        gettingUp = ragdoll is not null;
        return Inspect();
    }

    [DebugCommand("ragdoll.inspect")]
    public string Inspect() => string.Create(CultureInfo.InvariantCulture,
        $"falls={falls}; ragdoll={ragdoll is not null}; blend={blend:F2}; ticks={ticks}; restedAt={restedAt?.ToString(CultureInfo.InvariantCulture) ?? "-"}; lowest={(lowest == float.MaxValue ? 0 : lowest):F4} ({lowestBone}); lowestNow={lowestNow:F4}; leastFlexion={(leastFlexion == float.MaxValue ? 0 : leastFlexion):F3}; recorded=[{string.Join(" | ", recorded)}]");

    public void RegisterDebugCommands(IDebugCommandModuleRegistrar registrar) => registrar.Register(this);
    public void Start() { }
    public void Pause() { }
    public void Resume() { }
    public void Restart() { }
    public void Shutdown() { }

    public void Dispose()
    {
        ragdoll?.Dispose();
        world.Dispose();
        instance.Dispose();
        engine.Graphics.PublishSnapshot(ReadOnlySpan<AppearanceFact>.Empty);
        body.Dispose();
        floor.Dispose();
        bodyResource.Dispose();
        camera.Dispose();
    }
}
