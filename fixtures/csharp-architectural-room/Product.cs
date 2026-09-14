using System.Numerics;
using Rusty.Engine.Debugging;
using Rusty.Engine;
using Rusty.Engine.Implicit;
using Rusty.Engine.Input;

namespace CsharpArchitecturalRoom;

/// <summary>
/// An ordinary packaged C# product. The room layout and the door policy are
/// product-owned; extraction, retained resources, input delivery, the camera,
/// and browser rendering remain Engine services.
/// </summary>
public sealed class Product : IEngineProduct, IDebugCommandModuleSource, IDebugCommandModule
{
    private const float EyeHeight = 1.55f;
    private const float WalkSpeed = 3.2f;
    private const float AuditSampleSpacing = 0.20f;
    private const ulong AuditBudget = 500_000;
    private const ulong DoorObjectId = 8_100_001;
    private const ulong WindowObjectStartId = 8_100_010;

    private readonly IEngineContext _engine;
    private readonly FpsInput _input = new(FpsInputConfig.Standard);
    private readonly List<ArchitecturalSurface> _surfaces = [];
    private readonly List<Appearance> _windowPanes = [];
    private readonly List<Material> _materials = [];
    private readonly Materials _architectureMaterials;
    private readonly Camera _camera;
    private readonly Appearance _door;
    private Vector3 _position = new(0, -0.05f, 2.8f);
    private LookState _look;
    private bool _doorOpen;

    public Product(ProductCreateContext context)
    {
        _engine = context.Engine;
        _architectureMaterials = CreateMaterials();
        BuildArchitecture(_architectureMaterials);
        _door = _engine.Graphics.CreatePrimitive(new PrimitiveAppearanceRequest(
            PrimitiveGeometry.Cube, false, new Color(0.46f, 0.20f, 0.08f, 1)));
        CreateWindowPanes();
        _camera = _engine.CameraView.CreateCamera(CameraDescriptor());
        _engine.CameraView.SetActiveCamera(_camera);
        Publish();
    }

    public void Start() { }

    public ProductUpdateResult Update(ProductUpdate update)
    {
        float dt = (float)(update.Facts.FixedDeltaSeconds * update.Facts.AdmittedStepCount);
        FpsInputFrame frame = _input.Consume(update.Input, dt);
        _look = _input.IntegrateLook(_look, frame).After;
        Move(frame, dt);
        if (frame.UsePressed)
        {
            _doorOpen = !_doorOpen;
        }
        Publish();
        return ProductUpdateResult.None;
    }

    public void Pause() => _input.Physical.Clear();
    public void Resume() { }
    public void Restart() { }
    public void Shutdown() { }

    public void RegisterDebugCommands(IDebugCommandModuleRegistrar registrar) => registrar.Register(this);

    [DebugCommand("architecture.audit", Description = "Opt in to extract a disposable room audit and report declared joins/enclosure. Does not change the retained scene.")]
    public string AuditArchitecture()
    {
        WorldAudit report = RunAudit();
        string text = $"allComplete={report.AllComplete}; joins={report.ExpectedJoins}; joinSamples={report.JoinSamples}; joinDiagnostics={report.JoinDiagnostics}; enclosureSamples={report.EnclosureSamples}; enclosureDiagnostics={report.EnclosureDiagnostics}; classifications={report.EnclosureClassifications}";
        Console.WriteLine($"C# architectural room audit: {text}");
        return text;
    }

    public void Dispose()
    {
        _engine.Graphics.PublishSnapshot(ReadOnlySpan<AppearanceFact>.Empty);
        _engine.CameraView.ClearActiveCamera(new ClearActiveCameraRequest(0));
        _camera.Dispose();
        _door.Dispose();
        foreach (Appearance pane in _windowPanes) pane.Dispose();
        foreach (ArchitecturalSurface surface in _surfaces) surface.Dispose();
        foreach (Material material in _materials) material.Dispose();
    }

    private Materials CreateMaterials()
    {
        Material floor = Material(new Color(0.16f, 0.20f, 0.22f, 1), Vector3.Zero);
        Material walls = Material(new Color(0.49f, 0.43f, 0.34f, 1), Vector3.Zero);
        Material ceiling = Material(new Color(0.18f, 0.22f, 0.30f, 1), new Vector3(0.01f, 0.02f, 0.04f));
        return new Materials(floor, walls, ceiling);
    }

    private Material Material(Color color, Vector3 emission)
    {
        Material material = _engine.Graphics.CreateMaterial(new MaterialRequest(
            color, default, 0.62f, new Color(1, 1, 1, 1), emission, 1.0f, true));
        _materials.Add(material);
        return material;
    }

