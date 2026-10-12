using System.Diagnostics;
using System.Numerics;
using System.Text.Json;
using System.Text.Json.Serialization;
using Rusty.Engine;
using Rusty.Engine.Debugging;

namespace CsharpFloatingBodies;

/// <summary>
/// Bodies floating in Dynamics water: a raft, a ball and a capsule in a still lake, settling at the
/// drafts Archimedes gives; the raft tips under an offset load and rights itself; a second raft in a
/// river is carried downstream until its tether holds it.
/// </summary>
public sealed class Product : IEngineProduct, IDebugCommandModuleSource, IDebugCommandModule
{
    private const ulong RaftId = 1, BallId = 2, CapsuleId = 3, RiverRaftId = 4, TetherLineId = 5, LoadId = 6, LakeId = 7, RiverId = 8, SunId = 9, SkyId = 10;
    private const ulong TetherId = 1;
    private const float Surface = 0, Gravity = 9.81f;
    private static readonly Vector2 LakeMin = new(-20, -15), LakeMax = new(20, 15), RiverMin = new(20, -6), RiverMax = new(70, 6);
    private static readonly Vector2 RiverCurrent = new(1, 0);
    private static readonly Vector3 RaftHalf = new(1, .15f, .75f);
    private const float RaftMass = 180, BallRadius = .3f, BallMass = 40, CapsuleHalfHeight = .5f, CapsuleRadius = .2f, CapsuleMass = 60;
    private const float Load = 60, LoadArm = .8f;
    private static readonly Vector3 RaftStart = new(0, .5f, 0), BallStart = new(-5, .5f, 3), CapsuleStart = new(5, .5f, 3), RiverRaftStart = new(24, .5f, 0);
    private static readonly Vector3 TetherAnchor = new(22, .5f, 0);
    private const float TetherLength = 6;
    /// <summary>A capsule on end is unstable; this spin tips it so it lies down.</summary>
    private static readonly Vector3 CapsuleTip = new(0, 0, 2);
    private const float DraftTolerance = .01f, LevelTolerance = .01f, TipTolerance = .25f, HeldSpeed = .1f;
    private const int CostSteps = 120;
    private const float CostSpacing = 3;
    private static readonly Vector3 CameraEye = new(-12, 9, 18), CameraTarget = new(10, 0, 0);
    private static readonly Color RaftColor = new(.55f, .38f, .2f, 1), BallColor = new(.9f, .2f, .15f, 1), CapsuleColor = new(.95f, .8f, .2f, 1);
    private static readonly Color LoadColor = new(.3f, .3f, .35f, 1), LineColor = new(.9f, .9f, .85f, 1), WaterTint = new(1, 1, 1, .35f);

    private readonly IEngineContext engine;
    private readonly DynamicsWorld world;
    private readonly DynamicsBody raft, ball, capsule, riverRaft;
    private readonly DynamicsWaterPlane[] water;
    private readonly Appearance raftLook, ballLook, capsuleLook, lineLook, loadLook, lakeLook, riverLook;
    private readonly MeshResource lakeMesh, riverMesh;
    private readonly Material waterMaterial;
    private readonly RenderResource foam, ripples;
    private readonly Light sun, sky;
    private readonly Camera camera;
    private bool loaded;
    private double stepMicroseconds;
    private long steps;

