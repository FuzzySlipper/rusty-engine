using System.Diagnostics;
using System.Numerics;
using System.Text.Json;
using System.Text.Json.Serialization;
using Rusty.Engine;
using Rusty.Engine.Debugging;

namespace CsharpSoftBodies;

/// <summary>
/// Engine soft bodies: a flag pinned to a pole flaps in gusting wind against a held ball; a jelly
/// squashes under a dropped ball or a press from above and springs back; a blob jumps and wobbles.
/// The Engine simulates and draws them; this product chooses their shapes, materials, pins, wind
/// and kicks.
/// </summary>
public sealed class Product : IEngineProduct, IDebugCommandModuleSource, IDebugCommandModule
{
    private const ulong FloorId = 1, PoleId = 2, ObstacleId = 3, FlagId = 4, JellyId = 5, BlobId = 6, BallId = 7, SunId = 8, SkyId = 9;
    private const float Gravity = 9.81f, FloorHalfSize = 15, FloorThickness = .2f;
    // The flag: 20 × 13 particles, 2 m by 1.2 m, its hoist pinned along the pole.
    private const uint FlagColumns = 21, FlagRows = 13;
    private const float FlagCell = .1f, FlagMass = .6f, FlagThickness = .02f;
    private static readonly Vector3 PoleFoot = new(-3, 0, 0);
    private const float PoleHeight = 4, PoleHalfWidth = .04f, FlagTop = 3.8f;
    /// <summary>The hoist stands this far from the pole's axis, clear of its surface.</summary>
    private const float HoistGap = .07f;
    // Wind blows along +z; a held ball stands downwind of the pole, in the flag's way.
    private const float WindSpeed = 7, GustShare = .35f, FlutterShare = .15f, GustPeriod = 3.1f, FlutterPeriod = .7f;
    private static readonly Vector3 Obstacle = new(-2.75f, 3.2f, 1.25f);
    private const float ObstacleRadius = .3f;
    /// <summary>The flag's extra substeps: flapping, it moves its own thickness in a fraction of a step.</summary>
    private const uint FlagExtraSubsteps = 3;
    // The jelly: a 0.7 m cube of 6 particles a side, 20 kg, 6 kPa.
    private static readonly Vector3 JellyCenter = new(2, .36f, 0);
    private const float JellyHalf = .35f, JellyMass = 20, JellyStiffness = 6e3f, JellyThickness = .015f;
    private const uint JellyParticles = 6;
    // The blob: a 0.4 m shell refined twice, 5 kg.
    private static readonly Vector3 BlobCenter = new(4.5f, .45f, 0);
    private const float BlobRadius = .4f, BlobMass = 5, JumpSpeed = 5;
    private const uint BlobSubdivisions = 2;
    // The dropped ball and the press.
    private const float BallRadius = .25f, BallMass = 30, DropHeight = 3, DropSpeed = 3, SquashSpeed = 8, SquashReach = .8f;
    // Expectations.
    private const float FlutterOffset = .3f, FlutterSwing = .02f, TouchSlack = .05f, SquashShare = .9f, RecoverShare = .03f, VolumeShare = .97f, JumpRise = .5f;
    private const int SwingWindow = 60, CostSteps = 120;
    private static readonly Vector3 CameraEye = new(-8, 3.5f, -3), CameraTarget = new(-.5f, 1.8f, .8f);
    private static readonly Color FloorColor = new(.3f, .32f, .35f, 1), PoleColor = new(.75f, .75f, .7f, 1), ObstacleColor = new(.25f, .45f, .8f, 1);
    private static readonly Color JellyColor = new(.35f, .85f, .4f, 1), BlobColor = new(.95f, .45f, .6f, 1), BallColor = new(.4f, .4f, .42f, 1), White = new(1, 1, 1, 1);

