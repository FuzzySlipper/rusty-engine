using System.Numerics;
using System.Text.Json;
using System.Text.Json.Serialization;
using Rusty.Engine;
using Rusty.Engine.Debugging;

namespace CsharpPartsRigs;

/// <summary>
/// Parts rigs drawn as upright pieces in the world. A townsperson is four views of rusty-dagger's
/// prototype rigs (front, side, back and the side mirrored); the Engine draws the view each camera
/// sees, and every view walks on one clip time. Over the walk, the rig's `idle` breathing is added
/// on track 1. Beside it, rig-c's gallery `features` rig plays a clip that keys events, with its
/// `b` clip added over it on track 1 at part alpha, and rig-c's gallery `look` turns its face as a
/// slider reads the dial bone the product drives. In the lower left, rig-c's gallery `portrait`
/// is a dialogue bust placed in a screen square of whole pixels per texel, inside a frame the UI
/// draws and anchors, so the page's layout places it. The product chooses clips, tracks, alphas,
/// crossfades, the `turn` and dial values, the bust's fallback square and the camera.
/// </summary>
public sealed class Product : IEngineProduct, IDebugCommandModuleSource, IDebugCommandModule
{
    private const ulong TownspersonId = 1, FeaturesId = 2, FloorId = 3, MarkerId = 4, LookId = 5, BustId = 6;
    // The townsperson's rigs stand 896 units tall, feet at 0; a person about 1.75 m.
    private const float TownspersonMetresPerUnit = 1.75f / 896;
    private const string FrontModel = "people/townsperson/front/skeleton.model.json";
    private const string SideModel = "people/townsperson/side/skeleton.model.json";
    private const string BackModel = "people/townsperson/back/skeleton.model.json";
    private const string FeaturesModel = "people/features/skeleton.model.json";
    private const float FeaturesMetresPerUnit = .0012f;
    private static readonly Vector3 FeaturesPosition = new(1.4f, 0, -.6f);
    private const string LookModel = "people/look/skeleton.model.json";
    // The look rig stands 880 units tall: a head and shoulders about a metre high.
    private const float LookMetresPerUnit = .0012f;
    private static readonly Vector3 LookPosition = new(-1.4f, 0, -.6f);
    // Its `yaw` slider reads this dial's local rotation, -19 to 19 degrees, as a time in clip
    // `turn`: -19 faces one way, 0 ahead, 19 the other. `nose` shows where the face turned.
    private const string YawDial = "yaw_dial", NoseBone = "nose";
    // The side rig faces its page's right: a camera at the person's left (-x) sees it as drawn.
    private const float FrontHeading = 0, SideHeading = -MathF.PI / 2, BackHeading = MathF.PI, MirroredSideHeading = MathF.PI / 2;
    private const string WalkClip = "walk", FeaturesClip = "a", TurnBone = "turn", HeadBone = "head";
    // The effect layers each rig starts with: the townsperson's breathing over its walk, and the
    // gallery rig's `b` over its `a`, both added on track 1.
    private const uint LayerTrack = 1, ReadTrackCount = 3;
    private const string BreathingClip = "idle", FeaturesLayerClip = "b";
    private const float BreathingAlpha = 1, FeaturesLayerAlpha = .6f;
    private const string PersonRig = "person", FeaturesRig = "features", BustRig = "bust";
    private static readonly Color FogColor = new(.62f, .66f, .72f, 1);
    // A lit scene's finish for `rigs.finish`: ACES with a warm, contrasty grade and some bloom,
    // and auto exposure allowed above 1, as a night scene lifts; the bust takes none of it.
    private static readonly ColorGradingRequest FinishGrading = new(.35f, -.1f, .3f, -.2f);
    private static readonly BloomRequest FinishBloom = new(.8f, .6f);
    private const float FinishAdaptSpeed = 2, FinishMinExposure = 1, FinishMaxExposure = 4;
    // A marker the product keeps on a bone every view has, where the camera draws it.
    private const float MarkerSize = .06f;
    private static readonly Color MarkerColor = new(.95f, .15f, .1f, 1);
    private const float StartCrossfadeSeconds = 0;
    // Camera orbit about the townsperson.
    private const float OrbitDistance = 3.6f, OrbitHeight = 1.3f, OrbitStartDegrees = 20;
    private static readonly Vector3 OrbitTarget = new(0, .95f, 0);
    private const float FloorHalfSize = 4, FloorThickness = .1f;
    private static readonly Color FloorColor = new(.28f, .3f, .27f, 1);
    private const int RecentEventLimit = 6;
    // The dialogue bust: rig-c's `portrait`, 640 units wide on a plate 880 tall. Its square frame is
    // 127 texels of 640/127 units, 47 texels up from the rig's origin, where its texel grid starts, so
    // the frame's edges are texel edges; the UI's frame leaves a window that size (product-ui/main.js
    // lays it out by the same rule).
    private const string BustModel = "people/portrait/skeleton.model.json";
    private const int BustTexels = 127, BustFrameTexelsUp = 47;
    private const float BustFrameUnits = 640;
    private const float BustTexelSnap = BustFrameUnits / BustTexels;
    private static readonly Vector2 BustFrameMinimum = new(0, BustFrameTexelsUp * BustTexelSnap);
    private const string BustClip = "idle";
    // The square is 127 CSS pixels times the largest whole scale, at most 3, within 60% of the
    // window's height and 45% of its width, 32 pixels in from the lower left.
    private const int BustMaxScale = 3;
    private const double BustHeightShare = .6, BustWidthShare = .45, BustInset = 32;
    private const string FrameSetup = "setup", FrameSquare = "square";
    // The UI anchors its frame's window under this name (product-ui/main.js): while anchored, the
    // bust follows that element as the page lays it out, and the product's square is its fallback.
    private const string BustAnchor = "bust";

