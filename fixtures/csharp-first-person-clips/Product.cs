using System.Globalization;
using System.Numerics;
using Rusty.Engine;
using Rusty.Engine.Debugging;
using Rusty.Engine.FirstPerson;

namespace CsharpFirstPersonClips;

/// <summary>
/// Clip-led first-person arms on Drillimpact's PSX arms (CC0): the knife strike from a clip pack, fitted to two action
/// timings, with the look riding the main hand; a staff held in both hands over the guard clip, the off hand reaching
/// the staff; and a weapon-led look the arms fade to and from. Head bob carries the arms in every scene. The Engine
/// measures each drawn wrist against the point on the look it should hold (landmark pairs), and the product keeps the
/// worst of those and the largest move of the look in one update.
/// </summary>
public sealed class Product : IEngineProduct, IDebugCommandModuleSource, IDebugCommandModule
{
    private const ulong ArmsId = 10521, LookId = 10522;
    private const string Rest = "rest", Guard = "guard_idle", Idle = "knife_idle";
    private const string Pairs = "fixture.rides";
    /// <summary>The strike, its phases marked in the clip: drawn back to 0.3, the cut to 0.55, recovered by the end.</summary>
    private static readonly ArmClip Strike = new("knife_hit_01", new(0, 0.3f, 0.55f, 1));
    /// <summary>The same strike at two timings: a quick jab and a slow, wound-up cut.</summary>
    private static readonly PhaseTiming Quick = new(0.15f, 0.08f, 0.3f), Slow = new(0.5f, 0.12f, 0.7f);
    /// <summary>Seconds of idle clip between strikes, and seconds per scene in the cycle.</summary>
    private const float Between = 0.6f, SceneSeconds = 3;
    private static readonly string[] Cycle = ["strike-quick", "strike-slow", "weapon", "two-hand"];
    /// <summary>The box model (0.16 x 0.8 x 0.16 m along +Y) scaled to a knife and to a staff, each held at a point on its length.</summary>
    private static readonly (Vector3 Scale, float Grip) Knife = (new(0.2f, 0.35f, 0.2f), 0.12f), Staff = (new(0.18f, 1.3f, 0.18f), 0.35f);
    /// <summary>On the staff, the off grip from the main one (look space: its length runs along -Z).</summary>
    private static readonly Vector3 OffGrip = new(0, 0, -0.28f);
    /// <summary>The weapon-led look's grip and turn (camera space).</summary>
    private static readonly Vector3 WeaponGrip = new(0.2f, -0.12f, -0.36f);
    private static readonly Quaternion WeaponLook = Quaternion.CreateFromYawPitchRoll(0.15f, 0.1f, 0);
    /// <summary>How far the arms are raised from where the rig's camera joint puts them, to frame the strike (metres).</summary>
    private static readonly Vector3 Raise = new(0, 0.08f, 0);
    /// <summary>Head bob: metres up and across, and steps a second.</summary>
    private const float BobUp = 0.008f, BobAcross = 0.005f, BobSteps = 1.7f;

    private readonly IEngineContext engine;
    private readonly Camera camera;
    private readonly RenderResource lookModel;
    private readonly Appearance look;
    private readonly HeldArms held;
    private readonly FirstPersonArms arms;
    private readonly Vector3 hold;
    private string scene = "cycle";
    private double started, last = double.NaN;
    private float? scrub;
    private bool bob = true;
    private Vector3? lastGrip;
    private Quaternion lastLook = Quaternion.Identity;
    /// <summary>Per scene: the worst measured distance of the main wrist and the off palm from where the look holds them, and the look's largest move in one update while the arms fade.</summary>
    private readonly Dictionary<string, (float Main, float Off, float FadeStep)> worst = [];
    private string published = "";
    private string measured = "";

