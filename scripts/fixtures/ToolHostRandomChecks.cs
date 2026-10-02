using System;
using Rusty.Engine;
using Rusty.Engine.Testing;

// Runs in a tool executable (RustyEngineToolHost, not a test project): the
// Engine's Random draws the svc-rng golden values a running product draws.
internal static class ToolHostRandomChecks
{
    private static readonly ulong[] ScopedGolden =
    [
        13_308_457_129_731_648_163,
        2_232_188_147_979_576_733,
        7_550_675_432_941_175_364,
        6_414_595_564_799_098_339,
        6_771_639_045_484_950_311,
        14_345_072_847_349_944_858,
    ];

    public static void Run()
    {
        using var host = EngineTestHost.Create();
        host.Call(engine =>
        {
            using Rng scoped = engine.Random.CreateScoped(new ScopedRngCreateRequest(42, "level/tunnel"));
            foreach (ulong expected in ScopedGolden)
                Require(engine.Random.NextU64(scoped).Value == expected, "A scoped draw differed from the svc-rng golden sequence.");

            KeyedRngReceipt keyed = engine.Random.DrawKeyed(new KeyedRngRequest(7, "world/event", "alpha", -9, 9));
            Require(keyed.Value == 1, $"A keyed draw returned {keyed.Value}, not the svc-rng golden 1.");

            using Rng parent = engine.Random.CreateScoped(new ScopedRngCreateRequest(42, "level/tunnel"));
            using Rng first = engine.Random.ForkScoped(new ScopedRngForkRequest(parent, "child"));
            using Rng again = engine.Random.CreateScoped(new ScopedRngCreateRequest(42, "level/tunnel"));
            using Rng second = engine.Random.ForkScoped(new ScopedRngForkRequest(again, "child"));
            Require(engine.Random.NextU64(first) == engine.Random.NextU64(second), "Equal forks drew different values.");
        });
    }

    private static void Require(bool condition, string message)
    {
        if (!condition) throw new InvalidOperationException(message);
    }
}