    private readonly IEngineContext engine;
    private readonly PartsRig front, side, back, features, look, portrait;
    private readonly Appearance townspersonLook, featuresLook, lookLook, floorLook, markerLook, bustLook;
    private readonly Camera camera;
    private string attachedBone = HeadBone;
    private PartsRigBoneReadout attached;
    private float orbitDegrees = OrbitStartDegrees, spinDegreesPerSecond;
    private float? turn, yaw;
    private readonly Dictionary<string, int> eventCounts = [];
    private readonly Queue<string> recentEvents = new();
    private ulong surfaceRevision = ulong.MaxValue;
    private BustPlacement bust = new(0, 0, 0, 0, 0, FrameSquare, true);

    public Product(ProductCreateContext context)
    {
        engine = context.Engine;
        IPartsRigsService rigs = engine.PartsRigs;
        front = rigs.Load(new PartsRigLoadRequest(FrontModel, TextureFilter.Linear)).Rig;
        side = rigs.Load(new PartsRigLoadRequest(SideModel, TextureFilter.Linear)).Rig;
        back = rigs.Load(new PartsRigLoadRequest(BackModel, TextureFilter.Linear)).Rig;
        features = rigs.Load(new PartsRigLoadRequest(FeaturesModel, TextureFilter.Linear)).Rig;
        look = rigs.Load(new PartsRigLoadRequest(LookModel, TextureFilter.Linear)).Rig;
        portrait = rigs.Load(new PartsRigLoadRequest(BustModel, TextureFilter.Nearest)).Rig;
        townspersonLook = rigs.CreateAppearance(new PartsRigAppearanceRequest(new PartsRigView[]
        {
            new(front, FrontHeading, false),
            new(side, SideHeading, false),
            new(back, BackHeading, false),
            new(side, MirroredSideHeading, true),
        }, TownspersonMetresPerUnit));
        featuresLook = rigs.CreateAppearance(new PartsRigAppearanceRequest(
            new PartsRigView[] { new(features, 0, false) }, FeaturesMetresPerUnit));
        lookLook = rigs.CreateAppearance(new PartsRigAppearanceRequest(
            new PartsRigView[] { new(look, 0, false) }, LookMetresPerUnit));
        // Its scale is not read once it is placed in the viewport.
        bustLook = rigs.CreateAppearance(new PartsRigAppearanceRequest(
            new PartsRigView[] { new(portrait, 0, false) }, 1) with { TexelSnap = BustTexelSnap });
        floorLook = engine.Graphics.CreatePrimitive(new(PrimitiveGeometry.Cube, false, FloorColor));
        markerLook = engine.Graphics.CreatePrimitive(new(PrimitiveGeometry.Cube, false, MarkerColor));
        engine.Graphics.PublishSnapshot(
        [
            Fact(FloorId, floorLook, new Vector3(0, -FloorThickness / 2, 0), new Vector3(FloorHalfSize * 2, FloorThickness, FloorHalfSize * 2)),
            Fact(TownspersonId, townspersonLook, Vector3.Zero, Vector3.One),
            Fact(FeaturesId, featuresLook, FeaturesPosition, Vector3.One),
            Fact(LookId, lookLook, LookPosition, Vector3.One),
            BustFact(),
        ]);
        rigs.Play(new PartsRigPlayRequest(TownspersonId, WalkClip, StartCrossfadeSeconds));
        rigs.Play(new PartsRigPlayRequest(FeaturesId, FeaturesClip, StartCrossfadeSeconds));
        rigs.Play(PartsRigPlayRequest.Layer(TownspersonId, LayerTrack, BreathingClip, BreathingAlpha, true, StartCrossfadeSeconds));
        rigs.Play(PartsRigPlayRequest.Layer(FeaturesId, LayerTrack, FeaturesLayerClip, FeaturesLayerAlpha, true, StartCrossfadeSeconds));
        rigs.Play(new PartsRigPlayRequest(BustId, BustClip, StartCrossfadeSeconds));
        PlaceBust(true);
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
        Attach();
        PlaceBust(false);
        return ProductUpdateResult.None;
    }

