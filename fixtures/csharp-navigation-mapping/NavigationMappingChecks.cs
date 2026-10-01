using System;
using System.Numerics;
using Rusty.Engine;

namespace SdkPackageConsumer;

internal static class NavigationMappingChecks
{
    private const double CellSize = 0.5;
    private const uint ChunkSize = 8;
    private const ulong GridId = 93;
    private const uint MaximumCells = 64;
    private const uint MaximumVisited = 32;
    private const int FloorCells = 3;
    private const float FootClearance = 0.02f;
    private const ulong NoisyFloorAsset = 9_301;
    private const ulong NoisyFloorInstance = 9_302;
    private const float NoisyFloorLength = 4;
    // Each 0.5-unit column is a few nanometres higher than the last, as
    // mesh floors are after float rounding.
    private const float NoisyFloorRise = 1.0e-5f;

    private const int WallColumn = 3;
    private const int WallRow = 7;

    internal static void Run(IEngineContext engine)
    {
        CheckFloor(engine, new VoxelAddress(-21, -5, 35));
        CheckFloor(engine, new VoxelAddress(37, 4, -29));
        CheckNoisyMeshFloor(engine);
        CheckPolicyAndExplanations(engine);
    }

    // A 0.2-unit body on half-unit cells that steps and drops one cell.
    private static CollisionNavigationConfig Config(IEngineContext engine)
    {
        CollisionNavigationConfig defaults = engine.Spatial.DefaultCollisionNavigationConfig();
        CharacterControllerConfig body = defaults.Character with
        {
            Shape = defaults.Character.Shape with { Radius = 0.1f, StandingHeight = 0.4f, CrouchedHeight = 0.3f },
            Surface = defaults.Character.Surface with
            {
                MaximumStepHeight = (float)CellSize,
                MaximumSlopeRadians = 45 * MathF.PI / 180,
            },
        };
        return defaults with
        {
            GridId = GridId,
            CellSize = CellSize,
            ChunkSize = ChunkSize,
            MaximumCells = MaximumCells,
            Character = body,
            MaximumDrop = CellSize,
        };
    }

    // A row of floor with a wall two cells high across it: the route needs a
    // one-unit step up and drop down, and every refusal says why.
    private static void CheckPolicyAndExplanations(IEngineContext engine)
    {
        using SpatialSession session = engine.Spatial.CreateSession(new(CellSize, ChunkSize, VoxelSurfaceMode.GreedyCubes));
        var edits = new VoxelEdit[WallRow + 1 + 2];
        for (int x = 0; x <= WallRow; x++)
            edits[x] = new(VoxelEditKind.Set, new(x, 0, 0), 1);
        edits[WallRow + 1] = new(VoxelEditKind.Set, new(WallColumn, 1, 0), 1);
        edits[WallRow + 2] = new(VoxelEditKind.Set, new(WallColumn, 2, 0), 1);
        Require(engine.Voxel.ApplyEdits(new(session, edits)).Status == VoxelEditStatus.Accepted, "wall admission failed");
        Vector3 worldMin = new(0, -1, 0), worldMax = new((WallRow + 1) * (float)CellSize, 3, (float)CellSize);
        Vector3 west = new(0.25f, 0.5f, 0.25f), east = new(WallRow * (float)CellSize + 0.25f, 0.5f, 0.25f);
        CollisionNavigationConfig config = Config(engine);
        engine.Spatial.ReplaceCollisionNavigation(new(session, worldMin, worldMax, config));

        NavigationStepResult blocked = engine.Spatial.EvaluateNavigationStep(new(session, west, east, (float)CellSize, MaximumVisited));
        Require(blocked.Outcome == NavigationPathOutcome.NoPath && blocked.Visited == WallColumn && blocked.NearestPresent
                && blocked.NearestCell == new PlanarNavCell(WallColumn - 1, 1, 0) && blocked.Nearest.X == 1.25f,
            $"NoPath did not say how far it got: {blocked}");
        PlanarNavCell beforeWall = new(WallColumn - 1, 1, 0), wallTop = new(WallColumn, 3, 0);
        CollisionNavigationEdgeReadout tooHigh = engine.Spatial.ExplainCollisionNavigationEdge(new(session, beforeWall, wallTop));
        Require(tooHigh.Outcome == CollisionNavigationEdgeOutcome.NotNeighbor && !tooHigh.Admitted && tooHigh.ToY == 1.5,
            $"a two-cell rise was not explained: {tooHigh}");
        CollisionNavigationColumnResult wall = engine.Spatial.ExplainCollisionNavigationColumn(new(session, WallColumn, 0));
        Require(wall.Samples.Length == 1 && !wall.BudgetExhausted
                && wall.Samples.Span[0].Outcome == CollisionNavigationSampleOutcome.Support && wall.Samples.Span[0].StandingY == 1.5,
            "the wall column was not explained as one support on its top");

        // A one-unit step and drop cross the wall.
        config = config with
        {
            Character = config.Character with { Surface = config.Character.Surface with { MaximumStepHeight = 1 } },
            MaximumDrop = 1,
        };
        engine.Spatial.ReplaceCollisionNavigation(new(session, worldMin, worldMax, config));
        Require(engine.Spatial.EvaluateNavigationStep(new(session, west, east, (float)CellSize, MaximumVisited)).Outcome == NavigationPathOutcome.Reached,
            "a step and drop of one unit did not cross the wall");
        CollisionNavigationEdgeReadout step = engine.Spatial.ExplainCollisionNavigationEdge(new(session, beforeWall, wallTop));
        Require(step.Outcome == CollisionNavigationEdgeOutcome.Traversable && step.Admitted, $"the step was not admitted: {step}");

        // A start a fifth of a unit above the floor stands on it only within the snap.
        Vector3 hovering = west + new Vector3(0, 0.2f, 0);
        Require(engine.Spatial.EvaluateNavigationStep(new(session, hovering, east, (float)CellSize, MaximumVisited)).Outcome == NavigationPathOutcome.StartNotWalkable,
            "a start beyond the default snap stood on the floor");
        engine.Spatial.ReplaceCollisionNavigation(new(session, worldMin, worldMax, config with { SnapAbove = 0.25 }));
        Require(engine.Spatial.EvaluateNavigationStep(new(session, hovering, east, (float)CellSize, MaximumVisited)).Outcome == NavigationPathOutcome.Reached,
            "a start within the snap did not stand on the floor");
    }

