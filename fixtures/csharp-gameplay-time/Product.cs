using System.Numerics;
using System.Text;
using System.Text.Json;
using Rusty.Engine;
using Rusty.Engine.Application;
using Rusty.Engine.Debugging;
using Rusty.Engine.Input;

namespace CsharpGameplayTime;

/// <summary>
/// Movement-driven time: the world moves only as fast as the player does.
/// Looking costs nothing; moving runs the world up to realtime; a shot buys a
/// short bounded advance and then the world holds again. The Engine admits the
/// steps and delivers input every update; this product owns every rule here.
/// </summary>
public sealed class Product : IEngineProduct, IDebugCommandModuleSource, IDebugCommandModule
{
    // Time policy.
    private const double IdleRate = 0.0;
    private const double CrawlRate = 0.05;
    private const double ShotAdvanceSeconds = 0.35;
    private const double WaitAdvanceSeconds = 2.0;
    private const ulong ShotCooldownSteps = 60;
    private const float MovementForRealtime = 1f;
    // World.
    private const float ArenaHalfSize = 9f;
    private const float EyeHeight = 1.6f;
    private const float WalkSpeed = 4f;
    private const float ShotSpeed = 18f;
    private const float OrbSpeed = 3f;
    private const float DroneOrbitRadius = 5f;
    private const float DroneAngularSpeed = 0.6f;
    private const float HitRadius = 0.6f;
    private const ulong TurretPeriodSteps = 90;
    private const float ProjectileLifetimeSeconds = 4f;
    private static readonly Vector3 TurretPosition = new(0, 1.2f, -8);
    private const float RunnerSpeed = 2.5f;
    private const float RunnerLane = 6f;
    private const float RunnerScale = 1f;
    private const float SparkLifetimeSeconds = 2f;
    private const uint SparkCount = 60;
    private const float SmokePerSecond = 8f;
    // Object ids.
    private const ulong FloorId = 1, TurretId = 2, RunnerId = 3, DroneIdBase = 10, ProjectileIdBase = 1000;
    private const int DroneCount = 3;

    private readonly IEngineContext engine;
    private readonly Camera camera;
    private readonly UiStream hud;
    private readonly Appearance floorLook, turretLook, droneLook, shotLook, orbLook;
    private readonly RenderResource runnerMesh;
    private readonly Appearance runnerLook;
    private readonly AnimationInstance runnerAnimation;
    private readonly PresentationEmitter smoke;
    private float runnerX = -RunnerLane;
    private float runnerDirection = 1;
    private readonly FpsInput input = new(FpsInputConfig.Standard);
    private readonly SimulationScheduler scheduler = new();
    private readonly List<Projectile> projectiles = new();
    private readonly float[] droneAngles = new float[DroneCount];
    private LookState look;
    private Vector3 body = new(0, 0, 4);
    private ulong nextProjectileId = ProjectileIdBase;
    private ulong cooldownReadyStep;
    private ulong simulationStep;
    private bool crawl;
    private bool realtime;
    private int hits;
    private Vector3 lastHit;
    private int shots;
    private ProductUpdateFacts lastFacts;
    private ulong hudSequence;
    private bool turretScheduled;

    private sealed class Projectile(ulong id, Vector3 position, Vector3 velocity, bool fromPlayer, float age)
    {
        public ulong Id { get; } = id;
        public Vector3 Position { get; set; } = position;
        public Vector3 Velocity { get; } = velocity;
        public bool FromPlayer { get; } = fromPlayer;
        public float Age { get; set; } = age;
    }

    public Product(ProductCreateContext context)
    {
        engine = context.Engine;
        floorLook = Primitive(PrimitiveGeometry.Cube, new Color(.22f, .24f, .28f, 1));
        turretLook = Primitive(PrimitiveGeometry.Cube, new Color(.55f, .1f, .1f, 1));
        droneLook = Primitive(PrimitiveGeometry.Sphere, new Color(.9f, .3f, .2f, 1));
        shotLook = Primitive(PrimitiveGeometry.Sphere, new Color(1f, .9f, .2f, 1));
        orbLook = Primitive(PrimitiveGeometry.Sphere, new Color(.3f, .6f, 1f, 1));
        for (int index = 0; index < DroneCount; index++)
            droneAngles[index] = index * MathF.Tau / DroneCount;
        // The runner's clip, the turret's smoke and the hit sparks all play
        // on Engine world time: they hold with the world and slow with it.
        using (ContentReference runner = engine.Content.OpenReference(new("runner.glb")))
            runnerMesh = engine.Animation.OpenAnimatedMeshFromContent(new(runner));
        runnerLook = engine.Animation.CreateAnimatedMeshAppearance(new(runnerMesh));
        PublishWorld();
        runnerAnimation = engine.Animation.CreateInstance(new(runnerLook, RunnerId));
        string clip = engine.Animation.ReadClips(runnerMesh).Span[0].Name;
        engine.Animation.SetPlayback(new(runnerAnimation, AnimationPlaybackKind.Play, clip, AnimationLoopMode.Repeat, 1, 1, true, 0, false, 0));
        smoke = engine.Presentation.CreateEmitter(Smoke());
        camera = engine.CameraView.CreateCamera(CameraDescriptor());
        engine.CameraView.SetActiveCamera(camera);
        hud = engine.Ui.OpenStream(new UiStreamRequest("gameplay-time", "gameplay-time.hud.v1"));
        // Opting into gameplay time: from now on every observation updates.
        engine.GameplayTime.Hold();
        Publish();
    }