    public Product(ProductCreateContext context)
    {
        engine = context.Engine;
        CameraQueries.TryLookAtPose(Vector3.Zero, new(0, 0, -1), 0, out CameraPose pose);
        camera = engine.CameraView.CreateCamera(new(pose, CameraBasisMode.Derived, default,
            new(CameraProjectionKind.Perspective, 70, 0, .02f, 50), CameraViewports.Full));
        engine.CameraView.SetActiveCamera(camera);
        using ContentReference lookSource = engine.Content.OpenReference(new("weapon.glb"));
        lookModel = engine.Animation.OpenAnimatedMeshFromContent(new(lookSource));
        look = engine.Animation.CreateAnimatedMeshAppearance(new(lookModel));
        ArmJoints Arm(string s) => new($"upper_arm.{s}", $"forearm.{s}", $"hand.{s}",
            [.. new[] { "f_index", "f_middle", "f_ring", "f_pinky" }.Select(f => new[] { $"{f}.01.{s}", $"{f}.02.{s}", $"{f}.03.{s}" })],
            [$"thumb.01.{s}", $"thumb.02.{s}", $"thumb.03.{s}"]);
        ArmRig rig = new("arms.glb", Rest, new(0, 180, 0), Arm("R"), Arm("L"), DigitFrames.Measured, new(0, 0.08f, -0.03f), new(HandShape.None))
        {
            Packs = [new("arms.knife.glb", "Drillimpact", "CC0-1.0")],
        };
        DigitBend still = new([0, 0, 0], [[0, 0], [0, 0], [0, 0]]);
        // The knife held in the fist as the clips swing it, its length out of the little finger's side and its back toward
        // the back of the hand.
        hold = HeldFrames.Degrees(Quaternion.Inverse(new Quaternion(0.5f, -0.5f, -0.5f, 0.5f)));
        DigitBend fist = new([70, 80, 50], [[0, 0], [20, 0], [30, 0]]);
        held = new(Vector3.Zero, new(0.6f, -0.7f, 0.1f), hold, new(fist, still, HandShape.None));
        arms = new FirstPersonArms(engine, held, rig, ArmsId);
        // The arms' shoulder where the rig's own camera joint puts the eye at the camera, raised to frame the strike.
        using ContentReference armsSource = engine.Content.OpenReference(new("arms.glb"));
        using RenderResource model = engine.Animation.OpenAnimatedMeshFromContent(new(armsSource));
        ReadOnlySpan<AnimationJointInfo> joints = engine.Animation.ReadJoints(model).Span;
        ReadOnlySpan<JointPose> rest = engine.Animation.SampleModelJointPose(
            AnimationModelPoseSampleRequest.OfClip(model, Rest, 0, new(Vector3.Zero, Quaternion.Identity, Vector3.One))).Span;
        Vector3 upper = rest[(int)AnimationJoints.IndexOf(joints, "upper_arm.R")].Model.Translation;
        Vector3 eye = rest[(int)AnimationJoints.IndexOf(joints, "camera")].Model.Translation;
        held.Shoulder = Vector3.Transform(upper - eye, HeldFrames.Turn(rig.Turn)) + Raise;
    }

    public ProductUpdateResult Update(ProductUpdate update)
    {
        double now = update.Facts.SimulationStep * update.Facts.FixedDeltaSeconds;
        float delta = double.IsNaN(last) ? 0 : (float)(now - last);
        last = now;
        float t = scrub ?? (float)(now - started);
        (string shown, float into) = scene == "cycle" ? (Cycle[(int)(t / SceneSeconds) % Cycle.Length], t % SceneSeconds) : (scene, t);
        bool twoHanded = shown == "two-hand";
        (Vector3 scale, float gripAt) = twoHanded ? Staff : Knife;

        arms.Lead(Frame(shown, into), delta);
        Vector3 carry = bob ? new(BobAcross * MathF.Sin(MathF.PI * BobSteps * t), BobUp * MathF.Sin(MathF.Tau * BobSteps * t), 0) : Vector3.Zero;
        arms.Carry = carry;
        HandAim main = new(WeaponGrip + carry, WeaponLook, null, 1, held.Pole);
        (Vector3 grip, Quaternion turn) = arms.Ride(main);
        HandAim off = twoHanded
            ? new(grip + Vector3.Transform(OffGrip, turn), turn, null, 1, HeldFrames.Mirror(held.Pole))
            : new(HeldFrames.Mirror(WeaponGrip) + carry, HeldFrames.Mirror(WeaponLook), null, 0, HeldFrames.Mirror(held.Pole), Reach: 0);
        if (arms.Clip.Led < 1) arms.Aim(main with { Grip = grip, Look = turn }, off);

        // The box's length (+Y) along the look's (-Z), its grip point on the look's grip.
        Quaternion fitTurn = Quaternion.CreateFromAxisAngle(Vector3.UnitX, -MathF.PI / 2);
        Quaternion drawnTurn = Quaternion.Normalize(turn * fitTurn);
        Vector3 at = grip - Vector3.Transform(new Vector3(0, gripAt, 0) * scale, drawnTurn);
        engine.Graphics.PublishSnapshot([
            arms.Fact(true),
            new(LookId, false, 0, new(at, drawnTurn, scale), look, true, RenderLayer.Viewmodel),
        ]);
        arms.Reach(main with { Grip = grip, Look = turn }, off);
        Measure(shown, into, grip, turn, twoHanded, scale, gripAt, fitTurn);
        return ProductUpdateResult.None;
    }

    // What the arms play in a scene at a moment: a strike after a stretch of idle, the guard, or nothing (weapon-led).
    private static ArmClipFrame? Frame(string shown, float into)
    {
        switch (shown)
        {
            case "strike-quick" or "strike-slow":
                PhaseTiming timing = shown == "strike-quick" ? Quick : Slow;
                float cycle = timing.Duration + Between, at = into % cycle;
                return at < Between ? new(Idle, at / Between % 1) : Strike.At(timing, at - Between);
            case "two-hand":
                return new(Guard, into / 2.1667f % 1);
            default:
                return null;
        }
    }

