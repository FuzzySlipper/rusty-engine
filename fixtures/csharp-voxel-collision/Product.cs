using System.Numerics;
using Rusty.Engine;
using Rusty.Engine.Debugging;
namespace CsharpVoxelCollision;
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
    private ulong updates;
    private Vector3 position = new(0.5f, 6f, 0.5f);
    private CharacterMotion motion = new(Vector3.Zero, Vector3.Zero, false, CharacterStance.Standing,
        0, 0, 0, false, 0, Vector3.Zero, Vector3.Zero, Quaternion.Identity, Vector3.Zero, 6, 6, 0, 0);
    private CharacterControllerConfig config;
    private string lastEdit = "none";
    private static string Id(int i) => $"material/block-{i}";
    private static AuthoredCatalogEntryInput Entry(string id) => new(id, Version, true, Hash, id == Texture, id == Texture ? "tiles.png" : "", false, "");
    private static AuthoredCatalogDependencyInput Dependency(string owner, string target) => new(owner, target, AssetVersionRequirementKind.Exact, Version, true, Hash);
    private static AuthoredCatalogPayloadAdmitRequest Payload() => new(
        new[] { Entry(Texture), Entry(Atlas) }.Concat(Enumerable.Range(0, Count).Select(i => Entry(Id(i)))).ToArray(),
        new[] { Dependency(Atlas, Texture) }.Concat(Enumerable.Range(0, Count).SelectMany(i => new[] { Dependency(Id(i), Texture), Dependency(Id(i), Atlas) })).ToArray(),
        Enumerable.Range(0, Count).Select(i => new AuthoredMaterialInput(Id(i), i != 10, i != 10, i != 10, i == 10 ? AuthoredStructuralClass.Decorative : AuthoredStructuralClass.Solid, new(1,1,1,1), true, Texture, AssetVersionRequirementKind.Exact, Version, true, Hash, 1, new(1,1,1,1), new(0,0,0,1), 0, AuthoredUvStrategy.Atlas)).ToArray(),
        new AuthoredTextureInput[] { new(Texture, 32, 2, AuthoredTextureFilter.Nearest, AuthoredTextureWrap.Clamp) },
        new AuthoredVoxelAtlasInput[] { new(Atlas, Version, Texture, AssetVersionRequirementKind.Exact, Version, true, Hash) },
        Enumerable.Range(0, Count).Select(i => new AuthoredAtlasRegionInput(Atlas, $"tile-{i}", (uint)i*2, 0, 2, 2, 0, 0, 0, 0, AuthoredAtlasInset.HalfTexel)).ToArray(),
        Enumerable.Range(0, Count).Select(i => new AuthoredVoxelSurfaceInput(Id(i), Version, AuthoredVoxelSurfaceMappingKind.Atlas, "", AssetVersionRequirementKind.Any, 0, false, "", Atlas, AssetVersionRequirementKind.Exact, Version, true, Hash, $"tile-{i}", 1, 1, 0, 0, AuthoredVoxelAlphaModeKind.Opaque, 0)).ToArray());
    public Product(ProductCreateContext context)
    {
        engine=context.Engine;
        scene=engine.Spatial.CreateSession(new(1, 8, VoxelSurfaceMode.GreedyCubes));
        engine.Voxel.ConfigureMaterialCollision(new(scene,
            Enumerable.Range(0,Count).Select(i=>new VoxelMaterialCollision((uint)i+1,i!=10)).ToArray()));
        engine.Voxel.ConfigureMaterialOcclusion(new(scene,
            Enumerable.Range(0,Count).Select(i=>new VoxelMaterialOcclusion((uint)i+1,i!=10)).ToArray()));
        config=engine.Spatial.DefaultCharacterControllerConfig();
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
        List<VoxelEdit> edits=[];
        for(int x=-2;x<=2;x++) for(int z=-2;z<=2;z++) for(int y=0;y<=3;y++)
            edits.Add(new(VoxelEditKind.Set,new(x,y,z),y==0?1u:11u));
        engine.Voxel.ApplyEdits(new(scene,edits.ToArray()));
        presentation=engine.VoxelScenePresentation.ProjectSceneDirectional(new(scene,Bindings(),ReadOnlyMemory<VoxelSceneFaceMaterialBinding>.Empty));
    }
    [DebugCommand("voxel.proof.edit")]
    public string Edit(int count = 64, uint slot = 11)
    {
        if(count<1 || count>64 || slot<1 || slot>Count) throw new ArgumentOutOfRangeException(nameof(count));
        VoxelEdit[] edits=Enumerable.Range(0,count).Select(i=>new VoxelEdit(VoxelEditKind.Set,
            new(8+i%4,4+(i/4)%4,8+i/16),slot)).ToArray();
        lastEdit=engine.Voxel.ApplyEdits(new(scene,edits)).ToString();
        // Deliberately no product residency follow-up: subsequent updates must run.
        return $"updates={updates};edit={lastEdit}";
    }
    [DebugCommand("voxel.proof.reject-embedded")]
    public string RejectEmbedded()
    {
        using SpatialSession embedded = engine.Spatial.CreateSession(new(1, 8, VoxelSurfaceMode.GreedyCubes));
        VoxelEdit[] solid = Enumerable.Range(0,64).Select(i=>new VoxelEdit(VoxelEditKind.Set,
            new(i%4,(i/4)%4,i/16),1)).ToArray();
        engine.Voxel.ApplyEdits(new(embedded,solid));
        try
        {
            CharacterMotion initial = new(Vector3.Zero,Vector3.Zero,false,CharacterStance.Standing,
                0,0,0,false,0,Vector3.Zero,Vector3.Zero,Quaternion.Identity,Vector3.Zero,1.5f,1.5f,0,0);
            engine.Spatial.ProposeCharacterStep(new(default,embedded,new(1.5f,1.5f,1.5f),initial,default,
                ReadOnlyMemory<CharacterObstacle>.Empty,ReadOnlyMemory<CharacterMeshInstance>.Empty,config,
                new(default,Vector2.Zero,0,false,false,false,Vector3.Zero,Vector3.Zero,1f/60,1)));
            throw new InvalidOperationException("Expected bounded penetration rejection");
        }
        catch(EngineCallException error)
        {
            string diagnostics=string.Join("; ",error.Diagnostics.ToArray().Select(d=>$"{d.Code}: {d.Message}"));
            if(!diagnostics.Contains("unresolved-character-controller-penetration",StringComparison.Ordinal)) throw;
            return diagnostics;
        }
    }
    [DebugCommand("voxel.proof.inspect")]
    public string Inspect()
    {
        SpatialHit hit=engine.Spatial.CastRay(new(scene,new(.5f,8,.5f),-Vector3.UnitY,10,
            new(uint.MaxValue,uint.MaxValue),ReadOnlyMemory<SpatialEntityCollider>.Empty,ReadOnlyMemory<ulong>.Empty,ReadOnlyMemory<SpatialEntityCollider>.Empty));
        return $"updates={updates};position={position};grounded={motion.Grounded};ray={hit};edit={lastEdit}";
    }
    public void RegisterDebugCommands(IDebugCommandModuleRegistrar registrar)=>registrar.Register(this);
    public ProductUpdateResult Update(ProductUpdate update)
    {
        updates++;
        CharacterStepReceipt step=engine.Spatial.ProposeCharacterStep(new(default,scene,position,motion,default,
            ReadOnlyMemory<CharacterObstacle>.Empty,ReadOnlyMemory<CharacterMeshInstance>.Empty,config,
            new(default,Vector2.Zero,0,false,false,false,Vector3.Zero,Vector3.Zero,1f/60,updates)));
        position=step.Transform.Translation; motion=step.Motion;
        if(updates%60==0) Console.WriteLine($"VOXEL_PROOF updates={updates};position={position}");
        return ProductUpdateResult.None;
    }
    public void Pause(){} public void Resume(){} public void Restart(){} public void Shutdown(){}
    public void Dispose(){
        engine.Diagnostics.Publish(new(DiagnosticsSeverity.Info,DiagnosticsDisposition.Accepted,
            "voxel-proof","DISPOSED","fixture disposal ran",""));
        Console.WriteLine("VOXEL_PROOF disposed");presentation?.Dispose();camera.Dispose();foreach(var material in materials)material.Dispose();catalog.Dispose();texture.Dispose();scene.Dispose();}
}
