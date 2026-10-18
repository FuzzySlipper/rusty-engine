using System.Diagnostics;
using System.Numerics;
using Rusty.Engine;
using Rusty.Engine.Debugging;
using Rusty.Engine.Heightfields;

namespace CsharpHeightfield;

/// <summary>
/// Rolling hills, ridges and a lake drawn by the Engine's heightfield service (#9842): three rings
/// of tiles about the camera, sampled by the product on worker threads through
/// <see cref="HeightfieldStreamer"/>, meshed and geomorphed by the Engine. The camera flies a
/// circle by itself, holding its height above the ground with <c>Heightfield.Pick</c>. The ground
/// draws with linear vertex colours, or with a two-layer terrain material blended by slope; the
/// lake is its own unblended material.
/// </summary>
public sealed class Product : IEngineProduct, IDebugCommandModuleSource, IDebugCommandModule
{
    private const uint TileCells = 32;
    // 1 m cells within 48 m, 4 m within 400 m, 32 m within 4 km: tiles of 32 m, 128 m and 1 km.
    private static readonly HeightfieldRing[] Rings = [new(1, 48), new(4, 400), new(32, 4000)];
    private const uint GroundIndex = 0, WaterIndex = 1;
    private const int CommitsPerUpdate = 12;
    private const double FlightRadius = 1500, FlightSpeed = 40, FlightHeight = 45, LookDown = 14, PickFrom = 5000;
    private const double FieldOfView = 60, NearClip = .5, FarClip = 6000;
    private const float FogDensity = 1f / 5000, ContinentFogDensity = 1f / 60_000;
    private static readonly Color FogColor = new(.62f, .7f, .8f, 1), WaterColor = new(.05f, .16f, .24f, 1);
    private const float LayerContrast = 2, LayerTileCells = 4;
    // The continent (#9842 cost): a 390 km map sampled in metres, drawn at 1:1000 in the backdrop,
    // in rings of 31.25 m, 250 m and 1 km cells as CraftSurvive's horizon tiers (8 km, 64 km, the
    // whole continent), or halving rings from 31.25 m.
    private const double BackdropScale = 1000, ContinentRadius = 195_000;
    private static readonly HeightfieldRing[] Tiers = [new(31.25, 4_000), new(250, 32_000), new(1_000, ContinentRadius)];
    private static readonly HeightfieldRing[] Halving =
        [new(31.25, 4_000), new(62.5, 8_000), new(125, 16_000), new(250, 32_000), new(500, 64_000), new(1_000, ContinentRadius)];
    private const uint DefaultContinentTileCells = 16;
    // Snow (#10069): the sky's occluded ambient light keeps the ground under trees clear over a
    // square this many metres either side of the camera, and lights it a little.
    private const ulong SkyLightId = 1;
    private const float SkyHalfExtent = 96, SkyIntensity = .05f;
    private static readonly Vector3 SkyColor = new(.6f, .7f, .9f), SnowColor = new(.82f, .86f, .92f);
    // Decals (#10070): marks from the 2x2 `marks.png` atlas scattered on a spiral about where the camera
    // looks, each box this many times as tall as it is wide so it reaches over the slopes under it.
    private const float DecalDepth = 1.5f, DecalOpacity = .9f, DecalSpiralMetres = .9f;
    private static readonly double GoldenAngle = Math.PI * (3 - Math.Sqrt(5));
    private const double ContinentFlightRadius = 100_000, ContinentFlightSpeed = 300, ContinentFlightHeight = 400;

    private readonly IEngineContext engine;
    private readonly Camera camera;
    private readonly Material white, water, grass, rock, layered;
    private readonly RenderResource grassTexture, rockTexture;
    private readonly AuthoredCatalog catalog;
    private Heightfield? heightfield;
    private HeightfieldStreamer? streamer;
    private readonly Vegetation vegetation;
    private bool layers;
    /// <summary>0: the hills in the scene; 1: the continent in tiers; 2: the continent in halving rings.</summary>
    private int continent;
    private long settleStarted;
    private double? flightSpeed;
    private uint continentTileCells = DefaultContinentTileCells;
    private double settleSeconds = double.NaN;
    private bool flying = true;
    private double angle;
    private double eyeHeight = FlightHeight + 100;
    private Vector3 eye;
    private Light? sky;
    private RenderResource? marks;
    private ulong decals;
    private double flightHeight = FlightHeight;

