using System.Numerics;
using Rusty.Engine;
using Rusty.Engine.Debugging;

namespace CsharpSurfaceCrawler;

/// <summary>
/// A surface crawler routed across a trench (#10967). The room's floor and side wall are cut in two
/// by a bottomless trench, so the halves join only over the ceiling: the Engine's surface grid
/// routes the crawler up a wall, across the ceiling and down the far side, and the product steers
/// the crawler at each waypoint with <c>Spatial.ProposeCrawlerStep</c>, drawing a body longer than
/// its ball turned by the step's rotation. The room is open at the front for the camera.
/// </summary>
public sealed class Product : IEngineProduct, IDebugCommandModuleSource, IDebugCommandModule
{
    private const float VoxelSize = 1;
    private const uint ChunkSize = 8;
    private const uint StoneSlot = 1;
    private const ulong Grid = 1;
    // The room's shell, in voxels: inside x and z from -3 to 3, y from 1 to 5; the trench takes
    // x -1 and 0 below the ceiling, and the front wall (z = 3) is left out.
    private const int ShellLow = -4, ShellHigh = 3, ShellTop = 5, InsideLow = -3, InsideHigh = 2;
    private const int TrenchLow = -1, TrenchHigh = 0;
    private static readonly Vector3 RegionMin = new(-3, 1, -3), RegionMax = new(3, 5, 3);
    private static readonly Vector3 Spawn = new(-2.5f, 1.27f, .5f), Goal = new(2.5f, 1.27f, .5f);
    private const uint MaximumVisited = 10_000;
    // How near a waypoint counts as reached; nearer for the last.
    private const float Arrived = .35f, ArrivedLast = .1f;
    // A waypoint whose surface faces the crawler from another surface is round a concave corner.
    private const float SameSurface = .9f, FacingBack = -1e-3f;
    // The drawn body: longer than the 0.25 m ball, lying on the surface.
    private static readonly Vector3 BodyScale = new(.42f, .16f, .9f);
    private const float BodyDrop = .14f, HeadAhead = .5f, HeadDrop = .1f, HeadSize = .24f, MarkerSize = .08f;
    private const ulong BodyId = 1, HeadId = 2, MarkerIdBase = 100;
    private static readonly Color StoneColor = new(.62f, .6f, .56f, 1), BodyColor = new(.25f, .65f, .3f, 1);
    private static readonly Color HeadColor = new(.85f, .75f, .2f, 1), MarkerColor = new(.9f, .3f, .2f, 1);
    private const double FieldOfView = 60, NearClip = .05, FarClip = 100;
    private static readonly Vector3 Eye = new(1.5f, 2.2f, 4.5f), Target = new(-1, 2.8f, -1);

    private enum Stop { Wall, Ceiling, Trench, End }

    private readonly IEngineContext engine;
    private readonly SpatialSession session;
    private readonly Material stone;
    private readonly Appearance body, head, marker;
    private readonly Camera camera;
    private readonly CrawlerConfig config;
    private VoxelScenePresentation? presentation;
    private Vector3 position = Spawn;
    private CrawlerMotion motion;
    private CrawlerStepReceipt last;
    private SurfaceWaypoint[] waypoints = [];
    private int next;
    private Stop? following;
    private bool dropping;
    private ulong sequence;
    private readonly List<CrawlerSurface> crossed = [];
    private readonly List<CrawlerTransition> transitions = [];
    private string route = "no route yet";

    public Product(ProductCreateContext context)
    {
        engine = context.Engine;
        session = engine.Spatial.CreateSession(new(VoxelSize, ChunkSize, VoxelSurfaceMode.GreedyCubes));
        stone = engine.Graphics.CreateMaterial(new MaterialRequest(StoneColor, default, 1, new Color(1, 1, 1, 1), default, 0, false));
        body = engine.Graphics.CreatePrimitive(new(PrimitiveGeometry.Cube, false, BodyColor));
        head = engine.Graphics.CreatePrimitive(new(PrimitiveGeometry.Sphere, false, HeadColor));
        marker = engine.Graphics.CreatePrimitive(new(PrimitiveGeometry.Sphere, false, MarkerColor));
        camera = engine.CameraView.CreateCamera(Descriptor(Eye, Target));
        engine.CameraView.SetActiveCamera(camera);
        config = engine.Spatial.DefaultCrawlerConfig();
    }

