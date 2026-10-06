using System.Diagnostics;
using System.Numerics;
using Rusty.Engine;
using Rusty.Engine.Debugging;

namespace CsharpVoxelLod;

/// <summary>
/// A dual-contoured landscape of 16 × 3 × 16 chunks drawn with distance level of detail:
/// chunks beyond the coarse distance from the camera are drawn from the Engine's coarse
/// meshes, while collision keeps every chunk at full resolution.
/// </summary>
public sealed class Product : IEngineProduct, IDebugCommandModuleSource, IDebugCommandModule
{
    private const float VoxelSize = 1;
    private const uint ChunkSize = 16;
    private const int ChunksAcross = 16, ChunksHigh = 3;
    private const uint GroundSlot = 1, RockSlot = 2;
    // Ground covers the top few voxels; rock lies below.
    private const double GroundDepth = 3;
    private const float MinimumDensity = 0.001f;
    // The terrain: a base height with a ridge along x, valleys along z and a diagonal ripple.
    private const double BaseHeight = 22, RidgeHeight = 9, RidgeFrequency = .045;
    private const double ValleyDepth = 7, ValleyFrequency = .06, RippleHeight = 3, RippleFrequency = .11;
    // Voxels are sampled at their centres.
    private const double CellCentre = .5;
    private const double DefaultCoarseDistance = 64;
    private const double FieldOfView = 60, NearClip = .1, FarClip = 600;
    private static readonly Vector3 Eye = new(10, 46, 10), Target = new(200, 18, 200);
    private readonly IEngineContext engine;
    private readonly SpatialSession session;
    private readonly Material ground, rock;
    private readonly Camera camera;
    private VoxelScenePresentation? presentation;
    private double coarseDistance = DefaultCoarseDistance;
    private double admissionMs;

    public Product(ProductCreateContext context)
    {
        engine = context.Engine;
        session = engine.Spatial.CreateSession(new(VoxelSize, ChunkSize, VoxelSurfaceMode.DualContouring));
        ground = CreateMaterial(new Color(.38f, .55f, .27f, 1));
        rock = CreateMaterial(new Color(.48f, .46f, .43f, 1));
        camera = engine.CameraView.CreateCamera(Descriptor(Eye, Target));
        engine.CameraView.SetActiveCamera(camera);
    }

    public void Start()
    {
        long started = Stopwatch.GetTimestamp();
        Admit();
        admissionMs = Stopwatch.GetElapsedTime(started).TotalMilliseconds;
        presentation = engine.VoxelScenePresentation.ProjectScene(new(session,
            new VoxelSceneMaterialBinding[] { new(GroundSlot, ground), new(RockSlot, rock) }));
        engine.VoxelScenePresentation.SetLevelOfDetail(new(presentation, coarseDistance));
    }

    /// <summary>The terrain height in metres at a column.</summary>
    private static double Height(double x, double z) =>
        BaseHeight + RidgeHeight * Math.Sin(x * RidgeFrequency) + ValleyDepth * Math.Cos(z * ValleyFrequency)
        + RippleHeight * Math.Sin((x - z) * RippleFrequency);

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

    private Material CreateMaterial(Color color) => engine.Graphics.CreateMaterial(
        new MaterialRequest(color, default, 1, new Color(1, 1, 1, 1), default, 0, false));

    private static CameraDescriptor Descriptor(Vector3 eye, Vector3 target)
    {
        CameraQueries.TryLookAtPose(eye, target, 0, out CameraPose pose);
        return new(pose, CameraBasisMode.Derived, default,
            new(CameraProjectionKind.Perspective, FieldOfView, 0, NearClip, FarClip), CameraViewports.Full);
    }

    [DebugCommand("lod.distance", Description = "Draws chunks farther than this many metres coarse; 0 draws every chunk at full resolution.")]
    public string Distance(double metres)
    {
        coarseDistance = metres;
        engine.VoxelScenePresentation.SetLevelOfDetail(new(presentation!, coarseDistance));
        return Inspect();
    }

    [DebugCommand("lod.camera", Description = "Moves the camera to an eye position looking at a target.")]
    public string MoveCamera(float x, float y, float z, float targetX, float targetY, float targetZ)
    {
        engine.CameraView.UpdateCamera(new(camera, Descriptor(new(x, y, z), new(targetX, targetY, targetZ))));
        return $"camera eye={x},{y},{z} target={targetX},{targetY},{targetZ}";
    }

    [DebugCommand("lod.inspect", Description = "Reports chunks, chunks drawn coarse and the coarse distance.")]
    public string Inspect()
    {
        VoxelScenePresentationReadout readout = engine.VoxelScenePresentation.RefreshScene(presentation!);
        return FormattableString.Invariant(
            $"chunks={readout.ChunkCount} coarse={readout.CoarseChunkCount} coarseDistance={coarseDistance} admissionMs={admissionMs:F0}");
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
        rock.Dispose();
        ground.Dispose();
        session.Dispose();
    }
}