    public Product(ProductCreateContext context)
    {
        engine = context.Engine;
        Color one = new(1, 1, 1, 1);
        white = engine.Graphics.CreateMaterial(new MaterialRequest(one, default, 1, one, Vector3.Zero, 0, false));
        water = engine.Graphics.CreateMaterial(new MaterialRequest(WaterColor, default, .15f, one, Vector3.Zero, 0, false));
        grassTexture = engine.Graphics.OpenResource(new("grass.png", TextureFilter.Linear, TextureWrap.Repeat)).Handle;
        rockTexture = engine.Graphics.OpenResource(new("rock.png", TextureFilter.Linear, TextureWrap.Repeat)).Handle;
        catalog = engine.AuthoredContent.AdmitCatalogPayload(Catalog());
        grass = engine.Graphics.CreateAuthoredMaterial(new(catalog, GrassMaterial, grassTexture));
        rock = engine.Graphics.CreateAuthoredMaterial(new(catalog, RockMaterial, rockTexture));
        layered = engine.Graphics.CreateTerrainLayerMaterial(new(grass, new[] { rock }, LayerContrast));
        camera = engine.CameraView.CreateCamera(Descriptor(new(0, 200, 0), 0));
        engine.CameraView.SetActiveCamera(camera);
        engine.CameraView.SetBackgroundColor(new(FogColor));
        vegetation = new Vegetation(engine, camera);
    }

    public void Start() => Build();

    /// <summary>The heightfield and its streamer, drawn with colours or with layers.</summary>
    private void Build()
    {
        streamer?.Dispose();
        heightfield?.Dispose();
        HeightfieldMaterial[] materials = [new(GroundIndex, layers ? layered : white), new(WaterIndex, water)];
        engine.CameraView.SetFog(new(FogMode.ExponentialSquared, FogColor, 0, 0, continent == 0 ? FogDensity : ContinentFogDensity));
        if (continent == 0)
        {
            engine.CameraView.ClearBackdrop(new(default));
            heightfield = engine.Heightfield.Create(new HeightfieldCreateRequest(TileCells, Rings, materials, RenderLayer.Scene,
                Vector3.Zero, 1, HeightfieldFocus.Following(camera)));
        }
        else
        {
            engine.CameraView.SetBackdrop(new(default, default, default, Vector3.Zero, (float)BackdropScale));
            heightfield = engine.Heightfield.Create(new HeightfieldCreateRequest(continentTileCells, continent == 1 ? Tiers : Halving,
                materials, RenderLayer.Backdrop, Vector3.Zero, (float)(1 / BackdropScale), HeightfieldFocus.Following(camera)));
        }

        streamer = new HeightfieldStreamer(engine, heightfield, new Ground(layers, continent == 0 ? 1 : 25), CommitsPerUpdate);
        // Grass and trees grow over the hills, not the continent.
        vegetation.Show(continent == 0);
        Settling();
    }

    /// <summary>Times how long the rings take to draw everything they want from now.</summary>
    private void Settling()
    {
        settleStarted = Stopwatch.GetTimestamp();
        settleSeconds = double.NaN;
    }

    public ProductUpdateResult Update(ProductUpdate update)
    {
        double seconds = update.Facts.FixedDeltaSeconds * update.Facts.AdmittedStepCount;
        if (flying) angle += seconds * Speed / Radius;
        Fly();
        streamer!.Update();
        vegetation.Update();
        if (double.IsNaN(settleSeconds) && engine.Heightfield.Read(heightfield!).Settled)
            settleSeconds = Stopwatch.GetElapsedTime(settleStarted).TotalSeconds;
        return ProductUpdateResult.None;
    }

    private double Radius => continent == 0 ? FlightRadius : ContinentFlightRadius;
    private double Speed => flightSpeed ?? (continent == 0 ? FlightSpeed : ContinentFlightSpeed);
    private double Scale => continent == 0 ? 1 : 1 / BackdropScale;

