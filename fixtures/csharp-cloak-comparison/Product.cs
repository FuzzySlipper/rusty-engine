using System.Diagnostics;
using System.Globalization;
using System.Numerics;
using System.Text;
using Rusty.Engine;
using Rusty.Engine.Debugging;

namespace CsharpCloakComparison;

/// <summary>
/// The owner's mage with her cloak simulated two ways, on the same scripted
/// run: <see cref="CloakMode.Bones"/> (cape bones simulated and skinned) or
/// <see cref="CloakMode.Soft"/> (a soft sheet under the cloak). One mode runs
/// at a time, so its costs are its own. <c>cloak.mode</c> switches and
/// restarts; <c>cloak.count</c> runs several mages side by side;
/// <c>cloak.report</c> gives each phase's measurements; <c>cloak.freeze</c>
/// holds a moment of the run for matched stills.
/// </summary>
public sealed class Product : IEngineProduct, IDebugCommandModuleSource, IDebugCommandModule
{
    private const ulong FloorId = 1, PillarId = 2, SunId = 3, SkyId = 4, FirstCharacterId = 100;
    private const float FloorHalfSize = 30, FloorThickness = .2f;
    /// <summary>Mages beside the first, this far apart (m).</summary>
    private const float LaneSpacing = 2.5f;
    /// <summary>The pillar the cloak brushes: beside the path halfway through the brush phase.</summary>
    private const float PillarRadius = .12f, PillarHeight = 2.2f, PillarOffset = .38f;
    /// <summary>The gust's mean speed (m/s, across the run), how much it swings and its period (s).</summary>
    private const float WindSpeed = 8, GustShare = .35f, GustPeriod = 1.7f;
    private const float CameraDistance = 2.8f, CameraHeight = 1.4f, LookHeight = 1.0f, CameraFollow = .12f;
    private static readonly Color White = new(1, 1, 1, 1);
    private static readonly Transform Identity = new(Vector3.Zero, Quaternion.Identity, Vector3.One);

    private readonly IEngineContext engine;
    private readonly Camera camera;
    private readonly Cast cast;
    private readonly SpatialSession spatial;
    private readonly Appearance floor, pillar;
    private readonly Light sun, sky;
    private readonly Dictionary<string, (uint Index, float Duration)> clips = [];
    private readonly Vector3 pillarAt;
    private readonly List<Character> characters = [];
    private DynamicsWorld world;
    private CloakMode mode = CloakMode.Bones;
    private int count = 1;
    private CloakTuning tuning = new();
    private Probe? probe;
    /// <summary>Whether the first mage's cloak is measured (its readouts cost time a cost run leaves out).</summary>
    private bool probing = true;
    private PhaseFacts[] facts = [];
    private float? freezeAt;
    private string view = "side";
    private Vector3 cameraAt;
    private double runs;

