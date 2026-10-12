using System.Numerics;
using System.Text.Json;
using System.Text.Json.Serialization;
using Rusty.Engine;
using Rusty.Engine.Debugging;
using Rusty.Engine.Input;

namespace CsharpWaterField;

/// <summary>
/// A river valley whose water is an Engine water field: a storm overbanks onto the floodplain and
/// fills a hollow, the flood drains as the river falls, and a dam backs the river up. The swimmer
/// swims wherever the field is wet and is carried by its current.
/// </summary>
public sealed class Product : IEngineProduct, IDebugCommandModuleSource, IDebugCommandModule
{
    private const ulong GroundId = 1, SwimmerId = 2, SunId = 3, SkyId = 4;
    private const float StepSeconds = 1f / 60f;
    private const float SwimSpeed = 2.5f, SwimAcceleration = 8, WaterDrag = 2, WaterGravity = 1, WaterBuoyancy = 1.1f;
    private const float SwimmerHeight = 1.8f;
    private const float CameraFov = 60, CameraNear = .1f, CameraFar = 400;
    /// <summary>Forward is +x, down the valley.</summary>
    private const float HeadingYaw = MathF.PI / 2;
    private const float WetCellDepth = .05f, HollowPondDepth = .3f, DamBackupRise = .3f;
    private static readonly Vector3 SwimmerSize = new(.6f, SwimmerHeight, .6f);
    private static readonly Vector3 FollowOffset = new(-14, 9, 4), FollowLook = new(6, 0, 0);
    private static readonly Vector3 OverviewEye = new(-20, 55, 40), OverviewTarget = new(64, 0, 40);
    private static readonly Vector3 SunTravel = Vector3.Normalize(new(.4f, -1, .3f)), SunColor = new(1, .95f, .85f);
    private static readonly Vector3 SkyColor = new(.55f, .7f, 1), GroundColor = new(.35f, .3f, .2f);
    private static readonly Color GrassColor = new(.25f, .42f, .16f, 1), SwimmerColor = new(1, .55f, .1f, 1), WaterTint = new(1, 1, 1, .35f);
    private static readonly Vector3 SwimStart = Valley.Over(12, Valley.ChannelFirst + 1.5f);

    private readonly IEngineContext engine;
    private readonly Valley valley = new();
    private readonly ValleyWater water;
    private readonly SpatialSession scene;
    private readonly CharacterControllerConfig config;
    private readonly Camera camera;
    private readonly Material grass, waterMaterial;
    private readonly RenderResource foam, ripples;
    private readonly Appearance swimmer;
    private readonly Light sun, sky;
    private readonly PhysicalInputState input = new();
    private MeshResource groundMesh;
    private Appearance ground;
    private Vector3 position = SwimStart;
    private CharacterMotion motion;
    private CharacterStepReceipt lastStep;
    private WaterFieldSample lastSample;
    private ulong sequence;
    private bool follow = true;
    private float? surfaceBeforeDam;

