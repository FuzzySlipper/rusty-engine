using System.Numerics;
using System.Text.Json;
using System.Text.Json.Serialization;
using Rusty.Engine;
using Rusty.Engine.Debugging;

namespace CsharpPartsRigs;

/// <summary>
/// Parts rigs drawn as upright pieces in the world. A townsperson is four views of rusty-dagger's
/// prototype rigs (front, side, back and the side mirrored); the Engine draws the view each camera
/// sees, and every view walks on one clip time. Beside it, rig-c's gallery `features` rig plays a
/// clip that keys events. The product chooses clips, crossfades, the `turn` value and the camera.
/// </summary>
public sealed class Product : IEngineProduct, IDebugCommandModuleSource, IDebugCommandModule
{
    private const ulong TownspersonId = 1, FeaturesId = 2, FloorId = 3;
    // The townsperson's rigs stand 896 units tall, feet at 0; a person about 1.75 m.
    private const float TownspersonMetresPerUnit = 1.75f / 896;
    private const string FrontModel = "people/townsperson/front/skeleton.model.json";
    private const string SideModel = "people/townsperson/side/skeleton.model.json";
    private const string BackModel = "people/townsperson/back/skeleton.model.json";
    private const string FeaturesModel = "people/features/skeleton.model.json";
    private const float FeaturesMetresPerUnit = .0012f;
    private static readonly Vector3 FeaturesPosition = new(1.4f, 0, -.6f);
    // The side rig faces its page's right: a camera at the person's left (-x) sees it as drawn.
    private const float FrontHeading = 0, SideHeading = -MathF.PI / 2, BackHeading = MathF.PI, MirroredSideHeading = MathF.PI / 2;
    private const string WalkClip = "walk", FeaturesClip = "a", TurnBone = "turn", HeadBone = "head";
    private const float StartCrossfadeSeconds = 0;
    // Camera orbit about the townsperson.
    private const float OrbitDistance = 3.6f, OrbitHeight = 1.3f, OrbitStartDegrees = 20;
    private static readonly Vector3 OrbitTarget = new(0, .95f, 0);
    private const float FloorHalfSize = 4, FloorThickness = .1f;
    private static readonly Color FloorColor = new(.28f, .3f, .27f, 1);
    private const int RecentEventLimit = 6;

    private readonly IEngineContext engine;
    private readonly PartsRig front, side, back, features;
    private readonly Appearance townspersonLook, featuresLook, floorLook;
    private readonly Camera camera;
    private readonly float[] headings = [FrontHeading, SideHeading, BackHeading, MirroredSideHeading];
    private float orbitDegrees = OrbitStartDegrees, spinDegreesPerSecond;
    private float? turn;
    private readonly Dictionary<string, int> eventCounts = [];
    private readonly Queue<string> recentEvents = new();

    public Product(ProductCreateContext context)
    {
        engine = context.Engine;
        IPartsRigsService rigs = engine.PartsRigs;
        front = rigs.Load(new PartsRigLoadRequest(FrontModel, TextureFilter.Linear)).Rig;
        side = rigs.Load(new PartsRigLoadRequest(SideModel, TextureFilter.Linear)).Rig;
        back = rigs.Load(new PartsRigLoadRequest(BackModel, TextureFilter.Linear)).Rig;
        features = rigs.Load(new PartsRigLoadRequest(FeaturesModel, TextureFilter.Linear)).Rig;
        townspersonLook = rigs.CreateAppearance(new PartsRigAppearanceRequest(new PartsRigView[]
        {
            new(front, FrontHeading, false),
            new(side, SideHeading, false),
            new(back, BackHeading, false),
            new(side, MirroredSideHeading, true),
        }, TownspersonMetresPerUnit));
        featuresLook = rigs.CreateAppearance(new PartsRigAppearanceRequest(
            new PartsRigView[] { new(features, 0, false) }, FeaturesMetresPerUnit));
        floorLook = engine.Graphics.CreatePrimitive(new(PrimitiveGeometry.Cube, false, FloorColor));
        engine.Graphics.PublishSnapshot(
        [
            Fact(FloorId, floorLook, new Vector3(0, -FloorThickness / 2, 0), new Vector3(FloorHalfSize * 2, FloorThickness, FloorHalfSize * 2)),
            Fact(TownspersonId, townspersonLook, Vector3.Zero, Vector3.One),
            Fact(FeaturesId, featuresLook, FeaturesPosition, Vector3.One),
        ]);
        rigs.Play(new PartsRigPlayRequest(TownspersonId, WalkClip, StartCrossfadeSeconds));
        rigs.Play(new PartsRigPlayRequest(FeaturesId, FeaturesClip, StartCrossfadeSeconds));
        camera = engine.CameraView.CreateCamera(CameraDescriptor());
        engine.CameraView.SetActiveCamera(camera);
    }