    public Product(ProductCreateContext context)
    {
        engine = context.Engine;
        camera = engine.CameraView.CreateCamera(CameraLooking(new(CameraDistance, CameraHeight, 0), new(0, LookHeight, 0)));
        engine.CameraView.SetActiveCamera(camera);

        using ContentReference bodySource = engine.Content.OpenReference(new("mage.glb"));
        using ContentReference capeSource = engine.Content.OpenReference(new("mage-cape.glb"));
        using ContentReference clipSource = engine.Content.OpenReference(new("mage.clips.glb"));
        RenderResource bodyOnly = engine.Animation.OpenAnimatedMeshFromContent(new(bodySource));
        RenderResource bodyAndCape = engine.Animation.OpenAnimatedMeshFromContent(new(capeSource));
        RenderResource pack = engine.Animation.OpenAnimationClipPackFromContent(new(clipSource));
        foreach (RenderResource mesh in new[] { bodyOnly, bodyAndCape })
        {
            engine.Animation.AssociateAnimationClipPack(new(mesh, pack, "Quaternius UAL1 retargeted", "CC0-1.0"));
        }
        ReadOnlySpan<AnimationClipInfo> listed = engine.Animation.ReadClips(bodyOnly).Span;
        for (int index = 0; index < listed.Length; index++)
        {
            clips[listed[index].Id] = ((uint)index, listed[index].DurationSeconds);
        }
        RenderResourceReference texture = engine.Graphics.OpenResource(new("cloak.png", TextureFilter.Linear, TextureWrap.Repeat, TextureColorSpace.Srgb, "")).Handle;
        Material cloakMaterial = engine.Graphics.CreateMaterial(new MaterialRequest(White, texture, .85f, White, Vector3.Zero, 0, false, MaterialAlphaMode.Opaque, 0));
        cast = new Cast(bodyOnly, bodyAndCape, CloakAsset.Read(engine, "cloak.json"), cloakMaterial);

        // Low and from the side, so the cloak's folds shade.
        sun = engine.Graphics.CreateLight(new(SunId, false, 0, new(LightKind.Directional, new(1, .95f, .85f), 3, true, Vector3.Zero, Vector3.Normalize(new(.6f, -.8f, .4f)), false, 0, 0, 0, 0, LightShadowIntent.Requested)));
        sky = engine.Graphics.CreateLight(new(SkyId, false, 0, LightDescriptor.Hemisphere(new(.55f, .7f, 1), new(.3f, .3f, .25f), .6f)));
        floor = engine.Graphics.CreatePrimitive(new(PrimitiveGeometry.Cube, false, new Color(.32f, .34f, .3f, 1)));
        pillar = engine.Graphics.CreatePrimitive(new(PrimitiveGeometry.Cube, false, new Color(.55f, .5f, .45f, 1)));
        pillarAt = PillarPlace();
        spatial = engine.Spatial.CreateSession(new SpatialSessionConfig(1.0, 16, VoxelSurfaceMode.GreedyCubes));
        engine.Spatial.ReplaceCollision(Collision());
        world = CreateWorld();
        Reset();
    }

    /// <summary>Beside the path where the mage is halfway through the brush phase, on her right.</summary>
    private static Vector3 PillarPlace()
    {
        var runner = new Runner(Vector3.Zero);
        float start = Sequence.Phases.TakeWhile(phase => !phase.Brush).Sum(phase => phase.Seconds);
        float middle = start + Sequence.Phases.First(phase => phase.Brush).Seconds / 2;
        float dt = 1f / 60;
        while (runner.Seconds < middle) runner.Advance(dt);
        Vector3 right = Vector3.Cross(runner.Forward, Vector3.UnitY);
        return runner.Position + right * PillarOffset;
    }

    private CollisionReplaceRequest Collision()
    {
        Vector3 p = pillarAt;
        float r = PillarRadius, h = PillarHeight;
        Vector3[] vertices =
        [
            new(-FloorHalfSize, 0, -FloorHalfSize), new(FloorHalfSize, 0, -FloorHalfSize),
            new(FloorHalfSize, 0, FloorHalfSize), new(-FloorHalfSize, 0, FloorHalfSize),
            p + new Vector3(-r, 0, -r), p + new Vector3(r, 0, -r), p + new Vector3(-r, h, -r), p + new Vector3(r, h, -r),
            p + new Vector3(-r, 0, r), p + new Vector3(r, 0, r), p + new Vector3(-r, h, r), p + new Vector3(r, h, r),
        ];
        // An asset's triangles count from its first vertex.
        uint[][] faces = [[0, 2, 1], [1, 2, 3], [4, 5, 6], [5, 7, 6], [0, 1, 4], [1, 5, 4], [2, 6, 3], [3, 6, 7], [0, 4, 2], [2, 4, 6], [1, 3, 5], [3, 7, 5]];
        Triangle[] triangles = [new(0, 2, 1), new(0, 3, 2), .. faces.Select(face => new Triangle(face[0], face[1], face[2]))];
        return new(spatial,
            new[] { new StaticMeshAsset(1, 0, 4, 0, 2), new StaticMeshAsset(2, 4, 8, 2, 12) },
            vertices, triangles,
            new[] { new StaticMeshInstance(1, 1, Identity), new StaticMeshInstance(2, 2, Identity) });
    }

