using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Numerics;
using Rusty.Engine;

namespace DynamicsExercise;

// #8738 evidence product. Everything happens in ordinary fixed updates:
// - a voxel floor, four crate towers, loose balls and a roped ball;
// - a character tethered to a heavy sled walks away for three seconds; its
//   pull reaches the sled as an anchor reaction through StepWithReactions;
// - update 300 teleports a ball, destroys another and adds a crate;
// - update 400 digs the floor out from under the first tower and rebinds.
// Update 600 prints the facts and DYNAMICS_EXERCISE_PASSED or the failures.
public sealed class Product : IEngineProduct
{
    private const float Step = 1.0f / 60.0f;
    private const float CrateHalf = 0.4f;
    private const float BallRadius = 0.3f;
    private const int Towers = 4;
    private const int TowerHeight = 6;
    private const int Balls = 16;
    private const float RopeLength = 3.0f;
    private const float CharacterTetherLength = 3.0f;
    private const long EditUpdate = 300;
    private const long DigUpdate = 400;
    private const long EndUpdate = 600;
    private const long WalkUpdates = 180;
    // Planar intent (0, -1) at zero yaw walks toward +Z, away from the sled.
    private static readonly Vector2 WalkIntent = new(0, -1);
    private const uint FloorMaterial = 3;
    private static readonly Vector3 RopeAnchor = new(16, 8, 4);
    private static readonly Vector3 SledStart = new(28.5f, 0.51f, 4.5f);
    private static readonly Vector3 CharacterStart = new(28.5f, 1.2f, 7.0f);

    private readonly IEngineContext _engine;
    private readonly SpatialSession _session;
    private readonly DynamicsWorld _world;
    private readonly List<DynamicsBody> _crates = new();
    private readonly List<DynamicsBody> _balls = new();
    private readonly DynamicsBody _ropeBall;
    private readonly DynamicsBody _sled;
    private readonly CharacterControllerConfig _characterConfig;
    private readonly List<double> _stepUs = new();
    private readonly List<string> _failures = new();
    private Vector3 _character = CharacterStart;
    private CharacterMotion _motion;
    private DynamicsBody? _addedCrate;
    private long _updates;
    private int _sleepingBeforeEdit;
    private int _reactions;
    private int _ropeCatches;
    private bool _characterTaut;
    private bool _reported;

