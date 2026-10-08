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
    // The room's far corner from inside, where the walls, floor and ceiling meet: the cube-only vertex occlusion view.
    private static readonly Vector3 CornerEye = new(5.5f,2.2f,5.5f), CornerTarget = new(1.4f,1.6f,1.4f);
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
    // The cloud layer: its drift over the ground (m/s), altitude and cloud size (m), and an untinted light.
    private static readonly Vector2 CloudDrift = new(8, 3);
    private const float CloudAltitude = 1200, CloudScale = 500;
    private static readonly Vector3 CloudTint = Vector3.One, CloudsEye = new(3.5f,8,14), CloudsTarget = new(3.5f,14,-10);
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
    // Flat shading (#9543): a smooth low-poly sphere and the low-poly tree, each twice: authored normals on the left, the flat-shading feature on the right, by the torch. The trees stand on the floor at the room's sides and the spheres between them, so nothing hides either pair.
    private const ulong SphereSmoothId = 20, SphereFlatId = 21, TreeSmoothId = 22, TreeFlatId = 23;
    private static readonly Color FacetColor = new(.75f,.7f,.6f,1), TreeColor = new(.4f,.6f,.3f,1);
    private static readonly Vector3 FacetsEye = new(3.5f,2.3f,6.9f), FacetsTarget = new(3.5f,1.7f,3f);
    private Material? facetSmooth, facetFlat, treeFlat;
    private MeshResource? sphereSmooth, sphereFlat;
    private RenderResource? treeMesh;
    private Appearance? sphereSmoothLook, sphereFlatLook, treeSmoothLook, treeFlatLook;
    // The wind (#9541): the tree with a wind bend, grass cards whose vertex alpha weights their flutter, and a banner a product displace stage waves, all on the room's floor by the torch.
    private const ulong WindTreeId = 30, GrassBaseId = 31, BannerId = 40, PoleId = 41;
    private const int GrassCards = 6;
    private const float TreeWindBend = .12f, GrassWindBend = .25f, GrassWindFlutter = .05f, GrassHeight = .55f, BannerWaveAmplitude = .3f, BannerWaveRate = 5f, WindGust = .6f;
    private static readonly Vector2 WindDirection = new(1, .25f);
    private static readonly Color GrassColor = new(.35f,.6f,.2f,1), BannerColor = new(.8f,.25f,.2f,1), PoleColor = new(.4f,.3f,.2f,1);
    private static readonly Vector3 WindEye = new(4.2f,2.3f,6.7f), WindTarget = new(4,1.5f,3);
    private Material? treeWind, grass, banner, pole;
    private MeshResource? grassMesh, bannerMesh, poleMesh;
    private RenderResource? windTreeMesh, waveShader;
    private Appearance? windTreeLook, grassLook, bannerLook, poleLook;
    // The shore (#9540): a dual-contoured sand bank sloping into a water slab under the sun, with a cube pier standing in it; seen from the bank.
    private const ulong WaterId = 50, PierBaseId = 51;
    private const uint ShoreChunkEdge = 16, SandSlot = 1;
    private const float ShoreVoxelSize = .5f, ShoreMinimumDensity = .001f;
    // The shore's session lives beside the room, its chunks from x 16 (chunk 2 of 8 m): the sand falls from y 4 at the near bank to the bed across z, and the water stands at y 2.
    private static readonly Vector3 ShoreOrigin = new(16, 0, 0);
    private const long ShoreFirstChunkX = 2;
    private const float ShoreSlope = .22f, ShoreBankHeight = 4f, WaterLevel = 2.05f, WaterSize = 24;
    private const int PierPosts = 3;
    private static readonly Color SandColor = new(.78f,.7f,.5f,1), PierColor = new(.45f,.32f,.2f,1), WaterTint = new(1,1,1,.35f);
    private static MaterialWater ShoreWater(RenderResource foam, RenderResource ripples) => new(new(.18f,.55f,.5f,1), new(.01f,.08f,.2f,1), 1.8f, .35f, .55f, new(.03f,.02f), new(.04f,.03f), new(-.02f,.035f), 3f, foam, ripples);
    private static readonly Vector3 ShoreEye = new(22, 5.5f, 3), ShoreTarget = new(28, 1.6f, 15);
    private SpatialSession? shore;
    private VoxelScenePresentation? shorePresentation;
    private Material? sand, water, pier;
    private RenderResource? foamSprite, ripplesMap;
    private MeshResource? waterMesh, pierMesh;
    private Appearance? waterLook, pierLook;
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
                Fact(SphereSmoothId,sphereSmoothLook,new(2.9f,1.6f,3.4f),.5f),
                Fact(SphereFlatId,sphereFlatLook,new(4.1f,1.6f,3.4f),.5f),
                Fact(TreeSmoothId,treeSmoothLook,new(1.6f,1,2.6f),1.2f),
                Fact(TreeFlatId,treeFlatLook,new(5.4f,1,2.6f),1.2f),
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
    // Blow the wind at the given strength over the tree, the grass and the banner (0 stills it; below 0 clears the scene), with the camera on them.
    [DebugCommand("lighting.wind")]
    public string Wind(float strength)
    {
        if (strength < 0)
        {
            if (windTreeLook is null) return Inspect();
            engine.CameraView.SetWind(new(WindDirection,0,WindGust));
            engine.Graphics.PublishSnapshot([]);
            windTreeLook.Dispose(); grassLook?.Dispose(); bannerLook?.Dispose(); poleLook?.Dispose();
            grassMesh?.Dispose(); bannerMesh?.Dispose(); poleMesh?.Dispose(); windTreeMesh?.Dispose();
            treeWind?.Dispose(); grass?.Dispose(); banner?.Dispose(); pole?.Dispose(); waveShader?.Dispose();
            windTreeLook = grassLook = bannerLook = poleLook = null; grassMesh = bannerMesh = poleMesh = null; windTreeMesh = waveShader = null; treeWind = grass = banner = pole = null;
            return Inspect();
        }
        if (windTreeLook is null)
        {
            Color white = new(1,1,1,1);
            treeWind = engine.Graphics.CreateMaterial(new MaterialRequest(TreeColor,default(RenderResourceReference),.9f,white,Vector3.Zero,0,false) with { FlatShading = true, WindBend = TreeWindBend });
            grass = engine.Graphics.CreateMaterial(new MaterialRequest(GrassColor,default(RenderResourceReference),.9f,white,Vector3.Zero,0,true) with { WindBend = GrassWindBend, WindFlutter = GrassWindFlutter });
            waveShader = engine.Graphics.OpenResource(new RenderResourceRequest("wave.wgsl")).Handle;
            banner = engine.Graphics.CreateMaterial(new MaterialRequest(BannerColor,default(RenderResourceReference),.8f,white,Vector3.Zero,0,true) with { Shader = new MaterialShader(waveShader,new Vector4(BannerWaveAmplitude,BannerWaveRate,0,0)) });
            pole = engine.Graphics.CreateMaterial(new(PoleColor,default(RenderResourceReference),.9f,white,Vector3.Zero,0,false));
            using (ContentReference tree = engine.Content.OpenReference(new("tree.glb"))) windTreeMesh = engine.Animation.OpenAnimatedMeshFromContent(new(tree));
            windTreeLook = engine.Animation.CreateAnimatedMeshAppearance(new(windTreeMesh));
            engine.Animation.UpdateAnimatedMeshMaterials(new(windTreeLook,new MeshMaterialBinding[]{new(0,treeWind)}));
            grassMesh = engine.Graphics.CreateMeshResource(Grass(grass));
            bannerMesh = engine.Graphics.CreateMeshResource(Card(banner,.9f,.5f,0,1));
            poleMesh = engine.Graphics.CreateMeshResource(Card(pole,.06f,2.4f,0,0));
            grassLook = engine.Graphics.CreateMeshAppearance(grassMesh);
            bannerLook = engine.Graphics.CreateMeshAppearance(bannerMesh);
            poleLook = engine.Graphics.CreateMeshAppearance(poleMesh);
            engine.Graphics.PublishSnapshot(new AppearanceFact[]
            {
                Fact(WindTreeId,windTreeLook,new(2.1f,1,3),1.3f),
                Fact(GrassBaseId,grassLook,new(4.3f,1,3.2f),1),
                Fact(PoleId,poleLook,new(6.3f,1,2.8f),1),
                Fact(BannerId,bannerLook,new(6.3f,2.85f,2.8f),1),
            });
            engine.CameraView.UpdateCamera(new(camera,Camera(WindEye,WindTarget)));
        }
        engine.CameraView.SetWind(new(WindDirection,strength,WindGust));
        return Inspect();
    }
    // Show (or clear) the shore: the sand bank, the water slab (depth tint, foam at the bank and the pier, scrolling ripples, Fresnel) and the pier; 0 builds the slab without the water feature, so the plain blended material shows for comparison; below 0 clears the scene.
    [DebugCommand("lighting.water")]
    public string Water(float mode)
    {
        if (mode < 0 || shore is not null)
        {
            if (shore is null) return Inspect();
            engine.Graphics.PublishSnapshot([]);
            waterLook?.Dispose(); pierLook?.Dispose(); waterMesh?.Dispose(); pierMesh?.Dispose();
            shorePresentation?.Dispose(); shore.Dispose();
            water?.Dispose(); pier?.Dispose(); sand?.Dispose(); foamSprite?.Dispose(); ripplesMap?.Dispose();
            waterLook = pierLook = null; waterMesh = pierMesh = null; shorePresentation = null; shore = null; water = pier = sand = null; foamSprite = ripplesMap = null;
            if (mode < 0) return Inspect();
        }
        Color white = new(1,1,1,1);
        ripplesMap ??= engine.Graphics.OpenResource(new("ripples.png",TextureFilter.Linear,TextureWrap.Repeat,TextureColorSpace.Linear)).Handle;
        foamSprite ??= engine.Graphics.OpenResource(new("foam.png",TextureFilter.Linear,TextureWrap.Repeat,TextureColorSpace.Linear)).Handle;
        // The ripple map twice: as the material's normal map and as the water's second, scrolled apart.
        MaterialRequest slab = new MaterialRequest(WaterTint,default(RenderResourceReference),.08f,white,Vector3.Zero,0,true,MaterialAlphaMode.Blend,0) with
        {
            NormalMap = ripplesMap,
            NormalScale = .6f,
            Water = mode > 0 ? ShoreWater(foamSprite,ripplesMap) : default,
        };
        {
            sand = engine.Graphics.CreateMaterial(new(SandColor,default(RenderResourceReference),.95f,white,Vector3.Zero,0,false));
            pier = engine.Graphics.CreateMaterial(new(PierColor,default(RenderResourceReference),.9f,white,Vector3.Zero,0,false));
            water = engine.Graphics.CreateMaterial(slab);
            shore = engine.Spatial.CreateSession(new(ShoreVoxelSize,ShoreChunkEdge,VoxelSurfaceMode.DualContouring));
            AdmitShore();
            shorePresentation = engine.VoxelScenePresentation.ProjectScene(new(shore,new VoxelSceneMaterialBinding[]{new(SandSlot,sand)}));
            waterMesh = engine.Graphics.CreateMeshResource(Slab(water,WaterSize));
            pierMesh = engine.Graphics.CreateMeshResource(Pier(pier));
            waterLook = engine.Graphics.CreateMeshAppearance(waterMesh);
            pierLook = engine.Graphics.CreateMeshAppearance(pierMesh);
            engine.Graphics.PublishSnapshot(new AppearanceFact[]
            {
                Fact(WaterId,waterLook,ShoreOrigin + new Vector3(WaterSize/2,WaterLevel,WaterSize/2),1),
                Fact(PierBaseId,pierLook,ShoreOrigin + new Vector3(13,0,10),1),
            });
            engine.CameraView.UpdateCamera(new(camera,Camera(ShoreEye,ShoreTarget)));
        }
        return Inspect();
    }
    // The bank: sand whose surface falls from the bank height along z (toward the far side) with the slope, dual contoured at half-metre voxels over a 24 m square, 8 m high.
    private void AdmitShore()
    {
        const int across = 3, high = 1;
        int volume = (int)(ShoreChunkEdge*ShoreChunkEdge*ShoreChunkEdge);
        List<VoxelResidencyOperation> operations = [];
        uint[] materials = new uint[across*across*high*volume];
        float[] densities = new float[materials.Length];
        int next = 0;
        for (long cz = 0; cz < across; cz++)
        for (long cy = 0; cy < high; cy++)
        for (long cx = ShoreFirstChunkX; cx < ShoreFirstChunkX + across; cx++)
        {
            uint offset = (uint)next;
            for (long z = 0; z < ShoreChunkEdge; z++)
            for (long y = 0; y < ShoreChunkEdge; y++)
            for (long x = 0; x < ShoreChunkEdge; x++)
            {
                float worldZ = (cz*ShoreChunkEdge + z + .5f)*ShoreVoxelSize, worldY = (cy*ShoreChunkEdge + y + .5f)*ShoreVoxelSize, worldX = (cx*ShoreChunkEdge + x + .5f)*ShoreVoxelSize;
                float surface = Math.Max(ShoreBankHeight - ShoreSlope*worldZ + .25f*MathF.Sin(worldX*.9f)*MathF.Sin(worldZ*.7f), .4f);
                float distance = (worldY - surface)/ShoreVoxelSize;
                bool solid = distance < 0;
                materials[next] = solid ? SandSlot : 0;
                densities[next] = solid ? Math.Min(distance,-ShoreMinimumDensity) : Math.Max(distance,ShoreMinimumDensity);
                next++;
            }
            operations.Add(new(VoxelResidencyOperationKind.Admit,new(cx,cy,cz),offset,(uint)volume,offset,(uint)volume));
        }
        engine.Voxel.ApplyResidency(new(ReadOnlyMemory<uint>.Empty,shore!,operations.ToArray(),materials,densities));
    }
    // A horizontal slab `size` across centred on its origin, facing up, with uvs over it.
    private static MeshResourceCreateRequest Slab(Material material, float size)
    {
        float h = size/2;
        Vector3[] positions = [new(-h,0,-h), new(h,0,-h), new(h,0,h), new(-h,0,h)];
        Vector3[] normals = [Vector3.UnitY, Vector3.UnitY, Vector3.UnitY, Vector3.UnitY];
        Vector2[] uvs = [new(0,0), new(1,0), new(1,1), new(0,1)];
        return new MeshResourceCreateRequest(positions, normals, uvs, new uint[]{0,2,1,0,3,2}, new MeshGroup[]{new(0,0,6)}, new MeshMaterialBinding[]{new(0,material)});
    }
    // A pier: a plank deck on posts standing in the water, built from boxes.
    private static MeshResourceCreateRequest Pier(Material material)
    {
        List<Vector3> positions = [], normals = []; List<Vector2> uvs = []; List<uint> indices = [];
        void Box(Vector3 min, Vector3 max)
        {
            Vector3[][] faces =
            [
                [new(min.X,min.Y,max.Z), new(max.X,min.Y,max.Z), new(max.X,max.Y,max.Z), new(min.X,max.Y,max.Z)],
                [new(max.X,min.Y,min.Z), new(min.X,min.Y,min.Z), new(min.X,max.Y,min.Z), new(max.X,max.Y,min.Z)],
                [new(max.X,min.Y,max.Z), new(max.X,min.Y,min.Z), new(max.X,max.Y,min.Z), new(max.X,max.Y,max.Z)],
                [new(min.X,min.Y,min.Z), new(min.X,min.Y,max.Z), new(min.X,max.Y,max.Z), new(min.X,max.Y,min.Z)],
                [new(min.X,max.Y,max.Z), new(max.X,max.Y,max.Z), new(max.X,max.Y,min.Z), new(min.X,max.Y,min.Z)],
                [new(min.X,min.Y,min.Z), new(max.X,min.Y,min.Z), new(max.X,min.Y,max.Z), new(min.X,min.Y,max.Z)],
            ];
            Vector3[] faceNormals = [Vector3.UnitZ, -Vector3.UnitZ, Vector3.UnitX, -Vector3.UnitX, Vector3.UnitY, -Vector3.UnitY];
            for (int face = 0; face < 6; face++)
            {
                uint first = (uint)positions.Count;
                positions.AddRange(faces[face]);
                normals.AddRange([faceNormals[face], faceNormals[face], faceNormals[face], faceNormals[face]]);
                uvs.AddRange([new(0,1), new(1,1), new(1,0), new(0,0)]);
                indices.AddRange([first, first+1, first+2, first, first+2, first+3]);
            }
        }
        for (int post = 0; post < PierPosts; post++)
        {
            float z = post*2.2f;
            Box(new(-1.1f,0,z-.15f), new(-.8f,2.6f,z+.15f));
            Box(new(.8f,0,z-.15f), new(1.1f,2.6f,z+.15f));
        }
        Box(new(-1.3f,2.6f,-.6f), new(1.3f,2.85f,(PierPosts-1)*2.2f+.6f));
        return new MeshResourceCreateRequest(positions.ToArray(), normals.ToArray(), uvs.ToArray(), indices.ToArray(), new MeshGroup[]{new(0,0,(uint)indices.Count)}, new MeshMaterialBinding[]{new(0,material)});
    }
    // A clump of grass: cards crossed at angles over a square metre, each 0.3 m wide and GrassHeight tall, its vertex alpha 0 at the root and 1 at the tip.
    private static MeshResourceCreateRequest Grass(Material material)
    {
        List<Vector3> positions = [], normals = []; List<Vector2> uvs = []; List<Color> colors = []; List<uint> indices = [];
        Random placement = new(9541);
        for (int card = 0; card < GrassCards; card++)
        {
            float angle = card * MathF.PI / GrassCards, x = (float)placement.NextDouble() - .5f, z = (float)placement.NextDouble() - .5f;
            Vector3 across = new(MathF.Cos(angle) * .15f, 0, MathF.Sin(angle) * .15f), normal = new(-MathF.Sin(angle), 0, MathF.Cos(angle)), root = new(x, 0, z);
            uint first = (uint)positions.Count;
            positions.AddRange([root - across, root + across, root + across + new Vector3(0, GrassHeight, 0), root - across + new Vector3(0, GrassHeight, 0)]);
            normals.AddRange([normal, normal, normal, normal]);
            uvs.AddRange([new(0, 1), new(1, 1), new(1, 0), new(0, 0)]);
            colors.AddRange([new(1, 1, 1, 0), new(1, 1, 1, 0), new(1, 1, 1, 1), new(1, 1, 1, 1)]);
            indices.AddRange([first, first + 1, first + 2, first, first + 2, first + 3]);
        }
        return new MeshResourceCreateRequest(positions.ToArray(), normals.ToArray(), uvs.ToArray(), colors.ToArray(), indices.ToArray(),
            new MeshGroup[]{new(0,0,(uint)indices.Count)}, new MeshMaterialBinding[]{new(0,material)});
    }
    // A card in the xy plane facing +z, `width` across from its origin (uv.x 0 at the origin's edge) and `height` tall, its vertex alpha `rootAlpha` along the bottom and `tipAlpha` along the top.
    private static MeshResourceCreateRequest Card(Material material, float width, float height, float rootAlpha, float tipAlpha)
    {
        Vector3[] positions = [new(0, 0, 0), new(width, 0, 0), new(width, height, 0), new(0, height, 0)];
        Vector3[] normals = [Vector3.UnitZ, Vector3.UnitZ, Vector3.UnitZ, Vector3.UnitZ];
        Vector2[] uvs = [new(0, 1), new(1, 1), new(1, 0), new(0, 0)];
        Color[] colors = [new(1, 1, 1, rootAlpha), new(1, 1, 1, rootAlpha), new(1, 1, 1, tipAlpha), new(1, 1, 1, tipAlpha)];
        return new MeshResourceCreateRequest(positions, normals, uvs, colors, new uint[]{0, 1, 2, 0, 2, 3}, new MeshGroup[]{new(0,0,6)}, new MeshMaterialBinding[]{new(0,material)});
    }
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
    // A drifting cloud layer over the panorama at the given coverage (0 to 1; 0 clears it), with the camera up toward it; lighting.sky moves the sun that lights it from noon to dusk.
    [DebugCommand("lighting.clouds")]
    public string Clouds(float coverage)
    {
        engine.CameraView.SetClouds(new(Math.Clamp(coverage,0,1),CloudDrift,CloudAltitude,CloudScale,CloudTint));
        engine.CameraView.UpdateCamera(new(camera,Camera(CloudsEye,CloudsTarget)));
        return Inspect();
    }
    // How wet the room's lit surfaces are after rain (0 to 1; 0 dries them), with puddles on the floor (0 to 1).
    [DebugCommand("lighting.wetness")]
    public string Wetness(float wetness, float puddles)
    {
        engine.CameraView.SetWetness(new(Math.Clamp(wetness,0,1),Math.Clamp(puddles,0,1)));
        return Inspect();
    }
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
    // Vertex occlusion (#9506): darken the room's cube vertices by the solid voxels around them at the given strength (0 off, 1 full), with the camera on the room's corner.
    [DebugCommand("lighting.vertexocclusion")]
    public string VertexOcclusion(float strength)
    {
        VoxelSceneReadout readout = engine.Voxel.ConfigureVertexOcclusion(new VoxelVertexOcclusionRequest(scene,strength));
        if(presentation is not null) engine.VoxelScenePresentation.RefreshScene(presentation);
        engine.CameraView.UpdateCamera(new(camera,Camera(CornerEye,CornerTarget)));
        return JsonSerializer.Serialize(readout,ProofJsonContext.Default.VoxelSceneReadout);
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
    public void Dispose(){engine.CameraView.ClearSkyBackground(default); Flame(false); Facets(false); Wind(-1); Water(-1); flameSprite?.Dispose(); emberSprite?.Dispose(); smokeSprite?.Dispose(); presentation?.Dispose(); torch.Dispose(); sun.Dispose(); hemisphere.Dispose(); skyAmbient.Dispose(); camera.Dispose(); stone.Dispose(); scene.Dispose(); day.Dispose(); night.Dispose();}
}
internal sealed record LightingSave(uint[] Room,LightDescriptor Torch);
internal sealed record LightingProof(bool RoundTrip,float Lit,float Blocked,float Dark,float Current,bool Torch,float Clock);
[JsonSourceGenerationOptions(PropertyNamingPolicy=JsonKnownNamingPolicy.CamelCase,IncludeFields=true,UseStringEnumConverter=true)]
[JsonSerializable(typeof(LightingSave))]
[JsonSerializable(typeof(LightingProof))]
[JsonSerializable(typeof(RendererSettingsReadout))]
[JsonSerializable(typeof(LightReadout))]
[JsonSerializable(typeof(VoxelSceneReadout))]
internal partial class ProofJsonContext : JsonSerializerContext { }
