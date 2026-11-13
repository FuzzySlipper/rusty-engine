using System.Diagnostics;
using System.Globalization;
using System.Numerics;
using System.Text;
using Rusty.Engine;
using Rusty.Engine.Debugging;

namespace CsharpLevelBuild;

/// <summary>
/// Builds a generated level of about a million faceted triangles, a field of stone blocks, and
/// shows it in parts, either all in one update (<c>inline</c>) or off the update (<c>deferred</c>):
/// the geometry on the worker lane, then the mesh created and cut deferred, its source destroyed
/// in the same call, and its parts taken deferred. The update's pacing is recorded while it builds.
/// </summary>
public sealed class Product : IEngineProduct, IDebugCommandModuleSource, IDebugCommandModule
{
    // 289² blocks of 12 triangles: 1,002,252 triangles, 3,006,756 vertices.
    private const int DefaultBlocksPerSide = 289;
    private const float Spacing = 1f, BlockSize = .8f, BlockHeight = .6f, HeightJitter = .5f;
    private const float PartCell = 32f;
    private const int TrianglesPerBlock = 12, VerticesPerBlock = TrianglesPerBlock * 3;
    private const ulong StepObjectId = 1, FirstPartObjectId = 100;
    private const double FieldOfView = 60, NearClip = .1, FarClip = 1000;
    // A step slower than this many fixed steps is a hitch.
    private const double HitchSteps = 2;
    // Updates still measured after a build is shown, so the one after an inline build's counts.
    private const int SettleUpdates = 30;
    private static readonly Vector3 Eye = new(-40, 120, -40), Target = new(150, 0, 150);
    private static readonly Color StoneColor = new(.55f, .52f, .48f, 1), MarkerColor = new(.9f, .3f, .2f, 1);

    private readonly IEngineContext engine;
    private readonly PreparationJobs jobs;
    private readonly Material stone, markerMaterial;
    private readonly Camera camera;
    private readonly MeshResource markerMesh;
    private readonly Appearance marker;
    private readonly List<(MeshResource Mesh, Appearance Appearance)> parts = [];
    private Build? build;
    private string lastReport = "no build yet";
    private long updates;

    public Product(ProductCreateContext context)
    {
        engine = context.Engine;
        jobs = context.Jobs;
        stone = engine.Graphics.CreateMaterial(new MaterialRequest(StoneColor, default, 1, new Color(1, 1, 1, 1), default, 0, false));
        markerMaterial = engine.Graphics.CreateMaterial(new MaterialRequest(MarkerColor, default, 1, new Color(1, 1, 1, 1), default, 0, false));
        camera = engine.CameraView.CreateCamera(Descriptor(Eye, Target));
        engine.CameraView.SetActiveCamera(camera);
        // A block that turns every update, so a frozen update shows.
        Streams one = Geometry.Blocks(1, BlockSize, CancellationToken.None);
        markerMesh = engine.Graphics.CreateMeshResource(one.Request(markerMaterial, deferred: false));
        marker = engine.Graphics.CreateMeshAppearance(markerMesh);
    }

    private enum Mode { Inline, Deferred }

    private enum Phase { Building, Cutting, Uploading, Settling }

    /// <summary>One build: its phase, its handles and what was measured.</summary>
    private sealed class Build(Mode mode, int blocksPerSide, long started)
    {
        public Mode Mode { get; } = mode;
        public int BlocksPerSide { get; } = blocksPerSide;
        public long Started { get; } = started;
        public Phase Phase { get; set; }
        public PreparationJob<Streams>? Job { get; set; }
        public MeshPartition? Partition { get; set; }
        public ulong Batch { get; set; }
        public int Triangles { get; set; }
        public double GeometryMs { get; set; }
        public double CreateMs { get; set; }
        public double PartitionMs { get; set; }
        public double TakeMs { get; set; }
        public double CutReadyMs { get; set; }
        public double ReadyMs { get; set; }
        public int SettleLeft { get; set; } = SettleUpdates;
        public List<double> CallbackMs { get; } = [];
        public List<double> IntervalMs { get; } = [];
        public ulong DroppedSteps { get; set; }
        public ulong AdmittedSteps { get; set; }
        public long LastUpdate { get; set; } = started;
    }

    public void Start() => Publish();

