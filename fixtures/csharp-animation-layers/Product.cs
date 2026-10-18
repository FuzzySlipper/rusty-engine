using System.Globalization;
using System.Numerics;
using Rusty.Engine;
using Rusty.Engine.Debugging;

namespace CsharpAnimationLayers;

/// <summary>
/// Clip layers over the joint-attachment fixture's character. It plays its
/// run, the right arm waves over it at half weight (an override masked to the
/// arm), and the head nods over both (an additive layer). The wave and nod
/// come from a clip pack that carries only the skeleton (`body.clips.glb`),
/// associated with the body, so `ReadClips` lists them after the body's own.
/// Every update the layers are set at the product's own times, and the pose
/// the update will draw is sampled to check where the hand is.
/// </summary>
public sealed class Product : IEngineProduct, IDebugCommandModuleSource, IDebugCommandModule
{
    private const ulong BodyId = 10073;
    private const string Base = "run";
    private const string Wave = "wave";
    private const string Nod = "nod";
    private const float WaveWeight = 0.5f;
    private const float WaveSecondsPerCycle = 2f;
    private const float NodSecondsPerCycle = 0.8f;
    private static readonly string[] Arm = ["RightArm", "RightForeArm"];

    private readonly IEngineContext engine;
    private readonly Camera camera;
    private readonly RenderResource body;
    private readonly RenderResource pack;
    private readonly Appearance appearance;
    private readonly AnimationInstance instance;
    private readonly AnimationClipInfo[] clips;
    private readonly uint wave, nod, hand, leftHand;
    private readonly AnimationJointWeight[] arm;
    private readonly AnimationLayerSet layers = new();
    private float waveWeight = WaveWeight;
    private bool nodding = true;
    private Vector3 handFromRun, leftHandFromRun;

    public Product(ProductCreateContext context)
    {
        engine = context.Engine;
        CameraQueries.TryLookAtPose(new(3.2f, 2.2f, 4.5f), new(0, 1.3f, 0), 0, out CameraPose cameraPose);
        camera = engine.CameraView.CreateCamera(new(cameraPose, CameraBasisMode.Derived, default,
            new(CameraProjectionKind.Perspective, 50, 0, .05f, 100), CameraViewports.Full));
        engine.CameraView.SetActiveCamera(camera);
        using var source = engine.Content.OpenReference(new("body.glb"));
        body = engine.Animation.OpenAnimatedMeshFromContent(new(source));
        using var packSource = engine.Content.OpenReference(new("body.clips.glb"));
        pack = engine.Animation.OpenAnimationClipPackFromContent(new(packSource));
        engine.Animation.AssociateAnimationClipPack(new(body, pack, "rusty-engine fixture", "CC0-1.0"));
        clips = engine.Animation.ReadClips(body).ToArray();
        wave = AnimationClips.IndexOf(clips, Wave);
        nod = AnimationClips.IndexOf(clips, Nod);
        ReadOnlyMemory<AnimationJointInfo> rig = engine.Animation.ReadJoints(body);
        arm = [.. Arm.Select(id => new AnimationJointWeight(AnimationJoints.IndexOf(rig.Span, id), 1))];
        hand = AnimationJoints.IndexOf(rig.Span, "RightHand");
        leftHand = AnimationJoints.IndexOf(rig.Span, "LeftHand");
        appearance = engine.Animation.CreateAnimatedMeshAppearance(new(body));
        engine.Graphics.PublishSnapshot([
            new(BodyId, false, 0, new(Vector3.Zero, Quaternion.Identity, Vector3.One), appearance, true, RenderLayer.Scene),
        ]);
        instance = engine.Animation.CreateInstance(new(appearance, BodyId));
        engine.Animation.SetPlayback(new(instance, AnimationPlaybackKind.Play, Base, AnimationLoopMode.Repeat, 1, 1, true, 0, false, 0));
    }

    public ProductUpdateResult Update(ProductUpdate update)
    {
        double seconds = update.Facts.SimulationStep * update.Facts.FixedDeltaSeconds;
        layers.Clear();
        layers.Override(wave, Cycle(seconds, WaveSecondsPerCycle), waveWeight, arm);
        if (nodding)
        {
            layers.Additive(nod, Cycle(seconds, NodSecondsPerCycle), 1);
        }
        engine.Animation.SetLayers(layers.ToRequest(instance));
        // The pose this update draws, layers and all, against the run alone.
        Transform placement = new(Vector3.Zero, Quaternion.Identity, Vector3.One);
        ReadOnlySpan<JointPose> layered = engine.Animation.SampleJointPose(
            AnimationPoseSampleRequest.OfCurrentPlayback(instance, placement, applyPoseControls: false)).Span;
        handFromRun = layered[(int)hand].World.Translation;
        leftHandFromRun = layered[(int)leftHand].World.Translation;
        return ProductUpdateResult.None;
    }

    private static float Cycle(double seconds, float period) => (float)(seconds / period % 1);

    [DebugCommand("layers.inspect")]
    public string Inspect() => string.Create(CultureInfo.InvariantCulture,
        $"clips=[{string.Join(", ", clips.Select(clip => $"{clip.Id}:{clip.ClipPack}"))}]; wave={wave}; nod={nod}; waveWeight={waveWeight:F2}; nodding={nodding}; rightHand={handFromRun}; leftHand={leftHandFromRun}");

    [DebugCommand("layers.wave")]
    public string SetWave(float weight)
    {
        waveWeight = Math.Clamp(weight, 0, 1);
        return Inspect();
    }

    [DebugCommand("layers.nod")]
    public string SetNod(bool enabled)
    {
        nodding = enabled;
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
        instance.Dispose();
        engine.Graphics.PublishSnapshot(ReadOnlySpan<AppearanceFact>.Empty);
        appearance.Dispose();
        body.Dispose();
        pack.Dispose();
        camera.Dispose();
    }
}
