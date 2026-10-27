using System.Diagnostics;
using System.Globalization;
using System.Numerics;
using System.Text;
using Rusty.Engine;
using Rusty.Engine.Debugging;

namespace CsharpVoxelEditCost;

/// <summary>
/// Times voxel edits in a populated scene: the dual-contoured landscape of 16 × 3 × 16 chunks
/// of 16³ one-metre voxels drawn with level of detail, edited by 1, 50 and 256 cells in one
/// chunk, 256 cells across 16 chunks, and two one-cell edits before one refresh. Each case
/// reports <c>ApplyEdits</c> and <c>RefreshScene</c> times with the Engine's breakdown.
/// </summary>
public sealed class Product : IEngineProduct, IDebugCommandModuleSource, IDebugCommandModule
{
    private const float VoxelSize = 1;
    private const int ChunkSize = 16;
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
    private const double CoarseDistance = 64;
    private const double FieldOfView = 60, NearClip = .1, FarClip = 600;
    private static readonly Vector3 Eye = new(10, 46, 10), Target = new(200, 18, 200);
    // The edited chunk column, within the coarse distance of the camera, and the first
    // column of the 4 × 4 block the spread case edits.
    private const int EditedChunkX = 2, EditedChunkZ = 2, SpreadChunkX = 1, SpreadChunkZ = 1, SpreadChunks = 4;
    // The one- and fifty-cell cases stay this far from the chunk's sides.
    private const int Inset = 4;
    private const int FewCells = 50, ManyCells = 256;
    private const int DefaultRepeats = 9;
    private readonly IEngineContext engine;
    private readonly SpatialSession session;
    private readonly Material ground, rock;
    private readonly Camera camera;
    private VoxelScenePresentation? presentation;
    private double admissionMs, projectionMs;
    // Each repeat sets the cells to the other material, so every edit changes them.
    private bool rockNext = true;

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
        started = Stopwatch.GetTimestamp();
        presentation = engine.VoxelScenePresentation.ProjectScene(new(session,
            new VoxelSceneMaterialBinding[] { new(GroundSlot, ground), new(RockSlot, rock) }));
        projectionMs = Stopwatch.GetElapsedTime(started).TotalMilliseconds;
        engine.VoxelScenePresentation.SetLevelOfDetail(new(presentation, CoarseDistance));
    }

    /// <summary>The terrain height in metres at a column.</summary>
    private static double Height(double x, double z) =>
        BaseHeight + RidgeHeight * Math.Sin(x * RidgeFrequency) + ValleyDepth * Math.Cos(z * ValleyFrequency)
        + RippleHeight * Math.Sin((x - z) * RippleFrequency);

    private void Admit()
    {
        int volume = ChunkSize * ChunkSize * ChunkSize;
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

    /// <summary>
    /// The top solid cell of a column, kept within the vertical chunk the surface crosses at the
    /// chunk column's centre, so every cell of a case lies in the chunks it names.
    /// </summary>
    private static VoxelAddress SurfaceCell(int chunkX, int chunkZ, int localX, int localZ)
    {
        long x = chunkX * ChunkSize + localX, z = chunkZ * ChunkSize + localZ;
        double centre = Height((chunkX + CellCentre) * ChunkSize, (chunkZ + CellCentre) * ChunkSize);
        long chunkY = Math.Clamp((long)Math.Floor(centre / ChunkSize), 0, ChunksHigh - 1);
        long top = (long)Math.Floor(Height(x + CellCentre, z + CellCentre) - CellCentre);
        return new(x, Math.Clamp(top, chunkY * ChunkSize, chunkY * ChunkSize + ChunkSize - 1), z);
    }

    private static VoxelAddress[] OneCell() => [SurfaceCell(EditedChunkX, EditedChunkZ, ChunkSize / 2, ChunkSize / 2)];

    private static VoxelAddress[] FewInOneChunk()
    {
        int across = ChunkSize - 2 * Inset;
        return Enumerable.Range(0, FewCells)
            .Select(index => SurfaceCell(EditedChunkX, EditedChunkZ, Inset + index % across, Inset + index / across))
            .ToArray();
    }

    private static VoxelAddress[] ManyInOneChunk() => Enumerable.Range(0, ManyCells)
        .Select(index => SurfaceCell(EditedChunkX, EditedChunkZ, index % ChunkSize, index / ChunkSize))
        .ToArray();

    private static VoxelAddress[] ManyAcrossChunks()
    {
        int perChunk = ManyCells / (SpreadChunks * SpreadChunks), side = (int)Math.Sqrt(perChunk);
        return Enumerable.Range(0, ManyCells)
            .Select(index =>
            {
                int chunk = index / perChunk, cell = index % perChunk;
                return SurfaceCell(SpreadChunkX + chunk % SpreadChunks, SpreadChunkZ + chunk / SpreadChunks,
                    Inset + cell % side, Inset + cell / side);
            })
            .ToArray();
    }

    private static VoxelAddress[] SecondCell() => [SurfaceCell(EditedChunkX + 1, EditedChunkZ + 1, ChunkSize / 2, ChunkSize / 2)];

    private sealed record EditCase(string Name, VoxelAddress[][] Batches);

    private static IReadOnlyList<EditCase> Cases() =>
    [
        new("1 cell", [OneCell()]),
        new("50 cells, one chunk", [FewInOneChunk()]),
        new("256 cells, one chunk", [ManyInOneChunk()]),
        new("256 cells, 16 chunks", [ManyAcrossChunks()]),
        new("1 cell twice, one refresh", [OneCell(), SecondCell()]),
    ];

    private sealed record Sample(double ApplyMs, double RefreshMs, VoxelEditReceipt Receipt, VoxelScenePresentationReadout Readout);

    private Sample Run(EditCase edit)
    {
        uint slot = rockNext ? RockSlot : GroundSlot;
        rockNext = !rockNext;
        VoxelEditReceipt receipt = default;
        long started = Stopwatch.GetTimestamp();
        foreach (VoxelAddress[] batch in edit.Batches)
        {
            VoxelEdit[] edits = batch.Select(address => new VoxelEdit(VoxelEditKind.Set, address, slot)).ToArray();
            receipt = engine.Voxel.ApplyEdits(new VoxelEditTransaction(session, edits));
        }
        double applyMs = Stopwatch.GetElapsedTime(started).TotalMilliseconds;
        started = Stopwatch.GetTimestamp();
        VoxelScenePresentationReadout readout = engine.VoxelScenePresentation.RefreshScene(presentation!);
        double refreshMs = Stopwatch.GetElapsedTime(started).TotalMilliseconds;
        return new(applyMs, refreshMs, receipt, readout);
    }

    private static double Median(IEnumerable<double> values)
    {
        double[] sorted = values.Order().ToArray();
        return sorted.Length % 2 == 1 ? sorted[sorted.Length / 2] : (sorted[sorted.Length / 2 - 1] + sorted[sorted.Length / 2]) / 2;
    }

    [DebugCommand("edits.run", Description = "Runs each edit case the given number of times and reports median and worst ApplyEdits and RefreshScene milliseconds with the Engine's breakdown of the last run.")]
    public string RunCases(int repeats = DefaultRepeats)
    {
        if (repeats < 1) throw new ArgumentOutOfRangeException(nameof(repeats));
        StringBuilder report = new(Inspect());
        foreach (EditCase edit in Cases())
        {
            Sample[] samples = Enumerable.Range(0, repeats).Select(_ => Run(edit)).ToArray();
            Sample last = samples[^1];
            report.AppendLine().Append(CultureInfo.InvariantCulture,
                $"{edit.Name}: cells={edit.Batches.Sum(batch => batch.Length)} " +
                $"applyMs median={Median(samples.Select(sample => sample.ApplyMs)):F2} max={samples.Max(sample => sample.ApplyMs):F2} " +
                $"refreshMs median={Median(samples.Select(sample => sample.RefreshMs)):F2} max={samples.Max(sample => sample.RefreshMs):F2} | " +
                $"rebuiltChunks={last.Receipt.RebuiltMeshChunks} meshUs={last.Receipt.MeshMicroseconds} " +
                $"colliderUs={last.Receipt.ColliderMicroseconds} settleUs={last.Receipt.SettleMicroseconds} | " +
                $"everyChunk={last.Readout.ProjectedEveryChunk} projected={last.Readout.ProjectedChunkCount} " +
                $"published={last.Readout.PublishedMeshCount} coarseBuilt={last.Readout.CoarseBuiltCount} " +
                $"coarsePlanned={last.Readout.CoarsePlannedCount} " +
                $"projectionUs={last.Readout.ProjectionMicroseconds}");
        }
        return report.ToString();
    }

    [DebugCommand("edits.inspect", Description = "Reports chunks, chunks drawn coarse, and admission and first projection milliseconds.")]
    public string Inspect()
    {
        VoxelScenePresentationReadout readout = engine.VoxelScenePresentation.RefreshScene(presentation!);
        return FormattableString.Invariant(
            $"chunks={readout.ChunkCount} coarse={readout.CoarseChunkCount} admissionMs={admissionMs:F0} projectionMs={projectionMs:F1}");
    }

    private Material CreateMaterial(Color color) => engine.Graphics.CreateMaterial(
        new MaterialRequest(color, default, 1, new Color(1, 1, 1, 1), default, 0, false));

    private static CameraDescriptor Descriptor(Vector3 eye, Vector3 target)
    {
        CameraQueries.TryLookAtPose(eye, target, 0, out CameraPose pose);
        return new(pose, CameraBasisMode.Derived, default,
            new(CameraProjectionKind.Perspective, FieldOfView, 0, NearClip, FarClip), CameraViewports.Full);
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