    public ProductUpdateResult Update(ProductUpdate update)
    {
        ProductUpdateFacts facts = update.Facts;
        lastFacts = facts;
        // Look and controls every update, in host time: they never wait for,
        // or slow with, the world.
        FpsInputFrame frame = input.Consume(update.Input, (float)facts.HostElapsedSeconds);
        look = input.IntegrateLook(look, frame).After;
        if (input.Physical.Pressed(KeyboardControl.KeyT) || input.Physical.Pressed(ControllerButton.Button3))
            realtime = !realtime;
        if (input.Physical.Pressed(KeyboardControl.KeyG) || input.Physical.Pressed(ControllerButton.Button2))
            crawl = !crawl;
        bool fire = input.Physical.Pressed(PointerButton.Primary) || input.Physical.Pressed(ControllerButton.Button5);
        bool wait = input.Physical.Pressed(KeyboardControl.KeyB) || input.Physical.Pressed(ControllerButton.Button4);

        // The world, once per admitted step.
        if (!turretScheduled && facts.AdmittedStepCount > 0)
        {
            scheduler.ScheduleRepeatingAt(facts.SimulationStep + TurretPeriodSteps, TurretPeriodSteps, _ => TurretFires());
            turretScheduled = true;
        }
        for (uint offset = 0; offset < facts.AdmittedStepCount; offset++)
            Step((float)facts.FixedDeltaSeconds, frame.Movement);
        scheduler.Advance(update);
        simulationStep = facts.SimulationStep + facts.AdmittedStepCount;

        // A shot is taken once, from its press, and buys its own duration.
        if (fire && simulationStep >= cooldownReadyStep)
        {
            Shoot();
            engine.GameplayTime.Advance(ShotAdvanceSeconds);
        }
        else if (wait)
        {
            // Let the world run two seconds, then hold again.
            engine.GameplayTime.Advance(WaitAdvanceSeconds);
        }
        else
        {
            ChooseRate(facts, frame.Movement);
        }
        Publish();
        return ProductUpdateResult.None;
    }

    /// <summary>Movement buys time in proportion to how much the player moves.</summary>
    private void ChooseRate(ProductUpdateFacts facts, Vector2 movement)
    {
        double idle = crawl ? CrawlRate : IdleRate;
        double rate = realtime ? 1.0 : Math.Max(idle, Math.Min(movement.Length() / MovementForRealtime, 1.0));
        bool advancing = facts.GameplayAdvanceRemainingSteps > 0;
        // Let a shot's advance finish unless the player moves faster.
        if (advancing && rate <= facts.GameplayRate)
            return;
        if (rate != facts.GameplayRate || advancing)
            engine.GameplayTime.SetRate(rate);
    }

    private void Step(float dt, Vector2 movement)
    {
        Vector3 forward = new(MathF.Sin(look.YawRadians), 0, -MathF.Cos(look.YawRadians));
        Vector3 right = new(-forward.Z, 0, forward.X);
        body += (forward * movement.Y + right * movement.X) * WalkSpeed * dt;
        body = Vector3.Clamp(body, new(-ArenaHalfSize, 0, -ArenaHalfSize), new(ArenaHalfSize, 0, ArenaHalfSize));
        for (int index = 0; index < DroneCount; index++)
            droneAngles[index] += DroneAngularSpeed * dt;
        runnerX += runnerDirection * RunnerSpeed * dt;
        if (MathF.Abs(runnerX) > RunnerLane)
        {
            runnerX = Math.Clamp(runnerX, -RunnerLane, RunnerLane);
            runnerDirection = -runnerDirection;
        }
        foreach (Projectile projectile in projectiles)
        {
            projectile.Position += projectile.Velocity * dt;
            projectile.Age += dt;
        }
        projectiles.RemoveAll(projectile => projectile.Age > ProjectileLifetimeSeconds || HitsDrone(projectile));
    }