    private readonly IEngineContext engine;
    private readonly SpatialSession spatial;
    private readonly DynamicsWorld world;
    private readonly DynamicsBody obstacle;
    private readonly DynamicsSoftBody flag, jelly, blob;
    private readonly RenderResource flagTexture;
    private readonly Material flagMaterial, jellyMaterial, blobMaterial;
    private readonly Appearance floorLook, poleLook, obstacleLook, ballLook, flagLook, jellyLook, blobLook;
    private readonly Light sun, sky;
    private readonly Camera camera;
    private readonly float jellyRestHeight, blobRestHeight;
    private DynamicsBody? ball;
    private float windSpeed = WindSpeed;
    private double seconds, stepMicroseconds;
    private long steps;
    private readonly Queue<float> flagSwing = new();
    private float closestToObstacle = float.MaxValue, jellyLowest = float.MaxValue, blobHighest;
    private uint flagContactsMost;

    public Product(ProductCreateContext context)
    {
        engine = context.Engine;
        spatial = engine.Spatial.CreateSession(new SpatialSessionConfig(1.0, 16, VoxelSurfaceMode.GreedyCubes));
        engine.Spatial.ReplaceCollision(Ground());
        world = engine.Dynamics.CreateWorld(new DynamicsWorldConfig(new Vector3(0, -Gravity, 0)));
        engine.Dynamics.BindWorldCollision(new DynamicsWorldCollisionBindingRequest(world, spatial));
        obstacle = engine.Dynamics.CreateSphereBodyWithProperties(new(world, new(new(Obstacle, Quaternion.Identity, Vector3.One), ObstacleRadius, Held())));

        DynamicsSoftBodyRequest flagShape = DynamicsSoftBodyRequest.Cloth(world,
            new(PoleFoot.X + HoistGap, FlagTop, PoleFoot.Z), new(FlagCell, 0, 0), new(0, -FlagCell, 0), FlagColumns, FlagRows, FlagMass);
        uint[] hoist = [.. Enumerable.Range(0, (int)FlagRows).Select(row => flagShape.ClothParticle(0, (uint)row))];
        flag = engine.Dynamics.CreateSoftBody(flagShape with { Pinned = hoist, ParticleRadius = FlagThickness, ExtraSubsteps = FlagExtraSubsteps });
        jelly = engine.Dynamics.CreateSoftBody(DynamicsSoftBodyRequest.Box(world, JellyCenter, new(JellyHalf), JellyParticles, JellyMass, JellyStiffness) with
        {
            ParticleRadius = JellyThickness,
        });
        blob = engine.Dynamics.CreateSoftBody(DynamicsSoftBodyRequest.Shell(world, BlobCenter, BlobRadius, BlobSubdivisions, BlobMass));
        jellyRestHeight = Height(engine.Dynamics.ReadSoftBody(jelly));
        blobRestHeight = engine.Dynamics.ReadSoftBody(blob).CenterOfMass.Y;

        flagTexture = engine.Graphics.OpenResource(new("flag.png", TextureFilter.Linear, TextureWrap.Clamp, TextureColorSpace.Srgb, "")).Handle;
        flagMaterial = engine.Graphics.CreateMaterial(new MaterialRequest(White, flagTexture, .8f, White, Vector3.Zero, 0, false, MaterialAlphaMode.Opaque, 0));
        jellyMaterial = engine.Graphics.CreateMaterial(new MaterialRequest(JellyColor, default(RenderResourceReference), .25f, White, Vector3.Zero, 0, false, MaterialAlphaMode.Opaque, 0));
        blobMaterial = engine.Graphics.CreateMaterial(new MaterialRequest(BlobColor, default(RenderResourceReference), .4f, White, Vector3.Zero, 0, false, MaterialAlphaMode.Opaque, 0));
        // Cloth is seen from both sides; a solid only from outside.
        flagLook = engine.Dynamics.CreateSoftBodyAppearance(new(flag, flagMaterial, true));
        jellyLook = engine.Dynamics.CreateSoftBodyAppearance(new(jelly, jellyMaterial, false));
        blobLook = engine.Dynamics.CreateSoftBodyAppearance(new(blob, blobMaterial, false));
        floorLook = engine.Graphics.CreatePrimitive(new(PrimitiveGeometry.Cube, false, FloorColor));
        poleLook = engine.Graphics.CreatePrimitive(new(PrimitiveGeometry.Cube, false, PoleColor));
        obstacleLook = engine.Graphics.CreatePrimitive(new(PrimitiveGeometry.Sphere, false, ObstacleColor));
        ballLook = engine.Graphics.CreatePrimitive(new(PrimitiveGeometry.Sphere, false, BallColor));
        sun = engine.Graphics.CreateLight(new(SunId, false, 0, new(LightKind.Directional, new(1, .95f, .85f), 3, true, Vector3.Zero, Vector3.Normalize(new(.4f, -1, .5f)), false, 0, 0, 0, 0, LightShadowIntent.Requested)));
        sky = engine.Graphics.CreateLight(new(SkyId, false, 0, LightDescriptor.Hemisphere(new(.55f, .7f, 1), new(.3f, .3f, .25f), .6f)));
        CameraQueries.TryLookAtPose(CameraEye, CameraTarget, 0, out CameraPose pose);
        camera = engine.CameraView.CreateCamera(new(pose, CameraBasisMode.Derived, default, new(CameraProjectionKind.Perspective, 55, 0, .1f, 200), CameraViewports.Full));
        engine.CameraView.SetActiveCamera(camera);
    }