    public void Start()
    {
        var room = new List<VoxelEdit>();
        for (int x = ShellLow; x <= ShellHigh; x++)
            for (int y = 0; y <= ShellTop; y++)
                for (int z = ShellLow; z < ShellHigh; z++)
                {
                    bool inside = x >= InsideLow && x <= InsideHigh && y >= 1 && y < ShellTop && z >= InsideLow;
                    bool trench = x >= TrenchLow && x <= TrenchHigh && y < ShellTop;
                    if (!inside && !trench) room.Add(new(VoxelEditKind.Set, new(x, y, z), StoneSlot));
                }
        engine.Voxel.ApplyEdits(new(session, room.ToArray()));
        presentation = engine.VoxelScenePresentation.ProjectScene(new(session, new VoxelSceneMaterialBinding[] { new(StoneSlot, stone) }));
        Step(Vector3.Zero, true, 1f / 60);
        Publish();
    }

    private static CameraDescriptor Descriptor(Vector3 eye, Vector3 target)
    {
        CameraQueries.TryLookAtPose(eye, target, 0, out CameraPose pose);
        return new(pose, CameraBasisMode.Derived, default,
            new(CameraProjectionKind.Perspective, FieldOfView, 0, NearClip, FarClip), CameraViewports.Full);
    }

    private CrawlerStepReceipt Step(Vector3 direction, bool cling, float seconds)
    {
        last = engine.Spatial.ProposeCrawlerStep(new(session, position, motion, config, new(direction, cling, seconds, ++sequence)));
        position = last.Position;
        motion = last.Motion;
        if (last.Transition != CrawlerTransition.None) transitions.Add(last.Transition);
        if (last.Surface != CrawlerSurface.None && (crossed.Count == 0 || crossed[^1] != last.Surface)) crossed.Add(last.Surface);
        return last;
    }

    /// <summary>Toward the next waypoint, leaning into its surface round a concave corner.</summary>
    private Vector3 Steer()
    {
        while (next < waypoints.Length)
        {
            Vector3 toward = waypoints[next].Position - position;
            if (toward.Length() >= (next + 1 == waypoints.Length ? ArrivedLast : Arrived))
            {
                Vector3 direction = Vector3.Normalize(toward), normal = waypoints[next].Normal;
                if (Vector3.Dot(normal, motion.Up) < SameSurface && Vector3.Dot(normal, position - waypoints[next].Position) >= FacingBack)
                    direction -= normal;
                return direction;
            }
            next++;
        }
        return Vector3.Zero;
    }

    private bool Stopped() => following switch
    {
        Stop.Wall => last.Surface == CrawlerSurface.Wall,
        Stop.Ceiling => last.Surface == CrawlerSurface.Ceiling,
        Stop.Trench => last.Surface == CrawlerSurface.Ceiling && position.X >= TrenchLow && position.X < TrenchHigh + 1,
        _ => next >= waypoints.Length,
    };

    public ProductUpdateResult Update(ProductUpdate update)
    {
        ProductUpdateFacts facts = update.Facts;
        float seconds = (float)facts.FixedDeltaSeconds;
        for (uint i = 0; i < facts.AdmittedStepCount; i++)
        {
            if (dropping)
            {
                if (Step(Vector3.Zero, false, seconds).Transition == CrawlerTransition.Landed) dropping = false;
            }
            else if (following is not null)
            {
                Step(Steer(), true, seconds);
                if (Stopped()) following = null;
            }
        }
        Publish();
        return ProductUpdateResult.None;
    }

    private void Publish()
    {
        Quaternion turn = last.Rotation == default ? Quaternion.Identity : last.Rotation;
        Vector3 up = Vector3.Transform(Vector3.UnitY, turn), forward = Vector3.Transform(Vector3.UnitZ, turn);
        var facts = new List<AppearanceFact>
        {
            Fact(BodyId, body, new(position - up * BodyDrop, turn, BodyScale)),
            Fact(HeadId, head, new(position + forward * HeadAhead - up * HeadDrop, turn, new(HeadSize))),
        };
        for (int i = 0; i < waypoints.Length; i++)
            facts.Add(Fact(MarkerIdBase + (ulong)i, marker, new(waypoints[i].Position, Quaternion.Identity, new(MarkerSize))));
        engine.Graphics.PublishSnapshot(facts.ToArray());
    }

