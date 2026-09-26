using System.Numerics;
using Rusty.Engine;
using Rusty.Engine.Debugging;

namespace CsharpJointAttachments;

public sealed class Product : IEngineProduct, IDebugCommandModuleSource, IDebugCommandModule
{
    private const ulong BodyId = 8654, WeaponId = 8655;
    private readonly IEngineContext engine;
    private readonly Camera camera;
    private readonly PortableAssetContent descriptor;
    private readonly PortableMeshAttachment attachment;
    private RenderResource? bodyResource;
    private RenderResource? weaponResource;
    private Appearance? body;
    private Appearance? weapon;
    private AnimationInstance? instance;
    private float pose;
    private int reloads;
    private string missing = "not exercised";
    public Product(ProductCreateContext context)
    {
        engine = context.Engine;
        using var source = engine.Content.OpenReference(new("asset.json"));
        descriptor = new(engine.Content, source, "grip");
        attachment = descriptor.Facts.Attachments.Span[0];
        CameraQueries.TryLookAtPose(new(4, 3, 6), new(0, 1.5f, 0), 0, out CameraPose cameraPose);
        camera = engine.CameraView.CreateCamera(new(cameraPose, CameraBasisMode.Derived, default,
            new(CameraProjectionKind.Perspective, 50, 0, .05f, 100), CameraViewports.Full));
        engine.CameraView.SetActiveCamera(camera);
        Load();
    }
    private void Load()
    {
        using var bodySource = descriptor.OpenMember(attachment.TargetId);
        using var weaponSource = descriptor.OpenMember(attachment.ChildId);
        bodyResource = engine.Animation.OpenAnimatedMeshFromContent(new(bodySource));
        body = engine.Animation.CreateAnimatedMeshAppearance(new(bodyResource));
        weaponResource = engine.Animation.OpenAnimatedMeshFromContent(new(weaponSource));
        weapon = engine.Animation.CreateAnimatedMeshAppearance(new(weaponResource));
        Publish(attachment.Joint);
        instance = engine.Animation.CreateInstance(new(body, BodyId));
        Pose(0);
    }
    private AppearanceFact[] Facts() => [
        new(BodyId, false, 0, new(Vector3.Zero, Quaternion.Identity, Vector3.One), body!, true, RenderLayer.Scene),
        new(WeaponId, true, BodyId, attachment.Transform, weapon!, true, RenderLayer.Scene),
    ];
    private void Publish(string joint) => engine.Graphics.PublishAttachedSnapshot(new(Facts(), new MeshJointAttachment[] { new(WeaponId, joint) }));
    [DebugCommand("attachment.pose")]
    public string Pose(float normalizedTime)
    {
        pose = normalizedTime;
        engine.Animation.SetPlayback(new(instance!, AnimationPlaybackKind.Sample, "run", AnimationLoopMode.Repeat, 1, 1, true, 0, false, pose));
        Publish(attachment.Joint);
        return Inspect();
    }
    [DebugCommand("attachment.missing")]
    public string Missing()
    {
        try { Publish("MissingHand8647"); throw new InvalidOperationException("Missing joint accepted"); }
        catch (EngineCallException error) {
            missing = string.Join(";", error.Diagnostics.ToArray().Select(d => d.Message));
            if (!missing.Contains("MissingHand8647")) throw;
        }
        return Inspect();
    }
    [DebugCommand("attachment.reload")]
    public string Reload() { Unload(); Load(); reloads++; return Inspect(); }
    [DebugCommand("attachment.inspect")]
    public string Inspect() => $"joint={attachment.Joint}; pose={pose}; reloads={reloads}; missing={missing}";
    private void Unload()
    {
        instance?.Dispose(); instance = null;
        engine.Graphics.PublishSnapshot(ReadOnlySpan<AppearanceFact>.Empty);
        weapon?.Dispose(); weapon = null; body?.Dispose(); body = null; bodyResource?.Dispose(); bodyResource = null; weaponResource?.Dispose(); weaponResource = null;
    }
    public void RegisterDebugCommands(IDebugCommandModuleRegistrar registrar) => registrar.Register(this);
    public void Start() { }
    public ProductUpdateResult Update(ProductUpdate update) => ProductUpdateResult.None;
    public void Pause() { }
    public void Resume() { }
    public void Restart() { }
    public void Shutdown() { }
    public void Dispose() { Unload(); descriptor.Dispose(); camera.Dispose(); }
}