    public void Start() { }

    public ProductUpdateResult Update(ProductUpdate update)
    {
        foreach (PartsRigEvent fired in engine.PartsRigs.ReadEvents().Span)
        {
            eventCounts[fired.Name] = eventCounts.GetValueOrDefault(fired.Name) + 1;
            recentEvents.Enqueue($"{fired.ObjectId}:{fired.Name}@{fired.ClipSeconds:0.###} int {fired.IntValue}");
            while (recentEvents.Count > RecentEventLimit) recentEvents.Dequeue();
        }
        if (spinDegreesPerSecond != 0)
        {
            float seconds = (float)(update.Facts.FixedDeltaSeconds * update.Facts.AdmittedStepCount);
            orbitDegrees = (orbitDegrees + spinDegreesPerSecond * seconds) % 360;
            engine.CameraView.UpdateCamera(new CameraUpdateRequest(camera, CameraDescriptor()));
        }
        return ProductUpdateResult.None;
    }

    public void Pause() { }
    public void Resume() { }
    public void Restart() { }
    public void Shutdown() { }

    public void Dispose()
    {
        engine.Graphics.PublishSnapshot([]);
        camera.Dispose();
        townspersonLook.Dispose();
        featuresLook.Dispose();
        floorLook.Dispose();
        front.Dispose();
        side.Dispose();
        back.Dispose();
        features.Dispose();
    }

    private CameraDescriptor CameraDescriptor()
    {
        float radians = orbitDegrees * MathF.PI / 180;
        Vector3 eye = OrbitTarget + new Vector3(MathF.Sin(radians) * OrbitDistance, OrbitHeight - OrbitTarget.Y, MathF.Cos(radians) * OrbitDistance);
        CameraQueries.TryLookAtPose(eye, OrbitTarget, 0, out CameraPose pose);
        return new CameraDescriptor(pose, CameraBasisMode.Derived, default,
            new CameraProjection(CameraProjectionKind.Perspective, 50, 0, .05, 100), CameraViewports.Full, 0);
    }

    /// <summary>The view the camera's bearing on the townsperson picks: the nearest heading, as the Engine picks it.</summary>
    private int ViewSeen()
    {
        float bearing = orbitDegrees * MathF.PI / 180;
        int nearest = 0;
        for (int index = 1; index < headings.Length; index++)
        {
            if (Distance(bearing, headings[index]) < Distance(bearing, headings[nearest])) nearest = index;
        }
        return nearest;

        static float Distance(float a, float b)
        {
            float turn = ((a - b) % MathF.Tau + MathF.Tau) % MathF.Tau;
            return MathF.Min(turn, MathF.Tau - turn);
        }
    }

    private static AppearanceFact Fact(ulong id, Appearance appearance, Vector3 position, Vector3 scale) =>
        new(id, false, 0, new Transform(position, Quaternion.Identity, scale), appearance, true, RenderLayer.Scene);

    public void RegisterDebugCommands(IDebugCommandModuleRegistrar registrar) => registrar.Register(this);