    public Product(ProductCreateContext context)
    {
        engine = context.Engine;
        water = new ValleyWater(engine.WaterField, valley);
        scene = engine.Spatial.CreateSession(new(1, 8, VoxelSurfaceMode.GreedyCubes));
        config = engine.Spatial.DefaultCharacterControllerConfig();
        Color white = new(1, 1, 1, 1);
        grass = engine.Graphics.CreateMaterial(new(GrassColor, default(RenderResourceReference), .95f, white, Vector3.Zero, 0, false));
        ripples = engine.Graphics.OpenResource(new("ripples.png", TextureFilter.Linear, TextureWrap.Repeat, TextureColorSpace.Linear)).Handle;
        foam = engine.Graphics.OpenResource(new("foam.png", TextureFilter.Linear, TextureWrap.Repeat, TextureColorSpace.Linear)).Handle;
        waterMaterial = engine.Graphics.CreateMaterial(new MaterialRequest(WaterTint, default(RenderResourceReference), .08f, white, Vector3.Zero, 0, true, MaterialAlphaMode.Blend, 0) with
        {
            NormalMap = ripples,
            NormalScale = .6f,
            Water = new(new(.2f, .45f, .35f, 1), new(.03f, .1f, .08f, 1), 1.5f, .3f, .55f, new(.03f, .02f), new(.05f, .03f), new(-.02f, .04f), 3, foam, ripples),
        });
        water.Present(waterMaterial);
        swimmer = engine.Graphics.CreatePrimitive(new(PrimitiveGeometry.Cube, false, SwimmerColor));
        groundMesh = engine.Graphics.CreateMeshResource(valley.Mesh(grass));
        ground = engine.Graphics.CreateMeshAppearance(groundMesh);
        sun = engine.Graphics.CreateLight(new(SunId, false, 0, new(LightKind.Directional, SunColor, 3, true, Vector3.Zero, SunTravel, false, 0, 0, 0, 0, LightShadowIntent.Requested)));
        sky = engine.Graphics.CreateLight(new(SkyId, false, 0, LightDescriptor.Hemisphere(SkyColor, GroundColor, .6f)));
        camera = engine.CameraView.CreateCamera(View(OverviewEye, OverviewTarget));
        engine.CameraView.SetActiveCamera(camera);
        Swim(SwimStart.X, SwimStart.Z);
    }

    public void Start() { }

    public ProductUpdateResult Update(ProductUpdate update)
    {
        input.Consume(update.Input);
        for (uint step = 0; step < update.Facts.AdmittedStepCount; step++) Advance();
        Publish();
        return ProductUpdateResult.None;
    }

    /// <summary>One step of the swimmer: Engine swimming in the field's water, carried by its current.</summary>
    private void Advance()
    {
        bool wet = water.TrySwimBox(position, out Vector3 minimum, out Vector3 maximum, out lastSample);
        CharacterMovementRequest movement = wet
            ? new(CharacterMovementMode.Swimming, Axis(KeyboardControl.Space, KeyboardControl.ShiftLeft), SwimSpeed, SwimAcceleration, WaterDrag, minimum, maximum, WaterGravity, WaterBuoyancy, 0)
            : default;
        Vector2 planar = new(Axis(KeyboardControl.KeyD, KeyboardControl.KeyA), Axis(KeyboardControl.KeyW, KeyboardControl.KeyS));
        Vector3 current = wet ? new(lastSample.VelocityX, 0, lastSample.VelocityZ) : Vector3.Zero;
        var command = new CharacterControllerCommand(movement, planar, HeadingYaw, false, false, false, current, Vector3.Zero, StepSeconds, ++sequence);
        lastStep = engine.Spatial.ProposeCharacterStep(new(scene, position, motion, default, ReadOnlyMemory<CharacterObstacle>.Empty, ReadOnlyMemory<CharacterMeshInstance>.Empty, config, command));
        position = lastStep.Transform.Translation;
        motion = lastStep.Motion;
        // The fixture has no terrain collision: below the bed the field reports, the swimmer
        // stands on it and walks from there.
        float floor = water.Sample(position).Bed + SwimmerHeight / 2;
        if (position.Y < floor)
        {
            position.Y = floor;
            motion = motion with { Grounded = true, ControlledVelocity = motion.ControlledVelocity with { Y = 0 }, ExternalVelocity = Vector3.Zero };
        }
    }

    private float Axis(KeyboardControl positive, KeyboardControl negative) => (input.Held(positive) ? 1 : 0) - (input.Held(negative) ? 1 : 0);

    private void Publish()
    {
        if (follow) engine.CameraView.UpdateCamera(new(camera, View(position + FollowOffset, position + FollowLook)));
        engine.Graphics.PublishSnapshot(new AppearanceFact[]
        {
            new(GroundId, false, 0, new(Vector3.Zero, Quaternion.Identity, Vector3.One), ground, true, RenderLayer.Scene),
            new(SwimmerId, false, 0, new(position, Quaternion.Identity, SwimmerSize), swimmer, true, RenderLayer.Scene),
        });
    }