    public ProductUpdateResult Update(ProductUpdate update)
    {
        long entered = Stopwatch.GetTimestamp();
        updates++;
        if (build is { } current)
        {
            current.IntervalMs.Add(Stopwatch.GetElapsedTime(current.LastUpdate, entered).TotalMilliseconds);
            current.LastUpdate = entered;
            current.DroppedSteps += update.Facts.DroppedStepCount;
            current.AdmittedSteps += update.Facts.AdmittedStepCount;
            Advance(current);
        }

        Publish();
        if (build is { } measured)
        {
            measured.CallbackMs.Add(Stopwatch.GetElapsedTime(entered).TotalMilliseconds);
            if (measured.Phase == Phase.Settling && measured.SettleLeft-- == 0)
            {
                lastReport = Report(measured, update.Facts.FixedDeltaSeconds * 1000);
                Console.WriteLine($"LEVEL_BUILD {lastReport}");
                build = null;
            }
        }

        return ProductUpdateResult.None;
    }

    private void Advance(Build current)
    {
        switch (current.Phase)
        {
            case Phase.Building when current.Mode == Mode.Inline:
                Streams level = Geometry.Blocks(current.BlocksPerSide, BlockSize, CancellationToken.None);
                current.GeometryMs = Stopwatch.GetElapsedTime(current.Started).TotalMilliseconds;
                CreateAndCut(current, level, deferred: false);
                current.CutReadyMs = Stopwatch.GetElapsedTime(current.Started).TotalMilliseconds;
                TakeParts(current);
                current.ReadyMs = Stopwatch.GetElapsedTime(current.Started).TotalMilliseconds;
                current.Phase = Phase.Settling;
                return;
            case Phase.Building:
                if (!current.Job!.TryTake(out Streams? streams)) return;
                current.GeometryMs = Stopwatch.GetElapsedTime(current.Started).TotalMilliseconds;
                CreateAndCut(current, streams, deferred: true);
                current.Job.Dispose();
                current.Job = null;
                current.Phase = Phase.Cutting;
                return;
            case Phase.Cutting:
                if (!engine.Graphics.ReadMeshPartition(current.Partition!).Ready) return;
                current.CutReadyMs = Stopwatch.GetElapsedTime(current.Started).TotalMilliseconds;
                TakeParts(current);
                current.Batch = engine.Graphics.ReadResourcePreparation().LastBatch;
                current.Phase = Phase.Uploading;
                return;
            case Phase.Uploading:
                if (engine.Graphics.ReadResourcePreparation().CompletedBatch < current.Batch) return;
                current.ReadyMs = Stopwatch.GetElapsedTime(current.Started).TotalMilliseconds;
                current.Phase = Phase.Settling;
                return;
        }
    }

    /// <summary>Creates the whole mesh, cuts it into parts and destroys it, all in this call.</summary>
    private void CreateAndCut(Build current, Streams streams, bool deferred)
    {
        current.Triangles = streams.Indices.Length / 3;
        long started = Stopwatch.GetTimestamp();
        using MeshResource whole = engine.Graphics.CreateMeshResource(streams.Request(stone, deferred));
        current.CreateMs = Stopwatch.GetElapsedTime(started).TotalMilliseconds;
        started = Stopwatch.GetTimestamp();
        current.Partition = engine.Graphics.PartitionMesh(new MeshPartitionRequest(whole, Vector3.Zero, new Vector3(PartCell), deferred));
        current.PartitionMs = Stopwatch.GetElapsedTime(started).TotalMilliseconds;
    }

    private void TakeParts(Build current)
    {
        long started = Stopwatch.GetTimestamp();
        uint count = engine.Graphics.ReadMeshPartition(current.Partition!).PartCount;
        for (uint index = 0; index < count; index++)
        {
            MeshResource part = engine.Graphics.TakeMeshPartitionPart(new MeshPartitionPartRequest(current.Partition!, index));
            parts.Add((part, engine.Graphics.CreateMeshAppearance(part)));
        }

        current.Partition!.Dispose();
        current.Partition = null;
        current.TakeMs = Stopwatch.GetElapsedTime(started).TotalMilliseconds;
    }

