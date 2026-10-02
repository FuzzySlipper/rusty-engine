using System;
using System.Numerics;
using Rusty.Engine;

namespace SdkPackageConsumer;

internal static class SpatialArtifactChecks
{
    private const string ValidPath = "spatial-artifact/valid.json";
    private const string InvalidPath = "spatial-artifact/bad-bounds.json";
    private const string BinaryPath = "spatial-artifact/valid.rspatial";
    private const ulong NavigationGridId = 7;
    private const uint ChunkSize = 8;
    private const uint MaximumStepCells = 1;

    internal static void Run(IEngineContext engine)
    {
        using SpatialSession session = engine.Spatial.CreateSession(new(1, ChunkSize, VoxelSurfaceMode.GreedyCubes));
        engine.Voxel.ApplyEdits(new(session,
            new[] { new VoxelEdit(VoxelEditKind.Set, new VoxelAddress(8, 0, 0), 1) }));
        VoxelSceneReadout resident = engine.Voxel.ReadScene(new(session));
        Require(resident.ResidentChunkCount > 0, "fixture must start with resident voxel state");

        using ContentReference valid = engine.Content.OpenReference(new(ValidPath));
        var request = new SpatialContentArtifactReplaceRequest(session, valid, NavigationGridId, ChunkSize, MaximumStepCells);
        SpatialContentArtifactReplaceReceipt admitted = engine.Spatial.ReplaceContentArtifact(request);
        Require(admitted.CollisionTriangleCount == 2 && admitted.NavigationCellCount == 3,
            "artifact did not install collision and navigation together");
        ContentReferenceInfo source = engine.Content.ReadReferenceInfo(valid).Span[0];
        Require(admitted.ContentSha256 == source.Sha256, "admission lost immutable content identity");
        Require(engine.Voxel.ReadScene(new(session)) == resident, "artifact admission changed voxel residency");
        CheckQueries(engine, session);

        // The binary encoding of the same facts admits the same collision and
        // navigation. Its collision projection hash differs: the collider asset
        // is named by the artifact's content digest.
        using (ContentReference binary = engine.Content.OpenReference(new(BinaryPath)))
        {
            SpatialContentArtifactReplaceReceipt packed = engine.Spatial.ReplaceContentArtifact(request with { Content = binary });
            Require(packed.CollisionVertexCount == admitted.CollisionVertexCount
                && packed.CollisionTriangleCount == admitted.CollisionTriangleCount
                && packed.NavigationCellCount == admitted.NavigationCellCount
                && packed.NavigationProjectionHash == admitted.NavigationProjectionHash,
                $"binary artifact admitted different facts than its JSON form: {packed} against {admitted}");
            Require(packed.ContentSha256 == engine.Content.ReadReferenceInfo(binary).Span[0].Sha256,
                "binary admission lost its content identity");
            CheckQueries(engine, session);
        }
        admitted = engine.Spatial.ReplaceContentArtifact(request);

        SpatialProjectionReadout collision = engine.Spatial.ReadProjection(new(session));
        NavigationProjectionReadout navigation = engine.Spatial.ReadNavigationProjection(new(session));
        SpatialContentArtifactReadout identity = engine.Spatial.ReadContentArtifact(new(session));
        using ContentReference invalid = engine.Content.OpenReference(new(InvalidPath));
        try
        {
            engine.Spatial.ReplaceContentArtifact(request with { Content = invalid });
            throw new InvalidOperationException("out-of-bounds artifact was accepted");
        }
        catch (EngineCallException error)
        {
            Require(error.Service == "Spatial" && error.Operation == "ReplaceContentArtifact"
                && error.Diagnostics.Length == 1
                && error.Diagnostics.Span[0].Code == "CSHARP_SPATIAL_CONTENT_BOUNDS"
                && !string.IsNullOrWhiteSpace(error.Diagnostics.Span[0].Message),
                "generated exception lost the named artifact refusal");
        }
        Require(engine.Spatial.ReadProjection(new(session)) == collision, "refusal changed collision or residency");
        Require(engine.Spatial.ReadNavigationProjection(new(session)) == navigation, "refusal changed navigation");
        Require(engine.Spatial.ReadContentArtifact(new(session)) == identity, "refusal changed artifact identity");
        Require(engine.Voxel.ReadScene(new(session)) == resident, "refusal changed resident voxel state");
        CheckQueries(engine, session);
        SpatialContentArtifactReplaceReceipt retry = engine.Spatial.ReplaceContentArtifact(request);
        Require(retry.NavigationRevision > admitted.NavigationRevision, "session did not accept a later valid artifact");
        CheckPlacedArtifacts(engine, session, valid);
    }

    // The same 3-cell floor placed twice beside the base one: one navigation
    // across all three, and each placement leaves under its own identity.
    private static void CheckPlacedArtifacts(IEngineContext engine, SpatialSession session, ContentReference floor)
    {
        SpatialContentArtifactResidencyReceipt placed = engine.Spatial.ApplyContentArtifactResidency(new(session,
            new SpatialContentArtifactInstance[] { new(11, floor, 3, 0, 0), new(12, floor, 6, 0, 0) },
            ReadOnlyMemory<ulong>.Empty, NavigationGridId, ChunkSize, MaximumStepCells));
        Require(placed.InstanceCount == 2 && placed.NavigationCellCount == 9, "placed artifacts did not compose");
        NavigationStepResult across = engine.Spatial.EvaluateNavigationStep(new(session,
            new Vector3(0.5f, 0, 0.5f), new Vector3(8.5f, 0, 0.5f), 0.5f, 64));
        Require(across.Outcome == NavigationPathOutcome.Reached, "navigation did not cross placed artifacts");
        SpatialContentArtifactResidencyReceipt removed = engine.Spatial.ApplyContentArtifactResidency(new(session,
            ReadOnlyMemory<SpatialContentArtifactInstance>.Empty, new ulong[] { 11 },
            NavigationGridId, ChunkSize, MaximumStepCells));
        Require(removed.InstanceCount == 1 && removed.NavigationCellCount == 6, "a placement did not leave alone");
        CheckQueries(engine, session);
    }

    private static void CheckQueries(IEngineContext engine, SpatialSession session)
    {
        SpatialHit hit = engine.Spatial.CastRay(new(session,
            new Vector3(0.5f, 1, 0.5f), new Vector3(0, -1, 0), 2, new SpatialQueryFilter(0, 0),
            ReadOnlyMemory<SpatialEntityCollider>.Empty, ReadOnlyMemory<ulong>.Empty,
            ReadOnlyMemory<SpatialEntityCollider>.Empty));
        Require(hit.Present && hit.Kind == SpatialHitKind.StaticMesh, "admitted floor is absent from collision");
        NavigationStepResult step = engine.Spatial.EvaluateNavigationStep(new(session,
            new Vector3(0.5f, 0, 0.5f), new Vector3(2.5f, 0, 0.5f), 0.5f, 16));
        Require(step.Outcome == NavigationPathOutcome.Reached, "admitted floor is absent from navigation");
    }

    private static void Require(bool condition, string message)
    {
        if (!condition) throw new InvalidOperationException(message);
    }
}
