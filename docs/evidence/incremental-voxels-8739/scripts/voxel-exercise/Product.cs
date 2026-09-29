using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Numerics;
using Rusty.Engine;

namespace VoxelExercise;

// #8739 evidence product. It streams a 16 x 16 chunk floor, then every update:
// - flips one far-away voxel with ApplyEdits;
// - steps a character standing on the far corner of the floor.
// Update 300 unloads and reloads a chunk, update 400 attempts an invalid edit,
// and update 450 places a noncollidable voxel. Update 600 prints the facts and
// VOXEL_EXERCISE_PASSED or the failures. Built with OLD_VOXEL_API it runs the
// same scene against the pre-#8739 SDK, which needs revisions and hashes.
public sealed class Product : IEngineProduct
{
    private const float Step = 1.0f / 60.0f;
    private const int Chunk = 8;
    private const int ChunksPerSide = 16;
    private const uint Floor = 1;
    private const uint Ghost = 9;
    private const long ResidencyUpdate = 300;
    private const long FailedEditUpdate = 400;
    private const long GhostUpdate = 450;
    private const long EndUpdate = 600;
    private static readonly VoxelAddress Flipped = new(3, 0, 3);
    private static readonly VoxelAddress GhostCell = new(40, 1, 40);
    private static readonly VoxelChunkIdentity Reloaded = new(1, 0, 1);
    private static readonly Vector3 CharacterStart = new(124.5f, 2.2f, 124.5f);

    private readonly IEngineContext _engine;
    private readonly SpatialSession _session;
    private readonly CharacterControllerConfig _config;
    private readonly List<double> _editUs = new();
    private readonly List<double> _residencyUs = new();
    private readonly List<string> _failures = new();
    private Vector3 _character = CharacterStart;
    private CharacterMotion _motion;
    private long _updates;
    private int _groundedUpdates;
    private float _lowestCharacterY = float.MaxValue;
    private bool _failedEditRejected;
    private bool _reported;

    public Product(ProductCreateContext context)
    {
        _engine = context.Engine;
        _session = _engine.Spatial.CreateSession(new SpatialSessionConfig(1, Chunk, VoxelSurfaceMode.GreedyCubes));
        _engine.Voxel.ConfigureMaterialCollision(new VoxelMaterialCollisionRequest(
            _session, new[] { new VoxelMaterialCollision(Ghost, false) }));
        var chunks = new List<VoxelChunkIdentity>();
        for (int x = 0; x < ChunksPerSide; x++)
            for (int z = 0; z < ChunksPerSide; z++)
                chunks.Add(new VoxelChunkIdentity(x, 0, z));
        // The old SDK admits at most 64 chunks per transaction.
        for (int start = 0; start < chunks.Count; start += 64)
        {
            Admit(chunks.GetRange(start, Math.Min(64, chunks.Count - start)));
        }
        _config = _engine.Spatial.DefaultCharacterControllerConfig();
        _motion = new CharacterMotion(Vector3.Zero, Vector3.Zero, false, CharacterStance.Standing, 0, 0, 0, false, 0,
            Vector3.Zero, Vector3.Zero, Quaternion.Identity, Vector3.Zero, CharacterStart.Y, CharacterStart.Y, 0, 0);
    }

    public void Start() { }
    public void Attach() { }