    private void BuildArchitecture(Materials materials)
    {
        RecipeWriter writer = CreateWriter(audit: null, pieces: null);
        EmitArchitecture(writer, materials);
    }

    private WorldAudit RunAudit()
    {
        var pieces = new Dictionary<string, ulong>(StringComparer.Ordinal);
        using ImplicitAudit audit = _engine.ImplicitSurfaces.CreateAudit();
        RecipeWriter writer = CreateWriter(audit, pieces);
        RecipeRoomContinuity main = EmitArchitecture(writer, _architectureMaterials);
        int joinDiagnostics = 0;
        int joinSamples = 0;
        bool joinsComplete = true;
        foreach (RecipeJoin join in main.Joins.Span)
        {
            ImplicitAnalysisReportLeaseReceipt report = _engine.ImplicitSurfaces.ReadExpectedJoin(
                join.Request(audit, name => pieces[name], .35f, .75f, AuditSampleSpacing, AuditBudget));
            joinDiagnostics += report.Diagnostics.Length;
            joinSamples += checked((int)report.Sampled);
            joinsComplete &= report.Complete != 0;
        }
        ImplicitAnalysisReportLeaseReceipt enclosure = _engine.ImplicitSurfaces.ReadEnclosure(
            main.Request(audit, AuditSampleSpacing, AuditBudget));
        return new WorldAudit(main.Joins.Length, joinSamples, joinDiagnostics, joinsComplete,
            enclosure.Sampled, enclosure.Diagnostics.Length, enclosure.Complete != 0,
            string.Join(",", enclosure.Diagnostics.ToArray().Select(d => d.Classification).Distinct()));
    }

    private RecipeWriter CreateWriter(ImplicitAudit? audit, Dictionary<string, ulong>? pieces)
    {
        ulong nextObjectId = 8_101_000;
        RecipeSampling sampling = new(
            CellSize: AuditSampleSpacing,
            CreaseDegrees: 55,
            TextureRepeats: 1,
            MaterialBoundaries: ImplicitMaterialBoundaryMode.Centroid,
            MaxExtractionVertices: 280_000,
            MaxExtractionTriangles: 560_000);
        return new RecipeWriter(_engine.ImplicitSurfaces, sampling, surface =>
        {
            MeshResource mesh = _engine.ImplicitSurfaces.Generate(new ImplicitGenerateRequest(
                surface.Field, surface.Root, surface.Min, surface.Max,
                surface.Sampling.CellSize, surface.Sampling.CreaseDegrees, surface.Sampling.TextureRepeats,
                surface.Sampling.TextureMapping, surface.Material, surface.Regions, surface.Sampling.MaterialBoundaries,
                surface.Sampling.MaterialSampleSpacing, surface.Sampling.MaxExtractionVertices,
                surface.Sampling.MaxExtractionTriangles));
            ulong objectId = nextObjectId++;
            if (audit is { } target)
            {
                pieces!.Add(surface.Name, objectId);
                float actualSpacing = _engine.ImplicitSurfaces.ReadGeneration(surface.Field).SampleSpacing;
                _engine.ImplicitSurfaces.CaptureAuditPiece(new ImplicitAuditPieceRequest(
                    target, objectId, surface.Field, surface.Root, mesh, surface.Placement, actualSpacing));
                mesh.Dispose();
                return;
            }
            Appearance appearance = _engine.Graphics.CreateMeshAppearance(mesh);
            _surfaces.Add(new ArchitecturalSurface(objectId, surface.Placement, mesh, appearance));
        });
    }