    /// <summary>Along the circle, looking ahead, at the flight height over the drawn ground.</summary>
    private void Fly()
    {
        double x = Math.Cos(angle) * Radius, z = Math.Sin(angle) * Radius;
        // Picked in the heightfield's layer: the backdrop's units for the continent.
        HeightfieldPickResult ground = engine.Heightfield.Pick(new(heightfield!,
            new((float)(x * Scale), (float)(PickFrom * 10 * Scale), (float)(z * Scale)), -Vector3.UnitY, 0));
        if (ground.Hit) eyeHeight = ground.Height + (continent == 0 ? flightHeight : ContinentFlightHeight);
        eye = new((float)x, (float)eyeHeight, (float)z);
        double heading = flying ? angle + (Math.PI / 2) : angle;
        engine.CameraView.UpdateCamera(new(camera, Descriptor(eye, heading)));
        if (sky is not null) engine.Graphics.UpdateLight(new(sky, Sky(eye)));
    }

    /// <summary>The sky's ambient light over the camera, shadowed by what stands above the ground.</summary>
    private static LightRequest Sky(Vector3 at) => new(SkyLightId, false, 0,
        new(LightKind.Ambient, SkyColor, SkyIntensity, true, at, Vector3.UnitY, true, SkyHalfExtent, 0, 0, 0, LightShadowIntent.Requested));

    private static CameraDescriptor Descriptor(Vector3 eye, double heading)
    {
        Vector3 ahead = new((float)Math.Cos(heading), -(float)Math.Tan(LookDown * Math.PI / 180), (float)Math.Sin(heading));
        CameraQueries.TryLookAtPose(eye, eye + ahead, 0, out CameraPose pose);
        return new(pose, CameraBasisMode.Derived, default,
            new(CameraProjectionKind.Perspective, (float)FieldOfView, 0, (float)NearClip, (float)FarClip), CameraViewports.Full);
    }

    private const string GrassTexture = "texture/grass", RockTexture = "texture/rock";
    private const string GrassMaterial = "material/grass", RockMaterial = "material/rock";
    private const uint Version = 1, TextureSize = 64;
    // The textures' SHA-256, which pins the surfaces to them (`generate-textures.py` writes them).
    private const string GrassHash = "6e717ed4ecaf3f414922a895f44346c5a834c8a8f3d519ca745e9b9583b029c3", RockHash = "abccb3161941412ed498797508d3eac39bc13c79a041bcb3a68c0439a35b02ff";

    /// <summary>Two repeat-mapped voxel surface materials, the terrain layers.</summary>
    private static AuthoredCatalogPayloadAdmitRequest Catalog()
    {
        static AuthoredCatalogEntryInput Texture(string id, string path, string hash) => new(id, Version, true, hash, true, path, false, "");
        static AuthoredCatalogEntryInput Entry(string id) => new(id, Version, false, "", false, "", false, "");
        static AuthoredMaterialInput Surface(string id, string texture, string hash) => new(id, true, true, true, AuthoredStructuralClass.Solid,
            new(1, 1, 1, 1), true, texture, AssetVersionRequirementKind.Exact, Version, true, hash, 1, new(1, 1, 1, 1), new(0, 0, 0, 1), 0,
            AuthoredUvStrategy.Flat);
        static AuthoredVoxelSurfaceInput Repeat(string material, string texture, string hash) => new(material, 1, AuthoredVoxelSurfaceMappingKind.Repeat,
            texture, AssetVersionRequirementKind.Exact, Version, true, hash, "", AssetVersionRequirementKind.Any, 0, false, "", "",
            LayerTileCells, LayerTileCells, 0, 0, AuthoredVoxelAlphaModeKind.Opaque, 0);
        static AuthoredCatalogDependencyInput Uses(string owner, string reference, string hash) =>
            new(owner, reference, AssetVersionRequirementKind.Exact, Version, true, hash);
        return new(
            new[] { Texture(GrassTexture, "grass.png", GrassHash), Texture(RockTexture, "rock.png", RockHash), Entry(GrassMaterial), Entry(RockMaterial) },
            new[] { Uses(GrassMaterial, GrassTexture, GrassHash), Uses(RockMaterial, RockTexture, RockHash) },
            new[] { Surface(GrassMaterial, GrassTexture, GrassHash), Surface(RockMaterial, RockTexture, RockHash) },
            new AuthoredTextureInput[]
            {
                new(GrassTexture, TextureSize, TextureSize, AuthoredTextureFilter.Linear, AuthoredTextureWrap.Repeat),
                new(RockTexture, TextureSize, TextureSize, AuthoredTextureFilter.Linear, AuthoredTextureWrap.Repeat),
            },
            ReadOnlyMemory<AuthoredVoxelAtlasInput>.Empty,
            ReadOnlyMemory<AuthoredAtlasRegionInput>.Empty,
            new[] { Repeat(GrassMaterial, GrassTexture, GrassHash), Repeat(RockMaterial, RockTexture, RockHash) });
    }

