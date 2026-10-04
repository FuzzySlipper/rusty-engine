using System.Numerics;
using Rusty.Engine;
using Rusty.Engine.Debugging;

namespace CsharpViewportAnchor;

/// <summary>
/// One camera whose view follows the UI's hero panel. The product never
/// measures the page: the shell reports the panel's rect and the Engine draws
/// the view there. Update only watches the presented surface and the hero's
/// rect change.
/// </summary>
public sealed class Product(ProductCreateContext context) : IEngineProduct, IDebugCommandModuleSource, IDebugCommandModule
{
    private const string HeroAnchor = "hero";
    private static readonly Color Sky = new(0.9f, 0.1f, 0.6f, 1);
    private static readonly CameraViewport Fallback = new(0, 0, 0.25, 0.25);
    private Camera? camera;
    private Appearance? cube;
    private CameraSurfaceReadout surface;
    private int surfaceChanges;
    private CameraViewportAnchorReadout hero;
    private int heroChanges;
    private bool watching = true;
    private int watchingChanges;
    private int unwatchedUpdates;

    public void Start()
    {
        camera = context.Engine.CameraView.CreateCamera(new CameraDescriptor(
            new CameraPose(new Vector3(0, 0, 4), 0, 0), CameraBasisMode.Derived, default,
            new CameraProjection(CameraProjectionKind.Perspective, 60, 0, 0.1f, 100), Fallback));
        context.Engine.CameraView.SetActiveCamera(camera);
        context.Engine.CameraView.SetBackgroundColor(new(Sky));
        context.Engine.CameraView.SetViewportAnchor(new(camera, HeroAnchor));
        cube = context.Engine.Graphics.CreatePrimitive(new PrimitiveAppearanceRequest(PrimitiveGeometry.Cube, false, new Color(0.2f, 0.8f, 0.3f, 1)));
        context.Engine.Graphics.PublishSnapshot([new AppearanceFact(1, false, 0,
            new Transform(Vector3.Zero, Quaternion.Identity, Vector3.One), cube, true, RenderLayer.Scene)]);
    }

    [DebugCommand("viewport.proof.surface", Description = "Read the presented surface and how often Update saw it change.")]
    public string Surface() => $"{surface};changes={surfaceChanges}";

    [DebugCommand("viewport.proof.watching", Description = "Whether a page watches, how often that changed, and how many updates ran unwatched.")]
    public string Watching() => $"watching={watching};changes={watchingChanges};unwatchedUpdates={unwatchedUpdates}";

    [DebugCommand("viewport.proof.hero", Description = "Read the hero panel's rect and how often Update saw it change.")]
    public string Hero() => $"{hero};changes={heroChanges}";

    [DebugCommand("viewport.proof.anchor", Description = "Anchor the camera to the hero panel (true) or draw at its own quarter viewport (false).")]
    public string Anchor(bool anchored)
    {
        context.Engine.CameraView.SetViewportAnchor(new(camera!, anchored ? HeroAnchor : string.Empty));
        return anchored ? "anchored to hero" : "own viewport";
    }

    public void RegisterDebugCommands(IDebugCommandModuleRegistrar registrar) => registrar.Register(this);

    public ProductUpdateResult Update(ProductUpdate update)
    {
        CameraSurfaceReadout current = context.Engine.CameraView.ReadSurface();
        if (current.Watching != watching)
        {
            watching = current.Watching;
            watchingChanges++;
        }
        if (!watching)
        {
            unwatchedUpdates++;
        }
        if (current.Revision != surface.Revision)
        {
            surface = current;
            surfaceChanges++;
        }
        CameraViewportAnchorReadout anchor = context.Engine.CameraView.ReadViewportAnchor(new(HeroAnchor));
        if (anchor != hero)
        {
            hero = anchor;
            heroChanges++;
        }
        return ProductUpdateResult.None;
    }

    public void Pause() { }
    public void Resume() { }
    public void Restart() { }
    public void Shutdown() { }
    public void Dispose()
    {
        context.Engine.Graphics.PublishSnapshot(ReadOnlySpan<AppearanceFact>.Empty);
        cube?.Dispose();
        context.Engine.CameraView.ClearActiveCamera(new ClearActiveCameraRequest(0));
        camera?.Dispose();
    }
}
