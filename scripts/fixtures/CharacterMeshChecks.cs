using System;
using System.Numerics;
using Rusty.Engine;

internal static class CharacterMeshChecks
{
    private const ulong MeshAssetId = 7_401;
    private const ulong MovingMeshInstanceId = 7_402;
    private const ulong MovingMeshEntityId = 7_403;
    private const ulong StaticMeshInstanceId = 7_404;
    private const float StepSeconds = 1.0f / 60.0f;
    private const float OffStepSeconds = 1.0f / 15.0f;
    private const float RotationRadians = 15.0f * (MathF.PI / 180.0f);

    // Two joined rectangular bars form one closed L prism. The enclosing
    // bounds are [-2, 2] on X/Z, while the upper-right quadrant is empty.
    private static readonly Vector3[] MeshVertices =
    [
        new(-2, 0.5f, -2), new(2, 0.5f, -2), new(2, 0.5f, -1),
        new(-1, 0.5f, -1), new(-1, 0.5f, 2), new(-2, 0.5f, 2),
        new(-2, 0, -2), new(2, 0, -2), new(2, 0, -1),
        new(-1, 0, -1), new(-1, 0, 2), new(-2, 0, 2),
    ];

    private static readonly Triangle[] MeshTriangles =
    [
        // Top surface, with +Y winding.
        new(0, 2, 1), new(0, 3, 2), new(0, 4, 3), new(0, 5, 4),
        // Bottom surface.
        new(6, 7, 8), new(6, 8, 9), new(6, 9, 10), new(6, 10, 11),
        // Boundary walls.
        new(0, 1, 7), new(0, 7, 6),
        new(1, 2, 8), new(1, 8, 7),
        new(2, 3, 9), new(2, 9, 8),
        new(3, 4, 10), new(3, 10, 9),
        new(4, 5, 11), new(4, 11, 10),
        new(5, 0, 6), new(5, 6, 11),
    ];

    public static void Run(IEngineContext engine)
    {
        CharacterControllerConfig defaults = engine.Spatial.DefaultCharacterControllerConfig();
        CharacterControllerConfig config = defaults with
        {
            Ground = defaults.Ground with
            {
                StrafeSpeed = 20.0f,
            },
        };

        ExerciseMovingMeshCarry(engine, config);
        ExerciseEnclosingBoundsHole(engine, config);
        ExerciseStaticMeshCollisionOnly(engine, config);
    }

