using System.Diagnostics;
using System.Numerics;
using Rusty.Engine;
using Rusty.Engine.Debugging;

namespace CsharpVoxelScatter;

/// <summary>
/// A dual-contoured hill the Engine grows grass and bushes on (#9546): the product names what
/// grows where and how densely through <c>VoxelScenePresentation.SetScatter</c>, and the Engine
/// places the copies on the reconstructed ground around the camera, draws each chunk's patch
/// as one instanced draw, fades it out at the edge of its reach and places it again when the
/// ground is dug or the camera walks on.
/// </summary>
public sealed class Product : IEngineProduct, IDebugCommandModuleSource, IDebugCommandModule
{
    private const float VoxelSize = 1;
    private const uint ChunkSize = 16;
    private const int ChunksAcross = 8, ChunksHigh = 2;
    private const uint GroundSlot = 1, RockSlot = 2;
    // Ground covers the top few voxels; rock lies below, and shows on the steep crown.
    private const double GroundDepth = 3;
    private const float MinimumDensity = 0.001f;
    // The hill: a base, a round rise in the middle of the land and a gentle ripple.
    private const double BaseHeight = 6, HillHeight = 14, HillSpread = 22, RippleHeight = 1.2, RippleFrequency = .13;
    private static readonly Vector2 HillCentre = new(64, 64);
    // Voxels are sampled at their centres.
    private const double CellCentre = .5;
    private const double FieldOfView = 60, NearClip = .1, FarClip = 600;
    private static readonly Vector3 Eye = new(30, 16, 30), Target = new(64, 12, 64);

    // What grows: grass clumps on the ground slot, bushes more sparsely on the same ground.
    private const uint GrassScatter = 0, BushScatter = 1;
    private const float DefaultGrassDensity = 3, DefaultBushDensity = .06f, DefaultRadius = 40;
    private const float GrassSlope = 35, BushSlope = 25, GrassAlign = .35f;
    private const uint MaximumGrass = 60_000, MaximumBushes = 2_000;
    private const int GrassCards = 4;
    private const float GrassWidth = .45f, GrassHeight = .5f, GrassSpread = .35f;
    private const float GrassWindBend = .08f, GrassWindFlutter = .06f;
    private const float AlphaCutoff = .5f;
    private static readonly Vector2 WindDirection = Vector2.Normalize(new(1, .4f));
    private const float DefaultWind = .8f, WindGust = .5f;
    private static readonly Color GrassColor = new(1, 1, 1, 1), BushColor = new(.2f, .36f, .14f, 1);
    private static readonly Vector3 GrassTintLow = new(.75f, .8f, .55f), GrassTintHigh = new(1.05f, 1.05f, .9f);
    private static readonly Vector3 BushTintLow = new(.7f, .85f, .7f), BushTintHigh = new(1.1f, 1.1f, 1);
    private const int BushRings = 4, BushSegments = 7;
    private const float BushWidth = .6f, BushHeight = .45f;

    private readonly IEngineContext engine;
    private readonly SpatialSession session;
    private readonly Material ground, rock, grass, bush;
    private readonly RenderResource grassTexture;
    private readonly MeshResource grassMesh, bushMesh;
    private readonly Appearance grassLook, bushLook;
    private readonly Camera camera;
    private VoxelScenePresentation? presentation;
    private readonly List<VoxelSceneScatterExclusion> exclusions = new();
    private float grassDensity = DefaultGrassDensity, bushDensity = DefaultBushDensity, radius = DefaultRadius;
    private double admissionMs, scatterMs;

    public Product(ProductCreateContext context)
    {
        engine = context.Engine;
        session = engine.Spatial.CreateSession(new(VoxelSize, ChunkSize, VoxelSurfaceMode.DualContouring));
        ground = Plain(new Color(.36f, .5f, .24f, 1));
        rock = Plain(new Color(.48f, .46f, .43f, 1));
        Color white = new(1, 1, 1, 1);
        grassTexture = engine.Graphics.OpenResource(new("grass.png", TextureFilter.Linear, TextureWrap.Clamp)).Handle;
        grass = engine.Graphics.CreateMaterial(new MaterialRequest(GrassColor, grassTexture, .9f, white, Vector3.Zero, 0, true) with
        {
            AlphaMode = MaterialAlphaMode.Mask,
            AlphaCutoff = AlphaCutoff,
            WindBend = GrassWindBend,
            WindFlutter = GrassWindFlutter,
        });
        bush = engine.Graphics.CreateMaterial(new MaterialRequest(BushColor, default(RenderResourceReference), .85f, white, Vector3.Zero, 0, false) with
        {
            FlatShading = true,
        });
        grassMesh = engine.Graphics.CreateMeshResource(Grass(grass));
        bushMesh = engine.Graphics.CreateMeshResource(Bush(bush));
        grassLook = engine.Graphics.CreateMeshAppearance(grassMesh);
        bushLook = engine.Graphics.CreateMeshAppearance(bushMesh);
        camera = engine.CameraView.CreateCamera(Descriptor(Eye, Target));
        engine.CameraView.SetActiveCamera(camera);
        engine.CameraView.SetWind(new(WindDirection, DefaultWind, WindGust));
    }

