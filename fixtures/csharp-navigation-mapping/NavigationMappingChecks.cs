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

    internal static void Run(IEngineContext engine)
    {
        CheckFloor(engine, new VoxelAddress(-21, -5, 35));
        CheckFloor(engine, new VoxelAddress(37, 4, -29));
        CheckNoisyMeshFloor(engine);
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
        CollisionNavigationConfig config = new(GridId, CellSize, ChunkSize, 1, 0.1, 0.4, 45, MaximumCells);
        engine.Spatial.ReplaceCollisionNavigation(new(session, new(0, -1, 0), new(NoisyFloorLength, 2, 1), config));

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
        CollisionNavigationConfig config = new(GridId, CellSize, ChunkSize, 1, 0.1, 0.4, 45, MaximumCells);
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
