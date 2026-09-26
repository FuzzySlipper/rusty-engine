using System.Numerics;
using Rusty.Engine;
using Rusty.Engine.Debugging;

namespace CsharpVoxelAtlases;

public sealed class Product : IEngineProduct, IDebugCommandModuleSource, IDebugCommandModule
{
    private const uint Version = 1, ChunkSize = 8, FirstSlot = 1, SecondSlot = 2;
    private const uint TextureWidth = 4, TextureHeight = 2, TileSize = 2;
    private const int VariantCount = 3;
    private const float VoxelSize = 1, FieldOfView = 60, NearClip = .05f, FarClip = 100;
    private static readonly Vector3 Eye = new(4, 3, 6), Target = new(1.5f, .5f, .5f);
    private static readonly VoxelAddress FirstAddress = new(0, 0, 0), SecondAddress = new(2, 0, 0);
    private const string Texture = "texture/tiles", FirstAtlas = "sprite-sheet/first", SecondAtlas = "sprite-sheet/second";
    private const string FirstMaterial = "material/first", SecondMaterial = "material/second";
    private const string Hash = "2df27291bae363a909fbc3a4fb2cd7d7d3a4196ff346bbb1a46db22fd78ebd15";
    private readonly IEngineContext engine;
    private readonly SpatialSession scene;
    private readonly AuthoredCatalog catalog;
    private readonly RenderResource texture;
    private readonly Material first;
    private readonly Material[] seconds;
    private int mode;
    private int caughtRejections;
    private readonly Camera camera;
    private VoxelScenePresentation? presentation;
    private string proof = "not started";

