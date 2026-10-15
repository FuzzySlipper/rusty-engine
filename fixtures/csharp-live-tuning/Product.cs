using System.Numerics;
using System.Text.Json;
using System.Text.Json.Nodes;
using Rusty.Engine;
using Rusty.Engine.Debugging;

namespace CsharpLiveTuning;

/// <summary>
/// A marker cube on a floor, shaped by one live content file. Under
/// <c>rusty dev</c> an edit to <c>tuning/marker.json</c> reaches the running
/// product as a content change: it reads the file again and applies it, with
/// no runtime replacement.
/// </summary>
public sealed class Product : IEngineProduct, IDebugCommandModuleSource, IDebugCommandModule
{
    private const string TuningPath = "tuning/marker.json";
    private const float SpinDegreesPerSecond = 45;
    private const float FloorSize = 6;
    private const float FloorThickness = 0.1f;
    private const ulong MarkerId = 1, FloorId = 2;
    private static readonly string[] Colors = ["orange", "teal", "violet"];
    private static readonly Color[] ColorValues =
    [
        new(1f, .55f, .2f, 1),
        new(.3f, .85f, .8f, 1),
        new(.6f, .4f, .95f, 1),
    ];

    private readonly IEngineContext engine;
    private readonly ProductContent content;
    private readonly Camera camera;
    /// <summary>Names this product instance, so a reader can tell a reload from a replacement.</summary>
    private readonly string instance = Guid.NewGuid().ToString("N");
    private Tuning tuning;
    private Appearance markerLook, floorLook;
    private double spinDegrees;
    private int reloads;
    private string? lastChange;

    public Product(ProductCreateContext context)
    {
        engine = context.Engine;
        content = context.Content;
        tuning = Tuning.Read(content.ReadText(TuningPath));
        camera = engine.CameraView.CreateCamera(new CameraDescriptor(
            new CameraPose(new Vector3(0, 3.5f, 5.5f), -28, 0),
            CameraBasisMode.Derived,
            default,
            new CameraProjection(CameraProjectionKind.Perspective, 50, 0, .05, 100),
            CameraViewports.Full,
            0));
        engine.CameraView.SetActiveCamera(camera);
        (markerLook, floorLook) = Looks();
        Publish();
    }

    /// <summary>The owner of the tuning: replaces the values the product applies.</summary>
    internal Tuning Values
    {
        get => tuning;
        set
        {
            tuning = value;
            (Appearance marker, Appearance floor) = (markerLook, floorLook);
            (markerLook, floorLook) = Looks();
            Publish();
            marker.Dispose();
            floor.Dispose();
        }
    }

    public void Start() { }

