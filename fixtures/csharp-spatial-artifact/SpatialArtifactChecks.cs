using System;
using System.Numerics;
using Rusty.Engine;

namespace SdkPackageConsumer;

internal static class SpatialArtifactChecks
{
    private const string ValidPath = "spatial-artifact/valid.json";
    private const string InvalidPath = "spatial-artifact/bad-bounds.json";
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
    }

    private static void CheckQueries(IEngineContext engine, SpatialSession session)
    {
        SpatialHit hit = engine.Spatial.CastRay(new(session,
            new Vector3(0.5f, 1, 0.5f), new Vector3(0, -1, 0), 2, new SpatialQueryFilter(0, 0),
            ReadOnlyMemory<SpatialEntityCollider>.Empty, ReadOnlyMemory<ulong>.Empty,
            ReadOnlyMemory<SpatialEntityCollider>.Empty));
        Require(hit.Present && hit.Kind == SpatialHitKind.StaticMesh, "admitted floor is absent from collision");
        NavigationStepReceipt step = engine.Spatial.EvaluateNavigationStep(new(session,
            new Vector3(0.5f, 0, 0.5f), new Vector3(2.5f, 0, 0.5f), 0.5f, 16));
        Require(step.Outcome == NavigationPathOutcome.Reached, "admitted floor is absent from navigation");
    }

    private static void Require(bool condition, string message)
    {
        if (!condition) throw new InvalidOperationException(message);
    }
}