    public Product(ProductCreateContext context)
    {
        engine = context.Engine;
        world = engine.Dynamics.CreateWorld(new DynamicsWorldConfig(new Vector3(0, -Gravity, 0)));
        raft = engine.Dynamics.CreateBody(new(world, new DynamicsBodyConfig(new(RaftStart, Quaternion.Identity, Vector3.One), RaftHalf, RaftMass, default, default, 1)));
        riverRaft = engine.Dynamics.CreateBody(new(world, new DynamicsBodyConfig(new(RiverRaftStart, Quaternion.Identity, Vector3.One), RaftHalf, RaftMass, default, default, 1)));
        ball = engine.Dynamics.CreateSphereBodyWithProperties(new(world, new(new(BallStart, Quaternion.Identity, Vector3.One), BallRadius, Properties(BallMass))));
        capsule = engine.Dynamics.CreateCapsuleBody(new(world, new(new(CapsuleStart, Quaternion.Identity, Vector3.One), CapsuleHalfHeight, CapsuleRadius, Properties(CapsuleMass) with { AngularVelocity = CapsuleTip })));
        // The river first: a body takes the first plane that holds it.
        water = [DynamicsWaterPlane.Still(RiverMin, RiverMax, Surface).Flowing(RiverCurrent), DynamicsWaterPlane.Still(LakeMin, LakeMax, Surface)];
        engine.Dynamics.SetWater(new DynamicsWaterRequest(world, water, new[] { raft, ball, capsule, riverRaft }));
        engine.Dynamics.SetFixedTether(new(world, riverRaft, new Vector3(-RaftHalf.X, 0, 0), TetherAnchor, new DynamicsTetherConfig(TetherId, TetherLength, TetherLength, 0, false)));

        Color white = new(1, 1, 1, 1);
        ripples = engine.Graphics.OpenResource(new("ripples.png", TextureFilter.Linear, TextureWrap.Repeat, TextureColorSpace.Linear)).Handle;
        foam = engine.Graphics.OpenResource(new("foam.png", TextureFilter.Linear, TextureWrap.Repeat, TextureColorSpace.Linear)).Handle;
        waterMaterial = engine.Graphics.CreateMaterial(new MaterialRequest(WaterTint, default(RenderResourceReference), .08f, white, Vector3.Zero, 0, true, MaterialAlphaMode.Blend, 0) with
        {
            NormalMap = ripples,
            NormalScale = .6f,
            Water = new(new(.2f, .45f, .4f, 1), new(.03f, .1f, .12f, 1), 2, .3f, .55f, new(.03f, .02f), new(.05f, .03f), new(-.02f, .04f), 3, foam, ripples),
        });
        lakeMesh = engine.Graphics.CreateMeshResource(Slab(LakeMin, LakeMax));
        riverMesh = engine.Graphics.CreateMeshResource(Slab(RiverMin, RiverMax));
        lakeLook = engine.Graphics.CreateMeshAppearance(lakeMesh);
        riverLook = engine.Graphics.CreateMeshAppearance(riverMesh);
        raftLook = engine.Graphics.CreatePrimitive(new(PrimitiveGeometry.Cube, false, RaftColor));
        ballLook = engine.Graphics.CreatePrimitive(new(PrimitiveGeometry.Sphere, false, BallColor));
        capsuleLook = engine.Graphics.CreatePrimitive(new(PrimitiveGeometry.Sphere, false, CapsuleColor));
        loadLook = engine.Graphics.CreatePrimitive(new(PrimitiveGeometry.Cube, false, LoadColor));
        lineLook = engine.Graphics.CreatePrimitive(new(PrimitiveGeometry.Line, false, LineColor));
        sun = engine.Graphics.CreateLight(new(SunId, false, 0, new(LightKind.Directional, new(1, .95f, .85f), 3, true, Vector3.Zero, Vector3.Normalize(new(.3f, -1, .4f)), false, 0, 0, 0, 0, LightShadowIntent.Requested)));
        sky = engine.Graphics.CreateLight(new(SkyId, false, 0, LightDescriptor.Hemisphere(new(.55f, .7f, 1), new(.3f, .3f, .25f), .6f)));
        CameraQueries.TryLookAtPose(CameraEye, CameraTarget, 0, out CameraPose pose);
        camera = engine.CameraView.CreateCamera(new(pose, CameraBasisMode.Derived, default, new(CameraProjectionKind.Perspective, 55, 0, .1f, 300), CameraViewports.Full));
        engine.CameraView.SetActiveCamera(camera);
    }

    private static DynamicsBodyProperties Properties(float mass) =>
        new(mass, default, Vector3.Zero, Vector3.Zero, default, 0, 0, 1, .5f, 0, uint.MaxValue, uint.MaxValue, true, false, false);

    private MeshResourceCreateRequest Slab(Vector2 min, Vector2 max)
    {
        Vector3[] positions = [new(min.X, Surface, min.Y), new(max.X, Surface, min.Y), new(max.X, Surface, max.Y), new(min.X, Surface, max.Y)];
        Vector3[] normals = [Vector3.UnitY, Vector3.UnitY, Vector3.UnitY, Vector3.UnitY];
        Vector2[] uvs = [new(min.X, min.Y), new(max.X, min.Y), new(max.X, max.Y), new(min.X, max.Y)];
        return new(positions, normals, uvs, new uint[] { 0, 2, 1, 0, 3, 2 }, new MeshGroup[] { new(0, 0, 6) }, new MeshMaterialBinding[] { new(0, waterMaterial) });
    }