    [DebugCommand("build.start", Description = "Builds the level: mode inline (all in one update) or deferred (worker lane, deferred create, cut and parts). blocks per side, 289 for about a million triangles.")]
    public string Start(string mode = "deferred", int blocksPerSide = DefaultBlocksPerSide)
    {
        if (build is not null) return "a build is running; build.cancel first";
        Clear();
        Build next = new(mode == "inline" ? Mode.Inline : Mode.Deferred, blocksPerSide, Stopwatch.GetTimestamp());
        // An inline build runs whole in the next update; a deferred one starts its job now.
        if (next.Mode == Mode.Deferred) next.Job = jobs.Run(token => Geometry.Blocks(blocksPerSide, BlockSize, token));
        build = next;
        return $"building {blocksPerSide}² blocks {mode}";
    }

    [DebugCommand("build.cancel", Description = "Cancels a running build at whatever phase it is in, then reports the Graphics counts.")]
    public string Cancel()
    {
        string phase = build?.Phase.ToString() ?? "none";
        if (build is { } current)
        {
            current.Job?.Dispose();
            current.Partition?.Dispose();
            build = null;
        }

        Clear();
        return $"cancelled during {phase}; {Counts()}";
    }

    [DebugCommand("build.report", Description = "The last build's report: triangles, phase milliseconds and the update's pacing while it built.")]
    public string LastReport() => $"{(build is null ? "idle" : $"building ({build.Phase})")}; {lastReport}; {Counts()}";

    private string Counts()
    {
        PresentationReadout presentation = engine.Graphics.ReadPresentation();
        ResourcePreparationReadout preparation = engine.Graphics.ReadResourcePreparation();
        return FormattableString.Invariant(
            $"parts={parts.Count} objects={presentation.RetainedObjectCount} appearances={presentation.AppearanceCount} resources={presentation.ResourceCount} batches={preparation.CompletedBatch}/{preparation.LastBatch} pending={preparation.PendingResources} failed={preparation.FailedResources} updates={updates}");
    }

    private static string Report(Build done, double stepMs)
    {
        double[] callbacks = done.CallbackMs.Order().ToArray();
        double[] intervals = done.IntervalMs.Order().ToArray();
        double Percentile(double[] sorted, double p) => sorted.Length == 0 ? 0 : sorted[Math.Min(sorted.Length - 1, (int)(p * sorted.Length))];
        int hitches = stepMs <= 0 ? 0 : intervals.Count(interval => interval > HitchSteps * stepMs);
        return string.Create(CultureInfo.InvariantCulture,
            $"mode={done.Mode.ToString().ToLowerInvariant()} triangles={done.Triangles} geometryMs={done.GeometryMs:F0} createMs={done.CreateMs:F1} partitionMs={done.PartitionMs:F1} " +
            $"cutReadyMs={done.CutReadyMs:F0} takeMs={done.TakeMs:F1} readyMs={done.ReadyMs:F0} updates={callbacks.Length} " +
            $"admittedSteps={done.AdmittedSteps} droppedSteps={done.DroppedSteps} " +
            $"callbackMs p50={Percentile(callbacks, .5):F2} p99={Percentile(callbacks, .99):F2} max={(callbacks.Length == 0 ? 0 : callbacks[^1]):F1} " +
            $"intervalMs p50={Percentile(intervals, .5):F1} p99={Percentile(intervals, .99):F1} max={(intervals.Length == 0 ? 0 : intervals[^1]):F1} hitches={hitches}");
    }

    /// <summary>Stops showing the level's parts, then disposes them.</summary>
    private void Clear()
    {
        (MeshResource Mesh, Appearance Appearance)[] shown = [.. parts];
        parts.Clear();
        Publish();
        foreach ((MeshResource mesh, Appearance appearance) in shown)
        {
            appearance.Dispose();
            mesh.Dispose();
        }
    }

    private void Publish()
    {
        float angle = updates * .05f;
        List<AppearanceFact> facts =
        [
            new(StepObjectId, false, 0, new Transform(new Vector3(0, 4, 0), Quaternion.CreateFromAxisAngle(Vector3.UnitY, angle), new Vector3(6)),
                marker, true, RenderLayer.Scene),
        ];
        // Parts show once the build is ready: until then the last level, or nothing, stays.
        if (build is null || build.Phase == Phase.Settling)
        {
            for (int index = 0; index < parts.Count; index++)
            {
                facts.Add(new(FirstPartObjectId + (ulong)index, false, 0, new Transform(Vector3.Zero, Quaternion.Identity, Vector3.One),
                    parts[index].Appearance, true, RenderLayer.Scene));
            }
        }

        engine.Graphics.PublishSnapshot(facts.ToArray());
    }

