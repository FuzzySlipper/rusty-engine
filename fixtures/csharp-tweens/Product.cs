using System.Numerics;
using System.Text.Json;
using Rusty.Engine;
using Rusty.Engine.Debugging;

namespace CsharpTweens;

/// <summary>
/// Engine-played tweens on a small board. The product moves a piece one cell
/// at a time and publishes it once per move; the Engine plays the hop, the
/// landing squash, the other piece's breath and the marker's flash. The
/// product sequences the next move from the hop's completion.
/// </summary>
public sealed class Product : IEngineProduct, IDebugCommandModuleSource, IDebugCommandModule
{
    // Board.
    private const float CellSpacing = 1.5f;
    private static readonly Vector3[] Cells =
    [
        new(-CellSpacing, 0, -CellSpacing),
        new(CellSpacing, 0, -CellSpacing),
        new(CellSpacing, 0, CellSpacing),
        new(-CellSpacing, 0, CellSpacing),
    ];
    private static readonly Vector3 BreatherPosition = Vector3.Zero;
    private static readonly Vector3 BeaconPosition = new(0, 0.25f, -3.2f);
    private static readonly Vector2 PieceSize = new(0.9f, 0.9f);
    // Motion.
    private const float HopSeconds = 0.45f;
    private const float HopHeight = 0.8f;
    private const float LandingSquash = 0.25f;
    private const double RestSeconds = 0.35;
    private const float BreathAmount = 0.06f;
    private const float BreathPeriodSeconds = 1.6f;
    private const float PunchAmount = 0.3f;
    private const float PunchSeconds = 0.35f;
    private const float FlashSeconds = 0.4f;
    private static readonly Vector4 FlashColor = new(2.5f, 2.5f, 1.2f, 1);
    private const ulong ApexMarkerId = 1;
    // Object ids.
    private const ulong HopperId = 1, BreatherId = 2, BeaconId = 3, FloorId = 4;

    private readonly IEngineContext engine;
    private readonly SpriteAtlas atlas;
    private readonly Appearance hopperLook, breatherLook, beaconLook, floorLook;
    private readonly Camera camera;
    private TweenHandle hop;
    private TweenHandle breath;
    private int cell;
    private double restRemaining = RestSeconds;
    private bool hopping;
    private int hops, hopsCompleted, apexes, skipped, retargeted, publishes, flashes;

    public Product(ProductCreateContext context)
    {
        engine = context.Engine;
        RenderResourceInfo texture = engine.Graphics.OpenResource(new RenderResourceRequest("piece.png"));
        atlas = engine.Graphics.CreateSpriteAtlas(new SpriteAtlasCreateRequest(
            texture.Handle,
            new SpriteAtlasFrame[] { new(0, Vector2.Zero, Vector2.One, false, Vector2.Zero) }));
        hopperLook = Piece(new Color(1f, .55f, .2f, 1));
        breatherLook = Piece(new Color(.3f, .85f, .8f, 1));
        beaconLook = engine.Graphics.CreatePrimitive(new(PrimitiveGeometry.Cube, false, new Color(.35f, .4f, .9f, 1)));
        floorLook = engine.Graphics.CreatePrimitive(new(PrimitiveGeometry.Cube, false, new Color(.2f, .22f, .25f, 1)));
        engine.Graphics.PublishSnapshot(
        [
            Fact(FloorId, floorLook, new Vector3(0, -0.05f, 0), new Vector3(6, 0.1f, 6)),
            Fact(HopperId, hopperLook, Cells[cell], Vector3.One),
            Fact(BreatherId, breatherLook, BreatherPosition, Vector3.One),
            Fact(BeaconId, beaconLook, BeaconPosition, new Vector3(0.5f)),
        ]);
        publishes++;
        camera = engine.CameraView.CreateCamera(new CameraDescriptor(
            new CameraPose(new Vector3(0, 6.5f, 7.5f), -40, 0),
            CameraBasisMode.Derived,
            default,
            new CameraProjection(CameraProjectionKind.Perspective, 50, 0, .05, 100),
            CameraViewports.Full,
            0));
        engine.CameraView.SetActiveCamera(camera);
        breath = engine.Tween.Start(Tweens.Breathe(BreatherId, BreathAmount, BreathPeriodSeconds)).Tween;
    }