    public ProductUpdateResult Update(ProductUpdate update)
    {
        _updates++;
        if (_updates > EndUpdate)
        {
            return ProductUpdateResult.None;
        }
        long started = Stopwatch.GetTimestamp();
        Edit(_updates % 2 == 1 ? VoxelEditKind.Clear : VoxelEditKind.Set, Flipped, Floor);
        _editUs.Add(Stopwatch.GetElapsedTime(started).TotalMicroseconds);

        if (_updates == ResidencyUpdate)
        {
            started = Stopwatch.GetTimestamp();
            Evict(Reloaded);
            Admit(new List<VoxelChunkIdentity> { Reloaded });
            _residencyUs.Add(Stopwatch.GetElapsedTime(started).TotalMicroseconds);
        }
        if (_updates == FailedEditUpdate)
        {
            VoxelSceneReadout before = _engine.Voxel.ReadScene(new VoxelSceneReadRequest(_session));
            try
            {
                Edit(VoxelEditKind.Set, new VoxelAddress(5, 1, 5), 70_000);
            }
            catch (EngineCallException)
            {
                _failedEditRejected = true;
            }
            VoxelSceneReadout after = _engine.Voxel.ReadScene(new VoxelSceneReadRequest(_session));
            Require(after.SourceRevision == before.SourceRevision && after.AuthorityHash == before.AuthorityHash,
                "a failed edit changed the scene");
        }
        if (_updates == GhostUpdate)
        {
            Edit(VoxelEditKind.Set, GhostCell, Ghost);
        }

        CharacterStepReceipt step = _engine.Spatial.ProposeCharacterStep(new CharacterStepRequest(
            _session, _character, _motion, new CharacterSupport(false, CharacterSupportLifecycle.Active, 0, Pose(Vector3.Zero)),
            ReadOnlyMemory<CharacterObstacle>.Empty, ReadOnlyMemory<CharacterMeshInstance>.Empty, _config,
            new CharacterControllerCommand(Vector2.Zero, 0, false, false, false, Vector3.Zero, Vector3.Zero, Step, (ulong)_updates)));
        _character = step.Transform.Translation;
        _motion = step.Motion;
        if (_updates > 60)
        {
            _groundedUpdates += step.Motion.Grounded ? 1 : 0;
            _lowestCharacterY = MathF.Min(_lowestCharacterY, _character.Y);
        }
        if (_updates == EndUpdate)
        {
            Report();
        }
        return ProductUpdateResult.None;
    }

    private void Report()
    {
        VoxelSceneReadout scene = _engine.Voxel.ReadScene(new VoxelSceneReadRequest(_session));
        VoxelReadout ghost = _engine.Voxel.Read(new VoxelReadRequest(_session, GhostCell));
        SpatialHit ray = _engine.Spatial.CastRay(new SpatialRaycastRequest(
            _session, new Vector3(40.5f, 6, 40.5f), -Vector3.UnitY, 10, new SpatialQueryFilter(0, 0),
            ReadOnlyMemory<SpatialEntityCollider>.Empty, ReadOnlyMemory<ulong>.Empty,
            ReadOnlyMemory<SpatialEntityCollider>.Empty));
        VoxelChunkReadout reloaded = _engine.Voxel.ReadChunk(new VoxelChunkReadRequest(_session, Reloaded));

        Require(_groundedUpdates == EndUpdate - 60, $"character left the ground on {EndUpdate - 60 - _groundedUpdates} updates");
        Require(_lowestCharacterY > 1.9f, $"character sank to {_lowestCharacterY}");
        Require(scene.ResidentChunkCount == ChunksPerSide * ChunksPerSide, $"{scene.ResidentChunkCount} resident chunks");
        Require(reloaded.Present && reloaded.SolidVoxelCount == Chunk * Chunk, "reloaded chunk lost its floor");
        Require(_failedEditRejected, "the invalid edit was accepted");
        Require(ghost.Present && ghost.MaterialSlot == Ghost, "noncollidable voxel was not stored");
        Require(ray.Present && ray.Point.Y < 1.01f, $"ray stopped at {ray.Point.Y}, not the floor below the noncollidable voxel");

        double[] sorted = _editUs.ToArray();
        Array.Sort(sorted);
        double Percentile(double p) => sorted[(int)Math.Round((sorted.Length - 1) * p)];
        Console.WriteLine(
            $"VOXEL_FACTS {{\"updates\":{_updates},\"editUsP50\":{Percentile(0.5):F1},\"editUsP95\":{Percentile(0.95):F1}," +
            $"\"editUsMax\":{sorted[^1]:F1},\"residencyUs\":{_residencyUs[0]:F1},\"sourceRevision\":{scene.SourceRevision}," +
            $"\"residentChunks\":{scene.ResidentChunkCount},\"solidVoxels\":{scene.SolidVoxelCount}," +
            $"\"navigationCells\":{scene.NavigationCellCount},\"groundedUpdates\":{_groundedUpdates}," +
            $"\"characterY\":{_character.Y:F3},\"failedEditRejected\":{(_failedEditRejected ? "true" : "false")}," +
            $"\"rayHitY\":{ray.Point.Y:F3}}}");
        Console.WriteLine(_failures.Count == 0
            ? "VOXEL_EXERCISE_PASSED"
            : "VOXEL_EXERCISE_FAILED " + string.Join("; ", _failures));
        _reported = true;
    }