    private bool HitsDrone(Projectile projectile)
    {
        if (!projectile.FromPlayer) return false;
        for (int index = 0; index < DroneCount; index++)
        {
            if (Vector3.Distance(projectile.Position, DronePosition(index)) > HitRadius) continue;
            hits++;
            lastHit = DronePosition(index);
            Sparks(lastHit);
            droneAngles[index] += MathF.PI;
            return true;
        }
        return false;
    }

    private void Shoot()
    {
        Vector3 eye = body + Vector3.UnitY * EyeHeight;
        Vector3 direction = Look.Rebase(new(look, look, FpsInputConfig.Standard.PointerLookConfig)).Forward;
        projectiles.Add(new(nextProjectileId++, eye + direction * .5f, direction * ShotSpeed, true, 0));
        cooldownReadyStep = simulationStep + ShotCooldownSteps;
        shots++;
    }

    private void TurretFires()
    {
        Vector3 toward = Vector3.Normalize(body + Vector3.UnitY * EyeHeight - TurretPosition);
        projectiles.Add(new(nextProjectileId++, TurretPosition, toward * OrbSpeed, false, 0));
    }

    private Vector3 DronePosition(int index) => new(
        MathF.Cos(droneAngles[index]) * DroneOrbitRadius,
        1.5f + .5f * MathF.Sin(droneAngles[index] * 2),
        -2 + MathF.Sin(droneAngles[index]) * DroneOrbitRadius);

    private void Sparks(Vector3 at) => engine.Presentation.EmitParticles(new PresentationParticleDescriptor
    {
        SignalId = "gameplay-time.spark",
        Visible = true,
        Anchor = new() { Kind = PresentationAnchorKind.World, Position = at },
        Visual = PresentationParticleVisual.Cube,
        BurstCount = SparkCount,
        MaxParticles = SparkCount,
        LifetimeMinSeconds = SparkLifetimeSeconds * .75f,
        LifetimeMaxSeconds = SparkLifetimeSeconds,
        VelocityMin = new(-3, 1, -3),
        VelocityMax = new(3, 5, 3),
        Acceleration = new(0, -6, 0),
        Seed = (ulong)hits,
        SizeCurve = new PresentationParticleScalarKey[] { new(0, .3f), new(1, .05f) },
        ColorCurve = new PresentationParticleColorKey[] { new(0, new Color(1, .85f, .3f, 1)), new(1, new Color(1, .2f, 0, 0)) },
    });

    private PresentationParticleDescriptor Smoke() => new()
    {
        LogicalId = TurretId,
        SignalId = "gameplay-time.smoke",
        Visible = true,
        Anchor = new() { Kind = PresentationAnchorKind.World, Position = TurretPosition + Vector3.UnitY * .5f },
        Visual = PresentationParticleVisual.Cube,
        RatePerSecond = SmokePerSecond,
        MaxParticles = 64,
        LifetimeMinSeconds = 2.5f,
        LifetimeMaxSeconds = 3.5f,
        VelocityMin = new(-.2f, .6f, -.2f),
        VelocityMax = new(.2f, 1f, .2f),
        Seed = TurretId,
        SizeCurve = new PresentationParticleScalarKey[] { new(0, .12f), new(1, .4f) },
        ColorCurve = new PresentationParticleColorKey[] { new(0, new Color(.6f, .6f, .6f, .9f)), new(1, new Color(.3f, .3f, .3f, 0)) },
    };

    private void Publish()
    {
        PublishWorld();
        engine.CameraView.UpdateCamera(new(camera, CameraDescriptor()));
        PublishHud();
    }

    private void PublishWorld()
    {
        List<AppearanceFact> facts =
        [
            Fact(FloorId, floorLook, new(0, -.05f, 0), new(ArenaHalfSize * 2, .1f, ArenaHalfSize * 2)),
            Fact(TurretId, turretLook, TurretPosition, new(.8f, .8f, .8f)),
            new(RunnerId, false, 0, new Transform(new(runnerX, 0, -4),
                Quaternion.CreateFromAxisAngle(Vector3.UnitY, runnerDirection * MathF.PI / 2), new(RunnerScale)), runnerLook, true, RenderLayer.Scene),
        ];
        for (int index = 0; index < DroneCount; index++)
            facts.Add(Fact(DroneIdBase + (ulong)index, droneLook, DronePosition(index), new(.7f, .7f, .7f)));
        foreach (Projectile projectile in projectiles)
            facts.Add(Fact(projectile.Id, projectile.FromPlayer ? shotLook : orbLook, projectile.Position,
                projectile.FromPlayer ? new(.15f, .15f, .15f) : new(.35f, .35f, .35f)));
        engine.Graphics.PublishSnapshot(facts.ToArray());
    }