    private static AppearanceFact Fact(ulong id, Appearance look, Transform transform) => new(id, false, 0, transform, look, true, RenderLayer.Scene);

    [DebugCommand("crawler.route", Description = "Derives the surface grid and routes the crawler from where it is to the far side of the trench.")]
    public string Route()
    {
        SurfaceNavigationConfig grid = engine.Spatial.DefaultSurfaceNavigationConfig() with { GridId = Grid };
        SurfaceNavigationReplaceReceipt receipt = engine.Spatial.ReplaceSurfaceNavigation(new(session, RegionMin, RegionMax, grid, false));
        SurfaceNavigationPathResult path = engine.Spatial.RequestSurfaceNavigationPath(new(session, Grid, position, Goal, MaximumVisited));
        waypoints = path.Waypoints.ToArray();
        next = 1;
        crossed.Clear();
        transitions.Clear();
        var runs = new List<string>();
        foreach (SurfaceWaypoint waypoint in waypoints)
            if (runs.Count == 0 || runs[^1] != waypoint.Surface.ToString()) runs.Add(waypoint.Surface.ToString());
        route = FormattableString.Invariant(
            $"outcome={path.Outcome} waypoints={waypoints.Length} cost={path.TotalCost} visited={path.Visited} surfaces={string.Join(">", runs)} cells={receipt.SurfaceCellCount} edges={receipt.EdgeCount} components={receipt.ComponentCount} deriveMs={receipt.DerivationMicroseconds / 1000.0:F2}");
        return route;
    }

    [DebugCommand("crawler.follow", Description = "Follows the route until on a wall, the ceiling, over the trench, or the end: wall, ceiling, trench or end.")]
    public string Follow(string until)
    {
        following = until switch
        {
            "wall" => Stop.Wall,
            "ceiling" => Stop.Ceiling,
            "trench" => Stop.Trench,
            "end" => Stop.End,
            _ => throw new ArgumentException($"unknown stop {until}: wall, ceiling, trench or end"),
        };
        return $"following until {until}";
    }

    [DebugCommand("crawler.drop", Description = "Stops and lets go of the surface the crawler holds; it falls straight and holds the first floor it lands on.")]
    public string Drop()
    {
        following = null;
        // Letting go keeps the crawler's velocity; stopped first, it falls straight.
        motion = motion with { Velocity = Vector3.Zero };
        dropping = true;
        return Inspect();
    }

    [DebugCommand("crawler.inspect", Description = "The crawler's position, surface, the surfaces it crossed and its transitions.")]
    public string Inspect() => FormattableString.Invariant(
        $"position=({position.X:F2},{position.Y:F2},{position.Z:F2}) surface={last.Surface} transition={last.Transition} up=({motion.Up.X:F2},{motion.Up.Y:F2},{motion.Up.Z:F2}) following={following?.ToString() ?? "none"} dropping={dropping} waypoint={next}/{waypoints.Length} crossed={string.Join(">", crossed)} transitions={string.Join(",", transitions)} route: {route}");

    [DebugCommand("crawler.camera", Description = "Moves the camera to an eye position looking at a target.")]
    public string MoveCamera(float x, float y, float z, float targetX, float targetY, float targetZ)
    {
        engine.CameraView.UpdateCamera(new(camera, Descriptor(new(x, y, z), new(targetX, targetY, targetZ))));
        return FormattableString.Invariant($"camera eye={x},{y},{z} target={targetX},{targetY},{targetZ}");
    }

    public void RegisterDebugCommands(IDebugCommandModuleRegistrar registrar) => registrar.Register(this);
    public void Pause() { }
    public void Resume() { }
    public void Restart() { }
    public void Shutdown() { }

    public void Dispose()
    {
        engine.Graphics.PublishSnapshot([]);
        presentation?.Dispose();
        foreach (IDisposable owner in new IDisposable[] { body, head, marker, stone, camera, session })
            owner.Dispose();
    }
}
