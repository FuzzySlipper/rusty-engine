using System.Diagnostics;
using System.Globalization;
using System.Numerics;
using Rusty.Engine;
using Rusty.Engine.Debugging;

namespace CsharpLevelBuild;

/// <summary>
/// Navigation over the level: the generated blocks and a floor admitted as collision, then a collision-derived grid
/// over them, derived in the call (<c>inline</c>) or on Engine workers (<c>deferred</c>) while the updates go on.
/// </summary>
public sealed partial class Product
{
    private const ulong NavigationGrid = 1, ComparisonGrid = 2, FloorAsset = 1, FirstBandAsset = 2;
    // A one-metre box admitted and removed again to time a collision edit while a grid derives and after.
    private const ulong EditAsset = 10_000;
    private static readonly Vector3 EditAt = new(-8, 0, 40);
    // Columns past the blocks on every side: 289 blocks and 16 on each side make 321² = 103,041 cells.
    private const int NavigationMargin = 16;
    // The grid published before a deferred one, to answer until it installs.
    private const int PreviousExtent = 24;
    private const float NavigationCell = 1, AgentRadius = .3f, AgentHeight = 1.6f, AgentStep = .5f;
    private const float BelowFloor = 1, AboveBlocks = 3;
    private const uint NavigationQueryVisited = 1 << 18;
    // Query pairs on the floor, in metres: the first within the previous grid, the rest across the level.
    private static readonly (Vector2 From, Vector2 To)[] NavigationQueries =
    [
        (new(-8, -8), new(-8, 20)),
        (new(-8, -8), new(298, 298)),
        (new(-8, 150), new(150, 150)),
        (new(298, -8), new(-8, 298)),
    ];

    private SpatialSession? navigationSession;
    private Navigating? navigating;
    private string lastNavigationReport = "no navigation yet";

    private enum NavigationPhase { Geometry, Admitting, Previous, Calling, Pending, Settling }

    /// <summary>One navigation run: its phase and what was measured.</summary>
    private sealed class Navigating(Mode mode, bool edit, long started)
    {
        public Mode Mode { get; } = mode;
        public bool Edit { get; } = edit;
        public double EditWhileDerivingMs { get; set; } = double.NaN;
        public double EditAfterMs { get; set; } = double.NaN;
        public long Started { get; } = started;
        public NavigationPhase Phase { get; set; }
        public PreparationJob<Streams[]>? Job { get; set; }
        public ulong CollisionBatch { get; set; }
        public CollisionNavigationReplaceReceipt Previous { get; set; }
        public NavigationOutcome[] PreviousAnswers { get; set; } = [];
        public double CallMs { get; set; }
        public CollisionNavigationReplaceReceipt Installed { get; set; }
        public CollisionNavigationReplaceReceipt Synchronous { get; set; }
        public int PendingUpdates { get; set; }
        public int PendingAnsweredByPrevious { get; set; }
        public bool AnswersMatch { get; set; }
        public double ReadyMs { get; set; }
        public int SettleLeft { get; set; } = SettleUpdates;
        public List<double> CallbackMs { get; } = [];
    }

    /// <summary>What one query answered, beyond revision numbers.</summary>
    private readonly record struct NavigationOutcome(NavigationPathOutcome Outcome, uint Reached, uint Visited, Vector3 NextWaypoint);

    [DebugCommand("nav.start", Description = "Derives a collision-derived navigation grid of 103,041 cells over the level's blocks: mode inline (in the call) or deferred (on Engine workers). edit adds a box to the collision in the first update the grid is pending and removes it once it is installed, timing both. The first run admits the level's collision off the update first.")]
    public string StartNavigation(string mode = "deferred", string edit = "")
    {
        if (navigating is not null) return "navigation is running";
        if (build is not null) return "a build is running";
        Navigating next = new(mode == "inline" ? Mode.Inline : Mode.Deferred, edit == "edit", Stopwatch.GetTimestamp());
        if (navigationSession is null)
        {
            next.Job = jobs.Run(token => Geometry.Bands(DefaultBlocksPerSide, BlockSize, RowsPerBand, token));
        }
        else
        {
            next.Phase = NavigationPhase.Previous;
        }

        navigating = next;
        return $"navigation {mode}";
    }

