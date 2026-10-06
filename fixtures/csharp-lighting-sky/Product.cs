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
    private const ulong TorchId = 1, SunId = 2;
    private const float TorchIntensity = 35, TorchRange = 12, Horizon = 64;
    private static readonly Vector3 TorchPosition = new(3.5f,2.5f,3.5f);
    private static readonly Vector3 RoomEye = new(3.5f,2.5f,6.5f), RoomTarget = new(3.5f,2,1);
    private static readonly Vector3 SkyEye = new(12,8,14), SkyTarget = new(3.5f,5,3.5f);
    private static readonly Color FogColor = new(.55f,.6f,.7f,1);
    // The sun follows the product's clock from noon (0) to dusk (1).
    private static readonly Vector3 NoonSunColor = new(1,.96f,.88f), DuskSunColor = new(1,.55f,.3f);
    private const float NoonElevation = 55, DuskElevation = 6, SunAzimuth = 210, NoonSunIntensity = 2.5f, DuskSunIntensity = .5f;
    // Height fog over the room, hazy toward the sun, with its disc and halo.
    private static readonly AtmosphereRequest Air = new(0,6,new Color(1,.7f,.45f,1),8,1.5f,.35f);
    private readonly IEngineContext engine;
    private readonly SpatialSession scene;
    private readonly Material stone;
    private readonly Camera camera;
    private readonly Light torch, sun;
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
        sun = engine.Graphics.CreateLight(new(SunId,false,0,Sun(clock)));
        day = engine.Graphics.OpenResource(new("day.png",TextureFilter.Linear,TextureWrap.Clamp)).Handle;
        night = engine.Graphics.OpenResource(new("night.png",TextureFilter.Linear,TextureWrap.Clamp)).Handle;
        engine.CameraView.SetSkyBackgroundBlend(new(day,night,clock));
    }
    private static LightDescriptor Sun(float clock)
    {
        float elevation = float.DegreesToRadians(NoonElevation+(DuskElevation-NoonElevation)*clock), azimuth = float.DegreesToRadians(SunAzimuth);
        // The light travels from the sun, down toward the room.
        var travel = -new Vector3(MathF.Cos(elevation)*MathF.Sin(azimuth),MathF.Sin(elevation),MathF.Cos(elevation)*MathF.Cos(azimuth));
        return new(LightKind.Directional,Vector3.Lerp(NoonSunColor,DuskSunColor,clock),NoonSunIntensity+(DuskSunIntensity-NoonSunIntensity)*clock,true,Vector3.Zero,travel,false,0,0,0,0,LightShadowIntent.Disabled);
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
        VoxelResidencyOperation[] ops=[new(VoxelResidencyOperationKind.Admit,new(0,0,0),0,(uint)slots.Length)];
        engine.Voxel.ApplyResidency(new(scene,ops,slots));
        presentation = engine.VoxelScenePresentation.ProjectScene(new(scene,new VoxelSceneMaterialBinding[]{new(1,stone)}));
        litValue = Sample(InsideSample);
        blockedValue = Sample(OutsideSample);
        SetTorch(false); darkValue=Sample(InsideSample); SetTorch(true);
        if (!(litValue>0 && blockedValue==0 && darkValue==0)) throw new InvalidOperationException("light propagation proof failed");
        // The product owns its room data and saves it with the light.
        var saved = new LightingSave(slots,descriptor);
        byte[] bytes=JsonSerializer.SerializeToUtf8Bytes(saved,ProofJsonContext.Default.LightingSave);
        SetTorch(false);
        var restored=JsonSerializer.Deserialize(bytes,ProofJsonContext.Default.LightingSave)!;
        VoxelResidencyOperation[] replace=[new(VoxelResidencyOperationKind.Replace,new(0,0,0),0,(uint)restored.Room.Length)];
        engine.Voxel.ApplyResidency(new(scene,replace,restored.Room));
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
    public string Sky(float amount) { clock=Math.Clamp(amount,0,1); engine.CameraView.SetSkyBackgroundBlend(new(day,night,clock)); engine.Graphics.UpdateLight(new(sun,new(SunId,false,0,Sun(clock)))); engine.CameraView.UpdateCamera(new(camera,Camera(SkyEye,SkyTarget))); return Inspect(); }
    [DebugCommand("lighting.atmosphere")]
    public string Atmosphere(bool enabled) { engine.CameraView.SetAtmosphere(enabled ? Air : default); return Inspect(); }
    [DebugCommand("lighting.room")]
    public string Room() { engine.CameraView.UpdateCamera(new(camera,Camera(RoomEye,RoomTarget))); return Inspect(); }
    [DebugCommand("lighting.fog")]
    public string Fog(float density) { engine.CameraView.SetFog(density>0 ? new(FogMode.ExponentialSquared,FogColor,0,0,density) : new(FogMode.Off,default,0,0,0)); return Inspect(); }
    [DebugCommand("lighting.exposure")]
    public string Exposure(float exposure) { engine.CameraView.SetToneMapping(new(ToneMappingOperator.AcesFilmic,exposure)); return Inspect(); }
    [DebugCommand("lighting.panorama")]
    public string Panorama() { engine.CameraView.SetSkyBackground(day); return Inspect(); }
    [DebugCommand("lighting.inspect")]
    public string Inspect() => JsonSerializer.Serialize(new LightingProof(roundTrip,litValue,blockedValue,darkValue,Sample(InsideSample),descriptor.Enabled,clock),ProofJsonContext.Default.LightingProof);
    public void RegisterDebugCommands(IDebugCommandModuleRegistrar registrar)=>registrar.Register(this);
    public ProductUpdateResult Update(ProductUpdate update)=>ProductUpdateResult.None;
    public void Pause(){} public void Resume(){} public void Restart(){SetTorch(true);} public void Shutdown(){}
    public void Dispose(){engine.CameraView.ClearSkyBackground(default); presentation?.Dispose(); torch.Dispose(); sun.Dispose(); camera.Dispose(); stone.Dispose(); scene.Dispose(); day.Dispose(); night.Dispose();}
}
internal sealed record LightingSave(uint[] Room,LightDescriptor Torch);
internal sealed record LightingProof(bool RoundTrip,float Lit,float Blocked,float Dark,float Current,bool Torch,float Clock);
[JsonSourceGenerationOptions(PropertyNamingPolicy=JsonKnownNamingPolicy.CamelCase,IncludeFields=true)]
[JsonSerializable(typeof(LightingSave))]
[JsonSerializable(typeof(LightingProof))]
internal partial class ProofJsonContext : JsonSerializerContext { }