    /// <summary>The floor and the pole, as the world's static collision.</summary>
    private CollisionReplaceRequest Ground()
    {
        Vector3 poleMin = PoleFoot - new Vector3(PoleHalfWidth, 0, PoleHalfWidth), poleMax = PoleFoot + new Vector3(PoleHalfWidth, PoleHeight, PoleHalfWidth);
        Vector3[] vertices =
        [
            new(-FloorHalfSize, 0, -FloorHalfSize), new(FloorHalfSize, 0, -FloorHalfSize), new(FloorHalfSize, 0, FloorHalfSize), new(-FloorHalfSize, 0, FloorHalfSize),
            .. Enumerable.Range(0, 8).Select(corner => new Vector3(
                (corner & 1) == 0 ? poleMin.X : poleMax.X, (corner & 2) == 0 ? poleMin.Y : poleMax.Y, (corner & 4) == 0 ? poleMin.Z : poleMax.Z)),
        ];
        // Outward-wound box faces over the pole's corners (x | y << 1 | z << 2);
        // an asset's triangles count from its first vertex.
        uint[][] faces = [[0, 2, 1], [1, 2, 3], [4, 5, 6], [5, 7, 6], [0, 1, 4], [1, 5, 4], [2, 6, 3], [3, 6, 7], [0, 4, 2], [2, 4, 6], [1, 3, 5], [3, 7, 5]];
        Triangle[] triangles = [new(0, 2, 1), new(0, 3, 2), .. faces.Select(face => new Triangle(face[0], face[1], face[2]))];
        return new(spatial,
            new[] { new StaticMeshAsset(1, 0, 4, 0, 2), new StaticMeshAsset(2, 4, 8, 2, 12) },
            vertices, triangles,
            new[] { new StaticMeshInstance(1, 1, Identity), new StaticMeshInstance(2, 2, Identity) });
    }

    private static readonly Transform Identity = new(Vector3.Zero, Quaternion.Identity, Vector3.One);

    /// <summary>A body that stays where it is put: every axis locked, no gravity.</summary>
    private static DynamicsBodyProperties Held() =>
        new(1, default, Vector3.Zero, Vector3.Zero, new AxisLocks(true, true, true, true, true, true), 0, 0, 0, .5f, 0, uint.MaxValue, uint.MaxValue, true, false, false);

    private static DynamicsBodyProperties Falling(float mass, Vector3 velocity) =>
        new(mass, default, velocity, Vector3.Zero, default, 0, 0, 1, .5f, 0, uint.MaxValue, uint.MaxValue, true, false, true);

    public void Start() { }