    [DebugCommand("heightfield.inspect", Description = "Reports the rings' focus, tiles held, drawn and waiting, geometry, meshing time and the streamer's sampling.")]
    public string Inspect()
    {
        HeightfieldReadout readout = engine.Heightfield.Read(heightfield!);
        return FormattableString.Invariant(
            $"continent={continent} settleS={settleSeconds:F1} focus={readout.FocusX:F1},{readout.FocusZ:F1} resident={readout.ResidentTiles} drawn={readout.DrawnTiles} pending={readout.PendingTiles} settled={readout.Settled} vertices={readout.Vertices} indices={readout.Indices} sampleMB={readout.SampleBytes / 1e6:F1} meshMB={readout.MeshBytes / 1e6:F1} meshedLast={readout.MeshedTiles} meshMsLast={readout.MeshMicroseconds / 1e3:F2} committed={streamer!.Committed} ignored={streamer.Ignored} inFlight={streamer.InFlight} sampleMs={streamer.SampleMilliseconds:F0} eye={eye.X:F0},{eye.Y:F0},{eye.Z:F0} layers={layers}");
    }

    [DebugCommand("heightfield.fly", Description = "1 flies the circle; 0 holds the camera where it is, looking outward.")]
    public string FlyMode(float on)
    {
        flying = on != 0;
        return Inspect();
    }

    [DebugCommand("heightfield.angle", Description = "Moves the camera to this angle (radians) on its circle: a jump of a few hundred metres to kilometres.")]
    public string Angle(float radians)
    {
        angle = radians;
        Fly();
        return Inspect();
    }

    [DebugCommand("heightfield.continent", Description = "0 draws the hills in the scene; 1 the 390 km continent at 1:1000 in the backdrop in three tiers (31.25 m, 250 m, 1 km cells); 2 in six halving rings.")]
    public string Continent(float mode)
    {
        continent = Math.Clamp((int)mode, 0, 2);
        Build();
        return Inspect();
    }

    [DebugCommand("heightfield.cells", Description = "Cells along a continent tile's side (16 by default); rebuilds the continent.")]
    public string ContinentCells(float cells)
    {
        continentTileCells = (uint)Math.Clamp(cells, 1, 256);
        if (continent != 0) Build();
        return Inspect();
    }

    [DebugCommand("heightfield.teleport", Description = "Jumps the camera this many metres in a straight line to a point further along its circle, and times the rings settling.")]
    public string Teleport(float metres)
    {
        angle += 2 * Math.Asin(Math.Clamp(metres / (2 * Radius), -1, 1));
        Fly();
        Settling();
        return Inspect();
    }

    [DebugCommand("heightfield.speed", Description = "Flies at this many metres a second (the continent's default is 300; walking pace is 1.5).")]
    public string FlySpeed(float metresPerSecond)
    {
        flightSpeed = metresPerSecond;
        return Inspect();
    }

    [DebugCommand("heightfield.layers", Description = "1 draws the ground with the two-layer terrain material blended by slope; 0 with vertex colours.")]
    public string Layers(float on)
    {
        layers = on != 0;
        Build();
        return Inspect();
    }