    public void Start() { }

    public ProductUpdateResult Update(ProductUpdate update)
    {
        ProductUpdateFacts facts = update.Facts;
        if (facts.AdmittedStepCount > 0)
        {
            ReadOnlyMemory<DynamicsAction> actions = loaded ? new[] { LoadOn(raft) } : ReadOnlyMemory<DynamicsAction>.Empty;
            long started = Stopwatch.GetTimestamp();
            engine.Dynamics.Step(new(world, (float)facts.FixedDeltaSeconds, facts.AdmittedStepCount, actions));
            stepMicroseconds += Stopwatch.GetElapsedTime(started).TotalMicroseconds;
            steps += facts.AdmittedStepCount;
        }
        Publish();
        return ProductUpdateResult.None;
    }

    /// <summary>The load's weight on the raft's +x side: a force at its centre and that force's moment.</summary>
    private static DynamicsAction LoadOn(DynamicsBody body) =>
        new(body, new(0, -Load * Gravity, 0), new(0, 0, -Load * Gravity * LoadArm), Vector3.Zero, Vector3.Zero, true);

    private Transform Pose(DynamicsBody body) => engine.Dynamics.Read(new(body)).Transform;

    private void Publish()
    {
        Transform raftPose = Pose(raft), riverPose = Pose(riverRaft), capsulePose = Pose(capsule);
        Vector3 tied = riverPose.Translation + Vector3.Transform(new(-RaftHalf.X, 0, 0), riverPose.Rotation);
        Vector3 line = tied - TetherAnchor;
        Quaternion lineTurn = Turn(Vector3.UnitY, Vector3.Normalize(line));
        Vector3 loadAt = raftPose.Translation + Vector3.Transform(new(LoadArm, RaftHalf.Y + .2f, 0), raftPose.Rotation);
        var facts = new List<AppearanceFact>
        {
            Fact(LakeId, lakeLook, new(Vector3.Zero, Quaternion.Identity, Vector3.One)),
            Fact(RiverId, riverLook, new(Vector3.Zero, Quaternion.Identity, Vector3.One)),
            Fact(RaftId, raftLook, raftPose with { Scale = RaftHalf * 2 }),
            Fact(RiverRaftId, raftLook, riverPose with { Scale = RaftHalf * 2 }),
            Fact(BallId, ballLook, Pose(ball) with { Scale = new(BallRadius * 2) }),
            Fact(CapsuleId, capsuleLook, capsulePose with { Scale = new(CapsuleRadius * 2, (CapsuleHalfHeight + CapsuleRadius) * 2, CapsuleRadius * 2) }),
            Fact(TetherLineId, lineLook, new(TetherAnchor, lineTurn, new(1, line.Length(), 1))),
        };
        if (loaded) facts.Add(Fact(LoadId, loadLook, new(loadAt, raftPose.Rotation, new(.4f))));
        engine.Graphics.PublishSnapshot(facts.ToArray());
    }

    private static AppearanceFact Fact(ulong id, Appearance look, Transform transform) => new(id, false, 0, transform, look, true, RenderLayer.Scene);

    private static Quaternion Turn(Vector3 from, Vector3 to)
    {
        Vector3 axis = Vector3.Cross(from, to);
        float dot = Math.Clamp(Vector3.Dot(from, to), -1, 1);
        return axis.LengthSquared() < 1e-12f ? Quaternion.Identity : Quaternion.CreateFromAxisAngle(Vector3.Normalize(axis), MathF.Acos(dot));
    }

    [DebugCommand("float.load", Description = "Put the offset load on the lake raft, or take it off.")]
    public string SetLoad(bool on) { loaded = on; return Inspect(); }

    [DebugCommand("float.inspect", Description = "Drafts against Archimedes, the raft's roll, the river raft and the water facts.")]
    public string Inspect() => JsonSerializer.Serialize(Observe(), FloatJsonContext.Default.FloatFacts);