    [DebugCommand("rigs.observe", Description = "Read-only: each rig's clip, clip time and speed, the townsperson's head in the view the camera sees, the camera bearing and the events read.")]
    public string Observe()
    {
        PartsRigReadout person = engine.PartsRigs.Read(TownspersonId);
        PartsRigReadout gallery = engine.PartsRigs.Read(FeaturesId);
        int view = ViewSeen();
        PartsRigBoneReadout head = engine.PartsRigs.ReadBone(new PartsRigBoneRequest(TownspersonId, HeadBone, (uint)view));
        return JsonSerializer.Serialize(new RigFacts(
            Rig(person), Rig(gallery), orbitDegrees, spinDegreesPerSecond, view, turn,
            new BoneFacts(head.Position.X, head.Position.Y, head.Offset.X, head.Offset.Y),
            eventCounts, [.. recentEvents]), RigJsonContext.Default.RigFacts);

        static RigState Rig(PartsRigReadout readout) =>
            new(readout.Views, readout.Playing ? readout.Clip : null, readout.ClipSeconds, readout.Speed, readout.Posed);
    }

    [DebugCommand("rigs.play", Description = "Play a clip on the townsperson: clip name, crossfade seconds, speed (1 as authored). Every view plays it.")]
    public string Play(string clip, float crossfadeSeconds, float speed)
    {
        engine.PartsRigs.Play(new PartsRigPlayRequest(TownspersonId, clip, true, speed, crossfadeSeconds));
        return Observe();
    }

    [DebugCommand("rigs.stop", Description = "Fade the townsperson back to its setup pose over the given seconds.")]
    public string Stop(float crossfadeSeconds)
    {
        engine.PartsRigs.Stop(new PartsRigStopRequest(TownspersonId, crossfadeSeconds));
        return Observe();
    }

    [DebugCommand("rigs.turn", Description = "Hold the townsperson's turn bone's x at a value (about -24 to 24 turns the head); its transform constraints follow.")]
    public string Turn(float value)
    {
        engine.PartsRigs.DriveBone(PartsRigDriveRequest.Hold(TownspersonId, TurnBone, PartsRigBoneChannel.X, value));
        turn = value;
        return Observe();
    }

    [DebugCommand("rigs.release", Description = "Let the townsperson's clips pose its turn bone again.")]
    public string Release()
    {
        engine.PartsRigs.DriveBone(PartsRigDriveRequest.Release(TownspersonId, TurnBone, PartsRigBoneChannel.X));
        turn = null;
        return Observe();
    }

    [DebugCommand("rigs.orbit", Description = "Put the camera at a bearing about the townsperson, in degrees from in front of it toward its left (+x).")]
    public string Orbit(float degrees)
    {
        orbitDegrees = degrees;
        spinDegreesPerSecond = 0;
        engine.CameraView.UpdateCamera(new CameraUpdateRequest(camera, CameraDescriptor()));
        return Observe();
    }

    [DebugCommand("rigs.spin", Description = "Orbit the camera about the townsperson at the given degrees per second of world time (0 stops).")]
    public string Spin(float degreesPerSecond)
    {
        spinDegreesPerSecond = degreesPerSecond;
        return Observe();
    }
}

internal sealed record RigState(uint Views, string? Clip, double ClipSeconds, float Speed, bool Posed);

internal sealed record BoneFacts(float RigX, float RigY, float OffsetX, float OffsetY);

internal sealed record RigFacts(
    RigState Townsperson,
    RigState Features,
    float OrbitDegrees,
    float SpinDegreesPerSecond,
    int ViewSeen,
    float? Turn,
    BoneFacts Head,
    Dictionary<string, int> EventCounts,
    string[] RecentEvents);

[JsonSourceGenerationOptions(PropertyNamingPolicy = JsonKnownNamingPolicy.CamelCase)]
[JsonSerializable(typeof(RigFacts))]
internal partial class RigJsonContext : JsonSerializerContext { }