    private DynamicsWorld CreateWorld()
    {
        DynamicsWorld created = engine.Dynamics.CreateWorld(new DynamicsWorldConfig(new Vector3(0, -9.81f, 0)));
        engine.Dynamics.BindWorldCollision(new DynamicsWorldCollisionBindingRequest(created, spatial));
        engine.Dynamics.ConfigureSoftBodies(new DynamicsSoftBodySettingsRequest(created) with { Substeps = tuning.SoftSubsteps });
        return created;
    }

    /// <summary>A fresh world and fresh mages at the start of the run, in the current mode.</summary>
    private void Reset()
    {
        // The last run's mages leave the scene before they are released.
        Character[] retiring = [.. characters];
        characters.Clear();
        Publish();
        foreach (Character character in retiring) character.Dispose();
        world.Dispose();
        world = CreateWorld();
        for (int lane = 0; lane < count; lane++)
        {
            characters.Add(new Character(engine, cast, tuning, world, mode, FirstCharacterId + 2 * (ulong)lane, new Vector3(lane * LaneSpacing, 0, 0)));
        }
        probe = null;
        facts = [.. Sequence.Phases.Select(_ => new PhaseFacts())];
        cameraAt = characters[0].Runner.Root + new Vector3(CameraDistance, CameraHeight, 0);
        Publish();
        runs++;
    }

    public void Start() { }

    public ProductUpdateResult Update(ProductUpdate update)
    {
        ProductUpdateFacts updateFacts = update.Facts;
        if (updateFacts.AdmittedStepCount > 0)
        {
            Run((float)updateFacts.FixedDeltaSeconds, (int)updateFacts.AdmittedStepCount);
        }
        Publish();
        return ProductUpdateResult.None;
    }

    /// <summary>
    /// One call's steps: the run advances step by step, the pose is set once
    /// for where it ends (the call evaluates it once), and the world steps
    /// the same steps in one request, so what follows the pose carries it
    /// over all of them.
    /// </summary>
    private void Run(float dt, int admitted)
    {
        foreach (Character character in characters.Where(character => !character.Attached))
        {
            AnimationJointPoseResult pose = engine.Animation.ReadJointPose(character.Instance);
            if (!pose.Reported) continue;
            character.Attach();
            if (character == characters[0] && probing)
            {
                probe = new Probe(engine, character, cast.Cloak, character.Bones, pose.Joints.Span);
            }
        }
        if (!characters.All(character => character.Attached)) return;
        probe?.MeasureCollar();
        Character first = characters[0];
        int steps = 0;
        while (steps < admitted && first.Runner.Seconds < (freezeAt ?? Sequence.Duration) && first.Runner.Seconds < Sequence.Duration)
        {
            foreach (Character character in characters) character.Runner.Advance(dt);
            steps++;
        }
        if (steps == 0) return;
        foreach (Character character in characters)
        {
            Animate(character);
            (Phase phase, _, _) = character.Runner.Now;
            // Still air drags a moving cloak too, so the drag always acts.
            character.Blow(phase.Windy ? Gust(character.Runner.Seconds) : Vector3.Zero);
        }
        long started = Stopwatch.GetTimestamp();
        engine.Dynamics.Step(new(world, dt, (uint)steps, ReadOnlyMemory<DynamicsAction>.Empty));
        double micro = Stopwatch.GetElapsedTime(started).TotalMicroseconds / steps;

        (Phase now, int index, float into) = first.Runner.Now;
        PhaseFacts phaseFacts = facts[index];
        phaseFacts.Steps += steps;
        phaseFacts.StepMicroseconds += micro * steps;
        phaseFacts.StepMicrosecondsMost = Math.Max(phaseFacts.StepMicrosecondsMost, micro);
        bool standing = now.Speed == 0 && !now.Jump && into > Sequence.Fade;
        probe?.Measure(phaseFacts, dt * steps, standing);
    }