    private static void CheckNoisyMeshFloor(IEngineContext engine)
    {
        using SpatialSession session = engine.Spatial.CreateSession(new(CellSize, ChunkSize, VoxelSurfaceMode.GreedyCubes));
        Vector3[] vertices =
        [
            new(0, 0, 0), new(0, 0, 1),
            new(NoisyFloorLength, NoisyFloorRise, 0), new(NoisyFloorLength, NoisyFloorRise, 1),
        ];
        Triangle[] triangles = [new(0, 1, 2), new(2, 1, 3)];
        engine.Spatial.ApplyCollisionResidency(new CollisionResidencyRequest(
            session,
            new[] { new StaticMeshAsset(NoisyFloorAsset, default, 0, (uint)vertices.Length, 0, (uint)triangles.Length) },
            vertices,
            triangles,
            new[] { new StaticMeshInstance(NoisyFloorInstance, NoisyFloorAsset, new Transform(Vector3.Zero, Quaternion.Identity, Vector3.One)) },
            ReadOnlyMemory<ulong>.Empty,
            ReadOnlyMemory<ulong>.Empty));
        engine.Spatial.ReplaceCollisionNavigation(new(session, new(0, -1, 0), new(NoisyFloorLength, 2, 1), Config(engine)));

        Vector3 west = new(0.25f, FootClearance, 0.25f);
        Vector3 east = new(NoisyFloorLength - 0.25f, FootClearance, 0.25f);
        NavigationStepResult rising = engine.Spatial.EvaluateNavigationStep(new(session, west, east, (float)CellSize, MaximumVisited));
        Require(rising.Outcome == NavigationPathOutcome.Reached && rising.Path.Length == 8,
            $"a floor rising by float noise was not level: {rising.Outcome}");
        NavigationStepResult falling = engine.Spatial.EvaluateNavigationStep(new(session, east, west, (float)CellSize, MaximumVisited));
        Require(falling.Outcome == NavigationPathOutcome.Reached && falling.Path.Length == 8,
            $"a floor falling by float noise was not level: {falling.Outcome}");
    }