    public void Start()
    {
        long started = Stopwatch.GetTimestamp();
        Admit();
        admissionMs = Stopwatch.GetElapsedTime(started).TotalMilliseconds;
        presentation = engine.VoxelScenePresentation.ProjectScene(new(session,
            new VoxelSceneMaterialBinding[] { new(GroundSlot, ground), new(RockSlot, rock) }));
        Grow();
    }

    /// <summary>Asks for both scatters at the current densities and radius.</summary>
    private void Grow()
    {
        long started = Stopwatch.GetTimestamp();
        uint[] onGround = [GroundSlot];
        Set(GrassScatter, grassDensity, new VoxelSceneScatterRequest(presentation!, GrassScatter, grassLook, grass, onGround, grassDensity, radius) with
        {
            ScaleMin = .8f,
            ScaleMax = 1.25f,
            TintLow = GrassTintLow,
            TintHigh = GrassTintHigh,
            SlopeLimitDegrees = GrassSlope,
            Align = GrassAlign,
            MaximumInstances = MaximumGrass,
        });
        Set(BushScatter, bushDensity, new VoxelSceneScatterRequest(presentation!, BushScatter, bushLook, bush, onGround, bushDensity, radius) with
        {
            ScaleMin = .7f,
            ScaleMax = 1.5f,
            TintLow = BushTintLow,
            TintHigh = BushTintHigh,
            SlopeLimitDegrees = BushSlope,
            CastsShadows = true,
            MaximumInstances = MaximumBushes,
            Seed = 1,
        });
        scatterMs = Stopwatch.GetElapsedTime(started).TotalMilliseconds;
    }

    private void Set(uint scatter, float density, VoxelSceneScatterRequest request)
    {
        if (density > 0) engine.VoxelScenePresentation.SetScatter(request);
        else engine.VoxelScenePresentation.RemoveScatter(new(presentation!, scatter));
    }

    /// <summary>The ground height in metres at a column.</summary>
    private static double Height(double x, double z)
    {
        double dx = x - HillCentre.X, dz = z - HillCentre.Y;
        return BaseHeight + HillHeight * Math.Exp(-(dx * dx + dz * dz) / (2 * HillSpread * HillSpread))
            + RippleHeight * Math.Sin(x * RippleFrequency) * Math.Cos(z * RippleFrequency);
    }

    private void Admit()
    {
        int volume = (int)(ChunkSize * ChunkSize * ChunkSize);
        List<VoxelResidencyOperation> operations = [];
        uint[] materials = new uint[ChunksAcross * ChunksAcross * ChunksHigh * volume];
        float[] densities = new float[materials.Length];
        int next = 0;
        for (long cz = 0; cz < ChunksAcross; cz++)
        for (long cy = 0; cy < ChunksHigh; cy++)
        for (long cx = 0; cx < ChunksAcross; cx++)
        {
            uint offset = (uint)next;
            // X-fastest, then Y, then Z: the Engine's dense payload order.
            for (long z = 0; z < ChunkSize; z++)
            for (long y = 0; y < ChunkSize; y++)
            for (long x = 0; x < ChunkSize; x++)
            {
                double worldX = (cx * ChunkSize + x + CellCentre) * VoxelSize, worldZ = (cz * ChunkSize + z + CellCentre) * VoxelSize;
                double worldY = (cy * ChunkSize + y + CellCentre) * VoxelSize;
                float distance = (float)((worldY - Height(worldX, worldZ)) / VoxelSize);
                bool solid = distance < 0;
                materials[next] = !solid ? 0 : distance > -GroundDepth ? GroundSlot : RockSlot;
                densities[next] = solid ? Math.Min(distance, -MinimumDensity) : Math.Max(distance, MinimumDensity);
                next++;
            }
            operations.Add(new(VoxelResidencyOperationKind.Admit, new(cx, cy, cz), offset, (uint)volume, offset, (uint)volume));
        }
        engine.Voxel.ApplyResidency(new(ReadOnlyMemory<uint>.Empty, session, operations.ToArray(), materials, densities));
    }

