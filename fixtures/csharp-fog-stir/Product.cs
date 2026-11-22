using System.Numerics;
using System.Text.Json;
using Rusty.Engine;
using Rusty.Engine.Debugging;

namespace CsharpFogStir;

/// <summary>
/// A mist sump stirred by what moves through it (fog regions and stirrers).
/// A dark body glides to and fro along a 30 x 6 x 4 m passage of dense mist
/// with a stirrer on it, so the Engine parts the mist ahead of it, clears a
/// wake behind it and lets the wake settle back. A lantern sweeps along the
/// front of the passage, lighting the mist it passes. Optional drifters add
/// slower stirrers for cost runs, and the same mist can be drawn as one plain
/// fog volume, or not at all, to compare.
/// </summary>
public sealed class Product : IEngineProduct, IDebugCommandModuleSource, IDebugCommandModule
{
    // The sump: a passage 30 m long, 6 m high and 4 m deep, mist filling it.
    private static readonly Vector3 SumpCentre = new(0, 3, 0);
    private static readonly Vector3 SumpHalfExtents = new(15, 3, 2);
    private const float CellSize = 0.25f;
    private const float MistDensity = 0.45f;
    private static readonly Vector3 MistAlbedo = new(0.62f, 0.64f, 0.7f);
    private static readonly Vector3 MistGlow = new(0.012f, 0.012f, 0.016f);
    private const float MistEdge = 0.08f;
    private const float NoiseCell = 1.1f;
    private const float NoiseStrength = 0.8f;
    private static readonly Vector3 Current = new(0.35f, 0, 0);
    private const float SettleSeconds = 3f;
    private const float FogReach = 40f;
    private const uint SumpId = 1;
    // The body: a sphere gliding along the passage and back, bobbing, on the
    // near side of the sump so its wake shows in the mist the camera sees most of.
    private const float BodyRadius = 0.9f;
    private const float BodyDepth = 0.9f;
    private const float BodySpeed = 3f;
    private const float BodyTravel = 12f;
    private const float BodyHeight = 2.4f;
    private const float BodyBob = 0.5f;
    private const float BodyBobSeconds = 2.3f;
    private const float BodyPush = 0.8f;
    private const float BodyClear = 1f;
    private const uint BodyStirrerId = 1;
    // Drifters: slower stirrers for cost runs, off by default.
    private const int MaxDrifters = 3;
    private const float DrifterRadius = 0.5f;
    private const float DrifterSpeed = 1.2f;
    private const float DrifterPush = 0.6f;
    private const float DrifterClear = 0.4f;
    // The lantern: a warm point light sweeping along the front of the passage.
    private static readonly Vector3 LanternColor = new(1f, 0.78f, 0.5f);
    private const float LanternIntensity = 30f;
    private const float LanternRange = 14f;
    private const float LanternTravel = 11f;
    private const float LanternSeconds = 9f;
    private const float LanternHeight = 2.8f;
    private const float LanternDepth = 2.8f;
    private const float FillIntensity = 0.12f;
    private static readonly Vector3 FillColor = new(0.5f, 0.55f, 0.7f);
    // Object ids.
    private const ulong FloorId = 1, WallId = 2, BodyId = 3, LanternId = 4, FillId = 5;
    private const ulong FirstDrifterId = 10;

    private readonly IEngineContext engine;
    private readonly Appearance rockLook, bodyLook, drifterLook;
    private readonly Light lantern;
    private double time;
    private int drifters;
    private string mode = "region";
    private bool paused;
    private Vector3 body;
    private Vector3 bodyVelocity;
    private long stirrerPublishes;