    private static AppearanceFact Fact(ulong id, Appearance appearance, Vector3 position, Vector3 scale) =>
        new(id, false, 0, new Transform(position, Quaternion.Identity, scale), appearance, true, RenderLayer.Scene);

    private Appearance Primitive(PrimitiveGeometry geometry, Color color) =>
        engine.Graphics.CreatePrimitive(new(geometry, false, color));

    private CameraDescriptor CameraDescriptor() => new(
        new(body + Vector3.UnitY * EyeHeight, float.RadiansToDegrees(look.PitchRadians), float.RadiansToDegrees(look.YawRadians)),
        CameraBasisMode.Derived, default, new(CameraProjectionKind.Perspective, 70, 0, .05, 100), CameraViewports.Full);

    private void PublishHud()
    {
        GameplayTimeReadout time = engine.GameplayTime.Read();
        HudWriter writer = new();
        writer.Number("rate", time.Rate);
        writer.Bool("held", time.Held);
        writer.Number("advanceSteps", time.AdvanceRemainingSteps);
        writer.Number("step", simulationStep);
        writer.Number("cooldownSteps", cooldownReadyStep > simulationStep ? cooldownReadyStep - simulationStep : 0);
        writer.Bool("crawl", crawl);
        writer.Bool("realtime", realtime);
        writer.Number("hits", hits);
        engine.Ui.PublishProjection(new UiProjection(hud, ++hudSequence, writer.Value()));
    }

    public void RegisterDebugCommands(IDebugCommandModuleRegistrar registrar) => registrar.Register(this);

    [DebugCommand("gameplay.observe", Description = "Read-only: simulation step, gameplay time, player body and look, drone and projectile positions, cooldown and hits.")]
    public string Observe() => JsonSerializer.Serialize(new
    {
        step = simulationStep,
        rate = lastFacts.GameplayRate,
        advanceSteps = lastFacts.GameplayAdvanceRemainingSteps,
        hostElapsedSeconds = lastFacts.HostElapsedSeconds,
        body = new[] { body.X, body.Y, body.Z },
        yaw = look.YawRadians,
        pitch = look.PitchRadians,
        drones = Enumerable.Range(0, DroneCount).Select(index => { Vector3 p = DronePosition(index); return new[] { p.X, p.Y, p.Z }; }),
        runner = runnerX,
        projectiles = projectiles.Select(p => new { id = p.Id, player = p.FromPlayer, position = new[] { p.Position.X, p.Position.Y, p.Position.Z } }),
        cooldownSteps = cooldownReadyStep > simulationStep ? cooldownReadyStep - simulationStep : 0,
        shots,
        hits,
        lastHit = new[] { lastHit.X, lastHit.Y, lastHit.Z },
        crawl,
        realtime,
    });

    public void Start() { }
    // A menu pause keeps no held controls for resume.
    public void Pause() => input.Physical.Clear();
    public void Resume() { }
    public void Restart() { }
    public void Shutdown() { }

    public void Dispose()
    {
        smoke.Dispose();
        runnerAnimation.Dispose();
        engine.Graphics.PublishSnapshot([]);
        runnerLook.Dispose();
        runnerMesh.Dispose();
        hud.Dispose();
        camera.Dispose();
        floorLook.Dispose();
        turretLook.Dispose();
        droneLook.Dispose();
        shotLook.Dispose();
        orbLook.Dispose();
    }

    /// <summary>One flat HUD object of numbers and booleans.</summary>
    private sealed class HudWriter
    {
        private readonly List<StructuredValueNode> nodes = [default];
        private readonly StringBuilder keys = new();

        public void Number(string key, double value) => Add(key, StructuredValueKind.Number, 0, value);
        public void Bool(string key, bool value) => Add(key, StructuredValueKind.Bool, value ? 1u : 0u, 0);

        private void Add(string key, StructuredValueKind kind, uint boolValue, double number)
        {
            uint offset = (uint)Encoding.UTF8.GetByteCount(keys.ToString());
            keys.Append(key);
            nodes.Add(new(kind, boolValue, number, offset, (uint)Encoding.UTF8.GetByteCount(key), 0, 0, 0, 0));
        }

        public UiValue Value()
        {
            uint count = (uint)nodes.Count - 1;
            nodes[0] = new(StructuredValueKind.Object, 0, 0, 0, 0, 0, 0, 0, count);
            uint[] edges = Enumerable.Range(1, (int)count).Select(index => (uint)index).ToArray();
            return new UiValue(nodes.ToArray(), edges, 0, Encoding.UTF8.GetBytes(keys.ToString()));
        }
    }
}