    /// <summary>The side gust: across the run (+X), swinging about its mean.</summary>
    private static Vector3 Gust(float seconds) =>
        new(WindSpeed * (1 + GustShare * MathF.Sin(MathF.Tau * seconds / GustPeriod)), 0, 0);

    /// <summary>Sample the run's clip at its time; for a moment after the clip changes, the last one fades out over it, carrying on from where it was.</summary>
    private void Animate(Character character)
    {
        (Phase phase, _, float into) = character.Runner.Now;
        float seconds = character.Runner.Seconds;
        (string clip, float time) = Sequence.ClipAt(phase, into, seconds);
        if (clip != character.Clips.Clip)
        {
            character.Clips = (clip, seconds - (Sequence.Loops(clip) ? 0 : time), character.Clips.Clip, character.Clips.Started);
        }
        var layers = new List<AnimationClipLayer>();
        float weight = Sequence.Weight(phase, into);
        if (weight < 1)
        {
            // Easing out of the rest pose: no playback, the clip laid over it.
            engine.Animation.SetPlayback(new(character.Instance, AnimationPlaybackKind.Stop, "", AnimationLoopMode.Repeat, 1, 1, false, 0, false, 0));
            layers.Add(new(clips[clip].Index, Normalized(clip, time), true, weight, false, 0, 0, 0));
            engine.Animation.SetLayers(new(character.Instance, layers.ToArray(), ReadOnlyMemory<AnimationJointWeight>.Empty));
            return;
        }
        engine.Animation.SetPlayback(new(character.Instance, AnimationPlaybackKind.Sample, clip, AnimationLoopMode.Repeat, 1, 1, false, 0, false, Normalized(clip, time)));
        float since = seconds - character.Clips.Started;
        if (character.Clips.Before != "" && since < Sequence.Fade)
        {
            string before = character.Clips.Before;
            float beforeTime = Sequence.Loops(before) ? seconds : seconds - character.Clips.BeforeStarted;
            layers.Add(new(clips[before].Index, Normalized(before, beforeTime), true, 1 - since / Sequence.Fade, false, 0, 0, 0));
        }
        engine.Animation.SetLayers(new(character.Instance, layers.ToArray(), ReadOnlyMemory<AnimationJointWeight>.Empty));
    }

    /// <summary>A clip's time as a fraction of it: loops wrap, the jump's clips hold their end.</summary>
    private float Normalized(string clip, float seconds)
    {
        float duration = clips[clip].Duration;
        if (duration <= 0) return 0;
        return Sequence.Loops(clip) ? seconds / duration % 1 : MathF.Min(seconds / duration, 1);
    }

    private void Publish()
    {
        var published = new List<AppearanceFact>
        {
            Fact(FloorId, floor, new(new Vector3(0, -FloorThickness / 2, 0), Quaternion.Identity, new(2 * FloorHalfSize, FloorThickness, 2 * FloorHalfSize))),
            Fact(PillarId, pillar, new(pillarAt + new Vector3(0, PillarHeight / 2, 0), Quaternion.Identity, new(2 * PillarRadius, PillarHeight, 2 * PillarRadius))),
        };
        foreach (Character character in characters)
        {
            published.Add(Fact(character.Id, character.Body, character.Placement));
            // A soft cloak is drawn in the world: its object stays at the origin.
            if (character.CloakLook is { } look) published.Add(Fact(character.CloakId, look, Identity));
        }
        engine.Graphics.PublishSnapshot(published.ToArray());
        Follow();
    }