    private static void ExerciseMovingMeshCarry(
        IEngineContext engine,
        CharacterControllerConfig config)
    {
        using SpatialSession session = engine.Spatial.CreateSession(
            new SpatialSessionConfig(1, 16, VoxelSurfaceMode.GreedyCubes));
        Transform initialMeshTransform = IdentityTransform(Vector3.Zero);
        AdmitMesh(engine, session, new StaticMeshInstance(
            MovingMeshInstanceId, MeshAssetId, initialMeshTransform));

        // The durable product record contains only ordinary value data. It
        // can be saved and restored without retaining a native Engine handle.
        SavedMeshPose saved = new(
            MovingMeshEntityId,
            MovingMeshInstanceId,
            initialMeshTransform);
        Require(saved.Entity == MovingMeshEntityId
            && saved.Instance == MovingMeshInstanceId
            && saved.Pose == initialMeshTransform,
            "moving mesh selection was not represented by saveable product values");

        // Rebuild the retained collision instance from those product-owned
        // values in a fresh session. This is the save/load proof: the Engine
        // receives a new retained admission plus the same stable IDs, with no
        // native instance handle crossing the persistence boundary.
        using (SpatialSession restoredSession = engine.Spatial.CreateSession(
                   new SpatialSessionConfig(1, 16, VoxelSurfaceMode.GreedyCubes)))
        {
            AdmitMesh(engine, restoredSession, new StaticMeshInstance(
                saved.Instance, MeshAssetId, saved.Pose));
            CharacterStepReceipt restored = Propose(
                engine,
                restoredSession,
                new Vector3(-1.25f, 1.4f, -1.5f),
                InitialMotion(1.4f),
                NoSupport(),
                new[] { new CharacterMeshInstance(
                    saved.Instance,
                    saved.Entity,
                    Vector3.Zero,
                    Vector3.Zero) },
                config,
                Idle(1));
            Require(restored.Ground.Present
                && restored.Motion.SupportEntityPresent
                && restored.Motion.SupportEntity == saved.Entity,
                "saved mesh instance/entity values did not rebuild the retained support");
        }

        CharacterMotion motion = InitialMotion(1.4f);
        CharacterSupport noSupport = NoSupport();
        CharacterMeshInstance[] meshInstances =
        [
            new CharacterMeshInstance(
                MovingMeshInstanceId,
                MovingMeshEntityId,
                Vector3.Zero,
                Vector3.Zero),
        ];

        CharacterStepReceipt landed = Propose(
            engine,
            session,
            new Vector3(-1.25f, 1.4f, -1.5f),
            motion,
            noSupport,
            meshInstances,
            config,
            Idle(1));
        Require(landed.Ground.Present
            && landed.Motion.Grounded
            && landed.Motion.SupportEntityPresent
            && landed.Motion.SupportEntity == MovingMeshEntityId,
            "character did not establish support on the retained collision mesh entity");
        Require(landed.Ground.SourceKind == CharacterCollisionSourceKind.ActiveEntity
            && landed.Ground.SourceEntity == MovingMeshEntityId
            && landed.Ground.SourceInstance == MovingMeshInstanceId,
            "mesh support did not preserve the entity and retained instance identity");

        Transform translatedMeshTransform = IdentityTransform(new Vector3(0.2f, 0, 0));
        UpdateMesh(engine, session, new StaticMeshInstance(
            MovingMeshInstanceId, MeshAssetId, translatedMeshTransform));
        CharacterStepReceipt translated = Propose(
            engine,
            session,
            landed.Transform.Translation,
            landed.Motion,
            NoSupport(),
            new[] { new CharacterMeshInstance(
                MovingMeshInstanceId,
                MovingMeshEntityId,
                new Vector3(12.0f, 0, 0),
                Vector3.Zero) },
            config,
            Idle(2));
        Require(translated.Platform.Present
            && !translated.Platform.Departed
            && translated.Platform.Entity == MovingMeshEntityId
            && translated.Motion.SupportEntity == MovingMeshEntityId,
            "translated collision mesh did not carry its supported character");
        Require(translated.Platform.CarriedDisplacement.X > 0.15f
            && translated.Platform.CarriedDisplacement.X < 0.25f
            && translated.Platform.PointVelocity.X > 10.0f,
            "translated mesh carry did not use the admitted linear velocity");

        Transform rotatedMeshTransform = new(
            new Vector3(0.4f, 0, 0),
            Quaternion.CreateFromAxisAngle(Vector3.UnitY, RotationRadians),
            Vector3.One);
        UpdateMesh(engine, session, new StaticMeshInstance(
            MovingMeshInstanceId, MeshAssetId, rotatedMeshTransform));
        CharacterStepReceipt rotated = Propose(
            engine,
            session,
            translated.Transform.Translation,
            translated.Motion,
            NoSupport(),
            new[] { new CharacterMeshInstance(
                MovingMeshInstanceId,
                MovingMeshEntityId,
                new Vector3(12.0f, 0, 0),
                new Vector3(0, RotationRadians / StepSeconds, 0)) },
            config,
            Idle(3));
        Require(rotated.Platform.Present
            && !rotated.Platform.Departed
            && rotated.Platform.Entity == MovingMeshEntityId
            && rotated.Motion.SupportEntity == MovingMeshEntityId,
            "rotated collision mesh did not keep its supported character");
        Vector3 expectedCarry = (rotatedMeshTransform.Translation
                - translated.Motion.SupportPreviousTranslation)
            + (Vector3.Transform(
                    translated.Motion.SupportLocalAnchor,
                    rotatedMeshTransform.Rotation)
                - Vector3.Transform(
                    translated.Motion.SupportLocalAnchor,
                    translated.Motion.SupportPreviousRotation));
        Require(Vector3.Distance(rotated.Platform.CarriedDisplacement, expectedCarry) < 0.01f
            && rotated.Platform.CarriedDisplacement.Length() > 0.05f
            && rotated.Platform.PointVelocity.Length() > 1.0f,
            "rotated mesh carry did not match the admitted pose rotation and angular velocity");

        CharacterStepReceipt off = rotated;
        for (int index = 0; index < 8 && !off.Platform.Departed; index++)
        {
            off = Propose(
                engine,
                session,
                off.Transform.Translation,
                off.Motion,
                NoSupport(),
                new[] { new CharacterMeshInstance(
                    MovingMeshInstanceId,
                    MovingMeshEntityId,
                    Vector3.Zero,
                    Vector3.Zero) },
                config,
                new CharacterControllerCommand(
                    new Vector2(1, 0),
                    0,
                    false,
                    false,
                    false,
                    Vector3.Zero,
                    Vector3.Zero,
                    OffStepSeconds,
                    (ulong)(4 + index)));
        }
        Require(off.Platform.Present
            && off.Platform.Departed
            && off.Platform.Entity == MovingMeshEntityId
            && !off.Motion.SupportEntityPresent
            && !off.Ground.Present,
            "character did not leave the retained mesh support through its open edge");
    }

