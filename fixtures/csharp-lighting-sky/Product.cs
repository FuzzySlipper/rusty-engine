using System.Numerics;
using System.Text.Json;
using System.Text.Json.Serialization;
using Rusty.Engine;
using Rusty.Engine.Debugging;

namespace CsharpLightingSky;

public sealed class Product : IEngineProduct, IDebugCommandModuleSource, IDebugCommandModule
{
    private const uint ChunkEdge = 8;
    private const int RoomWidth=8, RoomHeight=5;
    private const float CameraFov=65, CameraNear=.05f, CameraFar=100, RoundTripTolerance=.0001f;
    private static readonly Color StoneColor=new(.6f,.6f,.6f,1);
    private static readonly Vector3 TorchColor=new(1,.65f,.25f), CellCenter=new(.5f,.5f,.5f);
    private static readonly VoxelAddress InsideSample=new(3,1,3), OutsideSample=new(3,1,-2);
    private const ulong TorchId = 1;
    private const float TorchIntensity = 35, TorchRange = 12, Horizon = 64;
    private static readonly Vector3 TorchPosition = new(3.5f,2.5f,3.5f);
    private static readonly Vector3 RoomEye = new(3.5f,2.5f,6.5f), RoomTarget = new(3.5f,2,1);
    private static readonly Vector3 SkyEye = new(12,8,14), SkyTarget = new(3.5f,5,3.5f);
    private readonly IEngineContext engine;
    private readonly SpatialSession scene;
    private readonly Material stone;
    private readonly Camera camera;
    private readonly Light torch;
    private readonly RenderResource day, night;
    private VoxelScenePresentation? presentation;
    private LightDescriptor descriptor;
    private float clock;
    private bool roundTrip;
    private float litValue, blockedValue, darkValue;

