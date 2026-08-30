using System;
using Rusty.Engine.Debugging;

namespace DebugCommandCatalogFixture;

public enum FixtureMode
{
    Alpha,
    Beta,
}

public sealed class FixtureModule : IDebugCommandModule
{
    private int _total;

    [DebugCommand("fixture.add", Description = "Adds a typed amount to the fixture total.")]
    public string Add(int amount, FixtureMode mode, Guid key)
    {
        _total += amount;
        return $"{mode}:{key:N}:{_total}";
    }

    [DebugCommand("fixture.count", Description = "Reads the repeated invocation count.")]
    public DebugCommandResult Count() => DebugCommandResult.Success(_total.ToString());

    [DebugCommand("fixture.reset")]
    public void Reset(string reason)
    {
        _total = reason.Length;
    }
}

public sealed class FixtureDiagnosticsModule : IDebugCommandModule
{
    [DebugCommand("fixture.ping", Description = "Exercises a second live module registration.")]
    public string Ping(int sequence) => $"pong:{sequence}";
}