    [DebugCommand("heightfield.invalidate", Description = "Samples again every tile within this many metres of the camera.")]
    public string Invalidate(float metres)
    {
        engine.Heightfield.Invalidate(new(heightfield!, eye.X - metres, eye.Z - metres, eye.X + metres, eye.Z + metres));
        return Inspect();
    }

    [DebugCommand("vegetation.inspect", Description = "Reports the near and far vegetation fields: tiles, patches, copies, budget and deferred work, the last call's placement, selection and publication time, invalidations and the streamers' sampling.")]
    public string InspectVegetation() => vegetation.Inspect();

    [DebugCommand("vegetation.show", Description = "1 grows grass and trees over the hills; 0 removes them.")]
    public string ShowVegetation(float on)
    {
        vegetation.Show(on != 0 && continent == 0);
        return vegetation.Inspect();
    }

    [DebugCommand("vegetation.tune", Description = "Sets the grass per square metre and how far the far field grows it (metres).")]
    public string TuneVegetation(float grassPerSquareMetre, float reach)
    {
        vegetation.Tune(grassPerSquareMetre, reach);
        return vegetation.Inspect();
    }

    [DebugCommand("vegetation.invalidate", Description = "Samples and places again every vegetation tile within this many metres of the camera.")]
    public string InvalidateVegetation(float metres)
    {
        vegetation.Invalidate(eye.X, eye.Z, metres);
        return vegetation.Inspect();
    }

    [DebugCommand("weather.wetness", Description = "How wet the hills are after rain (0 to 1), how much water gathers in puddles (0 to 1) and how frozen the puddles are (0 water to 1 ice). The ground under the trees stays dry.")]
    public string Wetness(float wetness, float puddles, float frozen)
    {
        sky ??= engine.Graphics.CreateLight(Sky(eye));
        engine.CameraView.SetWetness(new(wetness, puddles, frozen));
        WetnessReadout wet = engine.CameraView.ReadWetness();
        return FormattableString.Invariant($"wetness={wet.Wetness:F2} puddles={wet.Puddles:F2} frozen={wet.Frozen:F2}");
    }

    [DebugCommand("weather.snow", Description = "Snow over the hills, grass and trees: how much (0 to 1; 0 clears it) and the texels per metre its edge steps at (0: smooth). The ground under the trees stays clear.")]
    public string Snow(float amount, float texelsPerMetre)
    {
        sky ??= engine.Graphics.CreateLight(Sky(eye));
        engine.CameraView.SetSnow(new(amount, SnowColor, texelsPerMetre));
        SnowReadout snow = engine.CameraView.ReadSnow();
        return FormattableString.Invariant(
            $"amount={snow.Amount:F2} color={snow.Color.X:F2},{snow.Color.Y:F2},{snow.Color.Z:F2} texelsPerMetre={snow.TexelsPerMetre:F0}");
    }

    [DebugCommand("heightfield.height", Description = "Flies this many metres above the ground over the hills (45 by default).")]
    public string Height(float metres)
    {
        flightHeight = Math.Max(metres, 1);
        Fly();
        return Inspect();
    }

    [DebugCommand("decals.scatter", Description = "Scatters this many decals of this size (metres) from the marks atlas on the ground where the camera looks, with the angle falloff (1) or without (0); 0 removes them.")]
    public string ScatterDecals(float count, float size, float falloff)
    {
        for (ulong id = 1; id <= decals; id++) engine.CameraView.RemoveDecal(new(id));
        marks ??= engine.Graphics.OpenResource(new("marks.png", TextureFilter.NearestMipmapped, TextureWrap.Clamp)).Handle;
        engine.CameraView.SetDecalAtlas(new(marks, default));
        double heading = flying ? angle + (Math.PI / 2) : angle;
        double reach = flightHeight / Math.Tan(LookDown * Math.PI / 180);
        double centerX = eye.X + Math.Cos(heading) * reach, centerZ = eye.Z + Math.Sin(heading) * reach;
        decals = (ulong)Math.Max(count, 0);
        for (ulong id = 1; id <= decals; id++)
        {
            double turn = id * GoldenAngle, radius = DecalSpiralMetres * Math.Sqrt(id);
            float x = (float)(centerX + Math.Cos(turn) * radius), z = (float)(centerZ + Math.Sin(turn) * radius);
            HeightfieldPickResult ground = engine.Heightfield.Pick(new(heightfield!, new(x, (float)PickFrom, z), -Vector3.UnitY, 0));
            float cell = id % 4;
            Vector4 region = new(cell % 2 / 2, MathF.Floor(cell / 2) / 2, cell % 2 / 2 + .5f, MathF.Floor(cell / 2) / 2 + .5f);
            engine.CameraView.SetDecal(new(id, new(x, (float)ground.Height, z), Quaternion.CreateFromAxisAngle(Vector3.UnitY, (float)turn),
                new(size, size * DecalDepth, size), region, new Color(1, 1, 1, DecalOpacity), 0, 0, falloff != 0, false, false, 0, 0));
        }
        DecalReadout readout = engine.CameraView.ReadDecals();
        return FormattableString.Invariant($"decals={readout.Decals} budget={readout.Budget} center={centerX:F1},{centerZ:F1}");
    }