    private Material Plain(Color color) => engine.Graphics.CreateMaterial(
        new MaterialRequest(color, default, 1, new Color(1, 1, 1, 1), default, 0, false));

    /// <summary>
    /// A clump of grass: cards crossed at even angles around its origin, each GrassWidth across and
    /// GrassHeight tall, the texture's blades on them; vertex alpha 0 at the root and 1 at the tip
    /// weights the flutter.
    /// </summary>
    private static MeshResourceCreateRequest Grass(Material material)
    {
        List<Vector3> positions = [], normals = [];
        List<Vector2> uvs = [];
        List<Color> colors = [];
        List<uint> indices = [];
        Random placement = new(9546);
        for (int card = 0; card < GrassCards; card++)
        {
            float angle = card * MathF.PI / GrassCards;
            Vector3 across = new(MathF.Cos(angle) * GrassWidth / 2, 0, MathF.Sin(angle) * GrassWidth / 2);
            Vector3 normal = new(-MathF.Sin(angle), 0, MathF.Cos(angle));
            Vector3 root = new(((float)placement.NextDouble() - .5f) * GrassSpread, 0, ((float)placement.NextDouble() - .5f) * GrassSpread);
            Vector3 up = new(0, GrassHeight, 0);
            uint first = (uint)positions.Count;
            positions.AddRange([root - across, root + across, root + across + up, root - across + up]);
            normals.AddRange([normal, normal, normal, normal]);
            uvs.AddRange([new(0, 1), new(1, 1), new(1, 0), new(0, 0)]);
            colors.AddRange([new(1, 1, 1, 0), new(1, 1, 1, 0), new(1, 1, 1, 1), new(1, 1, 1, 1)]);
            indices.AddRange([first, first + 1, first + 2, first, first + 2, first + 3]);
        }
        return new MeshResourceCreateRequest(positions.ToArray(), normals.ToArray(), uvs.ToArray(), colors.ToArray(), indices.ToArray(),
            new MeshGroup[] { new(0, 0, (uint)indices.Count) }, new MeshMaterialBinding[] { new(0, material) });
    }

    /// <summary>A low-poly bush: a squashed sphere of few rings, standing on its origin.</summary>
    private static MeshResourceCreateRequest Bush(Material material)
    {
        List<Vector3> positions = [], normals = [];
        List<Vector2> uvs = [];
        List<uint> indices = [];
        for (int ring = 0; ring <= BushRings; ring++)
        for (int segment = 0; segment <= BushSegments; segment++)
        {
            float theta = MathF.PI * ring / BushRings, phi = MathF.Tau * segment / BushSegments;
            Vector3 n = new(MathF.Sin(theta) * MathF.Cos(phi), MathF.Cos(theta), MathF.Sin(theta) * MathF.Sin(phi));
            positions.Add(new(n.X * BushWidth / 2, (n.Y + .7f) * BushHeight / 1.7f, n.Z * BushWidth / 2));
            normals.Add(n);
            uvs.Add(new((float)segment / BushSegments, (float)ring / BushRings));
        }
        uint across = BushSegments + 1;
        for (uint ring = 0; ring < BushRings; ring++)
        for (uint segment = 0; segment < BushSegments; segment++)
        {
            uint a = ring * across + segment, b = a + across;
            indices.AddRange([a, a + 1, b, a + 1, b + 1, b]);
        }
        return new MeshResourceCreateRequest(positions.ToArray(), normals.ToArray(), uvs.ToArray(), ReadOnlyMemory<Color>.Empty, indices.ToArray(),
            new MeshGroup[] { new(0, 0, (uint)indices.Count) }, new MeshMaterialBinding[] { new(0, material) });
    }

    private static CameraDescriptor Descriptor(Vector3 eye, Vector3 target)
    {
        CameraQueries.TryLookAtPose(eye, target, 0, out CameraPose pose);
        return new(pose, CameraBasisMode.Derived, default,
            new(CameraProjectionKind.Perspective, FieldOfView, 0, NearClip, FarClip), CameraViewports.Full);
    }