    public Product(ProductCreateContext context)
    {
        engine = context.Engine;
        scene = engine.Spatial.CreateSession(new(VoxelSize, ChunkSize, VoxelSurfaceMode.GreedyCubes));
        texture = engine.Graphics.OpenResource(new("tiles.png")).Handle;
        catalog = engine.AuthoredContent.AdmitCatalogPayload(Payload());
        first = engine.Graphics.CreateAuthoredMaterial(new(catalog, FirstMaterial, texture));
        seconds = Enumerable.Range(0, VariantCount).Select(i => engine.Graphics.CreateAuthoredMaterial(new AuthoredMaterialAppearanceRequest(catalog, SecondId(i), texture))).ToArray();
        CameraQueries.TryLookAtPose(Eye, Target, 0, out CameraPose pose);
        camera = engine.CameraView.CreateCamera(new(pose, CameraBasisMode.Derived, default,
            new(CameraProjectionKind.Perspective, FieldOfView, 0, NearClip, FarClip), CameraViewports.Full));
        engine.CameraView.SetActiveCamera(camera);
    }
    private static AuthoredCatalogEntryInput Entry(string id) => new(id, Version, true, Hash, id == Texture, id == Texture ? "tiles.png" : "", false, "");
    private static AuthoredCatalogDependencyInput Dependency(string owner, string reference) => new(owner, reference, AssetVersionRequirementKind.Exact, Version, true, Hash);
    private static string SecondId(int variant) => $"{SecondMaterial}-{variant}";
    private static AuthoredMaterialInput MaterialInput(string id, bool decorative = false) => new(id, true, true, true,
        decorative ? AuthoredStructuralClass.Decorative : AuthoredStructuralClass.Solid, new(1, 1, 1, 1), true, Texture, AssetVersionRequirementKind.Exact, Version, true, Hash,
        1, new(1, 1, 1, 1), new(0, 0, 0, 1), 0, AuthoredUvStrategy.Atlas);
    private static AuthoredVoxelSurfaceInput Surface(string material, string atlas, bool blend = false) => new(material, Version,
        AuthoredVoxelSurfaceMappingKind.Atlas, "", AssetVersionRequirementKind.Any, 0, false, "",
        atlas, AssetVersionRequirementKind.Exact, Version, true, Hash, "tile", 1, 1, 0, 0, blend ? AuthoredVoxelAlphaModeKind.Blend : AuthoredVoxelAlphaModeKind.Opaque, 0);
    private static AuthoredAtlasRegionInput Region(string atlas, uint x) => new(atlas, "tile", x, 0, TileSize, TileSize, 0, 0, 0, 0, AuthoredAtlasInset.HalfTexel);
    private static AuthoredCatalogPayloadAdmitRequest Payload() => new(
        new[] { Entry(Texture), Entry(FirstAtlas), Entry(SecondAtlas), Entry(FirstMaterial), Entry(SecondId(0)), Entry(SecondId(1)), Entry(SecondId(2)) },
        new[] { Dependency(FirstAtlas, Texture), Dependency(SecondAtlas, Texture), Dependency(FirstMaterial, Texture), Dependency(FirstMaterial, FirstAtlas) }.Concat(Enumerable.Range(0, VariantCount).SelectMany(i => new[] { Dependency(SecondId(i), Texture), Dependency(SecondId(i), SecondAtlas) })).ToArray(),
        new[] { MaterialInput(FirstMaterial), MaterialInput(SecondId(0)), MaterialInput(SecondId(1), true), MaterialInput(SecondId(2), true) },
        new AuthoredTextureInput[] { new(Texture, TextureWidth, TextureHeight, AuthoredTextureFilter.Nearest, AuthoredTextureWrap.Clamp) },
        new AuthoredVoxelAtlasInput[] { new(FirstAtlas, Version, Texture, AssetVersionRequirementKind.Exact, Version, true, Hash), new(SecondAtlas, Version, Texture, AssetVersionRequirementKind.Exact, Version, true, Hash) },
        new[] { Region(FirstAtlas, 0), Region(SecondAtlas, TileSize) }, new[] { Surface(FirstMaterial, FirstAtlas), Surface(SecondId(0), SecondAtlas), Surface(SecondId(1), SecondAtlas), Surface(SecondId(2), SecondAtlas, true) });
    private VoxelSceneMaterialBinding[] Bindings() => [new(FirstSlot, first), new(SecondSlot, seconds[mode])];
    private void SetSecond(bool present)
    {
        var revision = engine.Voxel.ReadScene(new(scene)).SourceRevision;
        engine.Voxel.ApplyEdits(new(scene, revision, new VoxelEdit[] { new(present ? VoxelEditKind.Set : VoxelEditKind.Clear, SecondAddress, SecondSlot) }));
    }
    public void Start()
    {
        engine.Voxel.ApplyEdits(new(scene, 0, new VoxelEdit[] { new(VoxelEditKind.Set, FirstAddress, FirstSlot) }));
        presentation = engine.VoxelScenePresentation.ProjectSceneDirectional(new(scene,
            Bindings(), ReadOnlyMemory<VoxelSceneFaceMaterialBinding>.Empty));
        SetSecond(true);
        var readout = engine.VoxelScenePresentation.RefreshScene(presentation);
        VerifyRejections();
        proof = $"two-atlas projection passed; materials={readout.MaterialCount}; chunks={readout.ChunkCount}";
    }
    private void VerifyRejections()
    {
        foreach (var bindings in new[] { Array.Empty<VoxelSceneMaterialBinding>(), new[] { new VoxelSceneMaterialBinding(FirstSlot, first), new VoxelSceneMaterialBinding(FirstSlot, first) } })
        {
            try
            {
                engine.VoxelScenePresentation.UpdateSceneDirectional(new(presentation!, bindings, ReadOnlyMemory<VoxelSceneFaceMaterialBinding>.Empty));
                throw new InvalidOperationException("Invalid bindings unexpectedly succeeded");
            }
            catch (EngineCallException error)
            {
                if (error.Diagnostics.Length == 0 || !error.Diagnostics.Span[0].Message.Contains("source slots")) throw;
                caughtRejections++;
            }
        }
        engine.VoxelScenePresentation.RefreshScene(presentation!);
    }
    [DebugCommand("atlases.mode")]
    public string Mode(int value)
    {
        if (value < 0 || value >= seconds.Length) throw new ArgumentOutOfRangeException(nameof(value));
        mode = value;
        engine.VoxelScenePresentation.UpdateSceneDirectional(new(presentation!, Bindings(), ReadOnlyMemory<VoxelSceneFaceMaterialBinding>.Empty));
        return Inspect();
    }
    [DebugCommand("atlases.palette")]
    public string Palette()
    {
        SetSecond(false); engine.VoxelScenePresentation.RefreshScene(presentation!);
        SetSecond(true); engine.VoxelScenePresentation.RefreshScene(presentation!);
        VerifyRejections(); return Inspect();
    }
    [DebugCommand("atlases.inspect")] public string Inspect() => $"{proof}; mode={mode}; caughtRejections={caughtRejections}";
    public void RegisterDebugCommands(IDebugCommandModuleRegistrar registrar) => registrar.Register(this);
    public ProductUpdateResult Update(ProductUpdate update) => ProductUpdateResult.None;
    public void Attach() { if (presentation is not null) engine.VoxelScenePresentation.RefreshScene(presentation); }
    public void Pause() { }
    public void Resume() { }
    public void Restart() { }
    public void Shutdown() { }
    public void Dispose() { presentation?.Dispose(); camera.Dispose(); first.Dispose(); foreach (var material in seconds) material.Dispose(); catalog.Dispose(); texture.Dispose(); scene.Dispose(); }
}