    private static AppearanceFact Fact(ulong id, Appearance look, Transform transform) => new(id, false, 0, transform, look, true, RenderLayer.Scene);

    /// <summary>The camera follows the first mage, from the side by default, easing after her.</summary>
    private void Follow()
    {
        if (characters.Count == 0) return;
        Runner runner = characters[0].Runner;
        Vector3 offset = view switch
        {
            "back" => -runner.Forward * CameraDistance,
            "front" => runner.Forward * CameraDistance,
            "quarter" => Vector3.Normalize(-runner.Forward + Vector3.Cross(runner.Forward, Vector3.UnitY)) * CameraDistance,
            _ => new Vector3(CameraDistance, 0, 0),
        };
        Vector3 want = runner.Root + offset + Vector3.UnitY * CameraHeight;
        cameraAt = freezeAt is null ? Vector3.Lerp(cameraAt, want, CameraFollow) : want;
        engine.CameraView.UpdateCamera(new(camera, CameraLooking(cameraAt, runner.Root + Vector3.UnitY * LookHeight)));
    }

    private static CameraDescriptor CameraLooking(Vector3 from, Vector3 at)
    {
        CameraQueries.TryLookAtPose(from, at, 0, out CameraPose pose);
        return new(pose, CameraBasisMode.Derived, default, new(CameraProjectionKind.Perspective, 45, 0, .05f, 200), CameraViewports.Full);
    }

    [DebugCommand("cloak.mode", Description = "Restart the run with the cloak's cape bones simulated (bones) or as a soft sheet (soft).")]
    public string SetMode(string name)
    {
        mode = name switch
        {
            "bones" => CloakMode.Bones,
            "soft" => CloakMode.Soft,
            _ => throw new ArgumentException("cloak.mode takes bones or soft"),
        };
        Reset();
        return Inspect();
    }

    [DebugCommand("cloak.count", Description = "Restart the run with this many mages side by side (the first is measured).")]
    public string SetCount(int mages)
    {
        count = Math.Clamp(mages, 1, 64);
        Reset();
        return Inspect();
    }

    [DebugCommand("cloak.tune", Description = "Restart the run with one cloak setting changed (plate, swing, twist, twistDamping, angularDamping, thickness, extraSubsteps, edge, bend, substeps, drag).")]
    public string Tune(string name, string value)
    {
        tuning = tuning.With(name, value);
        Reset();
        return Inspect();
    }

    [DebugCommand("cloak.probe", Description = "Restart the run measuring the first mage's cloak (on) or only the step's time (off, for cost runs).")]
    public string Probing(string on)
    {
        probing = on != "off";
        Reset();
        return Inspect();
    }

    [DebugCommand("cloak.restart", Description = "Run the sequence again from the start.")]
    public string Rerun() { Reset(); return Inspect(); }

    [DebugCommand("cloak.freeze", Description = "Hold the run at this many seconds in (the same moment in every mode); off runs on.")]
    public string Freeze(string seconds)
    {
        freezeAt = seconds == "off" ? null : float.Parse(seconds, CultureInfo.InvariantCulture);
        return Inspect();
    }

    [DebugCommand("cloak.camera", Description = "Watch the first mage from the side, back, front or quarter.")]
    public string Camera(string where)
    {
        view = where is "back" or "front" or "quarter" ? where : "side";
        return Inspect();
    }