    /// <summary>Asserts one stage of the README's headless sequence.</summary>
    [DebugCommand("float.expect", Description = "Assert a stage: drafts, tethered, tipped or righted.")]
    public string Expect(string stage)
    {
        FloatFacts facts = Observe();
        switch (stage)
        {
            case "drafts":
                Require(Math.Abs(facts.RaftDraft - facts.RaftArchimedes) < DraftTolerance, "the raft floats at its Archimedes draft");
                Require(Math.Abs(facts.BallDraft - facts.BallArchimedes) < DraftTolerance, "the ball floats at its Archimedes draft");
                Require(facts.CapsuleLying && Math.Abs(facts.CapsuleDraft - facts.CapsuleArchimedes) < DraftTolerance, "the capsule lies at its Archimedes draft");
                break;
            case "tethered":
                float reach = facts.RiverRaft[0] - TetherAnchor.X - RaftHalf.X;
                Require(reach > .9f * TetherLength && reach < TetherLength + .3f, "the river carried the raft to its tether's length");
                Require(Math.Abs(facts.RiverRaftSpeed) < HeldSpeed, "the tether holds it against the current");
                break;
            case "tipped":
                Require(loaded && Math.Abs(facts.RaftRoll - facts.LoadedRoll) < TipTolerance * Math.Abs(facts.LoadedRoll), "the load tips the raft as hydrostatics says");
                break;
            case "righted":
                Require(!loaded && Math.Abs(facts.RaftRoll) < LevelTolerance, "unloaded, the raft rights itself");
                break;
            default:
                throw new ArgumentException("drafts, tethered, tipped or righted", nameof(stage));
        }
        return Inspect();
    }

    /// <summary>
    /// The cost of water per floating body per step: <paramref name="bodies"/> balls at the surface of
    /// a fresh world, falling dry, and of another, floating, timed around the same generated Step call.
    /// </summary>
    [DebugCommand("float.cost", Description = "Microseconds of water per floating body per step, for that many bodies.")]
    public string Cost(int bodies)
    {
        double dry = TimeBalls(bodies, floating: false), wet = TimeBalls(bodies, floating: true);
        return JsonSerializer.Serialize(new CostFacts(bodies, CostSteps, dry, wet, (wet - dry) / bodies), FloatJsonContext.Default.CostFacts);
    }

    private double TimeBalls(int bodies, bool floating)
    {
        using DynamicsWorld trial = engine.Dynamics.CreateWorld(new DynamicsWorldConfig(new Vector3(0, -Gravity, 0)));
        int side = (int)MathF.Ceiling(MathF.Sqrt(bodies));
        var balls = new DynamicsBody[bodies];
        for (int i = 0; i < bodies; i++)
        {
            Vector3 at = new((i % side) * CostSpacing, Surface, (i / side) * CostSpacing);
            balls[i] = engine.Dynamics.CreateSphereBodyWithProperties(new(trial, new(new(at, Quaternion.Identity, Vector3.One), BallRadius, Properties(BallMass))));
        }
        if (floating)
        {
            Vector2 far = new(side * CostSpacing + 10);
            engine.Dynamics.SetWater(new DynamicsWaterRequest(trial, new[] { DynamicsWaterPlane.Still(-far, far, Surface) }, balls));
        }
        long started = Stopwatch.GetTimestamp();
        for (int step = 0; step < CostSteps; step++) engine.Dynamics.Step(new(trial, 1f / 60, 1, ReadOnlyMemory<DynamicsAction>.Empty));
        double perStep = Stopwatch.GetElapsedTime(started).TotalMicroseconds / CostSteps;
        foreach (DynamicsBody ball in balls) ball.Dispose();
        return perStep;
    }

    private FloatFacts Observe()
    {
        DynamicsReadout raftNow = engine.Dynamics.Read(new(raft)), ballNow = engine.Dynamics.Read(new(ball));
        DynamicsReadout capsuleNow = engine.Dynamics.Read(new(capsule)), riverNow = engine.Dynamics.Read(new(riverRaft));
        Quaternion q = raftNow.Transform.Rotation;
        float roll = MathF.Atan2(2 * (q.W * q.Z + q.X * q.Y), 1 - 2 * (q.Z * q.Z + q.X * q.X));
        Quaternion c = capsuleNow.Transform.Rotation;
        float capsuleAxisY = 1 - 2 * (c.X * c.X + c.Z * c.Z);
        return new(
            Surface - (raftNow.Transform.Translation.Y - RaftHalf.Y), RaftArchimedes(),
            Surface - (ballNow.Transform.Translation.Y - BallRadius), BallArchimedes(),
            Surface - (capsuleNow.Transform.Translation.Y - CapsuleRadius), CapsuleArchimedes(), MathF.Abs(capsuleAxisY) < .1f,
            roll, LoadedRoll(), loaded,
            new[] { riverNow.Transform.Translation.X, riverNow.Transform.Translation.Y, riverNow.Transform.Translation.Z }, riverNow.LinearVelocity.X,
            Water.Of(raftNow.Water), Water.Of(ballNow.Water), Water.Of(capsuleNow.Water), Water.Of(riverNow.Water),
            steps == 0 ? 0 : stepMicroseconds / steps);
    }