    public ProductUpdateResult Update(ProductUpdate update)
    {
        ProductUpdateFacts facts = update.Facts;
        for (uint step = 0; step < facts.AdmittedStepCount; step++)
        {
            float dt = (float)facts.FixedDeltaSeconds;
            seconds += dt;
            engine.Dynamics.SetSoftBodyForces(DynamicsSoftBodyForcesRequest.Blowing(flag, Wind()));
            long started = Stopwatch.GetTimestamp();
            engine.Dynamics.Step(new(world, dt, 1, ReadOnlyMemory<DynamicsAction>.Empty));
            stepMicroseconds += Stopwatch.GetElapsedTime(started).TotalMicroseconds;
            steps++;
            Track();
        }
        Publish();
        return ProductUpdateResult.None;
    }

    /// <summary>The wind now: its speed rising and falling in a slow gust and a quick flutter.</summary>
    private Vector3 Wind()
    {
        float gust = 1 + GustShare * MathF.Sin(MathF.Tau * (float)seconds / GustPeriod) + FlutterShare * MathF.Sin(MathF.Tau * (float)seconds / FlutterPeriod);
        return new Vector3(0, 0, windSpeed * gust);
    }

    private void Track()
    {
        DynamicsSoftBodyReadout flagNow = engine.Dynamics.ReadSoftBody(flag);
        flagSwing.Enqueue(flagNow.CenterOfMass.Z);
        if (flagSwing.Count > SwingWindow) flagSwing.Dequeue();
        flagContactsMost = Math.Max(flagContactsMost, flagNow.ContactCount);
        foreach (DynamicsSoftBodyParticle particle in engine.Dynamics.ReadSoftBodyParticles(flag).Span)
            closestToObstacle = MathF.Min(closestToObstacle, Vector3.Distance(particle.Position, Obstacle) - ObstacleRadius);
        jellyLowest = MathF.Min(jellyLowest, Height(engine.Dynamics.ReadSoftBody(jelly)));
        blobHighest = MathF.Max(blobHighest, engine.Dynamics.ReadSoftBody(blob).CenterOfMass.Y);
    }

    private static float Height(DynamicsSoftBodyReadout readout) => readout.BoundsMax.Y - readout.BoundsMin.Y;

    private void Publish()
    {
        var facts = new List<AppearanceFact>
        {
            Fact(FloorId, floorLook, new(new Vector3(0, -FloorThickness / 2, 0), Quaternion.Identity, new(2 * FloorHalfSize, FloorThickness, 2 * FloorHalfSize))),
            Fact(PoleId, poleLook, new(PoleFoot with { Y = PoleHeight / 2 }, Quaternion.Identity, new(2 * PoleHalfWidth, PoleHeight, 2 * PoleHalfWidth))),
            Fact(ObstacleId, obstacleLook, new(Obstacle, Quaternion.Identity, new(2 * ObstacleRadius))),
            // Soft bodies are drawn in the world: their objects stay at the origin.
            Fact(FlagId, flagLook, Identity),
            Fact(JellyId, jellyLook, Identity),
            Fact(BlobId, blobLook, Identity),
        };
        if (ball is not null) facts.Add(Fact(BallId, ballLook, engine.Dynamics.Read(new(ball)).Transform with { Scale = new(2 * BallRadius) }));
        engine.Graphics.PublishSnapshot(facts.ToArray());
    }

    private static AppearanceFact Fact(ulong id, Appearance look, Transform transform) => new(id, false, 0, transform, look, true, RenderLayer.Scene);

    [DebugCommand("soft.wind", Description = "Set the wind's mean speed in m/s (0 for calm).")]
    public string SetWind(float speed) { windSpeed = speed; return Inspect(); }

    [DebugCommand("soft.drop", Description = "Drop a 30 kg ball onto the jelly.")]
    public string Drop()
    {
        ball?.Dispose();
        jellyLowest = float.MaxValue;
        Vector3 above = JellyCenter with { Y = DropHeight };
        ball = engine.Dynamics.CreateSphereBodyWithProperties(new(world, new(new(above, Quaternion.Identity, Vector3.One), BallRadius, Falling(BallMass, new(0, -DropSpeed, 0)))));
        return Inspect();
    }

