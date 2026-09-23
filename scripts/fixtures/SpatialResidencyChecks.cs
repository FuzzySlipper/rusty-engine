using System;
using System.Numerics;
using Rusty.Engine;

internal static class SpatialResidencyChecks
{
    public static void Run(IEngineContext engine)
    {
        using SpatialSession session = engine.Spatial.CreateSession(new(1, 16, VoxelSurfaceMode.GreedyCubes));
        Vector3[] vertices = [new(0, 0, 0), new(1, 0, 0), new(1, 0, 1), new(0, 0, 1)];
        Triangle[] triangles = [new(0, 2, 1), new(0, 3, 2)];
        void Admit(ulong id, float x) => engine.Spatial.ApplyCollisionResidency(new(
            session, new StaticMeshAsset[] { new(id, default, 0, 4, 0, 2) }, vertices, triangles,
            new StaticMeshInstance[] { new(id, id, new(new(x, 0, 0), Quaternion.Identity, Vector3.One)) },
            ReadOnlyMemory<ulong>.Empty, ReadOnlyMemory<ulong>.Empty));
        void Remove(ulong id) => engine.Spatial.ApplyCollisionResidency(new(
            session, ReadOnlyMemory<StaticMeshAsset>.Empty, ReadOnlyMemory<Vector3>.Empty,
            ReadOnlyMemory<Triangle>.Empty, ReadOnlyMemory<StaticMeshInstance>.Empty,
            new ulong[] { id }, new ulong[] { id }));
        SpatialHit Hit(float x) => engine.Spatial.CastRay(new(
            session, new(x, 2, 0.5f), -Vector3.UnitY, 4, default,
            ReadOnlyMemory<SpatialEntityCollider>.Empty, ReadOnlyMemory<ulong>.Empty,
            ReadOnlyMemory<SpatialEntityCollider>.Empty));
        void Require(bool condition, string message)
        {
            if (!condition) throw new InvalidOperationException(message);
        }
        Admit(1, 0); Admit(2, 1);
        Require(Hit(0.5f).Instance == 1 && Hit(1.5f).Instance == 2, "Adjacent authored cells did not remain independently resident.");
        Remove(1);
        Require(!Hit(0.5f).Present && Hit(1.5f).Instance == 2, "Cell removal changed an unrelated resident cell.");
        Admit(1, 1000);
        Require(Hit(1000.5f).Instance == 1 && !Hit(0.5f).Present, "Teleport/reload retained stale cell geometry.");
        WorldOriginReadout origin = engine.WorldOrigin.Read(new(session));
        using WorldOriginPrepared rebase = engine.WorldOrigin.Prepare(new(
            session, origin.Revision, origin.VoxelSourceRevision, origin.StaticMeshRevision,
            1000, 0, 0, ReadOnlyMemory<WorldOriginEntityRow>.Empty));
        engine.WorldOrigin.Commit(new(rebase));
        Require(Hit(0.5f).Instance == 1 && Hit(-998.5f).Instance == 2, "Origin shift lost resident static cells.");
        Remove(1); Admit(1, 0);
        Require(Hit(0.5f).Instance == 1, "Cell reload after origin shift failed.");
        Remove(1); Remove(2);
        Require(!Hit(0.5f).Present, "Final cell teardown retained collision.");
    }
}