    [DebugCommand("nav.report", Description = "The last navigation run's report: the grid, the call, the longest update until it was installed, and whether queries matched a synchronous derivation.")]
    public string NavigationReport() => $"{(navigating is null ? "idle" : $"running ({navigating.Phase})")}; {lastNavigationReport}";

    private void AdvanceNavigation(Navigating current)
    {
        switch (current.Phase)
        {
            case NavigationPhase.Geometry:
                if (!current.Job!.TryTake(out Streams[]? bands)) return;
                current.Job.Dispose();
                current.Job = null;
                navigationSession = engine.Spatial.CreateSession(new SpatialSessionConfig(1, 16, VoxelSurfaceMode.GreedyCubes));
                current.CollisionBatch = engine.Spatial.ApplyCollisionResidency(Collision(navigationSession, bands)).DeferredBatch;
                current.Phase = NavigationPhase.Admitting;
                return;
            case NavigationPhase.Admitting:
                CollisionResidencyReadout residency = engine.Spatial.ReadCollisionResidency(new(navigationSession!));
                if (residency.CompletedBatch < current.CollisionBatch) return;
                current.Phase = NavigationPhase.Previous;
                return;
            case NavigationPhase.Previous:
                // A small grid, synchronous, that answers while the whole one derives.
                current.Previous = engine.Spatial.ReplaceCollisionNavigation(new(navigationSession!,
                    new Vector3(-NavigationMargin, -BelowFloor, -NavigationMargin), new Vector3(PreviousExtent, AboveBlocks, PreviousExtent),
                    NavigationConfig(NavigationGrid)));
                current.PreviousAnswers = Ask(NavigationGrid);
                current.Phase = NavigationPhase.Calling;
                return;
            case NavigationPhase.Calling:
                long calling = Stopwatch.GetTimestamp();
                CollisionNavigationReplaceReceipt receipt = engine.Spatial.ReplaceCollisionNavigation(
                    WholeLevel(NavigationGrid) with { Deferred = current.Mode == Mode.Deferred });
                current.CallMs = Stopwatch.GetElapsedTime(calling).TotalMilliseconds;
                if (!receipt.Pending) current.Installed = receipt;
                current.Phase = NavigationPhase.Pending;
                return;
            case NavigationPhase.Pending:
                CollisionNavigationReadout readout = engine.Spatial.ReadCollisionNavigation(new(navigationSession!, NavigationGrid));
                if (readout.Pending)
                {
                    if (current.Edit && current.PendingUpdates == 0) current.EditWhileDerivingMs = EditCollision(add: true);
                    current.PendingUpdates++;
                    NavigationProjectionReadout projection = engine.Spatial.ReadNavigationProjection(new(navigationSession!, NavigationGrid));
                    if (projection.NavigationRevision == current.Previous.NavigationRevision
                        && projection.ProjectionHash == current.Previous.ProjectionHash
                        && Ask(NavigationGrid).SequenceEqual(current.PreviousAnswers))
                    {
                        current.PendingAnsweredByPrevious++;
                    }

                    return;
                }

                current.ReadyMs = Stopwatch.GetElapsedTime(current.Started).TotalMilliseconds;
                if (current.Mode == Mode.Deferred) current.Installed = readout.Installed;
                if (!double.IsNaN(current.EditWhileDerivingMs))
                {
                    // The same edit with no derivation holding the collision.
                    EditCollision(add: false);
                    current.EditAfterMs = EditCollision(add: true);
                    EditCollision(add: false);
                }
                current.Phase = NavigationPhase.Settling;
                return;
        }
    }

    /// <summary>
    /// Once the measured updates are done: the same grid derived in the call beside the installed one, and each
    /// query asked of both.
    /// </summary>
    private void CompareNavigation(Navigating current)
    {
        current.Synchronous = engine.Spatial.ReplaceCollisionNavigation(WholeLevel(ComparisonGrid));
        current.AnswersMatch = Ask(NavigationGrid).SequenceEqual(Ask(ComparisonGrid));
        engine.Spatial.ClearNavigation(new(navigationSession!, ComparisonGrid));
    }