    [DebugCommand("heightfield.pick", Description = "Picks the drawn ground along a ray from a point in a direction.")]
    public string Pick(float x, float y, float z, float dx, float dy, float dz)
    {
        HeightfieldPickResult hit = engine.Heightfield.Pick(new(heightfield!, new(x, y, z), new(dx, dy, dz), 0));
        return FormattableString.Invariant(
            $"hit={hit.Hit} at={hit.Position.X:F2},{hit.Position.Y:F2},{hit.Position.Z:F2} normal={hit.Normal.X:F2},{hit.Normal.Y:F2},{hit.Normal.Z:F2} distance={hit.Distance:F2} ring={hit.Ring}");
    }

    public void RegisterDebugCommands(IDebugCommandModuleRegistrar registrar) => registrar.Register(this);
    public void Pause() { }
    public void Resume() { }
    public void Restart() { }
    public void Shutdown() { }

    public void Dispose()
    {
        sky?.Dispose();
        if (marks is not null)
        {
            // The atlas may not be released while decals sample it.
            engine.CameraView.SetDecalAtlas(default);
            marks.Dispose();
        }
        vegetation.Dispose();
        streamer?.Dispose();
        heightfield?.Dispose();
        layered.Dispose();
        grass.Dispose();
        rock.Dispose();
        water.Dispose();
        white.Dispose();
        camera.Dispose();
    }
}

/// <summary>
/// The ground: broad hills, ridged ranges and metre-scale roughness from value noise, flooded
/// below sea level. Pure and thread-safe, so the streamer samples tiles in parallel.
/// </summary>
/// <param name="spread">How much wider the land's features are than the hills' (25 for the continent); heights grow by
/// a tenth as much again.</param>
internal sealed class Ground(bool layers, double spread) : IHeightfieldSampler
{
    private readonly double rise = spread > 1 ? spread / 2.5 : 1;

    private const double SeaLevel = 0, HillScale = 2200, HillHeight = 140, RangeScale = 900, RangeHeight = 220;
    private const double RoughScale = 3, RoughHeight = .35, Offset = 30;
    private const double SandBand = 4, SnowLine = 260, RockSlopeFrom = .45, RockSlopeFull = .9;
    // Linear colours.
    private static readonly Vector3 Sand = new(.55f, .48f, .3f), Grass = new(.12f, .25f, .06f), Rock = new(.25f, .23f, .2f), Snow = new(.85f, .87f, .9f);