    [DebugCommand("cloak.inspect", Description = "Mode, mages, the run's time and phase, and what the cloak holds.")]
    public string Inspect()
    {
        Character first = characters[0];
        (Phase phase, _, float into) = first.Runner.Now;
        string cloth = first.Cloak is { } soft
            ? $"particles={engine.Dynamics.ReadSoftBody(soft).ParticleCount}; skinVertices={cast.Cloak.SkinPositions.Length}"
            : $"capeBones={first.CapeBones.Sum(column => column.Length)}; bodyBones={first.Bones.Count(bone => bone.Animated)}";
        return string.Create(CultureInfo.InvariantCulture,
            $"mode={mode.ToString().ToLowerInvariant()}; mages={count}; probe={probing}; tuning=[{tuning}]; attached={first.Attached}; " +
            $"t={first.Runner.Seconds:F2}/{Sequence.Duration:F1}; phase={phase.Name}+{into:F2}; frozen={(freezeAt is { } f ? f.ToString("F2", CultureInfo.InvariantCulture) : "off")}; " +
            $"done={first.Runner.Seconds >= Sequence.Duration}; {cloth}; run={runs}");
    }

    [DebugCommand("cloak.pose", Description = "The chest joint as the last evaluated pose and as sampled for this call, and where the first mage stands.")]
    public string Pose()
    {
        Character first = characters[0];
        uint chest = first.Joint("mixamorig:Spine2");
        AnimationJointPoseResult latest = engine.Animation.ReadJointPose(first.Instance);
        JointPose[] sampled = engine.Animation.SampleJointPose(new(first.Instance, "", 0, "", 0, 0, false, first.Placement, true, true)).ToArray();
        Transform a = latest.Joints.Span[(int)chest].World, b = sampled[chest].World;
        Transform body = engine.Dynamics.ReadRagdoll(first.Ragdoll!).Bones.Span[3];
        return string.Create(CultureInfo.InvariantCulture,
            $"t={first.Runner.Seconds:F3}; latest={a.Translation} at {latest.Seconds:F3}; sampled={b.Translation}; body={body.Translation}; placement={first.Placement.Translation}");
    }

    [DebugCommand("cloak.report", Description = "Each phase so far: mean and worst Dynamics step (us), collar gap, penetration, stretch (mm or ratio), hem jitter (m/s2), hem speed most and at the end (m/s).")]
    public string Report()
    {
        var text = new StringBuilder(Inspect()).Append('\n');
        double total = 0;
        int steps = 0;
        for (int index = 0; index < facts.Length; index++)
        {
            PhaseFacts phaseFacts = facts[index];
            if (phaseFacts.Steps == 0) continue;
            total += phaseFacts.StepMicroseconds;
            steps += phaseFacts.Steps;
            string stretch = mode == CloakMode.Soft
                ? $"stretch={phaseFacts.StretchMost:F3}"
                : $"linkGap={1000 * phaseFacts.StretchMost:F1}mm";
            text.Append(string.Create(CultureInfo.InvariantCulture,
                $"{Sequence.Phases[index].Name}: steps={phaseFacts.Steps}; stepUs={phaseFacts.MeanStepMicroseconds:F0}/{phaseFacts.StepMicrosecondsMost:F0}; " +
                $"collarGap={1000 * phaseFacts.CollarGapMost:F1}mm; penetration={1000 * phaseFacts.PenetrationMost:F1}mm; {stretch}; " +
                $"jitter={phaseFacts.Jitter:F2}m/s2; hemSpeed={phaseFacts.HemSpeedMost:F2}/{phaseFacts.HemSpeedEnd:F2}; contacts={phaseFacts.ContactsMost}\n"));
        }
        text.Append(string.Create(CultureInfo.InvariantCulture, $"all: steps={steps}; meanStepUs={(steps == 0 ? 0 : total / steps):F0}"));
        return text.ToString();
    }

    public void RegisterDebugCommands(IDebugCommandModuleRegistrar registrar) => registrar.Register(this);

    public void Pause() { }
    public void Resume() { }
    public void Restart() { }
    public void Shutdown() { }

    public void Dispose()
    {
        engine.Graphics.PublishSnapshot([]);
        foreach (Character character in characters) character.Dispose();
        foreach (IDisposable owner in new IDisposable[] { floor, pillar, sun, sky, camera, world, spatial })
            owner.Dispose();
    }
}