    /// <summary>Adds or removes the edit box synchronously; returns the call's milliseconds.</summary>
    private double EditCollision(bool add)
    {
        Vector3[] corners = [EditAt, EditAt + Vector3.UnitX, EditAt + new Vector3(1, 0, 1), EditAt + Vector3.UnitZ,
            EditAt + Vector3.UnitY, EditAt + new Vector3(1, 1, 0), EditAt + Vector3.One, EditAt + new Vector3(0, 1, 1)];
        Triangle[] faces =
        [
            new(0, 2, 1), new(0, 3, 2), new(4, 5, 6), new(4, 6, 7), new(0, 1, 5), new(0, 5, 4),
            new(1, 2, 6), new(1, 6, 5), new(2, 3, 7), new(2, 7, 6), new(3, 0, 4), new(3, 4, 7),
        ];
        long started = Stopwatch.GetTimestamp();
        engine.Spatial.ApplyCollisionResidency(add
            ? new(navigationSession!, new StaticMeshAsset[] { new(EditAsset, default, 0, 8, 0, 12) }, corners, faces,
                new StaticMeshInstance[] { new(EditAsset, EditAsset, new(Vector3.Zero, Quaternion.Identity, Vector3.One)) },
                ReadOnlyMemory<ulong>.Empty, ReadOnlyMemory<ulong>.Empty)
            : new(navigationSession!, ReadOnlyMemory<StaticMeshAsset>.Empty, ReadOnlyMemory<Vector3>.Empty,
                ReadOnlyMemory<Triangle>.Empty, ReadOnlyMemory<StaticMeshInstance>.Empty, new[] { EditAsset }, new[] { EditAsset }));
        return Stopwatch.GetElapsedTime(started).TotalMilliseconds;
    }

    private NavigationOutcome[] Ask(ulong grid)
    {
        NavigationOutcome[] answers = new NavigationOutcome[NavigationQueries.Length];
        for (int index = 0; index < answers.Length; index++)
        {
            (Vector2 from, Vector2 to) = NavigationQueries[index];
            NavigationStepResult step = engine.Spatial.EvaluateNavigationStep(new(navigationSession!,
                new Vector3(from.X, 0, from.Y), new Vector3(to.X, 0, to.Y),
                NavigationCell, NavigationQueryVisited, grid));
            answers[index] = new(step.Outcome, step.Reached, step.Visited, step.NextWaypoint);
        }

        return answers;
    }

    private CollisionNavigationReplaceRequest WholeLevel(ulong grid) => new(navigationSession!,
        new Vector3(-NavigationMargin, -BelowFloor, -NavigationMargin),
        new Vector3(DefaultBlocksPerSide + NavigationMargin, AboveBlocks, DefaultBlocksPerSide + NavigationMargin),
        NavigationConfig(grid));

    private CollisionNavigationConfig NavigationConfig(ulong grid)
    {
        CharacterControllerConfig character = engine.Spatial.DefaultCharacterControllerConfig();
        character = character with
        {
            Shape = character.Shape with { StandingHeight = AgentHeight, Radius = AgentRadius },
            Surface = character.Surface with { MaximumStepHeight = AgentStep },
        };
        int side = DefaultBlocksPerSide + 2 * NavigationMargin;
        return engine.Spatial.DefaultCollisionNavigationConfig() with
        {
            GridId = grid,
            CellSize = NavigationCell,
            MaximumCells = (uint)(side * side),
            Character = character,
            MaximumDrop = AgentStep,
        };
    }