    public void Start() { }

    public ProductUpdateResult Update(ProductUpdate update)
    {
        foreach (TweenEvent tweenEvent in engine.Tween.ReadEvents().Span)
        {
            if (tweenEvent.Tween != hop) continue;
            if (tweenEvent.Kind == TweenEventKind.Marker && tweenEvent.MarkerId == ApexMarkerId)
            {
                apexes++;
            }
            else if (tweenEvent.Kind == TweenEventKind.Completed)
            {
                // Sequenced from the hop's completion: the beacon answers.
                hopsCompleted++;
                hopping = false;
                restRemaining = RestSeconds;
                engine.Tween.Start(Tweens.PunchScale(BeaconId, PunchAmount, PunchSeconds));
                engine.Tween.Start(Tweens.Flash(BeaconId, FlashColor, FlashSeconds));
                flashes++;
            }
        }
        if (!hopping)
        {
            restRemaining -= update.Facts.FixedDeltaSeconds * update.Facts.AdmittedStepCount;
            if (restRemaining <= 0) Hop();
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
        hopperLook.Dispose();
        breatherLook.Dispose();
        beaconLook.Dispose();
        floorLook.Dispose();
        atlas.Dispose();
    }

    /// <summary>Moves the piece to the next cell now and lets the Engine show the hop.</summary>
    private void Hop()
    {
        Vector3 previous = Cells[cell];
        cell = (cell + 1) % Cells.Length;
        engine.Graphics.PublishChanges(new(
            new[] { Fact(HopperId, hopperLook, Cells[cell], Vector3.One) },
            ReadOnlyMemory<ulong>.Empty,
            ReadOnlyMemory<MeshJointAttachment>.Empty));
        publishes++;
        TweenStartRequest request = Tweens.HopFrom(HopperId, previous - Cells[cell], HopHeight, HopSeconds, LandingSquash) with
        {
            Markers = new[] { new TweenMarker(ApexMarkerId, HopSeconds / 2) },
        };
        hop = engine.Tween.Start(request).Tween;
        hops++;
        hopping = true;
    }

    private Appearance Piece(Color tint) => engine.Graphics.CreateSpriteFromAtlas(new SpriteFromAtlasRequest(
        atlas, 0, new Vector2(0.5f, 0), PieceSize, BillboardMode.Cylindrical, SpriteSizeMode.World, 0, SpriteDepthPolicy.Default, tint));

    private static AppearanceFact Fact(ulong id, Appearance appearance, Vector3 position, Vector3 scale) =>
        new(id, false, 0, new Transform(position, Quaternion.Identity, scale), appearance, true, RenderLayer.Scene);

    public void RegisterDebugCommands(IDebugCommandModuleRegistrar registrar) => registrar.Register(this);

    [DebugCommand("tweens.observe", Description = "Read-only: the piece's cell, hop and publish counts, apex markers, and the hop and breath readouts.")]
    public string Observe()
    {
        TweenReadout hopReadout = hop.Value == 0 ? default : engine.Tween.Read(hop);
        TweenReadout breathReadout = engine.Tween.Read(breath);
        return JsonSerializer.Serialize(new
        {
            cell,
            hops,
            hopsCompleted,
            apexes,
            skipped,
            retargeted,
            publishes,
            flashes,
            hop = new { state = hopReadout.State.ToString(), hopReadout.ElapsedSeconds, hopReadout.TotalSeconds },
            breath = new { state = breathReadout.State.ToString(), breathReadout.ElapsedSeconds, breathReadout.Iteration },
        });
    }

    [DebugCommand("tweens.skip", Description = "Skips the running hop to its end, as an input skip would; its completion arrives next update.")]
    public string Skip()
    {
        if (!hopping) return "no hop running";
        engine.Tween.Control(new TweenControlRequest(hop, TweenControl.Complete));
        skipped++;
        return "skipped";
    }

    [DebugCommand("tweens.retarget", Description = "Moves the piece on to the next cell now, mid-hop: the new hop starts from where the piece shows.")]
    public string Retarget()
    {
        Hop();
        retargeted++;
        return $"hopping to cell {cell}";
    }
}