    public void Sample(HeightfieldTileRequest tile, HeightfieldTileSamples samples)
    {
        for (int j = -1; j <= (int)tile.TileCells + 1; j++)
        for (int i = -1; i <= (int)tile.TileCells + 1; i++)
        {
            double height = Lift(tile.SampleX(i), tile.SampleZ(j));
            samples.SetHeight(i, j, (float)Math.Max(height, SeaLevel));
        }

        for (int j = 0; j <= tile.TileCells; j++)
        for (int i = 0; i <= tile.TileCells; i++)
        {
            double x = tile.SampleX(i), z = tile.SampleZ(j), height = Lift(x, z);
            if (height < SeaLevel) samples.SetMaterial(i, j, 1);
            double slope = Slope(x / spread, z / spread, Math.Max(tile.CellSize / spread, 1));
            float rocky = (float)Math.Clamp((slope - RockSlopeFrom) / (RockSlopeFull - RockSlopeFrom), 0, 1);
            if (layers)
            {
                samples.SetLayerWeights(i, j, new(1 - rocky, rocky, 0, 0));
                continue;
            }

            double shape = height / rise;
            Vector3 colour = shape < SeaLevel + SandBand ? Sand : Vector3.Lerp(Grass, Rock, rocky);
            if (shape > SnowLine) colour = Vector3.Lerp(colour, Snow, (float)Math.Clamp((shape - SnowLine) / 40, 0, 1) * (1 - rocky));
            samples.SetColor(i, j, new(colour.X, colour.Y, colour.Z, 1));
        }
    }

    internal const double SeaLevelMetres = SeaLevel;

    /// <summary>The hills' land at a place, in metres (spread 1).</summary>
    internal static double Lift1(double x, double z) => Height(x, z);

    /// <summary>Grass grows on the hills' land above the sand, below the snow and gentler than rock.</summary>
    internal static bool Grassland(double x, double z)
    {
        double height = Height(x, z);
        return height >= SeaLevel + SandBand && height <= SnowLine && Slope(x, z, 1) < RockSlopeFrom;
    }

    /// <summary>Broad patches for the woods, 0 to 1.</summary>
    internal static double Woods(double x, double z) => Noise(x, z, 3);

    /// <summary>A deterministic draw in 0 to 1 for a cell and a salt.</summary>
    internal static double Unit(long x, long z, int salt) => Hash(x, z, salt + 100);

    /// <summary>The land at a place: the hills' shape spread and raised.</summary>
    private double Lift(double x, double z) => rise * Height(x / spread, z / spread);

    internal static double Height(double x, double z) =>
        Offset + (HillHeight * (Noise(x / HillScale, z / HillScale, 4) - .5) * 2)
        + (RangeHeight * Math.Pow(1 - Math.Abs((Noise((x + 7_000) / RangeScale, z / RangeScale, 5) * 2) - 1), 3))
        + (RoughHeight * Noise(x / RoughScale, z / RoughScale, 2));

    private static double Slope(double x, double z, double step) =>
        Math.Sqrt(Math.Pow((Height(x + step, z) - Height(x - step, z)) / (2 * step), 2)
            + Math.Pow((Height(x, z + step) - Height(x, z - step)) / (2 * step), 2));

    /// <summary>Fractal value noise in 0 to 1 over <paramref name="octaves"/> octaves.</summary>
    private static double Noise(double x, double z, int octaves)
    {
        double sum = 0, amplitude = .5, total = 0;
        for (int octave = 0; octave < octaves; octave++)
        {
            sum += amplitude * Value(x, z, octave);
            total += amplitude;
            amplitude *= .5;
            x *= 2.03;
            z *= 2.03;
        }

        return sum / total;
    }

    private static double Value(double x, double z, int seed)
    {
        double fx = Math.Floor(x), fz = Math.Floor(z);
        long ix = (long)fx, iz = (long)fz;
        double u = Smooth(x - fx), v = Smooth(z - fz);
        double a = Hash(ix, iz, seed), b = Hash(ix + 1, iz, seed), c = Hash(ix, iz + 1, seed), d = Hash(ix + 1, iz + 1, seed);
        return (a + ((b - a) * u)) * (1 - v) + ((c + ((d - c) * u)) * v);
    }

    private static double Smooth(double t) => t * t * (3 - (2 * t));

    private static double Hash(long x, long z, int seed)
    {
        ulong h = unchecked(((ulong)x * 0x9E3779B97F4A7C15UL) ^ ((ulong)z * 0xC2B2AE3D27D4EB4FUL) ^ ((ulong)seed * 0x165667B19E3779F9UL));
        h ^= h >> 31;
        h = unchecked(h * 0xBF58476D1CE4E5B9UL);
        h ^= h >> 29;
        return (h >> 11) * (1.0 / (1UL << 53));
    }
}