    public Product(ProductCreateContext context)
    {
        _engine = context.Engine;
        _session = _engine.Spatial.CreateSession(new SpatialSessionConfig(1, 8, VoxelSurfaceMode.GreedyCubes));
        var floor = new List<VoxelEdit>();
        for (long x = 0; x < 32; x++)
            for (long z = 0; z < 32; z++)
                floor.Add(new VoxelEdit(VoxelEditKind.Set, new VoxelAddress(x, -1, z), FloorMaterial));
        ApplyEdits(floor);

        _world = _engine.Dynamics.CreateWorld(new DynamicsWorldConfig(new Vector3(0, -9.81f, 0)));
        _engine.Dynamics.BindWorldCollision(new DynamicsWorldCollisionBindingRequest(_world, _session));
        for (int tower = 0; tower < Towers; tower++)
            for (int level = 0; level < TowerHeight; level++)
                _crates.Add(Crate(new Vector3(6.5f + 6 * tower, CrateHalf + 0.01f + (2 * CrateHalf + 0.01f) * level, 16.5f)));
        for (int index = 0; index < Balls; index++)
            _balls.Add(Ball(new Vector3(4.5f + 1.5f * (index % 8), 3, 26.5f + 1.5f * (index / 8)), 1));
        _ropeBall = Ball(RopeAnchor + new Vector3(RopeLength, 0, 0), 2);
        _engine.Dynamics.SetFixedTether(new DynamicsFixedTetherRequest(
            _world, _ropeBall, Vector3.Zero, RopeAnchor, new DynamicsTetherConfig(1, RopeLength, RopeLength, 0, false)));
        _sled = _engine.Dynamics.CreateCuboidBody(new DynamicsCreateCuboidBodyRequest(_world, new DynamicsCuboidBodyConfig(
            Pose(SledStart), new Vector3(0.5f), Properties(80))));
        _characterConfig = _engine.Spatial.DefaultCharacterControllerConfig();
        _motion = new CharacterMotion(Vector3.Zero, Vector3.Zero, false, CharacterStance.Standing, 0, 0, 0, false, 0,
            Vector3.Zero, Vector3.Zero, Quaternion.Identity, Vector3.Zero, 0, 0, 0, 0);
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
        if (_updates == EditUpdate)
        {
            _sleepingBeforeEdit = Sleeping();
            _engine.Dynamics.Reset(new DynamicsResetRequest(_balls[0], Pose(new Vector3(20, 5, 26.5f)), Vector3.Zero, Vector3.Zero, false));
            _balls[1].Dispose();
            _addedCrate = Crate(new Vector3(24, 2, 28));
        }
        if (_updates == DigUpdate)
        {
            var dig = new List<VoxelEdit>();
            for (long x = 4; x < 9; x++)
                for (long z = 14; z < 19; z++)
                    dig.Add(new VoxelEdit(VoxelEditKind.Clear, new VoxelAddress(x, -1, z), 0));
            ApplyEdits(dig);
            _engine.Dynamics.BindWorldCollision(new DynamicsWorldCollisionBindingRequest(_world, _session));
        }

        // The character walks away from the sled on a rope tied to it.
        DynamicsAnchorObservation anchor = _engine.Dynamics.ObserveAnchor(new DynamicsObserveAnchorRequest(_world, _sled, Vector3.Zero));
        CharacterStepReceipt character = _engine.Spatial.ProposeCharacterStep(new CharacterStepRequest(
            _session, _character, _motion, new CharacterSupport(false, CharacterSupportLifecycle.Active, 0, Pose(Vector3.Zero)),
            ReadOnlyMemory<CharacterObstacle>.Empty, ReadOnlyMemory<CharacterMeshInstance>.Empty, _characterConfig,
            new CharacterControllerCommand(_updates <= WalkUpdates ? WalkIntent : Vector2.Zero, 0, false, false, false, Vector3.Zero, Vector3.Zero, Step, (ulong)_updates))
            with { Tether = CharacterTetherRequest.AtDynamicAnchor(7, anchor, CharacterTetherLength) });
        _character = character.Transform.Translation;
        _motion = character.Motion;
        _characterTaut |= character.Tether.Taut;
        ReadOnlyMemory<DynamicsAnchorReaction> reactions = character.Tether.Reaction.Present
            ? new[] { character.Tether.Reaction }
            : ReadOnlyMemory<DynamicsAnchorReaction>.Empty;
        _reactions += reactions.Length;

        long started = Stopwatch.GetTimestamp();
        _engine.Dynamics.StepWithReactions(new DynamicsStepWithReactionsRequest(
            _world, Step, 1, ReadOnlyMemory<DynamicsAction>.Empty, reactions));
        _stepUs.Add(Stopwatch.GetElapsedTime(started).TotalMicroseconds);
        if (_engine.Dynamics.ReadTether(new DynamicsTetherRequest(_world, 1)).Caught)
        {
            _ropeCatches++;
        }
        if (_updates == EndUpdate)
        {
            Report();
        }
        return ProductUpdateResult.None;
    }

