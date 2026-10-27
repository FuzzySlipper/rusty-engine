using System;
using System.Numerics;
using System.Text.Json;
using Rusty.Engine;
using Rusty.Engine.Debugging;

/// <summary>The SDK playtest commands a product registers, run under the
/// product's loader (CoreCLR, or NativeAOT when the pair check publishes it).</summary>
internal sealed class PlaytestDebugChecks(IEngineContext engine) : IDebugCommandModuleSource
{
    private const string ExpectedHelp = """{"observe":"playtest.observe","action":"playtest.action","look":"playtest.look","actions":["jump"],"inputPath":"physical-keyboard-or-pointer","lookAdvancesTime":false}""";
    private const string ExpectedAction = """{"id":"jump","key":"Space","durationMs":16.5,"hold":false,"available":true,"reason":null,"equipment":null,"heldKeys":["KeyW"]}""";
    private const int ProbeSamples = 32;
    private const float ProbeHeight = 1.8f, ProbeStepHeight = 0.35f;

    public PlaytestDebugModule Playtest { get; } = new(
        () => DebugCommandResult.Success("{}"),
        id => new PlaytestAction(id, "Space", 16.5, false, HeldKeys: ["KeyW"]),
        ["jump"],
        (_, _) => DebugCommandResult.Success("{}"));

    public void RegisterDebugCommands(IDebugCommandModuleRegistrar registrar)
    {
        registrar.Register(Playtest);
        registrar.Register(new SpatialInspectionDebugModule(
            (_, _, _) => DebugCommandResult.Success("{}"), Probe, (_, _, _) => DebugCommandResult.Success("{}")));
    }

    public void Run()
    {
        Require(Playtest.Help().Message == ExpectedHelp, "playtest.help output changed.");
        Require(Playtest.Action("jump").Message == ExpectedAction, "playtest.action output changed.");
        using JsonDocument probe = JsonDocument.Parse(Probe(2).Message);
        JsonElement samples = probe.RootElement.GetProperty("samples");
        Require(samples.GetArrayLength() == ProbeSamples
            && samples[0].GetProperty("label").GetString() == "heading-0-ankle"
            && !samples[0].GetProperty("hit").GetBoolean()
            && samples[0].GetProperty("distance").ValueKind == JsonValueKind.Null,
            "spatial.probe output changed.");
    }

    private DebugCommandResult Probe(double distance)
    {
        using SpatialSession session = engine.Spatial.CreateSession(new(1, 16, VoxelSurfaceMode.GreedyCubes));
        return DebugCommandResult.Success(PlaytestTraversal.Probe(engine.Spatial, session, Vector3.Zero,
            ProbeHeight, ProbeStepHeight, (float)distance, ReadOnlyMemory<SpatialEntityCollider>.Empty));
    }

    private static void Require(bool condition, string message)
    {
        if (!condition) throw new InvalidOperationException(message);
    }
}