    private RecipeRoomContinuity EmitArchitecture(RecipeWriter writer, Materials materials)
    {
        RecipeOpening[] mainOpenings =
        [
            // A doorway into the passage and three deliberately declared windows.
            new("passage-door", new(new(-1.05f, -0.30f, -4.70f), new(1.05f, 2.70f, -3.55f))),
            new("west-window-a", new(new(-5.65f, 1.20f, -1.80f), new(-4.35f, 2.85f, -0.45f))),
            new("west-window-b", new(new(-5.65f, 1.20f, 0.60f), new(-4.35f, 2.85f, 1.95f))),
            new("east-window", new(new(4.35f, 1.20f, 1.20f), new(5.65f, 2.85f, 2.55f))),
        ];
        RecipeRoomContinuity main = RoomRecipes.Shell(writer, "room",
            new RoomShellOptions(new ImplicitBounds(new(-5, -0.65f, -4), new(5, 3.85f, 4.5f)), .35f, mainOpenings,
                FloorPlatforms: new[]
                {
                    new ImplicitBounds(new(-.80f, -.65f, -3.70f), new(.80f, -.28f, 3.85f)),
                },
                CeilingSoffits: new[]
                {
                    new ImplicitBounds(new(-4.65f, 3.15f, -3.35f), new(-1.35f, 3.85f, 3.75f)),
                    new ImplicitBounds(new(1.35f, 3.35f, -3.35f), new(4.65f, 3.85f, 3.75f)),
                }),
            new RoomMaterials(materials.Floor, materials.Walls, materials.Ceiling));

        RecipeOpening[] passageOpenings =
        [
            new("room-door", new(new(-1.05f, -0.30f, -4.70f), new(1.05f, 2.70f, -3.55f))),
            new("passage-window", new(new(-1.55f, 1.05f, -8.80f), new(-0.85f, 2.20f, -7.30f))),
        ];
        RoomRecipes.Shell(writer, "passage",
            new RoomShellOptions(new ImplicitBounds(new(-1.20f, -0.65f, -9.5f), new(1.20f, 2.75f, -4.63f)), .28f, passageOpenings,
                FloorPlatforms: new[]
                {
                    new ImplicitBounds(new(-.78f, -.65f, -9.10f), new(.78f, -.28f, -4.72f)),
                }),
            new RoomMaterials(materials.Floor, materials.Walls, materials.Ceiling));
        return main;
    }

    private void CreateWindowPanes()
    {
        Color glass = new(0.16f, 0.78f, 0.92f, 1);
        for (int index = 0; index < 3; index++)
        {
            _windowPanes.Add(_engine.Graphics.CreatePrimitive(new PrimitiveAppearanceRequest(
                PrimitiveGeometry.Cube, false, glass)));
        }
    }

    private void Move(FpsInputFrame frame, float dt)
    {
        Vector3 forward = new(MathF.Sin(_look.YawRadians), 0, -MathF.Cos(_look.YawRadians));
        Vector3 right = new(-forward.Z, 0, forward.X);
        float vertical = (frame.JumpHeld ? 1 : 0) - (frame.CrouchHeld ? 1 : 0);
        _position += (right * frame.Movement.X + forward * frame.Movement.Y + Vector3.UnitY * vertical) * (WalkSpeed * dt);
    }

    private CameraDescriptor CameraDescriptor() => new(
        new CameraPose(_position + Vector3.UnitY * EyeHeight,
            float.RadiansToDegrees(_look.PitchRadians), float.RadiansToDegrees(_look.YawRadians)),
        CameraBasisMode.Derived, default,
        new CameraProjection(CameraProjectionKind.Perspective, 70, 0, .05f, 80), CameraViewports.Full);

    private void Publish()
    {
        List<AppearanceFact> facts = _surfaces.Select(surface => new AppearanceFact(
            surface.ObjectId, false, 0, surface.Placement, surface.Appearance, true, RenderLayer.Scene)).ToList();
        facts.Add(new AppearanceFact(DoorObjectId, false, 0,
            new Transform(new Vector3(0, _doorOpen ? 4.4f : 1.15f, -4.18f), Quaternion.Identity, new(1.72f, 2.30f, .16f)),
            _door, true, RenderLayer.Scene));
        for (int index = 0; index < _windowPanes.Count; index++)
        {
            Vector3 position = index switch
            {
                0 => new(-5.38f, 2.03f, -1.12f),
                1 => new(-5.38f, 2.03f, 1.27f),
                _ => new(5.38f, 2.03f, 1.88f),
            };
            facts.Add(new AppearanceFact(WindowObjectStartId + (ulong)index, false, 0,
                new Transform(position, Quaternion.Identity, new(.08f, 1.52f, 1.22f)),
                _windowPanes[index], true, RenderLayer.Scene));
        }
        _engine.Graphics.PublishSnapshot(facts.ToArray());
        _engine.CameraView.UpdateCamera(new CameraUpdateRequest(_camera, CameraDescriptor()));
    }

    private readonly record struct Materials(Material Floor, Material Walls, Material Ceiling);

    private sealed record ArchitecturalSurface(ulong ObjectId, Transform Placement, MeshResource Mesh, Appearance Appearance) : IDisposable
    {
        public void Dispose()
        {
            Appearance.Dispose();
            Mesh.Dispose();
        }
    }

    private readonly record struct WorldAudit(
        int ExpectedJoins, int JoinSamples, int JoinDiagnostics, bool JoinsComplete,
        ulong EnclosureSamples, int EnclosureDiagnostics, bool EnclosureComplete, string EnclosureClassifications)
    {
        public bool AllComplete => JoinsComplete && EnclosureComplete;
    }
}