    private static void ExerciseEnclosingBoundsHole(
        IEngineContext engine,
        CharacterControllerConfig config)
    {
        using SpatialSession session = engine.Spatial.CreateSession(
            new SpatialSessionConfig(1, 16, VoxelSurfaceMode.GreedyCubes));
        AdmitMesh(engine, session, new StaticMeshInstance(
            MovingMeshInstanceId, MeshAssetId, IdentityTransform(Vector3.Zero)));

        const float holeX = 1.25f;
        const float holeZ = 1.25f;
        CharacterStepReceipt hole = Propose(
            engine,
            session,
            new Vector3(holeX, 1.4f, holeZ),
            InitialMotion(1.4f),
            NoSupport(),
            new[] { new CharacterMeshInstance(
                MovingMeshInstanceId,
                MovingMeshEntityId,
                Vector3.Zero,
                Vector3.Zero) },
            config,
            new CharacterControllerCommand(
                new Vector2(1, 0),
                0,
                false,
                false,
                false,
                Vector3.Zero,
                Vector3.Zero,
                OffStepSeconds,
                1));
        Require(!hole.Ground.Present
            && !hole.Contact.Present
            && !hole.Motion.Grounded
            && hole.Transform.Translation.X > holeX,
            "empty space inside the retained mesh enclosing AABB was not passable");
    }

    private static void ExerciseStaticMeshCollisionOnly(
        IEngineContext engine,
        CharacterControllerConfig config)
    {
        using SpatialSession session = engine.Spatial.CreateSession(
            new SpatialSessionConfig(1, 16, VoxelSurfaceMode.GreedyCubes));
        Transform staticTransform = IdentityTransform(new Vector3(0, 0, 4));
        AdmitMesh(engine, session, new StaticMeshInstance(
            StaticMeshInstanceId, MeshAssetId, staticTransform));

        CharacterStepReceipt staticHit = Propose(
            engine,
            session,
            new Vector3(-1.5f, 1.4f, 5.0f),
            InitialMotion(1.4f),
            NoSupport(),
            ReadOnlyMemory<CharacterMeshInstance>.Empty,
            config,
            Idle(1));
        Require(staticHit.Ground.Present
            && staticHit.Ground.SourceKind == CharacterCollisionSourceKind.StaticMesh
            && staticHit.Ground.SourceEntity == 0
            && staticHit.Ground.SourceInstance == StaticMeshInstanceId
            && !staticHit.Motion.SupportEntityPresent,
            "unadmitted static mesh instance did not remain collision-only");
    }

    private static CharacterStepReceipt Propose(
        IEngineContext engine,
        SpatialSession session,
        Vector3 position,
        CharacterMotion motion,
        CharacterSupport support,
        ReadOnlyMemory<CharacterMeshInstance> meshInstances,
        CharacterControllerConfig config,
        CharacterControllerCommand command)
    {
        return engine.Spatial.ProposeCharacterStep(new CharacterStepRequest(
            session,
            position,
            motion,
            support,
            ReadOnlyMemory<CharacterObstacle>.Empty,
            meshInstances,
            config,
            command));
    }

    private static void AdmitMesh(
        IEngineContext engine,
        SpatialSession session,
        StaticMeshInstance instance)
    {
        engine.Spatial.ApplyCollisionResidency(new CollisionResidencyRequest(
            session,
            new[] { new StaticMeshAsset(
                MeshAssetId,
                default,
                0,
                (uint)MeshVertices.Length,
                0,
                (uint)MeshTriangles.Length) },
            MeshVertices,
            MeshTriangles,
            new[] { instance },
            ReadOnlyMemory<ulong>.Empty,
            ReadOnlyMemory<ulong>.Empty));
    }

    private static void UpdateMesh(
        IEngineContext engine,
        SpatialSession session,
        StaticMeshInstance instance)
    {
        engine.Spatial.ApplyCollisionResidency(new CollisionResidencyRequest(
            session,
            ReadOnlyMemory<StaticMeshAsset>.Empty,
            ReadOnlyMemory<Vector3>.Empty,
            ReadOnlyMemory<Triangle>.Empty,
            new[] { instance },
            ReadOnlyMemory<ulong>.Empty,
            ReadOnlyMemory<ulong>.Empty));
    }

    private static CharacterControllerCommand Idle(ulong sequence) => new(
        Vector2.Zero,
        0,
        false,
        false,
        false,
        Vector3.Zero,
        Vector3.Zero,
        StepSeconds,
        sequence);

    private static CharacterMotion InitialMotion(float y) => new(
        Vector3.Zero,
        Vector3.Zero,
        false,
        CharacterStance.Standing,
        0,
        0,
        0,
        false,
        0,
        Vector3.Zero,
        Vector3.Zero,
        Quaternion.Identity,
        Vector3.Zero,
        y,
        y,
        0,
        0);

    private static CharacterSupport NoSupport() => new(
        false,
        CharacterSupportLifecycle.Active,
        0,
        IdentityTransform(Vector3.Zero));

    private static Transform IdentityTransform(Vector3 translation) => new(
        translation,
        Quaternion.Identity,
        Vector3.One);

    private static void Require(bool condition, string message)
    {
        if (!condition)
        {
            throw new InvalidOperationException(message);
        }
    }

    private readonly record struct SavedMeshPose(
        ulong Entity,
        ulong Instance,
        Transform Pose);
}