    /// <summary>
    /// Fits the bust's square to the window as the page reports it: a new square on a resize while
    /// it is not anchored, or when <paramref name="force"/> asks after the frame or anchor changes.
    /// Anchored, the UI's window places it and a resize runs nothing here.
    /// </summary>
    private void PlaceBust(bool force)
    {
        CameraSurfaceReadout surface = engine.CameraView.ReadSurface();
        if (!force && (bust.Anchored || surface.Revision == surfaceRevision)) return;
        if (!surface.Reported)
        {
            // No page yet: a placement to anchor, its square sized for the largest scale.
            surface = surface with { CssWidth = BustTexels * BustMaxScale / BustWidthShare, CssHeight = BustTexels * BustMaxScale / BustHeightShare };
        }
        surfaceRevision = surface.Revision;
        double fits = Math.Min(surface.CssHeight * BustHeightShare, surface.CssWidth * BustWidthShare);
        int scale = Math.Clamp((int)(fits / BustTexels), 1, BustMaxScale);
        double side = BustTexels * scale;
        bust = bust with { Scale = scale, SideCss = side, CssWidth = surface.CssWidth, CssHeight = surface.CssHeight };
        var minimum = new Vector2((float)(BustInset / surface.CssWidth), (float)(BustInset / surface.CssHeight));
        var size = new Vector2((float)(side / surface.CssWidth), (float)(side / surface.CssHeight));
        var alignment = new Vector2(.5f, .5f);
        engine.PartsRigs.SetViewport(bust.Frame == FrameSetup
            ? new PartsRigViewportRequest(bustLook, minimum, size, alignment, SpriteViewportFit.Contain)
            : new PartsRigViewportRequest(bustLook, minimum, size, alignment, SpriteViewportFit.Contain,
                BustFrameMinimum, new Vector2(BustFrameUnits)));
        engine.PartsRigs.SetViewportAnchor(new PartsRigViewportAnchorRequest(bustLook, bust.Anchored ? BustAnchor : ""));
        bust = bust with { Placements = bust.Placements + 1 };
        engine.Graphics.PublishChanges(new(
            new[] { BustFact() }, ReadOnlyMemory<ulong>.Empty, ReadOnlyMemory<MeshJointAttachment>.Empty));
    }

