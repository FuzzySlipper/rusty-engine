using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using Rusty.Engine;
using Rusty.Engine.Testing;

// Runs in a tool executable (RustyEngineToolHost, not a test project): the
// Engine's Random draws the svc-rng golden values a running product draws,
// and the SDK's managed KeyedRng draws what the Engine's DrawKeyed draws, on
// any thread.
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

    // svc-rng's keyed_v1_vectors_are_stable.
    private static readonly (ulong Seed, string Scope, byte[] Key, long Minimum, long Maximum, long Expected)[] KeyedGolden =
    [
        (7, "world/event", "alpha"u8.ToArray(), -9, 9, 1),
        (7, "world/event", "alpha\0tail"u8.ToArray(), -9, 9, -9),
        (0x0123_4567_89ab_cdef, "procedural/placement", [0x10, 0x20, 0x30, 0x40, 0x50], -4_000_000_000, 4_000_000_000, 1_625_475_134),
        (99, "full-range", "all-bits"u8.ToArray(), long.MinValue, long.MaxValue, 6_514_469_275_967_966_983),
    ];

    private const int CrossCheckDraws = 4_000;
    private const int CrossCheckSeed = 11177;
    private const int LongKeyEvery = 97;
    private const int LongKeyChars = 70_000;

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

        CheckManagedKeyedGolden();
        CheckManagedKeyedAgainstNative(host);
    }

    private static void CheckManagedKeyedGolden()
    {
        foreach ((ulong seed, string scope, byte[] key, long minimum, long maximum, long expected) in KeyedGolden)
        {
            long bytes = KeyedRng.Draw(seed, Encoding.UTF8.GetBytes(scope), key, minimum, maximum);
            long chars = KeyedRng.Draw(seed, scope, Encoding.UTF8.GetString(key), minimum, maximum);
            Require(bytes == expected && chars == expected,
                $"KeyedRng drew {bytes} (bytes) and {chars} (string) for ({seed}, {scope}), not the svc-rng golden {expected}.");
        }
        Require(Refuses(() => KeyedRng.Draw(1, "scope", "key", 2, 1)), "KeyedRng took a minimum above its maximum.");
    }

    // Randomized seeds, scopes, keys and ranges: the Engine draws each on the
    // update, then worker threads draw them all again through KeyedRng.
    private static void CheckManagedKeyedAgainstNative(EngineTestHost host)
    {
        var random = new Random(CrossCheckSeed);
        string[] fixedScopes = ["", "world/event", "procedural/placement", "世界/イベント", "🌲/trees", "lone\uD800surrogate", new string('s', 5_000)];
        var cases = new List<(ulong Seed, string Scope, string Key, long Minimum, long Maximum)>(CrossCheckDraws);
        for (int index = 0; index < CrossCheckDraws; index++)
        {
            ulong seed = index switch { 0 => 0, 1 => ulong.MaxValue, _ => (ulong)random.NextInt64() ^ ((ulong)random.Next() << 63) };
            string scope = random.Next(4) == 0 ? RandomText(random, random.Next(0, 40)) : fixedScopes[random.Next(fixedScopes.Length)];
            int keyLength = index % LongKeyEvery == 0 ? LongKeyChars + random.Next(1_000) : random.Next(0, 48);
            (long minimum, long maximum) = RandomRange(random, index);
            cases.Add((seed, scope, RandomText(random, keyLength), minimum, maximum));
        }

        long[] native = host.Call(engine => cases
            .Select(draw => engine.Random.DrawKeyed(new KeyedRngRequest(draw.Seed, draw.Scope, draw.Key, draw.Minimum, draw.Maximum)).Value)
            .ToArray());
        host.Call(engine => Require(Refuses(() => engine.Random.DrawKeyed(new KeyedRngRequest(1, "scope", "key", 2, 1))),
            "The Engine's DrawKeyed took a minimum above its maximum."));

        var managed = new long[cases.Count];
        var managedBytes = new long[cases.Count];
        Parallel.For(0, cases.Count, new ParallelOptions { MaxDegreeOfParallelism = 8 }, index =>
        {
            (ulong seed, string scope, string key, long minimum, long maximum) = cases[index];
            managed[index] = KeyedRng.Draw(seed, scope, key, minimum, maximum);
            managedBytes[index] = KeyedRng.Draw(seed, Encoding.UTF8.GetBytes(scope), Encoding.UTF8.GetBytes(key), minimum, maximum);
        });

        for (int index = 0; index < cases.Count; index++)
        {
            (ulong seed, string scope, string key, long minimum, long maximum) = cases[index];
            Require(managed[index] == native[index] && managedBytes[index] == native[index],
                $"Draw {index} (seed {seed}, scope length {scope.Length}, key length {key.Length}, {minimum}..={maximum}): "
                + $"the Engine drew {native[index]}, KeyedRng {managed[index]} (string) and {managedBytes[index]} (bytes).");
            Require(managed[index] >= minimum && managed[index] <= maximum, $"Draw {index} left its range.");
        }
    }

    // Full-span, narrow and arbitrary ranges, and spans just past 2^63 or
    // 2^64/3 (an attempt rejected about half or a third of the time) or just
    // under 2^64 (almost never).
    private static (long Minimum, long Maximum) RandomRange(Random random, int index)
    {
        switch (index % 8)
        {
            case 0: return (long.MinValue, long.MaxValue);
            case 1:
            {
                long anchor = random.NextInt64(long.MinValue, long.MaxValue);
                long width = random.Next(0, 4);
                return anchor > long.MaxValue - width ? (anchor - width, anchor) : (anchor, anchor + width);
            }
            case 2:
            {
                long low = random.Next(-20, 20);
                return (low, low + random.Next(0, 20));
            }
            case 3:
            {
                long a = random.NextInt64(long.MinValue, long.MaxValue), b = random.NextInt64(long.MinValue, long.MaxValue);
                return (Math.Min(a, b), Math.Max(a, b));
            }
            case 4:
            {
                long low = long.MinValue + random.Next(0, 1_000);
                return (low, low + long.MaxValue + 1 + random.Next(0, 1_000));
            }
            case 5: return (long.MinValue + random.Next(0, 3), long.MaxValue - random.Next(0, 3));
            case 6:
            {
                long low = random.NextInt64(long.MinValue, long.MinValue / 3);
                return (low, low + ((long.MaxValue / 3) * 2) + random.Next(1, 1_000));
            }
            default: return (0, random.NextInt64(0, long.MaxValue));
        }
    }

    private static string RandomText(Random random, int length)
    {
        const string Pool = "abcxyz019/_-. ÆøéßЖж中文字世界イベント\0\t";
        const int AstralEvery = 16;
        var text = new StringBuilder(length + 2);
        while (text.Length < length)
        {
            if (random.Next(AstralEvery) == 0) text.Append(char.ConvertFromUtf32(0x1F300 + random.Next(0x200)));
            else text.Append(Pool[random.Next(Pool.Length)]);
        }
        return text.ToString();
    }

    private static bool Refuses(Action draw)
    {
        try { draw(); }
        catch (ArgumentOutOfRangeException) { return true; }
        catch (EngineCallException) { return true; }
        return false;
    }

    private static void Require(bool condition, string message)
    {
        if (!condition) throw new InvalidOperationException(message);
    }
}