    // The Engine's measure of each drawn wrist against where the look should hold it, and the look's move this update.
    private void Measure(string shown, float into, Vector3 grip, Quaternion turn, bool twoHanded, Vector3 scale, float gripAt, Quaternion fitTurn)
    {
        // The pairs read are those drawn for the last update, of the scene shown then.
        (float Main, float Off, float FadeStep) w = worst.GetValueOrDefault(measured);
        foreach (LandmarkMeasure pair in engine.Graphics.ReadLandmarks(Pairs).Pairs.Span)
        {
            if (!pair.Drawn) continue;
            if (pair.Name == "main-wrist") w.Main = MathF.Max(w.Main, pair.Distance);
            else w.Off = MathF.Max(w.Off, pair.Distance);
        }
        worst[measured] = w;
        measured = shown;
        ArmClipReading clip = arms.Clip;
        bool fading = clip.Led is > 0 and < 1 || clip.Blend < 1;
        if (fading && lastGrip is { } before)
            worst[shown] = worst.GetValueOrDefault(shown) with { FadeStep = MathF.Max(worst.GetValueOrDefault(shown).FadeStep, Vector3.Distance(before, grip)) };
        lastGrip = grip;
        lastLook = turn;
        // Where each wrist sits on the look (look space), as the hold and the rig's palm put it.
        Quaternion holdTurn = HeldFrames.Turn(hold);
        Vector3 palm = new(0, 0.08f, -0.03f);
        Vector3 mainWrist = -Vector3.Transform(palm, holdTurn);
        // Look space into the box's own: its grip point at the look's grip, its length along -Z.
        Quaternion unfit = Quaternion.Inverse(fitTurn);
        Vector3 Model(Vector3 point) => Vector3.Transform(point, unfit) / scale + new Vector3(0, gripAt, 0);
        string now = $"{twoHanded}|{scale}";
        if (now == published) return;
        published = now;
        List<LandmarkPair> pairs = [LandmarkPair.Distance("main-wrist", arms.Hand(false).AtWrist(), Landmark.At(OverlayPlace.Model(LookId), Model(mainWrist)), 0.005f)];
        // The off hand's palm on the staff's off grip, as the grip check measures a fore-end.
        if (twoHanded) pairs.Add(LandmarkPair.Distance("off-palm", arms.Hand(true).Palm(), Landmark.At(OverlayPlace.Model(LookId), Model(OffGrip)), 0.04f));
        engine.Graphics.PublishOverlaySet(new(Pairs, Array.Empty<OverlayItem>(), pairs.ToArray()));
    }

    [DebugCommand("clips.inspect")]
    public string Inspect() => string.Create(CultureInfo.InvariantCulture,
        $"scene={scene}; look={lastGrip} heading {Vector3.Transform(-Vector3.UnitZ, lastLook)}; scrub={(scrub is { } s ? s.ToString("0.000", CultureInfo.InvariantCulture) : "running")}; bob={bob}; clip={arms.Clip}; " +
        $"worst (main wrist, off palm, fade step mm)=[{string.Join(", ", worst.Select(w => $"{w.Key} {w.Value.Main * 1000:0.0}/{w.Value.Off * 1000:0.0}/{w.Value.FadeStep * 1000:0.0}"))}]; " +
        $"wristClamp={arms.WristClamp.Main.Clamped:0.0}/{arms.WristClamp.Off.Clamped:0.0}; shortfallMm={arms.Shortfall.Main * 1000:0.0}/{arms.Shortfall.Off * 1000:0.0}; " +
        $"pairs=[{string.Join("; ", engine.Graphics.ReadLandmarks(Pairs).Pairs.ToArray().Select(p => $"{p.Name} {(p.Drawn ? (p.Distance * 1000).ToString("0.00", CultureInfo.InvariantCulture) + " mm" : "not drawn")}"))}]");

    [DebugCommand("clips.scene")]
    public string Scene(string name)
    {
        if (name != "cycle" && !Cycle.Contains(name)) return $"scenes: cycle, {string.Join(", ", Cycle)}";
        (scene, started, scrub) = (name, last, null);
        return Inspect();
    }

    /// <summary>Holds the scene at <paramref name="seconds"/> in (negative runs it again).</summary>
    [DebugCommand("clips.at")]
    public string At(float seconds)
    {
        scrub = seconds < 0 ? null : seconds;
        if (seconds < 0) started = last;
        return Inspect();
    }

    [DebugCommand("clips.bob")]
    public string Bob(bool enabled)
    {
        bob = enabled;
        return Inspect();
    }

    [DebugCommand("clips.reset")]
    public string Reset()
    {
        worst.Clear();
        lastGrip = null;
        return Inspect();
    }

    public void RegisterDebugCommands(IDebugCommandModuleRegistrar registrar) => registrar.Register(this);
    public void Start() { }
    public void Pause() { }
    public void Resume() { }
    public void Restart() { }
    public void Shutdown() { }

    public void Dispose()
    {
        engine.Graphics.PublishSnapshot(ReadOnlySpan<AppearanceFact>.Empty);
        arms.Dispose();
        look.Dispose();
        lookModel.Dispose();
        camera.Dispose();
    }
}