    // Published away from everything, and turned: the placement, not the transform, puts it on screen.
    private AppearanceFact BustFact() =>
        new(BustId, false, 0, new Transform(new Vector3(40, -3, -60), Quaternion.CreateFromAxisAngle(Vector3.UnitY, 1),
            new Vector3(5)), bustLook, true, RenderLayer.Viewmodel);

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
        lookLook.Dispose();
        bustLook.Dispose();
        floorLook.Dispose();
        markerLook.Dispose();
        front.Dispose();
        side.Dispose();
        back.Dispose();
        features.Dispose();
        look.Dispose();
        portrait.Dispose();
    }

    private CameraDescriptor CameraDescriptor()
    {
        float radians = orbitDegrees * MathF.PI / 180;
        Vector3 eye = OrbitTarget + new Vector3(MathF.Sin(radians) * OrbitDistance, OrbitHeight - OrbitTarget.Y, MathF.Cos(radians) * OrbitDistance);
        CameraQueries.TryLookAtPose(eye, OrbitTarget, 0, out CameraPose pose);
        return new CameraDescriptor(pose, CameraBasisMode.Derived, default,
            new CameraProjection(CameraProjectionKind.Perspective, 50, 0, .05, 100), CameraViewports.Full, 0);
    }

    /// <summary>
    /// Puts the marker where the camera draws the attached bone, in the view it draws, as the
    /// townsperson was last posed.
    /// </summary>
    private void Attach()
    {
        attached = engine.PartsRigs.ReadBone(new PartsRigBoneRequest(TownspersonId, attachedBone, camera));
        engine.Graphics.PublishChanges(new(
            new[] { Fact(MarkerId, markerLook, attached.World, new Vector3(MarkerSize)) },
            ReadOnlyMemory<ulong>.Empty,
            ReadOnlyMemory<MeshJointAttachment>.Empty));
    }

    private static AppearanceFact Fact(ulong id, Appearance appearance, Vector3 position, Vector3 scale) =>
        new(id, false, 0, new Transform(position, Quaternion.Identity, scale), appearance, true, RenderLayer.Scene);

    public void RegisterDebugCommands(IDebugCommandModuleRegistrar registrar) => registrar.Register(this);

    [DebugCommand("rigs.observe", Description = "Read-only: each rig's tracks 0-2 (clip, clip time, alpha, additive) and speed, the view the camera sees, the attached bone where it draws, the camera bearing and the events read.")]
    public string Observe()
    {
        RigState person = Rig(TownspersonId);
        RigState gallery = Rig(FeaturesId);
        PartsRigBoneReadout bone = engine.PartsRigs.ReadBone(new PartsRigBoneRequest(TownspersonId, attachedBone, camera));
        PartsRigBoneReadout dial = engine.PartsRigs.ReadBone(new PartsRigBoneRequest(LookId, YawDial, camera));
        PartsRigBoneReadout nose = engine.PartsRigs.ReadBone(new PartsRigBoneRequest(LookId, NoseBone, camera));
        return JsonSerializer.Serialize(new RigFacts(
            person, gallery, orbitDegrees, spinDegreesPerSecond, (int)bone.View, turn,
            new BoneFacts(attachedBone, bone.Position.X, bone.Position.Y, bone.World.X, bone.World.Y, bone.World.Z,
                attached.World.X, attached.World.Y, attached.World.Z),
            new LookFacts(yaw, dial.RotationDegrees, nose.Position.X, nose.Position.Y),
            Rig(BustId), bust, eventCounts, [.. recentEvents]), RigJsonContext.Default.RigFacts);

        RigState Rig(ulong objectId)
        {
            PartsRigReadout baseTrack = engine.PartsRigs.Read(new PartsRigTrackRequest(objectId));
            var tracks = new TrackState?[ReadTrackCount];
            for (uint track = 0; track < ReadTrackCount; track++)
            {
                PartsRigReadout readout = engine.PartsRigs.Read(new PartsRigTrackRequest(objectId, track));
                tracks[track] = readout.Playing
                    ? new TrackState(readout.Clip, readout.ClipSeconds, readout.Looping, readout.Alpha, readout.Additive)
                    : null;
            }
            return new RigState(baseTrack.Views, tracks, baseTrack.Speed, baseTrack.Posed);
        }
    }

    [DebugCommand("rigs.play", Description = "Play a clip on the townsperson's base track (0): clip name, crossfade seconds, speed (1 as authored). Every view plays it.")]
    public string Play(string clip, float crossfadeSeconds, float speed)
    {
        engine.PartsRigs.Play(new PartsRigPlayRequest(TownspersonId, clip, true, speed, crossfadeSeconds));
        return Observe();
    }

    [DebugCommand("rigs.stop", Description = "Fade the townsperson's base track (0) back to its setup pose over the given seconds; layers above it play on.")]
    public string Stop(float crossfadeSeconds)
    {
        engine.PartsRigs.Stop(new PartsRigStopRequest(TownspersonId, crossfadeSeconds));
        return Observe();
    }

    [DebugCommand("rigs.layer", Description = "Play a looping clip on a track of a rig (person or features) over the tracks below: rig, track (1 and up layer over the base), clip, alpha 0-1, additive (true adds its keyed values, false replaces part way), crossfade seconds.")]
    public string Layer(string rig, int track, string clip, float alpha, bool additive, float crossfadeSeconds)
    {
        engine.PartsRigs.Play(PartsRigPlayRequest.Layer(RigId(rig), checked((uint)track), clip, alpha, additive, crossfadeSeconds));
        return Observe();
    }

    [DebugCommand("rigs.alpha", Description = "Set how strongly a rig's (person or features) track applies: rig, track, alpha 0-1.")]
    public string Alpha(string rig, int track, float alpha)
    {
        engine.PartsRigs.SetTrackAlpha(new PartsRigTrackAlphaRequest(RigId(rig), checked((uint)track), alpha));
        return Observe();
    }

    [DebugCommand("rigs.unlayer", Description = "Fade a rig's (person or features) track out over the given seconds, leaving the tracks below it: rig, track, crossfade seconds.")]
    public string Unlayer(string rig, int track, float crossfadeSeconds)
    {
        engine.PartsRigs.Stop(new PartsRigStopRequest(RigId(rig), crossfadeSeconds, checked((uint)track)));
        return Observe();
    }

    private static ulong RigId(string rig) => rig switch
    {
        PersonRig => TownspersonId,
        FeaturesRig => FeaturesId,
        BustRig => BustId,
        _ => throw new ArgumentException($"no rig `{rig}`: {PersonRig}, {FeaturesRig} or {BustRig}", nameof(rig)),
    };

    [DebugCommand("rigs.bust.play", Description = "Play a looping clip on the bust's base track: idle (breathing and blinks), gaze or turn, with a crossfade in seconds.")]
    public string PlayBust(string clip, float crossfadeSeconds)
    {
        engine.PartsRigs.Play(new PartsRigPlayRequest(BustId, clip, crossfadeSeconds));
        return Observe();
    }

    [DebugCommand("rigs.bust.frame", Description = "Fit the bust by its 127-texel square (square) or by its setup pose's bounds (setup), which takes the whole plate.")]
    public string BustFrame(string frame)
    {
        if (frame is not (FrameSetup or FrameSquare))
            throw new ArgumentException($"no frame `{frame}`: {FrameSquare} or {FrameSetup}", nameof(frame));
        bust = bust with { Frame = frame };
        PlaceBust(true);
        return Observe();
    }

    [DebugCommand("rigs.bust.anchor", Description = "Place the bust by the UI frame's anchored window (on: the page's layout moves it, no product call) or by the product's square from the window size (off).")]
    public string BustAnchored(bool anchored)
    {
        bust = bust with { Anchored = anchored };
        PlaceBust(true);
        return Observe();
    }

    [DebugCommand("rigs.fog", Description = "Exponential distance fog of the given density per metre over the scene (0 for none); the bust stays as drawn.")]
    public string Fog(float density)
    {
        engine.CameraView.SetFog(new FogRequest(density > 0 ? FogMode.Exponential : FogMode.Off, FogColor, 0, 0, density));
        return Observe();
    }

    [DebugCommand("rigs.finish", Description = "Finish the scene as a lit one is: ACES at the given exposure, graded, bloomed and auto exposed up to 4 (0 for none of it); the bust keeps its pages' colours.")]
    public string Finish(float exposure)
    {
        bool on = exposure > 0;
        engine.CameraView.SetToneMapping(new(on ? ToneMappingOperator.AcesFilmic : ToneMappingOperator.None, on ? exposure : 1));
        engine.CameraView.SetColorGrading(on ? FinishGrading : default);
        engine.CameraView.SetBloom(on ? FinishBloom : default);
        engine.CameraView.SetAutoExposure(new AutoExposureRequest(on, FinishAdaptSpeed, FinishMinExposure, FinishMaxExposure));
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

    [DebugCommand("rigs.yaw", Description = "Hold the look rig's yaw dial at a rotation in degrees (-19 to 19); its slider turns the face to match.")]
    public string Yaw(float degrees)
    {
        engine.PartsRigs.DriveBone(PartsRigDriveRequest.Hold(LookId, YawDial, PartsRigBoneChannel.Rotation, degrees));
        yaw = degrees;
        return Observe();
    }

    [DebugCommand("rigs.yaw.release", Description = "Let the look rig's dial rest where its clips pose it again.")]
    public string ReleaseYaw()
    {
        engine.PartsRigs.DriveBone(PartsRigDriveRequest.Release(LookId, YawDial, PartsRigBoneChannel.Rotation));
        yaw = null;
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

    [DebugCommand("rigs.attach", Description = "Keep the red marker on a named bone of the townsperson (one every view has: head, neck, foot_r, foot_l, leg_r1, ...).")]
    public string AttachTo(string bone)
    {
        attachedBone = bone;
        Attach();
        return Observe();
    }

    [DebugCommand("rigs.place", Description = "Move the townsperson to x, z and turn it to a yaw in degrees; the marker follows.")]
    public string Place(float x, float z, float yawDegrees)
    {
        engine.Graphics.PublishChanges(new(
            new[] { new AppearanceFact(TownspersonId, false, 0, new Transform(new Vector3(x, 0, z),
                Quaternion.CreateFromAxisAngle(Vector3.UnitY, yawDegrees * MathF.PI / 180), Vector3.One), townspersonLook, true, RenderLayer.Scene) },
            ReadOnlyMemory<ulong>.Empty,
            ReadOnlyMemory<MeshJointAttachment>.Empty));
        return Observe();
    }

    [DebugCommand("rigs.spin", Description = "Orbit the camera about the townsperson at the given degrees per second of world time (0 stops).")]
    public string Spin(float degreesPerSecond)
    {
        spinDegreesPerSecond = degreesPerSecond;
        return Observe();
    }
}

internal sealed record TrackState(string Clip, double ClipSeconds, bool Looping, float Alpha, bool Additive);

internal sealed record RigState(uint Views, TrackState?[] Tracks, float Speed, bool Posed);

internal sealed record BoneFacts(string Bone, float RigX, float RigY, float WorldX, float WorldY, float WorldZ,
    float MarkerX, float MarkerY, float MarkerZ);

internal sealed record LookFacts(float? Yaw, float DialDegrees, float NoseX, float NoseY);

/// <summary>
/// Where the product last placed the bust: CSS pixels a texel, the square's side and the window's size, how
/// many times, the frame, and whether it follows the UI's anchored window instead.
/// </summary>
internal sealed record BustPlacement(int Scale, double SideCss, double CssWidth, double CssHeight, int Placements, string Frame, bool Anchored);

internal sealed record RigFacts(
    RigState Townsperson,
    RigState Features,
    float OrbitDegrees,
    float SpinDegreesPerSecond,
    int ViewSeen,
    float? Turn,
    BoneFacts Attached,
    LookFacts Look,
    RigState Bust,
    BustPlacement BustPlacement,
    Dictionary<string, int> EventCounts,
    string[] RecentEvents);

[JsonSourceGenerationOptions(PropertyNamingPolicy = JsonKnownNamingPolicy.CamelCase)]
[JsonSerializable(typeof(RigFacts))]
internal partial class RigJsonContext : JsonSerializerContext { }