    [DebugCommand("soft.clear", Description = "Take the dropped ball away.")]
    public string Clear()
    {
        ball?.Dispose();
        ball = null;
        return Inspect();
    }

    [DebugCommand("soft.squash", Description = "Press the jelly's top down, as a stamp would.")]
    public string Squash()
    {
        jellyLowest = float.MaxValue;
        Vector3 top = JellyCenter with { Y = JellyCenter.Y + JellyHalf };
        engine.Dynamics.ApplySoftBodyVelocityChange(new(jelly, top, new(0, -SquashSpeed, 0), SquashReach));
        return Inspect();
    }

    [DebugCommand("soft.jump", Description = "Kick the blob upward.")]
    public string Jump()
    {
        blobHighest = 0;
        engine.Dynamics.ApplySoftBodyVelocityChange(new(blob, Vector3.Zero, new(0, JumpSpeed, 0), 0));
        return Inspect();
    }

    [DebugCommand("soft.inspect", Description = "Each soft body's facts, the wind and the mean Dynamics step time.")]
    public string Inspect() => JsonSerializer.Serialize(Observe(), SoftJsonContext.Default.SoftFacts);

    /// <summary>Asserts one stage of the README's headless sequence.</summary>
    [DebugCommand("soft.expect", Description = "Assert a stage: flutter, touched, squashed, recovered or jumped.")]
    public string Expect(string stage)
    {
        SoftFacts facts = Observe();
        switch (stage)
        {
            case "flutter":
                Require(facts.Flag.CenterOfMass[2] > FlutterOffset, "the wind holds the flag downwind");
                Require(facts.FlagSwing > FlutterSwing, "the gusts keep it moving");
                Require(facts.Flag.MaxStretch < .2f, "it stretches little");
                break;
            case "touched":
                Require(facts.ClosestToObstacle < TouchSlack && facts.FlagContactsMost >= 1, "the flag meets the held ball");
                Require(facts.ClosestToObstacle > -TouchSlack, "without passing into it");
                break;
            case "squashed":
                Require(facts.JellyLowest < SquashShare * facts.JellyRestHeight, "the jelly squashed");
                break;
            case "recovered":
                Require(ball is null, "nothing presses it");
                Require(MathF.Abs(facts.JellyHeight - facts.JellyRestHeight) < RecoverShare * facts.JellyRestHeight, "the jelly is its own height again");
                Require(facts.Jelly.Volume > VolumeShare * facts.Jelly.RestVolume, "and its own volume");
                break;
            case "jumped":
                Require(facts.BlobHighest > blobRestHeight + JumpRise, "the blob jumped");
                Require(facts.Blob.Volume > VolumeShare * facts.Blob.RestVolume, "and kept its volume");
                break;
            default:
                throw new ArgumentException("flutter, touched, squashed, recovered or jumped", nameof(stage));
        }
        return Inspect();
    }

    /// <summary>
    /// The cost of a sheet of <paramref name="columns"/> × <paramref name="columns"/> particles pinned
    /// along one edge in steady wind: microseconds per Dynamics step around the same generated Step
    /// call, in a fresh world, against the same world empty.
    /// </summary>
    [DebugCommand("soft.cost", Description = "Microseconds per step of a columns × columns flag in wind.")]
    public string Cost(int columns)
    {
        double empty = TimeSheet(0), sheet = TimeSheet((uint)columns);
        return JsonSerializer.Serialize(new CostFacts(columns, columns * columns, CostSteps, empty, sheet, (sheet - empty) / (columns * columns)), SoftJsonContext.Default.CostFacts);
    }