    private static void CheckFloor(IEngineContext engine, VoxelAddress first)
    {
        using SpatialSession session = engine.Spatial.CreateSession(new(CellSize, ChunkSize, VoxelSurfaceMode.GreedyCubes));
        var edits = new VoxelEdit[FloorCells];
        for (int index = 0; index < FloorCells; index++)
            edits[index] = new(VoxelEditKind.Set, new(first.X + index, first.Y, first.Z), 1);
        VoxelEditReceipt edit = engine.Voxel.ApplyEdits(new(session, edits));
        Require(edit.Status == VoxelEditStatus.Accepted, "floor admission failed");

        float supportY = (float)((first.Y + 1) * CellSize);
        Vector3 firstSupport = new((float)((first.X + 0.5) * CellSize), supportY, (float)((first.Z + 0.5) * CellSize));
        Vector3 lastSupport = firstSupport + new Vector3((float)((FloorCells - 1) * CellSize), 0, 0);
        // Deliberately nonzero and not aligned to either a cell or chunk boundary.
        Vector3 worldMin = firstSupport - new Vector3(0.35f, 1, 0.35f);
        Vector3 worldMax = lastSupport + new Vector3(0.35f, 2, 0.35f);
        CollisionNavigationConfig config = Config(engine);
        CollisionNavigationReplaceReceipt projection = engine.Spatial.ReplaceCollisionNavigation(new(session, worldMin, worldMax, config));
        Require(projection.WalkableCellCount == FloorCells, "unexpected reported walkable count");
        CollisionNavigationReplaceReceipt republished = engine.Spatial.ReplaceCollisionNavigation(new(session, worldMin, worldMax, config));
        Require(republished.DerivedColumnCount == 0 && republished.ReusedColumnCount == projection.DerivedColumnCount
                && republished.ProjectionHash == projection.ProjectionHash,
            "an unchanged republication derived columns again");

        ulong accepted = 0;
        for (int index = 0; index < FloorCells; index++)
        {
            Vector3 support = firstSupport + new Vector3((float)(index * CellSize), 0, 0);
            PlanarNavCell cell = CellAtSupport(support);
            Require(cell == new PlanarNavCell(first.X + index, first.Y + 1, first.Z), "world mapping lost signed coordinates");
            NavigationPathResult self = engine.Spatial.RequestNavigationPath(new(session, cell, cell, MaximumVisited));
            Require(self.Outcome == NavigationPathOutcome.Reached && self.Path.Length == 1,
                "reported walkable cell cannot be queried using world-aligned coordinates");
            accepted++;
        }
        Require(accepted == projection.WalkableCellCount, "queryable cells disagree with receipt count");

        PlanarNavCell start = CellAtSupport(firstSupport);
        PlanarNavCell goal = CellAtSupport(lastSupport);
        NavigationPathResult route = engine.Spatial.RequestNavigationPath(new(session, start, goal, MaximumVisited));
        Require(route.Outcome == NavigationPathOutcome.Reached && route.Path.Length == FloorCells,
            "world-aligned endpoints did not produce a multi-cell path");
        PlanarNavCell middle = route.Path.Span[1];
        Require(middle == new PlanarNavCell(first.X + 1, first.Y + 1, first.Z),
            "returned path cell changed coordinate space");

        PlanarNavCell relative = CellAtSupport(firstSupport - worldMin);
        NavigationPathResult wrongOrigin = engine.Spatial.RequestNavigationPath(new(session, relative, relative, MaximumVisited));
        Require(wrongOrigin.Outcome == NavigationPathOutcome.StartNotWalkable,
            "fixture did not reproduce the publication-box-relative mapping error");

        Vector3 clearance = new(0, FootClearance, 0);
        NavigationStepResult step = engine.Spatial.EvaluateNavigationStep(new(session,
            firstSupport + clearance, lastSupport + clearance, (float)CellSize, MaximumVisited));
        Require(step.Outcome == NavigationPathOutcome.Reached && step.Path.Length == FloorCells
            && step.NextPathCell == middle && step.NextWaypoint.X > firstSupport.X,
            "world-position query failed to resolve support and advance toward the next cell");
    }

    private static PlanarNavCell CellAtSupport(Vector3 support) => new(
        (long)Math.Floor((double)support.X / CellSize),
        (long)Math.Floor((double)support.Y / CellSize),
        (long)Math.Floor((double)support.Z / CellSize));

    private static void Require(bool condition, string message)
    {
        if (!condition) throw new InvalidOperationException($"Navigation mapping: {message}");
    }
}
