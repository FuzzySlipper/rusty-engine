using System.Diagnostics;
using System.Numerics;
using Rusty.Engine;
using Rusty.Engine.Debugging;
using Rusty.Engine.Implicit;

namespace CsharpVoxelStamp;

/// <summary>
/// A dual-contoured block of rock shaped with implicit field stamps (#9505): the product composes
/// a field with <see cref="ImplicitRecipe"/> and stamps a node into the session's densities with
/// <c>Voxel.StampImplicit</c>, which applies it as a density brush of that shape. A smooth union
/// of capsules carves a cave system into the block's face; a wave-displaced sphere adds an eroded
/// boulder on top.
/// </summary>
public sealed class Product : IEngineProduct, IDebugCommandModuleSource, IDebugCommandModule
{
    private const float VoxelSize = 1;
    private const uint ChunkSize = 16;
    private const int ChunksAcross = 4, ChunksHigh = 2;
    private const uint RockSlot = 1, MossSlot = 2;
    // The block: rock up to this height across the whole area.
    private const double BlockTop = 20;
    private const float MinimumDensity = 0.001f;
    private const double CellCentre = .5;
    private const double FieldOfView = 60, NearClip = .1, FarClip = 400;
    private static readonly Vector3 Eye = new(32, 22, -26), Target = new(32, 12, 20);

    // The caves: three capsules blended into one tunnel system entering the block's front face.
    private static readonly (Vector3 Start, Vector3 End, float Radius)[] Tunnels =
    [
        (new(18, 9, -4), new(30, 8, 22), 4.5f),
        (new(30, 8, 22), new(46, 10, 14), 3.5f),
        (new(30, 8, 22), new(36, 15, 40), 3f),
    ];
    private const float TunnelBlend = 3f;
    // The boulder: a sphere roughened by spectral waves, sitting on the block.
    private static readonly Vector3 BoulderCentre = new(40, 23, 34);
    private const float BoulderRadius = 6f;
    private static readonly Vector3 WaveFrequency = new(.35f, .35f, .35f);
    private const float WaveAmplitude = 1.2f, WaveLacunarity = 2f, WaveGain = .5f;
    private const uint WaveOctaves = 3;
    private const ulong WaveSeed = 9505;
    private const float BoundsSlack = 2f;

    private readonly IEngineContext engine;
    private readonly SpatialSession session;
    private readonly Material rock, moss;
    private readonly Camera camera;
    private VoxelScenePresentation? presentation;
    private string last = "none";

    public Product(ProductCreateContext context)
    {
        engine = context.Engine;
        session = engine.Spatial.CreateSession(new(VoxelSize, ChunkSize, VoxelSurfaceMode.DualContouring));
        rock = Plain(new Color(.5f, .47f, .43f, 1));
        moss = Plain(new Color(.34f, .42f, .25f, 1));
        camera = engine.CameraView.CreateCamera(Descriptor(Eye, Target));
        engine.CameraView.SetActiveCamera(camera);
    }