    public Product(ProductCreateContext context)
    {
        engine = context.Engine;
        // Primitives draw their colour unlit: near black, so the mist reads against the rock.
        rockLook = engine.Graphics.CreatePrimitive(new(PrimitiveGeometry.Cube, false, new Color(.012f, .011f, .01f, 1)));
        bodyLook = engine.Graphics.CreatePrimitive(new(PrimitiveGeometry.Sphere, false, new Color(.03f, .02f, .05f, 1)));
        drifterLook = engine.Graphics.CreatePrimitive(new(PrimitiveGeometry.Sphere, false, new Color(.12f, .1f, .14f, 1)));
        engine.Graphics.PublishSnapshot(
        [
            Fact(FloorId, rockLook, new Vector3(0, -0.25f, 0), new Vector3(40, 0.5f, 12)),
            Fact(WallId, rockLook, new Vector3(0, 4, -2.6f), new Vector3(40, 8, 1)),
            Fact(BodyId, bodyLook, BodyAt(0), new Vector3(2 * BodyRadius)),
        ]);
        lantern = engine.Graphics.CreateLight(new(LanternId, false, 0, Lantern(0)));
        engine.Graphics.CreateLight(new(FillId, false, 0, new LightDescriptor(
            LightKind.Ambient, FillColor, FillIntensity, true, Vector3.Zero, Vector3.UnitY, false, 0, 0, 0, 0, LightShadowIntent.Disabled)));
        Camera camera = engine.CameraView.CreateCamera(new CameraDescriptor(
            new CameraPose(new Vector3(0, 2.6f, 11), -3, 0),
            CameraBasisMode.Derived,
            default,
            new CameraProjection(CameraProjectionKind.Perspective, 60, 0, .05, 120),
            CameraViewports.Full,
            0));
        engine.CameraView.SetActiveCamera(camera);
        engine.CameraView.SetVolumetricFog(new(0, MistAlbedo, 0.3f, 0, 0, FogReach, 1));
        PlaceMist();
        body = BodyAt(0);
    }

    public void Start() { }

    public ProductUpdateResult Update(ProductUpdate update)
    {
        if (paused) return ProductUpdateResult.None;
        double step = update.Facts.FixedDeltaSeconds * update.Facts.AdmittedStepCount;
        if (step <= 0) return ProductUpdateResult.None;
        time += step;
        Vector3 next = BodyAt(time);
        bodyVelocity = (next - body) / (float)step;
        body = next;
        List<AppearanceFact> moved = [Fact(BodyId, bodyLook, body, new Vector3(2 * BodyRadius))];
        if (mode == "region")
        {
            engine.CameraView.SetFogStirrer(new(BodyStirrerId, body, bodyVelocity, BodyRadius, BodyPush, BodyClear));
            stirrerPublishes++;
        }
        for (int i = 0; i < drifters; i++)
        {
            (Vector3 at, Vector3 velocity) = DrifterAt(i, time);
            moved.Add(Fact(FirstDrifterId + (ulong)i, drifterLook, at, new Vector3(2 * DrifterRadius)));
            if (mode == "region")
            {
                engine.CameraView.SetFogStirrer(new(BodyStirrerId + 1 + (uint)i, at, velocity, DrifterRadius, DrifterPush, DrifterClear));
                stirrerPublishes++;
            }
        }
        engine.Graphics.PublishChanges(new(moved.ToArray(), ReadOnlyMemory<ulong>.Empty, ReadOnlyMemory<MeshJointAttachment>.Empty));
        engine.Graphics.UpdateLight(new(lantern, new(LanternId, false, 0, Lantern(time))));
        return ProductUpdateResult.None;
    }

    public void Pause() { }
    public void Resume() { }
    public void Restart() { }
    public void Shutdown() { }
    public void Dispose() { }

    /// <summary>Where the body is at <paramref name="seconds"/>: gliding along the passage and back, bobbing.</summary>
    private static Vector3 BodyAt(double seconds)
    {
        double period = 4 * BodyTravel / BodySpeed;
        double phase = seconds % period / period;
        double along = phase < 0.5 ? -BodyTravel + 4 * BodyTravel * phase : 3 * BodyTravel - 4 * BodyTravel * phase;
        float bob = BodyBob * MathF.Sin((float)(2 * Math.PI * seconds / BodyBobSeconds));
        return new Vector3((float)along, BodyHeight + bob, BodyDepth + 0.2f * bob);
    }

    /// <summary>Drifter <paramref name="index"/> circling slowly in its own part of the passage, and its velocity.</summary>
    private static (Vector3 At, Vector3 Velocity) DrifterAt(int index, double seconds)
    {
        const float Radius = 1.6f;
        float centre = -8 + 8 * index;
        float angle = (float)(seconds * DrifterSpeed / Radius) + index;
        Vector3 at = new(centre + Radius * MathF.Cos(angle), 1.4f + 0.6f * index, Radius * 0.6f * MathF.Sin(angle));
        Vector3 velocity = DrifterSpeed * new Vector3(-MathF.Sin(angle), 0, 0.6f * MathF.Cos(angle));
        return (at, velocity);
    }

    private static LightDescriptor Lantern(double seconds)
    {
        float x = LanternTravel * MathF.Sin((float)(2 * Math.PI * seconds / LanternSeconds));
        return new(LightKind.Point, LanternColor, LanternIntensity, true, new Vector3(x, LanternHeight, LanternDepth),
            Vector3.UnitY, true, LanternRange, 2, 0, 0, LightShadowIntent.Disabled);
    }