    private static float RaftArchimedes() => RaftMass / (DynamicsWaterPlane.FreshWaterDensity * 4 * RaftHalf.X * RaftHalf.Z);

    private static float BallArchimedes() =>
        Depth(BallMass / DynamicsWaterPlane.FreshWaterDensity, 2 * BallRadius, h => Cap(BallRadius, h));

    private static float CapsuleArchimedes() =>
        Depth(CapsuleMass / DynamicsWaterPlane.FreshWaterDensity, 2 * CapsuleRadius,
            d => 2 * CapsuleHalfHeight * Segment(CapsuleRadius, d) + Cap(CapsuleRadius, d));

    /// <summary>The roll an offset load gives: its moment against the loaded raft's metacentric height.</summary>
    private static float LoadedRoll()
    {
        float displaced = (RaftMass + Load) / DynamicsWaterPlane.FreshWaterDensity;
        float draft = displaced / (4 * RaftHalf.X * RaftHalf.Z);
        float secondMoment = 2 * RaftHalf.Z * MathF.Pow(2 * RaftHalf.X, 3) / 12;
        float metacentricHeight = secondMoment / displaced + draft / 2 - RaftHalf.Y;
        return -MathF.Asin(Load * LoadArm / ((RaftMass + Load) * metacentricHeight));
    }

    private static float Depth(float volume, float high, Func<float, float> submerged)
    {
        float low = 0;
        for (int i = 0; i < 60; i++)
        {
            float mid = (low + high) / 2;
            if (submerged(mid) < volume) low = mid; else high = mid;
        }
        return (low + high) / 2;
    }

    private static float Cap(float r, float h) => MathF.PI * h * h * (3 * r - h) / 3;

    private static float Segment(float r, float d)
    {
        float t = d - r;
        return r * r * (MathF.PI - MathF.Acos(t / r)) + t * MathF.Sqrt(r * r - t * t);
    }

    public void RegisterDebugCommands(IDebugCommandModuleRegistrar registrar) => registrar.Register(this);
    private static void Require(bool condition, string message) { if (!condition) throw new InvalidOperationException(message); }
    public void Pause() { }
    public void Resume() { }
    public void Restart() { }
    public void Shutdown() { }

    public void Dispose()
    {
        engine.Graphics.PublishSnapshot([]);
        foreach (IDisposable owner in new IDisposable[] { raftLook, ballLook, capsuleLook, lineLook, loadLook, lakeLook, riverLook, lakeMesh, riverMesh, waterMaterial, foam, ripples, sun, sky, camera, raft, ball, capsule, riverRaft, world })
            owner.Dispose();
    }
}

internal sealed record WaterFacts(bool Floating, float SubmergedFraction, float[] Buoyancy, float[] Drag, float[] Torque);

internal sealed record FloatFacts(
    float RaftDraft, float RaftArchimedes, float BallDraft, float BallArchimedes, float CapsuleDraft, float CapsuleArchimedes, bool CapsuleLying,
    float RaftRoll, float LoadedRoll, bool Loaded, float[] RiverRaft, float RiverRaftSpeed,
    WaterFacts RaftWater, WaterFacts BallWater, WaterFacts CapsuleWater, WaterFacts RiverRaftWater, double StepMicroseconds);

internal sealed record CostFacts(int Bodies, int Steps, double DryStepMicroseconds, double FloatingStepMicroseconds, double WaterMicrosecondsPerBody);

internal static class Water
{
    public static WaterFacts Of(DynamicsWaterFact w) => new(w.Floating, w.SubmergedFraction, [w.Buoyancy.X, w.Buoyancy.Y, w.Buoyancy.Z], [w.Drag.X, w.Drag.Y, w.Drag.Z], [w.Torque.X, w.Torque.Y, w.Torque.Z]);
}

[JsonSourceGenerationOptions(PropertyNamingPolicy = JsonKnownNamingPolicy.CamelCase)]
[JsonSerializable(typeof(FloatFacts))]
[JsonSerializable(typeof(CostFacts))]
internal partial class FloatJsonContext : JsonSerializerContext { }
