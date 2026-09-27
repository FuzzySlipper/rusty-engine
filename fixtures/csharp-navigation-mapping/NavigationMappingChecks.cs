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

    internal static void Run(IEngineContext engine)
    {
        CheckFloor(engine, new VoxelAddress(-21, -5, 35));
        CheckFloor(engine, new VoxelAddress(37, 4, -29));
    }

    private static void CheckFloor(IEngineContext engine, VoxelAddress first)
    {
        using SpatialSession session = engine.Spatial.CreateSession(new(CellSize, ChunkSize, VoxelSurfaceMode.GreedyCubes));
        VoxelSceneReadout empty = engine.Voxel.ReadScene(new(session));
        var edits = new VoxelEdit[FloorCells];
        for (int index = 0; index < FloorCells; index++)
            edits[index] = new(VoxelEditKind.Set, new(first.X + index, first.Y, first.Z), 1);
        VoxelEditReceipt edit = engine.Voxel.ApplyEdits(new(session, empty.SourceRevision, edits));
        Require(edit.Status == VoxelEditStatus.Accepted, "floor admission failed");

        float supportY = (float)((first.Y + 1) * CellSize);
        Vector3 firstSupport = new((float)((first.X + 0.5) * CellSize), supportY, (float)((first.Z + 0.5) * CellSize));
        Vector3 lastSupport = firstSupport + new Vector3((float)((FloorCells - 1) * CellSize), 0, 0);
        // Deliberately nonzero and not aligned to either a cell or chunk boundary.
        Vector3 worldMin = firstSupport - new Vector3(0.35f, 1, 0.35f);
        Vector3 worldMax = lastSupport + new Vector3(0.35f, 2, 0.35f);
        CollisionNavigationConfig config = new(GridId, CellSize, ChunkSize, 1, 0.1, 0.4, 45, MaximumCells);
        NavigationReplaceReceipt projection = engine.Spatial.ReplaceCollisionNavigation(new(session, worldMin, worldMax, config));
        Require(projection.WalkableCellCount == FloorCells, "unexpected reported walkable count");

        ulong accepted = 0;
        for (int index = 0; index < FloorCells; index++)
        {
            Vector3 support = firstSupport + new Vector3((float)(index * CellSize), 0, 0);
            PlanarNavCell cell = CellAtSupport(support);
            Require(cell == new PlanarNavCell(first.X + index, first.Y + 1, first.Z), "world mapping lost signed coordinates");
            NavigationPathReadout self = engine.Spatial.RequestNavigationPath(new(session, cell, cell, MaximumVisited));
            Require(self.Outcome == NavigationPathOutcome.Reached && self.PathLen == 1,
                "reported walkable cell cannot be queried using world-aligned coordinates");
            accepted++;
        }
        Require(accepted == projection.WalkableCellCount, "queryable cells disagree with receipt count");

        PlanarNavCell start = CellAtSupport(firstSupport);
        PlanarNavCell goal = CellAtSupport(lastSupport);
        NavigationPathReadout route = engine.Spatial.RequestNavigationPath(new(session, start, goal, MaximumVisited));
        Require(route.Outcome == NavigationPathOutcome.Reached && route.PathLen == FloorCells,
            "world-aligned endpoints did not produce a multi-cell path");
        NavigationPathCellAtReceipt middle = engine.Spatial.ReadNavigationPathCellAt(new(session, 1));
        Require(middle.Present && middle.Cell == new PlanarNavCell(first.X + 1, first.Y + 1, first.Z),
            "returned path cell changed coordinate space");

        PlanarNavCell relative = CellAtSupport(firstSupport - worldMin);
        NavigationPathReadout wrongOrigin = engine.Spatial.RequestNavigationPath(new(session, relative, relative, MaximumVisited));
        Require(wrongOrigin.Outcome == NavigationPathOutcome.StartNotWalkable,
            "fixture did not reproduce the publication-box-relative mapping error");

        Vector3 clearance = new(0, FootClearance, 0);
        NavigationStepReceipt step = engine.Spatial.EvaluateNavigationStep(new(session,
            firstSupport + clearance, lastSupport + clearance, (float)CellSize, MaximumVisited));
        Require(step.Outcome == NavigationPathOutcome.Reached && step.PathLen == FloorCells
            && step.NextPathCell == middle.Cell && step.NextWaypoint.X > firstSupport.X,
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
