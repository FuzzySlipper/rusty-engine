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
    private const ulong TorchId = 1, SunId = 2, HemisphereId = 3, SkyAmbientId = 4;
    // A hemisphere light for a product that owns its lights: pale sky, earthy ground.
    private static readonly Vector3 HemisphereSkyColor = new(.55f,.7f,1), HemisphereGroundColor = new(.35f,.3f,.25f);
    // The sky's ambient light, occluded by the room: its range is half the side of the square of sky it looks down over.
    private static readonly Vector3 SkyAmbientColor = new(.6f,.7f,.9f);
    private const float SkyAmbientIntensity = .3f;
    private const float TorchIntensity = 35, TorchRange = 12, Horizon = 64;
    private static readonly Vector3 TorchPosition = new(3.5f,2.5f,3.5f);
    private static readonly Vector3 RoomEye = new(3.5f,2.5f,6.5f), RoomTarget = new(3.5f,2,1);
    private static readonly Vector3 SkyEye = new(12,8,14), SkyTarget = new(3.5f,5,3.5f);
    // A doorway in the room's +z wall, so daylight has a way in; seen from the back wall.
    private const int DoorMinX = 3, DoorMaxX = 4, DoorMinY = 1, DoorMaxY = 3;
    private static readonly Vector3 CaveEye = new(3.5f,2.5f,1.2f), CaveTarget = new(3.5f,2,7.5f);
    // The indirect light volume: the room and the ground around it (32 bricks of 16 m, so an edit shows which bricks rebake), probes half a metre apart.
    private static readonly Vector3 IndirectCenter = new(3.5f,2.5f,3.5f), IndirectExtent = new(20,3,20);
    private const float IndirectSpacing = .5f;
    private const uint IndirectBounces = 2;
    private static readonly Color FogColor = new(.55f,.6f,.7f,1);
    // The sun follows the product's clock from noon (0) to dusk (1); it casts a shadow when renderer shadows are on, so the room's inside is lit only through its doorway.
    private static readonly Vector3 NoonSunColor = new(1,.96f,.88f), DuskSunColor = new(1,.55f,.3f);
    private const float NoonElevation = 55, DuskElevation = 6, SunAzimuth = 210, NoonSunIntensity = 2.5f, DuskSunIntensity = .5f;
    // Height fog over the room, hazy toward the sun, with its disc and halo.
    private static readonly AtmosphereRequest Air = new(0,6,new Color(1,.7f,.45f,1),8,1.5f,.35f);
    // The torch's fire on the floor by the -x wall: a soft additive flame flipbook, soft additive embers and soft alpha smoke rising along the wall, so each sheet meets the stone without a hard edge. Seen from the fire viewpoint.
    private const ulong FlameId = 10, EmberId = 11, SmokeId = 12;
    private static readonly Vector3 FirePosition = new(1.55f,1.05f,3.5f);
    private static readonly Vector3 FireEye = new(3.4f,1.9f,5.2f), FireTarget = new(1.5f,1.45f,3.5f);
    private const float FlameSoftness = .3f, EmberSoftness = .1f, SmokeSoftness = .5f, FlameFramesPerSecond = 12;
    private const ushort FlameFrames = 8;
    private readonly IEngineContext engine;
    private readonly SpatialSession scene;
    private readonly Material stone;
    private readonly Camera camera;
    private readonly Light torch, sun, hemisphere, skyAmbient;
    private readonly RenderResource day, night;
    private RenderResource? flameSprite, emberSprite, smokeSprite;
    // Flat shading (#9543): a smooth low-poly sphere and the low-poly tree, each twice: authored normals on the left, the flat-shading feature on the right, by the torch.
    private const ulong SphereSmoothId = 20, SphereFlatId = 21, TreeSmoothId = 22, TreeFlatId = 23;
    private static readonly Color FacetColor = new(.75f,.7f,.6f,1), TreeColor = new(.4f,.6f,.3f,1);
    private static readonly Vector3 FacetsEye = new(3.5f,2.4f,6.8f), FacetsTarget = new(3.5f,1.5f,3.2f);
    private Material? facetSmooth, facetFlat, treeFlat;
    private MeshResource? sphereSmooth, sphereFlat;
    private RenderResource? treeMesh;
    private Appearance? sphereSmoothLook, sphereFlatLook, treeSmoothLook, treeFlatLook;
    private PresentationEmitter? flame, embers, smoke;
    private PresentationParticleDescriptor flameFire, emberFire, smokeFire;
    private VoxelScenePresentation? presentation;
    private LightDescriptor descriptor;
    private uint[] room = [];
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
        hemisphere = engine.Graphics.CreateLight(new(HemisphereId,false,0,LightDescriptor.Hemisphere(HemisphereSkyColor,HemisphereGroundColor,0,false)));
        skyAmbient = engine.Graphics.CreateLight(new(SkyAmbientId,false,0,SkyAmbient(-1)));
        day = engine.Graphics.OpenResource(new("day.png",TextureFilter.Linear,TextureWrap.Clamp)).Handle;
        night = engine.Graphics.OpenResource(new("night.png",TextureFilter.Linear,TextureWrap.Clamp)).Handle;
        engine.CameraView.SetSkyBackgroundBlend(new(day,night,clock));
    }
    private static LightDescriptor Sun(float clock)
    {
        float elevation = float.DegreesToRadians(NoonElevation+(DuskElevation-NoonElevation)*clock), azimuth = float.DegreesToRadians(SunAzimuth);
        // The light travels from the sun, down toward the room.
        var travel = -new Vector3(MathF.Cos(elevation)*MathF.Sin(azimuth),MathF.Sin(elevation),MathF.Cos(elevation)*MathF.Cos(azimuth));
        return new(LightKind.Directional,Vector3.Lerp(NoonSunColor,DuskSunColor,clock),NoonSunIntensity+(DuskSunIntensity-NoonSunIntensity)*clock,true,Vector3.Zero,travel,false,0,0,0,0,LightShadowIntent.Requested);
    }
    // Half extent 0 leaves the Engine's default square (64 m a side); the light is disabled until asked for.
    private static LightDescriptor SkyAmbient(float halfExtent) => new(LightKind.Ambient,SkyAmbientColor,SkyAmbientIntensity,halfExtent>=0,Vector3.Zero,Vector3.UnitY,halfExtent>0,halfExtent,0,0,0,LightShadowIntent.Requested);
    private static CameraDescriptor Camera(Vector3 eye,Vector3 target)
    {
        CameraQueries.TryLookAtPose(eye,target,0,out CameraPose pose);
        return new(pose,CameraBasisMode.Derived,default,new(CameraProjectionKind.Perspective,CameraFov,0,CameraNear,CameraFar),CameraViewports.Full);
    }
    public void Start()
    {
        uint[] slots = new uint[ChunkEdge*ChunkEdge*ChunkEdge];
        for(int z=0;z<RoomWidth;z++) for(int y=0;y<RoomHeight;y++) for(int x=0;x<RoomWidth;x++)
            if((x==0||x==RoomWidth-1||y==0||y==RoomHeight-1||z==0||z==RoomWidth-1)&&!(z==RoomWidth-1&&x>=DoorMinX&&x<=DoorMaxX&&y>=DoorMinY&&y<=DoorMaxY)) slots[x+ChunkEdge*(y+ChunkEdge*z)] = 1;
        room = slots;
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
        room = restored.Room;
        descriptor=restored.Torch;
        engine.Graphics.UpdateLight(new(torch,new(TorchId,false,0,descriptor)));
        roundTrip=MathF.Abs(Sample(InsideSample)-litValue)<RoundTripTolerance;
        if(!roundTrip) throw new InvalidOperationException("light and scene persistence proof failed");
    }
    private float Sample(VoxelAddress address) => engine.Voxel.SampleDirectLighting(new(scene,address,CellCenter,Vector3.Zero,Horizon,new LightDescriptor[]{descriptor})).Luminance;
    private void SetTorch(bool enabled) { descriptor=descriptor with { Enabled=enabled }; engine.Graphics.UpdateLight(new(torch,new(TorchId,false,0,descriptor))); }
    [DebugCommand("lighting.torch")]
    public string Torch(bool enabled) { SetTorch(enabled); return Inspect(); }
    // The torch's fire (flame, embers and smoke) by the wall, and the camera on it; false puts it out and keeps the camera.
    [DebugCommand("lighting.torch.flame")]
    public string Flame(bool burning)
    {
        if (burning && flame is null)
        {
            flameSprite ??= engine.Graphics.OpenResource(new("flame.png",TextureFilter.Linear,TextureWrap.Clamp)).Handle;
            emberSprite ??= engine.Graphics.OpenResource(new("ember.png",TextureFilter.Linear,TextureWrap.Clamp)).Handle;
            smokeSprite ??= engine.Graphics.OpenResource(new("smoke.png",TextureFilter.Linear,TextureWrap.Clamp)).Handle;
            flameFire = Fire(FlameId,"lighting.flame",flameSprite,FlameFrames,FlameFramesPerSecond,24,.35f,.55f,new(-.08f,.5f,-.08f),new(.08f,.9f,.08f),Vector3.Zero,
                [new(0,.45f),new(.5f,.6f),new(1,.15f)],[new(0,new Color(1,.85f,.5f,1)),new(.6f,new Color(1,.45f,.1f,.7f)),new(1,new Color(.5f,.1f,0,0))],PresentationParticleBlendMode.Additive,FlameSoftness,32);
            emberFire = Fire(EmberId,"lighting.embers",emberSprite,1,0,10,1,2,new(-.35f,.8f,-.35f),new(.35f,1.6f,.35f),new(0,-.5f,0),
                [new(0,.06f),new(1,.02f)],[new(0,new Color(1,.6f,.2f,1)),new(1,new Color(1,.2f,0,0))],PresentationParticleBlendMode.Additive,EmberSoftness,48);
            smokeFire = Fire(SmokeId,"lighting.smoke",smokeSprite,1,0,5,2,3,new(-.25f,.45f,-.1f),new(.05f,.8f,.1f),Vector3.Zero,
                [new(0,.3f),new(1,1.1f)],[new(0,new Color(.3f,.27f,.25f,.45f)),new(1,new Color(.25f,.25f,.25f,0))],PresentationParticleBlendMode.Alpha,SmokeSoftness,32);
            flame = engine.Presentation.CreateEmitter(flameFire);
            embers = engine.Presentation.CreateEmitter(emberFire);
            smoke = engine.Presentation.CreateEmitter(smokeFire);
            engine.CameraView.UpdateCamera(new(camera,Camera(FireEye,FireTarget)));
        }
        else if (!burning) { flame?.Dispose(); embers?.Dispose(); smoke?.Dispose(); flame = embers = smoke = null; }
        return Inspect();
    }
    // Show (or clear) the flat-shading pair: a 6x12 smooth sphere and the tree with their authored normals on the left, and the same meshes under a flat-shaded material on the right.
    [DebugCommand("lighting.facets")]
    public string Facets(bool shown)
    {
        if (shown && sphereSmoothLook is null)
        {
            Color white = new(1,1,1,1);
            facetSmooth = engine.Graphics.CreateMaterial(new(FacetColor,default(RenderResourceReference),.9f,white,Vector3.Zero,0,false));
            facetFlat = engine.Graphics.CreateMaterial(new MaterialRequest(FacetColor,default(RenderResourceReference),.9f,white,Vector3.Zero,0,false) with { FlatShading = true });
            treeFlat = engine.Graphics.CreateMaterial(new MaterialRequest(TreeColor,default(RenderResourceReference),.9f,white,Vector3.Zero,0,false) with { FlatShading = true });
            sphereSmooth = engine.Graphics.CreateMeshResource(Sphere(facetSmooth));
            sphereFlat = engine.Graphics.CreateMeshResource(Sphere(facetFlat));
            sphereSmoothLook = engine.Graphics.CreateMeshAppearance(sphereSmooth);
            sphereFlatLook = engine.Graphics.CreateMeshAppearance(sphereFlat);
            using (ContentReference tree = engine.Content.OpenReference(new("tree.glb"))) treeMesh = engine.Animation.OpenAnimatedMeshFromContent(new(tree));
            treeSmoothLook = engine.Animation.CreateAnimatedMeshAppearance(new(treeMesh));
            treeFlatLook = engine.Animation.CreateAnimatedMeshAppearance(new(treeMesh));
            // The GLB's one material slot, rebound to the flat material: the override a product makes for an imported prop.
            engine.Animation.UpdateAnimatedMeshMaterials(new(treeFlatLook,new MeshMaterialBinding[]{new(0,treeFlat)}));
            engine.Graphics.PublishSnapshot(new AppearanceFact[]
            {
                Fact(SphereSmoothId,sphereSmoothLook,new(2.3f,1.7f,3.6f),.6f),
                Fact(SphereFlatId,sphereFlatLook,new(4.7f,1.7f,3.6f),.6f),
                Fact(TreeSmoothId,treeSmoothLook,new(1.2f,0,2.2f),1.6f),
                Fact(TreeFlatId,treeFlatLook,new(5.8f,0,2.2f),1.6f),
            });
            engine.CameraView.UpdateCamera(new(camera,Camera(FacetsEye,FacetsTarget)));
        }
        else if (!shown && sphereSmoothLook is not null)
        {
            engine.Graphics.PublishSnapshot([]);
            sphereSmoothLook.Dispose(); sphereFlatLook?.Dispose(); treeSmoothLook?.Dispose(); treeFlatLook?.Dispose();
            sphereSmooth?.Dispose(); sphereFlat?.Dispose(); treeMesh?.Dispose();
            facetSmooth?.Dispose(); facetFlat?.Dispose(); treeFlat?.Dispose();
            sphereSmoothLook = sphereFlatLook = treeSmoothLook = treeFlatLook = null; sphereSmooth = sphereFlat = null; treeMesh = null; facetSmooth = facetFlat = treeFlat = null;
        }
        return Inspect();
    }
    private static AppearanceFact Fact(ulong id,Appearance look,Vector3 at,float scale) => new(id,false,0,new Transform(at,Quaternion.Identity,new Vector3(scale)),look,true,RenderLayer.Scene);
    // A unit sphere of 6 rings and 12 segments with smooth normals: welded, as a low-poly export without split normals comes.
    private static MeshResourceCreateRequest Sphere(Material material)
    {
        const int rings = 6, segments = 12;
        List<Vector3> positions = [], normals = []; List<Vector2> uvs = []; List<uint> indices = [];
        for (int ring = 0; ring <= rings; ring++)
        for (int segment = 0; segment <= segments; segment++)
        {
            float theta = MathF.PI*ring/rings, phi = MathF.Tau*segment/segments;
            Vector3 n = new(MathF.Sin(theta)*MathF.Cos(phi),MathF.Cos(theta),MathF.Sin(theta)*MathF.Sin(phi));
            positions.Add(n); normals.Add(n); uvs.Add(new((float)segment/segments,(float)ring/rings));
        }
        for (uint ring = 0; ring < rings; ring++)
        for (uint segment = 0; segment < segments; segment++)
        {
            uint a = ring*(segments+1)+segment, b = a+segments+1;
            indices.AddRange([a,b,a+1,a+1,b,b+1]);
        }
        return new MeshResourceCreateRequest(positions.ToArray(),normals.ToArray(),uvs.ToArray(),indices.ToArray(),
            new MeshGroup[]{new(0,0,(uint)indices.Count)},new MeshMaterialBinding[]{new(0,material)});
    }
    // Scale the burning fire's softness: 0 gives every sheet a hard depth edge, 1 the authored softness.
    [DebugCommand("lighting.torch.softness")]
    public string Softness(float scale)
    {
        if (flame is null || embers is null || smoke is null) return Inspect();
        engine.Presentation.UpdateEmitter(flame,flameFire with { SoftnessMetres = FlameSoftness*scale });
        engine.Presentation.UpdateEmitter(embers,emberFire with { SoftnessMetres = EmberSoftness*scale });
        engine.Presentation.UpdateEmitter(smoke,smokeFire with { SoftnessMetres = SmokeSoftness*scale });
        return Inspect();
    }
    private static PresentationParticleDescriptor Fire(ulong id,string signal,RenderResource sprite,ushort frames,float framesPerSecond,float rate,float lifeMin,float lifeMax,Vector3 velocityMin,Vector3 velocityMax,Vector3 acceleration,
        PresentationParticleScalarKey[] size,PresentationParticleColorKey[] color,PresentationParticleBlendMode blend,float softness,uint max) => new()
    {
        LogicalId = id, SignalId = signal, Visible = true, Seed = id,
        Anchor = new() { Kind = PresentationAnchorKind.World, Position = FirePosition },
        Visual = PresentationParticleVisual.Billboard, Sprite = sprite, SpriteFrameCount = frames, FlipbookFramesPerSecond = framesPerSecond,
        SizeMode = PresentationParticleSizeMode.World, Blend = blend, SoftnessMetres = softness,
        RatePerSecond = rate, MaxParticles = max, LifetimeMinSeconds = lifeMin, LifetimeMaxSeconds = lifeMax,
        VelocityMin = velocityMin, VelocityMax = velocityMax, Acceleration = acceleration, SizeCurve = size, ColorCurve = color,
    };
    [DebugCommand("lighting.sky")]
    public string Sky(float amount) { clock=Math.Clamp(amount,0,1); engine.CameraView.SetSkyBackgroundBlend(new(day,night,clock)); engine.Graphics.UpdateLight(new(sun,new(SunId,false,0,Sun(clock)))); engine.CameraView.UpdateCamera(new(camera,Camera(SkyEye,SkyTarget))); return Inspect(); }
    [DebugCommand("lighting.atmosphere")]
    public string Atmosphere(bool enabled) { engine.CameraView.SetAtmosphere(enabled ? Air : default); return Inspect(); }
    // A hemisphere light at the given intensity (0 disables it), read back from the Engine.
    [DebugCommand("lighting.hemisphere")]
    public string Hemisphere(float intensity)
    {
        engine.Graphics.UpdateLight(new(hemisphere,new(HemisphereId,false,0,LightDescriptor.Hemisphere(HemisphereSkyColor,HemisphereGroundColor,intensity,intensity>0))));
        return JsonSerializer.Serialize(engine.Graphics.ReadLight(hemisphere),ProofJsonContext.Default.LightReadout);
    }
    // The sky's occluded ambient light over a square of the given half extent in metres (0 for the default square, below 0 to disable it).
    [DebugCommand("lighting.skyextent")]
    public string SkyExtent(float halfExtent)
    {
        engine.Graphics.UpdateLight(new(skyAmbient,new(SkyAmbientId,false,0,SkyAmbient(halfExtent))));
        return JsonSerializer.Serialize(engine.Graphics.ReadLight(skyAmbient),ProofJsonContext.Default.LightReadout);
    }
    // The indirect light volume over the room: "sky" lets the ambient light in only through the doorway and bounces the torch off the stone, "floor" keeps the ambient light everywhere and adds the bounce over it, "off" drops it.
    [DebugCommand("lighting.indirect")]
    public string Indirect(string mode)
    {
        engine.CameraView.SetIndirectLight(mode switch
        {
            "sky" => new(IndirectCenter,IndirectExtent,IndirectSpacing,IndirectBounces,IndirectAmbient.Sky),
            "floor" => new(IndirectCenter,IndirectExtent,IndirectSpacing,IndirectBounces,IndirectAmbient.Floor),
            _ => default,
        });
        return Inspect();
    }
    // Dig one voxel out of the room (chunk coordinates 0..7): the chunk re-meshes and only the probe bricks around it rebake.
    [DebugCommand("lighting.dig")]
    public string Dig(uint x,uint y,uint z)
    {
        room[x+ChunkEdge*(y+ChunkEdge*z)] = 0;
        VoxelResidencyOperation[] ops=[new(VoxelResidencyOperationKind.Replace,new(0,0,0),0,(uint)room.Length)];
        engine.Voxel.ApplyResidency(new(scene,ops,room));
        if(presentation is not null) engine.VoxelScenePresentation.RefreshScene(presentation);
        return Inspect();
    }
    // Move the indirect light volume's centre (sky mode): probes it still covers keep their values, only the newly covered bricks bake.
    [DebugCommand("lighting.follow")]
    public string Follow(float x,float y,float z)
    {
        engine.CameraView.SetIndirectLight(new(new Vector3(x,y,z),IndirectExtent,IndirectSpacing,IndirectBounces,IndirectAmbient.Sky));
        return Inspect();
    }
    [DebugCommand("lighting.cave")]
    public string Cave() { engine.CameraView.UpdateCamera(new(camera,Camera(CaveEye,CaveTarget))); return Inspect(); }
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
    // Renderer settings at runtime: read what draws, change one group, read again.
    [DebugCommand("lighting.settings")]
    public string Settings() => JsonSerializer.Serialize(engine.RendererSettings.Read(),ProofJsonContext.Default.RendererSettingsReadout);
    [DebugCommand("lighting.antialiasing")]
    public string Antialiasing(string samples)
    {
        Antialiasing level = samples switch { "off" => Rusty.Engine.Antialiasing.Off, "2x" => Rusty.Engine.Antialiasing.Msaa2, _ => Rusty.Engine.Antialiasing.Msaa4 };
        engine.RendererSettings.Set(engine.RendererSettings.Read().Requested with { Antialiasing = level });
        return Settings();
    }
    [DebugCommand("lighting.occlusion")]
    public string Occlusion(string mode,float strength,float radius)
    {
        AmbientOcclusionMode occlusion = mode switch { "screenSpace" => AmbientOcclusionMode.ScreenSpace, "distanceField" => AmbientOcclusionMode.DistanceField, _ => AmbientOcclusionMode.Disabled };
        engine.RendererSettings.Set(engine.RendererSettings.Read().Requested with { AmbientOcclusion = occlusion, AmbientOcclusionStrength = strength, AmbientOcclusionRadius = radius });
        return Settings();
    }
    [DebugCommand("lighting.scale")]
    public string Scale(float scale)
    {
        engine.RendererSettings.Set(engine.RendererSettings.Read().Requested with { RenderScale = scale });
        return Settings();
    }
    [DebugCommand("lighting.shadows")]
    public string Shadows(bool enabled,uint budget)
    {
        engine.RendererSettings.Set(engine.RendererSettings.Read().Requested with { Shadows = enabled, ShadowBudget = budget });
        return Settings();
    }
    [DebugCommand("lighting.pipeline")]
    public string Pipeline(bool clusteredLighting,bool gpuCulling,bool vsync)
    {
        engine.RendererSettings.Set(engine.RendererSettings.Read().Requested with { ClusteredLighting = clusteredLighting, GpuCulling = gpuCulling, Vsync = vsync });
        return Settings();
    }
    public void RegisterDebugCommands(IDebugCommandModuleRegistrar registrar)=>registrar.Register(this);
    public ProductUpdateResult Update(ProductUpdate update)=>ProductUpdateResult.None;
    public void Pause(){} public void Resume(){} public void Restart(){SetTorch(true);} public void Shutdown(){}
    public void Dispose(){engine.CameraView.ClearSkyBackground(default); Flame(false); Facets(false); flameSprite?.Dispose(); emberSprite?.Dispose(); smokeSprite?.Dispose(); presentation?.Dispose(); torch.Dispose(); sun.Dispose(); hemisphere.Dispose(); skyAmbient.Dispose(); camera.Dispose(); stone.Dispose(); scene.Dispose(); day.Dispose(); night.Dispose();}
}
internal sealed record LightingSave(uint[] Room,LightDescriptor Torch);
internal sealed record LightingProof(bool RoundTrip,float Lit,float Blocked,float Dark,float Current,bool Torch,float Clock);
[JsonSourceGenerationOptions(PropertyNamingPolicy=JsonKnownNamingPolicy.CamelCase,IncludeFields=true,UseStringEnumConverter=true)]
[JsonSerializable(typeof(LightingSave))]
[JsonSerializable(typeof(LightingProof))]
[JsonSerializable(typeof(RendererSettingsReadout))]
[JsonSerializable(typeof(LightReadout))]
internal partial class ProofJsonContext : JsonSerializerContext { }
