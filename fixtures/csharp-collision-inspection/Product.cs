using System.Numerics;
using Rusty.Engine;

namespace CsharpCollisionInspection;

/// <summary>
/// One collider of each kind in one Spatial session, for collision inspection
/// (<c>engine.inspect.overlay show collision</c>, <c>engine.inspect.collision</c>):
/// a voxel floor, a content artifact platform drawn a metre above its
/// collision, an undrawn product static-mesh ramp, a product obstacle box
/// beside a stepped character, a trigger zone, and a dynamics box resting on
/// the floor.
/// </summary>
public sealed class Product : IEngineProduct
{
    private const string PlatformPath = "platform.json";
    private const ulong NavigationGridId = 1;
    private const uint ChunkSize = 8, MaximumStepCells = 1;
    private const ulong FloorId = 1, PlatformId = 2, ObstacleId = 3, BoxId = 4, TriggerId = 5, CharacterId = 6;
    private const ulong RampAssetId = 50, RampInstanceId = 51;
    private const int FloorMin = -6, FloorMax = 5;
    /// <summary>The platform's collision is at y = 1; it is drawn this much higher.</summary>
    private const float PlatformDrawnRise = 1;
    private static readonly Vector3 PlatformDrawnCenter = new(1, 1.25f + PlatformDrawnRise, 1), PlatformDrawnSize = new(2, .5f, 2);
    private static readonly Vector3 ObstacleAt = new(2.5f, .5f, -1), ObstacleHalf = new(.5f, .5f, .5f);
    private static readonly Vector3 BoxStart = new(-3, 3, -3), BoxHalf = new(.4f, .4f, .4f);
    private static readonly Vector3 TriggerMin = new(-4, 0, 3), TriggerMax = new(-2, 2, 5);
    private static readonly Vector3 CharacterStart = new(0, 1.2f, -1);
    private static readonly Vector3 CameraEye = new(9, 7, 11), CameraTarget = new(0, .5f, 0);
    private static readonly Color FloorColor = new(.45f, .45f, .4f, 1), PlatformColor = new(.7f, .5f, .3f, 1);
    private static readonly Color ObstacleColor = new(.3f, .35f, .8f, 1), BoxColor = new(.8f, .2f, .2f, 1), CharacterColor = new(.9f, .9f, .9f, 1);

    private readonly IEngineContext engine;
    private readonly SpatialSession session;
    private readonly DynamicsWorld world;
    private readonly DynamicsBody box;
    private readonly Appearance floorLook, platformLook, obstacleLook, boxLook, characterLook;
    private readonly Camera camera;
    private readonly CharacterControllerConfig config;
    private Vector3 position = CharacterStart;
    private CharacterMotion motion = default(CharacterMotion) with
    {
        Stance = CharacterStance.Standing,
        FallOriginY = CharacterStart.Y,
        PeakY = CharacterStart.Y,
    };
    private ulong steps;

    public Product(ProductCreateContext context)
    {
        engine = context.Engine;
        session = engine.Spatial.CreateSession(new(1, ChunkSize, VoxelSurfaceMode.GreedyCubes));
        config = engine.Spatial.DefaultCharacterControllerConfig();
        world = engine.Dynamics.CreateWorld(new DynamicsWorldConfig(new Vector3(0, -9.81f, 0)));
        box = engine.Dynamics.CreateCuboidBody(new(world, new(new(BoxStart, Quaternion.Identity, Vector3.One), BoxHalf,
            new(1, default, Vector3.Zero, Vector3.Zero, default, 0, 0, 1, .5f, 0, uint.MaxValue, uint.MaxValue, true, false, false))));
        floorLook = engine.Graphics.CreatePrimitive(new(PrimitiveGeometry.Cube, false, FloorColor));
        platformLook = engine.Graphics.CreatePrimitive(new(PrimitiveGeometry.Cube, false, PlatformColor));
        obstacleLook = engine.Graphics.CreatePrimitive(new(PrimitiveGeometry.Cube, false, ObstacleColor));
        boxLook = engine.Graphics.CreatePrimitive(new(PrimitiveGeometry.Cube, false, BoxColor));
        characterLook = engine.Graphics.CreatePrimitive(new(PrimitiveGeometry.Sphere, false, CharacterColor));
        CameraQueries.TryLookAtPose(CameraEye, CameraTarget, 0, out CameraPose pose);
        camera = engine.CameraView.CreateCamera(new(pose, CameraBasisMode.Derived, default,
            new(CameraProjectionKind.Perspective, 55, 0, .1f, 200), CameraViewports.Full));
        engine.CameraView.SetActiveCamera(camera);
    }