    public Product(ProductCreateContext context)
    {
        engine = context.Engine;
        scene = engine.Spatial.CreateSession(new(1,ChunkEdge,VoxelSurfaceMode.GreedyCubes));
        stone = engine.Graphics.CreateMaterial(new(StoneColor,default(RenderResourceReference),1,new Color(1,1,1,1),Vector3.Zero,0,false));
        camera = engine.CameraView.CreateCamera(Camera(RoomEye,RoomTarget));
        engine.CameraView.SetActiveCamera(camera);
        descriptor = new(LightKind.Point,TorchColor,TorchIntensity,true,TorchPosition,Vector3.UnitY,true,TorchRange,2,0,0,LightShadowIntent.Requested);
        torch = engine.Graphics.CreateLight(new(TorchId,false,0,descriptor));
        day = engine.Graphics.OpenResource(new("day.png",TextureFilter.Linear,TextureWrap.Clamp)).Handle;
        night = engine.Graphics.OpenResource(new("night.png",TextureFilter.Linear,TextureWrap.Clamp)).Handle;
        engine.CameraView.SetSkyBackgroundBlend(new(day,night,clock));
    }
    private static CameraDescriptor Camera(Vector3 eye,Vector3 target)
    {
        CameraQueries.TryLookAtPose(eye,target,0,out CameraPose pose);
        return new(pose,CameraBasisMode.Derived,default,new(CameraProjectionKind.Perspective,CameraFov,0,CameraNear,CameraFar),CameraViewports.Full);
    }
    public void Start()
    {
        uint[] slots = new uint[ChunkEdge*ChunkEdge*ChunkEdge];
        for(int z=0;z<RoomWidth;z++) for(int y=0;y<RoomHeight;y++) for(int x=0;x<RoomWidth;x++)
            if(x==0||x==RoomWidth-1||y==0||y==RoomHeight-1||z==0||z==RoomWidth-1) slots[x+ChunkEdge*(y+ChunkEdge*z)] = 1;
        VoxelResidencyOperation[] ops=[new(VoxelResidencyOperationKind.Admit,new(0,0,0),0,0,(uint)slots.Length)];
        engine.Voxel.ApplyResidency(new(scene,0,VoxelResidencyHistoryPolicy.ResetToPublishedAuthority,ops,slots));
        presentation = engine.VoxelScenePresentation.ProjectScene(new(scene,new VoxelSceneMaterialBinding[]{new(1,stone)}));
        litValue = Sample(InsideSample);
        blockedValue = Sample(OutsideSample);
        SetTorch(false); darkValue=Sample(InsideSample); SetTorch(true);
        if (!(litValue>0 && blockedValue==0 && darkValue==0)) throw new InvalidOperationException("light propagation proof failed");
        var saved = new LightingSave(engine.Voxel.ExportHistory(new(scene)).ToArray(),descriptor);
        byte[] bytes=JsonSerializer.SerializeToUtf8Bytes(saved,ProofJsonContext.Default.LightingSave);
        SetTorch(false);
        var restored=JsonSerializer.Deserialize(bytes,ProofJsonContext.Default.LightingSave)!;
        engine.Voxel.RestoreHistory(new(scene,restored.Scene));
        descriptor=restored.Torch;
        engine.Graphics.UpdateLight(new(torch,new(TorchId,false,0,descriptor)));
        roundTrip=MathF.Abs(Sample(InsideSample)-litValue)<RoundTripTolerance;
        if(!roundTrip) throw new InvalidOperationException("light and scene persistence proof failed");
    }
    private float Sample(VoxelAddress address) => engine.Voxel.SampleDirectLighting(new(scene,address,CellCenter,Vector3.Zero,Horizon,new LightDescriptor[]{descriptor})).Luminance;
    private void SetTorch(bool enabled) { descriptor=descriptor with { Enabled=enabled }; engine.Graphics.UpdateLight(new(torch,new(TorchId,false,0,descriptor))); }
    [DebugCommand("lighting.torch")]
    public string Torch(bool enabled) { SetTorch(enabled); return Inspect(); }
    [DebugCommand("lighting.sky")]
    public string Sky(float amount) { clock=Math.Clamp(amount,0,1); engine.CameraView.SetSkyBackgroundBlend(new(day,night,clock)); engine.CameraView.UpdateCamera(new(camera,Camera(SkyEye,SkyTarget))); return Inspect(); }
    [DebugCommand("lighting.room")]
    public string Room() { engine.CameraView.UpdateCamera(new(camera,Camera(RoomEye,RoomTarget))); return Inspect(); }
    [DebugCommand("lighting.panorama")]
    public string Panorama() { engine.CameraView.SetSkyBackground(day); return Inspect(); }
    [DebugCommand("lighting.inspect")]
    public string Inspect() => JsonSerializer.Serialize(new LightingProof(roundTrip,litValue,blockedValue,darkValue,Sample(InsideSample),descriptor.Enabled,clock),ProofJsonContext.Default.LightingProof);
    public void RegisterDebugCommands(IDebugCommandModuleRegistrar registrar)=>registrar.Register(this);
    public ProductUpdateResult Update(ProductUpdate update)=>ProductUpdateResult.None;
    public void Attach() { if(presentation is not null)engine.VoxelScenePresentation.RefreshScene(presentation); }
    public void Pause(){} public void Resume(){} public void Restart(){SetTorch(true);} public void Shutdown(){}
    public void Dispose(){engine.CameraView.ClearSkyBackground(default); presentation?.Dispose(); torch.Dispose(); camera.Dispose(); stone.Dispose(); scene.Dispose(); day.Dispose(); night.Dispose();}
}
internal sealed record LightingSave(byte[] Scene,LightDescriptor Torch);
internal sealed record LightingProof(bool RoundTrip,float Lit,float Blocked,float Dark,float Current,bool Torch,float Clock);
[JsonSourceGenerationOptions(PropertyNamingPolicy=JsonKnownNamingPolicy.CamelCase,IncludeFields=true)]
[JsonSerializable(typeof(LightingSave))]
[JsonSerializable(typeof(LightingProof))]
internal partial class ProofJsonContext : JsonSerializerContext { }