    public ProductUpdateResult Update(ProductUpdate update)
    {
        foreach (ContentReferenceInfo change in update.ContentChanges)
        {
            if (change.Path != TuningPath) continue;
            Values = Tuning.Read(content.ReadText(TuningPath));
            reloads++;
            lastChange = $"{change.Path} {change.ByteLength} bytes";
        }
        if (tuning.Spin && update.Facts.AdmittedStepCount > 0)
        {
            spinDegrees += SpinDegreesPerSecond * update.Facts.FixedDeltaSeconds * update.Facts.AdmittedStepCount;
            Publish();
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
        markerLook.Dispose();
        floorLook.Dispose();
    }

    private (Appearance Marker, Appearance Floor) Looks()
    {
        int color = Math.Max(0, Array.IndexOf(Colors, tuning.Color));
        float shade = tuning.FloorShade;
        return (
            engine.Graphics.CreatePrimitive(new(PrimitiveGeometry.Cube, false, ColorValues[color])),
            engine.Graphics.CreatePrimitive(new(PrimitiveGeometry.Cube, false, new Color(shade, shade, shade, 1))));
    }

    private void Publish()
    {
        Quaternion spin = Quaternion.CreateFromAxisAngle(Vector3.UnitY, (float)(spinDegrees * Math.PI / 180));
        engine.Graphics.PublishSnapshot(
        [
            new(FloorId, false, 0, new Transform(new Vector3(0, -FloorThickness / 2, 0), Quaternion.Identity, new Vector3(FloorSize, FloorThickness, FloorSize)), floorLook, true, RenderLayer.Scene),
            new(MarkerId, false, 0, new Transform(new Vector3(0, tuning.Height + tuning.Size / 2, 0), spin, new Vector3(tuning.Size)), markerLook, true, RenderLayer.Scene),
        ]);
    }

    public void RegisterDebugCommands(IDebugCommandModuleRegistrar registrar)
    {
        registrar.Register(this);
        registrar.Register(new TunablesDebugModule(engine.ContentSource, Tunables));
    }

    /// <summary>The page the Engine tunables panel shows: each control applies to the live
    /// product and names where Save writes it.</summary>
    private TunablesPage Tunables()
    {
        static TunableSource Source(string pointer) => new(TuningPath, pointer);
        return new TunablesPage("Live tuning",
        [
            new TunableGroup("marker", "Marker",
            [
                new(new TunableControl("marker.size", "Size", TunableKind.Slider, 0.2f, 2f, 0.05f, tuning.Size) { Source = Source("/marker/size") },
                    value => Values = tuning with { Size = value }),
                new(new TunableControl("marker.height", "Height", TunableKind.Slider, 0f, 2f, 0.05f, tuning.Height) { Source = Source("/marker/height") },
                    value => Values = tuning with { Height = value }),
                new(new TunableControl("marker.spin", "Spin", TunableKind.Toggle, 0, 1, 1, tuning.Spin ? 1 : 0) { Source = Source("/marker/spin") },
                    value => Values = tuning with { Spin = value >= 0.5f }),
                new(new TunableControl("marker.color", "Colour", TunableKind.Choice, 0, Colors.Length - 1, 1, Math.Max(0, Array.IndexOf(Colors, tuning.Color)))
                    { Options = Colors, Source = Source("/marker/color") },
                    value => Values = tuning with { Color = Colors[(int)MathF.Round(value)] }),
            ]) { File = TuningPath },
            new TunableGroup("floor", "Floor",
            [
                new(new TunableControl("floor.shade", "Shade", TunableKind.Slider, 0f, 1f, 0.05f, tuning.FloorShade) { Source = Source("/floor/shade") },
                    value => Values = tuning with { FloorShade = value }),
            ]) { File = TuningPath },
        ]);
    }

    [DebugCommand("livetuning.observe", Description = "Read-only: the applied tuning, the reloads the product took, and its instance name.")]
    public string Observe() => JsonSerializer.Serialize(new
    {
        instance,
        tuning.Size,
        tuning.Height,
        tuning.Spin,
        tuning.Color,
        tuning.FloorShade,
        reloads,
        lastChange,
    });

    [DebugCommand("livetuning.source.size", Description = "Saves a marker size into the source tuning file through the ContentSource service, keeping every other byte, and applies it live.")]
    public string SaveSize(float size)
    {
        ContentSourceDocument read = engine.ContentSource.ReadDocument(TuningPath);
        ContentSha256 saved = engine.ContentSource.Edit(read, ContentSourceEdit.Replace("/marker/size", JsonValue.Create(size)));
        Values = tuning with { Size = size };
        return JsonSerializer.Serialize(new { saved = $"{saved.Word0:x16}{saved.Word1:x16}{saved.Word2:x16}{saved.Word3:x16}", size });
    }
}

/// <summary>The values <c>tuning/marker.json</c> holds.</summary>
internal readonly record struct Tuning(float Size, float Height, bool Spin, string Color, float FloorShade)
{
    public static Tuning Read(string json)
    {
        using JsonDocument document = JsonDocument.Parse(json);
        JsonElement marker = document.RootElement.GetProperty("marker");
        JsonElement floor = document.RootElement.GetProperty("floor");
        return new(
            marker.GetProperty("size").GetSingle(),
            marker.GetProperty("height").GetSingle(),
            marker.GetProperty("spin").GetBoolean(),
            marker.GetProperty("color").GetString() ?? "orange",
            floor.GetProperty("shade").GetSingle());
    }
}