    private static CameraDescriptor View(Vector3 eye, Vector3 target)
    {
        CameraQueries.TryLookAtPose(eye, target, 0, out CameraPose pose);
        return new(pose, CameraBasisMode.Derived, default, new(CameraProjectionKind.Perspective, CameraFov, 0, CameraNear, CameraFar), CameraViewports.Full);
    }

    [DebugCommand("water.storm", Description = "Raise the river to a storm's inflow.")]
    public string Storm() => Flow(ValleyWater.StormFlow);

    [DebugCommand("water.flow", Description = "Set the river's inflow, m³/s.")]
    public string Flow(float discharge) { water.SetDischarge(discharge); return Inspect(); }

    [DebugCommand("water.calm", Description = "Return the river to its base flow.")]
    public string Calm() => Flow(ValleyWater.BaseFlow);

    [DebugCommand("water.dam", Description = "Build a wall across the valley; the river backs up behind it.")]
    public string Dam()
    {
        surfaceBeforeDam = BehindDam().Surface;
        return Reshape(built: true);
    }

    [DebugCommand("water.breach", Description = "Remove the dam.")]
    public string Breach() => Reshape(built: false);

    private string Reshape(bool built)
    {
        water.EditBed(valley.Dam(built));
        (MeshResource oldMesh, Appearance oldGround) = (groundMesh, ground);
        groundMesh = engine.Graphics.CreateMeshResource(valley.Mesh(grass));
        ground = engine.Graphics.CreateMeshAppearance(groundMesh);
        Publish();
        oldGround.Dispose();
        oldMesh.Dispose();
        return Inspect();
    }

    [DebugCommand("water.sample", Description = "The water over cell (x, z).")]
    public string SampleCell(float x, float z) => JsonSerializer.Serialize(Facts.Of(water.Sample(Valley.Over(x, z))), FieldJsonContext.Default.SampleFacts);

    [DebugCommand("water.swim", Description = "Put the swimmer at scene point (x, z).")]
    public string Swim(float x, float z)
    {
        position = new(x, water.Sample(new(x, 0, z)).Surface, z);
        motion = new(false, 0, 0, false, 0, Vector3.Zero, Vector3.Zero, Vector3.Zero, Vector3.Zero, false, CharacterStance.Standing, 0, 0, 0, false, 0, Vector3.Zero, Vector3.Zero, Quaternion.Identity, Vector3.Zero, position.Y, position.Y, 0, 0);
        return Inspect();
    }

    [DebugCommand("water.camera", Description = "follow: behind the swimmer; overview: the whole valley.")]
    public string CameraMode(string mode)
    {
        follow = mode == "follow";
        if (!follow) engine.CameraView.UpdateCamera(new(camera, View(OverviewEye, OverviewTarget)));
        return Inspect();
    }

    [DebugCommand("water.inspect", Description = "The field's diagnostics, the flood, the hollow and the swimmer.")]
    public string Inspect() => JsonSerializer.Serialize(Observe(), FieldJsonContext.Default.ValleyFacts);

    /// <summary>
    /// Asserts one stage of the headless sequence (README): channel, overbank, drained, dammed.
    /// </summary>
    [DebugCommand("water.expect", Description = "Assert a stage: channel, overbank, drained or dammed.")]
    public string Expect(string stage)
    {
        ValleyFacts facts = Observe();
        WaterFieldSample river = water.Sample(Valley.Over(20, Valley.ChannelFirst + 1.5f));
        switch (stage)
        {
            case "channel":
                Require(facts.FloodplainWetCells == 0, "the base flow stays in the channel");
                Require(river.Wet && river.VelocityX > 0, "the river runs downstream");
                break;
            case "overbank":
                Require(facts.FloodplainWetCells > Valley.Width * 2, "the storm overbanks onto the floodplain");
                Require(facts.HollowDepth > HollowPondDepth, "water pools in the hollow");
                break;
            case "drained":
                Require(facts.FloodplainWetCells == facts.HollowWetCells, "the floodplain drains back to the river; only the hollow holds water");
                break;
            case "dammed":
                Require(surfaceBeforeDam is float before && BehindDam().Surface > before + DamBackupRise, "the dam backs the river up");
                Require(facts.Field.Incarnation == facts.Field.SnapshotIncarnation, "the snapshot carries the dam");
                break;
            default:
                throw new ArgumentException("channel, overbank, drained or dammed", nameof(stage));
        }
        return Inspect();
    }

