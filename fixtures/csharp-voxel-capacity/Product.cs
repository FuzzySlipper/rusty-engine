using System.Numerics;
using Rusty.Engine;
using Rusty.Engine.Debugging;
namespace CsharpVoxelCapacity;
public sealed class Product : IEngineProduct, IDebugCommandModuleSource, IDebugCommandModule
{
    private const int Count = 16;
    private const uint Version = 1, SlotLimit = 65535;
    private const string Texture = "texture/tiles", Atlas = "sprite-sheet/tiles";
    private const string Hash = "548b62ed15eeb916034870e9ca757a54e5d429e4e66eba4601cc98fd2c56cf4a";
    private readonly IEngineContext engine;
    private readonly SpatialSession scene;
    private readonly RenderResource texture;
    private readonly AuthoredCatalog catalog;
    private readonly Material[] materials;
    private readonly Camera camera;
    private VoxelScenePresentation? presentation;
    private string rejection = "not exercised";
    private static string Id(int i) => $"material/block-{i}";
    private static AuthoredCatalogEntryInput Entry(string id) => new(id, Version, true, Hash, id == Texture, id == Texture ? "tiles.png" : "", false, "");
    private static AuthoredCatalogDependencyInput Dependency(string owner, string target) => new(owner, target, AssetVersionRequirementKind.Exact, Version, true, Hash);
    private static AuthoredCatalogPayloadAdmitRequest Payload() => new(
        new[] { Entry(Texture), Entry(Atlas) }.Concat(Enumerable.Range(0, Count).Select(i => Entry(Id(i)))).ToArray(),
        new[] { Dependency(Atlas, Texture) }.Concat(Enumerable.Range(0, Count).SelectMany(i => new[] { Dependency(Id(i), Texture), Dependency(Id(i), Atlas) })).ToArray(),
        Enumerable.Range(0, Count).Select(i => new AuthoredMaterialInput(Id(i), true, true, true, AuthoredStructuralClass.Solid, new(1,1,1,1), true, Texture, AssetVersionRequirementKind.Exact, Version, true, Hash, 1, new(1,1,1,1), new(0,0,0,1), 0, AuthoredUvStrategy.Atlas)).ToArray(),
        new AuthoredTextureInput[] { new(Texture, 32, 2, AuthoredTextureFilter.Nearest, AuthoredTextureWrap.Clamp) },
        new AuthoredVoxelAtlasInput[] { new(Atlas, Version, Texture, AssetVersionRequirementKind.Exact, Version, true, Hash) },
        Enumerable.Range(0, Count).Select(i => new AuthoredAtlasRegionInput(Atlas, $"tile-{i}", (uint)i*2, 0, 2, 2, 0, 0, 0, 0, AuthoredAtlasInset.HalfTexel)).ToArray(),
        Enumerable.Range(0, Count).Select(i => new AuthoredVoxelSurfaceInput(Id(i), Version, AuthoredVoxelSurfaceMappingKind.Atlas, "", AssetVersionRequirementKind.Any, 0, false, "", Atlas, AssetVersionRequirementKind.Exact, Version, true, Hash, $"tile-{i}", 1, 1, 0, 0, AuthoredVoxelAlphaModeKind.Opaque, 0)).ToArray());
    public Product(ProductCreateContext context)
    {
        engine=context.Engine;
        scene=engine.Spatial.CreateSession(new(1, 8, VoxelSurfaceMode.GreedyCubes));
        texture=engine.Graphics.OpenResource(new("tiles.png")).Handle;
        catalog=engine.AuthoredContent.AdmitCatalogPayload(Payload());
        materials=Enumerable.Range(0, Count).Select(i => engine.Graphics.CreateAuthoredMaterial(new(catalog, Id(i), texture))).ToArray();
        CameraQueries.TryLookAtPose(new(11,12,15), new(3.5f,0,3.5f), 0, out CameraPose pose);
        camera=engine.CameraView.CreateCamera(new(pose,CameraBasisMode.Derived,default,new(CameraProjectionKind.Perspective,50,0,.05f,100),CameraViewports.Full));
        engine.CameraView.SetActiveCamera(camera);
    }
    private VoxelSceneMaterialBinding[] Bindings() => materials.Select((material,i)=>new VoxelSceneMaterialBinding((uint)i+1,material)).ToArray();
    public void Start()
    {
        engine.Voxel.ApplyEdits(new(scene,0,Enumerable.Range(0,Count).Select(i=>new VoxelEdit(VoxelEditKind.Set,new((i%4)*2,0,(i/4)*2),(uint)i+1)).ToArray()));
        presentation=engine.VoxelScenePresentation.ProjectSceneDirectional(new(scene,Bindings(),ReadOnlyMemory<VoxelSceneFaceMaterialBinding>.Empty));
        Reject();
    }
    [DebugCommand("capacity.reject")]
    public string Reject()
    {
        var invalid=Bindings().Append(new VoxelSceneMaterialBinding(SlotLimit+1,materials[0])).ToArray();
        try { using var unexpected=engine.VoxelScenePresentation.ProjectSceneDirectional(new(scene,invalid,ReadOnlyMemory<VoxelSceneFaceMaterialBinding>.Empty)); throw new InvalidOperationException("Out-of-range binding accepted"); }
        catch(EngineCallException error) { rejection=string.Join(";",error.Diagnostics.ToArray().Select(d=>d.Message)); if(!rejection.Contains("65536") || !rejection.Contains("65535") || !rejection.Contains("material")) throw; }
        engine.VoxelScenePresentation.RefreshScene(presentation!);
        return Inspect();
    }
    [DebugCommand("capacity.inspect")]
    public string Inspect() => $"materials={engine.VoxelScenePresentation.RefreshScene(presentation!).MaterialCount}; rejection={rejection}";
    public void RegisterDebugCommands(IDebugCommandModuleRegistrar registrar)=>registrar.Register(this);
    public ProductUpdateResult Update(ProductUpdate update)=>ProductUpdateResult.None;
    public void Pause(){} public void Resume(){} public void Restart(){} public void Shutdown(){}
    public void Dispose(){presentation?.Dispose();camera.Dispose();foreach(var material in materials)material.Dispose();catalog.Dispose();texture.Dispose();scene.Dispose();}
}
