using System;
using System.Collections.Generic;
using System.Text.Json;

namespace Rusty.Engine.Debugging;

/// <summary>Product-resolved action timing and ordinary physical controls for inspection tools. Key accepts keyboard codes or Primary/Secondary/Auxiliary pointer buttons.</summary>
public sealed record PlaytestAction(string Id, string Key, double DurationMs, bool Hold,
    bool Available = true, string? Reason = null, string? Equipment = null, IReadOnlyList<string>? HeldKeys = null);

/// <summary>Small product adapter over the existing generated debug catalog. Queries never act.</summary>
public sealed class PlaytestDebugModule(
    Func<DebugCommandResult> observe,
    Func<string, PlaytestAction> action,
    IReadOnlyList<string> actions,
    Func<double, double, DebugCommandResult> look) : IDebugCommandModule
{
    private static readonly JsonSerializerOptions Json = new() { PropertyNamingPolicy = JsonNamingPolicy.CamelCase };

    [DebugCommand("playtest.help", Description = "Read available observations and action timing queries. Queries never advance time.")]
    public DebugCommandResult Help() => DebugCommandResult.Success(JsonSerializer.Serialize(new
    {
        observe = "playtest.observe", action = "playtest.action", look = "playtest.look", actions,
        inputPath = "physical-keyboard-or-pointer", lookAdvancesTime = false,
    }, Json));

    [DebugCommand("playtest.observe", Description = "Read current product gameplay facts without advancing simulation.")]
    public DebugCommandResult Observe() => observe();

    [DebugCommand("playtest.action", Description = "Resolve an action's live duration, current equipment and ordinary control; does not execute.")]
    public DebugCommandResult Action(string id) => DebugCommandResult.Success(JsonSerializer.Serialize(action(id), Json));

    [DebugCommand("playtest.look", Description = "Apply relative yaw/pitch in degrees through product look rules without advancing simulation.")]
    public DebugCommandResult Look(double yawDegrees, double pitchDegrees) => look(yawDegrees, pitchDegrees);
}