    public void Start()
    {
        List<VoxelEdit> floor = [];
        for (int x = FloorMin; x <= FloorMax; x++)
            for (int z = FloorMin; z <= FloorMax; z++)
                floor.Add(new(VoxelEditKind.Set, new(x, -1, z), 1));
        engine.Voxel.ApplyEdits(new(session, floor.ToArray()));

        using (ContentReference platform = engine.Content.OpenReference(new(PlatformPath)))
            engine.Spatial.ReplaceContentArtifact(new(session, platform, NavigationGridId, ChunkSize, MaximumStepCells));

        // A ramp the product admits and never draws: a walkable 30-degree
        // slope up to a steep 70-degree one.
        Vector3[] ramp =
        [
            new(-1, 0, -4), new(-1, 0, -2), new(1, 1.15f, -4), new(1, 1.15f, -2),
            new(1.5f, 2.5f, -4), new(1.5f, 2.5f, -2),
        ];
        Triangle[] faces = [new(0, 1, 2), new(2, 1, 3), new(2, 3, 4), new(4, 3, 5)];
        engine.Spatial.ApplyCollisionResidency(new(session,
            new[] { new StaticMeshAsset(RampAssetId, default, 0, (uint)ramp.Length, 0, (uint)faces.Length) },
            ramp, faces,
            new[] { new StaticMeshInstance(RampInstanceId, RampAssetId, new(Vector3.Zero, Quaternion.Identity, Vector3.One)) },
            ReadOnlyMemory<ulong>.Empty, ReadOnlyMemory<ulong>.Empty));

        engine.Dynamics.BindWorldCollision(new(world, session));
        engine.Spatial.RegisterTrigger(new(session, TriggerId, "fixture", "zone", SpatialTriggerGeometry.EntityBounds));
    }

    public ProductUpdateResult Update(ProductUpdate update)
    {
        ProductUpdateFacts facts = update.Facts;
        for (uint step = 0; step < facts.AdmittedStepCount; step++)
        {
            steps++;
            CharacterStepReceipt receipt = engine.Spatial.ProposeCharacterStep(new(default, session, position, motion, default,
                new[] { new CharacterObstacle(ObstacleId, new(ObstacleAt, Quaternion.Identity, Vector3.One), -ObstacleHalf, ObstacleHalf, true, Vector3.Zero, Vector3.Zero) },
                ReadOnlyMemory<CharacterMeshInstance>.Empty, config,
                new(default, Vector2.Zero, 0, false, false, false, Vector3.Zero, Vector3.Zero, (float)facts.FixedDeltaSeconds, steps)));
            position = receipt.Transform.Translation;
            motion = receipt.Motion;
            engine.Spatial.ReconcileTriggers(new(session, steps, SpatialTriggerCause.Scheduled,
                new[] { new SpatialEntityCollider(TriggerId, TriggerMin, TriggerMax, uint.MaxValue, uint.MaxValue, true, true, true) }));
        }
        if (facts.AdmittedStepCount > 0)
            engine.Dynamics.Step(new(world, (float)facts.FixedDeltaSeconds, facts.AdmittedStepCount, ReadOnlyMemory<DynamicsAction>.Empty));
        Publish();
        return ProductUpdateResult.None;
    }

    private void Publish()
    {
        float floorWidth = FloorMax - FloorMin + 1, floorCenter = (FloorMax + FloorMin + 1) * .5f;
        Transform boxPose = engine.Dynamics.Read(new(box)).Transform;
        engine.Graphics.PublishSnapshot(new[]
        {
            Fact(FloorId, floorLook, new(new(floorCenter, -.5f, floorCenter), Quaternion.Identity, new(floorWidth, 1, floorWidth))),
            Fact(PlatformId, platformLook, new(PlatformDrawnCenter, Quaternion.Identity, PlatformDrawnSize)),
            Fact(ObstacleId, obstacleLook, new(ObstacleAt, Quaternion.Identity, ObstacleHalf * 2)),
            Fact(BoxId, boxLook, boxPose with { Scale = BoxHalf * 2 }),
            Fact(CharacterId, characterLook, new(position, Quaternion.Identity, new(.6f, 1.8f, .6f))),
        });
    }

    private static AppearanceFact Fact(ulong id, Appearance look, Transform transform) =>
        new(id, false, 0, transform, look, true, RenderLayer.Scene, ShadowCasting.Cast);

    public void Pause() { }
    public void Resume() { }
    public void Restart() { }
    public void Shutdown() { }

    public void Dispose()
    {
        camera.Dispose();
        foreach (Appearance look in new[] { floorLook, platformLook, obstacleLook, boxLook, characterLook })
            look.Dispose();
        world.Dispose();
        session.Dispose();
    }
}