    private double TimeSheet(uint columns)
    {
        using DynamicsWorld trial = engine.Dynamics.CreateWorld(new DynamicsWorldConfig(new Vector3(0, -Gravity, 0)));
        DynamicsSoftBody? sheet = null;
        if (columns > 0)
        {
            DynamicsSoftBodyRequest shape = DynamicsSoftBodyRequest.Cloth(trial, new(0, 10, 0), new(FlagCell, 0, 0), new(0, -FlagCell, 0), columns, columns, FlagMass);
            uint[] hoist = [.. Enumerable.Range(0, (int)columns).Select(row => shape.ClothParticle(0, (uint)row))];
            sheet = engine.Dynamics.CreateSoftBody(shape with { Pinned = hoist, ParticleRadius = FlagThickness });
            engine.Dynamics.SetSoftBodyForces(DynamicsSoftBodyForcesRequest.Blowing(sheet, new(0, 0, WindSpeed)));
        }
        long started = Stopwatch.GetTimestamp();
        for (int step = 0; step < CostSteps; step++) engine.Dynamics.Step(new(trial, 1f / 60, 1, ReadOnlyMemory<DynamicsAction>.Empty));
        double perStep = Stopwatch.GetElapsedTime(started).TotalMicroseconds / CostSteps;
        sheet?.Dispose();
        return perStep;
    }

    private SoftFacts Observe()
    {
        DynamicsSoftBodyReadout flagNow = engine.Dynamics.ReadSoftBody(flag), jellyNow = engine.Dynamics.ReadSoftBody(jelly), blobNow = engine.Dynamics.ReadSoftBody(blob);
        float swing = flagSwing.Count == 0 ? 0 : flagSwing.Max() - flagSwing.Min();
        return new(
            Body.Of(flagNow), swing, closestToObstacle, flagContactsMost,
            Body.Of(jellyNow), Height(jellyNow), jellyRestHeight, jellyLowest,
            Body.Of(blobNow), blobRestHeight, blobHighest,
            windSpeed, Wind().Z, ball is not null, steps == 0 ? 0 : stepMicroseconds / steps);
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
        ball?.Dispose();
        foreach (IDisposable owner in new IDisposable[] { flagLook, jellyLook, blobLook, floorLook, poleLook, obstacleLook, ballLook, flagMaterial, jellyMaterial, blobMaterial, flagTexture, sun, sky, camera, flag, jelly, blob, obstacle, world, spatial })
            owner.Dispose();
    }
}

internal sealed record BodyFacts(uint Particles, uint Pieces, float[] CenterOfMass, float[] BoundsMin, float[] BoundsMax, float Volume, float RestVolume, float MaxStretch, uint ContactCount, bool Sleeping);

internal sealed record SoftFacts(
    BodyFacts Flag, float FlagSwing, float ClosestToObstacle, uint FlagContactsMost,
    BodyFacts Jelly, float JellyHeight, float JellyRestHeight, float JellyLowest,
    BodyFacts Blob, float BlobRestHeight, float BlobHighest,
    float WindMeanSpeed, float WindNow, bool BallDropped, double StepMicroseconds);

internal sealed record CostFacts(int Columns, int Particles, int Steps, double EmptyStepMicroseconds, double SheetStepMicroseconds, double MicrosecondsPerParticle);

internal static class Body
{
    public static BodyFacts Of(DynamicsSoftBodyReadout r) => new(
        r.ParticleCount, r.PieceCount, [r.CenterOfMass.X, r.CenterOfMass.Y, r.CenterOfMass.Z],
        [r.BoundsMin.X, r.BoundsMin.Y, r.BoundsMin.Z], [r.BoundsMax.X, r.BoundsMax.Y, r.BoundsMax.Z],
        r.Volume, r.RestVolume, r.MaxStretch, r.ContactCount, r.Sleeping);
}

[JsonSourceGenerationOptions(PropertyNamingPolicy = JsonKnownNamingPolicy.CamelCase)]
[JsonSerializable(typeof(SoftFacts))]
[JsonSerializable(typeof(CostFacts))]
internal partial class SoftJsonContext : JsonSerializerContext { }