    private WaterFieldSample BehindDam() => water.Sample(Valley.Over(Valley.DamColumn - 1, Valley.ChannelFirst + 1.5f));

    private ValleyFacts Observe()
    {
        ReadOnlySpan<WaterFieldCell> cells = water.Cells().Span;
        int floodplain = 0, hollow = 0;
        float hollowDepth = 0;
        for (uint z = 0; z < Valley.Length; z++)
        for (uint x = 0; x < Valley.Width; x++)
        {
            float depth = cells[(int)Valley.Index(x, z)].Depth;
            if (Valley.InHollow(x, z)) hollowDepth = Math.Max(hollowDepth, depth);
            if (Valley.InChannel(z) || depth <= WetCellDepth) continue;
            floodplain++;
            if (Valley.InHollow(x, z)) hollow++;
        }
        WaterFieldReadout field = water.Read();
        return new(water.Discharge, floodplain, hollow, hollowDepth, Facts.Of(field), new[] { position.X, position.Y, position.Z }, lastStep.Movement.Mode.ToString(), lastStep.Movement.Immersion);
    }

    public void RegisterDebugCommands(IDebugCommandModuleRegistrar registrar) => registrar.Register(this);
    private static void Require(bool condition, string message) { if (!condition) throw new InvalidOperationException(message); }
    public void Pause() { }
    public void Resume() { }
    public void Restart() => Swim(SwimStart.X, SwimStart.Z);
    public void Shutdown() { }

    public void Dispose()
    {
        engine.Graphics.PublishSnapshot([]);
        water.Dispose();
        ground.Dispose();
        groundMesh.Dispose();
        swimmer.Dispose();
        waterMaterial.Dispose();
        grass.Dispose();
        foam.Dispose();
        ripples.Dispose();
        sun.Dispose();
        sky.Dispose();
        camera.Dispose();
        scene.Dispose();
    }
}

internal sealed record FieldFacts(ulong Incarnation, ulong SnapshotIncarnation, ulong Snapshots, double Time, double AdmittedTime, double Age, double DroppedSeconds, double Volume, double BoundaryInflow, double BoundaryOutflow, double SinkVolume, uint WetCells, uint Steps, ulong StepMicroseconds, float Load, uint DrawnCells);

internal sealed record SampleFacts(bool Inside, bool Wet, float Surface, float Bed, float Depth, float VelocityX, float VelocityZ, double Time, double Age);

internal sealed record ValleyFacts(float Discharge, int FloodplainWetCells, int HollowWetCells, float HollowDepth, FieldFacts Field, float[] Swimmer, string Mode, float Immersion);

internal static class Facts
{
    public static FieldFacts Of(WaterFieldReadout r) => new(r.Incarnation, r.SnapshotIncarnation, r.Snapshots, r.Time, r.AdmittedTime, r.Age, r.DroppedSeconds, r.Volume, r.BoundaryInflow, r.BoundaryOutflow, r.SinkVolume, r.WetCells, r.Steps, r.StepMicroseconds, r.Load, r.DrawnCells);

    public static SampleFacts Of(WaterFieldSample s) => new(s.Inside, s.Wet, s.Surface, s.Bed, s.Depth, s.VelocityX, s.VelocityZ, s.Time, s.Age);
}

[JsonSourceGenerationOptions(PropertyNamingPolicy = JsonKnownNamingPolicy.CamelCase)]
[JsonSerializable(typeof(ValleyFacts))]
[JsonSerializable(typeof(SampleFacts))]
internal partial class FieldJsonContext : JsonSerializerContext { }