    public void Start()
    {
        Admit();
        presentation = engine.VoxelScenePresentation.ProjectScene(new(session,
            new VoxelSceneMaterialBinding[] { new(RockSlot, rock), new(MossSlot, moss) }));
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
            for (long z = 0; z < ChunkSize; z++)
            for (long y = 0; y < ChunkSize; y++)
            for (long x = 0; x < ChunkSize; x++)
            {
                double worldY = (cy * ChunkSize + y + CellCentre) * VoxelSize;
                float distance = (float)((worldY - BlockTop) / VoxelSize);
                bool solid = distance < 0;
                materials[next] = solid ? RockSlot : 0;
                densities[next] = solid ? Math.Min(distance, -MinimumDensity) : Math.Max(distance, MinimumDensity);
                next++;
            }
            operations.Add(new(VoxelResidencyOperationKind.Admit, new(cx, cy, cz), offset, (uint)volume, offset, (uint)volume));
        }
        engine.Voxel.ApplyResidency(new(ReadOnlyMemory<uint>.Empty, session, operations.ToArray(), materials, densities));
    }

    private Material Plain(Color color) => engine.Graphics.CreateMaterial(
        new MaterialRequest(color, default, 1, new Color(1, 1, 1, 1), default, 0, false));

    private static CameraDescriptor Descriptor(Vector3 eye, Vector3 target)
    {
        CameraQueries.TryLookAtPose(eye, target, 0, out CameraPose pose);
        return new(pose, CameraBasisMode.Derived, default,
            new(CameraProjectionKind.Perspective, FieldOfView, 0, NearClip, FarClip), CameraViewports.Full);
    }

    /// <summary>Stamps a node and reports its receipt and how long the call took.</summary>
    private string Stamp(string name, ImplicitRecipe recipe, ImplicitNode node, Vector3 min, Vector3 max,
        VoxelDensityOperation operation, uint slot)
    {
        long started = Stopwatch.GetTimestamp();
        VoxelDensityReceipt receipt = engine.Voxel.StampImplicit(new(session, recipe.Field, node, min, max, operation, 0, slot));
        double ms = Stopwatch.GetElapsedTime(started).TotalMilliseconds;
        engine.VoxelScenePresentation.RefreshScene(presentation!);
        last = FormattableString.Invariant(
            $"{name}: status={receipt.Status} changed={receipt.ChangedVoxels} solidity={receipt.SolidityChanges} rebuilt={receipt.RebuiltMeshChunks} meshMs={receipt.MeshMicroseconds / 1000.0:F1} callMs={ms:F1} solid={receipt.SolidVoxelCount}");
        return last;
    }

    [DebugCommand("stamp.caves", Description = "Carves a smooth union of three capsules into the block's face.")]
    public string Caves()
    {
        using ImplicitRecipe recipe = new(engine.ImplicitSurfaces);
        ImplicitNode caves = recipe.Capsule(Tunnels[0].Start, Tunnels[0].End, Tunnels[0].Radius);
        Vector3 min = Vector3.Min(Tunnels[0].Start, Tunnels[0].End) - new Vector3(Tunnels[0].Radius);
        Vector3 max = Vector3.Max(Tunnels[0].Start, Tunnels[0].End) + new Vector3(Tunnels[0].Radius);
        foreach ((Vector3 start, Vector3 end, float radius) in Tunnels.Skip(1))
        {
            caves = recipe.Blend(caves, recipe.Capsule(start, end, radius), TunnelBlend);
            min = Vector3.Min(min, Vector3.Min(start, end) - new Vector3(radius));
            max = Vector3.Max(max, Vector3.Max(start, end) + new Vector3(radius));
        }
        Vector3 slack = new(TunnelBlend + BoundsSlack);
        return Stamp("caves", recipe, caves, min - slack, max + slack, VoxelDensityOperation.Subtract, RockSlot);
    }

    [DebugCommand("stamp.boulder", Description = "Adds a wave-roughened sphere of moss-covered rock on top of the block.")]
    public string Boulder()
    {
        using ImplicitRecipe recipe = new(engine.ImplicitSurfaces);
        ImplicitNode boulder = recipe.DisplaceWaves(recipe.Sphere(BoulderCentre, BoulderRadius), WaveFrequency, WaveAmplitude,
            WaveOctaves, WaveLacunarity, WaveGain, WaveSeed);
        Vector3 reach = new(BoulderRadius + WaveAmplitude + BoundsSlack);
        return Stamp("boulder", recipe, boulder, BoulderCentre - reach, BoulderCentre + reach, VoxelDensityOperation.Add, MossSlot);
    }

    [DebugCommand("stamp.camera", Description = "Moves the camera to an eye position looking at a target.")]
    public string MoveCamera(float x, float y, float z, float targetX, float targetY, float targetZ)
    {
        engine.CameraView.UpdateCamera(new(camera, Descriptor(new(x, y, z), new(targetX, targetY, targetZ))));
        return $"camera eye={x},{y},{z} target={targetX},{targetY},{targetZ}";
    }

    [DebugCommand("stamp.inspect", Description = "Reports the scene and the last stamp's receipt.")]
    public string Inspect()
    {
        VoxelScenePresentationReadout readout = engine.VoxelScenePresentation.RefreshScene(presentation!);
        return FormattableString.Invariant($"chunks={readout.ChunkCount} last={last}");
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
        moss.Dispose();
        rock.Dispose();
        session.Dispose();
    }
}