    [DebugCommand("scatter.grass", Description = "Grass clumps per square metre of ground; 0 removes the grass.")]
    public string GrassDensity(float density)
    {
        grassDensity = density;
        Grow();
        return Inspect();
    }

    [DebugCommand("scatter.bushes", Description = "Bushes per square metre of ground; 0 removes the bushes.")]
    public string BushDensity(float density)
    {
        bushDensity = density;
        Grow();
        return Inspect();
    }

    [DebugCommand("scatter.radius", Description = "How far from the camera things grow, in metres; they fade over the last quarter.")]
    public string Radius(float metres)
    {
        radius = metres;
        Grow();
        return Inspect();
    }

    [DebugCommand("scatter.wind", Description = "Blows the wind at this strength (0 stills it).")]
    public string Wind(float strength)
    {
        engine.CameraView.SetWind(new(WindDirection, strength, WindGust));
        return $"wind={strength}";
    }

    [DebugCommand("scatter.dig", Description = "Digs a sphere of this radius out of the ground at a point; the ground there grows again as it remeshes.")]
    public string Dig(float x, float y, float z, float sphere)
    {
        engine.Voxel.ApplyDensityEdits(new(session, new VoxelDensityEdit[]
        {
            new(VoxelDensityEditKind.Brush, default, 0, 0, 0, 0, 0, 0, 0, VoxelDensityShape.Sphere, VoxelDensityOperation.Subtract,
                new(x, y, z), sphere, default, default, 1, GroundSlot),
        }, ReadOnlyMemory<float>.Empty, ReadOnlyMemory<uint>.Empty));
        return Inspect();
    }

    [DebugCommand("scatter.exclude", Description = "Keeps everything from growing in a box from one corner to the other, as under a built floor; the chunks it touches are placed again.")]
    public string Exclude(float minX, float minY, float minZ, float maxX, float maxY, float maxZ)
    {
        exclusions.Add(new(new(minX, minY, minZ), new(maxX, maxY, maxZ)));
        engine.VoxelScenePresentation.SetScatterExclusions(new(presentation!, exclusions.ToArray()));
        return Inspect();
    }

    [DebugCommand("scatter.clearExclusions", Description = "Removes every exclusion box; the ground under them grows again.")]
    public string ClearExclusions()
    {
        exclusions.Clear();
        engine.VoxelScenePresentation.SetScatterExclusions(new(presentation!, Array.Empty<VoxelSceneScatterExclusion>()));
        return Inspect();
    }

    [DebugCommand("scatter.camera", Description = "Moves the camera to an eye position looking at a target; things grow around it on the next update.")]
    public string MoveCamera(float x, float y, float z, float targetX, float targetY, float targetZ)
    {
        engine.CameraView.UpdateCamera(new(camera, Descriptor(new(x, y, z), new(targetX, targetY, targetZ))));
        return $"camera eye={x},{y},{z} target={targetX},{targetY},{targetZ}";
    }

    [DebugCommand("scatter.inspect", Description = "Reports the chunks, scatter patches, copies and chunks left bare by a budget.")]
    public string Inspect()
    {
        VoxelScenePresentationReadout readout = engine.VoxelScenePresentation.RefreshScene(presentation!);
        return FormattableString.Invariant(
            $"chunks={readout.ChunkCount} patches={readout.ScatterPatchCount} copies={readout.ScatterInstanceCount} overBudget={readout.ScatterOverBudgetCount} grass={grassDensity} bushes={bushDensity} radius={radius} admissionMs={admissionMs:F0} scatterMs={scatterMs:F1}");
    }

    public void RegisterDebugCommands(IDebugCommandModuleRegistrar registrar) => registrar.Register(this);
    public ProductUpdateResult Update(ProductUpdate update) => ProductUpdateResult.None;
    public void Pause() { }
    public void Resume() { }
    public void Restart() { }
    public void Shutdown() { }

    public void Dispose()
    {
        presentation?.Dispose();
        camera.Dispose();
        grassLook.Dispose();
        bushLook.Dispose();
        grassMesh.Dispose();
        bushMesh.Dispose();
        grass.Dispose();
        bush.Dispose();
        grassTexture.Dispose();
        rock.Dispose();
        ground.Dispose();
        session.Dispose();
    }
}