    /// <summary>The level's bands as static collision, one asset each, and a floor under them, deferred.</summary>
    private static CollisionResidencyRequest Collision(SpatialSession session, Streams[] bands)
    {
        int vertexCount = 4 + bands.Sum(band => band.Positions.Length);
        Vector3[] vertices = new Vector3[vertexCount];
        Triangle[] triangles = new Triangle[vertexCount / 3 + 2];
        StaticMeshAsset[] assets = new StaticMeshAsset[bands.Length + 1];
        StaticMeshInstance[] instances = new StaticMeshInstance[assets.Length];
        Transform identity = new(Vector3.Zero, Quaternion.Identity, Vector3.One);
        float low = -NavigationMargin, high = DefaultBlocksPerSide + NavigationMargin;
        vertices[0] = new(low, 0, low);
        vertices[1] = new(low, 0, high);
        vertices[2] = new(high, 0, high);
        vertices[3] = new(high, 0, low);
        triangles[0] = new(0, 1, 2);
        triangles[1] = new(0, 2, 3);
        assets[0] = new(FloorAsset, default, 0, 4, 0, 2);
        instances[0] = new(FloorAsset, FloorAsset, identity);
        int nextVertex = 4, nextTriangle = 2;
        for (int band = 0; band < bands.Length; band++)
        {
            Streams streams = bands[band];
            int firstVertex = nextVertex, firstTriangle = nextTriangle;
            streams.Positions.CopyTo(vertices, nextVertex);
            nextVertex += streams.Positions.Length;
            for (int index = 0; index < streams.Indices.Length; index += 3)
            {
                triangles[nextTriangle++] = new(streams.Indices[index], streams.Indices[index + 1], streams.Indices[index + 2]);
            }

            ulong id = FirstBandAsset + (ulong)band;
            assets[band + 1] = new(id, default, (uint)firstVertex, (uint)streams.Positions.Length, (uint)firstTriangle,
                (uint)(nextTriangle - firstTriangle));
            instances[band + 1] = new(id, id, identity);
        }

        return new(session, assets, vertices, triangles.AsMemory(0, nextTriangle), instances,
            ReadOnlyMemory<ulong>.Empty, ReadOnlyMemory<ulong>.Empty, true);
    }

    private static string NavigationReportOf(Navigating done)
    {
        double[] callbacks = done.CallbackMs.Order().ToArray();
        double Percentile(double p) => callbacks.Length == 0 ? 0 : callbacks[Math.Min(callbacks.Length - 1, (int)(p * callbacks.Length))];
        CollisionNavigationReplaceReceipt installed = done.Installed, synchronous = done.Synchronous;
        bool matches = installed.ProjectionHash == synchronous.ProjectionHash
            && installed.WalkableCellCount == synchronous.WalkableCellCount
            && installed.ComponentCount == synchronous.ComponentCount
            && done.AnswersMatch;
        string edit = done.Edit
            ? string.Create(CultureInfo.InvariantCulture, $"editWhileDerivingMs={done.EditWhileDerivingMs:F2} editAfterMs={done.EditAfterMs:F2} ")
            : "";
        return string.Create(CultureInfo.InvariantCulture,
            $"mode={done.Mode.ToString().ToLowerInvariant()} cells={installed.DerivedColumnCount + installed.ReusedColumnCount} walkable={installed.WalkableCellCount} components={installed.ComponentCount} " +
            $"edgeTests={installed.EdgeTestCount} derivationMs={installed.DerivationMicroseconds / 1000.0:F0} callMs={done.CallMs:F1} readyMs={done.ReadyMs:F0} " +
            $"pendingUpdates={done.PendingUpdates} answeredByPrevious={done.PendingAnsweredByPrevious} " +
            $"{edit}updates={callbacks.Length} callbackMs p50={Percentile(.5):F2} p99={Percentile(.99):F2} max={(callbacks.Length == 0 ? 0 : callbacks[^1]):F1} " +
            $"synchronousHash={synchronous.ProjectionHash:x16} installedHash={installed.ProjectionHash:x16} synchronousDerivationMs={synchronous.DerivationMicroseconds / 1000.0:F0} " +
            $"matchesSynchronous={matches}");
    }

    /// <summary>Advances a navigation run inside the update.</summary>
    private void UpdateNavigation()
    {
        if (navigating is { } current) AdvanceNavigation(current);
    }

    /// <summary>
    /// Records the update's time from the one that makes the call on, and reports once the grid has settled.
    /// </summary>
    private void MeasureNavigation(long entered)
    {
        if (navigating is not { Phase: NavigationPhase.Pending or NavigationPhase.Settling } current) return;
        current.CallbackMs.Add(Stopwatch.GetElapsedTime(entered).TotalMilliseconds);
        if (current.Phase == NavigationPhase.Settling && current.SettleLeft-- == 0)
        {
            CompareNavigation(current);
            lastNavigationReport = NavigationReportOf(current);
            Console.WriteLine($"LEVEL_NAVIGATION {lastNavigationReport}");
            navigating = null;
        }
    }

    private void DisposeNavigation()
    {
        navigating?.Job?.Dispose();
        navigating = null;
        navigationSession?.Dispose();
        navigationSession = null;
    }
}
