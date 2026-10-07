using System.Numerics;
using Rusty.Engine;
using Rusty.Engine.Debugging;

namespace CsharpVoxelSharp;

/// <summary>
/// Sharp cuts in dual-contoured rock (#9504): a block of rock at Sharp placement with flat facets,
/// a box cut out of its corner and a sphere of rock added on top, both with the Engine's density
/// brushes. The brushes record the exact normal of their shape at every crossing they cut, so
/// Sharp placement puts the cut's edges where they are; runtimes before #9504 estimate those
/// normals from the densities and round the edges at the cell scale. The product uses only
/// brushes, so the same build runs on both for a same-camera comparison.
/// </summary>
public sealed class Product : IEngineProduct, IDebugCommandModuleSource, IDebugCommandModule
{
    private const float VoxelSize = 1;
    private const uint ChunkSize = 16;
    private const int ChunksAcross = 2, ChunksHigh = 2;
    private const uint RockSlot = 1;
    private const double BlockTop = 14;
    private const float MinimumDensity = 0.001f;
    private const double CellCentre = .5;
    private const float CreaseDegrees = 35, Roughness = 0;
    private const double FieldOfView = 50, NearClip = .1, FarClip = 300;
    private static readonly Vector3 Eye = new(-6, 22, -8), Target = new(14, 10, 12);
    // The box cut from the block's near corner, and the sphere set on top.
    private static readonly Vector3 CutMin = new(3.4f, 6.3f, 3.6f), CutMax = new(13.6f, 16f, 11.3f);
    private static readonly Vector3 KnobCentre = new(20.3f, 14.2f, 19.6f);
    private const float KnobRadius = 4.6f;

    private readonly IEngineContext engine;
    private readonly SpatialSession session;
    private readonly Material rock;
    private readonly Camera camera;
    private VoxelScenePresentation? presentation;
    private string last = "none";

    public Product(ProductCreateContext context)
    {
        engine = context.Engine;
        session = engine.Spatial.CreateSession(new(VoxelSize, ChunkSize, VoxelSurfaceMode.DualContouring));
        rock = engine.Graphics.CreateMaterial(new MaterialRequest(new Color(.55f, .52f, .48f, 1), default, 1, new Color(1, 1, 1, 1), default, 0, false));
        camera = engine.CameraView.CreateCamera(Descriptor(Eye, Target));
        engine.CameraView.SetActiveCamera(camera);
    }

    public void Start()
    {
        Admit();
        engine.Voxel.ConfigureMaterialSurfaces(new(session, VoxelSurfaceMode.DualContouring, new VoxelMaterialSurface[]
        {
            new(RockSlot, VoxelSurfaceMode.DualContouring, new SurfaceCharacter(VertexPlacement.Sharp, CreaseDegrees, Roughness)),
        }));
        presentation = engine.VoxelScenePresentation.ProjectScene(new(session, new VoxelSceneMaterialBinding[] { new(RockSlot, rock) }));
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

    private static CameraDescriptor Descriptor(Vector3 eye, Vector3 target)
    {
        CameraQueries.TryLookAtPose(eye, target, 0, out CameraPose pose);
        return new(pose, CameraBasisMode.Derived, default,
            new(CameraProjectionKind.Perspective, FieldOfView, 0, NearClip, FarClip), CameraViewports.Full);
    }

    private string Apply(string name, VoxelDensityEdit edit)
    {
        VoxelDensityReceipt receipt = engine.Voxel.ApplyDensityEdits(new VoxelDensityTransaction(session, new[] { edit }));
        engine.VoxelScenePresentation.RefreshScene(presentation!);
        last = FormattableString.Invariant(
            $"{name}: status={receipt.Status} changed={receipt.ChangedVoxels} solidity={receipt.SolidityChanges} rebuilt={receipt.RebuiltMeshChunks} meshMs={receipt.MeshMicroseconds / 1000.0:F1}");
        return last;
    }

    [DebugCommand("sharp.cut", Description = "Cuts a box out of the block's near corner with the box brush.")]
    public string Cut() => Apply("cut", VoxelDensityEdit.Box(CutMin, CutMax, VoxelDensityOperation.Subtract, RockSlot));

    [DebugCommand("sharp.knob", Description = "Adds a sphere of rock on top of the block with the sphere brush.")]
    public string Knob() => Apply("knob", VoxelDensityEdit.Sphere(KnobCentre, KnobRadius, VoxelDensityOperation.Add, RockSlot));

    [DebugCommand("sharp.camera", Description = "Moves the camera to an eye position looking at a target.")]
    public string MoveCamera(float x, float y, float z, float targetX, float targetY, float targetZ)
    {
        engine.CameraView.UpdateCamera(new(camera, Descriptor(new(x, y, z), new(targetX, targetY, targetZ))));
        return $"camera eye={x},{y},{z} target={targetX},{targetY},{targetZ}";
    }

    [DebugCommand("sharp.inspect", Description = "Reports the scene and the last brush's receipt.")]
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
        rock.Dispose();
        session.Dispose();
    }
}