    private void Report()
    {
        DynamicsWorldReadout world = _engine.Dynamics.ReadWorld(new DynamicsWorldReadRequest(_world));
        float Top(DynamicsBody body) => _engine.Dynamics.Read(new DynamicsReadRequest(body)).Transform.Translation.Y;
        DynamicsReadout sled = _engine.Dynamics.Read(new DynamicsReadRequest(_sled));
        DynamicsTetherReadout rope = _engine.Dynamics.ReadTether(new DynamicsTetherRequest(_world, 1));
        Vector3 sledTravel = sled.Transform.Translation - SledStart;
        Vector3 characterTravel = _character - CharacterStart;
        float lastTower = Top(_crates[Towers * TowerHeight - 1]);
        float dugTower = Top(_crates[TowerHeight - 1]);
        float teleported = Top(_balls[0]);
        float added = Top(_addedCrate!);
        int sleeping = Sleeping();

        Require(_sleepingBeforeEdit >= 30, $"only {_sleepingBeforeEdit} bodies slept before the edit");
        Require(MathF.Abs(lastTower - (CrateHalf + (2 * CrateHalf + 0.01f) * (TowerHeight - 1))) < 0.1f, $"undug tower top at {lastTower}");
        Require(dugTower < -5, $"dug tower top still at {dugTower}");
        Require(MathF.Abs(teleported - BallRadius) < 0.05f, $"teleported ball at {teleported}");
        Require(MathF.Abs(added - CrateHalf) < 0.05f, $"added crate at {added}");
        Require(MathF.Abs(rope.Distance - RopeLength) < 0.01f && rope.Taut, $"rope distance {rope.Distance}");
        Require(_ropeCatches == 1, $"rope caught {_ropeCatches} times");
        Require(_characterTaut && _reactions > 0, $"character tether never pulled (taut {_characterTaut}, reactions {_reactions})");
        Require(characterTravel.Z > 3 && sledTravel.Z > 0.5f,
            $"sled moved {sledTravel} while the character moved {characterTravel}");
        Require(MathF.Abs(sled.Transform.Translation.Y - 0.5f) < 0.1f, $"sled left the floor: {sled.Transform.Translation}");
        Require(world.BodyCount == (uint)(Towers * TowerHeight + Balls + 2), $"world has {world.BodyCount} bodies");

        double[] sorted = _stepUs.ToArray();
        Array.Sort(sorted);
        double Percentile(double p) => sorted[(int)Math.Round((sorted.Length - 1) * p)];
        double Mean(int from, int to)
        {
            double sum = 0;
            for (int index = from; index < to; index++) sum += _stepUs[index];
            return sum / (to - from);
        }
        Console.WriteLine(
            $"DYNAMICS_FACTS {{\"updates\":{_updates},\"stepUsP50\":{Percentile(0.5):F1},\"stepUsP95\":{Percentile(0.95):F1}," +
            $"\"meanUsSettling\":{Mean(0, 120):F1},\"meanUsSettled\":{Mean(200, (int)EditUpdate - 1):F1},\"meanUsAfterDig\":{Mean((int)DigUpdate + 60, (int)EndUpdate):F1}," +
            $"\"sleepingBeforeEdit\":{_sleepingBeforeEdit},\"sleepingAtEnd\":{sleeping},\"bodies\":{world.BodyCount},\"contacts\":{world.ContactCount}," +
            $"\"undugTowerTopY\":{lastTower:F3},\"dugTowerTopY\":{dugTower:F3},\"teleportedY\":{teleported:F3},\"addedCrateY\":{added:F3}," +
            $"\"ropeDistance\":{rope.Distance:F4},\"ropeCatches\":{_ropeCatches},\"characterReactions\":{_reactions}," +
            $"\"characterTravel\":\"{characterTravel}\",\"sledTravel\":\"{sledTravel}\"}}");
        Console.WriteLine(_failures.Count == 0
            ? "DYNAMICS_EXERCISE_PASSED"
            : "DYNAMICS_EXERCISE_FAILED " + string.Join("; ", _failures));
        _reported = true;
    }

    private int Sleeping()
    {
        int count = 0;
        foreach (DynamicsBody body in _crates) count += _engine.Dynamics.Read(new DynamicsReadRequest(body)).Sleeping ? 1 : 0;
        for (int index = 0; index < _balls.Count; index++)
        {
            if (_updates >= EditUpdate && index == 1) continue;
            count += _engine.Dynamics.Read(new DynamicsReadRequest(_balls[index])).Sleeping ? 1 : 0;
        }
        return count;
    }

    private void ApplyEdits(List<VoxelEdit> edits)
    {
        VoxelSceneReadout scene = _engine.Voxel.ReadScene(new VoxelSceneReadRequest(_session));
        _engine.Voxel.ApplyEdits(new VoxelEditTransaction(_session, scene.SourceRevision, edits.ToArray()));
    }

    private DynamicsBody Crate(Vector3 at) => _engine.Dynamics.CreateCuboidBody(new DynamicsCreateCuboidBodyRequest(
        _world, new DynamicsCuboidBodyConfig(Pose(at), new Vector3(CrateHalf), Properties(10))));

    private DynamicsBody Ball(Vector3 at, float mass) => _engine.Dynamics.CreateSphereBodyWithProperties(
        new DynamicsCreateSphereBodyPropertiesRequest(_world, new DynamicsSphereBodyPropertiesConfig(Pose(at), BallRadius, Properties(mass))));

    private static DynamicsBodyProperties Properties(float mass) => new(
        mass, default, Vector3.Zero, Vector3.Zero, default, 0, 0, 1, 0.5f, 0, uint.MaxValue, uint.MaxValue, true, false, false);

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
        if (!_reported) Console.WriteLine("DYNAMICS_EXERCISE_INCOMPLETE");
        _world.Dispose();
        _session.Dispose();
    }
}