    private void Edit(VoxelEditKind kind, VoxelAddress address, uint material)
    {
        VoxelEdit[] edits = { new(kind, address, material) };
#if OLD_VOXEL_API
        ulong revision = _engine.Voxel.ReadScene(new VoxelSceneReadRequest(_session)).SourceRevision;
        _engine.Voxel.ApplyEdits(new VoxelEditTransaction(_session, revision, edits));
#else
        _engine.Voxel.ApplyEdits(new VoxelEditTransaction(_session, edits));
#endif
    }

    private void Admit(List<VoxelChunkIdentity> chunks)
    {
        int volume = Chunk * Chunk * Chunk;
        uint[] slots = new uint[volume * chunks.Count];
        var operations = new VoxelResidencyOperation[chunks.Count];
        for (int index = 0; index < chunks.Count; index++)
        {
            for (int x = 0; x < Chunk; x++)
                for (int z = 0; z < Chunk; z++)
                    slots[index * volume + x + Chunk * Chunk * z] = Floor;
#if OLD_VOXEL_API
            operations[index] = new VoxelResidencyOperation(
                VoxelResidencyOperationKind.Admit, chunks[index], 0, (uint)(index * volume), (uint)volume);
#else
            operations[index] = new VoxelResidencyOperation(
                VoxelResidencyOperationKind.Admit, chunks[index], (uint)(index * volume), (uint)volume);
#endif
        }
        Residency(operations, slots);
    }

    private void Evict(VoxelChunkIdentity chunk)
    {
#if OLD_VOXEL_API
        ulong hash = _engine.Voxel.ReadChunk(new VoxelChunkReadRequest(_session, chunk)).ContentHash;
        Residency(new[] { new VoxelResidencyOperation(VoxelResidencyOperationKind.Evict, chunk, hash, 0, 0) },
            Array.Empty<uint>());
#else
        Residency(new[] { new VoxelResidencyOperation(VoxelResidencyOperationKind.Evict, chunk, 0, 0) },
            Array.Empty<uint>());
#endif
    }

    private void Residency(VoxelResidencyOperation[] operations, uint[] slots)
    {
#if OLD_VOXEL_API
        ulong revision = _engine.Voxel.ReadScene(new VoxelSceneReadRequest(_session)).SourceRevision;
        _engine.Voxel.ApplyResidency(new VoxelResidencyTransaction(
            _session, revision, VoxelResidencyHistoryPolicy.ResetToPublishedAuthority, operations, slots));
#else
        _engine.Voxel.ApplyResidency(new VoxelResidencyTransaction(_session, operations, slots));
#endif
    }

    private static Transform Pose(Vector3 at) => new(at, Quaternion.Identity, Vector3.One);

    private void Require(bool condition, string failure)
    {
        if (!condition) _failures.Add(failure);
    }

    public void Pause() { }
    public void Resume() { }
    public void Restart() { }
    public void Shutdown() { }
    public bool CompleteTimeline(ProductTimelineCompletion completion) => false;
    public void Dispose()
    {
        if (!_reported) Console.WriteLine("VOXEL_EXERCISE_INCOMPLETE");
        _session.Dispose();
    }
}
