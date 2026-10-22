using System.Globalization;
using System.Numerics;
using Rusty.Engine;
using Rusty.Engine.Debugging;

namespace CsharpDismemberment;

/// <summary>
/// The joint-attachment fixture's character runs in place on a floor. A cut
/// (<c>cut.head</c>, <c>cut.arm</c>) severs a joint's subtree mid-run: the
/// Engine stops drawing it on the character and spawns it as its own body,
/// which falls and settles while the character runs on. <c>cut.reset</c>
/// makes the character whole, <c>cut.despawn</c> and <c>cut.spawn</c> remove
/// and bring back the character, and <c>cut.inspect</c> reports the pieces.
/// </summary>
public sealed class Product : IEngineProduct, IDebugCommandModuleSource, IDebugCommandModule
{
    private const ulong BodyId = 10176, FloorId = 10177;
    private const string Run = "run";
    private const float FloorHalfSize = 10;
    private const float FloorThickness = 0.2f;
    private const uint PieceGroup = 2;
    private const float Friction = 0.8f, Restitution = 0.1f, LinearDamping = 0.2f, AngularDamping = 3;

    /// <summary>One cut: the joint, the piece's shape and the push it leaves with.</summary>
    private sealed record Cut(string Name, Func<Func<string, uint>, DynamicsRagdollBone> Shape, Vector3 Push, Vector3 Spin);

    private static readonly Cut[] Cuts =
    [
        new("head", joint => DynamicsRagdollBone.CapsuleAlong(joint("Head"), 0.5f, 0.24f, 6), new(0.8f, 2, 0.5f), new(3, 0, 4)),
        new("arm", joint => DynamicsRagdollBone.Capsule(joint("LeftArm"), joint("LeftHand"), 0.07f, 4), new(1.2f, 1, -0.3f), new(0, 3, 2)),
    ];

    private sealed class Piece(Cut cut, DynamicsSeveredPart part, long cutAt)
    {
        public Cut Cut { get; } = cut;
        public DynamicsSeveredPart Part { get; } = part;
        public long CutAt { get; } = cutAt;
        public long? RestedAt { get; set; }
    }