    private static AppearanceFact Fact(ulong id, Appearance appearance, Vector3 position, Vector3 scale) =>
        new(id, false, 0, new Transform(position, Quaternion.Identity, scale), appearance, true, RenderLayer.Scene);

    /// <summary>Draws the mist as the current mode asks: a stirred region, one plain box volume, or none.</summary>
    private void PlaceMist()
    {
        engine.CameraView.RemoveFogRegion(new(SumpId));
        engine.CameraView.RemoveFogVolume(new(SumpId));
        for (uint id = BodyStirrerId; id <= BodyStirrerId + MaxDrifters; id++)
        {
            engine.CameraView.RemoveFogStirrer(new(id));
        }
        if (mode == "region")
        {
            engine.CameraView.SetFogRegion(new(SumpId, SumpCentre, SumpHalfExtents, 0, CellSize, MistDensity, MistAlbedo, MistGlow,
                MistEdge, NoiseCell, NoiseStrength, Current, SettleSeconds));
        }
        else if (mode == "volume")
        {
            engine.CameraView.SetFogVolume(new(SumpId, FogVolumeShape.Box, SumpCentre, SumpHalfExtents, 0, MistDensity, MistAlbedo, MistGlow,
                MistEdge, NoiseCell, NoiseStrength, Current));
        }
    }

    public void RegisterDebugCommands(IDebugCommandModuleRegistrar registrar) => registrar.Register(this);

    [DebugCommand("fogstir.observe", Description = "Read-only: the mist's mode, the body's place and velocity, the drifters, stirrer publishes and the volumetric fog quality.")]
    public string Observe() => JsonSerializer.Serialize(new
    {
        mode,
        paused,
        time,
        body = new[] { body.X, body.Y, body.Z },
        bodyVelocity = new[] { bodyVelocity.X, bodyVelocity.Y, bodyVelocity.Z },
        drifters,
        stirrerPublishes,
        volumetricFog = engine.RendererSettings.Read().Effective.VolumetricFog.ToString(),
        volumetricFogRefusal = engine.RendererSettings.Read().VolumetricFogRefusal.ToString(),
    });

    [DebugCommand("fogstir.mode", Description = "Draws the mist as a stirred fog region (region), as one plain box fog volume of the same values (volume), or not at all (none).")]
    public string Mode(string next)
    {
        if (next is not ("region" or "volume" or "none")) return "mode is region, volume or none";
        mode = next;
        PlaceMist();
        return Observe();
    }

    [DebugCommand("fogstir.drifters", Description = "Sets how many slow drifters (0 to 3) circle in the sump beside the body, each with its own stirrer.")]
    public string Drifters(long count)
    {
        int next = (int)Math.Clamp(count, 0, MaxDrifters);
        if (next < drifters)
        {
            ulong[] gone = Enumerable.Range(next, drifters - next).Select(i => FirstDrifterId + (ulong)i).ToArray();
            engine.Graphics.PublishChanges(new(ReadOnlyMemory<AppearanceFact>.Empty, gone, ReadOnlyMemory<MeshJointAttachment>.Empty));
            for (int i = next; i < drifters; i++) engine.CameraView.RemoveFogStirrer(new(BodyStirrerId + 1 + (uint)i));
        }
        drifters = next;
        return Observe();
    }

    [DebugCommand("fogstir.quality", Description = "Volumetric fog's quality (off, low or high), as a player's video option sets it.")]
    public string Quality(string quality)
    {
        VolumetricFogQuality level = quality switch { "low" => VolumetricFogQuality.Low, "high" => VolumetricFogQuality.High, _ => VolumetricFogQuality.Off };
        engine.RendererSettings.Set(engine.RendererSettings.Read().Requested with { VolumetricFog = level });
        return Observe();
    }

    [DebugCommand("fogstir.pause", Description = "Holds the body, drifters and lantern where they are (true), still, so the wake settles; or lets them move on (false).")]
    public string Hold(bool hold)
    {
        paused = hold;
        if (hold && mode == "region")
        {
            bodyVelocity = Vector3.Zero;
            engine.CameraView.SetFogStirrer(new(BodyStirrerId, body, bodyVelocity, BodyRadius, BodyPush, BodyClear));
            for (int i = 0; i < drifters; i++)
            {
                engine.CameraView.SetFogStirrer(new(BodyStirrerId + 1 + (uint)i, DrifterAt(i, time).At, Vector3.Zero, DrifterRadius, DrifterPush, DrifterClear));
            }
        }
        return Observe();
    }
}