    private static CameraDescriptor Descriptor(Vector3 eye, Vector3 target)
    {
        CameraQueries.TryLookAtPose(eye, target, 0, out CameraPose pose);
        return new(pose, CameraBasisMode.Derived, default,
            new(CameraProjectionKind.Perspective, FieldOfView, 0, NearClip, FarClip), CameraViewports.Full);
    }

    public void RegisterDebugCommands(IDebugCommandModuleRegistrar registrar) => registrar.Register(this);
    public void Pause() { }
    public void Resume() { }

    /// <summary>A restart ends the build: its job is cancelled with the generation.</summary>
    public void Restart()
    {
        build?.Partition?.Dispose();
        build = null;
        Clear();
    }

    public void Shutdown() { }

    public void Dispose()
    {
        build?.Job?.Dispose();
        build?.Partition?.Dispose();
        Clear();
        marker.Dispose();
        markerMesh.Dispose();
        camera.Dispose();
        markerMaterial.Dispose();
        stone.Dispose();
    }

    private const int StonesSalt = 7919;

    /// <summary>A level's streams, built off the update and read on it.</summary>
    private sealed record Streams(Vector3[] Positions, Vector3[] Normals, uint[] Indices)
    {
        public MeshResourceCreateRequest Request(Material material, bool deferred) =>
            new MeshResourceCreateRequest(Positions, Normals, ReadOnlyMemory<Vector2>.Empty, Indices,
                new MeshGroup[] { new(0, 0, (uint)Indices.Length) }, new MeshMaterialBinding[] { new(0, material) }) with { Deferred = deferred };
    }

    /// <summary>Pure product geometry: reads nothing shared and calls no Engine service.</summary>
    private static class Geometry
    {
        private static readonly (Vector3 Normal, Vector3 U, Vector3 V)[] Faces =
        [
            // u × v is the normal, so each face winds counter-clockwise seen from outside.
            (Vector3.UnitX, Vector3.UnitY, Vector3.UnitZ), (-Vector3.UnitX, Vector3.UnitZ, Vector3.UnitY),
            (Vector3.UnitY, Vector3.UnitZ, Vector3.UnitX), (-Vector3.UnitY, Vector3.UnitX, Vector3.UnitZ),
            (Vector3.UnitZ, Vector3.UnitX, Vector3.UnitY), (-Vector3.UnitZ, Vector3.UnitY, Vector3.UnitX),
        ];

        public static Streams Blocks(int perSide, float size, CancellationToken token)
        {
            int blocks = perSide * perSide;
            Vector3[] positions = new Vector3[blocks * VerticesPerBlock];
            Vector3[] normals = new Vector3[positions.Length];
            uint[] indices = new uint[positions.Length];
            int next = 0;
            for (int z = 0; z < perSide; z++)
            {
                token.ThrowIfCancellationRequested();
                for (int x = 0; x < perSide; x++)
                {
                    float height = BlockHeight + HeightJitter * Hash(x, z);
                    Vector3 centre = new(x * Spacing, height / 2, z * Spacing);
                    Vector3 half = new(size / 2, height / 2, size / 2);
                    foreach ((Vector3 normal, Vector3 u, Vector3 v) in Faces)
                    {
                        Vector3 face = centre + normal * half;
                        Vector3 du = u * half, dv = v * half;
                        Vector3 a = face - du - dv, b = face + du - dv, c = face + du + dv, d = face - du + dv;
                        foreach (Vector3 corner in (ReadOnlySpan<Vector3>)[a, b, c, a, c, d])
                        {
                            positions[next] = corner;
                            normals[next] = normal;
                            indices[next] = (uint)next;
                            next++;
                        }
                    }
                }
            }

            return new Streams(positions, normals, indices);
        }

        private static float Hash(int x, int z)
        {
            uint h = (uint)(x * 73856093) ^ (uint)(z * 19349663) ^ StonesSalt;
            h ^= h >> 13;
            h *= 0x5bd1e995;
            h ^= h >> 15;
            return (h & 0xffff) / 65535f;
        }
    }
}