    private readonly IEngineContext engine;
    private readonly Camera camera;
    private readonly RenderResource bodyResource;
    private readonly Appearance body, floor;
    private readonly SpatialSession spatial;
    private readonly DynamicsWorld world;
    private readonly ReadOnlyMemory<AnimationJointInfo> rig;
    private readonly List<Piece> pieces = [];
    private readonly Queue<Cut> pending = new();
    private AnimationInstance? instance;
    private long steps;
    private uint bodies;

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
        rig = engine.Animation.ReadJoints(bodyResource);

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
        world = engine.Dynamics.CreateWorld(new DynamicsWorldConfig(new Vector3(0, -9.81f, 0)));
        engine.Dynamics.BindWorldCollision(new DynamicsWorldCollisionBindingRequest(world, spatial));
        SpawnCharacter();
    }

    /// <summary>Publish the character running in place; it can be cut once the Engine has posed it.</summary>
    private void SpawnCharacter()
    {
        var facts = new List<AppearanceFact>
        {
            new(FloorId, false, 0, new(new Vector3(0, -FloorThickness / 2, 0), Quaternion.Identity,
                new Vector3(2 * FloorHalfSize, FloorThickness, 2 * FloorHalfSize)), floor, true, RenderLayer.Scene),
            new(BodyId, false, 0, new(Vector3.Zero, Quaternion.Identity, Vector3.One), body, true, RenderLayer.Scene),
        };
        engine.Graphics.PublishSnapshot(facts.ToArray());
        instance = engine.Animation.CreateInstance(new(body, BodyId));
        engine.Animation.SetPlayback(new(instance, AnimationPlaybackKind.Play, Run, AnimationLoopMode.Repeat, 1, 1, true, 0, false, 0));
    }

    public ProductUpdateResult Update(ProductUpdate update)
    {
        ProductUpdateFacts facts = update.Facts;
        while (instance is not null && pending.Count > 0 && engine.Animation.ReadJointPose(instance).Reported)
        {
            Sever(pending.Dequeue());
        }
        if (facts.AdmittedStepCount == 0)
        {
            return ProductUpdateResult.None;
        }
        DynamicsStepReceipt receipt = engine.Dynamics.Step(new(world, (float)facts.FixedDeltaSeconds, facts.AdmittedStepCount, ReadOnlyMemory<DynamicsAction>.Empty));
        steps += facts.AdmittedStepCount;
        bodies = receipt.BodyCount;
        foreach (Piece piece in pieces)
        {
            if (piece.RestedAt is null && Read(piece) is { Resting: true })
            {
                piece.RestedAt = steps;
            }
        }
        return ProductUpdateResult.None;
    }

    private void Sever(Cut cut)
    {
        DynamicsRagdollBone bone = cut.Shape(id => AnimationJoints.IndexOf(rig.Span, id));
        DynamicsSeveredPart part = engine.Dynamics.CreateSeveredPart(new(world, instance!, bone, cut.Push, cut.Spin,
            PieceGroup, uint.MaxValue, Friction, Restitution, LinearDamping, AngularDamping));
        pieces.Add(new(cut, part, steps));
    }

    /// <summary>The piece's readout, or null once the Engine released it.</summary>
    private DynamicsSeveredPartReadout? Read(Piece piece)
    {
        try
        {
            return engine.Dynamics.ReadSeveredPart(piece.Part);
        }
        catch (EngineCallException)
        {
            return null;
        }
    }

    private string Queue(string name)
    {
        Cut cut = Cuts.Single(cut => cut.Name == name);
        if (pieces.All(piece => piece.Cut != cut) && !pending.Contains(cut))
        {
            pending.Enqueue(cut);
        }
        return Inspect();
    }

    [DebugCommand("cut.head")]
    public string CutHead() => Queue("head");

    [DebugCommand("cut.arm")]
    public string CutArm() => Queue("arm");

    /// <summary>Dispose the pieces: the character is whole again.</summary>
    [DebugCommand("cut.reset")]
    public string Reset()
    {
        foreach (Piece piece in pieces)
        {
            piece.Part.Dispose();
        }
        pieces.Clear();
        pending.Clear();
        return Inspect();
    }

    /// <summary>Remove the character, leaving its pieces to the Engine: they go with it.</summary>
    [DebugCommand("cut.despawn")]
    public string Despawn()
    {
        instance?.Dispose();
        instance = null;
        pending.Clear();
        engine.Graphics.PublishSnapshot([
            new(FloorId, false, 0, new(new Vector3(0, -FloorThickness / 2, 0), Quaternion.Identity,
                new Vector3(2 * FloorHalfSize, FloorThickness, 2 * FloorHalfSize)), floor, true, RenderLayer.Scene),
        ]);
        return Inspect();
    }

    [DebugCommand("cut.spawn")]
    public string Spawn()
    {
        if (instance is null)
        {
            Reset();
            SpawnCharacter();
        }
        return Inspect();
    }

    [DebugCommand("cut.inspect")]
    public string Inspect()
    {
        IEnumerable<string> described = pieces.Select(piece =>
        {
            DynamicsSeveredPartReadout? read = Read(piece);
            if (read is not { } readout)
            {
                return $"{piece.Cut.Name}: released";
            }
            Vector3 at = readout.Placement.Translation;
            string rested = piece.RestedAt is { } restedAt ? (restedAt - piece.CutAt).ToString(CultureInfo.InvariantCulture) : "-";
            return string.Create(CultureInfo.InvariantCulture,
                $"{piece.Cut.Name}: at=({at.X:F3},{at.Y:F3},{at.Z:F3}) resting={readout.Resting} restedAfter={rested}");
        });
        string character = "despawned";
        if (instance is not null)
        {
            AnimationJointPoseResult pose = engine.Animation.ReadJointPose(instance);
            if (pose.Reported)
            {
                Vector3 foot = pose.Joints.Span[(int)AnimationJoints.IndexOf(rig.Span, "LeftFoot")].World.Translation;
                character = string.Create(CultureInfo.InvariantCulture,
                    $"running t={pose.Seconds:F3} leftFoot=({foot.X:F3},{foot.Y:F3},{foot.Z:F3})");
            }
        }
        return string.Create(CultureInfo.InvariantCulture,
            $"steps={steps}; bodies={bodies}; character={character}; pending={pending.Count}; pieces=[{string.Join(" | ", described)}]");
    }

    public void RegisterDebugCommands(IDebugCommandModuleRegistrar registrar) => registrar.Register(this);
    public void Start() { }
    public void Pause() { }
    public void Resume() { }
    public void Restart() { }
    public void Shutdown() { }

    public void Dispose()
    {
        Reset();
        world.Dispose();
        instance?.Dispose();
        engine.Graphics.PublishSnapshot(ReadOnlySpan<AppearanceFact>.Empty);
        body.Dispose();
        floor.Dispose();
        bodyResource.Dispose();
        spatial.Dispose();
        camera.Dispose();
    }
}
